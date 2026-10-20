use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
};

use kilogram_identity::{AccountId, DeviceId, DeviceIdentity, IdentityError};
use kilogram_state::{StateDirectoryLock, StateError};
use redb::{
    Database, Durability, ReadOnlyDatabase, ReadableDatabase, ReadableTable, ReadableTableMetadata,
    TableDefinition,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::coordinator::RecipientRecoveryCoordinatorImportObservation;
use crate::recipient_import::RecipientRecoveryImportCandidateBinding;
use crate::{
    DurableRecipientHistoryImportReceipt, MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_EVENTS,
    MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_SOURCES, RecipientHistoryImportError,
    RecipientHistoryImportStore, RecipientHistoryRecoveryCoordinator,
    RecipientHistoryRecoveryImporter, RecipientHistoryTransferInbox,
    RecipientRecoveryCoordinationPlan, RecipientRecoveryCoordinatorError,
    RecipientRecoveryImportPlanError, RecipientRecoveryImportRoundError,
    RecipientRecoveryImportRoundPlan, RecipientRecoveryImportRoundSource,
};

const INTENT_DIRECTORY: &str = "history-recovery";
const DATABASE_FILE: &str = "recipient-import-round-intents-v1.redb";
const SCHEMA_KEY: &str = "schema-version";
const OWNER_ACCOUNT_KEY: &str = "owner-account-id";
const OWNER_DEVICE_KEY: &str = "owner-device-id";
const ACTIVE_KEY: &str = "active";
const SCHEMA_VERSION: u8 = 1;
const INTENT_VERSION: u8 = 1;
const CONSUMPTION_VERSION: u8 = 1;
const INTENT_SIGNATURE_DOMAIN: &[u8] = b"kilogram:recipient-recovery-import-round-intent:v1\0";
const INTENT_ID_DOMAIN: &[u8] = b"kilogram:recipient-recovery-import-round-intent-id:v1\0";
const CONSUMPTION_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:recipient-recovery-import-round-consumption:v1\0";
const CONSUMPTION_ID_DOMAIN: &[u8] =
    b"kilogram:recipient-recovery-import-round-consumption-id:v1\0";

const META_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("recipient-import-round-intent-meta-v1");
const INTENT_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("recipient-import-round-intent-record-v1");
const ACTIVE_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("recipient-import-round-intent-active-v1");
const CONSUMED_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("recipient-import-round-intent-consumed-v1");

pub const MAX_DURABLE_RECIPIENT_RECOVERY_IMPORT_ROUND_INTENTS: u64 = 4_096;
pub const MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_INTENT_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct IntentCoordinatorSourceBaseline {
    source_key: [u8; 32],
    inbox_head_id: Option<[u8; 32]>,
    import_head_id: Option<[u8; 32]>,
    staged_chunk_count: u64,
    imported_chunk_count: u64,
    imported_event_count: u64,
    approved_range_start: u64,
    approved_range_end: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct IntentImportEntry {
    source_ordinal: u16,
    import_plan_id: [u8; 32],
    source_key: [u8; 32],
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

impl IntentImportEntry {
    fn candidate_binding(self) -> RecipientRecoveryImportCandidateBinding {
        RecipientRecoveryImportCandidateBinding {
            source_key: self.source_key,
            source_ordinal: usize::from(self.source_ordinal),
            transfer_id: self.transfer_id,
            execution_record_id: self.execution_record_id,
            expected_inbox_head_id: self.expected_inbox_head_id,
            expected_import_head_id: self.expected_import_head_id,
            staged_chunk_count: self.staged_chunk_count,
            imported_chunk_count: self.imported_chunk_count,
            range_start: self.range_start,
            range_end: self.range_end,
            approved_range_end: self.approved_range_end,
            event_count: usize::from(self.event_count),
            membership_digest: self.membership_digest,
            membership_revision: self.membership_revision,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RecipientRecoveryImportRoundIntentContent {
    version: u8,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    round_id: [u8; 32],
    coordinator_plan_id: [u8; 32],
    coordinator_sequence: u64,
    max_sources: u8,
    max_events: u16,
    planned_event_count: u16,
    coordinator_sources: Vec<IntentCoordinatorSourceBaseline>,
    entries: Vec<IntentImportEntry>,
    committed_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SignedRecipientRecoveryImportRoundIntentWire {
    content: RecipientRecoveryImportRoundIntentContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedRecipientRecoveryImportRoundIntent {
    content: RecipientRecoveryImportRoundIntentContent,
    signature: Vec<u8>,
}

impl SignedRecipientRecoveryImportRoundIntent {
    fn issue(
        plan: &RecipientRecoveryImportRoundPlan,
        recipient: &DeviceIdentity,
        committed_at_unix_seconds: u64,
    ) -> Result<Self, RecipientRecoveryImportRoundIntentError> {
        let coordination = plan.coordination_plan();
        if recipient.device_id() != coordination.owner_device_id() {
            return Err(RecipientRecoveryImportRoundIntentError::OwnerDeviceMismatch);
        }
        let mut coordinator_sources = Vec::with_capacity(coordination.sources().len());
        for ordinal in 0..coordination.sources().len() {
            let observation = coordination.import_observation(ordinal)?;
            coordinator_sources.push(baseline_from_observation(observation));
        }
        let mut entries = Vec::with_capacity(plan.import_plans().len());
        for import_plan in plan.import_plans() {
            let binding = import_plan.candidate_binding();
            entries.push(IntentImportEntry {
                source_ordinal: u16::try_from(binding.source_ordinal)
                    .map_err(|_| RecipientRecoveryImportRoundIntentError::InvalidIntent)?,
                import_plan_id: import_plan.plan_id(),
                source_key: binding.source_key,
                transfer_id: binding.transfer_id,
                execution_record_id: binding.execution_record_id,
                expected_inbox_head_id: binding.expected_inbox_head_id,
                expected_import_head_id: binding.expected_import_head_id,
                staged_chunk_count: binding.staged_chunk_count,
                imported_chunk_count: binding.imported_chunk_count,
                range_start: binding.range_start,
                range_end: binding.range_end,
                approved_range_end: binding.approved_range_end,
                event_count: u16::try_from(binding.event_count)
                    .map_err(|_| RecipientRecoveryImportRoundIntentError::InvalidIntent)?,
                membership_digest: binding.membership_digest,
                membership_revision: binding.membership_revision,
            });
        }
        let budget = plan.budget();
        let content = RecipientRecoveryImportRoundIntentContent {
            version: INTENT_VERSION,
            owner_account_id: coordination.owner_account_id(),
            owner_device_id: coordination.owner_device_id(),
            round_id: plan.round_id(),
            coordinator_plan_id: coordination.plan_id(),
            coordinator_sequence: coordination.sequence(),
            max_sources: u8::try_from(budget.max_sources())
                .map_err(|_| RecipientRecoveryImportRoundIntentError::InvalidIntent)?,
            max_events: u16::try_from(budget.max_events())
                .map_err(|_| RecipientRecoveryImportRoundIntentError::InvalidIntent)?,
            planned_event_count: u16::try_from(plan.planned_event_count())
                .map_err(|_| RecipientRecoveryImportRoundIntentError::InvalidIntent)?,
            coordinator_sources,
            entries,
            committed_at_unix_seconds,
        };
        validate_intent_content(&content)?;
        let signature = recipient.sign(&intent_signing_bytes(&content)?).to_vec();
        let intent = Self { content, signature };
        intent.verify()?;
        Ok(intent)
    }

    fn decode_and_verify(
        encoded: &[u8],
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Result<Self, RecipientRecoveryImportRoundIntentError> {
        if encoded.is_empty() || encoded.len() > MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_INTENT_BYTES {
            return Err(RecipientRecoveryImportRoundIntentError::IntentLength(
                encoded.len(),
            ));
        }
        let wire: SignedRecipientRecoveryImportRoundIntentWire = postcard::from_bytes(encoded)?;
        let intent = Self {
            content: wire.content,
            signature: wire.signature,
        };
        intent.verify()?;
        if intent.owner_account_id() != owner_account_id {
            return Err(RecipientRecoveryImportRoundIntentError::OwnerAccountMismatch);
        }
        if intent.owner_device_id() != owner_device_id {
            return Err(RecipientRecoveryImportRoundIntentError::OwnerDeviceMismatch);
        }
        if intent.encode()? != encoded {
            return Err(RecipientRecoveryImportRoundIntentError::NonCanonicalIntent);
        }
        Ok(intent)
    }

    pub fn encode(&self) -> Result<Vec<u8>, RecipientRecoveryImportRoundIntentError> {
        self.verify()?;
        let encoded = postcard::to_allocvec(&SignedRecipientRecoveryImportRoundIntentWire {
            content: self.content.clone(),
            signature: self.signature.clone(),
        })?;
        if encoded.is_empty() || encoded.len() > MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_INTENT_BYTES {
            return Err(RecipientRecoveryImportRoundIntentError::IntentLength(
                encoded.len(),
            ));
        }
        Ok(encoded)
    }

    pub fn intent_id(&self) -> Result<[u8; 32], RecipientRecoveryImportRoundIntentError> {
        Ok(domain_digest(INTENT_ID_DOMAIN, &self.encode()?))
    }

    pub fn owner_account_id(&self) -> AccountId {
        self.content.owner_account_id
    }

    pub fn owner_device_id(&self) -> DeviceId {
        self.content.owner_device_id
    }

    pub fn round_id(&self) -> [u8; 32] {
        self.content.round_id
    }

    pub fn coordinator_plan_id(&self) -> [u8; 32] {
        self.content.coordinator_plan_id
    }

    pub fn coordinator_sequence(&self) -> u64 {
        self.content.coordinator_sequence
    }

    pub fn source_count(&self) -> usize {
        self.content.entries.len()
    }

    pub fn planned_event_count(&self) -> usize {
        usize::from(self.content.planned_event_count)
    }

    pub fn source_ordinals(&self) -> impl ExactSizeIterator<Item = usize> + '_ {
        self.content
            .entries
            .iter()
            .map(|entry| usize::from(entry.source_ordinal))
    }

    pub fn import_plan_ids(&self) -> impl ExactSizeIterator<Item = [u8; 32]> + '_ {
        self.content
            .entries
            .iter()
            .map(|entry| entry.import_plan_id)
    }

    pub fn committed_at_unix_seconds(&self) -> u64 {
        self.content.committed_at_unix_seconds
    }

    fn same_intent_as(&self, other: &Self) -> bool {
        let mut left = self.content.clone();
        let mut right = other.content.clone();
        left.committed_at_unix_seconds = 0;
        right.committed_at_unix_seconds = 0;
        left == right
    }

    fn verify(&self) -> Result<(), RecipientRecoveryImportRoundIntentError> {
        validate_intent_content(&self.content)?;
        self.content
            .owner_device_id
            .verify(&intent_signing_bytes(&self.content)?, &self.signature)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RecipientRecoveryImportRoundConsumptionContent {
    version: u8,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    round_id: [u8; 32],
    intent_id: [u8; 32],
    final_coordinator_plan_id: [u8; 32],
    final_coordinator_sequence: u64,
    completed_source_count: u16,
    imported_event_count: u64,
    consumed_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SignedRecipientRecoveryImportRoundConsumptionWire {
    content: RecipientRecoveryImportRoundConsumptionContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedRecipientRecoveryImportRoundConsumption {
    content: RecipientRecoveryImportRoundConsumptionContent,
    signature: Vec<u8>,
}

impl SignedRecipientRecoveryImportRoundConsumption {
    fn issue(
        intent: &SignedRecipientRecoveryImportRoundIntent,
        progress: &RecipientRecoveryImportRoundIntentProgress,
        recipient: &DeviceIdentity,
        consumed_at_unix_seconds: u64,
    ) -> Result<Self, RecipientRecoveryImportRoundIntentError> {
        if recipient.device_id() != intent.owner_device_id() {
            return Err(RecipientRecoveryImportRoundIntentError::OwnerDeviceMismatch);
        }
        let content = RecipientRecoveryImportRoundConsumptionContent {
            version: CONSUMPTION_VERSION,
            owner_account_id: intent.owner_account_id(),
            owner_device_id: intent.owner_device_id(),
            round_id: intent.round_id(),
            intent_id: intent.intent_id()?,
            final_coordinator_plan_id: progress.coordinator_plan_id,
            final_coordinator_sequence: progress.coordinator_sequence,
            completed_source_count: u16::try_from(progress.completed_source_count)
                .map_err(|_| RecipientRecoveryImportRoundIntentError::InvalidConsumption)?,
            imported_event_count: u64::try_from(progress.imported_event_count)
                .map_err(|_| RecipientRecoveryImportRoundIntentError::InvalidConsumption)?,
            consumed_at_unix_seconds,
        };
        validate_consumption_content(&content, intent)?;
        let signature = recipient
            .sign(&consumption_signing_bytes(&content)?)
            .to_vec();
        let consumption = Self { content, signature };
        consumption.verify(intent)?;
        Ok(consumption)
    }

    fn decode_and_verify(
        encoded: &[u8],
        intent: &SignedRecipientRecoveryImportRoundIntent,
    ) -> Result<Self, RecipientRecoveryImportRoundIntentError> {
        if encoded.is_empty() || encoded.len() > MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_INTENT_BYTES {
            return Err(RecipientRecoveryImportRoundIntentError::ConsumptionLength(
                encoded.len(),
            ));
        }
        let wire: SignedRecipientRecoveryImportRoundConsumptionWire =
            postcard::from_bytes(encoded)?;
        let consumption = Self {
            content: wire.content,
            signature: wire.signature,
        };
        consumption.verify(intent)?;
        if consumption.encode(intent)? != encoded {
            return Err(RecipientRecoveryImportRoundIntentError::NonCanonicalConsumption);
        }
        Ok(consumption)
    }

    fn encode(
        &self,
        intent: &SignedRecipientRecoveryImportRoundIntent,
    ) -> Result<Vec<u8>, RecipientRecoveryImportRoundIntentError> {
        self.verify(intent)?;
        let encoded = postcard::to_allocvec(&SignedRecipientRecoveryImportRoundConsumptionWire {
            content: self.content.clone(),
            signature: self.signature.clone(),
        })?;
        if encoded.is_empty() || encoded.len() > MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_INTENT_BYTES {
            return Err(RecipientRecoveryImportRoundIntentError::ConsumptionLength(
                encoded.len(),
            ));
        }
        Ok(encoded)
    }

    pub fn consumption_id(
        &self,
        intent: &SignedRecipientRecoveryImportRoundIntent,
    ) -> Result<[u8; 32], RecipientRecoveryImportRoundIntentError> {
        Ok(domain_digest(CONSUMPTION_ID_DOMAIN, &self.encode(intent)?))
    }

    pub fn consumed_at_unix_seconds(&self) -> u64 {
        self.content.consumed_at_unix_seconds
    }

    pub fn final_coordinator_plan_id(&self) -> [u8; 32] {
        self.content.final_coordinator_plan_id
    }

    fn verify(
        &self,
        intent: &SignedRecipientRecoveryImportRoundIntent,
    ) -> Result<(), RecipientRecoveryImportRoundIntentError> {
        validate_consumption_content(&self.content, intent)?;
        self.content
            .owner_device_id
            .verify(&consumption_signing_bytes(&self.content)?, &self.signature)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableRecipientRecoveryImportRoundIntent {
    intent_id: [u8; 32],
    intent: SignedRecipientRecoveryImportRoundIntent,
    consumption: Option<SignedRecipientRecoveryImportRoundConsumption>,
}

impl DurableRecipientRecoveryImportRoundIntent {
    pub fn intent_id(&self) -> [u8; 32] {
        self.intent_id
    }

    pub fn intent(&self) -> &SignedRecipientRecoveryImportRoundIntent {
        &self.intent
    }

    pub fn consumed(&self) -> bool {
        self.consumption.is_some()
    }

    pub fn consumption(&self) -> Option<&SignedRecipientRecoveryImportRoundConsumption> {
        self.consumption.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientRecoveryImportRoundIntentPlan {
    state_directory: PathBuf,
    round_plan: RecipientRecoveryImportRoundPlan,
    candidate: SignedRecipientRecoveryImportRoundIntent,
    existing: Option<DurableRecipientRecoveryImportRoundIntent>,
    pub contains_membership_snapshot: bool,
    pub contains_message_bodies: bool,
    pub contains_plaintext: bool,
    pub grants_import_authority: bool,
    pub grants_network_authority: bool,
    pub grants_background_authority: bool,
}

impl RecipientRecoveryImportRoundIntentPlan {
    pub fn intent_id(&self) -> Result<[u8; 32], RecipientRecoveryImportRoundIntentError> {
        self.existing.as_ref().map_or_else(
            || self.candidate.intent_id(),
            |intent| Ok(intent.intent_id()),
        )
    }

    pub fn round_id(&self) -> [u8; 32] {
        self.candidate.round_id()
    }

    pub fn already_committed(&self) -> bool {
        self.existing
            .as_ref()
            .is_some_and(|intent| !intent.consumed())
    }

    pub fn already_consumed(&self) -> bool {
        self.existing
            .as_ref()
            .is_some_and(DurableRecipientRecoveryImportRoundIntent::consumed)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecipientRecoveryImportRoundIntentProgressState {
    NotStarted,
    Partial,
    Complete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientRecoveryImportRoundIntentProgress {
    pub intent_id: [u8; 32],
    pub round_id: [u8; 32],
    pub state: RecipientRecoveryImportRoundIntentProgressState,
    pub completed_source_count: usize,
    pub total_source_count: usize,
    pub imported_event_count: usize,
    pub coordinator_plan_id: [u8; 32],
    pub coordinator_sequence: u64,
    pub coordinator_refresh_required: bool,
    pub contains_membership_snapshot: bool,
    pub contains_message_bodies: bool,
    pub contains_plaintext: bool,
    pub opened_network: bool,
    pub scheduled_background_work: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipientRecoveryImportRoundIntentCommitOutcome {
    Committed(DurableRecipientRecoveryImportRoundIntent),
    AlreadyCommitted(DurableRecipientRecoveryImportRoundIntent),
    AlreadyConsumed(DurableRecipientRecoveryImportRoundIntent),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipientRecoveryImportRoundIntentConsumeOutcome {
    Consumed(DurableRecipientRecoveryImportRoundIntent),
    AlreadyConsumed(DurableRecipientRecoveryImportRoundIntent),
}

pub struct RecipientRecoveryImportRoundIntentStore {
    state_directory: PathBuf,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

impl RecipientRecoveryImportRoundIntentStore {
    pub fn new(
        state_directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Self {
        Self {
            state_directory: state_directory.as_ref().to_path_buf(),
            owner_account_id,
            owner_device_id,
        }
    }

    pub fn is_initialized(state_directory: impl AsRef<Path>) -> bool {
        database_path(state_directory.as_ref()).is_file()
    }

    pub fn plan(
        &self,
        round_plan: &RecipientRecoveryImportRoundPlan,
        recipient: &DeviceIdentity,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        committed_at_unix_seconds: u64,
    ) -> Result<RecipientRecoveryImportRoundIntentPlan, RecipientRecoveryImportRoundIntentError>
    {
        self.verify_round_owner(round_plan)?;
        round_plan.verify_exact_unstarted(inbox, imports)?;
        let candidate = SignedRecipientRecoveryImportRoundIntent::issue(
            round_plan,
            recipient,
            committed_at_unix_seconds,
        )?;
        let snapshot = load_snapshot(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        )?;
        let existing = reconcile_candidate(&snapshot, &candidate)?;
        Ok(RecipientRecoveryImportRoundIntentPlan {
            state_directory: self.state_directory.clone(),
            round_plan: round_plan.clone(),
            candidate,
            existing,
            contains_membership_snapshot: false,
            contains_message_bodies: false,
            contains_plaintext: false,
            grants_import_authority: false,
            grants_network_authority: false,
            grants_background_authority: false,
        })
    }

    pub fn inspect_active(
        &self,
    ) -> Result<
        Option<DurableRecipientRecoveryImportRoundIntent>,
        RecipientRecoveryImportRoundIntentError,
    > {
        let snapshot = load_snapshot(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        )?;
        snapshot
            .active_round_id
            .map(|round_id| durable_from_snapshot(&snapshot, round_id))
            .transpose()
    }

    pub fn load(
        &self,
        round_id: [u8; 32],
    ) -> Result<
        Option<DurableRecipientRecoveryImportRoundIntent>,
        RecipientRecoveryImportRoundIntentError,
    > {
        let snapshot = load_snapshot(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        )?;
        if snapshot
            .intents
            .iter()
            .any(|intent| intent.round_id() == round_id)
        {
            Ok(Some(durable_from_snapshot(&snapshot, round_id)?))
        } else {
            Ok(None)
        }
    }

    pub fn commit(
        &self,
        plan: &RecipientRecoveryImportRoundIntentPlan,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
    ) -> Result<
        RecipientRecoveryImportRoundIntentCommitOutcome,
        RecipientRecoveryImportRoundIntentError,
    > {
        self.commit_inner(plan, inbox, imports, false)
    }

    fn commit_inner(
        &self,
        plan: &RecipientRecoveryImportRoundIntentPlan,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        fail_before_commit: bool,
    ) -> Result<
        RecipientRecoveryImportRoundIntentCommitOutcome,
        RecipientRecoveryImportRoundIntentError,
    > {
        validate_commit_plan(plan)?;
        if plan.state_directory != self.state_directory
            || plan.candidate.owner_account_id() != self.owner_account_id
            || plan.candidate.owner_device_id() != self.owner_device_id
        {
            return Err(RecipientRecoveryImportRoundIntentError::StoreMismatch);
        }
        let state_lock = StateDirectoryLock::acquire(&self.state_directory)?;
        ensure_same_state_directory(state_lock.root(), inbox.state_directory())?;
        ensure_same_state_directory(state_lock.root(), imports.state_directory())?;
        let existing_snapshot = load_snapshot(
            state_lock.root(),
            self.owner_account_id,
            self.owner_device_id,
        )?;
        if let Some(existing) = reconcile_candidate(&existing_snapshot, &plan.candidate)? {
            return Ok(if existing.consumed() {
                RecipientRecoveryImportRoundIntentCommitOutcome::AlreadyConsumed(existing)
            } else {
                RecipientRecoveryImportRoundIntentCommitOutcome::AlreadyCommitted(existing)
            });
        }
        plan.round_plan.verify_exact_unstarted(inbox, imports)?;

        let path = prepare_database_path(state_lock.root())?;
        let existed = path.is_file();
        let database = Database::create(&path).map_err(database_error)?;
        if existed {
            verify_database(&database, self.owner_account_id, self.owner_device_id)?;
        } else {
            initialize_database(&database, self.owner_account_id, self.owner_device_id)?;
        }
        let snapshot =
            snapshot_from_database(&database, self.owner_account_id, self.owner_device_id)?;
        if let Some(existing) = reconcile_candidate(&snapshot, &plan.candidate)? {
            return Ok(if existing.consumed() {
                RecipientRecoveryImportRoundIntentCommitOutcome::AlreadyConsumed(existing)
            } else {
                RecipientRecoveryImportRoundIntentCommitOutcome::AlreadyCommitted(existing)
            });
        }
        if snapshot.intents.len() as u64 >= MAX_DURABLE_RECIPIENT_RECOVERY_IMPORT_ROUND_INTENTS {
            return Err(RecipientRecoveryImportRoundIntentError::IntentCapacity);
        }
        let encoded = plan.candidate.encode()?;
        let round_id = plan.candidate.round_id();
        let mut write = database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut intents = write.open_table(INTENT_TABLE).map_err(database_error)?;
            let mut active = write.open_table(ACTIVE_TABLE).map_err(database_error)?;
            if intents
                .get(round_id.as_slice())
                .map_err(database_error)?
                .is_some()
                || active.get(ACTIVE_KEY).map_err(database_error)?.is_some()
            {
                return Err(RecipientRecoveryImportRoundIntentError::ActiveIntentConflict);
            }
            intents
                .insert(round_id.as_slice(), encoded.as_slice())
                .map_err(database_error)?;
            active
                .insert(ACTIVE_KEY, round_id.as_slice())
                .map_err(database_error)?;
        }
        if fail_before_commit {
            return Err(RecipientRecoveryImportRoundIntentError::InjectedBeforeCommit);
        }
        write.commit().map_err(database_error)?;
        let snapshot =
            snapshot_from_database(&database, self.owner_account_id, self.owner_device_id)?;
        let durable = durable_from_snapshot(&snapshot, round_id)?;
        Ok(RecipientRecoveryImportRoundIntentCommitOutcome::Committed(
            durable,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn inspect_progress(
        &self,
        durable: &DurableRecipientRecoveryImportRoundIntent,
        current: &RecipientRecoveryCoordinationPlan,
        sources: &[RecipientRecoveryImportRoundSource<'_>],
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
    ) -> Result<RecipientRecoveryImportRoundIntentProgress, RecipientRecoveryImportRoundIntentError>
    {
        self.verify_durable_owner(durable)?;
        inspect_progress(durable, current, sources, inbox, imports)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn consume(
        &self,
        round_id: [u8; 32],
        expected_intent_id: [u8; 32],
        current: &RecipientRecoveryCoordinationPlan,
        sources: &[RecipientRecoveryImportRoundSource<'_>],
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        recipient: &DeviceIdentity,
        consumed_at_unix_seconds: u64,
    ) -> Result<
        RecipientRecoveryImportRoundIntentConsumeOutcome,
        RecipientRecoveryImportRoundIntentError,
    > {
        self.consume_inner(
            round_id,
            expected_intent_id,
            current,
            sources,
            inbox,
            imports,
            recipient,
            consumed_at_unix_seconds,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn consume_inner(
        &self,
        round_id: [u8; 32],
        expected_intent_id: [u8; 32],
        current: &RecipientRecoveryCoordinationPlan,
        sources: &[RecipientRecoveryImportRoundSource<'_>],
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        recipient: &DeviceIdentity,
        consumed_at_unix_seconds: u64,
        fail_before_commit: bool,
    ) -> Result<
        RecipientRecoveryImportRoundIntentConsumeOutcome,
        RecipientRecoveryImportRoundIntentError,
    > {
        if recipient.device_id() != self.owner_device_id {
            return Err(RecipientRecoveryImportRoundIntentError::OwnerDeviceMismatch);
        }
        let snapshot = load_snapshot(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        )?;
        let durable = durable_from_snapshot(&snapshot, round_id)?;
        if durable.intent_id != expected_intent_id {
            return Err(RecipientRecoveryImportRoundIntentError::IntentChanged);
        }
        if durable.consumed() {
            return Ok(RecipientRecoveryImportRoundIntentConsumeOutcome::AlreadyConsumed(durable));
        }
        if snapshot.active_round_id != Some(round_id) {
            return Err(RecipientRecoveryImportRoundIntentError::ActiveIntentChanged);
        }
        let progress = inspect_progress(&durable, current, sources, inbox, imports)?;
        ensure_consumable(&progress)?;
        let consumption = SignedRecipientRecoveryImportRoundConsumption::issue(
            &durable.intent,
            &progress,
            recipient,
            consumed_at_unix_seconds,
        )?;

        let state_lock = StateDirectoryLock::acquire(&self.state_directory)?;
        ensure_same_state_directory(state_lock.root(), inbox.state_directory())?;
        ensure_same_state_directory(state_lock.root(), imports.state_directory())?;
        let snapshot = load_snapshot(
            state_lock.root(),
            self.owner_account_id,
            self.owner_device_id,
        )?;
        let durable = durable_from_snapshot(&snapshot, round_id)?;
        if durable.intent_id != expected_intent_id {
            return Err(RecipientRecoveryImportRoundIntentError::IntentChanged);
        }
        if durable.consumed() {
            return Ok(RecipientRecoveryImportRoundIntentConsumeOutcome::AlreadyConsumed(durable));
        }
        if snapshot.active_round_id != Some(round_id) {
            return Err(RecipientRecoveryImportRoundIntentError::ActiveIntentChanged);
        }
        let progress = inspect_progress(&durable, current, sources, inbox, imports)?;
        ensure_consumable(&progress)?;
        let expected_consumption = SignedRecipientRecoveryImportRoundConsumption::issue(
            &durable.intent,
            &progress,
            recipient,
            consumed_at_unix_seconds,
        )?;
        if consumption != expected_consumption {
            return Err(RecipientRecoveryImportRoundIntentError::ProgressChanged);
        }
        let path = database_path(state_lock.root());
        reject_intent_path_symlinks(state_lock.root())?;
        let database = Database::create(path).map_err(database_error)?;
        verify_database(&database, self.owner_account_id, self.owner_device_id)?;
        let encoded = consumption.encode(&durable.intent)?;
        let mut write = database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut active = write.open_table(ACTIVE_TABLE).map_err(database_error)?;
            let active_round_id = active
                .get(ACTIVE_KEY)
                .map_err(database_error)?
                .map(|value| copy_id(value.value()))
                .transpose()?;
            if active_round_id != Some(round_id) {
                return Err(RecipientRecoveryImportRoundIntentError::ActiveIntentChanged);
            }
            let mut consumed = write.open_table(CONSUMED_TABLE).map_err(database_error)?;
            if consumed
                .get(round_id.as_slice())
                .map_err(database_error)?
                .is_some()
            {
                return Err(RecipientRecoveryImportRoundIntentError::ConsumptionFork);
            }
            consumed
                .insert(round_id.as_slice(), encoded.as_slice())
                .map_err(database_error)?;
            active.remove(ACTIVE_KEY).map_err(database_error)?;
        }
        if fail_before_commit {
            return Err(RecipientRecoveryImportRoundIntentError::InjectedBeforeCommit);
        }
        write.commit().map_err(database_error)?;
        let snapshot =
            snapshot_from_database(&database, self.owner_account_id, self.owner_device_id)?;
        let durable = durable_from_snapshot(&snapshot, round_id)?;
        Ok(RecipientRecoveryImportRoundIntentConsumeOutcome::Consumed(
            durable,
        ))
    }

    fn verify_round_owner(
        &self,
        plan: &RecipientRecoveryImportRoundPlan,
    ) -> Result<(), RecipientRecoveryImportRoundIntentError> {
        let coordination = plan.coordination_plan();
        ensure_same_state_directory(&self.state_directory, coordination.state_directory())?;
        if coordination.owner_account_id() != self.owner_account_id {
            return Err(RecipientRecoveryImportRoundIntentError::OwnerAccountMismatch);
        }
        if coordination.owner_device_id() != self.owner_device_id {
            return Err(RecipientRecoveryImportRoundIntentError::OwnerDeviceMismatch);
        }
        Ok(())
    }

    fn verify_durable_owner(
        &self,
        durable: &DurableRecipientRecoveryImportRoundIntent,
    ) -> Result<(), RecipientRecoveryImportRoundIntentError> {
        if durable.intent.owner_account_id() != self.owner_account_id {
            return Err(RecipientRecoveryImportRoundIntentError::OwnerAccountMismatch);
        }
        if durable.intent.owner_device_id() != self.owner_device_id {
            return Err(RecipientRecoveryImportRoundIntentError::OwnerDeviceMismatch);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn commit_with_fault_before_commit(
        &self,
        plan: &RecipientRecoveryImportRoundIntentPlan,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
    ) -> Result<
        RecipientRecoveryImportRoundIntentCommitOutcome,
        RecipientRecoveryImportRoundIntentError,
    > {
        self.commit_inner(plan, inbox, imports, true)
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn consume_with_fault_before_commit(
        &self,
        round_id: [u8; 32],
        expected_intent_id: [u8; 32],
        current: &RecipientRecoveryCoordinationPlan,
        sources: &[RecipientRecoveryImportRoundSource<'_>],
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        recipient: &DeviceIdentity,
        consumed_at_unix_seconds: u64,
    ) -> Result<
        RecipientRecoveryImportRoundIntentConsumeOutcome,
        RecipientRecoveryImportRoundIntentError,
    > {
        self.consume_inner(
            round_id,
            expected_intent_id,
            current,
            sources,
            inbox,
            imports,
            recipient,
            consumed_at_unix_seconds,
            true,
        )
    }
}

struct IntentStoreSnapshot {
    intents: Vec<SignedRecipientRecoveryImportRoundIntent>,
    active_round_id: Option<[u8; 32]>,
    consumptions: Vec<SignedRecipientRecoveryImportRoundConsumption>,
}

impl IntentStoreSnapshot {
    fn empty() -> Self {
        Self {
            intents: Vec::new(),
            active_round_id: None,
            consumptions: Vec::new(),
        }
    }

    fn intent(&self, round_id: [u8; 32]) -> Option<&SignedRecipientRecoveryImportRoundIntent> {
        self.intents
            .iter()
            .find(|intent| intent.round_id() == round_id)
    }

    fn consumption(
        &self,
        round_id: [u8; 32],
    ) -> Option<&SignedRecipientRecoveryImportRoundConsumption> {
        self.consumptions
            .iter()
            .find(|consumption| consumption.content.round_id == round_id)
    }
}

fn reconcile_candidate(
    snapshot: &IntentStoreSnapshot,
    candidate: &SignedRecipientRecoveryImportRoundIntent,
) -> Result<
    Option<DurableRecipientRecoveryImportRoundIntent>,
    RecipientRecoveryImportRoundIntentError,
> {
    if let Some(active_round_id) = snapshot.active_round_id
        && active_round_id != candidate.round_id()
    {
        return Err(RecipientRecoveryImportRoundIntentError::ActiveIntentConflict);
    }
    let Some(existing) = snapshot.intent(candidate.round_id()) else {
        return Ok(None);
    };
    if !existing.same_intent_as(candidate) {
        return Err(RecipientRecoveryImportRoundIntentError::IntentFork);
    }
    Ok(Some(durable_from_snapshot(snapshot, candidate.round_id())?))
}

fn durable_from_snapshot(
    snapshot: &IntentStoreSnapshot,
    round_id: [u8; 32],
) -> Result<DurableRecipientRecoveryImportRoundIntent, RecipientRecoveryImportRoundIntentError> {
    let intent = snapshot
        .intent(round_id)
        .ok_or(RecipientRecoveryImportRoundIntentError::IntentMissing)?
        .clone();
    let intent_id = intent.intent_id()?;
    let consumption = snapshot.consumption(round_id).cloned();
    Ok(DurableRecipientRecoveryImportRoundIntent {
        intent_id,
        intent,
        consumption,
    })
}

fn inspect_progress(
    durable: &DurableRecipientRecoveryImportRoundIntent,
    current: &RecipientRecoveryCoordinationPlan,
    sources: &[RecipientRecoveryImportRoundSource<'_>],
    inbox: &RecipientHistoryTransferInbox,
    imports: &RecipientHistoryImportStore,
) -> Result<RecipientRecoveryImportRoundIntentProgress, RecipientRecoveryImportRoundIntentError> {
    durable.intent.verify()?;
    if current.owner_account_id() != durable.intent.owner_account_id() {
        return Err(RecipientRecoveryImportRoundIntentError::OwnerAccountMismatch);
    }
    if current.owner_device_id() != durable.intent.owner_device_id() {
        return Err(RecipientRecoveryImportRoundIntentError::OwnerDeviceMismatch);
    }
    RecipientHistoryRecoveryCoordinator::verify_exact_observation(current, inbox, imports)?;
    RecipientHistoryRecoveryCoordinator::verify_chain_member(
        current,
        durable.intent.coordinator_sequence(),
        durable.intent.coordinator_plan_id(),
    )?;
    if current.sources().len() != durable.intent.content.coordinator_sources.len()
        || sources.len() != durable.intent.content.entries.len()
    {
        return Err(RecipientRecoveryImportRoundIntentError::ProgressChanged);
    }
    validate_progress_sources(&durable.intent, sources)?;

    let planned_ordinals = durable
        .intent
        .content
        .entries
        .iter()
        .map(|entry| usize::from(entry.source_ordinal))
        .collect::<HashSet<_>>();
    for (ordinal, baseline) in durable
        .intent
        .content
        .coordinator_sources
        .iter()
        .enumerate()
    {
        let observed = current.import_observation(ordinal)?;
        if !planned_ordinals.contains(&ordinal) && baseline_from_observation(observed) != *baseline
        {
            return Err(RecipientRecoveryImportRoundIntentError::UnplannedSourceChanged(ordinal));
        }
    }

    let mut completed_prefix = 0_usize;
    let mut imported_event_count = 0_usize;
    let mut saw_pending = false;
    for (index, entry) in durable.intent.content.entries.iter().enumerate() {
        let ordinal = usize::from(entry.source_ordinal);
        let baseline = durable
            .intent
            .content
            .coordinator_sources
            .get(ordinal)
            .ok_or(RecipientRecoveryImportRoundIntentError::InvalidIntent)?;
        let observed = current.import_observation(ordinal)?;
        if !same_static_observation(*baseline, observed) {
            return Err(RecipientRecoveryImportRoundIntentError::ProgressChanged);
        }
        if observed.imported_chunk_count == baseline.imported_chunk_count {
            if baseline_from_observation(observed) != *baseline {
                return Err(RecipientRecoveryImportRoundIntentError::ProgressChanged);
            }
            saw_pending = true;
            let candidate = RecipientHistoryRecoveryImporter::preflight_candidate_binding(
                current,
                ordinal,
                inbox,
                imports,
                sources[index].membership(),
            )?;
            if candidate != entry.candidate_binding() {
                return Err(RecipientRecoveryImportRoundIntentError::ProgressChanged);
            }
        } else if observed.imported_chunk_count
            == baseline
                .imported_chunk_count
                .checked_add(1)
                .ok_or(RecipientRecoveryImportRoundIntentError::ProgressChanged)?
        {
            if saw_pending || index != completed_prefix {
                return Err(RecipientRecoveryImportRoundIntentError::NonPrefixProgress);
            }
            let receipt = imports.verify_staged_transfer_imported(
                inbox,
                entry.transfer_id,
                sources[index].membership(),
            )?;
            verify_completed_entry(*baseline, *entry, observed, &receipt)?;
            imported_event_count = imported_event_count
                .checked_add(receipt.imported_event_count())
                .ok_or(RecipientRecoveryImportRoundIntentError::ProgressChanged)?;
            completed_prefix += 1;
        } else {
            return Err(RecipientRecoveryImportRoundIntentError::ProgressBeyondRound(ordinal));
        }
    }

    let expected_sequence = durable
        .intent
        .coordinator_sequence()
        .checked_add(
            u64::try_from(completed_prefix)
                .map_err(|_| RecipientRecoveryImportRoundIntentError::ProgressChanged)?,
        )
        .ok_or(RecipientRecoveryImportRoundIntentError::ProgressChanged)?;
    if current.sequence() != expected_sequence {
        return Err(RecipientRecoveryImportRoundIntentError::ProgressChanged);
    }
    if current.requires_apply()
        && (completed_prefix == 0
            || current.previous_plan_id().is_none()
            || current.previous_plan_id() == Some(durable.intent.coordinator_plan_id())
                && completed_prefix != 1)
    {
        return Err(RecipientRecoveryImportRoundIntentError::ProgressChanged);
    }
    let state = if completed_prefix == 0 {
        RecipientRecoveryImportRoundIntentProgressState::NotStarted
    } else if completed_prefix == durable.intent.source_count() {
        RecipientRecoveryImportRoundIntentProgressState::Complete
    } else {
        RecipientRecoveryImportRoundIntentProgressState::Partial
    };
    Ok(RecipientRecoveryImportRoundIntentProgress {
        intent_id: durable.intent_id,
        round_id: durable.intent.round_id(),
        state,
        completed_source_count: completed_prefix,
        total_source_count: durable.intent.source_count(),
        imported_event_count,
        coordinator_plan_id: current.plan_id(),
        coordinator_sequence: current.sequence(),
        coordinator_refresh_required: current.requires_apply(),
        contains_membership_snapshot: false,
        contains_message_bodies: false,
        contains_plaintext: false,
        opened_network: false,
        scheduled_background_work: false,
    })
}

fn validate_progress_sources(
    intent: &SignedRecipientRecoveryImportRoundIntent,
    sources: &[RecipientRecoveryImportRoundSource<'_>],
) -> Result<(), RecipientRecoveryImportRoundIntentError> {
    for (source, entry) in sources.iter().zip(&intent.content.entries) {
        let digest = crate::recipient_import::membership_digest(source.membership())?;
        if source.source_ordinal() != usize::from(entry.source_ordinal)
            || digest != entry.membership_digest
            || source.membership().revision() != entry.membership_revision
        {
            return Err(RecipientRecoveryImportRoundIntentError::SourceMismatch);
        }
    }
    Ok(())
}

fn verify_completed_entry(
    baseline: IntentCoordinatorSourceBaseline,
    entry: IntentImportEntry,
    observed: RecipientRecoveryCoordinatorImportObservation,
    receipt: &DurableRecipientHistoryImportReceipt,
) -> Result<(), RecipientRecoveryImportRoundIntentError> {
    let expected_events = baseline
        .imported_event_count
        .checked_add(
            u64::try_from(receipt.imported_event_count())
                .map_err(|_| RecipientRecoveryImportRoundIntentError::ProgressChanged)?,
        )
        .ok_or(RecipientRecoveryImportRoundIntentError::ProgressChanged)?;
    if receipt.execution_record_id() != entry.execution_record_id
        || receipt.imported_event_count() != usize::from(entry.event_count)
        || observed.import_head_id != Some(receipt.receipt_id())
        || observed.imported_event_count != expected_events
    {
        return Err(RecipientRecoveryImportRoundIntentError::ProgressChanged);
    }
    Ok(())
}

fn ensure_consumable(
    progress: &RecipientRecoveryImportRoundIntentProgress,
) -> Result<(), RecipientRecoveryImportRoundIntentError> {
    if progress.state != RecipientRecoveryImportRoundIntentProgressState::Complete {
        return Err(RecipientRecoveryImportRoundIntentError::RoundIncomplete);
    }
    if progress.coordinator_refresh_required {
        return Err(RecipientRecoveryImportRoundIntentError::CoordinatorRefreshRequired);
    }
    Ok(())
}

fn validate_commit_plan(
    plan: &RecipientRecoveryImportRoundIntentPlan,
) -> Result<(), RecipientRecoveryImportRoundIntentError> {
    plan.candidate.verify()?;
    if plan.candidate.round_id() != plan.round_plan.round_id()
        || plan.candidate.coordinator_plan_id() != plan.round_plan.coordination_plan().plan_id()
        || plan.candidate.coordinator_sequence() != plan.round_plan.coordination_plan().sequence()
        || plan.contains_membership_snapshot
        || plan.contains_message_bodies
        || plan.contains_plaintext
        || plan.grants_import_authority
        || plan.grants_network_authority
        || plan.grants_background_authority
    {
        return Err(RecipientRecoveryImportRoundIntentError::InvalidPlan);
    }
    if let Some(existing) = &plan.existing {
        existing.intent.verify()?;
        if !existing.intent.same_intent_as(&plan.candidate) {
            return Err(RecipientRecoveryImportRoundIntentError::IntentFork);
        }
    }
    Ok(())
}

fn baseline_from_observation(
    observation: RecipientRecoveryCoordinatorImportObservation,
) -> IntentCoordinatorSourceBaseline {
    IntentCoordinatorSourceBaseline {
        source_key: observation.source_key,
        inbox_head_id: observation.inbox_head_id,
        import_head_id: observation.import_head_id,
        staged_chunk_count: observation.staged_chunk_count,
        imported_chunk_count: observation.imported_chunk_count,
        imported_event_count: observation.imported_event_count,
        approved_range_start: observation.approved_range_start,
        approved_range_end: observation.approved_range_end,
    }
}

fn same_static_observation(
    baseline: IntentCoordinatorSourceBaseline,
    observed: RecipientRecoveryCoordinatorImportObservation,
) -> bool {
    baseline.source_key == observed.source_key
        && baseline.inbox_head_id == observed.inbox_head_id
        && baseline.staged_chunk_count == observed.staged_chunk_count
        && baseline.approved_range_start == observed.approved_range_start
        && baseline.approved_range_end == observed.approved_range_end
}

fn validate_intent_content(
    content: &RecipientRecoveryImportRoundIntentContent,
) -> Result<(), RecipientRecoveryImportRoundIntentError> {
    if content.version != INTENT_VERSION
        || content.round_id == [0_u8; 32]
        || content.coordinator_plan_id == [0_u8; 32]
        || content.max_sources == 0
        || usize::from(content.max_sources) > MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_SOURCES
        || content.max_events == 0
        || usize::from(content.max_events) > MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_EVENTS
        || content.planned_event_count == 0
        || content.planned_event_count > content.max_events
        || content.committed_at_unix_seconds == 0
        || content.coordinator_sources.is_empty()
        || content.coordinator_sources.len() > MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_SOURCES
        || content.entries.is_empty()
        || content.entries.len() > usize::from(content.max_sources)
    {
        return Err(RecipientRecoveryImportRoundIntentError::InvalidIntent);
    }
    let mut source_keys = HashSet::with_capacity(content.coordinator_sources.len());
    for source in &content.coordinator_sources {
        if source.source_key == [0_u8; 32]
            || source.approved_range_start >= source.approved_range_end
            || source.imported_chunk_count > source.staged_chunk_count
            || (source.inbox_head_id.is_none() != (source.staged_chunk_count == 0))
            || (source.import_head_id.is_none() != (source.imported_chunk_count == 0))
            || !source_keys.insert(source.source_key)
        {
            return Err(RecipientRecoveryImportRoundIntentError::InvalidIntent);
        }
    }
    let mut ordinals = HashSet::with_capacity(content.entries.len());
    let mut event_count = 0_usize;
    for entry in &content.entries {
        let ordinal = usize::from(entry.source_ordinal);
        if ordinal >= content.coordinator_sources.len()
            || !ordinals.insert(ordinal)
            || entry.import_plan_id == [0_u8; 32]
            || entry.source_key != content.coordinator_sources[ordinal].source_key
            || entry.transfer_id == [0_u8; 32]
            || entry.execution_record_id == [0_u8; 32]
            || entry.expected_inbox_head_id == [0_u8; 32]
            || entry.staged_chunk_count <= entry.imported_chunk_count
            || entry.range_start >= entry.range_end
            || entry.range_end > entry.approved_range_end
            || entry.event_count == 0
            || entry.membership_digest == [0_u8; 32]
            || entry.membership_revision == 0
        {
            return Err(RecipientRecoveryImportRoundIntentError::InvalidIntent);
        }
        event_count = event_count
            .checked_add(usize::from(entry.event_count))
            .ok_or(RecipientRecoveryImportRoundIntentError::InvalidIntent)?;
    }
    if event_count != usize::from(content.planned_event_count) {
        return Err(RecipientRecoveryImportRoundIntentError::InvalidIntent);
    }
    Ok(())
}

fn validate_consumption_content(
    content: &RecipientRecoveryImportRoundConsumptionContent,
    intent: &SignedRecipientRecoveryImportRoundIntent,
) -> Result<(), RecipientRecoveryImportRoundIntentError> {
    if content.version != CONSUMPTION_VERSION
        || content.owner_account_id != intent.owner_account_id()
        || content.owner_device_id != intent.owner_device_id()
        || content.round_id != intent.round_id()
        || content.intent_id != intent.intent_id()?
        || content.final_coordinator_plan_id == [0_u8; 32]
        || usize::from(content.completed_source_count) != intent.source_count()
        || content.imported_event_count
            != u64::try_from(intent.planned_event_count())
                .map_err(|_| RecipientRecoveryImportRoundIntentError::InvalidConsumption)?
        || content.consumed_at_unix_seconds < intent.committed_at_unix_seconds()
    {
        return Err(RecipientRecoveryImportRoundIntentError::InvalidConsumption);
    }
    Ok(())
}

fn intent_signing_bytes(
    content: &RecipientRecoveryImportRoundIntentContent,
) -> Result<Vec<u8>, RecipientRecoveryImportRoundIntentError> {
    validate_intent_content(content)?;
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(INTENT_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(INTENT_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn consumption_signing_bytes(
    content: &RecipientRecoveryImportRoundConsumptionContent,
) -> Result<Vec<u8>, RecipientRecoveryImportRoundIntentError> {
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(CONSUMPTION_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(CONSUMPTION_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

fn initialize_database(
    database: &Database,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), RecipientRecoveryImportRoundIntentError> {
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
        let _ = write.open_table(INTENT_TABLE).map_err(database_error)?;
        let _ = write.open_table(ACTIVE_TABLE).map_err(database_error)?;
        let _ = write.open_table(CONSUMED_TABLE).map_err(database_error)?;
    }
    write.commit().map_err(database_error)
}

fn load_snapshot(
    state_directory: &Path,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<IntentStoreSnapshot, RecipientRecoveryImportRoundIntentError> {
    let path = database_path(state_directory);
    if !path.is_file() {
        reject_intent_path_symlinks(state_directory)?;
        return Ok(IntentStoreSnapshot::empty());
    }
    reject_intent_path_symlinks(state_directory)?;
    let database = ReadOnlyDatabase::open(path).map_err(database_error)?;
    snapshot_from_database(&database, owner_account_id, owner_device_id)
}

fn snapshot_from_database<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<IntentStoreSnapshot, RecipientRecoveryImportRoundIntentError> {
    verify_database(database, owner_account_id, owner_device_id)?;
    let read = database.begin_read().map_err(database_error)?;
    let intents = read.open_table(INTENT_TABLE).map_err(database_error)?;
    let active = read.open_table(ACTIVE_TABLE).map_err(database_error)?;
    let consumed = read.open_table(CONSUMED_TABLE).map_err(database_error)?;
    let mut loaded_intents = Vec::new();
    for row in intents.iter().map_err(database_error)? {
        let (_, value) = row.map_err(database_error)?;
        loaded_intents.push(SignedRecipientRecoveryImportRoundIntent::decode_and_verify(
            value.value(),
            owner_account_id,
            owner_device_id,
        )?);
    }
    let active_round_id = active
        .get(ACTIVE_KEY)
        .map_err(database_error)?
        .map(|value| copy_id(value.value()))
        .transpose()?;
    let mut loaded_consumptions = Vec::new();
    for row in consumed.iter().map_err(database_error)? {
        let (key, value) = row.map_err(database_error)?;
        let round_id = copy_id(key.value())?;
        let intent = loaded_intents
            .iter()
            .find(|intent| intent.round_id() == round_id)
            .ok_or(RecipientRecoveryImportRoundIntentError::InvalidDatabase)?;
        loaded_consumptions.push(
            SignedRecipientRecoveryImportRoundConsumption::decode_and_verify(
                value.value(),
                intent,
            )?,
        );
    }
    Ok(IntentStoreSnapshot {
        intents: loaded_intents,
        active_round_id,
        consumptions: loaded_consumptions,
    })
}

fn verify_database<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), RecipientRecoveryImportRoundIntentError> {
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
        return Err(RecipientRecoveryImportRoundIntentError::MetadataMismatch);
    }
    let intents = read.open_table(INTENT_TABLE).map_err(database_error)?;
    let active = read.open_table(ACTIVE_TABLE).map_err(database_error)?;
    let consumed = read.open_table(CONSUMED_TABLE).map_err(database_error)?;
    if intents.len().map_err(database_error)? > MAX_DURABLE_RECIPIENT_RECOVERY_IMPORT_ROUND_INTENTS
        || consumed.len().map_err(database_error)?
            > MAX_DURABLE_RECIPIENT_RECOVERY_IMPORT_ROUND_INTENTS
        || active.len().map_err(database_error)? > 1
    {
        return Err(RecipientRecoveryImportRoundIntentError::InvalidDatabase);
    }
    let mut round_ids = HashSet::new();
    let mut unconsumed = HashSet::new();
    for row in intents.iter().map_err(database_error)? {
        let (key, value) = row.map_err(database_error)?;
        let round_id = copy_id(key.value())?;
        let intent = SignedRecipientRecoveryImportRoundIntent::decode_and_verify(
            value.value(),
            owner_account_id,
            owner_device_id,
        )?;
        if intent.round_id() != round_id || !round_ids.insert(round_id) {
            return Err(RecipientRecoveryImportRoundIntentError::InvalidDatabase);
        }
        unconsumed.insert(round_id);
    }
    for row in consumed.iter().map_err(database_error)? {
        let (key, value) = row.map_err(database_error)?;
        let round_id = copy_id(key.value())?;
        let intent_bytes = intents
            .get(round_id.as_slice())
            .map_err(database_error)?
            .ok_or(RecipientRecoveryImportRoundIntentError::InvalidDatabase)?;
        let intent = SignedRecipientRecoveryImportRoundIntent::decode_and_verify(
            intent_bytes.value(),
            owner_account_id,
            owner_device_id,
        )?;
        let consumption = SignedRecipientRecoveryImportRoundConsumption::decode_and_verify(
            value.value(),
            &intent,
        )?;
        if consumption.content.round_id != round_id || !unconsumed.remove(&round_id) {
            return Err(RecipientRecoveryImportRoundIntentError::InvalidDatabase);
        }
    }
    let active_round_id = active
        .get(ACTIVE_KEY)
        .map_err(database_error)?
        .map(|value| copy_id(value.value()))
        .transpose()?;
    match active_round_id {
        Some(round_id) if unconsumed == HashSet::from([round_id]) => {}
        None if unconsumed.is_empty() => {}
        _ => return Err(RecipientRecoveryImportRoundIntentError::InvalidDatabase),
    }
    Ok(())
}

fn prepare_database_path(
    state_directory: &Path,
) -> Result<PathBuf, RecipientRecoveryImportRoundIntentError> {
    let directory = intent_directory(state_directory);
    reject_symlink_if_exists(&directory)?;
    fs::create_dir_all(&directory).map_err(|source| io_error(directory.clone(), source))?;
    let path = database_path(state_directory);
    reject_symlink_if_exists(&path)?;
    Ok(path)
}

fn intent_directory(state_directory: &Path) -> PathBuf {
    state_directory.join(INTENT_DIRECTORY)
}

fn database_path(state_directory: &Path) -> PathBuf {
    intent_directory(state_directory).join(DATABASE_FILE)
}

fn reject_intent_path_symlinks(
    state_directory: &Path,
) -> Result<(), RecipientRecoveryImportRoundIntentError> {
    reject_symlink_if_exists(&intent_directory(state_directory))?;
    reject_symlink_if_exists(&database_path(state_directory))
}

fn reject_symlink_if_exists(path: &Path) -> Result<(), RecipientRecoveryImportRoundIntentError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(
            RecipientRecoveryImportRoundIntentError::Symlink(path.to_path_buf()),
        ),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(io_error(path.to_path_buf(), source)),
    }
}

fn ensure_same_state_directory(
    left: &Path,
    right: &Path,
) -> Result<(), RecipientRecoveryImportRoundIntentError> {
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
    Err(RecipientRecoveryImportRoundIntentError::StoreMismatch)
}

fn copy_id(bytes: &[u8]) -> Result<[u8; 32], RecipientRecoveryImportRoundIntentError> {
    bytes
        .try_into()
        .map_err(|_| RecipientRecoveryImportRoundIntentError::InvalidDatabase)
}

fn database_error(error: impl std::fmt::Display) -> RecipientRecoveryImportRoundIntentError {
    RecipientRecoveryImportRoundIntentError::Database(error.to_string())
}

fn io_error(path: PathBuf, source: io::Error) -> RecipientRecoveryImportRoundIntentError {
    RecipientRecoveryImportRoundIntentError::Io { path, source }
}

#[derive(Debug, Error)]
pub enum RecipientRecoveryImportRoundIntentError {
    #[error("recipient recovery import-round intent belongs to another Account")]
    OwnerAccountMismatch,
    #[error("recipient recovery import-round intent belongs to another Device")]
    OwnerDeviceMismatch,
    #[error("recipient recovery import-round intent store does not match the round state")]
    StoreMismatch,
    #[error("recipient recovery import-round intent is invalid")]
    InvalidIntent,
    #[error("recipient recovery import-round intent plan is invalid")]
    InvalidPlan,
    #[error("recipient recovery import-round consumption is invalid")]
    InvalidConsumption,
    #[error("recipient recovery import-round intent encoding has invalid length {0}")]
    IntentLength(usize),
    #[error("recipient recovery import-round consumption encoding has invalid length {0}")]
    ConsumptionLength(usize),
    #[error("recipient recovery import-round intent is not canonical")]
    NonCanonicalIntent,
    #[error("recipient recovery import-round consumption is not canonical")]
    NonCanonicalConsumption,
    #[error("another recipient recovery import-round intent is active")]
    ActiveIntentConflict,
    #[error("active recipient recovery import-round intent changed")]
    ActiveIntentChanged,
    #[error("recipient recovery import-round intent changed")]
    IntentChanged,
    #[error("recipient recovery import-round intent is missing")]
    IntentMissing,
    #[error("recipient recovery import-round intent fork was detected")]
    IntentFork,
    #[error("recipient recovery import-round consumption fork was detected")]
    ConsumptionFork,
    #[error("recipient recovery import-round progress changed")]
    ProgressChanged,
    #[error("recipient recovery import-round progress is not an exact ordered prefix")]
    NonPrefixProgress,
    #[error("recipient recovery import-round source {0} progressed more than once")]
    ProgressBeyondRound(usize),
    #[error("recipient recovery import-round observed a change to unplanned source {0}")]
    UnplannedSourceChanged(usize),
    #[error("recipient recovery import-round source membership does not match the intent")]
    SourceMismatch,
    #[error("recipient recovery import-round is incomplete")]
    RoundIncomplete,
    #[error("recipient recovery import-round coordinator refresh is still required")]
    CoordinatorRefreshRequired,
    #[error("recipient recovery import-round intent capacity is exhausted")]
    IntentCapacity,
    #[error("recipient recovery import-round intent database metadata does not match its owner")]
    MetadataMismatch,
    #[error("recipient recovery import-round intent database is inconsistent")]
    InvalidDatabase,
    #[error("recipient recovery import-round intent path is a symlink: {0}")]
    Symlink(PathBuf),
    #[error("recipient recovery import-round intent database error: {0}")]
    Database(String),
    #[error("recipient recovery import-round intent I/O failed at {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("recipient recovery import-round intent injected failure before commit")]
    InjectedBeforeCommit,
    #[error(transparent)]
    Coordinator(#[from] RecipientRecoveryCoordinatorError),
    #[error(transparent)]
    Round(Box<RecipientRecoveryImportRoundError>),
    #[error(transparent)]
    ImportPlan(Box<RecipientRecoveryImportPlanError>),
    #[error(transparent)]
    Import(#[from] RecipientHistoryImportError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    State(#[from] StateError),
    #[error(transparent)]
    Postcard(#[from] postcard::Error),
}

impl From<RecipientRecoveryImportRoundError> for RecipientRecoveryImportRoundIntentError {
    fn from(error: RecipientRecoveryImportRoundError) -> Self {
        Self::Round(Box::new(error))
    }
}

impl From<RecipientRecoveryImportPlanError> for RecipientRecoveryImportRoundIntentError {
    fn from(error: RecipientRecoveryImportPlanError) -> Self {
        Self::ImportPlan(Box::new(error))
    }
}
