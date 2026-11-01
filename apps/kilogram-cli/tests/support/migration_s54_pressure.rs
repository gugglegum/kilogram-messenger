//! Inactive consuming P8. The same live hold/owners carry into P9; no executable PASS.
use super::*;
#[cfg(test)]
#[path = "migration_s54_pressure_tests.rs"]
mod tests;

pub(super) struct Hold {
    pub marker: Value,
    pub generations: [u64; 2],
    pub pids: [u32; 2],
    pub c2_generations: [u64; 2],
    pub until: Instant,
}
pub(super) struct Continuation {
    pub previous: offline::Continuation,
    pub hold: Hold,
    pub successors: Vec<(u64, old::Id)>,
    pub pending: Vec<observations::Pending>,
    pub c2_history: [BTreeMap<String, Value>; 2],
}
fn successors(all: &[(u64, old::Id)]) -> Result<Vec<(u64, old::Id)>> {
    ensure!(
        all.len() == 256
            && all.iter().enumerate().all(|(i, r)| r.0 == 385 + i as u64)
            && all.iter().map(|r| r.1).collect::<BTreeSet<_>>().len() == 256,
        "original actual P7 vector"
    );
    Ok(all[1..65].to_vec())
}
fn pending(
    rows: &BTreeMap<(u8, u8, u64), observations::Pending>,
    ids: &[(u64, old::Id)],
) -> Result<Vec<observations::Pending>> {
    ensure!(ids.len() == 64, "bounded real successor comparison");
    ensure!(
        rows.values()
            .filter(|p| p.context == 0 && p.role == 0)
            .count()
            == 64,
        "no extra actual C1 pending association"
    );
    ids.iter()
        .map(|(n, id)| {
            let p = rows
                .get(&(0, 0, *n))
                .context("actual committed C1 A successor pending")?;
            ensure!(
                p.context == 0
                    && p.role == 0
                    && p.ordinal == *n
                    && p.event == *id
                    && p.generation > 0,
                "exact actual P7/B pending source association"
            );
            Ok(p.clone())
        })
        .collect()
}
fn c1(rows: &[Value; 2]) -> Result<()> {
    ensure!(
        rows[0]["next"] == json!([641, 640])
            && rows[1]["next"] == json!([385, 641])
            && rows[0]["counts"]["debt"] == 256
            && rows[1]["counts"]["debt"] == 1
            && rows[0]["counts"]["pending"] == 0
            && rows[1]["counts"]["pending"] == 64
            && rows
                .iter()
                .all(|r| r["counts"]["queued"] == 0 && r["head_pending"].is_null()),
        "actual A debt256/B pending64 missing385/B640 held, no funded head"
    );
    Ok(())
}
pub(super) fn resources(full: &Value, c1_slot: usize, held: bool) -> Result<()> {
    let w = &full["window"];
    ensure!(
        w[0] == json!([64, 64])
            && w[1].as_array().is_some_and(|v| v.len() == 2)
            && w[2].as_array().is_some_and(|v| v.len() == 2),
        "two actual independent64 shares"
    );
    for slot in 0..2 {
        for part in [1, 2] {
            ensure!(
                w[part][slot].as_u64().is_some_and(|n| n <= 64),
                "bounded actual window"
            );
        }
    }
    ensure!(
        !held || w[1][c1_slot] == 64,
        "same live real64 C1 reservation"
    );
    let r = &full["resources"];
    for key in ["connections", "peak_connections"] {
        ensure!(
            r[key].as_u64().is_some_and(|n| n <= 2),
            "actual global connections bound"
        );
    }
    for key in ["mailbox", "peak_mailbox"] {
        ensure!(
            r[key].as_u64().is_some_and(|n| n <= 16),
            "actual mailbox bound"
        );
    }
    ensure!(
        r["connection_limit"] == 2
            && r["mailbox_limit"] == 16
            && r["frame_limit"] == 1048576
            && r["bidi_limit"] == 4
            && r["uni_limit"] == 0,
        "actual resource limits unchanged"
    );
    Ok(())
}
pub(super) fn full(s: &mut control::Controller, owner: usize) -> Result<(Value, [Value; 2])> {
    s.check()?;
    let node = s.view(owner, 0)?;
    let raw = node.status()?;
    let rows = [
        node.context(&raw)?.clone(),
        s.view(owner, 1)?.context(&raw)?.clone(),
    ];
    ensure!(
        rows.iter()
            .enumerate()
            .all(|(c, r)| r["context"] == s.origin.contexts[c]
                && r["role"] == owner as u64
                && r["policy_error"].is_null()
                && (c != 0 || r["handoff_policy_error"].is_null())),
        "actual same owner/C/current permission mapping"
    );
    s.original[owner]
        .as_ref()
        .context("original two-context identity")?
        .identity(&s.origin.pair.nodes[owner], &rows)?;
    consent1(
        &rows[1],
        &s.rows.get("p7").context("original P7 status witness")?["final_reopen"][1]["current_consent"],
    )?;
    for c in 0..2 {
        let slot = s.view(owner, c)?.slot as usize;
        ensure!(
            raw["routes"][slot]["slot"] == slot
                && raw["routes"][slot]["policy"] == "direct-only"
                && raw["routes"][slot]["selected_path"] == "direct",
            "actual DirectOnly route for both same-process contexts"
        );
    }
    s.counters[owner] = observations::stats(&raw["stats"])?;
    Ok((raw, rows))
}
fn consent1(row: &Value, original: &Value) -> Result<()> {
    ensure!(
        original.is_string()
            && row["mode"] == "L1"
            && row["alpn1_allowed"] == true
            && row["lease_current"] == true
            && row["lease_family"] == 1
            && row["policy_error"].is_null()
            && row["current_consent"] == *original
            && row["current_consent2"].is_null(),
        "same original current C2 Consent1/ALPN1, no C2 handoff"
    );
    Ok(())
}
pub(super) fn bracket(
    s: &mut control::Controller,
    hold: &Hold,
    ids: &[(u64, old::Id)],
) -> Result<[Value; 2]> {
    ensure!(
        Instant::now() < hold.until,
        "same conservative hold bound, armed before observation"
    );
    let mut facts = Vec::new();
    let mut rows = Vec::new();
    for owner in 0..2 {
        let (raw, both) = full(s, owner)?;
        ensure!(
            both[1]["lease_generation"] == hold.c2_generations[owner],
            "same actual C2 current lease throughout overlap"
        );
        resources(&raw, s.view(owner, 0)?.slot as usize, owner == 0)?;
        rows.push(both[0].clone());
        facts.push(raw);
        let run = s.running[owner]
            .as_ref()
            .context("same live owned process")?;
        ensure!(
            run.child.0.id() == hold.pids[owner] && run.decoder.max_packet_delta <= 4,
            "same actual process/bounded observed Native batch"
        );
        ensure!(
            s.controls[owner].old["disable_ack"] == true
                && s.controls[owner].current["suspend_prefix"] == true,
            "C1 competing prefix/ACK suppression still active"
        );
    }
    let pair: [Value; 2] = rows
        .try_into()
        .map_err(|_| anyhow::anyhow!("two actual C1 rows"))?;
    transition::lease_rows(&pair, ["Active", "Active"], Some(hold.generations))?;
    c1(&pair)?;
    ensure!(
        s.controls[0].current["hold_source64"] == true
            && s.controls[0].current["source64_after"] == 385,
        "same unreleased source64 control"
    );
    let marker: Value = serde_json::from_slice(&kilogram_cli::ordered_dialog::admin::read_public(
        &s.controls[0].s54.with_extension("s54-source64.json"),
        4096,
    )?)?;
    ensure!(
        marker == hold.marker,
        "same actual pause interval, not replacement marker"
    );
    s.running[0]
        .as_ref()
        .context("live A")?
        .decoder
        .marker(&marker)?;
    pending(
        &s.running[1]
            .as_ref()
            .context("live B")?
            .decoder
            .pending_rows,
        ids,
    )?;
    facts
        .try_into()
        .map_err(|_| anyhow::anyhow!("two full status records"))
}
fn history(rows: &[Value]) -> Result<BTreeMap<String, Value>> {
    let map = source::history_map(rows)?;
    ensure!(map.len() == 64, "actual C2 readable64");
    let mut requests = BTreeSet::new();
    for r in map.values() {
        let request = oracle::Request::new(
            1,
            r["role"]
                .as_u64()
                .context("actual history role")?
                .try_into()?,
            r["ordinal"]
                .as_u64()
                .context("actual history ordinal")?
                .try_into()?,
        )?;
        ensure!(
            r["format"] == 1
                && r["request"] == source::request_id(request)
                && r["text"] == request.body()
                && requests.insert(request),
            "actual C2 original format1/request/body/source"
        );
    }
    Ok(map)
}
pub(super) fn p8(mut previous: offline::Continuation) -> Result<Continuation> {
    let ids = successors(&previous.offline_sources())?;
    let s = &mut previous.controller;
    let _phase = s53_deadline::Phase::until(s.until)?;
    ensure!(
        s.phases.next == 8 && !s.phases.complete() && s.running[0].is_some(),
        "consume same owned P7 input"
    );
    s.stopped(1)?;
    let a = s.status(0, 0)?;
    ensure!(
        a["next"] == json!([641, 640])
            && a["counts"]["debt"] == 256
            && a["lease_current"] == false
            && a["lease_generation"].is_null(),
        "actual offline P7 A input"
    );
    ensure!(
        source::debt(&s.view(1, 0)?, oracle::Request::new(0, 1, 640)?)? == previous.held_b640,
        "same actual original B640 before restart"
    );
    for owner in 0..2 {
        let c = &mut s.controls[owner];
        c.old["disable_ack"] = true.into();
        c.current["phase"] = "pressure".into();
        c.current["retry_live"] = Value::Null;
        c.current["hold_source64"] = false.into();
        c.current["source64_after"] = Value::Null;
        c.current["hold_ordinals"] = json!([if owner == 0 { 385 } else { 640 }]);
        c.current["suspend_prefix"] = true.into();
        c.current["suspend_live"] = (owner == 1).into();
        c.write()?;
    }
    let path = s.controls[0].s54.with_extension("s54-source64.json");
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    s.start(1)?;
    s.import2()?;
    s.import_legacy(1)?;
    s.wait(30, |s| {
        let a = s.status(0, 0)?;
        let b = s.status(1, 0)?;
        Ok((a["lease_current"] == true
            && b["lease_current"] == true
            && b["counts"]["pending"] == 64)
            .then_some(()))
    })?;
    let rows = [s.status(0, 0)?, s.status(1, 0)?];
    c1(&rows)?;
    let generations = transition::lease_rows(&rows, ["Active", "Active"], None)?;
    s.wait(30, |s| {
        let a = s.status(0, 1)?;
        let b = s.status(1, 1)?;
        Ok([a, b]
            .iter()
            .all(|r| {
                r["lease_current"] == true
                    && r["lease_family"] == 1
                    && r["mode"] == "L1"
                    && r["next"] == json!([1, 1])
                    && r["counts"]["queued"] == 0
                    && r["counts"]["pending"] == 0
                    && r["counts"]["debt"] == 0
            })
            .then_some(()))
    })?;
    let observed = pending(
        &s.running[1]
            .as_ref()
            .context("current B")?
            .decoder
            .pending_rows,
        &ids,
    )?;
    let (binding, generation, received) = s.running[0]
        .as_ref()
        .context("current A")?
        .decoder
        .current_head(0)
        .context("actual accepted A peer Head before source64")?;
    ensure!(
        generation == generations[0] && received == 385,
        "current accepted peer receive frontier"
    );
    // History3 reserves34 C1 refs. Read the original cursor before the64 hold.
    s.frozen
        .as_ref()
        .context("original HVI1 cursor")?
        .verify(&s.view(0, 0)?)?;
    s.running[0]
        .as_mut()
        .context("current A")?
        .decoder
        .expect_source64(385, &ids, binding, generation)?;
    s.controls[0].current["hold_source64"] = true.into();
    s.controls[0].current["source64_after"] = 385.into();
    let pause_ms = s.controls[0].current["max_pause_ms"]
        .as_u64()
        .filter(|n| (1..=120000).contains(n))
        .context("existing bounded pause")?;
    let hold_until = (Instant::now()
        .checked_add(Duration::from_millis(pause_ms))
        .context("hold bound")?)
    .min(s.until);
    s.controls[0].write()?;
    s.wait(30, |s| {
        Ok(s.running[0]
            .as_ref()
            .and_then(|r| r.decoder.source64.as_ref())
            .is_some()
            .then_some(()))
    })?;
    let marker: Value = serde_json::from_slice(&kilogram_cli::ordered_dialog::admin::read_public(
        &path, 4096,
    )?)?;
    let pids = [
        s.running[0].as_ref().context("A")?.child.0.id(),
        s.running[1].as_ref().context("B")?.child.0.id(),
    ];
    let c2_generations = [
        s.status(0, 1)?["lease_generation"]
            .as_u64()
            .context("actual C2 A generation")?,
        s.status(1, 1)?["lease_generation"]
            .as_u64()
            .context("actual C2 B generation")?,
    ];
    let hold = Hold {
        marker,
        generations,
        pids,
        c2_generations,
        until: hold_until,
    };
    let _hold_phase = s53_deadline::Phase::until(hold.until)?;
    let before = bracket(s, &hold, &ids)?;
    for (owner, raw) in before.iter().enumerate() {
        let slot = s.view(owner, 1)?.slot as usize;
        let r = &raw["contexts"][slot];
        ensure!(
            r["next"] == json!([1, 1])
                && r["counts"]["queued"] == 0
                && r["counts"]["pending"] == 0
                && r["counts"]["debt"] == 0,
            "actual original C2 zero-Text start"
        );
    }
    let counters = s.counters;
    for start in (1u16..=32).step_by(4) {
        bracket(s, &hold, &ids)?;
        let end = start + 3;
        for owner in 0..2 {
            for n in start..=end {
                s.send_context(owner, 1, n)?;
            }
        }
        s.wait(30, |s| {
            let a = s.status(0, 1)?;
            let b = s.status(1, 1)?;
            Ok([a, b]
                .iter()
                .all(|r| {
                    r["next"] == json!([end + 1, end + 1])
                        && r["counts"]["queued"] == 0
                        && r["counts"]["pending"] == 0
                        && r["counts"]["debt"] == 0
                })
                .then_some(()))
        })?;
        bracket(s, &hold, &ids)?;
    }
    s.wait(15, |s| {
        let mut idle = true;
        for owner in 0..2 {
            let node = s.view(owner, 1)?;
            let raw = node.status()?;
            idle &= raw["window"][1][usize::from(node.slot)] == 0;
        }
        Ok(idle.then_some(()))
    })?;
    bracket(s, &hold, &ids)?;
    let c2_history = [
        history(&early::history(&s.view(0, 1)?)?)?,
        history(&early::history(&s.view(1, 1)?)?)?,
    ];
    ensure!(
        c2_history[0] == c2_history[1],
        "same actual C2 immutable64 sources at both peers"
    );
    let after = bracket(s, &hold, &ids)?;
    for (owner, raw) in after.iter().enumerate() {
        let slot = s.view(owner, 1)?.slot as usize;
        let r = &raw["contexts"][slot];
        ensure!(
            r["next"] == json!([33, 33])
                && r["counts"]["queued"] == 0
                && r["counts"]["pending"] == 0
                && r["counts"]["debt"] == 0,
            "actual C2 complete readable/receipted duplex under live C1 hold"
        );
    }
    for (owner, before_counts) in counters.into_iter().enumerate() {
        let delta = s.counters[owner].delta(before_counts)?;
        ensure!(
            delta.encrypt == 32
                && delta.decrypt == 32
                && delta.attempts == 64
                && delta.aborted == 0
                && delta.ambiguous == 0
                && delta.accounts == 0
                && delta.outbound == 0
                && delta.inbound == 0,
            "actual C2 duplex effects while same C1 source hold lives"
        );
    }
    ensure!(
        s.requests.keys().filter(|r| r.context == 1).count() == 64,
        "same controller actual C2 request map"
    );
    s.remember("p8",json!({"before":before,"after":after,"source64":hold.marker,"pending":observed.iter().map(|p|json!({"context":p.context,"role":p.role,"ordinal":p.ordinal,"event":state::hex(&p.event),"generation":p.generation,"operation":state::hex(&p.operation)})).collect::<Vec<_>>(),"c2_history_digest":blake3::hash(&serde_json::to_vec(&c2_history)?).to_hex().to_string(),"classification":"owned live overlap observations, not whole PASS"}))?;
    s.phases.accept(oracle::Phase::Overlap)?;
    ensure!(
        s.phases.next == 9 && !s.phases.complete(),
        "owned incomplete9/12"
    );
    Ok(Continuation {
        previous,
        hold,
        successors: ids,
        pending: observed,
        c2_history,
    })
}
