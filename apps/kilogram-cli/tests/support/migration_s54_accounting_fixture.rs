//! One supplied full geometry for accounting only. No authenticated DB/lineage claim.
use super::*;
use intervals::tests::{Case, Model};
pub(super) fn uid(c: u8, author: u8, n: u64, tag: u8) -> old::Id {
    let mut b = vec![c, author, tag];
    b.extend(n.to_le_bytes());
    digest(&b)
}
pub(super) struct Fixture {
    pub model: Model,
    pub early: Early,
    pub rows: Vec<records::Record>,
    pub genesis: Vec<Atom>,
    pub graph: graph::Terminal,
    pub lineages: [[Lineage; 2]; 2],
    pub marks: [Mark; 2],
}
fn row(c: u8, author: u8, n: u64) -> Result<records::Record> {
    let r = oracle::Request::new(c, author, n.try_into()?)?;
    Ok(records::Record {
        format: if c == 0 && n >= 25 { 2 } else { 1 },
        event: uid(c, author, n, 1),
        context: [c + 3; 32],
        d: [c + 5; 32],
        e: [c + 7; 32],
        sid: "S".repeat(43),
        selection: [c + 9; 32],
        role: author,
        ordinal: n,
        request: source::parse_request_id(&source::request_id(r))?,
        previous: Some(uid(c, author, n - 1, 1)),
        parents: if n == 1 {
            vec![]
        } else {
            vec![uid(c, author, n - 1, 1)]
        },
        sequence: n,
        author_auth: [author + 11; 32],
        source_hash: uid(c, author, n, 2),
        ciphertext_hash: uid(c, author, n, 3),
        body_hash: body_hash(&r.body()),
        receipt_format: 2,
        receipt_hash: uid(c, author, n, 4),
        receiver_auth: [author + 13; 32],
    })
}
fn chunks(packets: &mut Vec<(u8, Work)>, c: u8, e: u64, d: u64) {
    for (direction, n) in [(0, e), (1, d)] {
        let mut left = n;
        while left > 0 {
            let n = left.min(4);
            packets.push((
                c,
                if direction == 0 {
                    Work {
                        encrypt: n,
                        decrypt: 0,
                    }
                } else {
                    Work {
                        encrypt: 0,
                        decrypt: n,
                    }
                },
            ));
            left -= n;
        }
    }
}
fn endpoint(owner: u8) -> intervals::Endpoint {
    intervals::Endpoint {
        source: [owner + 1; 32],
        lanes: [
            intervals::Lane {
                context: [3; 32],
                slot: 1 - owner,
                role: owner,
                next: if owner == 0 { [17, 16] } else { [25, 25] },
            },
            intervals::Lane {
                context: [4; 32],
                slot: owner,
                role: owner,
                next: [1; 2],
            },
        ],
    }
}
impl Fixture {
    pub fn new() -> Result<Self> {
        let mut rows = Vec::new();
        let mut genesis = Vec::new();
        for c in 0..2u8 {
            for a in 0..2u8 {
                genesis.push(Atom {
                    index: c,
                    context: [c + 3; 32],
                    d: [c + 5; 32],
                    e: [c + 7; 32],
                    sid: digest("S".repeat(43).as_bytes()),
                    author: a,
                    n: 0,
                    event: uid(c, a, 0, 1),
                    format: 1,
                    request: None,
                    raw: uid(c, a, 0, 2),
                    cipher: uid(c, a, 0, 3),
                    body: None,
                });
                for n in 1..=if c == 0 { 640 } else { 32 } {
                    rows.push(row(c, a, n)?);
                }
            }
        }
        let graph = graph::Terminal {
            contexts: std::array::from_fn(|c| {
                let c = c as u8;
                let limit = if c == 0 { 640 } else { 32 };
                graph::Context {
                    context: [c + 3; 32],
                    scope_commitment: [c + 15; 32],
                    genesis: [uid(c, 0, 0, 1), uid(c, 1, 0, 1)],
                    count: limit as usize * 2,
                    next: [limit + 1; 2],
                    tails: [uid(c, 0, limit, 1), uid(c, 1, limit, 1)],
                    ordered: (1..=limit)
                        .flat_map(|n| [uid(c, 0, n, 1), uid(c, 1, n, 1)])
                        .collect(),
                    records_commitment: [c + 17; 32],
                }
            }),
            texts: 1344,
            genesis: 4,
        };
        let mut model = Model::with_migrations([endpoint(0), endpoint(1)])?;
        model.current(Case {
            owner: 0,
            lifetime: 1,
            pid: 7,
            nonce: 30,
            packets: vec![
                (
                    0,
                    Work {
                        encrypt: 4,
                        decrypt: 0,
                    },
                ),
                (
                    0,
                    Work {
                        encrypt: 3,
                        decrypt: 0,
                    },
                ),
            ],
            tail: [
                Work {
                    encrypt: 1,
                    decrypt: 0,
                },
                Work::default(),
            ],
            normal: false,
        })?;
        model.current(Case::zero(0, 2, 31))?;
        model.current(Case::zero(1, 1, 32))?;
        let mut packets = vec![
            (
                0,
                Work {
                    encrypt: 0,
                    decrypt: 4,
                },
            ),
            (
                0,
                Work {
                    encrypt: 0,
                    decrypt: 4,
                },
            ),
            (
                0,
                Work {
                    encrypt: 0,
                    decrypt: 2,
                },
            ),
        ];
        chunks(&mut packets, 0, 616, 614);
        chunks(&mut packets, 1, 32, 31);
        model.current(Case {
            owner: 0,
            lifetime: 3,
            pid: 7,
            nonce: 33,
            packets,
            tail: [
                Work::default(),
                Work {
                    encrypt: 0,
                    decrypt: 1,
                },
            ],
            normal: false,
        })?;
        let mut packets = vec![(
            0,
            Work {
                encrypt: 1,
                decrypt: 0,
            },
        )];
        chunks(&mut packets, 0, 615, 616);
        chunks(&mut packets, 1, 32, 32);
        model.current(Case {
            owner: 1,
            lifetime: 2,
            pid: 8,
            nonce: 34,
            packets,
            tail: Default::default(),
            normal: false,
        })?;
        model.current(Case {
            packets: vec![(
                0,
                Work {
                    encrypt: 0,
                    decrypt: 1,
                },
            )],
            ..Case::zero(0, 4, 35)
        })?;
        model.current(Case::zero(1, 3, 36))?;
        let marks = [(3, 3), (4, 1)].map(|(i, n)| -> Result<Mark> {
            let c = &model.ledger.closed[i];
            let s = model.ledger.starts[c.start]
                .endpoint
                .as_ref()
                .context("model start")?;
            let mut prefix = intervals::Prefix::new(c.key);
            for p in c.prefix.entries.iter().take(n) {
                prefix.observe(*p)?;
            }
            let mut next = s.lanes[0].next;
            let owner = model.ledger.starts[c.start].owner as usize;
            next[owner] += prefix.work[0].encrypt;
            next[1 - owner] += prefix.work[0].decrypt;
            Mark::live(c.start, c.key, next, &prefix)
        });
        let [a, b] = marks;
        let marks = [a?, b?];
        let lineages = std::array::from_fn(|owner| {
            std::array::from_fn(|c| Lineage {
                owner: owner as u8,
                source: [owner as u8 + 1; 32],
                index: c as u8,
                context: [c as u8 + 3; 32],
                slot: if c == 0 { 1 - owner as u8 } else { owner as u8 },
                scope: [c as u8 + 21; 32],
                graph_scope: graph.contexts[c].scope_commitment,
                offer: [c as u8 + 23; 32],
                grant: [c as u8 + 25; 32],
                genesis: graph.contexts[c].genesis,
                curve: uid(c as u8, owner as u8, 0, 9),
                native_public: uid(c as u8, owner as u8, 0, 10),
            })
        });
        let mut f = Self {
            model,
            early: Early::default(),
            rows,
            genesis,
            graph,
            lineages,
            marks,
        };
        f.capture()?;
        Ok(f)
    }
    fn atoms(&self) -> Vec<Atom> {
        self.genesis
            .iter()
            .cloned()
            .chain(
                self.rows
                    .iter()
                    .map(|r| Atom::record(if r.context == [3; 32] { 0 } else { 1 }, r)),
            )
            .collect()
    }
    fn facts(&self, owner: u8, next: [u64; 2], late: bool) -> Result<Vec<Fact>> {
        self.atoms()
            .into_iter()
            .filter(|a| a.n == 0 || (a.index == 0 && a.n < next[a.author as usize]))
            .map(|a| {
                let witness = if late && owner == 0 && a.author == 1 && a.n >= 16 {
                    [2; 32]
                } else {
                    [owner + 1; 32]
                };
                Fact::new(owner, [owner + 1; 32], witness, a)
            })
            .collect()
    }
    fn capture(&mut self) -> Result<()> {
        for o in 0..2u8 {
            let e = self.model.ledger.migrations[o as usize]
                .as_ref()
                .context("migration")?;
            self.early
                .migration(e, self.facts(o, e.lanes[0].next, false)?)?;
        }
        let mut p0 = self.facts(0, [17, 16], false)?;
        p0.extend(self.facts(1, [17, 17], false)?);
        self.early.capture(Stage::P0, p0)?;
        let mut p2 = self.facts(0, [25, 16], false)?;
        p2.extend(self.facts(1, [25, 25], false)?);
        self.early.capture(Stage::P2, p2)?;
        let encrypts = self
            .facts(0, [25, 16], false)?
            .into_iter()
            .filter(|f| f.atom.index == 0 && f.atom.author == 0 && (17..=24).contains(&f.atom.n))
            .collect::<Vec<_>>();
        self.early.bind_encrypts(&self.model.ledger, &encrypts)?;
        let a = self.facts(0, [25, 25], true)?;
        let late = a
            .iter()
            .filter(|f| f.late_position())
            .cloned()
            .collect::<Vec<_>>();
        let mut p4 = a;
        p4.extend(self.facts(1, [25, 25], false)?);
        self.early.capture(Stage::P4, p4)?;
        let readable = self.history()?;
        self.early.readable(&readable)?;
        self.early
            .bind_late(&self.model.ledger, &late, self.marks[0].clone())?;
        let b = self
            .rows
            .iter()
            .find(|r| r.context == [3; 32] && r.role == 1 && r.ordinal == 25)
            .context("B25")?;
        let live = self.history_row(b)?;
        for o in 0..2 {
            self.early
                .b25(&self.model.ledger, o, &live, self.marks[o as usize].clone())?;
        }
        Ok(())
    }
    fn history_row(&self, r: &records::Record) -> Result<Value> {
        let req = oracle::Request::new(0, r.role, r.ordinal.try_into()?)?;
        Ok(
            json!({"id":state::hex(&r.event),"format":r.format,"role":r.role,"ordinal":r.ordinal,"request":state::hex(&r.request),"text":req.body(),"delivered":true}),
        )
    }
    fn history(&self) -> Result<Vec<Value>> {
        self.rows
            .iter()
            .filter(|r| {
                r.context == [3; 32] && (r.ordinal <= 24 || (r.role == 1 && r.ordinal == 25))
            })
            .map(|r| self.history_row(r))
            .collect()
    }
    pub fn draft(&self) -> Result<Draft> {
        Draft::new(
            &self.early,
            &self.model.ledger,
            &self.model.ops.summary()?,
            &self.model.outcomes,
        )
    }
    pub fn consume(&self, d: &mut Draft) -> Result<()> {
        for o in 0..2u8 {
            let e = self.model.ledger.migrations[o as usize]
                .as_ref()
                .context("migration")?;
            d.begin_owner(o, e.source, e.lanes.each_ref().map(|l| l.slot))?;
            for g in &self.genesis {
                d.genesis(g.clone())?;
            }
            for r in &self.rows {
                d.text(if r.context == [3; 32] { 0 } else { 1 }, r)?;
            }
            d.finish_owner(&self.graph, self.lineages[o as usize].clone())?;
        }
        Ok(())
    }
    pub fn run(&self) -> Result<Accounted> {
        seal_read(|| {
            let mut d = self.draft()?;
            self.consume(&mut d)?;
            Ok(((), d.finish([&self.graph, &self.graph])?))
        })
        .map(|(_, a)| a)
    }
    pub fn restart(&mut self) -> Result<[control::Closure; 2]> {
        self.model.current(Case::zero(0, 5, 37))?;
        self.model.current(Case::zero(1, 4, 38))?;
        [0, 1]
            .map(|o| -> Result<control::Closure> {
                let c = self
                    .model
                    .ledger
                    .closed
                    .iter()
                    .rev()
                    .find(|c| self.model.ledger.starts[c.start].owner == o)
                    .context("new normal closure")?;
                let p = c.endpoint.point()?;
                Ok(control::Closure {
                    key: c.key,
                    sequence: c.sequence,
                    counts: c.counts,
                    eof: 0,
                    generation: c.generation,
                    operation: c.operation,
                    point: [p.0, p.1],
                    outcome: c.outcome,
                })
            })
            .into_iter()
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| anyhow::anyhow!("model pair"))
    }
}
