//! Pure predicates used by the real original-handle lifecycle. No OS fixture.
use super::*;
#[test]
fn s54_p11a5_last_actual_normal_pair_excludes_empty_stale_killed_and_foreign_outcomes() -> Result<()>
{
    let pair = std::array::from_fn(|o| Closure {
        key: oracle::Incarnation {
            owner: [o as u8 + 1; 32],
            pid: o as u32 + 7,
            nonce: [o as u8 + 3; 32],
        },
        sequence: 0,
        counts: Default::default(),
        eof: 123,
        generation: 5,
        operation: [6; 32],
        point: [672; 2],
        outcome: o,
    });
    let rows=pair.iter().enumerate().map(|(o,c)|json!({"owner":o,"pid":c.key.pid,"code":0,"success":true,"waited":true,"counter_outcome":"normal exit0/final/EOF/source resolved","normal_stop":{"source":state::hex(&c.key.owner),"incarnation":state::hex(&c.key.nonce),"acknowledged":true,"waited":true},"normal_closure":{"source":state::hex(&c.key.owner),"pid":c.key.pid,"incarnation":state::hex(&c.key.nonce),"eof":c.eof,"sequence":c.sequence,"root_generation":c.generation,"last_commit":state::hex(&c.operation),"source_point":c.point}})).collect::<Vec<_>>();
    let sources = pair.each_ref().map(|c| c.key.owner);
    ensure!(
        closed_pair(Some(&pair), &rows, sources)? == pair,
        "actual typed normal closure gate"
    );
    ensure!(
        closed_pair(None, &rows, sources).is_err()
            && closed_pair(Some(&pair), &[], sources).is_err()
            && closed_pair(Some(&pair), &rows, [[99; 32]; 2]).is_err(),
        "no empty/foreign success shortcut"
    );
    for field in [
        "owner",
        "pid",
        "code",
        "success",
        "waited",
        "counter_outcome",
        "normal_stop",
        "normal_closure",
    ] {
        let mut bad = rows.clone();
        bad[0][field] = Value::Null;
        ensure!(
            closed_pair(Some(&pair), &bad, sources).is_err(),
            "resolved actual outcome field {field}"
        );
    }
    let mut later = rows.clone();
    later.push(json!({"owner":0,"counter_outcome":"source/counter resolved"}));
    ensure!(
        closed_pair(Some(&pair), &later, sources).is_err(),
        "earlier normal closure cannot survive later killed cut"
    );
    let mut foreign = pair.clone();
    foreign[0].key.nonce = [99; 32];
    ensure!(
        closed_pair(Some(&foreign), &rows, sources).is_err(),
        "actual PID/source/nonce binding"
    );
    Ok(())
}
fn key(pid: u32) -> oracle::Incarnation {
    oracle::Incarnation {
        owner: [1; 32],
        pid,
        nonce: [pid as u8; 32],
    }
}
#[test]
fn s54_p11a1_expected_stop_preserves_key_and_original_minimum_deadline() -> Result<()> {
    let now = Instant::now();
    let stop = Stop::new(key(7), now, now + Duration::from_secs(100))?;
    ensure!(stop.until == now + Duration::from_secs(15), "normal15 cap");
    stop.check(key(7), 7, now)?;
    ensure!(
        stop.check(key(8), 7, now).is_err()
            && stop.check(key(7), 8, now).is_err()
            && stop.check(key(7), 7, stop.until).is_err(),
        "foreign source/nonce/PID/expired request"
    );
    ensure!(
        Stop::new(key(7), now, now).is_err()
            && Stop::new(key(7), now, now + Duration::from_secs(2))?.until
                == now + Duration::from_secs(2),
        "whole never extended"
    );
    Ok(())
}
#[test]
fn s54_p11a1_only_requested_exit0_is_expected_and_acknowledgement_is_not_wait() -> Result<()> {
    let now = Instant::now();
    let mut stop = Stop::new(key(7), now, now + Duration::from_secs(30))?;
    observed_exit(None, None)?;
    ensure!(
        observed_exit(None, Some(Some(0))).is_err()
            && stop.wait(Some(0), now).is_err()
            && stop.complete().is_err(),
        "unexpected exit or missing ack fails"
    );
    ensure!(
        stop.acknowledge("status=failed").is_err()
            && stop
                .acknowledge("status=shutdown-accepted\nstatus=failed")
                .is_err(),
        "actual exact acknowledgement"
    );
    stop.acknowledge("status=shutdown-accepted\r\n")?;
    ensure!(
        stop.complete().is_err() && stop.acknowledge("status=shutdown-accepted").is_err(),
        "ack alone cannot close and cannot be repeated"
    );
    observed_exit(Some(&stop), Some(Some(0)))?;
    ensure!(
        observed_exit(Some(&stop), Some(Some(1))).is_err()
            && observed_exit(Some(&stop), Some(None)).is_err(),
        "requested nonzero still fails"
    );
    stop.wait(Some(0), now)?;
    stop.complete()?;
    ensure!(stop.wait(Some(0), now).is_err(), "exactly one actual wait");
    Ok(())
}
#[test]
fn s54_p11a1_either_exit_order_still_requires_both_waits_before_sources_and_reopen() -> Result<()> {
    for order in [[0, 1], [1, 0]] {
        let now = Instant::now();
        let mut stops = [
            Stop::new(key(7), now, now + Duration::from_secs(30))?,
            Stop::new(key(8), now, now + Duration::from_secs(30))?,
        ];
        for stop in &mut stops {
            stop.acknowledge("status=shutdown-accepted")?;
        }
        stops[order[0]].wait(Some(0), now)?;
        ensure!(
            ready_to_read([false, false], [Some(&stops[0]), Some(&stops[1])]).is_err(),
            "acknowledged second child not yet waited"
        );
        stops[order[1]].wait(Some(0), now)?;
        ready_to_read([false, false], [Some(&stops[0]), Some(&stops[1])])?;
        ensure!(
            ready_to_read([false, true], [Some(&stops[0]), Some(&stops[1])]).is_err()
                && ready_to_read([false, false], [None, Some(&stops[1])]).is_err(),
            "both original retained waited records required"
        );
    }
    ensure!(
        reopen(true, false).is_err() && reopen(false, true).is_err(),
        "unresolved stop or finished relay excludes restart"
    );
    reopen(false, false)?;
    Ok(())
}
