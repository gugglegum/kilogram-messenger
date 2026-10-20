use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};

use kilogram_identity::{AccountId, DeviceId};
use kilogram_peer_history_recovery_consent::{
    DurableOldHistoryRecoveryConsent, OldHistoryRecoveryConsentStoreError,
};
use kilogram_state::{StateDirectoryLock, StateError};
use redb::{
    Database, Durability, ReadOnlyDatabase, ReadableDatabase, ReadableTable, ReadableTableMetadata,
    TableDefinition,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    HistoryRecoveryTransferError, RecipientHistoryImportError, RecipientHistoryImportStore,
    RecipientHistoryTransferInbox,
};

const DATABASE_FILE: &str = "recipient-recovery-coordinator-v1.redb";
const COORDINATOR_DIRECTORY: &str = "history-recovery";
const SCHEMA_KEY: &str = "schema-version";
const OWNER_ACCOUNT_KEY: &str = "owner-account-id";
const OWNER_DEVICE_KEY: &str = "owner-device-id";
const CHAIN_HEAD_KEY: &str = "head";
const SCHEMA_VERSION: u8 = 1;
const RECORD_VERSION: u8 = 1;
const SOURCE_HEAD_VERSION: u8 = 1;
const CHAIN_HEAD_VERSION: u8 = 1;
const SOURCE_KEY_DOMAIN: &[u8] = b"kilogram:recipient-recovery-coordinator-source:v1\0";
const PLAN_ID_DOMAIN: &[u8] = b"kilogram:recipient-recovery-coordinator-plan:v1\0";

const META_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("recipient-recovery-coordinator-meta-v1");
const PLAN_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("recipient-recovery-coordinator-plan-v1");
const SOURCE_HEAD_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("recipient-recovery-coordinator-source-head-v1");
const CHAIN_HEAD_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("recipient-recovery-coordinator-chain-head-v1");

pub const MAX_RECIPIENT_RECOVERY_COORDINATOR_SOURCES: usize = 8;
pub const MAX_DURABLE_RECIPIENT_RECOVERY_COORDINATOR_PLANS: u64 = 16_384;
pub const MAX_DURABLE_RECIPIENT_RECOVERY_COORDINATOR_SOURCES: u64 = 16_384;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovedRecipientHistoryRecoverySource {
    owner_account_id: AccountId,
    source_device_id: DeviceId,
    recipient_device_id: DeviceId,
    consent_record_id: [u8; 32],
    plan_id: [u8; 32],
    selection_index: u16,
    approved_range_start: u64,
    approved_range_end: u64,
    source_key: [u8; 32],
}

