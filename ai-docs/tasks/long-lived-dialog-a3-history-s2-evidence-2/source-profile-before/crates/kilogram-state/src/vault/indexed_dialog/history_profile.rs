//! Private replay/profiling over explicit synthetic copies, never a runtime importer.
use super::*;
use super::{scale as s, tests as f};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};

#[cfg(windows)]
fn sha256(path: &Path) -> std::result::Result<String, Box<dyn std::error::Error>> {
    use std::os::windows::process::CommandExt;
    let quoted = path.to_string_lossy().replace('\'', "''");
    let command = format!(
        "$fixtureStream=[IO.File]::OpenRead('{quoted}'); try {{ $fixtureHasher=[Security.Cryptography.SHA256]::Create(); try {{ ([BitConverter]::ToString($fixtureHasher.ComputeHash($fixtureStream))).Replace('-','').ToLowerInvariant() }} finally {{ $fixtureHasher.Dispose() }} }} finally {{ $fixtureStream.Dispose() }}"
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
        return Err(format!(
            "fixture hash query failed: {} {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
        .into());
    }
    Ok(std::str::from_utf8(&out.stdout)?.trim().to_owned())
}
#[cfg(not(windows))]
fn sha256(_path: &Path) -> std::result::Result<String, Box<dyn std::error::Error>> {
    Err("SHA provenance helper requires Windows".into())
}

fn open_copy(
    root: &Path,
    provenance: &Path,
) -> std::result::Result<IndexedDialogVault, Box<dyn std::error::Error>> {
    let entries: Vec<Value> = serde_json::from_slice(&fs::read(provenance)?)?;
    let canonical = root.canonicalize()?;
    let entry = entries
        .iter()
        .find(|e| {
            e["path"]
                .as_str()
                .is_some_and(|p| Path::new(p).canonicalize().is_ok_and(|p| p == canonical))
        })
        .ok_or("copy absent from provenance")?;
    if entry["synthetic"] != true
        || entry["expected_user_rows"] != 10016
        || sha256(&root.join(STATE_VAULT_FILE))?
            != entry["initial_db_sha256"].as_str().ok_or("copy hash")?
    {
        return Err("fixture provenance/hash mismatch; no adoption".into());
    }
    let mut store =
        IndexedDialogVault::open(StateDirectoryLock::acquire(root)?, f::owner(), s::config())?;
    let before = store.root.encode();
    let repeat = f::apply(&mut store, 0, HistoryOperation::Init(f::init(0)))?;
    if !repeat.duplicate
        || store.root.encode() != before
        || s::count(&s::current_heads(&mut store)?) != 10016
    {
        return Err("fixture scope/coherence/count mismatch".into());
    }
    Ok(store)
}
fn at_cut(store: &mut IndexedDialogVault, cut: u64) -> Result<(HistoryCursor, [u64; 2])> {
    let own = if cut == 0 { 0 } else { (cut - 1) / 16 + 1 };
    let next = [own + 1, cut - own + 1];
    let tx = store.vault.database.begin_read().map_err(db)?;
    let nodes = tx.open_table(NODES).map_err(db)?;
    let values = tx.open_table(VALUES).map_err(db)?;
    let mut w = Work::new(&nodes, &values, &store.crypto, store.root.clone());
    let (identity, heads) = w.history_view(0)?;
    Ok((
        cursor::seal(
            &store.crypto,
            &identity,
            &Position {
                frontiers: heads.frontiers,
                next,
            },
        )?,
        next,
    ))
}
fn oracle_at(store: &mut IndexedDialogVault, next: [u64; 2]) -> Result<Heads> {
    let a = id_value(
        &store
            .get(lane(0, 0, next[0] - 1)?)?
            .ok_or(Error::Integrity("oracle Lane A"))?,
    )?;
    let b = id_value(
        &store
            .get(lane(0, 1, next[1] - 1)?)?
            .ok_or(Error::Integrity("oracle Lane B"))?,
    )?;
    Ok(Heads {
        view: [0; 32],
        frontiers: next,
        tails: [Some(a), Some(b)],
    })
}
fn read_samples(
    store: &mut IndexedDialogVault,
) -> std::result::Result<Value, Box<dyn std::error::Error>> {
    let mut stats = profile::Stats::default();
    let mut page_ms = Vec::new();
    let mut point_ms = Vec::new();
    let mut oracle_ms = 0.0;
    let mut external_cbor_ms = 0.0;
    let mut external_json_ms = 0.0;
    let mut max_gets = 0;
    let mut max_candidates = 0;
    let before = store.backend_stats()?;
    for cut in [
        0, 32, 512, 1024, 2048, 4096, 4992, 6144, 7168, 8192, 9216, 9952,
    ] {
        let (position, next) = at_cut(store, cut)?;
        let started = Instant::now();
        let (page, p) = profile::capture(|| store.history_page(0, &position, 32));
        page_ms.push(started.elapsed().as_secs_f64() * 1000.0);
        stats.add(p);
        let page = page?;
        max_gets = max_gets.max(page.stats.point_gets);
        max_candidates = max_candidates.max(page.stats.max_candidates);
        let oracle = Instant::now();
        let mut expected = oracle_at(store, next)?;
        let mut index = cut;
        s::validate_items(&page.items, &mut expected, &mut index)?;
        oracle_ms += oracle.elapsed().as_secs_f64() * 1000.0;
        let encode = Instant::now();
        page.encode_cbor()?;
        external_cbor_ms += encode.elapsed().as_secs_f64() * 1000.0;
        let encode = Instant::now();
        page.encode_json()?;
        external_json_ms += encode.elapsed().as_secs_f64() * 1000.0;
        let event = page.items.first().ok_or("profile page")?.id;
        let started = Instant::now();
        let (point, p) = profile::capture(|| store.history_point(0, event));
        point_ms.push(started.elapsed().as_secs_f64() * 1000.0);
        stats.add(p);
        if point?.is_none() {
            return Err("profile point".into());
        }
    }
    let after = store.backend_stats()?;
    Ok(
        json!({"kind":"bounded_read_replay","instrumented":true,"page_api_ms":page_ms,"point_api_ms":point_ms,
        "oracle_ms_excluded":oracle_ms,"external_cbor_ms_excluded":external_cbor_ms,"external_json_ms_excluded":external_json_ms,
        "api_encodings_included_in_api_and_phase_timings":true,"profile":profile::json(stats),"max_point_gets":max_gets,"max_candidates":max_candidates,
        "backend_read_bytes":after.read_bytes-before.read_bytes,"backend_write_bytes":after.write_bytes-before.write_bytes,
        "backend_sync_ms":(after.sync_nanos-before.sync_nanos) as f64/1e6,"cursor_seeding":"private known synthetic chain cuts, no prefix traversal"}),
    )
}
fn write_samples(
    store: &mut IndexedDialogVault,
) -> std::result::Result<Value, Box<dyn std::error::Error>> {
    let mut heads = s::current_heads(store)?;
    let initial = s::count(&heads);
    let mut rows = Vec::new();
    let prepared = Instant::now();
    for i in initial..initial + 16 {
        rows.push(s::next_row(&mut heads, i)?);
    }
    let ordinary_oracle_ms = prepared.elapsed().as_secs_f64() * 1000.0;
    let before = store.backend_stats()?;
    let start = Instant::now();
    let (out, ordinary) = profile::capture(|| -> Result<()> {
        for row in rows {
            f::append(store, 0, row)?;
        }
        Ok(())
    });
    out?;
    let ordinary_ms = start.elapsed().as_secs_f64() * 1000.0;
    let after = store.backend_stats()?;
    let mut rows = Vec::new();
    let current = s::count(&heads);
    let prepared = Instant::now();
    for i in current..current + 128 {
        rows.push(s::next_row(&mut heads, i)?);
    }
    let bulk_oracle_ms = prepared.elapsed().as_secs_f64() * 1000.0;
    let fill_before = store.backend_stats()?;
    let start = Instant::now();
    let (out, fill) = profile::capture(|| -> Result<CommitReport> {
        for row in &rows {
            if row.role == 0 {
                f::apply(
                    store,
                    0,
                    HistoryOperation::QueueAccept {
                        request: row.request,
                        text: row.text.clone(),
                    },
                )?;
            }
        }
        store.history_fixture_commit(rows)
    });
    let report = out?;
    let fill_ms = start.elapsed().as_secs_f64() * 1000.0;
    let fill_after = store.backend_stats()?;
    Ok(
        json!({"kind":"synthetic_copy_write","instrumented":true,"initial_user_rows":initial,"final_user_rows":s::count(&heads),
        "ordinary16_ms":ordinary_ms,"ordinary_oracle_ms_excluded":ordinary_oracle_ms,"ordinary_profile":profile::json(ordinary),
        "ordinary_sync_ms":(after.sync_nanos-before.sync_nanos) as f64/1e6,"ordinary_write_bytes":after.write_bytes-before.write_bytes,
        "fill128_ms":fill_ms,"fill_oracle_ms_excluded":bulk_oracle_ms,"fill_profile":profile::json(fill),
        "fill_sync_ms":(fill_after.sync_nanos-fill_before.sync_nanos) as f64/1e6,"fill_write_bytes":fill_after.write_bytes-fill_before.write_bytes,
        "actual_plan_bytes":report.encoded_plan_bytes,"app_buffer_bound":report.app_buffer_bound_bytes}),
    )
}
#[test]
fn history_scale_profile_saved_fixture() -> f::TestResult {
    let Ok(kind) = std::env::var("KILOGRAM_S2_PROFILE_KIND") else {
        println!("A3_S2_PROFILE_NOT_SELECTED");
        return Ok(());
    };
    let root = std::env::var_os("KILOGRAM_S2_PROFILE_COPY")
        .map(PathBuf::from)
        .ok_or("set synthetic profile copy")?;
    let provenance = std::env::var_os("KILOGRAM_S2_PROFILE_PROVENANCE")
        .map(PathBuf::from)
        .ok_or("set copy provenance")?;
    let mut store = open_copy(&root, &provenance)?;
    let result = match kind.as_str() {
        "read" => read_samples(&mut store)?,
        "write" => write_samples(&mut store)?,
        _ => return Err("profile kind".into()),
    };
    println!("A3_S2_PROFILE {}", serde_json::to_string(&result)?);
    Ok(())
}
