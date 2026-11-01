//! P10A predicates over actual public observations; no stored-state constructor.
use super::*;
pub(super) fn owned(next: usize, live: [bool; 2]) -> Result<()> {
    ensure!(
        next == 10 && live == [true; 2],
        "same owned incomplete10/12 P9 input"
    );
    Ok(())
}
pub(super) fn request_digest(requests: &BTreeMap<oracle::Request, Value>) -> Result<old::Id> {
    ensure!(
        requests.len() == 1344,
        "unchanged original1344 Text requests"
    );
    Ok(*blake3::hash(&serde_json::to_vec(
        &requests
            .iter()
            .map(|(r, v)| (r.numeric(), v))
            .collect::<Vec<_>>(),
    )?)
    .as_bytes())
}
pub(super) fn intents(prior: &Value, text_ids: &[[u8; 16]]) -> Result<[[u8; 16]; 2]> {
    let prior = prior
        .as_array()
        .context("actual earlier issued C1 intents")?;
    ensure!(
        prior.len() == 2
            && text_ids.len() == 1344
            && text_ids.iter().collect::<BTreeSet<_>>().len() == 1344,
        "original control/Text request inventory"
    );
    let mut seen = BTreeSet::new();
    let mut owners = BTreeSet::new();
    for r in prior {
        owners.insert(r["owner"].as_u64().context("issued intent owner")?);
        ensure!(
            seen.insert(source::parse_request_id(
                r["request"].as_str().context("issued intent request")?
            )?),
            "distinct earlier issued intents"
        );
    }
    ensure!(
        owners == BTreeSet::from([0, 1]),
        "both actual earlier intent owners"
    );
    let next = [[0xa1; 16], [0xa2; 16]];
    for r in next {
        ensure!(
            !text_ids.contains(&r) && seen.insert(r),
            "fresh distinct control IDs versus original controls/Text"
        );
    }
    Ok(next)
}
pub(super) fn controls(current: &[[Value; 2]; 2], expected: &[[Value; 2]; 2]) -> Result<()> {
    ensure!(
        current == expected,
        "unchanged original OD1/S54 holds/suppressions"
    );
    Ok(())
}
pub(super) fn held(controls: &[[Value; 2]; 2]) -> Result<()> {
    for (owner, c) in controls.iter().enumerate() {
        ensure!(
            c[0]["disable_ack"] == true
                && c[0]["suspend_prefix_requests"] == true
                && c[0]["hold_ordinals"] == json!([])
                && c[1]["phase"] == "catch_up"
                && c[1]["suspend_live"] == true
                && c[1]["suspend_prefix"] == true
                && c[1]["hold_ordinals"] == json!([if owner == 0 { 385 } else { 640 }])
                && c[1]["hold_source64"] == false
                && c[1]["source64_after"].is_null()
                && c[1]["hold_ack"] == false
                && c[1]["hold_ack_from"].is_null()
                && c[1]["pause_after_normal_page"].is_null()
                && c[1]["retry_live"].is_null(),
            "all actual original P9 C1 holds remain armed"
        );
    }
    Ok(())
}
pub(super) fn c1(row: &Value, before: &Value, owner: usize) -> Result<()> {
    for key in [
        "context",
        "role",
        "d",
        "e",
        "sid",
        "q",
        "cuts",
        "controls",
        "current_consent2",
        "control_debt",
        "next",
    ] {
        ensure!(row[key] == before[key], "same actual C1 {key}");
    }
    ensure!(
        row["mode"] == "Active"
            && row["next"]
                == if owner == 0 {
                    json!([641, 640])
                } else {
                    json!([641, 641])
                }
            && row["counts"]["debt"] == owner as u64
            && row["counts"]["queued"] == 0
            && row["counts"]["pending"] == 0
            && row["head_pending"].is_null(),
        "original C1 Active/debt/source frontiers unchanged"
    );
    Ok(())
}
pub(super) fn c2(
    row: &Value,
    before: &Value,
    expected: Option<old::Id>,
    upgrading: bool,
) -> Result<()> {
    for key in ["context", "role", "d", "e", "sid"] {
        ensure!(row[key] == before[key], "same original C2 {key}");
    }
    ensure!(
        row["native"]["keys"] == before["native"]["keys"]
            && row["next"] == json!([33, 33])
            && row["counts"]["sources"] == 66
            && row["counts"]["queued"] == 0
            && row["counts"]["pending"] == 0
            && row["counts"]["debt"] == 0
            && row["head_pending"].is_null(),
        "same actual C2 exact64 Text/keys, no new packets or repair"
    );
    let present = !row["current_consent2"].is_null();
    if present {
        ensure!(
            expected.is_some_and(|id| row["current_consent2"] == state::hex(&id))
                && row["current_consent"] == row["current_consent2"]
                && row["handoff_policy_error"].is_null(),
            "actual ceremony Consent2/current permission, no unrelated consent"
        );
    } else {
        ensure!(
            !upgrading && row["current_consent"] == before["current_consent"],
            "original Consent1 until actual import"
        );
    }
    if upgrading {
        ensure!(
            present
                && ["L1", "Offering", "Prepared", "Selected", "Active"]
                    .iter()
                    .any(|m| row["mode"] == *m),
            "genuine C2 handoff stages after actual Consent2 import"
        );
    } else {
        ensure!(
            row["mode"] == "L1"
                && row["q"].is_null()
                && ["p", "a", "k", "committed"]
                    .iter()
                    .all(|k| row["controls"][k].is_null())
                && row["upgrade"] == "NoLocalIntent",
            "Consent2 alone creates no intent/P/Q/Text"
        );
    }
    Ok(())
}
pub(super) fn admission(
    row: &Value,
    route: &Value,
    head: Option<(old::Id, u64, u64)>,
    stable: bool,
) -> Result<bool> {
    ensure!(
        route["slot"] == row["slot"]
            && route["policy"] == "direct-only"
            && row["policy_error"].is_null(),
        "correct slot/current permission/DirectOnly throughout"
    );
    if row["lease_current"] != true {
        ensure!(
            route["selected_path"].is_null(),
            "no selected path without current lease"
        );
        return Ok(false);
    }
    ensure!(
        route["selected_path"].is_null() || route["selected_path"] == "direct",
        "actual selected direct path, not relay"
    );
    if route["selected_path"].is_null() {
        return Ok(false);
    }
    if !stable {
        return Ok(true);
    }
    Ok(row["lease_family"] == 2
        && row["publication_ready"] == true
        && row["publication_error"].is_null()
        && row["existing_origin_ready"] == true
        && row["origin_publication_error"].is_null()
        && head.is_some_and(|(_, g, _)| row["lease_generation"] == g)
        && row["handoff_policy_error"].is_null())
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
            "actual per-lane source order"
        );
        next[role] += 1;
    }
    ensure!(
        next == frontier && source::history_map(rows)? == *expected,
        "same actual source/EventID/request/body/format map"
    );
    Ok(())
}
pub(super) fn zero(actual: oracle::Counts, before: oracle::Counts) -> Result<()> {
    ensure!(
        actual.delta(before)? == oracle::Counts::default(),
        "actual observed zero P10A packet/constructor/abort/ambiguity work"
    );
    Ok(())
}

