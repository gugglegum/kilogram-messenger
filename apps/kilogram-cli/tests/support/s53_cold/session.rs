//! One fixed, explicitly activated held session. Never selected by focused tests.
use super::*;
use kilogram_state::{
    StateDirectoryLock,
    verification_lock::{ExistingStateLockInputs, FileIdentity},
};
use sha2::{Digest as _, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle, process::CommandExt},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
};

fn sha(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}
fn public(path: &Path, cap: u64) -> Result<Vec<u8>> {
    use std::os::windows::fs::MetadataExt;
    for parent in path.ancestors() {
        ensure!(
            fs::symlink_metadata(parent)?.file_attributes() & 0x400 == 0,
            "input reparse ancestor"
        );
    }
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .custom_flags(0x0020_0000)
        .open(path)?;
    let before = file.metadata()?;
    ensure!(
        before.is_file() && before.file_attributes() & 0x400 == 0 && before.len() <= cap,
        "bounded no-follow input"
    );
    let mut raw = Vec::new();
    Read::by_ref(&mut file)
        .take(cap + 1)
        .read_to_end(&mut raw)?;
    let after = file.metadata()?;
    ensure!(
        raw.len() as u64 <= cap
            && raw.len() as u64 == before.len()
            && before.len() == after.len()
            && before.last_write_time() == after.last_write_time(),
        "bounded stable input read"
    );
    Ok(raw)
}
fn private(name: &str, value: &impl serde::Serialize, cap: usize) -> Result<()> {
    ensure!(
        [
            "typed.private.json",
            "failure.private.json",
            "progress-failure.private.json"
        ]
        .contains(&name),
        "fixed private receipt name"
    );
    let raw = serde_json::to_vec(value)?;
    ensure!(raw.len() <= cap, "private typed receipt cap");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .share_mode(0)
        .open(Path::new(fixed::EVIDENCE).join(name))?;
    file.write_all(&raw)?;
    file.sync_all()?;
    Ok(())
}
struct Worker {
    child: Child,
    input: Option<ChildStdin>,
    events: Receiver<Result<Vec<u8>, String>>,
    pump: Option<JoinHandle<()>>,
    error_pump: Option<JoinHandle<std::result::Result<(), String>>>,
    transcript: File,
    transcript_bytes: usize,
    nonce: String,
    source: String,
    frames: usize,
    until: Instant,
    stderr: PathBuf,
}
impl Worker {
    fn new(
        source: String,
        assembly: String,
        decision: &str,
        ticks: u64,
        until: Instant,
    ) -> Result<Self> {
        let mut random = [0; 16];
        getrandom::fill(&mut random)?;
        let nonce = state::hex(&random);
        let stderr = Path::new(fixed::EVIDENCE).join("worker.stderr.private.log");
        let mut error = OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(0)
            .open(&stderr)?;
        let transcript = OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(0)
            .open(Path::new(fixed::EVIDENCE).join("worker.control.private.jsonl"))?;
        let mut child = Command::new(fixed::SHELL)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-OutputFormat",
                "Text",
                "-File",
            ])
            .arg(Path::new(fixed::WORKSPACE).join("scripts/invoke-migration-s53-cold-b-worker.ps1"))
            .args([
                "-AssemblySha256",
                &assembly,
                "-SourceSha256",
                &source,
                "-DecisionSha256",
                decision,
                "-WorkDeadlineTimestamp",
                &ticks.to_string(),
            ])
            .creation_flags(0x0800_4000)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let input = child.stdin.take().context("worker stdin")?;
        let output = child.stdout.take().context("worker stdout")?;
        let mut error_pipe = child.stderr.take().context("worker stderr")?;
        let (sender, events) = mpsc::sync_channel(16);
        let error_sender = sender.clone();
        let error_pump = thread::spawn(move || {
            let capture = (|| -> std::result::Result<(), String> {
                let mut scratch = [0u8; 8192];
                let mut total = 0;
                loop {
                    let n = error_pipe.read(&mut scratch).map_err(|_| "stderr read")?;
                    if n == 0 {
                        break;
                    }
                    if n > 131072 - total {
                        return Err("stderr cap".into());
                    }
                    error.write_all(&scratch[..n]).map_err(|_| "stderr write")?;
                    total += n;
                }
                error.sync_all().map_err(|_| "stderr sync")?;
                Ok(())
            })();
            if let Err(ref reason) = capture {
                let _ = error.sync_all();
                let _ = error_sender.try_send(Err(reason.clone()));
            }
            capture
        });
        let pump = thread::spawn(move || {
            let mut reader = BufReader::new(output);
            for _ in 0..16 {
                let mut frame = Vec::with_capacity(4097);
                let event = match reader.by_ref().take(4097).read_until(b'\n', &mut frame) {
                    Ok(0) => Err("EOF".into()),
                    Ok(_) if frame.len() > 4096 || frame.last() != Some(&b'\n') => {
                        Err("frame cap".into())
                    }
                    Ok(_) => Ok(frame),
                    Err(_) => Err("pipe read".into()),
                };
                let end = event.is_err();
                if sender.send(event).is_err() || end {
                    return;
                }
            }
            let _ = sender.send(Err("frame count".into()));
        });
        Ok(Self {
            child,
            input: Some(input),
            events,
            pump: Some(pump),
            error_pump: Some(error_pump),
            transcript,
            transcript_bytes: 0,
            nonce,
            source,
            frames: 0,
            until,
            stderr,
        })
    }
    fn common(&self, kind: &str) -> Value {
        json!({"kind":kind,"version":1,"nonce":self.nonce,"source_sha256":self.source,"attempt":fixed::ATTEMPT})
    }
    fn send(&mut self, frame: &Value) -> Result<()> {
        readers::check(self.until)?;
        self.frames += 1;
        let raw = serde_json::to_vec(frame)?;
        ensure!(
            self.frames <= 16 && raw.len() < 4096,
            "outgoing protocol cap"
        );
        let input = self.input.as_mut().context("worker stdin closed")?;
        input.write_all(&raw)?;
        input.write_all(b"\n")?;
        input.flush()?;
        Ok(())
    }
    fn receive(&mut self, kind: &str) -> Result<Value> {
        let left = self
            .until
            .checked_duration_since(Instant::now())
            .context("worker deadline")?;
        let raw = self
            .events
            .recv_timeout(left)
            .context("worker bounded receive")?
            .map_err(anyhow::Error::msg)?;
        self.frames += 1;
        ensure!(self.frames <= 16, "incoming protocol count");
        self.transcript_bytes = self
            .transcript_bytes
            .checked_add(raw.len())
            .context("control transcript overflow")?;
        ensure!(self.transcript_bytes <= 16 * 4096, "control transcript cap");
        self.transcript.write_all(&raw)?;
        self.transcript.sync_all()?;
        let frame: Value = serde_json::from_slice(&raw)?;
        ensure!(
            frame["version"] == 1
                && frame["nonce"] == self.nonce
                && frame["source_sha256"] == self.source
                && frame["attempt"] == fixed::ATTEMPT,
            "worker binding"
        );
        ensure!(frame["kind"] == kind, "worker phase refused");
        Ok(frame)
    }
    fn reap(&mut self) -> Result<i32> {
        self.input.take();
        let end = (Instant::now() + Duration::from_secs(3)).min(self.until);
        let status = loop {
            if let Some(status) = self.child.try_wait()? {
                break status;
            }
            readers::check(end)?;
            thread::sleep(Duration::from_millis(5));
        };
        while self.pump.as_ref().is_some_and(|p| !p.is_finished()) {
            readers::check(end)?;
            thread::sleep(Duration::from_millis(5));
        }
        if let Some(p) = self.pump.take() {
            p.join().map_err(|_| anyhow::anyhow!("stdout pump panic"))?;
        }
        while self.error_pump.as_ref().is_some_and(|p| !p.is_finished()) {
            readers::check(end)?;
            thread::sleep(Duration::from_millis(5));
        }
        if let Some(p) = self.error_pump.take() {
            p.join()
                .map_err(|_| anyhow::anyhow!("stderr pump panic"))?
                .map_err(anyhow::Error::msg)?;
        }
        ensure!(
            fs::metadata(&self.stderr)?.len() <= 131072,
            "worker stderr cap"
        );
        status.code().context("worker native exit")
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.input.take();
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
            let end = Instant::now() + Duration::from_secs(1);
            while Instant::now() < end && !matches!(self.child.try_wait(), Ok(Some(_))) {
                thread::sleep(Duration::from_millis(5));
            }
        }
        if self.pump.as_ref().is_some_and(|p| p.is_finished())
            && let Some(p) = self.pump.take()
        {
            let _ = p.join();
        }
        if self.error_pump.as_ref().is_some_and(|p| p.is_finished())
            && let Some(p) = self.error_pump.take()
        {
            let _ = p.join();
        }
        let _ = self.transcript.sync_all();
    }
}

