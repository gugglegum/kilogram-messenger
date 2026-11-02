//! Actual decoder/Operations paths with synthetic public observation records.
use super::*;
fn id(n: u8) -> String {
    state::hex(&[n; 32])
}
fn counts(e: u64, d: u64, a: u64, b: u64) -> Value {
    json!({"encrypt":e,"decrypt":d,"attempts":a,"aborted":b,"ambiguous":0,"accounts":0,"outbound":0,"inbound":0})
}
fn record(kind: &str, seq: u64, facts: Value) -> String {
    format!(
        "s54_observation {}",
        json!({"kind":kind,"pid":7,"owner":id(1),"incarnation":id(2),"sequence":seq,"facts":facts})
    )
}
fn feed(
    d: &mut Decoder,
    o: &mut oracle::Operations,
    kind: &str,
    seq: u64,
    facts: Value,
) -> Result<()> {
    d.line(
        &record(kind, seq, facts),
        o,
        &mut Default::default(),
        &mut 0,
    )
}
fn start() -> Result<(Decoder, oracle::Operations)> {
    let mut d = Decoder::new(7, [1; 32], [id(3), id(4)], 0);
    let mut o = Default::default();
    feed(
        &mut d,
        &mut o,
        "start",
        0,
        json!({"counters":counts(0,0,0,0)}),
    )?;
    Ok((d, o))
}
fn commit(d: &mut Decoder, o: &mut oracle::Operations) -> Result<()> {
    feed(
        d,
        o,
        "native_commit",
        1,
        json!({"context":id(3),"operation":id(8),"generation":5,"counters":counts(0,1,1,0)}),
    )
}
fn end(point: (u64, u64), generation: u64, op: u8) -> source::End {
    source::End {
        point,
        endpoint: intervals::Endpoint {
            source: [1; 32],
            lanes: [
                intervals::Lane {
                    context: [3; 32],
                    slot: 0,
                    role: 0,
                    next: [point.0 + 1, point.1 + 1],
                },
                intervals::Lane {
                    context: [4; 32],
                    slot: 1,
                    role: 0,
                    next: [1, 1],
                },
            ],
        },
        generation,
        operation: [op; 32],
    }
}
#[test]
fn s54_p11a1_normal_final_is_read_only_preserves_actual_adjustment_and_packet_metadata()
-> Result<()> {
    let (mut d, mut o) = start()?;
    commit(&mut d, &mut o)?;
    let key = d.incarnation()?;
    let packet = d.packet()?;
    feed(
        &mut d,
        &mut o,
        "final",
        1,
        json!({"resolved":true,"counters":counts(0,1,2,1)}),
    )?;
    let final_record = d.normal_final(&o, key, 0)?;
    ensure!(
        final_record.counts.attempts == 2
            && final_record.counts.aborted == 1
            && d.counts.attempts == 1
            && d.last_packet == packet
            && d.normal_identity()? == key
            && d.incarnation().is_err(),
        "actual final adjustment retained independently of last native commit"
    );
    d.close_normal(&o, key, (10, 20), &end((10, 21), 5, 8), 0)?;
    d.close_normal(&o, key, (10, 20), &end((10, 21), 6, 9), 0)?;
    let a = o.summary()?;
    let b = o.summary()?;
    ensure!(
        a.counts == b.counts
            && a.counts == final_record.counts
            && a.streams.len() == 1
            && a.streams[0].key == key
            && a.streams[0].sequence == 1
            && a.streams[0].counts == final_record.counts
            && a.operations == vec![([1; 32], [8; 32])]
            && a.generations == vec![([1; 32], 5)]
            && o.complete()? == (0, 1),
        "read-only complete actual counts and identities"
    );
    ensure!(
        o.finish(key, final_record.counts).is_err(),
        "second finish rejected, read-only normal validation did not finish again"
    );
    Ok(())
}
#[test]
fn s54_p11a1_normal_final_rejects_missing_duplicate_unresolved_regressing_or_gapped_records()
-> Result<()> {
    let (mut d, mut o) = start()?;
    let key = d.incarnation()?;
    ensure!(
        d.normal_final(&o, key, 0).is_err() && o.summary().is_err(),
        "missing final/open ledger"
    );
    ensure!(
        feed(
            &mut d,
            &mut o,
            "final",
            0,
            json!({"resolved":true,"counters":counts(0,1,1,0)})
        )
        .is_err(),
        "normal source counts cannot synthesize a missing native commit"
    );
    ensure!(
        feed(
            &mut d,
            &mut o,
            "final",
            0,
            json!({"resolved":false,"counters":counts(0,0,0,0)})
        )
        .is_err(),
        "unresolved final"
    );
    commit(&mut d, &mut o)?;
    ensure!(
        feed(
            &mut d,
            &mut o,
            "final",
            2,
            json!({"resolved":true,"counters":counts(0,1,1,0)})
        )
        .is_err(),
        "sequence gap"
    );
    ensure!(
        feed(
            &mut d,
            &mut o,
            "final",
            1,
            json!({"resolved":true,"counters":counts(0,0,0,0)})
        )
        .is_err(),
        "counter regression"
    );
    feed(
        &mut d,
        &mut o,
        "final",
        1,
        json!({"resolved":true,"counters":counts(0,1,1,0)}),
    )?;
    ensure!(
        feed(
            &mut d,
            &mut o,
            "final",
            1,
            json!({"resolved":true,"counters":counts(0,1,1,0)})
        )
        .is_err()
            && commit(&mut d, &mut o).is_err(),
        "duplicate final/post-final native observation"
    );
    Ok(())
}
#[test]
fn s54_p11a1_normal_eof_identity_source_and_root_order_cannot_be_guessed() -> Result<()> {
    let (mut d, mut o) = start()?;
    commit(&mut d, &mut o)?;
    let key = d.incarnation()?;
    feed(
        &mut d,
        &mut o,
        "final",
        1,
        json!({"resolved":true,"counters":counts(0,1,1,0)}),
    )?;
    ensure!(d.normal_final(&o, key, 1).is_err(), "unconsumed EOF bytes");
    d.fragment.push(b'x');
    ensure!(d.normal_final(&o, key, 0).is_err(), "truncated line");
    d.fragment.clear();
    let mut foreign = key;
    foreign.pid = 8;
    ensure!(
        d.normal_final(&o, foreign, 0).is_err(),
        "foreign final owner key"
    );
    for stopped in [
        end((10, 20), 6, 9),
        end((11, 21), 6, 9),
        end((10, 21), 4, 8),
        end((10, 21), 5, 9),
    ] {
        ensure!(
            d.close_normal(&o, key, (10, 20), &stopped, 0).is_err(),
            "source gap/extra effect/stale root/conflicting operation"
        );
    }
    d.close_normal(&o, key, (10, 20), &end((10, 21), 9, 9), 0)?;
    Ok(())
}
#[test]
fn s54_p11a1_zero_work_restart_and_closed_summary_do_not_invent_operations() -> Result<()> {
    let (mut d, mut o) = start()?;
    let key = d.incarnation()?;
    feed(
        &mut d,
        &mut o,
        "final",
        0,
        json!({"resolved":true,"counters":counts(0,0,0,0)}),
    )?;
    d.close_normal(&o, key, (100, 100), &end((100, 100), 200, 9), 0)?;
    let actual = o.summary()?;
    ensure!(
        actual.counts == Default::default()
            && actual.operations.is_empty()
            && actual.generations.is_empty()
            && actual.streams.len() == 1
            && actual.streams[0].sequence == 0,
        "actual closed zero work, no fabricated native operation"
    );
    let next = oracle::Incarnation {
        owner: key.owner,
        pid: 8,
        nonce: [9; 32],
    };
    o.start(next)?;
    ensure!(
        o.summary().is_err(),
        "second open incarnation prevents complete summary"
    );
    o.finish(next, Default::default())?;
    ensure!(o.summary()?.streams.len() == 2, "both closed keys retained");
    Ok(())
}
