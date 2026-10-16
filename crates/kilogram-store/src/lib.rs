use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{RwLock, RwLockReadGuard},
};

#[cfg(unix)]
use std::fs::File;

use kilogram_identity::{ConversationMembershipSnapshot, ConversationScopeId, DeviceId};
use kilogram_protocol::{
    AuthorizedEvent, ConversationId, EventId, LocalTextProjection, ProtocolError, SignedEvent,
    canonical_sync_history_v2::{
        CANONICAL_CBOR_V1_EVENT_NAMESPACE, CanonicalSyncEventCarrierV2, CanonicalSyncEventKeyV2,
        CanonicalSyncEventNamespaceV2, CanonicalSyncHistoryV2Error,
        LEGACY_POSTCARD_V6_EVENT_NAMESPACE,
    },
    canonical_sync_legacy_event_v1::CanonicalSyncLegacyEventBridgeV1,
    canonical_sync_session_v2::{
        CanonicalSignedSyncSessionCheckpointV2, CanonicalSyncNextActionV2,
        CanonicalSyncSessionV2Error, MAX_CANONICAL_SYNC_SESSION_CHECKPOINT_BYTES_V2,
        decode_and_verify_signed_sync_session_checkpoint_v2,
        encode_signed_sync_session_checkpoint_v2, sync_session_checkpoint_id_v2,
        verify_signed_sync_session_checkpoint_v2,
    },
    canonical_sync_v2::{
        CanonicalSyncEventBatchV2, CanonicalSyncV2Error, MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH,
        MAX_CANONICAL_SYNC_V2_INVENTORY_EVENT_KEYS, encode_sync_event_batch_v2,
    },
};
use redb::{Database, Durability, ReadableDatabase, ReadableTable, TableDefinition};
use tempfile::NamedTempFile;
use thiserror::Error;

const EVENT_FILE_EXTENSION: &str = "event";
const AUTHORIZATION_FILE_EXTENSION: &str = "authorization";
const LOCAL_TEXT_PROJECTION_FILE_EXTENSION: &str = "local-text";
pub const CANONICAL_SYNC_V2_STORE_FILE: &str = "canonical-sync-v2.redb";
const CANONICAL_SYNC_V2_STORE_SCHEMA_KEY: &str = "schema";
const CANONICAL_SYNC_V2_STORE_SCHEMA: &[u8] =
    b"kilogram:canonical-sync-v2-carrier-store:v1\0kilogram/m1/sync/cbor/2";
const CANONICAL_SYNC_V2_STORAGE_KEY_BYTES: usize = 72;
const CANONICAL_SYNC_V2_META_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("canonical-sync-v2-meta-v1");
const CANONICAL_SYNC_V2_CARRIER_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("canonical-sync-v2-carriers-v1");
const CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("canonical-sync-v2-legacy-import-checkpoints-v1");
const CANONICAL_SYNC_V2_SESSION_CHECKPOINT_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("canonical-sync-v2-session-checkpoints-v1");
const CANONICAL_SYNC_V2_SESSION_CHECKPOINT_HEAD_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("canonical-sync-v2-session-checkpoint-heads-v1");
const CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_VERSION: u8 = 1;
const CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_KEY_BYTES: usize = 64;
const CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_BYTES: usize = 129;
const CANONICAL_SYNC_V2_SESSION_IDENTITY_KEY_BYTES: usize = 128;
const CANONICAL_SYNC_V2_SESSION_CHECKPOINT_KEY_BYTES: usize = 136;
const CANONICAL_SYNC_V2_SESSION_CHECKPOINT_RECORD_VERSION: u8 = 1;
const CANONICAL_SYNC_V2_SESSION_CHECKPOINT_RECORD_HEADER_BYTES: usize = 65;
const CANONICAL_SYNC_V2_SESSION_CHECKPOINT_HEAD_VERSION: u8 = 1;
const CANONICAL_SYNC_V2_SESSION_CHECKPOINT_HEAD_BYTES: usize = 73;
const CANONICAL_SYNC_V2_LEGACY_IMPORT_MEMBERSHIP_DIGEST_CONTEXT: &str =
    "kilogram:canonical-sync-v2-legacy-import-membership:v1";
const CANONICAL_SYNC_V2_LEGACY_IMPORT_EVENT_DIGEST_CONTEXT: &str =
    "kilogram:canonical-sync-v2-legacy-import-event:v1";
const CANONICAL_SYNC_V2_LEGACY_IMPORT_PLAN_ID_CONTEXT: &str =
    "kilogram:canonical-sync-v2-legacy-import-plan:v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreOutcome {
    Inserted,
    AlreadyPresent,
}

/// Exact append-only filesystem paths touched by a repository write.
///
/// The receipt lets the transaction coordinator consume a domain-owned write
/// set without duplicating repository directory layouts or file extensions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppendOnlyWriteReceipt {
    paths: Vec<PathBuf>,
}

impl AppendOnlyWriteReceipt {
    pub fn single(path: PathBuf) -> Self {
        Self { paths: vec![path] }
    }

    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    fn extend(&mut self, other: Self) {
        self.paths.extend(other.paths);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredEvent {
    pub id: EventId,
    pub event: SignedEvent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredAuthorizedEvent {
    pub id: EventId,
    pub event: AuthorizedEvent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncPlan {
    pub requested_from_remote: Vec<EventId>,
    pub events_for_remote: Vec<SignedEvent>,
    pub more_available: bool,
}

pub struct EventStore {
    root: PathBuf,
}

/// Dormant transactional carrier store for `kilogram/m1/sync/cbor/2`.
///
/// This database is deliberately separate from the active Postcard event
/// store. Each key includes the conversation and the complete typed Event key;
/// one redb write transaction preflights and commits an entire carrier batch.
/// Runtime migration/import and membership authorization remain external
/// activation requirements.
pub struct CanonicalSyncV2CarrierStore {
    database: Database,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CanonicalSyncV2LegacyImportItem {
    event_id: EventId,
    authorized_event_digest: [u8; 32],
}

/// Immutable, deterministic source claim for one dormant legacy-history import.
///
/// The plan binds the exact owner-signed membership snapshot and exact Postcard
/// bytes of every authorized legacy event. Rebuilding the plan after restart
/// produces the same ID only while the source claim is byte-identical.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncV2LegacyImportPlan {
    conversation_id: ConversationId,
    membership_revision: u64,
    membership_digest: [u8; 32],
    items: Vec<CanonicalSyncV2LegacyImportItem>,
    plan_id: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalSyncV2LegacyImportCursor {
    plan_id: [u8; 32],
    next_index: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncV2LegacyImportProgress {
    cursor: CanonicalSyncV2LegacyImportCursor,
    imported_event_keys: Vec<CanonicalSyncEventKeyV2>,
    total_event_count: u64,
    complete: bool,
}

/// Durable local progress for one exact legacy-import plan.
///
/// This record is written in the same redb transaction as the carrier page it
/// acknowledges. It is local migration state, not a signed sync checkpoint and
/// never crosses the wire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalSyncV2LegacyImportCheckpoint {
    conversation_id: [u8; 32],
    plan_id: [u8; 32],
    membership_revision: u64,
    membership_digest: [u8; 32],
    total_event_count: u64,
    last_page_start: u64,
    next_index: u64,
}

/// One verified Device-signed `/cbor/2` checkpoint tied to the immutable
/// legacy-import plan that authorized its local session bootstrap.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncV2StoredSessionCheckpoint {
    import_plan_id: [u8; 32],
    checkpoint_id: [u8; 32],
    checkpoint: CanonicalSignedSyncSessionCheckpointV2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CanonicalSyncV2SessionCheckpointHead {
    import_plan_id: [u8; 32],
    checkpoint_sequence: u64,
    checkpoint_id: [u8; 32],
}

impl CanonicalSyncV2LegacyImportPlan {
    pub fn plan_id(&self) -> [u8; 32] {
        self.plan_id
    }

    pub fn conversation_id(&self) -> ConversationId {
        self.conversation_id
    }

    pub fn membership_revision(&self) -> u64 {
        self.membership_revision
    }

    pub fn event_count(&self) -> u64 {
        self.items.len() as u64
    }

    pub fn start_cursor(&self) -> CanonicalSyncV2LegacyImportCursor {
        CanonicalSyncV2LegacyImportCursor {
            plan_id: self.plan_id,
            next_index: 0,
        }
    }

    pub fn resume_cursor(
        &self,
        next_index: u64,
    ) -> Result<CanonicalSyncV2LegacyImportCursor, StoreError> {
        self.verify()?;
        if next_index > self.event_count() {
            return Err(StoreError::CanonicalSyncV2LegacyImportCursorOutOfRange {
                next_index,
                event_count: self.event_count(),
            });
        }
        Ok(CanonicalSyncV2LegacyImportCursor {
            plan_id: self.plan_id,
            next_index,
        })
    }

    fn verify(&self) -> Result<(), StoreError> {
        if self.items.len() > MAX_CANONICAL_SYNC_V2_INVENTORY_EVENT_KEYS {
            return Err(StoreError::CanonicalSyncV2LegacyImportSourceTooLarge {
                actual: self.items.len(),
                maximum: MAX_CANONICAL_SYNC_V2_INVENTORY_EVENT_KEYS,
            });
        }
        if self
            .items
            .windows(2)
            .any(|pair| pair[0].event_id >= pair[1].event_id)
        {
            return Err(StoreError::CanonicalSyncV2LegacyImportPlanOrder);
        }
        let actual = canonical_sync_v2_legacy_import_plan_id(
            self.conversation_id,
            self.membership_revision,
            self.membership_digest,
            &self.items,
        );
        if actual != self.plan_id {
            return Err(StoreError::CanonicalSyncV2LegacyImportPlanIdMismatch {
                expected: self.plan_id,
                actual,
            });
        }
        Ok(())
    }
}

impl CanonicalSyncV2LegacyImportCursor {
    pub fn plan_id(self) -> [u8; 32] {
        self.plan_id
    }

    pub fn next_index(self) -> u64 {
        self.next_index
    }
}

impl CanonicalSyncV2LegacyImportProgress {
    pub fn cursor(&self) -> CanonicalSyncV2LegacyImportCursor {
        self.cursor
    }

    pub fn imported_event_keys(&self) -> &[CanonicalSyncEventKeyV2] {
        &self.imported_event_keys
    }

    pub fn total_event_count(&self) -> u64 {
        self.total_event_count
    }

    pub fn is_complete(&self) -> bool {
        self.complete
    }
}

impl CanonicalSyncV2LegacyImportCheckpoint {
    pub fn conversation_id(self) -> [u8; 32] {
        self.conversation_id
    }

    pub fn plan_id(self) -> [u8; 32] {
        self.plan_id
    }

    pub fn membership_revision(self) -> u64 {
        self.membership_revision
    }

    pub fn total_event_count(self) -> u64 {
        self.total_event_count
    }

    pub fn last_page_start(self) -> u64 {
        self.last_page_start
    }

    pub fn next_index(self) -> u64 {
        self.next_index
    }

    pub fn is_complete(self) -> bool {
        self.next_index == self.total_event_count
    }

    pub fn cursor(self) -> CanonicalSyncV2LegacyImportCursor {
        CanonicalSyncV2LegacyImportCursor {
            plan_id: self.plan_id,
            next_index: self.next_index,
        }
    }

    fn for_plan(
        plan: &CanonicalSyncV2LegacyImportPlan,
        last_page_start: u64,
        next_index: u64,
    ) -> Self {
        Self {
            conversation_id: *plan.conversation_id.as_bytes(),
            plan_id: plan.plan_id,
            membership_revision: plan.membership_revision,
            membership_digest: plan.membership_digest,
            total_event_count: plan.event_count(),
            last_page_start,
            next_index,
        }
    }

    fn verify_for_plan(self, plan: &CanonicalSyncV2LegacyImportPlan) -> Result<(), StoreError> {
        if self.conversation_id != *plan.conversation_id.as_bytes()
            || self.plan_id != plan.plan_id
            || self.membership_revision != plan.membership_revision
            || self.membership_digest != plan.membership_digest
            || self.total_event_count != plan.event_count()
        {
            return Err(StoreError::CanonicalSyncV2LegacyImportCheckpointPlanMismatch);
        }
        if self.last_page_start > self.next_index || self.next_index > self.total_event_count {
            return Err(
                StoreError::CanonicalSyncV2LegacyImportCheckpointIndexOutOfRange {
                    last_page_start: self.last_page_start,
                    next_index: self.next_index,
                    event_count: self.total_event_count,
                },
            );
        }
        Ok(())
    }
}

impl CanonicalSyncV2StoredSessionCheckpoint {
    pub fn import_plan_id(&self) -> [u8; 32] {
        self.import_plan_id
    }

    pub fn checkpoint_id(&self) -> [u8; 32] {
        self.checkpoint_id
    }

    pub fn checkpoint(&self) -> &CanonicalSignedSyncSessionCheckpointV2 {
        &self.checkpoint
    }
}

/// Immutable local-only storage for plaintext projections encrypted to one device.
///
/// Projection files are deliberately outside the replicated event store. The sync
/// protocol exchanges `AuthorizedEvent` values only and therefore cannot leak these
/// sender-readable copies to relays or peers.
pub struct LocalMessageStore {
    root: PathBuf,
}

pub trait EventReadRepository: Send + Sync {
    fn load_authorized_conversation(
        &self,
        conversation_id: ConversationId,
        membership: &ConversationMembershipSnapshot,
    ) -> Result<Vec<StoredAuthorizedEvent>, StoreError>;

    fn frontier(&self, conversation_id: ConversationId) -> Result<Vec<EventId>, StoreError>;

    fn authorized_inventory(
        &self,
        conversation_id: ConversationId,
        membership: &ConversationMembershipSnapshot,
    ) -> Result<Vec<EventId>, StoreError> {
        Ok(self
            .load_authorized_conversation(conversation_id, membership)?
            .into_iter()
            .map(|stored| stored.id)
            .collect())
    }

    fn authorized_events_by_id(
        &self,
        conversation_id: ConversationId,
        requested_event_ids: &[EventId],
        membership: &ConversationMembershipSnapshot,
    ) -> Result<Vec<AuthorizedEvent>, StoreError> {
        let available: HashMap<_, _> = self
            .load_authorized_conversation(conversation_id, membership)?
            .into_iter()
            .map(|stored| (stored.id, stored.event))
            .collect();
        requested_event_ids
            .iter()
            .map(|event_id| {
                available
                    .get(event_id)
                    .cloned()
                    .ok_or(StoreError::RequestedEventMissing {
                        conversation_id,
                        event_id: *event_id,
                    })
            })
            .collect()
    }
}

pub trait LocalMessageReadRepository: Send + Sync {
    fn get(&self, event_id: EventId) -> Result<LocalTextProjection, StoreError>;
}

pub struct ImmutableEventReadSnapshot {
    records: BTreeMap<String, Vec<u8>>,
}

pub struct ImmutableLocalMessageReadSnapshot {
    records: BTreeMap<String, Vec<u8>>,
}

/// A command-scoped authenticated read view over an immutable base snapshot.
///
/// Callers stage records before their durable filesystem transaction and only
/// publish the returned batch after that transaction commits. Staged records
/// are deliberately invisible to all read methods.
pub struct CommandEventReadOverlay {
    base: Box<dyn EventReadRepository>,
    committed: RwLock<HashMap<EventId, AuthorizedEvent>>,
}

/// Command-scoped local-projection counterpart to [`CommandEventReadOverlay`].
pub struct CommandLocalMessageReadOverlay {
    base: Box<dyn LocalMessageReadRepository>,
    committed: RwLock<HashMap<EventId, LocalTextProjection>>,
}

impl CommandEventReadOverlay {
    pub fn new(base: Box<dyn EventReadRepository>) -> Self {
        Self {
            base,
            committed: RwLock::new(HashMap::new()),
        }
    }

    pub fn stage_committed(
        &self,
        events: &[AuthorizedEvent],
        membership: &ConversationMembershipSnapshot,
    ) -> Result<Vec<AuthorizedEvent>, StoreError> {
        membership.verify().map_err(ProtocolError::from)?;
        let Some(first) = events.first() else {
            return Ok(Vec::new());
        };
        let conversation_id = first.event().conversation_id();
        if conversation_id.scope_id() != membership.conversation_id() {
            return Err(StoreError::ConversationMembershipMismatch {
                conversation_id,
                membership_conversation_id: membership.conversation_id(),
            });
        }
        let existing = self.load_authorized_conversation(conversation_id, membership)?;
        let mut events_by_id: HashMap<_, _> = existing
            .into_iter()
            .map(|stored| (stored.id, stored.event))
            .collect();
        let mut writer_positions: HashMap<_, _> = events_by_id
            .iter()
            .map(|(event_id, event)| {
                (
                    (
                        event.event().author_device_id(),
                        event.event().author_sequence(),
                    ),
                    *event_id,
                )
            })
            .collect();
        let mut staged = Vec::new();
        for event in events {
            event.verify_for_membership(membership)?;
            if event.event().conversation_id() != conversation_id {
                return Err(StoreError::ConversationMembershipMismatch {
                    conversation_id: event.event().conversation_id(),
                    membership_conversation_id: membership.conversation_id(),
                });
            }
            let event_id = event.event().event_id()?;
            if let Some(existing) = events_by_id.get(&event_id) {
                if existing != event {
                    return Err(StoreError::CommandOverlayEventConflict { event_id });
                }
                continue;
            }
            let writer_position = (
                event.event().author_device_id(),
                event.event().author_sequence(),
            );
            if let Some(existing_event_id) = writer_positions.get(&writer_position) {
                return Err(StoreError::WriterSequenceConflict {
                    author_device_id: writer_position.0,
                    author_sequence: writer_position.1,
                    existing_event_id: *existing_event_id,
                    rejected_event_id: event_id,
                });
            }
            writer_positions.insert(writer_position, event_id);
            events_by_id.insert(event_id, event.clone());
            staged.push(event.clone());
        }
        Ok(staged)
    }

    pub fn commit_staged(&self, staged: Vec<AuthorizedEvent>) -> Result<usize, StoreError> {
        let mut committed = self
            .committed
            .write()
            .map_err(|_| StoreError::CommandOverlayLockPoisoned("event"))?;
        let mut inserted = 0;
        for event in staged {
            let event_id = event.event().event_id()?;
            if let Some(existing) = committed.get(&event_id) {
                if existing != &event {
                    return Err(StoreError::CommandOverlayEventConflict { event_id });
                }
                continue;
            }
            committed.insert(event_id, event);
            inserted += 1;
        }
        Ok(inserted)
    }

    pub fn committed_count(&self) -> Result<usize, StoreError> {
        Ok(self.read_committed()?.len())
    }

    fn read_committed(
        &self,
    ) -> Result<RwLockReadGuard<'_, HashMap<EventId, AuthorizedEvent>>, StoreError> {
        self.committed
            .read()
            .map_err(|_| StoreError::CommandOverlayLockPoisoned("event"))
    }
}

impl EventReadRepository for CommandEventReadOverlay {
    fn load_authorized_conversation(
        &self,
        conversation_id: ConversationId,
        membership: &ConversationMembershipSnapshot,
    ) -> Result<Vec<StoredAuthorizedEvent>, StoreError> {
        let mut merged: HashMap<_, _> = self
            .base
            .load_authorized_conversation(conversation_id, membership)?
            .into_iter()
            .map(|stored| (stored.id, stored.event))
            .collect();
        for event in self.read_committed()?.values() {
            if event.event().conversation_id() != conversation_id {
                continue;
            }
            event.verify_for_membership(membership)?;
            let event_id = event.event().event_id()?;
            if let Some(existing) = merged.get(&event_id)
                && existing != event
            {
                return Err(StoreError::CommandOverlayEventConflict { event_id });
            }
            merged.insert(event_id, event.clone());
        }
        let mut writer_positions = HashSet::new();
        let mut stored = Vec::with_capacity(merged.len());
        for (id, event) in merged {
            let position = (
                event.event().author_device_id(),
                event.event().author_sequence(),
            );
            if !writer_positions.insert(position) {
                return Err(StoreError::DuplicateWriterSequence {
                    conversation_id,
                    author_device_id: position.0,
                    author_sequence: position.1,
                });
            }
            stored.push(StoredAuthorizedEvent { id, event });
        }
        stored.sort_by_cached_key(|event| event.id.to_string());
        Ok(stored)
    }

    fn frontier(&self, conversation_id: ConversationId) -> Result<Vec<EventId>, StoreError> {
        let mut frontier: HashSet<_> = self.base.frontier(conversation_id)?.into_iter().collect();
        let committed = self.read_committed()?;
        let overlay_events = committed
            .iter()
            .filter(|(_, event)| event.event().conversation_id() == conversation_id)
            .collect::<Vec<_>>();
        frontier.extend(overlay_events.iter().map(|(event_id, _)| **event_id));
        for parent in overlay_events
            .iter()
            .flat_map(|(_, event)| event.event().parents())
        {
            frontier.remove(parent);
        }
        let mut frontier = frontier.into_iter().collect::<Vec<_>>();
        frontier.sort_by_cached_key(ToString::to_string);
        Ok(frontier)
    }
}

impl CommandLocalMessageReadOverlay {
    pub fn new(base: Box<dyn LocalMessageReadRepository>) -> Self {
        Self {
            base,
            committed: RwLock::new(HashMap::new()),
        }
    }

    pub fn stage_committed(
        &self,
        projections: &[LocalTextProjection],
    ) -> Result<Vec<LocalTextProjection>, StoreError> {
        let mut staged = Vec::new();
        for projection in projections {
            let event_id = projection.event_id();
            match self.get(event_id) {
                Ok(existing) if existing == *projection => {}
                Ok(_) => {
                    return Err(StoreError::CommandOverlayLocalProjectionConflict { event_id });
                }
                Err(StoreError::LocalTextProjectionMissing { .. }) => {
                    staged.push(projection.clone());
                }
                Err(error) => return Err(error),
            }
        }
        Ok(staged)
    }

    pub fn commit_staged(&self, staged: Vec<LocalTextProjection>) -> Result<usize, StoreError> {
        let mut committed = self
            .committed
            .write()
            .map_err(|_| StoreError::CommandOverlayLockPoisoned("local-projection"))?;
        let mut inserted = 0;
        for projection in staged {
            let event_id = projection.event_id();
            if let Some(existing) = committed.get(&event_id) {
                if existing != &projection {
                    return Err(StoreError::CommandOverlayLocalProjectionConflict { event_id });
                }
                continue;
            }
            committed.insert(event_id, projection);
            inserted += 1;
        }
        Ok(inserted)
    }

    pub fn committed_count(&self) -> Result<usize, StoreError> {
        Ok(self
            .committed
            .read()
            .map_err(|_| StoreError::CommandOverlayLockPoisoned("local-projection"))?
            .len())
    }
}

impl LocalMessageReadRepository for CommandLocalMessageReadOverlay {
    fn get(&self, event_id: EventId) -> Result<LocalTextProjection, StoreError> {
        if let Some(projection) = self
            .committed
            .read()
            .map_err(|_| StoreError::CommandOverlayLockPoisoned("local-projection"))?
            .get(&event_id)
        {
            return Ok(projection.clone());
        }
        self.base.get(event_id)
    }
}

impl LocalMessageStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, StoreError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        let root = fs::canonicalize(root)?;
        Ok(Self { root })
    }

    pub fn put(&self, projection: &LocalTextProjection) -> Result<StoreOutcome, StoreError> {
        self.put_with_receipt(projection)
            .map(|(outcome, _)| outcome)
    }

    pub fn put_with_receipt(
        &self,
        projection: &LocalTextProjection,
    ) -> Result<(StoreOutcome, AppendOnlyWriteReceipt), StoreError> {
        let event_id = projection.event_id();
        let encoded = projection.encode()?;
        let destination = local_text_projection_path(&self.root, event_id);
        let outcome = if destination.try_exists()? {
            validate_existing_local_text_projection(&destination, &encoded, event_id)?
        } else {
            persist_local_text_projection(&self.root, &destination, &encoded, event_id)?
        };
        Ok((outcome, AppendOnlyWriteReceipt::single(destination)))
    }

    pub fn get(&self, event_id: EventId) -> Result<LocalTextProjection, StoreError> {
        let path = local_text_projection_path(&self.root, event_id);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(StoreError::LocalTextProjectionMissing { path, event_id });
            }
            Err(error) => return Err(error.into()),
        };
        let projection = LocalTextProjection::decode(&bytes).map_err(|source| {
            StoreError::InvalidStoredLocalTextProjection {
                path: path.clone(),
                source,
            }
        })?;
        if projection.event_id() != event_id {
            return Err(StoreError::LocalTextProjectionFileNameMismatch { path, event_id });
        }
        Ok(projection)
    }
}

