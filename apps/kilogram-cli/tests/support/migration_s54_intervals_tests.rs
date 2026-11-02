//! Supplied observation/metadata models, never actual process/source provenance.
use super::*;
fn counters(c: oracle::Counts) -> Value {
    json!({"encrypt":c.encrypt,"decrypt":c.decrypt,"attempts":c.attempts,"aborted":c.aborted,"ambiguous":c.ambiguous,"accounts":c.accounts,"outbound":c.outbound,"inbound":c.inbound})
}
fn counts(w: Work) -> Result<oracle::Counts> {
    Ok(oracle::Counts {
        encrypt: w.encrypt,
        decrypt: w.decrypt,
        attempts: w.calls()?,
        ..Default::default()
    })
}
fn endpoint(owner: u8) -> Endpoint {
    Endpoint {
        source: [owner + 1; 32],
        lanes: [
            Lane {
                context: [3; 32],
                slot: 1 - owner,
                role: owner,
                next: [17, 16],
            },
            Lane {
                context: [4; 32],
                slot: owner,
                role: owner,
                next: [1, 1],
            },
        ],
    }
}
fn feed(
    d: &mut observations::Decoder,
    ops: &mut oracle::Operations,
    key: oracle::Incarnation,
    kind: &str,
    sequence: u64,
    facts: Value,
) -> Result<()> {
    d.line(&format!("s54_observation {}",json!({"kind":kind,"pid":key.pid,"owner":state::hex(&key.owner),"incarnation":state::hex(&key.nonce),"sequence":sequence,"facts":facts})),ops,&mut Default::default(),&mut 0)
}
pub(crate) struct Case {
    pub owner: u8,
    pub lifetime: u64,
    pub pid: u32,
    pub nonce: u8,
    pub packets: Vec<(u8, Work)>,
    pub tail: [Work; 2],
    pub normal: bool,
}
impl Case {
    pub fn zero(owner: u8, lifetime: u64, nonce: u8) -> Self {
        Self {
            owner,
            lifetime,
            pid: 7 + u32::from(owner),
            nonce,
            packets: vec![],
            tail: Default::default(),
            normal: true,
        }
    }
}
pub(crate) struct Model {
    pub ledger: Ledger,
    pub ops: oracle::Operations,
    pub outcomes: Vec<Value>,
    generation: [u64; 2],
}
impl Model {
    fn new() -> Result<Self> {
        Self::with_migrations([endpoint(0), endpoint(1)])
    }
    pub fn with_migrations(endpoints: [Endpoint; 2]) -> Result<Self> {
        let mut ledger = Ledger::default();
        for o in 0..2 {
            ledger.migration(o, endpoints[o as usize].clone())?;
        }
        Ok(Self {
            ledger,
            ops: Default::default(),
            outcomes: vec![],
            generation: [0; 2],
        })
    }
    pub fn current(&mut self, c: Case) -> Result<usize> {
        let owner = c.owner as usize;
        let initial = self
            .ledger
            .closed
            .iter()
            .rev()
            .find(|x| self.ledger.starts[x.start].owner == c.owner)
            .map(|x| x.endpoint.clone())
            .unwrap_or(
                self.ledger.migrations[owner]
                    .as_ref()
                    .context("supplied migration")?
                    .clone(),
            );
        let index = self.ledger.started(Started {
            owner: c.owner,
            lifetime: c.lifetime,
            pid: c.pid,
            endpoint: Some(initial.clone()),
        })?;
        let key = oracle::Incarnation {
            owner: initial.source,
            pid: c.pid,
            nonce: [c.nonce; 32],
        };
        let mut d = observations::Decoder::new(
            c.pid,
            key.owner,
            initial.lanes.each_ref().map(|l| state::hex(&l.context)),
            c.owner,
        );
        feed(
            &mut d,
            &mut self.ops,
            key,
            "start",
            0,
            json!({"counters":counters(Default::default())}),
        )?;
        let mut endpoint = initial.clone();
        let mut observed = oracle::Counts::default();
        for (n, (context, w)) in c.packets.iter().enumerate() {
            self.generation[owner] += 1;
            let g = self.generation[owner];
            observed = sum_counts(observed, counts(*w)?)?;
            endpoint.lanes[*context as usize].next[owner] += w.encrypt;
            endpoint.lanes[*context as usize].next[1 - owner] += w.decrypt;
            feed(
                &mut d,
                &mut self.ops,
                key,
                "native_commit",
                (n + 1) as u64,
                json!({"context":state::hex(&initial.lanes[*context as usize].context),"operation":state::hex(blake3::hash(&g.to_le_bytes()).as_bytes()),"generation":g,"counters":counters(observed)}),
            )?;
        }
        for (x, w) in c.tail.iter().enumerate() {
            endpoint.lanes[x].next[owner] += w.encrypt;
            endpoint.lanes[x].next[1 - owner] += w.decrypt;
        }
        let actual = sum_counts(observed, counts(c.tail[0].add(c.tail[1])?)?)?;
        // A later real root write is deliberately distinct from observed native work.
        self.generation[owner] += 1;
        let root = self.generation[owner];
        let end = source::End {
            point: endpoint.point()?,
            endpoint,
            generation: root,
            operation: *blake3::hash(&root.to_le_bytes()).as_bytes(),
        };
        let outcome = self.outcomes.len();
        let code = if c.normal { 0 } else { 9 };
        self.ledger.waited(
            index,
            c.pid,
            Some(key),
            outcome,
            Some(code),
            if c.normal {
                WaitKind::Normal
            } else {
                WaitKind::Killed
            },
        )?;
        let prefix = d.interval_prefix()?;
        let gap = Work::of(actual.delta(prefix.counts)?).calls()?;
        let sequence = prefix.sequence + u64::from(gap > 0);
        let kind = if c.normal {
            feed(
                &mut d,
                &mut self.ops,
                key,
                "final",
                prefix.sequence,
                json!({"resolved":true,"counters":counters(actual)}),
            )?;
            let facts = d.close_normal(&self.ops, key, initial.point()?, &end, 0)?;
            self.outcomes.push(json!({"owner":owner,"pid":c.pid,"waited":true,"code":0,"success":true,"counter_outcome":"normal exit0/final/EOF/source resolved","normal_stop":{"source":state::hex(&key.owner),"incarnation":state::hex(&key.nonce),"acknowledged":true,"waited":true},"normal_closure":facts}));
            ClosureKind::Normal { eof: 0 }
        } else {
            d.close_killed(&mut self.ops, initial.point()?, end.clone(), actual)?;
            self.outcomes.push(json!({"owner":owner,"pid":c.pid,"waited":true,"code":code,"success":false,"counter_outcome":"source/counter resolved"}));
            ClosureKind::Killed
        };
        let closed = self.ledger.build(
            CloseInput {
                start: index,
                prefix: d.interval_prefix()?,
                counts: actual,
                sequence,
                end: &end,
                kind,
            },
            &self.ops,
        )?;
        self.ledger.retain(vec![closed])?;
        Ok(index)
    }
    fn verify(&self) -> Result<Verified> {
        self.ledger.verify(&self.ops.summary()?, &self.outcomes)
    }
}
fn positive() -> Result<Model> {
    let mut m = Model::new()?;
    m.current(Case {
        owner: 0,
        lifetime: 3,
        pid: 7,
        nonce: 11,
        packets: vec![
            (
                0,
                Work {
                    encrypt: 1,
                    decrypt: 1,
                },
            ),
            (
                1,
                Work {
                    encrypt: 1,
                    decrypt: 0,
                },
            ),
        ],
        tail: [
            Work::default(),
            Work {
                encrypt: 0,
                decrypt: 1,
            },
        ],
        normal: false,
    })?;
    m.current(Case {
        owner: 1,
        lifetime: 4,
        pid: 8,
        nonce: 12,
        packets: vec![(
            1,
            Work {
                encrypt: 1,
                decrypt: 1,
            },
        )],
        tail: Default::default(),
        normal: true,
    })?;
    Ok(m)
}
#[test]
fn s54_b1a_observed_mixed_context_prefix_and_reconciled_root_membership_are_separate() -> Result<()>
{
    let m = positive()?;
    let v = m.verify()?;
    assert!(
        v.streams == 2
            && v.observed_operations == 3
            && v.reconciled_entries == 1
            && v.counts.encrypt == 3
            && v.counts.decrypt == 3
    );
    let c = &m.ledger.closed[0];
    assert!(
        c.prefix.sequence == 2
            && c.sequence == 3
            && c.prefix.entries.len() == 2
            && c.tail.as_ref().context("tail")?.work[1].decrypt == 1
    );
    assert!(c.prefix.entries.iter().all(|p| p.operation != c.operation));
    let before = m.ops.summary()?;
    m.verify()?;
    let after = m.ops.summary()?;
    assert!(before.operations == after.operations && before.counts == after.counts);
    Ok(())
}
#[test]
fn s54_b1a_committed_metadata_ignores_unmaterialized_pending_sources() -> Result<()> {
    let mut meta = db::migration7::live1::L1Metadata {
        policy_generation: 1,
        wall_high_water: 1,
        next: [25, 16],
        tails: [Some([9; 32]); 2],
        next_queue: 1,
        request_index_after: None,
        request_index_ready: true,
        ack_next: 1,
        peer_source_next: 16,
        peer_source_last: Some([8; 32]),
    };
    let before = committed_lane([3; 32], 1, 0, &meta);
    // Physical peer source watermark can advance while native materialization waits.
    meta.peer_source_next = 26;
    meta.peer_source_last = Some([7; 32]);
    assert_eq!(before, committed_lane([3; 32], 1, 0, &meta));
    meta.next[1] = 26;
    assert_ne!(before, committed_lane([3; 32], 1, 0, &meta));
    let mut e = endpoint(0);
    e.lanes[0] = before;
    assert_eq!(e.point()?, (24, 15));
    Ok(())
}
#[test]
fn s54_b1a_reused_pid_new_nonce_lifetime_is_valid_but_reused_nonce_is_not() -> Result<()> {
    let mut m = positive()?;
    let prior = m.ledger.closed[0].key;
    let i = m.current(Case::zero(0, 4, 13))?;
    let new = m
        .ledger
        .closed
        .iter()
        .find(|c| c.start == i)
        .context("new")?;
    assert!(
        new.key.pid == prior.pid
            && new.key.nonce != prior.nonce
            && new.counts == Default::default()
    );
    m.verify()?;
    assert!(m.current(Case::zero(0, 5, 13)).is_err());
    let mut m = positive()?;
    assert!(m.current(Case::zero(0, 3, 14)).is_err());
    Ok(())
}
#[test]
fn s54_b1a_missing_duplicate_foreign_open_or_unresolved_outcomes_refuse() -> Result<()> {
    for field in [
        "source",
        "pid",
        "incarnation",
        "sequence",
        "eof",
        "root_generation",
        "last_commit",
        "source_point",
        "final_counts",
    ] {
        let mut m = positive()?;
        m.outcomes[1]["normal_closure"][field] = Value::Null;
        assert!(m.verify().is_err(), "normal {field}");
    }
    for field in [
        "owner",
        "pid",
        "waited",
        "code",
        "success",
        "counter_outcome",
    ] {
        let mut m = positive()?;
        m.outcomes[0][field] = Value::Null;
        assert!(m.verify().is_err(), "{field}");
    }
    let mut m = positive()?;
    m.ledger.waits.pop();
    assert!(m.verify().is_err());
    let mut m = positive()?;
    m.ledger.closed.pop();
    assert!(m.verify().is_err());
    let mut m = positive()?;
    m.ledger.waits[1] = m.ledger.waits[0].clone();
    assert!(m.verify().is_err());
    let mut m = positive()?;
    m.ledger.closed[1].key.owner = [77; 32];
    assert!(m.verify().is_err());
    let mut m = positive()?;
    m.ledger.closed[1].key.nonce = m.ledger.closed[0].key.nonce;
    assert!(m.verify().is_err());
    let mut m = positive()?;
    m.outcomes.push(json!({"waited":false}));
    assert!(m.verify().is_err());
    let mut m = positive()?;
    m.ops.start(oracle::Incarnation {
        owner: [1; 32],
        pid: 7,
        nonce: [99; 32],
    })?;
    assert!(m.verify().is_err());
    let mut m = positive()?;
    let k = oracle::Incarnation {
        owner: [88; 32],
        pid: 9,
        nonce: [98; 32],
    };
    m.ops.start(k)?;
    m.ops.finish(k, Default::default())?;
    assert!(m.verify().is_err());
    Ok(())
}
#[test]
fn s54_b1a_lane_gaps_regressions_mapping_direction_and_tail_are_checked() -> Result<()> {
    for change in 0..7 {
        let mut m = positive()?;
        match change {
            0 => m.ledger.closed[0].endpoint.lanes[0].next[0] += 1,
            1 => m.ledger.closed[0].endpoint.lanes[0].next[1] = 1,
            2 => m.ledger.closed[0].endpoint.lanes.swap(0, 1),
            3 => m.ledger.closed[1].endpoint.lanes[0].role = 0,
            4 => m.ledger.closed[0].prefix.work.swap(0, 1),
            5 => m.ledger.closed[0]
                .tail
                .as_mut()
                .context("tail")?
                .work
                .swap(0, 1),
            _ => m.ledger.closed[0].prefix.entries[0].counts.encrypt = 0,
        }
        assert!(m.verify().is_err(), "mutation {change}");
    }
    let mut m = positive()?;
    let mut next = m.ledger.closed[0].endpoint.clone();
    next.lanes[0].next[0] += 1;
    assert!(
        m.ledger
            .started(Started {
                owner: 0,
                lifetime: 4,
                pid: 7,
                endpoint: Some(next)
            })
            .is_err()
    );
    let mut m = positive()?;
    m.ledger.migrations[0].as_mut().context("migration")?.source = [0; 32];
    assert!(m.verify().is_err());
    assert!(
        Work {
            encrypt: u64::MAX,
            decrypt: 1
        }
        .calls()
        .is_err()
    );
    assert!(
        Work {
            encrypt: u64::MAX,
            decrypt: 0
        }
        .add(Work {
            encrypt: 1,
            decrypt: 0
        })
        .is_err()
    );
    Ok(())
}
#[test]
fn s54_b1a_synthetic_tail_cannot_be_normal_or_observed_or_unexplained() -> Result<()> {
    let mut m = positive()?;
    m.ledger.closed[0].kind = ClosureKind::Normal { eof: 0 };
    assert!(m.verify().is_err());
    let mut m = positive()?;
    let tail = m.ledger.closed[0].tail.take().context("tail")?;
    let c = &mut m.ledger.closed[0];
    c.prefix.entries.push(observations::PacketObservation {
        context: 1,
        counts: counts(tail.work[1])?,
        operation: tail.operation,
        generation: tail.generation,
        sequence: c.sequence,
    });
    assert!(m.verify().is_err()); // root entry added as native without real observed prefix binding.
    let m = positive()?;
    let mut summary = m.ops.summary()?;
    summary.operations.push(([1; 32], [88; 32]));
    assert!(m.ledger.verify(&summary, &m.outcomes).is_err());
    let mut m = positive()?;
    m.ledger.closed[0].sequence = m.ledger.closed[0].prefix.sequence;
    assert!(m.verify().is_err());
    let mut m = positive()?;
    m.ledger.closed[1].tail = Some(tail);
    assert!(m.verify().is_err());
    Ok(())
}
#[test]
fn s54_b1a_zero_work_frozen_format_and_dirty_counters_do_not_bypass_coverage() -> Result<()> {
    let mut m = positive()?;
    m.current(Case::zero(0, 4, 13))?;
    m.current(Case::zero(1, 5, 14))?;
    let v = m.verify()?;
    assert!(
        v.counts.encrypt == 3
            && v.counts.decrypt == 3
            && v.streams == 4
            && v.reconciled_entries == 1
    );
    for field in 0..6 {
        let mut m = positive()?;
        let mut summary = m.ops.summary()?;
        let c = &mut m.ledger.closed[0].counts;
        match field {
            0 => c.aborted = 1,
            1 => c.ambiguous = 1,
            2 => c.accounts = 1,
            3 => c.outbound = 1,
            4 => c.inbound = 1,
            _ => c.attempts += 1,
        };
        let key = m.ledger.closed[0].key;
        let bad = m.ledger.closed[0].counts;
        summary
            .streams
            .iter_mut()
            .find(|s| s.key == key)
            .context("stream")?
            .counts = bad;
        assert!(m.ledger.verify(&summary, &m.outcomes).is_err());
    }
    let mut m = positive()?;
    m.ledger.starts[0].endpoint = None;
    assert!(m.verify().is_err());
    let mut l = Ledger::default();
    let i = l.started(Started {
        owner: 0,
        lifetime: 1,
        pid: 7,
        endpoint: None,
    })?;
    assert!(l.waited(i, 7, None, 0, Some(0), WaitKind::Normal).is_err());
    l.waited(i, 7, None, 0, Some(9), WaitKind::Killed)?;
    for o in 0..2 {
        l.migration(o, endpoint(o))?;
    }
    let mut m = Model {
        ledger: l,
        ops: Default::default(),
        generation: [0; 2],
        outcomes: vec![
            json!({"owner":0,"pid":7,"waited":true,"code":9,"success":false,"counter_outcome":"frozen process; outside current ledger"}),
        ],
    };
    m.current(Case::zero(0, 2, 15))?;
    m.current(Case::zero(1, 1, 16))?;
    m.verify()?;
    let i = m.ledger.started(Started {
        owner: 0,
        lifetime: 3,
        pid: 7,
        endpoint: Some(endpoint(0)),
    })?;
    assert!(
        m.ledger
            .waited(i, 7, None, 3, Some(9), WaitKind::Killed)
            .is_err()
    );
    assert!(m.verify().is_err());
    Ok(())
}
#[test]
fn s54_b1a_bound32_and_prior_decrypt_cannot_move_to_zero_restart() -> Result<()> {
    let mut m = Model::new()?;
    m.current(Case {
        packets: vec![(
            0,
            Work {
                encrypt: 0,
                decrypt: 1,
            },
        )],
        ..Case::zero(0, 1, 20)
    })?;
    let prior = m.ledger.closed[0].key;
    m.current(Case::zero(0, 2, 21))?;
    m.current(Case::zero(1, 1, 22))?;
    assert!(
        m.verify()?.counts.decrypt == 1
            && m.ledger.closed[0].key == prior
            && m.ledger.closed[0].counts.decrypt == 1
            && m.ledger.closed[1].counts.decrypt == 0
    );
    let mut m = Model::new()?;
    for n in 0..32u8 {
        m.current(Case::zero(n % 2, u64::from(n / 2) + 1, n + 30))?;
    }
    assert!(m.verify()?.streams == 32);
    assert!(m.current(Case::zero(0, 17, 99)).is_err());
    let mut p = Prefix::new(prior);
    p.sequence = u64::MAX;
    assert!(
        p.observe(observations::PacketObservation {
            context: 0,
            counts: Default::default(),
            operation: [8; 32],
            generation: 1,
            sequence: 0
        })
        .is_err()
    );
    Ok(())
}
