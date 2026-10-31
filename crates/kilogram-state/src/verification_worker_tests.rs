use super::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    io::{BufRead, BufReader, Read, Write},
    os::windows::io::AsRawHandle,
    process::{Child, ChildStdin, Command, ExitStatus, Stdio},
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const SHELL: &str = r"C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe";
const START: Duration = Duration::from_secs(15);
const STEP: Duration = Duration::from_secs(3);

struct Pair {
    leases: Vec<StateDirectoryLock>,
    inputs: Vec<ExistingStateLockInputs>,
    // Retain the actual Rust-origin guards even when data handles are released.
    _guards: Vec<Arc<Origin>>,
    roots: Vec<PathBuf>,
    locks: Value,
}
impl Pair {
    fn new() -> TestResult<Self> {
        let base =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/C-s53-cold-b0b-synthetic");
        fs::create_dir_all(&base)?;
        let run = tempfile::Builder::new()
            .prefix("b0b-")
            .tempdir_in(fs::canonicalize(base)?)?
            .keep();
        let mut pair = Self {
            leases: Vec::new(),
            inputs: Vec::new(),
            _guards: Vec::new(),
            roots: Vec::new(),
            locks: json!([]),
        };
        let mut rows = Vec::new();
        for role in ["a", "b"] {
            let root = run.join(role);
            fs::create_dir(&root)?;
            fs::write(root.join(LOCK_FILE), [])?;
            let dir = FileIdentity::from_file(&guard(&root, true)?)?;
            let lock = FileIdentity::from_file(&guard(&root.join(LOCK_FILE), false)?)?;
            let inputs = ExistingStateLockInputs::open_existing(&root, dir, lock)?;
            rows.push(json!({"handle":inputs.borrow_lock_handle().as_raw_handle() as usize as u64,
                "path":fs::canonicalize(&root)?.join(LOCK_FILE), "volume":lock.volume_serial_number,"index":lock.file_index}));
            pair._guards.push(Arc::clone(&inputs.origin));
            pair.leases
                .push(StateDirectoryLock::adopt_existing_no_recovery(
                    inputs.try_clone()?,
                )?);
            pair.inputs.push(inputs);
            pair.roots.push(root);
        }
        pair.locks = Value::Array(rows);
        Ok(pair)
    }
    fn release(&mut self) -> TestResult {
        for lease in self.leases.drain(..) {
            lease.release_adopted()?;
        }
        self.inputs.clear();
        Ok(())
    }
    fn excluded(&self) -> TestResult {
        for root in &self.roots {
            let error = match OpenOptions::new()
                .read(true)
                .share_mode(7)
                .open(root.join(LOCK_FILE))
            {
                Err(error) => error,
                Ok(_) => return Err("expected actual share0 exclusion".into()),
            };
            assert_eq!(error.raw_os_error(), Some(32));
        }
        Ok(())
    }
    fn released(&self) -> TestResult {
        for root in &self.roots {
            let file = OpenOptions::new()
                .read(true)
                .share_mode(7)
                .open(root.join(LOCK_FILE))?;
            file.try_lock()?;
            file.unlock()?;
            assert_eq!(file.metadata()?.len(), 0);
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
enum ReceiveError {
    #[error("worker stdout EOF")]
    Eof,
    #[error("worker response deadline")]
    Deadline,
    #[error("worker frame cap")]
    Limit,
    #[error("worker I/O: {0}")]
    Io(#[from] io::Error),
    #[error("worker JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("worker binding")]
    Binding,
}
enum Event {
    Frame(Vec<u8>),
    Eof,
    Limit,
    Io(io::Error),
}
struct Worker {
    child: Child,
    input: Option<ChildStdin>,
    events: Receiver<Event>,
    pump: Option<JoinHandle<()>>,
    attempt: String,
    nonce: String,
    hash: String,
    frames: usize,
}
impl Worker {
    fn spawn(case: &str, early_eof: bool) -> TestResult<Self> {
        let script = fs::canonicalize(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../scripts/migration-s53-typed-lock-worker.ps1"),
        )?;
        let hash = format!("{:x}", Sha256::digest(fs::read(&script)?));
        // -File with a verbatim \\?\ path can fail AuthorizationManager.
        // Keep the canonical source pin, but invoke its checked ordinary path.
        let command_script = literal(&script)?;
        if fs::canonicalize(&command_script)? != script {
            return Err("worker source path identity changed".into());
        }
        let mut random = [0u8; 16];
        getrandom::fill(&mut random)?;
        let nonce = random.iter().map(|byte| format!("{byte:02x}")).collect();
        let attempt = format!("C-s53-cold-b0b-{case}");
        let mut command = Command::new(SHELL);
        command
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-OutputFormat",
                "Text",
                "-File",
            ])
            .arg(command_script)
            .arg("-Attempt")
            .arg(&attempt)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        if early_eof {
            command.arg("-SyntheticEarlyEof");
        }
        let mut child = command.spawn()?;
        let input = child.stdin.take().ok_or("child stdin")?;
        let output = child.stdout.take().ok_or("child stdout")?;
        let (sender, events) = mpsc::channel();
        let pump = thread::spawn(move || {
            let mut reader = BufReader::new(output);
            for _ in 0..16 {
                let mut frame = Vec::with_capacity(4097);
                let result = reader.by_ref().take(4097).read_until(b'\n', &mut frame);
                let event = match result {
                    Ok(0) => {
                        let _ = sender.send(Event::Eof);
                        return;
                    }
                    Ok(_) if frame.len() > 4096 || frame.last() != Some(&b'\n') => Event::Limit,
                    Ok(_) => Event::Frame(frame),
                    Err(error) => Event::Io(error),
                };
                let stop = !matches!(event, Event::Frame(_));
                if sender.send(event).is_err() || stop {
                    return;
                }
            }
            let _ = sender.send(Event::Limit);
        });
        Ok(Self {
            child,
            input: Some(input),
            events,
            pump: Some(pump),
            attempt,
            nonce,
            hash,
            frames: 0,
        })
    }
    fn common(&self, kind: &str) -> Value {
        json!({"kind":kind,"version":1,"attempt":self.attempt,"nonce":self.nonce,"source_sha256":self.hash})
    }
    fn bind(&self, pair: &Pair) -> Value {
        let mut frame = self.common("bind");
        frame["parent_pid"] = json!(std::process::id());
        frame["locks"] = pair.locks.clone();
        frame
    }
    fn send(&mut self, frame: &Value) -> TestResult {
        self.frames += 1;
        let bytes = serde_json::to_vec(frame)?;
        if self.frames > 16 || bytes.len() + 1 > 4096 {
            return Err("outbound frame cap".into());
        }
        let input = self.input.as_mut().ok_or("closed stdin")?;
        input.write_all(&bytes)?;
        input.write_all(b"\n")?;
        input.flush()?;
        Ok(())
    }
    fn receive(&mut self, timeout: Duration) -> Result<Value, ReceiveError> {
        let event = match self.events.recv_timeout(timeout) {
            Ok(event) => event,
            Err(mpsc::RecvTimeoutError::Timeout) => return Err(ReceiveError::Deadline),
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(ReceiveError::Eof),
        };
        match event {
            Event::Eof => Err(ReceiveError::Eof),
            Event::Limit => Err(ReceiveError::Limit),
            Event::Io(error) => Err(ReceiveError::Io(error)),
            Event::Frame(bytes) => {
                self.frames += 1;
                if self.frames > 16 {
                    return Err(ReceiveError::Limit);
                }
                let frame: Value = serde_json::from_slice(&bytes)?;
                if frame["version"] != 1
                    || frame["attempt"] != self.attempt
                    || frame["nonce"] != self.nonce
                    || frame["source_sha256"] != self.hash
                {
                    return Err(ReceiveError::Binding);
                }
                Ok(frame)
            }
        }
    }
    fn reap(&mut self) -> TestResult<ExitStatus> {
        self.input.take();
        let until = Instant::now() + STEP;
        let status = loop {
            if let Some(status) = self.child.try_wait()? {
                break status;
            }
            if Instant::now() >= until {
                return Err("owned child reap deadline".into());
            }
            thread::sleep(Duration::from_millis(5));
        };
        let until = Instant::now() + STEP;
        while self.pump.as_ref().is_some_and(|pump| !pump.is_finished()) {
            if Instant::now() >= until {
                return Err("owned stdout pump cleanup deadline".into());
            }
            thread::sleep(Duration::from_millis(5));
        }
        if let Some(pump) = self.pump.take() {
            pump.join().map_err(|_| "stdout pump panicked")?;
        }
        Ok(status)
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.input.take();
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
            let until = Instant::now() + Duration::from_secs(1);
            while Instant::now() < until && !matches!(self.child.try_wait(), Ok(Some(_))) {
                thread::sleep(Duration::from_millis(5));
            }
        }
        if self.pump.as_ref().is_some_and(|pump| pump.is_finished())
            && let Some(pump) = self.pump.take()
        {
            let _ = pump.join();
        }
    }
}
fn duplicated(frame: &Value, pair: &Pair) {
    assert_eq!(frame["kind"], "duplicated");
    assert_eq!(frame["parent_pid"], std::process::id());
    assert_eq!(frame["read_attempts"], 0);
    assert_eq!(frame["owned_handles"], 2);
    assert_eq!(frame["parent_closed"], true);
    assert_eq!(frame["snapshot_closed"], true);
    for n in 0..2 {
        for field in ["path", "volume", "index"] {
            assert_eq!(frame["locks"][n][field], pair.locks[n][field]);
        }
        assert_eq!(frame["locks"][n]["noninheritable"], true);
        assert_eq!(frame["locks"][n]["links"], 1);
        assert_eq!(frame["locks"][n]["bytes"], 0);
    }
}
fn cleaned(frame: &Value, created: u64) {
    assert_eq!(frame["duplicates_created"], created);
    assert_eq!(
        frame["closed_handles"],
        if created == 0 { 0 } else { created + 1 }
    );
    assert_eq!(frame["handles_after"], 0);
    assert_eq!(frame["parent_closed"], true);
    assert_eq!(frame["snapshot_closed"], true);
    assert_eq!(frame["cleanup_ok"], true);
}

#[test]
fn happy_lifecycle() -> TestResult {
    let mut pair = Pair::new()?;
    let mut worker = Worker::spawn("happy", false)?;
    worker.send(&worker.bind(&pair))?;
    let ack = worker.receive(START)?;
    duplicated(&ack, &pair);
    println!("duplicate_witness={ack}");
    pair.release()?;
    // Only the child's data handles now enforce share0; Rust keeps metadata guards.
    pair.excluded()?;
    worker.send(&worker.common("released"))?;
    let read = worker.receive(STEP)?;
    assert_eq!(read["kind"], "read");
    assert_eq!(read["read_attempts"], 2);
    assert_eq!(read["read_bytes"], 0);
    assert_eq!(read["write_attempts"], 2);
    assert_eq!(read["write_denied_native"], 5);
    pair.excluded()?;
    println!("after_checked_release={read}");
    worker.send(&worker.common("finish"))?;
    let done = worker.receive(STEP)?;
    assert_eq!(done["kind"], "done");
    cleaned(&done, 2);
    assert_eq!(worker.reap()?.code(), Some(0));
    drop(worker);
    pair.released()?;
    println!("cleanup_witness={done}");
    Ok(())
}

#[test]
fn binding_identity_and_protocol_refusals() -> TestResult {
    for case in ["source-binding", "parent-binding", "identity", "protocol"] {
        let mut pair = Pair::new()?;
        let mut worker = Worker::spawn(case, false)?;
        let mut bind = worker.bind(&pair);
        if case == "source-binding" {
            bind["source_sha256"] = json!("0".repeat(64));
        }
        if case == "parent-binding" {
            bind["parent_pid"] = json!(std::process::id() + 1);
        }
        if case == "identity" {
            let id = bind["locks"][1]["index"].as_u64().ok_or("index")?;
            bind["locks"][1]["index"] = json!(id ^ 1);
        }
        worker.send(&bind)?;
        if case == "protocol" {
            duplicated(&worker.receive(START)?, &pair);
            worker.send(&worker.common("finish"))?;
        }
        let refused = worker.receive(START)?;
        assert_eq!(refused["kind"], "refused");
        assert_eq!(
            refused["code"],
            match case {
                "source-binding" => "binding",
                "parent-binding" => "parent_binding",
                "identity" => "identity",
                _ => "protocol_state",
            }
        );
        assert_eq!(refused["read_attempts"], 0);
        cleaned(
            &refused,
            if case == "identity" || case == "protocol" {
                2
            } else {
                0
            },
        );
        assert_eq!(worker.reap()?.code(), Some(40));
        drop(worker);
        pair.release()?;
        pair.released()?;
        println!("refusal_case={case} actual={refused}");
    }
    Ok(())
}

#[test]
fn early_child_eof_has_exact_exit_and_cleanup() -> TestResult {
    let mut pair = Pair::new()?;
    let mut worker = Worker::spawn("early-eof", true)?;
    assert!(matches!(worker.receive(START), Err(ReceiveError::Eof)));
    assert_eq!(worker.reap()?.code(), Some(23));
    drop(worker);
    pair.release()?;
    pair.released()?;
    println!("early_eof_exit=23 owned_child_reaped=true both_roots_released=true");
    Ok(())
}

#[test]
fn bounded_timeout_kills_only_owned_worker_and_releases_duplicates() -> TestResult {
    let mut pair = Pair::new()?;
    let mut worker = Worker::spawn("timeout", false)?;
    worker.send(&worker.bind(&pair))?;
    duplicated(&worker.receive(START)?, &pair);
    // The acknowledged worker is waiting for RELEASED. Withhold it deliberately.
    assert!(matches!(
        worker.receive(Duration::from_millis(150)),
        Err(ReceiveError::Deadline)
    ));
    assert!(worker.child.try_wait()?.is_none());
    worker.child.kill()?;
    assert_eq!(worker.reap()?.code(), Some(1));
    assert!(matches!(worker.receive(STEP), Err(ReceiveError::Eof)));
    drop(worker);
    pair.release()?;
    pair.released()?;
    println!(
        "timeout_phase=await-release exact_deadline=true terminated_owned_exit=1 owned_child_reaped=true both_roots_released=true"
    );
    Ok(())
}