impl CanonicalSyncV2CarrierStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, StoreError> {
        let requested = root.as_ref();
        fs::create_dir_all(requested)?;
        let root = fs::canonicalize(requested)?;
        let database = Database::create(root.join(CANONICAL_SYNC_V2_STORE_FILE))
            .map_err(canonical_sync_v2_database_error)?;
        let mut write = database
            .begin_write()
            .map_err(canonical_sync_v2_database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(canonical_sync_v2_database_error)?;
        {
            let mut metadata = write
                .open_table(CANONICAL_SYNC_V2_META_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            let existing = metadata
                .get(CANONICAL_SYNC_V2_STORE_SCHEMA_KEY)
                .map_err(canonical_sync_v2_database_error)?
                .map(|value| value.value().to_vec());
            match existing {
                Some(value) if value != CANONICAL_SYNC_V2_STORE_SCHEMA => {
                    return Err(StoreError::CanonicalSyncV2StoreSchemaMismatch);
                }
                Some(_) => {}
                None => {
                    metadata
                        .insert(
                            CANONICAL_SYNC_V2_STORE_SCHEMA_KEY,
                            CANONICAL_SYNC_V2_STORE_SCHEMA,
                        )
                        .map_err(canonical_sync_v2_database_error)?;
                }
            }
        }
        {
            let _carriers = write
                .open_table(CANONICAL_SYNC_V2_CARRIER_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
        }
        {
            let _legacy_import_checkpoints = write
                .open_table(CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
        }
        {
            let _session_checkpoints = write
                .open_table(CANONICAL_SYNC_V2_SESSION_CHECKPOINT_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
        }
        {
            let _session_checkpoint_heads = write
                .open_table(CANONICAL_SYNC_V2_SESSION_CHECKPOINT_HEAD_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
        }
        write.commit().map_err(canonical_sync_v2_database_error)?;
        Ok(Self { database })
    }

    pub fn inventory(
        &self,
        conversation_id: [u8; 32],
    ) -> Result<Vec<CanonicalSyncEventKeyV2>, StoreError> {
        let read = self
            .database
            .begin_read()
            .map_err(canonical_sync_v2_database_error)?;
        let table = read
            .open_table(CANONICAL_SYNC_V2_CARRIER_TABLE)
            .map_err(canonical_sync_v2_database_error)?;
        let mut event_keys = Vec::new();
        for entry in table.iter().map_err(canonical_sync_v2_database_error)? {
            let (stored_key, stored_value) = entry.map_err(canonical_sync_v2_database_error)?;
            let (stored_conversation_id, event_key) =
                decode_canonical_sync_v2_storage_key(stored_key.value())?;
            if stored_conversation_id != conversation_id {
                continue;
            }
            validate_canonical_sync_v2_stored_carrier(
                conversation_id,
                event_key,
                stored_value.value(),
            )?;
            event_keys.push(event_key);
            if event_keys.len() > MAX_CANONICAL_SYNC_V2_INVENTORY_EVENT_KEYS {
                return Err(StoreError::CanonicalSyncV2InventoryTooLarge {
                    actual: event_keys.len(),
                    maximum: MAX_CANONICAL_SYNC_V2_INVENTORY_EVENT_KEYS,
                });
            }
        }
        event_keys.sort_unstable();
        Ok(event_keys)
    }

    pub fn events_by_key(
        &self,
        conversation_id: [u8; 32],
        requested_event_keys: &[CanonicalSyncEventKeyV2],
    ) -> Result<Vec<CanonicalSyncEventCarrierV2>, StoreError> {
        let read = self
            .database
            .begin_read()
            .map_err(canonical_sync_v2_database_error)?;
        let table = read
            .open_table(CANONICAL_SYNC_V2_CARRIER_TABLE)
            .map_err(canonical_sync_v2_database_error)?;
        requested_event_keys
            .iter()
            .map(|event_key| {
                let storage_key = canonical_sync_v2_storage_key(conversation_id, *event_key);
                let encoded = table
                    .get(storage_key.as_slice())
                    .map_err(canonical_sync_v2_database_error)?
                    .map(|value| value.value().to_vec())
                    .ok_or(StoreError::CanonicalSyncV2CarrierMissing {
                        conversation_id,
                        event_key: *event_key,
                    })?;
                validate_canonical_sync_v2_stored_carrier(conversation_id, *event_key, &encoded)
            })
            .collect()
    }

    pub fn put_batch(
        &self,
        conversation_id: [u8; 32],
        events: &[CanonicalSyncEventCarrierV2],
    ) -> Result<(), StoreError> {
        if events.is_empty() {
            return Ok(());
        }

        let mut keyed_events = events
            .iter()
            .map(|event| {
                event
                    .key()
                    .map(|key| (key, event))
                    .map_err(CanonicalSyncV2Error::from)
            })
            .collect::<Result<Vec<_>, _>>()?;
        keyed_events.sort_unstable_by_key(|(key, _)| *key);
        let canonical_events: Vec<_> = keyed_events
            .iter()
            .map(|(_, event)| (*event).clone())
            .collect();
        encode_sync_event_batch_v2(&CanonicalSyncEventBatchV2 {
            conversation_id,
            events: canonical_events,
        })?;
        let prepared = keyed_events
            .into_iter()
            .map(|(key, event)| {
                event
                    .encode()
                    .map(|encoded| (key, encoded))
                    .map_err(CanonicalSyncV2Error::from)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut write = self
            .database
            .begin_write()
            .map_err(canonical_sync_v2_database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(canonical_sync_v2_database_error)?;
        {
            let mut table = write
                .open_table(CANONICAL_SYNC_V2_CARRIER_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            let mut missing = Vec::new();
            for (index, (event_key, encoded)) in prepared.iter().enumerate() {
                let storage_key = canonical_sync_v2_storage_key(conversation_id, *event_key);
                let existing = table
                    .get(storage_key.as_slice())
                    .map_err(canonical_sync_v2_database_error)?
                    .map(|value| value.value().to_vec());
                match existing {
                    Some(value) => {
                        validate_canonical_sync_v2_stored_carrier(
                            conversation_id,
                            *event_key,
                            &value,
                        )?;
                        if value != *encoded {
                            return Err(StoreError::CanonicalSyncV2CarrierConflict {
                                conversation_id,
                                event_key: *event_key,
                            });
                        }
                    }
                    None => missing.push(index),
                }
            }
            for index in missing {
                let (event_key, encoded) = &prepared[index];
                let storage_key = canonical_sync_v2_storage_key(conversation_id, *event_key);
                table
                    .insert(storage_key.as_slice(), encoded.as_slice())
                    .map_err(canonical_sync_v2_database_error)?;
            }
        }
        write.commit().map_err(canonical_sync_v2_database_error)
    }

    pub fn legacy_import_checkpoint(
        &self,
        plan: &CanonicalSyncV2LegacyImportPlan,
    ) -> Result<Option<CanonicalSyncV2LegacyImportCheckpoint>, StoreError> {
        plan.verify()?;
        let read = self
            .database
            .begin_read()
            .map_err(canonical_sync_v2_database_error)?;
        let table = read
            .open_table(CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_TABLE)
            .map_err(canonical_sync_v2_database_error)?;
        let key = canonical_sync_v2_legacy_import_checkpoint_key(plan);
        let checkpoint = table
            .get(key.as_slice())
            .map_err(canonical_sync_v2_database_error)?
            .map(|value| decode_canonical_sync_v2_legacy_import_checkpoint(value.value()))
            .transpose()?;
        if let Some(checkpoint) = checkpoint {
            checkpoint.verify_for_plan(plan)?;
        }
        Ok(checkpoint)
    }

    /// Returns the verified local head for one exact dormant `/cbor/2` session.
    pub fn session_checkpoint_head(
        &self,
        conversation_id: [u8; 32],
        local_device_id: [u8; 32],
        peer_device_id: [u8; 32],
        session_binding: [u8; 32],
    ) -> Result<Option<CanonicalSyncV2StoredSessionCheckpoint>, StoreError> {
        let identity_key = canonical_sync_v2_session_identity_key(
            conversation_id,
            local_device_id,
            peer_device_id,
            session_binding,
        );
        let read = self
            .database
            .begin_read()
            .map_err(canonical_sync_v2_database_error)?;
        let head = {
            let table = read
                .open_table(CANONICAL_SYNC_V2_SESSION_CHECKPOINT_HEAD_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            table
                .get(identity_key.as_slice())
                .map_err(canonical_sync_v2_database_error)?
                .map(|value| decode_canonical_sync_v2_session_checkpoint_head(value.value()))
                .transpose()?
        };
        let Some(head) = head else {
            return Ok(None);
        };
        let record_key =
            canonical_sync_v2_session_checkpoint_key(identity_key, head.checkpoint_sequence);
        let record = {
            let table = read
                .open_table(CANONICAL_SYNC_V2_SESSION_CHECKPOINT_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            table
                .get(record_key.as_slice())
                .map_err(canonical_sync_v2_database_error)?
                .map(|value| decode_canonical_sync_v2_session_checkpoint_record(value.value()))
                .transpose()?
                .ok_or(StoreError::CanonicalSyncV2SessionCheckpointHeadMissingRecord)?
        };
        validate_canonical_sync_v2_session_checkpoint_record(&record, identity_key, head)?;
        Ok(Some(record))
    }

    /// Atomically installs the first Device-signed session checkpoint only
    /// after the exact legacy-import plan is durably complete.
    ///
    /// The plan binding is local evidence stored beside the signed checkpoint;
    /// it is not added to or inferred from the peer-visible checkpoint bytes.
    pub fn install_legacy_import_session_bootstrap(
        &self,
        plan: &CanonicalSyncV2LegacyImportPlan,
        checkpoint: &CanonicalSignedSyncSessionCheckpointV2,
    ) -> Result<CanonicalSyncV2StoredSessionCheckpoint, StoreError> {
        plan.verify()?;
        validate_canonical_sync_v2_legacy_import_session_bootstrap(plan, checkpoint)?;
        let checkpoint_id = sync_session_checkpoint_id_v2(checkpoint)?;
        let stored = CanonicalSyncV2StoredSessionCheckpoint {
            import_plan_id: plan.plan_id,
            checkpoint_id,
            checkpoint: checkpoint.clone(),
        };
        let encoded_record = encode_canonical_sync_v2_session_checkpoint_record(&stored)?;
        let content = &checkpoint.content;
        let identity_key = canonical_sync_v2_session_identity_key(
            content.conversation_id,
            content.local_device_id,
            content.peer_device_id,
            content.session_binding,
        );
        let record_key =
            canonical_sync_v2_session_checkpoint_key(identity_key, content.checkpoint_sequence);
        let head = CanonicalSyncV2SessionCheckpointHead {
            import_plan_id: plan.plan_id,
            checkpoint_sequence: content.checkpoint_sequence,
            checkpoint_id,
        };
        let encoded_head = encode_canonical_sync_v2_session_checkpoint_head(head);

        let mut write = self
            .database
            .begin_write()
            .map_err(canonical_sync_v2_database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(canonical_sync_v2_database_error)?;

        let import_checkpoint = {
            let table = write
                .open_table(CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            let import_key = canonical_sync_v2_legacy_import_checkpoint_key(plan);
            table
                .get(import_key.as_slice())
                .map_err(canonical_sync_v2_database_error)?
                .map(|value| decode_canonical_sync_v2_legacy_import_checkpoint(value.value()))
                .transpose()?
                .ok_or(StoreError::CanonicalSyncV2SessionBootstrapImportMissing)?
        };
        import_checkpoint.verify_for_plan(plan)?;
        if !import_checkpoint.is_complete() {
            return Err(
                StoreError::CanonicalSyncV2SessionBootstrapImportIncomplete {
                    next_index: import_checkpoint.next_index,
                    event_count: import_checkpoint.total_event_count,
                },
            );
        }

        let existing_head = {
            let table = write
                .open_table(CANONICAL_SYNC_V2_SESSION_CHECKPOINT_HEAD_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            table
                .get(identity_key.as_slice())
                .map_err(canonical_sync_v2_database_error)?
                .map(|value| decode_canonical_sync_v2_session_checkpoint_head(value.value()))
                .transpose()?
        };
        if let Some(existing_head) = existing_head {
            if existing_head != head {
                return Err(StoreError::CanonicalSyncV2SessionCheckpointHeadConflict);
            }
            let existing = {
                let table = write
                    .open_table(CANONICAL_SYNC_V2_SESSION_CHECKPOINT_TABLE)
                    .map_err(canonical_sync_v2_database_error)?;
                table
                    .get(record_key.as_slice())
                    .map_err(canonical_sync_v2_database_error)?
                    .map(|value| decode_canonical_sync_v2_session_checkpoint_record(value.value()))
                    .transpose()?
                    .ok_or(StoreError::CanonicalSyncV2SessionCheckpointHeadMissingRecord)?
            };
            validate_canonical_sync_v2_session_checkpoint_record(
                &existing,
                identity_key,
                existing_head,
            )?;
            if existing != stored {
                return Err(StoreError::CanonicalSyncV2SessionCheckpointConflict);
            }
            return Ok(existing);
        }

        {
            let mut table = write
                .open_table(CANONICAL_SYNC_V2_SESSION_CHECKPOINT_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            if table
                .get(record_key.as_slice())
                .map_err(canonical_sync_v2_database_error)?
                .is_some()
            {
                return Err(StoreError::CanonicalSyncV2SessionCheckpointOrphanRecord);
            }
            table
                .insert(record_key.as_slice(), encoded_record.as_slice())
                .map_err(canonical_sync_v2_database_error)?;
        }
        {
            let mut table = write
                .open_table(CANONICAL_SYNC_V2_SESSION_CHECKPOINT_HEAD_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            table
                .insert(identity_key.as_slice(), encoded_head.as_slice())
                .map_err(canonical_sync_v2_database_error)?;
        }
        write.commit().map_err(canonical_sync_v2_database_error)?;
        Ok(stored)
    }

    fn commit_legacy_import_page(
        &self,
        plan: &CanonicalSyncV2LegacyImportPlan,
        cursor: CanonicalSyncV2LegacyImportCursor,
        next_index: u64,
        carriers: &[CanonicalSyncEventCarrierV2],
    ) -> Result<(), StoreError> {
        let conversation_id = *plan.conversation_id.as_bytes();
        let mut keyed_events = carriers
            .iter()
            .map(|event| {
                event
                    .key()
                    .map(|key| (key, event))
                    .map_err(CanonicalSyncV2Error::from)
            })
            .collect::<Result<Vec<_>, _>>()?;
        keyed_events.sort_unstable_by_key(|(key, _)| *key);
        let canonical_events = keyed_events
            .iter()
            .map(|(_, event)| (*event).clone())
            .collect::<Vec<_>>();
        encode_sync_event_batch_v2(&CanonicalSyncEventBatchV2 {
            conversation_id,
            events: canonical_events,
        })?;
        let prepared = keyed_events
            .into_iter()
            .map(|(key, event)| {
                event
                    .encode()
                    .map(|encoded| (key, encoded))
                    .map_err(CanonicalSyncV2Error::from)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let checkpoint_key = canonical_sync_v2_legacy_import_checkpoint_key(plan);
        let next_checkpoint =
            CanonicalSyncV2LegacyImportCheckpoint::for_plan(plan, cursor.next_index, next_index);
        next_checkpoint.verify_for_plan(plan)?;
        let encoded_checkpoint = encode_canonical_sync_v2_legacy_import_checkpoint(next_checkpoint);

        let mut write = self
            .database
            .begin_write()
            .map_err(canonical_sync_v2_database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(canonical_sync_v2_database_error)?;

        let durable_checkpoint = {
            let table = write
                .open_table(CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            table
                .get(checkpoint_key.as_slice())
                .map_err(canonical_sync_v2_database_error)?
                .map(|value| decode_canonical_sync_v2_legacy_import_checkpoint(value.value()))
                .transpose()?
        };
        if let Some(durable_checkpoint) = durable_checkpoint {
            durable_checkpoint.verify_for_plan(plan)?;
            if durable_checkpoint.next_index == next_index {
                let completed_noop =
                    cursor.next_index == durable_checkpoint.next_index && carriers.is_empty();
                if !completed_noop && cursor.next_index != durable_checkpoint.last_page_start {
                    return Err(
                        StoreError::CanonicalSyncV2LegacyImportDurableCursorMismatch {
                            requested_next_index: cursor.next_index,
                            requested_page_end: next_index,
                            durable_last_page_start: durable_checkpoint.last_page_start,
                            durable_next_index: durable_checkpoint.next_index,
                        },
                    );
                }
                let table = write
                    .open_table(CANONICAL_SYNC_V2_CARRIER_TABLE)
                    .map_err(canonical_sync_v2_database_error)?;
                for (event_key, encoded) in &prepared {
                    let storage_key = canonical_sync_v2_storage_key(conversation_id, *event_key);
                    let existing = table
                        .get(storage_key.as_slice())
                        .map_err(canonical_sync_v2_database_error)?
                        .map(|value| value.value().to_vec())
                        .ok_or(StoreError::CanonicalSyncV2CarrierMissing {
                            conversation_id,
                            event_key: *event_key,
                        })?;
                    validate_canonical_sync_v2_stored_carrier(
                        conversation_id,
                        *event_key,
                        &existing,
                    )?;
                    if existing != *encoded {
                        return Err(StoreError::CanonicalSyncV2CarrierConflict {
                            conversation_id,
                            event_key: *event_key,
                        });
                    }
                }
                return Ok(());
            }
            if durable_checkpoint.next_index != cursor.next_index {
                return Err(
                    StoreError::CanonicalSyncV2LegacyImportDurableCursorMismatch {
                        requested_next_index: cursor.next_index,
                        requested_page_end: next_index,
                        durable_last_page_start: durable_checkpoint.last_page_start,
                        durable_next_index: durable_checkpoint.next_index,
                    },
                );
            }
        } else if cursor.next_index != 0 {
            return Err(StoreError::CanonicalSyncV2LegacyImportCheckpointMissing {
                requested_next_index: cursor.next_index,
            });
        }

        {
            let mut table = write
                .open_table(CANONICAL_SYNC_V2_CARRIER_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            let mut missing = Vec::new();
            for (index, (event_key, encoded)) in prepared.iter().enumerate() {
                let storage_key = canonical_sync_v2_storage_key(conversation_id, *event_key);
                let existing = table
                    .get(storage_key.as_slice())
                    .map_err(canonical_sync_v2_database_error)?
                    .map(|value| value.value().to_vec());
                match existing {
                    Some(value) => {
                        validate_canonical_sync_v2_stored_carrier(
                            conversation_id,
                            *event_key,
                            &value,
                        )?;
                        if value != *encoded {
                            return Err(StoreError::CanonicalSyncV2CarrierConflict {
                                conversation_id,
                                event_key: *event_key,
                            });
                        }
                    }
                    None => missing.push(index),
                }
            }
            for index in missing {
                let (event_key, encoded) = &prepared[index];
                let storage_key = canonical_sync_v2_storage_key(conversation_id, *event_key);
                table
                    .insert(storage_key.as_slice(), encoded.as_slice())
                    .map_err(canonical_sync_v2_database_error)?;
            }
        }
        {
            let mut table = write
                .open_table(CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            table
                .insert(checkpoint_key.as_slice(), encoded_checkpoint.as_slice())
                .map_err(canonical_sync_v2_database_error)?;
        }
        write.commit().map_err(canonical_sync_v2_database_error)
    }

    /// Imports at most one canonical-sync batch from an exact legacy source plan.
    ///
    /// A caller may retry the same cursor after interruption. Exact carrier
    /// replay is idempotent. A cursor that skips a missing prefix, belongs to a
    /// different plan, or is paired with changed membership/source bytes fails
    /// before the next atomic target batch is committed.
    pub fn import_legacy_page(
        &self,
        source: &EventStore,
        membership: &ConversationMembershipSnapshot,
        plan: &CanonicalSyncV2LegacyImportPlan,
        cursor: CanonicalSyncV2LegacyImportCursor,
        maximum_events: usize,
    ) -> Result<CanonicalSyncV2LegacyImportProgress, StoreError> {
        plan.verify()?;
        if maximum_events == 0 || maximum_events > MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH {
            return Err(StoreError::CanonicalSyncV2LegacyImportPageSize {
                actual: maximum_events,
                maximum: MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH,
            });
        }
        if cursor.plan_id != plan.plan_id {
            return Err(StoreError::CanonicalSyncV2LegacyImportCursorPlanMismatch {
                expected: plan.plan_id,
                actual: cursor.plan_id,
            });
        }

        let membership_bytes = membership.encode().map_err(ProtocolError::from)?;
        let membership_digest =
            canonical_sync_v2_legacy_import_membership_digest(&membership_bytes);
        if membership.conversation_id() != plan.conversation_id.scope_id()
            || membership.revision() != plan.membership_revision
            || membership_digest != plan.membership_digest
        {
            return Err(StoreError::CanonicalSyncV2LegacyImportMembershipMismatch);
        }

        let next_index = usize::try_from(cursor.next_index).map_err(|_| {
            StoreError::CanonicalSyncV2LegacyImportCursorOutOfRange {
                next_index: cursor.next_index,
                event_count: plan.event_count(),
            }
        })?;
        if next_index > plan.items.len() {
            return Err(StoreError::CanonicalSyncV2LegacyImportCursorOutOfRange {
                next_index: cursor.next_index,
                event_count: plan.event_count(),
            });
        }

        self.verify_legacy_import_prefix(membership, plan, next_index)?;

        let end = next_index
            .saturating_add(maximum_events)
            .min(plan.items.len());
        let page = &plan.items[next_index..end];
        let requested_event_ids = page.iter().map(|item| item.event_id).collect::<Vec<_>>();
        let authorized_events = source.authorized_events_by_id(
            plan.conversation_id,
            &requested_event_ids,
            membership,
        )?;
        let mut carriers = Vec::with_capacity(authorized_events.len());
        for (item, event) in page.iter().zip(authorized_events) {
            let encoded = event.encode()?;
            let actual_digest =
                canonical_sync_v2_legacy_import_event_digest(item.event_id, &encoded);
            if actual_digest != item.authorized_event_digest {
                return Err(StoreError::CanonicalSyncV2LegacyImportSourceEventChanged {
                    event_id: item.event_id,
                });
            }
            let bridge = CanonicalSyncLegacyEventBridgeV1::from_active_event(&event)
                .map_err(CanonicalSyncHistoryV2Error::from)
                .map_err(CanonicalSyncV2Error::from)?;
            carriers.push(CanonicalSyncEventCarrierV2::LegacyPostcardV6(bridge));
        }

        let imported_event_keys = carriers
            .iter()
            .map(CanonicalSyncEventCarrierV2::key)
            .collect::<Result<Vec<_>, _>>()
            .map_err(CanonicalSyncV2Error::from)?;
        let next_index = end as u64;
        self.commit_legacy_import_page(plan, cursor, next_index, &carriers)?;
        Ok(CanonicalSyncV2LegacyImportProgress {
            cursor: CanonicalSyncV2LegacyImportCursor {
                plan_id: plan.plan_id,
                next_index,
            },
            imported_event_keys,
            total_event_count: plan.event_count(),
            complete: end == plan.items.len(),
        })
    }

    fn verify_legacy_import_prefix(
        &self,
        membership: &ConversationMembershipSnapshot,
        plan: &CanonicalSyncV2LegacyImportPlan,
        next_index: usize,
    ) -> Result<(), StoreError> {
        if next_index == 0 {
            return Ok(());
        }
        let expected_items = &plan.items[..next_index];
        let expected_keys = expected_items
            .iter()
            .map(|item| {
                CanonicalSyncEventKeyV2::new(
                    CanonicalSyncEventNamespaceV2::LegacyPostcardV6,
                    *item.event_id.as_bytes(),
                )
            })
            .collect::<Vec<_>>();
        let stored = self.events_by_key(*plan.conversation_id.as_bytes(), &expected_keys)?;
        for (item, carrier) in expected_items.iter().zip(stored) {
            let CanonicalSyncEventCarrierV2::LegacyPostcardV6(bridge) = carrier else {
                return Err(StoreError::CanonicalSyncV2LegacyImportPrefixMismatch {
                    event_id: item.event_id,
                });
            };
            bridge
                .active_authorized_event()
                .verify_for_membership(membership)?;
            let actual_digest = canonical_sync_v2_legacy_import_event_digest(
                item.event_id,
                bridge.active_authorized_event_postcard(),
            );
            if actual_digest != item.authorized_event_digest {
                return Err(StoreError::CanonicalSyncV2LegacyImportPrefixMismatch {
                    event_id: item.event_id,
                });
            }
        }
        Ok(())
    }
}

fn canonical_sync_v2_storage_key(
    conversation_id: [u8; 32],
    event_key: CanonicalSyncEventKeyV2,
) -> [u8; CANONICAL_SYNC_V2_STORAGE_KEY_BYTES] {
    let mut output = [0_u8; CANONICAL_SYNC_V2_STORAGE_KEY_BYTES];
    output[..32].copy_from_slice(&conversation_id);
    output[32..40].copy_from_slice(&event_key.namespace.code().to_be_bytes());
    output[40..].copy_from_slice(&event_key.event_id);
    output
}

fn decode_canonical_sync_v2_storage_key(
    encoded: &[u8],
) -> Result<([u8; 32], CanonicalSyncEventKeyV2), StoreError> {
    if encoded.len() != CANONICAL_SYNC_V2_STORAGE_KEY_BYTES {
        return Err(StoreError::CanonicalSyncV2InvalidStorageKeyLength {
            actual: encoded.len(),
            expected: CANONICAL_SYNC_V2_STORAGE_KEY_BYTES,
        });
    }
    let conversation_id = encoded[..32].try_into().map_err(|_| {
        StoreError::CanonicalSyncV2InvalidStorageKeyLength {
            actual: encoded.len(),
            expected: CANONICAL_SYNC_V2_STORAGE_KEY_BYTES,
        }
    })?;
    let namespace_code = u64::from_be_bytes(encoded[32..40].try_into().map_err(|_| {
        StoreError::CanonicalSyncV2InvalidStorageKeyLength {
            actual: encoded.len(),
            expected: CANONICAL_SYNC_V2_STORAGE_KEY_BYTES,
        }
    })?);
    let namespace = match namespace_code {
        LEGACY_POSTCARD_V6_EVENT_NAMESPACE => CanonicalSyncEventNamespaceV2::LegacyPostcardV6,
        CANONICAL_CBOR_V1_EVENT_NAMESPACE => CanonicalSyncEventNamespaceV2::CanonicalCborV1,
        other => return Err(StoreError::CanonicalSyncV2UnknownStorageNamespace(other)),
    };
    let event_id = encoded[40..].try_into().map_err(|_| {
        StoreError::CanonicalSyncV2InvalidStorageKeyLength {
            actual: encoded.len(),
            expected: CANONICAL_SYNC_V2_STORAGE_KEY_BYTES,
        }
    })?;
    Ok((
        conversation_id,
        CanonicalSyncEventKeyV2::new(namespace, event_id),
    ))
}

fn validate_canonical_sync_v2_stored_carrier(
    conversation_id: [u8; 32],
    expected_key: CanonicalSyncEventKeyV2,
    encoded: &[u8],
) -> Result<CanonicalSyncEventCarrierV2, StoreError> {
    let carrier = CanonicalSyncEventCarrierV2::decode_and_verify(encoded)
        .map_err(CanonicalSyncV2Error::from)?;
    if carrier.conversation_id() != &conversation_id {
        return Err(StoreError::CanonicalSyncV2StoredConversationMismatch {
            expected: conversation_id,
            actual: *carrier.conversation_id(),
        });
    }
    let actual_key = carrier.key().map_err(CanonicalSyncV2Error::from)?;
    if actual_key != expected_key {
        return Err(StoreError::CanonicalSyncV2StoredCarrierKeyMismatch {
            expected: expected_key,
            actual: actual_key,
        });
    }
    Ok(carrier)
}

fn canonical_sync_v2_database_error(error: impl std::fmt::Display) -> StoreError {
    StoreError::CanonicalSyncV2Database(error.to_string())
}

fn canonical_sync_v2_legacy_import_checkpoint_key(
    plan: &CanonicalSyncV2LegacyImportPlan,
) -> [u8; CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_KEY_BYTES] {
    let mut output = [0_u8; CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_KEY_BYTES];
    output[..32].copy_from_slice(plan.conversation_id.as_bytes());
    output[32..].copy_from_slice(&plan.plan_id);
    output
}

fn encode_canonical_sync_v2_legacy_import_checkpoint(
    checkpoint: CanonicalSyncV2LegacyImportCheckpoint,
) -> [u8; CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_BYTES] {
    let mut output = [0_u8; CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_BYTES];
    output[0] = CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_VERSION;
    output[1..33].copy_from_slice(&checkpoint.conversation_id);
    output[33..65].copy_from_slice(&checkpoint.plan_id);
    output[65..73].copy_from_slice(&checkpoint.membership_revision.to_be_bytes());
    output[73..105].copy_from_slice(&checkpoint.membership_digest);
    output[105..113].copy_from_slice(&checkpoint.total_event_count.to_be_bytes());
    output[113..121].copy_from_slice(&checkpoint.last_page_start.to_be_bytes());
    output[121..129].copy_from_slice(&checkpoint.next_index.to_be_bytes());
    output
}

fn decode_canonical_sync_v2_legacy_import_checkpoint(
    encoded: &[u8],
) -> Result<CanonicalSyncV2LegacyImportCheckpoint, StoreError> {
    if encoded.len() != CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_BYTES {
        return Err(
            StoreError::CanonicalSyncV2LegacyImportCheckpointInvalidLength {
                actual: encoded.len(),
                expected: CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_BYTES,
            },
        );
    }
    if encoded[0] != CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_VERSION {
        return Err(
            StoreError::CanonicalSyncV2LegacyImportCheckpointUnsupportedVersion(encoded[0]),
        );
    }
    let conversation_id = encoded[1..33].try_into().map_err(|_| {
        StoreError::CanonicalSyncV2LegacyImportCheckpointInvalidLength {
            actual: encoded.len(),
            expected: CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_BYTES,
        }
    })?;
    let plan_id = encoded[33..65].try_into().map_err(|_| {
        StoreError::CanonicalSyncV2LegacyImportCheckpointInvalidLength {
            actual: encoded.len(),
            expected: CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_BYTES,
        }
    })?;
    let membership_revision = u64::from_be_bytes(encoded[65..73].try_into().map_err(|_| {
        StoreError::CanonicalSyncV2LegacyImportCheckpointInvalidLength {
            actual: encoded.len(),
            expected: CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_BYTES,
        }
    })?);
    let membership_digest = encoded[73..105].try_into().map_err(|_| {
        StoreError::CanonicalSyncV2LegacyImportCheckpointInvalidLength {
            actual: encoded.len(),
            expected: CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_BYTES,
        }
    })?;
    let total_event_count = u64::from_be_bytes(encoded[105..113].try_into().map_err(|_| {
        StoreError::CanonicalSyncV2LegacyImportCheckpointInvalidLength {
            actual: encoded.len(),
            expected: CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_BYTES,
        }
    })?);
    let last_page_start = u64::from_be_bytes(encoded[113..121].try_into().map_err(|_| {
        StoreError::CanonicalSyncV2LegacyImportCheckpointInvalidLength {
            actual: encoded.len(),
            expected: CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_BYTES,
        }
    })?);
    let next_index = u64::from_be_bytes(encoded[121..129].try_into().map_err(|_| {
        StoreError::CanonicalSyncV2LegacyImportCheckpointInvalidLength {
            actual: encoded.len(),
            expected: CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_BYTES,
        }
    })?);
    Ok(CanonicalSyncV2LegacyImportCheckpoint {
        conversation_id,
        plan_id,
        membership_revision,
        membership_digest,
        total_event_count,
        last_page_start,
        next_index,
    })
}

fn validate_canonical_sync_v2_legacy_import_session_bootstrap(
    plan: &CanonicalSyncV2LegacyImportPlan,
    checkpoint: &CanonicalSignedSyncSessionCheckpointV2,
) -> Result<(), StoreError> {
    verify_signed_sync_session_checkpoint_v2(checkpoint)?;
    let content = &checkpoint.content;
    if content.conversation_id != *plan.conversation_id.as_bytes() {
        return Err(StoreError::CanonicalSyncV2SessionBootstrapConversationMismatch);
    }
    if content.checkpoint_sequence != 1
        || content.sync_round != 1
        || content.predecessor_checkpoint_id.is_some()
        || !content.received_event_keys.is_empty()
        || !content.sent_event_keys.is_empty()
        || content.local_more_available
        || content.peer_more_available
        || content.next_action != CanonicalSyncNextActionV2::SendInventory
        || content.paused
    {
        return Err(StoreError::CanonicalSyncV2SessionBootstrapShape);
    }
    Ok(())
}

fn canonical_sync_v2_session_identity_key(
    conversation_id: [u8; 32],
    local_device_id: [u8; 32],
    peer_device_id: [u8; 32],
    session_binding: [u8; 32],
) -> [u8; CANONICAL_SYNC_V2_SESSION_IDENTITY_KEY_BYTES] {
    let mut key = [0_u8; CANONICAL_SYNC_V2_SESSION_IDENTITY_KEY_BYTES];
    key[..32].copy_from_slice(&conversation_id);
    key[32..64].copy_from_slice(&local_device_id);
    key[64..96].copy_from_slice(&peer_device_id);
    key[96..128].copy_from_slice(&session_binding);
    key
}

fn canonical_sync_v2_session_checkpoint_key(
    identity_key: [u8; CANONICAL_SYNC_V2_SESSION_IDENTITY_KEY_BYTES],
    checkpoint_sequence: u64,
) -> [u8; CANONICAL_SYNC_V2_SESSION_CHECKPOINT_KEY_BYTES] {
    let mut key = [0_u8; CANONICAL_SYNC_V2_SESSION_CHECKPOINT_KEY_BYTES];
    key[..CANONICAL_SYNC_V2_SESSION_IDENTITY_KEY_BYTES].copy_from_slice(&identity_key);
    key[CANONICAL_SYNC_V2_SESSION_IDENTITY_KEY_BYTES..]
        .copy_from_slice(&checkpoint_sequence.to_be_bytes());
    key
}

fn encode_canonical_sync_v2_session_checkpoint_record(
    record: &CanonicalSyncV2StoredSessionCheckpoint,
) -> Result<Vec<u8>, StoreError> {
    verify_signed_sync_session_checkpoint_v2(&record.checkpoint)?;
    let checkpoint_id = sync_session_checkpoint_id_v2(&record.checkpoint)?;
    if checkpoint_id != record.checkpoint_id {
        return Err(StoreError::CanonicalSyncV2SessionCheckpointIdMismatch);
    }
    let encoded_checkpoint = encode_signed_sync_session_checkpoint_v2(&record.checkpoint)?;
    let mut encoded = Vec::with_capacity(
        CANONICAL_SYNC_V2_SESSION_CHECKPOINT_RECORD_HEADER_BYTES + encoded_checkpoint.len(),
    );
    encoded.push(CANONICAL_SYNC_V2_SESSION_CHECKPOINT_RECORD_VERSION);
    encoded.extend_from_slice(&record.import_plan_id);
    encoded.extend_from_slice(&record.checkpoint_id);
    encoded.extend_from_slice(&encoded_checkpoint);
    Ok(encoded)
}

fn decode_canonical_sync_v2_session_checkpoint_record(
    encoded: &[u8],
) -> Result<CanonicalSyncV2StoredSessionCheckpoint, StoreError> {
    let maximum = CANONICAL_SYNC_V2_SESSION_CHECKPOINT_RECORD_HEADER_BYTES
        + MAX_CANONICAL_SYNC_SESSION_CHECKPOINT_BYTES_V2;
    if encoded.len() <= CANONICAL_SYNC_V2_SESSION_CHECKPOINT_RECORD_HEADER_BYTES
        || encoded.len() > maximum
    {
        return Err(StoreError::CanonicalSyncV2SessionCheckpointRecordLength {
            actual: encoded.len(),
            maximum,
        });
    }
    if encoded[0] != CANONICAL_SYNC_V2_SESSION_CHECKPOINT_RECORD_VERSION {
        return Err(StoreError::CanonicalSyncV2SessionCheckpointRecordVersion(
            encoded[0],
        ));
    }
    let import_plan_id = encoded[1..33].try_into().map_err(|_| {
        StoreError::CanonicalSyncV2SessionCheckpointRecordLength {
            actual: encoded.len(),
            maximum,
        }
    })?;
    let checkpoint_id = encoded[33..65].try_into().map_err(|_| {
        StoreError::CanonicalSyncV2SessionCheckpointRecordLength {
            actual: encoded.len(),
            maximum,
        }
    })?;
    let checkpoint = decode_and_verify_signed_sync_session_checkpoint_v2(&encoded[65..])?;
    let record = CanonicalSyncV2StoredSessionCheckpoint {
        import_plan_id,
        checkpoint_id,
        checkpoint,
    };
    if sync_session_checkpoint_id_v2(&record.checkpoint)? != record.checkpoint_id {
        return Err(StoreError::CanonicalSyncV2SessionCheckpointIdMismatch);
    }
    Ok(record)
}

fn encode_canonical_sync_v2_session_checkpoint_head(
    head: CanonicalSyncV2SessionCheckpointHead,
) -> [u8; CANONICAL_SYNC_V2_SESSION_CHECKPOINT_HEAD_BYTES] {
    let mut encoded = [0_u8; CANONICAL_SYNC_V2_SESSION_CHECKPOINT_HEAD_BYTES];
    encoded[0] = CANONICAL_SYNC_V2_SESSION_CHECKPOINT_HEAD_VERSION;
    encoded[1..33].copy_from_slice(&head.import_plan_id);
    encoded[33..41].copy_from_slice(&head.checkpoint_sequence.to_be_bytes());
    encoded[41..73].copy_from_slice(&head.checkpoint_id);
    encoded
}

fn decode_canonical_sync_v2_session_checkpoint_head(
    encoded: &[u8],
) -> Result<CanonicalSyncV2SessionCheckpointHead, StoreError> {
    if encoded.len() != CANONICAL_SYNC_V2_SESSION_CHECKPOINT_HEAD_BYTES {
        return Err(StoreError::CanonicalSyncV2SessionCheckpointHeadLength {
            actual: encoded.len(),
            expected: CANONICAL_SYNC_V2_SESSION_CHECKPOINT_HEAD_BYTES,
        });
    }
    if encoded[0] != CANONICAL_SYNC_V2_SESSION_CHECKPOINT_HEAD_VERSION {
        return Err(StoreError::CanonicalSyncV2SessionCheckpointHeadVersion(
            encoded[0],
        ));
    }
    let import_plan_id = encoded[1..33].try_into().map_err(|_| {
        StoreError::CanonicalSyncV2SessionCheckpointHeadLength {
            actual: encoded.len(),
            expected: CANONICAL_SYNC_V2_SESSION_CHECKPOINT_HEAD_BYTES,
        }
    })?;
    let checkpoint_sequence = u64::from_be_bytes(encoded[33..41].try_into().map_err(|_| {
        StoreError::CanonicalSyncV2SessionCheckpointHeadLength {
            actual: encoded.len(),
            expected: CANONICAL_SYNC_V2_SESSION_CHECKPOINT_HEAD_BYTES,
        }
    })?);
    let checkpoint_id = encoded[41..73].try_into().map_err(|_| {
        StoreError::CanonicalSyncV2SessionCheckpointHeadLength {
            actual: encoded.len(),
            expected: CANONICAL_SYNC_V2_SESSION_CHECKPOINT_HEAD_BYTES,
        }
    })?;
    if checkpoint_sequence == 0 {
        return Err(StoreError::CanonicalSyncV2SessionCheckpointHeadSequence);
    }
    Ok(CanonicalSyncV2SessionCheckpointHead {
        import_plan_id,
        checkpoint_sequence,
        checkpoint_id,
    })
}

fn validate_canonical_sync_v2_session_checkpoint_record(
    record: &CanonicalSyncV2StoredSessionCheckpoint,
    expected_identity_key: [u8; CANONICAL_SYNC_V2_SESSION_IDENTITY_KEY_BYTES],
    expected_head: CanonicalSyncV2SessionCheckpointHead,
) -> Result<(), StoreError> {
    verify_signed_sync_session_checkpoint_v2(&record.checkpoint)?;
    let content = &record.checkpoint.content;
    let actual_identity_key = canonical_sync_v2_session_identity_key(
        content.conversation_id,
        content.local_device_id,
        content.peer_device_id,
        content.session_binding,
    );
    if actual_identity_key != expected_identity_key
        || content.checkpoint_sequence != expected_head.checkpoint_sequence
        || record.import_plan_id != expected_head.import_plan_id
        || record.checkpoint_id != expected_head.checkpoint_id
        || sync_session_checkpoint_id_v2(&record.checkpoint)? != record.checkpoint_id
    {
        return Err(StoreError::CanonicalSyncV2SessionCheckpointRecordMismatch);
    }
    Ok(())
}

fn canonical_sync_v2_legacy_import_membership_digest(encoded: &[u8]) -> [u8; 32] {
    let mut hasher =
        blake3::Hasher::new_derive_key(CANONICAL_SYNC_V2_LEGACY_IMPORT_MEMBERSHIP_DIGEST_CONTEXT);
    hasher.update(&(encoded.len() as u64).to_be_bytes());
    hasher.update(encoded);
    *hasher.finalize().as_bytes()
}

fn canonical_sync_v2_legacy_import_event_digest(event_id: EventId, encoded: &[u8]) -> [u8; 32] {
    let mut hasher =
        blake3::Hasher::new_derive_key(CANONICAL_SYNC_V2_LEGACY_IMPORT_EVENT_DIGEST_CONTEXT);
    hasher.update(event_id.as_bytes());
    hasher.update(&(encoded.len() as u64).to_be_bytes());
    hasher.update(encoded);
    *hasher.finalize().as_bytes()
}

fn canonical_sync_v2_legacy_import_plan_id(
    conversation_id: ConversationId,
    membership_revision: u64,
    membership_digest: [u8; 32],
    items: &[CanonicalSyncV2LegacyImportItem],
) -> [u8; 32] {
    let mut hasher =
        blake3::Hasher::new_derive_key(CANONICAL_SYNC_V2_LEGACY_IMPORT_PLAN_ID_CONTEXT);
    hasher.update(conversation_id.as_bytes());
    hasher.update(&membership_revision.to_be_bytes());
    hasher.update(&membership_digest);
    hasher.update(&(items.len() as u64).to_be_bytes());
    for item in items {
        hasher.update(item.event_id.as_bytes());
        hasher.update(&item.authorized_event_digest);
    }
    *hasher.finalize().as_bytes()
}

impl EventStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, StoreError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        let root = fs::canonicalize(root)?;
        Ok(Self { root })
    }

    pub fn canonical_sync_v2_legacy_import_plan(
        &self,
        conversation_id: ConversationId,
        membership: &ConversationMembershipSnapshot,
    ) -> Result<CanonicalSyncV2LegacyImportPlan, StoreError> {
        let stored = self.load_authorized_conversation(conversation_id, membership)?;
        if stored.len() > MAX_CANONICAL_SYNC_V2_INVENTORY_EVENT_KEYS {
            return Err(StoreError::CanonicalSyncV2LegacyImportSourceTooLarge {
                actual: stored.len(),
                maximum: MAX_CANONICAL_SYNC_V2_INVENTORY_EVENT_KEYS,
            });
        }
        let membership_bytes = membership.encode().map_err(ProtocolError::from)?;
        let membership_digest =
            canonical_sync_v2_legacy_import_membership_digest(&membership_bytes);
        let items = stored
            .into_iter()
            .map(|stored| {
                let encoded = stored.event.encode()?;
                Ok(CanonicalSyncV2LegacyImportItem {
                    event_id: stored.id,
                    authorized_event_digest: canonical_sync_v2_legacy_import_event_digest(
                        stored.id, &encoded,
                    ),
                })
            })
            .collect::<Result<Vec<_>, StoreError>>()?;
        let plan_id = canonical_sync_v2_legacy_import_plan_id(
            conversation_id,
            membership.revision(),
            membership_digest,
            &items,
        );
        let plan = CanonicalSyncV2LegacyImportPlan {
            conversation_id,
            membership_revision: membership.revision(),
            membership_digest,
            items,
            plan_id,
        };
        plan.verify()?;
        Ok(plan)
    }

    pub fn put(&self, event: &SignedEvent) -> Result<StoreOutcome, StoreError> {
        event.verify()?;
        let event_id = event.event_id()?;
        let encoded = event.encode()?;
        let conversation_directory = self.conversation_directory(event.conversation_id());
        fs::create_dir_all(&conversation_directory)?;
        let destination = event_path(&conversation_directory, event_id);

        if destination.try_exists()? {
            return validate_existing_event(&destination, &encoded, event_id);
        }
        for stored in self.load_conversation(event.conversation_id())? {
            if stored.event.author_device_id() == event.author_device_id()
                && stored.event.author_sequence() == event.author_sequence()
            {
                return Err(StoreError::WriterSequenceConflict {
                    author_device_id: event.author_device_id(),
                    author_sequence: event.author_sequence(),
                    existing_event_id: stored.id,
                    rejected_event_id: event_id,
                });
            }
        }

        persist_event(&conversation_directory, &destination, &encoded, event_id)
    }

    pub fn put_authorized(
        &self,
        event: &AuthorizedEvent,
        membership: &ConversationMembershipSnapshot,
    ) -> Result<StoreOutcome, StoreError> {
        self.put_authorized_with_receipt(event, membership)
            .map(|(outcome, _)| outcome)
    }

    pub fn put_authorized_with_receipt(
        &self,
        event: &AuthorizedEvent,
        membership: &ConversationMembershipSnapshot,
    ) -> Result<(StoreOutcome, AppendOnlyWriteReceipt), StoreError> {
        event.verify_for_membership(membership)?;
        let event_id = event.event().event_id()?;
        let outcome = self.put(event.event())?;
        let conversation_directory = self.conversation_directory(event.event().conversation_id());
        let event_destination = event_path(&conversation_directory, event_id);
        let authorization_destination = authorization_path(&conversation_directory, event_id);
        let encoded = event.encode()?;
        if authorization_destination.try_exists()? {
            validate_existing_authorization(&authorization_destination, &encoded, event_id)?;
        } else {
            persist_authorization(
                &conversation_directory,
                &authorization_destination,
                &encoded,
                event_id,
            )?;
        }
        Ok((
            outcome,
            AppendOnlyWriteReceipt {
                paths: vec![event_destination, authorization_destination],
            },
        ))
    }

    pub fn put_authorized_batch(
        &self,
        events: &[AuthorizedEvent],
        membership: &ConversationMembershipSnapshot,
    ) -> Result<(), StoreError> {
        self.put_authorized_batch_with_receipt(events, membership)
            .map(|_| ())
    }

    pub fn put_authorized_batch_with_receipt(
        &self,
        events: &[AuthorizedEvent],
        membership: &ConversationMembershipSnapshot,
    ) -> Result<AppendOnlyWriteReceipt, StoreError> {
        for event in events {
            event.verify_for_membership(membership)?;
        }
        let signed_events: Vec<_> = events.iter().map(|event| event.event().clone()).collect();
        self.put_batch(&signed_events)?;
        let mut receipt = AppendOnlyWriteReceipt { paths: Vec::new() };
        for event in events {
            let (_, event_receipt) = self.put_authorized_with_receipt(event, membership)?;
            receipt.extend(event_receipt);
        }
        Ok(receipt)
    }

    pub fn put_batch(&self, events: &[SignedEvent]) -> Result<(), StoreError> {
        let mut conversations: HashMap<ConversationId, Vec<&SignedEvent>> = HashMap::new();
        for event in events {
            event.verify()?;
            conversations
                .entry(event.conversation_id())
                .or_default()
                .push(event);
        }

        for (conversation_id, new_events) in conversations {
            let existing_events = self.load_conversation(conversation_id)?;
            let mut writer_positions: HashMap<(DeviceId, u64), EventId> = existing_events
                .into_iter()
                .map(|stored| {
                    (
                        (
                            stored.event.author_device_id(),
                            stored.event.author_sequence(),
                        ),
                        stored.id,
                    )
                })
                .collect();
            let conversation_directory = self.conversation_directory(conversation_id);
            fs::create_dir_all(&conversation_directory)?;

            for event in new_events {
                let event_id = event.event_id()?;
                let encoded = event.encode()?;
                let destination = event_path(&conversation_directory, event_id);
                if destination.try_exists()? {
                    validate_existing_event(&destination, &encoded, event_id)?;
                    continue;
                }

                let writer_position = (event.author_device_id(), event.author_sequence());
                if let Some(existing_event_id) = writer_positions.get(&writer_position) {
                    return Err(StoreError::WriterSequenceConflict {
                        author_device_id: event.author_device_id(),
                        author_sequence: event.author_sequence(),
                        existing_event_id: *existing_event_id,
                        rejected_event_id: event_id,
                    });
                }
                persist_event(&conversation_directory, &destination, &encoded, event_id)?;
                writer_positions.insert(writer_position, event_id);
            }
        }
        Ok(())
    }

    pub fn load_conversation(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Vec<StoredEvent>, StoreError> {
        let directory = self.conversation_directory(conversation_id);
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        let mut events = Vec::new();

        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_file()
                || entry.path().extension().and_then(|value| value.to_str())
                    != Some(EVENT_FILE_EXTENSION)
            {
                continue;
            }

            let path = entry.path();
            let encoded = fs::read(&path)?;
            let event = SignedEvent::decode_and_verify(&encoded).map_err(|source| {
                StoreError::InvalidStoredEvent {
                    path: path.clone(),
                    source,
                }
            })?;
            let event_id = event.event_id()?;
            let expected_name = format!("{event_id}.{EVENT_FILE_EXTENSION}");
            if entry.file_name() != expected_name.as_str() {
                return Err(StoreError::EventFileNameMismatch { path, event_id });
            }
            if event.conversation_id() != conversation_id {
                return Err(StoreError::ConversationDirectoryMismatch {
                    path,
                    expected: conversation_id,
                    actual: event.conversation_id(),
                });
            }
            events.push(StoredEvent {
                id: event_id,
                event,
            });
        }

        events.sort_by_cached_key(|stored| stored.id.to_string());
        let mut writer_positions = HashSet::new();
        for stored in &events {
            let position = (
                stored.event.author_device_id(),
                stored.event.author_sequence(),
            );
            if !writer_positions.insert(position) {
                return Err(StoreError::DuplicateWriterSequence {
                    conversation_id,
                    author_device_id: position.0,
                    author_sequence: position.1,
                });
            }
        }
        Ok(events)
    }

    pub fn load_authorized_conversation(
        &self,
        conversation_id: ConversationId,
        membership: &ConversationMembershipSnapshot,
    ) -> Result<Vec<StoredAuthorizedEvent>, StoreError> {
        membership.verify().map_err(ProtocolError::from)?;
        if membership.conversation_id() != conversation_id.scope_id() {
            return Err(StoreError::ConversationMembershipMismatch {
                conversation_id,
                membership_conversation_id: membership.conversation_id(),
            });
        }
        let stored_events = self.load_conversation(conversation_id)?;
        let directory = self.conversation_directory(conversation_id);
        stored_events
            .into_iter()
            .map(|stored| {
                let path = authorization_path(&directory, stored.id);
                let encoded = match fs::read(&path) {
                    Ok(encoded) => encoded,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        return Err(StoreError::EventAuthorizationMissing {
                            path,
                            event_id: stored.id,
                        });
                    }
                    Err(error) => return Err(error.into()),
                };
                let authorized =
                    AuthorizedEvent::decode_and_verify_author(&encoded).map_err(|source| {
                        StoreError::InvalidStoredAuthorization {
                            path: path.clone(),
                            source,
                        }
                    })?;
                authorized.verify_for_membership(membership)?;
                if authorized.event() != &stored.event
                    || authorized.event().event_id()? != stored.id
                {
                    return Err(StoreError::EventAuthorizationMismatch {
                        path,
                        event_id: stored.id,
                    });
                }
                Ok(StoredAuthorizedEvent {
                    id: stored.id,
                    event: authorized,
                })
            })
            .collect()
    }

    pub fn frontier(&self, conversation_id: ConversationId) -> Result<Vec<EventId>, StoreError> {
        let events = self.load_conversation(conversation_id)?;
        let referenced: HashSet<_> = events
            .iter()
            .flat_map(|stored| stored.event.parents().iter().copied())
            .collect();
        let mut frontier: Vec<_> = events
            .into_iter()
            .map(|stored| stored.id)
            .filter(|event_id| !referenced.contains(event_id))
            .collect();
        frontier.sort_by_cached_key(ToString::to_string);
        Ok(frontier)
    }

    pub fn inventory(&self, conversation_id: ConversationId) -> Result<Vec<EventId>, StoreError> {
        Ok(self
            .load_conversation(conversation_id)?
            .into_iter()
            .map(|stored| stored.id)
            .collect())
    }

    pub fn authorized_inventory(
        &self,
        conversation_id: ConversationId,
        membership: &ConversationMembershipSnapshot,
    ) -> Result<Vec<EventId>, StoreError> {
        Ok(self
            .load_authorized_conversation(conversation_id, membership)?
            .into_iter()
            .map(|stored| stored.id)
            .collect())
    }

    pub fn contains_author(
        &self,
        conversation_id: ConversationId,
        device_id: DeviceId,
    ) -> Result<bool, StoreError> {
        Ok(self
            .load_conversation(conversation_id)?
            .iter()
            .any(|stored| stored.event.author_device_id() == device_id))
    }

    pub fn plan_sync(
        &self,
        conversation_id: ConversationId,
        remote_inventory: &[EventId],
        maximum_events_per_direction: usize,
    ) -> Result<SyncPlan, StoreError> {
        let local_events = self.load_conversation(conversation_id)?;
        let local_ids: HashSet<_> = local_events.iter().map(|stored| stored.id).collect();
        let remote_ids: HashSet<_> = remote_inventory.iter().copied().collect();

        let mut requested_from_remote: Vec<_> =
            remote_ids.difference(&local_ids).copied().collect();
        requested_from_remote.sort_by_cached_key(ToString::to_string);

        let mut events_for_remote: Vec<_> = local_events
            .into_iter()
            .filter(|stored| !remote_ids.contains(&stored.id))
            .collect();
        events_for_remote.sort_by_cached_key(|stored| stored.id.to_string());

        let more_available = requested_from_remote.len() > maximum_events_per_direction
            || events_for_remote.len() > maximum_events_per_direction;
        requested_from_remote.truncate(maximum_events_per_direction);
        events_for_remote.truncate(maximum_events_per_direction);

        Ok(SyncPlan {
            requested_from_remote,
            events_for_remote: events_for_remote
                .into_iter()
                .map(|stored| stored.event)
                .collect(),
            more_available,
        })
    }

    pub fn events_by_id(
        &self,
        conversation_id: ConversationId,
        requested_event_ids: &[EventId],
    ) -> Result<Vec<SignedEvent>, StoreError> {
        let available: HashMap<_, _> = self
            .load_conversation(conversation_id)?
            .into_iter()
            .map(|stored| (stored.id, stored.event))
            .collect();

        requested_event_ids
            .iter()
            .map(|event_id| {
                available
                    .get(event_id)
                    .cloned()
                    .ok_or(StoreError::RequestedEventMissing {
                        conversation_id,
                        event_id: *event_id,
                    })
            })
            .collect()
    }

    pub fn authorized_events_by_id(
        &self,
        conversation_id: ConversationId,
        requested_event_ids: &[EventId],
        membership: &ConversationMembershipSnapshot,
    ) -> Result<Vec<AuthorizedEvent>, StoreError> {
        let available: HashMap<_, _> = self
            .load_authorized_conversation(conversation_id, membership)?
            .into_iter()
            .map(|stored| (stored.id, stored.event))
            .collect();
        requested_event_ids
            .iter()
            .map(|event_id| {
                available
                    .get(event_id)
                    .cloned()
                    .ok_or(StoreError::RequestedEventMissing {
                        conversation_id,
                        event_id: *event_id,
                    })
            })
            .collect()
    }

    fn conversation_directory(&self, conversation_id: ConversationId) -> PathBuf {
        self.root.join(conversation_id.to_string())
    }
}

impl EventReadRepository for EventStore {
    fn load_authorized_conversation(
        &self,
        conversation_id: ConversationId,
        membership: &ConversationMembershipSnapshot,
    ) -> Result<Vec<StoredAuthorizedEvent>, StoreError> {
        EventStore::load_authorized_conversation(self, conversation_id, membership)
    }

    fn frontier(&self, conversation_id: ConversationId) -> Result<Vec<EventId>, StoreError> {
        EventStore::frontier(self, conversation_id)
    }

    fn authorized_inventory(
        &self,
        conversation_id: ConversationId,
        membership: &ConversationMembershipSnapshot,
    ) -> Result<Vec<EventId>, StoreError> {
        EventStore::authorized_inventory(self, conversation_id, membership)
    }

    fn authorized_events_by_id(
        &self,
        conversation_id: ConversationId,
        requested_event_ids: &[EventId],
        membership: &ConversationMembershipSnapshot,
    ) -> Result<Vec<AuthorizedEvent>, StoreError> {
        EventStore::authorized_events_by_id(self, conversation_id, requested_event_ids, membership)
    }
}

impl LocalMessageReadRepository for LocalMessageStore {
    fn get(&self, event_id: EventId) -> Result<LocalTextProjection, StoreError> {
        LocalMessageStore::get(self, event_id)
    }
}

impl ImmutableEventReadSnapshot {
    pub fn from_records(
        records: impl IntoIterator<Item = (String, Vec<u8>)>,
    ) -> Result<Self, StoreError> {
        let mut validated = BTreeMap::new();
        for (relative_path, content) in records {
            let components = validate_snapshot_path(&relative_path, 2)?;
            if !is_canonical_hex_identifier(components[0])
                || !is_canonical_snapshot_file(
                    components[1],
                    &[EVENT_FILE_EXTENSION, AUTHORIZATION_FILE_EXTENSION],
                )
            {
                return Err(StoreError::InvalidImmutableSnapshotPath(relative_path));
            }
            if validated.insert(relative_path.clone(), content).is_some() {
                return Err(StoreError::DuplicateImmutableSnapshotPath(relative_path));
            }
        }
        Ok(Self { records: validated })
    }

    fn load_conversation(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Vec<StoredEvent>, StoreError> {
        let prefix = format!("{conversation_id}/");
        let mut events = Vec::new();
        for (relative_path, encoded) in &self.records {
            let Some(file_name) = relative_path.strip_prefix(&prefix) else {
                continue;
            };
            if !file_name.ends_with(&format!(".{EVENT_FILE_EXTENSION}")) {
                continue;
            }
            let path = PathBuf::from(relative_path);
            let event = SignedEvent::decode_and_verify(encoded).map_err(|source| {
                StoreError::InvalidStoredEvent {
                    path: path.clone(),
                    source,
                }
            })?;
            let event_id = event.event_id()?;
            let expected_name = format!("{event_id}.{EVENT_FILE_EXTENSION}");
            if file_name != expected_name {
                return Err(StoreError::EventFileNameMismatch { path, event_id });
            }
            if event.conversation_id() != conversation_id {
                return Err(StoreError::ConversationDirectoryMismatch {
                    path,
                    expected: conversation_id,
                    actual: event.conversation_id(),
                });
            }
            events.push(StoredEvent {
                id: event_id,
                event,
            });
        }
        validate_loaded_event_positions(conversation_id, &mut events)?;
        Ok(events)
    }
}

impl EventReadRepository for ImmutableEventReadSnapshot {
    fn load_authorized_conversation(
        &self,
        conversation_id: ConversationId,
        membership: &ConversationMembershipSnapshot,
    ) -> Result<Vec<StoredAuthorizedEvent>, StoreError> {
        membership.verify().map_err(ProtocolError::from)?;
        if membership.conversation_id() != conversation_id.scope_id() {
            return Err(StoreError::ConversationMembershipMismatch {
                conversation_id,
                membership_conversation_id: membership.conversation_id(),
            });
        }
        self.load_conversation(conversation_id)?
            .into_iter()
            .map(|stored| {
                let relative_path = format!(
                    "{conversation_id}/{}.{AUTHORIZATION_FILE_EXTENSION}",
                    stored.id
                );
                let path = PathBuf::from(&relative_path);
                let encoded = self.records.get(&relative_path).ok_or_else(|| {
                    StoreError::EventAuthorizationMissing {
                        path: path.clone(),
                        event_id: stored.id,
                    }
                })?;
                let authorized =
                    AuthorizedEvent::decode_and_verify_author(encoded).map_err(|source| {
                        StoreError::InvalidStoredAuthorization {
                            path: path.clone(),
                            source,
                        }
                    })?;
                authorized.verify_for_membership(membership)?;
                if authorized.event() != &stored.event
                    || authorized.event().event_id()? != stored.id
                {
                    return Err(StoreError::EventAuthorizationMismatch {
                        path,
                        event_id: stored.id,
                    });
                }
                Ok(StoredAuthorizedEvent {
                    id: stored.id,
                    event: authorized,
                })
            })
            .collect()
    }

    fn frontier(&self, conversation_id: ConversationId) -> Result<Vec<EventId>, StoreError> {
        let events = self.load_conversation(conversation_id)?;
        let referenced: HashSet<_> = events
            .iter()
            .flat_map(|stored| stored.event.parents().iter().copied())
            .collect();
        let mut frontier: Vec<_> = events
            .into_iter()
            .map(|stored| stored.id)
            .filter(|event_id| !referenced.contains(event_id))
            .collect();
        frontier.sort_by_cached_key(ToString::to_string);
        Ok(frontier)
    }
}

impl ImmutableLocalMessageReadSnapshot {
    pub fn from_records(
        records: impl IntoIterator<Item = (String, Vec<u8>)>,
    ) -> Result<Self, StoreError> {
        let mut validated = BTreeMap::new();
        for (relative_path, content) in records {
            let components = validate_snapshot_path(&relative_path, 1)?;
            if !is_canonical_snapshot_file(components[0], &[LOCAL_TEXT_PROJECTION_FILE_EXTENSION]) {
                return Err(StoreError::InvalidImmutableSnapshotPath(relative_path));
            }
            if validated.insert(relative_path.clone(), content).is_some() {
                return Err(StoreError::DuplicateImmutableSnapshotPath(relative_path));
            }
        }
        Ok(Self { records: validated })
    }
}

impl LocalMessageReadRepository for ImmutableLocalMessageReadSnapshot {
    fn get(&self, event_id: EventId) -> Result<LocalTextProjection, StoreError> {
        let relative_path = format!("{event_id}.{LOCAL_TEXT_PROJECTION_FILE_EXTENSION}");
        let path = PathBuf::from(&relative_path);
        let bytes = self.records.get(&relative_path).ok_or_else(|| {
            StoreError::LocalTextProjectionMissing {
                path: path.clone(),
                event_id,
            }
        })?;
        let projection = LocalTextProjection::decode(bytes).map_err(|source| {
            StoreError::InvalidStoredLocalTextProjection {
                path: path.clone(),
                source,
            }
        })?;
        if projection.event_id() != event_id {
            return Err(StoreError::LocalTextProjectionFileNameMismatch { path, event_id });
        }
        Ok(projection)
    }
}

fn validate_snapshot_path(
    relative_path: &str,
    expected_components: usize,
) -> Result<Vec<&str>, StoreError> {
    if relative_path.is_empty() || relative_path.contains('\\') {
        return Err(StoreError::InvalidImmutableSnapshotPath(
            relative_path.to_owned(),
        ));
    }
    let components = relative_path.split('/').collect::<Vec<_>>();
    if components.len() != expected_components
        || components
            .iter()
            .any(|component| component.is_empty() || matches!(*component, "." | ".."))
    {
        return Err(StoreError::InvalidImmutableSnapshotPath(
            relative_path.to_owned(),
        ));
    }
    Ok(components)
}

fn is_canonical_snapshot_file(file_name: &str, allowed_extensions: &[&str]) -> bool {
    let Some((identifier, extension)) = file_name.split_once('.') else {
        return false;
    };
    is_canonical_hex_identifier(identifier) && allowed_extensions.contains(&extension)
}

fn is_canonical_hex_identifier(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_loaded_event_positions(
    conversation_id: ConversationId,
    events: &mut [StoredEvent],
) -> Result<(), StoreError> {
    events.sort_by_cached_key(|stored| stored.id.to_string());
    let mut writer_positions = HashSet::new();
    for stored in events {
        let position = (
            stored.event.author_device_id(),
            stored.event.author_sequence(),
        );
        if !writer_positions.insert(position) {
            return Err(StoreError::DuplicateWriterSequence {
                conversation_id,
                author_device_id: position.0,
                author_sequence: position.1,
            });
        }
    }
    Ok(())
}

fn event_path(directory: &Path, event_id: EventId) -> PathBuf {
    directory.join(format!("{event_id}.{EVENT_FILE_EXTENSION}"))
}

fn authorization_path(directory: &Path, event_id: EventId) -> PathBuf {
    directory.join(format!("{event_id}.{AUTHORIZATION_FILE_EXTENSION}"))
}

fn local_text_projection_path(directory: &Path, event_id: EventId) -> PathBuf {
    directory.join(format!("{event_id}.{LOCAL_TEXT_PROJECTION_FILE_EXTENSION}"))
}

fn validate_existing_authorization(
    path: &Path,
    expected: &[u8],
    event_id: EventId,
) -> Result<(), StoreError> {
    if fs::read(path)? == expected {
        Ok(())
    } else {
        Err(StoreError::ImmutableEventAuthorizationConflict {
            path: path.to_path_buf(),
            event_id,
        })
    }
}

fn persist_authorization(
    conversation_directory: &Path,
    destination: &Path,
    encoded: &[u8],
    event_id: EventId,
) -> Result<(), StoreError> {
    let mut temporary = NamedTempFile::new_in(conversation_directory)?;
    temporary.write_all(encoded)?;
    temporary.as_file().sync_all()?;
    match temporary.persist_noclobber(destination) {
        Ok(file) => {
            file.sync_all()?;
            sync_directory(conversation_directory)?;
            Ok(())
        }
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
            validate_existing_authorization(destination, encoded, event_id)
        }
        Err(error) => Err(error.error.into()),
    }
}

fn validate_existing_event(
    path: &Path,
    expected: &[u8],
    event_id: EventId,
) -> Result<StoreOutcome, StoreError> {
    let actual = fs::read(path)?;
    if actual == expected {
        Ok(StoreOutcome::AlreadyPresent)
    } else {
        Err(StoreError::ImmutableEventConflict {
            path: path.to_path_buf(),
            event_id,
        })
    }
}

fn persist_event(
    conversation_directory: &Path,
    destination: &Path,
    encoded: &[u8],
    event_id: EventId,
) -> Result<StoreOutcome, StoreError> {
    let mut temporary = NamedTempFile::new_in(conversation_directory)?;
    temporary.write_all(encoded)?;
    temporary.as_file().sync_all()?;

    match temporary.persist_noclobber(destination) {
        Ok(file) => {
            file.sync_all()?;
            sync_directory(conversation_directory)?;
            Ok(StoreOutcome::Inserted)
        }
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
            validate_existing_event(destination, encoded, event_id)
        }
        Err(error) => Err(error.error.into()),
    }
}

fn validate_existing_local_text_projection(
    path: &Path,
    expected: &[u8],
    event_id: EventId,
) -> Result<StoreOutcome, StoreError> {
    let actual = fs::read(path)?;
    if actual == expected {
        Ok(StoreOutcome::AlreadyPresent)
    } else {
        Err(StoreError::ImmutableLocalTextProjectionConflict {
            path: path.to_path_buf(),
            event_id,
        })
    }
}

fn persist_local_text_projection(
    directory: &Path,
    destination: &Path,
    encoded: &[u8],
    event_id: EventId,
) -> Result<StoreOutcome, StoreError> {
    let mut temporary = NamedTempFile::new_in(directory)?;
    temporary.write_all(encoded)?;
    temporary.as_file().sync_all()?;
    match temporary.persist_noclobber(destination) {
        Ok(file) => {
            file.sync_all()?;
            sync_directory(directory)?;
            Ok(StoreOutcome::Inserted)
        }
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
            validate_existing_local_text_projection(destination, encoded, event_id)
        }
        Err(error) => Err(error.error.into()),
    }
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("event store I/O failed")]
    Io(#[from] io::Error),

    #[error("event validation failed")]
    Protocol(#[from] ProtocolError),

    #[error("canonical sync v2 carrier validation failed")]
    CanonicalSyncV2(#[from] CanonicalSyncV2Error),

    #[error("canonical sync v2 signed session checkpoint validation failed")]
    CanonicalSyncV2SessionCheckpoint(#[from] CanonicalSyncSessionV2Error),

    #[error("canonical sync v2 carrier database operation failed: {0}")]
    CanonicalSyncV2Database(String),

    #[error("canonical sync v2 carrier database has an incompatible schema marker")]
    CanonicalSyncV2StoreSchemaMismatch,

    #[error("canonical sync v2 storage key has {actual} bytes; expected {expected}")]
    CanonicalSyncV2InvalidStorageKeyLength { actual: usize, expected: usize },

    #[error("canonical sync v2 storage key has unknown Event namespace {0}")]
    CanonicalSyncV2UnknownStorageNamespace(u64),

    #[error(
        "canonical sync v2 conversation contains {actual} typed Event keys; maximum is {maximum}"
    )]
    CanonicalSyncV2InventoryTooLarge { actual: usize, maximum: usize },

    #[error("canonical sync v2 carrier {event_key:?} is missing from {conversation_id:?}")]
    CanonicalSyncV2CarrierMissing {
        conversation_id: [u8; 32],
        event_key: CanonicalSyncEventKeyV2,
    },

    #[error(
        "canonical sync v2 carrier {event_key:?} conflicts with immutable data in {conversation_id:?}"
    )]
    CanonicalSyncV2CarrierConflict {
        conversation_id: [u8; 32],
        event_key: CanonicalSyncEventKeyV2,
    },

    #[error(
        "canonical sync v2 stored carrier belongs to conversation {actual:?}, expected {expected:?}"
    )]
    CanonicalSyncV2StoredConversationMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },

    #[error(
        "canonical sync v2 stored carrier key {actual:?} does not match database key {expected:?}"
    )]
    CanonicalSyncV2StoredCarrierKeyMismatch {
        expected: CanonicalSyncEventKeyV2,
        actual: CanonicalSyncEventKeyV2,
    },

    #[error(
        "legacy import source contains {actual} events; canonical sync v2 maximum is {maximum}"
    )]
    CanonicalSyncV2LegacyImportSourceTooLarge { actual: usize, maximum: usize },

    #[error("legacy import plan event IDs are not strictly ordered and unique")]
    CanonicalSyncV2LegacyImportPlanOrder,

    #[error("legacy import plan ID {actual:?} does not match embedded ID {expected:?}")]
    CanonicalSyncV2LegacyImportPlanIdMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },

    #[error("legacy import cursor belongs to plan {actual:?}; expected {expected:?}")]
    CanonicalSyncV2LegacyImportCursorPlanMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },

    #[error("legacy import cursor index {next_index} exceeds event count {event_count}")]
    CanonicalSyncV2LegacyImportCursorOutOfRange { next_index: u64, event_count: u64 },

    #[error("legacy import page size is {actual}; expected 1..={maximum}")]
    CanonicalSyncV2LegacyImportPageSize { actual: usize, maximum: usize },

    #[error("legacy import membership snapshot does not match the exact source plan")]
    CanonicalSyncV2LegacyImportMembershipMismatch,

    #[error("legacy import source bytes changed for event {event_id}")]
    CanonicalSyncV2LegacyImportSourceEventChanged { event_id: EventId },

    #[error("legacy import resume prefix does not match event {event_id}")]
    CanonicalSyncV2LegacyImportPrefixMismatch { event_id: EventId },

    #[error("legacy import checkpoint has {actual} bytes; expected {expected}")]
    CanonicalSyncV2LegacyImportCheckpointInvalidLength { actual: usize, expected: usize },

    #[error("legacy import checkpoint uses unsupported version {0}")]
    CanonicalSyncV2LegacyImportCheckpointUnsupportedVersion(u8),

    #[error("legacy import checkpoint does not match the exact source plan")]
    CanonicalSyncV2LegacyImportCheckpointPlanMismatch,

    #[error(
        "legacy import checkpoint page start {last_page_start}, next index {next_index}, event count {event_count} are inconsistent"
    )]
    CanonicalSyncV2LegacyImportCheckpointIndexOutOfRange {
        last_page_start: u64,
        next_index: u64,
        event_count: u64,
    },

    #[error(
        "legacy import has no durable checkpoint for requested cursor index {requested_next_index}"
    )]
    CanonicalSyncV2LegacyImportCheckpointMissing { requested_next_index: u64 },

    #[error(
        "legacy import durable page is {durable_last_page_start}..{durable_next_index}; requested {requested_next_index}..{requested_page_end}"
    )]
    CanonicalSyncV2LegacyImportDurableCursorMismatch {
        requested_next_index: u64,
        requested_page_end: u64,
        durable_last_page_start: u64,
        durable_next_index: u64,
    },

    #[error("canonical sync v2 session bootstrap belongs to a different conversation")]
    CanonicalSyncV2SessionBootstrapConversationMismatch,

    #[error("canonical sync v2 session bootstrap has a non-initial checkpoint shape")]
    CanonicalSyncV2SessionBootstrapShape,

    #[error("canonical sync v2 session bootstrap has no durable legacy-import checkpoint")]
    CanonicalSyncV2SessionBootstrapImportMissing,

    #[error(
        "canonical sync v2 session bootstrap import is incomplete at {next_index} of {event_count}"
    )]
    CanonicalSyncV2SessionBootstrapImportIncomplete { next_index: u64, event_count: u64 },

