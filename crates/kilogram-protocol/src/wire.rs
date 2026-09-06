use std::collections::HashSet;

use kilogram_identity::{
    AccountAuthoritySnapshot, AccountId, DeviceCertificate, DeviceId, DeviceIdentity,
};
use serde::{Deserialize, Serialize};

use crate::{
    AuthorizedEvent, ConversationId, EventId, ProtocolError, SignedHistoryRewrapRequest,
    SignedHistoryRewrapTransfer,
};

pub const MAX_INVENTORY_EVENT_IDS: usize = 4096;
pub const MAX_SYNC_EVENTS_PER_BATCH: usize = 64;
pub const MAX_ENDPOINT_ANNOUNCEMENT_WIRE_BYTES: usize = 7 * 1024 * 1024;
pub const MAX_ENDPOINT_ANNOUNCEMENT_ACKNOWLEDGEMENT_WIRE_BYTES: usize = 4 * 1024;
pub const MAX_MAILBOX_CAPABILITY_UPDATE_WIRE_BYTES: usize = 40 * 1024;
pub const MAX_MAILBOX_CAPABILITY_ACKNOWLEDGEMENT_WIRE_BYTES: usize = 4 * 1024;

const SYNC_VERSION: u8 = 6;
const SYNC_DIFF_SIGNATURE_DOMAIN: &[u8] = b"kilogram:sync-diff-signature:v6\0";
const SYNC_INVENTORY_SIGNATURE_DOMAIN: &[u8] = b"kilogram:sync-inventory-signature:v6\0";
const SYNC_SESSION_DOMAIN: &[u8] = b"kilogram:sync-session:v1\0";
const DEVICE_AUTHORIZATION_VERSION: u8 = 6;
const DEVICE_AUTHORIZATION_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:device-session-authorization-signature:v6\0";
const HISTORY_REWRAP_RESPONSE_VERSION: u8 = 1;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct SyncSessionBinding([u8; 32]);

