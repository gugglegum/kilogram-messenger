use std::{collections::HashSet, fmt};

use kilogram_crypto::{
    CryptoError, DeviceEncryptionIdentity, ENCRYPTION_KEY_BYTES, EncryptionPublicKey, SealedMessage,
};
use kilogram_identity::{
    AccountAuthoritySnapshot, AccountDeviceListSnapshot, AccountId, ConversationMembershipSnapshot,
    ConversationScopeId, DeviceCapability, DeviceCertificate, DeviceId, DeviceIdentity,
    IdentityError, MAX_ACCOUNT_DEVICES, verify_device_authorization_with_snapshot,
};
use kilogram_ratchet::{DecryptedMessage, RatchetCiphertext, RatchetError, SignedRatchetIdentity};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod recovery;
mod rewrap;
mod wire;

pub use recovery::{HistoryRecoveryId, SignedHistoryRecoveryCheckpoint};
pub use rewrap::{
    HistoryRewrapBundle, HistoryRewrapEntry, HistoryRewrapId, HistoryRewrapManifest,
    HistoryRewrapSas, MAX_HISTORY_REWRAP_ENTRIES, SignedHistoryRewrapRequest,
    SignedHistoryRewrapTransfer,
};

pub use wire::{
    ClientRequest, DeviceAuthorizationAccepted, DeviceAuthorizationRejected, HistoryRewrapRejected,
    HistoryRewrapRejectionReason, MAX_ENDPOINT_ANNOUNCEMENT_ACKNOWLEDGEMENT_WIRE_BYTES,
    MAX_ENDPOINT_ANNOUNCEMENT_WIRE_BYTES, MAX_INVENTORY_EVENT_IDS,
    MAX_MAILBOX_CAPABILITY_ACKNOWLEDGEMENT_WIRE_BYTES, MAX_MAILBOX_CAPABILITY_UPDATE_WIRE_BYTES,
    MAX_MAILBOX_PROVIDER_GOSSIP_WIRE_BYTES, MAX_SYNC_EVENTS_PER_BATCH, ServerResponse,
    SignedDeviceSessionAuthorization, SignedSyncInventory, SyncComplete, SyncDiff, SyncEventBatch,
    SyncPause, SyncPaused, SyncRejected, SyncRejectionReason, SyncSessionBinding,
};

const EVENT_VERSION: u8 = 5;
const EVENT_SIGNATURE_DOMAIN: &[u8] = b"kilogram:event-signature:v5\0";
const EVENT_ID_DOMAIN: &[u8] = b"kilogram:event-id:v5\0";
const DIRECT_LOCAL_TEXT_PROJECTION_VERSION: u8 = 1;
const REWRAPPED_LOCAL_TEXT_PROJECTION_VERSION: u8 = 2;
const LOCAL_TEXT_PROJECTION_HPKE_INFO: &[u8] = b"kilogram:local-text-projection-hpke:v1\0";
const LOCAL_TEXT_PROJECTION_AAD_DOMAIN: &[u8] = b"kilogram:local-text-projection-aad:v1\0";
const CONVERSATION_LABEL_DOMAIN: &[u8] = b"kilogram:conversation-label:v1\0";
const MAX_PARENTS: usize = 64;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_CIPHERTEXT_BYTES: usize = MAX_TEXT_BYTES + 16;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
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

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ConversationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct EventId([u8; 32]);

impl EventId {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RatchetRecipient {
    device_id: DeviceId,
    ciphertext: RatchetCiphertext,
}

impl RatchetRecipient {
    pub fn new(device_id: DeviceId, ciphertext: RatchetCiphertext) -> Result<Self, ProtocolError> {
        ciphertext.validate()?;
        Ok(Self {
            device_id,
            ciphertext,
        })
    }

    pub fn device_id(&self) -> DeviceId {
        self.device_id
    }

