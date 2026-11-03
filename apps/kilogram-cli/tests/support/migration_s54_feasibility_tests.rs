//! Pure public regressions. These prove predicates, never scheduler feasibility.
use super::*;
fn id(n: u16) -> old::Id {
    let mut id = [0; 32];
    id[..2].copy_from_slice(&n.to_be_bytes());
    id
}
#[test]
fn s54_f_legacy_offline_barrier_requires_disconnected_preserved_state() -> Result<()> {
    // Sanitized F06 command0108 C1: peer stopped, but the idle link still says Active.
    let saved_active =
        json!({"phase":3,"send_next":3,"receive_next":3,"unacked":0,"status":"Active"});
    ensure!(
        !legacy_offline_before_queue(&saved_active),
        "saved Active is not offline evidence"
    );
    // Sanitized command0022 C1: Backoff is exposed, but startup has not restored C1.
    let saved_bootstrap = json!({"phase":0,"send_next":0,"receive_next":0,"unacked":0,"status":"ConnectionBackoff: static diagnostic omitted"});
    ensure!(
        !legacy_offline_before_queue(&saved_bootstrap),
        "bootstrap/reopen Backoff is not preserved C1 state"
    );
    // Source-backed positive contract fixture, not an observation of another Run.
    let mut disconnected = saved_active.clone();
    disconnected["status"] = "ConnectionBackoff: static diagnostic omitted".into();
    ensure!(
        legacy_offline_before_queue(&disconnected),
        "actual Backoff with preserved legacy frontier is admissible"
    );
    for (field, wrong) in [
        ("phase", json!(1)),
        ("send_next", json!(4)),
        ("receive_next", json!(2)),
        ("unacked", json!(1)),
        ("status", json!("Negotiating")),
        ("status", json!("ConnectionBackoffish")),
        ("status", json!("")),
        ("status", Value::Null),
        ("status", json!(false)),
    ] {
        let mut changed = disconnected.clone();
        changed[field] = wrong;
        ensure!(
            !legacy_offline_before_queue(&changed),
            "offline barrier refuses changed state or absent status"
        );
    }
    println!(
        "s54_legacy_offline_fixture=Active_refused;bootstrap_refused;Backoff_preserved_required;negative_mutations=9;runtime=false"
    );
    Ok(())
}
fn rows(last: [u16; 2]) -> Result<Vec<Value>> {
    let mut out = vec![];
    for role in 0..2u8 {
        for n in 1..=last[role as usize] {
            let r = oracle::Request::new(0, role, n)?;
            out.push(json!({"id":state::hex(&id(n+1000*u16::from(role))),"role":role,"ordinal":n,
            "request":source::request_id(r),"text":r.body(),"format":if n >= [5,3][role as usize]{2}else{1}}));
        }
    }
    Ok(out)
}
#[test]
fn s54_f_closed_purpose_rejects_cross_case_selector_attempt_and_keeps_p_limits() -> Result<()> {
    ensure!(
        Purpose::FeasibilityF.attempt() == "F-s54-mixed-hooks-feasibility-01-attempt-06"
            && Purpose::WholeP.attempt() == "P-s54-whole-resumed-l1-01-attempt-01",
        "closed F06/P01"
    );
    let mut refused = 0;
    for suffix in ["01", "02", "03", "04", "05"] {
        ensure!(
            Purpose::identity(
                Purpose::FeasibilityF.case(),
                &format!("F-s54-mixed-hooks-feasibility-01-attempt-{suffix}"),
                Purpose::FeasibilityF.selector(),
            )
            .is_err(),
            "consumed F{suffix} refused"
        );
        refused += 1;
        println!("s54_f_purpose_refusal=F{suffix}");
    }
    ensure!(refused == 5, "all five consumed attempts checked");
    for p in [Purpose::WholeP, Purpose::FeasibilityF] {
        ensure!(
            Purpose::identity(p.case(), &p.attempt(), p.selector())? == p,
            "exact fixed purpose"
        );
        println!("s54_purpose_accepted={}", p.attempt());
        let other = if p == Purpose::WholeP {
            Purpose::FeasibilityF
        } else {
            Purpose::WholeP
        };
        ensure!(
            Purpose::identity(p.case(), &p.attempt(), other.selector()).is_err()
                && Purpose::identity(p.case(), &other.attempt(), p.selector()).is_err(),
            "cross purpose before setup"
        );
        ensure!(
            p.run(&workspace())
                .file_name()
                .is_some_and(|n| n == p.attempt().as_str()),
            "fixed namespace"
        );
    }
    ensure!(
        Purpose::WholeP.limits() == [2350, 2375, 2400]
            && Purpose::FeasibilityF.limits() == [450, 480, 510]
            && Purpose::from_case("F-custom").is_err(),
        "closed budgets and no arbitrary purpose"
    );
    Ok(())
}
#[test]
fn s54_f_real_a40_cursor_oracle_retains_suffix_after_current_169_and_rejects_new_snapshot()
-> Result<()> {
    let old = rows([6, 34])?;
    let first = json!({"items":old[..16],"view":"actual-old-A-view","F":[7,35],"finished":false,"cursor":"actual-A-cursor"});
    let suffix = json!({"items":old[16..],"view":"actual-old-A-view","F":[7,35],"finished":true});
    let frozen = data::Frozen40::from_pages(&first, |cursor| {
        ensure!(cursor == "actual-A-cursor", "original cursor");
        Ok(suffix.clone())
    })?;
    let current = rows([135, 34])?;
    ensure!(current.len() == 169, "current A is169, not40");
    data::retained(&frozen.map, &data::mapped(&current, 0, [135, 34])?)?;
    frozen.verify_with(|_| Ok(suffix.clone()))?;
    let mut reordered = suffix.clone();
    reordered["items"]
        .as_array_mut()
        .context("suffix rows")?
        .swap(0, 1);
    ensure!(
        frozen.verify_with(|_| Ok(reordered.clone())).is_err(),
        "original suffix order is retained"
    );
    let mut changed = suffix.clone();
    changed["F"] = json!([136, 35]);
    ensure!(
        frozen.verify_with(|_| Ok(changed.clone())).is_err(),
        "current view is not old cursor continuation"
    );
    let mut wrong = old;
    wrong[4]["format"] = 1.into();
    ensure!(
        data::mapped(&wrong, 0, [6, 34]).is_err(),
        "F A5 is Event2 despite P25 boundary"
    );
    Ok(())
}
#[test]
fn s54_f_pages_use_real_owner_baseline_and_new_local3_after_phase2() -> Result<()> {
    let mut pages = oracle::Pages::default();
    let first = oracle::Incarnation {
        owner: id(1),
        pid: 4,
        nonce: id(2),
    };
    pages.start(first)?;
    pages.accepted(first, 1, id(20), false, 1)?;
    let base = pages.baseline(first)?;
    pages.accepted(first, 1, id(21), true, 1)?;
    pages.accepted(first, 1, id(22), false, 0)?;
    pages.accepted(first, 2, id(23), false, 16)?;
    pages.accepted(first, 3, id(24), false, 16)?;
    ensure!(
        pages.phase_total(first, base)? == 2,
        "exclude prior calibration/head/replay"
    );
    let second = oracle::Incarnation {
        owner: id(1),
        pid: 5,
        nonce: id(3),
    };
    pages.start(second)?;
    ensure!(
        pages.phase_local_target(second, base, 5)? == 3,
        "new local3 is phase5"
    );
    ensure!(
        (129u64 - 65).div_ceil(16) == 4 && (178u64 - 65).div_ceil(16) == 8,
        "cardinality premise, not runtime evidence"
    );
    Ok(())
}
fn ack() -> Result<observations::AckObservation> {
    let mut d = observations::Decoder::new(73, id(1), [state::hex(&id(3)), state::hex(&id(4))], 1);
    let mut ops = oracle::Operations::default();
    let mut pages = oracle::Pages::default();
    let mut line = |d: &mut observations::Decoder, kind: &str, facts: Value| {
        d.line(&format!("s54_observation {}",
        json!({"kind":kind,"pid":73,"owner":state::hex(&id(1)),"incarnation":state::hex(&id(2)),"sequence":0,"facts":facts})),&mut ops,&mut pages,&mut 0)
    };
    line(
        &mut d,
        "start",
        json!({"counters":{"encrypt":0,"decrypt":0,"attempts":0,"aborted":0,"ambiguous":0,"accounts":0,"outbound":0,"inbound":0}}),
    )?;
    line(
        &mut d,
        "validated_head",
        json!({"context":state::hex(&id(3)),"binding":state::hex(&id(9)),"lease_generation":6,"pre_q":false,"received":185}),
    )?;
    d.arm_ack(0, id(9), 6, Some(7))?;
    line(
        &mut d,
        "ack",
        json!({"context":state::hex(&id(3)),"binding":state::hex(&id(9)),"lease_generation":6,
        "request":state::hex(&[8;16]),"query_reply":false,"receipt_count":2,"saved_receipts_digest":state::hex(&id(900)),
        "lane":0,"receipts":[[1,1,0,4,state::hex(&id(4)),state::hex(&id(904))],[2,2,0,7,state::hex(&id(7)),state::hex(&id(907))]],"hold_ack_from":7}),
    )?;
    d.paused_ack()
        .cloned()
        .context("actual shared decoder ACK predicate")
}
#[test]
fn s54_f_targeted_ack_rejects_old_only_and_wrong_source_format() -> Result<()> {
    let map = data::mapped(&rows([184, 34])?, 0, [184, 34])?;
    let actual = ack()?;
    data::ack_sources(&actual, &map)?;
    let mut old = actual.clone();
    old.receipts.retain(|r| r.ordinal < 7);
    ensure!(
        data::ack_sources(&old, &map).is_err(),
        "nonempty old-only packet is insufficient"
    );
    let mut wrong = actual;
    wrong.receipts[1].source_format = 1;
    ensure!(
        data::ack_sources(&wrong, &map).is_err(),
        "actual source format association"
    );
    Ok(())
}
