//! P10B predicates, shared by the inactive controller and focused pure tests.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stage {
    Held,
    Ack,
    Released,
    Final,
}
pub(super) fn schedules(base: &[[Value; 2]; 2], stage: Stage) -> [[Value; 2]; 2] {
    let mut out = base.clone();
    if stage != Stage::Held {
        out[0][0]["disable_ack"] = false.into();
        out[0][1]["phase"] = "relay".into();
    }
    if matches!(stage, Stage::Released | Stage::Final) {
        out[1][1]["phase"] = "relay".into();
        out[1][1]["hold_ordinals"] = json!([]);
        out[1][1]["suspend_live"] = false.into();
    }
    if stage == Stage::Final {
        for c in &mut out {
            c[0]["disable_ack"] = false.into();
            c[0]["suspend_prefix_requests"] = false.into();
            c[1]["phase"] = "relay".into();
            c[1]["hold_ordinals"] = json!([]);
            c[1]["suspend_live"] = false.into();
            c[1]["suspend_prefix"] = false.into();
        }
    }
    out
}
pub(super) fn controls(
    actual: &[[Value; 2]; 2],
    base: &[[Value; 2]; 2],
    stage: Stage,
) -> Result<()> {
    ensure!(
        *actual == schedules(base, stage),
        "exact staged OD1/S54 suppressions and write order"
    );
    for c in actual {
        ensure!(
            c[0]["hold_ordinals"] == json!([])
                && c[1]["hold_source64"] == false
                && c[1]["source64_after"].is_null()
                && c[1]["hold_ack"] == false
                && c[1]["hold_ack_from"].is_null()
                && c[1]["pause_after_normal_page"].is_null()
                && c[1]["retry_live"].is_null(),
            "no legacy640/leftover selector/pause/retry"
        );
    }
    Ok(())
}
pub(super) fn held(base: &[[Value; 2]; 2]) -> Result<()> {
    for (owner, c) in base.iter().enumerate() {
        ensure!(
            c[0]["disable_ack"] == true
                && c[0]["suspend_prefix_requests"] == true
                && c[1]["phase"] == "catch_up"
                && c[1]["suspend_live"] == true
                && c[1]["suspend_prefix"] == true
                && c[1]["hold_ordinals"] == json!([if owner == 0 { 385 } else { 640 }]),
            "original P10A C1 holds"
        );
    }
    controls(base, base, Stage::Held)
}
pub(super) fn same(
    row: &Value,
    before: &Value,
    owner: usize,
    context: usize,
    stage: Stage,
) -> Result<()> {
    for key in [
        "context",
        "slot",
        "role",
        "d",
        "e",
        "sid",
        "q",
        "cuts",
        "controls",
        "current_consent",
        "current_consent2",
    ] {
        ensure!(
            row[key] == before[key],
            "same original Active scope/selection {key}"
        );
    }
    ensure!(
        row["native"]["keys"] == before["native"]["keys"]
            && row["mode"] == "Active"
            && row["policy_error"].is_null()
            && row["handoff_policy_error"].is_null()
            && row["control_debt"] == false
            && row["counts"]["queued"] == 0
            && row["head_pending"].is_null(),
        "same original active identity/current permissions, no queued/control/head work"
    );
    if context == 1 {
        ensure!(
            row["next"] == json!([33, 33])
                && row["counts"]["sources"] == 66
                && row["counts"]["pending"] == 0
                && row["counts"]["debt"] == 0,
            "unchanged C2 exact64/no work"
        );
    } else {
        let delivering = matches!(stage, Stage::Released | Stage::Final);
        if owner == 0 {
            let n = row["next"][1].as_u64().context("actual A peer frontier")?;
            ensure!(
                row["next"][0] == 641
                    && (n == 640 || (delivering && n == 641))
                    && row["counts"]["debt"] == 0
                    && row["counts"]["sources"] == n + 641
                    && row["counts"]["pending"]
                        .as_u64()
                        .is_some_and(|p| p <= u64::from(delivering && n == 640)),
                "only original B640 may advance A"
            );
        } else {
            ensure!(
                row["next"] == json!([641, 641])
                    && row["counts"]["sources"] == 1282
                    && row["counts"]["pending"] == 0
                    && row["counts"]["debt"]
                        .as_u64()
                        .is_some_and(|d| d == 1 || (delivering && d == 0)),
                "same B source/debt until original receipt"
            );
        }
    }
    if stage == Stage::Final {
        ensure!(
            row["counts"]["pending"] == 0
                && row["counts"]["debt"] == 0
                && row["next"]
                    == if context == 0 {
                        json!([641, 641])
                    } else {
                        json!([33, 33])
                    }
                && row["counts"]["receipts"]
                    == if context == 0 {
                        json!([640, 640])
                    } else {
                        json!([32, 32])
                    },
            "actual complete frontiers/receipts/empty obligations"
        );
    }
    Ok(())
}
pub(super) fn work(
    actual: oracle::Counts,
    owner: usize,
    stage: Stage,
    complete: bool,
) -> Result<()> {
    ensure!(
        actual.encrypt == 0
            && actual.aborted == 0
            && actual.ambiguous == 0
            && actual.accounts == 0
            && actual.outbound == 0
            && actual.inbound == 0
            && actual.attempts == actual.decrypt
            && actual.decrypt
                <= u64::from(owner == 0 && matches!(stage, Stage::Released | Stage::Final))
            && (!complete || actual.decrypt == u64::from(owner == 0)),
        "actual fresh relay work: only one A decrypt/attempt, no constructors/other work"
    );
    Ok(())
}
pub(super) fn packet(
    packet: Option<observations::PacketObservation>,
    owner: usize,
    complete: bool,
) -> Result<()> {
    if let Some(p) = packet {
        ensure!(
            owner == 0
                && p.context == 0
                && p.counts.encrypt == 0
                && p.counts.decrypt == 1
                && p.counts.attempts == 1
                && p.counts.aborted == 0
                && p.counts.ambiguous == 0
                && p.counts.accounts == 0
                && p.counts.outbound == 0
                && p.counts.inbound == 0,
            "actual successful packet belongs to A C1, never C2"
        );
    } else {
        ensure!(
            !complete || owner == 1,
            "actual A C1 packet observation required"
        );
    }
    Ok(())
}
pub(super) fn request(value: &Value, event: old::Id) -> Result<bool> {
    let rows = value.as_array().context("actual request-status3 option")?;
    ensure!(rows.len() == 1, "original durable request exists");
    let row = rows[0].as_array().context("actual request fields")?;
    ensure!(row.len() == 3, "exact request status fields");
    let queue = row[0].as_array().context("actual queue sequence")?;
    let ids = row[1].as_array().context("actual materialized EventID")?;
    ensure!(
        queue.len() <= 1
            && queue.iter().all(Value::is_u64)
            && ids.len() == 1
            && ids[0] == state::hex(&event)
            && matches!(row[2].as_u64(), Some(0 | 1)),
        "same original request/EventID and actual receipt state"
    );
    Ok(row[2] == 1)
}
pub(super) fn acknowledged(before: &Value, after: &Value, event: old::Id) -> Result<()> {
    ensure!(
        !request(before, event)?
            && request(after, event)?
            && before[0][0] == after[0][0]
            && before[0][1] == after[0][1],
        "same original request/source/queue association now has real stored receipt"
    );
    Ok(())
}
pub(super) fn closed(outcomes: &[Value], keys: [oracle::Incarnation; 2]) -> Result<()> {
    ensure!(outcomes.len() == 2, "two actual stopped owner outcomes");
    for (owner, row) in outcomes.iter().enumerate() {
        ensure!(
            row["owner"] == owner
                && row["pid"] == keys[owner].pid
                && row["waited"] == true
                && row["success"] == false
                && row["counter_outcome"] == "source/counter resolved",
            "actual known PID waited/reconciled, not live sample completion"
        );
    }
    Ok(())
}
pub(super) fn overlay(before: &Value, url: &str) -> Result<Value> {
    relay_owner::url(url)?;
    let mut after = before.clone();
    after["route"] = json!({"policy":"relay-only","relay":url,"bind":null});
    Ok(after)
}
pub(super) fn profile(before: &Value, actual: &Value, url: &str) -> Result<()> {
    ensure!(
        *actual == overlay(before, url)?,
        "full public profile changes only accepted route"
    );
    Ok(())
}
pub(super) fn history(
    rows: &[Value],
    expected: &BTreeMap<String, Value>,
    frontier: [u64; 2],
) -> Result<()> {
    let mut next = [1; 2];
    for r in rows {
        let role = r["role"].as_u64().context("actual history role")? as usize;
        ensure!(
            role < 2 && r["ordinal"] == next[role],
            "actual per-lane history order"
        );
        next[role] += 1;
    }
    ensure!(
        next == frontier && source::history_map(rows)? == *expected,
        "exact original source/request/body/format maps"
    );
    Ok(())
}
