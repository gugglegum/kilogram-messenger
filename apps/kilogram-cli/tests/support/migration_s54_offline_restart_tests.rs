//! Real fresh Engine/Native semantics; status rows and lifetime records below
//! are injected component inputs, not actual process admission evidence.
use super::super::cold_witness_fixture::Fresh;
use super::*;
use kilogram_cli::ordered_dialog::{
    admin,
    engine::{Changes, Engine},
    runtime,
};
use kilogram_ratchet::ordered::native_call_counts;
use kilogram_state::{OrderedVault, StateDirectoryLock};

fn open(f: &Fresh, owner: usize) -> Result<Engine> {
    Engine::new(OrderedVault::open(StateDirectoryLock::acquire(
        &f.nodes[owner].branch.state,
    )?)?)
}
fn profile(f: &Fresh, owner: usize) -> Profile {
    let n = &f.nodes[owner];
    Profile {
        version: 1,
        profile_mode: "ordered-dialog-v1".into(),
        state_dir: n.branch.state.clone(),
        ipc_file: n.ipc.clone(),
        locator_directory: n.locators.clone(),
        enabled_contexts: f.contexts.to_vec(),
        bind: "127.0.0.1:0".into(),
    }
}
fn capture(f: &Fresh, owner: usize) -> Result<Snapshot> {
    let n = &f.nodes[owner];
    let source = db::migration7::V4Source::open(
        StateDirectoryLock::acquire(&n.branch.state)?,
        db::Config::default(),
    )?;
    Snapshot::capture_source(
        n,
        &f.contexts,
        f.purpose,
        &source,
        &Profile::load(&n.branch.config)?,
    )
}
// Genuine issue + normal non-Native save, exactly the frozen runtime operation.
// No target Head/Native/frontier is written directly by this fixture.
fn renew(f: &Fresh, owner: usize, engine: &mut Engine) -> Result<Vec<Vec<u8>>> {
    let mut raws = Vec::new();
    for c in &f.contexts {
        let c = state::parse_id(c)?;
        let revision = match engine
            .store
            .get(&format!("{}local-locator", engine::prefix(c)))?
        {
            Some(raw) => Descriptor::parse(engine.spec(c)?, owner as u64, &raw)?.revision + 1,
            None => 1,
        };
        let descriptor = Descriptor::issue(
            engine.spec(c)?,
            owner as u64,
            admin::random()?,
            vec!["127.0.0.1:49152".parse()?],
            revision,
            engine::now()?,
            engine.device.identity(),
        )?;
        let mut changes = Changes::default();
        changes.put(
            format!("{}local-locator", engine::prefix(c)),
            descriptor.raw.clone(),
        );
        let mut h = engine.head(c)?;
        h.bump(false)?;
        engine.save(&mut h, None, changes)?;
        // Runtime renewal atomically replaces the old public locator.
        let mut temp = tempfile::NamedTempFile::new_in(&f.nodes[owner].locators)?;
        use std::io::Write;
        temp.write_all(&descriptor.raw)?;
        temp.as_file().sync_all()?;
        temp.persist(
            f.nodes[owner]
                .locators
                .join(format!("{}.locator", state::hex(&c))),
        )
        .map_err(|e| e.error)
        .context("fresh fixture locator renewal")?;
        raws.push(descriptor.raw);
    }
    Ok(raws)
}
fn setup(f: &Fresh) -> Result<()> {
    for owner in 0..2 {
        std::fs::create_dir_all(&f.nodes[owner].locators)?;
        admin::write_public(
            &f.nodes[owner].branch.config,
            &serde_json::to_vec(&profile(f, owner))?,
            &f.nodes[owner].branch.state,
        )?;
    }
    let mut a = open(f, 0)?;
    let mut b = open(f, 1)?;
    let a_raw = renew(f, 0, &mut a)?;
    let b_raw = renew(f, 1, &mut b)?;
    for (i, c) in f.contexts.iter().enumerate() {
        let c = state::parse_id(c)?;
        runtime::import_locator(&mut a, c, &b_raw[i])?;
        runtime::import_locator(&mut b, c, &a_raw[i])?;
    }
    Ok(())
}
fn injected_status(a: &Snapshot) -> [Value; 2] {
    std::array::from_fn(|i| {
        let p = &a.lanes[i].position;
        json!({"context":state::hex(&a.witness().contexts[i].c),"phase":3,
            "native_generation":p.native_generation,"session_id":p.sid,"send_next":p.next[0],
            "receive_next":p.next[1],"own_generation":p.generations[0],"peer_generation":p.generations[1],
            "pending":0,"unacked":0,"status":"ConnectionBackoff: injected component input"})
    })
}
#[test]
fn aq3_real_semantic_preservation_and_refusals() -> Result<()> {
    for slot in 0..2 {
        let (mut f, genesis) = Fresh::p_before_queue(slot)?;
        setup(&f)?;
        let a = capture(&f, 0)?;
        let b = capture(&f, 1)?;
        for (original, current) in genesis.iter().zip([a.witness(), b.witness()]) {
            original.same_identity(current)?;
        }
        let now = engine::now()?;
        a.initial(&b, now)?;
        let rows = injected_status(&a);
        assert!(a.ready(&rows, &b, now)?);
        let mut refusals = 0;
        {
            let n = &f.nodes[0];
            let source = db::migration7::V4Source::open(
                StateDirectoryLock::acquire(&n.branch.state)?,
                db::Config::default(),
            )?;
            for case in 0..3 {
                let mut wrong = profile(&f, 0);
                match case {
                    0 => {
                        wrong.enabled_contexts.pop();
                    }
                    1 => wrong.enabled_contexts[1] = wrong.enabled_contexts[0].clone(),
                    _ => wrong.bind = "192.0.2.1:49152".into(),
                }
                assert!(
                    Snapshot::capture_source(n, &f.contexts, f.purpose, &source, &wrong).is_err()
                );
                refusals += 1;
            }
        }
        for case in 0..8 {
            let mut rows = rows.clone();
            match case {
                0 => rows[1] = Value::Null,
                1 => rows[1]["context"] = rows[0]["context"].clone(),
                2 => {
                    rows[0]
                        .as_object_mut()
                        .context("injected row")?
                        .remove("native_generation");
                }
                3 => rows[1]["peer_generation"] = u64::MAX.into(),
                4 => rows[0]["status"] = "Connected".into(),
                5 => rows[1]["pending"] = 1.into(),
                6 => rows[0]["session_id"] = "wrong".into(),
                _ => rows[0]["unacked"] = 1.into(),
            }
            assert!(!a.ready(&rows, &b, now)?);
            refusals += 1;
        }
        // Signed descriptor substitutions, never persisted to target storage.
        let b_engine = open(&f, 1)?;
        let c = state::parse_id(&f.contexts[0])?;
        let wrong_target = Descriptor::issue(
            b_engine.spec(c)?,
            1,
            admin::random()?,
            vec!["127.0.0.1:49153".parse()?],
            2,
            now,
            b_engine.device.identity(),
        )?
        .raw;
        let expired = Descriptor::issue(
            b_engine.spec(c)?,
            1,
            admin::random()?,
            vec!["127.0.0.1:49153".parse()?],
            2,
            now - 3601,
            b_engine.device.identity(),
        )?
        .raw;
        drop(b_engine);
        for case in 0..4 {
            let mut wrong = a.clone();
            let mut target = b.clone();
            match case {
                0 => wrong.lanes[0].peer = wrong.lanes[0].local.clone(),
                1 => {
                    wrong.lanes[0].peer = expired.clone();
                    target.lanes[0].local = expired.clone();
                }
                2 => wrong.lanes[0].peer = wrong_target.clone(),
                _ => target.lanes[0].local = wrong_target.clone(), // A retains valid older B target.
            }
            assert!(wrong.initial(&target, now).is_err());
            assert!(wrong.ready(&rows, &target, now).is_err());
            refusals += 1;
        }
        assert!(a.ready(&rows, &b, now + 3601).is_err());
        refusals += 1;
        let calls = native_call_counts();
        {
            let mut engine = open(&f, 0)?;
            renew(&f, 0, &mut engine)?;
            for n in [17, 18] {
                let r = oracle::Request::new(0, 0, n)?;
                engine.queue(c, parse_request_id(&request_id(r))?, &r.body())?;
            }
        }
        let after = capture(&f, 0)?;
        assert!(a.witness().source_id != after.witness().source_id);
        assert!(native_call_counts() == calls);
        // Mutations below are retained in-memory witness substitutions only.
        for case in 0..20 {
            let mut z = after.clone();
            match case {
                0 => z.proof.witness.contexts[0].native[0] ^= 1,
                1 => z.lanes[0].position.next[0] += 1,
                2 => z.proof.witness.signing[0] ^= 1,
                3 => {
                    z.proof.witness.contexts[0].receipts.pop_first();
                }
                4 => {
                    z.proof.witness.contexts[0].events.pop_first();
                }
                5 => {
                    z.proof.witness.contexts[0].history.pop_first();
                }
                6 => {
                    z.proof.exact.queue.insert([255; 16], vec![1]);
                }
                7 => z.proof.exact.debt.push((1, [1; 32])),
                8 => z.lanes[0].pending = 1,
                9 => z.lanes[1].pending = 1,
                10 => z.lanes[1].outbox = 1,
                11 => z.lanes[0].position.generations[0] += 1,
                12 => z.lanes[0].local = a.lanes[0].local.clone(),
                13 => z.profile.push(b' '),
                14 => z.unchanged.clear(),
                15 => z.proof.witness.contexts[0].keys[0].push('x'),
                16 => z.lanes[0].position.native_generation += 1,
                17 => z.proof.witness.contexts[0].spec[0] ^= 1,
                18 => z.proof.witness.contexts[1]
                    .queue
                    .insert([255; 16], vec![1])
                    .map(|_| ())
                    .unwrap_or(()),
                _ => {
                    z.proof.witness.contexts[0].head[0] ^= 1;
                }
            }
            assert!(
                a.finish(z, &b, now, &f.contexts).is_err(),
                "semantic refusal {case}"
            );
            refusals += 1;
        }
        let final_id = after.witness().source_id;
        let checked = a.finish(after, &b, engine::now()?, &f.contexts)?;
        assert!(checked.witness().source_id == final_id);
        f.migrate_owner(0)?;
        checked.after_migration(&f.nodes[0], &f.ready[0])?;
        assert!(native_call_counts() == calls);
        println!(
            "aq3_real_semantic_slot={slot} injected_status_refusals_and_memory_substitutions={refusals} retained_final_source_cold_verified=true actual_runtime_admission=false"
        );
    }
    Ok(())
}

