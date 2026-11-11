//! Explicit first-dialog local connection pilot; messaging remains separate.
mod artifact;
mod connection;
mod consent;
mod consent_artifact;
mod helper;
mod locator_artifact;
mod model;
#[cfg(test)]
mod tests;

use anyhow::{Result, ensure};
use artifact::{Artifact, Kind};
use eframe::egui;
use kilogram_bootstrap_contract::Ordered2BootstrapOutput;
use model::Setup;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

struct Work {
    generation: String,
    cancel: Arc<AtomicBool>,
    result: mpsc::Receiver<Result<Ordered2BootstrapOutput>>,
    join: Option<thread::JoinHandle<()>>,
}
struct ConsentWork {
    generation: String,
    cancel: Arc<AtomicBool>,
    result: mpsc::Receiver<(Setup, Result<()>)>,
    join: Option<thread::JoinHandle<()>>,
}
impl Drop for ConsentWork {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
impl Drop for Work {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        // Helper uses a 20ms cancellation probe, owned kill/wait and 5s reap.
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
        // A late secret result is dropped with the receiver, never logged.
    }
}
pub(super) struct Panel {
    home: String,
    executable: PathBuf,
    cli_executable: PathBuf,
    input: String,
    setup: Option<Setup>,
    preview: Option<Artifact>,
    verified: bool,
    saved: bool,
    reveal: bool,
    work: Option<Work>,
    consent_work: Option<ConsentWork>,
    connection_work: Option<connection::Work>,
    locator_input: String,
    close_requested: bool,
    consent_input: String,
    consent_next_check: Instant,
    consent_check_error: Option<String>,
    consent_period: Option<(u64, u64)>,
    error: Option<String>,
    notice: Option<String>,
}
impl Panel {
    pub fn new(home: PathBuf, executable: PathBuf) -> Self {
        let exists = home.join("setup.json").exists();
        let mut panel = Self {
            home: home.display().to_string(),
            executable,
            cli_executable: super::default_runtime_executable(),
            input: String::new(),
            setup: None,
            preview: None,
            verified: false,
            saved: false,
            reveal: false,
            work: None,
            consent_work: None,
            connection_work: None,
            locator_input: String::new(),
            close_requested: false,
            consent_input: String::new(),
            consent_next_check: Instant::now(),
            consent_check_error: None,
            consent_period: None,
            error: None,
            notice: None,
        };
        if exists {
            panel.open();
        }
        panel
    }
    pub fn pause(&mut self) {
        if let Some(work) = &mut self.connection_work {
            work.cancel();
        }
        self.work = None;
        self.consent_work = None;
        if let Some(setup) = &mut self.setup {
            setup.phrase = None;
        }
        self.reveal = false;
        self.saved = false;
    }
    pub fn set_cli(&mut self, path: PathBuf) {
        self.cli_executable = path;
    }
    fn start_consent(&mut self, action: consent::Action) -> Result<()> {
        ensure!(
            self.work.is_none() && self.consent_work.is_none(),
            "Another account operation is running"
        );
        let mut setup = self
            .setup
            .take()
            .ok_or_else(|| anyhow::anyhow!("Open the saved setup"))?;
        let generation = setup.record.local_id.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let signal = cancel.clone();
        let exe = self.cli_executable.clone();
        let (tx, rx) = mpsc::channel();
        let join = thread::Builder::new()
            .name("fresh-consent-helper".into())
            .spawn(move || {
                let result = consent::execute(&mut setup, &exe, action, signal.clone());
                if !signal.load(Ordering::Acquire) {
                    let _ = tx.send((setup, result));
                }
            })?;
        self.consent_work = Some(ConsentWork {
            generation,
            cancel,
            result: rx,
            join: Some(join),
        });
        Ok(())
    }
    fn poll_consent(&mut self) {
        let Some(work) = &self.consent_work else {
            return;
        };
        let received = match work.result.try_recv() {
            Ok(r) => Some(Ok(r)),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(_) => Some(Err(())),
        };
        if let Some(result) = received {
            let generation = work.generation.clone();
            self.consent_work = None;
            match result{
                Ok((setup,result)) if setup.record.local_id==generation && self.setup.is_none()=>{
                    self.setup=Some(setup);self.record_result(result);
                    if self.error.is_none(){self.notice=Some("Consent action completed locally. Export the public file or continue local installation; no network connection was started.".into());}
                }
                _=>self.error=Some("Consent action ended without a current result. Reopen the saved setup to inspect or resume its exact artifacts.".into()),
            }
        }
    }
    fn open(&mut self) {
        self.consent_next_check = Instant::now();
        self.pause();
        self.setup = None;
        self.preview = None;
        self.verified = false;
        match Setup::open(Path::new(&self.home)) {
            Ok(setup) => {
                self.setup = Some(setup);
                self.error = None;
            }
            Err(_) => {
                self.error = Some(
                    "Cannot reopen this setup. Keep its files; no account was reset or retried."
                        .into(),
                )
            }
        }
    }
    fn start(&mut self, join: bool) -> Result<()> {
        ensure!(
            self.work.is_none() && self.setup.is_none(),
            "Open the existing setup instead of creating another account"
        );
        let invite = if join {
            ensure!(
                self.verified,
                "Compare the invitation identity independently first"
            );
            Some(
                self.preview
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Choose an invitation first"))?
                    .clone(),
            )
        } else {
            None
        };
        let setup = Setup::begin(Path::new(&self.home), invite)?;
        let generation = setup.record.local_id.clone();
        let workspace = setup.record.workspace.clone();
        let contexts = setup.record.contexts.clone();
        let exe = self.executable.clone();
        self.setup = Some(setup);
        self.preview = None;
        self.verified = false;
        self.saved = false;
        self.reveal = false;
        let cancel = Arc::new(AtomicBool::new(false));
        let signal = cancel.clone();
        let (tx, rx) = mpsc::channel();
        let join = thread::Builder::new()
            .name("fresh-account-helper".into())
            .spawn(move || {
                let result = helper::run(&exe, &workspace, &contexts, signal.clone());
                if !signal.load(Ordering::Acquire) {
                    let _ = tx.send(result);
                }
            })?;
        self.work = Some(Work {
            generation,
            cancel,
            result: rx,
            join: Some(join),
        });
        Ok(())
    }
    fn poll(&mut self) {
        let Some(work) = &self.work else { return };
        let result = match work.result.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(_) => Some(Err(anyhow::anyhow!(
                "Account helper ended without a result"
            ))),
        };
        if let Some(result) = result {
            let generation = work.generation.clone();
            self.work = None;
            let result = result.and_then(|output| {
                self.setup
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("Setup no longer selected"))?
                    .accept(output, &generation)
            });
            match result {
                Ok(()) => {
                    self.error = None;
                    self.notice = Some(
                        "Account created. Save your recovery words before exchanging public files."
                            .into(),
                    );
                }
                Err(_) => {
                    self.error=Some("Creation did not return a verified result. Reopen this setup to inspect its public status; do not retry in the same account folder.".into());
                }
            }
        }
    }
    fn preview(&mut self) -> Result<()> {
        // Clear confirmation before any parsing, including failed imports.
        self.preview = None;
        self.verified = false;
        let artifact = Artifact::decode(&model::read(Path::new(&self.input), artifact::FILE_CAP)?)?;
        if let Some(setup) = &self.setup {
            setup.check_reply(&artifact)?;
        } else {
            ensure!(
                artifact.kind == Kind::Invite,
                "Joining requires an invitation"
            );
        }
        self.preview = Some(artifact);
        Ok(())
    }
    fn record_result(&mut self, result: Result<()>) {
        self.consent_next_check = Instant::now();
        if let Err(e) = result {
            self.error = Some(e.to_string());
        } else {
            self.error = None;
        }
    }
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        self.poll();
        self.poll_consent();
        self.poll_connection();
        if self.connection_work.is_some() {
            self.draw_connection(ui);
            return;
        }
        ui.heading("First-dialog pilot");
        ui.label("Create an invitation, or join one before creating your first account.");
        ui.small("One peer only. Local connection is available after mutual consent. Messaging comes in the next step. Direct connections between different PCs and volunteer storage are not enabled in this pilot.");
        ui.separator();
        if self.consent_work.is_some() {
            ui.spinner();
            ui.label("Processing mutual consent locally. Existing successful steps are kept.");
            if ui.button("Cancel and keep consent progress").clicked() {
                self.pause();
                self.notice =
                    Some("Consent action cancelled. Reopen the same setup before resuming.".into());
            }
            return;
        }
        if self.setup.is_none() {
            ui.label("Account setup folder");
            ui.add(egui::TextEdit::singleline(&mut self.home).desired_width(f32::INFINITY));
            ui.horizontal(|ui| {
                if ui.button("Create invitation").clicked() {
                    let r = self.start(false);
                    self.record_result(r);
                }
                if ui.button("Open saved setup").clicked() {
                    self.open();
                }
            });
        } else if let Some(setup) = &self.setup {
            ui.label(format!("Setup: {}", setup.home.display()));
            if let Some(info) = &setup.record.public {
                ui.label(format!("Your account: {}", info.account_id));
                ui.label(format!("Your device: {}", info.device_id));
            }
        }
        if self.work.is_some() {
            ui.spinner();
            ui.label("Creating your account. Recovery words stay inside this app.");
            if ui.button("Cancel creation and keep its files").clicked() {
                self.pause();
                self.notice=Some("Creation cancelled. Reopen to check whether the account completed before cancellation.".into());
            }
        } else {
            let mut acknowledge = false;
            let mut export = false;
            let mut reopen = false;
            if let Some(setup) = &self.setup {
                if setup.incomplete {
                    ui.label("Creation incomplete or result not received. Files are preserved; this destination will not be reused.");
                    reopen = ui.button("Reopen and check saved setup").clicked();
                } else if !setup.record.acknowledged {
                    ui.heading("Save your recovery words");
                    if let Some(phrase) = &setup.phrase {
                        ui.checkbox(&mut self.reveal, "Reveal recovery words privately");
                        if self.reveal {
                            // Never selectable/copyable by the ordinary label action.
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(phrase.as_str()).monospace().size(18.0),
                                )
                                .selectable(false),
                            );
                        } else {
                            ui.label(
                                "Words hidden. Reveal them only when nobody can see your screen.",
                            );
                        }
                    } else {
                        ui.colored_label(ui.visuals().warn_fg_color,"This account completed, but this app no longer has its one-time words. They cannot be shown again. Continue only if you really saved them before closing.");
                    }
                    ui.label("The words restore your account key and identity. Current authority needs a separate package and latest witness. Messages require their own backup or another device.");
                    ui.checkbox(
                        &mut self.saved,
                        "I actually saved the recovery words offline",
                    );
                    acknowledge = ui
                        .add_enabled(self.saved, egui::Button::new("Hide words and continue"))
                        .clicked();
                } else {
                    ui.heading(if setup.record.consent.is_some() {
                        "First peer consent setup"
                    } else if setup.record.joined {
                        "Awaiting invitation owner's confirmation"
                    } else if setup.record.peer.is_some() {
                        "Peer selected: awaiting mutual consent"
                    } else {
                        "Awaiting your peer's reply"
                    });
                    ui.label(
                        "Account creation is complete. Your conversation is not connected yet.",
                    );
                    if let Some(peer) = &setup.record.peer {
                        ui.label(format!("Selected peer account: {}", peer.account));
                        ui.label(format!("Selected peer device: {}", peer.device));
                    }
                    export = ui
                        .button(if setup.record.joined {
                            "Export public reply"
                        } else {
                            "Export public invitation"
                        })
                        .clicked();
                    ui.add_enabled(
                        false,
                        egui::Button::new("Connect and send messages (next step)"),
                    );
                    ui.small("Recovery authority package/witness export is a separate existing account recovery action. Never share the recovery words or account folder.");
                }
            }
            if reopen {
                self.open();
            }
            if acknowledge {
                let r = self
                    .setup
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("Missing setup"))
                    .and_then(|s| s.acknowledge(self.saved));
                if r.is_ok() {
                    self.reveal = false;
                    self.saved = false;
                    self.notice = Some(
                        "Recovery words cleared from the view. Export only your public file."
                            .into(),
                    );
                }
                self.record_result(r);
            }
            if export {
                let r = self
                    .setup
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Missing setup"))
                    .and_then(Setup::export)
                    .map(|p| {
                        self.notice = Some(format!("Public file saved: {}", p.display()));
                    });
                self.record_result(r);
            }
            let can_import = self
                .setup
                .as_ref()
                .is_none_or(|s| !s.record.joined && s.record.acknowledged);
            if can_import {
                ui.separator();
                ui.label(if self.setup.is_some() {
                    "Public reply file"
                } else {
                    "Public invitation file to join"
                });
                let changed = ui
                    .add(egui::TextEdit::singleline(&mut self.input).desired_width(f32::INFINITY))
                    .changed();
                if changed {
                    self.preview = None;
                    self.verified = false;
                }
                if ui.button("Inspect public file").clicked() {
                    let r = self.preview();
                    self.record_result(r);
                }
                if let Some(artifact) = &self.preview {
                    ui.label(format!("Peer account: {}", artifact.sender.account));
                    ui.label(format!("Peer device: {}", artifact.sender.device));
                    ui.small("A valid signature does not prove who sent this file. Compare these identifiers with your peer over a known independent channel.");
                    ui.checkbox(
                        &mut self.verified,
                        "I compared this peer's identity independently",
                    );
                    let selected = self.setup.is_some();
                    if ui
                        .add_enabled(
                            self.verified,
                            egui::Button::new(if selected {
                                "Keep this peer selection"
                            } else {
                                "Join invitation and create account"
                            }),
                        )
                        .clicked()
                    {
                        let r = if selected {
                            let reply = artifact.clone();
                            self.setup
                                .as_mut()
                                .ok_or_else(|| anyhow::anyhow!("Missing setup"))
                                .and_then(|s| s.select_reply(reply, self.verified))
                        } else {
                            self.start(true)
                        };
                        if r.is_ok() && selected {
                            self.preview = None;
                            self.verified = false;
                            self.notice=Some("Peer selection saved. Mutual consent and connection are the next step.".into());
                        }
                        self.record_result(r);
                    }
                }
            }
        }
        self.draw_consent(ui);
        if self.work.is_none()
            && self.consent_work.is_none()
            && self.setup.as_ref().is_some_and(|s| {
                s.record
                    .consent
                    .as_ref()
                    .is_some_and(|c| c.final_artifact.is_some())
            })
        {
            ui.separator();
            ui.heading("Local connection pilot");
            ui.label("Two windows on this computer only. Both permissions are checked before each start. No messages or remote-device connection yet.");
            for (label, fresh) in [
                ("Start local connection", false),
                ("Retry with a new local profile file", true),
            ] {
                if ui.button(label).clicked() {
                    let result = self.start_connection(fresh);
                    self.record_result(result);
                }
            }
        }
        if let Some(notice) = &self.notice {
            ui.separator();
            ui.label(notice);
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }
    fn start_connection(&mut self, fresh: bool) -> Result<()> {
        ensure!(
            self.connection_work.is_none() && self.consent_work.is_none() && self.work.is_none(),
            "Another operation is running"
        );
        let setup = self
            .setup
            .take()
            .ok_or_else(|| anyhow::anyhow!("Open the saved setup"))?;
        self.connection_work = Some(connection::Work::start(
            setup,
            self.cli_executable.clone(),
            fresh,
        )?);
        Ok(())
    }
    pub fn poll_connection(&mut self) {
        let done = self
            .connection_work
            .as_mut()
            .and_then(connection::Work::poll);
        if let Some((setup, result)) = done {
            self.connection_work = None;
            self.setup = Some(setup);
            match result {
                Ok(stop) if stop.graceful && stop.reaped && stop.eof && stop.exit==Some(0)=>{self.notice=Some("Owned runtime stopped gracefully. The same account and permissions are ready for an explicit restart.".into());self.error=None;},
                _=>self.error=Some("Runtime start/stop did not complete gracefully. Existing account and public artifacts were kept; any forced cleanup is not a successful graceful stop.".into()),
            }
        }
    }
    pub fn handle_close(&mut self, ctx: &egui::Context) {
        self.poll_connection();
        if ctx.input(|i| i.viewport().close_requested()) && self.connection_work.is_some() {
            self.close_requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if let Some(w) = &mut self.connection_work {
                w.cancel();
            }
        }
        if self.close_requested && self.connection_work.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
    fn draw_connection(&mut self, ui: &mut egui::Ui) {
        let Some(work) = &mut self.connection_work else {
            return;
        };
        ui.heading("Local connection pilot - two windows on this computer");
        ui.label(format!("Saved setup: {}", self.home));
        let current = work
            .view
            .current(Instant::now(), consent::now().unwrap_or(u64::MAX));
        if current {
            ui.heading("Connected locally - authenticated Active2 on both slots");
        } else {
            ui.heading("Connection not currently confirmed");
        }
        ui.label(&work.view.text);
        ui.small("A connected badge reflects a recent checked runtime lease. Peer availability may change. Messaging, remote devices and volunteer storage come later.");
        if ui.button("Export current public locators").clicked() {
            let _ = work.action(connection::Action::Export);
        }
        ui.label("Public locator file from the selected peer");
        ui.add(egui::TextEdit::singleline(&mut self.locator_input).desired_width(f32::INFINITY));
        if ui.button("Inspect and import both peer locators").clicked() {
            let _ = work.action(connection::Action::Import(PathBuf::from(
                &self.locator_input,
            )));
        }
        if ui.button("Retry retained peer locators").clicked() {
            let _ = work.action(connection::Action::Resume);
        }
        if ui.button("Stop local connection").clicked() {
            work.cancel();
        }
        ui.add_enabled(false, egui::Button::new("Send messages (next step)"));
    }
    fn draw_consent(&mut self, ui: &mut egui::Ui) {
        let Some(setup) = &self.setup else { return };
        if self.work.is_some() || !setup.record.acknowledged || setup.record.peer.is_none() {
            return;
        }
        let r = match consent::role(setup) {
            Ok(r) => r,
            Err(e) => {
                ui.label(e.to_string());
                return;
            }
        };
        ui.separator();
        ui.heading("Mutual consent");
        ui.label("Agree to this first peer dialog for up to 30 days. Retrying keeps the original validity and identities; it does not renew permission.");
        let mut action = None;
        let mut select = false;
        let mut export = false;
        // Rendering is not authority: avoid re-reading/verifying every file at
        // 20Hz. Each action revalidates independently; expiry is checked every
        // frame even between these bounded public-data refreshes.
        if Instant::now() >= self.consent_next_check {
            self.consent_check_error = consent::review(setup, true).err().map(|e| e.to_string());
            self.consent_period = consent::period(setup).ok();
            self.consent_next_check = Instant::now() + Duration::from_secs(1);
        }
        let mut error = self.consent_check_error.clone();
        if let Some((start, end)) = self.consent_period
            && let Err(e) =
                consent::now().and_then(|wall| consent::valid_interval(start, end, wall))
        {
            error = Some(e.to_string());
        }
        let current = error.is_none();
        if let Some(e) = &error {
            ui.colored_label(ui.visuals().warn_fg_color, e);
        }
        if let Some(state) = &setup.record.consent {
            if let Some((start, end)) = self.consent_period {
                let remaining = end.saturating_sub(consent::now().unwrap_or(end));
                ui.label(format!("Fixed permission period: {} days. Remaining: {}h {}m. Retrying does not extend it.",(end-start)/86400,remaining/3600,(remaining%3600)/60));
            }
            if setup.consent_confirmed && current {
                ui.heading("Mutual consent installed locally; connection is the next step");
                ui.small("This does not prove your peer imported it or that a network session is active.");
            } else if state
                .slots
                .iter()
                .all(|s| s.importing == consent::Outcome::Accepted)
            {
                ui.label("Both local imports were acknowledged earlier. Recheck the exact finals to confirm this reopening; the UI record alone does not prove installation.");
            }
            for (slot, s) in state.slots.iter().enumerate() {
                let text = |o| match o {
                    consent::Outcome::NotStarted => "not started",
                    consent::Outcome::Started => "result uncertain; resume",
                    consent::Outcome::Accepted => "acknowledged locally",
                    consent::Outcome::Failed => "failed; retry exact inputs",
                };
                ui.label(format!(
                    "Permission {}: signature {}; import {}",
                    slot + 1,
                    text(s.signing),
                    text(s.importing)
                ));
            }
        }
        ui.add_enabled_ui(current, |ui| {
            if r == 0 {
                ui.label("Your device creates the proposal for this peer.");
                if ui.button("Create or resume consent proposal").clicked() {
                    action = Some(consent::Action::Propose);
                }
            } else {
                ui.label("Your peer creates the proposal. Inspect it before choosing to sign.");
                let available = setup
                    .record
                    .consent
                    .as_ref()
                    .is_some_and(|s| s.proposal.is_some());
                if ui
                    .add_enabled(
                        available,
                        egui::Button::new("Verify and sign both permissions"),
                    )
                    .clicked()
                {
                    action = Some(consent::Action::Cosign);
                }
            }
            let exportable = setup.record.consent.as_ref().is_some_and(|s| {
                if r == 0 {
                    s.slots.iter().all(|s| s.output.is_some())
                } else {
                    s.final_artifact.is_some()
                }
            });
            if ui
                .add_enabled(
                    exportable,
                    egui::Button::new(if r == 0 {
                        "Export consent proposal"
                    } else {
                        "Export signed consent"
                    }),
                )
                .clicked()
            {
                export = true;
            }
            let importable = setup
                .record
                .consent
                .as_ref()
                .is_some_and(|s| s.final_artifact.is_some());
            if ui
                .add_enabled(
                    importable,
                    egui::Button::new("Install or recheck both local permissions"),
                )
                .clicked()
            {
                action = Some(consent::Action::Import);
            }
        });
        ui.label(if r == 0 {
            "Signed consent file from your peer"
        } else {
            "Consent proposal file from your peer"
        });
        ui.add(egui::TextEdit::singleline(&mut self.consent_input).desired_width(f32::INFINITY));
        if ui.button("Inspect and keep this consent file").clicked() {
            select = true;
        }
        if select {
            let result = (|| {
                let e = consent_artifact::Envelope::decode(&model::read(
                    Path::new(&self.consent_input),
                    artifact::FILE_CAP,
                )?)?;
                consent::select(
                    self.setup
                        .as_mut()
                        .ok_or_else(|| anyhow::anyhow!("Setup"))?,
                    &e,
                )
            })();
            self.record_result(result);
        }
        if export {
            let result = self
                .setup
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Setup"))
                .and_then(consent::export)
                .map(|p| self.notice = Some(format!("Public consent file saved: {}", p.display())));
            self.record_result(result);
        }
        if let Some(action) = action {
            let result = self.start_consent(action);
            self.record_result(result);
        }
    }
}
impl Drop for Panel {
    fn drop(&mut self) {
        self.pause();
    }
}
