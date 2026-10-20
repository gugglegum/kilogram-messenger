#![forbid(unsafe_code)]

//! Bounded, local/offline execution of one explicitly consented old-history
//! recovery source.
//!
//! The execution boundary deliberately remains smaller than a production
//! transfer service. It creates existing authenticated history-rewrap bundles,
//! lets the recipient validate/decrypt them into encrypted local projections,
//! and atomically journals the exact bundle/checkpoint transition. It does not
//! open a network endpoint, schedule multiple sources, mutate the ordinary
//! event store, expose plaintext or grant IPC/desktop authority.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use kilogram_identity::{
    AccountDeviceListSnapshot, AccountId, ConversationMembershipSnapshot, DeviceEncryptionIdentity,
    DeviceId, DeviceIdentity,
};
use kilogram_peer_history_recovery::{
    BoundHistoryRecoverySelection, HistoryRecoveryRangeSelection, OldHistoryRecoveryInventory,
    PeerHistoryRecoveryError, PeerReenrollmentHistoryBinding, SignedOldHistoryRecoveryPlan,
};
use kilogram_peer_history_recovery_consent::{
    OldHistoryRecoveryConsentStore, OldHistoryRecoveryConsentStoreError,
};
use kilogram_protocol::{
    AuthorizedEvent, HistoryRewrapBundle, HistoryRewrapSas, LocalTextProjection, ProtocolError,
    SignedHistoryRecoveryCheckpoint,
};
use redb::{
    Database, Durability, ReadOnlyDatabase, ReadableDatabase, ReadableTable, ReadableTableMetadata,
    TableDefinition,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod import;

pub use import::{
    DurableRecipientHistoryImportReceipt, RecipientHistoryImportError,
    RecipientHistoryImportOutcome, RecipientHistoryImportStatus, RecipientHistoryImportStatusView,
    RecipientHistoryImportStore,
};

const DATABASE_FILE: &str = "old-history-recovery-execution-v1.redb";
const META_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("old-history-recovery-execution-meta-v1");
const RECORD_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("old-history-recovery-execution-record-v1");
const HEAD_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("old-history-recovery-execution-head-v1");
const BUNDLE_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("old-history-recovery-execution-bundle-v1");
const CHECKPOINT_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("old-history-recovery-execution-checkpoint-v1");
const PROJECTION_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("old-history-recovery-execution-projections-v1");

const SCHEMA_KEY: &str = "schema-version";
const OWNER_ACCOUNT_KEY: &str = "owner-account-id";
const OWNER_DEVICE_KEY: &str = "owner-device-id";
const SCHEMA_VERSION: u8 = 1;
const RECORD_VERSION: u8 = 1;
const HEAD_VERSION: u8 = 1;
const RECORD_SIGNATURE_DOMAIN: &[u8] = b"kilogram:old-history-recovery-execution-record:v1\0";
const RECORD_ID_DOMAIN: &[u8] = b"kilogram:old-history-recovery-execution-record-id:v1\0";
const SELECTION_KEY_DOMAIN: &[u8] = b"kilogram:old-history-recovery-execution-selection:v1\0";
const SELECTION_TOKEN_DOMAIN: &[u8] =
    b"kilogram:old-history-recovery-execution-selection-token:v1\0";
const BUNDLE_DIGEST_DOMAIN: &[u8] = b"kilogram:old-history-recovery-execution-bundle:v1\0";
const PROJECTION_DIGEST_DOMAIN: &[u8] = b"kilogram:old-history-recovery-execution-projections:v1\0";

pub const MAX_HISTORY_RECOVERY_EXECUTION_CHUNK_EVENTS: usize = 32;
pub const MAX_HISTORY_RECOVERY_EXECUTION_RECORD_BYTES: usize = 16 * 1024;
pub const MAX_HISTORY_RECOVERY_EXECUTION_HEAD_BYTES: usize = 2 * 1024;
pub const MAX_HISTORY_RECOVERY_EXECUTION_BUNDLE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_HISTORY_RECOVERY_EXECUTION_CHECKPOINT_BYTES: usize = 64 * 1024;
pub const MAX_HISTORY_RECOVERY_EXECUTION_PROJECTION_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_DURABLE_HISTORY_RECOVERY_EXECUTION_CHUNKS: u64 = 16_384;

pub struct OldHistoryRecoveryLiveContext<'a> {
    pub binding: &'a PeerReenrollmentHistoryBinding,
    pub authority: &'a AccountDeviceListSnapshot,
    pub memberships: &'a [ConversationMembershipSnapshot],
    pub inventory: &'a OldHistoryRecoveryInventory,
    pub selections: &'a [HistoryRecoveryRangeSelection],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedHistoryRecoveryChunk {
    consent_record_id: [u8; 32],
    plan_id: [u8; 32],
    transition_id: [u8; 32],
    selection_index: u16,
    selection: BoundHistoryRecoverySelection,
    checkpoint_before: Option<SignedHistoryRecoveryCheckpoint>,
    checkpoint_started_here: bool,
    page_size: usize,
    bundle: HistoryRewrapBundle,
}

impl PreparedHistoryRecoveryChunk {
    pub fn bundle(&self) -> &HistoryRewrapBundle {
        &self.bundle
    }

    pub fn selection_index(&self) -> usize {
        usize::from(self.selection_index)
    }

    pub fn range_start(&self) -> u64 {
        self.bundle.manifest().range_start()
    }

