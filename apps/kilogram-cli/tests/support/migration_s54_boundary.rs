//! Shared stopped-source/cold proof. No Fresh, runtime, observation injection,
//! serialization or exclusive lease is retained by these owned records.
use super::*;
type Repo = db::migration7::live1::L1Repository;

#[derive(Clone)]
pub(in super::super) struct Proof {
    pub witness: Legacy,
    pub exact: Exact,
}
impl Proof {
    pub fn prepare(self, phase: Phase, owner: usize, contexts: &[String; 2]) -> Result<Checked> {
        ensure!(
            owner == phase.owner()
                && self.witness.contexts.len() == 2
                && contexts[0] != contexts[1]
                && self.exact.source == self.witness.source_id,
            "P phase owner/context/source binding"
        );
        self.witness.role(owner)?;
        for (index, ctx) in self.witness.contexts.iter().enumerate() {
            ensure!(
                state::hex(&ctx.c) == contexts[index],
                "P logical context mapping"
            );
            let spec = Spec::parse(&ctx.spec)?;
            let head =
                state::Head::parse(&ctx.head, ctx.c, spec.cert_refs()?[owner], owner as u64)?;
            ensure!(head.phase == 3, "P retained Ready phase");
            let next =
                [0, 1].map(|role| ctx.events.keys().filter(|(r, _)| *r == role).count() as u64);
            let expected = if index == 1 { [1, 1] } else { phase.next() };
            ensure!(
                next == expected
                    && ctx.events.len() as u64 == next.iter().sum::<u64>()
                    && (0..2).all(|role| (0..next[role as usize])
                        .all(|n| ctx.events.contains_key(&(role, n)))),
                "P exact retained committed frontiers"
            );
            if index == 1 {
                ensure!(
                    ctx.queue.is_empty() && ctx.receipts.is_empty() && ctx.history.is_empty(),
                    "P C2 remains zero Text and queue"
                );
            }
        }
        let ctx = &self.witness.contexts[0];
        ensure!(
            self.exact.queue == ctx.queue && self.exact.receipts == ctx.receipts,
            "P exact proof agrees with retained source"
        );
        match phase {
            Phase::FirstA => {
                // capture_source already decrypts/authenticates each sealed body.
                let expected = [17, 18]
                    .map(|n| {
                        oracle::Request::new(0, 0, n).and_then(|r| parse_request_id(&request_id(r)))
                    })
                    .into_iter()
                    .collect::<Result<BTreeSet<_>>>()?;
                ensure!(
                    self.exact.queue.keys().copied().collect::<BTreeSet<_>>() == expected
                        && self.exact.debt.is_empty(),
                    "P first A exact queue17/18 and no own debt"
                );
            }
            Phase::LateB => {
                let spec = Spec::parse(&ctx.spec)?;
                let expected = (16..=24)
                    .map(|n| {
                        let raw = ctx.events.get(&(1, n)).context("B16..24 retained source")?;
                        Ok((
                            n,
                            old::Event::parse(raw, spec.peers[1].cert.device_id())?.id()?,
                        ))
                    })
                    .collect::<Result<Vec<_>>>()?;
                ensure!(
                    self.exact.queue.is_empty() && self.exact.debt == expected,
                    "P late B exact nine source debts and no queue"
                );
                for n in 1..=24 {
                    ensure!(
                        ctx.receipts.contains_key(&(1, n)) == (n <= 15),
                        "P B1..15 retired and B16..24 lack opposite receipts"
                    );
                }
                let event = old::Event::parse(
                    ctx.events.get(&(0, 24)).context("actual A24 at B")?,
                    spec.peers[0].cert.device_id(),
                )?;
                let r1 = ctx
                    .receipts
                    .get(&(0, 24))
                    .context("P saved signed R1(A24)")?;
                verify_receipt(ctx, &event, r1)?;
            }
        }
        Ok(Checked { proof: self })
    }
    // Source-subtree only: the actual scenario enters via capture_boundary's
    // admission/ownership gate. Component tests supply their own fresh lease.
    pub(super) fn capture_source(
        node: &Node,
        contexts: &[String; 2],
        purpose: Purpose,
        source: &db::migration7::V4Source,
    ) -> Result<Self> {
        let witness = Legacy::capture_source(node, contexts, purpose, source)?;
        let exact = Exact::capture(source, &witness)?;
        Ok(Self { witness, exact })
    }
}

