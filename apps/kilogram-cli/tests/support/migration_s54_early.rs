//! P0-P2 and P5-P6 of one owned run; no executable partial-PASS selector.
use super::*;
use control::Controller;

enum RetainedMigration {
    Legacy(source::Legacy),
    P(source::boundary::Checked),
}
impl RetainedMigration {
    fn witness(&self) -> &source::Legacy {
        match self {
            Self::Legacy(w) => w,
            Self::P(p) => p.witness(),
        }
    }
    fn complete(self, node: &Node, ready: &Value) -> Result<(source::Legacy, intervals::Endpoint)> {
        match self {
            Self::Legacy(w) => {
                let endpoint = w.after_migration(node, ready)?;
                Ok((w, endpoint))
            }
            Self::P(p) => {
                let endpoint = p.after_migration(node, ready)?;
                Ok((p.into_witness(), endpoint))
            }
        }
    }
}

pub(super) struct Frozen {
    pub cursor: String,
    view: Value,
    frontiers: Value,
    suffix: Vec<Value>,
}
fn page_rows(page: &Value) -> Result<&Vec<Value>> {
    page["items"].as_array().context("actual HVI1 rows")
}
pub(super) fn history(node: &Node) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    let mut cursor = None;
    let mut seen = BTreeSet::new();
    let mut stamp = None;
    for _ in 0..43 {
        s53_deadline::check()?;
        let page = node.history(32, cursor.as_deref())?;
        ensure!(
            serde_json::to_vec(&page)?.len() <= 48 * 1024,
            "bounded actual history page"
        );
        let identity = (page["view"].clone(), page["F"].clone());
        if let Some(previous) = &stamp {
            ensure!(
                previous == &identity,
                "same actual frozen history view/frontiers"
            );
        } else {
            stamp = Some(identity);
        }
        out.extend(page_rows(&page)?.iter().cloned());
        ensure!(out.len() <= 1344, "bounded continuous history");
        if page["finished"] == true {
            return Ok(out);
        }
        let next = page["cursor"]
            .as_str()
            .context("real HVI1 cursor")?
            .to_owned();
        ensure!(
            seen.insert(next.clone()) && !page_rows(&page)?.is_empty(),
            "real HVI1 cursor progress"
        );
        cursor = Some(next);
    }
    anyhow::bail!("bounded complete history exceeded")
}
impl Frozen {
    pub fn capture(node: &Node) -> Result<Self> {
        let first = node.history(16, None)?;
        Self::from_pages(&first, |cursor| node.history(32, Some(cursor)))
    }
    pub fn from_pages(first: &Value, read: impl FnMut(&str) -> Result<Value>) -> Result<Self> {
        ensure!(
            page_rows(first)?.len() == 16 && first["finished"] == false,
            "real first16 of39"
        );
        let mut value = Self {
            cursor: first["cursor"]
                .as_str()
                .context("real cursor after16")?
                .into(),
            view: first["view"].clone(),
            frontiers: first["F"].clone(),
            suffix: vec![],
        };
        let suffix = value.read_with(read)?;
        ensure!(suffix.len() == 23, "actual frozen suffix39-16=23");
        value.suffix = suffix;
        Ok(value)
    }
    fn read_with(&self, mut read: impl FnMut(&str) -> Result<Value>) -> Result<Vec<Value>> {
        let mut cursor = self.cursor.clone();
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        for _ in 0..3 {
            let page = read(&cursor)?;
            ensure!(
                page["view"] == self.view && page["F"] == self.frontiers,
                "actual cursor remains bound to original frozen view/frontiers"
            );
            for row in page_rows(&page)? {
                let map = source::history_map(std::slice::from_ref(row))?;
                out.push(map.into_values().next().context("actual frozen row")?);
            }
            if page["finished"] == true {
                return Ok(out);
            }
            let next = page["cursor"]
                .as_str()
                .context("continued actual cursor")?
                .to_owned();
            ensure!(
                seen.insert(next.clone()) && !page_rows(&page)?.is_empty(),
                "bounded actual suffix progress"
            );
            cursor = next;
        }
        anyhow::bail!("frozen suffix bounds")
    }
    pub fn verify_with(&self, read: impl FnMut(&str) -> Result<Value>) -> Result<()> {
        ensure!(
            self.read_with(read)? == self.suffix,
            "actual resumed cursor returns original ordered23 sources"
        );
        Ok(())
    }
    pub fn verify(&self, node: &Node) -> Result<()> {
        self.verify_with(|cursor| node.history(32, Some(cursor)))
    }
}
pub(super) fn source_row(node: &Node, role: usize, number: u16) -> Result<Value> {
    let request = oracle::Request::new(0, role as u8, number)?;
    let rows = history(node)?;
    let found = rows
        .iter()
        .filter(|r| r["role"] == role as u64 && r["request"] == source::request_id(request))
        .collect::<Vec<_>>();
    ensure!(found.len() == 1, "exactly one actual materialized request");
    let row = found[0].clone();
    ensure!(
        row["ordinal"] == u64::from(number)
            && row["text"] == request.body()
            && row["format"] == if number < 25 { 1 } else { 2 },
        "actual original request/body/format/ordinal"
    );
    Ok(row)
}
impl Controller {
    pub fn p0(&mut self) -> Result<()> {
        self.start(0)?;
        self.start(1)?;
        self.import1()?;
        for context in 0..2 {
            for owner in 0..2 {
                self.wait(30, |s| {
                    let row = s.status(owner, context)?;
                    Ok(oracle::old_zero_text_ready(&row)?.then_some(()))
                })?;
            }
        }
        // Acquire the original private witnesses only after both real waits.
        self.stop(1)?;
        self.stop(0)?;
        for owner in 0..2 {
            self.stopped(owner)?;
            let witness =
                source::Legacy::capture(&self.origin.pair.nodes[owner], &self.origin.contexts)?;
            witness.role(owner)?;
            self.remember(&format!("genesis-{owner}"), witness.report())?;
            self.original[owner] = Some(witness);
        }
        self.start(0)?;
        self.start(1)?;
        self.import1()?;
        for round in 0..8u16 {
            for number in (1 + round * 2)..=(2 + round * 2) {
                self.send(0, number)?;
            }
            let a = 3 + u64::from(round) * 2;
            self.wait(30, |s| {
                let r = s.status(1, 0)?;
                Ok((r["receive_next"] == a && r["pending"] == 0).then_some(()))
            })?;
            for number in (1 + round * 2)..=(2 + round * 2) {
                self.send(1, number)?;
            }
            let b = if round == 7 { 16 } else { a };
            self.wait(30, |s| {
                let r = s.status(0, 0)?;
                Ok((r["receive_next"] == b && r["send_next"] == a).then_some(()))
            })?;
        }
        self.wait(30, |s| {
            let a = s.status(0, 0)?;
            let b = s.status(1, 0)?;
            Ok((a["unacked"] == 0 && b["send_next"] == 17 && b["unacked"] == 1).then_some(()))
        })?;
        let a = self.status(0, 0)?;
        let b = self.status(1, 0)?;
        ensure!(
            a["send_next"] == 17
                && a["receive_next"] == 16
                && b["send_next"] == 17
                && b["receive_next"] == 17
                && b["unacked"] == 1,
            "P0 actual committed B16 gap"
        );
        self.remember(
            "p0",
            json!({"a":a,"b":b,"expected_old_genesis_subtotal":71}),
        )?;
        self.phases.accept(oracle::Phase::OldReady)
    }
    pub fn migrate(&mut self, owner: usize) -> Result<()> {
        let _measurement = s53_deadline::measure("migration");
        self.stopped(owner)?;
        let node = &self.origin.pair.nodes[owner];
        let witness = source::Legacy::capture(node, &self.origin.contexts)?;
        self.migrate_retained(owner, RetainedMigration::Legacy(witness))
    }
    pub(super) fn migrate_p(&mut self, owner: usize, phase: source::boundary::Phase) -> Result<()> {
        let _measurement = s53_deadline::measure("migration");
        self.stopped(owner)?;
        let proof = source::Legacy::capture_boundary(
            &self.origin.pair.nodes[owner],
            &self.origin.contexts,
        )?;
        let checked = proof.prepare(phase, owner, &self.origin.contexts)?;
        self.migrate_retained(owner, RetainedMigration::P(checked))
    }
    fn migrate_retained(&mut self, owner: usize, retained: RetainedMigration) -> Result<()> {
        self.stopped(owner)?;
        let node = &self.origin.pair.nodes[owner];
        let witness = retained.witness();
        self.original[owner]
            .as_ref()
            .context("original enrollment witness")?
            .same_identity(witness)?;
        let preview = json_cli(&current(), &node.branch.upgrade(true))?;
        ensure!(
            preview["source_id"] == state::hex(&witness.source_id),
            "check binds actual original source"
        );
        let ready = self.origin.migrate_stopped(owner)?;
        let (witness, endpoint) = retained.complete(&self.origin.pair.nodes[owner], &ready)?;
        self.accounting
            .migration(&endpoint, witness.accounting_facts()?)?;
        self.intervals.migration(owner as u8, endpoint)?;
        self.remember(
            &format!("migration-{owner}"),
            json!({"preview":preview,"ready":ready,"original":witness.report()}),
        )?;
        self.rows.insert(format!("ready-{owner}"), ready);
        self.migration[owner] = Some(witness);
        self.migration_sources().bind_memory(owner)
    }
    pub fn p1(&mut self) -> Result<()> {
        self.stop(1)?;
        self.stopped(1)?;
        let stopped_b = source::Legacy::capture(&self.origin.pair.nodes[1], &self.origin.contexts)?;
        self.original[1]
            .as_ref()
            .context("original B")?
            .same_identity(&stopped_b)?;
        self.wait(30, |s| {
            let r = s.status(0, 0)?;
            Ok(r["status"]
                .as_str()
                .is_some_and(|v| v.starts_with("ConnectionBackoff"))
                .then_some(()))
        })?;
        let before = self.status(0, 0)?;
        let a17 = self.send(0, 17)?;
        let a18 = self.send(0, 18)?;
        let after = self.status(0, 0)?;
        ensure!(
            before["send_next"] == 17 && after["send_next"] == 17 && after["receive_next"] == 16,
            "original17/18 accepted without packet creation"
        );
        self.stop(0)?;
        self.migrate_p(0, source::boundary::Phase::FirstA)?;
        let observed = self.migration[0]
            .as_ref()
            .context("actual original A sources")?
            .packet_effects()
            + stopped_b.packet_effects();
        ensure!(
            observed == 71,
            "source-observed old/genesis subtotal71 before packet17/18"
        );
        let mut facts = self.migration[0]
            .as_ref()
            .context("original A migration")?
            .accounting_facts()?;
        facts.extend(stopped_b.accounting_facts()?);
        ensure!(
            self.accounting
                .capture(accounting::Stage::P0, facts)?
                .calls()?
                == u64::try_from(observed)?,
            "typed observed71 effects"
        );
        self.remember(
            "p0-source-subtotal",
            json!({"old_genesis_subtotal":observed,"b":stopped_b.report()}),
        )?;
        self.controls[0].old["suspend_materialization"] = true.into();
        self.controls[0].write()?;
        self.start(0)?;
        let rows = history(&self.view(0, 0)?)?;
        ensure!(rows.len() == 31, "first indexed readable31");
        self.migration[0]
            .as_ref()
            .context("A source witness")?
            .first_history(&rows)?;
        self.remember("p1",json!({"queue17":a17,"queue18":a18,"before":before,"after":after,"first_indexed_text":rows.len()}))?;
        self.phases.accept(oracle::Phase::Queued)
    }
    pub fn p2(&mut self) -> Result<()> {
        self.start(1)?;
        self.import1()?;
        self.controls[0].old["suspend_materialization"] = false.into();
        self.controls[0].write()?;
        self.wait(30, |s| {
            let a = s.status(0, 0)?;
            let b = s.status(1, 0)?;
            Ok((a["next"] == json!([19, 16]) && b["receive_next"] == 19).then_some(()))
        })?;
        for n in 19..=23 {
            self.send(0, n)?;
        }
        for n in 17..=23 {
            self.send(1, n)?;
        }
        self.wait(30, |s| {
            let a = s.status(0, 0)?;
            let b = s.status(1, 0)?;
            Ok((a["next"] == json!([24, 16])
                && a["counts"]["debt"] == 0
                && b["receive_next"] == 24
                && b["send_next"] == 24)
                .then_some(()))
        })?;
        // Current A is the sole ALPN1 ACK-query initiator for its own debt;
        // frozen B's disable_ack suppresses its saved-receipt push. Data remains real.
        self.controls[0].old["disable_ack"] = true.into();
        self.controls[1].old["disable_ack"] = true.into();
        for c in &self.controls {
            c.write()?;
        }
        self.send(0, 24)?;
        self.send(1, 24)?;
        self.wait(30, |s| {
            let a = s.status(0, 0)?;
            let b = s.status(1, 0)?;
            Ok((a["next"] == json!([25, 16])
                && a["counts"]["debt"] == 1
                && a["counts"]["pending"] == 8
                && b["receive_next"] == 25
                && b["send_next"] == 25)
                .then_some(()))
        })?;
        let request = source_row(&self.view(0, 0)?, 0, 24)?;
        ensure!(
            request["delivered"] == false,
            "actual A24 receipt still withheld"
        );
        self.frozen = Some(Frozen::capture(&self.view(0, 0)?)?);
        let a = self.status(0, 0)?;
        let b = self.status(1, 0)?;
        self.remember(
            "p2",
            json!({"a":a,"b":b,"A24":request,"expected_old_genesis_subtotal":95}),
        )?;
        self.phases.accept(oracle::Phase::Interop)
    }
    pub fn p5(&mut self) -> Result<()> {
        let b25 = self.rows["p4"]["B25"].clone();
        for round in 0..16 {
            let (a, b) = oracle::expected_turn(round)?;
            let a_end = u64::from(*a.end()) + 1;
            let b_end = u64::from(*b.end()) + 1;
            for n in a {
                self.send(0, n)?;
            }
            self.wait(30, |s| {
                let r = s.status(1, 0)?;
                Ok((r["next"][0] == a_end && r["counts"]["pending"] == 0).then_some(()))
            })?;
            for n in b {
                self.send(1, n)?;
            }
            self.wait(30, |s| {
                let r = s.status(0, 0)?;
                Ok((r["next"][1] == b_end && r["counts"]["pending"] == 0).then_some(()))
            })?;
        }
        let a = self.status(0, 0)?;
        let b = self.status(1, 0)?;
        ensure!(
            a["next"] == json!([57, 537])
                && b["next"] == json!([57, 537])
                && [a.clone(), b.clone()].iter().all(|r| {
                    r["native"]["verification_guard"]["peer_g"]
                        .as_u64()
                        .is_some_and(|g| g >= 16)
                }),
            "16 actual causal receiving DH generations and56/536 frontier"
        );
        ensure!(
            source_row(&self.view(1, 0)?, 1, 25)?["id"] == b25["id"],
            "idempotent B25 exact original source"
        );
        self.remember("p5", json!({"a":a,"b":b,"B25":b25}))?;
        self.phases.accept(oracle::Phase::Turns)
    }
    pub fn p6(&mut self) -> Result<Value> {
        for start in (57u16..=384).step_by(32) {
            let end = (start + 31).min(384);
            for n in start..=end {
                self.send(0, n)?;
            }
            self.wait(30, |s| {
                let r = s.status(1, 0)?;
                Ok((r["next"][0] == u64::from(end) + 1).then_some(()))
            })?;
        }
        self.controls[0].current["phase"] = "mixed".into();
        self.controls[0].current["suspend_prefix"] = true.into();
        self.controls[1].current["phase"] = "mixed".into();
        self.controls[1].current["hold_ordinals"] = json!([640]);
        for c in &self.controls {
            c.write()?;
        }
        for start in (537u16..=640).step_by(32) {
            let end = (start + 31).min(640);
            for n in start..=end {
                self.send(1, n)?;
            }
            let expected = if end == 640 { 640 } else { u64::from(end) + 1 };
            self.wait(30, |s| {
                let r = s.status(0, 0)?;
                Ok((r["next"][1] == expected).then_some(()))
            })?;
        }
        self.wait(30, |s| {
            let a = s.status(0, 0)?;
            let b = s.status(1, 0)?;
            Ok((a["next"] == json!([385, 640])
                && b["next"] == json!([385, 641])
                && a["counts"]["debt"] == 0
                && b["counts"]["debt"] == 1
                && b["counts"]["queued"] == 0)
                .then_some(()))
        })?;
        self.stop(1)?;
        self.stopped(1)?;
        let request = oracle::Request::new(0, 1, 640)?;
        let witness = source::debt(&self.view(1, 0)?, request)?;
        self.start(1)?;
        self.import2()?;
        for owner in 0..2 {
            let c2 = self.status(owner, 1)?;
            ensure!(
                c2["mode"] == "L1"
                    && c2["next"] == json!([1, 1])
                    && c2["counts"]["queued"] == 0
                    && c2["counts"]["pending"] == 0
                    && c2["counts"]["debt"] == 0,
                "same C2 still zero Text"
            );
        }
        self.frozen
            .as_ref()
            .context("original cursor")?
            .verify(&self.view(0, 0)?)?;
        let a = self.status(0, 0)?;
        let b = self.status(1, 0)?;
        self.remember("p6", json!({"a":a,"b":b,"B640":witness}))?;
        self.phases.accept(oracle::Phase::Tail)?;
        Ok(witness)
    }
}

// Owned future continuation; no serialization or substitute checkpoint exists.
pub(super) struct Continuation {
    pub controller: Controller,
    pub held_b640: Value,
}
pub(super) fn p0_to_p6(parent: &Path, until: Instant) -> Result<Continuation> {
    let _phase = s53_deadline::Phase::until(until)?;
    let mut s = Controller::new(parent, until)?;
    s.p0()?;
    s.p1()?;
    s.p2()?;
    s.p3()?;
    s.p4()?;
    s.p5()?;
    let held_b640 = s.p6()?;
    ensure!(
        s.phases.next == 7 && !s.phases.complete(),
        "explicit block3 continuation, never whole PASS"
    );
    Ok(Continuation {
        controller: s,
        held_b640,
    })
}
