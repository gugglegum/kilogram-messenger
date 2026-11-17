//! Terminal chat over the regular Setup worker or a read-only pinned local IPC session.
use crate::{
    artifact,
    connection::{
        self, Action, DialogScope, HistoryJob, HistoryPositionJob, LocatorCommand, LocatorJob,
        LocatorResult, RequestJob, Work,
    },
    consent,
    helper::HelperError,
    model::Setup,
    operation::{self, OperationControl},
    send_recovery::{IpcResult, RequestOperation, RequestReport, RequestResult, RequestState},
};
use anyhow::{Context, Result, ensure};
use kilogram_runtime_ipc::{
    RuntimeIpcDescriptor,
    ordered2::{
        Client2, HistoryCursor2, HistoryPage2, HistoryPosition2, MessageError2, Profile2Document,
        Status2, id,
    },
};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Owner,
    Observer,
}

#[derive(Serialize)]
pub struct ChatStatus {
    pub mode: Mode,
    pub runtime_instance: String,
    pub contexts: [String; 2],
    pub live_admitted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lan_discovery: Option<kilogram_runtime_ipc::ordered2::DiscoveryStatus2>,
    #[serde(skip)]
    pub own_roles: [u8; 2],
}

pub struct Chat {
    setup_dir: PathBuf,
    executable: PathBuf,
    session: Option<Session>,
    cleanup_unconfirmed: bool,
}

enum Session {
    Owner(Box<Work>),
    Observer(Box<Observer>),
}

/// Stop can be requested before Setup is opened or a child is spawned.
#[derive(Default)]
pub struct SessionStop(Mutex<StopState>);

#[derive(Default)]
struct StopState {
    requested: bool,
    owner: Option<Arc<AtomicBool>>,
}

impl SessionStop {
    pub fn stop(&self) {
        let mut state = self.0.lock().unwrap();
        state.requested = true;
        if let Some(owner) = &state.owner {
            owner.store(true, Ordering::Release);
        }
    }

    fn register(&self, owner: Arc<AtomicBool>) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        ensure!(!state.requested, "Connection is closing");
        state.owner = Some(owner);
        Ok(())
    }
}

impl Chat {
    pub fn new(setup_dir: PathBuf, executable: PathBuf) -> Self {
        Self {
            setup_dir,
            executable,
            session: None,
            cleanup_unconfirmed: false,
        }
    }

    pub fn connect(&mut self, mode: Mode, instance: Option<&str>) -> Result<ChatStatus> {
        let was_connected = self.session.is_some();
        let result = self.connect_controlled(mode, instance, None, &SessionStop::default());
        if result.is_err() && !was_connected {
            let _ = self.disconnect();
        }
        result
    }

    pub fn connect_controlled(
        &mut self,
        mode: Mode,
        expected_instance: Option<&str>,
        control: Option<&OperationControl>,
        stop: &SessionStop,
    ) -> Result<ChatStatus> {
        ensure!(
            self.session.is_none() && !self.cleanup_unconfirmed,
            "Session already open or cleanup unconfirmed"
        );
        let session = match mode {
            Mode::Owner => {
                ensure!(expected_instance.is_none(), "Owner starts a new instance");
                ensure!(
                    self.setup_dir.join("setup.json").is_file(),
                    "Saved Setup required"
                );
                let owner = Arc::new(AtomicBool::new(false));
                stop.register(owner.clone())?;
                ensure!(
                    control.is_none_or(|op| op.provisional_owner(owner.clone())),
                    "Connection cancelled"
                );
                let setup = Setup::open(&self.setup_dir)?;
                let lan = connection::lan_settings(&setup);
                Session::Owner(Box::new(Work::start_terminal(
                    setup,
                    self.executable.clone(),
                    lan,
                    owner,
                )?))
            }
            Mode::Observer => Session::Observer(Box::new(Observer::open(
                &self.setup_dir,
                expected_instance,
                control,
            )?)),
        };
        self.session = Some(session);
        self.status_with_limit(Duration::from_secs(45), control)
    }

    pub fn status(&mut self) -> Result<ChatStatus> {
        self.status_controlled(None)
    }

    pub fn status_controlled(&mut self, control: Option<&OperationControl>) -> Result<ChatStatus> {
        self.status_with_limit(Duration::from_secs(6), control)
    }

