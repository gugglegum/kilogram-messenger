//! Read-only chat projection over the regular Setup worker or a pinned local IPC session.
use crate::{
    artifact,
    connection::{self, Action, DialogScope, HistoryJob, Work},
    consent,
    model::{self, Setup},
};
use anyhow::{Context, Result, ensure};
use kilogram_runtime_ipc::{
    RuntimeIpcDescriptor,
    ordered2::{Client2, HistoryCursor2, HistoryPage2, PROFILE_CAP, Profile2Document, Status2, id},
};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc,
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

impl Chat {
    pub fn new(setup_dir: PathBuf, executable: PathBuf) -> Self {
        Self {
            setup_dir,
            executable,
            session: None,
            cleanup_unconfirmed: false,
        }
    }

    pub fn connect(&mut self, mode: Mode, expected_instance: Option<&str>) -> Result<ChatStatus> {
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
                let setup = Setup::open(&self.setup_dir)?;
                let lan = connection::lan_settings(&setup);
                Session::Owner(Box::new(Work::start(
                    setup,
                    self.executable.clone(),
                    false,
                    lan,
                )?))
            }
            Mode::Observer => Session::Observer(Box::new(Observer::open(
                &self.setup_dir,
                expected_instance,
            )?)),
        };
        self.session = Some(session);
        let status = self.status_with_limit(Duration::from_secs(45));
        if status.is_err() {
            let _ = self.disconnect();
        }
        status
    }

    pub fn status(&mut self) -> Result<ChatStatus> {
        self.status_with_limit(Duration::from_secs(6))
    }

    fn status_with_limit(&mut self, limit: Duration) -> Result<ChatStatus> {
        match self.session.as_mut().context("Connect first")? {
            Session::Owner(work) => {
                let scope = owner_scope(work, limit)?;
                Ok(ChatStatus {
                    mode: Mode::Owner,
                    runtime_instance: artifact::hex(&scope.instance),
                    contexts: scope.contexts.map(|context| artifact::hex(&context)),
                    live_admitted: work.view.current(Instant::now(), consent::now()?),
                })
            }
            Session::Observer(observer) => observer.status(),
        }
    }

    pub fn history(
        &mut self,
        slot: u8,
        cursor: Option<HistoryCursor2>,
        limit: usize,
    ) -> Result<HistoryPage2> {
        match self.session.as_mut().context("Connect first")? {
            Session::Owner(work) => {
                let scope = owner_scope(work, Duration::from_secs(6))?;
                let (reply, page) = mpsc::sync_channel(1);
                work.action(Action::History(Box::new(HistoryJob {
                    scope,
                    slot,
                    cursor,
                    limit,
                    reply,
                })))?;
                Ok(page.recv_timeout(Duration::from_secs(7))??)
            }
            Session::Observer(observer) => observer.history(slot, cursor.as_ref(), limit),
        }
    }

    pub fn disconnect(&mut self) -> Result<()> {
        if let Some(Session::Owner(mut work)) = self.session.take() {
            work.cancel();
            let end = Instant::now() + Duration::from_secs(30);
            loop {
                if let Some((setup, result)) = work.poll() {
                    let stopped = result.is_ok_and(|stop| {
                        stop.graceful && stop.reaped && stop.eof && stop.exit == Some(0)
                    });
                    self.cleanup_unconfirmed |=
                        !stopped || setup.home.join("connection/ipc.json").exists();
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

fn owner_scope(work: &mut Work, limit: Duration) -> Result<DialogScope> {
    let end = Instant::now() + limit;
    loop {
        // Keep the worker's terminal receipt for disconnect, even if it ended before readiness.
        work.refresh_view();
        ensure!(!work.finished(), "Owned runtime exited");
        if let Some(scope) = work.view.messages.scope.filter(|_| work.view.running) {
            return Ok(scope);
        }
        ensure!(Instant::now() < end, "Owned runtime status timed out");
        thread::sleep(Duration::from_millis(20));
    }
}

struct Observer {
    client: Client2,
    runtime: tokio::runtime::Runtime,
    profile: Profile2Document,
    instance: String,
}

impl Observer {
    fn open(setup_dir: &Path, expected_instance: Option<&str>) -> Result<Self> {
        let home = fs::canonicalize(setup_dir)?;
        let record = Setup::read_record(&home)?;
        ensure!(
            record.acknowledged && record.peer.is_some(),
            "Saved peer Setup required"
        );
        let state = record
            .connection
            .as_ref()
            .context("No prepared connection")?;
        state.validate()?;
        let base = home.join("connection");
        let path = base.join(state.profile.as_ref().context("No prepared profile")?);
        connection::validate_artifact_path(&home, &record, &path)?;
        let raw = model::read(&path, PROFILE_CAP)?;
        ensure!(
            state.profile_digest.as_deref() == Some(&connection::digest(&raw)),
            "Profile digest mismatch"
        );
        let profile = Profile2Document::decode(&raw)?;
        ensure!(
            profile == connection::expected_profile(&record, &base)?,
            "Profile binding mismatch"
        );
        connection::validate_artifact_path(&home, &record, &profile.ipc_file)?;
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
        let status = runtime.block_on(client.status())?;
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
            status.runtime_instance == self.instance && status.route == self.profile.route,
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

    fn status(&self) -> Result<ChatStatus> {
        self.check_descriptor()?;
        let status = self.runtime.block_on(self.client.status())?;
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
        })
    }

    fn history(
        &self,
        slot: u8,
        cursor: Option<&HistoryCursor2>,
        limit: usize,
    ) -> Result<HistoryPage2> {
        self.status()?;
        let context = self
            .profile
            .contexts
            .get(usize::from(slot))
            .context("Context slot")?;
        let page =
            self.runtime
                .block_on(self.client.history_page(slot, id(context)?, cursor, limit))?;
        self.status()?;
        Ok(page)
    }
}
