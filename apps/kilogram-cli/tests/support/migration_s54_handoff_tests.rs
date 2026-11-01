//! Synthetic public rows test the same controller predicates. No live fixtures.
use super::*;
fn id(n: u8) -> String {
    state::hex(&[n; 32])
}
fn row() -> Value {
    json!({"context":id(1),"role":0,"slot":0,"d":id(2),"e":id(3),"sid":"original","native":{"keys":[id(4)]},"mode":"L1","next":[33,33],"counts":{"sources":66,"queued":0,"pending":0,"debt":0},"head_pending":null,"q":null,"controls":{"p":null,"a":null,"k":null,"committed":null},"current_consent":id(5),"current_consent2":null,"handoff_policy_error":"Consent2 absent","policy_error":null,"upgrade":"NoLocalIntent","lease_current":true,"lease_family":1,"lease_generation":7,"publication_ready":true,"publication_error":null,"existing_origin_ready":true,"origin_publication_error":null,"control_debt":false})
}
fn authorized() -> Value {
    let mut r = row();
    r["current_consent2"] = id(6).into();
    r["current_consent"] = id(6).into();
    r["handoff_policy_error"] = Value::Null;
    r
}
fn active() -> Value {
    let mut r = authorized();
    r["mode"] = "Active".into();
    r["cuts"] = json!([33, 33]);
    r["q"] = id(7).into();
    r["controls"] = json!({"p":id(8),"a":id(9),"k":id(10),"committed":id(11)});
    r["lease_family"] = 2.into();
    r
}
fn map() -> Result<BTreeMap<oracle::Request, Value>> {
    let mut out = BTreeMap::new();
    for context in 0..2 {
        for role in 0..2 {
            for n in 1..=if context == 0 { 640 } else { 32 } {
                out.insert(
                    oracle::Request::new(context, role, n)?,
                    json!({"original":n}),
                );
            }
        }
    }
    Ok(out)
}
#[test]
fn s54_p10a_entry_and_request_digest_reject_missing_owner_phase_text_growth_or_changed_reply()
-> Result<()> {
    proofs::owned(10, [true; 2])?;
    ensure!(
        proofs::owned(11, [true; 2]).is_err() && proofs::owned(10, [true, false]).is_err(),
        "same owned phase input"
    );
    let mut requests = map()?;
    let digest = proofs::request_digest(&requests)?;
    let key = oracle::Request::new(1, 1, 32)?;
    requests.remove(&key);
    ensure!(
        proofs::request_digest(&requests).is_err(),
        "actual request cardinality"
    );
    requests.insert(key, json!({"foreign":true}));
    ensure!(
        proofs::request_digest(&requests)? != digest,
        "same-cardinality actual reply substitution"
    );
    Ok(())
}
#[test]
fn s54_p10a_intent_ids_bind_actual_prior_controls_and_text_inventory() -> Result<()> {
    let prior = json!([{"owner":1,"request":state::hex(&[71;16])},{"owner":0,"request":state::hex(&[72;16])}]);
    let ids = map()?
        .keys()
        .map(|r| source::parse_request_id(&source::request_id(*r)))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        proofs::intents(&prior, &ids)? == [[0xa1; 16], [0xa2; 16]],
        "actual disjoint C2 intent IDs"
    );
    let mut wrong = prior.clone();
    wrong[0]["request"] = state::hex(&[0xa1; 16]).into();
    ensure!(
        proofs::intents(&wrong, &ids).is_err(),
        "prior control collision"
    );
    let mut wrong = ids.clone();
    wrong[0] = [0xa2; 16];
    ensure!(
        proofs::intents(&prior, &wrong).is_err(),
        "actual Text request collision"
    );
    ensure!(
        proofs::intents(&Value::Null, &ids).is_err(),
        "missing actual prior control witness"
    );
    Ok(())
}
#[test]
fn s54_p10a_c2_identity_consent_sources_and_no_intent_are_checked_through_import() -> Result<()> {
    let before = row();
    proofs::c2(&before, &before, None, false)?;
    proofs::c2(&authorized(), &before, Some([6; 32]), false)?;
    for mutation in 0..9 {
        let mut bad = authorized();
        match mutation {
            0 => bad["role"] = 1.into(),
            1 => bad["d"] = id(99).into(),
            2 => bad["native"]["keys"] = json!([id(99)]),
            3 => bad["current_consent2"] = id(99).into(),
            4 => bad["next"] = json!([34, 33]),
            5 => bad["counts"]["sources"] = 67.into(),
            6 => bad["counts"]["debt"] = 1.into(),
            7 => bad["controls"]["p"] = id(99).into(),
            _ => bad["upgrade"] = "LocalIntentPending".into(),
        }
        ensure!(
            proofs::c2(&bad, &before, Some([6; 32]), false).is_err(),
            "before-intent C2 mismatch {mutation}"
        );
    }
    Ok(())
}
#[test]
fn s54_p10a_transient_lease_loss_waits_but_stable_requires_current_family2_head_direct()
-> Result<()> {
    let mut r = authorized();
    let mut route = json!({"slot":0,"policy":"direct-only","selected_path":"direct"});
    ensure!(
        proofs::admission(&r, &route, None, false)? && !proofs::admission(&r, &route, None, true)?,
        "family1 is valid transient but not stable completion"
    );
    route["selected_path"] = Value::Null;
    ensure!(
        !proofs::admission(&r, &route, None, false)? && !proofs::admission(&r, &route, None, true)?,
        "unselected path during lease replacement waits, cannot complete"
    );
    r["lease_current"] = false.into();
    r["lease_generation"] = Value::Null;
    route["selected_path"] = Value::Null;
    ensure!(
        !proofs::admission(&r, &route, None, false)? && !proofs::admission(&r, &route, None, true)?,
        "replacement gap waits"
    );
    r = active();
    route["selected_path"] = "direct".into();
    ensure!(
        proofs::admission(&r, &route, Some(([20; 32], 7, 32)), true)?,
        "real matching family2 Head"
    );
    ensure!(
        !proofs::admission(&r, &route, Some(([20; 32], 8, 32)), true)?,
        "stale Head generation"
    );
    for mutation in 0..3 {
        let mut bad = route.clone();
        match mutation {
            0 => bad["policy"] = "relay-only".into(),
            1 => bad["selected_path"] = "relay".into(),
            _ => bad["slot"] = 1.into(),
        }
        ensure!(
            proofs::admission(&r, &bad, None, false).is_err(),
            "premature route/mapping {mutation}"
        );
    }
    Ok(())
}
#[test]
fn s54_p10a_selection_is_anchored_to_actual_ceremony_and_retained_control_observations()
-> Result<()> {
    let mut selected = proofs::Selection::default();
    let actual = active();
    selected.observe(&actual, [6; 32])?;
    ensure!(
        selected.active(&actual, [6; 32])?,
        "actual retained selection"
    );
    for mutation in 0..5 {
        let mut bad = actual.clone();
        match mutation {
            0 => bad["current_consent2"] = id(99).into(),
            1 => bad["q"] = id(99).into(),
            2 => bad["controls"]["p"] = id(99).into(),
            3 => bad["cuts"] = json!([34, 33]),
            _ => bad["controls"]["committed"] = Value::Null,
        }
        ensure!(
            selected.active(&bad, [6; 32]).is_err(),
            "even matching peer rows cannot replace actual selection {mutation}"
        );
    }
    let mut lag = authorized();
    lag["mode"] = "Prepared".into();
    selected.observe(&lag, [6; 32])?;
    ensure!(
        !selected.active(&lag, [6; 32])?,
        "follower can lag, not complete"
    );
    Ok(())
}
#[test]
fn s54_p10a_c1_actual_controls_and_original_b640_frontiers_cannot_change() -> Result<()> {
    let mut before = active();
    before["next"] = json!([641, 640]);
    before["counts"]["debt"] = 0.into();
    proofs::c1(&before, &before, 0)?;
    for mutation in 0..4 {
        let mut bad = before.clone();
        match mutation {
            0 => bad["q"] = id(99).into(),
            1 => bad["controls"]["a"] = id(99).into(),
            2 => bad["next"] = json!([641, 641]),
            _ => bad["current_consent2"] = id(99).into(),
        }
        ensure!(
            proofs::c1(&bad, &before, 0).is_err(),
            "original C1 actual witness {mutation}"
        );
    }
    Ok(())
}
#[test]
fn s54_p10a_zero_conversion_work_rejects_every_packet_constructor_outcome_and_regression()
-> Result<()> {
    let zero = oracle::Counts::default();
    proofs::zero(zero, zero)?;
    for mutation in 0..8 {
        let mut bad = zero;
        match mutation {
            0 => bad.encrypt = 1,
            1 => bad.decrypt = 1,
            2 => bad.attempts = 1,
            3 => bad.aborted = 1,
            4 => bad.ambiguous = 1,
            5 => bad.accounts = 1,
            6 => bad.outbound = 1,
            _ => bad.inbound = 1,
        }
        ensure!(
            proofs::zero(bad, zero).is_err() && proofs::zero(zero, bad).is_err(),
            "actual conversion counter {mutation}"
        );
    }
    Ok(())
}
#[test]
fn s54_p10a_published_hold_equality_rejects_ack_prefix_live_selector_and_file_changes() -> Result<()>
{
    let controls = std::array::from_fn(|o| {
        [
            json!({"disable_ack":true,"suspend_prefix_requests":true,"hold_ordinals":[]}),
            json!({"phase":"catch_up","suspend_live":true,"suspend_prefix":true,"hold_ordinals":[if o==0{385}else{640}],"hold_source64":false,"source64_after":null,"hold_ack":false,"hold_ack_from":null,"pause_after_normal_page":null,"retry_live":null}),
        ]
    });
    proofs::held(&controls)?;
    proofs::controls(&controls, &controls)?;
    for mutation in 0..5 {
        let mut bad = controls.clone();
        match mutation {
            0 => bad[0][0]["disable_ack"] = false.into(),
            1 => bad[0][1]["suspend_prefix"] = false.into(),
            2 => bad[1][1]["suspend_live"] = false.into(),
            3 => bad[1][1]["hold_ordinals"] = json!([]),
            _ => bad[0][1]["source64_after"] = 385.into(),
        }
        ensure!(
            proofs::held(&bad).is_err() && proofs::controls(&bad, &controls).is_err(),
            "retained actual holds {mutation}"
        );
    }
    Ok(())
}
#[test]
fn s54_p10a_live_history_preserves_ids_requests_body_format_and_lane_order() -> Result<()> {
    let rows = vec![
        json!({"format":1,"id":id(1),"role":0,"ordinal":1,"request":"original-a","text":"original A"}),
        json!({"format":1,"id":id(2),"role":1,"ordinal":1,"request":"original-b","text":"original B"}),
    ];
    let expected = source::history_map(&rows)?;
    proofs::history(&rows, &expected, [2, 2])?;
    for mutation in 0..5 {
        let mut bad = rows.clone();
        match mutation {
            0 => bad[0]["id"] = id(99).into(),
            1 => bad[0]["request"] = "foreign".into(),
            2 => bad[0]["text"] = "foreign".into(),
            3 => bad[0]["format"] = 2.into(),
            _ => bad[0]["ordinal"] = 2.into(),
        }
        ensure!(
            proofs::history(&bad, &expected, [2, 2]).is_err(),
            "same actual history {mutation}"
        );
    }
    Ok(())
}
#[test]
fn s54_p10a_ready_resolves_c2_to_slot_zero_for_the_real_import_builder() -> Result<()> {
    let c1 = id(1);
    let c2 = id(2);
    let ready = json!({"contexts":[{"context":c2},{"context":c1}]});
    let slot = oracle::ready_slot(&ready, &c2)?;
    let node = Node {
        branch: Branch {
            base: "synthetic".into(),
            state: "synthetic/state".into(),
            account: id(3),
            device: id(4),
            config: "synthetic/config".into(),
            context: c2.clone(),
            claim: id(5),
        },
        profile: "synthetic/profile".into(),
        ipc: "synthetic/ipc".into(),
        locators: "synthetic/locators".into(),
        indexed: true,
        slot,
    };
    let peer = node.view(&c2, 1);
    let args = borrowed::locator_arguments(&node, &peer)?;
    let at = args
        .iter()
        .position(|a| a == "--slot")
        .context("real import slot")?;
    ensure!(
        slot == 0
            && args[at + 1] == "0"
            && args.last() == Some(&text(&peer.locators.join(format!("{c2}.locator2")))),
        "actual non-default Ready mapping reaches command builder"
    );
    ensure!(
        oracle::ready_slot(&json!({"contexts":[{"context":c2},{"context":c2}]}), &c2).is_err(),
        "ambiguous Ready mapping"
    );
    Ok(())
}
#[test]
fn s54_p10a_live_commit_counts_are_actual_decoder_records_not_expected_totals() -> Result<()> {
    let mut d = observations::Decoder::new(73, [1; 32], [id(3), id(4)], 0);
    ensure!(
        d.observed_counts().is_err(),
        "no live stream before actual start"
    );
    let mut operations = Default::default();
    let mut pages = Default::default();
    let counters = |encrypt| json!({"encrypt":encrypt,"decrypt":0,"attempts":encrypt,"aborted":0,"ambiguous":0,"accounts":0,"outbound":0,"inbound":0});
    let event = |kind: &str, sequence: u64, facts: Value| {
        format!(
            "s54_observation {}",
            json!({"kind":kind,"pid":73,"owner":id(1),"incarnation":id(2),"sequence":sequence,"facts":facts})
        )
    };
    d.line(
        &event("start", 0, json!({"counters":counters(0)})),
        &mut operations,
        &mut pages,
        &mut 0,
    )?;
    let baseline = d.observed_counts()?;
    d.line(
        &event(
            "native_commit",
            1,
            json!({"context":id(3),"operation":id(8),"generation":1,"counters":counters(1)}),
        ),
        &mut operations,
        &mut pages,
        &mut 0,
    )?;
    let actual = d.observed_counts()?;
    ensure!(
        actual.encrypt == 1 && actual.attempts == 1 && proofs::zero(actual, baseline).is_err(),
        "real accepted typed record cannot be erased by expected zero"
    );
    Ok(())
}
