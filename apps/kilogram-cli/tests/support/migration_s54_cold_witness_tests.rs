//! Fresh actual Engine/Native origins only; no historical fixture consumers.
use super::cold_witness_fixture as fixture;
use super::*;
use kilogram_ratchet::ordered::native_call_counts;
type Repo = db::migration7::live1::L1Repository;

fn writes(repo: &Repo) -> Result<[u64; 6]> {
    let s = repo.backend_stats()?;
    Ok([
        s.writes,
        s.write_bytes,
        s.set_len_calls,
        s.growth_bytes,
        s.sync_calls,
        s.length,
    ])
}
// In-memory comparison only. No source, Native, root or key export is persisted.
fn fingerprint(repo: &mut Repo) -> Result<old::Id> {
    let mut bytes = Vec::new();
    let mut add = |raw: &[u8]| {
        bytes.extend_from_slice(&(raw.len() as u64).to_be_bytes());
        bytes.extend_from_slice(raw);
    };
    add(&repo.generation().to_be_bytes());
    add(&repo.last_commit()?);
    add(&repo.source_id());
    add(&repo.migration_id());
    add(&serde_json::to_vec(&repo.accounting()?)?);
    add(&serde_json::to_vec(&repo.credits()?)?);
    for slot in 0..2 {
        let s = repo.cold_ready_snapshot(slot)?;
        add(&repo.spec(slot)?);
        add(&s.head);
        add(s.native.as_deref().context("actual Native")?);
        let counts = repo.source_counts(slot)?;
        for (role, n) in counts.lanes.into_iter().enumerate() {
            add(&n.to_be_bytes());
            for ordinal in 0..n {
                add(&repo
                    .event_at(slot, role as u8, ordinal)?
                    .context("committed source")?);
            }
        }
        add(&counts.projections.to_be_bytes());
        add(&counts.requests.to_be_bytes());
        let runtime = repo.counts(slot)?;
        for n in [
            runtime.sources,
            runtime.queued,
            runtime.debt,
            runtime.pending,
            runtime.receipts[0],
            runtime.receipts[1],
        ] {
            add(&n.to_be_bytes());
        }
        for q in repo.queue_page(slot, None, 32)? {
            add(&q.sequence.to_be_bytes());
            add(&q.request);
            add(&q.sealed);
        }
    }
    Ok(*blake3::hash(&bytes).as_bytes())
}
fn initialize(repo: &mut Repo) -> Result<()> {
    let mut pairs = Vec::new();
    let mut pins = BTreeMap::new();
    for slot in 0..2 {
        let spec = Spec::parse(&repo.spec(slot)?)?;
        let pair = spec
            .peers
            .each_ref()
            .map(|p| *p.cert.account_id().as_bytes());
        for account in pair {
            let raw = repo.authority(account)?;
            let revision =
                kilogram_identity::AccountAuthoritySnapshot::decode_and_verify(&raw)?.revision();
            pins.insert(
                account,
                db::migration7::live1::L1Authority {
                    account,
                    revision,
                    raw,
                },
            );
        }
        pairs.push(pair);
    }
    repo.initialize(pairs, pins.into_values().collect())?;
    Ok(())
}

