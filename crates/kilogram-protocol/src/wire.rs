use std::collections::HashSet;

use kilogram_identity::{DeviceId, DeviceIdentity};
use serde::{Deserialize, Serialize};

use crate::{ConversationId, EventId, ProtocolError, SignedEvent};

pub const MAX_INVENTORY_EVENT_IDS: usize = 4096;
pub const MAX_SYNC_EVENTS_PER_BATCH: usize = 64;

const SYNC_VERSION: u8 = 1;
const SYNC_DIFF_SIGNATURE_DOMAIN: &[u8] = b"kilogram:sync-diff-signature:v1\0";
const SYNC_INVENTORY_SIGNATURE_DOMAIN: &[u8] = b"kilogram:sync-inventory-signature:v1\0";
const SYNC_SESSION_DOMAIN: &[u8] = b"kilogram:sync-session:v1\0";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct SyncSessionBinding([u8; 32]);

impl SyncSessionBinding {
    pub fn from_transport_label(label: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(SYNC_SESSION_DOMAIN);
        hasher.update(label.as_bytes());
        Self(*hasher.finalize().as_bytes())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct SyncInventoryContent {
    version: u8,
    conversation_id: ConversationId,
    requester_device_id: DeviceId,
    session_binding: SyncSessionBinding,
    event_ids: Vec<EventId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignedSyncInventory {
    content: SyncInventoryContent,
    signature: Vec<u8>,
}

impl SignedSyncInventory {
    pub fn sign(
        identity: &DeviceIdentity,
        conversation_id: ConversationId,
        session_binding: SyncSessionBinding,
        mut event_ids: Vec<EventId>,
    ) -> Result<Self, ProtocolError> {
        event_ids.sort_by_cached_key(ToString::to_string);
        let content = SyncInventoryContent {
            version: SYNC_VERSION,
            conversation_id,
            requester_device_id: identity.device_id(),
            session_binding,
            event_ids,
        };
        validate_inventory_content(&content)?;
        let signature = identity.sign(&inventory_signing_bytes(&content)?).to_vec();
        Ok(Self { content, signature })
    }

    pub fn verify_for_session(
        &self,
        expected_session: SyncSessionBinding,
    ) -> Result<(), ProtocolError> {
        self.verify_signature()?;
        if self.content.session_binding != expected_session {
            return Err(ProtocolError::SyncSessionMismatch);
        }
        Ok(())
    }

    pub fn verify_signature(&self) -> Result<(), ProtocolError> {
        validate_inventory_content(&self.content)?;
        self.content
            .requester_device_id
            .verify(&inventory_signing_bytes(&self.content)?, &self.signature)?;
        Ok(())
    }

    pub fn conversation_id(&self) -> ConversationId {
        self.content.conversation_id
    }

    pub fn requester_device_id(&self) -> DeviceId {
        self.content.requester_device_id
    }

    pub fn event_ids(&self) -> &[EventId] {
        &self.content.event_ids
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SyncEventBatch {
    version: u8,
    conversation_id: ConversationId,
    events: Vec<SignedEvent>,
}

impl SyncEventBatch {
    pub fn new(
        conversation_id: ConversationId,
        events: Vec<SignedEvent>,
    ) -> Result<Self, ProtocolError> {
        let batch = Self {
            version: SYNC_VERSION,
            conversation_id,
            events,
        };
        batch.validate()?;
        Ok(batch)
    }

    pub fn conversation_id(&self) -> ConversationId {
        self.conversation_id
    }

    pub fn events(&self) -> &[SignedEvent] {
        &self.events
    }

    pub fn into_events(self) -> Vec<SignedEvent> {
        self.events
    }

    fn validate(&self) -> Result<(), ProtocolError> {
        validate_version(self.version)?;
        validate_events(self.conversation_id, &self.events)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct SyncDiffContent {
    version: u8,
    conversation_id: ConversationId,
    responder_device_id: DeviceId,
    session_binding: SyncSessionBinding,
    requested_event_ids: Vec<EventId>,
    events: Vec<SignedEvent>,
    more_available: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SyncDiff {
    content: SyncDiffContent,
    signature: Vec<u8>,
}

impl SyncDiff {
    pub fn sign(
        identity: &DeviceIdentity,
        conversation_id: ConversationId,
        session_binding: SyncSessionBinding,
        requested_event_ids: Vec<EventId>,
        events: Vec<SignedEvent>,
        more_available: bool,
    ) -> Result<Self, ProtocolError> {
        let content = SyncDiffContent {
            version: SYNC_VERSION,
            conversation_id,
            responder_device_id: identity.device_id(),
            session_binding,
            requested_event_ids,
            events,
            more_available,
        };
        validate_sync_diff_content(&content)?;
        let signature = identity.sign(&sync_diff_signing_bytes(&content)?).to_vec();
        Ok(Self { content, signature })
    }

    pub fn verify_for_session(
        &self,
        expected_session: SyncSessionBinding,
        expected_responder: DeviceId,
    ) -> Result<(), ProtocolError> {
        self.verify_signature()?;
        if self.content.session_binding != expected_session {
            return Err(ProtocolError::SyncSessionMismatch);
        }
        if self.content.responder_device_id != expected_responder {
            return Err(ProtocolError::SyncResponderMismatch);
        }
        Ok(())
    }

    pub fn verify_signature(&self) -> Result<(), ProtocolError> {
        validate_sync_diff_content(&self.content)?;
        self.content
            .responder_device_id
            .verify(&sync_diff_signing_bytes(&self.content)?, &self.signature)?;
        Ok(())
    }

    pub fn conversation_id(&self) -> ConversationId {
        self.content.conversation_id
    }

    pub fn requested_event_ids(&self) -> &[EventId] {
        &self.content.requested_event_ids
    }

    pub fn events(&self) -> &[SignedEvent] {
        &self.content.events
    }

    pub fn more_available(&self) -> bool {
        self.content.more_available
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SyncComplete {
    version: u8,
    conversation_id: ConversationId,
    stored_event_ids: Vec<EventId>,
    more_available: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SyncRejectionReason {
    RequesterNotAllowed,
    RequesterNotKnown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SyncRejected {
    version: u8,
    conversation_id: ConversationId,
    reason: SyncRejectionReason,
}

impl SyncRejected {
    pub fn new(conversation_id: ConversationId, reason: SyncRejectionReason) -> Self {
        Self {
            version: SYNC_VERSION,
            conversation_id,
            reason,
        }
    }

    pub fn conversation_id(&self) -> ConversationId {
        self.conversation_id
    }

    pub fn reason(&self) -> SyncRejectionReason {
        self.reason
    }

    fn validate(&self) -> Result<(), ProtocolError> {
        validate_version(self.version)
    }
}

impl SyncComplete {
    pub fn new(
        conversation_id: ConversationId,
        stored_event_ids: Vec<EventId>,
        more_available: bool,
    ) -> Result<Self, ProtocolError> {
        let complete = Self {
            version: SYNC_VERSION,
            conversation_id,
            stored_event_ids,
            more_available,
        };
        complete.validate()?;
        Ok(complete)
    }

    pub fn conversation_id(&self) -> ConversationId {
        self.conversation_id
    }

    pub fn stored_event_ids(&self) -> &[EventId] {
        &self.stored_event_ids
    }

    pub fn more_available(&self) -> bool {
        self.more_available
    }

    fn validate(&self) -> Result<(), ProtocolError> {
        validate_version(self.version)?;
        validate_event_ids(&self.stored_event_ids, MAX_SYNC_EVENTS_PER_BATCH)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ClientRequest {
    DeliverEvent(SignedEvent),
    SyncInventory(SignedSyncInventory),
    SyncEvents(SyncEventBatch),
}

impl ClientRequest {
    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        self.validate()?;
        Ok(postcard::to_allocvec(self)?)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let request: Self = postcard::from_bytes(bytes)?;
        request.validate()?;
        Ok(request)
    }

    fn validate(&self) -> Result<(), ProtocolError> {
        match self {
            Self::DeliverEvent(event) => event.verify(),
            Self::SyncInventory(inventory) => inventory.verify_signature(),
            Self::SyncEvents(batch) => batch.validate(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ServerResponse {
    EventAcknowledgement(SignedEvent),
    SyncDiff(SyncDiff),
    SyncComplete(SyncComplete),
    SyncRejected(SyncRejected),
}

impl ServerResponse {
    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        self.validate()?;
        Ok(postcard::to_allocvec(self)?)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let response: Self = postcard::from_bytes(bytes)?;
        response.validate()?;
        Ok(response)
    }

    fn validate(&self) -> Result<(), ProtocolError> {
        match self {
            Self::EventAcknowledgement(event) => event.verify(),
            Self::SyncDiff(diff) => diff.verify_signature(),
            Self::SyncComplete(complete) => complete.validate(),
            Self::SyncRejected(rejected) => rejected.validate(),
        }
    }
}

fn inventory_signing_bytes(content: &SyncInventoryContent) -> Result<Vec<u8>, ProtocolError> {
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(SYNC_INVENTORY_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(SYNC_INVENTORY_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn sync_diff_signing_bytes(content: &SyncDiffContent) -> Result<Vec<u8>, ProtocolError> {
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(SYNC_DIFF_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(SYNC_DIFF_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn validate_inventory_content(content: &SyncInventoryContent) -> Result<(), ProtocolError> {
    validate_version(content.version)?;
    validate_event_ids(&content.event_ids, MAX_INVENTORY_EVENT_IDS)
}

fn validate_sync_diff_content(content: &SyncDiffContent) -> Result<(), ProtocolError> {
    validate_version(content.version)?;
    validate_event_ids(&content.requested_event_ids, MAX_SYNC_EVENTS_PER_BATCH)?;
    validate_events(content.conversation_id, &content.events)
}

fn validate_events(
    conversation_id: ConversationId,
    events: &[SignedEvent],
) -> Result<(), ProtocolError> {
    if events.len() > MAX_SYNC_EVENTS_PER_BATCH {
        return Err(ProtocolError::TooManySyncEvents(events.len()));
    }
    let mut event_ids = HashSet::with_capacity(events.len());
    for event in events {
        event.verify()?;
        let event_id = event.event_id()?;
        if event.conversation_id() != conversation_id {
            return Err(ProtocolError::SyncConversationMismatch { event_id });
        }
        if !event_ids.insert(event_id) {
            return Err(ProtocolError::DuplicateSyncEventId(event_id));
        }
    }
    Ok(())
}

fn validate_event_ids(event_ids: &[EventId], maximum: usize) -> Result<(), ProtocolError> {
    if event_ids.len() > maximum {
        if maximum == MAX_INVENTORY_EVENT_IDS {
            return Err(ProtocolError::TooManyInventoryEventIds(event_ids.len()));
        }
        return Err(ProtocolError::TooManySyncEventIds(event_ids.len()));
    }
    let mut unique = HashSet::with_capacity(event_ids.len());
    for event_id in event_ids {
        if !unique.insert(*event_id) {
            return Err(ProtocolError::DuplicateSyncEventId(*event_id));
        }
    }
    Ok(())
}

fn validate_version(version: u8) -> Result<(), ProtocolError> {
    if version != SYNC_VERSION {
        return Err(ProtocolError::UnsupportedSyncVersion(version));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_inventory_is_bound_to_device_and_session() -> Result<(), ProtocolError> {
        let identity = DeviceIdentity::generate()?;
        let expected = SyncSessionBinding::from_transport_label("listener-a");
        let other = SyncSessionBinding::from_transport_label("listener-b");
        let inventory = SignedSyncInventory::sign(
            &identity,
            ConversationId::from_label("test"),
            expected,
            Vec::new(),
        )?;

        inventory.verify_for_session(expected)?;
        assert!(matches!(
            inventory.verify_for_session(other),
            Err(ProtocolError::SyncSessionMismatch)
        ));
        Ok(())
    }

    #[test]
    fn tampered_inventory_is_rejected() -> Result<(), ProtocolError> {
        let identity = DeviceIdentity::generate()?;
        let session = SyncSessionBinding::from_transport_label("listener");
        let mut inventory = SignedSyncInventory::sign(
            &identity,
            ConversationId::from_label("test"),
            session,
            Vec::new(),
        )?;
        inventory.content.conversation_id = ConversationId::from_label("tampered");

        assert!(inventory.verify_signature().is_err());
        Ok(())
    }

    #[test]
    fn sync_wire_messages_round_trip() -> Result<(), ProtocolError> {
        let identity = DeviceIdentity::generate()?;
        let conversation_id = ConversationId::from_label("test");
        let event = SignedEvent::sign_text(
            &identity,
            conversation_id,
            0,
            Vec::new(),
            "hello".to_owned(),
        )?;
        let session = SyncSessionBinding::from_transport_label("listener");
        let response = ServerResponse::SyncDiff(SyncDiff::sign(
            &identity,
            conversation_id,
            session,
            Vec::new(),
            vec![event],
            false,
        )?);

        assert_eq!(ServerResponse::decode(&response.encode()?)?, response);
        let ServerResponse::SyncDiff(decoded_diff) = ServerResponse::decode(&response.encode()?)?
        else {
            return Err(ProtocolError::SyncResponderMismatch);
        };
        decoded_diff.verify_for_session(session, identity.device_id())?;
        assert!(matches!(
            decoded_diff.verify_for_session(session, DeviceIdentity::generate()?.device_id()),
            Err(ProtocolError::SyncResponderMismatch)
        ));
        assert!(matches!(
            decoded_diff.verify_for_session(
                SyncSessionBinding::from_transport_label("other-listener"),
                identity.device_id()
            ),
            Err(ProtocolError::SyncSessionMismatch)
        ));

        let rejected = ServerResponse::SyncRejected(SyncRejected::new(
            conversation_id,
            SyncRejectionReason::RequesterNotKnown,
        ));
        assert_eq!(ServerResponse::decode(&rejected.encode()?)?, rejected);
        Ok(())
    }

    #[test]
    fn inventory_size_and_uniqueness_are_enforced() -> Result<(), ProtocolError> {
        let identity = DeviceIdentity::generate()?;
        let conversation_id = ConversationId::from_label("test");
        let session = SyncSessionBinding::from_transport_label("listener");
        let event = SignedEvent::sign_text(
            &identity,
            conversation_id,
            0,
            Vec::new(),
            "hello".to_owned(),
        )?;
        let event_id = event.event_id()?;

        assert!(matches!(
            SignedSyncInventory::sign(
                &identity,
                conversation_id,
                session,
                vec![event_id; MAX_INVENTORY_EVENT_IDS + 1]
            ),
            Err(ProtocolError::TooManyInventoryEventIds(_))
        ));
        assert!(matches!(
            SignedSyncInventory::sign(
                &identity,
                conversation_id,
                session,
                vec![event_id, event_id]
            ),
            Err(ProtocolError::DuplicateSyncEventId(_))
        ));
        Ok(())
    }
}