    #[error("canonical sync v2 session checkpoint record has {actual} bytes; maximum is {maximum}")]
    CanonicalSyncV2SessionCheckpointRecordLength { actual: usize, maximum: usize },

    #[error("canonical sync v2 session checkpoint record uses unsupported version {0}")]
    CanonicalSyncV2SessionCheckpointRecordVersion(u8),

    #[error("canonical sync v2 session checkpoint ID does not match its signed bytes")]
    CanonicalSyncV2SessionCheckpointIdMismatch,

    #[error("canonical sync v2 session checkpoint head has {actual} bytes; expected {expected}")]
    CanonicalSyncV2SessionCheckpointHeadLength { actual: usize, expected: usize },

    #[error("canonical sync v2 session checkpoint head uses unsupported version {0}")]
    CanonicalSyncV2SessionCheckpointHeadVersion(u8),

    #[error("canonical sync v2 session checkpoint head sequence must be positive")]
    CanonicalSyncV2SessionCheckpointHeadSequence,

    #[error("canonical sync v2 session checkpoint head points to a missing immutable record")]
    CanonicalSyncV2SessionCheckpointHeadMissingRecord,

    #[error("canonical sync v2 session checkpoint record does not match its key or head")]
    CanonicalSyncV2SessionCheckpointRecordMismatch,

