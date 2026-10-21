//! Dormant process owner for one explicit guided recipient-import ceremony.
//!
//! The host is the only public local-UX surface that composes the controller,
//! sealed start hand-off, one-shot producer, and UX adapter. It deliberately
//! has no IPC, CLI, desktop, timer, background, filesystem-discovery, or
//! network registration.

use std::path::Path;

use kilogram_identity::{AccountId, DeviceIdentity};
use kilogram_runtime_ipc::RuntimeIpcRecipientImportArtifactId;
use thiserror::Error;

use crate::{
    AuthenticatedGuidedRecipientImportSession,
    AuthenticatedGuidedRecipientImportSessionStartHandoff, RecipientImportGuidedSessionController,
    RecipientImportGuidedSessionControllerError, RecipientImportGuidedSessionLocalUxAdapter,
    RecipientImportGuidedSessionLocalUxApply, RecipientImportGuidedSessionLocalUxConfirm,
    RecipientImportGuidedSessionLocalUxError, RecipientImportGuidedSessionLocalUxRefresh,
    RecipientImportGuidedSessionLocalUxView, RecipientImportGuidedSessionPhase,
    RecipientImportGuidedSessionStartProducer, RecipientImportGuidedSessionStartProducerError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecipientImportGuidedRecoveryHostPrepare {
    session_lifetime_seconds: u64,
}

impl RecipientImportGuidedRecoveryHostPrepare {
    pub fn new(session_lifetime_seconds: u64) -> Self {
        Self {
            session_lifetime_seconds,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RecipientImportGuidedRecoveryHostCancelTarget {
    PendingHandoff,
    ActiveSession(RuntimeIpcRecipientImportArtifactId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientImportGuidedRecoveryHostCancel {
    target: RecipientImportGuidedRecoveryHostCancelTarget,
}

impl RecipientImportGuidedRecoveryHostCancel {
    pub fn pending_handoff() -> Self {
        Self {
            target: RecipientImportGuidedRecoveryHostCancelTarget::PendingHandoff,
        }
    }

    pub fn active_session(session_id: RuntimeIpcRecipientImportArtifactId) -> Self {
        Self {
            target: RecipientImportGuidedRecoveryHostCancelTarget::ActiveSession(session_id),
        }
    }
}

/// Owns all ephemeral authority for at most one guided recovery ceremony.
///
/// The host is intentionally neither Clone nor serializable. Process restart
/// constructs an empty host; no pending hand-off or active session authority
/// is recovered from durable state.
pub struct RecipientImportGuidedRecoveryHost<'identity> {
    controller: RecipientImportGuidedSessionController<'identity>,
    pending_handoff: Option<AuthenticatedGuidedRecipientImportSessionStartHandoff>,
}

impl<'identity> RecipientImportGuidedRecoveryHost<'identity> {
    pub fn new(
        state_directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        signer: &'identity DeviceIdentity,
    ) -> Result<Self, RecipientImportGuidedRecoveryHostError> {
        Ok(Self {
            controller: RecipientImportGuidedSessionController::new(
                state_directory,
                owner_account_id,
                signer,
            )?,
            pending_handoff: None,
        })
    }

    /// Installs authority from the already authenticated in-process owner.
    ///
    /// This is crate-private so a UI, IPC request, CLI argument, environment
    /// variable, or filesystem scan cannot construct pending authority. The
    /// incoming value is consumed on every result. Existing pending or active
    /// custody is never displaced by a replacement attempt.
    #[allow(dead_code)]
    pub(crate) fn install_authenticated_handoff(
        &mut self,
        authority: AuthenticatedGuidedRecipientImportSession,
        issued_at_unix_seconds: u64,
        lifetime_seconds: u64,
    ) -> Result<(), RecipientImportGuidedRecoveryHostError> {
        self.purge_expired_custody(issued_at_unix_seconds)?;
        if self.pending_handoff.is_some() {
            return Err(RecipientImportGuidedRecoveryHostError::PendingHandoffAlreadyInstalled);
        }
        if self.controller.phase() != RecipientImportGuidedSessionPhase::Idle
            || self.controller.context_attached()
        {
            return Err(RecipientImportGuidedRecoveryHostError::CeremonyAlreadyActive);
        }

        self.controller.preflight_local_ux_start(
            &authority,
            issued_at_unix_seconds,
            lifetime_seconds,
        )?;
        let handoff =
            AuthenticatedGuidedRecipientImportSessionStartHandoff::from_authenticated_recovery(
                authority,
                issued_at_unix_seconds,
                lifetime_seconds,
            )?;
        self.pending_handoff = Some(handoff);
        Ok(())
    }

    pub fn prepare(
        &mut self,
        command: RecipientImportGuidedRecoveryHostPrepare,
        now_unix_seconds: u64,
    ) -> Result<RecipientImportGuidedSessionLocalUxView, RecipientImportGuidedRecoveryHostError>
    {
        let mut handoff = self
            .pending_handoff
            .take()
            .ok_or(RecipientImportGuidedRecoveryHostError::PendingHandoffUnavailable)?;
        let start = RecipientImportGuidedSessionStartProducer::new(&mut self.controller).produce(
            &mut handoff,
            now_unix_seconds,
            command.session_lifetime_seconds,
        )?;
        RecipientImportGuidedSessionLocalUxAdapter::new(&mut self.controller)
            .start(start, now_unix_seconds)
            .map_err(Into::into)
    }

    pub fn confirm(
        &mut self,
        command: RecipientImportGuidedSessionLocalUxConfirm,
        now_unix_seconds: u64,
    ) -> Result<RecipientImportGuidedSessionLocalUxView, RecipientImportGuidedRecoveryHostError>
    {
        RecipientImportGuidedSessionLocalUxAdapter::new(&mut self.controller)
            .confirm(command, now_unix_seconds)
            .map_err(Into::into)
    }

    pub fn apply(
        &mut self,
        command: RecipientImportGuidedSessionLocalUxApply,
        now_unix_seconds: u64,
    ) -> Result<RecipientImportGuidedSessionLocalUxView, RecipientImportGuidedRecoveryHostError>
    {
        RecipientImportGuidedSessionLocalUxAdapter::new(&mut self.controller)
            .apply(command, now_unix_seconds)
            .map_err(Into::into)
    }

    pub fn cancel(
        &mut self,
        command: RecipientImportGuidedRecoveryHostCancel,
        now_unix_seconds: u64,
    ) -> Result<RecipientImportGuidedSessionLocalUxView, RecipientImportGuidedRecoveryHostError>
    {
        self.purge_expired_custody(now_unix_seconds)?;
        match command.target {
            RecipientImportGuidedRecoveryHostCancelTarget::PendingHandoff => {
                if self.controller.phase() != RecipientImportGuidedSessionPhase::Idle {
                    return Err(RecipientImportGuidedRecoveryHostError::StaleCancel);
                }
                let mut handoff = self
                    .pending_handoff
                    .take()
                    .ok_or(RecipientImportGuidedRecoveryHostError::PendingHandoffUnavailable)?;
                if !handoff.cancel() {
                    return Err(RecipientImportGuidedRecoveryHostError::PendingHandoffUnavailable);
                }
                RecipientImportGuidedSessionLocalUxAdapter::new(&mut self.controller)
                    .refresh(
                        RecipientImportGuidedSessionLocalUxRefresh::current(),
                        now_unix_seconds,
                    )
                    .map_err(Into::into)
            }
            RecipientImportGuidedRecoveryHostCancelTarget::ActiveSession(session_id) => {
                if self.pending_handoff.is_some() {
                    return Err(RecipientImportGuidedRecoveryHostError::StaleCancel);
                }
                RecipientImportGuidedSessionLocalUxAdapter::new(&mut self.controller)
                    .cancel(crate::RecipientImportGuidedSessionLocalUxCancel::new(
                        session_id,
                    ))
                    .map_err(Into::into)
            }
        }
    }

    pub fn refresh(
        &mut self,
        command: RecipientImportGuidedSessionLocalUxRefresh,
        now_unix_seconds: u64,
    ) -> Result<RecipientImportGuidedSessionLocalUxView, RecipientImportGuidedRecoveryHostError>
    {
        self.purge_expired_custody(now_unix_seconds)?;
        RecipientImportGuidedSessionLocalUxAdapter::new(&mut self.controller)
            .refresh(command, now_unix_seconds)
            .map_err(Into::into)
    }

    pub fn opened_network(&self) -> bool {
        false
    }

    #[allow(dead_code)]
    pub(crate) fn has_pending_handoff(&self) -> bool {
        self.pending_handoff.is_some()
    }

    fn purge_expired_custody(
        &mut self,
        now_unix_seconds: u64,
    ) -> Result<(), RecipientImportGuidedRecoveryHostError> {
        let pending_expired = self
            .pending_handoff
            .as_ref()
            .is_some_and(|handoff| now_unix_seconds >= handoff.expires_at_unix_seconds());
        if pending_expired && let Some(mut handoff) = self.pending_handoff.take() {
            let _ = handoff.cancel();
        }
        self.controller.purge_expired(now_unix_seconds)?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum RecipientImportGuidedRecoveryHostError {
    #[error("guided recipient-import host already owns a pending start hand-off")]
    PendingHandoffAlreadyInstalled,
    #[error("guided recipient-import host already owns an active ceremony")]
    CeremonyAlreadyActive,
    #[error("guided recipient-import host has no pending start hand-off")]
    PendingHandoffUnavailable,
    #[error("guided recipient-import host cancel command is stale")]
    StaleCancel,
    #[error(transparent)]
    StartProducer(Box<RecipientImportGuidedSessionStartProducerError>),
    #[error(transparent)]
    LocalUx(Box<RecipientImportGuidedSessionLocalUxError>),
    #[error(transparent)]
    Controller(Box<RecipientImportGuidedSessionControllerError>),
}

impl From<RecipientImportGuidedSessionStartProducerError>
    for RecipientImportGuidedRecoveryHostError
{
    fn from(error: RecipientImportGuidedSessionStartProducerError) -> Self {
        Self::StartProducer(Box::new(error))
    }
}

impl From<RecipientImportGuidedSessionLocalUxError> for RecipientImportGuidedRecoveryHostError {
    fn from(error: RecipientImportGuidedSessionLocalUxError) -> Self {
        Self::LocalUx(Box::new(error))
    }
}

impl From<RecipientImportGuidedSessionControllerError> for RecipientImportGuidedRecoveryHostError {
    fn from(error: RecipientImportGuidedSessionControllerError) -> Self {
        Self::Controller(Box::new(error))
    }
}
