//! One owned local runtime. No adoption of a process found through a descriptor.
use super::{
    artifact, consent, helper, locator_artifact,
    model::{self, Setup},
    send_intent,
    send_recovery::{IpcResult, Projection, Recovery, SendRuntime, Submission},
};
use anyhow::{Context, Result, ensure};
use kilogram_runtime_ipc::{
    RuntimeIpcDescriptor,
    ordered2::{
        Client2, Delivery2, HistoryCursor2, HistoryPage2, MAX_HISTORY_ROWS, MessageError2,
        Profile2Document, ProfileRoute2, ProfileRoutePolicy2, RequestStatus2, SendReceipt2,
        Status2, id,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct State {
    pub version: u8,
    pub profile: Option<String>,
    pub profile_digest: Option<String>,
    pub incoming: Option<String>,
    pub incoming_digest: Option<String>,
    pub imports: [consent::Outcome; 2],
    pub last_outcome: Option<String>,
}
impl State {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.version == 1, "Unsupported connection progress");
        for (name, prefix) in [(&self.profile, "profile-"), (&self.incoming, "peer-")] {
            if let Some(n) = name {
                let id = n
                    .strip_prefix(prefix)
                    .and_then(|n| n.strip_suffix(".json"))
                    .context("Local connection filename")?;
                artifact::canonical_id(id)?;
            }
        }
        for digest in [&self.profile_digest, &self.incoming_digest]
            .into_iter()
            .flatten()
        {
            artifact::canonical_id(digest)?;
        }
        ensure!(
            self.last_outcome.as_deref().is_none_or(|s| matches!(
                s,
                "prepared"
                    | "running"
                    | "graceful-stop"
                    | "forced-stop"
                    | "failed-stop"
                    | "start-failed"
                    | "locator-partial"
                    | "locator-imported"
            )),
            "Connection outcome"
        );
        Ok(())
    }
}
fn change(setup: &mut Setup, f: impl FnOnce(&mut State)) -> Result<()> {
    setup.update(|r| {
        let s = r.connection.get_or_insert_with(|| State {
            version: 1,
            ..Default::default()
        });
        f(s)
    })
}
fn digest(raw: &[u8]) -> String {
    artifact::hex(blake3::hash(raw).as_bytes())
}
fn directory(setup: &Setup) -> Result<PathBuf> {
    let path = setup.home.join("connection");
    fs::create_dir_all(&path)?;
    let canonical = fs::canonicalize(&path)?;
    let info = setup.record.public.as_ref().context("Account")?;
    ensure!(
        canonical == setup.home.join("connection")
            && !canonical.starts_with(fs::canonicalize(&info.state_dir)?)
            && !canonical.starts_with(fs::canonicalize(&info.account_root_dir)?),
        "Connection directory escaped its local location"
    );
    Ok(canonical)
}
fn check_artifact_path(setup: &Setup, path: &Path) -> Result<()> {
    let info = setup.record.public.as_ref().context("Account")?;
    let state = fs::canonicalize(&info.state_dir)?;
    let root = fs::canonicalize(&info.account_root_dir)?;
    let mut at = path;
    loop {
        match fs::symlink_metadata(at) {
            Ok(meta) => {
                ensure!(
                    !meta.file_type().is_symlink(),
                    "Public artifact reparse conflict"
                );
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    ensure!(
                        meta.file_attributes() & 0x400 == 0,
                        "Public artifact reparse conflict"
                    );
                }
                let canonical = fs::canonicalize(at)?;
                ensure!(
                    !canonical.starts_with(&state) && !canonical.starts_with(&root),
                    "Public artifact inside private state"
                );
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        if at == setup.home {
            break;
        }
        at = at.parent().context("Local artifact ancestor")?;
    }
    Ok(())
}
fn expected(setup: &Setup) -> Result<Profile2Document> {
    let info = setup.record.public.as_ref().context("Account")?;
    let base = directory(setup)?;
    Ok(Profile2Document {
        version: 2,
        profile_mode: "ordered-dialog-v2".into(),
        state_dir: info.state_dir.clone(),
        account: info.account_id.to_string(),
        claim: info.claim.clone(),
        device: info.device_id.to_string(),
        contexts: setup.record.contexts.clone(),
        enabled: vec![0, 1],
        ipc_file: base.join("ipc.json"),
        locator_directory: base.join("locators"),
        route: ProfileRoute2 {
            policy: ProfileRoutePolicy2::DirectOnly,
            bind: Some("127.0.0.1:0".parse()?),
            relay: None,
        },
        logical_quota: Some([16 * 1024 * 1024 * 1024; 2]),
        physical_quota: Some(48 * 1024 * 1024 * 1024),
    })
}
pub(super) fn prepare(
    setup: &mut Setup,
    exe: &Path,
    cancel: Arc<AtomicBool>,
    fresh_profile: bool,
) -> Result<(PathBuf, Profile2Document)> {
    consent::execute(setup, exe, consent::Action::Import, cancel.clone())?;
    let (nb, end) = consent::period(setup)?;
    let wall = consent::now()?;
    ensure!(
        nb <= wall && wall < end,
        "Permission is not currently valid"
    );
    let doc = expected(setup)?;
    check_artifact_path(setup, &doc.ipc_file)?;
    check_artifact_path(setup, &doc.locator_directory)?;
    if fresh_profile
        || setup
            .record
            .connection
            .as_ref()
            .and_then(|s| s.profile.as_ref())
            .is_none()
    {
        let name = format!("profile-{}.json", artifact::id()?);
        change(setup, |s| {
            s.profile = Some(name);
            s.profile_digest = None;
        })?;
    }
    let state = setup.record.connection.as_ref().context("Connection")?;
    let path = directory(setup)?.join(state.profile.as_ref().context("Profile")?);
    check_artifact_path(setup, &path)?;
    if !path.exists() {
        let args: Vec<std::ffi::OsString> = vec![
            "runtime-profile-create".into(),
            "--profile-mode".into(),
            "ordered-dialog-v2".into(),
            "--state-dir".into(),
            doc.state_dir.clone().into_os_string(),
            "--profile-file".into(),
            path.clone().into_os_string(),
            "--expect-account".into(),
            doc.account.clone().into(),
            "--expect-claim".into(),
            doc.claim.clone().into(),
            "--expect-device".into(),
            doc.device.clone().into(),
            "--ordered-context".into(),
            doc.contexts[0].clone().into(),
            "--ordered-context".into(),
            doc.contexts[1].clone().into(),
            "--ipc-file".into(),
            doc.ipc_file.clone().into_os_string(),
            "--locator-directory".into(),
            doc.locator_directory.clone().into_os_string(),
            "--route-policy".into(),
            "direct-only".into(),
            "--ordered-bind".into(),
            "127.0.0.1:0".into(),
        ];
        // A lost acknowledgement may leave a complete no-clobber output.
        let _outcome = helper::public_command(exe, &args, cancel.clone());
    }
    ensure!(!cancel.load(Ordering::Acquire), "Connection cancelled");
    let raw = model::read(&path, kilogram_runtime_ipc::ordered2::PROFILE_CAP)?;
    ensure!(
        Profile2Document::decode(&raw)? == doc,
        "Profile conflict; keep the file and explicitly retry using a new local profile"
    );
    let hash = digest(&raw);
    ensure!(
        setup
            .record
            .connection
            .as_ref()
            .and_then(|s| s.profile_digest.as_ref())
            .is_none_or(|old| *old == hash),
        "Recorded profile changed"
    );
    change(setup, |s| {
        s.profile_digest = Some(hash);
        s.last_outcome = Some("prepared".into());
    })?;
    Ok((path, doc))
}

struct Drain {
    buffer: Arc<Mutex<std::collections::VecDeque<u8>>>,
    eof: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<()>>,
}
impl Drain {
    fn start(mut pipe: impl Read + Send + 'static) -> Result<Self> {
        let eof = Arc::new(AtomicBool::new(false));
        let failed = Arc::new(AtomicBool::new(false));
        let buffer = Arc::new(Mutex::new(std::collections::VecDeque::with_capacity(65536)));
        let retained = buffer.clone();
        let done = eof.clone();
        let error = failed.clone();
        let join = thread::Builder::new()
            .name("owned-runtime-drain".into())
            .spawn(move || {
                let mut chunk = [0; 4096];
                loop {
                    match pipe.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => {
                            let Ok(mut ring) = retained.lock() else {
                                error.store(true, Ordering::Release);
                                break;
                            };
                            while ring.len() + n > 65536 {
                                ring.pop_front();
                            }
                            ring.extend(&chunk[..n]);
                        }
                        Err(_) => {
                            error.store(true, Ordering::Release);
                            break;
                        }
                    }
                }
                // Never return raw runtime errors, tokens or local claim to UI/logs.
                chunk.fill(0);
                done.store(true, Ordering::Release);
            })?;
        Ok(Self {
            buffer,
            eof,
            failed,
            join: Some(join),
        })
    }
    fn join(&mut self) {
        if self.eof.load(Ordering::Acquire)
            && let Some(j) = self.join.take()
        {
            let _ = j.join();
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct DialogScope {
    pub instance: [u8; 32],
    pub contexts: [[u8; 32]; 2],
    pub own_role: u8,
}

#[derive(Clone, Default)]
pub(super) struct Messages {
    pub scope: Option<DialogScope>,
    pub send_slots: [bool; 2],
    pub pending: Projection,
}

#[derive(Clone)]
pub(super) struct View {
    pub text: String,
    pub generation: u64,
    pub request_started: Instant,
    pub connected: [bool; 2],
    pub lease_expires: [u64; 2],
    pub consent_expires: u64,
    pub running: bool,
    pub messages: Messages,
}
impl View {
    pub fn current(&self, now: Instant, wall: u64) -> bool {
        self.running
            && now.saturating_duration_since(self.request_started) < Duration::from_secs(2)
            && self.connected.iter().all(|v| *v)
            && wall < self.consent_expires
            && self.lease_expires.iter().all(|end| wall < *end)
    }
    fn waiting(text: &str) -> Self {
        Self {
            text: text.into(),
            generation: 0,
            request_started: Instant::now(),
            connected: [false; 2],
            lease_expires: [0; 2],
            consent_expires: 0,
            running: false,
            messages: Messages::default(),
        }
    }
}
#[derive(Clone, Debug)]
pub(super) struct Stop {
    pub graceful: bool,
    pub forced: bool,
    pub reaped: bool,
    pub exit: Option<i32>,
    pub eof: bool,
}
pub(super) struct Session {
    child: Child,
    drains: Vec<Drain>,
    client: Option<Client2>,
    rt: tokio::runtime::Runtime,
    pub instance: Option<String>,
    doc: Profile2Document,
    pub last_status: Option<Status2>,
    consent_refs: [String; 2],
    consent_interval: (u64, u64),
    role: u8,
}
fn wait_owned(child: &mut Child, limit: Duration) -> Result<Option<std::process::ExitStatus>> {
    let end = Instant::now() + limit;
    loop {
        if let Some(s) = child.try_wait()? {
            return Ok(Some(s));
        }
        if Instant::now() >= end {
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(20));
    }
}

#[derive(Default)]
struct Cleanup {
    stopping: AtomicBool,
    unconfirmed: AtomicBool,
    #[cfg(test)]
    reveal_exit: Option<Arc<AtomicBool>>,
}
impl Cleanup {
    fn stopping(&self) -> bool {
        self.stopping.load(Ordering::Acquire)
    }

    fn blocked(&self) -> bool {
        self.unconfirmed.load(Ordering::Acquire)
    }

    fn wait(&self, child: &mut Child, limit: Duration) -> Result<Option<std::process::ExitStatus>> {
        #[cfg(test)]
        if let Some(reveal) = &self.reveal_exit {
            return if reveal.load(Ordering::Acquire) {
                Ok(child.try_wait()?)
            } else {
                Ok(None)
            };
        }
        wait_owned(child, limit)
    }

    fn observe_exit(&self, child: &mut Child) -> std::process::ExitStatus {
        self.unconfirmed.store(true, Ordering::Release);
        loop {
            if let Ok(Some(exit)) = self.wait(child, Duration::ZERO) {
                return exit;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}
fn cancellable<T>(
    rt: &tokio::runtime::Runtime,
    cancel: &AtomicBool,
    f: impl std::future::Future<Output = Result<T>>,
) -> Result<T> {
    rt.block_on(async{tokio::pin!(f);loop{tokio::select!{v=&mut f=>return v,_=tokio::time::sleep(Duration::from_millis(20))=>ensure!(!cancel.load(Ordering::Acquire),"Connection operation cancelled")}}})
}
impl Session {
    fn start(
        setup: &mut Setup,
        exe: &Path,
        cancel: Arc<AtomicBool>,
        fresh_profile: bool,
        cleanup: &Cleanup,
    ) -> Result<Self> {
        let (path, doc) = prepare(setup, exe, cancel.clone(), fresh_profile)?;
        // Immutable bindings, not an authority cache. Runtime checks current
        // consent/authority and the live lease on every status observation.
        let consent_refs = consent::connection_refs(setup)?;
        let consent_interval = consent::period(setup)?;
        let role = consent::role(setup)? as u8;
        let before = if doc.ipc_file.exists() {
            let d = RuntimeIpcDescriptor::load(&doc.ipc_file)?;
            let info = setup.record.public.as_ref().context("Account")?;
            ensure!(
                d.account_id() == info.account_id && d.device_id() == info.device_id,
                "Existing descriptor identity conflict"
            );
            Some(d)
        } else {
            None
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let mut command = Command::new(exe);
        command
            .args(["runtime-from-profile", "--profile-file"])
            .arg(&path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_4000);
        }
        let child = command.spawn()?;

        let mut session = Self {
            child,
            drains: Vec::with_capacity(2),
            client: None,
            rt,
            instance: None,
            doc,
            last_status: None,
            consent_refs,
            consent_interval,
            role,
        };
        let startup = (|| {
            session.drains.push(Drain::start(
                session.child.stdout.take().context("Runtime stdout")?,
            )?);
            session.drains.push(Drain::start(
                session.child.stderr.take().context("Runtime stderr")?,
            )?);
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                session.check_drains()?;
                ensure!(!cancel.load(Ordering::Acquire), "Connection cancelled");
                ensure!(
                    session.child.try_wait()?.is_none(),
                    "Owned runtime exited during startup"
                );
                ensure!(Instant::now() < deadline, "Owned runtime startup timed out");
                if let Ok(d) = RuntimeIpcDescriptor::load(&session.doc.ipc_file)
                    && before.as_ref() != Some(&d)
                {
                    let info = setup.record.public.as_ref().context("Account")?;
                    let candidate = Client2::new(
                        d,
                        info.account_id,
                        info.device_id,
                        [
                            kilogram_runtime_ipc::ordered2::id(&session.doc.contexts[0])?,
                            kilogram_runtime_ipc::ordered2::id(&session.doc.contexts[1])?,
                        ],
                    )?;
                    let left = deadline.saturating_duration_since(Instant::now());
                    let status = cancellable(&session.rt, &cancel, async {
                        tokio::time::timeout(left, candidate.status())
                            .await
                            .map_err(|_| anyhow::anyhow!("startup timed out"))?
                    })?;
                    session.validate_status(&status)?;
                    session.instance = Some(status.runtime_instance.clone());
                    session.last_status = Some(status);
                    session.client = Some(candidate);
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
            change(setup, |s| s.last_outcome = Some("running".into()))?;
            Ok(())
        })();
        if let Err(e) = startup {
            let stop = session.finish(cleanup);
            let _ = change(setup, |s| s.last_outcome = Some("start-failed".into()));
            let detail = if cleanup.blocked() {
                Some("forced cleanup was unconfirmed until the owned runtime exited")
            } else if stop.forced {
                Some("the owned runtime required forced cleanup")
            } else {
                None
            };
            return Err(match detail {
                Some(detail) => {
                    let context = format!("{e}; {detail}");
                    e.context(context)
                }
                None => e,
            });
        }
        Ok(session)
    }
    fn validate_status(&mut self, status: &Status2) -> Result<()> {
        ensure!(
            self.child.try_wait()?.is_none()
                && status.runtime_pid == self.child.id()
                && self
                    .instance
                    .as_ref()
                    .is_none_or(|i| *i == status.runtime_instance),
            "Status is not from the live owned run"
        );
        ensure!(
            status.route == self.doc.route && status.endpoint.is_some(),
            "Runtime route/endpoint mismatch"
        );
        for (slot, reference) in self.consent_refs.iter().enumerate() {
            let c = status.installed(slot)?;
            ensure!(
                c.context == self.doc.contexts[slot]
                    && c.role == self.role
                    && c.details.current_consent.as_ref() == Some(reference),
                "Runtime context/role/consent mismatch"
            );
        }
        Ok(())
    }
    fn check_drains(&self) -> Result<()> {
        ensure!(
            self.drains
                .iter()
                .all(|d| !d.failed.load(Ordering::Acquire)),
            "Runtime output reader failed"
        );
        Ok(())
    }
    pub fn poll(&mut self, cancel: &AtomicBool) -> Result<View> {
        let started = Instant::now();
        self.check_drains()?;
        let client = self.client.as_ref().context("Owned client")?;
        ensure!(
            RuntimeIpcDescriptor::load(&self.doc.ipc_file)
                .as_ref()
                .is_ok_and(|d| d == client.descriptor()),
            "Runtime descriptor replaced; pinned client retained"
        );
        let status = cancellable(&self.rt, cancel, client.status())?;
        self.validate_status(&status)?;
        let (_, end) = self.consent_interval;
        let mut view = View::waiting(
            "Runtime running; exchange current public locators. Waiting for authenticated connection.",
        );
        view.running = true;
        view.request_started = started;
        view.consent_expires = end;
        let wall = consent::now()?;
        for slot in 0..2 {
            let c = status.installed(slot)?;
            view.messages.send_slots[slot] = self.doc.enabled.contains(&(slot as u8))
                && c.phase == 3
                && c.blocked.is_none()
                && c.policy_error.is_none()
                && (self.consent_interval.0..end).contains(&wall);
            view.connected[slot] = c.phase == 3
                && c.active
                && c.live_admitted
                && c.blocked.is_none()
                && c.policy_error.is_none();
            view.lease_expires[slot] = c.lease_expires.unwrap_or(0);
        }
        if view.current(Instant::now(), consent::now()?) {
            view.text =
                "Last checked runtime observation: both slots reached authenticated Active2".into();
        }
        self.last_status = Some(status);
        Ok(view)
    }
    pub fn export(&mut self, setup: &Setup, cancel: &AtomicBool) -> Result<PathBuf> {
        let _ = self.poll(cancel)?;
        let client = self.client.as_ref().context("Owned client")?;
        let first = cancellable(&self.rt, cancel, client.export_locator(0))?;
        let second = cancellable(&self.rt, cancel, client.export_locator(1))?;
        let envelope = locator_artifact::Envelope::create(setup, [first, second])?;
        let blobs = envelope.validate(setup, false)?;
        let _ = self.poll(cancel)?;
        let status = self.last_status.as_ref().context("Status")?;
        let endpoint = &status.endpoint.as_ref().context("Endpoint")?.id;
        for (slot, raw) in blobs.iter().enumerate() {
            let loc = kilogram_protocol::ordered_dialog_v2::ParsedRuntimeLocator2::parse(raw)?;
            ensure!(
                artifact::hex(&loc.fields().endpoint) == *endpoint
                    && loc.fields().revision == status.installed(slot)?.own_locator_revision,
                "Own locators changed; export again"
            );
        }
        let raw = envelope.encode()?;
        let path = directory(setup)?.join(format!("public-{}.json", artifact::id()?));
        model::write(&path, &raw, false)?;
        Ok(path)
    }
    pub fn import(&mut self, setup: &mut Setup, path: &Path, cancel: &AtomicBool) -> Result<()> {
        self.import_with(setup, path, cancel, |_| Ok(()))
    }
    fn import_with(
        &mut self,
        setup: &mut Setup,
        path: &Path,
        cancel: &AtomicBool,
        mut after: impl FnMut(usize) -> Result<()>,
    ) -> Result<()> {
        let raw = model::read(path, locator_artifact::CAP)?;
        let envelope = locator_artifact::Envelope::decode(&raw)?;
        let blobs = envelope.validate(setup, true)?;
        let _ = self.poll(cancel)?;
        let status = self.last_status.as_ref().context("Status")?;
        for (slot, blob) in blobs.iter().enumerate() {
            let loc = kilogram_protocol::ordered_dialog_v2::ParsedRuntimeLocator2::parse(blob)?;
            ensure!(
                loc.fields().revision >= status.installed(slot)?.peer_locator_revision,
                "Peer locator rollback"
            );
        }
        let hash = digest(&raw);
        let name = format!("peer-{hash}.json");
        let retained = directory(setup)?.join(&name);
        if retained.exists() {
            ensure!(
                model::read(&retained, locator_artifact::CAP)? == raw,
                "Locator input conflict"
            );
        } else {
            model::write(&retained, &raw, false)?;
        }
        let same = setup
            .record
            .connection
            .as_ref()
            .is_some_and(|s| s.incoming_digest.as_ref() == Some(&hash));
        change(setup, |s| {
            if !same {
                s.imports = [consent::Outcome::NotStarted; 2];
            }
            s.incoming = Some(name);
            s.incoming_digest = Some(hash);
        })?;
        for (slot, blob) in blobs.iter().enumerate() {
            envelope.validate(setup, true)?;
            change(setup, |s| s.imports[slot] = consent::Outcome::Started)?;
            let client = self.client.as_ref().context("Owned client")?;
            let result = cancellable(&self.rt, cancel, client.import_locator(slot as u8, blob));
            if result.is_err() {
                change(setup, |s| {
                    s.imports[slot] = consent::Outcome::Failed;
                    s.last_outcome = Some("locator-partial".into());
                })?;
                anyhow::bail!(
                    "Local locator import unconfirmed; retry the same retained file. Existing accepted slots were kept."
                );
            }
            after(slot)?;
            change(setup, |s| s.imports[slot] = consent::Outcome::Accepted)?;
        }
        change(setup, |s| s.last_outcome = Some("locator-imported".into()))?;
        Ok(())
    }
    fn finish(&mut self, cleanup: &Cleanup) -> Stop {
        cleanup.stopping.store(true, Ordering::Release);
        let mut ack = false;
        let mut conflict = false;
        if matches!(self.child.try_wait(), Ok(None))
            && let Some(client) = &self.client
        {
            conflict = !RuntimeIpcDescriptor::load(&self.doc.ipc_file)
                .as_ref()
                .is_ok_and(|d| d == client.descriptor());
            // Never reload the replacement and never address its server.
            ack = self.rt.block_on(client.shutdown()).is_ok();
        }
        self.client = None;
        let exit = cleanup
            .wait(&mut self.child, Duration::from_secs(10))
            .ok()
            .flatten();
        let forced = exit.is_none();
        let exit = exit.unwrap_or_else(|| {
            let _ = self.child.kill();
            cleanup
                .wait(&mut self.child, Duration::from_secs(5))
                .ok()
                .flatten()
                .unwrap_or_else(|| cleanup.observe_exit(&mut self.child))
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.drains.iter().any(|d| !d.eof.load(Ordering::Acquire))
            && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(20));
        }
        let eof = self.drains.len() == 2
            && self
                .drains
                .iter()
                .all(|d| d.eof.load(Ordering::Acquire) && !d.failed.load(Ordering::Acquire));
        for d in &mut self.drains {
            d.join();
        }
        Stop {
            graceful: ack && !conflict && !forced && exit.success() && eof,
            forced,
            reaped: true,
            exit: exit.code(),
            eof,
        }
    }
}
impl Drop for Drain {
    fn drop(&mut self) {
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.make_contiguous().fill(0);
        }
        self.join();
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
            let _ = Cleanup::default().observe_exit(&mut self.child);
        }
        for d in &mut self.drains {
            d.join();
        }
    }
}

pub(super) enum Action {
    Export,
    Import(PathBuf),
    Resume,
    Stop,
    Send {
        slot: u8,
        text: Zeroizing<String>,
        reply: mpsc::SyncSender<Submission>,
    },
    RetrySend,
    History(Box<HistoryJob>),
}

pub(super) struct HistoryJob {
    pub scope: DialogScope,
    pub slot: u8,
    pub cursor: Option<HistoryCursor2>,
    pub reply: mpsc::SyncSender<IpcResult<HistoryPage2>>,
}

pub(super) struct DeliveryJob {
    pub scope: DialogScope,
    pub slot: u8,
    pub request: [u8; 16],
    pub event: [u8; 32],
    pub reply: mpsc::SyncSender<Option<Delivery2>>,
}

struct Inbox {
    actions: mpsc::Receiver<(u64, Action)>,
    deliveries: mpsc::Receiver<DeliveryJob>,
}

struct SessionSend<'a> {
    session: &'a mut Session,
    cancel: &'a AtomicBool,
}

impl SessionSend<'_> {
    fn history(&mut self, slot: u8, cursor: Option<&HistoryCursor2>) -> IpcResult<HistoryPage2> {
        let context = self.context(slot)?;
        let client = self
            .session
            .client
            .as_ref()
            .ok_or(MessageError2::Unavailable)?;
        let response = cancellable(&self.session.rt, self.cancel, async {
            Ok(client
                .history_page(slot, context, cursor, MAX_HISTORY_ROWS)
                .await)
        })
        .map_err(|_| MessageError2::Unavailable)?;
        self.context(slot)?;
        response
    }

    fn context(&mut self, slot: u8) -> IpcResult<[u8; 32]> {
        let unavailable = MessageError2::Unavailable;
        if self.cancel.load(Ordering::Acquire)
            || self.session.check_drains().is_err()
            || self.session.instance.is_none()
            || !matches!(self.session.child.try_wait(), Ok(None))
        {
            return Err(unavailable);
        }
        let client = self.session.client.as_ref().ok_or(unavailable)?;
        if !RuntimeIpcDescriptor::load(&self.session.doc.ipc_file)
            .is_ok_and(|descriptor| &descriptor == client.descriptor())
        {
            return Err(unavailable);
        }
        id(self
            .session
            .doc
            .contexts
            .get(usize::from(slot))
            .ok_or(MessageError2::InvalidInput)?)
        .map_err(|_| MessageError2::InvalidInput)
    }

    fn authorize(&mut self, slot: u8) -> IpcResult<Instant> {
        self.context(slot)?;
        let view = self
            .session
            .poll(self.cancel)
            .map_err(|_| MessageError2::Unavailable)?;
        let context = self
            .session
            .last_status
            .as_ref()
            .and_then(|status| status.installed(usize::from(slot)).ok())
            .ok_or(MessageError2::Unavailable)?;
        let wall = consent::now().map_err(|_| MessageError2::Unavailable)?;
        let (start, end) = self.session.consent_interval;
        if !self.session.doc.enabled.contains(&slot)
            || context.phase != 3
            || context.policy_error.is_some()
            || context.blocked.is_some()
            || !(start..end).contains(&wall)
            || view.request_started.elapsed() >= Duration::from_secs(2)
        {
            return Err(MessageError2::Unavailable);
        }
        Ok(view.request_started)
    }
}

impl SendRuntime for SessionSend<'_> {
    fn check_send(&mut self, slot: u8) -> IpcResult<()> {
        self.authorize(slot).map(|_| ())
    }

    fn lookup(&mut self, slot: u8, request: [u8; 16]) -> IpcResult<Option<RequestStatus2>> {
        let context = self.context(slot)?;
        let client = self
            .session
            .client
            .as_ref()
            .ok_or(MessageError2::Unavailable)?;
        let response = cancellable(&self.session.rt, self.cancel, async {
            Ok(client.request_status(slot, context, request).await)
        })
        .map_err(|_| MessageError2::Unavailable)?;
        self.context(slot)?;
        response
    }

    fn send(&mut self, slot: u8, request: [u8; 16], text: &str) -> IpcResult<SendReceipt2> {
        let started = self.authorize(slot)?;
        let context = self.context(slot)?;
        if self.cancel.load(Ordering::Acquire) || started.elapsed() >= Duration::from_secs(2) {
            return Err(MessageError2::Unavailable);
        }
        let client = self
            .session
            .client
            .as_ref()
            .ok_or(MessageError2::Unavailable)?;
        let response = cancellable(&self.session.rt, self.cancel, async {
            Ok(client.send_text(slot, context, request, text).await)
        })
        .map_err(|_| MessageError2::Unavailable)?;
        self.context(slot)?;
        response
    }
}

fn run_worker(
    setup: &mut Setup,
    exe: &Path,
    signal: Arc<AtomicBool>,
    fresh_profile: bool,
    inbox: Inbox,
    views: mpsc::SyncSender<View>,
    cleanup: &Cleanup,
) -> Result<Stop> {
    let mut session = Session::start(setup, exe, signal.clone(), fresh_profile, cleanup)?;
    let running = run_connected(&mut session, setup, &signal, inbox, views);
    let stop = session.finish(cleanup);
    let saved = change(setup, |s| {
        s.last_outcome = Some(
            if stop.forced {
                "forced-stop"
            } else if stop.graceful && running.is_ok() {
                "graceful-stop"
            } else {
                "failed-stop"
            }
            .into(),
        )
    });
    running?;
    saved?;
    Ok(stop)
}

fn run_connected(
    session: &mut Session,
    setup: &mut Setup,
    signal: &AtomicBool,
    inbox: Inbox,
    views: mpsc::SyncSender<View>,
) -> Result<()> {
    let mut next = Instant::now();
    let mut next_pending = Instant::now();
    let mut recovery = Recovery::new(send_intent::Store::new(setup, &session.doc));
    let scope = DialogScope {
        instance: id(session
            .instance
            .as_deref()
            .context("Owned runtime instance")?)?,
        contexts: [id(&session.doc.contexts[0])?, id(&session.doc.contexts[1])?],
        own_role: session.role,
    };
    let mut last_notice: Option<String> = None;
    let mut handled = 0;
    loop {
        session.check_drains()?;
        if signal.load(Ordering::Acquire) {
            break;
        }
        let action_pending = match inbox.actions.try_recv() {
            Ok(action) => Some(action),
            Err(mpsc::TryRecvError::Disconnected) => break,
            Err(mpsc::TryRecvError::Empty) => None,
        };
        let idle = action_pending.is_none();
        match action_pending {
            Some((_, Action::Stop)) => break,
            Some((generation, Action::Export)) => {
                handled = generation;
                last_notice = Some(match session.export(setup, signal) {
                    Ok(path) => format!("Public locators saved: {}", path.display()),
                    Err(_) => "Locator export failed; current files were preserved.".into(),
                });
            }
            Some((generation, Action::Import(path))) => {
                handled = generation;
                let result = session.import(setup, &path, signal);
                last_notice = Some(
                    if result.is_ok() {
                        "Both locator imports were acknowledged."
                    } else {
                        "Locator import unconfirmed/partial; existing progress was preserved. Retry the same file."
                    }
                    .into(),
                );
            }
            Some((generation, Action::Resume)) => {
                handled = generation;
                let result = (|| {
                    let state = setup.record.connection.as_ref().context("Connection")?;
                    let name = state
                        .incoming
                        .as_ref()
                        .context("No retained locator file")?;
                    let path = directory(setup)?.join(name);
                    let raw = model::read(&path, locator_artifact::CAP)?;
                    ensure!(
                        state.incoming_digest.as_ref() == Some(&digest(&raw)),
                        "Retained locator changed"
                    );
                    session.import(setup, &path, signal)
                })();
                last_notice = Some(
                    if result.is_ok() {
                        "Retained locator imports acknowledged."
                    } else {
                        "Retained locator retry failed; existing progress was kept."
                    }
                    .into(),
                );
            }
            Some((generation, Action::Send { slot, text, reply })) => {
                handled = generation;
                let result = recovery.submit(
                    &mut SessionSend {
                        session,
                        cancel: signal,
                    },
                    slot,
                    text,
                );
                let _ = reply.try_send(result);
                next_pending = Instant::now() + Duration::from_secs(1);
            }
            Some((generation, Action::RetrySend)) => {
                handled = generation;
                recovery.retry(&mut SessionSend {
                    session,
                    cancel: signal,
                });
                next_pending = Instant::now() + Duration::from_secs(1);
            }
            Some((generation, Action::History(job))) => {
                handled = generation;
                let result = if job.scope == scope {
                    SessionSend {
                        session,
                        cancel: signal,
                    }
                    .history(job.slot, job.cursor.as_ref())
                } else {
                    Err(MessageError2::InvalidInput)
                };
                let _ = job.reply.try_send(result);
            }
            None => {}
        }
        if signal.load(Ordering::Acquire) {
            break;
        }
        if Instant::now() >= next {
            let mut view=session.poll(signal).unwrap_or_else(|_|View::waiting("Current runtime status unavailable or descriptor changed; connection is not confirmed."));
            view.generation = handled;
            if let Some(notice) = &last_notice {
                view.text = format!("{notice}\n{}", view.text);
            }
            view.messages.scope = Some(scope);
            view.messages.pending = recovery.projection();
            if let Some(state) = &setup.record.connection {
                view.text = format!(
                    "{}\nLocal locator import outcomes: slot0={:?}; slot1={:?}. These alone do not prove connectivity.",
                    view.text, state.imports[0], state.imports[1]
                );
            }
            let _ = views.try_send(view);
            next = Instant::now() + Duration::from_secs(1);
            if session.child.try_wait()?.is_some() {
                break;
            }
        }
        if idle && Instant::now() >= next_pending && !signal.load(Ordering::Acquire) {
            recovery.refresh(&mut SessionSend {
                session,
                cancel: signal,
            });
            next_pending = Instant::now() + Duration::from_secs(1);
        } else if idle
            && !signal.load(Ordering::Acquire)
            && let Ok(job) = inbox.deliveries.try_recv()
        {
            let delivery = if job.scope == scope {
                SessionSend {
                    session,
                    cancel: signal,
                }
                .lookup(job.slot, job.request)
                .ok()
                .flatten()
                .filter(|status| status.receipt.event == Some(job.event))
                .map(|status| status.delivery)
            } else {
                None
            };
            let _ = job.reply.try_send(delivery);
        }
        thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}
pub(super) struct Work {
    cancel: Arc<AtomicBool>,
    epoch: Arc<AtomicU64>,
    actions: mpsc::SyncSender<(u64, Action)>,
    deliveries: mpsc::SyncSender<DeliveryJob>,
    views: mpsc::Receiver<View>,
    done: mpsc::Receiver<(Setup, Result<Stop>)>,
    cleanup: Arc<Cleanup>,
    pub view: View,
}
impl Work {
    pub fn start(setup: Setup, exe: PathBuf, fresh_profile: bool) -> Result<Self> {
        Self::start_with_cleanup(setup, exe, fresh_profile, Arc::new(Cleanup::default()))
    }

    fn start_with_cleanup(
        mut setup: Setup,
        exe: PathBuf,
        fresh_profile: bool,
        cleanup: Arc<Cleanup>,
    ) -> Result<Self> {
        let cancel = Arc::new(AtomicBool::new(false));
        let signal = cancel.clone();
        let epoch = Arc::new(AtomicU64::new(0));
        let (tx, rx) = mpsc::sync_channel(1);
        let (delivery_tx, delivery_rx) = mpsc::sync_channel(1);
        let (vtx, vrx) = mpsc::sync_channel(1);
        let (dtx, drx) = mpsc::sync_channel(1);
        let owner_cleanup = cleanup.clone();
        thread::Builder::new()
            .name("fresh-owned-runtime".into())
            .spawn(move || {
                let result = run_worker(
                    &mut setup,
                    &exe,
                    signal,
                    fresh_profile,
                    Inbox {
                        actions: rx,
                        deliveries: delivery_rx,
                    },
                    vtx,
                    &owner_cleanup,
                );
                let _ = dtx.send((setup, result));
            })?;
        Ok(Self {
            cancel,
            epoch,
            actions: tx,
            deliveries: delivery_tx,
            views: vrx,
            done: drx,
            cleanup,
            view: View::waiting(
                "Checking both local permissions and starting the owned runtime...",
            ),
        })
    }
    pub fn action(&mut self, action: Action) -> Result<()> {
        ensure!(!self.cleanup.stopping(), "Owned runtime is stopping");
        let previous = self.epoch.fetch_add(1, Ordering::AcqRel);
        if self.actions.try_send((previous + 1, action)).is_err() {
            self.epoch.store(previous, Ordering::Release);
            anyhow::bail!("A runtime operation is already pending");
        }
        self.view = View::waiting("Processing local runtime operation...");
        Ok(())
    }
    pub fn check_delivery(&self, job: DeliveryJob) -> bool {
        !self.cleanup.stopping()
            && !self.cancel.load(Ordering::Acquire)
            && self.deliveries.try_send(job).is_ok()
    }
    pub fn cancel(&mut self) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
        let _ = self
            .actions
            .try_send((self.epoch.load(Ordering::Acquire), Action::Stop));
        self.view = View::waiting("Stopping owned runtime...");
        self.cancel.store(true, Ordering::Release);
    }
    pub fn poll(&mut self) -> Option<(Setup, Result<Stop>)> {
        while let Ok(v) = self.views.try_recv() {
            if !self.cancel.load(Ordering::Acquire)
                && v.generation == self.epoch.load(Ordering::Acquire)
            {
                self.view = v;
            }
        }
        if self.cleanup.blocked() {
            self.view =
                View::waiting("Owned runtime cleanup unconfirmed; account actions remain blocked.");
        } else if self.cleanup.stopping() {
            self.view = View::waiting("Stopping owned runtime...");
        }
        self.done.try_recv().ok()
    }
}
impl Drop for Work {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

#[cfg(test)]
#[path = "connection_tests.rs"]
mod tests;
