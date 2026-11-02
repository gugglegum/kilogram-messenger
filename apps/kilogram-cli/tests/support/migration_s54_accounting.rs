//! Bounded early witnesses and terminal accounting. Never a whole-run PASS.
use super::*;
use intervals::Work;
#[path = "migration_s54_accounting_tap.rs"]
mod tap;
pub(super) use tap::{Accounted, Draft, seal_read};
#[cfg(test)]
#[path = "migration_s54_accounting_fixture.rs"]
mod fixture;
#[cfg(test)]
#[path = "migration_s54_accounting_tests.rs"]
mod tests;
fn digest(raw: &[u8]) -> old::Id {
    *blake3::hash(raw).as_bytes()
}
fn body_hash(text: &str) -> old::Id {
    old::hash(b"kg:od1:request-text\0", text.as_bytes())
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Atom {
    index: u8,
    context: old::Id,
    d: old::Id,
    e: old::Id,
    sid: old::Id,
    author: u8,
    n: u64,
    event: old::Id,
    format: u8,
    request: Option<[u8; 16]>,
    raw: old::Id,
    cipher: old::Id,
    body: Option<old::Id>,
}
impl Atom {
    pub fn legacy(index: u8, raw: &[u8], event: &old::Event, body: Option<&str>) -> Result<Self> {
        let c = &event.core;
        let author = u8::try_from(c.role)?;
        ensure!(
            index < 2
                && author < 2
                && (if c.n == 0 {
                    c.kind == u64::from(author) + 1 && body.is_none()
                } else {
                    c.kind == 3 && body.is_some()
                }),
            "typed original source kind"
        );
        Ok(Self {
            index,
            context: c.c,
            d: c.d,
            e: c.e,
            sid: digest(c.sid.as_bytes()),
            author,
            n: c.n,
            event: event.id()?,
            format: 1,
            request: if c.n == 0 { None } else { Some(c.request) },
            raw: digest(raw),
            cipher: digest(
                old::decode(raw, kilogram_protocol::ordered_dialog_v2::MAX_EVENT)?.fields(5)?[3]
                    .bytes()?,
            ),
            body: body.map(body_hash),
        })
    }
    fn record(index: u8, r: &records::Record) -> Self {
        Self {
            index,
            context: r.context,
            d: r.d,
            e: r.e,
            sid: digest(r.sid.as_bytes()),
            author: r.role,
            n: r.ordinal,
            event: r.event,
            format: r.format,
            request: Some(r.request),
            raw: r.source_hash,
            cipher: r.ciphertext_hash,
            body: Some(r.body_hash),
        }
    }
    fn position(&self, owner: u8) -> Position {
        Position {
            owner,
            context: self.index,
            author: self.author,
            n: self.n,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Position {
    owner: u8,
    context: u8,
    author: u8,
    n: u64,
}
impl Position {
    fn direction(self) -> usize {
        usize::from(self.owner != self.author)
    }
    fn work(self) -> Work {
        if self.direction() == 0 {
            Work {
                encrypt: 1,
                decrypt: 0,
            }
        } else {
            Work {
                encrypt: 0,
                decrypt: 1,
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Fact {
    owner: u8,
    source: old::Id,
    witness: old::Id,
    atom: Atom,
}
impl Fact {
    pub fn late_position(&self) -> bool {
        self.atom.index == 0 && self.atom.author == 1 && (16..=24).contains(&self.atom.n)
    }
    pub fn new(owner: u8, source: old::Id, witness: old::Id, atom: Atom) -> Result<Self> {
        ensure!(
            owner < 2 && source != [0; 32] && witness != [0; 32],
            "original effect owner/source"
        );
        Ok(Self {
            owner,
            source,
            witness,
            atom,
        })
    }
    pub fn for_receiver(&self, owner: u8, source: old::Id) -> Result<Self> {
        Self::new(owner, source, self.witness, self.atom.clone())
    }
    pub fn legacy_json(&self) -> Value {
        json!({"role":self.atom.author,"ordinal":self.atom.n,"event":state::hex(&self.atom.event),"source_blake3":state::hex(&self.atom.raw),"effect":if self.owner==self.atom.author{"current-old-encrypt"}else{"current-old-decrypt"}})
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Old {
    fact: Fact,
    stages: u8,
    migration: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stage {
    P0,
    P2,
    P4,
}
impl Stage {
    fn index(self) -> usize {
        match self {
            Self::P0 => 0,
            Self::P2 => 1,
            Self::P4 => 2,
        }
    }
    fn bit(self) -> u8 {
        1 << self.index()
    }
    fn expected(self) -> Work {
        match self {
            Self::P0 => Work {
                encrypt: 36,
                decrypt: 35,
            },
            Self::P2 => Work {
                encrypt: 52,
                decrypt: 43,
            },
            Self::P4 => Work {
                encrypt: 52,
                decrypt: 52,
            },
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Mark {
    start: usize,
    key: oracle::Incarnation,
    next: [u64; 2],
    prefix_sequence: u64,
    prefix_counts: oracle::Counts,
    closed_source: bool,
}
impl Mark {
    pub fn live(
        start: usize,
        key: oracle::Incarnation,
        next: [u64; 2],
        prefix: &intervals::Prefix,
    ) -> Result<Self> {
        ensure!(prefix.key == key, "actual current phase mark");
        Ok(Self {
            start,
            key,
            next,
            prefix_sequence: prefix.sequence,
            prefix_counts: prefix.counts,
            closed_source: false,
        })
    }
    fn closed(c: &intervals::Closed) -> Self {
        Self {
            start: c.start,
            key: c.key,
            next: c.endpoint.lanes[0].next,
            prefix_sequence: c.prefix.sequence,
            prefix_counts: c.prefix.counts,
            closed_source: true,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Bound {
    mark: Mark,
    slot: u8,
    direction: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Live {
    source: old::Id,
    context: old::Id,
    event: old::Id,
    request: [u8; 16],
    body: old::Id,
    bound: Bound,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Early {
    old: BTreeMap<Position, Old>,
    stages: [Option<Work>; 3],
    sources: [Option<old::Id>; 2],
    overlap: BTreeMap<Position, Bound>,
    b25: BTreeMap<u8, Live>,
}
impl Early {
    fn insert(&mut self, f: Fact) -> Result<()> {
        ensure!(
            f.owner < 2
                && f.atom.index < 2
                && f.atom.author < 2
                && f.atom.format == 1
                && (f.atom.n == 0 || f.atom.index == 0),
            "old membership excludes later C2 Text"
        );
        let p = f.atom.position(f.owner);
        let src = &mut self.sources[f.owner as usize];
        ensure!(
            src.is_none_or(|old| old == f.source),
            "old owner source changed"
        );
        *src = Some(f.source);
        for (other, old) in &self.old {
            if old.fact.atom.event == f.atom.event {
                ensure!(
                    (other.context, other.author, other.n) == (p.context, p.author, p.n)
                        && old.fact.atom == f.atom,
                    "foreign/duplicate EventID old binding"
                );
            }
        }
        if let Some(old) = self.old.get(&p) {
            ensure!(old.fact == f, "changed early original effect");
        } else {
            ensure!(self.old.len() < 104, "bounded old effect inventory");
            self.old.insert(
                p,
                Old {
                    fact: f,
                    stages: 0,
                    migration: false,
                },
            );
        }
        Ok(())
    }
    pub fn migration(&mut self, endpoint: &intervals::Endpoint, facts: Vec<Fact>) -> Result<()> {
        let owner = endpoint.lanes[0].role;
        endpoint.validate(owner)?;
        let mut next = self.clone();
        let mut seen = BTreeSet::new();
        for f in facts {
            let p = f.atom.position(f.owner);
            ensure!(
                p.owner == owner
                    && f.source == endpoint.source
                    && f.atom.context == endpoint.lanes[p.context as usize].context
                    && p.n < endpoint.lanes[p.context as usize].next[p.author as usize]
                    && seen.insert(p),
                "actual committed migration prefix membership"
            );
            next.insert(f)?;
            next.old.get_mut(&p).context("prefix effect")?.migration = true;
        }
        let expected = endpoint.point()?;
        ensure!(
            seen.len()
                == usize::try_from(
                    expected
                        .0
                        .checked_add(expected.1)
                        .and_then(|n| n.checked_add(4))
                        .context("migration source cardinality overflow")?
                )?,
            "complete migration prefix including genesis"
        );
        *self = next;
        Ok(())
    }
    pub fn capture(&mut self, stage: Stage, facts: Vec<Fact>) -> Result<Work> {
        let i = stage.index();
        ensure!(
            self.stages[i].is_none() && (i == 0 || self.stages[i - 1].is_some()),
            "ordered actual old cuts"
        );
        let mut next = self.clone();
        let mut seen = BTreeSet::new();
        let mut total = Work::default();
        for f in facts {
            let p = f.atom.position(f.owner);
            ensure!(seen.insert(p), "duplicate directed old effect");
            next.insert(f)?;
            next.old.get_mut(&p).context("old cut member")?.stages |= stage.bit();
            total = total.add(p.work())?;
        }
        if i > 0 {
            ensure!(
                self.old
                    .iter()
                    .filter(|(_, e)| e.stages & (1 << (i - 1)) != 0)
                    .all(|(p, _)| seen.contains(p)),
                "old cut cannot lose earlier membership"
            );
        }
        ensure!(
            total == stage.expected(),
            "derived directed old/genesis subtotal"
        );
        next.stages[i] = Some(total);
        *self = next;
        Ok(total)
    }
    pub fn facts(&self, stage: Stage, owner: u8) -> Vec<Fact> {
        self.old
            .values()
            .filter(|e| e.fact.owner == owner && e.stages & stage.bit() != 0)
            .map(|e| e.fact.clone())
            .collect()
    }
    pub fn readable(&self, rows: &[Value]) -> Result<()> {
        let map = source::history_map(rows)?;
        let old = map
            .values()
            .filter(|r| r["format"] == 1)
            .collect::<Vec<_>>();
        let facts = self
            .old
            .values()
            .filter(|e| {
                e.fact.owner == 0
                    && e.fact.atom.index == 0
                    && e.fact.atom.n > 0
                    && e.stages & Stage::P4.bit() != 0
            })
            .collect::<Vec<_>>();
        ensure!(
            old.len() == facts.len(),
            "actual P4 readable old membership"
        );
        for f in facts {
            let a = &f.fact.atom;
            let row = map
                .get(&state::hex(&a.event))
                .context("old source missing from actual readability")?;
            ensure!(
                row["format"] == 1
                    && row["role"] == a.author
                    && row["ordinal"] == a.n
                    && row["request"] == state::hex(&a.request.context("Text request")?)
                    && body_hash(row["text"].as_str().context("readable body")?)
                        == a.body.context("Text body")?,
                "actual P4 old request/projection binding"
            );
        }
        Ok(())
    }
    pub fn bind_encrypts(&mut self, ledger: &intervals::Ledger, facts: &[Fact]) -> Result<()> {
        for f in facts {
            let p = f.atom.position(f.owner);
            let found = ledger
                .closed
                .iter()
                .filter(|c| contains(ledger, c, p, &f.atom))
                .collect::<Vec<_>>();
            ensure!(found.len() == 1, "actual current encrypt interval");
            self.bind(ledger, p, Mark::closed(found[0]))?;
        }
        Ok(())
    }
    pub fn bind_late(
        &mut self,
        ledger: &intervals::Ledger,
        facts: &[Fact],
        mark: Mark,
    ) -> Result<()> {
        for f in facts {
            self.bind(ledger, f.atom.position(f.owner), mark.clone())?;
        }
        Ok(())
    }
    fn bind(&mut self, ledger: &intervals::Ledger, p: Position, mark: Mark) -> Result<()> {
        ensure!(
            p.owner == 0
                && p.context == 0
                && ((p.author == 0 && (17..=24).contains(&p.n))
                    || (p.author == 1 && (16..=24).contains(&p.n)))
                && self.overlap.len() < 17
                && !self.overlap.contains_key(&p),
            "exact unique overlap positions/directions"
        );
        let fact = &self
            .old
            .get(&p)
            .context("overlap requires retained old source")?
            .fact;
        let started = ledger
            .starts
            .get(mark.start)
            .context("actual overlap start")?;
        let e = started
            .endpoint
            .as_ref()
            .context("current overlap format")?;
        ensure!(
            started.owner == p.owner
                && mark.key.owner == fact.source
                && mark.key.pid == started.pid
                && e.source == fact.source
                && e.lanes[0].context == fact.atom.context
                && p.n >= e.lanes[0].next[p.author as usize]
                && p.n < mark.next[p.author as usize],
            "actual source/owner/range overlap mark"
        );
        self.overlap.insert(
            p,
            Bound {
                mark,
                slot: e.lanes[0].slot,
                direction: p.direction(),
            },
        );
        Ok(())
    }
    pub fn b25(
        &mut self,
        ledger: &intervals::Ledger,
        owner: u8,
        row: &Value,
        mark: Mark,
    ) -> Result<()> {
        ensure!(
            owner < 2
                && self.b25.len() < 2
                && !self.b25.contains_key(&owner)
                && row["format"] == 2
                && row["role"] == 1
                && row["ordinal"] == 25,
            "unique actual format2 B25 effect"
        );
        let s = ledger.starts.get(mark.start).context("actual B25 start")?;
        let e = s.endpoint.as_ref().context("B25 current format")?;
        ensure!(
            s.owner == owner
                && mark.key.owner == e.source
                && mark.key.pid == s.pid
                && e.lanes[0].next[1] <= 25
                && mark.next[1] > 25,
            "B25 actual executing interval"
        );
        let live = Live {
            source: e.source,
            context: e.lanes[0].context,
            event: state::parse_id(row["id"].as_str().context("actual B25 ID")?)?,
            request: source::parse_request_id(
                row["request"].as_str().context("actual B25 request")?,
            )?,
            body: body_hash(row["text"].as_str().context("actual B25 body")?),
            bound: Bound {
                mark,
                slot: e.lanes[0].slot,
                direction: usize::from(owner == 0),
            },
        };
        if let Some(other) = self.b25.values().next() {
            ensure!(
                (other.context, other.event, other.request, other.body)
                    == (live.context, live.event, live.request, live.body),
                "B25 sender/receiver same live source commitment"
            );
        }
        self.b25.insert(owner, live);
        Ok(())
    }
    fn validate(&self, ledger: &intervals::Ledger) -> Result<()> {
        ensure!(
            self.stages
                == [
                    Some(Stage::P0.expected()),
                    Some(Stage::P2.expected()),
                    Some(Stage::P4.expected())
                ]
                && self.old.len() == 104
                && self.overlap.len() == 17
                && self.b25.len() == 2,
            "complete retained old/overlap/B25 observations"
        );
        for (o, s) in self.sources.iter().enumerate() {
            ensure!(
                *s == Some(
                    ledger.migrations[o]
                        .as_ref()
                        .context("actual migration")?
                        .source
                ),
                "original retained source owner"
            );
        }
        for (p, e) in &self.old {
            ensure!(
                e.stages & Stage::P4.bit() != 0 && self.sources.contains(&Some(e.fact.witness)),
                "final old membership/witness source"
            );
            let m = ledger.migrations[p.owner as usize]
                .as_ref()
                .context("migration prefix")?;
            ensure!(
                e.migration == (p.n < m.lanes[p.context as usize].next[p.author as usize]),
                "old original/migration prefix distinction"
            );
        }
        for (p, b) in &self.overlap {
            let f = &self.old.get(p).context("retained overlap")?.fact;
            validate_bound(ledger, *p, &f.atom, b)?;
        }
        for (o, b) in &self.b25 {
            let p = Position {
                owner: *o,
                context: 0,
                author: 1,
                n: 25,
            };
            validate_mark(ledger, p, &b.bound)?;
            ensure!(
                b.bound.direction == p.direction()
                    && b.source == self.sources[*o as usize].context("B25 source")?,
                "B25 directed original source"
            );
        }
        Ok(())
    }
}
fn contains(ledger: &intervals::Ledger, c: &intervals::Closed, p: Position, a: &Atom) -> bool {
    let Some(s) = ledger.starts.get(c.start) else {
        return false;
    };
    let Some(e) = &s.endpoint else { return false };
    let x = p.context as usize;
    let r = p.author as usize;
    p.owner == s.owner
        && c.key.owner == e.source
        && e.lanes[x].context == a.context
        && p.n >= e.lanes[x].next[r]
        && p.n < c.endpoint.lanes[x].next[r]
}
fn validate_mark<'a>(
    ledger: &'a intervals::Ledger,
    p: Position,
    b: &Bound,
) -> Result<&'a intervals::Closed> {
    let c = ledger
        .closed
        .iter()
        .find(|c| c.start == b.mark.start)
        .context("marked current lifetime must actually close")?;
    let s = ledger.starts.get(c.start).context("marked owned start")?;
    let e = s.endpoint.as_ref().context("marked current endpoint")?;
    ensure!(
        c.key == b.mark.key
            && c.prefix.sequence >= b.mark.prefix_sequence
            && e.source == c.key.owner
            && s.owner == p.owner
            && b.slot == e.lanes[p.context as usize].slot
            && b.direction == p.direction(),
        "marked full incarnation/context/slot/direction"
    );
    let mut prefix = intervals::Prefix::new(c.key);
    for entry in c
        .prefix
        .entries
        .iter()
        .take(usize::try_from(b.mark.prefix_sequence)?)
    {
        prefix.observe(*entry)?;
    }
    ensure!(
        prefix.sequence == b.mark.prefix_sequence && prefix.counts == b.mark.prefix_counts,
        "actual observed phase prefix, not root reconciliation entry"
    );
    let lane = e.lanes[0];
    let mut capacity = Work::default();
    for author in 0..2 {
        let n = b.mark.next[author]
            .checked_sub(lane.next[author])
            .context("negative marked materialization")?;
        if author == p.owner as usize {
            capacity.encrypt = n
        } else {
            capacity.decrypt = n
        };
        ensure!(
            b.mark.next[author] <= c.endpoint.lanes[0].next[author],
            "marked frontier beyond actual closed source"
        );
    }
    let tail = if b.mark.closed_source {
        ensure!(
            b.mark.prefix_sequence == c.prefix.sequence && b.mark.next == c.endpoint.lanes[0].next,
            "actual closed source boundary"
        );
        c.tail.as_ref().map_or([Work::default(); 2], |t| t.work)
    } else {
        [Work::default(); 2]
    };
    ensure!(
        prefix.work[0].add(tail[0])? == capacity
            && prefix.work[1].add(tail[1])? == Work::default()
            && p.n >= lane.next[p.author as usize]
            && p.n < b.mark.next[p.author as usize],
        "actual materialized/context prefix and separately reconciled tail"
    );
    Ok(c)
}
fn validate_bound(ledger: &intervals::Ledger, p: Position, a: &Atom, b: &Bound) -> Result<()> {
    let c = validate_mark(ledger, p, b)?;
    ensure!(
        contains(ledger, c, p, a),
        "overlap not in actual current interval"
    );
    Ok(())
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Lineage {
    pub owner: u8,
    pub source: old::Id,
    pub index: u8,
    pub context: old::Id,
    pub slot: u8,
    pub scope: old::Id,
    pub graph_scope: old::Id,
    pub offer: old::Id,
    pub grant: old::Id,
    pub genesis: [old::Id; 2],
    pub curve: old::Id,
    pub native_public: old::Id,
}
