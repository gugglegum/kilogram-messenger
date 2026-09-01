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
};
use tempfile::NamedTempFile;
use thiserror::Error;

const EVENT_FILE_EXTENSION: &str = "event";
const AUTHORIZATION_FILE_EXTENSION: &str = "authorization";
const LOCAL_TEXT_PROJECTION_FILE_EXTENSION: &str = "local-text";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreOutcome {
    Inserted,
    AlreadyPresent,
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
        let event_id = projection.event_id();
        let encoded = projection.encode()?;
        let destination = local_text_projection_path(&self.root, event_id);
        if destination.try_exists()? {
            return validate_existing_local_text_projection(&destination, &encoded, event_id);
        }
        persist_local_text_projection(&self.root, &destination, &encoded, event_id)
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

impl EventStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, StoreError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        let root = fs::canonicalize(root)?;
        Ok(Self { root })
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
        event.verify_for_membership(membership)?;
        let event_id = event.event().event_id()?;
        let outcome = self.put(event.event())?;
        let conversation_directory = self.conversation_directory(event.event().conversation_id());
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
        Ok(outcome)
    }

    pub fn put_authorized_batch(
        &self,
        events: &[AuthorizedEvent],
        membership: &ConversationMembershipSnapshot,
    ) -> Result<(), StoreError> {
        for event in events {
            event.verify_for_membership(membership)?;
        }
        let signed_events: Vec<_> = events.iter().map(|event| event.event().clone()).collect();
        self.put_batch(&signed_events)?;
        for event in events {
            self.put_authorized(event, membership)?;
        }
        Ok(())
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
    use std::error::Error;

    use kilogram_identity::{
        AccountRootState, DeviceCapability, DeviceEncryptionIdentity, DeviceIdentity,
    };
    use kilogram_protocol::{AuthorizedEvent, EventPayload, LocalTextProjection};
    use kilogram_ratchet::RatchetState;
    use tempfile::tempdir;

    use super::*;

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

        assert_eq!(
            store.put_authorized(&event, &membership)?,
            StoreOutcome::Inserted
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

        assert_eq!(store.put(&projection)?, StoreOutcome::Inserted);
        assert_eq!(store.put(&projection)?, StoreOutcome::AlreadyPresent);
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
