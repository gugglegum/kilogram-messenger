use super::{
    Running, Shared, State,
    wire::{Command, Payload},
};
use kilogram_bootstrap_contract::Ordered2BootstrapOutput;
use kilogram_first_dialog::{
    artifact::Artifact,
    consent::Action,
    consent_artifact::Envelope,
    helper::{self, HelperError},
    model::{Setup, SetupAccessError},
};
use serde_json::{Value, json};
use std::{
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
            Command::ClearPreview => {
                self.preview = None;
                return Ok(Payload::Public(json!({"cleared":true})));
            }
            _ => unreachable!("only local setup operations are sent to the worker"),
        }
        self.snapshot().map(Payload::Public)
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
