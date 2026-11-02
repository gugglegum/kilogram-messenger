//! Synthetic operation adapter over the actual compose driver; no runtime scenario.
use super::*;
use std::{cell::RefCell, rc::Rc};
struct Trace {
    log: Rc<RefCell<Vec<Step>>>,
    fail: Option<Step>,
}
impl Adapter for Trace {
    type Output = Vec<Step>;
    fn perform(&mut self, step: Step) -> Result<()> {
        self.log.borrow_mut().push(step);
        ensure!(self.fail != Some(step), "injected actual-stage failure");
        Ok(())
    }
    fn finish(self) -> Result<Self::Output> {
        Ok(self.log.borrow().clone())
    }
}
#[test]
fn s54_p11b2_actual_driver_two_cuts_wait_reconcile_release_reopen_order() -> Result<()> {
    let log = Rc::new(RefCell::new(Vec::new()));
    let got = compose(Trace { log, fail: None })?;
    let position = |s| {
        got.iter()
            .position(|v| *v == s)
            .context("required actual driver stage")
    };
    for (a, b) in [
        (Step::RequestFirstA, Step::WaitFirst),
        (Step::RequestFirstB, Step::WaitFirst),
        (Step::WaitFirst, Step::ReconcileFirst),
        (Step::ReconcileFirst, Step::ReadFirst),
        (Step::ReadFirst, Step::Reopen),
        (Step::Reopen, Step::ObserveRestart),
        (Step::ObserveRestart, Step::RequestSecondA),
        (Step::ObserveRestart, Step::RequestSecondB),
        (Step::RequestSecondA, Step::WaitSecond),
        (Step::RequestSecondB, Step::WaitSecond),
        (Step::WaitSecond, Step::ReconcileSecond),
        (Step::ReconcileSecond, Step::ReadSecond),
        (Step::ReadSecond, Step::Compare),
        (Step::Compare, Step::Final),
        (Step::Final, Step::RelayWait),
        (Step::RelayWait, Step::Memory),
    ] {
        ensure!(position(a)? < position(b)?, "mandatory order {a:?}/{b:?}");
    }
    ensure!(
        got.len() == 18
            && got.iter().filter(|s| **s == Step::ReadFirst).count() == 1
            && got.iter().filter(|s| **s == Step::ReadSecond).count() == 1,
        "exact two cut invocations"
    );
    Ok(())
}
#[test]
fn s54_p11b2_every_failure_including_cut2_relay_and_post_final_memory_prevents_return() -> Result<()>
{
    for (i, step) in STEPS.into_iter().enumerate() {
        let log = Rc::new(RefCell::new(Vec::new()));
        ensure!(
            compose(Trace {
                log: log.clone(),
                fail: Some(step)
            })
            .is_err(),
            "no result on {step:?}"
        );
        ensure!(
            *log.borrow() == STEPS[..=i],
            "no dependent operations after failed stage"
        );
    }
    Ok(())
}
fn keys() -> [oracle::Incarnation; 2] {
    std::array::from_fn(|o| oracle::Incarnation {
        owner: [o as u8 + 1; 32],
        pid: o as u32 + 100,
        nonce: [o as u8 + 10; 32],
    })
}
#[test]
fn s54_p11b2_new_nonce_pid_reuse_and_restart_zero_never_move_p10_decrypt() -> Result<()> {
    let old = keys();
    let mut new = old;
    new[0].nonce = [20; 32];
    new[1].nonce = [21; 32];
    live::fresh(old, new)?;
    for owner in 0..2 {
        let mut bad = new;
        bad[owner].nonce = old[owner].nonce;
        ensure!(
            live::fresh(old, bad).is_err(),
            "nonce reuse refuses even same PID"
        );
        bad = new;
        bad[owner].owner = [90; 32];
        ensure!(live::fresh(old, bad).is_err(), "foreign source refuses");
    }
    let p10 = oracle::Counts {
        decrypt: 1,
        attempts: 1,
        ..Default::default()
    };
    let packet = Some(observations::PacketObservation {
        context: 0,
        counts: p10,
        operation: [30; 32],
        generation: 90,
        sequence: 1,
    });
    live::work(p10, packet, p10, packet)?;
    live::work(Default::default(), None, Default::default(), None)?;
    ensure!(
        live::work(p10, packet, Default::default(), None).is_err(),
        "P10 work cannot enter restart"
    );
    ensure!(
        live::work(Default::default(), packet, Default::default(), None).is_err(),
        "no old packet attribution"
    );
    for field in 0..8 {
        let mut c = Default::default();
        match field {
            0 => c = oracle::Counts { encrypt: 1, ..c },
            1 => c.decrypt = 1,
            2 => c.attempts = 1,
            3 => c.aborted = 1,
            4 => c.ambiguous = 1,
            5 => c.accounts = 1,
            6 => c.outbound = 1,
            _ => c.inbound = 1,
        }
        ensure!(
            live::work(c, None, Default::default(), None).is_err(),
            "every zero counter is required"
        );
    }
    Ok(())
}
#[test]
fn s54_p11b2_all_four_current_admissions_are_required() -> Result<()> {
    let row = |c| {
        json!({"slot":c,"mode":"Active","policy_error":null,"handoff_policy_error":null,
        "lease_current":true,"lease_family":2,"publication_ready":true,"publication_error":null,
        "existing_origin_ready":true,"origin_publication_error":null,"own_locator_revision":5,
        "peer_locator_revision":5,"lease_generation":9})
    };
    let facts = relay::live::Sample {
        rows: std::array::from_fn(|_| std::array::from_fn(row)),
        raw: std::array::from_fn(|_| {
            json!({"routes":[{"slot":0,"policy":"relay-only","selected_path":"relay"},
        {"slot":1,"policy":"relay-only","selected_path":"relay"}]})
        }),
    };
    let pubs = relay::locators::Publications {
        rows: std::array::from_fn(|_| {
            std::array::from_fn(|_| relay::locators::Locator {
                revision: 5,
                endpoint: [1; 32],
                raw: vec![],
            })
        }),
    };
    // Public admission metadata only. Signed same-route bytes are tested by A2.
    let heads = [[Some(([4; 32], 9, 1)); 2]; 2];
    let slots = [[0, 1]; 2];
    ensure!(
        live::admissions(&facts, &pubs, slots, heads)?,
        "four actual row gates"
    );
    for o in 0..2 {
        for c in 0..2 {
            let mut missing = heads;
            missing[o][c] = None;
            ensure!(
                !live::admissions(&facts, &pubs, slots, missing)?,
                "each of four Heads required"
            );
            missing = heads;
            missing[o][c] = Some(([4; 32], 8, 1));
            ensure!(
                !live::admissions(&facts, &pubs, slots, missing)?,
                "current generation required"
            );
            for (field, value) in [
                ("lease_family", json!(1)),
                ("lease_current", json!(false)),
                ("own_locator_revision", json!(4)),
                ("peer_locator_revision", json!(4)),
            ] {
                let mut bad = relay::live::Sample {
                    rows: facts.rows.clone(),
                    raw: facts.raw.clone(),
                };
                bad.rows[o][c][field] = value;
                ensure!(
                    !live::admissions(&bad, &pubs, slots, heads)?,
                    "each individual current admission"
                );
            }
            let mut foreign = slots;
            foreign[o][c] = 1 - slots[o][c];
            ensure!(
                live::admissions(&facts, &pubs, foreign, heads).is_err(),
                "foreign Ready slot"
            );
            let mut bad = relay::live::Sample {
                rows: facts.rows.clone(),
                raw: facts.raw.clone(),
            };
            bad.raw[o]["routes"][c]["selected_path"] = json!("direct");
            ensure!(
                live::admissions(&bad, &pubs, slots, heads).is_err(),
                "current selected path is mandatory"
            );
        }
    }
    Ok(())
}
#[test]
fn s54_p11b2_exact_outcome_lifetime_cardinality_and_actual_relay_wait() -> Result<()> {
    cardinality([7, 9], [11, 11])?;
    for after in [[10, 11], [12, 11], [11, 10], [11, 12]] {
        ensure!(
            cardinality([7, 9], after).is_err(),
            "no extra/missing cuts' normal lifetimes/outcomes"
        );
    }
    let url = "http://127.0.0.1:40123/";
    let relay = json!({"pid":100,"url":url,"stop_published":true,"waited":true,"success":true,"code":0,"forced":false});
    relay_waited(&relay, 100, url)?;
    for (field, value) in [
        ("pid", json!(101)),
        ("url", json!("http://127.0.0.1:40124/")),
        ("stop_published", json!(false)),
        ("waited", json!(false)),
        ("success", json!(false)),
        ("code", json!(1)),
        ("forced", json!(true)),
    ] {
        let mut bad = relay.clone();
        bad[field] = value;
        ensure!(
            relay_waited(&bad, 100, url).is_err(),
            "relay evidence {field} cannot substitute wait"
        );
    }
    Ok(())
}
#[test]
fn s54_p11b2_required_memory_missing_fixture_incomplete_or_failed_refuses() -> Result<()> {
    ensure!(
        memory_scope_required(&Value::Null, false).is_err(),
        "missing mandatory scope"
    );
    let entry = json!({"synthetic_fixture":false,"finished":false});
    memory_scope_required(&entry, false)?;
    let done = json!({"synthetic_fixture":false,"finished":true,"original_lifetime_coverage_complete":true,
        "coverage":{"coverage_valid":true}});
    memory_scope_required(&done, true)?;
    for (field, value) in [
        ("synthetic_fixture", json!(true)),
        ("finished", json!(false)),
        ("original_lifetime_coverage_complete", json!(false)),
        ("coverage", json!({"coverage_valid":false})),
    ] {
        let mut bad = done.clone();
        bad[field] = value;
        ensure!(
            memory_scope_required(&bad, true).is_err(),
            "required memory {field}"
        );
    }
    Ok(())
}
#[test]
fn s54_p11b2_functional_result_is_bounded_and_public_keys_are_fingerprinted() -> Result<()> {
    let completed = bounded(json!({"whole_attempt_verdict":"not_established",
        "pending":["outer admission","outer cleanup","actual harness return"],
        "live_cut_associations":[{"native":{"keys":["synthetic-public-key"]}}]}))?;
    let bytes = serde_json::to_string(completed.facts())?;
    ensure!(
        !bytes.contains("synthetic-public-key")
            && !bytes.contains("\"keys\"")
            && bytes.contains("keys_fingerprint"),
        "sanitize before any result escapes"
    );
    ensure!(
        completed.facts()["whole_attempt_verdict"] == "not_established",
        "functional completion is not whole acceptance"
    );
    ensure!(
        bounded(json!({"oversize":"x".repeat(256*1024)})).is_err(),
        "bounded result assembly failure has no Completed"
    );
    Ok(())
}
fn observe(
    d: &mut observations::Decoder,
    ops: &mut oracle::Operations,
    key: oracle::Incarnation,
    kind: &str,
    sequence: u64,
    facts: Value,
) -> Result<()> {
    d.line(
        &format!(
            "s54_observation {}",
            json!({"kind":kind,"pid":key.pid,
        "owner":state::hex(&key.owner),"incarnation":state::hex(&key.nonce),
        "sequence":sequence,"facts":facts})
        ),
        ops,
        &mut Default::default(),
        &mut 0,
    )
}
fn started(
    keys: [oracle::Incarnation; 2],
) -> Result<([observations::Decoder; 2], oracle::Operations)> {
    let mut pair = std::array::from_fn(|o| {
        observations::Decoder::new(
            keys[o].pid,
            keys[o].owner,
            [state::hex(&[3; 32]), state::hex(&[4; 32])],
            o as u8,
        )
    });
    let mut ops = Default::default();
    for o in 0..2 {
        observe(
            &mut pair[o],
            &mut ops,
            keys[o],
            "start",
            0,
            json!({"counters":counts(Default::default())}),
        )?;
    }
    Ok((pair, ops))
}
fn pending_fact(ordinal: u64, event: u8, generation: u64) -> Value {
    json!({"context":state::hex(&[3;32]),"role":1,"ordinal":ordinal,
        "event":state::hex(&[event;32]),"generation":generation,"operation":state::hex(&[6;32])})
}
fn delivered() -> Result<([observations::Decoder; 2], oracle::Operations)> {
    let key = keys();
    let (mut pair, mut ops) = started(key)?;
    observe(
        &mut pair[0],
        &mut ops,
        key[0],
        "saved_pending",
        0,
        pending_fact(640, 55, 4),
    )?;
    let before = pair[0].pending_rows.clone();
    observe(
        &mut pair[0],
        &mut ops,
        key[0],
        "native_commit",
        1,
        json!({"context":state::hex(&[3;32]),
        "operation":state::hex(&[8;32]),"generation":5,
        "counters":counts(oracle::Counts {decrypt:1,attempts:1,..Default::default()})}),
    )?;
    ensure!(
        pair[0].pending_rows == before && before.len() == 1,
        "actual Decoder materialization retains append-only pending observation history"
    );
    Ok((pair, ops))
}
#[test]
fn s54_p11b2_r1_materialized_pending_history_survives_with_zero_current_pending() -> Result<()> {
    let (pair, _) = delivered()?;
    let history = live::PendingHistory::capture([55; 32], keys(), [&pair[0], &pair[1]])?;
    for (o, d) in pair.iter().enumerate() {
        history.check(o, d, false)?;
    }
    let packet = pair[0].packet()?;
    live::work(
        pair[0].observed_counts()?,
        packet,
        oracle::Counts {
            decrypt: 1,
            attempts: 1,
            ..Default::default()
        },
        packet,
    )?;
    // Fresh live/source obligations are separate from the historical hook map.
    let row = json!({"context":state::hex(&[3;32]),"slot":0,"role":0,
        "d":state::hex(&[10;32]),"e":state::hex(&[11;32]),"sid":"original",
        "q":state::hex(&[12;32]),"cuts":[25,25],"controls":{"k":"original"},
        "current_consent":"original","current_consent2":"original","native":{"keys":["original"]},
        "mode":"Active","policy_error":null,"handoff_policy_error":null,"control_debt":false,
        "head_pending":null,"next":[641,641],"counts":{"queued":0,"sources":1282,
        "pending":0,"debt":0,"receipts":[640,640]}});
    relay::proofs::same(&row, &row, 0, 0, relay::proofs::Stage::Final)?;
    let mut pending = row.clone();
    pending["counts"]["pending"] = json!(1);
    ensure!(
        relay::proofs::same(&pending, &row, 0, 0, relay::proofs::Stage::Final).is_err(),
        "historical witness never authorizes current pending debt"
    );
    Ok(())
}
#[test]
fn s54_p11b2_r1_pending_history_rejects_missing_foreign_changed_extra_owner_context_role_key()
-> Result<()> {
    let (mut pair, mut ops) = delivered()?;
    let original = pair[0].pending_rows.clone();
    let history = live::PendingHistory::capture([55; 32], keys(), [&pair[0], &pair[1]])?;
    ensure!(
        live::PendingHistory::capture([56; 32], keys(), [&pair[0], &pair[1]]).is_err(),
        "original EventID only"
    );
    let mut foreign = keys();
    foreign[0].owner = [99; 32];
    ensure!(
        live::PendingHistory::capture([55; 32], foreign, [&pair[0], &pair[1]]).is_err(),
        "actual source binding"
    );
    foreign = keys();
    foreign[0].nonce = [99; 32];
    ensure!(
        live::PendingHistory::capture([55; 32], foreign, [&pair[0], &pair[1]]).is_err(),
        "actual nonce binding"
    );
    ensure!(
        history.check(1, &pair[0], false).is_err(),
        "executing owner cannot be swapped"
    );
    for mutation in 0..8 {
        pair[0].pending_rows = original.clone();
        let r = pair[0]
            .pending_rows
            .get_mut(&(0, 1, 640))
            .context("decoded original witness")?;
        match mutation {
            0 => r.event = [99; 32],
            1 => r.context = 1,
            2 => r.role = 0,
            3 => r.ordinal = 639,
            4 => r.generation += 1,
            5 => r.operation = [99; 32],
            6 => pair[0].pending_rows.clear(),
            _ => {
                let row = pair[0]
                    .pending_rows
                    .remove(&(0, 1, 640))
                    .context("original tuple")?;
                pair[0].pending_rows.insert((1, 1, 640), row);
            }
        }
        ensure!(
            history.check(0, &pair[0], false).is_err(),
            "changed old history {mutation}"
        );
        if matches!(mutation, 0..=3 | 6 | 7) {
            ensure!(
                live::PendingHistory::capture([55; 32], keys(), [&pair[0], &pair[1]]).is_err(),
                "entry validates exact original membership {mutation}"
            );
        }
    }
    pair[0].pending_rows = original.clone();
    pair[1].pending_rows = original;
    ensure!(
        live::PendingHistory::capture([55; 32], keys(), [&pair[0], &pair[1]]).is_err(),
        "no B pending history"
    );
    ensure!(
        history.check(1, &pair[1], false).is_err(),
        "unchanged B history required"
    );
    pair[1].pending_rows.clear();
    observe(
        &mut pair[0],
        &mut ops,
        keys()[0],
        "saved_pending",
        1,
        pending_fact(641, 56, 6),
    )?;
    ensure!(
        history.check(0, &pair[0], false).is_err(),
        "new actual pending observation after entry refuses"
    );
    ensure!(
        live::PendingHistory::capture([55; 32], keys(), [&pair[0], &pair[1]]).is_err(),
        "extra source refuses at entry"
    );
    Ok(())
}
#[test]
fn s54_p11b2_r1_fresh_zero_restart_requires_empty_pending_observation_history() -> Result<()> {
    let (old, _) = delivered()?;
    let history = live::PendingHistory::capture([55; 32], keys(), [&old[0], &old[1]])?;
    let mut fresh = keys();
    fresh[0].nonce = [20; 32];
    fresh[1].nonce = [21; 32];
    let (mut pair, mut ops) = started(fresh)?;
    for (o, d) in pair.iter().enumerate() {
        history.check(o, d, true)?;
    }
    observe(
        &mut pair[0],
        &mut ops,
        fresh[0],
        "saved_pending",
        0,
        pending_fact(640, 55, 4),
    )?;
    ensure!(
        pair[0].observed_counts()? == Default::default()
            && pair[0].packet()?.is_none()
            && pair[0].pending_rows == old[0].pending_rows,
        "same historical observation with otherwise zero new stream"
    );
    ensure!(
        history.check(0, &pair[0], true).is_err(),
        "even original B640 history is forbidden on fresh restart"
    );
    ensure!(
        history.check(0, &old[0], true).is_err(),
        "old nonce cannot be reclassified as restart"
    );
    Ok(())
}
