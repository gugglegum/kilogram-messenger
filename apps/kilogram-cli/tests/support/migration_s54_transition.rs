//! P3/P4: pre-Q saved R1, real lost Committed, both restart, Selected receive-only.
use super::*;
use control::Controller;

pub(super) fn lease_rows(
    rows: &[Value; 2],
    modes: [&str; 2],
    generation: Option<[u64; 2]>,
) -> Result<[u64; 2]> {
    let mut out = [0; 2];
    for (role, row) in rows.iter().enumerate() {
        ensure!(
            row["role"] == role as u64
                && row["mode"] == modes[role]
                && row["lease_current"] == true
                && row["lease_family"] == 2
                && row["policy_error"].is_null()
                && row["handoff_policy_error"].is_null(),
            "actual current family2/mode bracket"
        );
        out[role] = row["lease_generation"]
            .as_u64()
            .context("actual current lease generation")?;
    }
    if let Some(expected) = generation {
        ensure!(
            out == expected,
            "same local family generations around early readability"
        );
    }
    Ok(out)
}
impl Controller {
    fn both(&mut self) -> Result<[Value; 2]> {
        Ok([self.status(0, 0)?, self.status(1, 0)?])
    }
    fn identity(&mut self) -> Result<()> {
        for owner in 0..2 {
            let rows = [self.status(owner, 0)?, self.status(owner, 1)?];
            self.original[owner]
                .as_ref()
                .context("original two-C Native identity")?
                .identity(&self.origin.pair.nodes[owner], &rows)?;
        }
        Ok(())
    }
    pub fn p3(&mut self) -> Result<()> {
        self.stop(1)?;
        self.stop(0)?;
        self.stopped(0)?;
        self.old_overlap = source::current_old_encrypts(&self.view(0, 0)?)?;
        self.migrate(1)?;
        let prior = &self.rows["p2"]["a"]["next"];
        let readable = prior[0].as_u64().context("observed A own frontier")?
            + prior[1].as_u64().context("observed A peer frontier")?
            - 2;
        let observed = readable
            + u64::try_from(
                self.original[0]
                    .as_ref()
                    .context("original A genesis sources")?
                    .packet_effects()
                    + self.migration[1]
                        .as_ref()
                        .context("actual B legacy sources")?
                        .packet_effects(),
            )?;
        ensure!(observed == 95, "source-observed old/genesis subtotal95");
        self.remember(
            "p2-source-subtotal",
            json!({"old_genesis_subtotal":observed}),
        )?;
        let r1 = self.migration[1]
            .as_ref()
            .context("B original source")?
            .r1(&self.view(0, 0)?)?;
        self.remember("saved-r1", r1)?;
        // Pause actual signed P outside Owner7. B's independent worker can
        // perform pre-Q Head and saved-ACK work before Q exists on either side.
        self.controls[0].arm(Some("s53-P-before-publication"))?;
        self.controls[1].arm(Some("s53-Committed-before-publication"))?;
        self.start(0)?;
        self.start(1)?;
        self.import1()?;
        self.origin.pair.ceremony("p5-c1", "absent", 30 * 86400)?;
        self.import2()?;
        let intents = [(1, 71u8), (0, 72u8)];
        for (owner, id) in intents {
            self.origin.pair.upgrade(owner, id)?;
        }
        self.remember(
            "c1-upgrade-requests",
            json!(intents.map(|(owner, id)| json!({"owner":owner,"request":state::hex(&[id;16])}))),
        )?;
        self.wait(30, |s| {
            Ok((s.controls[0].hit("s53-P-before-publication")?
                && s.pre_q_heads.iter().all(|n| *n > 0))
            .then_some(()))
        })?;
        let before = self.both()?;
        ensure!(
            before.iter().all(|r| r["q"].is_null()) && before[0]["counts"]["debt"] == 1,
            "actual pre-Q Head before saved R1 retirement"
        );
        self.controls[0].old["disable_ack"] = false.into();
        self.controls[1].old["disable_ack"] = false.into();
        for c in &self.controls {
            c.write()?;
        }
        let retired = self.wait(30, |s| {
            let rows = s.both()?;
            ensure!(
                rows.iter().all(|r| r["q"].is_null()),
                "saved R1 must retire before Q"
            );
            Ok((rows[0]["counts"]["debt"] == 0).then_some(rows))
        })?;
        let a24 = early::source_row(&self.view(0, 0)?, 0, 24)?;
        ensure!(
            a24["delivered"] == true && a24["id"] == self.rows["saved-r1"]["event"],
            "same saved R1 source retired, not a substitute packet"
        );
        self.controls[0].arm(Some("s53-K-before-publication"))?;
        let selected = self.wait(30, |s| {
            if !s.controls[0].hit("s53-K-before-publication")? {
                return Ok(None);
            }
            let rows = s.both()?;
            Ok((rows[0]["mode"] == "Selected" && rows[1]["mode"] == "Prepared").then_some(rows))
        })?;
        lease_rows(&selected, ["Selected", "Prepared"], None)?;
        ensure!(
            selected[0]["cuts"] == json!([25, 25])
                && selected[1]["cuts"] == json!([25, 25])
                && selected[0]["next"] == json!([25, 16])
                && selected[1]["next"] == json!([25, 25]),
            "real25/25 cuts and missing16..24 preserved"
        );
        // No data release occurs in this bracket. All-zero real counters are
        // scoped to these fresh incarnations' ceremony/selection/conversion.
        ensure!(
            self.counters == [oracle::Counts::default(); 2],
            "conversion-only current process bracket has no packet/constructor operations"
        );
        let k = self.controls[0].facts("K", &selected[0]["controls"]["k"])?;
        let p = self.controls[0].facts("P", &selected[0]["controls"]["p"])?;
        self.identity()?;
        self.remember("p3",json!({"before":before,"retired_pre_q":retired,"selected":selected,"p":p,"k":k,"A24":a24,"pre_q_heads":self.pre_q_heads}))?;
        self.phases.accept(oracle::Phase::Handoff)
    }
    pub fn p4(&mut self) -> Result<()> {
        let lost_started = Instant::now();
        let lost_until = control::interval10(lost_started, self.until);
        let lost_phase = s53_deadline::Phase::until(lost_until)?;
        self.controls[0].arm(None)?;
        let committed = self.wait(10, |s| {
            if !s.controls[1].hit("s53-Committed-before-publication")? {
                return Ok(None);
            }
            let rows = s.both()?;
            Ok((rows[0]["mode"] == "Selected" && rows[1]["mode"] == "Active").then_some(rows))
        })?;
        let facts = self.controls[1].facts("Committed", &committed[1]["controls"]["committed"])?;
        ensure!(
            self.counters == [oracle::Counts::default(); 2],
            "B conversion-only bracket has no packet/constructor operations"
        );
        // No source::end/Native/repository reconciliation may separate these waits.
        self.kill_wait(1)?;
        self.kill_wait(0)?;
        s53_deadline::check()?;
        let lost_elapsed = lost_started.elapsed().as_secs_f64();
        drop(lost_phase);
        self.reconcile_both()?;
        self.stopped(0)?;
        let accepted = source::accepted_r1(&self.view(0, 0)?, &self.rows["saved-r1"])?;
        self.remember("accepted-r1", accepted)?;
        // Re-arm before either reopen; immutable facts must match the exact retry.
        self.controls[0].arm(Some("s53-K-before-publication"))?;
        self.controls[1].arm(Some("s53-Committed-before-publication"))?;
        self.start(0)?;
        self.start(1)?;
        self.import2()?;
        let rows = self.wait(15, |s| {
            if !s.controls[0].hit("s53-K-before-publication")? {
                return Ok(None);
            }
            let rows = s.both()?;
            let observed = rows[1]["lease_generation"].as_u64().is_some_and(|g| {
                s.running[1]
                    .as_ref()
                    .is_some_and(|r| r.decoder.has_current_lease(g))
            });
            Ok(
                (rows[0]["lease_current"] == true && rows[1]["lease_current"] == true && observed)
                    .then_some(rows),
            )
        })?;
        let generations = lease_rows(&rows, ["Selected", "Active"], None)?;
        let k_retry = self.controls[0].facts("K", &rows[0]["controls"]["k"])?;
        ensure!(
            k_retry == self.rows["p3"]["k"]
                && rows[0]["q"] == committed[0]["q"]
                && rows[1]["q"] == committed[1]["q"]
                && rows[0]["controls"]["k"] == committed[0]["controls"]["k"],
            "same exact durable selected controls across both real restarts"
        );
        self.identity()?;
        // Includes send, pending proof, old-tail release and early readability;
        // it cannot renew or extend the pre-existing protocol RPC15 timeout.
        let early_started = Instant::now();
        let until = control::interval10(early_started, self.until);
        let _early = s53_deadline::Phase::until(until)?;
        self.controls[0].arm(None)?;
        self.wait(10, |s| {
            Ok(s.controls[1]
                .hit("s53-Committed-before-publication")?
                .then_some(()))
        })?;
        let retry = self.controls[1].facts("Committed", &rows[1]["controls"]["committed"])?;
        ensure!(retry == facts, "same actual Committed retry before B25");
        self.send(1, 25)?;
        let b25 = self.wait(10, |s| {
            let current = s.both()?;
            lease_rows(&current, ["Selected", "Active"], Some(generations))?;
            ensure!(
                current[0]["next"][0] == 25 && current[0]["next"][1] == 16,
                "Selected A remains receive-only before late old-tail repair"
            );
            if current[0]["counts"]["pending"] != 9 {
                return Ok(None);
            }
            let source = early::source_row(&s.view(1, 0)?, 1, 25)?;
            let id = state::parse_id(source["id"].as_str().context("B25 source ID")?)?;
            let observed = s.running[0]
                .as_ref()
                .context("actual Selected A child")?
                .decoder
                .pending
                .contains(&id);
            Ok(observed.then_some(source))
        })?;
        self.controls[1].old["hold_ordinals"] = json!([]);
        self.controls[1].write()?;
        let b16 = self.migration[1]
            .as_ref()
            .context("original B16 witness")?
            .text_id(1, 16)?;
        let request = self.running[1]
            .as_mut()
            .context("same actual B process")?
            .decoder
            .expect_live_retry(generations[1], 16, b16)?;
        self.controls[1].current["phase"] = "mixed".into();
        self.controls[1].current["retry_live"] = request;
        self.controls[1].write()?;
        ensure!(
            self.controls[0].old["suspend_prefix_requests"] == true,
            "A prefix remains suspended during direct retry proof"
        );
        let readable = self.wait(10, |s| {
            let current = s.both()?;
            lease_rows(&current, ["Selected", "Active"], Some(generations))?;
            ensure!(
                current[0]["next"][0] == 25 && current[0]["q"] == rows[0]["q"],
                "Selected A cannot send a new packet while receiving B25"
            );
            if current[0]["next"][1] != 26 || current[0]["counts"]["pending"] != 0 {
                return Ok(None);
            }
            let Some(retransmission)=s.running[1].as_ref().context("actual B Complete observer")?.decoder.live_retry_complete.clone() else {return Ok(None);};
            let received = early::source_row(&s.view(0, 0)?, 1, 25)?;
            ensure!(
                received["id"] == b25["id"] && received["delivered"] == true,
                "actual pending B25 source becomes readable behind original16..24"
            );
            let after = s.both()?;
            lease_rows(&after, ["Selected", "Active"], Some(generations))?;
            Ok(Some(
                json!({"before":current,"after":after,"received":received,"retry_possession":retransmission}),
            ))
        })?;
        self.controls[1].current["retry_live"] = Value::Null;
        self.controls[1].arm(None)?;
        self.wait(10, |s| {
            let active = s.both()?;
            if active.iter().any(|r| r["mode"] != "Active") {
                return Ok(None);
            }
            lease_rows(&active, ["Active", "Active"], Some(generations))?;
            ensure!(
                active[0]["controls"]["committed"] == facts["control_ref"]
                    && active[0]["next"][0] == 25
                    && s.counters[0].encrypt == 0,
                "same Committed activation without a new A packet"
            );
            Ok(Some(()))
        })?;
        let early_elapsed = early_started.elapsed().as_secs_f64();
        drop(_early);
        self.controls[0].old["suspend_prefix_requests"] = false.into();
        self.controls[0].write()?;
        self.wait(30, |s| {
            let rows = s.both()?;
            Ok(rows
                .iter()
                .all(|r| {
                    r["mode"] == "Active"
                        && r["counts"]["queued"] == 0
                        && r["counts"]["pending"] == 0
                        && r["counts"]["debt"] == 0
                        && r["control_debt"] == false
                })
                .then_some(()))
        })?;
        self.identity()?;
        let actual = early::history(&self.view(0, 0)?)?;
        self.old_overlap.extend(
            self.migration[1]
                .as_ref()
                .context("actual B legacy sources")?
                .late_old(&actual)?,
        );
        ensure!(
            self.old_overlap.len() == 17
                && self
                    .old_overlap
                    .iter()
                    .filter_map(|r| r["event"].as_str())
                    .collect::<BTreeSet<_>>()
                    .len()
                    == 17,
            "17 distinct actually source-bound overlapping effects"
        );
        self.frozen
            .as_ref()
            .context("original HVI1 cursor")?
            .verify(&self.view(0, 0)?)?;
        let observed = actual.iter().filter(|r| r["format"] == 1).count()
            + self.original[0]
                .as_ref()
                .context("original A genesis")?
                .packet_effects()
            + self.migration[1]
                .as_ref()
                .context("actual B old sources")?
                .packet_effects();
        ensure!(
            observed == 104,
            "source-observed old/genesis subtotal104, separate B25"
        );
        let b25_copies = usize::from(b25["id"].is_string())
            + usize::from(readable["received"]["id"].is_string());
        ensure!(b25_copies == 2, "actual source/readable B25 effects");
        self.remember("p4",json!({"lost":committed,"lost_elapsed_seconds":lost_elapsed,"early_elapsed_seconds":early_elapsed,"reopened":rows,"committed":facts,"retry":retry,"B25":b25,"early_readable":readable,"old_genesis_subtotal":observed,"chronological_with_B25":observed+b25_copies}))?;
        self.phases.accept(oracle::Phase::Early)
    }
}