    fn status_with_limit(
        &mut self,
        limit: Duration,
        control: Option<&OperationControl>,
    ) -> Result<ChatStatus> {
        match self.session.as_mut().context("Connect first")? {
            Session::Owner(work) => {
                let scope = owner_scope(work, limit, control)?;
                Ok(ChatStatus {
                    mode: Mode::Owner,
                    runtime_instance: artifact::hex(&scope.instance),
                    contexts: scope.contexts.map(|context| artifact::hex(&context)),
                    live_admitted: work.view.current(Instant::now(), consent::now()?),
                    lan_discovery: work.view.discovery,
                    own_roles: [scope.own_role; 2],
                })
            }
            Session::Observer(observer) => observer.status(control),
        }
    }

    pub fn history(
        &mut self,
        slot: u8,
        cursor: Option<HistoryCursor2>,
        limit: usize,
    ) -> Result<HistoryPage2> {
        self.history_controlled(slot, cursor, limit, None)
    }

    pub fn history_controlled(
        &mut self,
        slot: u8,
        cursor: Option<HistoryCursor2>,
        limit: usize,
        control: Option<&Arc<OperationControl>>,
    ) -> Result<HistoryPage2> {
        match self.session.as_mut().context("Connect first")? {
            Session::Owner(work) => {
                let scope = owner_scope(work, Duration::from_secs(6), control.map(Arc::as_ref))?;
                let (reply, page) = mpsc::sync_channel(1);
                let job = Box::new(HistoryJob {
                    scope,
                    slot,
                    cursor,
                    limit,
                    reply,
                });
                if let Some(control) = control {
                    work.action(Action::TerminalHistory(job, control.clone()))?;
                    Ok(page.recv()??)
                } else {
                    work.action(Action::History(job))?;
                    Ok(page.recv_timeout(Duration::from_secs(7))??)
                }
            }
            Session::Observer(observer) => {
                observer.history(slot, cursor.as_ref(), limit, control.map(Arc::as_ref))
            }
        }
    }

    pub fn history_position_controlled(
        &mut self,
        slot: u8,
        completed: Option<HistoryCursor2>,
        control: &Arc<OperationControl>,
    ) -> IpcResult<HistoryPosition2> {
        if slot >= 2 {
            return Err(MessageError2::InvalidInput);
        }
        let unavailable = MessageError2::Unavailable;
        if control.stopped() {
            return Err(unavailable);
        }
        let result = match self.session.as_mut().ok_or(unavailable)? {
            Session::Owner(work) => {
                let scope = owner_scope(work, Duration::from_secs(6), Some(control))
                    .map_err(|_| unavailable)?;
                let (reply, done) = mpsc::sync_channel(1);
                work.action(Action::HistoryPosition(Box::new(HistoryPositionJob {
                    scope,
                    slot,
                    completed,
                    control: control.clone(),
                    reply,
                })))
                .map_err(|_| unavailable)?;
                done.recv().map_err(|_| unavailable)?
            }
            Session::Observer(observer) => {
                observer.history_position(slot, completed.as_ref(), control)
            }
        };
        if control.stopped() {
            return Err(unavailable);
        }
        result
    }

    pub fn request(
        &mut self,
        slot: u8,
        request: [u8; 16],
        operation: RequestOperation,
    ) -> RequestResult {
        self.request_controlled(slot, request, operation, None)
    }

    pub fn request_controlled(
        &mut self,
        slot: u8,
        request: [u8; 16],
        operation: RequestOperation,
        control: Option<&Arc<OperationControl>>,
    ) -> RequestResult {
        match self.session.as_mut().ok_or("not_connected")? {
            Session::Observer(_) if !matches!(operation, RequestOperation::Status) => {
                Err("observer_read_only")
            }
            Session::Observer(observer) => {
                observer.request_status(slot, request, control.map(Arc::as_ref))
            }
            Session::Owner(work) => {
                let scope = owner_scope(work, Duration::from_secs(6), control.map(Arc::as_ref))
                    .map_err(|_| "session_unavailable")?;
                let (reply, done) = mpsc::sync_channel(1);
                work.action(Action::Request(Box::new(RequestJob {
                    scope,
                    slot,
                    request,
                    operation,
                    control: control.cloned(),
                    reply,
                })))
                .map_err(|_| "session_unavailable")?;
                // The terminal may publish a conservative result earlier. Its
                // internal slot still represents this actual worker operation.
                if control.is_some() {
                    return done.recv().map_err(|_| "session_unavailable")?;
                }
                done.recv_timeout(Duration::from_secs(30))
                    .unwrap_or_else(|_| {
                        Ok(RequestReport {
                            state: RequestState::Unconfirmed,
                            protected: None,
                            event_id: None,
                        })
                    })
            }
        }
    }

