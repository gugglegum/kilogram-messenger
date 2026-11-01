//! Inactive P7 consumes P6 ownership; no separate prefix-PASS test exists.
use super::*;
#[cfg(test)]
#[path = "migration_s54_offline_tests.rs"]
mod tests;
#[path = "migration_s54_offline_witness.rs"]
mod witness;

#[derive(Clone, PartialEq, Eq)]
struct Scope {
    source: old::Id,
    migration: old::Id,
    context: old::Id,
    q: old::Id,
    d: old::Id,
    e: old::Id,
    sid: String,
    identity: old::Id,
}
#[derive(Clone, PartialEq, Eq)]
struct Source {
    ordinal: u64,
    request: [u8; 16],
    id: old::Id,
    previous: old::Id,
    source_hash: old::Id,
    ciphertext_hash: old::Id,
    text_hash: old::Id,
}
#[derive(Clone)]
pub(super) struct Snapshot {
    state: PathBuf,
    scope: Scope,
    anchor: old::Id,
    sources: Vec<Source>,
    native_content: old::Id,
    native_seal: old::Id,
    c2_identity: old::Id,
}
impl Snapshot {
    fn validate(&self) -> Result<()> {
        ensure!(self.sources.len() == 256, "actual offline debt cardinality");
        let mut ids = BTreeSet::new();
        let mut requests = BTreeSet::new();
        let mut previous = self.anchor;
        for (index, row) in self.sources.iter().enumerate() {
            let ordinal = 385 + index as u64;
            let request = oracle::Request::new(0, 0, ordinal.try_into()?)?;
            ensure!(
                row.ordinal == ordinal
                    && row.request == source::parse_request_id(&source::request_id(request))?
                    && row.previous == previous
                    && ids.insert(row.id)
                    && requests.insert(row.request),
                "actual ordered original offline request/source chain"
            );
            ensure!(
                row.text_hash == old::hash(b"kg:od1:request-text\0", request.body().as_bytes()),
                "actual offline body association"
            );
            previous = row.id;
        }
        Ok(())
    }
    fn same(&self, after: &Self) -> Result<()> {
        self.validate()?;
        after.validate()?;
        ensure!(
            self.state == after.state
                && self.scope == after.scope
                && self.anchor == after.anchor
                && self.sources == after.sources
                && self.c2_identity == after.c2_identity,
            "same original state/identity/256 requests/EventIDs/source/ciphertext after real reopen"
        );
        // The randomized outer HPKE seal and unrelated control/lease metadata
        // are not session identity. Compare its authenticated opened content.
        ensure!(
            self.native_content == after.native_content,
            "unresolved Native continuation content change, not inferred new constructor"
        );
        Ok(())
    }
    pub(super) fn source_ids(&self) -> Vec<(u64, old::Id)> {
        self.sources.iter().map(|s| (s.ordinal, s.id)).collect()
    }
    fn report(&self) -> Result<Value> {
        let ids = self.source_ids();
        Ok(
            json!({"source":state::hex(&self.scope.source),"migration":state::hex(&self.scope.migration),"context":state::hex(&self.scope.context),"q":state::hex(&self.scope.q),"debt":ids.len(),"first":ids.first().map(|r|r.0),"last":ids.last().map(|r|r.0),"ids_digest":blake3::hash(&serde_json::to_vec(&ids)?).to_hex().to_string(),"source_cipher_digest":blake3::hash(&serde_json::to_vec(&self.sources.iter().map(|s|(s.ordinal,s.id,s.source_hash,s.ciphertext_hash)).collect::<Vec<_>>())?).to_hex().to_string(),"native_seal_blake3":state::hex(&self.native_seal),"opened_native_content_compared_in_memory":true,"identity_fingerprint":state::hex(&self.scope.identity)}),
        )
    }
}
fn offline_row(row: &Value, own: u64, debt: u64) -> Result<bool> {
    ensure!(
        row["context"].is_string()
            && row["role"] == 0
            && row["mode"] == "Active"
            && row["lease_current"] == false
            && row["lease_generation"].is_null()
            && row["policy_error"].is_null()
            && row["handoff_policy_error"].is_null(),
        "actual Active/current permission without a live A lease"
    );
    ensure!(
        row["next"][1] == 640,
        "B640 still unread, no peer packet while B stopped"
    );
    Ok(row["next"][0] == own
        && row["counts"]["queued"] == 0
        && row["counts"]["pending"] == 0
        && row["counts"]["debt"] == debt)
}
fn no_native_calls(counts: oracle::Counts) -> Result<()> {
    ensure!(
        counts == oracle::Counts::default(),
        "new offline incarnation has no constructor/encrypt/decrypt/attempt effects"
    );
    Ok(())
}
fn identity(s: &mut control::Controller) -> Result<[Value; 2]> {
    let rows = [s.status(0, 0)?, s.status(0, 1)?];
    s.original[0]
        .as_ref()
        .context("actual original two-C witness")?
        .identity(&s.origin.pair.nodes[0], &rows)?;
    ensure!(
        rows[1]["mode"] == "L1"
            && rows[1]["next"] == json!([1, 1])
            && rows[1]["counts"]["queued"] == 0
            && rows[1]["counts"]["pending"] == 0
            && rows[1]["counts"]["debt"] == 0,
        "same C2 remains zero Text"
    );
    Ok(rows)
}
fn reopen(s: &mut control::Controller) -> Result<[Value; 2]> {
    s.stopped(1)?;
    s.start(0)?;
    let rows = s.wait(30, |s| {
        let row = s.status(0, 0)?;
        Ok(offline_row(&row, 641, 256)?.then_some(row))
    })?;
    no_native_calls(s.counters[0])?;
    let both = identity(s)?;
    ensure!(
        both[0]["native_hash"] == rows["native_hash"],
        "quiescent Native observation bracket"
    );
    s.frozen
        .as_ref()
        .context("original real HVI1 cursor")?
        .verify(&s.view(0, 0)?)?;
    let after = identity(s)?;
    ensure!(
        after[0]["native_hash"] == both[0]["native_hash"],
        "actual Native bracket around original frozen cursor read"
    );
    no_native_calls(s.counters[0])?;
    Ok(after)
}

