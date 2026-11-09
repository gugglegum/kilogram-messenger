//! Actual fresh P storage/crypto. Both traffic engines are legacy; no mixed
//! runtime, process, transport or measured memory evidence is claimed here.
use super::*;
use boundary::{Exact, PendingRow, Proof, verify_pending, verify_receipt};
use cold_witness_fixture::Fresh;
use control::MigrationSources;
use kilogram_ratchet::ordered::native_call_counts;
use kilogram_state::StateDirectoryLock;
use memory_scope::tests::DeferredModel;

type Repo = db::migration7::live1::L1Repository;

// Component-only caller: one owned source lease, released on return.
fn capture(f: &Fresh, owner: usize) -> Result<Proof> {
    let source = db::migration7::V4Source::open(
        StateDirectoryLock::acquire(&f.nodes[owner].branch.state)?,
        db::Config::default(),
    )?;
    Proof::capture_source(&f.nodes[owner], &f.contexts, f.purpose, &source)
}

fn read_pending(f: &Fresh, a: &Legacy, b: &Legacy) -> Result<Vec<PendingRow>> {
    let ctx = &b.contexts[0];
    let spec = Spec::parse(&ctx.spec)?;
    let source = db::migration7::V4Source::open(
        StateDirectoryLock::acquire(&f.nodes[0].branch.state)?,
        db::Config::default(),
    )?;
    ensure!(
        source.source_id() == a.source_id,
        "same stopped real pending inventory"
    );
    let prefix = format!("{}pending/", engine::prefix(ctx.c));
    let mut rows = Vec::new();
    for entry in source
        .entries()
        .iter()
        .filter(|e| e.path.starts_with(&prefix))
    {
        let ordinal = entry
            .path
            .rsplit('/')
            .next()
            .context("pending ordinal suffix")?
            .parse()?;
        let raw = source.required(&entry.path)?.to_vec();
        let id = old::Event::parse(&raw, spec.peers[1].cert.device_id())?.id()?;
        rows.push(PendingRow { ordinal, id, raw });
    }
    Ok(rows)
}

fn pending_substitutions(b: &Legacy, rows: &[PendingRow]) -> Result<usize> {
    let ctx = &b.contexts[0];
    let spec = Spec::parse(&ctx.spec)?;
    // These replacements are already genuinely signed, retained source events.
    let signed = |ctx: &SavedContext, lane: u8, n: u64, slot: u64| -> Result<PendingRow> {
        let raw = ctx
            .events
            .get(&(lane, n))
            .context("real substitution source")?
            .clone();
        let id = old::Event::parse(&raw, spec.peers[lane as usize].cert.device_id())?.id()?;
        Ok(PendingRow {
            ordinal: slot,
            id,
            raw,
        })
    };
    for case in 0..8 {
        let mut wrong = rows.to_vec();
        match case {
            0 => {
                wrong.pop();
            }
            1 => {
                wrong.push(signed(ctx, 1, 16, 16)?);
            }
            2 => {
                wrong[7] = wrong[0].clone();
            }
            3 => {
                wrong[0].id[0] ^= 1;
            }
            4 => {
                wrong[0] = signed(ctx, 0, 17, 17)?;
            }
            5 => {
                wrong[0] = signed(&b.contexts[1], 1, 0, 17)?;
            }
            6 => {
                *wrong[0].raw.last_mut().context("signed packet bytes")? ^= 1;
            }
            _ => {
                wrong[0] = signed(ctx, 1, 16, 16)?;
            }
        }
        ensure!(
            verify_pending(b, &wrong).is_err(),
            "shared pending comparator refuses substitution {case}"
        );
    }
    // Ordinary row order does not affect the exact set; duplicates do.
    let mut reversed = rows.to_vec();
    reversed.reverse();
    verify_pending(b, &reversed)?;
    Ok(8)
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
        let Proof { witness: a, exact } = capture(&f, 0)?;
        let b = f.capture(1)?;
        genesis[0].same_identity(&a)?;
        genesis[1].same_identity(&b)?;
        ensure!(
            genesis[0].source_id != a.source_id && genesis[1].source_id != b.source_id,
            "actual genesis to P source growth"
        );
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
        let Proof {
            witness: a,
            exact: exact_a,
        } = capture(&f, 0)?;
        let Proof { witness: b, exact } = capture(&f, 1)?;
        let pending = read_pending(&f, &a, &b)?;
        verify_pending(&b, &pending)?;
        let pending_refused = if slot == 0 {
            pending_substitutions(&b, &pending)?
        } else {
            0
        };
        genesis[0].same_identity(&a)?;
        genesis[1].same_identity(&b)?;
        ensure!(
            genesis[0].source_id != a.source_id && genesis[1].source_id != b.source_id,
            "late source growth real"
        );
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
            "ap_late_B=passed;C1slot={slot};C1=25_25;C2=1_1;B_debt16_24_exact=9;B1_15_signed_receipts=true;A24_debt_and_saved_R1_exact=true;A_pending17_24_exact=8;only_B_migrated=true;shared_pending=true;pending_negatives={pending_refused};debt_receipt_negatives={refused};cold_zero_Native=true;storage_only=true;ledger_observations=injected;mixed_runtime=not_established"
        );
    }
    Ok(())
}