#[derive(Default)]
pub(super) struct Selection {
    pub q: Option<old::Id>,
    pub controls: [Option<old::Id>; 4],
}
impl Selection {
    pub fn observe(&mut self, row: &Value, consent: old::Id) -> Result<()> {
        ensure!(
            row["current_consent2"] == state::hex(&consent),
            "selection belongs to actual imported ceremony consent"
        );
        fn retain(saved: &mut Option<old::Id>, raw: &Value) -> Result<()> {
            if !raw.is_null() {
                let id = state::parse_id(raw.as_str().context("actual selected control ID")?)?;
                ensure!(
                    saved.is_none_or(|old| old == id),
                    "actual selected control/Q changed after observation"
                );
                *saved = Some(id);
            } else {
                ensure!(saved.is_none(), "selected control/Q regressed");
            }
            Ok(())
        }
        // A peer may legitimately lag: validate present refs against retained
        // actual refs, but do not require a follower to have them prematurely.
        if !row["q"].is_null() {
            retain(&mut self.q, &row["q"])?;
        }
        for (i, key) in ["p", "a", "k", "committed"].iter().enumerate() {
            if !row["controls"][key].is_null() {
                retain(&mut self.controls[i], &row["controls"][key])?;
            }
        }
        if ["Selected", "Active"].iter().any(|m| row["mode"] == *m) {
            ensure!(
                row["cuts"] == json!([33, 33]) && !row["q"].is_null(),
                "actual selected C2 cut33/33/Q"
            );
        }
        Ok(())
    }
    pub fn active(&self, row: &Value, consent: old::Id) -> Result<bool> {
        if row["mode"] != "Active" {
            return Ok(false);
        }
        ensure!(
            row["current_consent2"] == state::hex(&consent)
                && row["cuts"] == json!([33, 33])
                && self.q.is_some_and(|id| row["q"] == state::hex(&id))
                && self
                    .controls
                    .iter()
                    .zip(["p", "a", "k", "committed"])
                    .all(|(id, k)| id.is_some_and(|id| row["controls"][k] == state::hex(&id))),
            "actual ceremony/retained selected controls, not arbitrary agreement between rows"
        );
        Ok(row["control_debt"] == false)
    }
    pub fn facts(&self) -> Value {
        json!({"q":self.q.map(|id|state::hex(&id)),"controls":self.controls.map(|v|v.map(|id|state::hex(&id)))})
    }
}
