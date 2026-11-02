//! Current accounting inputs only; no old/terminal union or whole-run result.
use super::*;
#[cfg(test)]
#[path = "migration_s54_intervals_tests.rs"]
mod tests;
const LIFETIMES: usize = 32;
const OPERATIONS: usize = 4096;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Work {
    pub encrypt: u64,
    pub decrypt: u64,
}
impl Work {
    fn add(self, b: Self) -> Result<Self> {
        Ok(Self {
            encrypt: self
                .encrypt
                .checked_add(b.encrypt)
                .context("interval encrypt overflow")?,
            decrypt: self
                .decrypt
                .checked_add(b.decrypt)
                .context("interval decrypt overflow")?,
        })
    }
    fn sub(self, b: Self) -> Result<Self> {
        Ok(Self {
            encrypt: self
                .encrypt
                .checked_sub(b.encrypt)
                .context("negative interval encrypt tail")?,
            decrypt: self
                .decrypt
                .checked_sub(b.decrypt)
                .context("negative interval decrypt tail")?,
        })
    }
    fn calls(self) -> Result<u64> {
        self.encrypt
            .checked_add(self.decrypt)
            .context("interval calls overflow")
    }
    fn of(c: oracle::Counts) -> Self {
        Self {
            encrypt: c.encrypt,
            decrypt: c.decrypt,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Lane {
    pub context: old::Id,
    pub slot: u8,
    pub role: u8,
    pub next: [u64; 2],
}
pub(super) fn committed_lane(
    context: old::Id,
    slot: u8,
    role: u8,
    meta: &db::migration7::live1::L1Metadata,
) -> Lane {
    Lane {
        context,
        slot,
        role,
        next: meta.next,
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Endpoint {
    pub source: old::Id,
    pub lanes: [Lane; 2],
}
impl Endpoint {
    pub fn validate(&self, owner: u8) -> Result<()> {
        ensure!(
            owner < 2
                && self.source != [0; 32]
                && self.lanes[0].context != self.lanes[1].context
                && self.lanes[0].slot != self.lanes[1].slot,
            "interval source/context identity"
        );
        for (c, l) in self.lanes.iter().enumerate() {
            ensure!(
                l.context != [0; 32]
                    && l.role == owner
                    && l.slot < 2
                    && l.next
                        .iter()
                        .all(|n| *n >= 1 && *n <= if c == 0 { 641 } else { 33 }),
                "committed lane mapping/bounds"
            );
        }
        Ok(())
    }
    pub fn point(&self) -> Result<(u64, u64)> {
        let owner = self.lanes[0].role;
        self.validate(owner)?;
        let mut w = Work::default();
        for l in &self.lanes {
            w = w.add(Work {
                encrypt: l.next[owner as usize]
                    .checked_sub(1)
                    .context("Text own next")?,
                decrypt: l.next[1 - owner as usize]
                    .checked_sub(1)
                    .context("Text peer next")?,
            })?;
        }
        Ok((w.encrypt, w.decrypt))
    }
    fn mapping(&self, b: &Self) -> bool {
        self.source == b.source
            && self
                .lanes
                .iter()
                .zip(&b.lanes)
                .all(|(a, b)| a.context == b.context && a.slot == b.slot && a.role == b.role)
    }
    fn capacity(&self, start: &Self, owner: u8) -> Result<[Work; 2]> {
        self.validate(owner)?;
        start.validate(owner)?;
        ensure!(self.mapping(start), "interval foreign source/mapping");
        let mut w = [Work::default(); 2];
        for (c, w) in w.iter_mut().enumerate() {
            *w = Work {
                encrypt: self.lanes[c].next[owner as usize]
                    .checked_sub(start.lanes[c].next[owner as usize])
                    .context("regressing committed own lane")?,
                decrypt: self.lanes[c].next[1 - owner as usize]
                    .checked_sub(start.lanes[c].next[1 - owner as usize])
                    .context("regressing committed peer lane")?,
            };
        }
        Ok(w)
    }
}
fn sum_counts(a: oracle::Counts, b: oracle::Counts) -> Result<oracle::Counts> {
    let add = |x: u64, y: u64| x.checked_add(y).context("interval counter overflow");
    Ok(oracle::Counts {
        encrypt: add(a.encrypt, b.encrypt)?,
        decrypt: add(a.decrypt, b.decrypt)?,
        attempts: add(a.attempts, b.attempts)?,
        aborted: add(a.aborted, b.aborted)?,
        ambiguous: add(a.ambiguous, b.ambiguous)?,
        accounts: add(a.accounts, b.accounts)?,
        outbound: add(a.outbound, b.outbound)?,
        inbound: add(a.inbound, b.inbound)?,
    })
}
fn clean(c: oracle::Counts) -> Result<()> {
    ensure!(
        c.aborted == 0
            && c.ambiguous == 0
            && c.accounts == 0
            && c.outbound == 0
            && c.inbound == 0
            && c.attempts == Work::of(c).calls()?,
        "interval unresolved/abort/ambiguity/constructors"
    );
    Ok(())
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Prefix {
    pub key: oracle::Incarnation,
    pub sequence: u64,
    pub counts: oracle::Counts,
    pub work: [Work; 2],
    pub entries: Vec<observations::PacketObservation>,
}
impl Prefix {
    pub fn new(key: oracle::Incarnation) -> Self {
        Self {
            key,
            sequence: 0,
            counts: Default::default(),
            work: Default::default(),
            entries: vec![],
        }
    }
    pub fn observe(&mut self, p: observations::PacketObservation) -> Result<()> {
        ensure!(
            p.context < 2
                && self.entries.len() < OPERATIONS
                && p.sequence
                    == self
                        .sequence
                        .checked_add(1)
                        .context("observed sequence overflow")?,
            "observed interval sequence/context/bound"
        );
        let counts = sum_counts(self.counts, p.counts)?;
        let c = p.context as usize;
        let work = self.work[c].add(Work::of(p.counts))?;
        self.entries.push(p);
        self.work[c] = work;
        self.counts = counts;
        self.sequence = p.sequence;
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Started {
    pub owner: u8,
    pub lifetime: u64,
    pub pid: u32,
    pub endpoint: Option<Endpoint>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WaitKind {
    Killed,
    Normal,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Waited {
    pub start: usize,
    pub outcome: usize,
    pub key: Option<oracle::Incarnation>,
    pub code: i32,
    pub kind: WaitKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ClosureKind {
    Killed,
    Normal { eof: u64 },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Tail {
    pub work: [Work; 2],
    pub generation: u64,
    pub operation: old::Id,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Closed {
    pub start: usize,
    pub outcome: usize,
    pub key: oracle::Incarnation,
    pub endpoint: Endpoint,
    pub prefix: Prefix,
    pub counts: oracle::Counts,
    pub sequence: u64,
    pub tail: Option<Tail>,
    pub generation: u64,
    pub operation: old::Id,
    pub kind: ClosureKind,
}
pub(super) struct CloseInput<'a> {
    pub start: usize,
    pub prefix: Prefix,
    pub counts: oracle::Counts,
    pub sequence: u64,
    pub end: &'a source::End,
    pub kind: ClosureKind,
}
#[derive(Default)]
pub(super) struct Ledger {
    pub migrations: [Option<Endpoint>; 2],
    pub starts: Vec<Started>,
    pub waits: Vec<Waited>,
    pub closed: Vec<Closed>,
}
pub(super) struct Verified {
    pub counts: oracle::Counts,
    pub streams: usize,
    pub observed_operations: usize,
    pub reconciled_entries: usize,
}
impl Ledger {
    pub fn migration(&mut self, owner: u8, e: Endpoint) -> Result<()> {
        e.validate(owner)?;
        let slot = self
            .migrations
            .get_mut(owner as usize)
            .context("migration owner")?;
        ensure!(
            slot.is_none()
                && !self
                    .starts
                    .iter()
                    .any(|s| s.owner == owner && s.endpoint.is_some()),
            "one original migration endpoint"
        );
        *slot = Some(e);
        Ok(())
    }
    pub fn started(&mut self, s: Started) -> Result<usize> {
        ensure!(
            s.owner < 2 && s.pid > 0 && s.lifetime > 0 && self.starts.len() < LIFETIMES,
            "owned interval start bounds"
        );
        ensure!(
            !self
                .starts
                .iter()
                .enumerate()
                .any(|(i, p)| p.pid == s.pid && !self.waits.iter().any(|w| w.start == i)),
            "PID still belongs to an unawaited lifetime"
        );
        if let Some((i, last)) = self
            .starts
            .iter()
            .enumerate()
            .rev()
            .find(|(_, a)| a.owner == s.owner)
        {
            ensure!(
                s.lifetime > last.lifetime && self.waits.iter().any(|w| w.start == i),
                "new waited owned lifetime, PID may repeat"
            );
        }
        if let Some(e) = &s.endpoint {
            e.validate(s.owner)?;
            let baseline = self
                .closed
                .iter()
                .rev()
                .find(|c| self.starts[c.start].owner == s.owner)
                .map(|c| &c.endpoint)
                .or(self.migrations[s.owner as usize].as_ref())
                .context("verified migration start missing")?;
            ensure!(
                e == baseline,
                "current start has gapped/overlapping committed lanes"
            );
            if let Some((i, _)) = self
                .starts
                .iter()
                .enumerate()
                .rev()
                .find(|(_, a)| a.owner == s.owner && a.endpoint.is_some())
            {
                ensure!(
                    self.closed.iter().any(|c| c.start == i),
                    "previous current interval unresolved"
                );
            }
        } else {
            ensure!(
                self.migrations[s.owner as usize].is_none(),
                "frozen start after current migration"
            );
        }
        let i = self.starts.len();
        self.starts.push(s);
        Ok(i)
    }
    pub fn waited(
        &mut self,
        start: usize,
        pid: u32,
        key: Option<oracle::Incarnation>,
        outcome: usize,
        code: Option<i32>,
        kind: WaitKind,
    ) -> Result<()> {
        let s = self
            .starts
            .get(start)
            .context("actual interval start missing")?;
        ensure!(
            s.pid == pid
                && self.waits.len() < LIFETIMES
                && outcome == self.waits.len()
                && !self.waits.iter().any(|w| w.start == start),
            "actual unique waited outcome"
        );
        match (&s.endpoint, key) {
            (Some(e), Some(k)) => ensure!(
                k.pid == pid
                    && k.owner == e.source
                    && k.nonce != [0; 32]
                    && !self
                        .waits
                        .iter()
                        .any(|w| w.key.is_some_and(|old| old.nonce == k.nonce)),
                "actual source/key/nonce lifetime"
            ),
            (None, None) => ensure!(
                kind == WaitKind::Killed,
                "frozen actual format cannot close normally"
            ),
            _ => anyhow::bail!("actual current/frozen start format mismatch"),
        }
        let code = code.context("actual waited exit code missing")?;
        ensure!(
            if kind == WaitKind::Normal {
                code == 0
            } else {
                code != 0
            },
            "actual waited exit kind"
        );
        self.waits.push(Waited {
            start,
            outcome,
            key,
            code,
            kind,
        });
        Ok(())
    }
    pub fn build(&self, input: CloseInput<'_>, ops: &oracle::Operations) -> Result<Closed> {
        let CloseInput {
            start,
            prefix,
            counts,
            sequence,
            end,
            kind,
        } = input;
        let s = self.starts.get(start).context("closed owned start")?;
        let initial = s
            .endpoint
            .as_ref()
            .context("current source endpoint missing")?;
        let w = self
            .waits
            .iter()
            .find(|w| w.start == start)
            .context("actual current wait missing")?;
        ensure!(
            w.key == Some(prefix.key)
                && prefix.key.pid == s.pid
                && prefix.key.owner == initial.source,
            "closed exact owned stream"
        );
        ensure!(
            end.point == end.endpoint.point()?,
            "aggregate Text endpoint mismatch"
        );
        let capacity = end.endpoint.capacity(initial, s.owner)?;
        let mut work = [Work::default(); 2];
        for c in 0..2 {
            work[c] = capacity[c].sub(prefix.work[c])?;
        }
        let delta = counts.delta(prefix.counts)?;
        ensure!(
            work[0].add(work[1])? == Work::of(delta),
            "context tails differ from closed counters"
        );
        let gap = Work::of(delta).calls()?;
        let tail = if gap > 0 {
            ensure!(
                kind == ClosureKind::Killed && gap <= 4,
                "normal cannot fabricate reconciled tail"
            );
            Some(Tail {
                work,
                generation: end.generation,
                operation: end.operation,
            })
        } else {
            None
        };
        ensure!(
            sequence
                == prefix
                    .sequence
                    .checked_add(u64::from(tail.is_some()))
                    .context("closed sequence overflow")?,
            "observed versus reconciled closed sequence"
        );
        ops.closed(prefix.key, sequence, counts)?;
        Ok(Closed {
            start,
            outcome: w.outcome,
            key: prefix.key,
            endpoint: end.endpoint.clone(),
            prefix,
            counts,
            sequence,
            tail,
            generation: end.generation,
            operation: end.operation,
            kind,
        })
    }
    pub fn retain(&mut self, rows: Vec<Closed>) -> Result<()> {
        ensure!(
            self.closed
                .len()
                .checked_add(rows.len())
                .is_some_and(|n| n <= LIFETIMES),
            "closed interval bound"
        );
        let mut starts = self.closed.iter().map(|c| c.start).collect::<BTreeSet<_>>();
        for c in &rows {
            ensure!(starts.insert(c.start), "duplicate interval closure");
        }
        self.closed.extend(rows);
        Ok(())
    }
    pub fn verify(&self, summary: &oracle::ClosedSummary, outcomes: &[Value]) -> Result<Verified> {
        ensure!(
            !self.closed.is_empty()
                && self.starts.len() <= LIFETIMES
                && self.starts.len() == self.waits.len()
                && self.waits.len() == outcomes.len()
                && self.closed.len() == summary.streams.len(),
            "bijective started/waited/closed current coverage"
        );
        let migrations = [
            self.migrations[0]
                .as_ref()
                .context("original migration A")?,
            self.migrations[1]
                .as_ref()
                .context("original migration B")?,
        ];
        for (o, e) in migrations.iter().enumerate() {
            e.validate(o as u8)?;
        }
        ensure!(
            migrations[0].source != migrations[1].source
                && migrations[0]
                    .lanes
                    .iter()
                    .zip(&migrations[1].lanes)
                    .all(|(a, b)| a.context == b.context),
            "original two-owner source/context identity"
        );
        let mut ends = [migrations[0].clone(), migrations[1].clone()];
        let mut ordinals = [0; 2];
        let mut seen_waits = BTreeSet::new();
        let mut seen_closed = BTreeSet::new();
        let mut nonces = BTreeSet::new();
        let mut operations = BTreeSet::new();
        let mut generations = BTreeMap::new();
        let mut roots: [Option<(u64, old::Id)>; 2] = [None, None];
        let mut counts = oracle::Counts::default();
        let mut observed = 0usize;
        let mut reconciled = 0usize;
        for (i, s) in self.starts.iter().enumerate() {
            ensure!(
                s.owner < 2 && s.pid > 0 && s.lifetime > ordinals[s.owner as usize],
                "new owned start identity/order"
            );
            let owner = s.owner as usize;
            ordinals[owner] = s.lifetime;
            let matches = self
                .waits
                .iter()
                .filter(|w| w.start == i)
                .collect::<Vec<_>>();
            ensure!(matches.len() == 1, "missing/duplicate actual wait");
            let w = matches[0];
            ensure!(
                seen_waits.insert(w.outcome) && w.outcome < outcomes.len(),
                "duplicate/foreign outcome index"
            );
            let row = &outcomes[w.outcome];
            ensure!(
                row["owner"] == owner
                    && row["pid"] == s.pid
                    && row["waited"] == true
                    && row["code"] == w.code
                    && row["success"] == (w.code == 0),
                "typed wait versus actual outcome"
            );
            let Some(initial) = &s.endpoint else {
                ensure!(
                    w.key.is_none()
                        && w.kind == WaitKind::Killed
                        && w.code != 0
                        && row["counter_outcome"] == "frozen process; outside current ledger"
                        && !self.closed.iter().any(|c| c.start == i),
                    "actual frozen classification"
                );
                continue;
            };
            initial.validate(s.owner)?;
            ensure!(
                *initial == ends[owner],
                "gapped/overlapping owner/context committed intervals"
            );
            let matches = self
                .closed
                .iter()
                .filter(|c| c.start == i)
                .collect::<Vec<_>>();
            ensure!(matches.len() == 1, "missing/duplicate current closure");
            let c = matches[0];
            ensure!(
                c.outcome == w.outcome
                    && w.key == Some(c.key)
                    && c.key.owner == migrations[owner].source
                    && c.key.pid == s.pid
                    && c.key.nonce != [0; 32]
                    && nonces.insert(c.key.nonce)
                    && c.prefix.key == c.key
                    && seen_closed.insert(c.key),
                "foreign/duplicate closed incarnation"
            );
            let matches = summary
                .streams
                .iter()
                .filter(|a| a.key == c.key)
                .collect::<Vec<_>>();
            ensure!(
                matches.len() == 1
                    && matches[0].counts == c.counts
                    && matches[0].sequence == c.sequence,
                "closed summary stream binding"
            );
            clean(c.counts)?;
            c.counts.delta(c.prefix.counts)?;
            let mut prefix = Prefix::new(c.key);
            for p in &c.prefix.entries {
                ensure!(
                    Work::of(p.counts).calls()? > 0 && Work::of(p.counts).calls()? <= 4,
                    "observed native bounded nonzero batch"
                );
                prefix.observe(*p)?;
                ensure!(
                    prefix.counts.attempts >= Work::of(prefix.counts).calls()?,
                    "observed prefix lacks actual attempts"
                );
                consume(
                    c.key.owner,
                    p.operation,
                    p.generation,
                    &mut operations,
                    &mut generations,
                )?;
                observed = observed
                    .checked_add(1)
                    .context("observed operation count")?;
            }
            ensure!(
                prefix == c.prefix,
                "observed prefix provenance/context accumulation"
            );
            let capacity = c.endpoint.capacity(initial, s.owner)?;
            let mut tail_work = [Work::default(); 2];
            for x in 0..2 {
                tail_work[x] = capacity[x].sub(prefix.work[x])?;
            }
            let gap = c.counts.delta(prefix.counts)?;
            ensure!(
                tail_work[0].add(tail_work[1])? == Work::of(gap),
                "wrong context/direction tail allocation"
            );
            if let Some(t) = &c.tail {
                ensure!(
                    c.kind == ClosureKind::Killed
                        && t.work == tail_work
                        && Work::of(gap).calls()? > 0
                        && Work::of(gap).calls()? <= 4
                        && t.generation == c.generation
                        && t.operation == c.operation
                        && c.sequence
                            == prefix
                                .sequence
                                .checked_add(1)
                                .context("reconciled sequence overflow")?,
                    "source-reconciled entry is not an observed packet"
                );
                consume(
                    c.key.owner,
                    t.operation,
                    t.generation,
                    &mut operations,
                    &mut generations,
                )?;
                reconciled += 1;
            } else {
                ensure!(
                    Work::of(gap) == Work::default() && c.sequence == prefix.sequence,
                    "missing/unexplained reconciliation entry"
                );
            }
            match c.kind {
                ClosureKind::Killed => ensure!(
                    w.kind == WaitKind::Killed
                        && w.code != 0
                        && row["counter_outcome"] == "source/counter resolved",
                    "resolved killed actual outcome"
                ),
                ClosureKind::Normal { eof } => ensure!(
                    w.kind == WaitKind::Normal
                        && w.code == 0
                        && c.tail.is_none()
                        && row["counter_outcome"] == "normal exit0/final/EOF/source resolved"
                        && row["normal_stop"]["source"] == state::hex(&c.key.owner)
                        && row["normal_stop"]["incarnation"] == state::hex(&c.key.nonce)
                        && row["normal_stop"]["acknowledged"] == true
                        && row["normal_stop"]["waited"] == true
                        && row["normal_closure"]["source"] == state::hex(&c.key.owner)
                        && row["normal_closure"]["pid"] == c.key.pid
                        && row["normal_closure"]["incarnation"] == state::hex(&c.key.nonce)
                        && row["normal_closure"]["sequence"] == c.sequence
                        && row["normal_closure"]["eof"] == eof
                        && row["normal_closure"]["root_generation"] == c.generation
                        && row["normal_closure"]["last_commit"] == state::hex(&c.operation)
                        && row["normal_closure"]["source_point"] == json!(c.endpoint.point()?)
                        && oracle::Counts::read(&row["normal_closure"]["final_counts"])?
                            == c.counts,
                    "normal actual final/EOF outcome"
                ),
            }
            if let Some((g, op)) = roots[owner] {
                ensure!(
                    c.generation >= g && (c.generation != g || c.operation == op),
                    "regressing stopped root boundary"
                );
            }
            if let Some(last) = generations.get(&c.key.owner) {
                ensure!(
                    c.generation >= *last,
                    "root before native/reconciled prefix"
                );
            }
            if let Some(p) = prefix.entries.last() {
                ensure!(
                    c.generation != p.generation || c.operation == p.operation,
                    "root versus observed native identity"
                );
            }
            roots[owner] = Some((c.generation, c.operation));
            ends[owner] = c.endpoint.clone();
            counts = sum_counts(counts, c.counts)?;
        }
        ensure!(
            seen_closed.len() == self.closed.len()
                && operations.len() <= OPERATIONS
                && operations.len() == summary.operations.len()
                && operations == summary.operations.iter().copied().collect()
                && generations.len() == summary.generations.len()
                && generations == summary.generations.iter().copied().collect()
                && counts == summary.counts,
            "unexplained summary stream/operation/generation/counters"
        );
        clean(summary.counts)?;
        Ok(Verified {
            counts,
            streams: self.closed.len(),
            observed_operations: observed,
            reconciled_entries: reconciled,
        })
    }
}
fn consume(
    owner: old::Id,
    op: old::Id,
    generation: u64,
    operations: &mut BTreeSet<(old::Id, old::Id)>,
    generations: &mut BTreeMap<old::Id, u64>,
) -> Result<()> {
    ensure!(
        op != [0; 32]
            && generation > generations.get(&owner).copied().unwrap_or(0)
            && operations.len() < OPERATIONS
            && operations.insert((owner, op)),
        "duplicate/foreign native or reconciliation membership"
    );
    generations.insert(owner, generation);
    Ok(())
}
