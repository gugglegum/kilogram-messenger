//! One fixed fresh F; never an alternate P workload or a source of P checkpoints.
use super::*;
use control::Controller;
#[path = "migration_s54_feasibility_data.rs"]
mod data;
#[cfg(test)]
#[path = "migration_s54_feasibility_tests.rs"]
mod tests;

pub(super) struct Completed(Value);
impl Completed {
    pub fn facts(&self) -> &Value {
        &self.0
    }
}

#[test]
#[ignore = "requires the exclusive externally activated S54 feasibility attempt"]
fn s54_short_mixed_hooks_feasibility_public() -> Result<()> {
    let admitted = admission::AdmittedRun::load_for(Purpose::FeasibilityF)?;
    let _deadline =
        s53_deadline::Guard::enter_original_feasibility(admitted.run(), admitted.origin())?;
    let _input = memory_scope::InputGuard::install(admitted.memory_input()?)?;
    let completed = run(admitted.run(), admitted.until())?;
    admitted.persist_feasibility(&completed)
}
fn timing(s: &mut Controller, label: &str, started: Instant) -> Result<()> {
    s.check()?;
    s.remember(label,json!({"elapsed_ms":started.elapsed().as_millis(),"counters":s.counters.map(|c|
        json!({"encrypt":c.encrypt,"decrypt":c.decrypt,"attempts":c.attempts,"accounts":c.accounts,"outbound":c.outbound,"inbound":c.inbound}))}))
}
fn traffic(
    actual: oracle::Counts,
    before: oracle::Counts,
    encrypt: u64,
    decrypt: u64,
) -> Result<()> {
    let d = actual.delta(before)?;
    ensure!(
        d.encrypt == encrypt
            && d.decrypt == decrypt
            && d.attempts == encrypt + decrypt
            && d.aborted == 0
            && d.ambiguous == 0
            && d.accounts == 0
            && d.outbound == 0
            && d.inbound == 0,
        "F actual native bracket, no replay/constructors/failed packets"
    );
    Ok(())
}
fn full(s: &mut Controller, owner: usize, held: bool) -> Result<[Value; 2]> {
    s.check()?;
    let node = s.view(owner, 0)?;
    let raw = node.status()?;
    let rows = [
        node.context(&raw)?.clone(),
        s.view(owner, 1)?.context(&raw)?.clone(),
    ];
    s.original[owner]
        .as_ref()
        .context("original owner")?
        .identity(&s.origin.pair.nodes[owner], &rows)?;
    pressure::resources(&raw, node.slot as usize, held)?;
    for (c, row) in rows.iter().enumerate() {
        let slot = s.view(owner, c)?.slot as usize;
        ensure!(
            row["role"] == owner
                && row["context"] == s.origin.contexts[c]
                && row["policy_error"].is_null()
                && raw["routes"][slot]["slot"] == slot
                && raw["routes"][slot]["policy"] == "direct-only"
                && raw["routes"][slot]["selected_path"] == "direct",
            "F original mapping/permission/direct route"
        );
    }
    s.counters[owner] = observations::stats(&raw["stats"])?;
    ensure!(
        s.running[owner]
            .as_ref()
            .context("owned live")?
            .decoder
            .max_packet_delta
            <= 4,
        "actual native packet batch<=4"
    );
    Ok(rows)
}
fn live(s: &mut Controller) -> Result<[[Value; 2]; 2]> {
    s.wait(30, |s| {
        let rows = [
            [s.status(0, 0)?, s.status(0, 1)?],
            [s.status(1, 0)?, s.status(1, 1)?],
        ];
        if !rows.iter().all(|r| {
            r[0]["mode"] == "Active"
                && r[0]["lease_family"] == 2
                && r[0]["lease_current"] == true
                && r[0]["handoff_policy_error"].is_null()
                && r[1]["mode"] == "L1"
                && r[1]["lease_family"] == 1
                && r[1]["lease_current"] == true
                && r[1]["current_consent2"].is_null()
        }) {
            return Ok(None);
        }
        for (owner, row) in rows.iter().enumerate() {
            if !s.running[owner]
                .as_ref()
                .context("live owner")?
                .decoder
                .current_head(0)
                .is_some_and(|(_, g, _)| row[0]["lease_generation"] == g)
            {
                return Ok(None);
            }
        }
        let result = [full(s, 0, false)?, full(s, 1, false)?];
        if let Some(original) = s.rows.get("f-c2-consent") {
            for owner in 0..2 {
                ensure!(
                    result[owner][1]["current_consent"] == original[owner],
                    "same C2 Consent1"
                );
            }
        }
        Ok(Some(result))
    })
}
fn reconnect(s: &mut Controller, owner: usize) -> Result<[[Value; 2]; 2]> {
    let _measurement = s53_deadline::measure("reopen");
    s.start(owner)?;
    s.import2()?;
    s.import_legacy(1)?;
    live(s)
}
fn cuts(s: &mut Controller, next: [u64; 2], debt: [u64; 2]) -> Result<()> {
    s.wait(60, |s| {
        for (owner, expected_debt) in debt.iter().enumerate() {
            let r = s.status(owner, 0)?;
            if r["next"] != json!(next)
                || r["counts"]["debt"] != *expected_debt
                || r["counts"]["pending"] != 0
                || r["counts"]["queued"] != 0
                || !r["head_pending"].is_null()
            {
                return Ok(None);
            }
        }
        Ok(Some(()))
    })
}
fn marker(s: &Controller, owner: usize, tag: &str) -> Result<Value> {
    Ok(serde_json::from_slice(
        &kilogram_cli::ordered_dialog::admin::read_public(
            &s.controls[owner]
                .s54
                .with_extension(format!("s54-{tag}.json")),
            4096,
        )?,
    )?)
}
fn clear(s: &Controller, tag: &str) -> Result<()> {
    let path = s.controls[1].s54.with_extension(format!("s54-{tag}.json"));
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}
fn prefix(s: &mut Controller, suspended: bool) -> Result<()> {
    s.controls[1].old["suspend_prefix_requests"] = suspended.into();
    s.controls[1].current["suspend_prefix"] = suspended.into();
    s.controls[1].write()
}
fn page(
    s: &mut Controller,
    base: oracle::PageBaseline,
    target: u64,
) -> Result<observations::PageObservation> {
    let _measurement = s53_deadline::measure("page_barrier");
    s.check()?;
    ensure!(
        s.controls[1].old["disable_ack"] == true && s.controls[1].current["suspend_prefix"] == true,
        "F page arm while ACK/prefix disabled"
    );
    let key = s.running[1].as_ref().context("B")?.decoder.incarnation()?;
    let local = s.pages.phase_local_target(key, base, target)?;
    clear(s, "page")?;
    s.controls[1].current["pause_after_normal_page"] = local.into();
    s.controls[1].write()?;
    prefix(s, false)?;
    let page = s.wait(60, |s| {
        let run = s.running[1].as_ref().context("same B")?;
        let progress = s.pages.phase_total(key, base)?;
        ensure!(progress <= target, "F phase page target overshoot");
        let Some(page) = run.decoder.paused_page().cloned() else {
            return Ok(None);
        };
        run.decoder.page_marker(&marker(s, 1, "page")?)?;
        ensure!(
            page.stamp.incarnation == key
                && page.context == 0
                && !page.head
                && page.new_sources > 0
                && page.local_page == local
                && progress == target,
            "actual F page phase/local/incarnation"
        );
        Ok(Some(page))
    })?;
    s.wait(30, |s| {
        let r = s.status(1, 0)?;
        let next = r["next"][0].as_u64().context("B frontier")?;
        ensure!(
            next <= page.end && r["next"][1] == 35 && r["counts"]["debt"] == 0,
            "no page drain overshoot"
        );
        Ok(
            (next == page.end && r["counts"]["pending"] == 0 && r["head_pending"].is_null())
                .then_some(()),
        )
    })?;
    prefix(s, true)?;
    s.remember(&format!("f-page-{target}"), page.facts().clone())?;
    Ok(page)
}
fn clear_page(s: &mut Controller) -> Result<()> {
    s.controls[1].current["pause_after_normal_page"] = Value::Null;
    s.controls[1].write()?;
    clear(s, "page")
}
fn arm_ack(s: &mut Controller) -> Result<()> {
    ensure!(
        s.controls.iter().all(|c| c.old["disable_ack"] == true),
        "ACK suppression before F arm"
    );
    let (binding, generation, _) = s.running[1]
        .as_ref()
        .context("B")?
        .decoder
        .current_head(0)
        .context("current B Head")?;
    s.running[1]
        .as_mut()
        .context("B")?
        .decoder
        .arm_ack(0, binding, generation, Some(7))?;
    clear(s, "ack")?;
    s.controls[1].current["hold_ack"] = true.into();
    s.controls[1].current["hold_ack_from"] = 7.into();
    s.controls[1].write()?; // S54 on disk before OD1 enable (Controls writes OD1 first).
    s.controls[1].old["disable_ack"] = false.into();
    s.controls[1].write()
}
fn ack(s: &mut Controller, map: &BTreeMap<String, Value>) -> Result<observations::AckObservation> {
    let _measurement = s53_deadline::measure("ack_barrier");
    s.wait(30, |s| {
        let run = s.running[1].as_ref().context("same B")?;
        let Some(ack) = run.decoder.paused_ack().cloned() else {
            return Ok(None);
        };
        run.decoder.ack_marker(&marker(s, 1, "ack")?)?;
        data::ack_sources(&ack, map)?;
        let r = s.status(0, 0)?;
        let raw = s.view(1, 0)?.status()?;
        ensure!(
            r["counts"]["debt"] == 178 && raw["window"][1][s.view(1, 0)?.slot as usize] == 32,
            "F real held ACK reserve32 and outstanding debt178"
        );
        Ok(Some(ack))
    })
}
fn histories(s: &mut Controller, frozen: &data::Frozen40) -> Result<[BTreeMap<String, Value>; 2]> {
    let _measurement = s53_deadline::measure("histories");
    frozen.verify(&s.view(0, 0)?)?;
    let mut maps = Vec::new();
    for c in 0..2 {
        let a = data::mapped(
            &early::history(&s.view(0, c)?)?,
            c as u8,
            if c == 0 { [184, 34] } else { [4, 4] },
        )?;
        let b = data::mapped(
            &early::history(&s.view(1, c)?)?,
            c as u8,
            if c == 0 { [184, 34] } else { [4, 4] },
        )?;
        ensure!(a == b, "same exact F history at both originals");
        maps.push(a);
    }
    data::retained(&frozen.map, &maps[0])?;
    maps.try_into()
        .map_err(|_| anyhow::anyhow!("two actual maps"))
}
fn normal(s: &mut Controller) -> Result<()> {
    let _measurement = s53_deadline::measure("normal_pair");
    s.request_normal(0)?;
    s.request_normal(1)?;
    s.wait_normal_both()?;
    s.reconcile_normal_both()?;
    s.feasibility_normal_pair()?;
    Ok(())
}
fn legacy_offline_before_queue(row: &Value) -> bool {
    row["phase"] == 3
        && row["send_next"] == 3
        && row["receive_next"] == 3
        && row["unacked"] == 0
        && row["status"]
            .as_str()
            .is_some_and(|v| v.starts_with("ConnectionBackoff: "))
}
fn run(parent: &Path, until: Instant) -> Result<Completed> {
    let started = Instant::now();
    let mut s = Controller::new_feasibility(parent, until)?;
    s.start(0)?;
    s.start(1)?;
    s.import1()?;
    for c in 0..2 {
        for owner in 0..2 {
            s.wait(30, |s| {
                Ok(oracle::old_zero_text_ready(&s.status(owner, c)?)?.then_some(()))
            })?;
        }
    }
    s.stop(1)?;
    s.stop(0)?;
    for owner in 0..2 {
        let witness = source::Legacy::capture(&s.origin.pair.nodes[owner], &s.origin.contexts)?;
        witness.role(owner)?;
        s.original[owner] = Some(witness);
        s.controls[owner].old["hold_ordinals"] = json!([]);
    }
    timing(&mut s, "f-setup", started)?;
    let old = Instant::now();
    s.start(0)?;
    s.start(1)?;
    s.import1()?;
    for owner in 0..2 {
        for n in 1..=2 {
            s.send(owner, n)?;
        }
    }
    s.wait(30, |s| {
        for owner in 0..2 {
            let r = s.status(owner, 0)?;
            if r["send_next"] != 3 || r["receive_next"] != 3 || r["unacked"] != 0 {
                return Ok(None);
            }
        }
        Ok(Some(()))
    })?;
    timing(&mut s, "f-legacy-two-each", old)?;
    s.stop(1)?;
    // Frozen OD1 probes an idle link every30s, then allows15s for its RPC.
    // Close/wait and reopen A after B is stopped: the fresh process has no
    // admitted writer, and Backoff now belongs to a fresh failed admission.
    // Preserve the same30s barrier and prove the original3/3 frontier below.
    s.stop(0)?;
    s.start(0)?;
    s.wait(30, |s| {
        Ok(legacy_offline_before_queue(&s.status(0, 0)?).then_some(()))
    })?;
    let queued = [s.send(0, 3)?, s.send(0, 4)?];
    ensure!(
        s.status(0, 0)?["send_next"] == 3,
        "genuine queued A3/4 not materialized"
    );
    s.stop(0)?;
    let migration = Instant::now();
    s.migrate(0)?;
    s.migrate(1)?;
    s.controls[0].old["suspend_materialization"] = true.into();
    s.start(0)?;
    s.start(1)?;
    s.import1()?;
    s.wait(30, |s| {
        let r = s.status(0, 0)?;
        Ok((r["next"] == json!([3, 3]) && r["counts"]["queued"] == 2).then_some(()))
    })?;
    let queued_status = s.status(0, 0)?;
    s.remember(
        "f-original-queued",
        json!({"replies":queued,"status":queued_status}),
    )?;
    s.controls[0].old["suspend_materialization"] = false.into();
    s.controls[0].write()?;
    cuts(&mut s, [5, 3], [0, 0])?;
    timing(&mut s, "f-migration", migration)?;
    let ceremony = Instant::now();
    s.origin.pair.ceremony("f-c1", "absent", 30 * 86400)?;
    s.import2()?;
    s.origin.pair.upgrade(1, 71)?;
    s.origin.pair.upgrade(0, 72)?;
    let active = live(&mut s)?;
    ensure!(
        active.iter().all(|r| r[0]["cuts"] == json!([5, 3])
            && r[0]["next"] == json!([5, 3])
            && r[1]["next"] == json!([1, 1])),
        "F genuine Active cuts5/3 and untouched C2"
    );
    s.remember(
        "f-c2-consent",
        json!([
            active[0][1]["current_consent"],
            active[1][1]["current_consent"]
        ]),
    )?;
    s.controls[1].current["phase"] = "mixed".into();
    s.controls[1].current["hold_ordinals"] = json!([640]);
    s.controls[1].write()?;
    full(&mut s, 1, false)?; // Runtime must have accepted the real mixed640 control.
    s.controls[1].current["hold_ordinals"] = json!([]);
    s.controls[1].write()?;
    timing(&mut s, "f-active", ceremony)?;
    let baseline = s.counters;
    let turn = Instant::now();
    for n in 5..=6 {
        s.send(0, n)?;
    }
    cuts(&mut s, [7, 3], [0, 0])?;
    for n in 3..=34 {
        s.send(1, n)?;
    }
    cuts(&mut s, [7, 35], [0, 0])?;
    traffic(s.counters[0], baseline[0], 2, 32)?;
    traffic(s.counters[1], baseline[1], 32, 2)?;
    // Create the actual A40 cursor NOW, before A current history grows to169.
    let frozen = data::Frozen40::capture(&s.view(0, 0)?)?;
    timing(&mut s, "f-live-turn-and-A40", turn)?;
    s.stop(1)?;
    let q = data::ack_cursor(&s.view(1, 0)?)?;
    s.remember("f-original-ack-cursor", json!({"q":q}))?;
    s.wait(30, |s| {
        Ok((s.status(0, 0)?["lease_current"] == false).then_some(()))
    })?;
    let before = s.counters[0];
    for (batch, start) in (7u16..=135).step_by(32).enumerate() {
        let clock = Instant::now();
        let end = (start + 31).min(135);
        for n in start..=end {
            s.send(0, n)?;
        }
        s.wait(30, |s| {
            let r = s.status(0, 0)?;
            Ok((r["next"] == json!([u64::from(end) + 1, 35])
                && r["counts"]["debt"] == u64::from(end) - 6
                && r["counts"]["queued"] == 0
                && r["lease_current"] == false)
                .then_some(()))
        })?;
        timing(&mut s, &format!("f-offline-batch-{batch}"), clock)?;
    }
    traffic(s.counters[0], before, 129, 0)?;
    let offline = data::mapped(&early::history(&s.view(0, 0)?)?, 0, [135, 34])?;
    let ids = data::ids(&offline, 7, 135)?;
    s.stop(0)?;
    s.start(0)?;
    let r = s.status(0, 0)?;
    ensure!(
        r["next"] == json!([136, 35])
            && r["counts"]["debt"] == 129
            && s.counters[0] == oracle::Counts::default(),
        "offline129 persistence and zero replay"
    );
    ensure!(
        data::mapped(&early::history(&s.view(0, 0)?)?, 0, [135, 34])? == offline,
        "same129 offline sources after A reopen"
    );
    for owner in 0..2 {
        s.controls[owner].old["disable_ack"] = true.into();
        s.controls[owner].old["suspend_prefix_requests"] = true.into();
        s.controls[owner].current["phase"] = "pressure".into();
        s.controls[owner].current["suspend_prefix"] = true.into();
        s.controls[owner].current["suspend_live"] = (owner == 1).into();
        s.controls[owner].current["hold_ordinals"] =
            if owner == 0 { json!([7]) } else { json!([]) };
        s.controls[owner].write()?;
    }
    reconnect(&mut s, 1)?;
    s.wait(30, |s| {
        let a = s.status(0, 0)?;
        let b = s.status(1, 0)?;
        Ok((a["next"] == json!([136, 35])
            && a["counts"]["debt"] == 129
            && b["next"] == json!([7, 35])
            && b["counts"]["pending"] == 64
            && b["counts"]["debt"] == 0)
            .then_some(()))
    })?;
    let successors = &ids[1..65];
    data::pending(&s, successors)?;
    let (binding, generation, received) = s.running[0]
        .as_ref()
        .context("A")?
        .decoder
        .current_head(0)
        .context("actual A accepted Head")?;
    ensure!(received == 7, "real accepted peer frontier7");
    frozen.verify(&s.view(0, 0)?)?; // retained A40, never current A169 ==40.
    s.running[0]
        .as_mut()
        .context("A")?
        .decoder
        .expect_source64(7, successors, binding, generation)?;
    s.controls[0].current["hold_source64"] = true.into();
    s.controls[0].current["source64_after"] = 7.into();
    let hold_until = (Instant::now() + Duration::from_secs(120)).min(s.until);
    s.controls[0].write()?;
    s.wait(30, |s| {
        Ok(s.running[0]
            .as_ref()
            .and_then(|r| r.decoder.source64.as_ref())
            .is_some()
            .then_some(()))
    })?;
    let held_marker = marker(&s, 0, "source64")?;
    let pids = [
        s.running[0].as_ref().context("A")?.child.0.id(),
        s.running[1].as_ref().context("B")?.child.0.id(),
    ];
    let held_rows = [full(&mut s, 0, true)?, full(&mut s, 1, false)?];
    let bracket = |s: &mut Controller| -> Result<()> {
        ensure!(
            Instant::now() < hold_until && marker(s, 0, "source64")? == held_marker,
            "same finite source64 hold interval"
        );
        for owner in 0..2 {
            let rows = full(s, owner, owner == 0)?;
            let run = s.running[owner].as_ref().context("same held owner")?;
            ensure!(
                run.child.0.id() == pids[owner]
                    && rows[0]["next"]
                        == if owner == 0 {
                            json!([136, 35])
                        } else {
                            json!([7, 35])
                        },
                "same PID/frontier while C2 advances"
            );
            ensure!(
                rows[0]["lease_generation"] == held_rows[owner][0]["lease_generation"]
                    && rows[1]["lease_generation"] == held_rows[owner][1]["lease_generation"]
                    && rows[0]["counts"]["debt"] == if owner == 0 { 129 } else { 0 }
                    && rows[0]["counts"]["pending"] == if owner == 0 { 0 } else { 64 }
                    && rows[0]["counts"]["queued"] == 0,
                "same C1/C2 leases and C1 work during hold"
            );
        }
        s.running[0]
            .as_ref()
            .context("A")?
            .decoder
            .marker(&held_marker)?;
        data::pending(s, successors)
    };
    bracket(&mut s)?;
    let before = s.counters;
    let c2 = Instant::now();
    for n in 1..=4 {
        s.send_context(0, 1, n)?;
        s.send_context(1, 1, n)?;
    }
    s.wait(30, |s| {
        for owner in 0..2 {
            let r = s.status(owner, 1)?;
            if r["next"] != json!([5, 5]) || r["counts"]["debt"] != 0 || r["counts"]["pending"] != 0
            {
                return Ok(None);
            }
        }
        Ok(Some(()))
    })?;
    bracket(&mut s)?;
    s.wait(30, |s| {
        for owner in 0..2 {
            let raw = s.view(owner, 1)?.status()?;
            if raw["window"][1][s.view(owner, 1)?.slot as usize] != 0 {
                return Ok(None);
            }
        }
        Ok(Some(()))
    })?;
    for (owner, baseline) in before.iter().enumerate() {
        traffic(s.counters[owner], *baseline, 4, 4)?;
        ensure!(
            data::mapped(&early::history(&s.view(owner, 1)?)?, 1, [4, 4])?.len() == 8,
            "actual C2 readable8"
        );
    }
    bracket(&mut s)?;
    timing(&mut s, "f-C2-under-ref64", c2)?;
    let b_baseline = s.counters[1];
    for c in &mut s.controls {
        c.current["phase"] = "catch_up".into();
        c.current["suspend_live"] = true.into();
        c.current["hold_source64"] = false.into();
        c.current["source64_after"] = Value::Null;
        c.write()?;
    }
    s.wait(30, |s| {
        let raw = s.view(0, 0)?.status()?;
        Ok((raw["window"][1][s.view(0, 0)?.slot as usize] == 0).then_some(()))
    })?;
    let before = s.counters[0];
    for (batch, start) in [136u16, 168].into_iter().enumerate() {
        let clock = Instant::now();
        let end = (start + 31).min(184);
        for n in start..=end {
            s.send(0, n)?;
        }
        s.wait(30, |s| {
            let r = s.status(0, 0)?;
            Ok((r["next"] == json!([u64::from(end) + 1, 35])
                && r["counts"]["debt"] == u64::from(end) - 6
                && r["counts"]["queued"] == 0
                && r["lease_current"] == true)
                .then_some(()))
        })?;
        timing(&mut s, &format!("f-online-batch-{batch}"), clock)?;
    }
    traffic(s.counters[0], before, 49, 0)?;
    let all = data::mapped(&early::history(&s.view(0, 0)?)?, 0, [184, 34])?;
    data::retained(&offline, &all)?;
    // Initial accepted Head already advertises136. The real signed head-prefix
    // reply may advance that watermark to185; prove its durable value after page2
    // while B is exclusively stopped. received in validated_head means the
    // opposite lane (35 here), so it must never be mistaken for source_next185.
    let key = s.running[1].as_ref().context("B1")?.decoder.incarnation()?;
    let base = s.pages.baseline(key)?;
    page(&mut s, base, 2)?;
    let decoder = &s.running[1].as_ref().context("B1")?.decoder;
    let head = decoder.accepted_head_page().context("actual head7")?;
    let tail = decoder
        .first_tail_after_head()
        .context("actual first tail72")?;
    ensure!(
        head.head
            && head.start == 7
            && head.end == 8
            && tail.start == 72
            && !tail.head
            && head.stamp.line < tail.stamp.line
            && head.binding == tail.binding
            && head.lease_generation == tail.lease_generation,
        "actual head1 plus pending64 split"
    );
    s.remember(
        "f-head-tail",
        json!({"head":head.facts(),"tail":tail.facts()}),
    )?;
    s.stop(1)?;
    let first = s.counters[1].delta(b_baseline)?;
    data::watermark(&s.view(1, 0)?, 185)?;
    clear_page(&mut s)?;
    reconnect(&mut s, 1)?;
    ensure!(s.counters[1] == oracle::Counts::default(), "B2 zero replay");
    let second_key = s.running[1].as_ref().context("B2")?.decoder.incarnation()?;
    ensure!(
        second_key != key
            && second_key.owner == key.owner
            && s.pages.phase_total(second_key, base)? == 2
            && s.pages.phase_local_target(second_key, base, 5)? == 3,
        "phase2 preserved, new local3 targets phase5"
    );
    page(&mut s, base, 5)?;
    s.stop(1)?;
    let second = s.counters[1];
    clear_page(&mut s)?;
    reconnect(&mut s, 1)?;
    ensure!(s.counters[1] == oracle::Counts::default(), "B3 zero replay");
    prefix(&mut s, false)?;
    cuts(&mut s, [185, 35], [178, 0])?;
    prefix(&mut s, true)?;
    ensure!(
        s.pages.phase_total(
            s.running[1].as_ref().context("B3")?.decoder.incarnation()?,
            base
        )? >= 8,
        "actual at least8 normal new-source pages"
    );
    traffic(first, oracle::Counts::default(), 0, first.decrypt)?;
    traffic(second, oracle::Counts::default(), 0, second.decrypt)?;
    let third = s.counters[1];
    traffic(third, oracle::Counts::default(), 0, third.decrypt)?;
    ensure!(
        first.decrypt + second.decrypt + third.decrypt == 178,
        "actual catchup178 decrypt across B incarnations"
    );
    live(&mut s)?;
    let maps = histories(&mut s, &frozen)?;
    data::retained(&offline, &maps[0])?;
    arm_ack(&mut s)?;
    let lost = ack(&mut s, &maps[0])?;
    s.controls[1].old["disable_ack"] = true.into();
    s.controls[1].write()?;
    s.stop(0)?;
    s.wait(30, |s| {
        let row = s.status(1, 0)?;
        let raw = s.view(1, 0)?.status()?;
        Ok((row["lease_current"] == false
            && row["lease_generation"].is_null()
            && raw["window"][1][s.view(1, 0)?.slot as usize] == 0)
            .then_some(()))
    })?;
    reconnect(&mut s, 0)?;
    ensure!(s.counters[0] == oracle::Counts::default(), "A2 zero replay");
    arm_ack(&mut s)?;
    let replay = ack(&mut s, &maps[0])?;
    ensure!(
        lost.binding != replay.binding
            && lost.request != replay.request
            && lost.stamp.incarnation == replay.stamp.incarnation
            && lost.stamp.line < replay.stamp.line
            && lost.receipts == replay.receipts
            && lost.digest == replay.digest,
        "same saved receipt multiset on fresh binding"
    );
    s.remember(
        "f-targeted-ACK",
        json!({"lost":lost.facts(),"replayed":replay.facts()}),
    )?;
    s.controls[1].current["hold_ack"] = false.into();
    s.controls[1].current["hold_ack_from"] = Value::Null;
    s.controls[1].write()?;
    cuts(&mut s, [185, 35], [0, 0])?;
    s.controls[1].old["disable_ack"] = true.into();
    s.controls[1].write()?;
    ensure!(
        histories(&mut s, &frozen)? == maps,
        "ACK changes no immutable source/body"
    );
    let final_started = Instant::now();
    normal(&mut s)?;
    let terminal = data::terminal(&mut s, &maps)?;
    s.start(0)?;
    s.start(1)?;
    s.import2()?;
    s.import_legacy(1)?;
    live(&mut s)?;
    cuts(&mut s, [185, 35], [0, 0])?;
    ensure!(
        s.counters == [oracle::Counts::default(); 2] && histories(&mut s, &frozen)? == maps,
        "final both reopen zero replay and original226 sources"
    );
    normal(&mut s)?;
    ensure!(
        data::terminal(&mut s, &maps)? == terminal,
        "actual signed source/receipt/parent cut survives normal reopen"
    );
    timing(&mut s, "f-final-read-reopen-normal", final_started)?;
    let intervals = s.current_intervals()?;
    let memory = s.finish_feasibility_memory()?;
    let mut facts = json!({"classification":"F feasibility functional only","capture_attempt":Purpose::FeasibilityF.attempt(),
        "offline_sources":ids.iter().map(|(n,id)|json!([n,state::hex(id)])).collect::<Vec<_>>(),
        "online_additional_sources":49,"rows":s.rows,"outcomes":s.outcomes,"terminal":terminal,
        "timings":s53_deadline::feasibility_timings()?,"memory":memory,"intervals":{"streams":intervals.streams,"observed_operations":intervals.observed_operations,"reconciled_entries":intervals.reconciled_entries,"encrypt":intervals.counts.encrypt,"decrypt":intervals.counts.decrypt},"scenario_elapsed_ms":started.elapsed().as_millis(),"controller_elapsed_ms":s53_deadline::observer_context()?.origin.elapsed().as_millis(),
        "P_projection":"unknown until actual F observations and uncovered P-specific costs are reviewed","whole_attempt_verdict":"not_established"});
    public_fingerprints(&mut facts)?;
    Ok(Completed(facts))
}
