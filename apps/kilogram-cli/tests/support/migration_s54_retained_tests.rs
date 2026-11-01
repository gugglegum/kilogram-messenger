//! Immutable proof predicates only: no real database or Native session.
use super::*;
fn id(n: u8) -> old::Id {
    [n; 32]
}
fn original() -> Immutable {
    Immutable {
        state: "synthetic-owned-only".into(),
        scope: Scope {
            source: id(1),
            migration: id(2),
            context: id(3),
            q: id(4),
            d: id(5),
            e: id(6),
            sid: "synthetic".into(),
            identity: id(7),
        },
        anchor: id(8),
        sources: vec![Source {
            ordinal: 385,
            request: [9; 16],
            id: id(10),
            previous: id(8),
            source_hash: id(11),
            ciphertext_hash: id(12),
            text_hash: id(13),
        }],
    }
}
#[test]
fn s54_p9b_retained_ordinal_rejects_request_id_cipher_source_body_and_chain_mismatch() -> Result<()>
{
    let original = original();
    let row = original.sources[0].clone();
    original.same_row(0, &row)?;
    for mutation in 0..7 {
        let mut bad = row.clone();
        match mutation {
            0 => bad.ordinal += 1,
            1 => bad.request[0] ^= 1,
            2 => bad.id = id(99),
            3 => bad.previous = id(99),
            4 => bad.source_hash = id(99),
            5 => bad.ciphertext_hash = id(99),
            _ => bad.text_hash = id(99),
        }
        ensure!(
            original.same_row(0, &bad).is_err(),
            "immutable mismatch {mutation}"
        );
    }
    ensure!(
        original.same_row(1, &row).is_err(),
        "missing ordinal rejects"
    );
    Ok(())
}
#[test]
fn s54_p9b_terminal_metadata_excludes_mutable_native_and_c2_zero_state() -> Result<()> {
    let original = original();
    let mut snapshot = Snapshot {
        state: original.state.clone(),
        scope: original.scope.clone(),
        anchor: original.anchor,
        sources: original.sources.clone(),
        native_content: id(14),
        native_seal: id(15),
        c2_identity: id(16),
    };
    let before = Immutable::from_snapshot(&snapshot);
    snapshot.native_content = id(90);
    snapshot.native_seal = id(91);
    snapshot.c2_identity = id(92);
    let terminal = Immutable::from_snapshot(&snapshot);
    before.same_row(0, &terminal.sources[0])?;
    ensure!(
        before.scope == terminal.scope
            && before.state == terminal.state
            && before.anchor == terminal.anchor,
        "only immutable provenance carried to terminal reader"
    );
    Ok(())
}
#[test]
fn s54_p9b_saved_receipt_digest_rejects_changed_bytes_encoding_and_batch() -> Result<()> {
    let mut d = observations::Decoder::new(73, id(1), [state::hex(&id(3)), state::hex(&id(4))], 1);
    let mut operations = Default::default();
    let mut pages = Default::default();
    let envelope = |kind: &str, f: Value| {
        format!(
            "s54_observation {}",
            json!({"kind":kind,"pid":73,"owner":state::hex(&id(1)),"incarnation":state::hex(&id(2)),"sequence":0,"facts":f})
        )
    };
    d.line(&envelope("start",json!({"counters":{"encrypt":0,"decrypt":0,"attempts":0,"aborted":0,"ambiguous":0,"accounts":0,"outbound":0,"inbound":0}})),&mut operations,&mut pages,&mut 0)?;
    d.line(&envelope("validated_head",json!({"context":state::hex(&id(3)),"binding":state::hex(&id(5)),"lease_generation":6,"pre_q":false,"received":640})),&mut operations,&mut pages,&mut 0)?;
    d.arm_ack(0, id(5), 6, Some(385))?;
    let actual = vec![v2::TaggedItem {
        format: 2,
        raw: vec![10, 20, 30],
    }];
    let batch = old::encode(
        &old::a([old::a([old::u(2), old::b(&actual[0].raw)])]),
        16 * 1024,
    )?;
    let facts = json!({"context":state::hex(&id(3)),"binding":state::hex(&id(5)),"lease_generation":6,"request":state::hex(&[7;16]),"query_reply":false,"receipt_count":1,"saved_receipts_digest":state::hex(&digest(&batch)),"lane":0,"receipts":[[2,2,0,385,state::hex(&id(85)),state::hex(&digest(&actual[0].raw))]],"hold_ack_from":385});
    d.line(&envelope("ack", facts), &mut operations, &mut pages, &mut 0)?;
    let ack = d
        .paused_ack()
        .context("parsed synthetic receipt metadata")?;
    saved_ack(ack, &actual)?;
    let mut changed = actual.clone();
    changed[0].raw.push(40);
    ensure!(
        saved_ack(ack, &changed).is_err(),
        "changed saved receipt bytes"
    );
    changed = actual.clone();
    changed[0].format = 1;
    ensure!(
        saved_ack(ack, &changed).is_err(),
        "changed receipt encoding"
    );
    let mut wrong_batch = ack.clone();
    wrong_batch.digest = id(99);
    ensure!(
        saved_ack(&wrong_batch, &actual).is_err() && saved_ack(ack, &[]).is_err(),
        "batch digest/cardinality mismatch"
    );
    Ok(())
}
