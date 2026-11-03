//! Actual fresh P storage/crypto. Both traffic engines are legacy; no mixed
//! runtime, process, transport or measured memory evidence is claimed here.
use super::*;
use cold_witness_fixture::Fresh;
use control::MigrationSources;
use kilogram_ratchet::ordered::native_call_counts;
use kilogram_state::StateDirectoryLock;
use memory_scope::tests::DeferredModel;

type Repo = db::migration7::live1::L1Repository;

// Exact cold comparison shared by positive checks and in-memory substitutions.
// Expected values below are captured from authenticated real legacy records.
#[derive(Clone)]
struct Exact {
    queue: BTreeMap<[u8; 16], Vec<u8>>,
    debt: Vec<(u64, old::Id)>,
    receipts: BTreeMap<(u8, u64), Vec<u8>>,
}
impl Exact {
    fn capture(f: &Fresh, owner: usize, witness: &Legacy) -> Result<Self> {
        let ctx = &witness.contexts[0];
        let source = db::migration7::V4Source::open(
            StateDirectoryLock::acquire(&f.nodes[owner].branch.state)?,
            db::Config::default(),
        )?;
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
            queue: ctx.queue.clone(),
            debt,
            receipts: ctx.receipts.clone(),
        })
    }
    fn verify(&self, repo: &mut Repo, slot: u8, witness: &Legacy) -> Result<()> {
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

fn verify_receipt(ctx: &SavedContext, event: &old::Event, raw: &[u8]) -> Result<()> {
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

fn cold_unchanged(repo: &Repo) -> Result<(u64, old::Id, [u64; 6])> {
    let b = repo.backend_stats()?;
    Ok((
        repo.generation(),
        repo.last_commit()?,
        [
            b.writes,
            b.write_bytes,
            b.set_len_calls,
            b.growth_bytes,
            b.sync_calls,
            b.length,
        ],
    ))
}

fn verify_binding(
    f: &Fresh,
    owner: usize,
    witness: Legacy,
    endpoint: intervals::Endpoint,
) -> Result<()> {
    let other = 1 - owner;
    ensure!(
        f.nodes[owner].indexed && !f.nodes[other].indexed && f.ready[other].is_null(),
        "only designated owner migrated"
    );
    let witnesses = std::array::from_fn(|i| (i == owner).then(|| witness.clone()));
    let endpoints = std::array::from_fn(|i| (i == owner).then(|| endpoint.clone()));
    let rows = [(format!("ready-{owner}"), f.ready[owner].clone())].into();
    let sources = MigrationSources::new(&f.contexts, &witnesses, &rows, &endpoints);
    ensure!(
        sources.source(owner)? == witness.source_id && sources.source(other).is_err(),
        "AN real single-owner verified source"
    );
    // Only lifecycle records are injected; binding calls the unchanged AO seam.
    let model = DeferredModel::enter()?;
    model.legacy(0, 100, true)?;
    model.legacy(1, 101, true)?;
    ensure!(
        model.source(0)?.is_none() && model.source(1)?.is_none(),
        "no genesis binding"
    );
    sources.bind_memory(owner)?;
    ensure!(
        model.source(owner)? == Some(witness.source_id)
            && model.bound(owner as u8)? == 1
            && model.source(other)?.is_none()
            && model.bound(other as u8)? == 0,
        "deferred binding affects only actually migrated owner"
    );
    ensure!(!model.failed()?, "injected observer healthy");
    Ok(())
}

fn substitutions(
    exact: &Exact,
    repo: &mut Repo,
    slot: u8,
    witness: &Legacy,
    queue: bool,
) -> Result<usize> {
    let before = cold_unchanged(repo)?;
    for case in 0..4 {
        let mut wrong = exact.clone();
        if queue {
            match case {
                0 => {
                    wrong.queue.pop_first();
                }
                1 => {
                    wrong.queue.insert([0; 16], vec![0]);
                }
                2 => {
                    let (mut id, raw) = wrong.queue.pop_first().context("real queue")?;
                    id[0] ^= 1;
                    wrong.queue.insert(id, raw);
                }
                _ => {
                    wrong.queue.values_mut().next().context("queue bytes")?[0] ^= 1;
                }
            }
        } else {
            match case {
                0 => {
                    wrong.debt.pop();
                }
                1 => {
                    wrong.debt.push((25, [0; 32]));
                }
                2 => {
                    wrong.debt[0].1[0] ^= 1;
                }
                _ => {
                    // Counterfactual receipt at an actual debt slot; never persisted.
                    let raw = wrong
                        .receipts
                        .values()
                        .next()
                        .context("signed receipt")?
                        .clone();
                    wrong.receipts.insert((1, 16), raw);
                }
            }
        }
        ensure!(
            wrong.verify(repo, slot, witness).is_err(),
            "exact verifier refuses substitution {case}"
        );
    }
    ensure!(
        cold_unchanged(repo)? == before,
        "substitutions never mutate cold repository"
    );
    Ok(4)
}

#[test]
fn ap_real_p_first_a_storage_roles_and_slots() -> Result<()> {
    for slot in [0, 1] {
        let (mut f, genesis) = Fresh::p_with_genesis(slot, false)?;
        let a = f.capture(0)?;
        let b = f.capture(1)?;
        genesis[0].same_identity(&a)?;
        genesis[1].same_identity(&b)?;
        ensure!(
            genesis[0].source_id != a.source_id && genesis[1].source_id != b.source_id,
            "actual genesis to P source growth"
        );
        let exact = Exact::capture(&f, 0, &a)?;
        let expected = [17, 18]
            .map(|n| oracle::Request::new(0, 0, n).and_then(|r| parse_request_id(&request_id(r))))
            .into_iter()
            .collect::<Result<BTreeSet<_>>>()?;
        ensure!(
            exact.queue.keys().copied().collect::<BTreeSet<_>>() == expected
                && exact.debt.is_empty(),
            "exact A17/18 requests with validated sealed bytes; own debt zero"
        );
        let calls = native_call_counts();
        f.migrate_owner(0)?;
        let endpoint = a.after_migration(&f.nodes[0], &f.ready[0])?;
        ensure!(
            endpoint.lanes[0].next == [17, 16]
                && endpoint.lanes[0].slot == slot
                && endpoint.lanes[1].next == [1, 1],
            "early A exact committed frontiers and physical mapping"
        );
        let mut repo = f.open(0)?;
        let before = cold_unchanged(&repo)?;
        exact.verify(&mut repo, slot, &a)?;
        let refused = if slot == 0 {
            substitutions(&exact, &mut repo, slot, &a, true)?
        } else {
            0
        };
        ensure!(
            cold_unchanged(&repo)? == before && native_call_counts() == calls,
            "cold reads perform no writes or Native work"
        );
        drop(repo);
        let after_b = f.capture(1)?;
        b.same_identity(&after_b)?;
        ensure!(
            b.source_id == after_b.source_id,
            "B unmigrated storage untouched by A conversion"
        );
        verify_binding(&f, 0, a, endpoint)?;
        println!(
            "ap_first_A=passed;C1slot={slot};C1=17_16;C2=1_1;queue17_18_exact=true;genesis_growth=real;only_A_migrated=true;B_unbound=true;queue_negatives={refused};cold_zero_Native=true;ledger_observations=injected;runtime_transport=not_established"
        );
    }
    Ok(())
}

#[test]
fn ap_real_p_late_b_storage_roles_and_slots() -> Result<()> {
    for slot in [0, 1] {
        let (mut f, genesis) = Fresh::p_with_genesis(slot, true)?;
        let a = f.capture(0)?;
        let b = f.capture(1)?;
        genesis[0].same_identity(&a)?;
        genesis[1].same_identity(&b)?;
        ensure!(
            genesis[0].source_id != a.source_id && genesis[1].source_id != b.source_id,
            "late source growth real"
        );
        let exact_a = Exact::capture(&f, 0, &a)?;
        let exact = Exact::capture(&f, 1, &b)?;
        let ctx = &b.contexts[0];
        let spec = Spec::parse(&ctx.spec)?;
        let expected = (16..=24)
            .map(|n| {
                let raw = ctx.events.get(&(1, n)).context("B16..24 actual source")?;
                Ok((
                    n,
                    old::Event::parse(raw, spec.peers[1].cert.device_id())?.id()?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            exact.debt == expected && exact.queue.is_empty(),
            "nine observed exact B16..24 debts; no queue"
        );
        for n in 1..=24 {
            ensure!(
                ctx.receipts.contains_key(&(1, n)) == (n <= 15),
                "B1..15 genuinely retired; B16..24 no opposite receipt"
            );
        }
        let a24 = old::Event::parse(
            ctx.events.get(&(0, 24)).context("A24 at B")?,
            spec.peers[0].cert.device_id(),
        )?;
        let r1 = ctx.receipts.get(&(0, 24)).context("actual saved R1(A24)")?;
        verify_receipt(ctx, &a24, r1)?;
        ensure!(
            exact_a.debt == vec![(24, a24.id()?)] && !exact_a.receipts.contains_key(&(0, 24)),
            "same actual A24 debt while signed R1 retained at B"
        );
        let calls = native_call_counts();
        f.migrate_owner(1)?;
        let endpoint = b.after_migration(&f.nodes[1], &f.ready[1])?;
        ensure!(
            endpoint.lanes[0].next == [25, 25]
                && endpoint.lanes[0].slot == slot
                && endpoint.lanes[1].next == [1, 1],
            "late B exact frontiers and mapping"
        );
        let mut repo = f.open(1)?;
        let before = cold_unchanged(&repo)?;
        exact.verify(&mut repo, slot, &b)?;
        ensure!(
            repo.receipt(slot, 0, 24)?.as_ref() == Some(r1),
            "exact cold saved R1(A24) bytes"
        );
        let refused = if slot == 0 {
            substitutions(&exact, &mut repo, slot, &b, false)?
        } else {
            0
        };
        ensure!(
            cold_unchanged(&repo)? == before && native_call_counts() == calls,
            "late cold reads immutable and zero Native"
        );
        drop(repo);
        ensure!(
            a.source_id == f.capture(0)?.source_id,
            "legacy A source untouched by B migration"
        );
        verify_binding(&f, 1, b, endpoint)?;
        println!(
            "ap_late_B=passed;C1slot={slot};C1=25_25;C2=1_1;B_debt16_24_exact=9;B1_15_signed_receipts=true;A24_debt_and_saved_R1_exact=true;A_pending17_24_exact=8;only_B_migrated=true;debt_receipt_negatives={refused};cold_zero_Native=true;storage_only=true;ledger_observations=injected;mixed_runtime=not_established"
        );
    }
    Ok(())
}
