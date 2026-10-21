use kilogram_identity::DeviceIdentity;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ApplyRecipientRecoveryCoordinationOutcome, ApplyRecipientRecoveryImportOutcome,
    MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_SOURCES, RecipientHistoryImportStore,
    RecipientHistoryRecoveryCoordinator, RecipientHistoryRecoveryImporter,
    RecipientHistoryTransferInbox, RecipientRecoveryCoordinationPlan,
    RecipientRecoveryCoordinatorError, RecipientRecoveryImportPlanError,
    RecipientRecoveryImportRoundIntentError, RecipientRecoveryImportRoundIntentProgress,
    RecipientRecoveryImportRoundIntentProgressState, RecipientRecoveryImportRoundIntentStore,
    RecipientRecoveryImportRoundSource, RecipientRecoveryImportRoundStepDisposition,
};

const EXECUTION_PLAN_VERSION: u8 = 1;
const EXECUTION_PLAN_ID_DOMAIN: &[u8] =
    b"kilogram:recipient-recovery-import-round-execution-plan:v1\0";

pub const DEFAULT_RECIPIENT_RECOVERY_IMPORT_ROUND_EXECUTION_MAX_STEPS: usize = 1;
pub const MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_EXECUTION_STEPS: usize =
    MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_SOURCES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecipientRecoveryImportRoundExecutionBudget {
    max_steps: usize,
}

impl RecipientRecoveryImportRoundExecutionBudget {
    pub fn new(max_steps: usize) -> Result<Self, RecipientRecoveryImportRoundExecutionError> {
        if max_steps == 0 || max_steps > MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_EXECUTION_STEPS {
            return Err(RecipientRecoveryImportRoundExecutionError::InvalidBudget);
        }
        Ok(Self { max_steps })
    }

    pub fn max_steps(self) -> usize {
        self.max_steps
    }
}