    pub fn ciphertext(&self) -> &RatchetCiphertext {
        &self.ciphertext
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum EventPayload {
    RatchetText {
        recipient_device_list: Box<AccountDeviceListSnapshot>,
        sender_ratchet_identity: SignedRatchetIdentity,
        recipients: Vec<RatchetRecipient>,
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalTextProjection {
    content: LocalTextProjectionContent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum LocalTextProjectionContent {
    Direct(DirectLocalTextProjection),
    HistoryRewrap(Box<RewrappedLocalTextProjection>),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct DirectLocalTextProjection {
    version: u8,
    event_id: EventId,
    local_device_id: DeviceId,
    sealed: SealedMessage,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct RewrappedLocalTextProjection {
    version: u8,
    event_id: EventId,
    local_device_id: DeviceId,
    manifest: HistoryRewrapManifest,
    entry: HistoryRewrapEntry,
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
            content: LocalTextProjectionContent::Direct(DirectLocalTextProjection {
                version: DIRECT_LOCAL_TEXT_PROJECTION_VERSION,
                event_id,
                local_device_id,
                sealed: local_public_key.seal(
                    body.as_bytes(),
                    LOCAL_TEXT_PROJECTION_HPKE_INFO,
                    &aad,
                )?,
            }),
        };
        projection.validate()?;
        Ok(projection)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let Some(version) = bytes.first().copied() else {
            return Err(ProtocolError::UnsupportedLocalTextProjectionVersion(0));
        };
        let content = match version {
            DIRECT_LOCAL_TEXT_PROJECTION_VERSION => LocalTextProjectionContent::Direct(
                postcard::from_bytes::<DirectLocalTextProjection>(bytes)?,
            ),
            REWRAPPED_LOCAL_TEXT_PROJECTION_VERSION => {
                LocalTextProjectionContent::HistoryRewrap(Box::new(postcard::from_bytes::<
                    RewrappedLocalTextProjection,
                >(bytes)?))
            }
            version => {
                return Err(ProtocolError::UnsupportedLocalTextProjectionVersion(
                    version,
                ));
            }
        };
        let projection = Self { content };
        projection.validate()?;
        Ok(projection)
    }

    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        self.validate()?;
        match &self.content {
            LocalTextProjectionContent::Direct(projection) => {
                Ok(postcard::to_allocvec(projection)?)
            }
            LocalTextProjectionContent::HistoryRewrap(projection) => {
                Ok(postcard::to_allocvec(projection)?)
            }
        }
    }

    pub fn from_history_rewrap(
        event: &SignedEvent,
        bundle: &HistoryRewrapBundle,
        entry_offset: usize,
        local_device_id: DeviceId,
        local_account_id: AccountId,
        encryption: &DeviceEncryptionIdentity,
    ) -> Result<Self, ProtocolError> {
        let (authorized_event, _) = bundle.open_entry(entry_offset, local_device_id, encryption)?;
        if authorized_event.event() != event {
            return Err(ProtocolError::HistoryRewrapProjectionEventMismatch);
        }
        if bundle.manifest().account_id() != local_account_id {
            return Err(ProtocolError::HistoryRewrapAccountMismatch {
                expected: local_account_id,
                actual: bundle.manifest().account_id(),
            });
        }
        let projection = Self {
            content: LocalTextProjectionContent::HistoryRewrap(Box::new(
                RewrappedLocalTextProjection {
                    version: REWRAPPED_LOCAL_TEXT_PROJECTION_VERSION,
                    event_id: event.event_id()?,
                    local_device_id,
                    manifest: bundle.manifest().clone(),
                    entry: bundle.entries()[entry_offset].clone(),
                },
            )),
        };
        projection.validate()?;
        Ok(projection)
    }

    pub fn open(
        &self,
        event: &SignedEvent,
        local_device_id: DeviceId,
        encryption: &DeviceEncryptionIdentity,
    ) -> Result<String, ProtocolError> {
        self.open_inner(event, local_device_id, None, encryption)
    }

    pub fn open_for_account(
        &self,
        event: &SignedEvent,
        local_device_id: DeviceId,
        local_account_id: AccountId,
        encryption: &DeviceEncryptionIdentity,
    ) -> Result<String, ProtocolError> {
        self.open_inner(event, local_device_id, Some(local_account_id), encryption)
    }

    fn open_inner(
        &self,
        event: &SignedEvent,
        local_device_id: DeviceId,
        local_account_id: Option<AccountId>,
        encryption: &DeviceEncryptionIdentity,
    ) -> Result<String, ProtocolError> {
        self.validate()?;
        event.verify()?;
        let actual_event_id = event.event_id()?;
        if self.event_id() != actual_event_id {
            return Err(ProtocolError::LocalProjectionEventMismatch {
                expected: actual_event_id,
                actual: self.event_id(),
            });
        }
        if self.local_device_id() != local_device_id {
            return Err(ProtocolError::LocalProjectionDeviceMismatch {
                expected: local_device_id,
                actual: self.local_device_id(),
            });
        }
        match &self.content {
            LocalTextProjectionContent::Direct(projection) => {
                require_text_participant(event, local_device_id)?;
                let aad =
                    local_text_projection_aad(projection.event_id, projection.local_device_id)?;
                let plaintext =
                    encryption.open(&projection.sealed, LOCAL_TEXT_PROJECTION_HPKE_INFO, &aad)?;
                String::from_utf8(plaintext).map_err(ProtocolError::InvalidTextEncoding)
            }
            LocalTextProjectionContent::HistoryRewrap(projection) => {
                let local_account_id =
                    local_account_id.ok_or(ProtocolError::HistoryRewrapLocalAccountRequired)?;
                if projection.manifest.account_id() != local_account_id {
                    return Err(ProtocolError::HistoryRewrapAccountMismatch {
                        expected: local_account_id,
                        actual: projection.manifest.account_id(),
                    });
                }
                if projection.entry.event().event() != event {
                    return Err(ProtocolError::HistoryRewrapProjectionEventMismatch);
                }
                projection.entry.open_for_manifest(
                    &projection.manifest,
                    local_device_id,
                    encryption,
                )
            }
        }
    }

    pub fn event_id(&self) -> EventId {
        match &self.content {
            LocalTextProjectionContent::Direct(projection) => projection.event_id,
            LocalTextProjectionContent::HistoryRewrap(projection) => projection.event_id,
        }
    }

    pub fn local_device_id(&self) -> DeviceId {
        match &self.content {
            LocalTextProjectionContent::Direct(projection) => projection.local_device_id,
            LocalTextProjectionContent::HistoryRewrap(projection) => projection.local_device_id,
        }
    }

    pub fn history_rewrap_manifest(&self) -> Option<&HistoryRewrapManifest> {
        match &self.content {
            LocalTextProjectionContent::Direct(_) => None,
            LocalTextProjectionContent::HistoryRewrap(projection) => Some(&projection.manifest),
        }
    }

    fn validate(&self) -> Result<(), ProtocolError> {
        match &self.content {
            LocalTextProjectionContent::Direct(projection) => {
                if projection.version != DIRECT_LOCAL_TEXT_PROJECTION_VERSION {
                    return Err(ProtocolError::UnsupportedLocalTextProjectionVersion(
                        projection.version,
                    ));
                }
                validate_local_projection_sealed_message(&projection.sealed)?;
            }
            LocalTextProjectionContent::HistoryRewrap(projection) => {
                if projection.version != REWRAPPED_LOCAL_TEXT_PROJECTION_VERSION {
                    return Err(ProtocolError::UnsupportedLocalTextProjectionVersion(
                        projection.version,
                    ));
                }
                projection.manifest.verify()?;
                projection.entry.verify_for_manifest(&projection.manifest)?;
                if projection.entry.event().event().event_id()? != projection.event_id {
                    return Err(ProtocolError::HistoryRewrapProjectionEventMismatch);
                }
                if projection.local_device_id != projection.manifest.recipient_device_id() {
                    return Err(ProtocolError::HistoryRewrapRecipientMismatch {
                        expected: projection.manifest.recipient_device_id(),
                        actual: projection.local_device_id,
                    });
                }
            }
        }
        Ok(())
    }
}

fn validate_local_projection_sealed_message(sealed: &SealedMessage) -> Result<(), ProtocolError> {
    if sealed.encapsulated_key.len() != ENCRYPTION_KEY_BYTES {
        return Err(ProtocolError::InvalidEncapsulatedKeyLength(
            sealed.encapsulated_key.len(),
        ));
    }
    if sealed.ciphertext.len() > MAX_CIPHERTEXT_BYTES {
        return Err(ProtocolError::CiphertextTooLarge(sealed.ciphertext.len()));
    }
    Ok(())
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
        recipient_device_list: AccountDeviceListSnapshot,
        sender_ratchet_identity: SignedRatchetIdentity,
        mut recipients: Vec<RatchetRecipient>,
    ) -> Result<Self, ProtocolError> {
        recipients.sort_by_key(|recipient| *recipient.device_id().as_bytes());
        Self::sign(
            identity,
            EventContent {
                version: EVENT_VERSION,
                conversation_id,
                author_device_id: identity.device_id(),
                author_sequence,
                parents,
                payload: EventPayload::RatchetText {
                    recipient_device_list: Box::new(recipient_device_list),
                    sender_ratchet_identity,
                    recipients,
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

    pub fn ratchet_recipients(&self) -> Result<&[RatchetRecipient], ProtocolError> {
        let EventPayload::RatchetText { recipients, .. } = &self.content.payload else {
            return Err(ProtocolError::EventIsNotEncryptedText);
        };
        Ok(recipients)
    }

    pub fn recipient_device_list(&self) -> Result<&AccountDeviceListSnapshot, ProtocolError> {
        let EventPayload::RatchetText {
            recipient_device_list,
            ..
        } = &self.content.payload
        else {
            return Err(ProtocolError::EventIsNotEncryptedText);
        };
        Ok(recipient_device_list)
    }

    pub fn ratchet_message_for(
        &self,
        recipient_device_id: DeviceId,
    ) -> Result<(&SignedRatchetIdentity, &RatchetCiphertext), ProtocolError> {
        let EventPayload::RatchetText {
            sender_ratchet_identity,
            recipients,
            ..
        } = &self.content.payload
        else {
            return Err(ProtocolError::EventIsNotEncryptedText);
        };
        let index = recipients
            .binary_search_by(|recipient| recipient.device_id().cmp(&recipient_device_id))
            .map_err(|_| ProtocolError::MissingEncryptedRecipient(recipient_device_id))?;
        Ok((sender_ratchet_identity, recipients[index].ciphertext()))
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
        recipient_device_list,
        sender_ratchet_identity,
        recipients,
    } = &content.payload
    {
        sender_ratchet_identity.verify()?;
        if sender_ratchet_identity.device_id() != content.author_device_id {
            return Err(ProtocolError::RatchetIdentityAuthorMismatch);
        }
        recipient_device_list.verify()?;
        if recipients.is_empty() {
            return Err(ProtocolError::EmptyRatchetRecipients);
        }
        if recipients.len() > MAX_ACCOUNT_DEVICES {
            return Err(ProtocolError::TooManyRatchetRecipients(recipients.len()));
        }
        for recipient in recipients {
            if recipient.device_id() == content.author_device_id {
                return Err(ProtocolError::EncryptedRecipientIsAuthor(
                    recipient.device_id(),
                ));
            }
            recipient.ciphertext().validate()?;
        }
        for pair in recipients.windows(2) {
            match pair[0].device_id().cmp(&pair[1].device_id()) {
                std::cmp::Ordering::Less => {}
                std::cmp::Ordering::Equal => {
                    return Err(ProtocolError::DuplicateRatchetRecipient(
                        pair[0].device_id(),
                    ));
                }
                std::cmp::Ordering::Greater => {
                    return Err(ProtocolError::NonCanonicalRatchetRecipients);
                }
            }
        }
        if recipient_device_list.devices().len() != recipients.len()
            || recipient_device_list
                .devices()
                .iter()
                .zip(recipients)
                .any(|(certificate, recipient)| certificate.device_id() != recipient.device_id())
        {
            return Err(ProtocolError::RatchetRecipientDeviceListMismatch);
        }
    }
    Ok(())
}

fn require_text_participant(
    event: &SignedEvent,
    local_device_id: DeviceId,
) -> Result<(), ProtocolError> {
    let is_recipient = event
        .ratchet_recipients()?
        .binary_search_by(|recipient| recipient.device_id().cmp(&local_device_id))
        .is_ok();
    if event.author_device_id() != local_device_id && !is_recipient {
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
        version: DIRECT_LOCAL_TEXT_PROJECTION_VERSION,
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

    #[error("ratchet text must contain at least one recipient")]
    EmptyRatchetRecipients,

    #[error("ratchet text has {0} recipients; maximum is 32")]
    TooManyRatchetRecipients(usize),

    #[error("ratchet text contains duplicate recipient {0}")]
    DuplicateRatchetRecipient(DeviceId),

    #[error("ratchet text recipients are not in canonical device-ID order")]
    NonCanonicalRatchetRecipients,

    #[error("ratchet recipient slots do not exactly match the embedded root-signed device list")]
    RatchetRecipientDeviceListMismatch,

    #[error(
        "ratchet recipient device list belongs to account {actual}; expected local account {expected}"
    )]
    RatchetRecipientAccountMismatch {
        expected: AccountId,
        actual: AccountId,
    },

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

    #[error("unsupported history rewrap version: {0}")]
    UnsupportedHistoryRewrapVersion(u8),

    #[error("history rewrap source and recipient are the same device {0}")]
    HistoryRewrapSameDevice(DeviceId),

    #[error("history rewrap device list does not contain device {0}")]
    HistoryRewrapDeviceMissing(DeviceId),

    #[error(
        "invalid history rewrap range [{start}, {end}) for inventory with {inventory_len} text events"
    )]
    InvalidHistoryRewrapRange {
        start: usize,
        end: usize,
        inventory_len: usize,
    },

    #[error("history rewrap has {0} entries; maximum is {MAX_HISTORY_REWRAP_ENTRIES}")]
    TooManyHistoryRewrapEntries(usize),

    #[error("history rewrap inventory with {0} entries cannot be represented")]
    HistoryRewrapInventoryTooLarge(usize),

    #[error("history rewrap entries are not in canonical event-ID order")]
    NonCanonicalHistoryRewrapEntries,

    #[error("history rewrap event belongs to a different conversation")]
    HistoryRewrapConversationMismatch,

    #[error("history rewrap contains an event that is not ratchet text")]
    HistoryRewrapEventIsNotText,

    #[error("history rewrap contains {actual} entries; expected {expected}")]
    HistoryRewrapEntryCountMismatch { expected: usize, actual: usize },

    #[error("history rewrap entry index is {actual}; expected {expected}")]
    HistoryRewrapEntryIndexMismatch { expected: u64, actual: u64 },

    #[error("history rewrap entry index {index} is outside [{start}, {end})")]
    HistoryRewrapEntryIndexOutsideRange { index: u64, start: u64, end: u64 },

    #[error("history rewrap recipient is device {actual}; expected {expected}")]
    HistoryRewrapRecipientMismatch {
        expected: DeviceId,
        actual: DeviceId,
    },

    #[error("history rewrap entry offset {0} is out of range")]
    HistoryRewrapEntryOffsetOutOfRange(usize),

    #[error("local account ID is required to open a history-rewrapped projection")]
    HistoryRewrapLocalAccountRequired,

    #[error("history rewrap belongs to account {actual}; expected local account {expected}")]
    HistoryRewrapAccountMismatch {
        expected: AccountId,
        actual: AccountId,
    },

    #[error("history rewrap projection does not contain the expected immutable event")]
    HistoryRewrapProjectionEventMismatch,

    #[error("unsupported history rewrap request version: {0}")]
    UnsupportedHistoryRewrapRequestVersion(u8),

    #[error("history rewrap request is bound to a different transport session")]
    HistoryRewrapRequestSessionMismatch,

    #[error("history rewrap request source is device {actual}; expected {expected}")]
    HistoryRewrapRequestSourceMismatch {
        expected: DeviceId,
        actual: DeviceId,
    },

    #[error("history rewrap SAS does not match the authorized device pair")]
    HistoryRewrapSasMismatch,

    #[error("history rewrap request range overflows")]
    HistoryRewrapRequestRangeOverflow,

    #[error("unsupported history rewrap transfer version: {0}")]
    UnsupportedHistoryRewrapTransferVersion(u8),

    #[error("history rewrap transfer is bound to a different signed request")]
    HistoryRewrapTransferRequestMismatch,

    #[error("history rewrap transfer range exceeds the signed request")]
    HistoryRewrapTransferRangeMismatch,

    #[error("unsupported history rewrap response version: {0}")]
    UnsupportedHistoryRewrapResponseVersion(u8),

    #[error("unsupported history recovery checkpoint version: {0}")]
    UnsupportedHistoryRecoveryCheckpointVersion(u8),

    #[error("history recovery approved range overflows")]
    HistoryRecoveryRangeOverflow,

    #[error("invalid history recovery checkpoint range [{start}, {end}) at next index {next}")]
    InvalidHistoryRecoveryCheckpointRange { start: u64, end: u64, next: u64 },

    #[error("history recovery checkpoint signer is device {actual}; expected {expected}")]
    HistoryRecoveryCheckpointSignerMismatch {
        expected: DeviceId,
        actual: DeviceId,
    },

    #[error("history recovery page does not match the signed checkpoint")]
    HistoryRecoveryCheckpointPageMismatch,

    #[error("history recovery source inventory claim changed during resume")]
    HistoryRecoveryCheckpointClaimMismatch,

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

    #[error("endpoint announcement wire frame must not be empty")]
    EmptyEndpointAnnouncementFrame,

    #[error("endpoint announcement wire frame has {actual} bytes; maximum is {maximum}")]
    EndpointAnnouncementFrameTooLarge { actual: usize, maximum: usize },

    #[error("mailbox capability wire frame must not be empty")]
    EmptyMailboxCapabilityFrame,

    #[error("mailbox capability wire frame has {actual} bytes; maximum is {maximum}")]
    MailboxCapabilityFrameTooLarge { actual: usize, maximum: usize },

    #[error("mailbox provider gossip wire frame must not be empty")]
    EmptyMailboxProviderGossipFrame,

    #[error("mailbox provider gossip wire frame has {actual} bytes; maximum is {maximum}")]
    MailboxProviderGossipFrameTooLarge { actual: usize, maximum: usize },

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
        assert_eq!(decoded.ratchet_recipients()?.len(), 1);
        assert_ne!(
            decoded.ratchet_recipients()?[0].device_id(),
            identity.device_id()
        );
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
        match &mut tampered_projection.content {
            LocalTextProjectionContent::Direct(projection) => {
                projection.sealed.ciphertext[0] ^= 1;
            }
            LocalTextProjectionContent::HistoryRewrap(_) => {
                return Err(ProtocolError::HistoryRewrapProjectionEventMismatch.into());
            }
        }
        assert!(
            tampered_projection
                .open(&decoded, identity.device_id(), &own_encryption)
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn authenticated_history_rewrap_preserves_event_and_plaintext_provenance()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let conversation_id = ConversationId::from_label("history-rewrap");
        let source_identity = DeviceIdentity::generate()?;
        let source_encryption = DeviceEncryptionIdentity::generate()?;
        let recipient_identity = DeviceIdentity::generate()?;
        let recipient_encryption = DeviceEncryptionIdentity::generate()?;
        let account = AccountRootState::create(directory.path().join("account"))?;
        let source_certificate = account.issue_device_certificate(
            source_identity.device_id(),
            source_encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let recipient_certificate = account.issue_device_certificate(
            recipient_identity.device_id(),
            recipient_encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let device_list =
            account.publish_device_list(&[source_certificate, recipient_certificate])?;

        let author_identity = DeviceIdentity::generate()?;
        let author_encryption = DeviceEncryptionIdentity::generate()?;
        let author_account = AccountRootState::create(directory.path().join("author-account"))?;
        let author_certificate = author_account.issue_device_certificate(
            author_identity.device_id(),
            author_encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let author_snapshot = author_account.authority_snapshot()?;
        let first = AuthorizedEvent::new(
            sign_test_text(
                &author_identity,
                conversation_id,
                0,
                Vec::new(),
                "rewrapped-one",
            )?,
            author_certificate.clone(),
            author_snapshot.clone(),
        )?;
        let second = AuthorizedEvent::new(
            sign_test_text(
                &author_identity,
                conversation_id,
                1,
                Vec::new(),
                "rewrapped-two",
            )?,
            author_certificate,
            author_snapshot,
        )?;
        let mut keyed_inventory = vec![
            (first.event().event_id()?, first, "rewrapped-one".to_owned()),
            (
                second.event().event_id()?,
                second,
                "rewrapped-two".to_owned(),
            ),
        ];
        keyed_inventory.sort_by_key(|(event_id, _, _)| *event_id);
        let inventory = keyed_inventory
            .into_iter()
            .map(|(_, event, body)| (event, body))
            .collect::<Vec<_>>();
        assert!(matches!(
            HistoryRewrapBundle::seal(
                &source_identity,
                device_list.clone(),
                recipient_identity.device_id(),
                conversation_id,
                &inventory,
                0,
                0,
            ),
            Err(ProtocolError::InvalidHistoryRewrapRange { .. })
        ));
        let bundle = HistoryRewrapBundle::seal(
            &source_identity,
            device_list,
            recipient_identity.device_id(),
            conversation_id,
            &inventory,
            0,
            inventory.len(),
        )?;
        assert!(bundle.is_complete_source_inventory());
        let encoded = bundle.encode()?;
        let decoded = HistoryRewrapBundle::decode_and_verify(&encoded)?;
        assert_eq!(decoded.bundle_id()?, bundle.bundle_id()?);
        let mut tampered = encoded;
        if let Some(last) = tampered.last_mut() {
            *last ^= 1;
        } else {
            return Err(ProtocolError::HistoryRewrapEntryCountMismatch {
                expected: 1,
                actual: 0,
            }
            .into());
        }
        assert!(HistoryRewrapBundle::decode_and_verify(&tampered).is_err());

        for (offset, expected) in inventory.iter().enumerate() {
            let (authorized, body) = decoded.open_entry(
                offset,
                recipient_identity.device_id(),
                &recipient_encryption,
            )?;
            assert_eq!(&authorized, &expected.0);
            assert_eq!(body, expected.1);
            let projection = LocalTextProjection::from_history_rewrap(
                authorized.event(),
                &decoded,
                offset,
                recipient_identity.device_id(),
                account.account_id(),
                &recipient_encryption,
            )?;
            let projection = LocalTextProjection::decode(&projection.encode()?)?;
            assert!(projection.history_rewrap_manifest().is_some());
            assert!(matches!(
                projection.open(
                    authorized.event(),
                    recipient_identity.device_id(),
                    &recipient_encryption,
                ),
                Err(ProtocolError::HistoryRewrapLocalAccountRequired)
            ));
            assert_eq!(
                projection.open_for_account(
                    authorized.event(),
                    recipient_identity.device_id(),
                    account.account_id(),
                    &recipient_encryption,
                )?,
                expected.1
            );
        }
        assert!(
            decoded
                .open_entry(0, recipient_identity.device_id(), &source_encryption,)
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn network_history_rewrap_binds_sas_session_request_and_transfer()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let conversation_id = ConversationId::from_label("network-history-rewrap");
        let source = DeviceIdentity::generate()?;
        let source_encryption = DeviceEncryptionIdentity::generate()?;
        let recipient = DeviceIdentity::generate()?;
        let recipient_encryption = DeviceEncryptionIdentity::generate()?;
        let account = AccountRootState::create(directory.path().join("account"))?;
        let source_certificate = account.issue_device_certificate(
            source.device_id(),
            source_encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let recipient_certificate = account.issue_device_certificate(
            recipient.device_id(),
            recipient_encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let device_list =
            account.publish_device_list(&[source_certificate, recipient_certificate])?;
        let author = DeviceIdentity::generate()?;
        let author_account = AccountRootState::create(directory.path().join("author-account"))?;
        let author_certificate = author_account.issue_device_certificate(
            author.device_id(),
            DeviceEncryptionIdentity::generate()?.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let author_snapshot = author_account.authority_snapshot()?;
        let first_event = AuthorizedEvent::new(
            sign_test_text(&author, conversation_id, 0, Vec::new(), "recover me")?,
            author_certificate.clone(),
            author_snapshot.clone(),
        )?;
        let second_event = AuthorizedEvent::new(
            sign_test_text(&author, conversation_id, 1, Vec::new(), "recover more")?,
            author_certificate.clone(),
            author_snapshot.clone(),
        )?;
        let mut keyed_inventory = vec![
            (
                first_event.event().event_id()?,
                first_event.clone(),
                "recover me".to_owned(),
            ),
            (
                second_event.event().event_id()?,
                second_event,
                "recover more".to_owned(),
            ),
        ];
        keyed_inventory.sort_by_key(|(event_id, _, _)| *event_id);
        let inventory = keyed_inventory
            .into_iter()
            .map(|(_, event, body)| (event, body))
            .collect::<Vec<_>>();
        let sas =
            HistoryRewrapSas::derive(&device_list, source.device_id(), recipient.device_id())?;
        assert_ne!(
            sas,
            HistoryRewrapSas::derive(&device_list, recipient.device_id(), source.device_id(),)?
        );
        assert_eq!(sas.to_string().len(), 15);
        let session = SyncSessionBinding::from_transport_label("listener-endpoint");
        let request = SignedHistoryRewrapRequest::sign(
            &recipient,
            conversation_id,
            source.device_id(),
            session,
            0,
            2,
            sas,
        )?;
        request.verify_for_session(session, source.device_id(), recipient.device_id(), sas)?;
        assert!(matches!(
            request.verify_for_session(
                SyncSessionBinding::from_transport_label("other-endpoint"),
                source.device_id(),
                recipient.device_id(),
                sas,
            ),
            Err(ProtocolError::HistoryRewrapRequestSessionMismatch)
        ));
        let bundle = HistoryRewrapBundle::seal(
            &source,
            device_list.clone(),
            recipient.device_id(),
            conversation_id,
            &inventory,
            0,
            1,
        )?;
        let transfer = SignedHistoryRewrapTransfer::sign(&source, request.clone(), bundle)?;
        let recovery_bundle = transfer.bundle().clone();
        transfer.verify_for_request(&request)?;
        let encoded = transfer.encode()?;
        assert_eq!(
            SignedHistoryRewrapTransfer::decode_and_verify(&encoded)?,
            transfer
        );
        assert_eq!(
            ServerResponse::decode(
                &ServerResponse::HistoryRewrapTransfer(Box::new(transfer.clone())).encode()?
            )?,
            ServerResponse::HistoryRewrapTransfer(Box::new(transfer))
        );
        let other_request = SignedHistoryRewrapRequest::sign(
            &recipient,
            conversation_id,
            source.device_id(),
            session,
            0,
            1,
            sas,
        )?;
        assert!(matches!(
            SignedHistoryRewrapTransfer::decode_and_verify(&encoded)?
                .verify_for_request(&other_request),
            Err(ProtocolError::HistoryRewrapTransferRequestMismatch)
        ));
        let mut tampered = encoded;
        *tampered
            .last_mut()
            .ok_or(ProtocolError::HistoryRewrapTransferRequestMismatch)? ^= 1;
        assert!(SignedHistoryRewrapTransfer::decode_and_verify(&tampered).is_err());
        assert!(
            SignedHistoryRewrapRequest::sign(
                &recipient,
                conversation_id,
                source.device_id(),
                session,
                0,
                MAX_HISTORY_REWRAP_ENTRIES + 1,
                sas,
            )
            .is_err()
        );

        let initial_checkpoint = SignedHistoryRecoveryCheckpoint::start(
            &recipient,
            account.account_id(),
            conversation_id,
            source.device_id(),
            sas,
            0,
            2,
            1,
        )?;
        let advanced_checkpoint =
            initial_checkpoint.advance(&recipient, recovery_bundle.manifest())?;
        assert_eq!(advanced_checkpoint.next_range_start(), 1);
        assert_eq!(advanced_checkpoint.inventory_event_count(), Some(2));
        assert!(!advanced_checkpoint.is_complete());
        assert_eq!(
            advanced_checkpoint.previous_checkpoint_id(),
            Some(&initial_checkpoint.checkpoint_id()?)
        );
        assert_eq!(
            SignedHistoryRecoveryCheckpoint::decode_and_verify(&advanced_checkpoint.encode()?)?,
            advanced_checkpoint
        );
        assert!(matches!(
            advanced_checkpoint.advance(&recipient, recovery_bundle.manifest()),
            Err(ProtocolError::HistoryRecoveryCheckpointPageMismatch)
        ));
        let second_bundle = HistoryRewrapBundle::seal(
            &source,
            device_list.clone(),
            recipient.device_id(),
            conversation_id,
            &inventory,
            1,
            2,
        )?;
        let completed_checkpoint =
            advanced_checkpoint.advance(&recipient, second_bundle.manifest())?;
        assert!(completed_checkpoint.is_complete());

        let alternate_event = AuthorizedEvent::new(
            sign_test_text(&author, conversation_id, 2, Vec::new(), "alternate")?,
            author_certificate,
            author_snapshot,
        )?;
        let mut keyed_divergent_inventory = vec![
            (
                first_event.event().event_id()?,
                first_event,
                "recover me".to_owned(),
            ),
            (
                alternate_event.event().event_id()?,
                alternate_event,
                "alternate".to_owned(),
            ),
        ];
        keyed_divergent_inventory.sort_by_key(|(event_id, _, _)| *event_id);
        let divergent_inventory = keyed_divergent_inventory
            .into_iter()
            .map(|(_, event, body)| (event, body))
            .collect::<Vec<_>>();
        let divergent_bundle = HistoryRewrapBundle::seal(
            &source,
            device_list,
            recipient.device_id(),
            conversation_id,
            &divergent_inventory,
            1,
            2,
        )?;
        assert!(matches!(
            advanced_checkpoint.advance(&recipient, divergent_bundle.manifest()),
            Err(ProtocolError::HistoryRecoveryCheckpointClaimMismatch)
        ));
        let outsider = DeviceIdentity::generate()?;
        assert!(matches!(
            initial_checkpoint.advance(&outsider, recovery_bundle.manifest()),
            Err(ProtocolError::HistoryRecoveryCheckpointSignerMismatch { .. })
        ));
        Ok(())
    }

    #[test]
    fn ratchet_text_is_bound_to_recipient_device_and_metadata()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempdir()?;
        let identity = DeviceIdentity::generate()?;
        let peer_identity = DeviceIdentity::generate()?;
        let second_peer_identity = DeviceIdentity::generate()?;
        let peer_encryption = DeviceEncryptionIdentity::generate()?;
        let second_peer_encryption = DeviceEncryptionIdentity::generate()?;
        let peer_account = AccountRootState::create(root.path().join("peer-account"))?;
        let peer_certificate = peer_account.issue_device_certificate(
            peer_identity.device_id(),
            peer_encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let second_peer_certificate = peer_account.issue_device_certificate(
            second_peer_identity.device_id(),
            second_peer_encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let peer_device_list =
            peer_account.publish_device_list(&[peer_certificate, second_peer_certificate])?;
        let outsider_identity = DeviceIdentity::generate()?;
        let mut sender_ratchet = RatchetState::load_or_create(root.path().join("sender"))?;
        let mut peer_ratchet = RatchetState::load_or_create(root.path().join("peer"))?;
        let mut second_peer_ratchet =
            RatchetState::load_or_create(root.path().join("second-peer"))?;
        let peer_bundle = peer_ratchet.prekey_bundle(&peer_identity)?;
        let second_peer_bundle = second_peer_ratchet.prekey_bundle(&second_peer_identity)?;
        let (sender_ratchet_identity, peer_ciphertext, _) =
            sender_ratchet.encrypt(&identity, &peer_bundle, "for the intended peer device")?;
        let (second_sender_identity, second_peer_ciphertext, _) = sender_ratchet.encrypt(
            &identity,
            &second_peer_bundle,
            "for the intended peer device",
        )?;
        assert_eq!(second_sender_identity, sender_ratchet_identity);
        let peer_recipient = RatchetRecipient::new(peer_identity.device_id(), peer_ciphertext)?;
        let second_peer_recipient =
            RatchetRecipient::new(second_peer_identity.device_id(), second_peer_ciphertext)?;
        assert!(matches!(
            SignedEvent::sign_ratchet_text(
                &identity,
                ConversationId::from_label("incomplete-recipient-binding"),
                2,
                Vec::new(),
                peer_device_list.clone(),
                sender_ratchet_identity.clone(),
                vec![peer_recipient.clone()],
            ),
            Err(ProtocolError::RatchetRecipientDeviceListMismatch)
        ));
        let event = SignedEvent::sign_ratchet_text(
            &identity,
            ConversationId::from_label("recipient-binding"),
            3,
            Vec::new(),
            peer_device_list,
            sender_ratchet_identity,
            vec![second_peer_recipient, peer_recipient],
        )?;

        let (ratchet_identity, ciphertext) =
            event.ratchet_message_for(peer_identity.device_id())?;
        let (peer_body, _) = peer_ratchet.decrypt(&peer_identity, ratchet_identity, ciphertext)?;
        assert_eq!(peer_body.as_str(), "for the intended peer device");
        let (ratchet_identity, ciphertext) =
            event.ratchet_message_for(second_peer_identity.device_id())?;
        let (second_peer_body, _) =
            second_peer_ratchet.decrypt(&second_peer_identity, ratchet_identity, ciphertext)?;
        assert_eq!(second_peer_body.as_str(), peer_body.as_str());
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
        let second_peer_projection = LocalTextProjection::seal_received(
            &event,
            second_peer_identity.device_id(),
            second_peer_encryption.public_key(),
            &second_peer_body,
        )?;
        assert_eq!(
            second_peer_projection.open(
                &event,
                second_peer_identity.device_id(),
                &second_peer_encryption,
            )?,
            second_peer_body.as_str()
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
        let EventPayload::RatchetText { recipients, .. } = &mut event.content.payload else {
            return Err(Box::new(ProtocolError::EventIsNotEncryptedText));
        };
        recipients[0].device_id = DeviceIdentity::generate()?.device_id();

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
