use super::*;

fn dummy() -> admission::Admission {
    admission::Admission {
        version: 1,
        attempt: "pure-no-child".into(),
        owner_pid: 1,
        launcher_pid: 2,
        harness_path: PathBuf::from("unused"),
        whole_utc_ms: 1,
    }
}
fn scratch(name: &str) -> Result<PathBuf> {
    let base = workspace().join(".tmp/s54-p11a7-capture-tests");
    std::fs::create_dir_all(&base)?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let path = base.join(format!("{name}-{}-{nanos}", std::process::id()));
    std::fs::create_dir(&path)?;
    Ok(path)
}
#[test]
fn s54_capture_transform_original_nested_and_safe_errors() -> Result<()> {
    let original = json!({"keys":{"public":"S54_STATUS_SENTINEL"},"array":[{"keys":[1,2]}]});
    let raw = serde_json::to_vec(&original)?;
    let mut facts = Facts::default();
    let (retained, durable) = transform(&raw, &mut facts, live_l1::status_fingerprints)?;
    assert!(retained == original);
    let public: Value = serde_json::from_slice(&durable)?;
    assert!(
        public["keys_fingerprint"]
            == blake3::hash(&serde_json::to_vec(&original["keys"])?)
                .to_hex()
                .as_str()
    );
    assert!(public["array"][0]["keys_fingerprint"] == blake3::hash(b"[1,2]").to_hex().as_str());
    assert!(!String::from_utf8(durable)?.contains("S54_STATUS_SENTINEL"));
    for bytes in [b"{\"keys\":\"S54_STATUS_SENTINEL\"".as_slice(), &[255, 254]] {
        let error = transform(bytes, &mut facts, live_l1::status_fingerprints)
            .err()
            .context("must reject malformed")?;
        assert!(!format!("{error:#}").contains("S54_STATUS_SENTINEL"));
    }
    let error = transform(&raw, &mut facts, |_| anyhow::bail!("S54_STATUS_SENTINEL"))
        .err()
        .context("sanitizer rejection")?;
    assert!(error.to_string() == "capture sanitizer_failure");
    assert!(facts.classification == "sanitizer_failure");
    Ok(())
}
#[test]
fn s54_capture_scope_sequence_restore_and_pending_is_permanent_failure() -> Result<()> {
    assert!(Guard::enter(Instant::now() + Duration::from_secs(1)).is_err());
    let args: Vec<String> = ["ordered", "status3", "--ipc-file", "unused"]
        .map(String::from)
        .into();
    assert!(intercept(Path::new("unused-no-spawn"), &args).is_none());
    let path = scratch("pure-scope")?;
    let _deadline = s53_deadline::Guard::enter(&path, 60)?;
    let first = s53_deadline::capture_bound()?;
    {
        let local = Instant::now() + Duration::from_secs(10);
        let _phase = s53_deadline::Phase::until(local)?;
        let _capture = Guard::admit(local, first.whole, dummy())?;
        assert!(Guard::admit(local, first.whole, dummy()).is_err());
        assert!(
            intercept(Path::new("unused"), &["ordered".into(), "status".into()])
                .context("status intercepted")?
                .is_err()
        );
        assert!(
            intercept(
                Path::new("unused"),
                &["ordered".into(), "status-ish".into()]
            )
            .is_none()
        );
        let second = s53_deadline::capture_bound()?;
        assert!(second.sequence == 1 && second.work == local && second.whole == first.whole);
        let mut pending = Facts::default();
        pending.unresolved();
        assert!(pending.cleanup_pending && healthy().is_err());
        assert!(run(command(Path::new("must-not-spawn"))).is_err());
    }
    assert!(!active());
    let restored = s53_deadline::capture_bound()?;
    assert!(restored.sequence == 2 && restored.work == first.work && restored.whole == first.whole);
    let _new_scope = Guard::admit(first.work, first.whole, dummy())?;
    healthy()?;
    Ok(())
}
#[test]
fn s54_capture_log_collisions_and_writer_io_never_use_raw_fallback() -> Result<()> {
    let path = scratch("pure-io")?;
    let _deadline = s53_deadline::Guard::enter(&path, 60)?;
    let _capture = Guard::admit(
        Instant::now() + Duration::from_secs(30),
        Instant::now() + Duration::from_secs(60),
        dummy(),
    )?;
    let bound = s53_deadline::capture_bound()?;
    std::fs::write(&bound.out, b"existing evidence")?;
    assert!(Logs::new(&bound).is_err());
    assert!(std::fs::read(&bound.out)? == b"existing evidence");
    // Inject a real write failure with a read-only file handle; no process.
    let next = s53_deadline::capture_bound()?;
    let mut logs = Logs::new(&next)?;
    logs.err = File::open(&bound.out)?;
    assert!(
        logs.facts(&Facts {
            classification: "safe_failure",
            ..Default::default()
        })
        .is_err()
    );
    assert!(std::fs::read(&bound.out)? == b"existing evidence");
    Ok(())
}