fn positive(purpose: Purpose) -> Result<()> {
    let mut f = fixture::Fresh::legacy(purpose)?;
    let witnesses = [f.capture(0)?, f.capture(1)?];
    f.migrate()?;
    for (owner, witness) in witnesses.iter().enumerate() {
        let node = &f.nodes[owner];
        let mut repo = f.open(owner)?;
        let before = fingerprint(&mut repo)?;
        let before_writes = writes(&repo)?;
        let calls = native_call_counts();
        let endpoint = witness.migrated_endpoint(node, &f.ready[owner], &mut repo)?;
        ensure!(
            endpoint.lanes[0].next == f.next(owner)[0] && endpoint.lanes[1].next == [1, 1],
            "actual cold next, no queue advance"
        );
        let expected_point = if purpose == Purpose::FeasibilityF {
            (2, 2)
        } else if owner == 0 {
            (16, 15)
        } else {
            (16, 16)
        };
        ensure!(
            endpoint.point()? == expected_point,
            "genesis subtracted exactly once"
        );
        ensure!(
            repository_end(&mut repo, &f.contexts, ExpectedStart::Strict).is_err(),
            "warm expectation rejects actual cold storage"
        );
        // The ledger uses actual legacy/current distinctions, not the shared
        // Pair.starts total. These are component ledger records, no child spawn.
        let mut ledger = intervals::Ledger::default();
        ensure!(
            ledger.expected_current_start(owner as u8).is_err()
                && ledger.expected_current_start(2).is_err(),
            "migration/owner required"
        );
        for lifetime in 1..=2 {
            let i = ledger.started(intervals::Started {
                owner: owner as u8,
                lifetime,
                pid: 100 + owner as u32,
                endpoint: None,
            })?;
            ledger.waited(
                i,
                100 + owner as u32,
                None,
                (lifetime - 1) as usize,
                Some(1),
                intervals::WaitKind::Killed,
            )?;
        }
        ledger.migration(owner as u8, endpoint.clone())?;
        let phase = ledger.expected_current_start(owner as u8)?;
        ensure!(
            matches!(phase, ExpectedStart::FirstCurrent(_)),
            "first current remains cold after two legacy lifetimes"
        );
        let end = repository_end(&mut repo, &f.contexts, phase)?;
        ensure!(
            end.endpoint == endpoint && end.point == expected_point,
            "shared pre-spawn endpoint equals migration"
        );
        for field in 0..3 {
            let mut wrong = endpoint.clone();
            match field {
                0 => wrong.source[0] ^= 1,
                1 => wrong.lanes[0].next[0] += 1,
                _ => wrong.lanes[0].slot = wrong.lanes[1].slot,
            };
            ensure!(
                repository_end(&mut repo, &f.contexts, ExpectedStart::FirstCurrent(&wrong))
                    .is_err(),
                "changed migration source/frontier/mapping refused before spawn"
            );
        }
        ledger.started(intervals::Started {
            owner: owner as u8,
            lifetime: 3,
            pid: 200 + owner as u32,
            endpoint: Some(endpoint.clone()),
        })?;
        ensure!(
            matches!(
                ledger.expected_current_start(owner as u8)?,
                ExpectedStart::Strict
            ),
            "prior indexed start forces strict phase"
        );
        ensure!(
            repository_end(
                &mut repo,
                &f.contexts,
                ledger.expected_current_start(owner as u8)?
            )
            .is_err(),
            "unexpectedly cold restart never falls back"
        );
        ensure!(
            fingerprint(&mut repo)? == before
                && writes(&repo)? == before_writes
                && native_call_counts() == calls,
            "shared cold reads do not mutate durable state or perform Native work"
        );
        drop(repo);
        // Also exercise the real after_migration wrapper with its original
        // directory/ready/profile gates, not just the borrowed verifier.
        ensure!(
            witness.after_migration(node, &f.ready[owner])? == endpoint,
            "actual after_migration wrapper"
        );
        let mut repo = f.open(owner)?;
        ensure!(
            fingerprint(&mut repo)? == before && native_call_counts() == calls,
            "authenticated state and source bytes unchanged across reopen"
        );
        initialize(&mut repo)?; // Legitimate separate warm control, outside read-only interval.
        let warm_writes = writes(&repo)?;
        let warm_generation = repo.generation();
        let warm_commit = repo.last_commit()?;
        let warm = repository_end(&mut repo, &f.contexts, ExpectedStart::Strict)?;
        ensure!(
            warm.endpoint == endpoint,
            "strict initialized restart/end endpoint retains original lanes"
        );
        ensure!(
            repository_end(
                &mut repo,
                &f.contexts,
                ExpectedStart::FirstCurrent(&endpoint)
            )
            .is_err()
                && witness
                    .migrated_endpoint(node, &f.ready[owner], &mut repo)
                    .is_err(),
            "global-present warm state cannot use cold paths"
        );
        ensure!(
            writes(&repo)? == warm_writes
                && repo.generation() == warm_generation
                && repo.last_commit()? == warm_commit
                && native_call_counts() == calls,
            "warm control reads immutable with zero Native work"
        );
        drop(repo);
        let mut repo = f.open(owner)?;
        ensure!(
            repository_end(&mut repo, &f.contexts, ExpectedStart::Strict)?.endpoint == endpoint,
            "strict warm endpoint persists across reopen"
        );
        println!(
            "af_positive_purpose={};owner={owner};next={:?};point={expected_point:?};shared_after_migration_and_pre_spawn=true;legacy_lifetimes=2;later_current_strict=true;read_only_across_reopen=true;zero_extra_Native=true",
            if purpose == Purpose::FeasibilityF {
                "F"
            } else {
                "P"
            },
            f.next(owner)
        );
    }
    Ok(())
}

#[test]
fn af_real_f_cold_witness_first_start_and_strict_warm_restart() -> Result<()> {
    positive(Purpose::FeasibilityF)
}
#[test]
fn af_real_p_cold_witness_first_start_and_strict_warm_restart() -> Result<()> {
    positive(Purpose::WholeP)
}