#[derive(Clone, Copy)]
pub(in super::super) enum Phase {
    FirstA,
    LateB,
}
impl Phase {
    fn owner(self) -> usize {
        match self {
            Self::FirstA => 0,
            Self::LateB => 1,
        }
    }
    fn next(self) -> [u64; 2] {
        match self {
            Self::FirstA => [17, 16],
            Self::LateB => [25, 25],
        }
    }
}

/// Constructed only after the retained pre-conversion phase checks succeed.
pub(in super::super) struct Checked {
    proof: Proof,
}
impl Checked {
    pub fn witness(&self) -> &Legacy {
        &self.proof.witness
    }
    pub(super) fn exact(&self) -> &Exact {
        &self.proof.exact
    }
    pub fn into_witness(self) -> Legacy {
        self.proof.witness
    }
    pub fn after_migration(&self, node: &Node, ready: &Value) -> Result<intervals::Endpoint> {
        ensure!(node.indexed, "converted P owner before cold verification");
        let endpoint = self.proof.witness.after_migration(node, ready)?;
        // after_migration above has returned and closed its repository handle.
        {
            let profile = Profile3::load(&node.profile)?;
            let mut repo = profile.open(&node.profile)?;
            self.proof
                .exact
                .verify(&mut repo, endpoint.lanes[0].slot, &self.proof.witness)?;
        }
        Ok(endpoint)
    }
}

// Exact cold comparison shared by positive checks and in-memory substitutions.
// Expected values below are captured from authenticated real legacy records.
#[derive(Clone)]
pub(in super::super) struct Exact {
    source: old::Id,
    pub(super) queue: BTreeMap<[u8; 16], Vec<u8>>,
    pub(super) debt: Vec<(u64, old::Id)>,
    pub(super) receipts: BTreeMap<(u8, u64), Vec<u8>>,
}
impl Exact {
    pub(super) fn capture(source: &db::migration7::V4Source, witness: &Legacy) -> Result<Self> {
        let ctx = &witness.contexts[0];
        ensure!(
            source.source_id() == witness.source_id,
            "same quiescent inventory"
        );
        let mut debt = Vec::new();
        let outbox_entries = source
            .entries()
            .iter()
            .filter(|e| {
                e.path
                    .starts_with(&format!("{}outbox/", engine::prefix(ctx.c)))
            })
            .count();
        ensure!(
            outbox_entries
                == ctx
                    .events
                    .keys()
                    .filter(|(r, n)| *r == ctx.role && *n > 0)
                    .count(),
            "all authenticated outbox records covered exactly"
        );
        for ((role, n), raw) in &ctx.events {
            if *n == 0 {
                continue;
            }
            let spec = Spec::parse(&ctx.spec)?;
            let event = old::Event::parse(raw, spec.peers[*role as usize].cert.device_id())?;
            let id = event.id()?;
            let receipt = ctx.receipts.get(&(*role, *n));
            if let Some(receipt) = receipt {
                verify_receipt(ctx, &event, receipt)?;
            }
            if *role != ctx.role {
                continue;
            }
            let raw = source.required(&engine::outbox_path(ctx.c, *n))?;
            let value = old::decode(&raw, 1024)?;
            let fields = value.fields(3)?;
            let status = fields[2].uint()?;
            ensure!(
                fields[0].uint()? == 1 && fields[1].fixed::<32>()? == id && status <= 1,
                "authenticated outbox exact ordinal/event/state"
            );
            ensure!(
                (status == 1) == receipt.is_some(),
                "delivered iff actual signed opposite receipt"
            );
            if status == 0 {
                debt.push((*n, id));
            }
        }
        Ok(Self {
            source: source.source_id(),
            queue: ctx.queue.clone(),
            debt,
            receipts: ctx.receipts.clone(),
        })
    }
    pub fn verify(&self, repo: &mut Repo, slot: u8, witness: &Legacy) -> Result<()> {
        let ctx = &witness.contexts[0];
        ensure!(
            self.source == witness.source_id
                && repo.source_id() == self.source
                && repo.contexts().get(slot as usize) == Some(&(ctx.c, ctx.role)),
            "exact cold source/context/role binding"
        );
        let rows = repo.queue_page(slot, None, 32)?;
        let queue = rows
            .iter()
            .map(|q| (q.request, q.sealed.clone()))
            .collect::<BTreeMap<_, _>>();
        ensure!(
            queue == self.queue && rows.len() == self.queue.len(),
            "exact queue ID/bytes/cardinality"
        );
        ensure!(
            repo.debt_page(slot, None, 32)? == self.debt,
            "exact debt ordinal/ID/cardinality"
        );
        let counts = repo.counts(slot)?;
        ensure!(
            counts.debt == self.debt.len() as u64 && counts.queued == self.queue.len() as u64,
            "exact cold debt and queue counts"
        );
        ensure!(
            counts.receipts
                == [0, 1]
                    .map(|role| self.receipts.keys().filter(|(r, _)| *r == role).count() as u64),
            "exact cold receipt cardinality"
        );
        for ((role, n), raw) in &witness.contexts[0].events {
            ensure!(
                repo.event_at(slot, *role, *n)?.as_ref() == Some(raw),
                "exact event bytes"
            );
            ensure!(
                repo.receipt(slot, *role, *n)?.as_ref() == self.receipts.get(&(*role, *n)),
                "exact receipt bytes or absence"
            );
        }
        ensure!(
            !repo.initialized()? && repo.snapshot(slot).is_err(),
            "cold metadata remains absent"
        );
        Ok(())
    }
}

