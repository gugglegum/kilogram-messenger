use std::collections::HashSet;

use kilogram_identity::{ConversationMembershipSnapshot, DeviceIdentity, IdentityError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::coordinator::RecipientRecoveryCoordinatorImportObservation;
use crate::{
    ApplyRecipientRecoveryCoordinationOutcome, ApplyRecipientRecoveryImportOutcome,
    DurableRecipientHistoryImportReceipt, MAX_HISTORY_RECOVERY_EXECUTION_CHUNK_EVENTS,
    MAX_RECIPIENT_RECOVERY_COORDINATOR_SOURCES, RecipientHistoryImportError,
    RecipientHistoryImportStore, RecipientHistoryRecoveryCoordinator,
    RecipientHistoryRecoveryImporter, RecipientHistoryTransferInbox,
    RecipientRecoveryCoordinationPlan, RecipientRecoveryCoordinatorError,
    RecipientRecoveryImportPlan, RecipientRecoveryImportPlanError,
};

const ROUND_VERSION: u8 = 1;
const ROUND_ID_DOMAIN: &[u8] = b"kilogram:recipient-recovery-import-round:v1\0";

pub const DEFAULT_RECIPIENT_RECOVERY_IMPORT_ROUND_MAX_SOURCES: usize = 4;
pub const MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_SOURCES: usize =
    MAX_RECIPIENT_RECOVERY_COORDINATOR_SOURCES;
pub const DEFAULT_RECIPIENT_RECOVERY_IMPORT_ROUND_MAX_EVENTS: usize =
    DEFAULT_RECIPIENT_RECOVERY_IMPORT_ROUND_MAX_SOURCES
        * MAX_HISTORY_RECOVERY_EXECUTION_CHUNK_EVENTS;
pub const MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_EVENTS: usize =
    MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_SOURCES * MAX_HISTORY_RECOVERY_EXECUTION_CHUNK_EVENTS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecipientRecoveryImportRoundBudget {
    max_sources: usize,
    max_events: usize,
}

impl RecipientRecoveryImportRoundBudget {
    pub fn new(
        max_sources: usize,
        max_events: usize,
    ) -> Result<Self, RecipientRecoveryImportRoundError> {
        let budget = Self {
            max_sources,
            max_events,
        };
        validate_budget(budget)?;
        Ok(budget)
    }

    pub fn max_sources(self) -> usize {
        self.max_sources
    }

    pub fn max_events(self) -> usize {
        self.max_events
    }
}

impl Default for RecipientRecoveryImportRoundBudget {
    fn default() -> Self {
        Self {
            max_sources: DEFAULT_RECIPIENT_RECOVERY_IMPORT_ROUND_MAX_SOURCES,
            max_events: DEFAULT_RECIPIENT_RECOVERY_IMPORT_ROUND_MAX_EVENTS,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RecipientRecoveryImportRoundSource<'a> {
    source_ordinal: usize,
    membership: &'a ConversationMembershipSnapshot,
}

impl<'a> RecipientRecoveryImportRoundSource<'a> {
    pub fn new(source_ordinal: usize, membership: &'a ConversationMembershipSnapshot) -> Self {
        Self {
            source_ordinal,
            membership,
        }
    }

    pub fn source_ordinal(self) -> usize {
        self.source_ordinal
    }

    pub fn membership(self) -> &'a ConversationMembershipSnapshot {
        self.membership
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RecipientRecoveryImportRoundEntryWire {
    source_ordinal: u16,
    import_plan_id: [u8; 32],
    membership_digest: [u8; 32],
    membership_revision: u64,
    event_count: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RecipientRecoveryImportRoundWire {
    version: u8,
    coordinator_plan_id: [u8; 32],
    coordinator_sequence: u64,
    max_sources: u8,
    max_events: u16,
    planned_event_count: u16,
    entries: Vec<RecipientRecoveryImportRoundEntryWire>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientRecoveryImportRoundPlan {
    coordination_plan: RecipientRecoveryCoordinationPlan,
    import_plans: Vec<RecipientRecoveryImportPlan>,
    wire: RecipientRecoveryImportRoundWire,
    round_id: [u8; 32],
    pub contains_message_bodies: bool,
    pub contains_plaintext: bool,
    pub grants_network_authority: bool,
    pub grants_background_authority: bool,
    pub preflights_all_sources_before_import: bool,
    pub imports_at_most_one_transfer_per_source: bool,
    pub executes_one_import_at_a_time: bool,
}

impl RecipientRecoveryImportRoundPlan {
    pub fn round_id(&self) -> [u8; 32] {
        self.round_id
    }

    pub fn source_count(&self) -> usize {
        self.import_plans.len()
    }

    pub fn planned_event_count(&self) -> usize {
        usize::from(self.wire.planned_event_count)
    }

    pub fn source_ordinals(&self) -> impl ExactSizeIterator<Item = usize> + '_ {
        self.wire
            .entries
            .iter()
            .map(|entry| usize::from(entry.source_ordinal))
    }

    pub fn budget(&self) -> RecipientRecoveryImportRoundBudget {
        RecipientRecoveryImportRoundBudget {
            max_sources: usize::from(self.wire.max_sources),
            max_events: usize::from(self.wire.max_events),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecipientRecoveryImportRoundStepDisposition {
    Imported,
    AlreadyImported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientRecoveryImportRoundStepReceipt {
    pub source_ordinal: usize,
    pub disposition: RecipientRecoveryImportRoundStepDisposition,
    pub imported_event_count: usize,
    pub coordinator_sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientRecoveryImportRoundApplyReceipt {
    pub round_id: [u8; 32],
    pub steps: Vec<RecipientRecoveryImportRoundStepReceipt>,
    pub imported_now_count: usize,
    pub already_imported_count: usize,
    pub imported_event_count: usize,
    pub final_coordinator_sequence: u64,
    pub contains_message_bodies: bool,
    pub contains_plaintext: bool,
    pub opened_network: bool,
    pub scheduled_background_work: bool,
    pub concurrent_imports: usize,
}

pub struct RecipientHistoryRecoveryImportRound;

impl RecipientHistoryRecoveryImportRound {
    pub fn plan(
        coordination_plan: &RecipientRecoveryCoordinationPlan,
        sources: &[RecipientRecoveryImportRoundSource<'_>],
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
    ) -> Result<RecipientRecoveryImportRoundPlan, RecipientRecoveryImportRoundError> {
        Self::plan_with_budget(
            coordination_plan,
            sources,
            RecipientRecoveryImportRoundBudget::default(),
            inbox,
            imports,
        )
    }

    pub fn plan_with_budget(
        coordination_plan: &RecipientRecoveryCoordinationPlan,
        sources: &[RecipientRecoveryImportRoundSource<'_>],
        budget: RecipientRecoveryImportRoundBudget,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
    ) -> Result<RecipientRecoveryImportRoundPlan, RecipientRecoveryImportRoundError> {
        validate_budget(budget)?;
        validate_source_count(sources.len(), budget)?;
        RecipientHistoryRecoveryCoordinator::verify_exact_current(
            coordination_plan,
            inbox,
            imports,
        )?;

        let mut seen = HashSet::with_capacity(sources.len());
        let mut import_plans = Vec::with_capacity(sources.len());
        let mut entries = Vec::with_capacity(sources.len());
        let mut planned_event_count = 0_usize;
        for source in sources {
            if !seen.insert(source.source_ordinal) {
                return Err(RecipientRecoveryImportRoundError::DuplicateSourceOrdinal(
                    source.source_ordinal,
                ));
            }
            if source.source_ordinal >= coordination_plan.sources().len() {
                return Err(RecipientRecoveryImportRoundError::SourceOrdinal(
                    source.source_ordinal,
                ));
            }
            let import_plan = RecipientHistoryRecoveryImporter::plan(
                coordination_plan,
                source.source_ordinal,
                inbox,
                imports,
                source.membership,
            )?;
            planned_event_count = planned_event_count
                .checked_add(import_plan.planned_event_count())
                .ok_or(RecipientRecoveryImportRoundError::EventBudgetExceeded {
                    planned: usize::MAX,
                    maximum: budget.max_events,
                })?;
            if planned_event_count > budget.max_events {
                return Err(RecipientRecoveryImportRoundError::EventBudgetExceeded {
                    planned: planned_event_count,
                    maximum: budget.max_events,
                });
            }
            let binding = import_plan.candidate_binding();
            entries.push(RecipientRecoveryImportRoundEntryWire {
                source_ordinal: u16::try_from(source.source_ordinal).map_err(|_| {
                    RecipientRecoveryImportRoundError::SourceOrdinal(source.source_ordinal)
                })?,
                import_plan_id: import_plan.plan_id(),
                membership_digest: binding.membership_digest,
                membership_revision: binding.membership_revision,
                event_count: u16::try_from(import_plan.planned_event_count())
                    .map_err(|_| RecipientRecoveryImportRoundError::InvalidPlan)?,
            });
            import_plans.push(import_plan);
        }

        let wire = RecipientRecoveryImportRoundWire {
            version: ROUND_VERSION,
            coordinator_plan_id: coordination_plan.plan_id(),
            coordinator_sequence: coordination_plan.sequence(),
            max_sources: u8::try_from(budget.max_sources)
                .map_err(|_| RecipientRecoveryImportRoundError::InvalidBudget)?,
            max_events: u16::try_from(budget.max_events)
                .map_err(|_| RecipientRecoveryImportRoundError::InvalidBudget)?,
            planned_event_count: u16::try_from(planned_event_count)
                .map_err(|_| RecipientRecoveryImportRoundError::InvalidPlan)?,
            entries,
        };
        validate_wire(&wire)?;
        let round_id = wire_id(&wire)?;
        let plan = RecipientRecoveryImportRoundPlan {
            coordination_plan: coordination_plan.clone(),
            import_plans,
            wire,
            round_id,
            contains_message_bodies: false,
            contains_plaintext: false,
            grants_network_authority: false,
            grants_background_authority: false,
            preflights_all_sources_before_import: true,
            imports_at_most_one_transfer_per_source: true,
            executes_one_import_at_a_time: true,
        };
        validate_plan(&plan)?;
        Ok(plan)
    }

    pub fn apply(
        plan: &RecipientRecoveryImportRoundPlan,
        sources: &[RecipientRecoveryImportRoundSource<'_>],
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        recipient: &DeviceIdentity,
        committed_at_unix_seconds: u64,
    ) -> Result<RecipientRecoveryImportRoundApplyReceipt, RecipientRecoveryImportRoundError> {
        Self::apply_inner(
            plan,
            sources,
            inbox,
            imports,
            recipient,
            committed_at_unix_seconds,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_inner(
        plan: &RecipientRecoveryImportRoundPlan,
        sources: &[RecipientRecoveryImportRoundSource<'_>],
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        recipient: &DeviceIdentity,
        committed_at_unix_seconds: u64,
    ) -> Result<RecipientRecoveryImportRoundApplyReceipt, RecipientRecoveryImportRoundError> {
        validate_plan(plan)?;
        if recipient.device_id() != plan.coordination_plan.owner_device_id() {
            return Err(RecipientRecoveryImportRoundError::WrongRecipientDevice);
        }
        validate_apply_sources(plan, sources)?;

        let mut current = current_coordination(plan, inbox, imports)?;
        let mut inspection = inspect_current(plan, sources, &current, inbox, imports)?;
        if current.requires_apply() {
            if inspection.completed_prefix == 0 {
                return Err(RecipientRecoveryImportRoundError::StalePlan);
            }
            match RecipientHistoryRecoveryCoordinator::apply(&current, inbox, imports)? {
                ApplyRecipientRecoveryCoordinationOutcome::Applied(_)
                | ApplyRecipientRecoveryCoordinationOutcome::AlreadyApplied(_) => {}
            }
            current = current_coordination(plan, inbox, imports)?;
            inspection = inspect_current(plan, sources, &current, inbox, imports)?;
            if current.requires_apply() {
                return Err(RecipientRecoveryImportRoundError::StalePlan);
            }
        }

        let mut steps = Vec::with_capacity(plan.import_plans.len());
        let mut imported_now_count = 0_usize;
        let mut already_imported_count = 0_usize;
        let mut imported_event_count = 0_usize;
        for (index, completed) in inspection
            .completed_receipts
            .iter()
            .take(inspection.completed_prefix)
            .enumerate()
        {
            let receipt = completed
                .as_ref()
                .ok_or(RecipientRecoveryImportRoundError::NonPrefixProgress)?;
            let event_count = receipt.imported_event_count();
            imported_event_count = imported_event_count
                .checked_add(event_count)
                .ok_or(RecipientRecoveryImportRoundError::InvalidPlan)?;
            already_imported_count += 1;
            steps.push(RecipientRecoveryImportRoundStepReceipt {
                source_ordinal: plan.import_plans[index].source_ordinal(),
                disposition: RecipientRecoveryImportRoundStepDisposition::AlreadyImported,
                imported_event_count: event_count,
                coordinator_sequence: plan
                    .coordination_plan
                    .sequence()
                    .checked_add(
                        u64::try_from(index + 1)
                            .map_err(|_| RecipientRecoveryImportRoundError::SequenceExhausted)?,
                    )
                    .ok_or(RecipientRecoveryImportRoundError::SequenceExhausted)?,
            });
        }

        for index in inspection.completed_prefix..plan.import_plans.len() {
            let source = sources[index];
            let fresh = RecipientHistoryRecoveryImporter::plan(
                &current,
                source.source_ordinal,
                inbox,
                imports,
                source.membership,
            )?;
            if fresh.candidate_binding() != plan.import_plans[index].candidate_binding() {
                return Err(RecipientRecoveryImportRoundError::StalePlan);
            }
            let outcome = RecipientHistoryRecoveryImporter::apply(
                &fresh,
                inbox,
                imports,
                recipient,
                source.membership,
                committed_at_unix_seconds
                    .checked_add(
                        u64::try_from(index)
                            .map_err(|_| RecipientRecoveryImportRoundError::TimestampExhausted)?,
                    )
                    .ok_or(RecipientRecoveryImportRoundError::TimestampExhausted)?,
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
                .ok_or(RecipientRecoveryImportRoundError::InvalidPlan)?;
            steps.push(RecipientRecoveryImportRoundStepReceipt {
                source_ordinal: receipt.source_ordinal,
                disposition,
                imported_event_count: receipt.imported_event_count,
                coordinator_sequence: receipt.coordinator_sequence,
            });
            current = current_coordination(plan, inbox, imports)?;
            inspection = inspect_current(plan, sources, &current, inbox, imports)?;
            if inspection.completed_prefix != index + 1 || current.requires_apply() {
                return Err(RecipientRecoveryImportRoundError::StalePlan);
            }
        }

        if steps.len() != plan.import_plans.len() {
            return Err(RecipientRecoveryImportRoundError::InvalidPlan);
        }
        Ok(RecipientRecoveryImportRoundApplyReceipt {
            round_id: plan.round_id,
            steps,
            imported_now_count,
            already_imported_count,
            imported_event_count,
            final_coordinator_sequence: current.sequence(),
            contains_message_bodies: false,
            contains_plaintext: false,
            opened_network: false,
            scheduled_background_work: false,
            concurrent_imports: 1,
        })
    }

    #[cfg(test)]
    pub(crate) fn import_plan(
        plan: &RecipientRecoveryImportRoundPlan,
        index: usize,
    ) -> Option<&RecipientRecoveryImportPlan> {
        plan.import_plans.get(index)
    }
}

struct CurrentInspection {
    completed_prefix: usize,
    completed_receipts: Vec<Option<DurableRecipientHistoryImportReceipt>>,
}

fn current_coordination(
    plan: &RecipientRecoveryImportRoundPlan,
    inbox: &RecipientHistoryTransferInbox,
    imports: &RecipientHistoryImportStore,
) -> Result<RecipientRecoveryCoordinationPlan, RecipientRecoveryImportRoundError> {
    Ok(RecipientHistoryRecoveryCoordinator::plan(
        plan.coordination_plan.state_directory(),
        plan.coordination_plan.owner_account_id(),
        plan.coordination_plan.owner_device_id(),
        plan.coordination_plan.approved_sources(),
        inbox,
        imports,
    )?)
}

fn inspect_current(
    plan: &RecipientRecoveryImportRoundPlan,
    sources: &[RecipientRecoveryImportRoundSource<'_>],
    current: &RecipientRecoveryCoordinationPlan,
    inbox: &RecipientHistoryTransferInbox,
    imports: &RecipientHistoryImportStore,
) -> Result<CurrentInspection, RecipientRecoveryImportRoundError> {
    let planned_ordinals = plan
        .import_plans
        .iter()
        .map(RecipientRecoveryImportPlan::source_ordinal)
        .collect::<HashSet<_>>();
    let mut completed_receipts = vec![None; plan.import_plans.len()];
    let mut saw_pending = false;
    let mut completed_prefix = 0_usize;

    for ordinal in 0..plan.coordination_plan.sources().len() {
        if planned_ordinals.contains(&ordinal) {
            continue;
        }
        let baseline = plan.coordination_plan.import_observation(ordinal)?;
        let observed = current.import_observation(ordinal)?;
        if observed != baseline {
            return Err(RecipientRecoveryImportRoundError::UnplannedSourceChanged(
                ordinal,
            ));
        }
    }

    for (index, import_plan) in plan.import_plans.iter().enumerate() {
        let ordinal = import_plan.source_ordinal();
        let baseline = plan.coordination_plan.import_observation(ordinal)?;
        let observed = current.import_observation(ordinal)?;
        let binding = plan.import_plans[index].candidate_binding();
        if !same_static_observation(baseline, observed)
            || binding.source_key != baseline.source_key
            || binding.expected_inbox_head_id != baseline.inbox_head_id.unwrap_or([0_u8; 32])
            || binding.expected_import_head_id != baseline.import_head_id
            || binding.staged_chunk_count != baseline.staged_chunk_count
            || binding.imported_chunk_count != baseline.imported_chunk_count
        {
            return Err(RecipientRecoveryImportRoundError::StalePlan);
        }
        if observed.imported_chunk_count == baseline.imported_chunk_count {
            if observed != baseline {
                return Err(RecipientRecoveryImportRoundError::StalePlan);
            }
            saw_pending = true;
            let candidate = RecipientHistoryRecoveryImporter::preflight_candidate_binding(
                current,
                ordinal,
                inbox,
                imports,
                sources[index].membership,
            )?;
            if candidate != binding {
                return Err(RecipientRecoveryImportRoundError::StalePlan);
            }
        } else if observed.imported_chunk_count
            == baseline
                .imported_chunk_count
                .checked_add(1)
                .ok_or(RecipientRecoveryImportRoundError::SequenceExhausted)?
        {
            if saw_pending || index != completed_prefix {
                return Err(RecipientRecoveryImportRoundError::NonPrefixProgress);
            }
            let receipt = imports.verify_staged_transfer_imported(
                inbox,
                binding.transfer_id,
                sources[index].membership,
            )?;
            let expected_events = baseline
                .imported_event_count
                .checked_add(
                    u64::try_from(receipt.imported_event_count())
                        .map_err(|_| RecipientRecoveryImportRoundError::InvalidPlan)?,
                )
                .ok_or(RecipientRecoveryImportRoundError::InvalidPlan)?;
            if receipt.execution_record_id() != binding.execution_record_id
                || observed.import_head_id != Some(receipt.receipt_id())
                || observed.imported_event_count != expected_events
            {
                return Err(RecipientRecoveryImportRoundError::StalePlan);
            }
            completed_receipts[index] = Some(receipt);
            completed_prefix += 1;
        } else {
            return Err(RecipientRecoveryImportRoundError::ProgressBeyondRound(
                ordinal,
            ));
        }
    }

    let expected_sequence = plan
        .coordination_plan
        .sequence()
        .checked_add(
            u64::try_from(completed_prefix)
                .map_err(|_| RecipientRecoveryImportRoundError::SequenceExhausted)?,
        )
        .ok_or(RecipientRecoveryImportRoundError::SequenceExhausted)?;
    if current.sequence() != expected_sequence {
        return Err(RecipientRecoveryImportRoundError::StalePlan);
    }
    Ok(CurrentInspection {
        completed_prefix,
        completed_receipts,
    })
}

fn same_static_observation(
    baseline: RecipientRecoveryCoordinatorImportObservation,
    observed: RecipientRecoveryCoordinatorImportObservation,
) -> bool {
    baseline.source_key == observed.source_key
        && baseline.source_device_id == observed.source_device_id
        && baseline.consent_record_id == observed.consent_record_id
        && baseline.selection_index == observed.selection_index
        && baseline.inbox_head_id == observed.inbox_head_id
        && baseline.staged_chunk_count == observed.staged_chunk_count
        && baseline.approved_range_start == observed.approved_range_start
        && baseline.approved_range_end == observed.approved_range_end
}

fn validate_apply_sources(
    plan: &RecipientRecoveryImportRoundPlan,
    sources: &[RecipientRecoveryImportRoundSource<'_>],
) -> Result<(), RecipientRecoveryImportRoundError> {
    if sources.len() != plan.import_plans.len() {
        return Err(RecipientRecoveryImportRoundError::ApplySourceMismatch);
    }
    for ((source, entry), import_plan) in sources
        .iter()
        .zip(&plan.wire.entries)
        .zip(&plan.import_plans)
    {
        let digest = crate::recipient_import::membership_digest(source.membership)?;
        if source.source_ordinal != usize::from(entry.source_ordinal)
            || source.source_ordinal != import_plan.source_ordinal()
            || digest != entry.membership_digest
            || source.membership.revision() != entry.membership_revision
        {
            return Err(RecipientRecoveryImportRoundError::ApplySourceMismatch);
        }
    }
    Ok(())
}

fn validate_plan(
    plan: &RecipientRecoveryImportRoundPlan,
) -> Result<(), RecipientRecoveryImportRoundError> {
    validate_wire(&plan.wire)?;
    validate_budget(plan.budget())?;
    if wire_id(&plan.wire)? != plan.round_id
        || plan.wire.coordinator_plan_id != plan.coordination_plan.plan_id()
        || plan.wire.coordinator_sequence != plan.coordination_plan.sequence()
        || plan.coordination_plan.requires_apply()
        || plan.import_plans.len() != plan.wire.entries.len()
        || plan.contains_message_bodies
        || plan.contains_plaintext
        || plan.grants_network_authority
        || plan.grants_background_authority
        || !plan.preflights_all_sources_before_import
        || !plan.imports_at_most_one_transfer_per_source
        || !plan.executes_one_import_at_a_time
    {
        return Err(RecipientRecoveryImportRoundError::InvalidPlan);
    }
    let mut seen = HashSet::with_capacity(plan.import_plans.len());
    let mut event_count = 0_usize;
    for (import_plan, entry) in plan.import_plans.iter().zip(&plan.wire.entries) {
        RecipientHistoryRecoveryImporter::verify_plan_object(import_plan)?;
        if import_plan.coordination_plan_id() != plan.coordination_plan.plan_id()
            || import_plan.plan_id() != entry.import_plan_id
            || import_plan.source_ordinal() != usize::from(entry.source_ordinal)
            || !seen.insert(import_plan.source_ordinal())
            || import_plan.planned_event_count() != usize::from(entry.event_count)
        {
            return Err(RecipientRecoveryImportRoundError::InvalidPlan);
        }
        let binding = import_plan.candidate_binding();
        if binding.membership_digest != entry.membership_digest
            || binding.membership_revision != entry.membership_revision
        {
            return Err(RecipientRecoveryImportRoundError::InvalidPlan);
        }
        event_count = event_count
            .checked_add(import_plan.planned_event_count())
            .ok_or(RecipientRecoveryImportRoundError::InvalidPlan)?;
    }
    if event_count != usize::from(plan.wire.planned_event_count) {
        return Err(RecipientRecoveryImportRoundError::InvalidPlan);
    }
    Ok(())
}

fn validate_source_count(
    count: usize,
    budget: RecipientRecoveryImportRoundBudget,
) -> Result<(), RecipientRecoveryImportRoundError> {
    if count == 0 || count > budget.max_sources {
        return Err(RecipientRecoveryImportRoundError::SourceCount {
            requested: count,
            maximum: budget.max_sources,
        });
    }
    Ok(())
}

fn validate_budget(
    budget: RecipientRecoveryImportRoundBudget,
) -> Result<(), RecipientRecoveryImportRoundError> {
    if budget.max_sources == 0
        || budget.max_sources > MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_SOURCES
        || budget.max_events == 0
        || budget.max_events > MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_EVENTS
    {
        return Err(RecipientRecoveryImportRoundError::InvalidBudget);
    }
    Ok(())
}

fn validate_wire(
    wire: &RecipientRecoveryImportRoundWire,
) -> Result<(), RecipientRecoveryImportRoundError> {
    if wire.version != ROUND_VERSION
        || wire.coordinator_plan_id == [0_u8; 32]
        || wire.entries.is_empty()
        || wire.entries.len() > usize::from(wire.max_sources)
        || wire.max_sources == 0
        || usize::from(wire.max_sources) > MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_SOURCES
        || wire.max_events == 0
        || usize::from(wire.max_events) > MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_EVENTS
        || wire.planned_event_count == 0
        || wire.planned_event_count > wire.max_events
        || wire.entries.iter().any(|entry| {
            entry.import_plan_id == [0_u8; 32]
                || entry.membership_digest == [0_u8; 32]
                || entry.membership_revision == 0
                || entry.event_count == 0
        })
    {
        return Err(RecipientRecoveryImportRoundError::InvalidPlan);
    }
    Ok(())
}

fn wire_id(
    wire: &RecipientRecoveryImportRoundWire,
) -> Result<[u8; 32], RecipientRecoveryImportRoundError> {
    validate_wire(wire)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(ROUND_ID_DOMAIN);
    hasher.update(&postcard::to_allocvec(wire)?);
    Ok(*hasher.finalize().as_bytes())
}

#[derive(Debug, Error)]
pub enum RecipientRecoveryImportRoundError {
    #[error("recipient recovery import round budget is invalid")]
    InvalidBudget,
    #[error("recipient recovery import round requested {requested} sources, maximum is {maximum}")]
    SourceCount { requested: usize, maximum: usize },
    #[error("recipient recovery import round source ordinal {0} is invalid")]
    SourceOrdinal(usize),
    #[error("recipient recovery import round source ordinal {0} is duplicated")]
    DuplicateSourceOrdinal(usize),
    #[error("recipient recovery import round planned {planned} events, maximum is {maximum}")]
    EventBudgetExceeded { planned: usize, maximum: usize },
    #[error("recipient recovery import round plan is invalid")]
    InvalidPlan,
    #[error("recipient recovery import round plan is stale")]
    StalePlan,
    #[error("recipient recovery import round apply sources do not match the plan")]
    ApplySourceMismatch,
    #[error("recipient recovery import round progress is not an exact ordered prefix")]
    NonPrefixProgress,
    #[error("recipient recovery import round source {0} progressed more than once")]
    ProgressBeyondRound(usize),
    #[error("recipient recovery import round observed a change to unplanned source {0}")]
    UnplannedSourceChanged(usize),
    #[error("recipient recovery import round was authorized for another Device")]
    WrongRecipientDevice,
    #[error("recipient recovery import round sequence is exhausted")]
    SequenceExhausted,
    #[error("recipient recovery import round timestamp is exhausted")]
    TimestampExhausted,
    #[error(transparent)]
    Coordinator(#[from] RecipientRecoveryCoordinatorError),
    #[error(transparent)]
    ImportPlan(Box<RecipientRecoveryImportPlanError>),
    #[error(transparent)]
    Import(#[from] RecipientHistoryImportError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Postcard(#[from] postcard::Error),
}

impl From<RecipientRecoveryImportPlanError> for RecipientRecoveryImportRoundError {
    fn from(error: RecipientRecoveryImportPlanError) -> Self {
        Self::ImportPlan(Box::new(error))
    }
}
