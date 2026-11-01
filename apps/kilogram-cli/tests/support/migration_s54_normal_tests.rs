//! Pure predicates used by the real original-handle lifecycle. No OS fixture.
use super::*;
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
