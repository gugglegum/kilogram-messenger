//! Dormant process-owned custody for one pending guided-recovery launch.
//!
//! The slot owns at most one complete M1.2.54 authorization and, after exact
//! approval, at most one installed recovery host. It deliberately has no IPC,
//! CLI, desktop, timer, background, persistence, filesystem-discovery, or
//! network registration.

use thiserror::Error;

use crate::{
    RecipientImportGuidedRecoveryHost, RecipientImportGuidedRecoveryHostCancel,
    RecipientImportGuidedRecoveryHostError, RecipientImportGuidedRecoveryHostPrepare,
    RecipientImportGuidedRecoveryLaunchApproval, RecipientImportGuidedRecoveryLaunchAuthorization,
    RecipientImportGuidedRecoveryLaunchAuthorizationError,
    RecipientImportGuidedRecoveryLaunchPreview, RecipientImportGuidedSessionLocalUxApply,
    RecipientImportGuidedSessionLocalUxConfirm, RecipientImportGuidedSessionLocalUxRefresh,
    RecipientImportGuidedSessionLocalUxView,
};

/// Safe process-local state projection. It contains no recovery identity,
/// authority, host, session, hand-off, signer, path, or membership material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecipientImportGuidedRecoveryLaunchSlotState {
    Empty,
    PendingAuthorization,
    HostInstalled,
}

/// Typed approval routed through the process-owned slot.
///
/// The host hand-off lifetime is intentionally supplied only at the final
/// one-shot transition. It cannot manufacture or replace launch authority.
pub struct RecipientImportGuidedRecoveryLaunchSlotApprove {
    approval: RecipientImportGuidedRecoveryLaunchApproval,
    host_handoff_lifetime_seconds: u64,
}

impl RecipientImportGuidedRecoveryLaunchSlotApprove {
    pub fn new(confirmation_phrase: impl Into<String>, host_handoff_lifetime_seconds: u64) -> Self {
        Self {
            approval: RecipientImportGuidedRecoveryLaunchApproval::new(confirmation_phrase),
            host_handoff_lifetime_seconds,
        }
    }
}

/// Typed cancellation of the one pending authorization.
pub struct RecipientImportGuidedRecoveryLaunchSlotCancel {
    _private: (),
}

impl RecipientImportGuidedRecoveryLaunchSlotCancel {
    pub fn pending() -> Self {
        Self { _private: () }
    }
}

/// Owns the complete pre-launch grant and the installed host for one process.
///
/// The slot is intentionally neither Clone nor serializable. Its constructor
/// and install seam are crate-private while the feature is dormant. A process
/// restart therefore recreates no pending authorization and no installed host.
pub struct RecipientImportGuidedRecoveryLaunchSlot<'identity> {
    pending_authorization: Option<RecipientImportGuidedRecoveryLaunchAuthorization<'identity>>,
    installed_host: Option<RecipientImportGuidedRecoveryHost<'identity>>,
}

impl<'identity> RecipientImportGuidedRecoveryLaunchSlot<'identity> {
    /// Dormant in-process constructor. Ordinary product startup does not call
    /// it yet; a later explicit integration may own exactly one empty slot.
    #[allow(dead_code)]
    pub(crate) fn new_empty() -> Self {
        Self {
            pending_authorization: None,
            installed_host: None,
        }
    }

    /// Installs one complete authorization without permitting replacement.
    ///
    /// The value is accepted by ownership. Rejection for existing custody,
    /// clock regression, or expiry drops the incoming authorization and never
    /// returns it to the caller.
    #[allow(dead_code)]
    pub(crate) fn install(
        &mut self,
        authorization: RecipientImportGuidedRecoveryLaunchAuthorization<'identity>,
        now_unix_seconds: u64,
    ) -> Result<(), RecipientImportGuidedRecoveryLaunchSlotError> {
        if self.pending_authorization.is_some() {
            return Err(RecipientImportGuidedRecoveryLaunchSlotError::PendingAlreadyInstalled);
        }
        if self.installed_host.is_some() {
            return Err(RecipientImportGuidedRecoveryLaunchSlotError::HostAlreadyInstalled);
        }
        authorization.validate_pending_at(now_unix_seconds)?;
        self.pending_authorization = Some(authorization);
        Ok(())
    }

    pub fn state(&self) -> RecipientImportGuidedRecoveryLaunchSlotState {
        if self.installed_host.is_some() {
            RecipientImportGuidedRecoveryLaunchSlotState::HostInstalled
        } else if self.pending_authorization.is_some() {
            RecipientImportGuidedRecoveryLaunchSlotState::PendingAuthorization
        } else {
            RecipientImportGuidedRecoveryLaunchSlotState::Empty
        }
    }

    /// Borrows only the safe M1.2.54 preview. No retained authority leaves the
    /// slot and a host cannot be recovered through this projection.
    pub fn preview(
        &self,
    ) -> Result<
        &RecipientImportGuidedRecoveryLaunchPreview,
        RecipientImportGuidedRecoveryLaunchSlotError,
    > {
        self.pending_authorization
            .as_ref()
            .map(RecipientImportGuidedRecoveryLaunchAuthorization::preview)
            .ok_or(RecipientImportGuidedRecoveryLaunchSlotError::PendingUnavailable)
    }

