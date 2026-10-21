//! Dormant local-UX facade for the explicit guided recipient-import session.
//!
//! This module deliberately has no IPC, CLI, desktop, timer, filesystem-scan,
//! or network registration. It borrows the process-owned controller and only
//! projects privacy-safe state needed by a future local UI.

use anyhow::Error as ContractError;
use kilogram_runtime_ipc::{RuntimeIpcRecipientImportArtifactId, RuntimeIpcRequestId};
use thiserror::Error;

use crate::recipient_import_guided_session_controller::RecipientImportGuidedSessionLocalSnapshot;
use crate::{
    AuthenticatedGuidedRecipientImportSession, RecipientImportGuidedSessionConfirmation,
    RecipientImportGuidedSessionController, RecipientImportGuidedSessionControllerError,
    RecipientImportGuidedSessionMutation, RecipientImportGuidedSessionPhase,
};

const CONFIRMATION_NONCE_DOMAIN: &[u8] =
    b"kilogram:recipient-import-guided-session-local-ux:confirm:v1\0";
const MUTATION_NONCE_DOMAIN: &[u8] =
    b"kilogram:recipient-import-guided-session-local-ux:apply:v1\0";

/// Unforgeable start command handed over only by the authenticated recovery
/// owner in this crate. It intentionally has no public constructor, Clone,
/// Debug, Serialize, or Deserialize implementation.
pub struct RecipientImportGuidedSessionLocalUxStart {
    authority: AuthenticatedGuidedRecipientImportSession,
    lifetime_seconds: u64,
}