#[cfg(windows)]
fn actual(mode: &str, expected: &str) -> Result<()> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let shell = Path::new(
        "C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe",
    );
    let mut hash = Sha256::new();
    let mut file = File::open(shell)?;
    let mut chunk = [0; 8192];
    loop {
        let n = file.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        hash.update(&chunk[..n]);
    }
    ensure!(
        format!("{:x}", hash.finalize())
            == "362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139",
        "pinned fixture executable"
    );
    let fixture =
        workspace().join("apps/kilogram-cli/tests/support/migration_s54_capture_fixture.ps1");
    ensure!(
        std::fs::read(&fixture)? == include_bytes!("migration_s54_capture_fixture.ps1"),
        "immutable fixture source"
    );
    let path = scratch(mode)?;
    let _deadline = s53_deadline::Guard::enter(&path, 40)?;
    let start = Instant::now();
    let work = start + Duration::from_secs(if mode == "stall" { 8 } else { 25 });
    let _capture = Guard::enter(work)?;
    let mut cmd = command(shell);
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-NoLogo",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
    ])
    .arg(fixture)
    .args(["-Mode", mode]);
    let result = run(cmd);
    ensure!(
        start.elapsed() < Duration::from_secs(35),
        "bounded actual capture return"
    );
    let stdout = std::fs::read(path.join("command-0000.stdout.log"))?;
    let stderr = std::fs::read(path.join("command-0000.stderr.log"))?;
    for bytes in [&stdout, &stderr] {
        let text = std::str::from_utf8(bytes)?;
        ensure!(
            !text.contains("S54_STATUS_SENTINEL")
                && !text.contains("S54_STDERR_SENTINEL")
                && !text.contains("\"keys\":"),
            "durable sentinel absence"
        );
    }
    let record: Value = serde_json::from_slice(&stderr)?;
    let facts = &record["capture"];
    ensure!(
        facts["classification"] == expected
            && facts["waited"] == true
            && facts["exit_observed"] == true
            && facts["cleanup_pending"] == false
            && facts["stdout_eof"] == true
            && facts["stderr_eof"] == true,
        "actual exit/wait/EOF facts"
    );
    if expected == "success" {
        let original = result?;
        ensure!(
            original["keys"]["public"] == "S54_STATUS_SENTINEL"
                && original["nested"][0]["keys"][0] == "S54_STATUS_SENTINEL",
            "original identity fields retained"
        );
        let public: Value = serde_json::from_slice(&stdout)?;
        ensure!(
            public["keys_fingerprint"].is_string() && public.get("keys").is_none(),
            "durable sanitized value"
        );
        if mode == "pressure" {
            ensure!(
                facts["stdout_bytes"].as_u64().unwrap_or(0) > 524288
                    && facts["stderr_bytes"].as_u64().unwrap_or(0) > 524288,
                "both actual streams under pressure"
            );
        }
    } else {
        let error = result.err().context("actual case must fail")?.to_string();
        ensure!(
            !error.contains("S54_STATUS_SENTINEL")
                && !error.contains("S54_STDERR_SENTINEL")
                && stdout.is_empty(),
            "safe actual error without partial success"
        );
        ensure!(
            healthy().is_err(),
            "failed capture scope cannot resume success"
        );
    }
    std::fs::write(
        path.join("case-result.json"),
        serde_json::to_vec(
            &json!({"mode":mode,"expected":expected,"actual":record,"elapsed_seconds":start.elapsed().as_secs_f64(),"classification":"synthetic capture only; outer cleanup receipt required"}),
        )?,
    )?;
    println!(
        "synthetic_capture_case={} pid={} evidence={}",
        mode,
        facts["pid"],
        path.display()
    );
    Ok(())
}
#[cfg(windows)]
#[test]
fn s54_capture_actual_valid() -> Result<()> {
    actual("valid", "success")
}
#[cfg(windows)]
#[test]
fn s54_capture_actual_pressure() -> Result<()> {
    actual("pressure", "success")
}
#[cfg(windows)]
#[test]
fn s54_capture_actual_malformed() -> Result<()> {
    actual("malformed", "invalid_json")
}
#[cfg(windows)]
#[test]
fn s54_capture_actual_invalid_utf8() -> Result<()> {
    actual("invalid-utf8", "invalid_utf8")
}
#[cfg(windows)]
#[test]
fn s54_capture_actual_nonzero() -> Result<()> {
    actual("nonzero", "nonzero_exit")
}
#[cfg(windows)]
#[test]
fn s54_capture_actual_stdout_overflow() -> Result<()> {
    actual("stdout-overflow", "stdout_cap")
}
#[cfg(windows)]
#[test]
fn s54_capture_actual_stderr_overflow() -> Result<()> {
    actual("stderr-overflow", "stderr_cap")
}
#[cfg(windows)]
#[test]
fn s54_capture_actual_stall() -> Result<()> {
    actual("stall", "deadline")
}
