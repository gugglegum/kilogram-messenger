//! Public synthetic metadata only. No owner/relay/process/private-state fixture.
use super::*;
fn id(n: u8) -> String {
    state::hex(&[n; 32])
}
fn holds() -> [[Value; 2]; 2] {
    std::array::from_fn(|owner| {
        [
            json!({"disable_ack":true,"suspend_prefix_requests":true,"hold_ordinals":[],"trace_rpc":true}),
            json!({"context":id(1),"lane":owner,"phase":"catch_up","suspend_live":true,"suspend_prefix":true,"hold_ordinals":[if owner==0{385}else{640}],"hold_source64":false,"source64_after":null,"hold_ack":false,"hold_ack_from":null,"pause_after_normal_page":null,"retry_live":null}),
        ]
    })
}
fn row(owner: usize, context: usize) -> Value {
    json!({"context":id(1+context as u8),"slot":context,"role":owner,"d":id(3),"e":id(4),"sid":"original","q":id(5),"cuts":if context==0{json!([25,25])}else{json!([33,33])},"controls":{"p":id(6),"a":id(7),"k":id(8),"committed":id(9)},"current_consent":id(10),"current_consent2":id(10),"native":{"keys":[id(11)]},"mode":"Active","policy_error":null,"handoff_policy_error":null,"control_debt":false,"head_pending":null,"next":if context==1{json!([33,33])}else if owner==0{json!([641,640])}else{json!([641,641])},"counts":{"sources":if context==1{66}else{1281+owner},"queued":0,"pending":0,"debt":if context==0{owner}else{0},"receipts":if context==1{json!([32,32])}else{json!([640,639])}}})
}
#[test]
fn s54_p10b_relay_unique_url_and_original_deadline_predicates() -> Result<()> {
    relay_owner::unique(false)?;
    ensure!(relay_owner::unique(true).is_err(), "no second owned relay");
    for good in [
        "http://127.0.0.1:1/",
        "http://127.0.0.1:40123/",
        "http://127.0.0.1:65535/",
    ] {
        ensure!(relay_owner::url(good)? == good, "exact canonical owned URL");
    }
    for bad in [
        "http://127.0.0.1:0/",
        "http://localhost:40123/",
        "https://127.0.0.1:40123/",
        "http://127.0.0.1:40123",
        "http://127.0.0.1:40123/?x=1",
        "http://127.0.0.1:040123/",
        "http://user@127.0.0.1:40123/",
        "http://127.0.0.2:40123/",
    ] {
        ensure!(relay_owner::url(bad).is_err(), "wrong owned relay {bad}");
    }
    let now = Instant::now();
    ensure!(
        relay_owner::ready_until(now, now + Duration::from_secs(4))?
            == now + Duration::from_secs(4)
            && relay_owner::ready_until(now, now + Duration::from_secs(100))?
                == now + Duration::from_secs(15)
            && relay_owner::ready_until(now, now).is_err(),
        "bounded readiness never renews whole deadline"
    );
    Ok(())
}
#[test]
fn s54_p10b_route_requires_both_stopped_and_reconciled_outcomes() -> Result<()> {
    control::reconciled_pair([false; 2], [false; 2])?;
    for running in [[true, false], [false, true], [true, true]] {
        ensure!(
            control::reconciled_pair(running, [false; 2]).is_err(),
            "live owners forbid profile edit"
        );
    }
    ensure!(
        control::reconciled_pair([false; 2], [true, false]).is_err(),
        "wait without reconciliation is insufficient"
    );
    let keys = [
        oracle::Incarnation {
            owner: [1; 32],
            pid: 11,
            nonce: [2; 32],
        },
        oracle::Incarnation {
            owner: [3; 32],
            pid: 22,
            nonce: [4; 32],
        },
    ];
    let outcomes = [
        json!({"owner":0,"pid":11,"waited":true,"success":false,"counter_outcome":"source/counter resolved"}),
        json!({"owner":1,"pid":22,"waited":true,"success":false,"counter_outcome":"source/counter resolved"}),
    ];
    proofs::closed(&outcomes, keys)?;
    for mutation in 0..3 {
        let mut bad = outcomes.clone();
        match mutation {
            0 => bad[0]["pid"] = 12.into(),
            1 => bad[1]["waited"] = false.into(),
            _ => bad[1]["counter_outcome"] = "pending stopped-source reconciliation".into(),
        }
        ensure!(
            proofs::closed(&bad, keys).is_err(),
            "actual closed outcome {mutation}"
        );
    }
    Ok(())
}
#[test]
fn s54_p10b_profile_overlay_rejects_any_nonroute_mutation() -> Result<()> {
    let before = json!({"account":id(1),"device":id(2),"state_dir":"owned","enabled_contexts":[id(3),id(4)],"route_policy":"direct-only","route":{"policy":"direct-only"},"future":{"retain":true}});
    let url = "http://127.0.0.1:40123/";
    let actual = proofs::overlay(&before, url)?;
    proofs::profile(&before, &actual, url)?;
    for mutation in 0..5 {
        let mut bad = actual.clone();
        match mutation {
            0 => bad["account"] = id(99).into(),
            1 => bad["state_dir"] = "foreign".into(),
            2 => bad["enabled_contexts"] = json!([id(3)]),
            3 => bad["future"]["retain"] = false.into(),
            _ => bad["route"]["bind"] = "127.0.0.1:0".into(),
        }
        ensure!(
            proofs::profile(&before, &bad, url).is_err(),
            "profile drift {mutation}"
        );
    }
    Ok(())
}
#[test]
fn s54_p10b_controls_keep_a_prefix_and_b_hold_until_ordered_release() -> Result<()> {
    let base = holds();
    proofs::held(&base)?;
    let ack = proofs::schedules(&base, proofs::Stage::Ack);
    proofs::controls(&ack, &base, proofs::Stage::Ack)?;
    ensure!(
        ack[0][0]["disable_ack"] == false
            && ack[0][0]["suspend_prefix_requests"] == true
            && ack[0][1]["suspend_prefix"] == true
            && ack[1][1]["hold_ordinals"] == json!([640])
            && ack[1][1]["suspend_live"] == true,
        "A ACK enabled while prefix and original B640 remain held"
    );
    let release = proofs::schedules(&base, proofs::Stage::Released);
    ensure!(
        release[1][1]["hold_ordinals"] == json!([])
            && release[1][1]["suspend_live"] == false
            && release[0][1]["suspend_prefix"] == true
            && release[1][0]["disable_ack"] == true,
        "atomic B live/640 release, no A prefix trigger"
    );
    let final_ = proofs::schedules(&base, proofs::Stage::Final);
    proofs::controls(&final_, &base, proofs::Stage::Final)?;
    for mutation in 0..4 {
        let mut bad = final_.clone();
        match mutation {
            0 => bad[0][0]["suspend_prefix_requests"] = true.into(),
            1 => bad[1][1]["suspend_live"] = true.into(),
            2 => bad[1][0]["hold_ordinals"] = json!([640]),
            _ => bad[1][1]["hold_ack_from"] = 385.into(),
        }
        ensure!(
            proofs::controls(&bad, &base, proofs::Stage::Final).is_err(),
            "stale suppression/selector {mutation}"
        );
    }
    Ok(())
}
#[test]
fn s54_p10b_source_frontiers_allow_only_original_b640_then_require_complete_receipts() -> Result<()>
{
    let before = row(0, 0);
    proofs::same(&before, &before, 0, 0, proofs::Stage::Held)?;
    let mut pending = before.clone();
    pending["counts"]["pending"] = 1.into();
    proofs::same(&pending, &before, 0, 0, proofs::Stage::Released)?;
    let mut done = before.clone();
    done["next"] = json!([641, 641]);
    done["counts"]["sources"] = 1282.into();
    done["counts"]["receipts"] = json!([640, 640]);
    proofs::same(&done, &before, 0, 0, proofs::Stage::Final)?;
    ensure!(
        proofs::same(&done, &before, 0, 0, proofs::Stage::Held).is_err(),
        "premature original B640 release"
    );
    for mutation in 0..5 {
        let mut bad = done.clone();
        match mutation {
            0 => bad["next"] = json!([641, 642]),
            1 => bad["counts"]["sources"] = 1283.into(),
            2 => bad["counts"]["receipts"] = json!([640, 639]),
            3 => bad["q"] = id(99).into(),
            _ => bad["native"]["keys"] = json!([id(99)]),
        }
        ensure!(
            proofs::same(&bad, &before, 0, 0, proofs::Stage::Final).is_err(),
            "source/identity/receipt contradiction {mutation}"
        );
    }
    let c2 = row(0, 1);
    let mut changed = c2.clone();
    changed["next"] = json!([34, 33]);
    ensure!(
        proofs::same(&changed, &c2, 0, 1, proofs::Stage::Released).is_err(),
        "C2 cannot grow Text"
    );
    Ok(())
}
#[test]
fn s54_p10b_original_request_receipt_status_rejects_new_event_queue_or_fake_delivery() -> Result<()>
{
    let event = [64; 32];
    let before = json!([[[7], [state::hex(&event)], 0]]);
    let after = json!([[[7], [state::hex(&event)], 1]]);
    ensure!(
        !proofs::request(&before, event)? && proofs::request(&after, event)?,
        "actual stored receipt status"
    );
    proofs::acknowledged(&before, &after, event)?;
    for mutation in 0..5 {
        let mut bad = after.clone();
        match mutation {
            0 => bad[0][1] = json!([id(99)]),
            1 => bad[0][0] = json!([8]),
            2 => bad[0][2] = 2.into(),
            3 => bad[0][1] = json!([]),
            _ => bad = Value::Null,
        }
        ensure!(
            proofs::acknowledged(&before, &bad, event).is_err(),
            "original request mismatch {mutation}"
        );
    }
    Ok(())
}
#[test]
fn s54_p10b_actual_work_and_packet_scope_never_substitute_expected_one_decrypt() -> Result<()> {
    let one = oracle::Counts {
        decrypt: 1,
        attempts: 1,
        ..Default::default()
    };
    proofs::work(one, 0, proofs::Stage::Final, true)?;
    ensure!(
        proofs::work(one, 0, proofs::Stage::Held, false).is_err()
            && proofs::work(one, 1, proofs::Stage::Released, false).is_err()
            && proofs::work(oracle::Counts::default(), 0, proofs::Stage::Final, true).is_err(),
        "premature/other owner/missing actual work"
    );
    let packet = observations::PacketObservation {
        context: 0,
        counts: one,
        operation: [8; 32],
        generation: 1,
        sequence: 1,
    };
    proofs::packet(Some(packet), 0, true)?;
    let mut wrong = packet;
    wrong.context = 1;
    ensure!(
        proofs::packet(Some(wrong), 0, true).is_err() && proofs::packet(None, 0, true).is_err(),
        "C2 packet or expected-only count cannot satisfy C1 proof"
    );
    for mutation in 0..5 {
        let mut bad = one;
        match mutation {
            0 => bad.accounts = 1,
            1 => bad.encrypt = 1,
            2 => bad.aborted = 1,
            3 => {
                bad.decrypt = 2;
                bad.attempts = 2
            }
            _ => bad.ambiguous = 1,
        }
        ensure!(
            proofs::work(bad, 0, proofs::Stage::Released, false).is_err(),
            "unexpected actual work {mutation}"
        );
    }
    Ok(())
}
#[test]
fn s54_p10b_history_maps_reject_body_id_request_format_or_order_substitution() -> Result<()> {
    let rows = vec![
        json!({"format":2,"id":id(1),"role":0,"ordinal":1,"request":"original-a","text":"original A"}),
        json!({"format":2,"id":id(2),"role":1,"ordinal":1,"request":"original-b","text":"original B"}),
    ];
    let expected = source::history_map(&rows)?;
    proofs::history(&rows, &expected, [2, 2])?;
    for mutation in 0..5 {
        let mut bad = rows.clone();
        match mutation {
            0 => bad[1]["id"] = id(99).into(),
            1 => bad[1]["request"] = "foreign".into(),
            2 => bad[1]["text"] = "foreign".into(),
            3 => bad[1]["format"] = 1.into(),
            _ => bad[1]["ordinal"] = 2.into(),
        }
        ensure!(
            proofs::history(&bad, &expected, [2, 2]).is_err(),
            "history contradiction {mutation}"
        );
    }
    Ok(())
}
#[test]
fn s54_p10b_decoder_packet_metadata_is_from_accepted_real_context_commit_records() -> Result<()> {
    let mut d = observations::Decoder::new(73, [1; 32], [id(3), id(4)], 0);
    ensure!(d.packet().is_err(), "no packet before stream start");
    let mut ops = Default::default();
    let mut pages = Default::default();
    let counts = |d| json!({"encrypt":0,"decrypt":d,"attempts":d,"aborted":0,"ambiguous":0,"accounts":0,"outbound":0,"inbound":0});
    let event = |kind: &str, sequence: u64, facts: Value| {
        format!(
            "s54_observation {}",
            json!({"kind":kind,"pid":73,"owner":id(1),"incarnation":id(2),"sequence":sequence,"facts":facts})
        )
    };
    d.line(
        &event("start", 0, json!({"counters":counts(0)})),
        &mut ops,
        &mut pages,
        &mut 0,
    )?;
    ensure!(
        d.packet()?.is_none(),
        "no invented packet in new incarnation"
    );
    d.line(
        &event(
            "native_commit",
            1,
            json!({"context":id(4),"operation":id(8),"generation":1,"counters":counts(1)}),
        ),
        &mut ops,
        &mut pages,
        &mut 0,
    )?;
    let actual = d.packet()?.context("actual accepted commit")?;
    ensure!(
        actual.context == 1
            && actual.counts.decrypt == 1
            && actual.sequence == 1
            && proofs::packet(Some(actual), 0, true).is_err(),
        "actual C2 commit cannot be relabelled C1"
    );
    Ok(())
}
