#![forbid(unsafe_code)]

//! Dormant local-runtime integration for explicit old-history recovery.
//!
//! This crate owns the dependency direction between the offline recovery
//! execution engine and the authenticated runtime IPC vocabulary. Nothing in
//! this crate is registered with the active runtime server.

mod execution_api {
    pub use kilogram_peer_history_recovery_execution::{
        DurableRecipientRecoveryImportRoundIntent, RecipientHistoryImportStore,
        RecipientHistoryRecoveryCoordinator, RecipientHistoryRecoveryImportRoundExecutor,
        RecipientHistoryTransferInbox, RecipientRecoveryCoordinationPlan,
        RecipientRecoveryCoordinatorError, RecipientRecoveryImportRoundExecutionBudget,
        RecipientRecoveryImportRoundExecutionError, RecipientRecoveryImportRoundExecutionPlan,
        RecipientRecoveryImportRoundExecutionReceipt,
        RecipientRecoveryImportRoundIntentConsumeOutcome, RecipientRecoveryImportRoundIntentError,
        RecipientRecoveryImportRoundIntentProgressState, RecipientRecoveryImportRoundIntentStore,
        RecipientRecoveryImportRoundSource, RecipientRecoveryImportRoundStepDisposition,
        recipient_import_membership_digest,
    };
}

mod recipient_import_confirmation;
mod recipient_import_consumption_authorization;
mod recipient_import_consumption_mutation_adapter;
mod recipient_import_consumption_read_adapter;
mod recipient_import_consumption_response_dispatcher;
mod recipient_import_consumption_runtime_attachment;
mod recipient_import_consumption_runtime_service;
mod recipient_import_consumption_session_cache;
mod recipient_import_guided_recovery_host;
mod recipient_import_guided_recovery_launch_authorization;
mod recipient_import_guided_recovery_launch_service;
mod recipient_import_guided_recovery_launch_slot;
mod recipient_import_guided_session_controller;
mod recipient_import_guided_session_local_ux;
mod recipient_import_guided_session_start_producer;
mod recipient_import_mutation_adapter;
mod recipient_import_read_adapter;