    pub fn range_end(&self) -> u64 {
        self.bundle.manifest().range_end()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppliedHistoryRecoveryEntry {
    event: AuthorizedEvent,
    projection: LocalTextProjection,
}

impl AppliedHistoryRecoveryEntry {
    pub fn event(&self) -> &AuthorizedEvent {
        &self.event
    }

    pub fn projection(&self) -> &LocalTextProjection {
        &self.projection
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppliedHistoryRecoveryChunk {
    prepared: PreparedHistoryRecoveryChunk,
    checkpoint_before: SignedHistoryRecoveryCheckpoint,
    checkpoint_after: SignedHistoryRecoveryCheckpoint,
    entries: Vec<AppliedHistoryRecoveryEntry>,
}

impl AppliedHistoryRecoveryChunk {
    pub fn bundle(&self) -> &HistoryRewrapBundle {
        &self.prepared.bundle
    }

    pub fn checkpoint_before(&self) -> &SignedHistoryRecoveryCheckpoint {
        &self.checkpoint_before
    }

    pub fn checkpoint_after(&self) -> &SignedHistoryRecoveryCheckpoint {
        &self.checkpoint_after
    }

    pub fn entries(&self) -> &[AppliedHistoryRecoveryEntry] {
        &self.entries
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrepareHistoryRecoveryChunkOutcome {
    Prepared(Box<PreparedHistoryRecoveryChunk>),
    AlreadyCommitted(Box<DurableHistoryRecoveryChunk>),
    Complete(HistoryRecoveryExecutionStatusView),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommitHistoryRecoveryChunkOutcome {
    Committed(DurableHistoryRecoveryChunk),
    AlreadyCommitted(DurableHistoryRecoveryChunk),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistoryRecoveryExecutionStatus {
    Absent,
    InProgress(HistoryRecoveryExecutionStatusView),
    Complete(HistoryRecoveryExecutionStatusView),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryRecoveryExecutionStatusView {
    pub selection_token: [u8; 16],
    pub committed_chunk_count: u64,
    pub next_range_start: u64,
    pub approved_range_end: u64,
    pub contains_message_bodies: bool,
    pub contains_plaintext: bool,
    pub grants_network_authority: bool,
    pub grants_store_import_authority: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ExecutionRecordContent {
    version: u8,
    owner_account_id: AccountId,
    source_device_id: DeviceId,
    recipient_device_id: DeviceId,
    consent_record_id: [u8; 32],
    plan_id: [u8; 32],
    transition_id: [u8; 32],
    selection_key: [u8; 32],
    selection_index: u16,
    conversation_id: kilogram_protocol::ConversationId,
    inventory_event_count: u64,
    inventory_digest: [u8; 32],
    approved_range_start: u64,
    approved_range_end: u64,
    generation: u64,
    previous_record_id: Option<[u8; 32]>,
    checkpoint_started_here: bool,
    checkpoint_before_id: [u8; 32],
    checkpoint_after_id: [u8; 32],
    chunk_range_start: u64,
    chunk_range_end: u64,
    bundle_id: [u8; 32],
    bundle_digest: [u8; 32],
    projection_count: u16,
    projection_digest: [u8; 32],
    committed_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SignedExecutionRecordWire {
    content: ExecutionRecordContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SignedExecutionRecord {
    content: ExecutionRecordContent,
    signature: Vec<u8>,
}

impl SignedExecutionRecord {
    fn issue(
        source: &DeviceIdentity,
        content: ExecutionRecordContent,
    ) -> Result<Self, HistoryRecoveryExecutionError> {
        validate_record_content(&content)?;
        if source.device_id() != content.source_device_id {
            return Err(HistoryRecoveryExecutionError::OwnerDeviceMismatch);
        }
        let signature = source.sign(&record_signing_bytes(&content)?).to_vec();
        let record = Self { content, signature };
        record.verify()?;
        Ok(record)
    }

    fn encode(&self) -> Result<Vec<u8>, HistoryRecoveryExecutionError> {
        self.verify()?;
        let encoded = postcard::to_allocvec(&SignedExecutionRecordWire {
            content: self.content.clone(),
            signature: self.signature.clone(),
        })?;
        validate_record_length(&encoded)?;
        Ok(encoded)
    }

    fn decode_and_verify(
        encoded: &[u8],
        expected_account_id: AccountId,
        expected_source_device_id: DeviceId,
    ) -> Result<Self, HistoryRecoveryExecutionError> {
        validate_record_length(encoded)?;
        let wire: SignedExecutionRecordWire = postcard::from_bytes(encoded)?;
        let record = Self {
            content: wire.content,
            signature: wire.signature,
        };
        record.verify()?;
        if record.content.owner_account_id != expected_account_id {
            return Err(HistoryRecoveryExecutionError::OwnerAccountMismatch);
        }
        if record.content.source_device_id != expected_source_device_id {
            return Err(HistoryRecoveryExecutionError::OwnerDeviceMismatch);
        }
        if record.encode()? != encoded {
            return Err(HistoryRecoveryExecutionError::NonCanonicalRecord);
        }
        Ok(record)
    }

    fn verify(&self) -> Result<(), HistoryRecoveryExecutionError> {
        validate_record_content(&self.content)?;
        self.content
            .source_device_id
            .verify(&record_signing_bytes(&self.content)?, &self.signature)?;
        Ok(())
    }

    fn record_id(&self) -> Result<[u8; 32], HistoryRecoveryExecutionError> {
        Ok(domain_digest(RECORD_ID_DOMAIN, &self.encode()?))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ExecutionHeadWire {
    version: u8,
    selection_key: [u8; 32],
    consent_record_id: [u8; 32],
    plan_id: [u8; 32],
    selection_index: u16,
    record_id: [u8; 32],
    generation: u64,
    checkpoint_after_id: [u8; 32],
    next_range_start: u64,
    approved_range_end: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ProjectionArtifactWire {
    entries: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableHistoryRecoveryChunk {
    record_id: [u8; 32],
    record: SignedExecutionRecord,
    bundle: HistoryRewrapBundle,
    checkpoint_before: SignedHistoryRecoveryCheckpoint,
    checkpoint_after: SignedHistoryRecoveryCheckpoint,
    projections: Vec<LocalTextProjection>,
}

impl DurableHistoryRecoveryChunk {
    pub fn record_id(&self) -> [u8; 32] {
        self.record_id
    }

    pub fn generation(&self) -> u64 {
        self.record.content.generation
    }

    pub fn owner_account_id(&self) -> AccountId {
        self.record.content.owner_account_id
    }

    pub fn source_device_id(&self) -> DeviceId {
        self.record.content.source_device_id
    }

    pub fn recipient_device_id(&self) -> DeviceId {
        self.record.content.recipient_device_id
    }

    pub fn consent_record_id(&self) -> [u8; 32] {
        self.record.content.consent_record_id
    }

    pub fn selection_index(&self) -> usize {
        usize::from(self.record.content.selection_index)
    }

    pub fn conversation_id(&self) -> kilogram_protocol::ConversationId {
        self.record.content.conversation_id
    }

    pub fn range_start(&self) -> u64 {
        self.record.content.chunk_range_start
    }

    pub fn range_end(&self) -> u64 {
        self.record.content.chunk_range_end
    }

    pub fn approved_range_end(&self) -> u64 {
        self.record.content.approved_range_end
    }

    pub fn bundle(&self) -> &HistoryRewrapBundle {
        &self.bundle
    }

    pub fn checkpoint_before(&self) -> &SignedHistoryRecoveryCheckpoint {
        &self.checkpoint_before
    }

    pub fn checkpoint_after(&self) -> &SignedHistoryRecoveryCheckpoint {
        &self.checkpoint_after
    }

    pub fn projections(&self) -> &[LocalTextProjection] {
        &self.projections
    }

    pub fn is_complete(&self) -> bool {
        self.record.content.chunk_range_end >= self.record.content.approved_range_end
    }
}

pub struct OldHistoryRecoveryExecutionStore {
    directory: PathBuf,
    database: Database,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

pub struct OldHistoryRecoveryExecutionReadStore {
    directory: PathBuf,
    database: ReadOnlyDatabase,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

impl OldHistoryRecoveryExecutionReadStore {
    pub fn open_existing(
        directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Result<Self, HistoryRecoveryExecutionError> {
        let directory = directory.as_ref().to_path_buf();
        let database_path = directory.join(DATABASE_FILE);
        if !database_path.is_file() {
            return Err(HistoryRecoveryExecutionError::StoreNotInitialized);
        }
        let database = ReadOnlyDatabase::open(database_path).map_err(database_error)?;
        verify_database_metadata(&database, owner_account_id, owner_device_id)?;
        verify_all_execution(&database, owner_account_id, owner_device_id)?;
        Ok(Self {
            directory,
            database,
            owner_account_id,
            owner_device_id,
        })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn status(
        &self,
        consent_record_id: [u8; 32],
        selection_index: usize,
    ) -> Result<HistoryRecoveryExecutionStatus, HistoryRecoveryExecutionError> {
        status_for_selection(
            &self.database,
            self.owner_account_id,
            self.owner_device_id,
            consent_record_id,
            selection_index,
        )
    }

    pub fn committed_chunk(
        &self,
        record_id: [u8; 32],
    ) -> Result<DurableHistoryRecoveryChunk, HistoryRecoveryExecutionError> {
        load_durable_record(
            &self.database,
            self.owner_account_id,
            self.owner_device_id,
            record_id,
        )
    }

    pub fn committed_head_chunk(
        &self,
        consent_record_id: [u8; 32],
        selection_index: usize,
        expected_record_id: [u8; 32],
    ) -> Result<DurableHistoryRecoveryChunk, HistoryRecoveryExecutionError> {
        load_committed_head_chunk(
            &self.database,
            self.owner_account_id,
            self.owner_device_id,
            consent_record_id,
            selection_index,
            expected_record_id,
        )
    }
}

impl OldHistoryRecoveryExecutionStore {
    pub fn is_initialized(directory: impl AsRef<Path>) -> bool {
        directory.as_ref().join(DATABASE_FILE).is_file()
    }

    pub fn open(
        directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Result<Self, HistoryRecoveryExecutionError> {
        let directory = directory.as_ref().to_path_buf();
        std::fs::create_dir_all(&directory)?;
        let database_path = directory.join(DATABASE_FILE);
        let existed = database_path.is_file();
        let database = Database::create(database_path).map_err(database_error)?;
        if existed {
            verify_database_metadata(&database, owner_account_id, owner_device_id)?;
        } else {
            initialize_database(&database, owner_account_id, owner_device_id)?;
        }
        verify_all_execution(&database, owner_account_id, owner_device_id)?;
        Ok(Self {
            directory,
            database,
            owner_account_id,
            owner_device_id,
        })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn status(
        &self,
        consent_record_id: [u8; 32],
        selection_index: usize,
    ) -> Result<HistoryRecoveryExecutionStatus, HistoryRecoveryExecutionError> {
        status_for_selection(
            &self.database,
            self.owner_account_id,
            self.owner_device_id,
            consent_record_id,
            selection_index,
        )
    }

    pub fn committed_chunk(
        &self,
        record_id: [u8; 32],
    ) -> Result<DurableHistoryRecoveryChunk, HistoryRecoveryExecutionError> {
        load_durable_record(
            &self.database,
            self.owner_account_id,
            self.owner_device_id,
            record_id,
        )
    }

    pub fn committed_head_chunk(
        &self,
        consent_record_id: [u8; 32],
        selection_index: usize,
        expected_record_id: [u8; 32],
    ) -> Result<DurableHistoryRecoveryChunk, HistoryRecoveryExecutionError> {
        load_committed_head_chunk(
            &self.database,
            self.owner_account_id,
            self.owner_device_id,
            consent_record_id,
            selection_index,
            expected_record_id,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_next_chunk(
        &self,
        consent_store: &OldHistoryRecoveryConsentStore,
        transition_id: [u8; 32],
        expected_consent_record_id: [u8; 32],
        source: &DeviceIdentity,
        context: &OldHistoryRecoveryLiveContext<'_>,
        selection_index: usize,
        current_checkpoint: Option<&SignedHistoryRecoveryCheckpoint>,
        source_inventory: &[(AuthorizedEvent, String)],
    ) -> Result<PrepareHistoryRecoveryChunkOutcome, HistoryRecoveryExecutionError> {
        if source.device_id() != self.owner_device_id {
            return Err(HistoryRecoveryExecutionError::OwnerDeviceMismatch);
        }
        let plan = load_exact_consent_plan(
            consent_store,
            transition_id,
            expected_consent_record_id,
            self.owner_account_id,
            self.owner_device_id,
            context,
        )?;
        let selection = plan.selection(selection_index).ok_or(
            HistoryRecoveryExecutionError::SelectionMissing(selection_index),
        )?;
        let selection_index_u16 = u16::try_from(selection_index)
            .map_err(|_| HistoryRecoveryExecutionError::SelectionMissing(selection_index))?;
        validate_source_inventory_claim(source_inventory, selection)?;
        let key = selection_key(expected_consent_record_id, selection_index_u16);

        if let Some(replay) = find_record_for_checkpoint_input(
            &self.database,
            self.owner_account_id,
            self.owner_device_id,
            key,
            current_checkpoint,
        )? {
            return Ok(PrepareHistoryRecoveryChunkOutcome::AlreadyCommitted(
                Box::new(replay),
            ));
        }

        let head = load_head(&self.database, key)?;
        let range_start = match (&head, current_checkpoint) {
            (Some(head), Some(checkpoint)) => {
                if checkpoint.checkpoint_id()? != head.checkpoint_after_id {
                    return Err(HistoryRecoveryExecutionError::CheckpointHeadMismatch);
                }
                validate_checkpoint_for_selection(
                    checkpoint,
                    selection,
                    plan.history_account_id(),
                    plan.source_device_id(),
                    plan.replacement_device_id(),
                    context.authority,
                    head.next_range_start,
                )?;
                if head.next_range_start >= selection.range_end() {
                    return Ok(PrepareHistoryRecoveryChunkOutcome::Complete(status_view(
                        head,
                    )));
                }
                head.next_range_start
            }
            (Some(_), None) => return Err(HistoryRecoveryExecutionError::CheckpointHeadMismatch),
            (None, checkpoint) => {
                validate_initial_checkpoint(
                    checkpoint,
                    selection,
                    plan.history_account_id(),
                    plan.source_device_id(),
                    plan.replacement_device_id(),
                    context.authority,
                )?;
                checkpoint.map_or(selection.range_start(), |value| value.next_range_start())
            }
        };
        let page_size =
            current_checkpoint.map_or(MAX_HISTORY_RECOVERY_EXECUTION_CHUNK_EVENTS, |checkpoint| {
                usize::try_from(checkpoint.page_size())
                    .unwrap_or(MAX_HISTORY_RECOVERY_EXECUTION_CHUNK_EVENTS)
                    .min(MAX_HISTORY_RECOVERY_EXECUTION_CHUNK_EVENTS)
            });
        let range_end = selection.range_end().min(
            range_start.saturating_add(
                u64::try_from(page_size)
                    .map_err(|_| HistoryRecoveryExecutionError::InvalidPreparedChunk)?,
            ),
        );
        let bundle = HistoryRewrapBundle::seal(
            source,
            context.authority.clone(),
            plan.replacement_device_id(),
            selection.conversation_id(),
            source_inventory,
            usize::try_from(range_start)
                .map_err(|_| HistoryRecoveryExecutionError::InvalidPreparedChunk)?,
            usize::try_from(range_end)
                .map_err(|_| HistoryRecoveryExecutionError::InvalidPreparedChunk)?,
        )?;
        validate_bundle_for_selection(&bundle, selection, plan.history_account_id())?;
        Ok(PrepareHistoryRecoveryChunkOutcome::Prepared(Box::new(
            PreparedHistoryRecoveryChunk {
                consent_record_id: expected_consent_record_id,
                plan_id: plan.plan_id()?,
                transition_id,
                selection_index: selection_index_u16,
                selection,
                checkpoint_before: current_checkpoint.cloned(),
                checkpoint_started_here: current_checkpoint.is_none(),
                page_size,
                bundle,
            },
        )))
    }

    pub fn apply_prepared_chunk(
        prepared: PreparedHistoryRecoveryChunk,
        recipient: &DeviceIdentity,
        recipient_encryption: &DeviceEncryptionIdentity,
    ) -> Result<AppliedHistoryRecoveryChunk, HistoryRecoveryExecutionError> {
        validate_prepared_chunk(&prepared)?;
        if recipient.device_id() != prepared.bundle.manifest().recipient_device_id() {
            return Err(HistoryRecoveryExecutionError::WrongRecipientDevice);
        }
        let checkpoint_before = match &prepared.checkpoint_before {
            Some(checkpoint) => checkpoint.clone(),
            None => {
                let approved_count = usize::try_from(
                    prepared
                        .selection
                        .range_end()
                        .saturating_sub(prepared.selection.range_start()),
                )
                .map_err(|_| HistoryRecoveryExecutionError::InvalidPreparedChunk)?;
                SignedHistoryRecoveryCheckpoint::start(
                    recipient,
                    prepared.bundle.manifest().account_id(),
                    prepared.selection.conversation_id(),
                    prepared.bundle.manifest().source_device_id(),
                    HistoryRewrapSas::derive(
                        prepared.bundle.manifest().account_device_list(),
                        prepared.bundle.manifest().source_device_id(),
                        recipient.device_id(),
                    )?,
                    usize::try_from(prepared.selection.range_start())
                        .map_err(|_| HistoryRecoveryExecutionError::InvalidPreparedChunk)?,
                    approved_count,
                    prepared.page_size,
                )?
            }
        };
        validate_checkpoint_for_selection(
            &checkpoint_before,
            prepared.selection,
            prepared.bundle.manifest().account_id(),
            prepared.bundle.manifest().source_device_id(),
            recipient.device_id(),
            prepared.bundle.manifest().account_device_list(),
            prepared.bundle.manifest().range_start(),
        )?;

        let mut entries = Vec::with_capacity(prepared.bundle.entries().len());
        for (offset, rewrap_entry) in prepared.bundle.entries().iter().enumerate() {
            let event = rewrap_entry.event().clone();
            let projection = LocalTextProjection::from_history_rewrap(
                event.event(),
                &prepared.bundle,
                offset,
                recipient.device_id(),
                prepared.bundle.manifest().account_id(),
                recipient_encryption,
            )?;
            entries.push(AppliedHistoryRecoveryEntry { event, projection });
        }
        let checkpoint_after = checkpoint_before.advance(recipient, prepared.bundle.manifest())?;
        let applied = AppliedHistoryRecoveryChunk {
            prepared,
            checkpoint_before,
            checkpoint_after,
            entries,
        };
        validate_applied_chunk(&applied)?;
        Ok(applied)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn commit_chunk(
        &self,
        consent_store: &OldHistoryRecoveryConsentStore,
        expected_consent_record_id: [u8; 32],
        source: &DeviceIdentity,
        context: &OldHistoryRecoveryLiveContext<'_>,
        applied: &AppliedHistoryRecoveryChunk,
        committed_at_unix_seconds: u64,
    ) -> Result<CommitHistoryRecoveryChunkOutcome, HistoryRecoveryExecutionError> {
        self.commit_chunk_inner(
            consent_store,
            expected_consent_record_id,
            source,
            context,
            applied,
            committed_at_unix_seconds,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn commit_chunk_inner(
        &self,
        consent_store: &OldHistoryRecoveryConsentStore,
        expected_consent_record_id: [u8; 32],
        source: &DeviceIdentity,
        context: &OldHistoryRecoveryLiveContext<'_>,
        applied: &AppliedHistoryRecoveryChunk,
        committed_at_unix_seconds: u64,
        fail_before_commit: bool,
    ) -> Result<CommitHistoryRecoveryChunkOutcome, HistoryRecoveryExecutionError> {
        if source.device_id() != self.owner_device_id {
            return Err(HistoryRecoveryExecutionError::OwnerDeviceMismatch);
        }
        validate_applied_chunk(applied)?;
        if applied.prepared.consent_record_id != expected_consent_record_id {
            return Err(HistoryRecoveryExecutionError::ConsentRecordMismatch);
        }
        let plan = load_exact_consent_plan(
            consent_store,
            applied.prepared.transition_id,
            expected_consent_record_id,
            self.owner_account_id,
            self.owner_device_id,
            context,
        )?;
        if plan.plan_id()? != applied.prepared.plan_id
            || plan.selection(applied.prepared.selection_index())
                != Some(applied.prepared.selection)
        {
            return Err(HistoryRecoveryExecutionError::PlanContextMismatch);
        }

        let key = selection_key(expected_consent_record_id, applied.prepared.selection_index);
        if let Some(existing) = find_record_for_checkpoint_before_id(
            &self.database,
            self.owner_account_id,
            self.owner_device_id,
            key,
            applied.checkpoint_before.checkpoint_id()?,
        )? {
            if durable_matches_applied(&existing, applied)? {
                return Ok(CommitHistoryRecoveryChunkOutcome::AlreadyCommitted(
                    existing,
                ));
            }
            return Err(HistoryRecoveryExecutionError::DivergentCommittedChunk);
        }

        let head = load_head(&self.database, key)?;
        let (generation, previous_record_id) = match &head {
            Some(head) => {
                if applied.prepared.checkpoint_started_here
                    || applied.checkpoint_before.checkpoint_id()? != head.checkpoint_after_id
                    || applied.checkpoint_before.next_range_start() != head.next_range_start
                {
                    return Err(HistoryRecoveryExecutionError::CheckpointHeadMismatch);
                }
                (
                    head.generation
                        .checked_add(1)
                        .ok_or(HistoryRecoveryExecutionError::ExecutionCapacity)?,
                    Some(head.record_id),
                )
            }
            None => {
                validate_initial_applied_checkpoint(applied)?;
                (0, None)
            }
        };

        let bundle_bytes = applied.prepared.bundle.encode()?;
        validate_bundle_length(&bundle_bytes)?;
        let bundle_id = *applied.prepared.bundle.bundle_id()?.as_bytes();
        let before_bytes = applied.checkpoint_before.encode()?;
        let after_bytes = applied.checkpoint_after.encode()?;
        validate_checkpoint_length(&before_bytes)?;
        validate_checkpoint_length(&after_bytes)?;
        let checkpoint_before_id = applied.checkpoint_before.checkpoint_id()?;
        let checkpoint_after_id = applied.checkpoint_after.checkpoint_id()?;
        let projection_bytes = encode_projection_artifact(
            &applied
                .entries
                .iter()
                .map(|entry| entry.projection.clone())
                .collect::<Vec<_>>(),
        )?;
        let content = ExecutionRecordContent {
            version: RECORD_VERSION,
            owner_account_id: self.owner_account_id,
            source_device_id: self.owner_device_id,
            recipient_device_id: plan.replacement_device_id(),
            consent_record_id: expected_consent_record_id,
            plan_id: plan.plan_id()?,
            transition_id: plan.transition_id(),
            selection_key: key,
            selection_index: applied.prepared.selection_index,
            conversation_id: applied.prepared.selection.conversation_id(),
            inventory_event_count: applied.prepared.selection.inventory_event_count(),
            inventory_digest: applied.prepared.selection.inventory_digest(),
            approved_range_start: applied.prepared.selection.range_start(),
            approved_range_end: applied.prepared.selection.range_end(),
            generation,
            previous_record_id,
            checkpoint_started_here: applied.prepared.checkpoint_started_here,
            checkpoint_before_id,
            checkpoint_after_id,
            chunk_range_start: applied.prepared.bundle.manifest().range_start(),
            chunk_range_end: applied.prepared.bundle.manifest().range_end(),
            bundle_id,
            bundle_digest: domain_digest(BUNDLE_DIGEST_DOMAIN, &bundle_bytes),
            projection_count: u16::try_from(applied.entries.len())
                .map_err(|_| HistoryRecoveryExecutionError::InvalidProjectionArtifact)?,
            projection_digest: domain_digest(PROJECTION_DIGEST_DOMAIN, &projection_bytes),
            committed_at_unix_seconds,
        };
        let record = SignedExecutionRecord::issue(source, content)?;
        let record_id = record.record_id()?;
        let record_bytes = record.encode()?;
        let new_head = head_wire(&record, record_id);
        let head_bytes = postcard::to_allocvec(&new_head)?;
        validate_head_length(&head_bytes)?;

        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut records = write.open_table(RECORD_TABLE).map_err(database_error)?;
            let mut heads = write.open_table(HEAD_TABLE).map_err(database_error)?;
            let mut bundles = write.open_table(BUNDLE_TABLE).map_err(database_error)?;
            let mut checkpoints = write.open_table(CHECKPOINT_TABLE).map_err(database_error)?;
            let mut projections = write.open_table(PROJECTION_TABLE).map_err(database_error)?;

            verify_write_head(&heads, &records, key, head.as_ref())?;
            if records.len().map_err(database_error)?
                >= MAX_DURABLE_HISTORY_RECOVERY_EXECUTION_CHUNKS
            {
                return Err(HistoryRecoveryExecutionError::ExecutionCapacity);
            }
            insert_exact_artifact(&mut bundles, &bundle_id, &bundle_bytes)?;
            insert_exact_artifact(&mut checkpoints, &checkpoint_before_id, &before_bytes)?;
            insert_exact_artifact(&mut checkpoints, &checkpoint_after_id, &after_bytes)?;
            if let Some(existing) = records.get(record_id.as_slice()).map_err(database_error)? {
                if existing.value() != record_bytes.as_slice() {
                    return Err(HistoryRecoveryExecutionError::RecordIdCollision);
                }
                return Err(HistoryRecoveryExecutionError::OrphanRecord);
            }
            records
                .insert(record_id.as_slice(), record_bytes.as_slice())
                .map_err(database_error)?;
            insert_exact_artifact(&mut projections, &record_id, &projection_bytes)?;
            heads
                .insert(key.as_slice(), head_bytes.as_slice())
                .map_err(database_error)?;
        }
        if fail_before_commit {
            return Err(HistoryRecoveryExecutionError::InjectedBeforeCommit);
        }
        write.commit().map_err(database_error)?;
        let durable = load_durable_record(
            &self.database,
            self.owner_account_id,
            self.owner_device_id,
            record_id,
        )?;
        if !durable_matches_applied(&durable, applied)? {
            return Err(HistoryRecoveryExecutionError::CommittedRecordMismatch);
        }
        Ok(CommitHistoryRecoveryChunkOutcome::Committed(durable))
    }
}

fn load_exact_consent_plan(
    consent_store: &OldHistoryRecoveryConsentStore,
    transition_id: [u8; 32],
    expected_consent_record_id: [u8; 32],
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    context: &OldHistoryRecoveryLiveContext<'_>,
) -> Result<SignedOldHistoryRecoveryPlan, HistoryRecoveryExecutionError> {
    let consent = consent_store
        .load_for_transition(transition_id)?
        .ok_or(HistoryRecoveryExecutionError::ConsentMissing)?;
    if consent.record_id() != expected_consent_record_id {
        return Err(HistoryRecoveryExecutionError::ConsentRecordMismatch);
    }
    let plan = consent.signed_plan()?;
    if plan.history_account_id() != owner_account_id
        || plan.source_device_id() != owner_device_id
        || plan.transition_id() != transition_id
    {
        return Err(HistoryRecoveryExecutionError::ConsentOwnerMismatch);
    }
    plan.verify_committed_context(
        context.binding,
        context.authority,
        context.memberships,
        context.inventory,
        owner_device_id,
        context.selections,
    )?;
    Ok(plan)
}

fn validate_source_inventory_claim(
    source_inventory: &[(AuthorizedEvent, String)],
    selection: BoundHistoryRecoverySelection,
) -> Result<(), HistoryRecoveryExecutionError> {
    if u64::try_from(source_inventory.len()).ok() != Some(selection.inventory_event_count()) {
        return Err(HistoryRecoveryExecutionError::SourceInventoryMismatch);
    }
    Ok(())
}

fn validate_prepared_chunk(
    prepared: &PreparedHistoryRecoveryChunk,
) -> Result<(), HistoryRecoveryExecutionError> {
    if prepared.page_size == 0
        || prepared.page_size > MAX_HISTORY_RECOVERY_EXECUTION_CHUNK_EVENTS
        || prepared.bundle.manifest().range_start() < prepared.selection.range_start()
        || prepared.bundle.manifest().range_end() > prepared.selection.range_end()
        || prepared.bundle.manifest().range_start() >= prepared.bundle.manifest().range_end()
    {
        return Err(HistoryRecoveryExecutionError::InvalidPreparedChunk);
    }
    validate_bundle_for_selection(
        &prepared.bundle,
        prepared.selection,
        prepared.bundle.manifest().account_id(),
    )?;
    if prepared.checkpoint_started_here != prepared.checkpoint_before.is_none() {
        return Err(HistoryRecoveryExecutionError::InvalidPreparedChunk);
    }
    Ok(())
}

fn validate_bundle_for_selection(
    bundle: &HistoryRewrapBundle,
    selection: BoundHistoryRecoverySelection,
    expected_account_id: AccountId,
) -> Result<(), HistoryRecoveryExecutionError> {
    bundle.verify()?;
    let manifest = bundle.manifest();
    if manifest.account_id() != expected_account_id
        || manifest.conversation_id() != selection.conversation_id()
        || manifest.inventory_event_count() != selection.inventory_event_count()
        || *manifest.inventory_digest() != selection.inventory_digest()
        || manifest.range_start() < selection.range_start()
        || manifest.range_end() > selection.range_end()
        || manifest.range_end().saturating_sub(manifest.range_start())
            > MAX_HISTORY_RECOVERY_EXECUTION_CHUNK_EVENTS as u64
    {
        return Err(HistoryRecoveryExecutionError::BundlePlanMismatch);
    }
    Ok(())
}

fn validate_initial_checkpoint(
    checkpoint: Option<&SignedHistoryRecoveryCheckpoint>,
    selection: BoundHistoryRecoverySelection,
    account_id: AccountId,
    source_device_id: DeviceId,
    recipient_device_id: DeviceId,
    authority: &AccountDeviceListSnapshot,
) -> Result<(), HistoryRecoveryExecutionError> {
    match (selection.checkpoint_id(), checkpoint) {
        (None, None) => Ok(()),
        (Some(expected), Some(checkpoint)) if checkpoint.checkpoint_id()? == expected => {
            if selection.checkpoint_next_range_start() != Some(checkpoint.next_range_start()) {
                return Err(HistoryRecoveryExecutionError::CheckpointPlanMismatch);
            }
            validate_checkpoint_for_selection(
                checkpoint,
                selection,
                account_id,
                source_device_id,
                recipient_device_id,
                authority,
                selection.range_start(),
            )
        }
        _ => Err(HistoryRecoveryExecutionError::CheckpointPlanMismatch),
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_checkpoint_for_selection(
    checkpoint: &SignedHistoryRecoveryCheckpoint,
    selection: BoundHistoryRecoverySelection,
    account_id: AccountId,
    source_device_id: DeviceId,
    recipient_device_id: DeviceId,
    authority: &AccountDeviceListSnapshot,
    expected_next: u64,
) -> Result<(), HistoryRecoveryExecutionError> {
    checkpoint.verify_signature()?;
    if checkpoint.account_id() != account_id
        || checkpoint.conversation_id() != selection.conversation_id()
        || checkpoint.source_device_id() != source_device_id
        || checkpoint.recipient_device_id() != recipient_device_id
        || checkpoint.approved_range_start() > selection.range_start()
        || checkpoint.approved_range_end() < selection.range_end()
        || checkpoint.next_range_start() != expected_next
        || checkpoint.sas()
            != HistoryRewrapSas::derive(authority, source_device_id, recipient_device_id)?
    {
        return Err(HistoryRecoveryExecutionError::CheckpointPlanMismatch);
    }
    match (
        checkpoint.inventory_event_count(),
        checkpoint.inventory_digest(),
    ) {
        (None, None) => {}
        (Some(count), Some(digest))
            if count == selection.inventory_event_count()
                && *digest == selection.inventory_digest() => {}
        _ => return Err(HistoryRecoveryExecutionError::CheckpointPlanMismatch),
    }
    Ok(())
}

fn validate_applied_chunk(
    applied: &AppliedHistoryRecoveryChunk,
) -> Result<(), HistoryRecoveryExecutionError> {
    validate_prepared_chunk(&applied.prepared)?;
    let manifest = applied.prepared.bundle.manifest();
    if applied.entries.len() != applied.prepared.bundle.entries().len()
        || applied.checkpoint_before.next_range_start() != manifest.range_start()
        || applied.checkpoint_after.next_range_start() != manifest.range_end()
        || applied.checkpoint_after.previous_checkpoint_id()
            != Some(&applied.checkpoint_before.checkpoint_id()?)
    {
        return Err(HistoryRecoveryExecutionError::InvalidAppliedChunk);
    }
    for (entry, rewrap_entry) in applied
        .entries
        .iter()
        .zip(applied.prepared.bundle.entries())
    {
        let encoded = entry.projection.encode()?;
        let decoded = LocalTextProjection::decode(&encoded)?;
        if decoded != entry.projection
            || entry.event != *rewrap_entry.event()
            || entry.projection.event_id() != entry.event.event().event_id()?
            || entry.projection.local_device_id() != manifest.recipient_device_id()
            || entry.projection.history_rewrap_manifest() != Some(manifest)
        {
            return Err(HistoryRecoveryExecutionError::InvalidAppliedChunk);
        }
    }
    Ok(())
}

fn validate_initial_applied_checkpoint(
    applied: &AppliedHistoryRecoveryChunk,
) -> Result<(), HistoryRecoveryExecutionError> {
    let selection = applied.prepared.selection;
    let before = &applied.checkpoint_before;
    if applied.prepared.checkpoint_started_here {
        if selection.checkpoint_id().is_some()
            || before.previous_checkpoint_id().is_some()
            || before.approved_range_start() != selection.range_start()
            || before.approved_range_end() != selection.range_end()
            || before.next_range_start() != selection.range_start()
        {
            return Err(HistoryRecoveryExecutionError::CheckpointPlanMismatch);
        }
    } else if selection.checkpoint_id() != Some(before.checkpoint_id()?)
        || selection.checkpoint_next_range_start() != Some(before.next_range_start())
    {
        return Err(HistoryRecoveryExecutionError::CheckpointPlanMismatch);
    }
    Ok(())
}

fn durable_matches_applied(
    durable: &DurableHistoryRecoveryChunk,
    applied: &AppliedHistoryRecoveryChunk,
) -> Result<bool, HistoryRecoveryExecutionError> {
    let projections = applied
        .entries
        .iter()
        .map(|entry| entry.projection.clone())
        .collect::<Vec<_>>();
    Ok(durable.bundle == applied.prepared.bundle
        && durable.checkpoint_before == applied.checkpoint_before
        && durable.checkpoint_after == applied.checkpoint_after
        && durable.projections == projections)
}

fn selection_key(consent_record_id: [u8; 32], selection_index: u16) -> [u8; 32] {
    let mut bytes = Vec::with_capacity(34);
    bytes.extend_from_slice(&consent_record_id);
    bytes.extend_from_slice(&selection_index.to_le_bytes());
    domain_digest(SELECTION_KEY_DOMAIN, &bytes)
}

fn status_view(head: &ExecutionHeadWire) -> HistoryRecoveryExecutionStatusView {
    let digest = domain_digest(SELECTION_TOKEN_DOMAIN, &head.selection_key);
    let mut selection_token = [0_u8; 16];
    selection_token.copy_from_slice(&digest[..16]);
    HistoryRecoveryExecutionStatusView {
        selection_token,
        committed_chunk_count: head.generation.saturating_add(1),
        next_range_start: head.next_range_start,
        approved_range_end: head.approved_range_end,
        contains_message_bodies: false,
        contains_plaintext: false,
        grants_network_authority: false,
        grants_store_import_authority: false,
    }
}

fn status_for_selection<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    consent_record_id: [u8; 32],
    selection_index: usize,
) -> Result<HistoryRecoveryExecutionStatus, HistoryRecoveryExecutionError> {
    let selection_index = u16::try_from(selection_index)
        .map_err(|_| HistoryRecoveryExecutionError::SelectionMissing(selection_index))?;
    let key = selection_key(consent_record_id, selection_index);
    let Some(head) = load_head(database, key)? else {
        return Ok(HistoryRecoveryExecutionStatus::Absent);
    };
    let record = load_durable_record(database, owner_account_id, owner_device_id, head.record_id)?;
    if record.record.content.consent_record_id != consent_record_id
        || record.record.content.selection_index != selection_index
    {
        return Err(HistoryRecoveryExecutionError::HeadRecordMismatch);
    }
    let view = status_view(&head);
    if head.next_range_start >= head.approved_range_end {
        Ok(HistoryRecoveryExecutionStatus::Complete(view))
    } else {
        Ok(HistoryRecoveryExecutionStatus::InProgress(view))
    }
}

fn initialize_database(
    database: &Database,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), HistoryRecoveryExecutionError> {
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
        let _ = write.open_table(RECORD_TABLE).map_err(database_error)?;
        let _ = write.open_table(HEAD_TABLE).map_err(database_error)?;
        let _ = write.open_table(BUNDLE_TABLE).map_err(database_error)?;
        let _ = write.open_table(CHECKPOINT_TABLE).map_err(database_error)?;
        let _ = write.open_table(PROJECTION_TABLE).map_err(database_error)?;
    }
    write.commit().map_err(database_error)
}

fn verify_database_metadata<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), HistoryRecoveryExecutionError> {
    let read = database.begin_read().map_err(database_error)?;
    let meta = read.open_table(META_TABLE).map_err(database_error)?;
    let schema = meta
        .get(SCHEMA_KEY)
        .map_err(database_error)?
        .ok_or(HistoryRecoveryExecutionError::SchemaMismatch)?;
    if schema.value() != [SCHEMA_VERSION] {
        return Err(HistoryRecoveryExecutionError::SchemaMismatch);
    }
    let account = meta
        .get(OWNER_ACCOUNT_KEY)
        .map_err(database_error)?
        .ok_or(HistoryRecoveryExecutionError::OwnerAccountMismatch)?;
    if account.value() != owner_account_id.as_bytes() {
        return Err(HistoryRecoveryExecutionError::OwnerAccountMismatch);
    }
    let device = meta
        .get(OWNER_DEVICE_KEY)
        .map_err(database_error)?
        .ok_or(HistoryRecoveryExecutionError::OwnerDeviceMismatch)?;
    if device.value() != owner_device_id.as_bytes() {
        return Err(HistoryRecoveryExecutionError::OwnerDeviceMismatch);
    }
    Ok(())
}

fn verify_all_execution<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), HistoryRecoveryExecutionError> {
    let read = database.begin_read().map_err(database_error)?;
    let records_table = read.open_table(RECORD_TABLE).map_err(database_error)?;
    let heads_table = read.open_table(HEAD_TABLE).map_err(database_error)?;
    if records_table.len().map_err(database_error)? > MAX_DURABLE_HISTORY_RECOVERY_EXECUTION_CHUNKS
        || heads_table.len().map_err(database_error)?
            > MAX_DURABLE_HISTORY_RECOVERY_EXECUTION_CHUNKS
    {
        return Err(HistoryRecoveryExecutionError::ExecutionCapacity);
    }
    let mut records = HashMap::new();
    let mut referenced_bundles = HashSet::new();
    let mut referenced_checkpoints = HashSet::new();
    let mut referenced_projections = HashSet::new();
    for row in records_table.iter().map_err(database_error)? {
        let (key, value) = row.map_err(database_error)?;
        let record = SignedExecutionRecord::decode_and_verify(
            value.value(),
            owner_account_id,
            owner_device_id,
        )?;
        let record_id = copy_id(key.value())?;
        if record.record_id()? != record_id {
            return Err(HistoryRecoveryExecutionError::RecordIdBinding);
        }
        verify_record_artifacts(&read, &record, record_id)?;
        referenced_bundles.insert(record.content.bundle_id);
        referenced_checkpoints.insert(record.content.checkpoint_before_id);
        referenced_checkpoints.insert(record.content.checkpoint_after_id);
        referenced_projections.insert(record_id);
        records.insert(record_id, record);
    }

    let bundle_keys = table_id_keys(&read.open_table(BUNDLE_TABLE).map_err(database_error)?)?;
    let checkpoint_keys =
        table_id_keys(&read.open_table(CHECKPOINT_TABLE).map_err(database_error)?)?;
    let projection_keys =
        table_id_keys(&read.open_table(PROJECTION_TABLE).map_err(database_error)?)?;
    if bundle_keys != referenced_bundles
        || checkpoint_keys != referenced_checkpoints
        || projection_keys != referenced_projections
    {
        return Err(HistoryRecoveryExecutionError::OrphanArtifact);
    }

    let mut heads = HashMap::new();
    for row in heads_table.iter().map_err(database_error)? {
        let (key, value) = row.map_err(database_error)?;
        let selection_key = copy_id(key.value())?;
        let head = decode_head(value.value(), selection_key)?;
        let record = records
            .get(&head.record_id)
            .ok_or(HistoryRecoveryExecutionError::MissingRecord)?;
        if head != head_wire(record, head.record_id) {
            return Err(HistoryRecoveryExecutionError::HeadRecordMismatch);
        }
        heads.insert(selection_key, head);
    }

    let mut children = HashMap::<[u8; 32], usize>::new();
    for record in records.values() {
        if let Some(previous) = record.content.previous_record_id {
            let parent = records
                .get(&previous)
                .ok_or(HistoryRecoveryExecutionError::OrphanRecord)?;
            if parent.content.selection_key != record.content.selection_key
                || parent.content.generation.checked_add(1) != Some(record.content.generation)
                || parent.content.checkpoint_after_id != record.content.checkpoint_before_id
                || parent.content.chunk_range_end != record.content.chunk_range_start
            {
                return Err(HistoryRecoveryExecutionError::BrokenRecordChain);
            }
            *children.entry(previous).or_default() += 1;
        } else if record.content.generation != 0 {
            return Err(HistoryRecoveryExecutionError::BrokenRecordChain);
        }
    }
    if children.values().any(|count| *count > 1) {
        return Err(HistoryRecoveryExecutionError::ForkedRecordChain);
    }

    let mut reachable = HashSet::new();
    for head in heads.values() {
        let mut current = Some(head.record_id);
        while let Some(record_id) = current {
            if !reachable.insert(record_id) {
                return Err(HistoryRecoveryExecutionError::DuplicateHeadChain);
            }
            current = records
                .get(&record_id)
                .ok_or(HistoryRecoveryExecutionError::MissingRecord)?
                .content
                .previous_record_id;
        }
    }
    if reachable.len() != records.len() {
        return Err(HistoryRecoveryExecutionError::OrphanRecord);
    }
    Ok(())
}

fn table_id_keys(
    table: &impl ReadableTable<&'static [u8], &'static [u8]>,
) -> Result<HashSet<[u8; 32]>, HistoryRecoveryExecutionError> {
    let mut keys = HashSet::new();
    for row in table.iter().map_err(database_error)? {
        let (key, _) = row.map_err(database_error)?;
        if !keys.insert(copy_id(key.value())?) {
            return Err(HistoryRecoveryExecutionError::ArtifactIdCollision);
        }
    }
    Ok(keys)
}

fn verify_record_artifacts(
    read: &redb::ReadTransaction,
    record: &SignedExecutionRecord,
    record_id: [u8; 32],
) -> Result<(), HistoryRecoveryExecutionError> {
    let bundles = read.open_table(BUNDLE_TABLE).map_err(database_error)?;
    let checkpoints = read.open_table(CHECKPOINT_TABLE).map_err(database_error)?;
    let projections = read.open_table(PROJECTION_TABLE).map_err(database_error)?;
    let bundle_bytes = bundles
        .get(record.content.bundle_id.as_slice())
        .map_err(database_error)?
        .ok_or(HistoryRecoveryExecutionError::MissingBundle)?;
    validate_bundle_length(bundle_bytes.value())?;
    if domain_digest(BUNDLE_DIGEST_DOMAIN, bundle_bytes.value()) != record.content.bundle_digest {
        return Err(HistoryRecoveryExecutionError::BundleDigestMismatch);
    }
    let bundle = HistoryRewrapBundle::decode_and_verify(bundle_bytes.value())?;
    if bundle.bundle_id()?.as_bytes() != &record.content.bundle_id {
        return Err(HistoryRecoveryExecutionError::BundleIdBinding);
    }
    verify_bundle_against_record(&bundle, &record.content)?;

    let before_bytes = checkpoints
        .get(record.content.checkpoint_before_id.as_slice())
        .map_err(database_error)?
        .ok_or(HistoryRecoveryExecutionError::MissingCheckpoint)?;
    let after_bytes = checkpoints
        .get(record.content.checkpoint_after_id.as_slice())
        .map_err(database_error)?
        .ok_or(HistoryRecoveryExecutionError::MissingCheckpoint)?;
    validate_checkpoint_length(before_bytes.value())?;
    validate_checkpoint_length(after_bytes.value())?;
    let before = SignedHistoryRecoveryCheckpoint::decode_and_verify(before_bytes.value())?;
    let after = SignedHistoryRecoveryCheckpoint::decode_and_verify(after_bytes.value())?;
    if before.checkpoint_id()? != record.content.checkpoint_before_id
        || after.checkpoint_id()? != record.content.checkpoint_after_id
    {
        return Err(HistoryRecoveryExecutionError::CheckpointIdBinding);
    }
    verify_checkpoint_transition_against_record(&before, &after, &record.content)?;

    let projection_bytes = projections
        .get(record_id.as_slice())
        .map_err(database_error)?
        .ok_or(HistoryRecoveryExecutionError::MissingProjectionArtifact)?;
    validate_projection_length(projection_bytes.value())?;
    if domain_digest(PROJECTION_DIGEST_DOMAIN, projection_bytes.value())
        != record.content.projection_digest
    {
        return Err(HistoryRecoveryExecutionError::ProjectionDigestMismatch);
    }
    let decoded = decode_projection_artifact(projection_bytes.value())?;
    verify_projections_against_bundle(&decoded, &bundle, record.content.recipient_device_id)?;
    if usize::from(record.content.projection_count) != decoded.len() {
        return Err(HistoryRecoveryExecutionError::InvalidProjectionArtifact);
    }
    Ok(())
}

fn load_durable_record<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    record_id: [u8; 32],
) -> Result<DurableHistoryRecoveryChunk, HistoryRecoveryExecutionError> {
    let read = database.begin_read().map_err(database_error)?;
    let records = read.open_table(RECORD_TABLE).map_err(database_error)?;
    let encoded = records
        .get(record_id.as_slice())
        .map_err(database_error)?
        .ok_or(HistoryRecoveryExecutionError::MissingRecord)?;
    let record = SignedExecutionRecord::decode_and_verify(
        encoded.value(),
        owner_account_id,
        owner_device_id,
    )?;
    if record.record_id()? != record_id {
        return Err(HistoryRecoveryExecutionError::RecordIdBinding);
    }
    verify_record_artifacts(&read, &record, record_id)?;
    let bundles = read.open_table(BUNDLE_TABLE).map_err(database_error)?;
    let checkpoints = read.open_table(CHECKPOINT_TABLE).map_err(database_error)?;
    let projections = read.open_table(PROJECTION_TABLE).map_err(database_error)?;
    let bundle = HistoryRewrapBundle::decode_and_verify(
        bundles
            .get(record.content.bundle_id.as_slice())
            .map_err(database_error)?
            .ok_or(HistoryRecoveryExecutionError::MissingBundle)?
            .value(),
    )?;
    let checkpoint_before = SignedHistoryRecoveryCheckpoint::decode_and_verify(
        checkpoints
            .get(record.content.checkpoint_before_id.as_slice())
            .map_err(database_error)?
            .ok_or(HistoryRecoveryExecutionError::MissingCheckpoint)?
            .value(),
    )?;
    let checkpoint_after = SignedHistoryRecoveryCheckpoint::decode_and_verify(
        checkpoints
            .get(record.content.checkpoint_after_id.as_slice())
            .map_err(database_error)?
            .ok_or(HistoryRecoveryExecutionError::MissingCheckpoint)?
            .value(),
    )?;
    let projection_bytes = projections
        .get(record_id.as_slice())
        .map_err(database_error)?
        .ok_or(HistoryRecoveryExecutionError::MissingProjectionArtifact)?;
    let decoded_projections = decode_projection_artifact(projection_bytes.value())?;
    Ok(DurableHistoryRecoveryChunk {
        record_id,
        record,
        bundle,
        checkpoint_before,
        checkpoint_after,
        projections: decoded_projections,
    })
}

fn load_committed_head_chunk<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    consent_record_id: [u8; 32],
    selection_index: usize,
    expected_record_id: [u8; 32],
) -> Result<DurableHistoryRecoveryChunk, HistoryRecoveryExecutionError> {
    let selection_index = u16::try_from(selection_index)
        .map_err(|_| HistoryRecoveryExecutionError::SelectionMissing(selection_index))?;
    let key = selection_key(consent_record_id, selection_index);
    let head =
        load_head(database, key)?.ok_or(HistoryRecoveryExecutionError::CommittedHeadMismatch)?;
    if head.consent_record_id != consent_record_id
        || head.selection_index != selection_index
        || head.record_id != expected_record_id
    {
        return Err(HistoryRecoveryExecutionError::CommittedHeadMismatch);
    }
    let chunk = load_durable_record(
        database,
        owner_account_id,
        owner_device_id,
        expected_record_id,
    )?;
    if chunk.consent_record_id() != consent_record_id
        || chunk.selection_index() != usize::from(selection_index)
        || chunk.generation() != head.generation
        || chunk.checkpoint_after().checkpoint_id()? != head.checkpoint_after_id
        || chunk.range_end() != head.next_range_start
        || chunk.approved_range_end() != head.approved_range_end
    {
        return Err(HistoryRecoveryExecutionError::HeadRecordMismatch);
    }
    Ok(chunk)
}

fn find_record_for_checkpoint_input<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    key: [u8; 32],
    current_checkpoint: Option<&SignedHistoryRecoveryCheckpoint>,
) -> Result<Option<DurableHistoryRecoveryChunk>, HistoryRecoveryExecutionError> {
    let read = database.begin_read().map_err(database_error)?;
    let records = read.open_table(RECORD_TABLE).map_err(database_error)?;
    let current_id = current_checkpoint
        .map(SignedHistoryRecoveryCheckpoint::checkpoint_id)
        .transpose()?;
    for row in records.iter().map_err(database_error)? {
        let (record_id, encoded) = row.map_err(database_error)?;
        let record = SignedExecutionRecord::decode_and_verify(
            encoded.value(),
            owner_account_id,
            owner_device_id,
        )?;
        if record.content.selection_key == key
            && match current_id {
                Some(id) => record.content.checkpoint_before_id == id,
                None => record.content.generation == 0 && record.content.checkpoint_started_here,
            }
        {
            return Ok(Some(load_durable_record(
                database,
                owner_account_id,
                owner_device_id,
                copy_id(record_id.value())?,
            )?));
        }
    }
    Ok(None)
}

fn find_record_for_checkpoint_before_id<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    key: [u8; 32],
    checkpoint_before_id: [u8; 32],
) -> Result<Option<DurableHistoryRecoveryChunk>, HistoryRecoveryExecutionError> {
    let read = database.begin_read().map_err(database_error)?;
    let records = read.open_table(RECORD_TABLE).map_err(database_error)?;
    for row in records.iter().map_err(database_error)? {
        let (record_id, encoded) = row.map_err(database_error)?;
        let record = SignedExecutionRecord::decode_and_verify(
            encoded.value(),
            owner_account_id,
            owner_device_id,
        )?;
        if record.content.selection_key == key
            && record.content.checkpoint_before_id == checkpoint_before_id
        {
            return Ok(Some(load_durable_record(
                database,
                owner_account_id,
                owner_device_id,
                copy_id(record_id.value())?,
            )?));
        }
    }
    Ok(None)
}

fn load_head<D: ReadableDatabase>(
    database: &D,
    key: [u8; 32],
) -> Result<Option<ExecutionHeadWire>, HistoryRecoveryExecutionError> {
    let read = database.begin_read().map_err(database_error)?;
    let heads = read.open_table(HEAD_TABLE).map_err(database_error)?;
    heads
        .get(key.as_slice())
        .map_err(database_error)?
        .map(|encoded| decode_head(encoded.value(), key))
        .transpose()
}

fn decode_head(
    encoded: &[u8],
    expected_key: [u8; 32],
) -> Result<ExecutionHeadWire, HistoryRecoveryExecutionError> {
    validate_head_length(encoded)?;
    let head: ExecutionHeadWire = postcard::from_bytes(encoded)?;
    if head.version != HEAD_VERSION
        || head.selection_key != expected_key
        || head.consent_record_id == [0_u8; 32]
        || head.plan_id == [0_u8; 32]
        || head.record_id == [0_u8; 32]
        || head.checkpoint_after_id == [0_u8; 32]
        || head.next_range_start > head.approved_range_end
    {
        return Err(HistoryRecoveryExecutionError::InvalidHead);
    }
    if postcard::to_allocvec(&head)? != encoded {
        return Err(HistoryRecoveryExecutionError::NonCanonicalHead);
    }
    Ok(head)
}

fn head_wire(record: &SignedExecutionRecord, record_id: [u8; 32]) -> ExecutionHeadWire {
    ExecutionHeadWire {
        version: HEAD_VERSION,
        selection_key: record.content.selection_key,
        consent_record_id: record.content.consent_record_id,
        plan_id: record.content.plan_id,
        selection_index: record.content.selection_index,
        record_id,
        generation: record.content.generation,
        checkpoint_after_id: record.content.checkpoint_after_id,
        next_range_start: record.content.chunk_range_end,
        approved_range_end: record.content.approved_range_end,
    }
}

fn verify_write_head(
    heads: &redb::Table<&[u8], &[u8]>,
    records: &redb::Table<&[u8], &[u8]>,
    key: [u8; 32],
    expected: Option<&ExecutionHeadWire>,
) -> Result<(), HistoryRecoveryExecutionError> {
    let actual = heads
        .get(key.as_slice())
        .map_err(database_error)?
        .map(|encoded| decode_head(encoded.value(), key))
        .transpose()?;
    if actual.as_ref() != expected {
        return Err(HistoryRecoveryExecutionError::ConcurrentHeadChange);
    }
    if let Some(head) = &actual {
        let encoded = records
            .get(head.record_id.as_slice())
            .map_err(database_error)?
            .ok_or(HistoryRecoveryExecutionError::MissingRecord)?;
        let record: SignedExecutionRecordWire = postcard::from_bytes(encoded.value())?;
        if record.content.selection_key != key {
            return Err(HistoryRecoveryExecutionError::HeadRecordMismatch);
        }
    }
    Ok(())
}

fn insert_exact_artifact(
    table: &mut redb::Table<&[u8], &[u8]>,
    key: &[u8; 32],
    bytes: &[u8],
) -> Result<(), HistoryRecoveryExecutionError> {
    if let Some(existing) = table.get(key.as_slice()).map_err(database_error)? {
        if existing.value() != bytes {
            return Err(HistoryRecoveryExecutionError::ArtifactIdCollision);
        }
        return Ok(());
    }
    table
        .insert(key.as_slice(), bytes)
        .map_err(database_error)?;
    Ok(())
}

fn verify_bundle_against_record(
    bundle: &HistoryRewrapBundle,
    content: &ExecutionRecordContent,
) -> Result<(), HistoryRecoveryExecutionError> {
    let manifest = bundle.manifest();
    if manifest.account_id() != content.owner_account_id
        || manifest.source_device_id() != content.source_device_id
        || manifest.recipient_device_id() != content.recipient_device_id
        || manifest.conversation_id() != content.conversation_id
        || manifest.inventory_event_count() != content.inventory_event_count
        || *manifest.inventory_digest() != content.inventory_digest
        || manifest.range_start() != content.chunk_range_start
        || manifest.range_end() != content.chunk_range_end
    {
        return Err(HistoryRecoveryExecutionError::BundleRecordMismatch);
    }
    Ok(())
}

fn verify_checkpoint_transition_against_record(
    before: &SignedHistoryRecoveryCheckpoint,
    after: &SignedHistoryRecoveryCheckpoint,
    content: &ExecutionRecordContent,
) -> Result<(), HistoryRecoveryExecutionError> {
    for checkpoint in [before, after] {
        if checkpoint.account_id() != content.owner_account_id
            || checkpoint.conversation_id() != content.conversation_id
            || checkpoint.source_device_id() != content.source_device_id
            || checkpoint.recipient_device_id() != content.recipient_device_id
            || checkpoint.approved_range_start() > content.approved_range_start
            || checkpoint.approved_range_end() < content.approved_range_end
        {
            return Err(HistoryRecoveryExecutionError::CheckpointRecordMismatch);
        }
    }
    if before.next_range_start() != content.chunk_range_start
        || after.next_range_start() != content.chunk_range_end
        || after.previous_checkpoint_id() != Some(&content.checkpoint_before_id)
        || after.inventory_event_count() != Some(content.inventory_event_count)
        || after.inventory_digest() != Some(&content.inventory_digest)
    {
        return Err(HistoryRecoveryExecutionError::CheckpointRecordMismatch);
    }
    Ok(())
}

fn verify_projections_against_bundle(
    projections: &[LocalTextProjection],
    bundle: &HistoryRewrapBundle,
    recipient_device_id: DeviceId,
) -> Result<(), HistoryRecoveryExecutionError> {
    if projections.len() != bundle.entries().len() {
        return Err(HistoryRecoveryExecutionError::InvalidProjectionArtifact);
    }
    for (projection, entry) in projections.iter().zip(bundle.entries()) {
        if projection.event_id() != entry.event().event().event_id()?
            || projection.local_device_id() != recipient_device_id
            || projection.history_rewrap_manifest() != Some(bundle.manifest())
        {
            return Err(HistoryRecoveryExecutionError::InvalidProjectionArtifact);
        }
    }
    Ok(())
}

fn encode_projection_artifact(
    projections: &[LocalTextProjection],
) -> Result<Vec<u8>, HistoryRecoveryExecutionError> {
    let mut entries = Vec::with_capacity(projections.len());
    for projection in projections {
        entries.push(projection.encode()?);
    }
    let encoded = postcard::to_allocvec(&ProjectionArtifactWire { entries })?;
    validate_projection_length(&encoded)?;
    Ok(encoded)
}

fn decode_projection_artifact(
    encoded: &[u8],
) -> Result<Vec<LocalTextProjection>, HistoryRecoveryExecutionError> {
    validate_projection_length(encoded)?;
    let artifact: ProjectionArtifactWire = postcard::from_bytes(encoded)?;
    if artifact.entries.is_empty()
        || artifact.entries.len() > MAX_HISTORY_RECOVERY_EXECUTION_CHUNK_EVENTS
        || postcard::to_allocvec(&artifact)? != encoded
    {
        return Err(HistoryRecoveryExecutionError::InvalidProjectionArtifact);
    }
    artifact
        .entries
        .iter()
        .map(|bytes| LocalTextProjection::decode(bytes).map_err(Into::into))
        .collect()
}

fn validate_record_content(
    content: &ExecutionRecordContent,
) -> Result<(), HistoryRecoveryExecutionError> {
    if content.version != RECORD_VERSION
        || content.source_device_id == content.recipient_device_id
        || content.consent_record_id == [0_u8; 32]
        || content.plan_id == [0_u8; 32]
        || content.transition_id == [0_u8; 32]
        || content.selection_key
            != selection_key(content.consent_record_id, content.selection_index)
        || content.inventory_event_count == 0
        || content.inventory_digest == [0_u8; 32]
        || content.approved_range_start >= content.approved_range_end
        || content.approved_range_end > content.inventory_event_count
        || content.chunk_range_start < content.approved_range_start
        || content.chunk_range_start >= content.chunk_range_end
        || content.chunk_range_end > content.approved_range_end
        || content
            .chunk_range_end
            .saturating_sub(content.chunk_range_start)
            > MAX_HISTORY_RECOVERY_EXECUTION_CHUNK_EVENTS as u64
        || content.bundle_id == [0_u8; 32]
        || content.bundle_digest == [0_u8; 32]
        || content.checkpoint_before_id == [0_u8; 32]
        || content.checkpoint_after_id == [0_u8; 32]
        || content.checkpoint_before_id == content.checkpoint_after_id
        || content.projection_count == 0
        || u64::from(content.projection_count)
            != content
                .chunk_range_end
                .saturating_sub(content.chunk_range_start)
        || content.projection_digest == [0_u8; 32]
        || (content.generation == 0) != content.previous_record_id.is_none()
        || (content.checkpoint_started_here && content.generation != 0)
    {
        return Err(HistoryRecoveryExecutionError::InvalidRecord);
    }
    Ok(())
}

fn record_signing_bytes(
    content: &ExecutionRecordContent,
) -> Result<Vec<u8>, HistoryRecoveryExecutionError> {
    validate_record_content(content)?;
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(RECORD_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(RECORD_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn validate_record_length(encoded: &[u8]) -> Result<(), HistoryRecoveryExecutionError> {
    if encoded.is_empty() || encoded.len() > MAX_HISTORY_RECOVERY_EXECUTION_RECORD_BYTES {
        return Err(HistoryRecoveryExecutionError::RecordLength(encoded.len()));
    }
    Ok(())
}

fn validate_head_length(encoded: &[u8]) -> Result<(), HistoryRecoveryExecutionError> {
    if encoded.is_empty() || encoded.len() > MAX_HISTORY_RECOVERY_EXECUTION_HEAD_BYTES {
        return Err(HistoryRecoveryExecutionError::HeadLength(encoded.len()));
    }
    Ok(())
}

fn validate_bundle_length(encoded: &[u8]) -> Result<(), HistoryRecoveryExecutionError> {
    if encoded.is_empty() || encoded.len() > MAX_HISTORY_RECOVERY_EXECUTION_BUNDLE_BYTES {
        return Err(HistoryRecoveryExecutionError::BundleLength(encoded.len()));
    }
    Ok(())
}

fn validate_checkpoint_length(encoded: &[u8]) -> Result<(), HistoryRecoveryExecutionError> {
    if encoded.is_empty() || encoded.len() > MAX_HISTORY_RECOVERY_EXECUTION_CHECKPOINT_BYTES {
        return Err(HistoryRecoveryExecutionError::CheckpointLength(
            encoded.len(),
        ));
    }
    Ok(())
}

fn validate_projection_length(encoded: &[u8]) -> Result<(), HistoryRecoveryExecutionError> {
    if encoded.is_empty() || encoded.len() > MAX_HISTORY_RECOVERY_EXECUTION_PROJECTION_BYTES {
        return Err(HistoryRecoveryExecutionError::ProjectionLength(
            encoded.len(),
        ));
    }
    Ok(())
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

fn copy_id(bytes: &[u8]) -> Result<[u8; 32], HistoryRecoveryExecutionError> {
    bytes
        .try_into()
        .map_err(|_| HistoryRecoveryExecutionError::StoredIdLength(bytes.len()))
}

fn database_error(error: impl std::fmt::Display) -> HistoryRecoveryExecutionError {
    HistoryRecoveryExecutionError::Database(error.to_string())
}

#[derive(Debug, Error)]
pub enum HistoryRecoveryExecutionError {
    #[error("old-history recovery execution store is not initialized")]
    StoreNotInitialized,
    #[error("old-history recovery execution store I/O failed")]
    Io(#[from] std::io::Error),
    #[error("old-history recovery execution database failed: {0}")]
    Database(String),
    #[error("old-history recovery execution schema does not match")]
    SchemaMismatch,
    #[error("old-history recovery execution owner Account does not match")]
    OwnerAccountMismatch,
    #[error("old-history recovery execution owner Device does not match")]
    OwnerDeviceMismatch,
    #[error("durable old-history consent is missing")]
    ConsentMissing,
    #[error("durable old-history consent record does not match")]
    ConsentRecordMismatch,
    #[error("durable old-history consent owner does not match")]
    ConsentOwnerMismatch,
    #[error("old-history recovery selection {0} is missing")]
    SelectionMissing(usize),
    #[error("old-history recovery signed plan context changed")]
    PlanContextMismatch,
    #[error("old-history source inventory does not match the consented claim")]
    SourceInventoryMismatch,
    #[error("old-history recovery checkpoint does not match the consented plan")]
    CheckpointPlanMismatch,
    #[error("old-history recovery checkpoint does not match the durable execution head")]
    CheckpointHeadMismatch,
    #[error("old-history recovery bundle does not match the consented plan")]
    BundlePlanMismatch,
    #[error("old-history recovery bundle does not match its signed execution record")]
    BundleRecordMismatch,
    #[error("old-history recovery checkpoint does not match its signed execution record")]
    CheckpointRecordMismatch,
    #[error("old-history recovery prepared chunk is invalid")]
    InvalidPreparedChunk,
    #[error("old-history recovery applied chunk is invalid")]
    InvalidAppliedChunk,
    #[error("old-history recovery recipient Device does not match")]
    WrongRecipientDevice,
    #[error("another output is already committed for this checkpoint")]
    DivergentCommittedChunk,
    #[error("old-history recovery execution head changed concurrently")]
    ConcurrentHeadChange,
    #[error("old-history recovery execution record is invalid")]
    InvalidRecord,
    #[error("old-history recovery execution head is invalid")]
    InvalidHead,
    #[error("old-history recovery execution record is not canonical")]
    NonCanonicalRecord,
    #[error("old-history recovery execution head is not canonical")]
    NonCanonicalHead,
    #[error("old-history recovery execution record has invalid length {0}")]
    RecordLength(usize),
    #[error("old-history recovery execution head has invalid length {0}")]
    HeadLength(usize),
    #[error("old-history recovery bundle has invalid length {0}")]
    BundleLength(usize),
    #[error("old-history recovery checkpoint has invalid length {0}")]
    CheckpointLength(usize),
    #[error("old-history recovery projection artifact has invalid length {0}")]
    ProjectionLength(usize),
    #[error("old-history recovery projection artifact is invalid")]
    InvalidProjectionArtifact,
    #[error("old-history recovery execution record references a missing bundle")]
    MissingBundle,
    #[error("old-history recovery execution record references a missing checkpoint")]
    MissingCheckpoint,
    #[error("old-history recovery execution record references missing projections")]
    MissingProjectionArtifact,
    #[error("old-history recovery execution head references a missing record")]
    MissingRecord,
    #[error("old-history recovery execution record ID does not bind its bytes")]
    RecordIdBinding,
    #[error("old-history recovery bundle ID does not bind its bytes")]
    BundleIdBinding,
    #[error("old-history recovery checkpoint ID does not bind its bytes")]
    CheckpointIdBinding,
    #[error("old-history recovery bundle digest does not match")]
    BundleDigestMismatch,
    #[error("old-history recovery projection digest does not match")]
    ProjectionDigestMismatch,
    #[error("old-history recovery execution head does not match its record")]
    HeadRecordMismatch,
    #[error("old-history recovery execution record chain is broken")]
    BrokenRecordChain,
    #[error("old-history recovery execution record chain is forked")]
    ForkedRecordChain,
    #[error("old-history recovery execution record is orphaned")]
    OrphanRecord,
    #[error("old-history recovery execution artifact is orphaned")]
    OrphanArtifact,
    #[error("multiple old-history recovery heads reach one record")]
    DuplicateHeadChain,
    #[error("distinct old-history recovery records produced the same ID")]
    RecordIdCollision,
    #[error("distinct old-history recovery artifacts produced the same ID")]
    ArtifactIdCollision,
    #[error("old-history recovery execution capacity is exhausted")]
    ExecutionCapacity,
    #[error("stored old-history recovery execution ID has invalid length {0}")]
    StoredIdLength(usize),
    #[error("committed old-history recovery record differs from the requested chunk")]
    CommittedRecordMismatch,
    #[error("old-history recovery execution record is not the current committed head")]
    CommittedHeadMismatch,
    #[error("test-only failure injected before the atomic execution commit")]
    InjectedBeforeCommit,
    #[error("old-history recovery execution encoding is invalid: {0}")]
    Codec(#[from] postcard::Error),
    #[error(transparent)]
    Identity(#[from] kilogram_identity::IdentityError),
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error(transparent)]
    Recovery(#[from] PeerHistoryRecoveryError),
    #[error(transparent)]
    ConsentStore(#[from] OldHistoryRecoveryConsentStoreError),
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use kilogram_identity::{AccountRootState, DeviceCapability, DeviceState};
    use kilogram_peer_capability_store::{PeerCapabilityKey, PeerCapabilityStore};
    use kilogram_peer_history_recovery::{
        ConversationHistoryCoverage, ExplicitOldHistoryRecoveryConsent, OldHistoryRecoveryPlan,
        ReadableHistoryRange, SignedHistorySourceClaim,
    };
    use kilogram_peer_history_recovery_consent::{
        OldHistoryRecoveryConsentOutcome, OldHistoryRecoveryConsentStore,
    };
    use kilogram_peer_reenrollment::{ExplicitPeerReenrollmentApproval, PeerReenrollmentPlan};
    use kilogram_peer_reenrollment_acceptance::{
        DurablePeerReenrollmentAcceptance, PeerReenrollmentAcceptanceOutcome,
        PeerReenrollmentAcceptanceStore,
    };
    use kilogram_peer_reenrollment_runtime::{
        PeerReenrollmentRuntimeBinding, PeerReenrollmentRuntimeConsumeOutcome,
        PeerReenrollmentRuntimePrepareOutcome, PeerReenrollmentRuntimeStore,
    };
    use kilogram_protocol::{ConversationId, EventId, RatchetRecipient, SignedEvent};
    use kilogram_ratchet::RatchetState;
    use kilogram_sync_capability::{
        CurrentPeerInvitationBinding, PeerSyncCapabilityEvidence, SignedCurrentPeerInvitation,
        SyncTransportCapabilityError,
    };
    use tempfile::TempDir;

    struct Fixture {
        root: TempDir,
        owner_account_id: AccountId,
        source: DeviceState,
        replacement: DeviceState,
        authority: AccountDeviceListSnapshot,
        membership: ConversationMembershipSnapshot,
        binding: PeerReenrollmentHistoryBinding,
        inventory: OldHistoryRecoveryInventory,
        selections: Vec<HistoryRecoveryRangeSelection>,
        signed_plan: SignedOldHistoryRecoveryPlan,
        source_inventory: Vec<(AuthorizedEvent, String)>,
        initial_checkpoint: Option<SignedHistoryRecoveryCheckpoint>,
    }

    impl Fixture {
        fn context(&self) -> OldHistoryRecoveryLiveContext<'_> {
            OldHistoryRecoveryLiveContext {
                binding: &self.binding,
                authority: &self.authority,
                memberships: std::slice::from_ref(&self.membership),
                inventory: &self.inventory,
                selections: &self.selections,
            }
        }

        fn open_consent(
            &self,
        ) -> Result<OldHistoryRecoveryConsentStore, OldHistoryRecoveryConsentStoreError> {
            OldHistoryRecoveryConsentStore::open(
                self.root.path().join("consent"),
                self.owner_account_id,
                self.source.identity().device_id(),
            )
        }

        fn commit_consent(
            &self,
            store: &OldHistoryRecoveryConsentStore,
        ) -> Result<[u8; 32], OldHistoryRecoveryConsentStoreError> {
            let outcome = store.commit(
                self.source.identity(),
                &self.signed_plan,
                &self.binding,
                &self.authority,
                std::slice::from_ref(&self.membership),
                &self.inventory,
                &self.selections,
                1_500,
            )?;
            Ok(match outcome {
                OldHistoryRecoveryConsentOutcome::Committed(value)
                | OldHistoryRecoveryConsentOutcome::AlreadyCommitted(value) => value.record_id(),
            })
        }

        fn open_execution(
            &self,
            name: &str,
        ) -> Result<OldHistoryRecoveryExecutionStore, HistoryRecoveryExecutionError> {
            OldHistoryRecoveryExecutionStore::open(
                self.root.path().join(name),
                self.owner_account_id,
                self.source.identity().device_id(),
            )
        }
    }

    fn fixture(
        event_count: usize,
        checkpoint_page_size: Option<usize>,
    ) -> Result<Fixture, Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let local_root = AccountRootState::create(root.path().join("local-root"))?;
        let local_device = DeviceState::load_or_create(root.path().join("local-device"))?;
        let local_certificate = local_root.issue_device_certificate(
            local_device.identity().device_id(),
            local_device.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let local_authority = local_root.publish_device_list(&[local_certificate])?;

        let history_root = AccountRootState::create(root.path().join("history-root"))?;
        let source = DeviceState::load_or_create(root.path().join("history-source"))?;
        let source_certificate = history_root.issue_device_certificate(
            source.identity().device_id(),
            source.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let old_authority =
            history_root.publish_device_list(std::slice::from_ref(&source_certificate))?;
        let old_invitation = invitation(
            source.identity(),
            history_root.account_id(),
            local_root.account_id(),
            old_authority.revision(),
            7,
            [7_u8; 32],
        )?;
        let capability = PeerCapabilityStore::open(
            root.path().join("peer-capability"),
            local_device.identity().device_id(),
        )?;
        capability.apply_current_invitation(
            local_device.identity(),
            local_root.account_id(),
            &old_invitation,
            1_100,
        )?;
        let old_key =
            PeerCapabilityKey::new(history_root.account_id(), source.identity().device_id());
        capability.apply_authenticated_observation(
            local_device.identity(),
            old_key,
            PeerSyncCapabilityEvidence::LegacyTicketWithoutExtension,
        )?;
        let quarantine = capability.load_quarantined_head(old_key)?;

        let replacement = DeviceState::load_or_create(root.path().join("replacement"))?;
        let replacement_certificate = history_root.issue_device_certificate(
            replacement.identity().device_id(),
            replacement.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let authority =
            history_root.publish_device_list(&[source_certificate, replacement_certificate])?;
        let current_invitation = invitation(
            replacement.identity(),
            history_root.account_id(),
            local_root.account_id(),
            authority.revision(),
            1,
            [9_u8; 32],
        )?;
        let acceptance = accept_reenrollment(
            root.path(),
            local_device.identity(),
            &local_authority,
            &quarantine,
            &authority,
            &current_invitation,
        )?;
        let runtime_store = PeerReenrollmentRuntimeStore::open(
            root.path().join("runtime-transition"),
            local_root.account_id(),
            local_device.identity().device_id(),
        )?;
        let prepared = runtime_store.prepare(
            local_device.identity(),
            &acceptance,
            &current_invitation,
            PeerReenrollmentRuntimeBinding {
                contact_id: [11_u8; 32],
                conversation_id: [12_u8; 32],
                predecessor_contact: vec![1, 2, 3],
                successor_contact: vec![4, 5, 6],
                successor_endpoint_candidate: vec![7, 8, 9],
            },
            1_350,
        )?;
        let prepared = match prepared {
            PeerReenrollmentRuntimePrepareOutcome::Prepared(value)
            | PeerReenrollmentRuntimePrepareOutcome::AlreadyPrepared(value) => value,
        };
        let consumed =
            runtime_store.consume(prepared.transition().contact_id(), prepared.transition_id())?;
        let consumed = match consumed {
            PeerReenrollmentRuntimeConsumeOutcome::Consumed(value)
            | PeerReenrollmentRuntimeConsumeOutcome::AlreadyConsumed(value) => value,
        };
        let binding = PeerReenrollmentHistoryBinding::from_consumed_transition(
            acceptance.receipt(),
            &consumed,
            &authority,
        )?;

        let conversation_id = ConversationId::from_label("bounded-history-execution");
        let mut source_inventory =
            make_source_inventory(root.path(), conversation_id, event_count)?;
        source_inventory.sort_by_key(|(event, _)| event.event().event_id().ok());
        let history_author_account_id = source_inventory
            .first()
            .ok_or("history execution fixture requires at least one event")?
            .0
            .author_account_id();
        let membership = history_root.create_conversation_membership(
            conversation_id.scope_id(),
            &[local_root.account_id(), history_author_account_id],
        )?;
        let probe = HistoryRewrapBundle::seal(
            source.identity(),
            authority.clone(),
            replacement.identity().device_id(),
            conversation_id,
            &source_inventory,
            0,
            event_count,
        )?;
        let coverage = ConversationHistoryCoverage::new(
            conversation_id,
            u64::try_from(event_count)?,
            *probe.manifest().inventory_digest(),
            vec![ReadableHistoryRange::new(0, u64::try_from(event_count)?)?],
        )?;
        let source_claim =
            SignedHistorySourceClaim::issue(source.identity(), &authority, vec![coverage])?;
        let initial_checkpoint = checkpoint_page_size
            .map(|page_size| {
                SignedHistoryRecoveryCheckpoint::start(
                    replacement.identity(),
                    history_root.account_id(),
                    conversation_id,
                    source.identity().device_id(),
                    HistoryRewrapSas::derive(
                        &authority,
                        source.identity().device_id(),
                        replacement.identity().device_id(),
                    )?,
                    0,
                    event_count,
                    page_size,
                )
            })
            .transpose()?;
        let checkpoints = initial_checkpoint.iter().cloned().collect::<Vec<_>>();
        let inventory = OldHistoryRecoveryInventory::assemble(
            &authority,
            replacement.identity().device_id(),
            vec![source_claim],
            &checkpoints,
        )?;
        let selections = vec![HistoryRecoveryRangeSelection {
            conversation_id,
            range_start: 0,
            range_end: u64::try_from(event_count)?,
        }];
        let plan = OldHistoryRecoveryPlan::prepare(
            &binding,
            &authority,
            std::slice::from_ref(&membership),
            &inventory,
            source.identity().device_id(),
            &selections,
            1_400,
        )?;
        let phrase = plan.confirmation_phrase()?;
        let signed_plan = plan.confirm_and_sign(
            source.identity(),
            &phrase,
            ExplicitOldHistoryRecoveryConsent::HistoryOwnerApproved,
        )?;
        Ok(Fixture {
            root,
            owner_account_id: history_root.account_id(),
            source,
            replacement,
            authority,
            membership,
            binding,
            inventory,
            selections,
            signed_plan,
            source_inventory,
            initial_checkpoint,
        })
    }

    fn make_source_inventory(
        root: &Path,
        conversation_id: ConversationId,
        event_count: usize,
    ) -> Result<Vec<(AuthorizedEvent, String)>, Box<dyn std::error::Error>> {
        let author_root = AccountRootState::create(root.join("author-root"))?;
        let author = DeviceState::load_or_create(root.join("author-device"))?;
        let author_certificate = author_root.issue_device_certificate(
            author.identity().device_id(),
            author.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let author_snapshot = author_root.authority_snapshot()?;
        let mut inventory = Vec::with_capacity(event_count);
        for index in 0..event_count {
            let body = format!("recoverable-body-{index:03}");
            let event = sign_test_text(
                root,
                author.identity(),
                conversation_id,
                u64::try_from(index)?,
                Vec::new(),
                &body,
            )?;
            inventory.push((
                AuthorizedEvent::new(event, author_certificate.clone(), author_snapshot.clone())?,
                body,
            ));
        }
        Ok(inventory)
    }

    fn sign_test_text(
        root: &Path,
        identity: &DeviceIdentity,
        conversation_id: ConversationId,
        author_sequence: u64,
        parents: Vec<EventId>,
        body: &str,
    ) -> Result<SignedEvent, Box<dyn std::error::Error>> {
        let sender_directory = root.join(format!("sender-ratchet-{author_sequence}"));
        let peer_directory = root.join(format!("peer-ratchet-{author_sequence}"));
        let peer_identity = DeviceIdentity::generate()?;
        let peer_encryption = DeviceEncryptionIdentity::generate()?;
        let peer_root =
            AccountRootState::create(root.join(format!("peer-root-{author_sequence}")))?;
        let peer_certificate = peer_root.issue_device_certificate(
            peer_identity.device_id(),
            peer_encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let peer_device_list =
            peer_root.publish_device_list(std::slice::from_ref(&peer_certificate))?;
        let peer_bundle =
            RatchetState::load_or_create(&peer_directory)?.prekey_bundle(&peer_identity)?;
        let (sender_ratchet_identity, ciphertext, _) = RatchetState::load_or_create(
            &sender_directory,
        )?
        .encrypt(identity, &peer_bundle, body)?;
        Ok(SignedEvent::sign_ratchet_text(
            identity,
            conversation_id,
            author_sequence,
            parents,
            peer_device_list,
            sender_ratchet_identity,
            vec![RatchetRecipient::new(
                peer_identity.device_id(),
                ciphertext,
            )?],
        )?)
    }

    fn invitation(
        listener: &DeviceIdentity,
        listener_account_id: AccountId,
        requester_account_id: AccountId,
        authority_revision: u64,
        epoch: u64,
        capability_id: [u8; 32],
    ) -> Result<SignedCurrentPeerInvitation, SyncTransportCapabilityError> {
        SignedCurrentPeerInvitation::issue(
            listener,
            &CurrentPeerInvitationBinding {
                listener_account_id,
                listener_device_id: listener.device_id(),
                allowed_requester_account_id: requester_account_id,
                authority_revision,
                endpoint_id: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                ticket_publication_channel_epoch: epoch,
                issued_at_unix_seconds: 1_000,
                expires_at_unix_seconds: 2_000,
                connection_ticket: "Exact_current_ticket_01",
                required_capability_id: capability_id,
            },
        )
    }

    fn accept_reenrollment(
        root: &Path,
        local_identity: &DeviceIdentity,
        local_authority: &AccountDeviceListSnapshot,
        quarantine: &kilogram_peer_capability_store::DurablePeerCapabilityQuarantineHead,
        peer_authority: &AccountDeviceListSnapshot,
        invitation: &SignedCurrentPeerInvitation,
    ) -> Result<DurablePeerReenrollmentAcceptance, Box<dyn std::error::Error>> {
        let plan = PeerReenrollmentPlan::prepare(
            local_identity.device_id(),
            local_authority,
            quarantine,
            peer_authority,
            invitation,
            1_200,
        )?;
        let phrase = plan.confirmation_phrase()?;
        let tombstone = plan.confirm_and_sign(
            local_identity,
            &phrase,
            ExplicitPeerReenrollmentApproval::UserOrTestOperatorApproved,
        )?;
        let store = PeerReenrollmentAcceptanceStore::open(
            root.join("peer-reenrollment-acceptance"),
            local_authority.account_id(),
            local_identity.device_id(),
        )?;
        Ok(
            match store.accept(
                local_identity,
                &tombstone,
                local_authority,
                quarantine,
                peer_authority,
                invitation,
                1_300,
            )? {
                PeerReenrollmentAcceptanceOutcome::Accepted(value)
                | PeerReenrollmentAcceptanceOutcome::AlreadyAccepted(value) => value,
            },
        )
    }

    fn prepare(
        store: &OldHistoryRecoveryExecutionStore,
        consent: &OldHistoryRecoveryConsentStore,
        consent_record_id: [u8; 32],
        fixture: &Fixture,
        checkpoint: Option<&SignedHistoryRecoveryCheckpoint>,
    ) -> Result<PreparedHistoryRecoveryChunk, HistoryRecoveryExecutionError> {
        match store.prepare_next_chunk(
            consent,
            fixture.signed_plan.transition_id(),
            consent_record_id,
            fixture.source.identity(),
            &fixture.context(),
            0,
            checkpoint,
            &fixture.source_inventory,
        )? {
            PrepareHistoryRecoveryChunkOutcome::Prepared(value) => Ok(*value),
            _ => Err(HistoryRecoveryExecutionError::InvalidPreparedChunk),
        }
    }

    fn apply(
        fixture: &Fixture,
        prepared: PreparedHistoryRecoveryChunk,
    ) -> Result<AppliedHistoryRecoveryChunk, HistoryRecoveryExecutionError> {
        OldHistoryRecoveryExecutionStore::apply_prepared_chunk(
            prepared,
            fixture.replacement.identity(),
            fixture.replacement.encryption(),
        )
    }

    #[test]
    fn atomic_chunk_commit_reopen_and_privacy_safe_status() -> Result<(), Box<dyn std::error::Error>>
    {
        let fixture = fixture(5, None)?;
        let consent = fixture.open_consent()?;
        let consent_record_id = fixture.commit_consent(&consent)?;
        let directory = fixture.root.path().join("execution");
        assert!(matches!(
            OldHistoryRecoveryExecutionReadStore::open_existing(
                &directory,
                fixture.owner_account_id,
                fixture.source.identity().device_id(),
            ),
            Err(HistoryRecoveryExecutionError::StoreNotInitialized)
        ));
        let store = fixture.open_execution("execution")?;
        let applied = apply(
            &fixture,
            prepare(&store, &consent, consent_record_id, &fixture, None)?,
        )?;
        let CommitHistoryRecoveryChunkOutcome::Committed(committed) = store.commit_chunk(
            &consent,
            consent_record_id,
            fixture.source.identity(),
            &fixture.context(),
            &applied,
            9_000,
        )?
        else {
            return Err(HistoryRecoveryExecutionError::CommittedRecordMismatch.into());
        };
        assert!(committed.is_complete());
        assert_eq!(committed.projections().len(), 5);
        for ((authorized, body), projection) in
            fixture.source_inventory.iter().zip(committed.projections())
        {
            assert_eq!(
                projection.open_for_account(
                    authorized.event(),
                    fixture.replacement.identity().device_id(),
                    fixture.owner_account_id,
                    fixture.replacement.encryption(),
                )?,
                *body
            );
        }
        drop(store);
        let read = OldHistoryRecoveryExecutionReadStore::open_existing(
            &directory,
            fixture.owner_account_id,
            fixture.source.identity().device_id(),
        )?;
        let HistoryRecoveryExecutionStatus::Complete(view) = read.status(consent_record_id, 0)?
        else {
            return Err(HistoryRecoveryExecutionError::InvalidHead.into());
        };
        assert_eq!(view.committed_chunk_count, 1);
        assert_eq!(view.next_range_start, 5);
        assert!(!view.contains_message_bodies);
        assert!(!view.contains_plaintext);
        assert!(!view.grants_network_authority);
        assert!(!view.grants_store_import_authority);
        Ok(())
    }

    #[test]
    fn bounded_multi_chunk_resume_reaches_exact_completion()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture(5, Some(2))?;
        let consent = fixture.open_consent()?;
        let consent_record_id = fixture.commit_consent(&consent)?;
        let store = fixture.open_execution("resume")?;
        let mut checkpoint = fixture.initial_checkpoint.clone();
        let mut committed = Vec::new();
        for committed_at in 2_000..2_003 {
            let prepared = prepare(
                &store,
                &consent,
                consent_record_id,
                &fixture,
                checkpoint.as_ref(),
            )?;
            assert!(prepared.range_end().saturating_sub(prepared.range_start()) <= 2);
            let applied = apply(&fixture, prepared)?;
            checkpoint = Some(applied.checkpoint_after().clone());
            let durable = match store.commit_chunk(
                &consent,
                consent_record_id,
                fixture.source.identity(),
                &fixture.context(),
                &applied,
                committed_at,
            )? {
                CommitHistoryRecoveryChunkOutcome::Committed(value) => value,
                CommitHistoryRecoveryChunkOutcome::AlreadyCommitted(_) => {
                    return Err(HistoryRecoveryExecutionError::DivergentCommittedChunk.into());
                }
            };
            committed.push(durable);
        }
        assert_eq!(committed.len(), 3);
        assert_eq!(committed[0].generation(), 0);
        assert_eq!(committed[2].generation(), 2);
        assert!(committed[2].is_complete());
        assert!(matches!(
            store.prepare_next_chunk(
                &consent,
                fixture.signed_plan.transition_id(),
                consent_record_id,
                fixture.source.identity(),
                &fixture.context(),
                0,
                checkpoint.as_ref(),
                &fixture.source_inventory,
            )?,
            PrepareHistoryRecoveryChunkOutcome::Complete(_)
        ));
        Ok(())
    }

    #[test]
    fn exact_replay_is_idempotent_and_divergent_output_fails_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture(4, None)?;
        let consent = fixture.open_consent()?;
        let consent_record_id = fixture.commit_consent(&consent)?;
        let store = fixture.open_execution("replay")?;
        let first = apply(
            &fixture,
            prepare(&store, &consent, consent_record_id, &fixture, None)?,
        )?;
        let divergent = apply(
            &fixture,
            prepare(&store, &consent, consent_record_id, &fixture, None)?,
        )?;
        let committed = match store.commit_chunk(
            &consent,
            consent_record_id,
            fixture.source.identity(),
            &fixture.context(),
            &first,
            2_000,
        )? {
            CommitHistoryRecoveryChunkOutcome::Committed(value) => value,
            CommitHistoryRecoveryChunkOutcome::AlreadyCommitted(_) => {
                return Err(HistoryRecoveryExecutionError::CommittedRecordMismatch.into());
            }
        };
        assert!(matches!(
            store.commit_chunk(
                &consent,
                consent_record_id,
                fixture.source.identity(),
                &fixture.context(),
                &first,
                2_001,
            )?,
            CommitHistoryRecoveryChunkOutcome::AlreadyCommitted(ref value)
                if value.record_id() == committed.record_id()
        ));
        assert!(matches!(
            store.commit_chunk(
                &consent,
                consent_record_id,
                fixture.source.identity(),
                &fixture.context(),
                &divergent,
                2_001,
            ),
            Err(HistoryRecoveryExecutionError::DivergentCommittedChunk)
        ));
        assert!(matches!(
            store.prepare_next_chunk(
                &consent,
                fixture.signed_plan.transition_id(),
                consent_record_id,
                fixture.source.identity(),
                &fixture.context(),
                0,
                None,
                &fixture.source_inventory,
            )?,
            PrepareHistoryRecoveryChunkOutcome::AlreadyCommitted(ref value)
                if value.record_id() == committed.record_id()
        ));
        Ok(())
    }

    #[test]
    fn interrupted_atomic_commit_leaves_no_partial_state() -> Result<(), Box<dyn std::error::Error>>
    {
        let fixture = fixture(3, None)?;
        let consent = fixture.open_consent()?;
        let consent_record_id = fixture.commit_consent(&consent)?;
        let store = fixture.open_execution("crash")?;
        let applied = apply(
            &fixture,
            prepare(&store, &consent, consent_record_id, &fixture, None)?,
        )?;
        assert!(matches!(
            store.commit_chunk_inner(
                &consent,
                consent_record_id,
                fixture.source.identity(),
                &fixture.context(),
                &applied,
                2_000,
                true,
            ),
            Err(HistoryRecoveryExecutionError::InjectedBeforeCommit)
        ));
        assert_eq!(
            store.status(consent_record_id, 0)?,
            HistoryRecoveryExecutionStatus::Absent
        );
        assert!(matches!(
            store.commit_chunk(
                &consent,
                consent_record_id,
                fixture.source.identity(),
                &fixture.context(),
                &applied,
                2_001,
            )?,
            CommitHistoryRecoveryChunkOutcome::Committed(_)
        ));
        Ok(())
    }

    #[test]
    fn foreign_identity_recipient_and_context_drift_fail_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture(3, None)?;
        let consent = fixture.open_consent()?;
        let consent_record_id = fixture.commit_consent(&consent)?;
        let store = fixture.open_execution("context")?;
        let foreign = DeviceIdentity::generate()?;
        assert!(matches!(
            store.prepare_next_chunk(
                &consent,
                fixture.signed_plan.transition_id(),
                consent_record_id,
                &foreign,
                &fixture.context(),
                0,
                None,
                &fixture.source_inventory,
            ),
            Err(HistoryRecoveryExecutionError::OwnerDeviceMismatch)
        ));
        let prepared = prepare(&store, &consent, consent_record_id, &fixture, None)?;
        let wrong_recipient =
            DeviceState::load_or_create(fixture.root.path().join("wrong-recipient"))?;
        assert!(matches!(
            OldHistoryRecoveryExecutionStore::apply_prepared_chunk(
                prepared.clone(),
                wrong_recipient.identity(),
                wrong_recipient.encryption(),
            ),
            Err(HistoryRecoveryExecutionError::WrongRecipientDevice)
        ));
        let drifted_selections = [HistoryRecoveryRangeSelection {
            conversation_id: fixture.selections[0].conversation_id,
            range_start: 0,
            range_end: 2,
        }];
        let drifted_context = OldHistoryRecoveryLiveContext {
            binding: &fixture.binding,
            authority: &fixture.authority,
            memberships: std::slice::from_ref(&fixture.membership),
            inventory: &fixture.inventory,
            selections: &drifted_selections,
        };
        assert!(matches!(
            store.prepare_next_chunk(
                &consent,
                fixture.signed_plan.transition_id(),
                consent_record_id,
                fixture.source.identity(),
                &drifted_context,
                0,
                None,
                &fixture.source_inventory,
            ),
            Err(HistoryRecoveryExecutionError::Recovery(
                PeerHistoryRecoveryError::PlanContextMismatch
            ))
        ));
        Ok(())
    }

    #[test]
    fn corruption_and_orphaned_record_fail_closed_on_reopen()
    -> Result<(), Box<dyn std::error::Error>> {
        let first_fixture = fixture(3, None)?;
        let consent = first_fixture.open_consent()?;
        let consent_record_id = first_fixture.commit_consent(&consent)?;
        let directory = first_fixture.root.path().join("corrupt");
        let store = first_fixture.open_execution("corrupt")?;
        let applied = apply(
            &first_fixture,
            prepare(&store, &consent, consent_record_id, &first_fixture, None)?,
        )?;
        let committed = match store.commit_chunk(
            &consent,
            consent_record_id,
            first_fixture.source.identity(),
            &first_fixture.context(),
            &applied,
            2_000,
        )? {
            CommitHistoryRecoveryChunkOutcome::Committed(value) => value,
            CommitHistoryRecoveryChunkOutcome::AlreadyCommitted(_) => {
                return Err(HistoryRecoveryExecutionError::CommittedRecordMismatch.into());
            }
        };
        {
            let mut write = store.database.begin_write().map_err(database_error)?;
            write
                .set_durability(Durability::Immediate)
                .map_err(database_error)?;
            {
                let mut projections = write.open_table(PROJECTION_TABLE).map_err(database_error)?;
                let mut encoded = projections
                    .get(committed.record_id.as_slice())
                    .map_err(database_error)?
                    .ok_or(HistoryRecoveryExecutionError::MissingProjectionArtifact)?
                    .value()
                    .to_vec();
                let last = encoded
                    .last_mut()
                    .ok_or(HistoryRecoveryExecutionError::InvalidProjectionArtifact)?;
                *last ^= 1;
                projections
                    .insert(committed.record_id.as_slice(), encoded.as_slice())
                    .map_err(database_error)?;
            }
            write.commit().map_err(database_error)?;
        }
        drop(store);
        assert!(
            OldHistoryRecoveryExecutionReadStore::open_existing(
                &directory,
                first_fixture.owner_account_id,
                first_fixture.source.identity().device_id(),
            )
            .is_err()
        );

        let second = fixture(2, None)?;
        let consent = second.open_consent()?;
        let consent_record_id = second.commit_consent(&consent)?;
        let directory = second.root.path().join("orphan");
        let store = second.open_execution("orphan")?;
        let applied = apply(
            &second,
            prepare(&store, &consent, consent_record_id, &second, None)?,
        )?;
        store.commit_chunk(
            &consent,
            consent_record_id,
            second.source.identity(),
            &second.context(),
            &applied,
            2_000,
        )?;
        {
            let mut write = store.database.begin_write().map_err(database_error)?;
            write
                .set_durability(Durability::Immediate)
                .map_err(database_error)?;
            {
                let mut heads = write.open_table(HEAD_TABLE).map_err(database_error)?;
                heads
                    .remove(selection_key(consent_record_id, 0).as_slice())
                    .map_err(database_error)?;
            }
            write.commit().map_err(database_error)?;
        }
        drop(store);
        assert!(matches!(
            OldHistoryRecoveryExecutionReadStore::open_existing(
                &directory,
                second.owner_account_id,
                second.source.identity().device_id(),
            ),
            Err(HistoryRecoveryExecutionError::OrphanRecord)
        ));

        let orphan_artifact_directory = second.root.path().join("orphan-artifact");
        let orphan_artifact_store = second.open_execution("orphan-artifact")?;
        {
            let mut write = orphan_artifact_store
                .database
                .begin_write()
                .map_err(database_error)?;
            write
                .set_durability(Durability::Immediate)
                .map_err(database_error)?;
            {
                let mut bundles = write.open_table(BUNDLE_TABLE).map_err(database_error)?;
                bundles
                    .insert([55_u8; 32].as_slice(), [1_u8, 2, 3].as_slice())
                    .map_err(database_error)?;
            }
            write.commit().map_err(database_error)?;
        }
        drop(orphan_artifact_store);
        assert!(matches!(
            OldHistoryRecoveryExecutionReadStore::open_existing(
                &orphan_artifact_directory,
                second.owner_account_id,
                second.source.identity().device_id(),
            ),
            Err(HistoryRecoveryExecutionError::OrphanArtifact)
        ));
        Ok(())
    }

    fn commit_next_for_import(
        fixture: &Fixture,
        store: &OldHistoryRecoveryExecutionStore,
        consent: &OldHistoryRecoveryConsentStore,
        consent_record_id: [u8; 32],
        checkpoint: Option<&SignedHistoryRecoveryCheckpoint>,
        committed_at_unix_seconds: u64,
    ) -> Result<DurableHistoryRecoveryChunk, HistoryRecoveryExecutionError> {
        let applied = apply(
            fixture,
            prepare(store, consent, consent_record_id, fixture, checkpoint)?,
        )?;
        match store.commit_chunk(
            consent,
            consent_record_id,
            fixture.source.identity(),
            &fixture.context(),
            &applied,
            committed_at_unix_seconds,
        )? {
            CommitHistoryRecoveryChunkOutcome::Committed(chunk) => Ok(chunk),
            CommitHistoryRecoveryChunkOutcome::AlreadyCommitted(_) => {
                Err(HistoryRecoveryExecutionError::CommittedRecordMismatch)
            }
        }
    }

    #[test]
    fn recipient_import_atomically_publishes_events_projections_checkpoint_and_vault()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture(5, None)?;
        let consent = fixture.open_consent()?;
        let consent_record_id = fixture.commit_consent(&consent)?;
        let execution_directory = fixture.root.path().join("recipient-import-execution");
        let execution = fixture.open_execution("recipient-import-execution")?;
        let committed = commit_next_for_import(
            &fixture,
            &execution,
            &consent,
            consent_record_id,
            None,
            2_000,
        )?;
        drop(execution);
        let execution = OldHistoryRecoveryExecutionReadStore::open_existing(
            execution_directory,
            fixture.owner_account_id,
            fixture.source.identity().device_id(),
        )?;

        let state = fixture.root.path().join("recipient-state");
        std::fs::create_dir_all(&state)?;
        kilogram_state::EncryptedStateVault::open_or_create(&state)?.migrate_legacy_snapshot()?;
        let imports = RecipientHistoryImportStore::new(
            &state,
            fixture.owner_account_id,
            fixture.replacement.identity().device_id(),
        );
        assert_eq!(
            imports.status(consent_record_id, 0)?,
            RecipientHistoryImportStatus::Absent
        );
        let imported = match imports.import_committed_chunk(
            &execution,
            consent_record_id,
            0,
            committed.record_id(),
            fixture.replacement.identity(),
            &fixture.membership,
            3_000,
        )? {
            RecipientHistoryImportOutcome::Imported(receipt) => receipt,
            RecipientHistoryImportOutcome::AlreadyImported(_) => {
                return Err("fresh import unexpectedly replayed".into());
            }
        };
        assert_eq!(imported.imported_event_count(), 5);
        assert!(imported.is_complete());

        let stored = kilogram_store::EventStore::open(state.join("events"))?
            .load_authorized_conversation(committed.conversation_id(), &fixture.membership)?;
        let expected = fixture
            .source_inventory
            .iter()
            .map(|(event, _)| event.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            stored
                .into_iter()
                .map(|event| event.event)
                .collect::<Vec<_>>(),
            expected
        );
        let projections = kilogram_store::LocalMessageStore::open(state.join("local-messages"))?;
        for ((event, body), projection) in
            fixture.source_inventory.iter().zip(committed.projections())
        {
            let stored = projections.get(projection.event_id())?;
            assert_eq!(stored, *projection);
            assert_eq!(
                stored.open_for_account(
                    event.event(),
                    fixture.replacement.identity().device_id(),
                    fixture.owner_account_id,
                    fixture.replacement.encryption(),
                )?,
                *body
            );
        }
        let RecipientHistoryImportStatus::Complete(status) =
            imports.status(consent_record_id, 0)?
        else {
            return Err("completed recipient import did not report complete".into());
        };
        assert_eq!(status.imported_chunk_count, 1);
        assert_eq!(status.imported_event_count, 5);
        assert!(!status.contains_message_bodies);
        assert!(!status.contains_plaintext);
        assert!(!status.grants_network_authority);
        assert!(!status.grants_execution_authority);
        assert!(!status.whole_database_rollback_supported);
        kilogram_state::EncryptedStateVault::open_existing(&state)?.verify()?;

        let replay = match imports.import_committed_chunk(
            &execution,
            consent_record_id,
            0,
            committed.record_id(),
            fixture.replacement.identity(),
            &fixture.membership,
            9_999,
        )? {
            RecipientHistoryImportOutcome::AlreadyImported(receipt) => receipt,
            RecipientHistoryImportOutcome::Imported(_) => {
                return Err("exact replay created another import".into());
            }
        };
        assert_eq!(replay.receipt_id(), imported.receipt_id());
        Ok(())
    }

    #[test]
    fn recipient_import_rollback_and_crash_recovery_leave_no_partial_page()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture(4, None)?;
        let consent = fixture.open_consent()?;
        let consent_record_id = fixture.commit_consent(&consent)?;
        let execution_directory = fixture.root.path().join("import-fault-execution");
        let execution = fixture.open_execution("import-fault-execution")?;
        let committed = commit_next_for_import(
            &fixture,
            &execution,
            &consent,
            consent_record_id,
            None,
            2_000,
        )?;
        drop(execution);
        let execution = OldHistoryRecoveryExecutionReadStore::open_existing(
            execution_directory,
            fixture.owner_account_id,
            fixture.source.identity().device_id(),
        )?;

        for (index, fault) in [
            crate::import::ImportFault::AfterEvents,
            crate::import::ImportFault::AfterProjections,
            crate::import::ImportFault::AfterCheckpoints,
            crate::import::ImportFault::AfterReceipt,
        ]
        .into_iter()
        .enumerate()
        {
            let state = fixture.root.path().join(format!("recipient-fault-{index}"));
            let imports = RecipientHistoryImportStore::new(
                &state,
                fixture.owner_account_id,
                fixture.replacement.identity().device_id(),
            );
            let result = imports.import_committed_chunk_with_fault(
                &execution,
                consent_record_id,
                0,
                committed.record_id(),
                fixture.replacement.identity(),
                &fixture.membership,
                3_000,
                fault,
            );
            assert!(
                matches!(result, Err(RecipientHistoryImportError::InjectedFailure(_))),
                "unexpected import fault result: {result:?}"
            );
            assert_eq!(
                imports.status(consent_record_id, 0)?,
                RecipientHistoryImportStatus::Absent
            );
            assert!(
                kilogram_store::EventStore::open(state.join("events"))?
                    .load_authorized_conversation(
                        committed.conversation_id(),
                        &fixture.membership,
                    )?
                    .is_empty()
            );
        }

        let crash_state = fixture.root.path().join("recipient-crash");
        let imports = RecipientHistoryImportStore::new(
            &crash_state,
            fixture.owner_account_id,
            fixture.replacement.identity().device_id(),
        );
        assert!(matches!(
            imports.import_committed_chunk_with_fault(
                &execution,
                consent_record_id,
                0,
                committed.record_id(),
                fixture.replacement.identity(),
                &fixture.membership,
                3_000,
                crate::import::ImportFault::CrashAfterReceipt,
            ),
            Err(RecipientHistoryImportError::InjectedFailure(
                "crash-after-receipt"
            ))
        ));
        assert!(matches!(
            imports.status(consent_record_id, 0),
            Err(RecipientHistoryImportError::StateTransactionInProgress(_))
        ));
        assert!(matches!(
            imports.import_committed_chunk(
                &execution,
                consent_record_id,
                0,
                committed.record_id(),
                fixture.replacement.identity(),
                &fixture.membership,
                3_001,
            )?,
            RecipientHistoryImportOutcome::Imported(_)
        ));
        Ok(())
    }

    #[test]
    fn recipient_import_requires_current_head_sequential_chain_and_current_device()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture(4, Some(2))?;
        let consent = fixture.open_consent()?;
        let consent_record_id = fixture.commit_consent(&consent)?;
        let execution_directory = fixture.root.path().join("import-chain-execution");
        let execution = fixture.open_execution("import-chain-execution")?;
        let first = commit_next_for_import(
            &fixture,
            &execution,
            &consent,
            consent_record_id,
            fixture.initial_checkpoint.as_ref(),
            2_000,
        )?;
        let second = commit_next_for_import(
            &fixture,
            &execution,
            &consent,
            consent_record_id,
            Some(first.checkpoint_after()),
            2_001,
        )?;
        drop(execution);
        let execution = OldHistoryRecoveryExecutionReadStore::open_existing(
            execution_directory,
            fixture.owner_account_id,
            fixture.source.identity().device_id(),
        )?;
        let stale_state = fixture.root.path().join("stale-recipient-state");
        let stale_imports = RecipientHistoryImportStore::new(
            &stale_state,
            fixture.owner_account_id,
            fixture.replacement.identity().device_id(),
        );
        assert!(matches!(
            stale_imports.import_committed_chunk(
                &execution,
                consent_record_id,
                0,
                first.record_id(),
                fixture.replacement.identity(),
                &fixture.membership,
                3_000,
            ),
            Err(RecipientHistoryImportError::Execution(
                HistoryRecoveryExecutionError::CommittedHeadMismatch
            ))
        ));
        assert!(matches!(
            stale_imports.import_committed_chunk(
                &execution,
                consent_record_id,
                0,
                second.record_id(),
                fixture.replacement.identity(),
                &fixture.membership,
                3_001,
            ),
            Err(RecipientHistoryImportError::MissingPreviousReceipt)
        ));
        let foreign = DeviceIdentity::generate()?;
        assert!(matches!(
            stale_imports.import_committed_chunk(
                &execution,
                consent_record_id,
                0,
                second.record_id(),
                &foreign,
                &fixture.membership,
                3_002,
            ),
            Err(RecipientHistoryImportError::WrongRecipientDevice)
        ));
        Ok(())
    }

    #[test]
    fn recipient_import_checkpoint_and_receipt_corruption_fail_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture(3, None)?;
        let consent = fixture.open_consent()?;
        let consent_record_id = fixture.commit_consent(&consent)?;
        let execution_directory = fixture.root.path().join("import-corrupt-execution");
        let execution = fixture.open_execution("import-corrupt-execution")?;
        let committed = commit_next_for_import(
            &fixture,
            &execution,
            &consent,
            consent_record_id,
            None,
            2_000,
        )?;
        drop(execution);
        let execution = OldHistoryRecoveryExecutionReadStore::open_existing(
            execution_directory,
            fixture.owner_account_id,
            fixture.source.identity().device_id(),
        )?;

        let conflict_state = fixture.root.path().join("checkpoint-conflict-state");
        let checkpoint = committed.checkpoint_after();
        let checkpoint_path = conflict_state
            .join("history-recovery")
            .join("import-checkpoints")
            .join(format!(
                "{}.checkpoint",
                checkpoint
                    .checkpoint_id()?
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            ));
        std::fs::create_dir_all(
            checkpoint_path
                .parent()
                .ok_or("checkpoint path has no parent")?,
        )?;
        std::fs::write(&checkpoint_path, b"conflicting-checkpoint")?;
        let conflict_imports = RecipientHistoryImportStore::new(
            &conflict_state,
            fixture.owner_account_id,
            fixture.replacement.identity().device_id(),
        );
        let conflict_result = conflict_imports.import_committed_chunk(
            &execution,
            consent_record_id,
            0,
            committed.record_id(),
            fixture.replacement.identity(),
            &fixture.membership,
            3_000,
        );
        assert!(
            matches!(
                conflict_result,
                Err(RecipientHistoryImportError::AppendOnlyConflict(_))
            ),
            "unexpected checkpoint conflict result: {conflict_result:?}"
        );
        assert!(
            kilogram_store::EventStore::open(conflict_state.join("events"))?
                .load_authorized_conversation(committed.conversation_id(), &fixture.membership)?
                .is_empty()
        );

        let corrupt_state = fixture.root.path().join("receipt-corrupt-state");
        let corrupt_imports = RecipientHistoryImportStore::new(
            &corrupt_state,
            fixture.owner_account_id,
            fixture.replacement.identity().device_id(),
        );
        corrupt_imports.import_committed_chunk(
            &execution,
            consent_record_id,
            0,
            committed.record_id(),
            fixture.replacement.identity(),
            &fixture.membership,
            3_001,
        )?;
        let receipt_directory = corrupt_state
            .join("history-recovery")
            .join("import-receipts");
        let receipt_path = std::fs::read_dir(receipt_directory)?
            .next()
            .ok_or("import receipt is missing")??
            .path();
        let mut bytes = std::fs::read(&receipt_path)?;
        let last = bytes.last_mut().ok_or("empty import receipt")?;
        *last ^= 1;
        std::fs::write(receipt_path, bytes)?;
        assert!(corrupt_imports.status(consent_record_id, 0).is_err());
        Ok(())
    }
}