fn identities(path: &Path, directory: bool) -> Result<FileIdentity> {
    let file = OpenOptions::new()
        .access_mode(0)
        .share_mode(3)
        .custom_flags(0x0220_0000)
        .open(path)?;
    let meta = file.metadata()?;
    ensure!(meta.is_dir() == directory, "typed input kind");
    Ok(FileIdentity::from_file(&file)?)
}
fn artifact() -> Result<(String, String)> {
    let base = Path::new(fixed::WORKSPACE).join("target/verification-scale/s53-cold-worker");
    let source = std::env::var("KILOGRAM_S53_COLD_SOURCE_SHA256")?;
    let assembly = std::env::var("KILOGRAM_S53_COLD_ASSEMBLY_SHA256")?;
    ensure!(
        source.len() == 64
            && assembly.len() == 64
            && sha(&public(&base.join("worker.cs"), 256 * 1024)?) == source
            && sha(&public(&base.join("worker.dll"), 4 * 1024 * 1024)?) == assembly,
        "reviewed materialized artifacts"
    );
    Ok((source, assembly))
}

#[test]
fn cold_b_private_one_session() -> Result<()> {
    // Fixed opt-in is checked before any original private path/stat/open. The
    // root-approved outer guardian owns E creation, process tree and deadline.
    ensure!(
        std::env::var("KILOGRAM_S53_COLD_ATTEMPT")? == fixed::ATTEMPT,
        "root-dispatched exact private selector"
    );
    for name in [
        "KILOGRAM_TEST_OD1_SCHEDULE",
        "KILOGRAM_TEST_OD1_CLOCK_FILE",
        "KILOGRAM_TEST_OD1_PRIMARY_PAUSE_FILE",
        "KILOGRAM_TEST_MIGRATION_PAUSE_FILE",
    ] {
        ensure!(
            std::env::var_os(name).is_none(),
            "private inherited test hook"
        );
    }
    let (source, assembly) = artifact()?;
    let decision = std::env::var("KILOGRAM_S53_COLD_DECISION_SHA256")?;
    ensure!(
        decision.len() == 64 && decision.bytes().all(|b| b.is_ascii_hexdigit()),
        "reviewed invocation decision hash"
    );
    let absolute = std::env::var("KILOGRAM_S53_COLD_UNTIL_UNIX_MS")?.parse::<u128>()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis();
    ensure!(
        absolute > now && absolute - now <= 210_000,
        "common absolute work deadline"
    );
    let until = Instant::now() + Duration::from_millis((absolute - now) as u64);
    let ticks = std::env::var("KILOGRAM_S53_COLD_DEADLINE_TICKS")?.parse::<u64>()?;
    let mut progress = super::progress::Progress::fixed(until)?;
    progress.record("entry", 0)?;
    let mut inputs = Vec::new();
    let mut held = Vec::new();
    let mut locks = Vec::new();
    for side in ["two", "one"] {
        readers::check(until)?;
        let root = Path::new(fixed::STATE).join(side).join("state");
        let path = root.join(".kilogram-state.lock");
        let dir = identities(&root, true)?;
        let lock = identities(&path, false)?;
        let input = ExistingStateLockInputs::open_existing(&root, dir, lock)?;
        held.push(Some(StateDirectoryLock::adopt_existing_no_recovery(
            input.try_clone()?,
        )?));
        locks.push(json!({"handle":input.borrow_lock_handle().as_raw_handle()as usize as u64,"path":fs::canonicalize(&root)?.join(".kilogram-state.lock"),"volume":lock.volume_serial_number,"index":lock.file_index}));
        inputs.push(input);
    }
    let mut worker = Worker::new(source, assembly, &decision, ticks, until)?;
    let mut bind = worker.common("bind");
    bind["parent_pid"] = json!(std::process::id());
    bind["locks"] = json!(locks);
    progress.record("baseline.start", 0)?;
    worker.send(&bind)?;
    let baseline = worker.receive("baseline")?;
    progress.record("baseline.done", 0)?;
    ensure!(
        baseline["core"]["baseline_completed"] == true && baseline["core"]["slots_released"] == 0,
        "held pre-open equality ACK"
    );
    let mut repositories: Vec<Option<store::L1Repository>> = vec![None, None];
    let mut b0 = Vec::new();
    let mut b1 = Vec::new();
    let mut release_attempted = false;
    let mut typed = (|| -> Result<()> {
        let prepared = [
            fixed::prepare(&Path::new(fixed::STATE).join("two"), 0)?,
            fixed::prepare(&Path::new(fixed::STATE).join("one"), 1)?,
        ];
        let mut proofs = [Vec::new(), Vec::new()];
        for (role, name) in ["a.authority", "b.authority"].into_iter().enumerate() {
            proofs[role] = public(&Path::new(fixed::PROOF).join(name), 16384)?;
            ensure!(
                proofs[role].len() == 100
                    && sha(&proofs[role])
                        == [
                            "6ff24b4dd8e598b1c1b8ea0c3466a75f276753a34b387ff9cb69d373c5a0d912",
                            "3f0b172d5f8eb4a8a43e82856d666768ca71593612d4bcb011bb41e691004dc9"
                        ][role],
                "original R proof byte pin"
            );
        }
        progress.record("slots.start", 0)?;
        release_attempted = true;
        worker.send(&worker.common("release-slots"))?;
        let released = worker.receive("slots")?;
        progress.record("slots.done", 4)?;
        ensure!(
            released["core"]["slots_released"] == 4,
            "exact selective slot ACK"
        );
        for role in 0..2 {
            readers::check(until)?;
            progress.context(role, 0);
            progress.record("open.start", 0)?;
            let lease = held[role].take().context("actual adopted lease")?;
            repositories[role] = Some(fixed::open(&prepared[role], lease)?);
            progress.record("open.done", 0)?;
        }
        for role in 0..2 {
            progress.context(role, 0);
            progress.record("b0.before.start", 0)?;
            b0.push(readers::b0(
                repositories[role].as_mut().context("open repo")?,
                &prepared[role].profile_raw,
                &prepared[role].config_raw,
                &fixed::Expected::production(role)?,
                &proofs,
                until,
            )?);
            progress.record("b0.before.done", 0)?;
        }
        for (role, repository) in repositories.iter_mut().enumerate() {
            let expected = fixed::Expected::production(role)?;
            let repo = repository.as_mut().context("open repo")?;
            progress.context(role, 1);
            progress.record("full.start", 0)?;
            let first = readers::full(repo, &expected, &proofs, until, Some(&mut progress))?;
            progress.record("full.done", first.text_count)?;
            progress.context(role, 2);
            progress.record("readback.start", 0)?;
            let second = readers::full(repo, &expected, &proofs, until, Some(&mut progress))?;
            progress.record("readback.done", second.text_count)?;
            ensure!(
                first.digest == second.digest
                    && first.history_digest == second.history_digest
                    && first.text_count == 528,
                "B1 complete logical repeat"
            );
            b1.push(json!({"digest":first.digest,"history":first.history_digest,"texts":first.text_count,"pages":first.pages}));
        }
        for role in 0..2 {
            let base = Path::new(fixed::STATE).join(if role == 0 { "two" } else { "one" });
            let profile_after = public(&base.join("indexed.profile"), 64 * 1024)?;
            let config_after = public(&base.join("migration.json"), 64 * 1024)?;
            progress.context(role, 0);
            progress.record("b0.after.start", 0)?;
            let after = readers::b0(
                repositories[role].as_mut().context("open repo")?,
                &profile_after,
                &config_after,
                &fixed::Expected::production(role)?,
                &proofs,
                until,
            )?;
            ensure!(after == b0[role], "B0 unchanged after B1");
            progress.record("b0.after.done", 0)?;
        }
        private(
            "typed.private.json",
            &json!({"b0_before_after":b0,"b1_readback":b1,"both_open_same_session":true}),
            8 * 1024 * 1024,
        )?;
        Ok(())
    })();
    let mut close_progress_failure = None;
    for role in 0..2 {
        if let Some(repo) = repositories[role].take() {
            progress.context(role, 0);
            if let Err(error) = progress.record("close.start", 0) {
                close_progress_failure.get_or_insert_with(|| error.to_string());
            }
            held[role] = Some(repo.verification_close_to_held_lease());
            if let Err(error) = progress.record("close.done", 0) {
                close_progress_failure.get_or_insert_with(|| error.to_string());
            }
        }
    }
    if close_progress_failure.is_some() && typed.is_ok() {
        typed = Err(anyhow::anyhow!("protected close progress failed"));
    }
    let lease_count = held.iter().filter(|x| x.is_some()).count();
    if let Err(error) = &typed {
        private(
            "failure.private.json",
            &json!({"phase":"typed","error":format!("{error:#}"),"close_progress_failure":close_progress_failure,"actual_rust_leases_retained":lease_count,"no_restore":true}),
            65536,
        )?;
    }
    // Pre-loader refusal still advances the already-bound worker through its
    // exact release/recapture path, with typed_ok=false and no logical success.
    if !release_attempted {
        worker.send(&worker.common("release-slots"))?;
        worker.receive("slots")?;
    }
    let mut d0 = readers::Digest::new(b"B0-pair");
    d0.value(&b0)?;
    let mut d1 = readers::Digest::new(b"B1-pair");
    d1.value(&b1)?;
    let mut complete = worker.common("typed-complete");
    complete["typed_ok"] = json!(typed.is_ok());
    complete["leases_held"] = json!(lease_count);
    complete["b0"] = json!(d0.finish());
    complete["b1"] = json!(d1.finish());
    let mut final_progress_failure = None;
    progress.pair();
    if let Err(error) = progress.record("final.start", 0) {
        final_progress_failure.get_or_insert_with(|| error.to_string());
    }
    worker.send(&complete)?;
    let final_frame = worker.receive("final")?;
    if let Err(error) = progress.record("final.done", 0) {
        final_progress_failure.get_or_insert_with(|| error.to_string());
    }
    worker.send(&worker.common("finish"))?;
    let done = worker.receive(if typed.is_ok() { "done" } else { "refused" })?;
    let exit = worker.reap()?;
    ensure!(
        done["transport_cleanup_ok"] == true
            && done["transport_handles_after"] == 0
            && done["core"]["remaining_owned_handles"] == 0
            && done["core"]["cleanup_unknown"] == false,
        "actual worker/Core cleanup"
    );
    for lease in held.into_iter().flatten() {
        lease.release_adopted()?;
    }
    drop(inputs);
    if let Some(error) = final_progress_failure {
        private(
            "progress-failure.private.json",
            &json!({"phase":"final","error":error,"worker_reaped":true,"leases_released":true}),
            65536,
        )?;
        anyhow::bail!("protected final progress failed after cleanup");
    }
    // Public output contains only aggregate digests/pinned public identities.
    ensure!(
        typed.is_ok(),
        "typed reader failure; protected failure receipt retained"
    );
    ensure!(
        lease_count == 2
            && exit == 0
            && final_frame["typed_ok"] == true
            && final_frame["both_actual_rust_leases_retained"] == true
            && final_frame["core"]["final_verified"] == true
            && final_frame["core"]["hash_passes"] == 8,
        "combined typed/physical success"
    );
    println!(
        "cold_b_public={}",
        json!({"attempt":fixed::ATTEMPT,"accounts":fixed::ACCOUNTS,"devices":fixed::DEVICES,"context":fixed::CONTEXT,"b0":b0,"b1":b1,"physical":final_frame["core"],"private_receipts":final_frame["private_receipts"],"cleanup":done["core"],"worker_exit":exit})
    );
    Ok(())
}
