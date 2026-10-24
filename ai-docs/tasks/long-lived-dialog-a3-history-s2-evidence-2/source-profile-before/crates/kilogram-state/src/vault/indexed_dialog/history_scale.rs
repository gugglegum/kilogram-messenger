//! Private synthetic fixture: real storage/credits, zero native E2E messages.
use super::tests as f;
use super::*;
use serde_json::{Value, json};
use std::{path::PathBuf, time::Instant};

const MIB: u64 = 1024 * 1024;
const APPEND_COUNT: u64 = 16;
pub(super) fn config() -> Config {
    Config {
        physical_quota: 2 * 1024 * 1024 * 1024,
        ..Config::default()
    }
}
pub(super) fn current_heads(store: &mut IndexedDialogVault) -> Result<Heads> {
    Heads::decode(&store.get(heads_key(0)?)?.ok_or(Error::HistoryUnavailable)?)
}
pub(super) fn count(heads: &Heads) -> u64 {
    heads.frontiers[0] + heads.frontiers[1] - 2
}
fn budget(start: Instant) -> Result<()> {
    if start.elapsed().as_secs_f64() > 1200.0 {
        return Err(Error::Bound(
            "history scale 1200s ceiling; fixture retained",
        ));
    }
    Ok(())
}
fn text(i: u64) -> String {
    let mut text = format!("{i:016x}: synthetic history, native E2E=0; ");
    text.extend(std::iter::repeat_n('s', 1024 - text.len()));
    text
}
pub(super) fn next_row(heads: &mut Heads, i: u64) -> Result<PreparedReadable> {
    let role = u8::from(!i.is_multiple_of(16));
    let mut parents = Vec::new();
    for r in 0..2 {
        if heads.frontiers[r] > 1 {
            parents.push(HistoryParent {
                event: heads.tails[r].ok_or(Error::Integrity("fixture tail"))?,
                role: r as u8,
                ordinal: heads.frontiers[r] - 1,
            });
        }
    }
    let mut request = [0; 16];
    request.copy_from_slice(&f::id(i)[..16]);
    let row = f::prepared(
        0,
        role,
        heads.frontiers[usize::from(role)],
        heads.tails[usize::from(role)],
        request,
        &text(i),
        parents,
    )?;
    heads.tails[usize::from(role)] = Some(row.event_id()?);
    heads.frontiers[usize::from(role)] += 1;
    Ok(row)
}
fn oracle_heads() -> Result<Heads> {
    Ok(Heads {
        view: [0; 32],
        frontiers: [1, 1],
        tails: [
            Some(f::prepared(0, 0, 0, None, [0; 16], "", vec![])?.event_id()?),
            Some(f::prepared(0, 1, 0, None, [0; 16], "", vec![])?.event_id()?),
        ],
    })
}
pub(super) fn validate_items(
    items: &[HistoryRow],
    expected: &mut Heads,
    index: &mut u64,
) -> Result<()> {
    for item in items {
        let row = next_row(expected, *index)?;
        if item.id != row.event_id()?
            || item.role != row.role
            || item.ordinal != row.ordinal
            || item.sequence != row.sequence
            || item.request != row.request
            || item.text != row.text
        {
            return Err(Error::Integrity("history scale exact streamed domain"));
        }
        *index += 1;
    }
    Ok(())
}
fn p95(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values[(values.len() - 1) * 95 / 100]
}

impl IndexedDialogVault {
    // cfg(test) module + private method: there is no runtime importer entry point.
    pub(super) fn history_fixture_commit(
        &mut self,
        rows: Vec<PreparedReadable>,
    ) -> Result<CommitReport> {
        if rows.is_empty() || rows.len() > 128 {
            return Err(Error::Bound("fixture input count"));
        }
        for row in &rows {
            row.check_bound()?;
        }
        let raw = Zeroizing::new(
            postcard::to_allocvec(&(self.generation(), &rows))
                .map_err(|_| Error::Bound("fixture input codec"))?,
        );
        if raw.len() > MAX_PLAN {
            return Err(Error::Bound("fixture input bytes"));
        }
        let digest = self.crypto.hash(0, b"history-synthetic-fixture/v1", &raw);
        self.commit_with(
            self.generation(),
            f::id(self.generation()),
            digest,
            CommitInput::HistoryFixture { context: 0, rows },
            raw.len() * 3 + 16384,
        )
    }
}

