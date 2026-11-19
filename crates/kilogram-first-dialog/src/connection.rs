//! One owned local runtime. No adoption of a process found through a descriptor.
use super::{
    artifact, consent, helper, locator_artifact,
    model::{self, Record, Setup},
    operation::{self, OperationControl},
    send_intent,
    send_recovery::{
        IpcResult, Projection, Recovery, RequestOperation, RequestResult, SendRuntime, Submission,
    },
};
use anyhow::{Context, Result, ensure};
use kilogram_runtime_ipc::lab_diagnostics::{Kind as DiagnosticKind, Sink as DiagnosticSink};
use kilogram_runtime_ipc::{
    RuntimeIpcDescriptor,
    ordered2::{
        Client2, Delivery2, DiscoveryStatus2, HistoryCursor2, HistoryPage2, HistoryPosition2,
        LanDiscovery2, MessageError2, Profile2Document, ProfileRoute2, ProfileRoutePolicy2,
        RequestStatus2, SendReceipt2, Status2, id,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    net::{Ipv4Addr, SocketAddr},
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanSettings {
    pub local: Ipv4Addr,
    pub peer: Ipv4Addr,
}
impl LanSettings {
    pub fn validate(self) -> Result<()> {
        kilogram_protocol::ordered_dialog_v2::DirectLan2::new(self.local, self.peer)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub version: u8,
    pub profile: Option<String>,
    pub profile_digest: Option<String>,
    pub incoming: Option<String>,
    pub incoming_digest: Option<String>,
    pub imports: [consent::Outcome; 2],
    pub last_outcome: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lan: Option<LanSettings>,
    #[serde(default, skip_serializing_if = "LanDiscovery2::is_manual")]
    pub lan_discovery: LanDiscovery2,
}
impl State {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.version == 1, "Unsupported connection progress");
        if let Some(lan) = self.lan {
            lan.validate()?;
        }
        self.lan_discovery.validate(
            self.lan
                .map(|lan| {
                    kilogram_protocol::ordered_dialog_v2::DirectLan2::new(lan.local, lan.peer)
                })
                .transpose()?,
        )?;
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
pub fn lan_settings(setup: &Setup) -> Option<LanSettings> {
    setup.record.connection.as_ref().and_then(|state| state.lan)
}
pub fn lan_discovery(setup: &Setup) -> LanDiscovery2 {
    setup
        .record
        .connection
        .as_ref()
        .map(|state| state.lan_discovery.clone())
        .unwrap_or_default()
}
#[derive(Debug)]
pub struct ConnectionPathError;

impl std::fmt::Display for ConnectionPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Unexpected local connection path")
    }
}

impl std::error::Error for ConnectionPathError {}

pub fn select_route(setup: &mut Setup, lan: Option<LanSettings>) -> Result<()> {
    let discovery = if lan_settings(setup) == lan {
        lan_discovery(setup)
    } else {
        LanDiscovery2::Manual {}
    };
    select_lan_discovery(setup, lan, discovery)
}

pub fn select_lan_discovery(
    setup: &mut Setup,
    lan: Option<LanSettings>,
    discovery: LanDiscovery2,
) -> Result<()> {
    check_pre_spawn_cleanup(setup)?;
    discovery.validate(
        lan.map(|lan| kilogram_protocol::ordered_dialog_v2::DirectLan2::new(lan.local, lan.peer))
            .transpose()?,
    )?;
    connection_paths(&setup.home, &setup.record).context(ConnectionPathError)?;
    if lan_settings(setup) != lan || lan_discovery(setup) != discovery {
        change(setup, |state| {
            state.lan = lan;
            state.lan_discovery = discovery;
            state.profile = None;
            state.profile_digest = None;
            state.imports = [consent::Outcome::NotStarted; 2];
            state.last_outcome = None;
        })?;
    }
    Ok(())
}
pub(super) fn digest(raw: &[u8]) -> String {
    artifact::hex(blake3::hash(raw).as_bytes())
}
fn directory(setup: &Setup) -> Result<PathBuf> {
    let path = setup.home.join("connection");
    check_target_kind(&setup.home, &setup.record, &path, true)?;
    fs::create_dir_all(&path)?;
    check_target_kind(&setup.home, &setup.record, &path, true)?;
    ensure!(
        fs::canonicalize(&path)? == path,
        "Connection directory escaped its local location"
    );
    Ok(path)
}
fn check_artifact_path(setup: &Setup, path: &Path) -> Result<()> {
    validate_artifact_path(&setup.home, &setup.record, path)
}
pub(super) fn validate_artifact_path(home: &Path, record: &Record, path: &Path) -> Result<()> {
    ensure!(path.starts_with(home), "Artifact outside Setup");
    let info = record.public.as_ref().context("Account")?;
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
        if at == home {
            break;
        }
        at = at.parent().context("Local artifact ancestor")?;
    }
    Ok(())
}
fn check_target_kind(home: &Path, record: &Record, path: &Path, directory: bool) -> Result<()> {
    validate_artifact_path(home, record, path)?;
    match fs::symlink_metadata(path) {
        Ok(meta) => ensure!(
            (directory && meta.is_dir()) || (!directory && meta.is_file()),
            "Unexpected connection target type"
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

fn connection_paths(home: &Path, record: &Record) -> Result<Option<PathBuf>> {
    let base = home.join("connection");
    check_target_kind(home, record, &base, true)?;
    check_target_kind(home, record, &base.join("ipc.json"), false)?;
    check_target_kind(home, record, &base.join("locators"), true)?;
    let Some(state) = &record.connection else {
        return Ok(None);
    };
    state.validate()?;
    let Some(name) = &state.profile else {
        return Ok(None);
    };
    let path = base.join(name);
    check_target_kind(home, record, &path, false)?;
    Ok(Some(path))
}

fn read_expected_profile(
    path: &Path,
    expected: &Profile2Document,
    recorded_digest: Option<&str>,
) -> Result<(Profile2Document, String)> {
    let raw = model::read(path, kilogram_runtime_ipc::ordered2::PROFILE_CAP)?;
    let profile = Profile2Document::decode(&raw)?;
    ensure!(
        &profile == expected,
        "Profile conflict; keep the file and explicitly retry using a new local profile"
    );
    let hash = digest(&raw);
    ensure!(
        recorded_digest.is_none_or(|old| old == hash),
        "Recorded profile changed"
    );
    Ok((profile, hash))
}

pub(super) fn prepared_profile(home: &Path, record: &Record) -> Result<Option<Profile2Document>> {
    let path = connection_paths(home, record)?;
    let recorded_digest = record
        .connection
        .as_ref()
        .and_then(|s| s.profile_digest.as_deref());
    if let Some(path) = path
        && path.try_exists()?
    {
        let expected = expected_profile(record, &home.join("connection"))?;
        let (profile, _) = read_expected_profile(&path, &expected, recorded_digest)?;
        return Ok(recorded_digest.map(|_| profile));
    }
    ensure!(recorded_digest.is_none(), "Recorded profile is missing");
    Ok(None)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileState {
    NotPrepared,
    Prepared,
    Conflict,
}

pub fn profile_state(setup: &Setup) -> ProfileState {
    match prepared_profile(&setup.home, &setup.record) {
        Ok(Some(_)) => ProfileState::Prepared,
        Ok(None) => ProfileState::NotPrepared,
        Err(_) => ProfileState::Conflict,
    }
}
pub(super) fn expected_profile(record: &Record, base: &Path) -> Result<Profile2Document> {
    let info = record.public.as_ref().context("Account")?;
    let lan = record.connection.as_ref().and_then(|state| state.lan);
    Ok(Profile2Document {
        version: 2,
        profile_mode: "ordered-dialog-v2".into(),
        state_dir: info.state_dir.clone(),
        account: info.account_id.to_string(),
        claim: info.claim.clone(),
        device: info.device_id.to_string(),
        contexts: record.contexts.clone(),
        enabled: vec![0, 1],
        ipc_file: base.join("ipc.json"),
        locator_directory: base.join("locators"),
        route: ProfileRoute2 {
            policy: ProfileRoutePolicy2::DirectOnly,
            bind: Some(SocketAddr::from((
                lan.map_or(Ipv4Addr::LOCALHOST, |lan| lan.local),
                0,
            ))),
            relay: None,
            lan_peer_ipv4: lan.map(|lan| lan.peer),
        },
        lan_discovery: record
            .connection
            .as_ref()
            .map(|s| s.lan_discovery.clone())
            .unwrap_or_default(),
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
    prepare_with(setup, exe, cancel, fresh_profile, PrepareHelpers::default())
}

#[derive(Clone, Copy)]
struct PrepareHelpers {
    import: fn(&mut Setup, &Path, consent::Action, Arc<AtomicBool>) -> Result<()>,
    profile: fn(&Path, &[std::ffi::OsString], Arc<AtomicBool>) -> Result<()>,
}

impl Default for PrepareHelpers {
    fn default() -> Self {
        Self {
            import: consent::execute,
            profile: helper::public_command,
        }
    }
}

fn remember_helper_cleanup(setup: &mut Setup, result: Result<()>) -> Result<()> {
    if let Err(error) = &result
        && error.downcast_ref::<helper::HelperError>()
            == Some(&helper::HelperError::CleanupUnconfirmed)
    {
        setup.pre_spawn_cleanup_unconfirmed = true;
    }
    result
}

fn check_pre_spawn_cleanup(setup: &Setup) -> Result<()> {
    ensure!(
        !setup.pre_spawn_cleanup_unconfirmed(),
        helper::HelperError::CleanupUnconfirmed
    );
    Ok(())
}

fn prepare_with(
    setup: &mut Setup,
    exe: &Path,
    cancel: Arc<AtomicBool>,
    fresh_profile: bool,
    helpers: PrepareHelpers,
) -> Result<(PathBuf, Profile2Document)> {
    check_pre_spawn_cleanup(setup)?;
    ensure!(
        !cancel.load(Ordering::Acquire),
        helper::HelperError::Cancelled
    );
    if fresh_profile {
        connection_paths(&setup.home, &setup.record)?;
    } else {
        prepared_profile(&setup.home, &setup.record)?;
    }
    let imported = (helpers.import)(setup, exe, consent::Action::Import, cancel.clone());
    remember_helper_cleanup(setup, imported)?;
    ensure!(
        !cancel.load(Ordering::Acquire),
        helper::HelperError::Cancelled
    );
    let (nb, end) = consent::period(setup)?;
    let wall = consent::now()?;
    ensure!(
        nb <= wall && wall < end,
        "Permission is not currently valid"
    );
    connection_paths(&setup.home, &setup.record)?;
    let base = directory(setup)?;
    let doc = expected_profile(&setup.record, &base)?;
    if fresh_profile
        || setup
            .record
            .connection
            .as_ref()
            .and_then(|s| s.profile.as_ref())
            .is_none()
    {
        let name = format!("profile-{}.json", artifact::id()?);
        check_target_kind(&setup.home, &setup.record, &base.join(&name), false)?;
        change(setup, |s| {
            s.profile = Some(name);
            s.profile_digest = None;
        })?;
    }
    let path = connection_paths(&setup.home, &setup.record)?.context("Profile")?;
    if !path.try_exists()? {
        let mut args: Vec<std::ffi::OsString> = vec![
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
            doc.route
                .bind
                .context("Missing direct bind")?
                .to_string()
                .into(),
        ];
        if let Some(peer) = doc.route.lan_peer_ipv4 {
            args.extend(["--ordered-lan-peer-ipv4".into(), peer.to_string().into()]);
        }
        if !doc.lan_discovery.is_manual() {
            args.extend([
                "--ordered-lan-discovery".into(),
                serde_json::to_string(&doc.lan_discovery)?.into(),
            ]);
        }
        // A lost acknowledgement may leave a complete no-clobber output.
        let outcome = remember_helper_cleanup(setup, (helpers.profile)(exe, &args, cancel.clone()));
        if let Err(error) = outcome
            && matches!(
                error.downcast_ref::<helper::HelperError>(),
                Some(helper::HelperError::CleanupUnconfirmed | helper::HelperError::Cancelled)
            )
        {
            return Err(error);
        }
    }
    ensure!(
        !cancel.load(Ordering::Acquire),
        helper::HelperError::Cancelled
    );
    check_target_kind(&setup.home, &setup.record, &path, false)?;
    let (_, hash) = read_expected_profile(
        &path,
        &doc,
        setup
            .record
            .connection
            .as_ref()
            .and_then(|s| s.profile_digest.as_deref()),
    )?;
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
pub struct DialogScope {
    pub instance: [u8; 32],
    pub contexts: [[u8; 32]; 2],
    pub own_role: u8,
}

#[derive(Clone, Default)]
pub struct Messages {
    pub scope: Option<DialogScope>,
    pub send_slots: [bool; 2],
    pub pending: Projection,
}

#[derive(Clone)]
pub struct View {
    pub text: String,
    pub generation: u64,
    pub request_started: Instant,
    pub connected: [bool; 2],
    pub lease_expires: [u64; 2],
    pub consent_expires: u64,
    pub running: bool,
    pub discovery: Option<DiscoveryStatus2>,
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
            discovery: None,
            messages: Messages::default(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct Stop {
    pub graceful: bool,
    pub forced: bool,
    pub reaped: bool,
    pub exit: Option<i32>,
    pub eof: bool,
}
pub(super) struct Session {
    diagnostics: Option<DiagnosticSink>,
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
    spawned: AtomicBool,
    #[cfg(test)]
    reveal_exit: Option<Arc<AtomicBool>>,
    #[cfg(test)]
    prepare_helpers: Option<PrepareHelpers>,
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
    operation::wait(rt, Some(cancel), None, f)?
}
impl Session {
    fn start(
        setup: &mut Setup,
        exe: &Path,
        cancel: Arc<AtomicBool>,
        fresh_profile: bool,
        cleanup: &Cleanup,
    ) -> Result<Self> {
        #[cfg(test)]
        let prepared = if let Some(helpers) = cleanup.prepare_helpers {
            prepare_with(setup, exe, cancel.clone(), fresh_profile, helpers)
        } else {
            prepare(setup, exe, cancel.clone(), fresh_profile)
        };
        #[cfg(not(test))]
        let prepared = prepare(setup, exe, cancel.clone(), fresh_profile);
        let (path, doc) = prepared?;
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
        ensure!(
            !cancel.load(Ordering::Acquire),
            "Connection cancelled before spawn"
        );
        let child = command.spawn()?;
        cleanup.spawned.store(true, Ordering::Release);

        let mut session = Self {
            diagnostics: DiagnosticSink::open(&setup.home),
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
            status.route == self.doc.route
                && status.lan_discovery.as_ref().map_or_else(
                    || self.doc.lan_discovery.is_manual(),
                    |view| view.selection == self.doc.lan_discovery,
                )
                && (status.endpoint.is_some() || self.doc.route.lan_peer_ipv4.is_some()),
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
        self.poll_controlled(cancel, None)
    }

    fn poll_controlled(
        &mut self,
        cancel: &AtomicBool,
        control: Option<&OperationControl>,
    ) -> Result<View> {
        let started = Instant::now();
        self.check_drains()?;
        let client = self.client.as_ref().context("Owned client")?;
        ensure!(
            RuntimeIpcDescriptor::load(&self.doc.ipc_file)
                .as_ref()
                .is_ok_and(|d| d == client.descriptor()),
            "Runtime descriptor replaced; pinned client retained"
        );
        let status = operation::wait(&self.rt, Some(cancel), control, client.status())??;
        self.validate_status(&status)?;
        let (_, end) = self.consent_interval;
        let mut view = View::waiting(if status.endpoint.is_none() {
            "LAN address unavailable. The local runtime is still running."
        } else {
            "Runtime running; exchange current public locators. Waiting for authenticated connection."
        });
        view.running = true;
        view.discovery = status
            .lan_discovery
            .as_ref()
            .map(|discovery| discovery.state);
        if let Some(discovery) = view.discovery {
            view.text = match discovery {
                DiscoveryStatus2::WaitingNetwork => "Local runtime ready. Waiting for the saved Private LAN.",
                DiscoveryStatus2::WaitingPolicy => "Local runtime ready. LAN publication is waiting for current permission and locators.",
                DiscoveryStatus2::Discovering => "Local runtime ready. Discovering the agreed LAN peer.",
                DiscoveryStatus2::Retrying => "Local runtime ready. Authenticating or retrying the peer.",
                DiscoveryStatus2::Connected => "Both LAN contexts are connected.",
            }.into();
        }
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
                && (self.doc.route.lan_peer_ipv4.is_none() || c.selected_lan_peer.is_some())
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
    fn finish(&mut self, cleanup: &Cleanup) -> Stop {
        cleanup.stopping.store(true, Ordering::Release);
        let mut ack = false;
        let mut conflict = false;
        let observed = self.child.try_wait();
        let was_running = matches!(&observed, Ok(None));
        let child_query_failed = observed.is_err();
        let client_present = self.client.is_some();
        let mut ack_attempted = false;
        let mut descriptor_read_ok = false;
        if was_running && let Some(client) = &self.client {
            let descriptor = RuntimeIpcDescriptor::load(&self.doc.ipc_file);
            descriptor_read_ok = descriptor.is_ok();
            conflict = !descriptor.as_ref().is_ok_and(|d| d == client.descriptor());
            // Never reload the replacement and never address its server.
            ack_attempted = true;
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
        if let Some(sink) = &self.diagnostics {
            let _=sink.record(DiagnosticKind::ConnectionFinal,Some(self.child.id()),self.instance.as_deref(),
                &serde_json::json!({"client_present":client_present,"was_running":was_running,"child_query_failed":child_query_failed,
                    "ack_attempted":ack_attempted,"ack":ack,"descriptor_read_ok":descriptor_read_ok,"conflict":conflict,
                    "forced":forced,"exit_success":exit.success(),"exit":exit.code(),"eof":eof,"drain_count":self.drains.len(),
                    "drain_eof":self.drains.iter().map(|d|d.eof.load(Ordering::Acquire)).collect::<Vec<_>>(),
                    "drain_failed":self.drains.iter().map(|d|d.failed.load(Ordering::Acquire)).collect::<Vec<_>>()}));
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

mod locators;
pub use locators::{LocatorAttempt, LocatorCommand, LocatorReport, LocatorResult, LocatorState};

pub enum Action {
    Export,
    Import(PathBuf),
    Resume,
    Locator(Box<LocatorJob>),
    Stop,
    Send {
        slot: u8,
        text: Zeroizing<String>,
        reply: mpsc::SyncSender<Submission>,
    },
    RetrySend,
    Request(Box<RequestJob>),
    History(Box<HistoryJob>),
    TerminalHistory(Box<HistoryJob>, Arc<OperationControl>),
    HistoryPosition(Box<HistoryPositionJob>),
}

pub struct LocatorJob {
    pub scope: DialogScope,
    pub command: LocatorCommand,
    pub control: Arc<OperationControl>,
    pub reply: mpsc::SyncSender<LocatorResult>,
}

pub struct RequestJob {
    pub scope: DialogScope,
    pub slot: u8,
    pub request: [u8; 16],
    pub operation: RequestOperation,
    pub control: Option<Arc<OperationControl>>,
    pub reply: mpsc::SyncSender<RequestResult>,
}

pub struct HistoryJob {
    pub scope: DialogScope,
    pub slot: u8,
    pub cursor: Option<HistoryCursor2>,
    pub limit: usize,
    pub reply: mpsc::SyncSender<IpcResult<HistoryPage2>>,
}

pub struct HistoryPositionJob {
    pub scope: DialogScope,
    pub slot: u8,
    pub completed: Option<HistoryCursor2>,
    pub control: Arc<OperationControl>,
    pub reply: mpsc::SyncSender<IpcResult<HistoryPosition2>>,
}

pub(super) async fn read_history_position(
    client: &Client2,
    slot: u8,
    context: [u8; 32],
    completed: Option<&HistoryCursor2>,
) -> IpcResult<HistoryPosition2> {
    match completed {
        Some(cursor) => client.history_follow(slot, context, cursor).await,
        None => client.history_anchor(slot, context).await,
    }
}

pub struct DeliveryJob {
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
    control: Option<&'a OperationControl>,
}

impl SessionSend<'_> {
    fn wait<T>(&self, future: impl std::future::Future<Output = IpcResult<T>>) -> IpcResult<T> {
        operation::wait(&self.session.rt, Some(self.cancel), self.control, future)
            .map_err(|_| MessageError2::Unavailable)?
    }

    fn check_running(&self) -> IpcResult<()> {
        if self.cancel.load(Ordering::Acquire)
            || self.control.is_some_and(OperationControl::stopped)
        {
            return Err(MessageError2::Unavailable);
        }
        Ok(())
    }

    fn history(
        &mut self,
        slot: u8,
        cursor: Option<&HistoryCursor2>,
        limit: usize,
    ) -> IpcResult<HistoryPage2> {
        self.check_running()?;
        let context = self.context(slot)?;
        let client = self
            .session
            .client
            .as_ref()
            .ok_or(MessageError2::Unavailable)?;
        let response = self.wait(client.history_page(slot, context, cursor, limit));
        self.context(slot)?;
        response
    }

    fn context(&mut self, slot: u8) -> IpcResult<[u8; 32]> {
        let unavailable = MessageError2::Unavailable;
        if self.session.check_drains().is_err()
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

    fn history_position(
        &mut self,
        slot: u8,
        completed: Option<&HistoryCursor2>,
    ) -> IpcResult<HistoryPosition2> {
        self.check_running()?;
        let context = self.context(slot)?;
        let client = self
            .session
            .client
            .as_ref()
            .ok_or(MessageError2::Unavailable)?;
        let response = self.wait(read_history_position(client, slot, context, completed));
        self.check_running()?;
        self.context(slot)?;
        response
    }

    fn authorize(&mut self, slot: u8) -> IpcResult<Instant> {
        self.check_running()?;
        self.context(slot)?;
        let view = self
            .session
            .poll_controlled(self.cancel, self.control)
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
    fn operation(&self) -> Option<&OperationControl> {
        self.control
    }
    fn check_send(&mut self, slot: u8) -> IpcResult<()> {
        self.authorize(slot).map(|_| ())
    }

    fn lookup(&mut self, slot: u8, request: [u8; 16]) -> IpcResult<Option<RequestStatus2>> {
        self.check_running()?;
        let context = self.context(slot)?;
        let client = self
            .session
            .client
            .as_ref()
            .ok_or(MessageError2::Unavailable)?;
        let response = self.wait(client.request_status(slot, context, request));
        self.context(slot)?;
        response
    }

    fn send(&mut self, slot: u8, request: [u8; 16], text: &str) -> IpcResult<SendReceipt2> {
        let started = self.authorize(slot)?;
        let context = self.context(slot)?;
        if self.cancel.load(Ordering::Acquire)
            || started.elapsed() >= Duration::from_secs(2)
            || self.control.is_some_and(|op| !op.begin_ipc())
        {
            return Err(MessageError2::Unavailable);
        }
        let client = self
            .session
            .client
            .as_ref()
            .ok_or(MessageError2::Unavailable)?;
        let response = self.wait(client.send_text(slot, context, request, text));
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
    if let Some(sink) = &session.diagnostics {
        let _=sink.record(DiagnosticKind::WorkerFinal,Some(session.child.id()),session.instance.as_deref(),
            &serde_json::json!({"running_ok":running.is_ok(),"saved_ok":saved.is_ok(),"stop_graceful":stop.graceful,
                "stop_forced":stop.forced,"reaped":stop.reaped,"exit":stop.exit,"eof":stop.eof}));
    }
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
            Some((
                generation,
                action @ (Action::Export | Action::Import(_) | Action::Resume | Action::Locator(_)),
            )) => {
                handled = generation;
                let (command, job) = match action {
                    Action::Export => (LocatorCommand::Export, None),
                    Action::Import(path) => (LocatorCommand::Import(path), None),
                    Action::Resume => (LocatorCommand::Retry, None),
                    Action::Locator(job) => {
                        (job.command, Some((job.scope, job.control, job.reply)))
                    }
                    _ => unreachable!(),
                };
                let export = matches!(command, LocatorCommand::Export);
                let retry = matches!(command, LocatorCommand::Retry);
                let result = if job
                    .as_ref()
                    .is_none_or(|(expected, _, _)| expected == &scope)
                {
                    session.locator(
                        setup,
                        command,
                        signal,
                        job.as_ref().map(|(_, op, _)| op.as_ref()),
                    )
                } else {
                    Err("locator_unavailable")
                };
                last_notice = Some(match &result {
                    Ok(report) if report.state == LocatorState::Exported => format!("Public locators saved: {}", report.path.as_deref().unwrap_or_default()),
                    Ok(report) if report.state == LocatorState::Imported && retry => "Retained locator imports acknowledged.".into(),
                    Ok(report) if report.state == LocatorState::Imported => "Both locator imports were acknowledged.".into(),
                    _ if export => "Locator export not confirmed; existing files were preserved.".into(),
                    _ if retry => "Retained locator retry unconfirmed; existing progress was kept.".into(),
                    _ => "Local locator import unconfirmed/partial; existing progress was preserved. Retry the same file.".into(),
                });
                if let Some((_, _, reply)) = job {
                    let _ = reply.try_send(result);
                }
            }
            Some((generation, Action::Send { slot, text, reply })) => {
                handled = generation;
                let result = recovery.submit(
                    &mut SessionSend {
                        session,
                        cancel: signal,
                        control: None,
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
                    control: None,
                });
                next_pending = Instant::now() + Duration::from_secs(1);
            }
            Some((generation, Action::Request(job))) => {
                handled = generation;
                let result = if job.scope == scope {
                    recovery.request(
                        &mut SessionSend {
                            session,
                            cancel: signal,
                            control: job.control.as_deref(),
                        },
                        job.slot,
                        job.request,
                        job.operation,
                    )
                } else {
                    Err("session_unavailable")
                };
                let _ = job.reply.try_send(result);
                next_pending = Instant::now() + Duration::from_secs(1);
            }
            Some((generation, action @ (Action::History(_) | Action::TerminalHistory(..)))) => {
                handled = generation;
                let (job, control) = match action {
                    Action::History(job) => (job, None),
                    Action::TerminalHistory(job, control) => (job, Some(control)),
                    _ => unreachable!(),
                };
                let result = if job.scope == scope {
                    SessionSend {
                        session,
                        cancel: signal,
                        control: control.as_deref(),
                    }
                    .history(job.slot, job.cursor.as_ref(), job.limit)
                } else {
                    Err(MessageError2::InvalidInput)
                };
                let _ = job.reply.try_send(result);
            }
            Some((generation, Action::HistoryPosition(job))) => {
                handled = generation;
                let result = if job.scope == scope {
                    SessionSend {
                        session,
                        cancel: signal,
                        control: Some(&job.control),
                    }
                    .history_position(job.slot, job.completed.as_ref())
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
                control: None,
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
                    control: None,
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
pub struct Work {
    cancel: Arc<AtomicBool>,
    epoch: Arc<AtomicU64>,
    actions: mpsc::SyncSender<(u64, Action)>,
    deliveries: mpsc::SyncSender<DeliveryJob>,
    views: mpsc::Receiver<View>,
    done: mpsc::Receiver<(Setup, Result<Stop>)>,
    completion: Option<(Setup, Result<Stop>)>,
    cleanup: Arc<Cleanup>,
    pub view: View,
}
impl Work {
    pub fn start(
        setup: Setup,
        exe: PathBuf,
        fresh_profile: bool,
        lan: Option<LanSettings>,
    ) -> Result<Self> {
        Self::start_with_cleanup(setup, exe, fresh_profile, lan, Arc::new(Cleanup::default()))
    }

    fn start_with_cleanup(
        setup: Setup,
        exe: PathBuf,
        fresh_profile: bool,
        lan: Option<LanSettings>,
        cleanup: Arc<Cleanup>,
    ) -> Result<Self> {
        Self::spawn_work(
            setup,
            exe,
            fresh_profile,
            lan,
            cleanup,
            Arc::new(AtomicBool::new(false)),
        )
    }

    pub(super) fn start_terminal(
        setup: Setup,
        exe: PathBuf,
        lan: Option<LanSettings>,
        cancel: Arc<AtomicBool>,
    ) -> Result<Self> {
        Self::spawn_work(setup, exe, false, lan, Arc::new(Cleanup::default()), cancel)
    }

    fn spawn_work(
        mut setup: Setup,
        exe: PathBuf,
        fresh_profile: bool,
        lan: Option<LanSettings>,
        cleanup: Arc<Cleanup>,
        cancel: Arc<AtomicBool>,
    ) -> Result<Self> {
        check_pre_spawn_cleanup(&setup)?;
        ensure!(
            !cancel.load(Ordering::Acquire),
            "Connection cancelled before worker start"
        );
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
                let result = select_route(&mut setup, lan).and_then(|()| {
                    run_worker(
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
                    )
                });
                let _ = dtx.send((setup, result));
            })?;
        Ok(Self {
            cancel,
            epoch,
            actions: tx,
            deliveries: delivery_tx,
            views: vrx,
            done: drx,
            completion: None,
            cleanup,
            view: View::waiting(
                "Checking both local permissions and starting the owned runtime...",
            ),
        })
    }
    pub fn action(&mut self, action: Action) -> Result<()> {
        ensure!(
            !self.cleanup.stopping() && !self.cancel.load(Ordering::Acquire),
            "Owned runtime is stopping"
        );
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
    pub(super) fn refresh_view(&mut self) {
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
        if self.completion.is_none() {
            self.completion = self.done.try_recv().ok();
        }
    }
    pub(super) fn finished(&self) -> bool {
        self.completion.is_some()
    }
    pub(super) fn pre_spawn_cleanup_unconfirmed(&self) -> bool {
        self.completion
            .as_ref()
            .is_some_and(|(setup, _)| setup.pre_spawn_cleanup_unconfirmed())
    }
    pub(super) fn spawned(&self) -> bool {
        self.cleanup.spawned.load(Ordering::Acquire)
    }
    pub fn poll(&mut self) -> Option<(Setup, Result<Stop>)> {
        self.refresh_view();
        self.completion.take()
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
