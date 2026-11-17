use super::{
    Running, Shared, State,
    wire::{Command, Payload},
};
use kilogram_bootstrap_contract::Ordered2BootstrapOutput;
use kilogram_first_dialog::{
    artifact::Artifact,
    connection::{self, ConnectionPathError, LanSettings},
    consent::Action,
    consent_artifact::Envelope,
    helper::{self, HelperError},
    model::{Setup, SetupAccessError},
};
use kilogram_protocol::ordered_dialog_v2::DirectLan2;
use kilogram_runtime_ipc::ordered2::LanDiscovery2;
use serde_json::{Value, json};
use std::{
    net::Ipv4Addr,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};
use zeroize::Zeroizing;

mod agreement;
mod pairing;
use pairing::identity;

enum Preview {
    Pairing { id: String, artifact: Artifact },
    Consent { id: String, envelope: Envelope },
}

pub(super) fn unopened() -> Value {
    json!({"state":"unopened","account_id":null,"device_id":null,"contexts":null})
}

pub(super) struct Service {
    home: PathBuf,
    helper: PathBuf,
    cli: PathBuf,
    setup: Option<Setup>,
    preview: Option<Preview>,
    #[cfg(windows)]
    inspection: Option<kilogram_cli::ordered_dialog::runtime2::lan_metadata::Inspection>,
}

impl Service {
    #[cfg(test)]
    pub fn new(home: PathBuf, helper: PathBuf) -> Self {
        let cli = helper.with_file_name("kilogram-cli.exe");
        Self::with_cli(home, helper, cli)
    }

    pub fn with_cli(home: PathBuf, helper: PathBuf, cli: PathBuf) -> Self {
        Self {
            home,
            helper,
            cli,
            setup: None,
            preview: None,
            #[cfg(windows)]
            inspection: None,
        }
    }