#[cfg(windows)]
fn process_memory() -> std::result::Result<Option<Value>, Box<dyn std::error::Error>> {
    use std::os::windows::process::CommandExt;
    let command = format!(
        "$p=Get-Process -Id {}; [ordered]@{{private_bytes=$p.PrivateMemorySize64;peak_paged_bytes=$p.PeakPagedMemorySize64;working_set_bytes=$p.WorkingSet64}} | ConvertTo-Json -Compress",
        std::process::id()
    );
    let output = std::process::Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &command,
        ])
        .creation_flags(0x0800_4000)
        .output()?;
    if !output.status.success() {
        return Err("process memory query failed".into());
    }
    Ok(Some(serde_json::from_slice(&output.stdout)?))
}
#[cfg(not(windows))]
fn process_memory() -> std::result::Result<Option<Value>, Box<dyn std::error::Error>> {
    Ok(None)
}
fn memory(
    store: &IndexedDialogVault,
    phase: &str,
    records: u64,
    app_bound: u64,
) -> std::result::Result<Value, Box<dyn std::error::Error>> {
    let cache = store.vault.database.cache_stats();
    let available = cache.used_bytes() > 0
        || cache.read_hits() > 0
        || cache.read_misses() > 0
        || cache.write_hits() > 0
        || cache.write_misses() > 0;
    let sample = json!({"phase":phase,"records":records,"same_process":std::process::id(),"process":process_memory()?,
        "redb_cache_data_bytes":available.then_some(cache.used_bytes()),"cache_metrics_available":available,
        "configured_cache_bytes":64*MIB,"app_buffer_bound_bytes":app_bound,
        "app_buffer_bound_kind":"conservative live overlay sizes/capacities plus bounded input/plan/encoding; not allocator attribution"});
    println!("A3_S2_MEMORY {}", serde_json::to_string(&sample)?);
    Ok(sample)
}
fn limits(stages: &[Value], full: bool, seconds: f64) -> Value {
    let mut failures = Vec::<String>::new();
    let mut unavailable = Vec::<String>::new();
    for stage in stages {
        let n = stage["target_records"].as_u64().unwrap_or(0);
        if stage["max_page_point_gets"]
            .as_u64()
            .is_none_or(|v| v > 274)
            || stage["max_candidates"].as_u64().is_none_or(|v| v > 2)
            || stage["max_bulk_plan_bytes"]
                .as_u64()
                .is_none_or(|v| v > MAX_PLAN as u64)
            || stage["ordinary_max_plan_bytes"]
                .as_u64()
                .is_none_or(|v| v > MAX_PLAN as u64)
            || stage["max_app_buffer_bound_bytes"]
                .as_u64()
                .is_none_or(|v| v > 192 * MIB)
        {
            failures.push(format!("stage{n}:operation bounds"));
        }
        if let Some(samples) = stage["memory_phases"].as_array() {
            for sample in samples {
                let private = sample["process"]["private_bytes"].as_u64();
                let cache = sample["redb_cache_data_bytes"].as_u64();
                if private.is_none() || cache.is_none() {
                    unavailable.push(format!("stage{n}:memory/cache unavailable"));
                }
                if private.is_some_and(|v| v > 384 * MIB)
                    || cache.is_some_and(|v| v > 64 * MIB)
                    || sample["process"]["peak_paged_bytes"]
                        .as_u64()
                        .is_some_and(|v| v > 384 * MIB)
                {
                    failures.push(format!("stage{n}:absolute/cache memory"));
                }
            }
        } else {
            unavailable.push(format!("stage{n}:missing phases"));
        }
    }
    let mut comparisons = Vec::new();
    if full && stages.len() == 3 {
        let small = &stages[0];
        let large = &stages[2];
        for phase in ["after_warm_range", "after_ordinary_append"] {
            let find = |stage: &Value| -> Option<(i128, i128)> {
                let p = stage["memory_phases"]
                    .as_array()?
                    .iter()
                    .find(|v| v["phase"] == phase)?;
                Some((
                    i128::from(p["process"]["private_bytes"].as_u64()?),
                    i128::from(p["redb_cache_data_bytes"].as_u64()?),
                ))
            };
            if let (Some((sm, sc)), Some((lm, lc))) = (find(small), find(large)) {
                let growth = lm - sm;
                let residual = (lm - lc) - (sm - sc);
                if growth > 96 * i128::from(MIB) || residual > 32 * i128::from(MIB) {
                    failures.push(format!("{phase}:warm growth/residual"));
                }
                comparisons.push(
                    json!({"phase":phase,"raw_growth_bytes":growth,"residual_growth_bytes":residual,
                    "raw_limit_bytes":96*MIB,"residual_limit_bytes":32*MIB}),
                );
            } else {
                unavailable.push(format!("{phase}:comparison unavailable"));
            }
        }
        for metric in ["page32_p95_ms", "append_p95_ms"] {
            if let (Some(s), Some(l)) = (small[metric].as_f64(), large[metric].as_f64()) {
                if s <= 0.0 || l / s > 4.0 {
                    failures.push(format!("{metric}:ratio"));
                }
                comparisons.push(json!({"metric":metric,"ratio":l/s,"limit":4}));
            } else {
                unavailable.push(format!("{metric}:ratio unavailable"));
            }
        }
    } else if full {
        unavailable.push("incomplete1k/10k/100k stage set".into());
    }
    if seconds > 1200.0 {
        failures.push("1200s ceiling".into());
    }
    json!({"full_gate":full,"status":if !failures.is_empty(){"FAIL"}else if !unavailable.is_empty(){"UNAVAILABLE"}else if full{"PASS"}else{"SMALL_TIMINGS_ONLY"},
        "failures":failures,"unavailable":unavailable,"comparisons":comparisons,"seconds":seconds})
}

