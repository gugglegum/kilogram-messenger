use kilogram_identity::{ConversationMembershipSnapshot, DeviceIdentity, IdentityError};
use kilogram_state::{StateDirectoryLock, StateError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ApplyRecipientRecoveryCoordinationOutcome, HistoryRecoveryTransferError,
    RecipientHistoryImportError, RecipientHistoryImportOutcome, RecipientHistoryImportStore,
    RecipientHistoryRecoveryCoordinator, RecipientHistoryTransferInbox,
    RecipientRecoveryCoordinationPlan, RecipientRecoveryCoordinatorError,
    RecipientRecoveryCoordinatorSourceState,
};

const PLAN_VERSION: u8 = 1;
const PLAN_ID_DOMAIN: &[u8] = b"kilogram:recipient-recovery-import-plan:v1\0";
const MEMBERSHIP_DIGEST_DOMAIN: &[u8] = b"kilogram:recipient-recovery-import-membership:v1\0";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RecipientRecoveryImportPlanWire {
    version: u8,
    coordinator_plan_id: [u8; 32],
    source_key: [u8; 32],
    source_ordinal: u16,
    transfer_id: [u8; 32],
    execution_record_id: [u8; 32],
    expected_inbox_head_id: [u8; 32],
    expected_import_head_id: Option<[u8; 32]>,
    staged_chunk_count: u64,
    imported_chunk_count: u64,
    range_start: u64,
    range_end: u64,
    approved_range_end: u64,
    event_count: u16,
    membership_digest: [u8; 32],
    membership_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientRecoveryImportPlan {
    coordination_plan: RecipientRecoveryCoordinationPlan,
    wire: RecipientRecoveryImportPlanWire,
    plan_id: [u8; 32],
    pub contains_message_bodies: bool,
    pub contains_plaintext: bool,
    pub grants_network_authority: bool,
    pub grants_background_authority: bool,
    pub imports_exactly_one_transfer: bool,
}

impl RecipientRecoveryImportPlan {
    pub fn plan_id(&self) -> [u8; 32] {
        self.plan_id
    }

    pub fn source_ordinal(&self) -> usize {
        usize::from(self.wire.source_ordinal)
    }

    pub fn staged_chunk_count(&self) -> u64 {
        self.wire.staged_chunk_count
    }

    pub fn imported_chunk_count(&self) -> u64 {
        self.wire.imported_chunk_count
    }

    pub fn planned_event_count(&self) -> usize {
        usize::from(self.wire.event_count)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecipientRecoveryCoordinatorRefreshDisposition {
    Applied,
    AlreadyApplied,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientRecoveryImportApplyReceipt {
    pub source_ordinal: usize,
    pub imported_event_count: usize,
    pub coordinator_sequence: u64,
    pub coordinator_refresh: RecipientRecoveryCoordinatorRefreshDisposition,
    pub contains_message_bodies: bool,
    pub contains_plaintext: bool,
    pub opened_network: bool,
    pub scheduled_background_work: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplyRecipientRecoveryImportOutcome {
    Imported(RecipientRecoveryImportApplyReceipt),
    AlreadyImported(RecipientRecoveryImportApplyReceipt),
}

pub struct RecipientHistoryRecoveryImporter;

impl RecipientHistoryRecoveryImporter {
    pub fn plan(
        coordination_plan: &RecipientRecoveryCoordinationPlan,
        source_ordinal: usize,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        membership: &ConversationMembershipSnapshot,
    ) -> Result<RecipientRecoveryImportPlan, RecipientRecoveryImportPlanError> {
        RecipientHistoryRecoveryCoordinator::verify_exact_current(
            coordination_plan,
            inbox,
            imports,
        )?;
        let observation = coordination_plan.import_observation(source_ordinal)?;
        if !matches!(
            observation.state,
            RecipientRecoveryCoordinatorSourceState::StagedForImport
                | RecipientRecoveryCoordinatorSourceState::ImportInProgress
        ) || observation.staged_chunk_count <= observation.imported_chunk_count
        {
            return Err(RecipientRecoveryImportPlanError::NoPendingTransfer(
                source_ordinal,
            ));
        }
        let expected_inbox_head_id = observation.inbox_head_id.ok_or(
            RecipientRecoveryImportPlanError::NoPendingTransfer(source_ordinal),
        )?;
        let transfer = inbox.coordinator_transfer_at(
            observation.consent_record_id,
            observation.selection_index,
            observation.imported_chunk_count,
        )?;
        let candidate = imports.plan_staged_transfer(inbox, transfer.transfer_id(), membership)?;
        if candidate.source_device_id != observation.source_device_id
            || candidate.recipient_device_id != coordination_plan.owner_device_id()
            || candidate.consent_record_id != observation.consent_record_id
            || candidate.selection_index != observation.selection_index
            || candidate.approved_range_end != observation.approved_range_end
            || candidate.range_start < observation.approved_range_start
            || candidate.range_end > observation.approved_range_end
        {
            return Err(RecipientRecoveryImportPlanError::SourceBindingMismatch);
        }
        let source_ordinal_u16 = u16::try_from(source_ordinal)
            .map_err(|_| RecipientRecoveryImportPlanError::SourceOrdinal(source_ordinal))?;
        let event_count = u16::try_from(candidate.event_count)
            .map_err(|_| RecipientRecoveryImportPlanError::InvalidPlan)?;
        let wire = RecipientRecoveryImportPlanWire {
            version: PLAN_VERSION,
            coordinator_plan_id: coordination_plan.plan_id(),
            source_key: observation.source_key,
            source_ordinal: source_ordinal_u16,
            transfer_id: transfer.transfer_id(),
            execution_record_id: candidate.execution_record_id,
            expected_inbox_head_id,
            expected_import_head_id: observation.import_head_id,
            staged_chunk_count: observation.staged_chunk_count,
            imported_chunk_count: observation.imported_chunk_count,
            range_start: candidate.range_start,
            range_end: candidate.range_end,
            approved_range_end: candidate.approved_range_end,
            event_count,
            membership_digest: membership_digest(membership)?,
            membership_revision: membership.revision(),
        };
        validate_wire(&wire)?;
        let plan_id = wire_id(&wire)?;
        let plan = RecipientRecoveryImportPlan {
            coordination_plan: coordination_plan.clone(),
            wire,
            plan_id,
            contains_message_bodies: false,
            contains_plaintext: false,
            grants_network_authority: false,
            grants_background_authority: false,
            imports_exactly_one_transfer: true,
        };
        validate_plan(&plan)?;
        Ok(plan)
    }

    pub fn apply(
        plan: &RecipientRecoveryImportPlan,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        recipient: &DeviceIdentity,
        membership: &ConversationMembershipSnapshot,
        committed_at_unix_seconds: u64,
    ) -> Result<ApplyRecipientRecoveryImportOutcome, RecipientRecoveryImportPlanError> {
        Self::apply_inner(
            plan,
            inbox,
            imports,
            recipient,
            membership,
            committed_at_unix_seconds,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_inner(
        plan: &RecipientRecoveryImportPlan,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        recipient: &DeviceIdentity,
        membership: &ConversationMembershipSnapshot,
        committed_at_unix_seconds: u64,
        fail_after_import: bool,
    ) -> Result<ApplyRecipientRecoveryImportOutcome, RecipientRecoveryImportPlanError> {
        validate_plan(plan)?;
        if recipient.device_id() != plan.coordination_plan.owner_device_id() {
            return Err(RecipientRecoveryImportPlanError::WrongRecipientDevice);
        }
        if membership_digest(membership)? != plan.wire.membership_digest
            || membership.revision() != plan.wire.membership_revision
        {
            return Err(RecipientRecoveryImportPlanError::MembershipChanged);
        }
        let state_lock = StateDirectoryLock::acquire(plan.coordination_plan.state_directory())?;
        RecipientHistoryRecoveryCoordinator::verify_durable_current_for_stores(
            &plan.coordination_plan,
            inbox,
            imports,
        )?;
        let observation = plan
            .coordination_plan
            .import_observation(plan.source_ordinal())?;
        if observation.source_key != plan.wire.source_key
            || observation.inbox_head_id != Some(plan.wire.expected_inbox_head_id)
            || observation.import_head_id != plan.wire.expected_import_head_id
            || observation.staged_chunk_count != plan.wire.staged_chunk_count
            || observation.imported_chunk_count != plan.wire.imported_chunk_count
        {
            return Err(RecipientRecoveryImportPlanError::StalePlan);
        }
        let inbox_head = inbox
            .coordinator_head(observation.consent_record_id, observation.selection_index)?
            .ok_or(RecipientRecoveryImportPlanError::StalePlan)?;
        if inbox_head.transfer_id != plan.wire.expected_inbox_head_id
            || inbox_head.staged_chunk_count != plan.wire.staged_chunk_count
            || inbox_head.approved_range_end != plan.wire.approved_range_end
        {
            return Err(RecipientRecoveryImportPlanError::StalePlan);
        }
        let transfer = inbox.coordinator_transfer_at(
            observation.consent_record_id,
            observation.selection_index,
            plan.wire.imported_chunk_count,
        )?;
        if transfer.transfer_id() != plan.wire.transfer_id
            || transfer.execution_record_id() != plan.wire.execution_record_id
        {
            return Err(RecipientRecoveryImportPlanError::StalePlan);
        }

        let import_outcome = imports.import_staged_transfer_with_state_lock(
            inbox,
            plan.wire.transfer_id,
            recipient,
            membership,
            committed_at_unix_seconds,
        )?;
        drop(state_lock);
        if fail_after_import {
            return Err(RecipientRecoveryImportPlanError::InjectedAfterImport);
        }

        let refreshed = RecipientHistoryRecoveryCoordinator::plan(
            plan.coordination_plan.state_directory(),
            plan.coordination_plan.owner_account_id(),
            plan.coordination_plan.owner_device_id(),
            plan.coordination_plan.approved_sources(),
            inbox,
            imports,
        )
        .map_err(RecipientRecoveryImportPlanError::CoordinatorRefreshAfterImport)?;
        let coordinator_sequence = refreshed.sequence();
        let coordinator_refresh =
            match RecipientHistoryRecoveryCoordinator::apply(&refreshed, inbox, imports)
                .map_err(RecipientRecoveryImportPlanError::CoordinatorRefreshAfterImport)?
            {
                ApplyRecipientRecoveryCoordinationOutcome::Applied(_) => {
                    RecipientRecoveryCoordinatorRefreshDisposition::Applied
                }
                ApplyRecipientRecoveryCoordinationOutcome::AlreadyApplied(_) => {
                    RecipientRecoveryCoordinatorRefreshDisposition::AlreadyApplied
                }
            };
        let (imported, receipt) = match import_outcome {
            RecipientHistoryImportOutcome::Imported(receipt) => (true, receipt),
            RecipientHistoryImportOutcome::AlreadyImported(receipt) => (false, receipt),
        };
        let receipt = RecipientRecoveryImportApplyReceipt {
            source_ordinal: plan.source_ordinal(),
            imported_event_count: receipt.imported_event_count(),
            coordinator_sequence,
            coordinator_refresh,
            contains_message_bodies: false,
            contains_plaintext: false,
            opened_network: false,
            scheduled_background_work: false,
        };
        if imported {
            Ok(ApplyRecipientRecoveryImportOutcome::Imported(receipt))
        } else {
            Ok(ApplyRecipientRecoveryImportOutcome::AlreadyImported(
                receipt,
            ))
        }
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply_with_fault_after_import(
        plan: &RecipientRecoveryImportPlan,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        recipient: &DeviceIdentity,
        membership: &ConversationMembershipSnapshot,
        committed_at_unix_seconds: u64,
    ) -> Result<ApplyRecipientRecoveryImportOutcome, RecipientRecoveryImportPlanError> {
        Self::apply_inner(
            plan,
            inbox,
            imports,
            recipient,
            membership,
            committed_at_unix_seconds,
            true,
        )
    }
}

fn validate_plan(
    plan: &RecipientRecoveryImportPlan,
) -> Result<(), RecipientRecoveryImportPlanError> {
    validate_wire(&plan.wire)?;
    let observation = plan
        .coordination_plan
        .import_observation(plan.source_ordinal())?;
    if wire_id(&plan.wire)? != plan.plan_id
        || plan.wire.coordinator_plan_id != plan.coordination_plan.plan_id()
        || plan.coordination_plan.requires_apply()
        || observation.source_key != plan.wire.source_key
        || observation.inbox_head_id != Some(plan.wire.expected_inbox_head_id)
        || observation.import_head_id != plan.wire.expected_import_head_id
        || observation.staged_chunk_count != plan.wire.staged_chunk_count
        || observation.imported_chunk_count != plan.wire.imported_chunk_count
        || observation.approved_range_end != plan.wire.approved_range_end
        || plan.contains_message_bodies
        || plan.contains_plaintext
        || plan.grants_network_authority
        || plan.grants_background_authority
        || !plan.imports_exactly_one_transfer
    {
        return Err(RecipientRecoveryImportPlanError::InvalidPlan);
    }
    Ok(())
}

fn validate_wire(
    wire: &RecipientRecoveryImportPlanWire,
) -> Result<(), RecipientRecoveryImportPlanError> {
    if wire.version != PLAN_VERSION
        || wire.coordinator_plan_id == [0_u8; 32]
        || wire.source_key == [0_u8; 32]
        || wire.transfer_id == [0_u8; 32]
        || wire.execution_record_id == [0_u8; 32]
        || wire.expected_inbox_head_id == [0_u8; 32]
        || wire.membership_digest == [0_u8; 32]
        || wire.staged_chunk_count == 0
        || wire.staged_chunk_count <= wire.imported_chunk_count
        || wire.range_start >= wire.range_end
        || wire.range_end > wire.approved_range_end
        || wire.event_count == 0
        || wire.membership_revision == 0
    {
        return Err(RecipientRecoveryImportPlanError::InvalidPlan);
    }
    Ok(())
}

fn membership_digest(
    membership: &ConversationMembershipSnapshot,
) -> Result<[u8; 32], RecipientRecoveryImportPlanError> {
    membership.verify()?;
    Ok(domain_digest(
        MEMBERSHIP_DIGEST_DOMAIN,
        &membership.encode()?,
    ))
}

fn wire_id(
    wire: &RecipientRecoveryImportPlanWire,
) -> Result<[u8; 32], RecipientRecoveryImportPlanError> {
    validate_wire(wire)?;
    Ok(domain_digest(PLAN_ID_DOMAIN, &postcard::to_allocvec(wire)?))
}

fn domain_digest(domain: &[u8], payload: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(payload);
    *hasher.finalize().as_bytes()
}

#[derive(Debug, Error)]
pub enum RecipientRecoveryImportPlanError {
    #[error("recipient recovery import source ordinal {0} is invalid")]
    SourceOrdinal(usize),
    #[error("recipient recovery source ordinal {0} has no pending staged transfer")]
    NoPendingTransfer(usize),
    #[error("recipient recovery import source binding does not match the coordinator")]
    SourceBindingMismatch,
    #[error("recipient recovery import plan is invalid")]
    InvalidPlan,
    #[error("recipient recovery import plan is stale")]
    StalePlan,
    #[error("recipient recovery import membership changed after planning")]
    MembershipChanged,
    #[error("recipient recovery import was authorized for another Device")]
    WrongRecipientDevice,
    #[error("recipient recovery import committed before an injected coordinator refresh failure")]
    InjectedAfterImport,
    #[error("recipient recovery import committed, but coordinator refresh failed: {0}")]
    CoordinatorRefreshAfterImport(RecipientRecoveryCoordinatorError),
    #[error(transparent)]
    Coordinator(#[from] RecipientRecoveryCoordinatorError),
    #[error(transparent)]
    Transfer(#[from] HistoryRecoveryTransferError),
    #[error(transparent)]
    Import(#[from] RecipientHistoryImportError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    State(#[from] StateError),
    #[error(transparent)]
    Postcard(#[from] postcard::Error),
}
