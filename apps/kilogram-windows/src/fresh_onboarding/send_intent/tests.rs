use super::*;
use crate::fresh_onboarding::send_recovery::{IpcResult, Progress, Recovery, SendRuntime};
use kilogram_runtime_ipc::ordered2::{Delivery2, MessageError2, RequestStatus2, SendReceipt2};

const TEXT: &str = "  CS33A private synthetic sentinel \u{301} \n\0 ";

fn store(home: &Path) -> Store {
    Store {
        home: home.to_owned(),
        binding: Binding {
            setup: "11".repeat(32),
            invitation: "22".repeat(32),
            account: "33".repeat(32),
            device: "44".repeat(32),
            claim: "55".repeat(32),
            peer_account: "66".repeat(32),
            peer_device: "77".repeat(32),
            contexts: ["88".repeat(32), "99".repeat(32)],
        },
        fault: std::cell::Cell::default(),
    }
}

fn check_no_plaintext(dir: &Path) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let contents = fs::read(entry?.path())?;
        for sentinel in [
            b"CS33A private synthetic sentinel".as_slice(),
            TEXT.as_bytes(),
        ] {
            ensure!(
                !contents
                    .windows(sentinel.len())
                    .any(|window| window == sentinel),
                "Synthetic plaintext found in a persisted artifact"
            );
        }
    }
    Ok(())
}

#[test]
fn cs33a_dpapi_binding_caps_and_staging() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let home = fs::canonicalize(tmp.path())?;
    let source = store(&home);
    let request = [12; 16];
    let mut intent = source.prepare(0, request, Zeroizing::new(TEXT.into()))?;
    let dir = source.directory();
    let path = dir.join(FILE);
    let original = fs::read(&path)?;
    let staged = dir.join("stage-123456789ABC.dpapi.tmp");
    fs::write(&staged, &original)?;
    check_no_plaintext(&dir)?;

    for i in 0..8 {
        let mut wrong = store(&home);
        match i {
            0 => wrong.binding.setup = "00".repeat(32),
            1 => wrong.binding.invitation = "00".repeat(32),
            2 => wrong.binding.account = "00".repeat(32),
            3 => wrong.binding.device = "00".repeat(32),
            4 => wrong.binding.claim = "00".repeat(32),
            5 => wrong.binding.peer_account = "00".repeat(32),
            6 => wrong.binding.peer_device = "00".repeat(32),
            _ => wrong.binding.contexts.swap(0, 1),
        }
        ensure!(wrong.load().is_err(), "Wrong protected binding accepted");
        ensure!(
            staged.is_file() && fs::read(&path)? == original,
            "Wrong binding changed evidence"
        );
    }
    let reopened = store(&home).load()?.context("Protected intent absent")?;
    ensure!(
        reopened.request == request
            && reopened.text == TEXT
            && reopened.slot == 0
            && !staged.exists(),
        "DPAPI round-trip or orphan handling"
    );
    ensure!(
        source
            .prepare(1, [13; 16], Zeroizing::new("another draft".into()))
            .is_err(),
        "Second slot clobbered single pending intent"
    );
    intent.slot = 1;
    let clear = Zeroizing::new(serde_json::to_vec(&intent)?);
    fs::write(&path, protect(&clear)?)?;
    ensure!(
        source.load().is_err(),
        "Wrong slot/context binding accepted"
    );
    for invalid in [vec![0x41; CAP + 1], vec![0x21; original.len()]] {
        fs::write(&path, &invalid)?;
        ensure!(source.load().is_err(), "Invalid protected blob accepted");
        ensure!(fs::read(&path)? == invalid, "Invalid evidence overwritten");
    }
    fs::write(&path, &original)?;
    fs::write(&staged, &original)?;
    let unknown = dir.join("unexpected");
    fs::write(&unknown, b"unrelated")?;
    ensure!(source.load().is_err() && staged.exists() && unknown.exists());
    fs::remove_file(&unknown)?;
    source.load()?.context("Missing intact original intent")?;
    fs::remove_file(&path)?;
    fs::write(&staged, &original)?;
    ensure!(
        source.load()?.is_none() && !staged.exists(),
        "Staging alone authorized intent"
    );

    let target = home.join("outside-intent");
    fs::write(&target, &original)?;
    match std::os::windows::fs::symlink_file(&target, &path) {
        Ok(()) => {
            ensure!(source.load().is_err(), "Intent reparse path accepted");
            ensure!(
                fs::read(&target)? == original,
                "Foreign reparse target changed"
            );
            fs::remove_file(&path)?;
            println!("CS33A_REPARSE_REFUSED");
        }
        Err(e) if e.raw_os_error() == Some(1314) => {
            println!("CS33A_REPARSE_FIXTURE_PRIVILEGE_UNAVAILABLE")
        }
        Err(e) => return Err(e.into()),
    }
    let full = "\0".repeat(TEXT_CAP);
    let maximal = source.prepare(1, [14; 16], Zeroizing::new(full.clone()))?;
    ensure!(maximal.text == full && source.load()?.as_ref() == Some(&maximal));
    println!("CS33A_DPAPI_PASS binding_checks=9 stage_recovery=true max_escaped=true");
    Ok(())
}

