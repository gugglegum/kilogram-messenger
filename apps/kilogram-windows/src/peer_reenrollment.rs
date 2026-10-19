use anyhow::{Context, Result, ensure};
use eframe::egui;
use kilogram_runtime_ipc::{
    RuntimeIpcPeerReenrollmentArtifactId, RuntimeIpcPeerReenrollmentCommand,
    RuntimeIpcPeerReenrollmentContactId, RuntimeIpcPeerReenrollmentDurableOutcome,
    RuntimeIpcPeerReenrollmentExplicitApproval, RuntimeIpcPeerReenrollmentResponse,
    RuntimeIpcPeerReenrollmentStatus,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PeerReenrollmentUiAction {
    None,
    Refresh,
    Plan,
    Approve,
    Accept,
    GuidedRestart,
    RetryFreshVerification,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RestartExpectation {
    contact_id: RuntimeIpcPeerReenrollmentContactId,
    acceptance_receipt_id: RuntimeIpcPeerReenrollmentArtifactId,
    runtime_transition_id: RuntimeIpcPeerReenrollmentArtifactId,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum PeerReenrollmentRestartState {
    #[default]
    Idle,
    StopRequested(RestartExpectation),
    StartingFresh(RestartExpectation),
    Verifying(RestartExpectation),
    Verified {
        runtime_transition_id: RuntimeIpcPeerReenrollmentArtifactId,
    },
    Failed {
        detail: String,
        expectation: Option<RestartExpectation>,
    },
}

#[derive(Debug, Default)]
pub(crate) struct PeerReenrollmentView {
    pub(crate) contact_id: String,
    pub(crate) staged_invitation_id: String,
    pub(crate) confirmation_phrase: String,
    pub(crate) explicit_approval: bool,
    pub(crate) response: Option<RuntimeIpcPeerReenrollmentResponse>,
    descriptor_binding: Option<[u8; 32]>,
    pub(crate) restart: PeerReenrollmentRestartState,
}

impl PeerReenrollmentView {
    pub(crate) fn parsed_contact_id(&self) -> Result<RuntimeIpcPeerReenrollmentContactId> {
        self.contact_id
            .trim()
            .parse()
            .context("Peer re-enrollment contact ID must be exactly 64 lowercase hex characters")
    }

    pub(crate) fn install_response(
        &mut self,
        descriptor_binding: [u8; 32],
        response: RuntimeIpcPeerReenrollmentResponse,
    ) -> Result<()> {
        response.validate()?;
        ensure!(
            response.contact_id == self.parsed_contact_id()?,
            "Peer re-enrollment response names another opaque contact"
        );
        let changed = self.descriptor_binding != Some(descriptor_binding)
            || self.response.as_ref() != Some(&response);
        if changed {
            self.clear_confirmation();
            if !self.awaiting_fresh_runtime() {
                self.restart = PeerReenrollmentRestartState::Idle;
            }
        }
        self.descriptor_binding = Some(descriptor_binding);
        self.response = Some(response);
        Ok(())
    }

    pub(crate) fn invalidate_runtime_context(&mut self) {
        self.response = None;
        self.descriptor_binding = None;
        self.clear_confirmation();
        if !self.awaiting_fresh_runtime() {
            self.restart = PeerReenrollmentRestartState::Idle;
        }
    }

    pub(crate) fn invalidate_if_contact_changed(&mut self) {
        if self
            .response
            .as_ref()
            .is_some_and(|response| response.contact_id.as_str() != self.contact_id.trim())
        {
            self.invalidate_runtime_context();
        }
    }

    fn clear_confirmation(&mut self) {
        self.confirmation_phrase.clear();
        self.explicit_approval = false;
    }

    pub(crate) fn plan_command(&self) -> Result<RuntimeIpcPeerReenrollmentCommand> {
        let response = self
            .response
            .as_ref()
            .context("Refresh the signed peer re-enrollment status first")?;
        ensure!(
            response.status == RuntimeIpcPeerReenrollmentStatus::Ready,
            "The observed ceremony is not ready for planning"
        );
        let invitation = response
            .invitation
            .as_ref()
            .context("The ready response omitted its invitation summary")?;
        let quarantine = response
            .quarantine
            .as_ref()
            .context("The ready response omitted its durable quarantine")?;
        let selected: RuntimeIpcPeerReenrollmentArtifactId = self
            .staged_invitation_id
            .trim()
            .parse()
            .context("Select the exact 64-character staged invitation ID")?;
        ensure!(
            selected == invitation.invitation_id,
            "The selected staged invitation differs from the signed status"
        );
        Ok(RuntimeIpcPeerReenrollmentCommand::Plan {
            contact_id: response.contact_id.clone(),
            invitation_id: selected,
            quarantine_record_id: quarantine.record_id.clone(),
        })
    }

    pub(crate) fn approve_command(&self) -> Result<RuntimeIpcPeerReenrollmentCommand> {
        let response = self
            .response
            .as_ref()
            .context("Refresh the signed peer re-enrollment status first")?;
        ensure!(
            response.status == RuntimeIpcPeerReenrollmentStatus::ApprovalRequired,
            "The observed ceremony is not waiting for approval"
        );
        let plan = response
            .plan
            .as_ref()
            .context("The approval-required response omitted its exact plan")?;
        ensure!(
            self.confirmation_phrase == plan.confirmation_phrase,
            "Type the exact displayed confirmation phrase"
        );
        ensure!(
            self.explicit_approval,
            "Separate explicit approval is required"
        );
        Ok(RuntimeIpcPeerReenrollmentCommand::Approve {
            contact_id: response.contact_id.clone(),
            plan_id: plan.plan_id.clone(),
            confirmation_phrase: self.confirmation_phrase.clone(),
            explicit_approval: RuntimeIpcPeerReenrollmentExplicitApproval::UserConfirmed,
        })
    }

    pub(crate) fn accept_command(&self) -> Result<RuntimeIpcPeerReenrollmentCommand> {
        let response = self
            .response
            .as_ref()
            .context("Refresh the signed peer re-enrollment status first")?;
        ensure!(
            response.status == RuntimeIpcPeerReenrollmentStatus::ApprovedPrepared,
            "The observed ceremony has no durable approval ready to accept"
        );
        let quarantine = response
            .quarantine
            .as_ref()
            .context("The approved response omitted its durable quarantine")?;
        let RuntimeIpcPeerReenrollmentDurableOutcome::ApprovalPrepared { tombstone_id, .. } =
            &response.durable_outcome
        else {
            anyhow::bail!("The approved response omitted its exact tombstone");
        };
        Ok(RuntimeIpcPeerReenrollmentCommand::Accept {
            contact_id: response.contact_id.clone(),
            tombstone_id: tombstone_id.clone(),
            quarantine_record_id: quarantine.record_id.clone(),
        })
    }

    pub(crate) fn begin_guided_restart(&mut self) -> Result<()> {
        ensure!(
            matches!(self.restart, PeerReenrollmentRestartState::Idle),
            "A peer re-enrollment restart is already active"
        );
        let response = self
            .response
            .as_ref()
            .context("Refresh the signed peer re-enrollment status first")?;
        ensure!(
            response.status == RuntimeIpcPeerReenrollmentStatus::AcceptedRestartRequired
                && response.restart_required,
            "The exact accepted ceremony does not require a fresh runtime"
        );
        let RuntimeIpcPeerReenrollmentDurableOutcome::AcceptanceCommitted {
            acceptance_receipt_id,
            runtime_transition_id,
        } = &response.durable_outcome
        else {
            anyhow::bail!("The accepted response omitted its durable transition");
        };
        self.restart = PeerReenrollmentRestartState::StopRequested(RestartExpectation {
            contact_id: response.contact_id.clone(),
            acceptance_receipt_id: acceptance_receipt_id.clone(),
            runtime_transition_id: runtime_transition_id.clone(),
        });
        self.clear_confirmation();
        Ok(())
    }

    pub(crate) fn mark_runtime_exited_for_restart(&mut self) -> bool {
        let PeerReenrollmentRestartState::StopRequested(expectation) = &self.restart else {
            return false;
        };
        self.restart = PeerReenrollmentRestartState::StartingFresh(expectation.clone());
        self.response = None;
        self.descriptor_binding = None;
        true
    }

    pub(crate) fn awaiting_fresh_runtime(&self) -> bool {
        matches!(
            self.restart,
            PeerReenrollmentRestartState::StartingFresh(_)
                | PeerReenrollmentRestartState::Verifying(_)
        )
    }

    pub(crate) fn restart_in_progress(&self) -> bool {
        matches!(
            self.restart,
            PeerReenrollmentRestartState::StopRequested(_)
                | PeerReenrollmentRestartState::StartingFresh(_)
                | PeerReenrollmentRestartState::Verifying(_)
        )
    }

    pub(crate) fn can_retry_fresh_verification(&self) -> bool {
        matches!(
            self.restart,
            PeerReenrollmentRestartState::Failed {
                expectation: Some(_),
                ..
            }
        )
    }

    pub(crate) fn contact_for_fresh_verification(
        &mut self,
    ) -> Option<RuntimeIpcPeerReenrollmentContactId> {
        let PeerReenrollmentRestartState::StartingFresh(expectation) = &self.restart else {
            return None;
        };
        let expectation = expectation.clone();
        let contact_id = expectation.contact_id.clone();
        self.contact_id = contact_id.to_string();
        self.restart = PeerReenrollmentRestartState::Verifying(expectation);
        Some(contact_id)
    }

    pub(crate) fn verify_fresh_response(&mut self) -> Result<()> {
        let PeerReenrollmentRestartState::Verifying(expectation) = &self.restart else {
            anyhow::bail!("No peer re-enrollment fresh-start verification is pending");
        };
        let expectation = expectation.clone();
        let response = self
            .response
            .as_ref()
            .context("Fresh runtime returned no peer re-enrollment status")?;
        ensure!(
            response.contact_id == expectation.contact_id,
            "Fresh runtime status names another opaque contact"
        );
        ensure!(
            response.status == RuntimeIpcPeerReenrollmentStatus::Consumed
                && !response.restart_required,
            "Fresh runtime did not consume the exact accepted transition"
        );
        let RuntimeIpcPeerReenrollmentDurableOutcome::TransitionConsumed {
            acceptance_receipt_id,
            runtime_transition_id,
        } = &response.durable_outcome
        else {
            anyhow::bail!("Fresh runtime omitted the consumed transition proof");
        };
        ensure!(
            acceptance_receipt_id == &expectation.acceptance_receipt_id
                && runtime_transition_id == &expectation.runtime_transition_id,
            "Fresh runtime consumed a different peer re-enrollment transition"
        );
        self.restart = PeerReenrollmentRestartState::Verified {
            runtime_transition_id: runtime_transition_id.clone(),
        };
        Ok(())
    }

    pub(crate) fn fail_restart(&mut self, detail: String) {
        let expectation = match &self.restart {
            PeerReenrollmentRestartState::StopRequested(expectation)
            | PeerReenrollmentRestartState::StartingFresh(expectation)
            | PeerReenrollmentRestartState::Verifying(expectation) => Some(expectation.clone()),
            PeerReenrollmentRestartState::Failed { expectation, .. } => expectation.clone(),
            PeerReenrollmentRestartState::Idle | PeerReenrollmentRestartState::Verified { .. } => {
                None
            }
        };
        self.restart = PeerReenrollmentRestartState::Failed {
            detail,
            expectation,
        };
    }

    pub(crate) fn retry_fresh_verification(&mut self) -> Result<()> {
        let PeerReenrollmentRestartState::Failed {
            expectation: Some(expectation),
            ..
        } = &self.restart
        else {
            anyhow::bail!("No exact peer re-enrollment transition is retained for retry");
        };
        self.restart = PeerReenrollmentRestartState::StartingFresh(expectation.clone());
        self.response = None;
        self.descriptor_binding = None;
        Ok(())
    }

    pub(crate) fn draw(
        &mut self,
        ui: &mut egui::Ui,
        connected: bool,
        desktop_owned_runtime: bool,
        busy: bool,
        compact_id: fn(&str) -> String,
    ) -> PeerReenrollmentUiAction {
        let mut action = PeerReenrollmentUiAction::None;
        ui.strong("Peer re-enrollment ceremony");
        ui.small("Recover a quarantined peer only through signed, staged evidence. The desktop never receives identity secrets, transport tickets, endpoint bytes, reusable capabilities, or filesystem paths.");
        let contact_changed = ui
            .horizontal(|ui| {
                ui.label("Opaque contact ID");
                let changed = ui
                    .add_enabled(
                        !busy,
                        egui::TextEdit::singleline(&mut self.contact_id)
                            .desired_width(430.0)
                            .hint_text("64 lowercase hex characters"),
                    )
                    .changed();
                if ui
                    .add_enabled(
                        !busy && connected,
                        egui::Button::new("Refresh signed status"),
                    )
                    .clicked()
                {
                    action = PeerReenrollmentUiAction::Refresh;
                }
                changed
            })
            .inner;
        if contact_changed {
            self.invalidate_if_contact_changed();
        }

        let Some(response) = self.response.as_ref() else {
            ui.small("Refresh first. No mutation is offered from unverified or stale UI state.");
            self.draw_restart_state(ui, desktop_owned_runtime, busy, &mut action, compact_id);
            return action;
        };

        ui.horizontal_wrapped(|ui| {
            ui.label(format!("State: {:?}", response.status));
            if let Some(hazard) = response.hazard {
                ui.colored_label(
                    egui::Color32::from_rgb(239, 112, 112),
                    format!("Hazard: {hazard:?}"),
                );
            }
            ui.label(format!("Restart required: {}", response.restart_required));
        });
        if let Some(invitation) = response.invitation.as_ref() {
            ui.small(format!(
                "Invitation {} · replacement fingerprint {} · capability {} · authority {} · publication {} · expires {}",
                compact_id(invitation.invitation_id.as_str()),
                compact_id(invitation.replacement_device_fingerprint.as_str()),
                compact_id(invitation.required_capability_id.as_str()),
                invitation.authority_revision,
                invitation.publication_epoch,
                invitation.expires_at_unix_seconds
            ));
        }
        if let Some(quarantine) = response.quarantine.as_ref() {
            ui.small(format!(
                "Durable quarantine {} · sequence {} · reason {:?}",
                compact_id(quarantine.record_id.as_str()),
                quarantine.sequence,
                quarantine.reason
            ));
        }
        ui.small(format!(
            "Suppression: durable={} · this-process sync={} · this-process queues={}",
            response.suppression.durable_quarantine_present,
            response.suppression.process_local_sync_suppressed,
            response.suppression.process_local_queue_suppressed_count
        ));

        match response.status {
            RuntimeIpcPeerReenrollmentStatus::Ready => {
                ui.colored_label(
                    egui::Color32::from_rgb(246, 195, 93),
                    "Planning does not clear quarantine. Select the exact signed invitation shown above.",
                );
                ui.horizontal(|ui| {
                    ui.label("Exact invitation ID");
                    ui.add_enabled(
                        !busy,
                        egui::TextEdit::singleline(&mut self.staged_invitation_id)
                            .desired_width(430.0),
                    );
                });
                let selected = response.invitation.as_ref().is_some_and(|invitation| {
                    self.staged_invitation_id.trim() == invitation.invitation_id.as_str()
                });
                if ui
                    .add_enabled(
                        !busy && connected && selected,
                        egui::Button::new("Create exact plan"),
                    )
                    .clicked()
                {
                    action = PeerReenrollmentUiAction::Plan;
                }
            }
            RuntimeIpcPeerReenrollmentStatus::ApprovalRequired => {
                if let Some(plan) = response.plan.as_ref() {
                    ui.colored_label(
                        egui::Color32::from_rgb(246, 195, 93),
                        "Approval creates a durable tombstone but still does not accept the replacement Device.",
                    );
                    ui.monospace(format!("Type exactly: {}", plan.confirmation_phrase));
                    ui.horizontal(|ui| {
                        ui.label("Confirmation phrase");
                        ui.add_enabled(
                            !busy,
                            egui::TextEdit::singleline(&mut self.confirmation_phrase)
                                .desired_width(430.0),
                        );
                    });
                    ui.add_enabled(
                        !busy,
                        egui::Checkbox::new(
                            &mut self.explicit_approval,
                            "I separately approve this exact replacement plan",
                        ),
                    );
                    let exact = self.confirmation_phrase == plan.confirmation_phrase;
                    if ui
                        .add_enabled(
                            !busy && connected && exact && self.explicit_approval,
                            egui::Button::new("Approve exact plan"),
                        )
                        .clicked()
                    {
                        action = PeerReenrollmentUiAction::Approve;
                    }
                } else {
                    ui.colored_label(
                        egui::Color32::from_rgb(239, 112, 112),
                        "The signed approval state omitted its exact plan. Refresh is required; approval is disabled.",
                    );
                }
            }
            RuntimeIpcPeerReenrollmentStatus::ApprovedPrepared => {
                ui.colored_label(
                    egui::Color32::from_rgb(246, 195, 93),
                    "Approval is durable. Acceptance is a separate action and cannot be undone by this UI.",
                );
                if ui
                    .add_enabled(
                        !busy && connected,
                        egui::Button::new("Accept replacement Device"),
                    )
                    .clicked()
                {
                    action = PeerReenrollmentUiAction::Accept;
                }
            }
            RuntimeIpcPeerReenrollmentStatus::AcceptanceIncomplete => {
                ui.colored_label(
                    egui::Color32::from_rgb(239, 112, 112),
                    "Acceptance is incomplete. This is not success. Restart is forbidden; refresh after the durable operation is repaired or replayed.",
                );
            }
            RuntimeIpcPeerReenrollmentStatus::AcceptedRestartRequired => {
                ui.colored_label(
                    egui::Color32::from_rgb(246, 195, 93),
                    "Acceptance is committed, but the running process has not consumed it. No hot apply is permitted.",
                );
                if ui
                    .add_enabled(
                        !busy
                            && connected
                            && desktop_owned_runtime
                            && matches!(self.restart, PeerReenrollmentRestartState::Idle),
                        egui::Button::new("Restart owned runtime and verify exact transition"),
                    )
                    .clicked()
                {
                    action = PeerReenrollmentUiAction::GuidedRestart;
                }
                if !desktop_owned_runtime {
                    ui.small("Guided restart is available only for a runtime started by this desktop. Stop and restart an external runtime yourself, then refresh.");
                }
            }
            RuntimeIpcPeerReenrollmentStatus::Consumed => {
                ui.colored_label(
                    egui::Color32::from_rgb(92, 201, 137),
                    "The fresh runtime consumed the durable transition. Quarantine is not cleared by an implicit UI action.",
                );
            }
            RuntimeIpcPeerReenrollmentStatus::MissingInvitation => {
                ui.small(
                    "A durable quarantine exists, but no valid staged invitation is available.",
                );
            }
            RuntimeIpcPeerReenrollmentStatus::UnsafeOrDivergent => {
                ui.colored_label(
                    egui::Color32::from_rgb(239, 112, 112),
                    "The ceremony is unsafe or divergent. No mutation, retry, or quarantine clear is offered.",
                );
            }
        }
        ui.small(format!(
            "Durable outcome: {:?} · history recovery: {:?}",
            response.durable_outcome, response.history_recovery
        ));
        self.draw_restart_state(ui, desktop_owned_runtime, busy, &mut action, compact_id);
        action
    }

    fn draw_restart_state(
        &self,
        ui: &mut egui::Ui,
        desktop_owned_runtime: bool,
        busy: bool,
        action: &mut PeerReenrollmentUiAction,
        compact_id: fn(&str) -> String,
    ) {
        match &self.restart {
            PeerReenrollmentRestartState::Idle => {}
            PeerReenrollmentRestartState::StopRequested(_) => {
                ui.small("Guided restart: waiting for the old runtime to exit cleanly.");
            }
            PeerReenrollmentRestartState::StartingFresh(_) => {
                ui.small("Guided restart: starting a fresh runtime; no hot apply or hidden retry is used.");
            }
            PeerReenrollmentRestartState::Verifying(_) => {
                ui.small("Guided restart: verifying the exact consumed transition over authenticated IPC.");
            }
            PeerReenrollmentRestartState::Verified {
                runtime_transition_id,
            } => {
                ui.colored_label(
                    egui::Color32::from_rgb(92, 201, 137),
                    format!(
                        "Guided restart verified exact transition {}.",
                        compact_id(runtime_transition_id.as_str())
                    ),
                );
            }
            PeerReenrollmentRestartState::Failed {
                detail,
                expectation,
            } => {
                ui.colored_label(
                    egui::Color32::from_rgb(239, 112, 112),
                    format!("Guided restart stopped: {detail}"),
                );
                if expectation.is_some()
                    && ui
                        .add_enabled(
                            !busy && !desktop_owned_runtime,
                            egui::Button::new("Retry fresh runtime verification"),
                        )
                        .clicked()
                {
                    *action = PeerReenrollmentUiAction::RetryFreshVerification;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use kilogram_runtime_ipc::{
        RuntimeIpcPeerCapabilityQuarantineReason, RuntimeIpcPeerReenrollmentHistoryRecovery,
        RuntimeIpcPeerReenrollmentInvitationSummary, RuntimeIpcPeerReenrollmentPlanSummary,
        RuntimeIpcPeerReenrollmentQuarantineSummary, RuntimeIpcPeerReenrollmentSuppressionSummary,
    };

    use super::*;

    fn artifact(byte: u8) -> Result<RuntimeIpcPeerReenrollmentArtifactId> {
        RuntimeIpcPeerReenrollmentArtifactId::from_bytes([byte; 32])
    }

    fn contact() -> Result<RuntimeIpcPeerReenrollmentContactId> {
        "11".repeat(32).parse()
    }

    fn response(
        status: RuntimeIpcPeerReenrollmentStatus,
    ) -> Result<RuntimeIpcPeerReenrollmentResponse> {
        let invitation = RuntimeIpcPeerReenrollmentInvitationSummary {
            invitation_id: artifact(2)?,
            replacement_device_fingerprint: artifact(3)?,
            required_capability_id: artifact(4)?,
            authority_revision: 8,
            publication_epoch: 5,
            issued_at_unix_seconds: 100,
            expires_at_unix_seconds: 200,
        };
        let quarantine = RuntimeIpcPeerReenrollmentQuarantineSummary {
            record_id: artifact(5)?,
            sequence: 7,
            reason: RuntimeIpcPeerCapabilityQuarantineReason::RequiredInvitationCapabilityMissing,
        };
        let plan = RuntimeIpcPeerReenrollmentPlanSummary::new(artifact(6)?, 190)?;
        let durable_outcome = match status {
            RuntimeIpcPeerReenrollmentStatus::Ready => {
                RuntimeIpcPeerReenrollmentDurableOutcome::Unchanged
            }
            RuntimeIpcPeerReenrollmentStatus::ApprovalRequired => {
                RuntimeIpcPeerReenrollmentDurableOutcome::PlanPrepared {
                    plan_id: plan.plan_id.clone(),
                }
            }
            RuntimeIpcPeerReenrollmentStatus::ApprovedPrepared => {
                RuntimeIpcPeerReenrollmentDurableOutcome::ApprovalPrepared {
                    plan_id: plan.plan_id.clone(),
                    tombstone_id: artifact(7)?,
                }
            }
            RuntimeIpcPeerReenrollmentStatus::AcceptanceIncomplete => {
                RuntimeIpcPeerReenrollmentDurableOutcome::AcceptanceIncomplete {
                    acceptance_receipt_id: artifact(8)?,
                }
            }
            RuntimeIpcPeerReenrollmentStatus::AcceptedRestartRequired => {
                RuntimeIpcPeerReenrollmentDurableOutcome::AcceptanceCommitted {
                    acceptance_receipt_id: artifact(8)?,
                    runtime_transition_id: artifact(9)?,
                }
            }
            RuntimeIpcPeerReenrollmentStatus::Consumed => {
                RuntimeIpcPeerReenrollmentDurableOutcome::TransitionConsumed {
                    acceptance_receipt_id: artifact(8)?,
                    runtime_transition_id: artifact(9)?,
                }
            }
            _ => unreachable!("test response status"),
        };
        Ok(RuntimeIpcPeerReenrollmentResponse {
            contact_id: contact()?,
            status,
            hazard: None,
            invitation: Some(invitation),
            quarantine: Some(quarantine),
            plan: (!matches!(status, RuntimeIpcPeerReenrollmentStatus::Ready)).then_some(plan),
            durable_outcome,
            suppression: RuntimeIpcPeerReenrollmentSuppressionSummary {
                durable_quarantine_present: true,
                process_local_sync_suppressed: true,
                process_local_queue_suppressed_count: 1,
            },
            history_recovery: RuntimeIpcPeerReenrollmentHistoryRecovery::SeparateApprovalRequired,
            restart_required: status == RuntimeIpcPeerReenrollmentStatus::AcceptedRestartRequired,
        })
    }

    fn view_with(status: RuntimeIpcPeerReenrollmentStatus) -> Result<PeerReenrollmentView> {
        let mut view = PeerReenrollmentView {
            contact_id: contact()?.to_string(),
            ..PeerReenrollmentView::default()
        };
        view.install_response([1; 32], response(status)?)?;
        Ok(view)
    }

    #[test]
    fn plan_requires_exact_staged_invitation() -> Result<()> {
        let mut view = view_with(RuntimeIpcPeerReenrollmentStatus::Ready)?;
        assert!(view.plan_command().is_err());
        view.staged_invitation_id = artifact(10)?.to_string();
        assert!(view.plan_command().is_err());
        view.staged_invitation_id = artifact(2)?.to_string();
        assert!(matches!(
            view.plan_command()?,
            RuntimeIpcPeerReenrollmentCommand::Plan { .. }
        ));
        Ok(())
    }

    #[test]
    fn approval_requires_phrase_and_separate_confirmation() -> Result<()> {
        let mut view = view_with(RuntimeIpcPeerReenrollmentStatus::ApprovalRequired)?;
        let phrase = view
            .response
            .as_ref()
            .and_then(|response| response.plan.as_ref())
            .context("approval response plan")?
            .confirmation_phrase
            .clone();
        view.confirmation_phrase = phrase;
        assert!(view.approve_command().is_err());
        view.explicit_approval = true;
        assert!(matches!(
            view.approve_command()?,
            RuntimeIpcPeerReenrollmentCommand::Approve { .. }
        ));
        Ok(())
    }

    #[test]
    fn changed_status_or_descriptor_invalidates_confirmation() -> Result<()> {
        let mut view = view_with(RuntimeIpcPeerReenrollmentStatus::ApprovalRequired)?;
        view.confirmation_phrase = "KPR1-TEST".to_owned();
        view.explicit_approval = true;
        view.install_response(
            [1; 32],
            response(RuntimeIpcPeerReenrollmentStatus::ApprovedPrepared)?,
        )?;
        assert!(view.confirmation_phrase.is_empty());
        assert!(!view.explicit_approval);
        view.confirmation_phrase = "again".to_owned();
        view.explicit_approval = true;
        view.install_response(
            [2; 32],
            response(RuntimeIpcPeerReenrollmentStatus::ApprovedPrepared)?,
        )?;
        assert!(view.confirmation_phrase.is_empty());
        assert!(!view.explicit_approval);
        Ok(())
    }

    #[test]
    fn incomplete_acceptance_cannot_begin_restart() -> Result<()> {
        let mut view = view_with(RuntimeIpcPeerReenrollmentStatus::AcceptanceIncomplete)?;
        assert!(view.begin_guided_restart().is_err());
        assert_eq!(view.restart, PeerReenrollmentRestartState::Idle);
        Ok(())
    }

    #[test]
    fn guided_restart_verifies_exact_consumed_transition() -> Result<()> {
        let mut view = view_with(RuntimeIpcPeerReenrollmentStatus::AcceptedRestartRequired)?;
        view.begin_guided_restart()?;
        assert!(view.mark_runtime_exited_for_restart());
        assert_eq!(view.contact_for_fresh_verification(), Some(contact()?));
        view.install_response(
            [2; 32],
            response(RuntimeIpcPeerReenrollmentStatus::Consumed)?,
        )?;
        view.verify_fresh_response()?;
        assert!(matches!(
            view.restart,
            PeerReenrollmentRestartState::Verified { .. }
        ));
        Ok(())
    }
}
