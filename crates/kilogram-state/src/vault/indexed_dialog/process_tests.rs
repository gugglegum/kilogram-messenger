use super::*;
use std::time::{Duration, Instant};

#[test]
fn indexed_crash_child() -> TestResult {
    let Ok(root) = std::env::var("KILOGRAM_S1_CHILD_ROOT") else {
        return Ok(());
    };
    let pause = std::env::var("KILOGRAM_S1_PAUSE")?;
    let mut store = IndexedDialogVault::create(
        StateDirectoryLock::acquire(&root)?,
        claim(),
        Config::default(),
    )?;
    ARMED.store(true, Ordering::Release);
    if pause == "after_extension_sync" {
        for n in (0..4096).step_by(32) {
            let mut rows = Vec::new();
            for i in n..n + 32 {
                rows.push(Mutation::put(event(0, i)?, vec![9; 1024]));
            }
            apply(&mut store, rows, vec![])?;
        }
        return Err("expected extension checkpoint not reached".into());
    }
    store.commit(
        store.generation(),
        id(777),
        vec![Mutation::put(event(0, 0)?, b"durable fixture".to_vec())],
        vec![CreditAction::Queue {
            context: 0,
            sequence: 0,
            id: id(0),
            body: b"accepted queue fixture".to_vec(),
        }],
    )?;
    Err("expected primary checkpoint not reached".into())
}
#[test]
fn indexed_component_real_owned_child_kills() -> TestResult {
    for point in ["before_commit", "after_commit", "after_extension_sync"] {
        let dir = tempfile::tempdir()?;
        let root = dir.path().join("state");
        let marker = dir.path().join("paused");
        let exe = std::env::current_exe()?;
        let mut command = std::process::Command::new(&exe);
        command
            .args([
                "--exact",
                "vault::indexed_dialog::tests::processes::indexed_crash_child",
                "--nocapture",
            ])
            .env("KILOGRAM_S1_CHILD_ROOT", &root)
            .env("KILOGRAM_S1_PAUSE", point)
            .env("KILOGRAM_S1_MARKER", &marker)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_4000);
        }
        let mut child = command.spawn()?;
        let pid = child.id();
        let start = Instant::now();
        while !marker.exists() {
            if let Some(status) = child.try_wait()? {
                return Err(format!("own child exited before {point}: {status}").into());
            }
            if start.elapsed() > Duration::from_secs(60) {
                child.kill()?;
                child.wait()?;
                return Err(format!("own child checkpoint timeout {point}").into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        child.kill()?;
        child.wait()?;
        let mut store = IndexedDialogVault::open(
            StateDirectoryLock::acquire(&root)?,
            owner(),
            Config::default(),
        )?;
        let count = store.record_counts()?.0;
        match point {
            "before_commit" => {
                assert_eq!(count, 0);
                assert_eq!(store.credits()?.reserved, [0, 0]);
            }
            "after_commit" => {
                assert_eq!(count, 2);
                assert_eq!(store.last_commit()?, id(777));
                assert_eq!(store.credits()?.reserved, [FRAME_CREDIT, 0]);
            }
            _ => {
                let n = store.root.roots[Namespace::Event as usize].count;
                for i in 0..n {
                    assert_eq!(store.get(event(0, i)?)?, Some(vec![9; 1024]));
                }
            }
        }
        println!(
            "A3_S1_KILL point={point} owned_pid={pid} recovered_records={count} elapsed_ms={}",
            start.elapsed().as_millis()
        );
    }
    Ok(())
}

#[test]
fn indexed_component_file_capacity_refusal_and_continuation() -> TestResult {
    let config = Config {
        physical_quota: HEADROOM + 2 * 1024 * 1024,
        ..Config::default()
    };
    let (dir, mut store) = fixture(config)?;
    let mut n = 0u64;
    loop {
        let rows = (n..n + 32)
            .map(|i| event(0, i).map(|k| Mutation::put(k, vec![7; 1024])))
            .collect::<Result<Vec<_>>>()?;
        match apply(&mut store, rows, vec![]) {
            Ok(_) => {
                n += 32;
                if n > 4096 {
                    return Err("capacity boundary missing".into());
                }
            }
            Err(_) => break,
        }
    }
    let stats = store.backend_stats()?;
    assert_eq!(stats.last_failure.as_deref(), Some("ConfiguredCapacity"));
    let requested = stats.last_requested_length;
    drop(store);
    let external = account_external(dir.path(), Config::default())?.retained_external_bytes;
    let exact = Config {
        physical_quota: requested + external + HEADROOM - 1,
        ..Config::default()
    };
    let mut store =
        IndexedDialogVault::open(StateDirectoryLock::acquire(dir.path())?, owner(), exact)?;
    n = store.root.roots[Namespace::Event as usize].count;
    let (failed_id, failed_generation, failed_rows) = loop {
        let rows = (n..n + 32)
            .map(|i| event(0, i).map(|k| Mutation::put(k, vec![7; 1024])))
            .collect::<Result<Vec<_>>>()?;
        let attempt = id(900000 + n);
        let g = store.generation();
        match store.commit(g, attempt, rows.clone(), vec![]) {
            Ok(_) => {
                n += 32;
                if n > 4096 {
                    return Err("exact capacity boundary missing".into());
                }
            }
            Err(_) => break (attempt, g, rows),
        }
    };
    assert_eq!(
        store.backend_stats()?.last_failure.as_deref(),
        Some("ConfiguredCapacity")
    );
    assert_eq!(store.backend_stats()?.last_requested_length, requested);
    drop(store);
    let config = Config {
        physical_quota: 1024 * 1024 * 1024,
        ..Config::default()
    };
    let mut store =
        IndexedDialogVault::open(StateDirectoryLock::acquire(dir.path())?, owner(), config)?;
    assert_eq!(store.generation(), failed_generation);
    store.commit(failed_generation, failed_id, failed_rows, vec![])?;
    for i in 0..n + 32 {
        assert_eq!(store.get(event(0, i)?)?, Some(vec![7; 1024]));
    }
    drop(store);
    let mut store =
        IndexedDialogVault::open(StateDirectoryLock::acquire(dir.path())?, owner(), config)?;
    assert_eq!(store.last_commit()?, failed_id);
    assert!(store.get(event(0, n)?)?.is_some());
    println!(
        "A3_S1_CAPACITY requested={requested} external={external} refused_quota={} continued_records={}",
        exact.physical_quota,
        n + 32
    );
    Ok(())
}

#[test]
fn indexed_component_production_growth_single_append() -> TestResult {
    let config = Config {
        physical_quota: 1024 * 1024 * 1024,
        ..Config::default()
    };
    let (dir, mut store) = fixture(config)?;
    let start = Instant::now();
    let mut n = 0u64;
    while store.backend_stats()?.length < 128 * 1024 * 1024 {
        let rows = (n..n + 256)
            .map(|i| event(0, i).map(|k| Mutation::put(k, vec![8; 16 * 1024])))
            .collect::<Result<Vec<_>>>()?;
        apply(&mut store, rows, vec![])?;
        n += 256;
        if n > 20000 {
            return Err("growth setup bound".into());
        }
    }
    let (before, after, report, elapsed) = loop {
        let before = store.backend_stats()?;
        let t = Instant::now();
        let report = apply(
            &mut store,
            vec![Mutation::put(event(0, n)?, vec![8; 16 * 1024])],
            vec![],
        )?;
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let after = store.backend_stats()?;
        n += 1;
        if after.growth_bytes - before.growth_bytes >= 128 * 1024 * 1024 {
            break (before, after, report, ms);
        }
        if n > 20000 || start.elapsed() > Duration::from_secs(600) {
            return Err("production growth deadline".into());
        }
    };
    assert!(report.nodes_read <= 3 * 260);
    assert!(report.encoded_plan_bytes <= MAX_PLAN as u64);
    drop(store);
    let mut store =
        IndexedDialogVault::open(StateDirectoryLock::acquire(dir.path())?, owner(), config)?;
    assert_eq!(store.get(event(0, n - 1)?)?, Some(vec![8; 16 * 1024]));
    apply(
        &mut store,
        vec![Mutation::put(event(0, n)?, vec![8; 16 * 1024])],
        vec![],
    )?;
    println!(
        "A3_S1_GROW {}",
        serde_json::to_string(&serde_json::json!({
        "synthetic_records":n,"production_region_bytes":4u64*1024*1024*1024,"batch_at_boundary":1,
        "old_len":before.length,"new_len":after.length,"grow_delta":after.growth_bytes-before.growth_bytes,
        "write_delta":after.write_bytes-before.write_bytes,"sync_delta":after.sync_calls-before.sync_calls,
        "append_ms":elapsed,"plan_bytes":report.encoded_plan_bytes,"nodes_read":report.nodes_read,
        "total_seconds":start.elapsed().as_secs_f64(),"reopen_continuation":true}))?
    );
    Ok(())
}
