#![forbid(unsafe_code)]

//! Dormant local-runtime integration for explicit old-history recovery.
//!
//! This crate owns the dependency direction between the offline recovery
//! execution engine and the authenticated runtime IPC vocabulary. Nothing in
//! this crate is registered with the active runtime server.

mod execution_api {
    pub use kilogram_peer_history_recovery_execution::{
        RecipientHistoryImportStore, RecipientHistoryRecoveryImportRoundExecutor,
        RecipientHistoryTransferInbox, RecipientRecoveryCoordinationPlan,
        RecipientRecoveryImportRoundExecutionBudget, RecipientRecoveryImportRoundExecutionError,
        RecipientRecoveryImportRoundExecutionPlan, RecipientRecoveryImportRoundIntentProgressState,
        RecipientRecoveryImportRoundIntentStore, RecipientRecoveryImportRoundSource,
        recipient_import_membership_digest,
    };
}

mod recipient_import_read_adapter;

pub use recipient_import_read_adapter::{
    LoadedRecipientImportMembership, RecipientHistoryRecoveryImportReadAdapter,
    RecipientImportMembershipLoad, RecipientImportMembershipLoader,
    RecipientImportReadAdapterDispatch, RecipientImportReadAdapterError,
    RecipientImportReadKnownPlan,
};

// Re-export the exact contract vocabulary used by callers and by the real
// signed-fixture integration tests in the lower execution crate.
pub use kilogram_runtime_ipc::{
    RuntimeIpcDescriptor, RuntimeIpcRecipientImportArtifactId,
    RuntimeIpcRecipientImportExecutionCommand, RuntimeIpcRecipientImportExecutionHazard,
    RuntimeIpcRecipientImportExecutionOutcome, RuntimeIpcRecipientImportExecutionRequest,
    RuntimeIpcRecipientImportIntentState, RuntimeIpcRecipientImportMembershipReference,
    RuntimeIpcRequestId,
};