    pub fn snapshot(&self) -> Result<Value, &'static str> {
        let Some(setup) = &self.setup else {
            return Ok(unopened());
        };
        let phase = if setup.incomplete {
            if setup.workspace_reserved().map_err(access_error)? {
                "incomplete"
            } else {
                "prepared"
            }
        } else if setup.record.acknowledged {
            "recovery_saved"
        } else if setup.phrase.is_some() {
            "recovery_available"
        } else {
            "recovery_unavailable"
        };
        let public = setup.record.public.as_ref();
        let mut value = json!({
            "state":phase, "account_id":public.map(|p| p.account_id.to_string()),
            "device_id":public.map(|p| p.device_id.to_string()), "contexts":&setup.record.contexts,
        });
        if let Some(peer) = &setup.record.peer {
            value["peer_selection"] = json!({
                "role": if setup.record.joined { "joiner" } else { "inviter" },
                "invitation_id": &setup.record.invitation,
                "peer": identity(peer.account, peer.device),
            });
        }
        if let Some(consent) = agreement::snapshot(setup)? {
            value["consent"] = consent;
        }
        if !setup.incomplete {
            let lan = connection::lan_settings(setup);
            let discovery = connection::lan_discovery(setup);
            value["connection"] = json!({
                "mode": if lan.is_some() { "direct_lan" } else { "loopback" },
                "local_ipv4": lan.map_or(Ipv4Addr::LOCALHOST, |lan| lan.local),
                "peer_ipv4": lan.map(|lan| lan.peer),
                "profile_state": connection::profile_state(setup),
                "lan_discovery": discovery,
            });
            if !discovery.is_manual() {
                value["connection"]["lan_announcement_notice"] = json!(
                    "LAN Auto periodically sends announcements visible to devices on the selected LAN."
                );
            }
        }
        Ok(value)
    }

    pub fn operation(
        &mut self,
        command: Command,
        shown: bool,
        generation: u64,
        cancel: Arc<AtomicBool>,
        session: &Shared,
    ) -> Result<Payload, &'static str> {
        if matches!(command, Command::Begin | Command::Open) {
            self.preview = None;
        }
        match command {
            Command::Begin if self.setup.is_some() => return Err("setup_exists"),
            Command::Begin => {
                self.setup = Some(Setup::begin_terminal(&self.home).map_err(access_error)?)
            }
            Command::Open => {
                if self.setup.is_some() {
                    self.refresh()?;
                } else {
                    self.setup = Some(Setup::open_terminal(&self.home).map_err(access_error)?);
                }
            }
            Command::Create => self.create(generation, cancel, session)?,
            Command::Show => {
                let setup = self.refresh()?;
                let phrase = setup.phrase.as_ref().ok_or("recovery_unavailable")?;
                return Ok(Payload::Secret(Zeroizing::new(phrase.as_str().to_owned())));
            }
            Command::Acknowledge => {
                let setup = self.refresh()?;
                if setup.incomplete {
                    return Err("invalid_state");
                }
                if setup.phrase.is_some() && !shown {
                    return Err("recovery_show_required");
                }
                if !setup.record.acknowledged {
                    setup.acknowledge(true).map_err(|_| "setup_invalid")?;
                }
            }
            Command::Inspect(kind, path) => return self.inspect(kind, &path).map(Payload::Public),
            Command::Confirm(kind, id) => self.confirm(kind, &id)?,
            Command::Export(kind) => return self.export(kind).map(Payload::Public),
            Command::ConsentInspect(kind, path) => {
                return self.inspect_consent(kind, &path).map(Payload::Public);
            }
            Command::ConsentAccept(kind, id) => self.accept_consent(kind, &id)?,
            Command::ConsentExport => return self.export_consent().map(Payload::Public),
            Command::ConsentAction(action) => {
                let result = self.run_consent(action, cancel);
                self.finish_consent(action, generation, session, result)?;
            }
            Command::Route(lan) => self.save_route(lan)?,
            Command::RouteAuto(lan) => self.save_auto_route(lan, &cancel, generation, session)?,
            Command::ClearPreview => {
                self.preview = None;
                return Ok(Payload::Public(json!({"cleared":true})));
            }
            _ => unreachable!("only local setup operations are sent to the worker"),
        }
        self.snapshot().map(Payload::Public)
    }

    fn route_setup(&mut self) -> Result<&mut Setup, &'static str> {
        let setup = self.consent_setup()?;
        if !setup
            .record
            .consent
            .as_ref()
            .is_some_and(|s| s.final_artifact.is_some())
        {
            return Err("consent_not_ready");
        }
        Ok(setup)
    }

    fn save_route(&mut self, lan: Option<LanSettings>) -> Result<(), &'static str> {
        let setup = self.route_setup()?;
        if let Some(lan) = lan {
            let route = DirectLan2::new(lan.local, lan.peer).map_err(|_| "invalid_lan_route")?;
            kilogram_cli::ordered_dialog::runtime2::verify_lan_interface(route)
                .map_err(|_| "lan_interface_unavailable")?;
        }
        Self::record_route(setup, lan, LanDiscovery2::Manual {})
    }

    fn record_route(
        setup: &mut Setup,
        lan: Option<LanSettings>,
        discovery: LanDiscovery2,
    ) -> Result<(), &'static str> {
        let state = &setup
            .record
            .public
            .as_ref()
            .ok_or("invalid_state")?
            .state_dir;
        let _owner = kilogram_state::StateDirectoryLock::acquire(state)
            .map_err(|_| "runtime_must_be_stopped")?;
        connection::select_lan_discovery(setup, lan, discovery).map_err(|error| {
            if error.downcast_ref::<ConnectionPathError>().is_some() {
                "connection_path_invalid"
            } else {
                "route_save_failed"
            }
        })
    }

    #[cfg(windows)]
    fn save_auto_route(
        &mut self,
        lan: LanSettings,
        cancel: &AtomicBool,
        generation: u64,
        session: &Shared,
    ) -> Result<(), &'static str> {
        use kilogram_cli::ordered_dialog::runtime2::lan_metadata::{Inspection, setting};
        use std::{sync::atomic::Ordering, thread, time::Duration};
        self.route_setup()?;
        if cancel.load(Ordering::Acquire) {
            return Err("cancelled");
        }
        if let Some(inspection) = &mut self.inspection {
            if inspection.try_result().is_none() {
                return Err("lan_metadata_pending");
            }
            self.inspection = None;
        }
        let route = DirectLan2::new(lan.local, lan.peer).map_err(|_| "invalid_lan_route")?;
        self.inspection =
            Some(Inspection::start(route, None).map_err(|_| "lan_interface_unavailable")?);
        let binding = loop {
            let inspection = self
                .inspection
                .as_mut()
                .ok_or("lan_interface_unavailable")?;
            if let Some(result) = inspection.try_result() {
                let fresh = inspection.started.elapsed() < Duration::from_secs(2);
                self.inspection = None;
                if !fresh {
                    return Err("lan_metadata_timeout");
                }
                break result.map_err(|_| "lan_interface_unavailable")?;
            }
            if cancel.load(Ordering::Acquire) {
                return Err("cancelled");
            }
            if inspection.started.elapsed() >= Duration::from_secs(2) {
                return Err("lan_metadata_timeout");
            }
            thread::sleep(Duration::from_millis(25));
        };
        if !session
            .lock()
            .map_err(|_| "cleanup_unconfirmed")?
            .settle(generation)
        {
            return Err("cancelled");
        }
        Self::record_route(self.route_setup()?, Some(lan), setting(binding))
    }

    #[cfg(not(windows))]
    fn save_auto_route(
        &mut self,
        _lan: LanSettings,
        _cancel: &AtomicBool,
        _generation: u64,
        _session: &Shared,
    ) -> Result<(), &'static str> {
        Err("lan_auto_windows_only")
    }

    fn finish_consent(
        &mut self,
        action: Action,
        generation: u64,
        session: &Shared,
        result: Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        if !session.lock().unwrap().settle(generation) {
            if action == Action::Import
                && let Some(setup) = &mut self.setup
            {
                setup.consent_confirmed = false;
            }
            if result != Err("cleanup_unconfirmed") {
                return Err("cancelled");
            }
        }
        result
    }

    fn refresh(&mut self) -> Result<&mut Setup, &'static str> {
        let setup = self.setup.as_mut().ok_or("not_open")?;
        if setup.refresh_terminal().is_err() {
            self.setup = None;
            return Err("setup_invalid");
        }
        Ok(self.setup.as_mut().unwrap())
    }

    fn create(
        &mut self,
        generation: u64,
        cancel: Arc<AtomicBool>,
        session: &Shared,
    ) -> Result<(), &'static str> {
        self.create_with(generation, cancel, session, helper::run_checked)
    }

    fn create_with(
        &mut self,
        generation: u64,
        cancel: Arc<AtomicBool>,
        session: &Shared,
        launch: impl FnOnce(
            &std::path::Path,
            &std::path::Path,
            &[String; 2],
            Arc<AtomicBool>,
        ) -> Result<Ordered2BootstrapOutput, HelperError>,
    ) -> Result<(), &'static str> {
        let helper_path = self.helper.clone();
        let setup = self.refresh()?;
        if !setup.incomplete || setup.workspace_reserved().map_err(access_error)? {
            return Err("invalid_state");
        }
        let local_id = setup.record.local_id.clone();
        let result = launch(
            &helper_path,
            &setup.record.workspace,
            &setup.record.contexts,
            cancel,
        );
        let mut state = session.lock().unwrap();
        let adopt = state.settle(generation);
        drop(state);
        let result = match result {
            Err(HelperError::CleanupUnconfirmed) => return Err("cleanup_unconfirmed"),
            _ if !adopt => Err("cancelled"),
            Err(error) => Err(helper_error(error)),
            Ok(output) if output.public().root_key_protection == "windows-dpapi-current-user" => {
                setup.workspace_reserved().map_err(access_error)?;
                setup.accept(output, &local_id).map_err(|_| "setup_invalid")
            }
            Ok(_) => Err("setup_invalid"),
        };
        if result.is_err() {
            self.refresh()?;
        }
        result
    }
}