#[derive(Clone)]
struct Checkpoint {
    cursor: HistoryCursor,
    expected: Heads,
    index: u64,
}

fn measure(root: PathBuf, full: bool) -> f::TestResult {
    let overall = Instant::now();
    let exists = root.join(STATE_VAULT_FILE).exists();
    if exists {
        return Err("fresh scale fixture already exists; preserved, no rebuild/adoption".into());
    }
    let mut store =
        IndexedDialogVault::create(StateDirectoryLock::acquire(&root)?, f::claim(), config())?;
    f::setup(&mut store, 0)?;
    f::setup(&mut store, 1)?;
    let c2_roots = store.root.roots[24..48].to_vec();
    let c2_ledger = store.root.ledger[1].clone();
    let mut stages = Vec::new();
    let mut commits = 0;
    let mut max_bulk_plan = 0u64;
    let mut max_app = 0u64;
    let targets = if full {
        vec![1000, 10000, 100000]
    } else {
        vec![1000, 10000]
    };
    for target in targets {
        budget(overall)?;
        let mut heads = current_heads(&mut store)?;
        let existing = count(&heads);
        let before_fill = store.backend_stats()?;
        let memory_before = memory(&store, "before_fill", existing, max_app)?;
        let fill_start = Instant::now();
        while count(&heads) < target {
            budget(overall)?;
            let previous = count(&heads);
            let batch = (target - previous).min(128);
            let mut rows = Vec::with_capacity(batch as usize);
            for i in previous..previous + batch {
                let row = next_row(&mut heads, i)?;
                if row.role == 0 {
                    let r = f::apply(
                        &mut store,
                        0,
                        HistoryOperation::QueueAccept {
                            request: row.request,
                            text: row.text.clone(),
                        },
                    )?;
                    max_app = max_app.max(r.app_buffer_bound_bytes);
                    commits += 1;
                }
                rows.push(row);
            }
            let report = store.history_fixture_commit(rows)?;
            commits += 1;
            max_bulk_plan = max_bulk_plan.max(report.encoded_plan_bytes);
            max_app = max_app.max(report.app_buffer_bound_bytes);
            if count(&heads) / 1000 != previous / 1000 {
                println!(
                    "A3_S2_FILL {}",
                    serde_json::to_string(
                        &json!({"rows":count(&heads),"target":target,"seconds":overall.elapsed().as_secs_f64(),"max_plan":max_bulk_plan,"max_app_buffer_bound":max_app})
                    )?
                );
            }
        }
        let fill_seconds = fill_start.elapsed().as_secs_f64();
        let after_fill = store.backend_stats()?;
        let memory_fill = memory(&store, "after_fill", target, max_app)?;
        let counts = store.record_counts()?;
        drop(store);
        let reopened = Instant::now();
        store =
            IndexedDialogVault::open(StateDirectoryLock::acquire(&root)?, f::owner(), config())?;
        let reopen_ms = reopened.elapsed().as_secs_f64() * 1000.0;
        let after_open = store.backend_stats()?;
        let memory_open = memory(&store, "after_reopen", target, 0)?;
        let mut point_ms = Vec::new();
        let point_io_start = store.backend_stats()?;
        for role in 0..2 {
            for n in [
                1,
                (heads.frontiers[role] - 1).max(1) / 2 + 1,
                heads.frontiers[role] - 1,
            ] {
                if n >= heads.frontiers[role] {
                    continue;
                }
                let event = id_value(&store.get(lane(0, role as u8, n)?)?.ok_or("point Lane")?)?;
                let start = Instant::now();
                let point = store.history_point(0, event)?.ok_or("point row")?;
                point_ms.push(start.elapsed().as_secs_f64() * 1000.0);
                assert_eq!(point.id, event);
            }
        }
        let point_io_end = store.backend_stats()?;
        let range_start = Instant::now();
        let first_start = Instant::now();
        let mut page = store.history_first(0, 32)?;
        let mut latencies = vec![first_start.elapsed().as_secs_f64() * 1000.0];
        let memory_first = memory(
            &store,
            "first_page_live",
            target,
            page.stats.app_buffer_bound_bytes,
        )?;
        let mut expected = oracle_heads()?;
        let mut seen = 0u64;
        let mut checkpoints = Vec::new();
        let mut max_gets = 0;
        let mut max_candidates = 0;
        loop {
            budget(overall)?;
            max_gets = max_gets.max(page.stats.point_gets);
            max_candidates = max_candidates.max(page.stats.max_candidates);
            max_app = max_app.max(page.stats.app_buffer_bound_bytes);
            validate_items(&page.items, &mut expected, &mut seen)?;
            if checkpoints.is_empty()
                || checkpoints.len() == 1 && seen >= target / 2
                || checkpoints.len() == 2 && seen + 32 >= target
            {
                checkpoints.push(Checkpoint {
                    cursor: page.cursor.clone(),
                    expected: expected.clone(),
                    index: seen,
                });
            }
            if seen.is_multiple_of(8192) {
                println!(
                    "A3_S2_RANGE {}",
                    serde_json::to_string(
                        &json!({"seen":seen,"target":target,"seconds":overall.elapsed().as_secs_f64()})
                    )?
                );
            }
            if page.finished {
                break;
            }
            let start = Instant::now();
            page = store.history_page(0, &page.cursor, 32)?;
            if page.items.len() == 32 {
                latencies.push(start.elapsed().as_secs_f64() * 1000.0);
            }
        }
        assert_eq!(seen, target);
        let finished_cursor = page.cursor.clone();
        drop(page);
        let range_seconds = range_start.elapsed().as_secs_f64();
        let after_reads = store.backend_stats()?;
        let memory_warm = memory(&store, "after_warm_range", target, max_app)?;
        let mut appends = Vec::new();
        let mut max_append_nodes = 0;
        let mut max_append_plan = 0;
        let mut last_event = None;
        for i in target..target + APPEND_COUNT {
            budget(overall)?;
            let row = next_row(&mut heads, i)?;
            last_event = Some(row.event_id()?);
            let start = Instant::now();
            let accepted = if row.role == 0 {
                f::apply(
                    &mut store,
                    0,
                    HistoryOperation::QueueAccept {
                        request: row.request,
                        text: row.text.clone(),
                    },
                )?
            } else {
                f::accept_peer(&mut store, 0, &row)?
            };
            let native = f::native(&mut store, 0, std::slice::from_ref(&row))?;
            let report = f::apply(
                &mut store,
                0,
                HistoryOperation::TextBatch {
                    rows: vec![row],
                    native,
                },
            )?;
            appends.push(start.elapsed().as_secs_f64() * 1000.0);
            commits += 2;
            max_app = max_app
                .max(report.app_buffer_bound_bytes)
                .max(accepted.app_buffer_bound_bytes);
            max_append_nodes = max_append_nodes.max(report.nodes_read + accepted.nodes_read);
            max_append_plan = max_append_plan
                .max(report.encoded_plan_bytes)
                .max(accepted.encoded_plan_bytes);
        }
        let after_append = store.backend_stats()?;
        let memory_append = memory(
            &store,
            "after_ordinary_append",
            target + APPEND_COUNT,
            max_app,
        )?;
        assert_eq!(store.history_first(0, 1)?.frontiers, heads.frontiers);
        assert!(
            store
                .history_point(0, last_event.ok_or("last append")?)?
                .is_some()
        );
        assert!(
            store
                .history_page(0, &finished_cursor, 32)?
                .items
                .is_empty()
        );
        drop(store);
        let resume_open = Instant::now();
        store =
            IndexedDialogVault::open(StateDirectoryLock::acquire(&root)?, f::owner(), config())?;
        let resume_reopen_ms = resume_open.elapsed().as_secs_f64() * 1000.0;
        let resume_start = Instant::now();
        let mut resume_pages = 0;
        for checkpoint in &checkpoints {
            let page = store.history_page(0, &checkpoint.cursor, 32)?;
            let mut expected = checkpoint.expected.clone();
            let mut i = checkpoint.index;
            validate_items(&page.items, &mut expected, &mut i)?;
            assert!(i <= target);
            assert_eq!(
                page.frontiers,
                [heads.frontiers[0] - 1, heads.frontiers[1] - 15]
            );
        }
        let checkpoint = checkpoints.get(1).ok_or("midpoint cursor")?;
        let mut expected = checkpoint.expected.clone();
        let mut index = checkpoint.index;
        let mut cursor = checkpoint.cursor.clone();
        loop {
            budget(overall)?;
            let page = store.history_page(0, &cursor, 32)?;
            resume_pages += 1;
            validate_items(&page.items, &mut expected, &mut index)?;
            cursor = page.cursor;
            if page.finished {
                break;
            }
        }
        assert_eq!(index, target);
        let resume_ms = resume_start.elapsed().as_secs_f64() * 1000.0;
        let memory_resume = memory(
            &store,
            "after_resume_reopen",
            target + APPEND_COUNT,
            max_app,
        )?;
        assert_eq!(store.root.roots[24..48], c2_roots);
        assert!(store.root.ledger[1] == c2_ledger);
        let sample_bytes = latencies.capacity() * std::mem::size_of::<f64>();
        let mut stage = json!({"target_records":target,"source_records_c0":target+2,"records_after_append":target+APPEND_COUNT,
            "all_context_record_counts_before_append":counts,"native_e2e_messages":0,"synthetic_trusted_dto":true,
            "fixture_text_lanes":2,"own_row_every":16,"source_payload_bytes":1024,"text_payload_bytes":1024,
            "fill_seconds":fill_seconds,"fill_commits":commits,"fill_read_bytes":after_fill.read_bytes-before_fill.read_bytes,
            "fill_write_bytes":after_fill.write_bytes-before_fill.write_bytes,"fill_growth_bytes":after_fill.growth_bytes-before_fill.growth_bytes,
            "fill_syncs":after_fill.sync_calls-before_fill.sync_calls,"max_bulk_plan_bytes":max_bulk_plan,
            "cold_reopen_ms":reopen_ms,"open_backend":after_open,"point_p95_ms":p95(&mut point_ms)});
        let details = json!({
            "point_backend_read_bytes":point_io_end.read_bytes-point_io_start.read_bytes,
            "page32_p95_ms":p95(&mut latencies),"full_range_seconds":range_seconds,"page_backend_read_bytes":after_reads.read_bytes-point_io_end.read_bytes,
            "page_record_limit":32,"max_page_point_gets":max_gets,"max_candidates":max_candidates,
            "append_p95_ms":p95(&mut appends),"ordinary_append_count":APPEND_COUNT,
            "ordinary_max_node_reads":max_append_nodes,"ordinary_max_plan_bytes":max_append_plan,
            "append_read_bytes":after_append.read_bytes-after_reads.read_bytes,"append_write_bytes":after_append.write_bytes-after_reads.write_bytes,
            "append_growth_bytes":after_append.growth_bytes-after_reads.growth_bytes,"append_syncs":after_append.sync_calls-after_reads.sync_calls,
            "resume_reopen_ms":resume_reopen_ms,"resume_pages":resume_pages,"resume_ms":resume_ms,"resume_checkpoints":checkpoints.len(),
            "exact_streamed_domain":true,"no_n_sized_expected_vector":true,"latency_samples_bytes":sample_bytes,
            "max_app_buffer_bound_bytes":max_app,"c2_unchanged":true,
            "memory_phases":[memory_before,memory_fill,memory_open,memory_first,memory_warm,memory_append,memory_resume]});
        if let Value::Object(fields) = details {
            stage
                .as_object_mut()
                .ok_or("scale stage object")?
                .extend(fields);
        }
        println!("A3_S2_SCALE {}", serde_json::to_string(&stage)?);
        stages.push(stage);
    }
    let gates = limits(&stages, full, overall.elapsed().as_secs_f64());
    println!(
        "A3_S2_SCALE_FINAL {}",
        serde_json::to_string(
            &json!({"stages":stages,"gates":gates,"fixture":root,"commits":commits,"native_e2e_messages":0})
        )?
    );
    if !matches!(
        gates["status"].as_str(),
        Some("PASS" | "SMALL_TIMINGS_ONLY")
    ) {
        return Err("history scale gate failed/unavailable; evidence and fixture retained".into());
    }
    Ok(())
}

#[test]
fn history_scale_small_timings() -> f::TestResult {
    if std::env::var("KILOGRAM_S2_SCALE_MODE").ok().as_deref() != Some("small") {
        println!("A3_S2_SCALE_NOT_SELECTED small");
        return Ok(());
    }
    let root = std::env::var_os("KILOGRAM_S2_SCALE_DIR")
        .map(PathBuf::from)
        .ok_or("set private S2 fixture path")?;
    measure(root, false)
}
#[test]
fn history_scale_persistent_100k() -> f::TestResult {
    if std::env::var("KILOGRAM_S2_SCALE_MODE").ok().as_deref() != Some("full") {
        println!("A3_S2_SCALE_NOT_SELECTED full");
        return Ok(());
    }
    let root = std::env::var_os("KILOGRAM_S2_SCALE_DIR")
        .map(PathBuf::from)
        .ok_or("set private S2 fixture path")?;
    measure(root, true)
}