    #[error("canonical sync v2 session checkpoint head conflicts with immutable session state")]
    CanonicalSyncV2SessionCheckpointHeadConflict,

    #[error("canonical sync v2 session checkpoint conflicts with immutable signed bytes")]
    CanonicalSyncV2SessionCheckpointConflict,

    #[error("canonical sync v2 session checkpoint record exists without an atomic head")]
    CanonicalSyncV2SessionCheckpointOrphanRecord,

    #[error("immutable read snapshot contains an unsafe or unsupported path: {0}")]
    InvalidImmutableSnapshotPath(String),

    #[error("immutable read snapshot contains duplicate path: {0}")]
    DuplicateImmutableSnapshotPath(String),

    #[error("stored event at {path} is invalid")]
    InvalidStoredEvent {
        path: PathBuf,
        #[source]
        source: ProtocolError,
    },

    #[error("event {event_id} conflicts with immutable file {path}")]
    ImmutableEventConflict { path: PathBuf, event_id: EventId },

    #[error("event authorization for {event_id} conflicts with immutable file {path}")]
    ImmutableEventAuthorizationConflict { path: PathBuf, event_id: EventId },

    #[error("event authorization for {event_id} is missing at {path}")]
    EventAuthorizationMissing { path: PathBuf, event_id: EventId },

