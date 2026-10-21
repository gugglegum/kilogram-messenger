//! Dormant, transport-independent read adapter for recipient import execution.
//!
//! Only `Plan` and `Status` are handled. Membership authority remains owned by
//! the caller and is reloaded through an exact-reference callback. The adapter
//! neither persists execution plans nor accepts confirmation or mutation
//! commands.

use anyhow::Error as ContractError;
use kilogram_identity::ConversationMembershipSnapshot;
use kilogram_runtime_ipc::{
    RuntimeIpcRecipientImportArtifactId, RuntimeIpcRecipientImportExecutionCommand,
    RuntimeIpcRecipientImportExecutionHazard, RuntimeIpcRecipientImportExecutionOutcome,
    RuntimeIpcRecipientImportExecutionPlanSummary, RuntimeIpcRecipientImportExecutionRequest,
    RuntimeIpcRecipientImportIntentState, RuntimeIpcRecipientImportMembershipReference,
    RuntimeIpcRecipientImportStatusSummary, recipient_import_confirmation_phrase,
};
use thiserror::Error;

use crate::execution_api::{
    RecipientHistoryImportStore, RecipientHistoryRecoveryImportRoundExecutor,
    RecipientHistoryTransferInbox, RecipientRecoveryCoordinationPlan,
    RecipientRecoveryImportRoundExecutionBudget, RecipientRecoveryImportRoundExecutionError,
    RecipientRecoveryImportRoundExecutionPlan, RecipientRecoveryImportRoundIntentProgressState,
    RecipientRecoveryImportRoundIntentStore, RecipientRecoveryImportRoundSource,
    recipient_import_membership_digest,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedRecipientImportMembership {
    authority_reference_id: RuntimeIpcRecipientImportArtifactId,
    authenticated_context_id: RuntimeIpcRecipientImportArtifactId,
    membership: ConversationMembershipSnapshot,
}

impl LoadedRecipientImportMembership {
    pub fn new(
        authority_reference_id: RuntimeIpcRecipientImportArtifactId,
        authenticated_context_id: RuntimeIpcRecipientImportArtifactId,
        membership: ConversationMembershipSnapshot,
    ) -> Self {
        Self {
            authority_reference_id,
            authenticated_context_id,
            membership,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipientImportMembershipLoad {
    Exact(LoadedRecipientImportMembership),
    Missing,
    Ambiguous,
}

pub trait RecipientImportMembershipLoader {
    fn load_membership(
        &self,
        reference: &RuntimeIpcRecipientImportMembershipReference,
    ) -> RecipientImportMembershipLoad;
}

impl<F> RecipientImportMembershipLoader for F
where
    F: Fn(&RuntimeIpcRecipientImportMembershipReference) -> RecipientImportMembershipLoad,
{
    fn load_membership(
        &self,
        reference: &RuntimeIpcRecipientImportMembershipReference,
    ) -> RecipientImportMembershipLoad {
        self(reference)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RecipientImportReadKnownPlan<'a> {
    plan: &'a RecipientRecoveryImportRoundExecutionPlan,
    membership_references: &'a [RuntimeIpcRecipientImportMembershipReference],
    membership_set_digest: &'a RuntimeIpcRecipientImportArtifactId,
}

#[derive(Clone, Debug)]
pub struct RecipientImportReadAdapterDispatch {
    outcome: RuntimeIpcRecipientImportExecutionOutcome,
    execution_plan: Option<RecipientRecoveryImportRoundExecutionPlan>,
    membership_references: Vec<RuntimeIpcRecipientImportMembershipReference>,
    membership_set_digest: Option<RuntimeIpcRecipientImportArtifactId>,
}

impl RecipientImportReadAdapterDispatch {
    pub fn outcome(&self) -> &RuntimeIpcRecipientImportExecutionOutcome {
        &self.outcome
    }

    pub fn execution_plan(&self) -> Option<&RecipientRecoveryImportRoundExecutionPlan> {
        self.execution_plan.as_ref()
    }

    pub fn planned_summary(&self) -> Option<&RuntimeIpcRecipientImportExecutionPlanSummary> {
        match &self.outcome {
            RuntimeIpcRecipientImportExecutionOutcome::Planned(summary) => Some(summary),
            _ => None,
        }
    }

    // The same source is included by the lower execution crate's signed-fixture
    // tests, where the confirmation integration module is deliberately absent.
    #[allow(dead_code)]
    pub(crate) fn exact_membership_references(
        &self,
    ) -> &[RuntimeIpcRecipientImportMembershipReference] {
        &self.membership_references
    }

    pub fn known_plan(&self) -> Option<RecipientImportReadKnownPlan<'_>> {
        Some(RecipientImportReadKnownPlan {
            plan: self.execution_plan.as_ref()?,
            membership_references: &self.membership_references,
            membership_set_digest: self.membership_set_digest.as_ref()?,
        })
    }

    pub fn contains_membership_snapshot(&self) -> bool {
        false
    }

    pub fn persisted_execution_plan(&self) -> bool {
        false
    }

    pub fn granted_import_authority(&self) -> bool {
        false
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn planned_for_test(
        summary: RuntimeIpcRecipientImportExecutionPlanSummary,
        membership_references: Vec<RuntimeIpcRecipientImportMembershipReference>,
    ) -> Self {
        let membership_set_digest = summary.membership_set_digest.clone();
        Self {
            outcome: RuntimeIpcRecipientImportExecutionOutcome::Planned(summary),
            execution_plan: None,
            membership_references,
            membership_set_digest: Some(membership_set_digest),
        }
    }
}

pub struct RecipientHistoryRecoveryImportReadAdapter;

impl RecipientHistoryRecoveryImportReadAdapter {
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch<L: RecipientImportMembershipLoader>(
        request: &RuntimeIpcRecipientImportExecutionRequest,
        store: &RecipientRecoveryImportRoundIntentStore,
        current: &RecipientRecoveryCoordinationPlan,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        loader: &L,
        known_plan: Option<RecipientImportReadKnownPlan<'_>>,
    ) -> Result<RecipientImportReadAdapterDispatch, RecipientImportReadAdapterError> {
        match request.command() {
            RuntimeIpcRecipientImportExecutionCommand::Plan {
                intent_id,
                coordinator_plan_id,
                coordinator_sequence,
                max_steps,
                memberships,
            } => {
                if coordinator_plan_id.to_bytes()? != current.plan_id()
                    || *coordinator_sequence != current.sequence()
                {
                    return Ok(reject(
                        request,
                        RuntimeIpcRecipientImportExecutionHazard::CoordinatorChanged,
                    ));
                }
                let durable = match store.load_by_intent_id(intent_id.to_bytes()?) {
                    Ok(Some(durable)) => durable,
                    Ok(None) => {
                        return Ok(reject(
                            request,
                            RuntimeIpcRecipientImportExecutionHazard::NoActiveIntent,
                        ));
                    }
                    Err(_) => {
                        return Ok(reject(
                            request,
                            RuntimeIpcRecipientImportExecutionHazard::ImportRejected,
                        ));
                    }
                };
                if durable.consumed() {
                    return Ok(reject(
                        request,
                        RuntimeIpcRecipientImportExecutionHazard::IntentConsumed,
                    ));
                }
                if store.inspect_active().ok().flatten().as_ref() != Some(&durable) {
                    return Ok(reject(
                        request,
                        RuntimeIpcRecipientImportExecutionHazard::PlanChanged,
                    ));
                }
                let loaded = match load_exact_memberships(memberships, loader) {
                    Ok(loaded) => loaded,
                    Err(hazard) => {
                        return Ok(reject(request, hazard));
                    }
                };
                let sources = transient_sources(memberships, &loaded);
                let budget =
                    match RecipientRecoveryImportRoundExecutionBudget::new(usize::from(*max_steps))
                    {
                        Ok(budget) => budget,
                        Err(_) => {
                            return Ok(reject(
                                request,
                                RuntimeIpcRecipientImportExecutionHazard::ImportRejected,
                            ));
                        }
                    };
                let plan = match RecipientHistoryRecoveryImportRoundExecutor::plan(
                    store, current, &sources, budget, inbox, imports,
                ) {
                    Ok(plan) => plan,
                    Err(error) => {
                        return Ok(reject(request, execution_hazard(&error)));
                    }
                };
                if plan.intent_id() != intent_id.to_bytes()? {
                    return Ok(reject(
                        request,
                        RuntimeIpcRecipientImportExecutionHazard::PlanChanged,
                    ));
                }
                let membership_set_digest = request
                    .command()
                    .membership_set_digest()?
                    .ok_or(RecipientImportReadAdapterError::MissingMembershipDigest)?;
                let summary = execution_plan_summary(&plan, membership_set_digest.clone())?;
                Ok(RecipientImportReadAdapterDispatch {
                    outcome: RuntimeIpcRecipientImportExecutionOutcome::Planned(summary),
                    execution_plan: Some(plan),
                    membership_references: memberships.clone(),
                    membership_set_digest: Some(membership_set_digest),
                })
            }
            RuntimeIpcRecipientImportExecutionCommand::Status {
                intent_id,
                execution_plan_id,
            } => {
                let durable = match store.load_by_intent_id(intent_id.to_bytes()?) {
                    Ok(Some(durable)) => durable,
                    Ok(None) => {
                        return Ok(reject(
                            request,
                            RuntimeIpcRecipientImportExecutionHazard::NoActiveIntent,
                        ));
                    }
                    Err(_) => {
                        return Ok(reject(
                            request,
                            RuntimeIpcRecipientImportExecutionHazard::ImportRejected,
                        ));
                    }
                };
                if durable.consumed() {
                    let total = u8::try_from(durable.intent().source_count())
                        .map_err(|_| RecipientImportReadAdapterError::ProjectionOverflow)?;
                    let summary = RuntimeIpcRecipientImportStatusSummary {
                        intent_id: intent_id.clone(),
                        execution_plan_id: execution_plan_id.clone(),
                        coordinator_plan_id: RuntimeIpcRecipientImportArtifactId::from_bytes(
                            durable.intent().coordinator_plan_id(),
                        )?,
                        coordinator_sequence: durable.intent().coordinator_sequence(),
                        completed_source_count: total,
                        total_source_count: total,
                        state: RuntimeIpcRecipientImportIntentState::Consumed,
                    };
                    return Ok(outcome_dispatch(
                        RuntimeIpcRecipientImportExecutionOutcome::Status(summary),
                    ));
                }
                let active = match store.inspect_active() {
                    Ok(Some(active)) if active.intent_id() == durable.intent_id() => active,
                    Ok(_) => {
                        return Ok(reject(
                            request,
                            RuntimeIpcRecipientImportExecutionHazard::PlanChanged,
                        ));
                    }
                    Err(_) => {
                        return Ok(reject(
                            request,
                            RuntimeIpcRecipientImportExecutionHazard::ImportRejected,
                        ));
                    }
                };
                if let Some(known) = known_plan {
                    if known.plan.intent_id() != active.intent_id()
                        || execution_plan_id.as_ref().is_some_and(|expected| {
                            expected.to_bytes().ok() != Some(known.plan.plan_id())
                        })
                        || current.plan_id() != known.plan.progress().coordinator_plan_id
                        || current.sequence() != known.plan.progress().coordinator_sequence
                    {
                        return Ok(reject(
                            request,
                            RuntimeIpcRecipientImportExecutionHazard::PlanChanged,
                        ));
                    }
                    let loaded = match load_exact_memberships(known.membership_references, loader) {
                        Ok(loaded) => loaded,
                        Err(hazard) => {
                            return Ok(reject(request, hazard));
                        }
                    };
                    let sources = transient_sources(known.membership_references, &loaded);
                    let fresh = match RecipientHistoryRecoveryImportRoundExecutor::plan(
                        store,
                        current,
                        &sources,
                        known.plan.budget(),
                        inbox,
                        imports,
                    ) {
                        Ok(plan) => plan,
                        Err(error) => {
                            return Ok(reject(request, execution_hazard(&error)));
                        }
                    };
                    if fresh.plan_id() != known.plan.plan_id()
                        || request.command().intent_id().to_bytes()? != fresh.intent_id()
                    {
                        return Ok(reject(
                            request,
                            RuntimeIpcRecipientImportExecutionHazard::PlanChanged,
                        ));
                    }
                    let summary = status_summary(
                        &fresh,
                        Some(RuntimeIpcRecipientImportArtifactId::from_bytes(
                            fresh.plan_id(),
                        )?),
                    )?;
                    return Ok(RecipientImportReadAdapterDispatch {
                        outcome: RuntimeIpcRecipientImportExecutionOutcome::Status(summary),
                        execution_plan: Some(fresh),
                        membership_references: known.membership_references.to_vec(),
                        membership_set_digest: Some(known.membership_set_digest.clone()),
                    });
                }
                if execution_plan_id.is_some()
                    || current.plan_id() != active.intent().coordinator_plan_id()
                    || current.sequence() != active.intent().coordinator_sequence()
                {
                    return Ok(reject(
                        request,
                        RuntimeIpcRecipientImportExecutionHazard::PlanChanged,
                    ));
                }
                let total = u8::try_from(active.intent().source_count())
                    .map_err(|_| RecipientImportReadAdapterError::ProjectionOverflow)?;
                let summary = RuntimeIpcRecipientImportStatusSummary {
                    intent_id: intent_id.clone(),
                    execution_plan_id: None,
                    coordinator_plan_id: RuntimeIpcRecipientImportArtifactId::from_bytes(
                        current.plan_id(),
                    )?,
                    coordinator_sequence: current.sequence(),
                    completed_source_count: 0,
                    total_source_count: total,
                    state: RuntimeIpcRecipientImportIntentState::ActiveUnplanned,
                };
                Ok(outcome_dispatch(
                    RuntimeIpcRecipientImportExecutionOutcome::Status(summary),
                ))
            }
            RuntimeIpcRecipientImportExecutionCommand::Confirm { .. }
            | RuntimeIpcRecipientImportExecutionCommand::ApplyNext { .. }
            | RuntimeIpcRecipientImportExecutionCommand::ApplyBounded { .. } => Ok(reject(
                request,
                RuntimeIpcRecipientImportExecutionHazard::ImportRejected,
            )),
        }
    }
}

pub(crate) fn load_exact_memberships<L: RecipientImportMembershipLoader>(
    references: &[RuntimeIpcRecipientImportMembershipReference],
    loader: &L,
) -> Result<Vec<ConversationMembershipSnapshot>, RuntimeIpcRecipientImportExecutionHazard> {
    let mut loaded = Vec::with_capacity(references.len());
    for reference in references {
        let exact = match loader.load_membership(reference) {
            RecipientImportMembershipLoad::Exact(exact) => exact,
            RecipientImportMembershipLoad::Missing | RecipientImportMembershipLoad::Ambiguous => {
                return Err(RuntimeIpcRecipientImportExecutionHazard::MembershipReloadFailed);
            }
        };
        if exact.authority_reference_id != reference.authority_reference_id
            || exact.authenticated_context_id != reference.authenticated_context_id
        {
            return Err(RuntimeIpcRecipientImportExecutionHazard::MembershipReferenceChanged);
        }
        let digest = recipient_import_membership_digest(&exact.membership)
            .map_err(|_| RuntimeIpcRecipientImportExecutionHazard::MembershipReloadFailed)?;
        let expected = reference
            .membership_digest
            .to_bytes()
            .map_err(|_| RuntimeIpcRecipientImportExecutionHazard::MembershipReferenceChanged)?;
        if digest != expected {
            return Err(RuntimeIpcRecipientImportExecutionHazard::MembershipReferenceChanged);
        }
        loaded.push(exact.membership);
    }
    Ok(loaded)
}

pub(crate) fn transient_sources<'a>(
    references: &[RuntimeIpcRecipientImportMembershipReference],
    memberships: &'a [ConversationMembershipSnapshot],
) -> Vec<RecipientRecoveryImportRoundSource<'a>> {
    references
        .iter()
        .zip(memberships)
        .map(|(reference, membership)| {
            RecipientRecoveryImportRoundSource::new(
                usize::from(reference.source_ordinal),
                membership,
            )
        })
        .collect()
}

fn execution_plan_summary(
    plan: &RecipientRecoveryImportRoundExecutionPlan,
    membership_set_digest: RuntimeIpcRecipientImportArtifactId,
) -> Result<RuntimeIpcRecipientImportExecutionPlanSummary, RecipientImportReadAdapterError> {
    let progress = plan.progress();
    let execution_plan_id = RuntimeIpcRecipientImportArtifactId::from_bytes(plan.plan_id())?;
    Ok(RuntimeIpcRecipientImportExecutionPlanSummary {
        intent_id: RuntimeIpcRecipientImportArtifactId::from_bytes(plan.intent_id())?,
        execution_plan_id: execution_plan_id.clone(),
        coordinator_plan_id: RuntimeIpcRecipientImportArtifactId::from_bytes(
            progress.coordinator_plan_id,
        )?,
        coordinator_sequence: progress.coordinator_sequence,
        membership_set_digest,
        completed_source_count: u8::try_from(progress.completed_source_count)
            .map_err(|_| RecipientImportReadAdapterError::ProjectionOverflow)?,
        total_source_count: u8::try_from(progress.total_source_count)
            .map_err(|_| RecipientImportReadAdapterError::ProjectionOverflow)?,
        max_steps: u8::try_from(plan.budget().max_steps())
            .map_err(|_| RecipientImportReadAdapterError::ProjectionOverflow)?,
        planned_step_count: u8::try_from(plan.planned_step_count())
            .map_err(|_| RecipientImportReadAdapterError::ProjectionOverflow)?,
        next_source_ordinal: plan
            .next_source_ordinal()
            .map(u16::try_from)
            .transpose()
            .map_err(|_| RecipientImportReadAdapterError::ProjectionOverflow)?,
        coordinator_refresh_required: progress.coordinator_refresh_required,
        confirmation_phrase: recipient_import_confirmation_phrase(&execution_plan_id),
    })
}

fn status_summary(
    plan: &RecipientRecoveryImportRoundExecutionPlan,
    execution_plan_id: Option<RuntimeIpcRecipientImportArtifactId>,
) -> Result<RuntimeIpcRecipientImportStatusSummary, RecipientImportReadAdapterError> {
    let progress = plan.progress();
    let state = match progress.state {
        RecipientRecoveryImportRoundIntentProgressState::NotStarted => {
            RuntimeIpcRecipientImportIntentState::AwaitingConfirmation
        }
        RecipientRecoveryImportRoundIntentProgressState::Partial => {
            RuntimeIpcRecipientImportIntentState::InProgress
        }
        RecipientRecoveryImportRoundIntentProgressState::Complete => {
            RuntimeIpcRecipientImportIntentState::CompleteAwaitingSeparateConsume
        }
    };
    Ok(RuntimeIpcRecipientImportStatusSummary {
        intent_id: RuntimeIpcRecipientImportArtifactId::from_bytes(plan.intent_id())?,
        execution_plan_id,
        coordinator_plan_id: RuntimeIpcRecipientImportArtifactId::from_bytes(
            progress.coordinator_plan_id,
        )?,
        coordinator_sequence: progress.coordinator_sequence,
        completed_source_count: u8::try_from(progress.completed_source_count)
            .map_err(|_| RecipientImportReadAdapterError::ProjectionOverflow)?,
        total_source_count: u8::try_from(progress.total_source_count)
            .map_err(|_| RecipientImportReadAdapterError::ProjectionOverflow)?,
        state,
    })
}

pub(crate) fn execution_hazard(
    error: &RecipientRecoveryImportRoundExecutionError,
) -> RuntimeIpcRecipientImportExecutionHazard {
    match error {
        RecipientRecoveryImportRoundExecutionError::NoActiveIntent => {
            RuntimeIpcRecipientImportExecutionHazard::NoActiveIntent
        }
        RecipientRecoveryImportRoundExecutionError::IntentConsumed => {
            RuntimeIpcRecipientImportExecutionHazard::IntentConsumed
        }
        RecipientRecoveryImportRoundExecutionError::SourceMismatch
        | RecipientRecoveryImportRoundExecutionError::CandidateChanged => {
            RuntimeIpcRecipientImportExecutionHazard::MembershipReferenceChanged
        }
        RecipientRecoveryImportRoundExecutionError::ActiveIntentChanged
        | RecipientRecoveryImportRoundExecutionError::PlanChanged
        | RecipientRecoveryImportRoundExecutionError::ProgressChanged
        | RecipientRecoveryImportRoundExecutionError::InvalidPlan => {
            RuntimeIpcRecipientImportExecutionHazard::PlanChanged
        }
        _ => RuntimeIpcRecipientImportExecutionHazard::ImportRejected,
    }
}

fn reject(
    request: &RuntimeIpcRecipientImportExecutionRequest,
    hazard: RuntimeIpcRecipientImportExecutionHazard,
) -> RecipientImportReadAdapterDispatch {
    outcome_dispatch(RuntimeIpcRecipientImportExecutionOutcome::Rejected {
        intent_id: request.command().intent_id().clone(),
        hazard,
    })
}

fn outcome_dispatch(
    outcome: RuntimeIpcRecipientImportExecutionOutcome,
) -> RecipientImportReadAdapterDispatch {
    RecipientImportReadAdapterDispatch {
        outcome,
        execution_plan: None,
        membership_references: Vec::new(),
        membership_set_digest: None,
    }
}

#[derive(Debug, Error)]
pub enum RecipientImportReadAdapterError {
    #[error("recipient import read adapter projection exceeds the IPC bounds")]
    ProjectionOverflow,
    #[error("recipient import plan request has no membership-set digest")]
    MissingMembershipDigest,
    #[error(transparent)]
    Contract(#[from] ContractError),
}
