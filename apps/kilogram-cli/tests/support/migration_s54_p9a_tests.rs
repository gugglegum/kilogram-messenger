//! P9 supporting pure APIs only. No child runtime, enrollment or fixture access.
use super::*;
fn key(owner: u8, pid: u32, nonce: u8) -> oracle::Incarnation {
    oracle::Incarnation {
        owner: [owner; 32],
        pid,
        nonce: [nonce; 32],
    }
}
#[test]
fn s54_p9a_owner_phase_baseline_ignores_prior_other_owner_and_maps_reopen_local3() -> Result<()> {
    let a = key(1, 10, 1);
    let b = key(2, 20, 2);
    let mut pages = oracle::Pages::default();
    pages.start(a)?;
    pages.start(b)?;
    pages.accepted(b, 1, [1; 32], false, 16)?;
    pages.accepted(b, 2, [2; 32], false, 8)?;
    pages.accepted(a, 1, [3; 32], false, 16)?;
    let base = pages.baseline(b)?;
    ensure!(
        pages.phase_local_target(b, base, 2)? == 4,
        "nonzero actual local baseline"
    );
    pages.accepted(b, 2, [4; 32], true, 1)?;
    pages.accepted(b, 2, [5; 32], false, 0)?;
    pages.accepted(a, 2, [6; 32], false, 16)?;
    ensure!(
        pages.phase_total(b, base)? == 0,
        "head/replay/other owner excluded"
    );
    pages.accepted(b, 3, [7; 32], false, 16)?;
    pages.accepted(b, 4, [8; 32], false, 16)?;
    ensure!(pages.phase_total(b, base)? == 2, "actual phase2");
    let reopened = key(2, 21, 3);
    pages.start(reopened)?;
    ensure!(
        pages.phase_total(b, base).is_err() && pages.owner_total(reopened)? == 4,
        "old incarnation cannot query current phase; actual prior+phase pages retained"
    );
    ensure!(
        pages.phase_local_target(reopened, base, 5)? == 3,
        "fresh actual incarnation plus observed2 derives local3"
    );
    ensure!(
        pages.accepted(reopened, 1, [8; 32], false, 16).is_err(),
        "operation dedup survives reopen"
    );
    for n in 1..=3 {
        pages.accepted(reopened, n, [10 + n as u8; 32], false, 16)?;
    }
    ensure!(
        pages.phase_total(reopened, base)? == 5
            && pages.phase_local_target(reopened, base, 5).is_err(),
        "no extra fresh5 or target already passed"
    );
    Ok(())
}
#[test]
fn s54_p9a_page_view_rejects_unknown_source_nonce_alias_and_target_overflow() -> Result<()> {
    let mut p = oracle::Pages::default();
    let b = key(2, 20, 2);
    p.start(b)?;
    let base = p.baseline(b)?;
    ensure!(
        p.baseline(key(2, 21, 3)).is_err() && p.phase_local_target(key(3, 22, 4), base, 2).is_err(),
        "unknown real incarnation/source"
    );
    let a = key(1, 10, 1);
    p.start(a)?;
    ensure!(
        p.phase_total(a, base).is_err() && p.start(key(3, 20, 2)).is_err() && p.start(b).is_err(),
        "baseline/nonce/start contradiction"
    );
    ensure!(
        p.phase_local_target(b, base, u64::MAX).is_err()
            && p.phase_local_target(b, base, 0).is_err(),
        "checked future arithmetic"
    );
    Ok(())
}
fn zero() -> Value {
    json!({"encrypt":0,"decrypt":0,"attempts":0,"aborted":0,"ambiguous":0,"accounts":0,"outbound":0,"inbound":0})
}
fn marker(kind: &str, f: Value) -> Value {
    json!({"kind":kind,"pid":73,"owner":state::hex(&[1;32]),"incarnation":state::hex(&[2;32]),"sequence":0,"facts":f})
}
fn send(
    d: &mut observations::Decoder,
    o: &mut oracle::Operations,
    p: &mut oracle::Pages,
    kind: &str,
    f: Value,
) -> Result<()> {
    d.line(
        &format!("s54_observation {}", marker(kind, f)),
        o,
        p,
        &mut 0,
    )
}
fn head(binding: u8, generation: u64) -> Value {
    json!({"context":state::hex(&[3;32]),"binding":state::hex(&[binding;32]),"lease_generation":generation,"pre_q":false,"received":640})
}
fn setup() -> Result<(observations::Decoder, oracle::Operations, oracle::Pages)> {
    let mut d =
        observations::Decoder::new(73, [1; 32], [state::hex(&[3; 32]), state::hex(&[4; 32])], 1);
    let mut o = Default::default();
    let mut p = Default::default();
    send(&mut d, &mut o, &mut p, "start", json!({"counters":zero()}))?;
    send(&mut d, &mut o, &mut p, "validated_head", head(5, 6))?;
    Ok((d, o, p))
}
fn page(n: u64, first: u64, last: u64, is_head: bool, operation: u8) -> Value {
    json!({"context":state::hex(&[3;32]),"binding":state::hex(&[5;32]),"lease_generation":6,"local_page":n,"head":is_head,"grant":null,"start":first,"end":last,"new_sources":last-first,"generation":operation,"operation":state::hex(&[operation;32])})
}
#[test]
fn s54_p9a_typed_page_head_tail_and_barrier_are_ordered_and_counted_once() -> Result<()> {
    let (mut d, mut o, mut p) = setup()?;
    let base = p.baseline(d.incarnation()?)?;
    send(
        &mut d,
        &mut o,
        &mut p,
        "head_page",
        page(0, 385, 386, true, 10),
    )?;
    send(
        &mut d,
        &mut o,
        &mut p,
        "normal_page",
        page(1, 450, 466, false, 11),
    )?;
    let facts = page(2, 466, 482, false, 12);
    send(&mut d, &mut o, &mut p, "normal_page", facts.clone())?;
    send(&mut d, &mut o, &mut p, "page", facts.clone())?;
    let h = d.accepted_head_page().context("actual funded head")?;
    let tail = d.first_tail_after_head().context("ordered actual tail")?;
    let pause = d.paused_page().context("validated barrier")?;
    ensure!(
        h.start == 385
            && tail.start == 450
            && h.stamp.line < tail.stamp.line
            && tail.stamp.line < pause.stamp.line
            && pause.local_page == 2
            && pause.operation == [12; 32]
            && pause.generation == 12
            && p.phase_total(d.incarnation()?, base)? == 2,
        "typed evidence without polling transient next450"
    );
    d.page_marker(&marker("page", facts.clone()))?;
    ensure!(
        send(&mut d, &mut o, &mut p, "page", facts).is_err()
            && p.phase_total(d.incarnation()?, base)? == 2,
        "barrier cannot count twice"
    );
    send(
        &mut d,
        &mut o,
        &mut p,
        "final",
        json!({"counters":zero(),"resolved":true}),
    )?;
    ensure!(o.complete()? == (0, 0), "page facts remain non-Native");
    Ok(())
}
#[test]
fn s54_p9a_page_marker_rejects_pid_nonce_binding_operation_and_stale_lease() -> Result<()> {
    let (mut d, mut o, mut p) = setup()?;
    let f = page(1, 450, 466, false, 11);
    send(&mut d, &mut o, &mut p, "normal_page", f.clone())?;
    send(&mut d, &mut o, &mut p, "page", f.clone())?;
    let good = marker("page", f);
    for field in [
        "pid",
        "incarnation",
        "owner",
        "binding",
        "operation",
        "sequence",
    ] {
        let mut bad = good.clone();
        match field {
            "pid" => bad[field] = 74.into(),
            "sequence" => bad[field] = 1.into(),
            "binding" | "operation" => bad["facts"][field] = state::hex(&[9; 32]).into(),
            _ => bad[field] = state::hex(&[9; 32]).into(),
        };
        ensure!(d.page_marker(&bad).is_err(), "stale marker {field}");
    }
    send(&mut d, &mut o, &mut p, "validated_head", head(6, 7))?;
    ensure!(d.page_marker(&good).is_err(), "old barrier after rebound");
    Ok(())
}
fn ack(request: u8, binding: u8, generation: u64, from: Option<u64>) -> Value {
    json!({"context":state::hex(&[3;32]),"binding":state::hex(&[binding;32]),"lease_generation":generation,"request":state::hex(&[request;16]),"query_reply":false,"receipt_count":3,"saved_receipts_digest":state::hex(&[9;32]),"lane":0,"hold_ack_from":from,"receipts":[[1,1,0,24,state::hex(&[24;32]),state::hex(&[4;32])],[2,1,0,25,state::hex(&[25;32]),state::hex(&[5;32])],[2,2,0,385,state::hex(&[85;32]),state::hex(&[6;32])]]})
}
#[test]
fn s54_p9a_targeted_ack_arm_cannot_reuse_old_record_request_or_binding() -> Result<()> {
    let (mut d, mut o, mut p) = setup()?;
    let f = ack(1, 5, 6, Some(385));
    ensure!(
        send(&mut d, &mut o, &mut p, "ack", f.clone()).is_err(),
        "targeted arm required"
    );
    d.arm_ack(0, [5; 32], 6, Some(385))?;
    send(&mut d, &mut o, &mut p, "ack", f.clone())?;
    let old = d.paused_ack().context("actual targeted push")?.clone();
    d.ack_marker(&marker("ack", f.clone()))?;
    d.arm_ack(0, [5; 32], 6, Some(385))?;
    ensure!(
        d.paused_ack().is_none() && d.ack_marker(&marker("ack", f.clone())).is_err(),
        "rearm clears old observation"
    );
    ensure!(
        send(&mut d, &mut o, &mut p, "ack", f.clone()).is_err(),
        "old request replay cannot become new arm"
    );
    send(&mut d, &mut o, &mut p, "validated_head", head(6, 7))?;
    d.arm_ack(0, [6; 32], 7, Some(385))?;
    let fresh = ack(2, 6, 7, Some(385));
    send(&mut d, &mut o, &mut p, "ack", fresh.clone())?;
    let new = d.paused_ack().context("regenerated receipt observation")?;
    ensure!(
        old.receipts == new.receipts
            && old.digest == new.digest
            && old.binding != new.binding
            && old.request != new.request,
        "same saved payload classified separately from new current dispatch"
    );
    d.ack_marker(&marker("ack", fresh.clone()))?;
    ensure!(
        d.ack_marker(&marker("ack", f)).is_err(),
        "old marker rejected after fresh binding"
    );
    for field in [
        "pid",
        "incarnation",
        "binding",
        "request",
        "saved_receipts_digest",
    ] {
        let mut bad = marker("ack", fresh.clone());
        match field {
            "pid" => bad[field] = 74.into(),
            "incarnation" => bad[field] = state::hex(&[9; 32]).into(),
            "request" => bad["facts"][field] = state::hex(&[9; 16]).into(),
            _ => bad["facts"][field] = state::hex(&[0xff; 32]).into(),
        };
        ensure!(d.ack_marker(&bad).is_err(), "ACK marker {field}");
    }
    send(
        &mut d,
        &mut o,
        &mut p,
        "final",
        json!({"counters":zero(),"resolved":true}),
    )?;
    ensure!(
        o.complete()? == (0, 0),
        "ACK tuple observation is not retirement or Native effect"
    );
    Ok(())
}
#[test]
fn s54_p9a_typed_ack_rejects_old_only_wrong_lane_query_count_and_tuple_alias() -> Result<()> {
    for change in [
        "old",
        "query_reply",
        "lane",
        "receipt_count",
        "encoding",
        "ordinal",
        "duplicate",
        "scope",
        "missing_selector",
    ] {
        let (mut d, mut o, mut p) = setup()?;
        d.arm_ack(0, [5; 32], 6, Some(385))?;
        let mut f = ack(1, 5, 6, Some(385));
        match change {
            "old" => f["receipts"][2][3] = 384.into(),
            "query_reply" => f[change] = true.into(),
            "lane" => f[change] = 1.into(),
            "receipt_count" => f[change] = 2.into(),
            "encoding" => f["receipts"][2][0] = 3.into(),
            "ordinal" => f["receipts"][2][3] = 4097.into(),
            "duplicate" => f["receipts"][2] = f["receipts"][1].clone(),
            "missing_selector" => {
                let o = f.as_object_mut().context("test facts")?;
                o.remove("hold_ack_from");
                o.insert("unexpected".into(), Value::Null);
            }
            _ => f["context"] = state::hex(&[8; 32]).into(),
        }
        ensure!(
            send(&mut d, &mut o, &mut p, "ack", f).is_err(),
            "invalid actual tuple {change}"
        );
    }
    Ok(())
}