    #[error("local text projection for {event_id} is missing at {path}")]
    LocalTextProjectionMissing { path: PathBuf, event_id: EventId },

    #[error("stored local text projection at {path} is invalid")]
    InvalidStoredLocalTextProjection {
        path: PathBuf,
        #[source]
        source: ProtocolError,
    },

    #[error("local text projection for {event_id} conflicts with immutable file {path}")]
    ImmutableLocalTextProjectionConflict { path: PathBuf, event_id: EventId },

    #[error("stored local text projection {event_id} does not match its filename at {path}")]
    LocalTextProjectionFileNameMismatch { path: PathBuf, event_id: EventId },

    #[error("local text projection plaintext conflicts with event {event_id}")]
    LocalTextProjectionPlaintextConflict { event_id: EventId },

    #[error("command-local event overlay conflicts with event {event_id}")]
    CommandOverlayEventConflict { event_id: EventId },

    #[error("command-local local-projection overlay conflicts with event {event_id}")]
    CommandOverlayLocalProjectionConflict { event_id: EventId },

    #[error("command-local {0} overlay lock is poisoned")]
    CommandOverlayLockPoisoned(&'static str),

    #[error("stored event authorization at {path} is invalid")]
    InvalidStoredAuthorization {
        path: PathBuf,
        #[source]
        source: ProtocolError,
    },

    #[error("stored authorization at {path} does not match event {event_id}")]
    EventAuthorizationMismatch { path: PathBuf, event_id: EventId },

    #[error(
        "conversation {conversation_id} does not match membership {membership_conversation_id}"
    )]
    ConversationMembershipMismatch {
        conversation_id: ConversationId,
        membership_conversation_id: ConversationScopeId,
    },

    #[error(
        "device {author_device_id} sequence {author_sequence} already belongs to event {existing_event_id}; rejected {rejected_event_id}"
    )]
    WriterSequenceConflict {
        author_device_id: DeviceId,
        author_sequence: u64,
        existing_event_id: EventId,
        rejected_event_id: EventId,
    },

    #[error(
        "conversation {conversation_id} contains duplicate device {author_device_id} sequence {author_sequence}"
    )]
    DuplicateWriterSequence {
        conversation_id: ConversationId,
        author_device_id: DeviceId,
        author_sequence: u64,
    },

    #[error("requested event {event_id} is missing from conversation {conversation_id}")]
    RequestedEventMissing {
        conversation_id: ConversationId,
        event_id: EventId,
    },

    #[error("stored event {event_id} does not match its filename at {path}")]
    EventFileNameMismatch { path: PathBuf, event_id: EventId },

    #[error(
        "event at {path} belongs to conversation {actual}, not directory conversation {expected}"
    )]
    ConversationDirectoryMismatch {
        path: PathBuf,
        expected: ConversationId,
        actual: ConversationId,
    },
}

