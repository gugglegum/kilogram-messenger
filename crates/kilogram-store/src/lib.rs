use std::{
    collections::HashSet,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::fs::File;

use kilogram_identity::DeviceId;
use kilogram_protocol::{ConversationId, EventId, ProtocolError, SignedEvent};
use tempfile::NamedTempFile;
use thiserror::Error;

const EVENT_FILE_EXTENSION: &str = "event";

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

pub struct EventStore {
    root: PathBuf,
}

impl EventStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, StoreError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
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

        let mut temporary = NamedTempFile::new_in(&conversation_directory)?;
        temporary.write_all(&encoded)?;
        temporary.as_file().sync_all()?;

        match temporary.persist_noclobber(&destination) {
            Ok(file) => {
                file.sync_all()?;
                sync_directory(&conversation_directory)?;
                Ok(StoreOutcome::Inserted)
            }
            Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
                validate_existing_event(&destination, &encoded, event_id)
            }
            Err(error) => Err(error.error.into()),
        }
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

    fn conversation_directory(&self, conversation_id: ConversationId) -> PathBuf {
        self.root.join(conversation_id.to_string())
    }
}

fn event_path(directory: &Path, event_id: EventId) -> PathBuf {
    directory.join(format!("{event_id}.{EVENT_FILE_EXTENSION}"))
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

    use kilogram_identity::DeviceIdentity;
    use kilogram_protocol::EventPayload;
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