impl Default for RecipientRecoveryImportRoundExecutionBudget {
    fn default() -> Self {
        Self {
            max_steps: DEFAULT_RECIPIENT_RECOVERY_IMPORT_ROUND_EXECUTION_MAX_STEPS,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RecipientRecoveryImportRoundExecutionPlanWire {
    version: u8,
    intent_id: [u8; 32],
    round_id: [u8; 32],
    coordinator_plan_id: [u8; 32],
    coordinator_sequence: u64,
    completed_source_count: u16,
    total_source_count: u16,
    max_steps: u8,
    planned_step_count: u8,
    next_source_ordinal: Option<u16>,
    coordinator_refresh_required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientRecoveryImportRoundExecutionPlan {
    coordination_plan: RecipientRecoveryCoordinationPlan,
    progress: RecipientRecoveryImportRoundIntentProgress,
    wire: RecipientRecoveryImportRoundExecutionPlanWire,
    plan_id: [u8; 32],
    pub contains_membership_snapshot: bool,
    pub contains_message_bodies: bool,
    pub contains_plaintext: bool,
    pub grants_network_authority: bool,
    pub grants_background_authority: bool,
    pub executes_one_import_at_a_time: bool,
}

impl RecipientRecoveryImportRoundExecutionPlan {
    pub fn plan_id(&self) -> [u8; 32] {
        self.plan_id
    }

    pub fn intent_id(&self) -> [u8; 32] {
        self.wire.intent_id
    }

    pub fn round_id(&self) -> [u8; 32] {
        self.wire.round_id
    }

    pub fn progress(&self) -> &RecipientRecoveryImportRoundIntentProgress {
        &self.progress
    }

    pub fn budget(&self) -> RecipientRecoveryImportRoundExecutionBudget {
        RecipientRecoveryImportRoundExecutionBudget {
            max_steps: usize::from(self.wire.max_steps),
        }
    }

    pub fn planned_step_count(&self) -> usize {
        usize::from(self.wire.planned_step_count)
    }

    pub fn next_source_ordinal(&self) -> Option<usize> {
        self.wire.next_source_ordinal.map(usize::from)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientRecoveryImportRoundExecutionStepReceipt {
    pub source_ordinal: usize,
    pub disposition: RecipientRecoveryImportRoundStepDisposition,
    pub imported_event_count: usize,
    pub coordinator_sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientRecoveryImportRoundExecutionReceipt {
    pub plan_id: [u8; 32],
    pub intent_id: [u8; 32],
    pub round_id: [u8; 32],
    pub steps: Vec<RecipientRecoveryImportRoundExecutionStepReceipt>,
    pub reconciled_coordinator: bool,
    pub starting_completed_source_count: usize,
    pub final_completed_source_count: usize,
    pub total_source_count: usize,
    pub imported_now_count: usize,
    pub already_imported_count: usize,
    pub imported_event_count: usize,
    pub final_coordinator_plan_id: [u8; 32],
    pub final_coordinator_sequence: u64,
    pub complete: bool,
    pub contains_membership_snapshot: bool,
    pub contains_message_bodies: bool,
    pub contains_plaintext: bool,
    pub opened_network: bool,
    pub scheduled_background_work: bool,
    pub concurrent_imports: usize,
}

pub struct RecipientHistoryRecoveryImportRoundExecutor;

impl RecipientHistoryRecoveryImportRoundExecutor {
    #[allow(clippy::too_many_arguments)]
    pub fn plan(
        store: &RecipientRecoveryImportRoundIntentStore,
        current: &RecipientRecoveryCoordinationPlan,
        sources: &[RecipientRecoveryImportRoundSource<'_>],
        budget: RecipientRecoveryImportRoundExecutionBudget,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
    ) -> Result<RecipientRecoveryImportRoundExecutionPlan, RecipientRecoveryImportRoundExecutionError>
    {
        validate_budget(budget)?;
        let durable = store
            .inspect_active()?
            .ok_or(RecipientRecoveryImportRoundExecutionError::NoActiveIntent)?;
        if durable.consumed() {
            return Err(RecipientRecoveryImportRoundExecutionError::IntentConsumed);
        }
        let progress = store.inspect_progress(&durable, current, sources, inbox, imports)?;
        let remaining = progress
            .total_source_count
            .checked_sub(progress.completed_source_count)
            .ok_or(RecipientRecoveryImportRoundExecutionError::InvalidPlan)?;
        let planned_step_count = remaining.min(budget.max_steps);
        let next_source_ordinal = if planned_step_count == 0 {
            None
        } else {
            Some(
                u16::try_from(
                    durable
                        .intent()
                        .candidate_binding(progress.completed_source_count)
                        .ok_or(RecipientRecoveryImportRoundExecutionError::InvalidPlan)?
                        .source_ordinal,
                )
                .map_err(|_| RecipientRecoveryImportRoundExecutionError::InvalidPlan)?,
            )
        };
        let wire = RecipientRecoveryImportRoundExecutionPlanWire {
            version: EXECUTION_PLAN_VERSION,
            intent_id: durable.intent_id(),
            round_id: durable.intent().round_id(),
            coordinator_plan_id: current.plan_id(),
            coordinator_sequence: current.sequence(),
            completed_source_count: u16::try_from(progress.completed_source_count)
                .map_err(|_| RecipientRecoveryImportRoundExecutionError::InvalidPlan)?,
            total_source_count: u16::try_from(progress.total_source_count)
                .map_err(|_| RecipientRecoveryImportRoundExecutionError::InvalidPlan)?,
            max_steps: u8::try_from(budget.max_steps)
                .map_err(|_| RecipientRecoveryImportRoundExecutionError::InvalidBudget)?,
            planned_step_count: u8::try_from(planned_step_count)
                .map_err(|_| RecipientRecoveryImportRoundExecutionError::InvalidPlan)?,
            next_source_ordinal,
            coordinator_refresh_required: progress.coordinator_refresh_required,
        };
        let plan_id = wire_id(&wire)?;
        let plan = RecipientRecoveryImportRoundExecutionPlan {
            coordination_plan: current.clone(),
            progress,
            wire,
            plan_id,
            contains_membership_snapshot: false,
            contains_message_bodies: false,
            contains_plaintext: false,
            grants_network_authority: false,
            grants_background_authority: false,
            executes_one_import_at_a_time: true,
        };
        validate_plan(&plan)?;
        Ok(plan)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn apply_next(
        plan: &RecipientRecoveryImportRoundExecutionPlan,
        store: &RecipientRecoveryImportRoundIntentStore,
        sources: &[RecipientRecoveryImportRoundSource<'_>],
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        recipient: &DeviceIdentity,
        committed_at_unix_seconds: u64,
    ) -> Result<
        RecipientRecoveryImportRoundExecutionReceipt,
        RecipientRecoveryImportRoundExecutionError,
    > {
        Self::apply_inner(
            plan,
            store,
            sources,
            inbox,
            imports,
            recipient,
            committed_at_unix_seconds,
            1,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn apply_bounded(
        plan: &RecipientRecoveryImportRoundExecutionPlan,
        store: &RecipientRecoveryImportRoundIntentStore,
        sources: &[RecipientRecoveryImportRoundSource<'_>],
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        recipient: &DeviceIdentity,
        committed_at_unix_seconds: u64,
    ) -> Result<
        RecipientRecoveryImportRoundExecutionReceipt,
        RecipientRecoveryImportRoundExecutionError,
    > {
        Self::apply_inner(
            plan,
            store,
            sources,
            inbox,
            imports,
            recipient,
            committed_at_unix_seconds,
            plan.budget().max_steps(),
        )
    }

    /// Repairs only the durable coordinator projection after an import that
    /// was committed before its coordinator refresh completed. No additional
    /// history source is imported. The returned plan is freshly bound to the
    /// repaired coordinator head and to the caller-selected remaining budget.
    #[allow(clippy::too_many_arguments)]
    pub fn reconcile_pending(
        plan: &RecipientRecoveryImportRoundExecutionPlan,
        store: &RecipientRecoveryImportRoundIntentStore,
        sources: &[RecipientRecoveryImportRoundSource<'_>],
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        next_budget: RecipientRecoveryImportRoundExecutionBudget,
    ) -> Result<RecipientRecoveryImportRoundExecutionPlan, RecipientRecoveryImportRoundExecutionError>
    {
        validate_plan(plan)?;
        validate_budget(next_budget)?;
        let durable = reopen_exact(store, plan.intent_id(), plan.round_id())?;
        let progress =
            store.inspect_progress(&durable, &plan.coordination_plan, sources, inbox, imports)?;
        if progress != plan.progress || !progress.coordinator_refresh_required {
            return Err(RecipientRecoveryImportRoundExecutionError::PlanChanged);
        }
        match RecipientHistoryRecoveryCoordinator::apply(&plan.coordination_plan, inbox, imports)? {
            ApplyRecipientRecoveryCoordinationOutcome::Applied(_)
            | ApplyRecipientRecoveryCoordinationOutcome::AlreadyApplied(_) => {}
        }
        let refreshed = refresh_coordination(&plan.coordination_plan, inbox, imports)?;
        let next = Self::plan(store, &refreshed, sources, next_budget, inbox, imports)?;
        if next.progress.coordinator_refresh_required
            || next.progress.completed_source_count != progress.completed_source_count
            || next.progress.total_source_count != progress.total_source_count
            || next.progress.imported_event_count != progress.imported_event_count
        {
            return Err(RecipientRecoveryImportRoundExecutionError::ProgressChanged);
        }
        Ok(next)
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_inner(
        plan: &RecipientRecoveryImportRoundExecutionPlan,
        store: &RecipientRecoveryImportRoundIntentStore,
        sources: &[RecipientRecoveryImportRoundSource<'_>],
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        recipient: &DeviceIdentity,
        committed_at_unix_seconds: u64,
        requested_steps: usize,
    ) -> Result<
        RecipientRecoveryImportRoundExecutionReceipt,
        RecipientRecoveryImportRoundExecutionError,
    > {
        validate_plan(plan)?;
        if requested_steps == 0 || requested_steps > plan.budget().max_steps() {
            return Err(RecipientRecoveryImportRoundExecutionError::InvalidBudget);
        }

        let mut durable = reopen_exact(store, plan.intent_id(), plan.round_id())?;
        let mut current = plan.coordination_plan.clone();
        let mut progress = store.inspect_progress(&durable, &current, sources, inbox, imports)?;
        if progress != plan.progress {
            return Err(RecipientRecoveryImportRoundExecutionError::PlanChanged);
        }

        let mut reconciled_coordinator = false;
        if progress.coordinator_refresh_required {
            match RecipientHistoryRecoveryCoordinator::apply(&current, inbox, imports)? {
                ApplyRecipientRecoveryCoordinationOutcome::Applied(_)
                | ApplyRecipientRecoveryCoordinationOutcome::AlreadyApplied(_) => {}
            }
            reconciled_coordinator = true;
            current = refresh_coordination(&current, inbox, imports)?;
            durable = reopen_exact(store, plan.intent_id(), plan.round_id())?;
            progress = store.inspect_progress(&durable, &current, sources, inbox, imports)?;
            if progress.coordinator_refresh_required
                || progress.completed_source_count != plan.progress.completed_source_count
            {
                return Err(RecipientRecoveryImportRoundExecutionError::PlanChanged);
            }
        }

        let starting_completed_source_count = progress.completed_source_count;
        let remaining = progress
            .total_source_count
            .checked_sub(progress.completed_source_count)
            .ok_or(RecipientRecoveryImportRoundExecutionError::InvalidPlan)?;
        let step_limit = requested_steps.min(remaining);
        let mut steps = Vec::with_capacity(step_limit);
        let mut imported_now_count = 0_usize;
        let mut already_imported_count = 0_usize;
        let mut imported_event_count = 0_usize;

        for offset in 0..step_limit {
            let index = progress.completed_source_count;
            let source = sources
                .get(index)
                .copied()
                .ok_or(RecipientRecoveryImportRoundExecutionError::SourceMismatch)?;
            let expected = durable
                .intent()
                .candidate_binding(index)
                .ok_or(RecipientRecoveryImportRoundExecutionError::SourceMismatch)?;
            if source.source_ordinal() != expected.source_ordinal {
                return Err(RecipientRecoveryImportRoundExecutionError::SourceMismatch);
            }
            let fresh = RecipientHistoryRecoveryImporter::plan(
                &current,
                source.source_ordinal(),
                inbox,
                imports,
                source.membership(),
            )?;
            if fresh.candidate_binding() != expected {
                return Err(RecipientRecoveryImportRoundExecutionError::CandidateChanged);
            }
            let outcome = RecipientHistoryRecoveryImporter::apply(
                &fresh,
                inbox,
                imports,
                recipient,
                source.membership(),
                committed_at_unix_seconds
                    .checked_add(u64::try_from(offset).map_err(|_| {
                        RecipientRecoveryImportRoundExecutionError::TimestampExhausted
                    })?)
                    .ok_or(RecipientRecoveryImportRoundExecutionError::TimestampExhausted)?,
            )?;
            let (disposition, receipt) = match outcome {
                ApplyRecipientRecoveryImportOutcome::Imported(receipt) => {
                    imported_now_count += 1;
                    (
                        RecipientRecoveryImportRoundStepDisposition::Imported,
                        receipt,
                    )
                }
                ApplyRecipientRecoveryImportOutcome::AlreadyImported(receipt) => {
                    already_imported_count += 1;
                    (
                        RecipientRecoveryImportRoundStepDisposition::AlreadyImported,
                        receipt,
                    )
                }
            };
            imported_event_count = imported_event_count
                .checked_add(receipt.imported_event_count)
                .ok_or(RecipientRecoveryImportRoundExecutionError::InvalidPlan)?;
            steps.push(RecipientRecoveryImportRoundExecutionStepReceipt {
                source_ordinal: receipt.source_ordinal,
                disposition,
                imported_event_count: receipt.imported_event_count,
                coordinator_sequence: receipt.coordinator_sequence,
            });

            current = refresh_coordination(&current, inbox, imports)?;
            durable = reopen_exact(store, plan.intent_id(), plan.round_id())?;
            let next = store.inspect_progress(&durable, &current, sources, inbox, imports)?;
            if next.coordinator_refresh_required
                || next.completed_source_count != progress.completed_source_count + 1
            {
                return Err(RecipientRecoveryImportRoundExecutionError::ProgressChanged);
            }
            progress = next;
        }

        Ok(RecipientRecoveryImportRoundExecutionReceipt {
            plan_id: plan.plan_id,
            intent_id: plan.intent_id(),
            round_id: plan.round_id(),
            steps,
            reconciled_coordinator,
            starting_completed_source_count,
            final_completed_source_count: progress.completed_source_count,
            total_source_count: progress.total_source_count,
            imported_now_count,
            already_imported_count,
            imported_event_count,
            final_coordinator_plan_id: current.plan_id(),
            final_coordinator_sequence: current.sequence(),
            complete: progress.state == RecipientRecoveryImportRoundIntentProgressState::Complete,
            contains_membership_snapshot: false,
            contains_message_bodies: false,
            contains_plaintext: false,
            opened_network: false,
            scheduled_background_work: false,
            concurrent_imports: 1,
        })
    }
}

fn reopen_exact(
    store: &RecipientRecoveryImportRoundIntentStore,
    expected_intent_id: [u8; 32],
    expected_round_id: [u8; 32],
) -> Result<
    crate::DurableRecipientRecoveryImportRoundIntent,
    RecipientRecoveryImportRoundExecutionError,
> {
    let durable = store
        .inspect_active()?
        .ok_or(RecipientRecoveryImportRoundExecutionError::ActiveIntentChanged)?;
    if durable.intent_id() != expected_intent_id
        || durable.intent().round_id() != expected_round_id
        || durable.consumed()
    {
        return Err(RecipientRecoveryImportRoundExecutionError::ActiveIntentChanged);
    }
    Ok(durable)
}

fn refresh_coordination(
    current: &RecipientRecoveryCoordinationPlan,
    inbox: &RecipientHistoryTransferInbox,
    imports: &RecipientHistoryImportStore,
) -> Result<RecipientRecoveryCoordinationPlan, RecipientRecoveryImportRoundExecutionError> {
    Ok(RecipientHistoryRecoveryCoordinator::plan(
        current.state_directory(),
        current.owner_account_id(),
        current.owner_device_id(),
        current.approved_sources(),
        inbox,
        imports,
    )?)
}

fn validate_budget(
    budget: RecipientRecoveryImportRoundExecutionBudget,
) -> Result<(), RecipientRecoveryImportRoundExecutionError> {
    if budget.max_steps == 0
        || budget.max_steps > MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_EXECUTION_STEPS
    {
        return Err(RecipientRecoveryImportRoundExecutionError::InvalidBudget);
    }
    Ok(())
}

fn validate_plan(
    plan: &RecipientRecoveryImportRoundExecutionPlan,
) -> Result<(), RecipientRecoveryImportRoundExecutionError> {
    validate_wire(&plan.wire)?;
    validate_budget(plan.budget())?;
    if wire_id(&plan.wire)? != plan.plan_id
        || plan.wire.intent_id != plan.progress.intent_id
        || plan.wire.round_id != plan.progress.round_id
        || plan.wire.coordinator_plan_id != plan.progress.coordinator_plan_id
        || plan.wire.coordinator_plan_id != plan.coordination_plan.plan_id()
        || plan.wire.coordinator_sequence != plan.progress.coordinator_sequence
        || plan.wire.coordinator_sequence != plan.coordination_plan.sequence()
        || usize::from(plan.wire.completed_source_count) != plan.progress.completed_source_count
        || usize::from(plan.wire.total_source_count) != plan.progress.total_source_count
        || plan.wire.coordinator_refresh_required != plan.progress.coordinator_refresh_required
        || plan.contains_membership_snapshot
        || plan.contains_message_bodies
        || plan.contains_plaintext
        || plan.grants_network_authority
        || plan.grants_background_authority
        || !plan.executes_one_import_at_a_time
    {
        return Err(RecipientRecoveryImportRoundExecutionError::InvalidPlan);
    }
    Ok(())
}

fn validate_wire(
    wire: &RecipientRecoveryImportRoundExecutionPlanWire,
) -> Result<(), RecipientRecoveryImportRoundExecutionError> {
    let remaining = usize::from(wire.total_source_count)
        .checked_sub(usize::from(wire.completed_source_count))
        .ok_or(RecipientRecoveryImportRoundExecutionError::InvalidPlan)?;
    if wire.version != EXECUTION_PLAN_VERSION
        || wire.intent_id == [0_u8; 32]
        || wire.round_id == [0_u8; 32]
        || wire.coordinator_plan_id == [0_u8; 32]
        || wire.total_source_count == 0
        || usize::from(wire.total_source_count) > MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_SOURCES
        || wire.max_steps == 0
        || usize::from(wire.max_steps) > MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_EXECUTION_STEPS
        || usize::from(wire.planned_step_count) != remaining.min(usize::from(wire.max_steps))
        || (wire.planned_step_count == 0) != wire.next_source_ordinal.is_none()
    {
        return Err(RecipientRecoveryImportRoundExecutionError::InvalidPlan);
    }
    Ok(())
}

fn wire_id(
    wire: &RecipientRecoveryImportRoundExecutionPlanWire,
) -> Result<[u8; 32], RecipientRecoveryImportRoundExecutionError> {
    validate_wire(wire)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(EXECUTION_PLAN_ID_DOMAIN);
    hasher.update(&postcard::to_allocvec(wire)?);
    Ok(*hasher.finalize().as_bytes())
}

#[derive(Debug, Error)]
pub enum RecipientRecoveryImportRoundExecutionError {
    #[error("recipient recovery import-round execution budget is invalid")]
    InvalidBudget,
    #[error("recipient recovery import-round execution plan is invalid")]
    InvalidPlan,
    #[error("no active recipient recovery import-round intent exists")]
    NoActiveIntent,
    #[error("active recipient recovery import-round intent was already consumed")]
    IntentConsumed,
    #[error("active recipient recovery import-round intent changed")]
    ActiveIntentChanged,
    #[error("recipient recovery import-round execution plan changed before apply")]
    PlanChanged,
    #[error("recipient recovery import-round source does not match the signed intent")]
    SourceMismatch,
    #[error("recipient recovery import-round candidate changed")]
    CandidateChanged,
    #[error("recipient recovery import-round durable progress changed unexpectedly")]
    ProgressChanged,
    #[error("recipient recovery import-round execution timestamp is exhausted")]
    TimestampExhausted,
    #[error(transparent)]
    Intent(Box<RecipientRecoveryImportRoundIntentError>),
    #[error(transparent)]
    Coordinator(#[from] RecipientRecoveryCoordinatorError),
    #[error(transparent)]
    Import(Box<RecipientRecoveryImportPlanError>),
    #[error(transparent)]
    Postcard(#[from] postcard::Error),
}

impl From<RecipientRecoveryImportRoundIntentError> for RecipientRecoveryImportRoundExecutionError {
    fn from(error: RecipientRecoveryImportRoundIntentError) -> Self {
        Self::Intent(Box::new(error))
    }
}

impl From<RecipientRecoveryImportPlanError> for RecipientRecoveryImportRoundExecutionError {
    fn from(error: RecipientRecoveryImportPlanError) -> Self {
        Self::Import(Box::new(error))
    }
}
