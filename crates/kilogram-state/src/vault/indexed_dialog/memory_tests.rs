//! Targeted diagnostic over preserved fixtures, not another initial100k import.
use super::*;
use std::time::Instant;

#[cfg(windows)]
fn memory() -> TestResultValue<Option<serde_json::Value>> {
    use std::os::windows::process::CommandExt;
    let command = format!(
        "$p=Get-Process -Id {}; [ordered]@{{private_bytes=$p.PrivateMemorySize64;peak_paged_bytes=$p.PeakPagedMemorySize64;working_set_bytes=$p.WorkingSet64}} | ConvertTo-Json -Compress",
        std::process::id()
    );
    let out = std::process::Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &command,
        ])
        .creation_flags(0x0800_4000)
        .output()?;
    if !out.status.success() {
        return Err("memory query failed".into());
    }
    Ok(Some(serde_json::from_slice(&out.stdout)?))
}
#[cfg(not(windows))]
fn memory() -> TestResultValue<Option<serde_json::Value>> {
    Ok(None)
}
type TestResultValue<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub(super) fn sample(
    store: Option<&IndexedDialogVault>,
    phase: &str,
    count: u64,
    test_expected_capacity_bytes: usize,
    app_returned_payload_bytes: usize,
) -> TestResultValue<serde_json::Value> {
    let cache = store.and_then(|s| {
        let stats = s.vault.database.cache_stats();
        // Without cache_metrics redb returns zero for every field. An actual
        // measured empty cache is distinguishable once a counter has advanced.
        (stats.used_bytes() > 0
            || stats.read_hits() > 0
            || stats.read_misses() > 0
            || stats.write_hits() > 0
            || stats.write_misses() > 0
            || stats.evictions() > 0)
            .then_some(stats.used_bytes())
    });
    let process = memory()?;
    let value = serde_json::json!({
        "phase":phase,"records":count,"process":process,"redb_cache_data_bytes":cache,
        "process_memory_available":process.is_some(),"cache_metrics_available":cache.is_some(),
        "process_unavailable_reason":if process.is_some(){None}else{Some("unsupported platform")},
        "cache_unavailable_reason":if cache.is_some(){None}else{Some("instrumentation unavailable or no counters observed")},
        "configured_cache_bytes":64*1024*1024,"test_expected_capacity_bytes":test_expected_capacity_bytes,
        "app_returned_payload_bytes":app_returned_payload_bytes,
        "debug_assertions":cfg!(debug_assertions),"same_process":std::process::id(),
        "backend":store.map(IndexedDialogVault::backend_stats).transpose()?,
    });
    println!("A3_S1_MEMORY {}", serde_json::to_string(&value)?);
    Ok(value)
}

fn body(n: u64) -> Vec<u8> {
    let mut value = vec![0x51; 1024];
    value[..8].copy_from_slice(&n.to_be_bytes());
    value
}

#[test]
fn indexed_memory_optional_measurement_preserves_storage_checks() -> TestResult {
    let (_dir, mut store) = fixture(Config::default())?;
    apply(
        &mut store,
        vec![Mutation::put(event(0, 0)?, body(0))],
        vec![],
    )?;
    let s = sample(Some(&store), "optional_measurement", 1, 0, 0)?;
    assert_eq!(s["process_memory_available"], cfg!(windows));
    if !cfg!(windows) {
        assert!(s["process"].is_null());
    }
    if s["cache_metrics_available"] == false {
        assert!(s["redb_cache_data_bytes"].is_null());
        assert!(s["cache_unavailable_reason"].is_string());
    }
    assert_eq!(store.get(event(0, 0)?)?, Some(body(0)));
    assert_eq!(
        store
            .range(Key::hash(0, Namespace::Event, [0; 32])?, 32)?
            .len(),
        1
    );
    Ok(())
}

#[test]
fn indexed_memory_diagnostic_preserved_fixtures() -> TestResult {
    if std::env::var("KILOGRAM_S1_MEMORY_DIAGNOSTIC")
        .ok()
        .as_deref()
        != Some("1")
    {
        return Ok(());
    }
    let base = std::env::var_os("KILOGRAM_S1_PREVIOUS_FIXTURE_BASE")
        .map(std::path::PathBuf::from)
        .ok_or("set KILOGRAM_S1_PREVIOUS_FIXTURE_BASE")?;
    let roots = [
        base.join("kilogram-a3-s1-small-scale/1000"),
        base.join("kilogram-a3-s1-small-scale/10000"),
        base.join("persistent-scale"),
    ];
    let config = Config {
        physical_quota: 1024 * 1024 * 1024,
        ..Config::default()
    };
    for root in roots {
        sample(None, "before_open", 0, 0, 0)?;
        let mut store =
            IndexedDialogVault::open(StateDirectoryLock::acquire(&root)?, owner(), config)?;
        let count = store.root.roots[Namespace::Event as usize].count;
        if store.vault.database.cache_stats().used_bytes() == 0 {
            return Err("run stable runner with -CargoFeatures redb/cache_metrics".into());
        }
        sample(Some(&store), "after_open", count, 0, 0)?;
        let start = Instant::now();
        let mut first = [0; 32];
        let mut last = None;
        let mut seen = 0u64;
        let mut peak_payload = 0;
        loop {
            let page = store.range(Key::hash(0, Namespace::Event, first)?, 32)?;
            if page.is_empty() {
                break;
            }
            peak_payload = peak_payload.max(page.iter().map(|(_, v)| v.capacity()).sum());
            for (key, value) in &page {
                if let Some(previous) = last {
                    assert!(previous < key.bytes);
                }
                let n = u64::from_be_bytes(value[..8].try_into()?);
                assert!(n < count);
                assert_eq!(key.bytes, id(n));
                assert_eq!(value, &body(n));
                last = Some(key.bytes);
                seen += 1;
            }
            first = page.last().ok_or("page")?.0.bytes;
            let mut carry = true;
            for byte in first.iter_mut().rev() {
                if carry {
                    let (v, c) = byte.overflowing_add(1);
                    *byte = v;
                    carry = c;
                }
            }
            if carry {
                break;
            }
        }
        assert_eq!(seen, count);
        println!(
            "A3_S1_MEMORY_RANGE records={count} elapsed_seconds={}",
            start.elapsed().as_secs_f64()
        );
        sample(
            Some(&store),
            "after_full_warm_no_expected_vector",
            count,
            0,
            peak_payload,
        )?;
        let mut expected: Vec<_> = (0..count).map(id).collect();
        expected.sort();
        let bytes = expected.capacity() * std::mem::size_of::<Id>();
        sample(Some(&store), "test_expected_vector_live", count, bytes, 0)?;
        std::hint::black_box(&expected);
        drop(expected);
        sample(Some(&store), "after_test_expected_vector_drop", count, 0, 0)?;
        for n in count..count + 16 {
            apply(
                &mut store,
                vec![Mutation::put(event(0, n)?, body(n))],
                vec![],
            )?;
        }
        sample(Some(&store), "after_ordinary_16_appends", count + 16, 0, 0)?;
        drop(store);
        sample(None, "after_store_drop", count + 16, 0, 0)?;
    }
    Ok(())
}