    pub fn locator(
        &mut self,
        command: LocatorCommand,
        control: &Arc<OperationControl>,
    ) -> LocatorResult {
        let work = match self.session.as_mut().ok_or("not_connected")? {
            Session::Observer(_) => return Err("observer_read_only"),
            Session::Owner(work) => work,
        };
        let scope = owner_scope(work, Duration::from_secs(6), Some(control))
            .map_err(|_| "locator_unavailable")?;
        let (reply, done) = mpsc::sync_channel(1);
        work.action(Action::Locator(Box::new(LocatorJob {
            scope,
            command,
            control: control.clone(),
            reply,
        })))
        .map_err(|_| "locator_unavailable")?;
        done.recv().map_err(|_| "locator_unavailable")?
    }

    pub fn disconnect(&mut self) -> Result<()> {
        self.disconnect_until(Instant::now() + Duration::from_secs(30))
    }

    pub fn disconnect_until(&mut self, end: Instant) -> Result<()> {
        if let Some(Session::Owner(mut work)) = self.session.take() {
            work.cancel();
            loop {
                if let Some((setup, result)) = work.poll() {
                    let stopped = !setup.pre_spawn_cleanup_unconfirmed()
                        && (!work.spawned()
                            || (result.is_ok_and(|stop| {
                                stop.graceful && stop.reaped && stop.eof && stop.exit == Some(0)
                            }) && !setup.home.join("connection/ipc.json").exists()));
                    self.cleanup_unconfirmed |= !stopped;
                    break;
                }
                if Instant::now() >= end {
                    self.cleanup_unconfirmed = true;
                    self.session = Some(Session::Owner(work));
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
        }
        ensure!(
            !self.cleanup_unconfirmed,
            "Owned runtime cleanup unconfirmed"
        );
        Ok(())
    }
}

fn owner_scope(
    work: &mut Work,
    limit: Duration,
    control: Option<&OperationControl>,
) -> Result<DialogScope> {
    let end = Instant::now() + limit;
    loop {
        ensure!(
            control.is_none_or(|op| !op.stopped()),
            "Connection operation cancelled"
        );
        work.refresh_view();
        ensure!(
            !work.pre_spawn_cleanup_unconfirmed(),
            HelperError::CleanupUnconfirmed
        );
        ensure!(!work.finished(), "Owned runtime exited");
        if let Some(scope) = work.view.messages.scope.filter(|_| work.view.running) {
            return Ok(scope);
        }
        ensure!(Instant::now() < end, "Owned runtime status timed out");
        thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(all(test, windows))]
pub(super) fn verify_pre_spawn_cleanup_handoff(
    home: PathBuf,
    exe: PathBuf,
    work: Work,
) -> Result<()> {
    let mut chat = Chat::new(home, exe);
    chat.session = Some(Session::Owner(Box::new(work)));
    let error = chat
        .status()
        .err()
        .context("Unknown helper exit appeared as an available runtime")?;
    ensure!(error.downcast_ref::<HelperError>() == Some(&HelperError::CleanupUnconfirmed));
    let Some(Session::Owner(work)) = chat.session.as_ref() else {
        anyhow::bail!("No owned work to retire");
    };
    ensure!(
        !work.spawned(),
        "Runtime spawned after uncertain helper cleanup"
    );
    let end = Instant::now() + Duration::from_secs(2);
    ensure!(
        chat.disconnect_until(end).is_err(),
        "Disconnect falsely reported clean shutdown"
    );
    ensure!(
        Instant::now() < end,
        "Completed pre-spawn failure stalled shutdown"
    );
    ensure!(chat.cleanup_unconfirmed && chat.session.is_none());
    ensure!(
        chat.disconnect().is_err(),
        "The cleanup fence was cleared on a second disconnect"
    );
    ensure!(
        chat.connect(Mode::Owner, None).is_err(),
        "A new owner bypassed the fence"
    );
    Ok(())
}

struct Observer {
    client: Client2,
    runtime: tokio::runtime::Runtime,
    profile: Profile2Document,
    instance: String,
}

impl Observer {
    fn open(
        setup_dir: &Path,
        expected_instance: Option<&str>,
        control: Option<&OperationControl>,
    ) -> Result<Self> {
        let home = fs::canonicalize(setup_dir)?;
        let record = Setup::read_record(&home)?;
        ensure!(
            record.acknowledged && record.peer.is_some(),
            "Saved peer Setup required"
        );
        let profile =
            connection::prepared_profile(&home, &record)?.context("No prepared profile")?;
        let public = record.public.as_ref().context("Account not ready")?;
        let descriptor = RuntimeIpcDescriptor::load(&profile.ipc_file)?;
        let client = Client2::new(
            descriptor,
            public.account_id,
            public.device_id,
            [id(&profile.contexts[0])?, id(&profile.contexts[1])?],
        )?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let status = operation::wait(&runtime, None, control, client.status())??;
        if let Some(instance) = expected_instance {
            ensure!(
                id(instance)? == id(&status.runtime_instance)?,
                "Runtime instance mismatch"
            );
        }
        let observer = Self {
            client,
            runtime,
            profile,
            instance: status.runtime_instance.clone(),
        };
        observer.check_status(&status)?;
        Ok(observer)
    }

    fn check_descriptor(&self) -> Result<()> {
        let descriptor = RuntimeIpcDescriptor::load(&self.profile.ipc_file)?;
        ensure!(
            &descriptor == self.client.descriptor(),
            "Observed descriptor changed; attach again"
        );
        Ok(())
    }

    fn check_status(&self, status: &Status2) -> Result<()> {
        self.check_descriptor()?;
        ensure!(
            status.runtime_instance == self.instance
                && status.route == self.profile.route
                && status.lan_discovery.as_ref().map_or_else(
                    || self.profile.lan_discovery.is_manual(),
                    |view| view.selection == self.profile.lan_discovery,
                ),
            "Observed runtime changed; attach again"
        );
        for slot in 0..2 {
            ensure!(
                status.installed(slot)?.context == self.profile.contexts[slot],
                "Context binding"
            );
        }
        Ok(())
    }

    fn status(&self, control: Option<&OperationControl>) -> Result<ChatStatus> {
        self.check_descriptor()?;
        let status = operation::wait(&self.runtime, None, control, self.client.status())??;
        self.check_status(&status)?;
        let now = consent::now()?;
        let live_admitted = (0..2).all(|slot| {
            status.installed(slot).is_ok_and(|row| {
                row.phase == 3
                    && row.active
                    && row.live_admitted
                    && row.lease_expires.is_some_and(|expires| now < expires)
                    && row.blocked.is_none()
                    && row.policy_error.is_none()
                    && (self.profile.route.lan_peer_ipv4.is_none()
                        || row.selected_lan_peer.is_some())
            })
        });
        Ok(ChatStatus {
            mode: Mode::Observer,
            runtime_instance: self.instance.clone(),
            contexts: self.profile.contexts.clone(),
            live_admitted,
            lan_discovery: status.lan_discovery.as_ref().map(|view| view.state),
            own_roles: [status.installed(0)?.role, status.installed(1)?.role],
        })
    }

    fn history(
        &self,
        slot: u8,
        cursor: Option<&HistoryCursor2>,
        limit: usize,
        control: Option<&OperationControl>,
    ) -> Result<HistoryPage2> {
        self.status(control)?;
        let context = self
            .profile
            .contexts
            .get(usize::from(slot))
            .context("Context slot")?;
        let page = operation::wait(
            &self.runtime,
            None,
            control,
            self.client.history_page(slot, id(context)?, cursor, limit),
        )??;
        self.status(control)?;
        Ok(page)
    }

    fn history_position(
        &self,
        slot: u8,
        completed: Option<&HistoryCursor2>,
        control: &OperationControl,
    ) -> IpcResult<HistoryPosition2> {
        let unavailable = MessageError2::Unavailable;
        self.status(Some(control)).map_err(|_| unavailable)?;
        let context = self
            .profile
            .contexts
            .get(usize::from(slot))
            .ok_or(MessageError2::InvalidInput)?;
        let context = id(context).map_err(|_| MessageError2::InvalidInput)?;
        let result = operation::wait(
            &self.runtime,
            None,
            Some(control),
            connection::read_history_position(&self.client, slot, context, completed),
        )
        .map_err(|_| unavailable)?;
        self.status(Some(control)).map_err(|_| unavailable)?;
        result
    }

    fn request_status(
        &self,
        slot: u8,
        request: [u8; 16],
        control: Option<&OperationControl>,
    ) -> RequestResult {
        let context = self
            .profile
            .contexts
            .get(usize::from(slot))
            .ok_or("invalid_context")?;
        let context = id(context).map_err(|_| "invalid_context")?;
        self.status(control).map_err(|_| "status_unavailable")?;
        let status = operation::wait(
            &self.runtime,
            None,
            control,
            self.client.request_status(slot, context, request),
        )
        .map_err(|_| "status_unavailable")?
        .map_err(|_| "status_unavailable")?;
        self.status(control).map_err(|_| "status_unavailable")?;
        Ok(RequestReport::from_status(status))
    }
}
