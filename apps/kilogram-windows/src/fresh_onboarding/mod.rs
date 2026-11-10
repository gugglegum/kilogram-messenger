//! Explicit first-dialog pilot. No legacy profile, IPC, signing or runtime start.
mod artifact;
mod helper;
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
};

struct Work {
    generation: String,
    cancel: Arc<AtomicBool>,
    result: mpsc::Receiver<Result<Ordered2BootstrapOutput>>,
    join: Option<thread::JoinHandle<()>>,
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
    input: String,
    setup: Option<Setup>,
    preview: Option<Artifact>,
    verified: bool,
    saved: bool,
    reveal: bool,
    work: Option<Work>,
    error: Option<String>,
    notice: Option<String>,
}
impl Panel {
    pub fn new(home: PathBuf, executable: PathBuf) -> Self {
        let exists = home.join("setup.json").exists();
        let mut panel = Self {
            home: home.display().to_string(),
            executable,
            input: String::new(),
            setup: None,
            preview: None,
            verified: false,
            saved: false,
            reveal: false,
            work: None,
            error: None,
            notice: None,
        };
        if exists {
            panel.open();
        }
        panel
    }
    pub fn pause(&mut self) {
        self.work = None;
        if let Some(setup) = &mut self.setup {
            setup.phrase = None;
        }
        self.reveal = false;
        self.saved = false;
    }
    fn open(&mut self) {
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
        if let Err(e) = result {
            self.error = Some(e.to_string());
        } else {
            self.error = None;
        }
    }
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        self.poll();
        ui.heading("First-dialog pilot");
        ui.label("Create an invitation, or join one before creating your first account.");
        ui.small("One peer only. Connection and messaging come in the next step. Direct connections between different PCs and volunteer storage are not enabled in this pilot.");
        ui.separator();
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
                    ui.heading(if setup.record.joined {
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
        if let Some(notice) = &self.notice {
            ui.separator();
            ui.label(notice);
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }
}
impl Drop for Panel {
    fn drop(&mut self) {
        self.pause();
    }
}