impl ApprovedRecipientHistoryRecoverySource {
    pub fn from_durable_consent(
        consent: &DurableOldHistoryRecoveryConsent,
        selection_index: usize,
    ) -> Result<Self, RecipientRecoveryCoordinatorError> {
        let signed_plan = consent.signed_plan()?;
        let selection = signed_plan.selection(selection_index).ok_or(
            RecipientRecoveryCoordinatorError::SelectionMissing(selection_index),
        )?;
        let selection_index = u16::try_from(selection_index)
            .map_err(|_| RecipientRecoveryCoordinatorError::SelectionMissing(selection_index))?;
        let owner_account_id = signed_plan.history_account_id();
        let source_device_id = signed_plan.source_device_id();
        let recipient_device_id = signed_plan.replacement_device_id();
        let consent_record_id = consent.record_id();
        let plan_id = signed_plan.plan_id().map_err(|error| {
            RecipientRecoveryCoordinatorError::ApprovedSource(error.to_string())
        })?;
        let source_key = source_key(
            owner_account_id,
            source_device_id,
            recipient_device_id,
            consent_record_id,
            plan_id,
            selection_index,
        );
        Ok(Self {
            owner_account_id,
            source_device_id,
            recipient_device_id,
            consent_record_id,
            plan_id,
            selection_index,
            approved_range_start: selection.range_start(),
            approved_range_end: selection.range_end(),
            source_key,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RecipientRecoveryCoordinatorSourceState {
    AwaitingTransfer,
    StagedForImport,
    ImportInProgress,
    Complete,
}

impl RecipientRecoveryCoordinatorSourceState {
    fn rank(self) -> u8 {
        match self {
            Self::AwaitingTransfer => 0,
            Self::StagedForImport => 1,
            Self::ImportInProgress => 2,
            Self::Complete => 3,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientRecoveryCoordinatorPlannedSource {
    pub ordinal: usize,
    pub state: RecipientRecoveryCoordinatorSourceState,
    pub staged_chunk_count: u64,
    pub imported_chunk_count: u64,
    pub imported_event_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientRecoveryCoordinationPlan {
    state_directory: PathBuf,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    approved_sources: Vec<ApprovedRecipientHistoryRecoverySource>,
    record: CoordinatorPlanRecordWire,
    plan_id: [u8; 32],
    requires_apply: bool,
    sources: Vec<RecipientRecoveryCoordinatorPlannedSource>,
    pub contains_message_bodies: bool,
    pub contains_plaintext: bool,
    pub grants_network_authority: bool,
    pub grants_import_authority: bool,
    pub background_work_authorized: bool,
    pub whole_database_rollback_detection_supported: bool,
}

impl RecipientRecoveryCoordinationPlan {
    pub fn plan_id(&self) -> [u8; 32] {
        self.plan_id
    }

    pub fn sequence(&self) -> u64 {
        self.record.sequence
    }

    pub fn requires_apply(&self) -> bool {
        self.requires_apply
    }

    pub fn sources(&self) -> &[RecipientRecoveryCoordinatorPlannedSource] {
        &self.sources
    }

    pub(crate) fn state_directory(&self) -> &Path {
        &self.state_directory
    }

    pub(crate) fn owner_account_id(&self) -> AccountId {
        self.owner_account_id
    }

    pub(crate) fn owner_device_id(&self) -> DeviceId {
        self.owner_device_id
    }

    pub(crate) fn approved_sources(&self) -> &[ApprovedRecipientHistoryRecoverySource] {
        &self.approved_sources
    }

    pub(crate) fn import_observation(
        &self,
        source_ordinal: usize,
    ) -> Result<RecipientRecoveryCoordinatorImportObservation, RecipientRecoveryCoordinatorError>
    {
        let approved = self.approved_sources.get(source_ordinal).ok_or(
            RecipientRecoveryCoordinatorError::SourceOrdinalMissing(source_ordinal),
        )?;
        let observation = self.record.sources.get(source_ordinal).ok_or(
            RecipientRecoveryCoordinatorError::SourceOrdinalMissing(source_ordinal),
        )?;
        if observation.source_key != approved.source_key {
            return Err(RecipientRecoveryCoordinatorError::SourceBindingChanged);
        }
        Ok(RecipientRecoveryCoordinatorImportObservation {
            source_key: observation.source_key,
            source_device_id: approved.source_device_id,
            consent_record_id: observation.consent_record_id,
            selection_index: usize::from(approved.selection_index),
            inbox_head_id: observation.inbox_head_id,
            import_head_id: observation.import_head_id,
            staged_chunk_count: observation.staged_chunk_count,
            imported_chunk_count: observation.imported_chunk_count,
            approved_range_start: observation.approved_range_start,
            approved_range_end: observation.approved_range_end,
            state: observation.state,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RecipientRecoveryCoordinatorImportObservation {
    pub source_key: [u8; 32],
    pub source_device_id: DeviceId,
    pub consent_record_id: [u8; 32],
    pub selection_index: usize,
    pub inbox_head_id: Option<[u8; 32]>,
    pub import_head_id: Option<[u8; 32]>,
    pub staged_chunk_count: u64,
    pub imported_chunk_count: u64,
    pub approved_range_start: u64,
    pub approved_range_end: u64,
    pub state: RecipientRecoveryCoordinatorSourceState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplyRecipientRecoveryCoordinationOutcome {
    Applied(RecipientRecoveryCoordinationPlan),
    AlreadyApplied(RecipientRecoveryCoordinationPlan),
}

pub struct RecipientHistoryRecoveryCoordinator;

impl RecipientHistoryRecoveryCoordinator {
    pub fn is_initialized(state_directory: impl AsRef<Path>) -> bool {
        database_path(state_directory.as_ref()).is_file()
    }

    pub fn plan(
        state_directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
        approved_sources: &[ApprovedRecipientHistoryRecoverySource],
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
    ) -> Result<RecipientRecoveryCoordinationPlan, RecipientRecoveryCoordinatorError> {
        build_plan(
            state_directory.as_ref(),
            owner_account_id,
            owner_device_id,
            approved_sources,
            inbox,
            imports,
        )
    }

    pub fn apply(
        plan: &RecipientRecoveryCoordinationPlan,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
    ) -> Result<ApplyRecipientRecoveryCoordinationOutcome, RecipientRecoveryCoordinatorError> {
        Self::apply_inner(plan, inbox, imports, false)
    }

    pub(crate) fn verify_durable_current(
        plan: &RecipientRecoveryCoordinationPlan,
    ) -> Result<(), RecipientRecoveryCoordinatorError> {
        validate_plan_object(plan)?;
        if plan.requires_apply {
            return Err(RecipientRecoveryCoordinatorError::PlanNotApplied);
        }
        let snapshot = load_snapshot(
            &plan.state_directory,
            plan.owner_account_id,
            plan.owner_device_id,
        )?
        .ok_or(RecipientRecoveryCoordinatorError::PlanNotApplied)?;
        if snapshot.current_plan_id() != Some(plan.plan_id)
            || snapshot.current_record.as_ref() != Some(&plan.record)
        {
            return Err(RecipientRecoveryCoordinatorError::StalePlan);
        }
        Ok(())
    }

    pub(crate) fn verify_durable_current_for_stores(
        plan: &RecipientRecoveryCoordinationPlan,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
    ) -> Result<(), RecipientRecoveryCoordinatorError> {
        Self::verify_durable_current(plan)?;
        ensure_same_state_directory(&plan.state_directory, inbox.state_directory())?;
        ensure_same_state_directory(&plan.state_directory, imports.state_directory())?;
        Ok(())
    }

    pub(crate) fn verify_exact_current(
        plan: &RecipientRecoveryCoordinationPlan,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
    ) -> Result<(), RecipientRecoveryCoordinatorError> {
        Self::verify_durable_current(plan)?;
        let current = build_plan(
            &plan.state_directory,
            plan.owner_account_id,
            plan.owner_device_id,
            &plan.approved_sources,
            inbox,
            imports,
        )?;
        if current != *plan || current.requires_apply {
            return Err(RecipientRecoveryCoordinatorError::StalePlan);
        }
        Ok(())
    }

    fn apply_inner(
        plan: &RecipientRecoveryCoordinationPlan,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        fail_before_commit: bool,
    ) -> Result<ApplyRecipientRecoveryCoordinationOutcome, RecipientRecoveryCoordinatorError> {
        validate_plan_object(plan)?;
        let state_lock = StateDirectoryLock::acquire(&plan.state_directory)?;
        ensure_same_state_directory(state_lock.root(), inbox.state_directory())?;
        ensure_same_state_directory(state_lock.root(), imports.state_directory())?;

        if let Some(snapshot) = load_snapshot(
            state_lock.root(),
            plan.owner_account_id,
            plan.owner_device_id,
        )? && snapshot.current_plan_id() == Some(plan.plan_id)
        {
            let current = plan_from_snapshot(plan, &snapshot)?;
            return Ok(ApplyRecipientRecoveryCoordinationOutcome::AlreadyApplied(
                current,
            ));
        }

        let current = build_plan(
            &plan.state_directory,
            plan.owner_account_id,
            plan.owner_device_id,
            &plan.approved_sources,
            inbox,
            imports,
        )?;
        if current != *plan || !plan.requires_apply {
            return Err(RecipientRecoveryCoordinatorError::StalePlan);
        }

        let directory = coordinator_directory(state_lock.root());
        reject_symlink_if_exists(&directory)?;
        fs::create_dir_all(&directory).map_err(|source| io_error(directory.clone(), source))?;
        let path = database_path(state_lock.root());
        reject_symlink_if_exists(&path)?;
        let existed = path.is_file();
        let database = Database::create(&path).map_err(database_error)?;
        if existed {
            verify_database(&database, plan.owner_account_id, plan.owner_device_id)?;
        } else {
            initialize_database(&database, plan.owner_account_id, plan.owner_device_id)?;
        }

        let snapshot =
            snapshot_from_database(&database, plan.owner_account_id, plan.owner_device_id)?;
        if snapshot.current_plan_id() != plan.record.previous_plan_id {
            return Err(RecipientRecoveryCoordinatorError::StalePlan);
        }
        let encoded_record = canonical_record_bytes(&plan.record)?;
        let chain_head = CoordinatorChainHeadWire {
            version: CHAIN_HEAD_VERSION,
            sequence: plan.record.sequence,
            plan_id: plan.plan_id,
        };
        let encoded_chain_head = canonical_chain_head_bytes(&chain_head)?;

        let mut write = database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut plans = write.open_table(PLAN_TABLE).map_err(database_error)?;
            let mut source_heads = write
                .open_table(SOURCE_HEAD_TABLE)
                .map_err(database_error)?;
            let mut chain_heads = write.open_table(CHAIN_HEAD_TABLE).map_err(database_error)?;
            if plans.len().map_err(database_error)?
                >= MAX_DURABLE_RECIPIENT_RECOVERY_COORDINATOR_PLANS
            {
                return Err(RecipientRecoveryCoordinatorError::PlanCapacity);
            }
            if let Some(existing) = plans.get(plan.plan_id.as_slice()).map_err(database_error)? {
                if existing.value() != encoded_record.as_slice() {
                    return Err(RecipientRecoveryCoordinatorError::PlanIdCollision);
                }
                return Err(RecipientRecoveryCoordinatorError::StalePlan);
            }
            plans
                .insert(plan.plan_id.as_slice(), encoded_record.as_slice())
                .map_err(database_error)?;
            for observation in &plan.record.sources {
                if source_heads
                    .get(observation.source_key.as_slice())
                    .map_err(database_error)?
                    .is_none()
                    && source_heads.len().map_err(database_error)?
                        >= MAX_DURABLE_RECIPIENT_RECOVERY_COORDINATOR_SOURCES
                {
                    return Err(RecipientRecoveryCoordinatorError::SourceCapacity);
                }
                let head = CoordinatorSourceHeadWire {
                    version: SOURCE_HEAD_VERSION,
                    applied_plan_id: plan.plan_id,
                    observation: observation.clone(),
                };
                let encoded = canonical_source_head_bytes(&head)?;
                source_heads
                    .insert(observation.source_key.as_slice(), encoded.as_slice())
                    .map_err(database_error)?;
            }
            chain_heads
                .insert(CHAIN_HEAD_KEY, encoded_chain_head.as_slice())
                .map_err(database_error)?;
        }
        if fail_before_commit {
            return Err(RecipientRecoveryCoordinatorError::InjectedBeforeCommit);
        }
        write.commit().map_err(database_error)?;
        verify_database(&database, plan.owner_account_id, plan.owner_device_id)?;
        Ok(ApplyRecipientRecoveryCoordinationOutcome::Applied(
            plan.clone(),
        ))
    }

    #[cfg(test)]
    pub(crate) fn apply_with_fault_before_commit(
        plan: &RecipientRecoveryCoordinationPlan,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
    ) -> Result<ApplyRecipientRecoveryCoordinationOutcome, RecipientRecoveryCoordinatorError> {
        Self::apply_inner(plan, inbox, imports, true)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct CoordinatorObservationWire {
    source_key: [u8; 32],
    consent_record_id: [u8; 32],
    approved_plan_id: [u8; 32],
    inbox_head_id: Option<[u8; 32]>,
    import_head_id: Option<[u8; 32]>,
    staged_chunk_count: u64,
    imported_chunk_count: u64,
    imported_event_count: u64,
    staged_next_range_start: u64,
    imported_next_range_start: u64,
    approved_range_start: u64,
    approved_range_end: u64,
    state: RecipientRecoveryCoordinatorSourceState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct CoordinatorPlanRecordWire {
    version: u8,
    sequence: u64,
    previous_plan_id: Option<[u8; 32]>,
    sources: Vec<CoordinatorObservationWire>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct CoordinatorSourceHeadWire {
    version: u8,
    applied_plan_id: [u8; 32],
    observation: CoordinatorObservationWire,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct CoordinatorChainHeadWire {
    version: u8,
    sequence: u64,
    plan_id: [u8; 32],
}

struct CoordinatorSnapshot {
    chain_head: Option<CoordinatorChainHeadWire>,
    current_record: Option<CoordinatorPlanRecordWire>,
    source_heads: HashMap<[u8; 32], CoordinatorSourceHeadWire>,
}

impl CoordinatorSnapshot {
    fn empty() -> Self {
        Self {
            chain_head: None,
            current_record: None,
            source_heads: HashMap::new(),
        }
    }

    fn current_plan_id(&self) -> Option<[u8; 32]> {
        self.chain_head.as_ref().map(|head| head.plan_id)
    }
}

fn build_plan(
    state_directory: &Path,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    approved_sources: &[ApprovedRecipientHistoryRecoverySource],
    inbox: &RecipientHistoryTransferInbox,
    imports: &RecipientHistoryImportStore,
) -> Result<RecipientRecoveryCoordinationPlan, RecipientRecoveryCoordinatorError> {
    validate_approved_sources(owner_account_id, owner_device_id, approved_sources)?;
    ensure_same_state_directory(state_directory, inbox.state_directory())?;
    ensure_same_state_directory(state_directory, imports.state_directory())?;
    reject_coordinator_path_symlinks(state_directory)?;
    let snapshot = load_snapshot(state_directory, owner_account_id, owner_device_id)?
        .unwrap_or_else(CoordinatorSnapshot::empty);
    let mut observations = Vec::with_capacity(approved_sources.len());
    let mut sources = Vec::with_capacity(approved_sources.len());
    for (ordinal, approved) in approved_sources.iter().enumerate() {
        let observation = observe_source(approved, inbox, imports)?;
        if let Some(previous) = snapshot.source_heads.get(&approved.source_key) {
            validate_monotonic_observation(&previous.observation, &observation)?;
        }
        sources.push(RecipientRecoveryCoordinatorPlannedSource {
            ordinal,
            state: observation.state,
            staged_chunk_count: observation.staged_chunk_count,
            imported_chunk_count: observation.imported_chunk_count,
            imported_event_count: observation.imported_event_count,
        });
        observations.push(observation);
    }

    let current_matches = snapshot.current_record.as_ref().is_some_and(|record| {
        record.sources == observations
            && record.sources.len() == snapshot.source_heads.len()
            && observations.iter().all(|observation| {
                snapshot
                    .source_heads
                    .get(&observation.source_key)
                    .is_some_and(|head| head.observation == *observation)
            })
    });
    let (record, plan_id, requires_apply) = if current_matches {
        let record = snapshot
            .current_record
            .clone()
            .ok_or(RecipientRecoveryCoordinatorError::InvalidDatabase)?;
        let plan_id = record_id(&record)?;
        (record, plan_id, false)
    } else {
        let (sequence, previous_plan_id) = match snapshot.chain_head {
            Some(head) => (
                head.sequence
                    .checked_add(1)
                    .ok_or(RecipientRecoveryCoordinatorError::SequenceExhausted)?,
                Some(head.plan_id),
            ),
            None => (0, None),
        };
        let record = CoordinatorPlanRecordWire {
            version: RECORD_VERSION,
            sequence,
            previous_plan_id,
            sources: observations,
        };
        validate_record(&record)?;
        let plan_id = record_id(&record)?;
        (record, plan_id, true)
    };
    let plan = RecipientRecoveryCoordinationPlan {
        state_directory: state_directory.to_path_buf(),
        owner_account_id,
        owner_device_id,
        approved_sources: approved_sources.to_vec(),
        record,
        plan_id,
        requires_apply,
        sources,
        contains_message_bodies: false,
        contains_plaintext: false,
        grants_network_authority: false,
        grants_import_authority: false,
        background_work_authorized: false,
        whole_database_rollback_detection_supported: false,
    };
    validate_plan_object(&plan)?;
    Ok(plan)
}

fn plan_from_snapshot(
    plan: &RecipientRecoveryCoordinationPlan,
    snapshot: &CoordinatorSnapshot,
) -> Result<RecipientRecoveryCoordinationPlan, RecipientRecoveryCoordinatorError> {
    let record = snapshot
        .current_record
        .clone()
        .ok_or(RecipientRecoveryCoordinatorError::InvalidDatabase)?;
    if record_id(&record)? != plan.plan_id || record.sources.len() != plan.sources.len() {
        return Err(RecipientRecoveryCoordinatorError::PlanReplayMismatch);
    }
    let mut replay = plan.clone();
    replay.record = record;
    replay.requires_apply = false;
    validate_plan_object(&replay)?;
    Ok(replay)
}

fn observe_source(
    approved: &ApprovedRecipientHistoryRecoverySource,
    inbox: &RecipientHistoryTransferInbox,
    imports: &RecipientHistoryImportStore,
) -> Result<CoordinatorObservationWire, RecipientRecoveryCoordinatorError> {
    let inbox_head = inbox.coordinator_head(
        approved.consent_record_id,
        usize::from(approved.selection_index),
    )?;
    let import_head = imports.coordinator_head(
        approved.consent_record_id,
        usize::from(approved.selection_index),
    )?;
    if import_head.is_some() && inbox_head.is_none() {
        return Err(RecipientRecoveryCoordinatorError::ImportWithoutTransfer);
    }
    if let Some(head) = inbox_head
        && (head.approved_range_end != approved.approved_range_end
            || head.next_range_start < approved.approved_range_start
            || head.next_range_start > approved.approved_range_end
            || head.complete != (head.next_range_start >= approved.approved_range_end))
    {
        return Err(RecipientRecoveryCoordinatorError::ApprovedRangeMismatch);
    }
    if let Some(head) = import_head {
        let staged = inbox_head.ok_or(RecipientRecoveryCoordinatorError::ImportWithoutTransfer)?;
        if head.approved_range_end != approved.approved_range_end
            || head.next_range_start < approved.approved_range_start
            || head.next_range_start > staged.next_range_start
            || head.imported_chunk_count > staged.staged_chunk_count
            || head.complete != (head.next_range_start >= approved.approved_range_end)
        {
            return Err(RecipientRecoveryCoordinatorError::ImportAheadOfTransfer);
        }
    }
    let state = match (inbox_head, import_head) {
        (None, None) => RecipientRecoveryCoordinatorSourceState::AwaitingTransfer,
        (Some(_), None) => RecipientRecoveryCoordinatorSourceState::StagedForImport,
        (Some(_), Some(head)) if head.complete => RecipientRecoveryCoordinatorSourceState::Complete,
        (Some(_), Some(_)) => RecipientRecoveryCoordinatorSourceState::ImportInProgress,
        (None, Some(_)) => return Err(RecipientRecoveryCoordinatorError::ImportWithoutTransfer),
    };
    Ok(CoordinatorObservationWire {
        source_key: approved.source_key,
        consent_record_id: approved.consent_record_id,
        approved_plan_id: approved.plan_id,
        inbox_head_id: inbox_head.map(|head| head.transfer_id),
        import_head_id: import_head.map(|head| head.receipt_id),
        staged_chunk_count: inbox_head.map_or(0, |head| head.staged_chunk_count),
        imported_chunk_count: import_head.map_or(0, |head| head.imported_chunk_count),
        imported_event_count: import_head.map_or(0, |head| head.imported_event_count),
        staged_next_range_start: inbox_head
            .map_or(approved.approved_range_start, |head| head.next_range_start),
        imported_next_range_start: import_head
            .map_or(approved.approved_range_start, |head| head.next_range_start),
        approved_range_start: approved.approved_range_start,
        approved_range_end: approved.approved_range_end,
        state,
    })
}

fn validate_approved_sources(
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    approved_sources: &[ApprovedRecipientHistoryRecoverySource],
) -> Result<(), RecipientRecoveryCoordinatorError> {
    if approved_sources.is_empty()
        || approved_sources.len() > MAX_RECIPIENT_RECOVERY_COORDINATOR_SOURCES
    {
        return Err(RecipientRecoveryCoordinatorError::SourceCount(
            approved_sources.len(),
        ));
    }
    let mut device_ids = HashSet::with_capacity(approved_sources.len());
    let mut source_keys = HashSet::with_capacity(approved_sources.len());
    for source in approved_sources {
        if source.owner_account_id != owner_account_id {
            return Err(RecipientRecoveryCoordinatorError::OwnerAccountMismatch);
        }
        if source.recipient_device_id != owner_device_id {
            return Err(RecipientRecoveryCoordinatorError::RecipientDeviceMismatch);
        }
        if !device_ids.insert(source.source_device_id) {
            return Err(RecipientRecoveryCoordinatorError::DuplicateSourceDevice);
        }
        if !source_keys.insert(source.source_key) {
            return Err(RecipientRecoveryCoordinatorError::DuplicateSourceKey);
        }
        if source.approved_range_start >= source.approved_range_end
            || source.consent_record_id == [0_u8; 32]
            || source.plan_id == [0_u8; 32]
            || source.source_key == [0_u8; 32]
        {
            return Err(RecipientRecoveryCoordinatorError::InvalidApprovedSource);
        }
    }
    Ok(())
}

fn validate_monotonic_observation(
    previous: &CoordinatorObservationWire,
    current: &CoordinatorObservationWire,
) -> Result<(), RecipientRecoveryCoordinatorError> {
    if previous.source_key != current.source_key
        || previous.consent_record_id != current.consent_record_id
        || previous.approved_plan_id != current.approved_plan_id
        || previous.approved_range_start != current.approved_range_start
        || previous.approved_range_end != current.approved_range_end
    {
        return Err(RecipientRecoveryCoordinatorError::SourceBindingChanged);
    }
    if previous.state == RecipientRecoveryCoordinatorSourceState::Complete
        && current.state != RecipientRecoveryCoordinatorSourceState::Complete
    {
        return Err(RecipientRecoveryCoordinatorError::CompletedSourceRegression);
    }
    if current.staged_chunk_count < previous.staged_chunk_count
        || current.imported_chunk_count < previous.imported_chunk_count
        || current.imported_event_count < previous.imported_event_count
        || current.staged_next_range_start < previous.staged_next_range_start
        || current.imported_next_range_start < previous.imported_next_range_start
        || current.state.rank() < previous.state.rank()
        || (previous.inbox_head_id.is_some() && current.inbox_head_id.is_none())
        || (previous.import_head_id.is_some() && current.import_head_id.is_none())
    {
        return Err(RecipientRecoveryCoordinatorError::ProgressRegression);
    }
    if (current.staged_chunk_count == previous.staged_chunk_count
        && current.inbox_head_id != previous.inbox_head_id)
        || (current.imported_chunk_count == previous.imported_chunk_count
            && current.import_head_id != previous.import_head_id)
    {
        return Err(RecipientRecoveryCoordinatorError::HeadFork);
    }
    Ok(())
}

fn validate_plan_object(
    plan: &RecipientRecoveryCoordinationPlan,
) -> Result<(), RecipientRecoveryCoordinatorError> {
    validate_approved_sources(
        plan.owner_account_id,
        plan.owner_device_id,
        &plan.approved_sources,
    )?;
    validate_record(&plan.record)?;
    if record_id(&plan.record)? != plan.plan_id
        || plan.sources.len() != plan.record.sources.len()
        || plan.contains_message_bodies
        || plan.contains_plaintext
        || plan.grants_network_authority
        || plan.grants_import_authority
        || plan.background_work_authorized
        || plan.whole_database_rollback_detection_supported
    {
        return Err(RecipientRecoveryCoordinatorError::InvalidPlan);
    }
    Ok(())
}

fn validate_observation(
    observation: &CoordinatorObservationWire,
) -> Result<(), RecipientRecoveryCoordinatorError> {
    if observation.source_key == [0_u8; 32]
        || observation.consent_record_id == [0_u8; 32]
        || observation.approved_plan_id == [0_u8; 32]
        || observation.approved_range_start >= observation.approved_range_end
        || observation.staged_next_range_start < observation.approved_range_start
        || observation.staged_next_range_start > observation.approved_range_end
        || observation.imported_next_range_start < observation.approved_range_start
        || observation.imported_next_range_start > observation.staged_next_range_start
        || observation.imported_chunk_count > observation.staged_chunk_count
        || (observation.inbox_head_id.is_none() != (observation.staged_chunk_count == 0))
        || (observation.import_head_id.is_none() != (observation.imported_chunk_count == 0))
    {
        return Err(RecipientRecoveryCoordinatorError::InvalidObservation);
    }
    let expected_state = if observation.staged_chunk_count == 0 {
        RecipientRecoveryCoordinatorSourceState::AwaitingTransfer
    } else if observation.imported_chunk_count == 0 {
        RecipientRecoveryCoordinatorSourceState::StagedForImport
    } else if observation.imported_next_range_start >= observation.approved_range_end {
        RecipientRecoveryCoordinatorSourceState::Complete
    } else {
        RecipientRecoveryCoordinatorSourceState::ImportInProgress
    };
    if observation.state != expected_state {
        return Err(RecipientRecoveryCoordinatorError::InvalidObservation);
    }
    Ok(())
}

fn validate_record(
    record: &CoordinatorPlanRecordWire,
) -> Result<(), RecipientRecoveryCoordinatorError> {
    if record.version != RECORD_VERSION
        || record.sources.is_empty()
        || record.sources.len() > MAX_RECIPIENT_RECOVERY_COORDINATOR_SOURCES
        || (record.sequence == 0) != record.previous_plan_id.is_none()
    {
        return Err(RecipientRecoveryCoordinatorError::InvalidRecord);
    }
    let mut keys = HashSet::with_capacity(record.sources.len());
    for source in &record.sources {
        validate_observation(source)?;
        if !keys.insert(source.source_key) {
            return Err(RecipientRecoveryCoordinatorError::DuplicateSourceKey);
        }
    }
    Ok(())
}

fn initialize_database(
    database: &Database,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), RecipientRecoveryCoordinatorError> {
    let mut write = database.begin_write().map_err(database_error)?;
    write
        .set_durability(Durability::Immediate)
        .map_err(database_error)?;
    {
        let mut meta = write.open_table(META_TABLE).map_err(database_error)?;
        meta.insert(SCHEMA_KEY, &[SCHEMA_VERSION][..])
            .map_err(database_error)?;
        meta.insert(OWNER_ACCOUNT_KEY, owner_account_id.as_bytes().as_slice())
            .map_err(database_error)?;
        meta.insert(OWNER_DEVICE_KEY, owner_device_id.as_bytes().as_slice())
            .map_err(database_error)?;
        let _ = write.open_table(PLAN_TABLE).map_err(database_error)?;
        let _ = write
            .open_table(SOURCE_HEAD_TABLE)
            .map_err(database_error)?;
        let _ = write.open_table(CHAIN_HEAD_TABLE).map_err(database_error)?;
    }
    write.commit().map_err(database_error)
}

fn load_snapshot(
    state_directory: &Path,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<Option<CoordinatorSnapshot>, RecipientRecoveryCoordinatorError> {
    let path = database_path(state_directory);
    if !path.is_file() {
        return Ok(None);
    }
    reject_coordinator_path_symlinks(state_directory)?;
    let database = ReadOnlyDatabase::open(path).map_err(database_error)?;
    Ok(Some(snapshot_from_database(
        &database,
        owner_account_id,
        owner_device_id,
    )?))
}

fn snapshot_from_database<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<CoordinatorSnapshot, RecipientRecoveryCoordinatorError> {
    verify_database(database, owner_account_id, owner_device_id)?;
    let read = database.begin_read().map_err(database_error)?;
    let plans = read.open_table(PLAN_TABLE).map_err(database_error)?;
    let source_heads = read.open_table(SOURCE_HEAD_TABLE).map_err(database_error)?;
    let chain_heads = read.open_table(CHAIN_HEAD_TABLE).map_err(database_error)?;
    let chain_head = chain_heads
        .get(CHAIN_HEAD_KEY)
        .map_err(database_error)?
        .map(|value| decode_chain_head(value.value()))
        .transpose()?;
    let current_record = chain_head
        .as_ref()
        .map(|head| {
            plans
                .get(head.plan_id.as_slice())
                .map_err(database_error)?
                .ok_or(RecipientRecoveryCoordinatorError::InvalidDatabase)
                .and_then(|value| decode_record(value.value()))
        })
        .transpose()?;
    let mut loaded_heads = HashMap::new();
    for row in source_heads.iter().map_err(database_error)? {
        let (key, value) = row.map_err(database_error)?;
        loaded_heads.insert(copy_id(key.value())?, decode_source_head(value.value())?);
    }
    Ok(CoordinatorSnapshot {
        chain_head,
        current_record,
        source_heads: loaded_heads,
    })
}

fn verify_database<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), RecipientRecoveryCoordinatorError> {
    let read = database.begin_read().map_err(database_error)?;
    let meta = read.open_table(META_TABLE).map_err(database_error)?;
    if meta
        .get(SCHEMA_KEY)
        .map_err(database_error)?
        .is_none_or(|value| value.value() != [SCHEMA_VERSION])
        || meta
            .get(OWNER_ACCOUNT_KEY)
            .map_err(database_error)?
            .is_none_or(|value| value.value() != owner_account_id.as_bytes().as_slice())
        || meta
            .get(OWNER_DEVICE_KEY)
            .map_err(database_error)?
            .is_none_or(|value| value.value() != owner_device_id.as_bytes().as_slice())
    {
        return Err(RecipientRecoveryCoordinatorError::MetadataMismatch);
    }
    let plans_table = read.open_table(PLAN_TABLE).map_err(database_error)?;
    let source_heads_table = read.open_table(SOURCE_HEAD_TABLE).map_err(database_error)?;
    let chain_heads_table = read.open_table(CHAIN_HEAD_TABLE).map_err(database_error)?;
    if plans_table.len().map_err(database_error)? > MAX_DURABLE_RECIPIENT_RECOVERY_COORDINATOR_PLANS
        || source_heads_table.len().map_err(database_error)?
            > MAX_DURABLE_RECIPIENT_RECOVERY_COORDINATOR_SOURCES
    {
        return Err(RecipientRecoveryCoordinatorError::InvalidDatabase);
    }
    let mut plans_by_id = HashMap::new();
    let mut plans_by_sequence = HashMap::new();
    for row in plans_table.iter().map_err(database_error)? {
        let (key, value) = row.map_err(database_error)?;
        let plan_id = copy_id(key.value())?;
        let record = decode_record(value.value())?;
        if record_id(&record)? != plan_id
            || plans_by_id.insert(plan_id, record.clone()).is_some()
            || plans_by_sequence.insert(record.sequence, plan_id).is_some()
        {
            return Err(RecipientRecoveryCoordinatorError::InvalidDatabase);
        }
    }
    let chain_head = chain_heads_table
        .get(CHAIN_HEAD_KEY)
        .map_err(database_error)?
        .map(|value| decode_chain_head(value.value()))
        .transpose()?;
    if chain_heads_table.len().map_err(database_error)? > 1 {
        return Err(RecipientRecoveryCoordinatorError::InvalidDatabase);
    }
    if chain_head.is_none() {
        if !plans_by_id.is_empty() || source_heads_table.len().map_err(database_error)? != 0 {
            return Err(RecipientRecoveryCoordinatorError::InvalidDatabase);
        }
        return Ok(());
    }
    let chain_head = chain_head.ok_or(RecipientRecoveryCoordinatorError::InvalidDatabase)?;
    let expected_len = chain_head
        .sequence
        .checked_add(1)
        .ok_or(RecipientRecoveryCoordinatorError::SequenceExhausted)?;
    if u64::try_from(plans_by_id.len())
        .map_err(|_| RecipientRecoveryCoordinatorError::PlanCapacity)?
        != expected_len
        || plans_by_sequence.get(&chain_head.sequence) != Some(&chain_head.plan_id)
    {
        return Err(RecipientRecoveryCoordinatorError::InvalidDatabase);
    }
    let mut previous = None;
    for sequence in 0..=chain_head.sequence {
        let plan_id = *plans_by_sequence
            .get(&sequence)
            .ok_or(RecipientRecoveryCoordinatorError::InvalidDatabase)?;
        let record = plans_by_id
            .get(&plan_id)
            .ok_or(RecipientRecoveryCoordinatorError::InvalidDatabase)?;
        if record.previous_plan_id != previous {
            return Err(RecipientRecoveryCoordinatorError::PlanFork);
        }
        previous = Some(plan_id);
    }
    let mut durable_heads = HashMap::new();
    for row in source_heads_table.iter().map_err(database_error)? {
        let (key, value) = row.map_err(database_error)?;
        let source_key = copy_id(key.value())?;
        let head = decode_source_head(value.value())?;
        if head.observation.source_key != source_key
            || !plans_by_id
                .get(&head.applied_plan_id)
                .is_some_and(|record| record.sources.contains(&head.observation))
            || durable_heads.insert(source_key, head).is_some()
        {
            return Err(RecipientRecoveryCoordinatorError::InvalidDatabase);
        }
    }
    let mut expected_heads = HashMap::new();
    for sequence in 0..=chain_head.sequence {
        let plan_id = plans_by_sequence[&sequence];
        for observation in &plans_by_id[&plan_id].sources {
            expected_heads.insert(
                observation.source_key,
                CoordinatorSourceHeadWire {
                    version: SOURCE_HEAD_VERSION,
                    applied_plan_id: plan_id,
                    observation: observation.clone(),
                },
            );
        }
    }
    if durable_heads != expected_heads {
        return Err(RecipientRecoveryCoordinatorError::InvalidDatabase);
    }
    Ok(())
}

fn validate_source_head(
    head: &CoordinatorSourceHeadWire,
) -> Result<(), RecipientRecoveryCoordinatorError> {
    if head.version != SOURCE_HEAD_VERSION || head.applied_plan_id == [0_u8; 32] {
        return Err(RecipientRecoveryCoordinatorError::InvalidSourceHead);
    }
    validate_observation(&head.observation)
}

fn decode_record(
    encoded: &[u8],
) -> Result<CoordinatorPlanRecordWire, RecipientRecoveryCoordinatorError> {
    let record: CoordinatorPlanRecordWire = postcard::from_bytes(encoded)?;
    validate_record(&record)?;
    if canonical_record_bytes(&record)? != encoded {
        return Err(RecipientRecoveryCoordinatorError::NonCanonicalRecord);
    }
    Ok(record)
}

fn decode_source_head(
    encoded: &[u8],
) -> Result<CoordinatorSourceHeadWire, RecipientRecoveryCoordinatorError> {
    let head: CoordinatorSourceHeadWire = postcard::from_bytes(encoded)?;
    validate_source_head(&head)?;
    if canonical_source_head_bytes(&head)? != encoded {
        return Err(RecipientRecoveryCoordinatorError::NonCanonicalSourceHead);
    }
    Ok(head)
}

fn decode_chain_head(
    encoded: &[u8],
) -> Result<CoordinatorChainHeadWire, RecipientRecoveryCoordinatorError> {
    let head: CoordinatorChainHeadWire = postcard::from_bytes(encoded)?;
    if head.version != CHAIN_HEAD_VERSION || head.plan_id == [0_u8; 32] {
        return Err(RecipientRecoveryCoordinatorError::InvalidChainHead);
    }
    if canonical_chain_head_bytes(&head)? != encoded {
        return Err(RecipientRecoveryCoordinatorError::NonCanonicalChainHead);
    }
    Ok(head)
}

fn canonical_record_bytes(
    record: &CoordinatorPlanRecordWire,
) -> Result<Vec<u8>, RecipientRecoveryCoordinatorError> {
    validate_record(record)?;
    postcard::to_allocvec(record).map_err(Into::into)
}

fn canonical_source_head_bytes(
    head: &CoordinatorSourceHeadWire,
) -> Result<Vec<u8>, RecipientRecoveryCoordinatorError> {
    validate_source_head(head)?;
    postcard::to_allocvec(head).map_err(Into::into)
}

fn canonical_chain_head_bytes(
    head: &CoordinatorChainHeadWire,
) -> Result<Vec<u8>, RecipientRecoveryCoordinatorError> {
    if head.version != CHAIN_HEAD_VERSION || head.plan_id == [0_u8; 32] {
        return Err(RecipientRecoveryCoordinatorError::InvalidChainHead);
    }
    postcard::to_allocvec(head).map_err(Into::into)
}

fn record_id(
    record: &CoordinatorPlanRecordWire,
) -> Result<[u8; 32], RecipientRecoveryCoordinatorError> {
    Ok(domain_digest(
        PLAN_ID_DOMAIN,
        &canonical_record_bytes(record)?,
    ))
}

fn source_key(
    account_id: AccountId,
    source_device_id: DeviceId,
    recipient_device_id: DeviceId,
    consent_record_id: [u8; 32],
    plan_id: [u8; 32],
    selection_index: u16,
) -> [u8; 32] {
    let mut bytes = Vec::with_capacity(32 * 5 + 2);
    bytes.extend_from_slice(account_id.as_bytes());
    bytes.extend_from_slice(source_device_id.as_bytes());
    bytes.extend_from_slice(recipient_device_id.as_bytes());
    bytes.extend_from_slice(&consent_record_id);
    bytes.extend_from_slice(&plan_id);
    bytes.extend_from_slice(&selection_index.to_be_bytes());
    domain_digest(SOURCE_KEY_DOMAIN, &bytes)
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

fn coordinator_directory(state_directory: &Path) -> PathBuf {
    state_directory.join(COORDINATOR_DIRECTORY)
}

fn database_path(state_directory: &Path) -> PathBuf {
    coordinator_directory(state_directory).join(DATABASE_FILE)
}

fn reject_coordinator_path_symlinks(
    state_directory: &Path,
) -> Result<(), RecipientRecoveryCoordinatorError> {
    reject_symlink_if_exists(&coordinator_directory(state_directory))?;
    reject_symlink_if_exists(&database_path(state_directory))
}

fn reject_symlink_if_exists(path: &Path) -> Result<(), RecipientRecoveryCoordinatorError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(
            RecipientRecoveryCoordinatorError::Symlink(path.to_path_buf()),
        ),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(io_error(path.to_path_buf(), source)),
    }
}

fn ensure_same_state_directory(
    left: &Path,
    right: &Path,
) -> Result<(), RecipientRecoveryCoordinatorError> {
    if left == right {
        return Ok(());
    }
    if left.exists() && right.exists() {
        let canonical_left =
            fs::canonicalize(left).map_err(|source| io_error(left.to_path_buf(), source))?;
        let canonical_right =
            fs::canonicalize(right).map_err(|source| io_error(right.to_path_buf(), source))?;
        if canonical_left == canonical_right {
            return Ok(());
        }
    }
    Err(RecipientRecoveryCoordinatorError::StateDirectoryMismatch)
}

fn copy_id(bytes: &[u8]) -> Result<[u8; 32], RecipientRecoveryCoordinatorError> {
    bytes
        .try_into()
        .map_err(|_| RecipientRecoveryCoordinatorError::InvalidDatabase)
}

fn database_error(error: impl std::fmt::Display) -> RecipientRecoveryCoordinatorError {
    RecipientRecoveryCoordinatorError::Database(error.to_string())
}

fn io_error(path: PathBuf, source: io::Error) -> RecipientRecoveryCoordinatorError {
    RecipientRecoveryCoordinatorError::Io { path, source }
}

#[derive(Debug, Error)]
pub enum RecipientRecoveryCoordinatorError {
    #[error(
        "recipient recovery coordinator requires 1..={MAX_RECIPIENT_RECOVERY_COORDINATOR_SOURCES} approved sources, got {0}"
    )]
    SourceCount(usize),
    #[error("approved recovery selection {0} is missing")]
    SelectionMissing(usize),
    #[error("approved recovery source is invalid")]
    InvalidApprovedSource,
    #[error("approved recovery source validation failed: {0}")]
    ApprovedSource(String),
    #[error("approved recovery source belongs to another Account")]
    OwnerAccountMismatch,
    #[error("approved recovery source targets another recipient Device")]
    RecipientDeviceMismatch,
    #[error("recovery coordinator source Device is duplicated")]
    DuplicateSourceDevice,
    #[error("recovery coordinator opaque source key is duplicated")]
    DuplicateSourceKey,
    #[error("recovery coordinator stores do not share the exact state directory")]
    StateDirectoryMismatch,
    #[error("recipient import exists without its staged transfer chain")]
    ImportWithoutTransfer,
    #[error("staged transfer does not match the approved recovery range")]
    ApprovedRangeMismatch,
    #[error("recipient import is ahead of or inconsistent with its staged transfer chain")]
    ImportAheadOfTransfer,
    #[error("durable coordinator source binding changed")]
    SourceBindingChanged,
    #[error("durable coordinator progress regressed")]
    ProgressRegression,
    #[error("a completed durable coordinator source regressed")]
    CompletedSourceRegression,
    #[error("an exact durable coordinator head fork was detected")]
    HeadFork,
    #[error("recipient recovery coordination plan is invalid")]
    InvalidPlan,
    #[error("recipient recovery coordinator observation is invalid")]
    InvalidObservation,
    #[error("recipient recovery coordinator plan record is invalid")]
    InvalidRecord,
    #[error("recipient recovery coordinator source head is invalid")]
    InvalidSourceHead,
    #[error("recipient recovery coordinator chain head is invalid")]
    InvalidChainHead,
    #[error("recipient recovery coordinator record is not canonical")]
    NonCanonicalRecord,
    #[error("recipient recovery coordinator source head is not canonical")]
    NonCanonicalSourceHead,
    #[error("recipient recovery coordinator chain head is not canonical")]
    NonCanonicalChainHead,
    #[error("recipient recovery coordinator database metadata does not match its owner")]
    MetadataMismatch,
    #[error("recipient recovery coordinator database is inconsistent")]
    InvalidDatabase,
    #[error("recipient recovery coordinator plan chain fork was detected")]
    PlanFork,
    #[error("recipient recovery coordination plan is stale")]
    StalePlan,
    #[error("recipient recovery coordination plan has not been durably applied")]
    PlanNotApplied,
    #[error("recipient recovery coordinator source ordinal {0} is missing")]
    SourceOrdinalMissing(usize),
    #[error("recipient recovery coordination replay does not match its durable plan")]
    PlanReplayMismatch,
    #[error("recipient recovery coordinator sequence is exhausted")]
    SequenceExhausted,
    #[error("recipient recovery coordinator plan capacity is exhausted")]
    PlanCapacity,
    #[error("recipient recovery coordinator source capacity is exhausted")]
    SourceCapacity,
    #[error("recipient recovery coordinator plan ID collision")]
    PlanIdCollision,
    #[error("recipient recovery coordinator path is a symlink: {0}")]
    Symlink(PathBuf),
    #[error("recipient recovery coordinator database error: {0}")]
    Database(String),
    #[error("recipient recovery coordinator I/O failed at {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("recipient recovery coordinator injected failure before commit")]
    InjectedBeforeCommit,
    #[error(transparent)]
    Consent(#[from] OldHistoryRecoveryConsentStoreError),
    #[error(transparent)]
    Transfer(#[from] HistoryRecoveryTransferError),
    #[error(transparent)]
    Import(#[from] RecipientHistoryImportError),
    #[error(transparent)]
    State(#[from] StateError),
    #[error(transparent)]
    Postcard(#[from] postcard::Error),
}