pub(super) fn verify_receipt(ctx: &SavedContext, event: &old::Event, raw: &[u8]) -> Result<()> {
    let spec = Spec::parse(&ctx.spec)?;
    let receiver = 1 - event.core.role as usize;
    let value = old::decode(raw, 1024)?;
    let record = value.fields(2)?;
    let f = record[0].fields(8)?;
    ensure!(
        f[0].uint()? == 1
            && f[1].fixed::<32>()? == event.core.d
            && f[2].fixed::<32>()? == event.core.e
            && f[3].uint()? == event.core.role
            && f[4].uint()? == event.core.n
            && f[5].fixed::<32>()? == event.id()?
            && f[6].fixed::<32>()? == spec.cert_refs()?[receiver],
        "signed opposite receipt exact source"
    );
    spec.peers[receiver].cert.device_id().verify(
        &old::signing_bytes(old::RECEIPT_SIGN, &record[0], 1024)?,
        record[1].bytes()?,
    )?;
    Ok(())
}

#[derive(Clone)]
pub(in super::super) struct PendingRow {
    pub ordinal: u64,
    pub id: old::Id,
    pub raw: Vec<u8>,
}

/// The narrow P2/P3 gap only. Rows may come from either storage backend;
/// collection and lease ownership belong to the caller, never this comparator.
pub(in super::super) fn verify_pending(b: &Legacy, rows: &[PendingRow]) -> Result<()> {
    b.role(1)?;
    let ctx = &b.contexts[0];
    let spec = Spec::parse(&ctx.spec)?;
    let head = state::Head::parse(&ctx.head, ctx.c, spec.cert_refs()?[1], 1)?;
    ensure!(rows.len() == 8, "exact eight pending rows");
    let mut seen = BTreeSet::new();
    for row in rows {
        ensure!(
            (17..=24).contains(&row.ordinal),
            "only pending B17..24, not held B16"
        );
        ensure!(seen.insert(row.ordinal), "duplicate pending ordinal");
        let event = old::Event::parse(&row.raw, spec.peers[1].cert.device_id())?;
        ensure!(
            event.core.c == ctx.c
                && Some(event.core.d) == head.d
                && Some(event.core.e) == head.e
                && event.core.role == 1
                && event.core.kind == 3
                && event.core.n == row.ordinal,
            "pending exact signed source/context/lane/ordinal"
        );
        ensure!(event.id()? == row.id, "pending exact event ID");
        ensure!(
            ctx.events.get(&(1, row.ordinal)) == Some(&row.raw),
            "pending raw bytes equal retained B source"
        );
    }
    ensure!(
        seen == (17..=24).collect(),
        "complete exact pending ordinal set"
    );
    Ok(())
}