pub(super) struct Continuation {
    pub controller: control::Controller,
    pub held_b640: Value,
    snapshot: Snapshot,
}
impl Continuation {
    pub(super) fn offline_sources(&self) -> Vec<(u64, old::Id)> {
        self.snapshot.source_ids()
    }
}
pub(super) fn p7(input: early::Continuation) -> Result<Continuation> {
    let early::Continuation {
        mut controller,
        held_b640,
    } = input;
    let s = &mut controller;
    let _phase = s53_deadline::Phase::until(s.until)?;
    ensure!(
        s.phases.next == 7 && !s.phases.complete() && s.running.iter().all(Option::is_some),
        "consume owned incomplete P6, not a replacement origin"
    );
    let a = s.status(0, 0)?;
    let b = s.status(1, 0)?;
    ensure!(
        a["next"] == json!([385, 640])
            && b["next"] == json!([385, 641])
            && a["counts"]["queued"] == 0
            && a["counts"]["debt"] == 0
            && b["counts"]["debt"] == 1
            && s.controls[1].current["hold_ordinals"] == json!([640])
            && s.controls[0].current["suspend_prefix"] == true,
        "actual P6 frontiers and retained B640 controls"
    );
    s.stop(1)?;
    s.stopped(1)?;
    ensure!(
        source::debt(&s.view(1, 0)?, oracle::Request::new(0, 1, 640)?)? == held_b640,
        "same actual original B640 source/ciphertext/debt"
    );
    s.wait(30, |s| {
        let row = s.status(0, 0)?;
        Ok((row["lease_current"] == false && row["lease_generation"].is_null()).then_some(()))
    })?;
    let before = identity(s)?;
    ensure!(
        offline_row(&before[0], 385, 0)?,
        "actual initial offline materialized frontier"
    );
    let counter_before = s.counters[0];
    for start in (385u16..=640).step_by(32) {
        s.stopped(1)?;
        let end = (start + 31).min(640);
        for n in start..=end {
            s.send(0, n)?;
        }
        s.wait(30, |s| {
            let row = s.status(0, 0)?;
            Ok(offline_row(&row, u64::from(end) + 1, u64::from(end) - 384)?.then_some(()))
        })?;
    }
    let materialized = identity(s)?;
    ensure!(
        offline_row(&materialized[0], 641, 256)?,
        "real offline source/debt256, not queued256"
    );
    let delta = s.counters[0].delta(counter_before)?;
    ensure!(
        delta.encrypt == 256
            && delta.decrypt == 0
            && delta.attempts == 256
            && delta.aborted == 0
            && delta.ambiguous == 0
            && delta.accounts == 0
            && delta.outbound == 0
            && delta.inbound == 0,
        "actual observed offline Native effects"
    );
    s.stop(0)?;
    s.stopped(0)?;
    s.stopped(1)?;
    let original = witness::capture(&s.view(0, 0)?, &s.origin.contexts, &materialized)?;
    original.validate()?;
    let first_reopen = reopen(s)?;
    s.stop(0)?;
    s.stopped(0)?;
    let after = witness::capture(&s.view(0, 0)?, &s.origin.contexts, &first_reopen)?;
    original.same(&after)?;
    ensure!(
        source::debt(&s.view(1, 0)?, oracle::Request::new(0, 1, 640)?)? == held_b640,
        "stopped B640 witness unchanged after A lifecycle"
    );
    let final_reopen = reopen(s)?;
    ensure!(
        s.running[0].is_some() && s.running[1].is_none(),
        "owned P8 input A running/B stopped"
    );
    s.remember("p7",json!({"before":before,"materialized":materialized,"first_reopen":first_reopen,"final_reopen":final_reopen,"durable_before":original.report()?,"durable_after":after.report()?,"B640":held_b640,"classification":"offline persistence observations; not whole PASS"}))?;
    s.phases.accept(oracle::Phase::Offline)?;
    ensure!(
        s.phases.next == 8 && !s.phases.complete(),
        "owned incomplete8/12 continuation"
    );
    Ok(Continuation {
        controller,
        held_b640,
        snapshot: after,
    })
}
