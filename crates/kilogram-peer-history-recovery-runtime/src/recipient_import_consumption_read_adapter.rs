//! Dormant read-only adapter for planning explicit import-intent consumption.
//!
//! The adapter reopens the exact completed mutation, reloads caller-owned live
//! membership state, and observes the still-active complete intent. It issues
//! only an ephemeral plan. Commit remains rejected until a separate mutation
//! adapter owns the durable consumption transition.

use anyhow::Error as ContractError;
use kilogram_runtime_ipc::{
    RuntimeIpcRecipientImportCompletedMutationEvidence,
    RuntimeIpcRecipientImportConsumptionCommand, RuntimeIpcRecipientImportConsumptionHazard,
    RuntimeIpcRecipientImportConsumptionOutcome, RuntimeIpcRecipientImportConsumptionPlanSummary,
    RuntimeIpcRecipientImportConsumptionRequest,
};
use thiserror::Error;

use crate::{
    RecipientHistoryRecoveryImportMutationAdapter, RecipientImportMembershipLoader,
    RecipientImportMutationAdapterError, RecipientImportMutationReadStatus,
    execution_api::{
        RecipientHistoryImportStore, RecipientHistoryTransferInbox,
        RecipientRecoveryCoordinationPlan, RecipientRecoveryImportRoundIntentError,
        RecipientRecoveryImportRoundIntentProgressState, RecipientRecoveryImportRoundIntentStore,
    },
    recipient_import_read_adapter::{load_exact_memberships, transient_sources},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientImportConsumptionReadAdapterDispatch {
    outcome: RuntimeIpcRecipientImportConsumptionOutcome,
    ephemeral_plan: Option<RuntimeIpcRecipientImportConsumptionPlanSummary>,
    loaded_membership_count: usize,
}

impl RecipientImportConsumptionReadAdapterDispatch {
    pub fn outcome(&self) -> &RuntimeIpcRecipientImportConsumptionOutcome {
        &self.outcome
    }

    pub fn ephemeral_plan(&self) -> Option<&RuntimeIpcRecipientImportConsumptionPlanSummary> {
        self.ephemeral_plan.as_ref()
    }

    pub fn loaded_membership_count(&self) -> usize {
        self.loaded_membership_count
    }

    pub fn persisted_consumption_plan(&self) -> bool {
        false
    }

    pub fn consumed_intent(&self) -> bool {
        false
    }

    pub fn opened_network(&self) -> bool {
        false
    }

    pub fn scheduled_background_work(&self) -> bool {
        false
    }
}

pub struct RecipientHistoryRecoveryImportConsumptionReadAdapter;

impl RecipientHistoryRecoveryImportConsumptionReadAdapter {
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch<L: RecipientImportMembershipLoader>(
        request: &RuntimeIpcRecipientImportConsumptionRequest,
        mutations: &RecipientHistoryRecoveryImportMutationAdapter,
        store: &RecipientRecoveryImportRoundIntentStore,
        current: &RecipientRecoveryCoordinationPlan,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        loader: &L,
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportConsumptionReadAdapterDispatch,
        RecipientImportConsumptionReadAdapterError,
    > {
        let (evidence, memberships, consumption_nonce, requested_lifetime_seconds) =
            match request.command() {
                RuntimeIpcRecipientImportConsumptionCommand::Plan {
                    evidence,
                    memberships,
                    consumption_nonce,
                    requested_lifetime_seconds,
                } => (
                    evidence,
                    memberships,
                    consumption_nonce,
                    *requested_lifetime_seconds,
                ),
                RuntimeIpcRecipientImportConsumptionCommand::Commit { .. } => {
                    return Ok(reject(
                        request,
                        RuntimeIpcRecipientImportConsumptionHazard::CommitRejected,
                    ));
                }
            };

        let completed = match mutations.inspect_by_nonce(&evidence.mutation_nonce)? {
            RecipientImportMutationReadStatus::Absent => {
                return Ok(reject(
                    request,
                    RuntimeIpcRecipientImportConsumptionHazard::CompletedMutationMissing,
                ));
            }
            RecipientImportMutationReadStatus::Prepared => {
                return Ok(reject(
                    request,
                    RuntimeIpcRecipientImportConsumptionHazard::IntentIncomplete,
                ));
            }
            RecipientImportMutationReadStatus::Completed(completed) => completed,
        };
        let exact_evidence =
            RuntimeIpcRecipientImportCompletedMutationEvidence::from_completed_receipt(
                completed.completed_record_id().clone(),
                completed.membership_set_digest().clone(),
                completed.receipt(),
            )?;
        if &exact_evidence != evidence {
            return Ok(reject(
                request,
                RuntimeIpcRecipientImportConsumptionHazard::CompletedMutationChanged,
            ));
        }
        if current.plan_id() != evidence.final_coordinator_plan_id.to_bytes()?
            || current.sequence() != evidence.final_coordinator_sequence
        {
            return Ok(reject(
                request,
                RuntimeIpcRecipientImportConsumptionHazard::CoordinatorChanged,
            ));
        }

        let durable = match store.load_by_intent_id(evidence.intent_id.to_bytes()?)? {
            Some(durable) => durable,
            None => {
                return Ok(reject(
                    request,
                    RuntimeIpcRecipientImportConsumptionHazard::NoActiveIntent,
                ));
            }
        };
        if durable.consumed() {
            return Ok(reject(
                request,
                RuntimeIpcRecipientImportConsumptionHazard::IntentAlreadyConsumed,
            ));
        }
        if store.inspect_active()?.as_ref() != Some(&durable) {
            return Ok(reject(
                request,
                RuntimeIpcRecipientImportConsumptionHazard::NoActiveIntent,
            ));
        }

        let loaded = match load_exact_memberships(memberships, loader) {
            Ok(loaded) => loaded,
            Err(_) => {
                return Ok(reject(
                    request,
                    RuntimeIpcRecipientImportConsumptionHazard::CompletedMutationChanged,
                ));
            }
        };
        if loaded.len() != usize::from(evidence.total_source_count) {
            return Ok(reject(
                request,
                RuntimeIpcRecipientImportConsumptionHazard::CompletedMutationChanged,
            ));
        }
        let sources = transient_sources(memberships, &loaded);
        let progress = match store.inspect_progress(&durable, current, &sources, inbox, imports) {
            Ok(progress) => progress,
            Err(_) => {
                return Ok(reject(
                    request,
                    RuntimeIpcRecipientImportConsumptionHazard::IntentIncomplete,
                ));
            }
        };
        let imported_event_count = u64::try_from(progress.imported_event_count)
            .map_err(|_| RecipientImportConsumptionReadAdapterError::ProjectionOverflow)?;
        if progress.state != RecipientRecoveryImportRoundIntentProgressState::Complete
            || progress.coordinator_refresh_required
            || progress.completed_source_count != usize::from(evidence.completed_source_count)
            || progress.total_source_count != usize::from(evidence.total_source_count)
            || imported_event_count != evidence.imported_event_count
            || progress.coordinator_plan_id != evidence.final_coordinator_plan_id.to_bytes()?
            || progress.coordinator_sequence != evidence.final_coordinator_sequence
            || progress.contains_membership_snapshot
            || progress.contains_message_bodies
            || progress.contains_plaintext
            || progress.opened_network
            || progress.scheduled_background_work
        {
            return Ok(reject(
                request,
                RuntimeIpcRecipientImportConsumptionHazard::IntentIncomplete,
            ));
        }

        let plan = RuntimeIpcRecipientImportConsumptionPlanSummary::issue(
            evidence,
            consumption_nonce.clone(),
            now_unix_seconds,
            requested_lifetime_seconds,
        )?;
        Ok(RecipientImportConsumptionReadAdapterDispatch {
            outcome: RuntimeIpcRecipientImportConsumptionOutcome::Planned(plan.clone()),
            ephemeral_plan: Some(plan),
            loaded_membership_count: loaded.len(),
        })
    }
}

fn reject(
    request: &RuntimeIpcRecipientImportConsumptionRequest,
    hazard: RuntimeIpcRecipientImportConsumptionHazard,
) -> RecipientImportConsumptionReadAdapterDispatch {
    RecipientImportConsumptionReadAdapterDispatch {
        outcome: RuntimeIpcRecipientImportConsumptionOutcome::Rejected {
            intent_id: request.command().intent_id().clone(),
            hazard,
        },
        ephemeral_plan: None,
        loaded_membership_count: 0,
    }
}

#[derive(Debug, Error)]
pub enum RecipientImportConsumptionReadAdapterError {
    #[error("recipient import consumption read projection exceeds its bounds")]
    ProjectionOverflow,
    #[error(transparent)]
    Mutation(#[from] RecipientImportMutationAdapterError),
    #[error(transparent)]
    Intent(Box<RecipientRecoveryImportRoundIntentError>),
    #[error(transparent)]
    Contract(#[from] ContractError),
}

impl From<RecipientRecoveryImportRoundIntentError> for RecipientImportConsumptionReadAdapterError {
    fn from(error: RecipientRecoveryImportRoundIntentError) -> Self {
        Self::Intent(Box::new(error))
    }
}
