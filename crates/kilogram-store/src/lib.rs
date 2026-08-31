use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::fs::File;

use kilogram_identity::{ConversationMembershipSnapshot, ConversationScopeId, DeviceId};
use kilogram_protocol::{AuthorizedEvent, ConversationId, EventId, ProtocolError, SignedEvent};
use tempfile::NamedTempFile;
use thiserror::Error;

const EVENT_FILE_EXTENSION: &str = "event";
const AUTHORIZATION_FILE_EXTENSION: &str = "authorization";

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

fn event_path(directory: &Path, event_id: EventId) -> PathBuf {
    directory.join(format!("{event_id}.{EVENT_FILE_EXTENSION}"))
}

fn authorization_path(directory: &Path, event_id: EventId) -> PathBuf {
    directory.join(format!("{event_id}.{AUTHORIZATION_FILE_EXTENSION}"))
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
    use std::error::Error;

    use kilogram_identity::{AccountRootState, DeviceCapability, DeviceIdentity};
    use kilogram_protocol::{AuthorizedEvent, EventPayload};
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
        let certificate =
            root.issue_device_certificate(identity.device_id(), &DeviceCapability::MESSAGING)?;
        let authority_snapshot = root.authority_snapshot()?;
        let conversation_id = ConversationId::from_label("authorized-persistence");
        let membership = root.create_conversation_membership(conversation_id.scope_id(), &[])?;
        let event = AuthorizedEvent::new(
            SignedEvent::sign_text(
                &identity,
                conversation_id,
                0,
                Vec::new(),
                "hello".to_owned(),
            )?,
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
        let first = SignedEvent::sign_text(
            &identity,
            conversation_id,
            9,
            Vec::new(),
            "first".to_owned(),
        )?;
        let conflicting = SignedEvent::sign_text(
            &identity,
            conversation_id,
            9,
            Vec::new(),
            "conflicting".to_owned(),
        )?;

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
        let first = SignedEvent::sign_text(
            &identity,
            conversation_id,
            0,
            Vec::new(),
            "first".to_owned(),
        )?;
        let second = SignedEvent::sign_text(
            &identity,
            conversation_id,
            1,
            Vec::new(),
            "second".to_owned(),
        )?;
        store.put_batch(&[first.clone(), second.clone()])?;
        store.put_batch(&[first])?;
        assert_eq!(store.load_conversation(conversation_id)?.len(), 2);

        let conflicting = SignedEvent::sign_text(
            &identity,
            conversation_id,
            1,
            Vec::new(),
            "conflicting".to_owned(),
        )?;
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
    ) -> Result<SignedEvent, ProtocolError> {
        SignedEvent::sign_text(
            &DeviceIdentity::generate()?,
            conversation_id,
            sequence,
            parents,
            body.to_owned(),
        )
    }
}