#[test]
fn af_real_witness_substitutions_and_cardinality_are_refused() -> Result<()> {
    let mut f = fixture::Fresh::legacy(Purpose::FeasibilityF)?;
    let original = f.capture(0)?;
    f.migrate()?;
    let mut repo = f.open(0)?;
    let before = fingerprint(&mut repo)?;
    let before_writes = writes(&repo)?;
    let calls = native_call_counts();
    original.migrated_endpoint(&f.nodes[0], &f.ready[0], &mut repo)?;
    let mut refused = 0;
    for case in 0..17 {
        let mut wrong = original.clone();
        match case {
            0 => wrong.source_id[0] ^= 1,
            1 => wrong.state = f.nodes[1].branch.state.clone(),
            2 => wrong.signing[0] ^= 1,
            3 => wrong.encryption[0] ^= 1,
            4 => wrong.contexts[0].c[0] ^= 1,
            5 => wrong.contexts[0].role = 1,
            6 => wrong.contexts[0].spec[0] ^= 1,
            7 => wrong.contexts[0].head[0] ^= 1,
            8 => wrong.contexts[0].native[0] ^= 1,
            9 => wrong.contexts[0].events.get_mut(&(0, 1)).context("Text")?[0] ^= 1,
            10 => {
                wrong.contexts[0].events.remove(&(0, 1));
            }
            11 => {
                wrong.contexts[0].events.insert((0, 3), vec![0]);
            }
            12 => {
                let raw = wrong.contexts[0].events.remove(&(0, 1)).context("Text")?;
                wrong.contexts[0].events.insert((0, 3), raw);
            }
            13 => {
                wrong.contexts[0]
                    .queue
                    .values_mut()
                    .next()
                    .context("queue")?[0] ^= 1
            }
            14 => {
                let (k, v) = wrong.contexts[0].queue.pop_first().context("queue")?;
                let mut changed = k;
                changed[0] ^= 1;
                wrong.contexts[0].queue.insert(changed, v);
            }
            15 => {
                wrong.contexts[0].queue.insert([0; 16], vec![0]);
            }
            _ => {
                wrong.contexts[0].queue.pop_first();
            }
        }
        ensure!(
            wrong
                .migrated_endpoint(&f.nodes[0], &f.ready[0], &mut repo)
                .is_err(),
            "expected witness substitution {case} refused"
        );
        refused += 1;
    }
    let mut wrong_ready = f.ready[0].clone();
    wrong_ready["contexts"]
        .as_array_mut()
        .context("contexts")?
        .swap(0, 1);
    ensure!(
        original
            .migrated_endpoint(&f.nodes[0], &wrong_ready, &mut repo)
            .is_err(),
        "wrong Ready slot mapping refused"
    );
    refused += 1;
    for field in 0..2 {
        {
            let (a, b) = f.nodes.split_at_mut(1);
            if field == 0 {
                std::mem::swap(&mut a[0].branch.account, &mut b[0].branch.account);
            } else {
                std::mem::swap(&mut a[0].branch.device, &mut b[0].branch.device);
            }
        }
        ensure!(
            original
                .migrated_endpoint(&f.nodes[0], &f.ready[0], &mut repo)
                .is_err(),
            "wrong declared account/device refused"
        );
        {
            let (a, b) = f.nodes.split_at_mut(1);
            if field == 0 {
                std::mem::swap(&mut a[0].branch.account, &mut b[0].branch.account);
            } else {
                std::mem::swap(&mut a[0].branch.device, &mut b[0].branch.device);
            }
        }
        refused += 1;
    }
    ensure!(
        fingerprint(&mut repo)? == before
            && writes(&repo)? == before_writes
            && native_call_counts() == calls,
        "all expected-witness negatives leave actual state unchanged"
    );
    println!(
        "af_actual_witness_substitution_negatives={refused};extra_missing_and_noncontiguous_sources_refused=true;queue_bytes_ID_cardinality_refused=true;owner_source_context_role_slot_refused=true;read_only_zero_Native=true"
    );
    Ok(())
}

#[test]
fn af_actual_f_p_ready_before_strict_metadata_refusal() -> Result<()> {
    let mut owners = 0;
    for purpose in [Purpose::FeasibilityF, Purpose::WholeP] {
        let mut f = fixture::Fresh::legacy(purpose)?;
        f.migrate()?;
        for owner in 0..2 {
            let mut repo = f.open(owner)?;
            ensure!(
                !repo.initialized()?,
                "actual Ready has no current owner initialization"
            );
            let expected = f.next(owner);
            for (logical, c) in f.contexts.iter().enumerate() {
                let slot = oracle::ready_slot(&f.ready[owner], c)?;
                ensure!(
                    slot == 1 - logical as u8,
                    "nontrivial logical/physical mapping"
                );
                ensure!(
                    repo.source_counts(slot)?.lanes == expected[logical],
                    "actual authenticated committed lanes"
                );
                ensure!(
                    matches!(
                        repo.snapshot(slot),
                        Err(db::Error::Integrity("L1 owner not initialized"))
                    ),
                    "original strict metadata refusal on genuine populated Ready"
                );
            }
            let c1 = oracle::ready_slot(&f.ready[owner], &f.contexts[0])?;
            ensure!(
                repo.counts(c1)?.queued == if owner == 0 { 2 } else { 0 },
                "actual queued-not-committed distinction"
            );
            owners += 1;
        }
    }
    println!(
        "af_actual_f_p_ready_baseline_owners={owners};contexts_per_owner=2;strict_missing_meta=true;real_Engine_Native_and_outbox=true;nontrivial_mapping=true;zero_migration_Native_calls=true"
    );
    Ok(())
}