#[cfg(test)]
mod tests {
    use kilogram_protocol::RatchetRecipient;
    use serde_json::Value;
    use std::error::Error;

    use kilogram_identity::{
        AccountRootState, DeviceCapability, DeviceEncryptionIdentity, DeviceIdentity,
    };
    use kilogram_protocol::{
        AuthorizedEvent, EventPayload, LocalTextProjection, SyncSessionBinding,
        canonical_sync_session_v2::{
            CANONICAL_SYNC_SESSION_PROTOCOL_IDENTIFIER_V2, CanonicalSyncSessionCheckpointContentV2,
            sync_session_checkpoint_signing_bytes_v2,
        },
    };
    use kilogram_ratchet::RatchetState;
    use tempfile::{TempDir, tempdir};

    use super::*;

    fn canonical_sync_v2_carrier(
        pointer: &str,
    ) -> Result<CanonicalSyncEventCarrierV2, Box<dyn Error>> {
        let document: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-SYNC-HISTORY-V2-VECTORS.json"
        )))?;
        let hex = document
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing carrier vector"))?;
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(CanonicalSyncEventCarrierV2::decode_and_verify(&bytes)?)
    }

    fn canonical_sync_v2_legacy_pair()
    -> Result<(CanonicalSyncEventCarrierV2, CanonicalSyncEventCarrierV2), Box<dyn Error>> {
        let root_directory = tempdir()?;
        let root = AccountRootState::create(root_directory.path())?;
        let identity = DeviceIdentity::generate()?;
        let encryption = DeviceEncryptionIdentity::generate()?;
        let certificate = root.issue_device_certificate(
            identity.device_id(),
            encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let authority_snapshot = root.authority_snapshot()?;
        let conversation_id = ConversationId::from_label("canonical-sync-v2-atomic-batch");
        let first = AuthorizedEvent::new(
            sign_test_text(&identity, conversation_id, 0, Vec::new(), "first")?,
            certificate.clone(),
            authority_snapshot.clone(),
        )?;
        let first_id = first.event().event_id()?;
        let second = AuthorizedEvent::new(
            sign_test_text(&identity, conversation_id, 1, vec![first_id], "second")?,
            certificate,
            authority_snapshot,
        )?;
        Ok((
            CanonicalSyncEventCarrierV2::LegacyPostcardV6(
                kilogram_protocol::canonical_sync_legacy_event_v1::CanonicalSyncLegacyEventBridgeV1::from_active_event(&first)?,
            ),
            CanonicalSyncEventCarrierV2::LegacyPostcardV6(
                kilogram_protocol::canonical_sync_legacy_event_v1::CanonicalSyncLegacyEventBridgeV1::from_active_event(&second)?,
            ),
        ))
    }

    struct LegacyImportFixture {
        _source_directory: TempDir,
        target_directory: TempDir,
        _root_directory: TempDir,
        source: EventStore,
        target: CanonicalSyncV2CarrierStore,
        root: AccountRootState,
        membership: ConversationMembershipSnapshot,
        events: Vec<AuthorizedEvent>,
    }

    fn legacy_import_fixture(
        label: &str,
        event_count: u64,
    ) -> Result<LegacyImportFixture, Box<dyn Error>> {
        let source_directory = tempdir()?;
        let target_directory = tempdir()?;
        let root_directory = tempdir()?;
        let source = EventStore::open(source_directory.path())?;
        let target = CanonicalSyncV2CarrierStore::open(target_directory.path())?;
        let root = AccountRootState::create(root_directory.path())?;
        let identity = DeviceIdentity::generate()?;
        let encryption = DeviceEncryptionIdentity::generate()?;
        let certificate = root.issue_device_certificate(
            identity.device_id(),
            encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let authority_snapshot = root.authority_snapshot()?;
        let conversation_id = ConversationId::from_label(label);
        let membership = root.create_conversation_membership(conversation_id.scope_id(), &[])?;
        let mut events: Vec<AuthorizedEvent> = Vec::new();
        for sequence in 0..event_count {
            let parents = events
                .last()
                .map(|event| event.event().event_id())
                .transpose()?
                .into_iter()
                .collect();
            events.push(AuthorizedEvent::new(
                sign_test_text(
                    &identity,
                    conversation_id,
                    sequence,
                    parents,
                    &format!("legacy import event {sequence}"),
                )?,
                certificate.clone(),
                authority_snapshot.clone(),
            )?);
        }
        source.put_authorized_batch(&events, &membership)?;
        Ok(LegacyImportFixture {
            _source_directory: source_directory,
            target_directory,
            _root_directory: root_directory,
            source,
            target,
            root,
            membership,
            events,
        })
    }

    fn signed_legacy_import_session_bootstrap(
        plan: &CanonicalSyncV2LegacyImportPlan,
        identity: &DeviceIdentity,
        peer: DeviceId,
        session_binding: SyncSessionBinding,
    ) -> Result<CanonicalSignedSyncSessionCheckpointV2, Box<dyn Error>> {
        let content = CanonicalSyncSessionCheckpointContentV2 {
            protocol_identifier: *CANONICAL_SYNC_SESSION_PROTOCOL_IDENTIFIER_V2,
            conversation_id: *plan.conversation_id().as_bytes(),
            local_device_id: *identity.device_id().as_bytes(),
            peer_device_id: *peer.as_bytes(),
            session_binding: *session_binding.as_bytes(),
            checkpoint_sequence: 1,
            sync_round: 1,
            predecessor_checkpoint_id: None,
            received_event_keys: Vec::new(),
            sent_event_keys: Vec::new(),
            local_more_available: false,
            peer_more_available: false,
            next_action: CanonicalSyncNextActionV2::SendInventory,
            paused: false,
        };
        Ok(CanonicalSignedSyncSessionCheckpointV2 {
            signature: identity.sign(&sync_session_checkpoint_signing_bytes_v2(&content)?),
            content,
        })
    }

    fn overwrite_canonical_sync_v2_raw(
        store: &CanonicalSyncV2CarrierStore,
        conversation_id: [u8; 32],
        event_key: CanonicalSyncEventKeyV2,
        encoded: &[u8],
    ) -> Result<(), StoreError> {
        let mut write = store
            .database
            .begin_write()
            .map_err(canonical_sync_v2_database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(canonical_sync_v2_database_error)?;
        {
            let mut table = write
                .open_table(CANONICAL_SYNC_V2_CARRIER_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            let storage_key = canonical_sync_v2_storage_key(conversation_id, event_key);
            table
                .insert(storage_key.as_slice(), encoded)
                .map_err(canonical_sync_v2_database_error)?;
        }
        write.commit().map_err(canonical_sync_v2_database_error)
    }

    fn canonical_sync_v2_raw_exists(
        store: &CanonicalSyncV2CarrierStore,
        conversation_id: [u8; 32],
        event_key: CanonicalSyncEventKeyV2,
    ) -> Result<bool, StoreError> {
        let read = store
            .database
            .begin_read()
            .map_err(canonical_sync_v2_database_error)?;
        let table = read
            .open_table(CANONICAL_SYNC_V2_CARRIER_TABLE)
            .map_err(canonical_sync_v2_database_error)?;
        let storage_key = canonical_sync_v2_storage_key(conversation_id, event_key);
        table
            .get(storage_key.as_slice())
            .map(|value| value.is_some())
            .map_err(canonical_sync_v2_database_error)
    }

    fn overwrite_canonical_sync_v2_legacy_import_checkpoint_raw(
        store: &CanonicalSyncV2CarrierStore,
        plan: &CanonicalSyncV2LegacyImportPlan,
        encoded: &[u8],
    ) -> Result<(), StoreError> {
        let mut write = store
            .database
            .begin_write()
            .map_err(canonical_sync_v2_database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(canonical_sync_v2_database_error)?;
        {
            let mut table = write
                .open_table(CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            let key = canonical_sync_v2_legacy_import_checkpoint_key(plan);
            table
                .insert(key.as_slice(), encoded)
                .map_err(canonical_sync_v2_database_error)?;
        }
        write.commit().map_err(canonical_sync_v2_database_error)
    }

    fn overwrite_canonical_sync_v2_session_head_raw(
        store: &CanonicalSyncV2CarrierStore,
        identity_key: [u8; CANONICAL_SYNC_V2_SESSION_IDENTITY_KEY_BYTES],
        encoded: &[u8],
    ) -> Result<(), StoreError> {
        let mut write = store
            .database
            .begin_write()
            .map_err(canonical_sync_v2_database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(canonical_sync_v2_database_error)?;
        {
            let mut table = write
                .open_table(CANONICAL_SYNC_V2_SESSION_CHECKPOINT_HEAD_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            table
                .insert(identity_key.as_slice(), encoded)
                .map_err(canonical_sync_v2_database_error)?;
        }
        write.commit().map_err(canonical_sync_v2_database_error)
    }

    fn overwrite_canonical_sync_v2_session_record_raw(
        store: &CanonicalSyncV2CarrierStore,
        record_key: [u8; CANONICAL_SYNC_V2_SESSION_CHECKPOINT_KEY_BYTES],
        encoded: &[u8],
    ) -> Result<(), StoreError> {
        let mut write = store
            .database
            .begin_write()
            .map_err(canonical_sync_v2_database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(canonical_sync_v2_database_error)?;
        {
            let mut table = write
                .open_table(CANONICAL_SYNC_V2_SESSION_CHECKPOINT_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            table
                .insert(record_key.as_slice(), encoded)
                .map_err(canonical_sync_v2_database_error)?;
        }
        write.commit().map_err(canonical_sync_v2_database_error)
    }

    #[test]
    fn canonical_sync_v2_store_persists_both_namespaces_across_reopen() -> Result<(), Box<dyn Error>>
    {
        let directory = tempdir()?;
        let legacy = canonical_sync_v2_carrier("/expected/legacy_carrier_cbor_hex")?;
        let canonical = canonical_sync_v2_carrier("/expected/canonical_carrier_cbor_hex")?;
        let legacy_conversation_id = *legacy.conversation_id();
        let canonical_conversation_id = *canonical.conversation_id();
        let legacy_key = legacy.key()?;
        let canonical_key = canonical.key()?;
        let store = CanonicalSyncV2CarrierStore::open(directory.path())?;

        store.put_batch(legacy_conversation_id, std::slice::from_ref(&legacy))?;
        store.put_batch(canonical_conversation_id, std::slice::from_ref(&canonical))?;
        store.put_batch(legacy_conversation_id, std::slice::from_ref(&legacy))?;
        assert_eq!(store.inventory(legacy_conversation_id)?, vec![legacy_key]);
        assert_eq!(
            store.inventory(canonical_conversation_id)?,
            vec![canonical_key]
        );

        drop(store);
        let reopened = CanonicalSyncV2CarrierStore::open(directory.path())?;
        assert_eq!(
            reopened.events_by_key(legacy_conversation_id, &[legacy_key])?,
            vec![legacy]
        );
        assert_eq!(
            reopened.events_by_key(canonical_conversation_id, &[canonical_key])?,
            vec![canonical]
        );
        Ok(())
    }

    #[test]
    fn canonical_sync_v2_store_conflict_aborts_the_whole_batch() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let (first, second) = canonical_sync_v2_legacy_pair()?;
        let conversation_id = *first.conversation_id();
        let first_key = first.key()?;
        let second_key = second.key()?;
        let store = CanonicalSyncV2CarrierStore::open(directory.path())?;
        overwrite_canonical_sync_v2_raw(&store, conversation_id, first_key, &second.encode()?)?;

        assert!(matches!(
            store.put_batch(conversation_id, &[first, second]),
            Err(StoreError::CanonicalSyncV2StoredCarrierKeyMismatch { .. })
        ));
        assert!(!canonical_sync_v2_raw_exists(
            &store,
            conversation_id,
            second_key
        )?);
        Ok(())
    }

    #[test]
    fn canonical_sync_v2_store_rejects_corruption_and_cross_conversation_writes()
    -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let legacy = canonical_sync_v2_carrier("/expected/legacy_carrier_cbor_hex")?;
        let conversation_id = *legacy.conversation_id();
        let legacy_key = legacy.key()?;
        let store = CanonicalSyncV2CarrierStore::open(directory.path())?;

        assert!(matches!(
            store.put_batch([0x99; 32], std::slice::from_ref(&legacy)),
            Err(StoreError::CanonicalSyncV2(
                CanonicalSyncV2Error::EventConversationMismatch
            ))
        ));
        assert!(store.inventory([0x99; 32])?.is_empty());

        store.put_batch(conversation_id, &[legacy])?;
        overwrite_canonical_sync_v2_raw(&store, conversation_id, legacy_key, &[0x80])?;
        assert!(matches!(
            store.inventory(conversation_id),
            Err(StoreError::CanonicalSyncV2(_))
        ));
        Ok(())
    }

    #[test]
    fn canonical_sync_v2_legacy_import_is_bounded_resumable_and_idempotent()
    -> Result<(), Box<dyn Error>> {
        let fixture = legacy_import_fixture("canonical-sync-v2-legacy-import", 3)?;
        let conversation_id = fixture.events[0].event().conversation_id();
        let plan = fixture
            .source
            .canonical_sync_v2_legacy_import_plan(conversation_id, &fixture.membership)?;
        assert_eq!(plan.conversation_id(), conversation_id);
        assert_eq!(plan.membership_revision(), fixture.membership.revision());
        assert_eq!(plan.event_count(), 3);

        let start = plan.start_cursor();
        let first = fixture.target.import_legacy_page(
            &fixture.source,
            &fixture.membership,
            &plan,
            start,
            2,
        )?;
        assert_eq!(first.imported_event_keys().len(), 2);
        assert_eq!(first.total_event_count(), 3);
        assert!(!first.is_complete());
        assert_eq!(
            fixture.target.inventory(*conversation_id.as_bytes())?.len(),
            2
        );
        let first_checkpoint = fixture
            .target
            .legacy_import_checkpoint(&plan)?
            .ok_or("legacy import checkpoint missing after first page")?;
        assert_eq!(
            first_checkpoint.conversation_id(),
            *conversation_id.as_bytes()
        );
        assert_eq!(first_checkpoint.plan_id(), plan.plan_id());
        assert_eq!(
            first_checkpoint.membership_revision(),
            fixture.membership.revision()
        );
        assert_eq!(first_checkpoint.total_event_count(), 3);
        assert_eq!(first_checkpoint.last_page_start(), 0);
        assert_eq!(first_checkpoint.next_index(), 2);
        assert_eq!(first_checkpoint.cursor(), first.cursor());
        assert!(!first_checkpoint.is_complete());

        let retry = fixture.target.import_legacy_page(
            &fixture.source,
            &fixture.membership,
            &plan,
            start,
            2,
        )?;
        assert_eq!(retry, first);
        assert_eq!(
            fixture.target.inventory(*conversation_id.as_bytes())?.len(),
            2
        );

        let completed = fixture.target.import_legacy_page(
            &fixture.source,
            &fixture.membership,
            &plan,
            first.cursor(),
            2,
        )?;
        assert_eq!(completed.imported_event_keys().len(), 1);
        assert_eq!(completed.cursor().next_index(), 3);
        assert!(completed.is_complete());
        assert_eq!(
            fixture.target.inventory(*conversation_id.as_bytes())?.len(),
            3
        );
        let completed_checkpoint = fixture
            .target
            .legacy_import_checkpoint(&plan)?
            .ok_or("legacy import checkpoint missing after completion")?;
        assert_eq!(completed_checkpoint.next_index(), 3);
        assert_eq!(completed_checkpoint.last_page_start(), 2);
        assert!(completed_checkpoint.is_complete());

        let complete_retry = fixture.target.import_legacy_page(
            &fixture.source,
            &fixture.membership,
            &plan,
            completed.cursor(),
            2,
        )?;
        assert!(complete_retry.imported_event_keys().is_empty());
        assert!(complete_retry.is_complete());
        Ok(())
    }

    #[test]
    fn canonical_sync_v2_legacy_import_rejects_skips_and_changed_claims()
    -> Result<(), Box<dyn Error>> {
        let fixture = legacy_import_fixture("canonical-sync-v2-legacy-import-reject", 2)?;
        let conversation_id = fixture.events[0].event().conversation_id();
        let plan = fixture
            .source
            .canonical_sync_v2_legacy_import_plan(conversation_id, &fixture.membership)?;

        let skipped = plan.resume_cursor(1)?;
        assert!(matches!(
            fixture.target.import_legacy_page(
                &fixture.source,
                &fixture.membership,
                &plan,
                skipped,
                1,
            ),
            Err(StoreError::CanonicalSyncV2CarrierMissing { .. })
        ));
        let first_authorized = fixture.source.authorized_events_by_id(
            conversation_id,
            &[plan.items[0].event_id],
            &fixture.membership,
        )?;
        let first_carrier = CanonicalSyncEventCarrierV2::LegacyPostcardV6(
            CanonicalSyncLegacyEventBridgeV1::from_active_event(&first_authorized[0])?,
        );
        fixture
            .target
            .put_batch(*conversation_id.as_bytes(), &[first_carrier])?;
        assert!(matches!(
            fixture.target.import_legacy_page(
                &fixture.source,
                &fixture.membership,
                &plan,
                skipped,
                1,
            ),
            Err(StoreError::CanonicalSyncV2LegacyImportCheckpointMissing {
                requested_next_index: 1
            })
        ));
        assert!(matches!(
            plan.resume_cursor(3),
            Err(StoreError::CanonicalSyncV2LegacyImportCursorOutOfRange { .. })
        ));
        assert!(matches!(
            fixture.target.import_legacy_page(
                &fixture.source,
                &fixture.membership,
                &plan,
                plan.start_cursor(),
                0,
            ),
            Err(StoreError::CanonicalSyncV2LegacyImportPageSize { .. })
        ));

        let changed_membership = fixture.root.add_conversation_members(
            conversation_id.scope_id(),
            &[kilogram_identity::AccountId::from_bytes([0x77; 32])],
        )?;
        assert!(matches!(
            fixture.target.import_legacy_page(
                &fixture.source,
                &changed_membership,
                &plan,
                plan.start_cursor(),
                1,
            ),
            Err(StoreError::CanonicalSyncV2LegacyImportMembershipMismatch)
        ));

        let mut changed_source_plan = plan.clone();
        changed_source_plan.items[0].authorized_event_digest[0] ^= 1;
        changed_source_plan.plan_id = canonical_sync_v2_legacy_import_plan_id(
            changed_source_plan.conversation_id,
            changed_source_plan.membership_revision,
            changed_source_plan.membership_digest,
            &changed_source_plan.items,
        );
        assert!(matches!(
            fixture.target.import_legacy_page(
                &fixture.source,
                &fixture.membership,
                &changed_source_plan,
                changed_source_plan.start_cursor(),
                1,
            ),
            Err(StoreError::CanonicalSyncV2LegacyImportSourceEventChanged { .. })
        ));

        let wrong_plan_cursor = CanonicalSyncV2LegacyImportCursor {
            plan_id: [0x55; 32],
            next_index: 0,
        };
        assert!(matches!(
            fixture.target.import_legacy_page(
                &fixture.source,
                &fixture.membership,
                &plan,
                wrong_plan_cursor,
                1,
            ),
            Err(StoreError::CanonicalSyncV2LegacyImportCursorPlanMismatch { .. })
        ));
        Ok(())
    }

    #[test]
    fn canonical_sync_v2_legacy_import_conflict_does_not_commit_later_page_items()
    -> Result<(), Box<dyn Error>> {
        let fixture = legacy_import_fixture("canonical-sync-v2-legacy-import-atomic", 3)?;
        let conversation_id = fixture.events[0].event().conversation_id();
        let plan = fixture
            .source
            .canonical_sync_v2_legacy_import_plan(conversation_id, &fixture.membership)?;
        let first = fixture.target.import_legacy_page(
            &fixture.source,
            &fixture.membership,
            &plan,
            plan.start_cursor(),
            1,
        )?;
        let second_key = CanonicalSyncEventKeyV2::new(
            CanonicalSyncEventNamespaceV2::LegacyPostcardV6,
            *plan.items[1].event_id.as_bytes(),
        );
        let third_key = CanonicalSyncEventKeyV2::new(
            CanonicalSyncEventNamespaceV2::LegacyPostcardV6,
            *plan.items[2].event_id.as_bytes(),
        );
        let mut conflicting_events = fixture.source.authorized_events_by_id(
            conversation_id,
            &[plan.items[2].event_id],
            &fixture.membership,
        )?;
        let Some(conflicting_event) = conflicting_events.pop() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "legacy import conflict fixture is missing its third event",
            )
            .into());
        };
        let conflicting_carrier = CanonicalSyncEventCarrierV2::LegacyPostcardV6(
            CanonicalSyncLegacyEventBridgeV1::from_active_event(&conflicting_event)?,
        );
        overwrite_canonical_sync_v2_raw(
            &fixture.target,
            *conversation_id.as_bytes(),
            second_key,
            &conflicting_carrier.encode()?,
        )?;

        assert!(matches!(
            fixture.target.import_legacy_page(
                &fixture.source,
                &fixture.membership,
                &plan,
                first.cursor(),
                2,
            ),
            Err(StoreError::CanonicalSyncV2StoredCarrierKeyMismatch { .. })
        ));
        assert!(!canonical_sync_v2_raw_exists(
            &fixture.target,
            *conversation_id.as_bytes(),
            third_key,
        )?);
        let checkpoint = fixture
            .target
            .legacy_import_checkpoint(&plan)?
            .ok_or("legacy import checkpoint disappeared after page conflict")?;
        assert_eq!(checkpoint.next_index(), 1);
        Ok(())
    }

    #[test]
    fn canonical_sync_v2_legacy_import_checkpoint_survives_reopen_and_rejects_stale_pages()
    -> Result<(), Box<dyn Error>> {
        let fixture = legacy_import_fixture("canonical-sync-v2-import-checkpoint-reopen", 3)?;
        let conversation_id = fixture.events[0].event().conversation_id();
        let plan = fixture
            .source
            .canonical_sync_v2_legacy_import_plan(conversation_id, &fixture.membership)?;
        let first = fixture.target.import_legacy_page(
            &fixture.source,
            &fixture.membership,
            &plan,
            plan.start_cursor(),
            2,
        )?;

        let LegacyImportFixture {
            _source_directory,
            target_directory,
            _root_directory,
            source,
            target,
            root: _,
            membership,
            events: _,
        } = fixture;
        drop(target);
        let reopened = CanonicalSyncV2CarrierStore::open(target_directory.path())?;
        let checkpoint = reopened
            .legacy_import_checkpoint(&plan)?
            .ok_or("legacy import checkpoint missing after reopen")?;
        assert_eq!(checkpoint.cursor(), first.cursor());
        assert_eq!(checkpoint.last_page_start(), 0);

        assert!(matches!(
            reopened.import_legacy_page(&source, &membership, &plan, plan.start_cursor(), 1,),
            Err(
                StoreError::CanonicalSyncV2LegacyImportDurableCursorMismatch {
                    requested_next_index: 0,
                    requested_page_end: 1,
                    durable_last_page_start: 0,
                    durable_next_index: 2
                }
            )
        ));
        let alternate_page_shape = plan.resume_cursor(1)?;
        assert!(matches!(
            reopened.import_legacy_page(&source, &membership, &plan, alternate_page_shape, 1,),
            Err(
                StoreError::CanonicalSyncV2LegacyImportDurableCursorMismatch {
                    requested_next_index: 1,
                    requested_page_end: 2,
                    durable_last_page_start: 0,
                    durable_next_index: 2
                }
            )
        ));

        let completed =
            reopened.import_legacy_page(&source, &membership, &plan, checkpoint.cursor(), 2)?;
        assert!(completed.is_complete());
        assert_eq!(completed.cursor().next_index(), 3);
        Ok(())
    }

    #[test]
    fn canonical_sync_v2_legacy_import_checkpoint_corruption_fails_closed()
    -> Result<(), Box<dyn Error>> {
        let fixture = legacy_import_fixture("canonical-sync-v2-import-checkpoint-corrupt", 1)?;
        let conversation_id = fixture.events[0].event().conversation_id();
        let plan = fixture
            .source
            .canonical_sync_v2_legacy_import_plan(conversation_id, &fixture.membership)?;

        overwrite_canonical_sync_v2_legacy_import_checkpoint_raw(
            &fixture.target,
            &plan,
            &[CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_VERSION],
        )?;
        assert!(matches!(
            fixture.target.legacy_import_checkpoint(&plan),
            Err(StoreError::CanonicalSyncV2LegacyImportCheckpointInvalidLength { .. })
        ));

        let mut wrong_plan = CanonicalSyncV2LegacyImportCheckpoint::for_plan(&plan, 0, 0);
        wrong_plan.plan_id[0] ^= 1;
        overwrite_canonical_sync_v2_legacy_import_checkpoint_raw(
            &fixture.target,
            &plan,
            &encode_canonical_sync_v2_legacy_import_checkpoint(wrong_plan),
        )?;
        assert!(matches!(
            fixture.target.legacy_import_checkpoint(&plan),
            Err(StoreError::CanonicalSyncV2LegacyImportCheckpointPlanMismatch)
        ));

        let invalid_page_bounds = CanonicalSyncV2LegacyImportCheckpoint::for_plan(&plan, 1, 0);
        overwrite_canonical_sync_v2_legacy_import_checkpoint_raw(
            &fixture.target,
            &plan,
            &encode_canonical_sync_v2_legacy_import_checkpoint(invalid_page_bounds),
        )?;
        assert!(matches!(
            fixture.target.legacy_import_checkpoint(&plan),
            Err(
                StoreError::CanonicalSyncV2LegacyImportCheckpointIndexOutOfRange {
                    last_page_start: 1,
                    next_index: 0,
                    event_count: 1
                }
            )
        ));
        Ok(())
    }

    #[test]
    fn canonical_sync_v2_session_bootstrap_requires_complete_import_and_survives_reopen()
    -> Result<(), Box<dyn Error>> {
        let fixture = legacy_import_fixture("canonical-sync-v2-session-bootstrap", 2)?;
        let conversation_id = fixture.events[0].event().conversation_id();
        let plan = fixture
            .source
            .canonical_sync_v2_legacy_import_plan(conversation_id, &fixture.membership)?;
        let identity = DeviceIdentity::from_secret_bytes([0x31; 32]);
        let peer = DeviceIdentity::from_secret_bytes([0x51; 32]);
        let session_binding = SyncSessionBinding::from_transport_label("canonical-v2-bootstrap");
        let checkpoint = signed_legacy_import_session_bootstrap(
            &plan,
            &identity,
            peer.device_id(),
            session_binding,
        )?;

        assert!(matches!(
            fixture
                .target
                .install_legacy_import_session_bootstrap(&plan, &checkpoint),
            Err(StoreError::CanonicalSyncV2SessionBootstrapImportMissing)
        ));
        let first = fixture.target.import_legacy_page(
            &fixture.source,
            &fixture.membership,
            &plan,
            plan.start_cursor(),
            1,
        )?;
        assert!(matches!(
            fixture
                .target
                .install_legacy_import_session_bootstrap(&plan, &checkpoint),
            Err(
                StoreError::CanonicalSyncV2SessionBootstrapImportIncomplete {
                    next_index: 1,
                    event_count: 2
                }
            )
        ));
        fixture.target.import_legacy_page(
            &fixture.source,
            &fixture.membership,
            &plan,
            first.cursor(),
            1,
        )?;

        let stored = fixture
            .target
            .install_legacy_import_session_bootstrap(&plan, &checkpoint)?;
        assert_eq!(stored.import_plan_id(), plan.plan_id());
        assert_eq!(stored.checkpoint(), &checkpoint);
        assert_eq!(
            stored.checkpoint_id(),
            sync_session_checkpoint_id_v2(&checkpoint)?
        );
        let head = fixture
            .target
            .session_checkpoint_head(
                *conversation_id.as_bytes(),
                *identity.device_id().as_bytes(),
                *peer.device_id().as_bytes(),
                *session_binding.as_bytes(),
            )?
            .ok_or("session bootstrap head missing")?;
        assert_eq!(head, stored);
        assert_eq!(
            fixture
                .target
                .install_legacy_import_session_bootstrap(&plan, &checkpoint)?,
            stored
        );

        let LegacyImportFixture {
            _source_directory,
            target_directory,
            _root_directory,
            source: _,
            target,
            root: _,
            membership: _,
            events: _,
        } = fixture;
        drop(target);
        let reopened = CanonicalSyncV2CarrierStore::open(target_directory.path())?;
        assert_eq!(
            reopened.session_checkpoint_head(
                *conversation_id.as_bytes(),
                *identity.device_id().as_bytes(),
                *peer.device_id().as_bytes(),
                *session_binding.as_bytes(),
            )?,
            Some(stored)
        );
        Ok(())
    }

    #[test]
    fn canonical_sync_v2_session_bootstrap_rejects_wrong_shape_and_conversation()
    -> Result<(), Box<dyn Error>> {
        let fixture = legacy_import_fixture("canonical-sync-v2-session-bootstrap-reject", 1)?;
        let conversation_id = fixture.events[0].event().conversation_id();
        let plan = fixture
            .source
            .canonical_sync_v2_legacy_import_plan(conversation_id, &fixture.membership)?;
        fixture.target.import_legacy_page(
            &fixture.source,
            &fixture.membership,
            &plan,
            plan.start_cursor(),
            1,
        )?;
        let identity = DeviceIdentity::from_secret_bytes([0x32; 32]);
        let peer = DeviceIdentity::from_secret_bytes([0x52; 32]);
        let session_binding =
            SyncSessionBinding::from_transport_label("canonical-v2-bootstrap-reject");
        let checkpoint = signed_legacy_import_session_bootstrap(
            &plan,
            &identity,
            peer.device_id(),
            session_binding,
        )?;

        let mut wrong_shape = checkpoint.clone();
        wrong_shape.content.received_event_keys =
            fixture.target.inventory(*conversation_id.as_bytes())?;
        wrong_shape.signature = identity.sign(&sync_session_checkpoint_signing_bytes_v2(
            &wrong_shape.content,
        )?);
        assert!(matches!(
            fixture
                .target
                .install_legacy_import_session_bootstrap(&plan, &wrong_shape),
            Err(StoreError::CanonicalSyncV2SessionBootstrapShape)
        ));

        let mut wrong_conversation = checkpoint;
        wrong_conversation.content.conversation_id = [0x99; 32];
        wrong_conversation.signature = identity.sign(&sync_session_checkpoint_signing_bytes_v2(
            &wrong_conversation.content,
        )?);
        assert!(matches!(
            fixture
                .target
                .install_legacy_import_session_bootstrap(&plan, &wrong_conversation),
            Err(StoreError::CanonicalSyncV2SessionBootstrapConversationMismatch)
        ));
        Ok(())
    }

    #[test]
    fn canonical_sync_v2_session_checkpoint_corruption_and_orphans_fail_closed()
    -> Result<(), Box<dyn Error>> {
        {
            let fixture = legacy_import_fixture("canonical-sync-v2-session-head-corrupt", 1)?;
            let conversation_id = fixture.events[0].event().conversation_id();
            let plan = fixture
                .source
                .canonical_sync_v2_legacy_import_plan(conversation_id, &fixture.membership)?;
            fixture.target.import_legacy_page(
                &fixture.source,
                &fixture.membership,
                &plan,
                plan.start_cursor(),
                1,
            )?;
            let identity = DeviceIdentity::from_secret_bytes([0x33; 32]);
            let peer = DeviceIdentity::from_secret_bytes([0x53; 32]);
            let session_binding =
                SyncSessionBinding::from_transport_label("canonical-v2-bootstrap-head-corrupt");
            let checkpoint = signed_legacy_import_session_bootstrap(
                &plan,
                &identity,
                peer.device_id(),
                session_binding,
            )?;
            fixture
                .target
                .install_legacy_import_session_bootstrap(&plan, &checkpoint)?;
            let identity_key = canonical_sync_v2_session_identity_key(
                *conversation_id.as_bytes(),
                *identity.device_id().as_bytes(),
                *peer.device_id().as_bytes(),
                *session_binding.as_bytes(),
            );
            overwrite_canonical_sync_v2_session_head_raw(
                &fixture.target,
                identity_key,
                &[CANONICAL_SYNC_V2_SESSION_CHECKPOINT_HEAD_VERSION],
            )?;
            assert!(matches!(
                fixture.target.session_checkpoint_head(
                    *conversation_id.as_bytes(),
                    *identity.device_id().as_bytes(),
                    *peer.device_id().as_bytes(),
                    *session_binding.as_bytes(),
                ),
                Err(StoreError::CanonicalSyncV2SessionCheckpointHeadLength { .. })
            ));
        }

        {
            let fixture = legacy_import_fixture("canonical-sync-v2-session-record-corrupt", 1)?;
            let conversation_id = fixture.events[0].event().conversation_id();
            let plan = fixture
                .source
                .canonical_sync_v2_legacy_import_plan(conversation_id, &fixture.membership)?;
            fixture.target.import_legacy_page(
                &fixture.source,
                &fixture.membership,
                &plan,
                plan.start_cursor(),
                1,
            )?;
            let identity = DeviceIdentity::from_secret_bytes([0x34; 32]);
            let peer = DeviceIdentity::from_secret_bytes([0x54; 32]);
            let session_binding =
                SyncSessionBinding::from_transport_label("canonical-v2-bootstrap-record-corrupt");
            let checkpoint = signed_legacy_import_session_bootstrap(
                &plan,
                &identity,
                peer.device_id(),
                session_binding,
            )?;
            let stored = fixture
                .target
                .install_legacy_import_session_bootstrap(&plan, &checkpoint)?;
            let identity_key = canonical_sync_v2_session_identity_key(
                *conversation_id.as_bytes(),
                *identity.device_id().as_bytes(),
                *peer.device_id().as_bytes(),
                *session_binding.as_bytes(),
            );
            let record_key = canonical_sync_v2_session_checkpoint_key(identity_key, 1);
            let mut encoded = encode_canonical_sync_v2_session_checkpoint_record(&stored)?;
            let final_byte = encoded
                .last_mut()
                .ok_or("encoded session checkpoint unexpectedly empty")?;
            *final_byte ^= 1;
            overwrite_canonical_sync_v2_session_record_raw(&fixture.target, record_key, &encoded)?;
            assert!(matches!(
                fixture.target.session_checkpoint_head(
                    *conversation_id.as_bytes(),
                    *identity.device_id().as_bytes(),
                    *peer.device_id().as_bytes(),
                    *session_binding.as_bytes(),
                ),
                Err(StoreError::CanonicalSyncV2SessionCheckpoint(_))
            ));
        }

        {
            let fixture = legacy_import_fixture("canonical-sync-v2-session-orphan", 1)?;
            let conversation_id = fixture.events[0].event().conversation_id();
            let plan = fixture
                .source
                .canonical_sync_v2_legacy_import_plan(conversation_id, &fixture.membership)?;
            fixture.target.import_legacy_page(
                &fixture.source,
                &fixture.membership,
                &plan,
                plan.start_cursor(),
                1,
            )?;
            let identity = DeviceIdentity::from_secret_bytes([0x35; 32]);
            let peer = DeviceIdentity::from_secret_bytes([0x55; 32]);
            let session_binding =
                SyncSessionBinding::from_transport_label("canonical-v2-bootstrap-orphan");
            let checkpoint = signed_legacy_import_session_bootstrap(
                &plan,
                &identity,
                peer.device_id(),
                session_binding,
            )?;
            let stored = CanonicalSyncV2StoredSessionCheckpoint {
                import_plan_id: plan.plan_id(),
                checkpoint_id: sync_session_checkpoint_id_v2(&checkpoint)?,
                checkpoint: checkpoint.clone(),
            };
            let identity_key = canonical_sync_v2_session_identity_key(
                *conversation_id.as_bytes(),
                *identity.device_id().as_bytes(),
                *peer.device_id().as_bytes(),
                *session_binding.as_bytes(),
            );
            let record_key = canonical_sync_v2_session_checkpoint_key(identity_key, 1);
            overwrite_canonical_sync_v2_session_record_raw(
                &fixture.target,
                record_key,
                &encode_canonical_sync_v2_session_checkpoint_record(&stored)?,
            )?;
            assert!(matches!(
                fixture
                    .target
                    .install_legacy_import_session_bootstrap(&plan, &checkpoint),
                Err(StoreError::CanonicalSyncV2SessionCheckpointOrphanRecord)
            ));
        }

        Ok(())
    }

    #[test]
    fn event_persists_and_loads() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let store = EventStore::open(directory.path())?;
        let conversation_id = ConversationId::from_label("persistence");
        let event = text_event(conversation_id, 0, Vec::new(), "hello")?;
        let event_id = event.event_id()?;

        assert_eq!(store.put(&event)?, StoreOutcome::Inserted);
        let event_bytes = fs::read(event_path(
            &store.conversation_directory(conversation_id),
            event_id,
        ))?;
        assert!(
            !event_bytes
                .windows(b"hello".len())
                .any(|window| window == b"hello")
        );
        let reopened = EventStore::open(directory.path())?;
        let stored = reopened.load_conversation(conversation_id)?;

        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].id, event_id);
        assert_eq!(stored[0].event, event);
        Ok(())
    }

    #[test]
    fn authorized_event_sidecar_is_required_and_persistent() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let root_directory = tempdir()?;
        let store = EventStore::open(directory.path())?;
        let root = AccountRootState::create(root_directory.path())?;
        let identity = DeviceIdentity::generate()?;
        let encryption = DeviceEncryptionIdentity::generate()?;
        let certificate = root.issue_device_certificate(
            identity.device_id(),
            encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let authority_snapshot = root.authority_snapshot()?;
        let conversation_id = ConversationId::from_label("authorized-persistence");
        let membership = root.create_conversation_membership(conversation_id.scope_id(), &[])?;
        let event = AuthorizedEvent::new(
            sign_test_text(&identity, conversation_id, 0, Vec::new(), "hello")?,
            certificate,
            authority_snapshot,
        )?;
        let event_id = event.event().event_id()?;

        let (outcome, receipt) = store.put_authorized_with_receipt(&event, &membership)?;
        assert_eq!(outcome, StoreOutcome::Inserted);
        assert_eq!(
            receipt.paths(),
            &[
                event_path(&store.conversation_directory(conversation_id), event_id),
                authorization_path(&store.conversation_directory(conversation_id), event_id),
            ]
        );
        let reopened = EventStore::open(directory.path())?;
        let stored = reopened.load_authorized_conversation(conversation_id, &membership)?;
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].event, event);
        let other_conversation = ConversationId::from_label("other-conversation");
        assert!(matches!(
            reopened.load_authorized_conversation(other_conversation, &membership),
            Err(StoreError::ConversationMembershipMismatch {
                conversation_id: actual,
                ..
            }) if actual == other_conversation
        ));

        fs::remove_file(authorization_path(
            &reopened.conversation_directory(conversation_id),
            event_id,
        ))?;
        assert!(matches!(
            reopened.load_authorized_conversation(conversation_id, &membership),
            Err(StoreError::EventAuthorizationMissing {
                event_id: missing,
                ..
            }) if missing == event_id
        ));
        Ok(())
    }

    #[test]
    fn immutable_snapshots_read_verified_events_and_local_projections_without_filesystem_fallback()
    -> Result<(), Box<dyn Error>> {
        let event_directory = tempdir()?;
        let projection_directory = tempdir()?;
        let root_directory = tempdir()?;
        let event_store = EventStore::open(event_directory.path())?;
        let projection_store = LocalMessageStore::open(projection_directory.path())?;
        let root = AccountRootState::create(root_directory.path())?;
        let identity = DeviceIdentity::generate()?;
        let encryption = DeviceEncryptionIdentity::generate()?;
        let certificate = root.issue_device_certificate(
            identity.device_id(),
            encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let conversation_id = ConversationId::from_label("immutable-snapshot-read");
        let membership = root.create_conversation_membership(conversation_id.scope_id(), &[])?;
        let signed = sign_test_text(&identity, conversation_id, 0, Vec::new(), "snapshot")?;
        let projection = LocalTextProjection::seal_authored(
            &signed,
            identity.device_id(),
            encryption.public_key(),
            "snapshot",
        )?;
        let authority_snapshot = root.authority_snapshot()?;
        let authorized =
            AuthorizedEvent::new(signed, certificate.clone(), authority_snapshot.clone())?;
        let event_id = authorized.event().event_id()?;
        event_store.put_authorized(&authorized, &membership)?;
        projection_store.put(&projection)?;
        let expected_events =
            event_store.load_authorized_conversation(conversation_id, &membership)?;
        let expected_frontier = event_store.frontier(conversation_id)?;

        let conversation_directory = event_store.conversation_directory(conversation_id);
        let stored_event_path = event_path(&conversation_directory, event_id);
        let stored_authorization_path = authorization_path(&conversation_directory, event_id);
        let stored_projection_path =
            local_text_projection_path(projection_directory.path(), event_id);
        let event_snapshot = ImmutableEventReadSnapshot::from_records([
            (
                format!("{conversation_id}/{event_id}.{EVENT_FILE_EXTENSION}"),
                fs::read(&stored_event_path)?,
            ),
            (
                format!("{conversation_id}/{event_id}.{AUTHORIZATION_FILE_EXTENSION}"),
                fs::read(&stored_authorization_path)?,
            ),
        ])?;
        let projection_snapshot = ImmutableLocalMessageReadSnapshot::from_records([(
            format!("{event_id}.{LOCAL_TEXT_PROJECTION_FILE_EXTENSION}"),
            fs::read(&stored_projection_path)?,
        )])?;
        fs::remove_file(stored_event_path)?;
        fs::remove_file(stored_authorization_path)?;
        fs::remove_file(stored_projection_path)?;

        assert_eq!(
            event_snapshot.load_authorized_conversation(conversation_id, &membership)?,
            expected_events
        );
        assert_eq!(event_snapshot.frontier(conversation_id)?, expected_frontier);
        assert_eq!(projection_snapshot.get(event_id)?, projection);
        assert!(matches!(
            ImmutableEventReadSnapshot::from_records([(String::from("../event.event"), vec![])]),
            Err(StoreError::InvalidImmutableSnapshotPath(_))
        ));

        let event_overlay = CommandEventReadOverlay::new(Box::new(event_snapshot));
        let projection_overlay = CommandLocalMessageReadOverlay::new(Box::new(projection_snapshot));
        let second_signed =
            sign_test_text(&identity, conversation_id, 1, vec![event_id], "overlay")?;
        let second_projection = LocalTextProjection::seal_authored(
            &second_signed,
            identity.device_id(),
            encryption.public_key(),
            "overlay",
        )?;
        let second = AuthorizedEvent::new(
            second_signed,
            certificate.clone(),
            authority_snapshot.clone(),
        )?;
        let second_id = second.event().event_id()?;
        let staged_events =
            event_overlay.stage_committed(std::slice::from_ref(&second), &membership)?;
        let staged_projections =
            projection_overlay.stage_committed(std::slice::from_ref(&second_projection))?;

        assert_eq!(
            event_overlay.authorized_inventory(conversation_id, &membership)?,
            vec![event_id]
        );
        assert!(matches!(
            projection_overlay.get(second_id),
            Err(StoreError::LocalTextProjectionMissing { event_id, .. })
                if event_id == second_id
        ));

        assert_eq!(event_overlay.commit_staged(staged_events)?, 1);
        assert_eq!(projection_overlay.commit_staged(staged_projections)?, 1);
        assert_eq!(event_overlay.committed_count()?, 1);
        assert_eq!(projection_overlay.committed_count()?, 1);
        assert_eq!(
            event_overlay.authorized_events_by_id(
                conversation_id,
                &[event_id, second_id],
                &membership,
            )?,
            vec![authorized.clone(), second.clone()]
        );
        assert_eq!(event_overlay.frontier(conversation_id)?, vec![second_id]);
        assert_eq!(projection_overlay.get(second_id)?, second_projection);

        let conflicting = AuthorizedEvent::new(
            sign_test_text(&identity, conversation_id, 1, vec![event_id], "conflict")?,
            certificate,
            authority_snapshot,
        )?;
        assert!(matches!(
            event_overlay.stage_committed(&[conflicting], &membership),
            Err(StoreError::WriterSequenceConflict {
                author_sequence: 1,
                existing_event_id,
                ..
            }) if existing_event_id == second_id
        ));
        Ok(())
    }

    #[test]
    fn duplicate_put_is_idempotent() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let store = EventStore::open(directory.path())?;
        let event = text_event(
            ConversationId::from_label("deduplication"),
            0,
            Vec::new(),
            "hello",
        )?;

        assert_eq!(store.put(&event)?, StoreOutcome::Inserted);
        assert_eq!(store.put(&event)?, StoreOutcome::AlreadyPresent);
        Ok(())
    }

    #[test]
    fn local_text_projection_is_encrypted_immutable_and_persistent() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let store = LocalMessageStore::open(directory.path())?;
        let author = DeviceIdentity::generate()?;
        let author_encryption = DeviceEncryptionIdentity::generate()?;
        let event = sign_test_text(
            &author,
            ConversationId::from_label("local-projection"),
            0,
            Vec::new(),
            "local plaintext",
        )?;
        let projection = LocalTextProjection::seal_authored(
            &event,
            author.device_id(),
            author_encryption.public_key(),
            "local plaintext",
        )?;
        let event_id = event.event_id()?;

        let (outcome, receipt) = store.put_with_receipt(&projection)?;
        assert_eq!(outcome, StoreOutcome::Inserted);
        assert_eq!(
            receipt.paths(),
            &[local_text_projection_path(
                &fs::canonicalize(directory.path())?,
                event_id
            )]
        );
        let (outcome, duplicate_receipt) = store.put_with_receipt(&projection)?;
        assert_eq!(outcome, StoreOutcome::AlreadyPresent);
        assert_eq!(duplicate_receipt, receipt);
        let independently_sealed = LocalTextProjection::seal_authored(
            &event,
            author.device_id(),
            author_encryption.public_key(),
            "local plaintext",
        )?;
        assert!(matches!(
            store.put(&independently_sealed),
            Err(StoreError::ImmutableLocalTextProjectionConflict {
                event_id: conflicting,
                ..
            }) if conflicting == event_id
        ));
        let bytes = fs::read(local_text_projection_path(directory.path(), event_id))?;
        assert!(
            !bytes
                .windows(b"local plaintext".len())
                .any(|window| window == b"local plaintext")
        );

        let reopened = LocalMessageStore::open(directory.path())?;
        let loaded = reopened.get(event_id)?;
        assert_eq!(loaded, projection);
        assert_eq!(
            loaded.open(&event, author.device_id(), &author_encryption)?,
            "local plaintext"
        );
        Ok(())
    }

    #[test]
    fn corrupted_event_is_rejected_on_read() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let store = EventStore::open(directory.path())?;
        let conversation_id = ConversationId::from_label("corruption");
        let event = text_event(conversation_id, 0, Vec::new(), "hello")?;
        let event_id = event.event_id()?;
        store.put(&event)?;

        let path = event_path(&store.conversation_directory(conversation_id), event_id);
        fs::write(path, b"corrupted")?;

        assert!(matches!(
            store.put(&event),
            Err(StoreError::ImmutableEventConflict { .. })
        ));
        assert!(matches!(
            store.load_conversation(conversation_id),
            Err(StoreError::InvalidStoredEvent { .. })
        ));
        Ok(())
    }

    #[test]
    fn writer_sequence_equivocation_is_rejected() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let store = EventStore::open(directory.path())?;
        let conversation_id = ConversationId::from_label("equivocation");
        let identity = DeviceIdentity::generate()?;
        let first = sign_test_text(&identity, conversation_id, 9, Vec::new(), "first")?;
        let conflicting = sign_test_text(&identity, conversation_id, 9, Vec::new(), "conflicting")?;

        store.put(&first)?;
        assert!(matches!(
            store.put(&conflicting),
            Err(StoreError::WriterSequenceConflict { .. })
        ));
        Ok(())
    }

    #[test]
    fn frontier_contains_only_unreferenced_events() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let store = EventStore::open(directory.path())?;
        let conversation_id = ConversationId::from_label("frontier");
        let first = text_event(conversation_id, 0, Vec::new(), "hello")?;
        let first_id = first.event_id()?;
        store.put(&first)?;

        let receiver = DeviceIdentity::generate()?;
        let acknowledgement = SignedEvent::sign_acknowledgement(
            &receiver,
            conversation_id,
            0,
            vec![first_id],
            first_id,
        )?;
        let acknowledgement_id = acknowledgement.event_id()?;
        store.put(&acknowledgement)?;

        assert_eq!(store.frontier(conversation_id)?, vec![acknowledgement_id]);
        assert!(matches!(
            acknowledgement.payload(),
            EventPayload::Acknowledgement { .. }
        ));
        Ok(())
    }

    #[test]
    fn sync_plan_is_bounded_and_bidirectional() -> Result<(), Box<dyn Error>> {
        let local_directory = tempdir()?;
        let local = EventStore::open(local_directory.path())?;
        let conversation_id = ConversationId::from_label("sync-plan");
        let local_only = text_event(conversation_id, 0, Vec::new(), "local")?;
        let local_only_id = local_only.event_id()?;
        local.put(&local_only)?;

        let remote_only = text_event(conversation_id, 0, Vec::new(), "remote")?;
        let remote_only_id = remote_only.event_id()?;
        let plan = local.plan_sync(conversation_id, &[remote_only_id], 1)?;

        assert_eq!(plan.requested_from_remote, vec![remote_only_id]);
        assert_eq!(plan.events_for_remote, vec![local_only]);
        assert!(!plan.more_available);
        assert_eq!(
            local.events_by_id(conversation_id, &[local_only_id])?,
            plan.events_for_remote
        );
        Ok(())
    }

    #[test]
    fn batch_put_inserts_events_and_rejects_equivocation() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let store = EventStore::open(directory.path())?;
        let conversation_id = ConversationId::from_label("batch-put");
        let identity = DeviceIdentity::generate()?;
        let first = sign_test_text(&identity, conversation_id, 0, Vec::new(), "first")?;
        let second = sign_test_text(&identity, conversation_id, 1, Vec::new(), "second")?;
        store.put_batch(&[first.clone(), second.clone()])?;
        store.put_batch(&[first])?;
        assert_eq!(store.load_conversation(conversation_id)?.len(), 2);

        let conflicting = sign_test_text(&identity, conversation_id, 1, Vec::new(), "conflicting")?;
        assert!(matches!(
            store.put_batch(&[conflicting]),
            Err(StoreError::WriterSequenceConflict { .. })
        ));
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn event_store_supports_paths_beyond_legacy_windows_max_path() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let root = directory.path().join("a".repeat(80)).join("b".repeat(80));
        let store = EventStore::open(&root)?;
        let conversation_id = ConversationId::from_label("windows-long-path");
        let event = text_event(conversation_id, 0, Vec::new(), "long path")?;
        let event_id = event.event_id()?;
        let path = event_path(&store.conversation_directory(conversation_id), event_id);

        assert!(path.to_string_lossy().encode_utf16().count() > 260);
        store.put(&event)?;
        assert_eq!(store.load_conversation(conversation_id)?.len(), 1);
        Ok(())
    }

    fn text_event(
        conversation_id: ConversationId,
        sequence: u64,
        parents: Vec<EventId>,
        body: &str,
    ) -> Result<SignedEvent, Box<dyn Error>> {
        sign_test_text(
            &DeviceIdentity::generate()?,
            conversation_id,
            sequence,
            parents,
            body,
        )
    }

    fn sign_test_text(
        identity: &DeviceIdentity,
        conversation_id: ConversationId,
        sequence: u64,
        parents: Vec<EventId>,
        body: &str,
    ) -> Result<SignedEvent, Box<dyn Error>> {
        let sender_directory = tempdir()?;
        let peer_directory = tempdir()?;
        let peer_identity = DeviceIdentity::generate()?;
        let peer_encryption = DeviceEncryptionIdentity::generate()?;
        let peer_root = AccountRootState::create(peer_directory.path().join("account"))?;
        let peer_certificate = peer_root.issue_device_certificate(
            peer_identity.device_id(),
            peer_encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let peer_device_list =
            peer_root.publish_device_list(std::slice::from_ref(&peer_certificate))?;
        let peer_bundle =
            RatchetState::load_or_create(peer_directory.path())?.prekey_bundle(&peer_identity)?;
        let (sender_ratchet_identity, ciphertext, _) = RatchetState::load_or_create(
            sender_directory.path(),
        )?
        .encrypt(identity, &peer_bundle, body)?;
        Ok(SignedEvent::sign_ratchet_text(
            identity,
            conversation_id,
            sequence,
            parents,
            peer_device_list,
            sender_ratchet_identity,
            vec![RatchetRecipient::new(
                peer_identity.device_id(),
                ciphertext,
            )?],
        )?)
    }
}