#[test]
fn aq3_injected_lifetime_anchor_and_deadline() -> Result<()> {
    let mut ledger = intervals::Ledger::default();
    let old = ledger.started(intervals::Started {
        owner: 0,
        lifetime: 3,
        pid: 7,
        endpoint: None,
    })?;
    let new = intervals::Started {
        owner: 0,
        lifetime: 4,
        pid: 7,
        endpoint: None,
    };
    assert!(ledger.started(new.clone()).is_err()); // Existing unawaited/PID guard.
    ledger.waited(old, 7, None, 0, Some(9), intervals::WaitKind::Killed)?;
    let start = ledger.started(new)?; // Waited PID reuse remains legal.
    let outcomes = vec![
        json!({"owner":0,"pid":7,"waited":true,"code":9,"counter_outcome":"frozen process; outside current ledger"}),
    ];
    let now = Instant::now();
    let until = now + Duration::from_secs(30);
    let life = Life::new(&ledger, &outcomes, old, start, 123, until)?;
    life.check(&ledger, start, 123, now, until)?;
    assert!(Life::new(&ledger, &[], old, start, 123, until).is_err());
    let mut bad = outcomes.clone();
    bad[0]["waited"] = false.into();
    assert!(Life::new(&ledger, &bad, old, start, 123, until).is_err());
    assert!(Life::new(&ledger, &outcomes, old, start, 123, now).is_err());
    assert!(life.check(&ledger, old, 123, now, until).is_err());
    assert!(life.check(&ledger, start, 124, now, until).is_err());
    assert!(life.check(&ledger, start, 123, until, until).is_err());
    let mut bad = ledger.clone();
    bad.starts[start].lifetime += 1;
    assert!(life.check(&bad, start, 123, now, until).is_err());
    let mut bad = ledger.clone();
    bad.waits.clear();
    assert!(life.check(&bad, start, 123, now, until).is_err());
    ledger.waited(start, 7, None, 1, Some(9), intervals::WaitKind::Killed)?;
    assert!(life.check(&ledger, start, 123, now, until).is_err());
    println!("aq3_injected_lifetime_anchor_refusals=10 actual_child_execution=false");
    Ok(())
}