impl RecipientImportGuidedSessionLocalUxStart {
    #[allow(dead_code)]
    pub(crate) fn from_authenticated_recovery(
        authority: AuthenticatedGuidedRecipientImportSession,
        lifetime_seconds: u64,
    ) -> Self {
        Self {
            authority,
            lifetime_seconds,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientImportGuidedSessionLocalUxConfirm {
    session_id: RuntimeIpcRecipientImportArtifactId,
    confirmation_phrase: String,
}

impl RecipientImportGuidedSessionLocalUxConfirm {
    pub fn new(
        session_id: RuntimeIpcRecipientImportArtifactId,
        confirmation_phrase: impl Into<String>,
    ) -> Self {
        Self {
            session_id,
            confirmation_phrase: confirmation_phrase.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientImportGuidedSessionLocalUxApply {
    session_id: RuntimeIpcRecipientImportArtifactId,
}

impl RecipientImportGuidedSessionLocalUxApply {
    pub fn new(session_id: RuntimeIpcRecipientImportArtifactId) -> Self {
        Self { session_id }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientImportGuidedSessionLocalUxCancel {
    session_id: RuntimeIpcRecipientImportArtifactId,
}

impl RecipientImportGuidedSessionLocalUxCancel {
    pub fn new(session_id: RuntimeIpcRecipientImportArtifactId) -> Self {
        Self { session_id }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientImportGuidedSessionLocalUxRefresh {
    expected_session_id: Option<RuntimeIpcRecipientImportArtifactId>,
}

impl RecipientImportGuidedSessionLocalUxRefresh {
    pub fn current() -> Self {
        Self {
            expected_session_id: None,
        }
    }

    pub fn exact(session_id: RuntimeIpcRecipientImportArtifactId) -> Self {
        Self {
            expected_session_id: Some(session_id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecipientImportGuidedSessionLocalUxPhase {
    Idle,
    Planned,
    Confirmed,
    Attached,
}

/// Privacy-safe local presentation model.
///
/// The view contains no state path, Account/Device identifier, descriptor,
/// coordinator identifier, membership reference/snapshot, signing material,
/// request ID, nonce, or authenticated start token. It has no serialization
/// implementation and is not an IPC contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientImportGuidedSessionLocalUxView {
    phase: RecipientImportGuidedSessionLocalUxPhase,
    session_id: Option<RuntimeIpcRecipientImportArtifactId>,
    completed_source_count: u8,
    total_source_count: u8,
    planned_step_count: u8,
    imported_event_count: u64,
    confirmation_phrase: Option<String>,
    expires_at_unix_seconds: Option<u64>,
    complete: bool,
}

impl RecipientImportGuidedSessionLocalUxView {
    pub fn phase(&self) -> RecipientImportGuidedSessionLocalUxPhase {
        self.phase
    }

    pub fn session_id(&self) -> Option<&RuntimeIpcRecipientImportArtifactId> {
        self.session_id.as_ref()
    }

    pub fn completed_source_count(&self) -> u8 {
        self.completed_source_count
    }

    pub fn total_source_count(&self) -> u8 {
        self.total_source_count
    }

    pub fn planned_step_count(&self) -> u8 {
        self.planned_step_count
    }

    pub fn imported_event_count(&self) -> u64 {
        self.imported_event_count
    }

    pub fn confirmation_phrase(&self) -> Option<&str> {
        self.confirmation_phrase.as_deref()
    }

    pub fn expires_at_unix_seconds(&self) -> Option<u64> {
        self.expires_at_unix_seconds
    }

    pub fn complete(&self) -> bool {
        self.complete
    }

    pub fn can_confirm(&self) -> bool {
        self.phase == RecipientImportGuidedSessionLocalUxPhase::Planned
    }

    pub fn can_apply(&self) -> bool {
        self.phase == RecipientImportGuidedSessionLocalUxPhase::Confirmed
    }

    pub fn can_cancel(&self) -> bool {
        self.phase != RecipientImportGuidedSessionLocalUxPhase::Idle
    }
}

pub struct RecipientImportGuidedSessionLocalUxAdapter<'controller, 'identity> {
    controller: &'controller mut RecipientImportGuidedSessionController<'identity>,
}

impl<'controller, 'identity> RecipientImportGuidedSessionLocalUxAdapter<'controller, 'identity> {
    pub fn new(
        controller: &'controller mut RecipientImportGuidedSessionController<'identity>,
    ) -> Self {
        Self { controller }
    }

    pub fn start(
        &mut self,
        command: RecipientImportGuidedSessionLocalUxStart,
        now_unix_seconds: u64,
    ) -> Result<RecipientImportGuidedSessionLocalUxView, RecipientImportGuidedSessionLocalUxError>
    {
        let RecipientImportGuidedSessionLocalUxStart {
            authority,
            lifetime_seconds,
        } = command;
        let plan = self
            .controller
            .begin(authority, now_unix_seconds, lifetime_seconds)?;
        self.project_or_cancel(plan.session_id())
    }

    pub fn confirm(
        &mut self,
        command: RecipientImportGuidedSessionLocalUxConfirm,
        now_unix_seconds: u64,
    ) -> Result<RecipientImportGuidedSessionLocalUxView, RecipientImportGuidedSessionLocalUxError>
    {
        self.require_exact_phase(
            &command.session_id,
            RecipientImportGuidedSessionPhase::Planned,
        )?;
        let request_id = match RuntimeIpcRequestId::generate() {
            Ok(request_id) => request_id,
            Err(error) => {
                self.cancel_exact_best_effort(&command.session_id);
                return Err(error.into());
            }
        };
        let confirmation_nonce =
            match transition_nonce(CONFIRMATION_NONCE_DOMAIN, &command.session_id, &request_id) {
                Ok(nonce) => nonce,
                Err(error) => {
                    self.cancel_exact_best_effort(&command.session_id);
                    return Err(error);
                }
            };
        let session_id = command.session_id.clone();
        self.controller.confirm(
            RecipientImportGuidedSessionConfirmation::new(
                command.session_id,
                request_id,
                confirmation_nonce,
                command.confirmation_phrase,
            ),
            now_unix_seconds,
        )?;
        self.project_or_cancel(&session_id)
    }

    pub fn apply(
        &mut self,
        command: RecipientImportGuidedSessionLocalUxApply,
        now_unix_seconds: u64,
    ) -> Result<RecipientImportGuidedSessionLocalUxView, RecipientImportGuidedSessionLocalUxError>
    {
        self.require_exact_phase(
            &command.session_id,
            RecipientImportGuidedSessionPhase::Confirmed,
        )?;
        let request_id = match RuntimeIpcRequestId::generate() {
            Ok(request_id) => request_id,
            Err(error) => {
                self.cancel_exact_best_effort(&command.session_id);
                return Err(error.into());
            }
        };
        let mutation_nonce =
            match transition_nonce(MUTATION_NONCE_DOMAIN, &command.session_id, &request_id) {
                Ok(nonce) => nonce,
                Err(error) => {
                    self.cancel_exact_best_effort(&command.session_id);
                    return Err(error);
                }
            };
        let session_id = command.session_id.clone();
        self.controller.apply(
            RecipientImportGuidedSessionMutation::new(
                command.session_id,
                request_id,
                mutation_nonce,
            ),
            now_unix_seconds,
        )?;
        self.project_or_cancel(&session_id)
    }

    pub fn cancel(
        &mut self,
        command: RecipientImportGuidedSessionLocalUxCancel,
    ) -> Result<RecipientImportGuidedSessionLocalUxView, RecipientImportGuidedSessionLocalUxError>
    {
        self.require_exact_session(&command.session_id)?;
        self.controller.cancel(&command.session_id)?;
        self.project()
    }

    pub fn refresh(
        &mut self,
        command: RecipientImportGuidedSessionLocalUxRefresh,
        now_unix_seconds: u64,
    ) -> Result<RecipientImportGuidedSessionLocalUxView, RecipientImportGuidedSessionLocalUxError>
    {
        self.controller.purge_expired(now_unix_seconds)?;
        if let (Some(expected), Some(actual)) = (
            command.expected_session_id.as_ref(),
            self.controller.active_session_id(),
        ) && expected != actual
        {
            return Err(RecipientImportGuidedSessionLocalUxError::StaleView);
        }
        self.project()
    }

    pub fn opened_network(&self) -> bool {
        false
    }

    fn project(
        &self,
    ) -> Result<RecipientImportGuidedSessionLocalUxView, RecipientImportGuidedSessionLocalUxError>
    {
        project_snapshot(self.controller.local_ux_snapshot()?)
    }

    fn project_or_cancel(
        &mut self,
        session_id: &RuntimeIpcRecipientImportArtifactId,
    ) -> Result<RecipientImportGuidedSessionLocalUxView, RecipientImportGuidedSessionLocalUxError>
    {
        match self.project() {
            Ok(view) => Ok(view),
            Err(error) => {
                self.cancel_exact_best_effort(session_id);
                Err(error)
            }
        }
    }

    fn cancel_exact_best_effort(&mut self, session_id: &RuntimeIpcRecipientImportArtifactId) {
        if self.controller.active_session_id() == Some(session_id) {
            let _ = self.controller.cancel(session_id);
        }
    }

    fn require_exact_session(
        &self,
        session_id: &RuntimeIpcRecipientImportArtifactId,
    ) -> Result<(), RecipientImportGuidedSessionLocalUxError> {
        if self.controller.active_session_id() != Some(session_id) {
            return Err(RecipientImportGuidedSessionLocalUxError::StaleView);
        }
        Ok(())
    }

    fn require_exact_phase(
        &self,
        session_id: &RuntimeIpcRecipientImportArtifactId,
        phase: RecipientImportGuidedSessionPhase,
    ) -> Result<(), RecipientImportGuidedSessionLocalUxError> {
        self.require_exact_session(session_id)?;
        if self.controller.phase() != phase {
            return Err(RecipientImportGuidedSessionLocalUxError::UnexpectedPhase);
        }
        Ok(())
    }
}

fn project_snapshot(
    snapshot: RecipientImportGuidedSessionLocalSnapshot,
) -> Result<RecipientImportGuidedSessionLocalUxView, RecipientImportGuidedSessionLocalUxError> {
    let view = match snapshot {
        RecipientImportGuidedSessionLocalSnapshot::Idle => {
            RecipientImportGuidedSessionLocalUxView {
                phase: RecipientImportGuidedSessionLocalUxPhase::Idle,
                session_id: None,
                completed_source_count: 0,
                total_source_count: 0,
                planned_step_count: 0,
                imported_event_count: 0,
                confirmation_phrase: None,
                expires_at_unix_seconds: None,
                complete: false,
            }
        }
        RecipientImportGuidedSessionLocalSnapshot::Planned {
            session_id,
            completed_source_count,
            total_source_count,
            planned_step_count,
            confirmation_phrase,
            expires_at_unix_seconds,
        } => RecipientImportGuidedSessionLocalUxView {
            phase: RecipientImportGuidedSessionLocalUxPhase::Planned,
            session_id: Some(session_id),
            completed_source_count,
            total_source_count,
            planned_step_count,
            imported_event_count: 0,
            confirmation_phrase: Some(confirmation_phrase),
            expires_at_unix_seconds: Some(expires_at_unix_seconds),
            complete: false,
        },
        RecipientImportGuidedSessionLocalSnapshot::Confirmed {
            session_id,
            completed_source_count,
            total_source_count,
            planned_step_count,
            expires_at_unix_seconds,
        } => RecipientImportGuidedSessionLocalUxView {
            phase: RecipientImportGuidedSessionLocalUxPhase::Confirmed,
            session_id: Some(session_id),
            completed_source_count,
            total_source_count,
            planned_step_count,
            imported_event_count: 0,
            confirmation_phrase: None,
            expires_at_unix_seconds: Some(expires_at_unix_seconds),
            complete: false,
        },
        RecipientImportGuidedSessionLocalSnapshot::Attached {
            session_id,
            final_completed_source_count,
            total_source_count,
            imported_event_count,
            complete,
            expires_at_unix_seconds,
        } => RecipientImportGuidedSessionLocalUxView {
            phase: RecipientImportGuidedSessionLocalUxPhase::Attached,
            session_id: Some(session_id),
            completed_source_count: final_completed_source_count,
            total_source_count,
            planned_step_count: 0,
            imported_event_count,
            confirmation_phrase: None,
            expires_at_unix_seconds: Some(expires_at_unix_seconds),
            complete,
        },
    };
    validate_view(&view)?;
    Ok(view)
}

fn validate_view(
    view: &RecipientImportGuidedSessionLocalUxView,
) -> Result<(), RecipientImportGuidedSessionLocalUxError> {
    let idle = view.phase == RecipientImportGuidedSessionLocalUxPhase::Idle;
    if idle
        != (view.session_id.is_none()
            && view.expires_at_unix_seconds.is_none()
            && view.confirmation_phrase.is_none()
            && view.completed_source_count == 0
            && view.total_source_count == 0
            && view.planned_step_count == 0
            && view.imported_event_count == 0
            && !view.complete)
    {
        return Err(RecipientImportGuidedSessionLocalUxError::InvalidView);
    }
    if !idle
        && (view.session_id.is_none()
            || view.expires_at_unix_seconds.is_none()
            || view.completed_source_count > view.total_source_count)
    {
        return Err(RecipientImportGuidedSessionLocalUxError::InvalidView);
    }
    if (view.phase == RecipientImportGuidedSessionLocalUxPhase::Planned)
        != view.confirmation_phrase.is_some()
    {
        return Err(RecipientImportGuidedSessionLocalUxError::InvalidView);
    }
    if view.complete
        != (view.phase == RecipientImportGuidedSessionLocalUxPhase::Attached
            && view.completed_source_count == view.total_source_count)
    {
        return Err(RecipientImportGuidedSessionLocalUxError::InvalidView);
    }
    Ok(())
}

fn transition_nonce(
    domain: &[u8],
    session_id: &RuntimeIpcRecipientImportArtifactId,
    request_id: &RuntimeIpcRequestId,
) -> Result<RuntimeIpcRecipientImportArtifactId, RecipientImportGuidedSessionLocalUxError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(session_id.as_str().as_bytes());
    hasher.update(request_id.as_bytes());
    RuntimeIpcRecipientImportArtifactId::from_bytes(*hasher.finalize().as_bytes())
        .map_err(Into::into)
}

#[derive(Debug, Error)]
pub enum RecipientImportGuidedSessionLocalUxError {
    #[error("guided recipient-import local UX view is stale")]
    StaleView,
    #[error("guided recipient-import local UX command is not valid in the current phase")]
    UnexpectedPhase,
    #[error("guided recipient-import local UX view is invalid")]
    InvalidView,
    #[error(transparent)]
    Controller(Box<RecipientImportGuidedSessionControllerError>),
    #[error(transparent)]
    Contract(Box<ContractError>),
}

impl From<RecipientImportGuidedSessionControllerError>
    for RecipientImportGuidedSessionLocalUxError
{
    fn from(error: RecipientImportGuidedSessionControllerError) -> Self {
        Self::Controller(Box::new(error))
    }
}

impl From<ContractError> for RecipientImportGuidedSessionLocalUxError {
    fn from(error: ContractError) -> Self {
        Self::Contract(Box::new(error))
    }
}
