//! Synthetic public metadata only. No child, network, fixture or enrolled source.
use super::*;
fn id(n: u64) -> old::Id {
    let mut id = [0; 32];
    id[..8].copy_from_slice(&n.to_be_bytes());
    id
}
fn ids() -> Vec<(u64, old::Id)> {
    (385..=640).map(|n| (n, id(n))).collect()
}
fn pending() -> Vec<observations::Pending> {
    (386..=449)
        .map(|n| observations::Pending {
            context: 0,
            role: 0,
            ordinal: n,
            event: id(n),
            generation: 1,
            operation: id(800 + n),
        })
        .collect()
}
fn row(end: u64) -> Value {
    json!({"next":[end,641],"counts":{"debt":1,"queued":0,"pending":0},"head_pending":null})
}
pub(super) fn ack(binding: u8, request: u8, line: u8) -> Result<observations::AckObservation> {
    let mut d = observations::Decoder::new(73, id(1), [state::hex(&id(3)), state::hex(&id(4))], 1);
    let mut operations = oracle::Operations::default();
    let mut pages = oracle::Pages::default();
    let mut send = |kind: &str, f: Value| {
        d.line(&format!("s54_observation {}",json!({"kind":kind,"pid":73,"owner":state::hex(&id(1)),"incarnation":state::hex(&id(2)),"sequence":0,"facts":f})),&mut operations,&mut pages,&mut 0)
    };
    send(
        "start",
        json!({"counters":{"encrypt":0,"decrypt":0,"attempts":0,"aborted":0,"ambiguous":0,"accounts":0,"outbound":0,"inbound":0}}),
    )?;
    for _ in 0..line {
        send(
            "validated_head",
            json!({"context":state::hex(&id(3)),"binding":state::hex(&[binding;32]),"lease_generation":6,"pre_q":false,"received":640}),
        )?;
    }
    d.arm_ack(0, [binding; 32], 6, Some(385))?;
    let facts = json!({"context":state::hex(&id(3)),"binding":state::hex(&[binding;32]),"lease_generation":6,"request":state::hex(&[request;16]),"query_reply":false,"receipt_count":2,"saved_receipts_digest":state::hex(&id(900)),"lane":0,"receipts":[[1,1,0,24,state::hex(&id(24)),state::hex(&id(924))],[2,2,0,385,state::hex(&id(385)),state::hex(&id(1285))]],"hold_ack_from":385});
    d.line(&format!("s54_observation {}",json!({"kind":"ack","pid":73,"owner":state::hex(&id(1)),"incarnation":state::hex(&id(2)),"sequence":0,"facts":facts})),&mut operations,&mut pages,&mut 0)?;
    d.paused_ack().cloned().context("synthetic parsed ACK")
}
fn history(owner: usize) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    for n in 1..=640u16 {
        for role in 0..2u8 {
            if owner == 0 && role == 1 && n == 640 {
                continue;
            }
            let request = oracle::Request::new(0, role, n)?;
            rows.push(json!({"format":if n<25{1}else{2},"id":state::hex(&id(u64::from(n)+u64::from(role)*1000)),"role":role,"ordinal":n,"request":source::request_id(request),"text":request.body()}));
        }
    }
    Ok(rows)
}
#[test]
fn s54_p9b_owned_entry_rejects_phase_process_request_and_pending_substitution() -> Result<()> {
    proofs::prerequisites(9, [true; 2], 1344, &pending(), &ids())?;
    ensure!(
        proofs::prerequisites(10, [true; 2], 1344, &pending(), &ids()).is_err()
            && proofs::prerequisites(9, [true, false], 1344, &pending(), &ids()).is_err()
            && proofs::prerequisites(9, [true; 2], 1343, &pending(), &ids()).is_err(),
        "same continuation prerequisites"
    );
    for mutation in 0..4 {
        let mut p = pending();
        match mutation {
            0 => p[0].event = id(999),
            1 => p[0].context = 1,
            2 => p[0].ordinal = 385,
            _ => {
                p.pop();
            }
        }
        ensure!(
            proofs::prerequisites(9, [true; 2], 1344, &p, &ids()).is_err(),
            "original pending mismatch {mutation}"
        );
    }
    Ok(())
}
#[test]
fn s54_p9b_barrier_requires_actual_end_drain_and_rejects_overshoot() -> Result<()> {
    ensure!(proofs::progress(&row(482), 482)?, "accepted actual end");
    let mut pending = row(482);
    pending["counts"]["pending"] = 1.into();
    ensure!(
        !proofs::progress(&pending, 482)?,
        "marker does not prove drain"
    );
    let mut head = row(482);
    head["head_pending"] = json!({"ordinal":482});
    ensure!(!proofs::progress(&head, 482)?, "funded head not clear");
    ensure!(
        !proofs::progress(&row(481), 482)? && proofs::progress(&row(483), 482).is_err(),
        "behind waits, overshoot fails"
    );
    Ok(())
}
#[test]
fn s54_p9b_actual_work_closes_once_and_requires_observed256() -> Result<()> {
    let mut w = proofs::Work::new([oracle::Counts::default(); 2]);
    let delta = |decrypt| oracle::Counts {
        decrypt,
        attempts: decrypt,
        ..Default::default()
    };
    w.close(1, delta(97))?;
    ensure!(
        w.close(1, delta(97)).is_err() && w.terminal().is_err(),
        "no double close/unfinished aggregate"
    );
    ensure!(
        w.reopen(1, delta(1)).is_err(),
        "new held incarnation must observe zero"
    );
    w.reopen(1, oracle::Counts::default())?;
    w.close(1, delta(159))?;
    w.close(0, delta(0))?;
    ensure!(
        w.terminal()?["actual_decrypt"] == 256,
        "aggregate actual pieces"
    );
    let mut bad = proofs::Work::new([delta(2); 2]);
    ensure!(
        bad.close(1, delta(1)).is_err() && bad.close(0, delta(3)).is_err(),
        "counter regression and A work reject"
    );
    Ok(())
}
#[test]
fn s54_p9b_histories_keep_one_original_b640_difference_and_lane_order() -> Result<()> {
    let held = json!({"id":state::hex(&id(1640))});
    let a = history(0)?;
    let b = history(1)?;
    let am = proofs::ordered_history(&a, 0, &ids(), &held)?;
    let bm = proofs::ordered_history(&b, 1, &ids(), &held)?;
    proofs::history_pair(&am, &bm, &held)?;
    ensure!(
        proofs::ordered_history(&b, 0, &ids(), &held).is_err(),
        "both1280 violates held B640"
    );
    for mutation in 0..5 {
        let mut bad = a.clone();
        match mutation {
            0 => bad.swap(0, 2),
            1 => bad[800]["id"] = state::hex(&id(9999)).into(),
            2 => bad[700]["request"] = "wrong".into(),
            3 => bad[700]["text"] = "wrong".into(),
            _ => bad[700]["format"] = 1.into(),
        }
        ensure!(
            proofs::ordered_history(&bad, 0, &ids(), &held).is_err(),
            "history mismatch {mutation}"
        );
    }
    Ok(())
}
#[test]
fn s54_p9b_ack_checks_every_historical_and_offline_source() -> Result<()> {
    let held = json!({"id":state::hex(&id(1640))});
    let mut map = proofs::ordered_history(&history(0)?, 0, &ids(), &held)?;
    let a = ack(5, 7, 1)?;
    proofs::ack_sources(&a, &map, &ids())?;
    map.remove(&state::hex(&id(24)));
    ensure!(
        proofs::ack_sources(&a, &map, &ids()).is_err(),
        "older tuple needs actual historical source"
    );
    let map = proofs::ordered_history(&history(0)?, 0, &ids(), &held)?;
    let mut bad = a.clone();
    bad.receipts[1].event = id(386);
    ensure!(
        proofs::ack_sources(&bad, &map, &ids()).is_err(),
        "offline tuple needs original P7 event"
    );
    Ok(())
}
#[test]
fn s54_p9b_replay_requires_new_binding_request_same_payload_and_owner() -> Result<()> {
    let old = ack(5, 7, 1)?;
    let new = ack(8, 9, 2)?;
    proofs::replay(&old, &new)?;
    for mutation in 0..6 {
        let mut bad = new.clone();
        match mutation {
            0 => bad.binding = old.binding,
            1 => bad.request = old.request,
            2 => bad.digest = id(999),
            3 => bad.receipts[1].digest = id(999),
            4 => bad.stamp.incarnation.pid += 1,
            _ => bad.stamp.line = old.stamp.line,
        }
        ensure!(
            proofs::replay(&old, &bad).is_err(),
            "replay mismatch {mutation}"
        );
    }
    Ok(())
}
#[test]
fn s54_p9b_held_schedule_requires_atomic_selector_clear_and_ack_suppression() -> Result<()> {
    let current = json!({"phase":"catch_up","suspend_live":true,"suspend_prefix":true,"hold_source64":false,"source64_after":null,"hold_ack":false,"hold_ack_from":null,"pause_after_normal_page":null,"retry_live":null,"hold_ordinals":[385]});
    let mut c = control::Controls {
        od1: PathBuf::new(),
        s54: PathBuf::new(),
        old: json!({"disable_ack":true,"suspend_prefix_requests":true}),
        current: current.clone(),
    };
    proofs::held_schedule(&c, 0)?;
    for mutation in 0..5 {
        c.current = current.clone();
        match mutation {
            0 => c.current["source64_after"] = 385.into(),
            1 => c.current["hold_source64"] = true.into(),
            2 => c.current["phase"] = "pressure".into(),
            3 => c.current["hold_ordinals"] = json!([]),
            _ => c.current["suspend_live"] = false.into(),
        }
        ensure!(
            proofs::held_schedule(&c, 0).is_err(),
            "invalid held schedule {mutation}"
        );
    }
    c.current = current;
    c.old["disable_ack"] = false.into();
    ensure!(
        proofs::held_schedule(&c, 0).is_err(),
        "ACK admission race rejects"
    );
    Ok(())
}
#[test]
fn s54_p9b_page_barrier_rejects_stale_process_nonce_source_and_wrong_target() -> Result<()> {
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
    let f = json!({"context":state::hex(&id(3)),"binding":state::hex(&id(5)),"lease_generation":6,"local_page":1,"head":false,"grant":null,"start":450,"end":466,"new_sources":16,"generation":10,"operation":state::hex(&id(10))});
    d.line(
        &envelope("normal_page", f.clone()),
        &mut operations,
        &mut pages,
        &mut 0,
    )?;
    d.line(&envelope("page", f), &mut operations, &mut pages, &mut 0)?;
    let p = d.paused_page().context("actual parsed page barrier")?;
    let key = d.incarnation()?;
    proofs::barrier(p, key, 1, 2, 2)?;
    for mutation in 0..3 {
        let mut old = key;
        match mutation {
            0 => old.pid += 1,
            1 => old.owner = id(99),
            _ => old.nonce = id(99),
        }
        ensure!(
            proofs::barrier(p, old, 1, 2, 2).is_err(),
            "stale barrier {mutation}"
        );
    }
    ensure!(
        proofs::barrier(p, key, 3, 2, 2).is_err() && proofs::barrier(p, key, 1, 3, 2).is_err(),
        "local or phase mismatch"
    );
    Ok(())
}