fn helper_error(error: HelperError) -> &'static str {
    match error {
        HelperError::Unavailable => "helper_unavailable",
        HelperError::Failed(_) => "helper_failed",
        HelperError::Timeout => "helper_timeout",
        HelperError::Cancelled => "cancelled",
        HelperError::CleanupUnconfirmed => "cleanup_unconfirmed",
    }
}

fn access_error(error: SetupAccessError) -> &'static str {
    match error {
        SetupAccessError::Exists => "setup_exists",
        SetupAccessError::Missing => "setup_missing",
        SetupAccessError::NotEmpty => "directory_not_empty",
        SetupAccessError::Locked => "setup_locked",
        SetupAccessError::Invalid => "setup_invalid",
    }
}

pub(super) fn execute(
    mut service: Service,
    jobs: std::sync::mpsc::Receiver<super::Job>,
    shared: Shared,
) {
    while let Ok(job) = jobs.recv() {
        let super::Job::Run {
            entry,
            shown,
            cancel,
        } = job
        else {
            break;
        };
        let generation = entry.tag.generation;
        let result = service.operation(entry.command, shown, generation, cancel, &shared);
        let snapshot = if matches!(result, Err("cleanup_unconfirmed")) {
            unopened()
        } else {
            service.snapshot().unwrap_or_else(|_| unopened())
        };
        let mut state: std::sync::MutexGuard<'_, State> = shared.lock().unwrap();
        if let Some(Running { tag, .. }) = state.running.take() {
            debug_assert_eq!(tag.generation, generation);
            state.complete(tag, result, snapshot);
        }
    }
    #[cfg(windows)]
    if let Some(mut inspection) = service.inspection.take()
        && inspection.try_result().is_none()
        && let Ok(mut state) = shared.lock()
    {
        state.close(None, true, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_setup::Tag;

    #[test]
    fn t12c_cancel_wins_import_completion_and_cleanup_error_wins_cancel() {
        for (worker_result, expected) in [
            (Ok(()), Err("cancelled")),
            (Err("cleanup_unconfirmed"), Err("cleanup_unconfirmed")),
        ] {
            let folder = tempfile::tempdir().unwrap();
            let home = folder.path().join("setup");
            let mut service = Service::new(home.clone(), folder.path().join("unused.exe"));
            let mut setup = Setup::begin(&home, None).unwrap();
            setup.consent_confirmed = true;
            service.setup = Some(setup);
            let before = std::fs::read(home.join("setup.json")).unwrap();
            let (session, _) = running();
            session.lock().unwrap().cancel(None).unwrap();
            assert_eq!(
                service.finish_consent(Action::Import, 1, &session, worker_result),
                expected
            );
            assert!(!service.setup.unwrap().consent_confirmed);
            assert_eq!(std::fs::read(home.join("setup.json")).unwrap(), before);
        }
    }
    use kilogram_identity::AccountRecoveryPhrase;
    use std::{fs, sync::Mutex};

    fn running() -> (Shared, Arc<AtomicBool>) {
        let mut state = State::new(true);
        let cancel = Arc::new(AtomicBool::new(false));
        state.running = Some(Running {
            tag: Tag {
                generation: 1,
                id: None,
            },
            cancel: Some(cancel.clone()),
            settling: false,
        });
        (Arc::new(Mutex::new(state)), cancel)
    }

    #[test]
    fn t12a_cancel_before_and_after_workspace_reservation() {
        let folder = tempfile::tempdir().unwrap();
        let home = folder.path().join("setup");
        let mut service = Service::new(home.clone(), folder.path().join("absent.exe"));
        service.setup = Some(Setup::begin_terminal(&home).unwrap());
        let (state, cancel) = running();
        assert_eq!(service.create(1, cancel, &state), Err("helper_unavailable"));
        assert_eq!(service.snapshot().unwrap()["state"], "prepared");
        for reserved in [false, true] {
            let (state, cancel) = running();
            let result = service.create_with(1, cancel, &state, |_, workspace, _, _| {
                if reserved {
                    fs::create_dir(workspace).unwrap();
                    fs::write(workspace.join("bootstrap-pending.json"), b"{}").unwrap();
                }
                state.lock().unwrap().cancel(None).unwrap();
                Err(HelperError::Cancelled)
            });
            assert_eq!(result, Err("cancelled"));
            assert_eq!(
                service.snapshot().unwrap()["state"],
                if reserved { "incomplete" } else { "prepared" }
            );
        }
        let (state, cancel) = running();
        assert_eq!(
            service.create_with(1, cancel, &state, |_, _, _, _| panic!(
                "reused reserved workspace"
            )),
            Err("invalid_state")
        );
        drop(service);
        let reopened = Setup::open_terminal(&home).unwrap();
        assert!(reopened.incomplete && reopened.workspace_reserved().unwrap());
    }

    #[cfg(windows)]
    #[test]
    fn t12a_lost_private_result_reopens_ready_without_words() {
        let Some(helper) = std::env::var_os("KILOGRAM_T12A_HELPER").map(PathBuf::from) else {
            return;
        };
        let folder = tempfile::tempdir().unwrap();
        let home = folder.path().join("setup");
        let mut service = Service::new(home.clone(), helper);
        service.setup = Some(Setup::begin_terminal(&home).unwrap());
        let (state, cancel) = running();
        let result = service.create_with(1, cancel, &state, |exe, workspace, contexts, token| {
            let output = helper::run_checked(exe, workspace, contexts, token)?;
            let phrase = AccountRecoveryPhrase::parse(output.recovery_phrase()).ok();
            assert!(
                phrase.and_then(|phrase| phrase.account_id().ok())
                    == Some(output.public().account_id)
            );
            state.lock().unwrap().cancel(None).unwrap();
            Ok(output)
        });
        assert_eq!(result, Err("cancelled"));
        assert_eq!(service.snapshot().unwrap()["state"], "recovery_unavailable");
        drop(service);
        let reopened = Setup::open_terminal(&home).unwrap();
        assert!(!reopened.incomplete && !reopened.record.acknowledged && reopened.phrase.is_none());
        println!("T12A_READY_WITHOUT_PRIVATE_RESULT_PASS");
    }
}
