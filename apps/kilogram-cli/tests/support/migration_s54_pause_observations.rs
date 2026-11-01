//! Typed bounded non-Native facts from existing debug hooks.
use super::types::{AckArm, Stamp, fields, id, number};
use super::*;
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Pending {
    pub context: u8,
    pub role: u8,
    pub ordinal: u64,
    pub event: old::Id,
    pub generation: u64,
    pub operation: old::Id,
}
impl Decoder {
    pub fn marker(&self, marker: &Value) -> Result<()> {
        let key = self.key.context("actual live start")?;
        ensure!(
            marker.as_object().is_some_and(|m| m.len() == 6)
                && marker["kind"] == "source64"
                && marker["pid"] == key.pid
                && marker["owner"] == state::hex(&key.owner)
                && marker["incarnation"] == state::hex(&key.nonce)
                && marker["sequence"]
                    .as_u64()
                    .is_some_and(|n| n <= self.sequence)
                && self.source64.as_ref() == Some(&marker["facts"]),
            "same actual decoded source64 marker/process"
        );
        Ok(())
    }
    pub(super) fn context_index(&self, f: &Value) -> Result<u8> {
        self.context(f)?;
        Ok(self
            .contexts
            .iter()
            .position(|c| f["context"] == *c)
            .context("actual context index")? as u8)
    }
    fn bound_head(&self, f: &Value) -> Result<()> {
        let c = self.context_index(f)?;
        let (binding, generation, _) = self.heads.get(&c).context("accepted Head required")?;
        ensure!(
            id(f, "binding")? == *binding && number(f, "lease_generation")? == *generation,
            "pause same observed current binding/generation"
        );
        Ok(())
    }
    pub(super) fn saved_pending(&mut self, f: &Value) -> Result<()> {
        fields(f, 6)?;
        let context = self.context_index(f)?;
        let role: u8 = number(f, "role")?.try_into()?;
        let ordinal = number(f, "ordinal")?;
        let generation = number(f, "generation")?;
        ensure!(
            role == 1 - self.role && (1..=4096).contains(&ordinal) && generation > 0,
            "real pending bounds"
        );
        let row = Pending {
            context,
            role,
            ordinal,
            generation,
            event: id(f, "event")?,
            operation: id(f, "operation")?,
        };
        let key = (context, role, ordinal);
        if let Some(prior) = self.pending_rows.get(&key) {
            ensure!(*prior == row, "contradictory committed pending association");
        } else {
            ensure!(
                self.pending_rows.len() < 4096
                    && !self.pending_rows.values().any(|p| p.event == row.event),
                "bounded unique pending associations"
            );
            if let Some(prior) = self.pending_rows.values().max_by_key(|p| p.generation) {
                ensure!(
                    generation > prior.generation
                        || (generation == prior.generation && row.operation == prior.operation),
                    "stale/contradictory pending commit generation"
                );
            }
            self.pending.insert(row.event);
            self.pending_rows.insert(key, row);
        }
        Ok(())
    }
    pub fn expect_source64(
        &mut self,
        after: u64,
        ids: &[(u64, old::Id)],
        binding: old::Id,
        generation: u64,
    ) -> Result<()> {
        ensure!(
            self.role == 0
                && self.source_expected.is_none()
                && self.source64.is_none()
                && ids.len() == 64
                && ids
                    .iter()
                    .enumerate()
                    .all(|(i, r)| after.checked_add(i as u64 + 1) == Some(r.0))
                && ids.iter().map(|r| r.1).collect::<BTreeSet<_>>().len() == 64,
            "one bounded actual P7 successor vector"
        );
        ensure!(
            self.heads.get(&0) == Some(&(binding, generation, after)),
            "arm from genuinely accepted same Head"
        );
        self.source_expected = Some(
            json!({"context":self.contexts[0],"binding":state::hex(&binding),"lease_generation":generation,"after":after,"first":ids[0].0,"last":ids[63].0,"count":ids.len(),"ids_digest":blake3::hash(&serde_json::to_vec(ids)?).to_hex().to_string()}),
        );
        Ok(())
    }
    pub(super) fn source_pause(&mut self, f: &Value) -> Result<()> {
        fields(f, 8)?;
        self.bound_head(f)?;
        ensure!(
            self.source64.is_none() && self.source_expected.as_ref() == Some(f),
            "actual selection matches armed original source vector/current Head"
        );
        self.source64 = Some(f.clone());
        Ok(())
    }
    pub fn current_head(&self, context: u8) -> Option<(old::Id, u64, u64)> {
        self.heads.get(&context).copied()
    }
    pub fn incarnation(&self) -> Result<oracle::Incarnation> {
        ensure!(!self.final_seen, "live observation incarnation");
        self.key.context("actual start required")
    }
    fn stamp(&self, sequence: u64) -> Result<Stamp> {
        Ok(Stamp {
            incarnation: self.incarnation()?,
            sequence,
            line: self.lines,
        })
    }
    pub(super) fn page_facts(&mut self, kind: &str, f: &Value, sequence: u64) -> Result<()> {
        self.bound_head(f)?;
        let c = self.context_index(f)?;
        let row = PageObservation::parse(f, self.stamp(sequence)?, c)?;
        ensure!(
            kind == match (row.head, row.new_sources > 0) {
                (true, true) => "head_page",
                (true, false) => "replayed_head_page",
                (false, true) => "normal_page",
                (false, false) => "replayed_page",
            },
            "page novelty/head classification"
        );
        if kind == "head_page" && c == 0 {
            self.head_page = Some(row.clone());
            self.first_tail = None;
        }
        if kind == "normal_page" {
            ensure!(row.local_page > 0, "forward normal page number");
            if self.first_tail.is_none()
                && self.head_page.as_ref().is_some_and(|h| {
                    h.context == c
                        && h.binding == row.binding
                        && h.lease_generation == row.lease_generation
                        && h.stamp.line < row.stamp.line
                })
            {
                self.first_tail = Some(row.clone());
            }
            self.normal_pages.insert(c, (row, false));
        }
        Ok(())
    }
    pub(super) fn page_barrier(&mut self, f: &Value, sequence: u64) -> Result<()> {
        self.bound_head(f)?;
        let c = self.context_index(f)?;
        let stamp = self.stamp(sequence)?;
        let (actual, seen) = self
            .normal_pages
            .get_mut(&c)
            .context("barrier requires actual prior forward normal_page")?;
        ensure!(
            actual.facts() == f
                && actual.stamp.incarnation == stamp.incarnation
                && actual.stamp.sequence == sequence
                && actual.stamp.line < stamp.line
                && !*seen,
            "stale/duplicate/foreign page barrier"
        );
        *seen = true;
        let mut row = actual.clone();
        row.stamp = stamp;
        self.page_pause = Some(row);
        Ok(())
    }
    pub fn paused_page(&self) -> Option<&PageObservation> {
        self.page_pause.as_ref()
    }
    pub fn accepted_head_page(&self) -> Option<&PageObservation> {
        self.head_page.as_ref()
    }
    pub fn first_tail_after_head(&self) -> Option<&PageObservation> {
        self.first_tail.as_ref()
    }
    fn pause_marker(&self, marker: &Value, kind: &str, stamp: Stamp, facts: &Value) -> Result<()> {
        let key = self.incarnation()?;
        ensure!(
            serde_json::to_vec(marker)?.len() <= 4096
                && marker.as_object().is_some_and(|m| m.len() == 6)
                && marker["kind"] == kind
                && stamp.incarnation == key
                && marker["pid"] == key.pid
                && marker["owner"] == state::hex(&key.owner)
                && marker["incarnation"] == state::hex(&key.nonce)
                && marker["sequence"] == stamp.sequence
                && stamp.sequence <= self.sequence
                && marker["facts"] == *facts,
            "same actual bounded decoded pause marker"
        );
        self.bound_head(facts)
    }
    pub fn page_marker(&self, marker: &Value) -> Result<()> {
        let row = self
            .page_pause
            .as_ref()
            .context("actual page barrier required")?;
        self.pause_marker(marker, "page", row.stamp, row.facts())
    }
    pub fn arm_ack(
        &mut self,
        context: u8,
        binding: old::Id,
        generation: u64,
        from: Option<u64>,
    ) -> Result<()> {
        ensure!(
            context < 2
                && generation > 0
                && self
                    .heads
                    .get(&context)
                    .is_some_and(|(b, g, _)| *b == binding && *g == generation)
                && from.is_none_or(|n| self.role == 1 && (1..=4096).contains(&n)),
            "ACK arm actual current context/binding/generation/selector"
        );
        self.ack_arm = Some(AckArm {
            stamp: self.stamp(self.sequence)?,
            context,
            binding,
            generation,
            from,
        });
        self.ack_pause = None;
        Ok(())
    }
    pub(super) fn ack(&mut self, f: &Value, sequence: u64) -> Result<()> {
        self.bound_head(f)?;
        let row =
            AckObservation::parse(f, self.stamp(sequence)?, self.context_index(f)?, self.role)?;
        if let Some(arm) = self.ack_arm {
            ensure!(
                arm.stamp.incarnation == row.stamp.incarnation
                    && arm.stamp.line < row.stamp.line
                    && arm.context == row.context
                    && arm.binding == row.binding
                    && arm.generation == row.lease_generation
                    && arm.from == row.from,
                "ACK matches current armed interval"
            );
            ensure!(
                row.from.is_none() || self.ack_pause.is_none(),
                "one targeted ACK per arm"
            );
        } else {
            ensure!(row.from.is_none(), "targeted ACK must be armed");
        }
        ensure!(
            self.last_ack_request
                .is_none_or(|(_, request)| request != row.request),
            "old ACK request cannot satisfy a fresh observation"
        );
        self.last_ack_request = Some((row.binding, row.request));
        self.ack_pause = Some(row);
        Ok(())
    }
    pub fn paused_ack(&self) -> Option<&AckObservation> {
        self.ack_pause.as_ref()
    }
    pub fn ack_marker(&self, marker: &Value) -> Result<()> {
        let row = self
            .ack_pause
            .as_ref()
            .context("actual ACK pause required")?;
        self.pause_marker(marker, "ack", row.stamp, row.facts())
    }
}
