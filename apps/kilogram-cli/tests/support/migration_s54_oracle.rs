//! Bounded, state-independent S54 expectations. No value here grants persistence.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Phase {
    OldReady,
    Queued,
    Interop,
    Handoff,
    Early,
    Turns,
    Tail,
    Offline,
    Overlap,
    CatchUp,
    Relay,
    Final,
}
pub(super) struct Phases {
    pub(super) next: usize,
}
const ORDER: [Phase; 12] = [
    Phase::OldReady,
    Phase::Queued,
    Phase::Interop,
    Phase::Handoff,
    Phase::Early,
    Phase::Turns,
    Phase::Tail,
    Phase::Offline,
    Phase::Overlap,
    Phase::CatchUp,
    Phase::Relay,
    Phase::Final,
];
impl Phases {
    pub(super) fn accept(&mut self, phase: Phase) -> Result<()> {
        ensure!(
            ORDER.get(self.next) == Some(&phase),
            "S54 continuous phase order"
        );
        self.next += 1;
        Ok(())
    }
    pub(super) fn complete(&self) -> bool {
        self.next == ORDER.len()
    }
}
pub(super) fn old_zero_text_ready(row: &Value) -> Result<bool> {
    let phase = row["phase"].as_u64().context("old phase")?;
    ensure!(phase <= 3, "old phase encoding");
    if phase != 3 {
        return Ok(false);
    }
    ensure!(
        row["session_id"].as_str().is_some_and(|s| !s.is_empty()),
        "actual SID"
    );
    Ok(row["send_next"] == 1
        && row["receive_next"] == 1
        && row["unacked"] == 0
        && row["pending"] == 0)
}
pub(super) fn ready_slot(ready: &Value, context: &str) -> Result<u8> {
    let rows = ready["contexts"].as_array().context("Ready context list")?;
    ensure!(rows.len() == 2, "two Ready contexts");
    let found = rows
        .iter()
        .enumerate()
        .filter(|(_, v)| v["context"] == context)
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    ensure!(found.len() == 1, "unique original Ready context");
    Ok(found[0] as u8)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Request {
    pub(super) context: u8,
    pub(super) role: u8,
    pub(super) number: u16,
}
impl Request {
    pub(super) fn new(context: u8, role: u8, number: u16) -> Result<Self> {
        ensure!(
            context < 2 && role < 2 && number > 0 && number <= if context == 0 { 640 } else { 32 },
            "S54 request scope"
        );
        Ok(Self {
            context,
            role,
            number,
        })
    }
    pub(super) fn numeric(self) -> u64 {
        (u64::from(self.context) << 32) | (u64::from(self.role) << 24) | u64::from(self.number)
    }
    pub(super) fn body(self) -> String {
        format!(
            "P5 C{} role{} #{} / Привет \"\\\n",
            self.context + 1,
            self.role,
            self.number
        )
    }
}
pub(super) fn expected_turn(
    round: u16,
) -> Result<(std::ops::RangeInclusive<u16>, std::ops::RangeInclusive<u16>)> {
    ensure!((0..16).contains(&round), "S54 round");
    Ok((
        25 + 2 * round..=26 + 2 * round,
        25 + 32 * round..=56 + 32 * round,
    ))
}
fn frozen_suffix(before: &[String], consumed: usize, after: &[String]) -> Result<Vec<String>> {
    ensure!(
        consumed <= before.len() && before.len() <= 1344 && after.len() <= 1344,
        "bounded history snapshot"
    );
    ensure!(
        before.iter().collect::<BTreeSet<_>>().len() == before.len(),
        "duplicate old view"
    );
    let current = after.iter().collect::<BTreeSet<_>>();
    ensure!(
        current.len() == after.len() && before.iter().all(|id| current.contains(id)),
        "lost/duplicate history"
    );
    Ok(before[consumed..].to_vec())
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct TextRow {
    id: [u8; 32],
    request: Request,
    format: u8,
    text: String,
    parents: Vec<[u8; 32]>,
}
fn text_history(context: u8, rows: &[TextRow]) -> Result<BTreeMap<Request, TextRow>> {
    ensure!(
        context < 2 && rows.len() <= if context == 0 { 1280 } else { 64 },
        "bounded Text history"
    );
    let mut ids = BTreeSet::new();
    let mut out = BTreeMap::new();
    for row in rows {
        let request = Request::new(row.request.context, row.request.role, row.request.number)?;
        ensure!(
            request.context == context && ids.insert(row.id) && row.parents.len() <= 2,
            "history context/id/parents"
        );
        let format = if context == 0 && request.number >= 25 {
            2
        } else {
            1
        };
        ensure!(
            row.format == format && row.text == request.body(),
            "exact original request/body/format"
        );
        ensure!(
            out.insert(request, row.clone()).is_none(),
            "duplicate request projection"
        );
    }
    Ok(out)
}
fn retained_rows(before: &[TextRow], after: &[TextRow], context: u8) -> Result<()> {
    let old = text_history(context, before)?;
    let new = text_history(context, after)?;
    ensure!(
        old.iter()
            .all(|(request, row)| new.get(request) == Some(row)),
        "old ID/body/parents/projection changed"
    );
    Ok(())
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Counts {
    pub(super) encrypt: u64,
    pub(super) decrypt: u64,
    pub(super) attempts: u64,
    pub(super) aborted: u64,
    pub(super) ambiguous: u64,
    pub(super) accounts: u64,
    pub(super) outbound: u64,
    pub(super) inbound: u64,
}
impl Counts {
    pub(super) fn read(value: &Value) -> Result<Self> {
        let row = value.as_object().context("counter object")?;
        ensure!(row.len() == 8, "exact counter fields");
        let n = |key: &str| -> Result<u64> {
            row.get(key)
                .and_then(Value::as_u64)
                .context("actual integer counter")
        };
        Ok(Self {
            encrypt: n("encrypt")?,
            decrypt: n("decrypt")?,
            attempts: n("attempts")?,
            aborted: n("aborted")?,
            ambiguous: n("ambiguous")?,
            accounts: n("accounts")?,
            outbound: n("outbound")?,
            inbound: n("inbound")?,
        })
    }
    pub(super) fn delta(self, previous: Self) -> Result<Self> {
        Ok(Self {
            encrypt: self
                .encrypt
                .checked_sub(previous.encrypt)
                .context("encrypt regression")?,
            decrypt: self
                .decrypt
                .checked_sub(previous.decrypt)
                .context("decrypt regression")?,
            attempts: self
                .attempts
                .checked_sub(previous.attempts)
                .context("attempt regression")?,
            aborted: self
                .aborted
                .checked_sub(previous.aborted)
                .context("abort regression")?,
            ambiguous: self
                .ambiguous
                .checked_sub(previous.ambiguous)
                .context("ambiguity regression")?,
            accounts: self
                .accounts
                .checked_sub(previous.accounts)
                .context("account counter regression")?,
            outbound: self
                .outbound
                .checked_sub(previous.outbound)
                .context("outbound counter regression")?,
            inbound: self
                .inbound
                .checked_sub(previous.inbound)
                .context("inbound counter regression")?,
        })
    }
    pub(super) fn resolved(self) -> bool {
        self.ambiguous == 0
            && self
                .encrypt
                .checked_add(self.decrypt)
                .and_then(|n| n.checked_add(self.aborted))
                == Some(self.attempts)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Incarnation {
    pub(super) owner: [u8; 32],
    pub(super) pid: u32,
    pub(super) nonce: [u8; 32],
}
struct Stream {
    sequence: u64,
    counts: Counts,
    closed: bool,
}
pub(super) struct ClosedStream {
    pub key: Incarnation,
    pub sequence: u64,
    pub counts: Counts,
}
pub(super) struct ClosedSummary {
    pub counts: Counts,
    pub streams: Vec<ClosedStream>,
    pub operations: Vec<([u8; 32], [u8; 32])>,
    pub generations: Vec<([u8; 32], u64)>,
}
#[derive(Default)]
pub(super) struct Operations {
    streams: BTreeMap<Incarnation, Stream>,
    operations: BTreeSet<([u8; 32], [u8; 32])>,
    generations: BTreeMap<[u8; 32], u64>,
}
impl Operations {
    pub(super) fn closed(&self, key: Incarnation, sequence: u64, counts: Counts) -> Result<()> {
        let stream = self
            .streams
            .get(&key)
            .context("known normal final stream")?;
        ensure!(
            stream.closed
                && stream.sequence == sequence
                && stream.counts == counts
                && counts.resolved(),
            "actual exactly-once closed stream/final counters"
        );
        Ok(())
    }
    pub(super) fn summary(&self) -> Result<ClosedSummary> {
        ensure!(
            !self.streams.is_empty() && self.streams.len() <= 32 && self.operations.len() <= 4096,
            "bounded nonempty closed ledger"
        );
        let mut counts = Counts::default();
        let mut streams = Vec::with_capacity(self.streams.len());
        for (key, stream) in &self.streams {
            self.closed(*key, stream.sequence, stream.counts)?;
            let c = stream.counts;
            ensure!(
                c.accounts == 0 && c.outbound == 0 && c.inbound == 0,
                "closed current constructor observations"
            );
            counts = Counts {
                encrypt: counts
                    .encrypt
                    .checked_add(c.encrypt)
                    .context("closed encrypt overflow")?,
                decrypt: counts
                    .decrypt
                    .checked_add(c.decrypt)
                    .context("closed decrypt overflow")?,
                attempts: counts
                    .attempts
                    .checked_add(c.attempts)
                    .context("closed attempts overflow")?,
                aborted: counts
                    .aborted
                    .checked_add(c.aborted)
                    .context("closed abort overflow")?,
                ambiguous: counts
                    .ambiguous
                    .checked_add(c.ambiguous)
                    .context("closed ambiguity overflow")?,
                accounts: counts
                    .accounts
                    .checked_add(c.accounts)
                    .context("closed accounts overflow")?,
                outbound: counts
                    .outbound
                    .checked_add(c.outbound)
                    .context("closed outbound overflow")?,
                inbound: counts
                    .inbound
                    .checked_add(c.inbound)
                    .context("closed inbound overflow")?,
            };
            streams.push(ClosedStream {
                key: *key,
                sequence: stream.sequence,
                counts: c,
            });
        }
        ensure!(counts.resolved(), "resolved closed sum");
        Ok(ClosedSummary {
            counts,
            streams,
            operations: self.operations.iter().copied().collect(),
            generations: self.generations.iter().map(|(o, g)| (*o, *g)).collect(),
        })
    }
    pub(super) fn start(&mut self, key: Incarnation) -> Result<()> {
        ensure!(
            self.streams.len() < 32
                && key.pid > 0
                && key.nonce != [0; 32]
                && !self.streams.contains_key(&key),
            "S54 incarnation bound/identity"
        );
        ensure!(
            !self
                .streams
                .iter()
                .any(|(k, s)| k.owner == key.owner && !s.closed),
            "overlapping owner incarnation"
        );
        ensure!(
            !self.streams.keys().any(|k| k.nonce == key.nonce),
            "reused incarnation nonce"
        );
        self.streams.insert(
            key,
            Stream {
                sequence: 0,
                counts: Counts::default(),
                closed: false,
            },
        );
        Ok(())
    }
    pub(super) fn commit(
        &mut self,
        key: Incarnation,
        sequence: u64,
        generation: u64,
        operation: [u8; 32],
        counts: Counts,
    ) -> Result<()> {
        ensure!(
            counts.accounts == 0 && counts.outbound == 0 && counts.inbound == 0,
            "P5 current owner must not reconstruct origin"
        );
        let s = self.streams.get(&key).context("unknown incarnation")?;
        ensure!(
            !s.closed && sequence == s.sequence + 1 && self.operations.len() < 4096,
            "S54 sequence/observation bound"
        );
        ensure!(
            !self.operations.contains(&(key.owner, operation))
                && generation > self.generations.get(&key.owner).copied().unwrap_or(0),
            "overlapping/repeated operation"
        );
        let delta = counts.delta(s.counts)?;
        let calls = delta
            .encrypt
            .checked_add(delta.decrypt)
            .context("Native count overflow")?;
        let successful = counts
            .encrypt
            .checked_add(counts.decrypt)
            .context("Native total overflow")?;
        ensure!(
            calls > 0 && calls <= 4 && counts.attempts >= successful,
            "actual bounded Native commit"
        );
        let s = self.streams.get_mut(&key).context("incarnation")?;
        s.sequence = sequence;
        s.counts = counts;
        self.operations.insert((key.owner, operation));
        self.generations.insert(key.owner, generation);
        Ok(())
    }
    pub(super) fn finish(&mut self, key: Incarnation, observed: Counts) -> Result<()> {
        let s = self
            .streams
            .get_mut(&key)
            .context("unknown final incarnation")?;
        observed.delta(s.counts)?;
        ensure!(
            observed.accounts == 0 && observed.outbound == 0 && observed.inbound == 0,
            "final constructors changed"
        );
        ensure!(
            !s.closed
                && observed.encrypt == s.counts.encrypt
                && observed.decrypt == s.counts.decrypt
                && observed.resolved(),
            "unresolved/missing final Native outcomes"
        );
        s.counts = observed;
        s.closed = true;
        Ok(())
    }
    pub(super) fn reconcile_final(
        &mut self,
        key: Incarnation,
        generation: u64,
        operation: [u8; 32],
        durable_before: (u64, u64),
        durable_after: (u64, u64),
        final_counts: Counts,
    ) -> Result<()> {
        let stream = self
            .streams
            .get(&key)
            .context("missing reconciled incarnation")?;
        reconcile_sources(durable_before, durable_after, stream.counts, final_counts)?;
        if final_counts.encrypt != stream.counts.encrypt
            || final_counts.decrypt != stream.counts.decrypt
        {
            let sequence = stream.sequence + 1;
            self.commit(key, sequence, generation, operation, final_counts)?;
        }
        self.finish(key, final_counts)
    }
    pub(super) fn complete(&self) -> Result<(u64, u64)> {
        ensure!(
            !self.streams.is_empty() && self.streams.values().all(|s| s.closed),
            "missing final process outcomes"
        );
        self.streams.values().try_fold((0u64, 0u64), |(a, b), s| {
            Ok((
                a.checked_add(s.counts.encrypt)
                    .context("encrypt total overflow")?,
                b.checked_add(s.counts.decrypt)
                    .context("decrypt total overflow")?,
            ))
        })
    }
}
// Source/outcome reconciliation never infers a missing success from elapsed time.
fn reconcile_sources(
    before: (u64, u64),
    after: (u64, u64),
    observed: Counts,
    final_counts: Counts,
) -> Result<()> {
    let delta = final_counts.delta(observed)?;
    ensure!(
        after.0.checked_sub(before.0) == Some(delta.encrypt)
            && after.1.checked_sub(before.1) == Some(delta.decrypt)
            && final_counts.resolved(),
        "source/readable/outcome mismatch"
    );
    Ok(())
}
fn whole_packet_total(
    old_genesis_subtotal: u64,
    current_old_source_effects: u64,
    current: &Operations,
) -> Result<u64> {
    let (encrypt, decrypt) = current.complete()?;
    let observed = encrypt
        .checked_add(decrypt)
        .context("current packet total overflow")?;
    ensure!(
        current_old_source_effects <= observed,
        "old/current overlap lacks actual current operations"
    );
    old_genesis_subtotal
        .checked_sub(current_old_source_effects)
        .and_then(|old| old.checked_add(observed))
        .context("invalid old/current overlap or packet total overflow")
}
#[derive(Default)]
pub(super) struct Pages {
    total: u64,
    owner_totals: BTreeMap<old::Id, u64>,
    current: BTreeMap<old::Id, Incarnation>,
    last_local: BTreeMap<Incarnation, u64>,
    operations: BTreeSet<([u8; 32], [u8; 32])>,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct PageBaseline {
    owner: old::Id,
    total: u64,
}
impl Pages {
    fn register(&mut self, key: Incarnation, new_start: bool) -> Result<()> {
        ensure!(
            key.pid > 0 && key.owner != [0; 32] && key.nonce != [0; 32],
            "actual page owner/incarnation identity"
        );
        if self.last_local.contains_key(&key) {
            ensure!(!new_start, "reused page incarnation start");
            return Ok(());
        }
        ensure!(
            self.last_local.len() < 32 && !self.last_local.keys().any(|k| k.nonce == key.nonce),
            "bounded unique page nonce/source/PID"
        );
        self.last_local.insert(key, 0);
        self.current.insert(key.owner, key);
        self.owner_totals.entry(key.owner).or_insert(0);
        Ok(())
    }
    pub(super) fn start(&mut self, key: Incarnation) -> Result<()> {
        self.register(key, true)
    }
    fn known(&self, key: Incarnation) -> Result<u64> {
        ensure!(
            self.last_local.contains_key(&key) && self.current.get(&key.owner) == Some(&key),
            "unknown page incarnation"
        );
        self.owner_totals
            .get(&key.owner)
            .copied()
            .context("unknown page source")
    }
    pub(super) fn owner_total(&self, key: Incarnation) -> Result<u64> {
        self.known(key)
    }
    pub(super) fn baseline(&self, key: Incarnation) -> Result<PageBaseline> {
        Ok(PageBaseline {
            owner: key.owner,
            total: self.known(key)?,
        })
    }
    pub(super) fn phase_total(&self, key: Incarnation, base: PageBaseline) -> Result<u64> {
        ensure!(base.owner == key.owner, "page baseline source mismatch");
        self.known(key)?
            .checked_sub(base.total)
            .context("page baseline regression")
    }
    pub(super) fn phase_local_target(
        &self,
        key: Incarnation,
        base: PageBaseline,
        target: u64,
    ) -> Result<u64> {
        let progress = self.phase_total(key, base)?;
        ensure!(
            target > progress
                && target <= 256
                && base.total.checked_add(target).is_some_and(|n| n <= 256),
            "bounded future owner phase target"
        );
        target
            .checked_sub(progress)
            .and_then(|left| left.checked_add(*self.last_local.get(&key)?))
            .context("local page target overflow")
    }
    pub(super) fn accepted(
        &mut self,
        key: Incarnation,
        local: u64,
        operation: [u8; 32],
        head: bool,
        new_sources: u64,
    ) -> Result<()> {
        if head || new_sources == 0 {
            return Ok(());
        }
        ensure!(
            new_sources <= 16
                && self.total < 256
                && local == self.last_local.get(&key).copied().unwrap_or(0) + 1,
            "S54 normal page ordering"
        );
        ensure!(
            !self.operations.contains(&(key.owner, operation)),
            "replayed page cannot advance interruption"
        );
        self.register(key, false)?;
        let next = self
            .known(key)?
            .checked_add(1)
            .context("owner page total overflow")?;
        self.operations.insert((key.owner, operation));
        self.owner_totals.insert(key.owner, next);
        self.last_local.insert(key, local);
        self.total += 1;
        Ok(())
    }
    pub(super) fn local_target(&self, key: Incarnation, target: u64) -> Result<u64> {
        ensure!(
            target > self.total && target <= 256,
            "interruption target already passed"
        );
        Ok(target - self.total + self.last_local.get(&key).copied().unwrap_or(0))
    }
}

#[cfg(test)]
#[path = "migration_s54_oracle_tests.rs"]
mod tests;