impl SyncSessionBinding {
    pub fn from_transport_label(label: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(SYNC_SESSION_DOMAIN);
        hasher.update(label.as_bytes());
        Self(*hasher.finalize().as_bytes())
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct DeviceSessionAuthorizationContent {
    version: u8,
    session_binding: SyncSessionBinding,
    certificate: DeviceCertificate,
    authority_snapshot: AccountAuthoritySnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignedDeviceSessionAuthorization {
    content: DeviceSessionAuthorizationContent,
    signature: Vec<u8>,
}

impl SignedDeviceSessionAuthorization {
    pub fn sign(
        identity: &DeviceIdentity,
        certificate: DeviceCertificate,
        authority_snapshot: AccountAuthoritySnapshot,
        session_binding: SyncSessionBinding,
    ) -> Result<Self, ProtocolError> {
        certificate.verify()?;
        authority_snapshot.verify_for_account(certificate.account_id())?;
        if certificate.device_id() != identity.device_id() {
            return Err(ProtocolError::DeviceAuthorizationSignerMismatch);
        }
        let content = DeviceSessionAuthorizationContent {
            version: DEVICE_AUTHORIZATION_VERSION,
            session_binding,
            certificate,
            authority_snapshot,
        };
        let signature = identity
            .sign(&device_authorization_signing_bytes(&content)?)
            .to_vec();
        Ok(Self { content, signature })
    }

    pub fn verify_for_session(
        &self,
        expected_session: SyncSessionBinding,
    ) -> Result<(), ProtocolError> {
        self.verify_signature()?;
        if self.content.session_binding != expected_session {
            return Err(ProtocolError::DeviceAuthorizationSessionMismatch);
        }
        Ok(())
    }

    pub fn verify_signature(&self) -> Result<(), ProtocolError> {
        validate_device_authorization_version(self.content.version)?;
        self.content.certificate.verify()?;
        self.content
            .authority_snapshot
            .verify_for_account(self.content.certificate.account_id())?;
        self.content.certificate.device_id().verify(
            &device_authorization_signing_bytes(&self.content)?,
            &self.signature,
        )?;
        Ok(())
    }

    pub fn certificate(&self) -> &DeviceCertificate {
        &self.content.certificate
    }

    pub fn authority_snapshot(&self) -> &AccountAuthoritySnapshot {
        &self.content.authority_snapshot
    }

    pub fn session_binding(&self) -> SyncSessionBinding {
        self.content.session_binding
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeviceAuthorizationAccepted {
    version: u8,
    session_binding: SyncSessionBinding,
    account_id: AccountId,
    device_id: DeviceId,
}

impl DeviceAuthorizationAccepted {
    pub fn new(
        session_binding: SyncSessionBinding,
        account_id: AccountId,
        device_id: DeviceId,
    ) -> Self {
        Self {
            version: DEVICE_AUTHORIZATION_VERSION,
            session_binding,
            account_id,
            device_id,
        }
    }

    pub fn verify(
        &self,
        expected_session: SyncSessionBinding,
        expected_account: AccountId,
        expected_device: DeviceId,
    ) -> Result<(), ProtocolError> {
        self.validate()?;
        if self.session_binding != expected_session {
            return Err(ProtocolError::DeviceAuthorizationSessionMismatch);
        }
        if self.account_id != expected_account {
            return Err(ProtocolError::DeviceAuthorizationAccountMismatch {
                expected: expected_account,
                actual: self.account_id,
            });
        }
        if self.device_id != expected_device {
            return Err(ProtocolError::DeviceAuthorizationDeviceMismatch {
                expected: expected_device,
                actual: self.device_id,
            });
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), ProtocolError> {
        validate_device_authorization_version(self.version)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeviceAuthorizationRejected {
    version: u8,
}

impl DeviceAuthorizationRejected {
    pub fn new() -> Self {
        Self {
            version: DEVICE_AUTHORIZATION_VERSION,
        }
    }

    fn validate(&self) -> Result<(), ProtocolError> {
        validate_device_authorization_version(self.version)
    }
}

impl Default for DeviceAuthorizationRejected {
    fn default() -> Self {
        Self::new()
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
    events: Vec<AuthorizedEvent>,
}

impl SyncEventBatch {
    pub fn new(
        conversation_id: ConversationId,
        events: Vec<AuthorizedEvent>,
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

    pub fn events(&self) -> &[AuthorizedEvent] {
        &self.events
    }

    pub fn into_events(self) -> Vec<AuthorizedEvent> {
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
    events: Vec<AuthorizedEvent>,
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
        events: Vec<AuthorizedEvent>,
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

    pub fn events(&self) -> &[AuthorizedEvent] {
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SyncPause {
    version: u8,
    conversation_id: ConversationId,
}

impl SyncPause {
    pub fn new(conversation_id: ConversationId) -> Self {
        Self {
            version: SYNC_VERSION,
            conversation_id,
        }
    }

    pub fn conversation_id(&self) -> ConversationId {
        self.conversation_id
    }

    fn validate(&self) -> Result<(), ProtocolError> {
        validate_version(self.version)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SyncPaused {
    version: u8,
    conversation_id: ConversationId,
}

impl SyncPaused {
    pub fn new(conversation_id: ConversationId) -> Self {
        Self {
            version: SYNC_VERSION,
            conversation_id,
        }
    }

    pub fn conversation_id(&self) -> ConversationId {
        self.conversation_id
    }

    fn validate(&self) -> Result<(), ProtocolError> {
        validate_version(self.version)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SyncRejectionReason {
    RequesterNotAllowed,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HistoryRewrapRejectionReason {
    NotApproved,
    ApprovalMismatch,
    TransferTooLarge,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HistoryRewrapRejected {
    version: u8,
    conversation_id: ConversationId,
    reason: HistoryRewrapRejectionReason,
}

impl HistoryRewrapRejected {
    pub fn new(conversation_id: ConversationId, reason: HistoryRewrapRejectionReason) -> Self {
        Self {
            version: HISTORY_REWRAP_RESPONSE_VERSION,
            conversation_id,
            reason,
        }
    }

    pub fn conversation_id(&self) -> ConversationId {
        self.conversation_id
    }

    pub fn reason(&self) -> HistoryRewrapRejectionReason {
        self.reason
    }

    fn validate(&self) -> Result<(), ProtocolError> {
        if self.version != HISTORY_REWRAP_RESPONSE_VERSION {
            return Err(ProtocolError::UnsupportedHistoryRewrapResponseVersion(
                self.version,
            ));
        }
        Ok(())
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
    AuthorizeDevice(SignedDeviceSessionAuthorization),
    DeliverEvent(Box<AuthorizedEvent>),
    SyncInventory(SignedSyncInventory),
    SyncEvents(SyncEventBatch),
    SyncPause(SyncPause),
    HistoryRewrap(SignedHistoryRewrapRequest),
    EndpointAnnouncementPush(Vec<u8>),
    MailboxCapabilityUpdatePush(Vec<u8>),
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
            Self::AuthorizeDevice(authorization) => authorization.verify_signature(),
            Self::DeliverEvent(event) => event.verify_author(),
            Self::SyncInventory(inventory) => inventory.verify_signature(),
            Self::SyncEvents(batch) => batch.validate(),
            Self::SyncPause(pause) => pause.validate(),
            Self::HistoryRewrap(request) => request.verify_signature(),
            Self::EndpointAnnouncementPush(envelope) => {
                validate_endpoint_announcement_frame(envelope, MAX_ENDPOINT_ANNOUNCEMENT_WIRE_BYTES)
            }
            Self::MailboxCapabilityUpdatePush(update) => {
                validate_mailbox_capability_frame(update, MAX_MAILBOX_CAPABILITY_UPDATE_WIRE_BYTES)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ServerResponse {
    DeviceAuthorized(DeviceAuthorizationAccepted),
    DeviceAuthorizationRejected(DeviceAuthorizationRejected),
    EventAcknowledgement(Box<AuthorizedEvent>),
    SyncDiff(SyncDiff),
    SyncComplete(SyncComplete),
    SyncRejected(SyncRejected),
    SyncPaused(SyncPaused),
    HistoryRewrapTransfer(Box<SignedHistoryRewrapTransfer>),
    HistoryRewrapRejected(HistoryRewrapRejected),
    EndpointAnnouncementAcknowledged(Vec<u8>),
    EndpointAnnouncementRejected,
    MailboxCapabilityUpdateAcknowledged(Vec<u8>),
    MailboxCapabilityUpdateRejected,
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
            Self::DeviceAuthorized(accepted) => accepted.validate(),
            Self::DeviceAuthorizationRejected(rejected) => rejected.validate(),
            Self::EventAcknowledgement(event) => event.verify_author(),
            Self::SyncDiff(diff) => diff.verify_signature(),
            Self::SyncComplete(complete) => complete.validate(),
            Self::SyncRejected(rejected) => rejected.validate(),
            Self::SyncPaused(paused) => paused.validate(),
            Self::HistoryRewrapTransfer(transfer) => transfer.verify_signature(),
            Self::HistoryRewrapRejected(rejected) => rejected.validate(),
            Self::EndpointAnnouncementAcknowledged(acknowledgement) => {
                validate_endpoint_announcement_frame(
                    acknowledgement,
                    MAX_ENDPOINT_ANNOUNCEMENT_ACKNOWLEDGEMENT_WIRE_BYTES,
                )
            }
            Self::EndpointAnnouncementRejected => Ok(()),
            Self::MailboxCapabilityUpdateAcknowledged(acknowledgement) => {
                validate_mailbox_capability_frame(
                    acknowledgement,
                    MAX_MAILBOX_CAPABILITY_ACKNOWLEDGEMENT_WIRE_BYTES,
                )
            }
            Self::MailboxCapabilityUpdateRejected => Ok(()),
        }
    }
}

fn validate_mailbox_capability_frame(bytes: &[u8], maximum: usize) -> Result<(), ProtocolError> {
    if bytes.is_empty() {
        return Err(ProtocolError::EmptyMailboxCapabilityFrame);
    }
    if bytes.len() > maximum {
        return Err(ProtocolError::MailboxCapabilityFrameTooLarge {
            actual: bytes.len(),
            maximum,
        });
    }
    Ok(())
}

fn validate_endpoint_announcement_frame(bytes: &[u8], maximum: usize) -> Result<(), ProtocolError> {
    if bytes.is_empty() {
        return Err(ProtocolError::EmptyEndpointAnnouncementFrame);
    }
    if bytes.len() > maximum {
        return Err(ProtocolError::EndpointAnnouncementFrameTooLarge {
            actual: bytes.len(),
            maximum,
        });
    }
    Ok(())
}

fn device_authorization_signing_bytes(
    content: &DeviceSessionAuthorizationContent,
) -> Result<Vec<u8>, ProtocolError> {
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(DEVICE_AUTHORIZATION_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(DEVICE_AUTHORIZATION_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn validate_device_authorization_version(version: u8) -> Result<(), ProtocolError> {
    if version != DEVICE_AUTHORIZATION_VERSION {
        return Err(ProtocolError::UnsupportedDeviceAuthorizationVersion(
            version,
        ));
    }
    Ok(())
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
    events: &[AuthorizedEvent],
) -> Result<(), ProtocolError> {
    if events.len() > MAX_SYNC_EVENTS_PER_BATCH {
        return Err(ProtocolError::TooManySyncEvents(events.len()));
    }
    let mut event_ids = HashSet::with_capacity(events.len());
    for event in events {
        event.verify_author()?;
        let event_id = event.event().event_id()?;
        if event.event().conversation_id() != conversation_id {
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
    use kilogram_crypto::DeviceEncryptionIdentity;
    use kilogram_identity::{AccountRootState, DeviceCapability, IdentityError};
    use kilogram_ratchet::RatchetState;
    use tempfile::tempdir;

    use super::*;
    use crate::{RatchetRecipient, SignedEvent};

    fn sign_test_text(
        identity: &DeviceIdentity,
        conversation_id: ConversationId,
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
        .encrypt(identity, &peer_bundle, "hello")?;
        Ok(SignedEvent::sign_ratchet_text(
            identity,
            conversation_id,
            0,
            Vec::new(),
            peer_device_list,
            sender_ratchet_identity,
            vec![RatchetRecipient::new(
                peer_identity.device_id(),
                ciphertext,
            )?],
        )?)
    }

    #[test]
    fn device_authorization_is_root_and_session_bound() -> Result<(), Box<dyn std::error::Error>> {
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
        let expected_session = SyncSessionBinding::from_transport_label("listener-a");
        let other_session = SyncSessionBinding::from_transport_label("listener-b");
        assert!(matches!(
            SignedDeviceSessionAuthorization::sign(
                &DeviceIdentity::generate()?,
                certificate.clone(),
                authority_snapshot.clone(),
                expected_session,
            ),
            Err(ProtocolError::DeviceAuthorizationSignerMismatch)
        ));
        let authorization = SignedDeviceSessionAuthorization::sign(
            &identity,
            certificate,
            authority_snapshot,
            expected_session,
        )?;

        let request = ClientRequest::AuthorizeDevice(authorization.clone());
        assert_eq!(ClientRequest::decode(&request.encode()?)?, request);
        authorization.verify_for_session(expected_session)?;
        assert!(matches!(
            authorization.verify_for_session(other_session),
            Err(ProtocolError::DeviceAuthorizationSessionMismatch)
        ));

        let accepted = DeviceAuthorizationAccepted::new(
            expected_session,
            root.account_id(),
            identity.device_id(),
        );
        accepted.verify(expected_session, root.account_id(), identity.device_id())?;
        let response = ServerResponse::DeviceAuthorized(accepted);
        assert_eq!(ServerResponse::decode(&response.encode()?)?, response);

        let rejected =
            ServerResponse::DeviceAuthorizationRejected(DeviceAuthorizationRejected::new());
        assert_eq!(ServerResponse::decode(&rejected.encode()?)?, rejected);
        Ok(())
    }

    #[test]
    fn signed_inventory_is_bound_to_device_and_session() -> Result<(), Box<dyn std::error::Error>> {
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
    fn tampered_inventory_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
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
    fn sync_wire_messages_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let identity = DeviceIdentity::generate()?;
        let encryption = DeviceEncryptionIdentity::generate()?;
        let root_directory = tempdir().map_err(IdentityError::Io)?;
        let root = AccountRootState::create(root_directory.path())?;
        let certificate = root.issue_device_certificate(
            identity.device_id(),
            encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let authority_snapshot = root.authority_snapshot()?;
        let conversation_id = ConversationId::from_label("test");
        let event = AuthorizedEvent::new(
            sign_test_text(&identity, conversation_id)?,
            certificate,
            authority_snapshot,
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
            return Err(Box::new(ProtocolError::SyncResponderMismatch));
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
            SyncRejectionReason::RequesterNotAllowed,
        ));
        assert_eq!(ServerResponse::decode(&rejected.encode()?)?, rejected);

        let pause = ClientRequest::SyncPause(SyncPause::new(conversation_id));
        assert_eq!(ClientRequest::decode(&pause.encode()?)?, pause);
        let paused = ServerResponse::SyncPaused(SyncPaused::new(conversation_id));
        assert_eq!(ServerResponse::decode(&paused.encode()?)?, paused);
        Ok(())
    }

    #[test]
    fn inventory_size_and_uniqueness_are_enforced() -> Result<(), Box<dyn std::error::Error>> {
        let identity = DeviceIdentity::generate()?;
        let conversation_id = ConversationId::from_label("test");
        let session = SyncSessionBinding::from_transport_label("listener");
        let event = sign_test_text(&identity, conversation_id)?;
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

    #[test]
    fn endpoint_announcement_frames_are_bounded_and_round_trip()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = ClientRequest::EndpointAnnouncementPush(vec![1, 2, 3]);
        assert_eq!(ClientRequest::decode(&request.encode()?)?, request);
        let response = ServerResponse::EndpointAnnouncementAcknowledged(vec![4, 5, 6]);
        assert_eq!(ServerResponse::decode(&response.encode()?)?, response);
        assert!(matches!(
            ClientRequest::EndpointAnnouncementPush(Vec::new()).encode(),
            Err(ProtocolError::EmptyEndpointAnnouncementFrame)
        ));
        assert!(matches!(
            ServerResponse::EndpointAnnouncementAcknowledged(vec![
                0;
                MAX_ENDPOINT_ANNOUNCEMENT_ACKNOWLEDGEMENT_WIRE_BYTES
                    + 1
            ])
            .encode(),
            Err(ProtocolError::EndpointAnnouncementFrameTooLarge { .. })
        ));
        Ok(())
    }

    #[test]
    fn mailbox_capability_frames_are_bounded_and_round_trip()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = ClientRequest::MailboxCapabilityUpdatePush(vec![1, 2, 3]);
        assert_eq!(ClientRequest::decode(&request.encode()?)?, request);
        let response = ServerResponse::MailboxCapabilityUpdateAcknowledged(vec![4, 5, 6]);
        assert_eq!(ServerResponse::decode(&response.encode()?)?, response);
        assert!(matches!(
            ClientRequest::MailboxCapabilityUpdatePush(Vec::new()).encode(),
            Err(ProtocolError::EmptyMailboxCapabilityFrame)
        ));
        assert!(matches!(
            ServerResponse::MailboxCapabilityUpdateAcknowledged(vec![
                0;
                MAX_MAILBOX_CAPABILITY_ACKNOWLEDGEMENT_WIRE_BYTES
                    + 1
            ])
            .encode(),
            Err(ProtocolError::MailboxCapabilityFrameTooLarge { .. })
        ));
        Ok(())
    }
}
