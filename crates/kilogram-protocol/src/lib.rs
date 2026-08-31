use std::{collections::HashSet, fmt};

use kilogram_identity::{
    AccountAuthoritySnapshot, AccountId, ConversationMembershipSnapshot, ConversationScopeId,
    DeviceCapability, DeviceCertificate, DeviceId, DeviceIdentity, IdentityError,
    verify_device_authorization_with_snapshot,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod wire;

pub use wire::{
    ClientRequest, DeviceAuthorizationAccepted, DeviceAuthorizationRejected,
    MAX_INVENTORY_EVENT_IDS, MAX_SYNC_EVENTS_PER_BATCH, ServerResponse,
    SignedDeviceSessionAuthorization, SignedSyncInventory, SyncComplete, SyncDiff, SyncEventBatch,
    SyncPause, SyncPaused, SyncRejected, SyncRejectionReason, SyncSessionBinding,
};

const EVENT_VERSION: u8 = 1;
const EVENT_SIGNATURE_DOMAIN: &[u8] = b"kilogram:event-signature:v1\0";
const EVENT_ID_DOMAIN: &[u8] = b"kilogram:event-id:v1\0";
const CONVERSATION_LABEL_DOMAIN: &[u8] = b"kilogram:conversation-label:v1\0";
const MAX_PARENTS: usize = 64;
const MAX_TEXT_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct ConversationId([u8; 32]);

impl ConversationId {
    pub fn from_label(label: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(CONVERSATION_LABEL_DOMAIN);
        hasher.update(label.as_bytes());
        Self(*hasher.finalize().as_bytes())
    }

    pub fn scope_id(self) -> ConversationScopeId {
        ConversationScopeId::from_bytes(self.0)
    }
}

impl fmt::Display for ConversationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct EventId([u8; 32]);

impl fmt::Display for EventId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum EventPayload {
    Text { body: String },
    Acknowledgement { acknowledged_event_id: EventId },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventContent {
    version: u8,
    conversation_id: ConversationId,
    author_device_id: DeviceId,
    author_sequence: u64,
    parents: Vec<EventId>,
    payload: EventPayload,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignedEvent {
    content: EventContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuthorizedEvent {
    event: SignedEvent,
    author_certificate: DeviceCertificate,
    author_authority_snapshot: AccountAuthoritySnapshot,
}

impl AuthorizedEvent {
    pub fn new(
        event: SignedEvent,
        author_certificate: DeviceCertificate,
        author_authority_snapshot: AccountAuthoritySnapshot,
    ) -> Result<Self, ProtocolError> {
        let authorized = Self {
            event,
            author_certificate,
            author_authority_snapshot,
        };
        authorized.verify_author()?;
        Ok(authorized)
    }

    pub fn decode_and_verify_author(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let event: Self = postcard::from_bytes(bytes)?;
        event.verify_author()?;
        Ok(event)
    }

    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        self.verify_author()?;
        Ok(postcard::to_allocvec(self)?)
    }

    pub fn verify_author(&self) -> Result<(), ProtocolError> {
        self.event.verify()?;
        if self.author_certificate.device_id() != self.event.author_device_id() {
            return Err(ProtocolError::EventAuthorCertificateMismatch);
        }
        verify_device_authorization_with_snapshot(
            self.author_certificate.account_id(),
            &self.author_certificate,
            &self.author_authority_snapshot,
            &[DeviceCapability::SignEvents],
        )?;
        Ok(())
    }

    pub fn verify_for_membership(
        &self,
        membership: &ConversationMembershipSnapshot,
    ) -> Result<(), ProtocolError> {
        self.verify_author()?;
        if membership.conversation_id() != self.event.conversation_id().scope_id() {
            return Err(ProtocolError::EventConversationMembershipMismatch);
        }
        membership.require_member(self.author_certificate.account_id())?;
        Ok(())
    }

    pub fn event(&self) -> &SignedEvent {
        &self.event
    }

    pub fn into_event(self) -> SignedEvent {
        self.event
    }

    pub fn author_account_id(&self) -> AccountId {
        self.author_certificate.account_id()
    }

    pub fn author_certificate(&self) -> &DeviceCertificate {
        &self.author_certificate
    }

    pub fn author_authority_snapshot(&self) -> &AccountAuthoritySnapshot {
        &self.author_authority_snapshot
    }
}

impl SignedEvent {
    pub fn sign_text(
        identity: &DeviceIdentity,
        conversation_id: ConversationId,
        author_sequence: u64,
        parents: Vec<EventId>,
        body: String,
    ) -> Result<Self, ProtocolError> {
        Self::sign(
            identity,
            EventContent {
                version: EVENT_VERSION,
                conversation_id,
                author_device_id: identity.device_id(),
                author_sequence,
                parents,
                payload: EventPayload::Text { body },
            },
        )
    }

    pub fn sign_acknowledgement(
        identity: &DeviceIdentity,
        conversation_id: ConversationId,
        author_sequence: u64,
        parents: Vec<EventId>,
        acknowledged_event_id: EventId,
    ) -> Result<Self, ProtocolError> {
        Self::sign(
            identity,
            EventContent {
                version: EVENT_VERSION,
                conversation_id,
                author_device_id: identity.device_id(),
                author_sequence,
                parents,
                payload: EventPayload::Acknowledgement {
                    acknowledged_event_id,
                },
            },
        )
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let event: Self = postcard::from_bytes(bytes)?;
        event.verify()?;
        Ok(event)
    }

    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        Ok(postcard::to_allocvec(self)?)
    }

    pub fn verify(&self) -> Result<(), ProtocolError> {
        validate_content(&self.content)?;
        let signing_bytes = signing_bytes(&self.content)?;
        self.content
            .author_device_id
            .verify(&signing_bytes, &self.signature)?;
        Ok(())
    }

    pub fn event_id(&self) -> Result<EventId, ProtocolError> {
        let encoded = self.encode()?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(EVENT_ID_DOMAIN);
        hasher.update(&encoded);
        Ok(EventId(*hasher.finalize().as_bytes()))
    }

    pub fn conversation_id(&self) -> ConversationId {
        self.content.conversation_id
    }

    pub fn author_device_id(&self) -> DeviceId {
        self.content.author_device_id
    }

    pub fn author_sequence(&self) -> u64 {
        self.content.author_sequence
    }

    pub fn parents(&self) -> &[EventId] {
        &self.content.parents
    }

    pub fn payload(&self) -> &EventPayload {
        &self.content.payload
    }

    fn sign(identity: &DeviceIdentity, content: EventContent) -> Result<Self, ProtocolError> {
        validate_content(&content)?;
        let signature = identity.sign(&signing_bytes(&content)?).to_vec();
        Ok(Self { content, signature })
    }
}

fn signing_bytes(content: &EventContent) -> Result<Vec<u8>, ProtocolError> {
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(EVENT_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(EVENT_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn validate_content(content: &EventContent) -> Result<(), ProtocolError> {
    if content.version != EVENT_VERSION {
        return Err(ProtocolError::UnsupportedEventVersion(content.version));
    }
    if content.parents.len() > MAX_PARENTS {
        return Err(ProtocolError::TooManyParents(content.parents.len()));
    }
    let unique_parents: HashSet<_> = content.parents.iter().collect();
    if unique_parents.len() != content.parents.len() {
        return Err(ProtocolError::DuplicateParent);
    }
    if let EventPayload::Text { body } = &content.payload
        && body.len() > MAX_TEXT_BYTES
    {
        return Err(ProtocolError::TextTooLarge(body.len()));
    }
    Ok(())
}

fn write_hex(formatter: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("event encoding is invalid")]
    Encoding(#[from] postcard::Error),

    #[error("event signature is invalid")]
    Identity(#[from] IdentityError),

    #[error("unsupported event version: {0}")]
    UnsupportedEventVersion(u8),

    #[error("event has {0} parents; maximum is {MAX_PARENTS}")]
    TooManyParents(usize),

    #[error("event contains a duplicate parent")]
    DuplicateParent,

    #[error("text has {0} bytes; maximum is {MAX_TEXT_BYTES}")]
    TextTooLarge(usize),

    #[error("unsupported sync protocol version: {0}")]
    UnsupportedSyncVersion(u8),

    #[error("sync inventory has {0} event IDs; maximum is {MAX_INVENTORY_EVENT_IDS}")]
    TooManyInventoryEventIds(usize),

    #[error("sync batch has {0} events; maximum is {MAX_SYNC_EVENTS_PER_BATCH}")]
    TooManySyncEvents(usize),

    #[error("sync batch has {0} event IDs; maximum is {MAX_SYNC_EVENTS_PER_BATCH}")]
    TooManySyncEventIds(usize),

    #[error("sync message contains duplicate event ID {0}")]
    DuplicateSyncEventId(EventId),

    #[error("event {event_id} belongs to a different conversation")]
    SyncConversationMismatch { event_id: EventId },

    #[error("sync message is bound to a different transport session")]
    SyncSessionMismatch,

    #[error("sync response was signed by an unexpected device")]
    SyncResponderMismatch,

    #[error("unsupported device authorization version: {0}")]
    UnsupportedDeviceAuthorizationVersion(u8),

    #[error("device session authorization was signed by a different device")]
    DeviceAuthorizationSignerMismatch,

    #[error("device session authorization is bound to a different transport session")]
    DeviceAuthorizationSessionMismatch,

    #[error("device authorization response belongs to account {actual}; expected {expected}")]
    DeviceAuthorizationAccountMismatch {
        expected: kilogram_identity::AccountId,
        actual: kilogram_identity::AccountId,
    },

    #[error("device authorization response belongs to device {actual}; expected {expected}")]
    DeviceAuthorizationDeviceMismatch {
        expected: DeviceId,
        actual: DeviceId,
    },

    #[error("event author does not match its root-signed device certificate")]
    EventAuthorCertificateMismatch,

    #[error("event and conversation membership refer to different conversations")]
    EventConversationMembershipMismatch,
}

#[cfg(test)]
mod tests {
    use kilogram_identity::AccountRootState;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn signed_event_round_trips_and_verifies() -> Result<(), ProtocolError> {
        let identity = DeviceIdentity::generate()?;
        let event = SignedEvent::sign_text(
            &identity,
            ConversationId::from_label("test"),
            7,
            Vec::new(),
            "hello".to_owned(),
        )?;
        let decoded = SignedEvent::decode_and_verify(&event.encode()?)?;

        assert_eq!(decoded, event);
        assert_eq!(decoded.author_device_id(), identity.device_id());
        Ok(())
    }

    #[test]
    fn tampering_is_rejected() -> Result<(), ProtocolError> {
        let identity = DeviceIdentity::generate()?;
        let mut event = SignedEvent::sign_text(
            &identity,
            ConversationId::from_label("test"),
            0,
            Vec::new(),
            "original".to_owned(),
        )?;
        event.content.payload = EventPayload::Text {
            body: "tampered".to_owned(),
        };

        assert!(event.verify().is_err());
        Ok(())
    }

    #[test]
    fn unknown_event_version_is_rejected() -> Result<(), ProtocolError> {
        let identity = DeviceIdentity::generate()?;
        let mut event = SignedEvent::sign_text(
            &identity,
            ConversationId::from_label("test"),
            0,
            Vec::new(),
            "hello".to_owned(),
        )?;
        event.content.version = EVENT_VERSION + 1;

        assert!(matches!(
            event.verify(),
            Err(ProtocolError::UnsupportedEventVersion(_))
        ));
        Ok(())
    }

    #[test]
    fn acknowledgement_references_the_received_event() -> Result<(), ProtocolError> {
        let sender = DeviceIdentity::generate()?;
        let receiver = DeviceIdentity::generate()?;
        let conversation_id = ConversationId::from_label("test");
        let sent =
            SignedEvent::sign_text(&sender, conversation_id, 0, Vec::new(), "hello".to_owned())?;
        let sent_id = sent.event_id()?;
        let acknowledgement = SignedEvent::sign_acknowledgement(
            &receiver,
            conversation_id,
            0,
            vec![sent_id],
            sent_id,
        )?;

        acknowledgement.verify()?;
        assert_eq!(acknowledgement.parents(), &[sent_id]);
        assert_eq!(
            acknowledgement.payload(),
            &EventPayload::Acknowledgement {
                acknowledged_event_id: sent_id
            }
        );
        Ok(())
    }

    #[test]
    fn authorized_event_rejects_an_account_outside_conversation_membership()
    -> Result<(), Box<dyn std::error::Error>> {
        let owner_directory = tempdir()?;
        let outsider_directory = tempdir()?;
        let owner = AccountRootState::create(owner_directory.path())?;
        let outsider = AccountRootState::create(outsider_directory.path())?;
        let identity = DeviceIdentity::generate()?;
        let certificate = outsider
            .issue_device_certificate(identity.device_id(), &DeviceCapability::MESSAGING)?;
        let authority_snapshot = outsider.authority_snapshot()?;
        let conversation_id = ConversationId::from_label("membership-rejection");
        let membership = owner.create_conversation_membership(conversation_id.scope_id(), &[])?;
        let event = AuthorizedEvent::new(
            SignedEvent::sign_text(
                &identity,
                conversation_id,
                0,
                Vec::new(),
                "not a member".to_owned(),
            )?,
            certificate,
            authority_snapshot,
        )?;

        assert!(matches!(
            event.verify_for_membership(&membership),
            Err(ProtocolError::Identity(
                IdentityError::AccountNotConversationMember(account_id)
            )) if account_id == outsider.account_id()
        ));
        Ok(())
    }
}
