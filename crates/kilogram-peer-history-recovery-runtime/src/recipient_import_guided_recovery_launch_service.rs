//! Dormant process-owned facade for one guided-recovery launch ceremony.
//!
//! One service owns exactly one launch slot. The raw slot, pending launch
//! authorization, installed host, issuer, authenticated session, and assembly
//! fragments never leave this facade. Construction and assembly staging stay
//! crate-private until a later explicit product integration exists.

use thiserror::Error;

use crate::{
    RecipientImportGuidedRecoveryAssemblyStagingInput, RecipientImportGuidedRecoveryHostCancel,
    RecipientImportGuidedRecoveryHostPrepare, RecipientImportGuidedRecoveryLaunchPreview,
    RecipientImportGuidedRecoveryLaunchSlot, RecipientImportGuidedRecoveryLaunchSlotApprove,
    RecipientImportGuidedRecoveryLaunchSlotCancel, RecipientImportGuidedRecoveryLaunchSlotError,
    RecipientImportGuidedRecoveryLaunchSlotState, RecipientImportGuidedSessionLocalUxApply,
    RecipientImportGuidedSessionLocalUxConfirm, RecipientImportGuidedSessionLocalUxRefresh,
    RecipientImportGuidedSessionLocalUxView,
};

/// Process-local owner for one complete guided-recovery launch lifecycle.
///
/// The service is intentionally neither Clone nor serializable. It has no
/// public constructor and no slot accessor, replacement seam, persistence, or
/// restart recovery. Ordinary runtime startup does not create it.
pub struct RecipientImportGuidedRecoveryLaunchService<'identity> {
    slot: RecipientImportGuidedRecoveryLaunchSlot<'identity>,
}

impl<'identity> RecipientImportGuidedRecoveryLaunchService<'identity> {
    /// Dormant crate-internal constructor. A later explicit integration may
    /// own one instance, but ordinary product startup does not call it.
    #[allow(dead_code)]
    pub(crate) fn new_dormant() -> Self {
        Self {
            slot: RecipientImportGuidedRecoveryLaunchSlot::new_empty(),
        }
    }

    /// Consumes one opaque authenticated assembly and stages it inside the
    /// owned slot. No ready issuer, authorization, or assembly fragment is
    /// ever returned to the caller.
    #[allow(dead_code)]
    pub(crate) fn stage_authenticated_assembly(
        &mut self,
        assembly: RecipientImportGuidedRecoveryAssemblyStagingInput<'identity>,
        issued_at_unix_seconds: u64,
        lifetime_seconds: u64,
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportGuidedRecoveryLaunchSlotState,
        RecipientImportGuidedRecoveryLaunchServiceError,
    > {
        self.slot
            .stage_from_authenticated_assembly(
                assembly,
                issued_at_unix_seconds,
                lifetime_seconds,
                now_unix_seconds,
            )
            .map_err(Into::into)
    }

    pub fn state(&self) -> RecipientImportGuidedRecoveryLaunchSlotState {
        self.slot.state()
    }

    pub fn preview(
        &self,
    ) -> Result<
        &RecipientImportGuidedRecoveryLaunchPreview,
        RecipientImportGuidedRecoveryLaunchServiceError,
    > {
        self.slot.preview().map_err(Into::into)
    }

    pub fn approve(
        &mut self,
        command: RecipientImportGuidedRecoveryLaunchSlotApprove,
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportGuidedRecoveryLaunchSlotState,
        RecipientImportGuidedRecoveryLaunchServiceError,
    > {
        self.slot
            .approve(command, now_unix_seconds)
            .map_err(Into::into)
    }

    pub fn cancel(
        &mut self,
        command: RecipientImportGuidedRecoveryLaunchSlotCancel,
    ) -> Result<
        RecipientImportGuidedRecoveryLaunchSlotState,
        RecipientImportGuidedRecoveryLaunchServiceError,
    > {
        self.slot.cancel(command).map_err(Into::into)
    }

    pub fn host_prepare(
        &mut self,
        command: RecipientImportGuidedRecoveryHostPrepare,
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportGuidedSessionLocalUxView,
        RecipientImportGuidedRecoveryLaunchServiceError,
    > {
        self.slot
            .host_prepare(command, now_unix_seconds)
            .map_err(Into::into)
    }

    pub fn host_confirm(
        &mut self,
        command: RecipientImportGuidedSessionLocalUxConfirm,
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportGuidedSessionLocalUxView,
        RecipientImportGuidedRecoveryLaunchServiceError,
    > {
        self.slot
            .host_confirm(command, now_unix_seconds)
            .map_err(Into::into)
    }

    pub fn host_apply(
        &mut self,
        command: RecipientImportGuidedSessionLocalUxApply,
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportGuidedSessionLocalUxView,
        RecipientImportGuidedRecoveryLaunchServiceError,
    > {
        self.slot
            .host_apply(command, now_unix_seconds)
            .map_err(Into::into)
    }

    pub fn host_cancel(
        &mut self,
        command: RecipientImportGuidedRecoveryHostCancel,
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportGuidedSessionLocalUxView,
        RecipientImportGuidedRecoveryLaunchServiceError,
    > {
        self.slot
            .host_cancel(command, now_unix_seconds)
            .map_err(Into::into)
    }

    pub fn host_refresh(
        &mut self,
        command: RecipientImportGuidedSessionLocalUxRefresh,
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportGuidedSessionLocalUxView,
        RecipientImportGuidedRecoveryLaunchServiceError,
    > {
        self.slot
            .host_refresh(command, now_unix_seconds)
            .map_err(Into::into)
    }

    pub fn opened_network(&self) -> bool {
        self.slot.opened_network()
    }
}

#[derive(Debug, Error)]
pub enum RecipientImportGuidedRecoveryLaunchServiceError {
    #[error(transparent)]
    Slot(Box<RecipientImportGuidedRecoveryLaunchSlotError>),
}

impl From<RecipientImportGuidedRecoveryLaunchSlotError>
    for RecipientImportGuidedRecoveryLaunchServiceError
{
    fn from(error: RecipientImportGuidedRecoveryLaunchSlotError) -> Self {
        Self::Slot(Box::new(error))
    }
}