struct FakeRuntime {
    store: Store,
    trace: Vec<&'static str>,
    status: IpcResult<Option<RequestStatus2>>,
    reply: IpcResult<SendReceipt2>,
    seen: Option<[u8; 16]>,
}
impl SendRuntime for FakeRuntime {
    fn check_send(&mut self, _: u8) -> IpcResult<()> {
        Ok(())
    }
    fn lookup(&mut self, _: u8, _: [u8; 16]) -> IpcResult<Option<RequestStatus2>> {
        self.trace.push("lookup");
        self.status
    }
    fn send(&mut self, slot: u8, request: [u8; 16], text: &str) -> IpcResult<SendReceipt2> {
        self.trace.push("send");
        let saved = self
            .store
            .load()
            .map_err(|_| MessageError2::Unavailable)?
            .ok_or(MessageError2::Unavailable)?;
        if saved.request != request || saved.slot != slot || saved.text != text || text != TEXT {
            return Err(MessageError2::InvalidInput);
        }
        self.seen = Some(request);
        self.reply
    }
}

#[test]
fn cs33a_recovery_publish_retry_and_faults() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let home = fs::canonicalize(tmp.path())?;
    let mut runtime = FakeRuntime {
        store: store(&home),
        trace: Vec::new(),
        status: Ok(None),
        reply: Err(MessageError2::Unavailable),
        seen: None,
    };
    let initial = store(&home);
    initial.fault.set(Some(Mutation::Publish));
    let mut recovery = Recovery::new(Ok(initial));
    let submit =
        |r: &mut Recovery, rt: &mut FakeRuntime| r.submit(rt, 0, Zeroizing::new(TEXT.into()));
    submit(&mut recovery, &mut runtime);
    ensure!(
        runtime.trace == ["lookup"]
            && runtime.store.load()?.is_none()
            && recovery.progress == Progress::StorageBlocked,
        "Failed publication dispatched"
    );
    runtime.trace.clear();
    submit(&mut recovery, &mut runtime);
    let request = runtime.seen.context("Published send did not dispatch")?;
    ensure!(runtime.trace == ["lookup", "send"] && recovery.progress == Progress::Unconfirmed);
    let original = fs::read(runtime.store.directory().join(FILE))?;
    drop(recovery);

    let mut reopened = Recovery::new(Ok(store(&home)));
    runtime.trace.clear();
    reopened.refresh(&mut runtime);
    ensure!(runtime.trace == ["lookup"] && reopened.progress == Progress::RetryAvailable);
    runtime.status = Err(MessageError2::Unavailable);
    reopened.retry(&mut runtime);
    ensure!(runtime.trace == ["lookup", "lookup"] && reopened.progress == Progress::Unconfirmed);
    ensure!(fs::read(runtime.store.directory().join(FILE))? == original);
    runtime.status = Ok(None);
    runtime.trace.clear();
    runtime.seen = None;
    runtime.reply = Ok(SendReceipt2 {
        queue_sequence: 7,
        event: None,
    });
    reopened.retry(&mut runtime);
    ensure!(
        runtime.trace == ["lookup", "send"]
            && runtime.seen == Some(request)
            && reopened.progress == Progress::Queued,
        "Retry failed to recheck or reuse exact saved bytes and ID"
    );
    reopened.retry(&mut runtime);
    ensure!(
        runtime.trace == ["lookup", "send", "lookup"]
            && reopened.progress == Progress::Conflict
            && runtime
                .store
                .load()?
                .is_some_and(|x| x.phase == Phase::Conflict),
        "Accepted-to-Absent dispatched or lost conflict"
    );
    drop(reopened);
    let mut blocked = Recovery::new(Ok(store(&home)));
    runtime.trace.clear();
    blocked.retry(&mut runtime);
    ensure!(runtime.trace.is_empty() && blocked.progress == Progress::Conflict);

    // A different synthetic record covers post-response failures separately.
    let next = tempfile::tempdir()?;
    let home = fs::canonicalize(next.path())?;
    runtime.store = store(&home);
    runtime.status = Ok(None);
    let writes = store(&home);
    writes.fault.set(Some(Mutation::Replace));
    let mut recovery = Recovery::new(Ok(writes));
    submit(&mut recovery, &mut runtime);
    ensure!(recovery.progress == Progress::StorageBlocked);
    ensure!(
        runtime
            .store
            .load()?
            .is_some_and(|x| x.phase == Phase::Prepared)
    );
    drop(recovery);
    let cleanup = store(&home);
    cleanup.fault.set(Some(Mutation::Remove));
    let mut recovery = Recovery::new(Ok(cleanup));
    runtime.status = Ok(Some(RequestStatus2 {
        receipt: SendReceipt2 {
            queue_sequence: 7,
            event: Some([8; 32]),
        },
        delivery: Delivery2::Delivered,
    }));
    runtime.trace.clear();
    recovery.retry(&mut runtime);
    ensure!(runtime.trace == ["lookup"] && recovery.progress == Progress::StorageBlocked);
    ensure!(
        runtime
            .store
            .load()?
            .is_some_and(|x| matches!(x.phase, Phase::Accepted { queue_sequence: 7 }))
    );
    recovery.refresh(&mut runtime);
    ensure!(
        runtime.trace == ["lookup", "lookup"]
            && recovery.progress == Progress::Delivered
            && runtime.store.load()?.is_none(),
        "Positive lookup replayed or cleanup was unsafe"
    );
    println!(
        "CS33A_INTENT_FAULTS_PASS publish_before_dispatch=true fresh_absent_retry=true conflict_persisted=true cleanup_rechecked=true"
    );
    Ok(())
}
