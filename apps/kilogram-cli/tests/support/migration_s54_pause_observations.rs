//! Typed bounded non-Native facts from existing debug hooks.
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
fn number(f: &Value, k: &str) -> Result<u64> {
    f[k].as_u64().context("actual observation integer")
}
fn id(f: &Value, k: &str) -> Result<old::Id> {
    state::parse_id(f[k].as_str().context("actual observation ID")?)
}
fn fields(f: &Value, n: usize) -> Result<()> {
    ensure!(
        f.as_object().is_some_and(|o| o.len() == n),
        "exact bounded fact schema"
    );
    Ok(())
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
    pub(super) fn page_facts(&mut self, kind: &str, f: &Value) -> Result<()> {
        fields(f, 11)?;
        self.bound_head(f)?;
        let start = number(f, "start")?;
        let end = number(f, "end")?;
        let new = number(f, "new_sources")?;
        let head = f["head"]
            .as_bool()
            .context("explicit head classification")?;
        ensure!(
            start > 0
                && end > start
                && end - start <= 16
                && new <= end - start
                && (!head || end - start == 1)
                && number(f, "generation")? > 0
                && number(f, "lease_generation")? > 0,
            "actual bounded page"
        );
        id(f, "operation")?;
        if !f["grant"].is_null() {
            id(f, "grant")?;
        }
        ensure!(
            kind == match (head, new > 0) {
                (true, true) => "head_page",
                (true, false) => "replayed_head_page",
                (false, true) => "normal_page",
                (false, false) => "replayed_page",
            },
            "page novelty/head classification"
        );
        if kind == "normal_page" {
            let c = self.context_index(f)?;
            self.normal_pages.insert(c, (f.clone(), false));
        }
        Ok(())
    }
    pub(super) fn page_barrier(&mut self, f: &Value) -> Result<()> {
        self.bound_head(f)?;
        let c = self.context_index(f)?;
        let (actual, seen) = self
            .normal_pages
            .get_mut(&c)
            .context("barrier requires actual prior forward normal_page")?;
        ensure!(
            actual == f && !*seen,
            "stale/duplicate/foreign page barrier"
        );
        *seen = true;
        Ok(())
    }
    pub(super) fn ack(&mut self, f: &Value) -> Result<()> {
        fields(f, 7)?;
        self.bound_head(f)?;
        ensure!(
            (1..=16).contains(&number(f, "receipt_count")?) && f["query_reply"].is_boolean(),
            "actual nonempty bounded ACK push/query reply"
        );
        source::parse_request_id(f["request"].as_str().context("actual ACK request")?)?;
        id(f, "saved_receipts_digest")?;
        self.ack_pause = Some(f.clone());
        Ok(())
    }
}
