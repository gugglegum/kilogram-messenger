use std::{collections::HashSet, fmt};

use kilogram_crypto::{
    CryptoError, DeviceEncryptionIdentity, ENCRYPTION_KEY_BYTES, EncryptionPublicKey, SealedMessage,
};
use kilogram_identity::{
    AccountAuthoritySnapshot, AccountId, ConversationMembershipSnapshot, ConversationScopeId,
    DeviceCapability, DeviceCertificate, DeviceId, DeviceIdentity, IdentityError,
    verify_device_authorization_with_snapshot,
};
use kilogram_ratchet::{DecryptedMessage, RatchetCiphertext, RatchetError, SignedRatchetIdentity};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod wire;

pub use wire::{
    ClientRequest, DeviceAuthorizationAccepted, DeviceAuthorizationRejected,
    MAX_INVENTORY_EVENT_IDS, MAX_SYNC_EVENTS_PER_BATCH, ServerResponse,
    SignedDeviceSessionAuthorization, SignedSyncInventory, SyncComplete, SyncDiff, SyncEventBatch,
    SyncPause, SyncPaused, SyncRejected, SyncRejectionReason, SyncSessionBinding,
};

const EVENT_VERSION: u8 = 4;
const EVENT_SIGNATURE_DOMAIN: &[u8] = b"kilogram:event-signature:v4\0";
const EVENT_ID_DOMAIN: &[u8] = b"kilogram:event-id:v4\0";
const LOCAL_TEXT_PROJECTION_VERSION: u8 = 1;
const LOCAL_TEXT_PROJECTION_HPKE_INFO: &[u8] = b"kilogram:local-text-projection-hpke:v1\0";
const LOCAL_TEXT_PROJECTION_AAD_DOMAIN: &[u8] = b"kilogram:local-text-projection-aad:v1\0";
const CONVERSATION_LABEL_DOMAIN: &[u8] = b"kilogram:conversation-label:v1\0";
const MAX_PARENTS: usize = 64;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_CIPHERTEXT_BYTES: usize = MAX_TEXT_BYTES + 16;

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
    RatchetText {
        recipient_device_id: DeviceId,
        sender_ratchet_identity: SignedRatchetIdentity,
        ciphertext: RatchetCiphertext,
    },
    Acknowledgement {
        acknowledged_event_id: EventId,
    },
}

#[derive(Serialize)]
struct LocalTextProjectionContext {
    version: u8,
    event_id: EventId,
    local_device_id: DeviceId,
}

/// Local-only encrypted plaintext projection for one immutable ciphertext event.
///
/// This object is never sent by the synchronization protocol. It allows an author
/// to read sent history without adding a static-key sender box to the replicated
/// event, which is a prerequisite for replacing the recipient HPKE box with a
/// forward-secret ratchet message.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LocalTextProjection {
    version: u8,
    event_id: EventId,
    local_device_id: DeviceId,
    sealed: SealedMessage,
}

impl LocalTextProjection {
    pub fn seal_authored(
        event: &SignedEvent,
        local_device_id: DeviceId,
        local_public_key: EncryptionPublicKey,
        body: &str,
    ) -> Result<Self, ProtocolError> {
        event.verify()?;
        if event.author_device_id() != local_device_id {
            return Err(ProtocolError::LocalProjectionAuthorMismatch {
                expected: local_device_id,
                actual: event.author_device_id(),
            });
        }
        Self::seal_participant(event, local_device_id, local_public_key, body)
    }

    pub fn seal_received(
        event: &SignedEvent,
        local_device_id: DeviceId,
        local_public_key: EncryptionPublicKey,
        decrypted: &DecryptedMessage,
    ) -> Result<Self, ProtocolError> {
        Self::seal_participant(event, local_device_id, local_public_key, decrypted.as_str())
    }

