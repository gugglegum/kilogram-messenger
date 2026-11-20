use anyhow::{Context, Result};
use kilogram_runtime_ipc::lab_diagnostics::{DIRECTORY, Kind, MARKER, Sink};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
const NONCE: &str = "cccccccccccccccccccccccccccccccc";
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
impl OwnedChild {
    fn finish(&mut self) -> Result<()> {
        let end = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(status) = self.0.try_wait()? {
                assert!(status.success(), "real writer child failed: {status}");
                return Ok(());
            }
            assert!(Instant::now() < end, "real writer child deadline");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
fn child(
    home: &Path,
    role: &str,
    runtime: u32,
    instance: &str,
    go: &Path,
    logs: &Path,
) -> Result<OwnedChild> {
    Ok(OwnedChild(
        Command::new(std::env::current_exe()?)
            .args([
                "--ignored",
                "--exact",
                "t13diag_real_writer_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("KILOGRAM_DIAG_CHILD_HOME", home)
            .env("KILOGRAM_DIAG_CHILD_ROLE", role)
            .env("KILOGRAM_DIAG_CHILD_RUNTIME", runtime.to_string())
            .env("KILOGRAM_DIAG_CHILD_INSTANCE", instance)
            .env("KILOGRAM_DIAG_CHILD_GO", go)
            .stdin(Stdio::null())
            .stdout(fs::File::create(logs.with_extension("stdout.log"))?)
            .stderr(fs::File::create(logs.with_extension("stderr.log"))?)
            .spawn()?,
    ))
}
fn receipts(home: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    fs::read_dir(home.join(DIRECTORY))?
        .map(|entry| {
            let entry = entry?;
            Ok((
                entry.file_name().to_string_lossy().into_owned(),
                fs::read(entry.path())?,
            ))
        })
        .collect()
}
fn marker(home: &Path) -> Result<()> {
    fs::create_dir_all(home.join(DIRECTORY))?;
    fs::write(
        home.join(DIRECTORY).join(MARKER),
        serde_json::to_vec(&json!({"schema":1,"run_nonce":NONCE}))?,
    )?;
    Ok(())
}
#[test]
#[ignore = "called only as a bounded owned child of the real-writer test"]
fn t13diag_real_writer_child() -> Result<()> {
    let home =
        PathBuf::from(std::env::var_os("KILOGRAM_DIAG_CHILD_HOME").context("owned child only")?);
    let role = std::env::var("KILOGRAM_DIAG_CHILD_ROLE")?;
    let runtime: u32 = std::env::var("KILOGRAM_DIAG_CHILD_RUNTIME")?.parse()?;
    let runtime = if role == "runtime" {
        std::process::id()
    } else {
        runtime
    };
    let instance = std::env::var("KILOGRAM_DIAG_CHILD_INSTANCE")?;
    let go = PathBuf::from(std::env::var_os("KILOGRAM_DIAG_CHILD_GO").context("start barrier")?);
    let end = Instant::now() + Duration::from_secs(10);
    while !go.exists() {
        assert!(Instant::now() < end, "start barrier deadline");
        std::thread::sleep(Duration::from_millis(2));
    }
    let sink = Sink::open(&home).context("real sink")?;
    let kinds = if role == "runtime" {
        vec![
            Kind::RuntimeStopping,
            Kind::RuntimeNetworkStopped,
            Kind::RuntimeFinal,
        ]
    } else {
        vec![
            Kind::StartupSetupOpen,
            Kind::StartupBegin,
            Kind::StartupImportBegin,
            Kind::StartupImportEnd,
            Kind::StartupProfileBegin,
            Kind::StartupPrepared,
            Kind::StartupSpawned,
            Kind::StartupDescriptor,
            Kind::StartupStatus,
            Kind::ConnectionFinal,
            Kind::WorkerFinal,
        ]
    };
    let mut written = 0;
    for (index, kind) in kinds.into_iter().enumerate() {
        let bound = role == "runtime" || index >= 8;
        let ok = sink.record(
            kind,
            bound.then_some(runtime),
            bound.then_some(instance.as_str()),
            &json!({"ok":true}),
        );
        if role != "quota" {
            assert!(ok, "real {role} record {index} rejected");
        }
        written += usize::from(ok);
        std::thread::sleep(Duration::from_millis(2));
    }
    let result = home
        .parent()
        .context("fixture parent")?
        .join(format!("writer-result-{}.json", std::process::id()));
    fs::write(
        result,
        serde_json::to_vec(&json!({"pid":std::process::id(),"written":written}))?,
    )?;
    Ok(())
}
#[test]
fn t13diag_real_writer_two_incarnations_and_concurrent_cap() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let evidence = std::env::var_os("KILOGRAM_DIAG_WRITER_EVIDENCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| temp.path().join("evidence"));
    fs::create_dir(&evidence).context("fresh writer evidence")?;
    let home = evidence.join("runs/one/setup");
    marker(&home)?;
    let mut scopes: Vec<Value> = Vec::new();
    let mut first = BTreeMap::new();
    for incarnation in 0..2 {
        let instance = if incarnation == 0 {
            "a".repeat(64)
        } else {
            "b".repeat(64)
        };
        let go = evidence.join(format!("go-{incarnation}"));
        let mut runtime = child(
            &home,
            "runtime",
            0,
            &instance,
            &go,
            &evidence.join(format!("runtime-{incarnation}")),
        )?;
        let runtime_pid = runtime.0.id();
        let mut chat = child(
            &home,
            "chat",
            runtime_pid,
            &instance,
            &go,
            &evidence.join(format!("chat-{incarnation}")),
        )?;
        let chat_pid = chat.0.id();
        assert_ne!(runtime_pid, chat_pid);
        fs::write(&go, b"go")?;
        runtime.finish()?;
        chat.finish()?;
        let current = receipts(&home)?;
        assert_eq!(current.len(), 1 + 14 * (incarnation + 1));
        if incarnation == 0 {
            first = current.clone();
            let copy = evidence.join("first-home").join(DIRECTORY);
            fs::create_dir_all(&copy)?;
            for (name, bytes) in &first {
                fs::write(copy.join(name), bytes)?;
            }
        } else {
            for (name, bytes) in &first {
                assert_eq!(current[name], *bytes, "old receipt changed");
            }
        }
        scopes.push(json!({"chat_pid":chat_pid,"runtime_pid":runtime_pid,"instance":instance}));
    }
    let mut pids = BTreeSet::new();
    for scope in &scopes {
        for key in ["chat_pid", "runtime_pid"] {
            pids.insert(scope[key].as_u64().context("actual child PID")?);
        }
    }
    assert_eq!(pids.len(), 4, "distinct actual child PIDs");
    assert_eq!(receipts(&home)?[MARKER], first[MARKER]);
    // Copy real writer bytes into a separate cap fixture; keep reader input intact.
    let cap_home = evidence.join("cap-home");
    fs::create_dir_all(cap_home.join(DIRECTORY))?;
    for (name, bytes) in receipts(&home)? {
        fs::write(cap_home.join(DIRECTORY).join(name), bytes)?;
    }
    let go = evidence.join("go-cap");
    let mut left = child(
        &cap_home,
        "quota",
        1,
        &"c".repeat(64),
        &go,
        &evidence.join("cap-left"),
    )?;
    let mut right = child(
        &cap_home,
        "quota",
        1,
        &"c".repeat(64),
        &go,
        &evidence.join("cap-right"),
    )?;
    let quota_pids = [left.0.id(), right.0.id()];
    fs::write(go, b"go")?;
    left.finish()?;
    right.finish()?;
    let counts: Vec<usize> = quota_pids
        .iter()
        .map(|pid| -> Result<usize> {
            let value: Value = serde_json::from_slice(&fs::read(
                evidence.join(format!("writer-result-{pid}.json")),
            )?)?;
            Ok(usize::try_from(
                value["written"].as_u64().context("writer count")?,
            )?)
        })
        .collect::<Result<_>>()?;
    assert_eq!(counts.iter().sum::<usize>(), 4);
    assert_eq!(
        receipts(&cap_home)?.len(),
        33,
        "32 records plus marker, no race overshoot"
    );
    let full = Sink::open(&cap_home).context("marker separate from record cap")?;
    assert!(!full.record(Kind::StartupFailed, None, None, &true));
    assert_eq!(receipts(&cap_home)?.len(), 33);
    fs::write(cap_home.join(DIRECTORY).join("overflow.json"), b"{}")?;
    assert!(Sink::open(&cap_home).is_none());
    fs::write(
        evidence.join("scopes.json"),
        serde_json::to_vec_pretty(
            &json!({"nonce":NONCE,"scopes":scopes,"old_files_unchanged":true,"marker_unchanged":true,"cap_successes":counts,"record_cap":32,"marker_separate":true}),
        )?,
    )?;
    Ok(())
}
