//! Transient consumer over the existing verified terminal traversal.
use super::*;
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Totals {
    pub current: oracle::Counts,
    pub old: Work,
    pub overlap: Work,
    pub union: Work,
    pub directed: u64,
    pub accounts: usize,
    pub outbound: usize,
    pub inbound: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Accounted {
    pub totals: Totals,
    pub(super) early: Early,
    pub(super) ledger: intervals::Ledger,
    pub(super) lineages: Vec<Lineage>,
    pub(super) records: [graph::Terminal; 2],
    pub(super) b25: Atom,
}
pub(crate) struct Pending(Accounted);
struct OwnerTap {
    owner: u8,
    next: [[u64; 2]; 2],
    genesis: [[bool; 2]; 2],
}
pub(crate) struct Draft {
    early: Early,
    ledger: intervals::Ledger,
    counts: oracle::Counts,
    consumed: Vec<[Work; 2]>,
    active: Option<OwnerTap>,
    finished: [bool; 2],
    matched_old: BTreeSet<Position>,
    matched_overlap: BTreeSet<Position>,
    matched_b25: BTreeSet<u8>,
    lineages: Vec<Lineage>,
    b25: Option<Atom>,
}
impl Draft {
    pub fn new(
        early: &Early,
        ledger: &intervals::Ledger,
        summary: &oracle::ClosedSummary,
        outcomes: &[Value],
    ) -> Result<Self> {
        let verified = ledger.verify(summary, outcomes)?;
        early.validate(ledger)?;
        ensure!(
            verified.streams == ledger.closed.len(),
            "validated current input layer required"
        );
        Ok(Self {
            early: early.clone(),
            ledger: ledger.clone(),
            counts: verified.counts,
            consumed: vec![[Work::default(); 2]; ledger.starts.len()],
            active: None,
            finished: [false; 2],
            matched_old: BTreeSet::new(),
            matched_overlap: BTreeSet::new(),
            matched_b25: BTreeSet::new(),
            lineages: vec![],
            b25: None,
        })
    }
    pub fn begin_owner(&mut self, owner: u8, source: old::Id, slots: [u8; 2]) -> Result<()> {
        ensure!(
            owner < 2 && self.active.is_none() && !self.finished[owner as usize],
            "one complete terminal owner tap"
        );
        let e = self.ledger.migrations[owner as usize]
            .as_ref()
            .context("owner migration")?;
        ensure!(
            e.source == source && e.lanes.each_ref().map(|l| l.slot) == slots,
            "actual terminal original source/Ready slots"
        );
        self.active = Some(OwnerTap {
            owner,
            next: [[1; 2]; 2],
            genesis: [[false; 2]; 2],
        });
        Ok(())
    }
    pub fn genesis(&mut self, a: Atom) -> Result<()> {
        let s = self.active.as_mut().context("active terminal owner")?;
        ensure!(
            a.index < 2
                && a.author < 2
                && a.n == 0
                && a.format == 1
                && a.body.is_none()
                && !s.genesis[a.index as usize][a.author as usize],
            "actual unique genesis tap"
        );
        let p = a.position(s.owner);
        let expected = self
            .early
            .old
            .get(&p)
            .context("original genesis observation")?;
        ensure!(
            expected.fact.atom == a && expected.migration && self.matched_old.insert(p),
            "terminal genesis matches original/migration signed source"
        );
        s.genesis[a.index as usize][a.author as usize] = true;
        Ok(())
    }
    pub fn text(&mut self, index: u8, row: &records::Record) -> Result<()> {
        let a = Atom::record(index, row);
        let s = self.active.as_mut().context("active terminal record tap")?;
        ensure!(
            index < 2
                && a.author < 2
                && a.n == s.next[index as usize][a.author as usize]
                && a.n <= if index == 0 { 640 } else { 32 },
            "complete unique terminal lane sequence"
        );
        let owner = s.owner;
        let p = a.position(owner);
        let e = self.ledger.migrations[owner as usize]
            .as_ref()
            .context("migration prefix")?;
        ensure!(
            a.context == e.lanes[index as usize].context,
            "logical context, not Ready slot"
        );
        let migrated = a.n < e.lanes[index as usize].next[a.author as usize];
        if let Some(old) = self.early.old.get(&p) {
            ensure!(
                old.fact.atom == a
                    && old.fact.source == e.source
                    && old.migration == migrated
                    && self.matched_old.insert(p),
                "terminal old raw/cipher/body and original prefix"
            );
        } else {
            ensure!(!migrated, "missing original/migration prefix membership");
        }
        if !migrated {
            let ranges = self
                .ledger
                .closed
                .iter()
                .filter(|c| contains(&self.ledger, c, p, &a))
                .collect::<Vec<_>>();
            ensure!(
                ranges.len() == 1,
                "one actual current interval for each directed source"
            );
            let c = ranges[0];
            let w = &mut self.consumed[c.start][index as usize];
            *w = w.add(p.work())?;
            let start = self.ledger.starts[c.start]
                .endpoint
                .as_ref()
                .context("current start")?;
            let cap = capacity(start, &c.endpoint, index as usize, owner)?;
            ensure!(
                w.encrypt <= cap.encrypt && w.decrypt <= cap.decrypt,
                "double/unused directed interval capacity"
            );
            if self.early.old.contains_key(&p) {
                let bound = self
                    .early
                    .overlap
                    .get(&p)
                    .context("current old source missing exact overlap")?;
                ensure!(
                    bound.mark.start == c.start
                        && bound.mark.key == c.key
                        && self.matched_overlap.insert(p),
                    "exact overlap EventID/current lifetime association"
                );
            }
        } else {
            ensure!(
                !self.early.overlap.contains_key(&p),
                "overlap cannot be assigned to migrated source"
            );
        }
        if index == 0 && a.author == 1 && a.n == 25 {
            let b = self
                .early
                .b25
                .get(&owner)
                .context("actual B25 effect witness")?;
            ensure!(
                !migrated
                    && a.format == 2
                    && b.source == e.source
                    && b.context == a.context
                    && b.event == a.event
                    && Some(b.request) == a.request
                    && Some(b.body) == a.body
                    && self.matched_b25.insert(owner),
                "one actual B25 format2 source per direction"
            );
            let c = self
                .ledger
                .closed
                .iter()
                .find(|c| c.start == b.bound.mark.start)
                .context("B25 closed interval")?;
            ensure!(
                contains(&self.ledger, c, p, &a),
                "B25 belongs to captured current lifetime"
            );
            if let Some(other) = &self.b25 {
                ensure!(
                    *other == a,
                    "both owners same signed B25 raw/cipher/projection"
                );
            } else {
                self.b25 = Some(a.clone());
            }
        }
        s.next[index as usize][a.author as usize] =
            a.n.checked_add(1).context("terminal ordinal overflow")?;
        Ok(())
    }
    pub fn finish_owner(&mut self, graph: &graph::Terminal, lineages: [Lineage; 2]) -> Result<()> {
        let s = self.active.as_ref().context("terminal owner tap")?;
        let owner = s.owner;
        ensure!(
            graph.texts == 1344
                && graph.genesis == 4
                && s.next == [[641; 2], [33; 2]]
                && s.genesis.iter().flatten().all(|v| *v),
            "partial terminal tap cannot finish owner"
        );
        for (c, l) in lineages.iter().enumerate() {
            let e = self.ledger.migrations[owner as usize]
                .as_ref()
                .context("original lineage mapping")?;
            let g = &graph.contexts[c];
            ensure!(
                l.owner == owner
                    && l.source == e.source
                    && l.index == c as u8
                    && l.context == e.lanes[c].context
                    && l.slot == e.lanes[c].slot
                    && l.context == g.context
                    && l.graph_scope == g.scope_commitment
                    && l.genesis == g.genesis
                    && l.offer != [0; 32]
                    && l.grant != [0; 32]
                    && l.scope != [0; 32]
                    && l.curve != [0; 32]
                    && l.native_public != [0; 32],
                "verified original Native/Ready/handshake lineage"
            );
            for author in 0..2u8 {
                let p = Position {
                    owner,
                    context: c as u8,
                    author,
                    n: 0,
                };
                ensure!(
                    self.early
                        .old
                        .get(&p)
                        .is_some_and(|e| e.fact.atom.event == l.genesis[author as usize]),
                    "original genesis/lineage association"
                );
            }
        }
        ensure!(self.lineages.len() < 4, "four lineage facts");
        self.lineages.extend(lineages);
        self.finished[owner as usize] = true;
        self.active = None;
        Ok(())
    }
    pub fn finish(self, graphs: [&graph::Terminal; 2]) -> Result<Pending> {
        ensure!(
            self.active.is_none()
                && self.finished == [true; 2]
                && self.matched_old.len() == 104
                && self.matched_overlap.len() == 17
                && self.matched_b25.len() == 2
                && self.lineages.len() == 4,
            "all owner/old/overlap/B25 inputs required"
        );
        graphs[0].same(graphs[1])?;
        for c in &self.ledger.closed {
            let s = &self.ledger.starts[c.start];
            let e = s.endpoint.as_ref().context("current endpoint")?;
            for x in 0..2 {
                ensure!(
                    self.consumed[c.start][x] == capacity(e, &c.endpoint, x, s.owner)?,
                    "every current interval capacity consumed exactly"
                );
            }
        }
        let mut curves = BTreeSet::new();
        for l in &self.lineages {
            ensure!(
                curves.insert(l.curve),
                "four distinct original account lineages"
            );
            let other = self
                .lineages
                .iter()
                .find(|a| a.owner != l.owner && a.index == l.index)
                .context("other original owner lineage")?;
            ensure!(
                l.scope == other.scope
                    && l.graph_scope == other.graph_scope
                    && l.genesis == other.genesis
                    && l.offer == other.offer
                    && l.grant == other.grant,
                "same authenticated two-context handshake lineage"
            );
        }
        let old = self
            .early
            .old
            .keys()
            .try_fold(Work::default(), |sum, p| sum.add(p.work()))?;
        let overlap = self
            .early
            .overlap
            .keys()
            .try_fold(Work::default(), |sum, p| sum.add(p.work()))?;
        let union = checked_union(old, Work::of(self.counts), overlap)?;
        ensure!(
            self.counts.encrypt == 1304
                && self.counts.decrypt == 1305
                && self.counts.attempts == 2609
                && self.counts.accounts == 0
                && self.counts.outbound == 0
                && self.counts.inbound == 0
                && self.counts.aborted == 0
                && self.counts.ambiguous == 0
                && old
                    == Work {
                        encrypt: 52,
                        decrypt: 52
                    }
                && overlap
                    == Work {
                        encrypt: 8,
                        decrypt: 9
                    }
                && union
                    == Work {
                        encrypt: 1348,
                        decrypt: 1348
                    },
            "final directed equations after complete source proof"
        );
        let directed = union.calls()?;
        ensure!(directed == 2696, "checked complete Text plus genesis union");
        Ok(Pending(Accounted {
            totals: Totals {
                current: self.counts,
                old,
                overlap,
                union,
                directed,
                accounts: self.lineages.len(),
                outbound: self.lineages.iter().filter(|l| l.owner == 0).count(),
                inbound: self.lineages.iter().filter(|l| l.owner == 1).count(),
            },
            early: self.early,
            ledger: self.ledger,
            lineages: self.lineages,
            records: [graphs[0].clone(), graphs[1].clone()],
            b25: self.b25.context("both B25 sources")?,
        }))
    }
}
fn capacity(
    start: &intervals::Endpoint,
    end: &intervals::Endpoint,
    index: usize,
    owner: u8,
) -> Result<Work> {
    Ok(Work {
        encrypt: end.lanes[index].next[owner as usize]
            .checked_sub(start.lanes[index].next[owner as usize])
            .context("negative terminal own capacity")?,
        decrypt: end.lanes[index].next[1 - owner as usize]
            .checked_sub(start.lanes[index].next[1 - owner as usize])
            .context("negative terminal peer capacity")?,
    })
}
pub(super) fn checked_union(old: Work, current: Work, overlap: Work) -> Result<Work> {
    old.add(current)?.sub(overlap)
}
pub(crate) fn seal_read<T>(read: impl FnOnce() -> Result<(T, Pending)>) -> Result<(T, Accounted)> {
    let before = kilogram_ratchet::ordered::native_call_counts();
    let result = read();
    ensure!(
        before == kilogram_ratchet::ordered::native_call_counts(),
        "stopped reader has zero Native constructors/packet calls, including failures"
    );
    let (value, pending) = result?;
    Ok((value, pending.0))
}
impl Accounted {
    pub fn after_zero_restart(&self, next: &Self, pair: &[control::Closure; 2]) -> Result<()> {
        ensure!(
            self.totals == next.totals
                && self.early == next.early
                && self.records == next.records
                && self.lineages == next.lineages
                && self.b25 == next.b25
                && self.ledger.migrations == next.ledger.migrations,
            "immutable accounting/source/lineage across cuts"
        );
        let n = self.ledger.starts.len();
        let w = self.ledger.waits.len();
        let c = self.ledger.closed.len();
        ensure!(
            next.ledger.starts.len() == n.checked_add(2).context("restart start bound")?
                && next.ledger.waits.len() == w + 2
                && next.ledger.closed.len() == c + 2
                && self.ledger.starts == next.ledger.starts[..n]
                && self.ledger.waits == next.ledger.waits[..w]
                && self.ledger.closed == next.ledger.closed[..c],
            "exact previous streams retained plus two new owned lifetimes"
        );
        let mut owners = BTreeSet::new();
        for closed in &next.ledger.closed[c..] {
            let s = &next.ledger.starts[closed.start];
            let o = s.owner as usize;
            ensure!(
                closed.start >= n
                    && closed.key == pair[o].key
                    && closed.sequence == pair[o].sequence
                    && closed.counts == pair[o].counts
                    && closed.outcome == pair[o].outcome
                    && closed.generation == pair[o].generation
                    && closed.operation == pair[o].operation
                    && closed.endpoint.point()? == (pair[o].point[0], pair[o].point[1])
                    && closed.kind == intervals::ClosureKind::Normal { eof: pair[o].eof }
                    && closed.sequence == 0
                    && closed.counts == Default::default()
                    && closed.prefix.entries.is_empty()
                    && closed.tail.is_none()
                    && s.endpoint.as_ref() == Some(&closed.endpoint)
                    && owners.insert(o)
                    && !self
                        .ledger
                        .closed
                        .iter()
                        .any(|p| p.key.nonce == closed.key.nonce),
                "actually new normal zero-work restart; PID may repeat"
            );
            ensure!(
                self.ledger
                    .starts
                    .iter()
                    .filter(|a| a.owner == s.owner)
                    .all(|a| a.lifetime < s.lifetime),
                "later captured owned lifetime"
            );
        }
        ensure!(
            owners == BTreeSet::from([0, 1]),
            "both original restart owners"
        );
        Ok(())
    }
}
