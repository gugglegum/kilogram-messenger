use super::*;
use std::time::Instant;
#[path = "scale_gates.rs"]
mod gates;

fn payload(n: u64) -> Vec<u8> {
    let mut value = vec![0x51; 1024];
    value[..8].copy_from_slice(&n.to_be_bytes());
    value
}
fn percentile(values: &mut [f64], p: usize) -> f64 {
    values.sort_by(f64::total_cmp);
    values[(values.len() - 1) * p / 100]
}

#[test]
fn indexed_scale_persistent_100k() -> TestResult {
    // A private, reusable fixture; never a runtime-enrolled account or 100k E2E claim.
    let root = std::env::var_os("KILOGRAM_S1_SCALE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("kilogram-a3-s1-persistent-scale"));
    measure_scale(root, None)
}

#[test]
fn indexed_scale_small_verification() -> TestResult {
    let base = std::env::temp_dir().join("kilogram-a3-s1-small-scale");
    for count in [1000u64, 10000] {
        measure_scale(base.join(count.to_string()), Some(count))?;
    }
    Ok(())
}

fn measure_scale(root: std::path::PathBuf, small: Option<u64>) -> TestResult {
    let overall = Instant::now();
    let normative = std::env::var("KILOGRAM_S1_NORMATIVE_SCALE").ok().as_deref() == Some("1");
    let require_new = std::env::var("KILOGRAM_S1_REQUIRE_NEW_SCALE")
        .ok()
        .as_deref()
        == Some("1");
    if normative && (!cfg!(windows) || !cfg!(debug_assertions) || !require_new || small.is_some()) {
        return Err(
            "normative run requires Windows verification, fresh full fixture and instrumentation"
                .into(),
        );
    }
    if std::env::var("KILOGRAM_S1_REQUIRE_NEW_SCALE")
        .ok()
        .as_deref()
        == Some("1")
        && root.join(STATE_VAULT_FILE).exists()
    {
        return Err("initial-import run requires a new fixture; existing store preserved".into());
    }
    let config = Config {
        physical_quota: 1024 * 1024 * 1024,
        ..Config::default()
    };
    let mut store = if root.join(STATE_VAULT_FILE).exists() {
        IndexedDialogVault::open(StateDirectoryLock::acquire(&root)?, owner(), config)?
    } else {
        IndexedDialogVault::create(StateDirectoryLock::acquire(&root)?, claim(), config)?
    };
    let mut stages = Vec::new();
    let mut total_commits = 0u64;
    let mut max_plan = 0;
    let existing = store.root.roots[Namespace::Event as usize].count;
    let targets = if let Some(count) = small {
        vec![existing.max(count)]
    } else if existing >= 100000 {
        vec![existing]
    } else {
        vec![1000u64, 10000, 100000]
    };
    for target in targets {
        let mut n = store.root.roots[Namespace::Event as usize].count;
        if n > target {
            continue;
        } // An already-completed fixture is not regenerated.
        let fill_start = Instant::now();
        let memory_before_fill = super::memory::sample(Some(&store), "before_fill", n, 0, 0)?;
        if normative && !gates::measurement_available(&memory_before_fill) {
            println!(
                "A3_S1_SCALE_GATES {{\"status\":\"UNAVAILABLE\",\"reason\":\"required process/cache instrumentation absent\"}}"
            );
            return Err("normative memory metrics unavailable; no fill attempted".into());
        }
        let before_fill = store.backend_stats()?;
        while n < target {
            let end = (n + 512).min(target);
            let rows = (n..end)
                .map(|i| event(0, i).map(|k| Mutation::put(k, payload(i))))
                .collect::<Result<Vec<_>>>()?;
            let r = apply(&mut store, rows, vec![])?;
            max_plan = max_plan.max(r.encoded_plan_bytes);
            total_commits += 1;
            n = end;
            if n % 10000 < 512 {
                println!(
                    "A3_S1_SCALE_PROGRESS records={n} elapsed_seconds={:.2}",
                    overall.elapsed().as_secs_f64()
                );
            }
            if overall.elapsed().as_secs() > 1200 {
                return Err("persistent100k budget exceeded".into());
            }
        }
        let fill_seconds = fill_start.elapsed().as_secs_f64();
        let after_fill = store.backend_stats()?;
        let memory_after_fill = super::memory::sample(Some(&store), "after_fill", target, 0, 0)?;
        let counts = store.record_counts()?;
        assert_eq!(counts, (target, 2 * target - 1, target));
        drop(store);
        let t = Instant::now();
        store = IndexedDialogVault::open(StateDirectoryLock::acquire(&root)?, owner(), config)?;
        let reopen_ms = t.elapsed().as_secs_f64() * 1000.0;
        let after_open = store.backend_stats()?;
        let memory_after_open = super::memory::sample(Some(&store), "after_reopen", target, 0, 0)?;
        let mut next = [0u8; 32];
        let mut seen = 0usize;
        let mut previous = None;
        let mut memory_first_page = None;
        let mut pages = Vec::new();
        let read_start = Instant::now();
        loop {
            let t = Instant::now();
            let page = store.range(Key::hash(0, Namespace::Event, next)?, 32)?;
            pages.push(t.elapsed().as_secs_f64() * 1000.0);
            if page.is_empty() {
                break;
            }
            if seen == 0 {
                memory_first_page = Some(super::memory::sample(
                    Some(&store),
                    "first_page_live",
                    target,
                    0,
                    page.iter().map(|(_, v)| v.capacity()).sum(),
                )?);
            }
            for (k, v) in &page {
                if let Some(previous) = previous {
                    assert!(previous < k.bytes);
                }
                let n = u64::from_be_bytes(v[..8].try_into()?);
                assert!(n < target);
                assert_eq!(v, &payload(n));
                assert_eq!(k.bytes, id(n));
                previous = Some(k.bytes);
                seen += 1;
            }
            next = page.last().ok_or("page end")?.0.bytes;
            let mut carry = true;
            for byte in next.iter_mut().rev() {
                if carry {
                    let (value, overflow) = byte.overflowing_add(1);
                    *byte = value;
                    carry = overflow;
                }
            }
            if carry {
                break;
            }
            if overall.elapsed().as_secs() > 1200 {
                return Err("persistent scale read budget exceeded; fixture retained".into());
            }
        }
        assert_eq!(seen, target as usize);
        let read_seconds = read_start.elapsed().as_secs_f64();
        let after_reads = store.backend_stats()?;
        let memory_after_reads = super::memory::sample(
            Some(&store),
            "after_warm_range_no_expected_vector",
            target,
            0,
            0,
        )?;
        let mut append_ms = Vec::new();
        let mut max_nodes = 0;
        let mut append_plan = 0;
        for n in target..target + 16 {
            let t = Instant::now();
            let r = apply(
                &mut store,
                vec![Mutation::put(event(0, n)?, payload(n))],
                vec![],
            )?;
            append_ms.push(t.elapsed().as_secs_f64() * 1000.0);
            max_nodes = max_nodes.max(r.nodes_read);
            append_plan = append_plan.max(r.encoded_plan_bytes);
            assert!(r.nodes_read <= 3 * 260);
            assert!(r.changed_nodes <= 260);
            total_commits += 1;
        }
        let after_append = store.backend_stats()?;
        let memory_after_append =
            super::memory::sample(Some(&store), "after_ordinary_append", target + 16, 0, 0)?;
        let memory = memory_after_append["process"]["private_bytes"].as_u64();
        if let Some(memory) = memory {
            assert!(memory <= 384 * 1024 * 1024);
        }
        let stage = serde_json::json!({"target_records":target,"physical_nodes":counts.1,"physical_values":counts.2,
            "synthetic_storage_records":true,"native_e2e_messages":0,"history_cursor_implemented":false,
            "fill_seconds":fill_seconds,"fill_read_bytes":after_fill.read_bytes-before_fill.read_bytes,
            "fill_write_bytes":after_fill.write_bytes-before_fill.write_bytes,"fill_grow_bytes":after_fill.growth_bytes-before_fill.growth_bytes,
            "fill_syncs":after_fill.sync_calls-before_fill.sync_calls,"max_bulk_plan_bytes":max_plan,
            "cold_reopen_ms":reopen_ms,"open_backend":after_open,"page32_p50_ms":percentile(&mut pages,50),"page32_p95_ms":percentile(&mut pages,95),
            "full_range_read_seconds":read_seconds,"page_backend_read_bytes":after_reads.read_bytes-after_open.read_bytes,
            "append_p50_ms":percentile(&mut append_ms,50),"append_p95_ms":percentile(&mut append_ms,95),
            "single_append_max_node_reads":max_nodes,"single_append_max_plan_bytes":append_plan,
            "append_read_bytes":after_append.read_bytes-after_reads.read_bytes,
            "append_write_bytes":after_append.write_bytes-after_reads.write_bytes,
            "append_growth_bytes":after_append.growth_bytes-after_reads.growth_bytes,
            "append_syncs":after_append.sync_calls-after_reads.sync_calls,
            "memory_phases":[memory_before_fill,memory_after_fill,memory_after_open,memory_first_page,memory_after_reads,memory_after_append],
            "page_record_limit":32,"event_payload_bytes":1024,"ordinary_append_count":16,
            "page_latency_sample_capacity_bytes":pages.capacity()*std::mem::size_of::<f64>(),
            "test_expected_vector_bytes":0,"strict_unique_complete_domain_check":true,
            "process_private_bytes":memory,"retained_records_after_samples":target+16});
        println!("A3_S1_SCALE {}", serde_json::to_string(&stage)?);
        stages.push(stage);
    }
    assert!(store.root.roots[Namespace::Event as usize].count >= small.unwrap_or(100000));
    let elapsed = overall.elapsed().as_secs_f64();
    let gate_result = gates::evaluate(&stages, elapsed);
    println!("A3_S1_SCALE_GATES {}", serde_json::to_string(&gate_result)?);
    println!(
        "A3_S1_SCALE_FINAL {}",
        serde_json::to_string(
            &serde_json::json!({"stages":stages,"commits_this_run":total_commits,
        "elapsed_seconds":elapsed,"fixture_preserved":true,"final_counts":store.record_counts()?,"normative_run":normative,"gate_result":gate_result})
        )?
    );
    if overall.elapsed().as_secs() > 1200 {
        return Err("persistent scale total budget exceeded; fixture retained".into());
    }
    if normative && gate_result["status"] != "PASS" {
        return Err("normative scale/memory gates did not pass; fixture retained".into());
    }
    Ok(())
}

/// Separate cold-read diagnostic: redb's debug allocator verification warms small
/// databases during open. This runs with production debug_assertions=false and
/// the same cache64MiB, not a test geometry/cache override.
#[cfg(not(debug_assertions))]
#[test]
fn indexed_release_cold_backend_pages() -> TestResult {
    for count in [1000u64, 10000] {
        let (dir, mut store) = fixture(Config::default())?;
        for start in (0..count).step_by(512) {
            let end = (start + 512).min(count);
            let rows = (start..end)
                .map(|n| event(0, n).map(|k| Mutation::put(k, payload(n))))
                .collect::<Result<Vec<_>>>()?;
            apply(&mut store, rows, vec![])?;
        }
        drop(store);
        let t = Instant::now();
        let mut store = IndexedDialogVault::open(
            StateDirectoryLock::acquire(dir.path())?,
            owner(),
            Config::default(),
        )?;
        let reopen_ms = t.elapsed().as_secs_f64() * 1000.0;
        let before = store.backend_stats()?;
        let t = Instant::now();
        let page = store.range(Key::hash(0, Namespace::Event, [0; 32])?, 32)?;
        let page_ms = t.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(page.len(), 32);
        for (key, value) in page {
            let n = u64::from_be_bytes(value[..8].try_into()?);
            assert_eq!(key.bytes, id(n));
            assert_eq!(value, payload(n));
        }
        let after = store.backend_stats()?;
        assert!(after.read_bytes > before.read_bytes);
        println!(
            "A3_S1_RELEASE_COLD {}",
            serde_json::to_string(&serde_json::json!({
            "synthetic_records":count,"cache_bytes":64*1024*1024,"debug_assertions":false,
            "cold_reopen_ms":reopen_ms,"open_read_bytes":before.read_bytes,"page32_ms":page_ms,
            "page_backend_read_bytes":after.read_bytes-before.read_bytes,"page_backend_reads":after.reads-before.reads,
            "native_e2e_messages":0,"not_full_history_gate":true}))?
        );
    }
    Ok(())
}