    fn seal_participant(
        event: &SignedEvent,
        local_device_id: DeviceId,
        local_public_key: EncryptionPublicKey,
        body: &str,
    ) -> Result<Self, ProtocolError> {
        if body.len() > MAX_TEXT_BYTES {
            return Err(ProtocolError::TextTooLarge(body.len()));
        }
        require_text_participant(event, local_device_id)?;
        let event_id = event.event_id()?;
        let aad = local_text_projection_aad(event_id, local_device_id)?;
        let projection = Self {
            version: LOCAL_TEXT_PROJECTION_VERSION,
            event_id,
            local_device_id,
            sealed: local_public_key.seal(
                body.as_bytes(),
                LOCAL_TEXT_PROJECTION_HPKE_INFO,
                &aad,
            )?,
        };
        projection.validate()?;
        Ok(projection)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let projection: Self = postcard::from_bytes(bytes)?;
        projection.validate()?;
        Ok(projection)
    }

    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        self.validate()?;
        Ok(postcard::to_allocvec(self)?)
    }

    pub fn open(
        &self,
        event: &SignedEvent,
        local_device_id: DeviceId,
        encryption: &DeviceEncryptionIdentity,
    ) -> Result<String, ProtocolError> {
        self.validate()?;
        event.verify()?;
        let actual_event_id = event.event_id()?;
        if self.event_id != actual_event_id {
            return Err(ProtocolError::LocalProjectionEventMismatch {
                expected: actual_event_id,
                actual: self.event_id,
            });
        }
        if self.local_device_id != local_device_id {
            return Err(ProtocolError::LocalProjectionDeviceMismatch {
                expected: local_device_id,
                actual: self.local_device_id,
            });
        }
        require_text_participant(event, local_device_id)?;
        let aad = local_text_projection_aad(self.event_id, self.local_device_id)?;
        let plaintext = encryption.open(&self.sealed, LOCAL_TEXT_PROJECTION_HPKE_INFO, &aad)?;
        String::from_utf8(plaintext).map_err(ProtocolError::InvalidTextEncoding)
    }

    pub fn event_id(&self) -> EventId {
        self.event_id
    }

    pub fn local_device_id(&self) -> DeviceId {
        self.local_device_id
    }

    fn validate(&self) -> Result<(), ProtocolError> {
        if self.version != LOCAL_TEXT_PROJECTION_VERSION {
            return Err(ProtocolError::UnsupportedLocalTextProjectionVersion(
                self.version,
            ));
        }
        if self.sealed.encapsulated_key.len() != ENCRYPTION_KEY_BYTES {
            return Err(ProtocolError::InvalidEncapsulatedKeyLength(
                self.sealed.encapsulated_key.len(),
            ));
        }
        if self.sealed.ciphertext.len() > MAX_CIPHERTEXT_BYTES {
            return Err(ProtocolError::CiphertextTooLarge(
                self.sealed.ciphertext.len(),
            ));
        }
        Ok(())
    }
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
    pub fn sign_ratchet_text(
        identity: &DeviceIdentity,
        conversation_id: ConversationId,
        author_sequence: u64,
        parents: Vec<EventId>,
        recipient_device_id: DeviceId,
        sender_ratchet_identity: SignedRatchetIdentity,
        ciphertext: RatchetCiphertext,
    ) -> Result<Self, ProtocolError> {
        if recipient_device_id == identity.device_id() {
            return Err(ProtocolError::EncryptedRecipientIsAuthor(
                recipient_device_id,
            ));
        }
        Self::sign(
            identity,
            EventContent {
                version: EVENT_VERSION,
                conversation_id,
                author_device_id: identity.device_id(),
                author_sequence,
                parents,
                payload: EventPayload::RatchetText {
                    recipient_device_id,
                    sender_ratchet_identity,
                    ciphertext,
                },
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

    pub fn text_recipient_device_id(&self) -> Result<DeviceId, ProtocolError> {
        let EventPayload::RatchetText {
            recipient_device_id,
            ..
        } = &self.content.payload
        else {
            return Err(ProtocolError::EventIsNotEncryptedText);
        };
        Ok(*recipient_device_id)
    }

    pub fn ratchet_message(
        &self,
    ) -> Result<(&SignedRatchetIdentity, &RatchetCiphertext), ProtocolError> {
        let EventPayload::RatchetText {
            sender_ratchet_identity,
            ciphertext,
            ..
        } = &self.content.payload
        else {
            return Err(ProtocolError::EventIsNotEncryptedText);
        };
        Ok((sender_ratchet_identity, ciphertext))
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
    if let EventPayload::RatchetText {
        recipient_device_id,
        sender_ratchet_identity,
        ciphertext,
    } = &content.payload
    {
        if *recipient_device_id == content.author_device_id {
            return Err(ProtocolError::EncryptedRecipientIsAuthor(
                *recipient_device_id,
            ));
        }
        sender_ratchet_identity.verify()?;
        if sender_ratchet_identity.device_id() != content.author_device_id {
            return Err(ProtocolError::RatchetIdentityAuthorMismatch);
        }
        ciphertext.validate()?;
    }
    Ok(())
}

fn require_text_participant(
    event: &SignedEvent,
    local_device_id: DeviceId,
) -> Result<(), ProtocolError> {
    let recipient_device_id = event.text_recipient_device_id()?;
    if event.author_device_id() != local_device_id && recipient_device_id != local_device_id {
        return Err(ProtocolError::LocalProjectionDeviceNotParticipant(
            local_device_id,
        ));
    }
    Ok(())
}

fn local_text_projection_aad(
    event_id: EventId,
    local_device_id: DeviceId,
) -> Result<Vec<u8>, ProtocolError> {
    let context = LocalTextProjectionContext {
        version: LOCAL_TEXT_PROJECTION_VERSION,
        event_id,
        local_device_id,
    };
    let encoded = postcard::to_allocvec(&context)?;
    let mut aad = Vec::with_capacity(LOCAL_TEXT_PROJECTION_AAD_DOMAIN.len() + encoded.len());
    aad.extend_from_slice(LOCAL_TEXT_PROJECTION_AAD_DOMAIN);
    aad.extend_from_slice(&encoded);
    Ok(aad)
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

    #[error("event encryption failed")]
    Crypto(#[from] CryptoError),

    #[error("ratchet payload is invalid")]
    Ratchet(#[from] RatchetError),

    #[error("unsupported event version: {0}")]
    UnsupportedEventVersion(u8),

    #[error("event has {0} parents; maximum is {MAX_PARENTS}")]
    TooManyParents(usize),

    #[error("event contains a duplicate parent")]
    DuplicateParent,

    #[error("text has {0} bytes; maximum is {MAX_TEXT_BYTES}")]
    TextTooLarge(usize),

    #[error("encrypted text recipient is the author device {0}")]
    EncryptedRecipientIsAuthor(DeviceId),

    #[error("HPKE encapsulated key has {0} bytes; expected {ENCRYPTION_KEY_BYTES}")]
    InvalidEncapsulatedKeyLength(usize),

    #[error("encrypted text ciphertext has {0} bytes; maximum is {MAX_CIPHERTEXT_BYTES}")]
    CiphertextTooLarge(usize),

    #[error("event is not an encrypted text event")]
    EventIsNotEncryptedText,

    #[error("encrypted text has no recipient box for device {0}")]
    MissingEncryptedRecipient(DeviceId),

    #[error("signed ratchet identity belongs to a different event author")]
    RatchetIdentityAuthorMismatch,

    #[error("decrypted text is not valid UTF-8")]
    InvalidTextEncoding(#[source] std::string::FromUtf8Error),

    #[error("unsupported local text projection version: {0}")]
    UnsupportedLocalTextProjectionVersion(u8),

    #[error("local text projection belongs to event {actual}; expected {expected}")]
    LocalProjectionEventMismatch { expected: EventId, actual: EventId },

    #[error("local text projection belongs to device {actual}; expected {expected}")]
    LocalProjectionDeviceMismatch {
        expected: DeviceId,
        actual: DeviceId,
    },

    #[error("device {0} is neither author nor recipient of the encrypted text event")]
    LocalProjectionDeviceNotParticipant(DeviceId),

    #[error("local text projection author is device {actual}; expected {expected}")]
    LocalProjectionAuthorMismatch {
        expected: DeviceId,
        actual: DeviceId,
    },

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
    use kilogram_ratchet::RatchetState;
    use tempfile::tempdir;

    use super::*;

    fn sign_test_text(
        identity: &DeviceIdentity,
        conversation_id: ConversationId,
        author_sequence: u64,
        parents: Vec<EventId>,
        body: &str,
    ) -> Result<SignedEvent, Box<dyn std::error::Error>> {
        let sender_directory = tempdir()?;
        let peer_directory = tempdir()?;
        let peer_identity = DeviceIdentity::generate()?;
        let peer_bundle =
            RatchetState::load_or_create(peer_directory.path())?.prekey_bundle(&peer_identity)?;
        let (sender_ratchet_identity, ciphertext, _) = RatchetState::load_or_create(
            sender_directory.path(),
        )?
        .encrypt(identity, &peer_bundle, body)?;
        Ok(SignedEvent::sign_ratchet_text(
            identity,
            conversation_id,
            author_sequence,
            parents,
            peer_identity.device_id(),
            sender_ratchet_identity,
            ciphertext,
        )?)
    }

    #[test]
    fn signed_event_round_trips_and_verifies() -> Result<(), Box<dyn std::error::Error>> {
        let identity = DeviceIdentity::generate()?;
        let own_encryption = DeviceEncryptionIdentity::generate()?;
        let event = sign_test_text(
            &identity,
            ConversationId::from_label("test"),
            7,
            Vec::new(),
            "hello",
        )?;
        let decoded = SignedEvent::decode_and_verify(&event.encode()?)?;
        let local_projection = LocalTextProjection::seal_authored(
            &decoded,
            identity.device_id(),
            own_encryption.public_key(),
            "hello",
        )?;

        assert_eq!(decoded, event);
        assert_eq!(decoded.author_device_id(), identity.device_id());
        assert_eq!(
            local_projection.open(&decoded, identity.device_id(), &own_encryption)?,
            "hello"
        );
        assert_ne!(decoded.text_recipient_device_id()?, identity.device_id());
        assert!(
            !decoded
                .encode()?
                .windows(b"hello".len())
                .any(|window| window == b"hello")
        );
        assert!(
            !local_projection
                .encode()?
                .windows(b"hello".len())
                .any(|window| window == b"hello")
        );
        let other_event = sign_test_text(
            &identity,
            ConversationId::from_label("test"),
            8,
            Vec::new(),
            "other",
        )?;
        assert!(matches!(
            local_projection.open(&other_event, identity.device_id(), &own_encryption),
            Err(ProtocolError::LocalProjectionEventMismatch { .. })
        ));
        let mut tampered_projection = local_projection;
        tampered_projection.sealed.ciphertext[0] ^= 1;
        assert!(
            tampered_projection
                .open(&decoded, identity.device_id(), &own_encryption)
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn ratchet_text_is_bound_to_recipient_device_and_metadata()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempdir()?;
        let identity = DeviceIdentity::generate()?;
        let peer_identity = DeviceIdentity::generate()?;
        let peer_encryption = DeviceEncryptionIdentity::generate()?;
        let outsider_identity = DeviceIdentity::generate()?;
        let mut sender_ratchet = RatchetState::load_or_create(root.path().join("sender"))?;
        let mut peer_ratchet = RatchetState::load_or_create(root.path().join("peer"))?;
        let peer_bundle = peer_ratchet.prekey_bundle(&peer_identity)?;
        let (sender_ratchet_identity, ciphertext, _) =
            sender_ratchet.encrypt(&identity, &peer_bundle, "for the intended peer device")?;
        let event = SignedEvent::sign_ratchet_text(
            &identity,
            ConversationId::from_label("recipient-binding"),
            3,
            Vec::new(),
            peer_identity.device_id(),
            sender_ratchet_identity,
            ciphertext,
        )?;

        let (ratchet_identity, ciphertext) = event.ratchet_message()?;
        let (peer_body, _) = peer_ratchet.decrypt(&peer_identity, ratchet_identity, ciphertext)?;
        assert_eq!(peer_body.as_str(), "for the intended peer device");
        let peer_projection = LocalTextProjection::seal_received(
            &event,
            peer_identity.device_id(),
            peer_encryption.public_key(),
            &peer_body,
        )?;
        assert_eq!(
            peer_projection.open(&event, peer_identity.device_id(), &peer_encryption)?,
            peer_body.as_str()
        );
        assert!(matches!(
            require_text_participant(&event, outsider_identity.device_id()),
            Err(ProtocolError::LocalProjectionDeviceNotParticipant(device_id))
                if device_id == outsider_identity.device_id()
        ));
        Ok(())
    }

    #[test]
    fn tampering_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let identity = DeviceIdentity::generate()?;
        let mut event = sign_test_text(
            &identity,
            ConversationId::from_label("test"),
            0,
            Vec::new(),
            "original",
        )?;
        let EventPayload::RatchetText {
            recipient_device_id,
            ..
        } = &mut event.content.payload
        else {
            return Err(Box::new(ProtocolError::EventIsNotEncryptedText));
        };
        *recipient_device_id = DeviceIdentity::generate()?.device_id();

        assert!(event.verify().is_err());
        Ok(())
    }

    #[test]
    fn unknown_event_version_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let identity = DeviceIdentity::generate()?;
        let mut event = sign_test_text(
            &identity,
            ConversationId::from_label("test"),
            0,
            Vec::new(),
            "hello",
        )?;
        event.content.version = EVENT_VERSION + 1;

        assert!(matches!(
            event.verify(),
            Err(ProtocolError::UnsupportedEventVersion(_))
        ));
        Ok(())
    }

    #[test]
    fn acknowledgement_references_the_received_event() -> Result<(), Box<dyn std::error::Error>> {
        let sender = DeviceIdentity::generate()?;
        let receiver = DeviceIdentity::generate()?;
        let conversation_id = ConversationId::from_label("test");
        let sent = sign_test_text(&sender, conversation_id, 0, Vec::new(), "hello")?;
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
        let encryption = DeviceEncryptionIdentity::generate()?;
        let certificate = outsider.issue_device_certificate(
            identity.device_id(),
            encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let authority_snapshot = outsider.authority_snapshot()?;
        let conversation_id = ConversationId::from_label("membership-rejection");
        let membership = owner.create_conversation_membership(conversation_id.scope_id(), &[])?;
        let event = AuthorizedEvent::new(
            sign_test_text(&identity, conversation_id, 0, Vec::new(), "not a member")?,
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