pub use recipient_import_confirmation::{
    DurableRecipientImportConfirmation, MAX_DURABLE_RECIPIENT_IMPORT_CONFIRMATIONS,
    MAX_RECIPIENT_IMPORT_CONFIRMATION_LIFETIME_SECONDS, RecipientImportConfirmationCommitOutcome,
    RecipientImportConfirmationLedger, RecipientImportConfirmationLedgerError,
    RecipientImportConfirmationPlan, RecipientImportConfirmationStatus,
};
pub use recipient_import_consumption_authorization::{
    DurableRecipientImportConsumptionAuthorization,
    MAX_DURABLE_RECIPIENT_IMPORT_CONSUMPTION_AUTHORIZATIONS,
    MAX_RECIPIENT_IMPORT_CONSUMPTION_AUTHORIZATION_LIFETIME_SECONDS,
    RecipientImportConsumptionAuthorizationCommitOutcome,
    RecipientImportConsumptionAuthorizationLedger,
    RecipientImportConsumptionAuthorizationLedgerError,
    RecipientImportConsumptionAuthorizationPlan, RecipientImportConsumptionAuthorizationStatus,
    RecipientImportConsumptionAuthorizationSummary,
};
pub use recipient_import_consumption_mutation_adapter::{
    MAX_DURABLE_RECIPIENT_IMPORT_CONSUMPTION_MUTATIONS,
    RecipientHistoryRecoveryImportConsumptionMutationAdapter,
    RecipientImportConsumptionMutationAdapterDispatch,
    RecipientImportConsumptionMutationAdapterError, RecipientImportConsumptionMutationReadStatus,
};
pub use recipient_import_consumption_read_adapter::{
    RecipientHistoryRecoveryImportConsumptionReadAdapter,
    RecipientImportConsumptionReadAdapterDispatch, RecipientImportConsumptionReadAdapterError,
};
pub use recipient_import_consumption_response_dispatcher::{
    RecipientHistoryRecoveryImportConsumptionResponseDispatcher,
    RecipientImportConsumptionResponseDispatch, RecipientImportConsumptionResponseDispatcherError,
};
pub use recipient_import_consumption_runtime_attachment::{
    AuthenticatedGuidedRecipientImportCompletion, GuidedRecipientImportConsumptionContext,
    GuidedRecipientImportConsumptionContextProducer,
    MAX_RECIPIENT_IMPORT_CONSUMPTION_CONTEXT_LIFETIME_SECONDS,
    RecipientImportConsumptionRuntimeActor, RecipientImportConsumptionRuntimeActorError,
    RecipientImportConsumptionRuntimeAttachmentError,
    RecipientImportConsumptionRuntimeContextInstallOutcome,
    RecipientImportConsumptionRuntimeContextSlot,
};
pub use recipient_import_consumption_runtime_service::{
    RecipientImportConsumptionRuntimeContext, RecipientImportConsumptionRuntimeDispatch,
    RecipientImportConsumptionRuntimeService, RecipientImportConsumptionRuntimeServiceError,
};
pub use recipient_import_consumption_session_cache::{
    MAX_RECIPIENT_IMPORT_CONSUMPTION_PLAN_SESSIONS, RecipientImportConsumptionPlanSessionCache,
    RecipientImportConsumptionPlanSessionCacheError,
    RecipientImportConsumptionPlanSessionInsertOutcome,
};
pub use recipient_import_guided_recovery_host::{
    RecipientImportGuidedRecoveryHost, RecipientImportGuidedRecoveryHostCancel,
    RecipientImportGuidedRecoveryHostError, RecipientImportGuidedRecoveryHostInstallationOwner,
    RecipientImportGuidedRecoveryHostPrepare,
};
pub use recipient_import_guided_recovery_launch_authorization::{
    MAX_GUIDED_RECIPIENT_IMPORT_LAUNCH_AUTHORIZATION_LIFETIME_SECONDS,
    RecipientImportGuidedRecoveryLaunchApproval, RecipientImportGuidedRecoveryLaunchAuthorization,
    RecipientImportGuidedRecoveryLaunchAuthorizationError,
    RecipientImportGuidedRecoveryLaunchPreview,
};
pub use recipient_import_guided_recovery_launch_service::{
    RecipientImportGuidedRecoveryLaunchService, RecipientImportGuidedRecoveryLaunchServiceError,
};
pub use recipient_import_guided_recovery_launch_slot::{
    RecipientImportGuidedRecoveryLaunchSlot, RecipientImportGuidedRecoveryLaunchSlotApprove,
    RecipientImportGuidedRecoveryLaunchSlotCancel, RecipientImportGuidedRecoveryLaunchSlotError,
    RecipientImportGuidedRecoveryLaunchSlotState,
};
pub(crate) use recipient_import_guided_session_controller::RecipientImportGuidedRecoveryAssemblyStagingInput;
pub use recipient_import_guided_session_controller::{
    AuthenticatedGuidedRecipientImportSession,
    MAX_GUIDED_RECIPIENT_IMPORT_SESSION_LIFETIME_SECONDS,
    RecipientImportGuidedRecoveryAssemblyIssuer, RecipientImportGuidedSessionAttachment,
    RecipientImportGuidedSessionConfirmation, RecipientImportGuidedSessionConfirmationReceipt,
    RecipientImportGuidedSessionController, RecipientImportGuidedSessionControllerError,
    RecipientImportGuidedSessionMutation, RecipientImportGuidedSessionPhase,
    RecipientImportGuidedSessionPlan,
};
pub use recipient_import_guided_session_local_ux::{
    RecipientImportGuidedSessionLocalUxAdapter, RecipientImportGuidedSessionLocalUxApply,
    RecipientImportGuidedSessionLocalUxCancel, RecipientImportGuidedSessionLocalUxConfirm,
    RecipientImportGuidedSessionLocalUxError, RecipientImportGuidedSessionLocalUxPhase,
    RecipientImportGuidedSessionLocalUxRefresh, RecipientImportGuidedSessionLocalUxStart,
    RecipientImportGuidedSessionLocalUxView,
};
pub use recipient_import_guided_session_start_producer::{
    AuthenticatedGuidedRecipientImportSessionStartHandoff,
    RecipientImportGuidedSessionStartProducer, RecipientImportGuidedSessionStartProducerError,
};
pub use recipient_import_mutation_adapter::{
    DurableRecipientImportCompletedMutation, MAX_DURABLE_RECIPIENT_IMPORT_MUTATION_RECORDS,
    MAX_DURABLE_RECIPIENT_IMPORT_MUTATIONS, RecipientHistoryRecoveryImportMutationAdapter,
    RecipientImportMutationAdapterDispatch, RecipientImportMutationAdapterError,
    RecipientImportMutationReadStatus,
};
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
    RuntimeIpcRecipientImportCompletedMutationEvidence,
    RuntimeIpcRecipientImportConsumptionCommand, RuntimeIpcRecipientImportConsumptionHazard,
    RuntimeIpcRecipientImportConsumptionOutcome, RuntimeIpcRecipientImportConsumptionPlanSummary,
    RuntimeIpcRecipientImportConsumptionRequest, RuntimeIpcRecipientImportExecutionCommand,
    RuntimeIpcRecipientImportExecutionConfirmationSummary,
    RuntimeIpcRecipientImportExecutionHazard, RuntimeIpcRecipientImportExecutionOutcome,
    RuntimeIpcRecipientImportExecutionPlanSummary,
    RuntimeIpcRecipientImportExecutionReceiptSummary, RuntimeIpcRecipientImportExecutionRequest,
    RuntimeIpcRecipientImportExplicitApproval, RuntimeIpcRecipientImportIntentState,
    RuntimeIpcRecipientImportMembershipReference, RuntimeIpcRecipientImportStepDisposition,
    RuntimeIpcRecipientImportStepReceipt, RuntimeIpcRequestId,
    SignedRuntimeIpcRecipientImportConsumptionResponse,
};