    /// Takes pending custody before any fallible M1.2.54 approval or host
    /// installation work. Success installs exactly one host inside this slot;
    /// mismatch, expiry, clock regression, issuer failure, and host failure all
    /// leave no reusable pending authorization.
    pub fn approve(
        &mut self,
        command: RecipientImportGuidedRecoveryLaunchSlotApprove,
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportGuidedRecoveryLaunchSlotState,
        RecipientImportGuidedRecoveryLaunchSlotError,
    > {
        let authorization = self
            .pending_authorization
            .take()
            .ok_or(RecipientImportGuidedRecoveryLaunchSlotError::PendingUnavailable)?;
        if self.installed_host.is_some() {
            return Err(RecipientImportGuidedRecoveryLaunchSlotError::HostAlreadyInstalled);
        }
        let owner = authorization.approve(command.approval, now_unix_seconds)?;
        let host = owner.install(now_unix_seconds, command.host_handoff_lifetime_seconds)?;
        self.installed_host = Some(host);
        Ok(RecipientImportGuidedRecoveryLaunchSlotState::HostInstalled)
    }

    /// Explicitly takes and destroys pending custody. The typed command carries
    /// no identifier or authority material and cannot target an installed host.
    pub fn cancel(
        &mut self,
        _command: RecipientImportGuidedRecoveryLaunchSlotCancel,
    ) -> Result<
        RecipientImportGuidedRecoveryLaunchSlotState,
        RecipientImportGuidedRecoveryLaunchSlotError,
    > {
        let authorization = self
            .pending_authorization
            .take()
            .ok_or(RecipientImportGuidedRecoveryLaunchSlotError::PendingUnavailable)?;
        authorization.cancel();
        Ok(RecipientImportGuidedRecoveryLaunchSlotState::Empty)
    }

    /// Routes a typed prepare command to the installed host without exposing
    /// or borrowing that host. Empty and pending slots are rejected before
    /// controller or durable work can begin.
    pub fn host_prepare(
        &mut self,
        command: RecipientImportGuidedRecoveryHostPrepare,
        now_unix_seconds: u64,
    ) -> Result<RecipientImportGuidedSessionLocalUxView, RecipientImportGuidedRecoveryLaunchSlotError>
    {
        self.installed_host_mut()?
            .prepare(command, now_unix_seconds)
            .map_err(Into::into)
    }

    /// Routes a typed confirmation command while retaining process ownership
    /// of the installed host on success and failure.
    pub fn host_confirm(
        &mut self,
        command: RecipientImportGuidedSessionLocalUxConfirm,
        now_unix_seconds: u64,
    ) -> Result<RecipientImportGuidedSessionLocalUxView, RecipientImportGuidedRecoveryLaunchSlotError>
    {
        self.installed_host_mut()?
            .confirm(command, now_unix_seconds)
            .map_err(Into::into)
    }

    /// Routes a typed apply command and returns only the existing privacy-safe
    /// local UX projection.
    pub fn host_apply(
        &mut self,
        command: RecipientImportGuidedSessionLocalUxApply,
        now_unix_seconds: u64,
    ) -> Result<RecipientImportGuidedSessionLocalUxView, RecipientImportGuidedRecoveryLaunchSlotError>
    {
        self.installed_host_mut()?
            .apply(command, now_unix_seconds)
            .map_err(Into::into)
    }

    /// Routes a typed host cancellation. This is distinct from cancelling the
    /// pending launch authorization and cannot target an uninstalled host.
    pub fn host_cancel(
        &mut self,
        command: RecipientImportGuidedRecoveryHostCancel,
        now_unix_seconds: u64,
    ) -> Result<RecipientImportGuidedSessionLocalUxView, RecipientImportGuidedRecoveryLaunchSlotError>
    {
        self.installed_host_mut()?
            .cancel(command, now_unix_seconds)
            .map_err(Into::into)
    }

    /// Routes a typed refresh without making the host, controller, authority,
    /// session context, or start hand-off observable.
    pub fn host_refresh(
        &mut self,
        command: RecipientImportGuidedSessionLocalUxRefresh,
        now_unix_seconds: u64,
    ) -> Result<RecipientImportGuidedSessionLocalUxView, RecipientImportGuidedRecoveryLaunchSlotError>
    {
        self.installed_host_mut()?
            .refresh(command, now_unix_seconds)
            .map_err(Into::into)
    }

    pub fn opened_network(&self) -> bool {
        false
    }

    fn installed_host_mut(
        &mut self,
    ) -> Result<
        &mut RecipientImportGuidedRecoveryHost<'identity>,
        RecipientImportGuidedRecoveryLaunchSlotError,
    > {
        if self.pending_authorization.is_some() {
            return Err(RecipientImportGuidedRecoveryLaunchSlotError::HostUnavailable);
        }
        self.installed_host
            .as_mut()
            .ok_or(RecipientImportGuidedRecoveryLaunchSlotError::HostUnavailable)
    }
}

#[derive(Debug, Error)]
pub enum RecipientImportGuidedRecoveryLaunchSlotError {
    #[error("guided recovery launch slot already owns a pending authorization")]
    PendingAlreadyInstalled,
    #[error("guided recovery launch slot already owns an installed host")]
    HostAlreadyInstalled,
    #[error("guided recovery launch slot has no pending authorization")]
    PendingUnavailable,
    #[error("guided recovery launch slot has no installed host")]
    HostUnavailable,
    #[error(transparent)]
    Authorization(Box<RecipientImportGuidedRecoveryLaunchAuthorizationError>),
    #[error(transparent)]
    Host(Box<RecipientImportGuidedRecoveryHostError>),
}

impl From<RecipientImportGuidedRecoveryLaunchAuthorizationError>
    for RecipientImportGuidedRecoveryLaunchSlotError
{
    fn from(error: RecipientImportGuidedRecoveryLaunchAuthorizationError) -> Self {
        Self::Authorization(Box::new(error))
    }
}

impl From<RecipientImportGuidedRecoveryHostError> for RecipientImportGuidedRecoveryLaunchSlotError {
    fn from(error: RecipientImportGuidedRecoveryHostError) -> Self {
        Self::Host(Box::new(error))
    }
}
