use anyhow::{Context, Result, ensure};
use kilogram_crypto::DeviceEncryptionIdentity;
use kilogram_identity::{AccountId, DeviceCertificate, DeviceId, DeviceIdentity};
use kilogram_mailbox::{
    MAX_MAILBOX_PLAINTEXT_BYTES, MAX_MAILBOX_TTL_SECONDS, MIN_MAILBOX_TTL_SECONDS, MailboxId,
    MailboxItemId,
};
use kilogram_mailbox_provisioning::{
    EncryptedMailboxOffer, LocalMailboxBinding, MailboxBindingId, MailboxScope, PeerMailboxBinding,
    SealedLocalMailboxBinding,
};
use kilogram_protocol::{AuthorizedEvent, ConversationId, EventId};
use serde::{Deserialize, Serialize};

use crate::runtime_queue::{MAX_RUNTIME_RECORD_BYTES, RuntimeContactId};

const VERSION: u8 = 1;
const LOCAL_SIGNATURE_DOMAIN: &[u8] = b"kilogram:runtime-local-mailbox-binding:v1\0";
const PEER_SIGNATURE_DOMAIN: &[u8] = b"kilogram:runtime-peer-mailbox-binding:v1\0";
const DISPATCH_SIGNATURE_DOMAIN: &[u8] = b"kilogram:runtime-mailbox-dispatch:v1\0";
const ITEM_ID_DOMAIN: &[u8] = b"kilogram:runtime-mailbox-item-id:v1\0";
const EVENT_ITEM_ID_DOMAIN: &[u8] = b"kilogram:runtime-mailbox-event-item-id:v1\0";
const PAYLOAD_VERSION: u8 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RuntimeLocalMailboxBindingContent {
    version: u8,
    local_account_id: AccountId,
    local_device_id: DeviceId,
    contact_id: RuntimeContactId,
    peer_account_id: AccountId,
    peer_device_id: DeviceId,
    conversation_id: ConversationId,
    binding_id: MailboxBindingId,
    created_at_unix_seconds: u64,
    sealed_binding: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedRuntimeLocalMailboxBinding {
    content: RuntimeLocalMailboxBindingContent,
    signature: Vec<u8>,
}

impl SignedRuntimeLocalMailboxBinding {
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        identity: &DeviceIdentity,
        local_account_id: AccountId,
        contact_id: RuntimeContactId,
        peer_account_id: AccountId,
        peer_device_id: DeviceId,
        conversation_id: ConversationId,
        created_at_unix_seconds: u64,
        sealed_binding: &SealedLocalMailboxBinding,
    ) -> Result<Self> {
        let content = RuntimeLocalMailboxBindingContent {
            version: VERSION,
            local_account_id,
            local_device_id: identity.device_id(),
            contact_id,
            peer_account_id,
            peer_device_id,
            conversation_id,
            binding_id: sealed_binding.binding_id(),
            created_at_unix_seconds,
            sealed_binding: sealed_binding.encode()?,
        };
        let signature = identity
            .sign(&signing_bytes(LOCAL_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let value = Self { content, signature };
        value.verify_local(local_account_id, identity.device_id())?;
        Ok(value)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify_local(self.local_account_id(), self.local_device_id())?;
        let bytes = postcard::to_allocvec(self).context("encode runtime local mailbox binding")?;
        ensure!(
            bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "runtime local mailbox binding is too large"
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "runtime local mailbox binding size is invalid"
        );
        let value: Self =
            postcard::from_bytes(bytes).context("decode runtime local mailbox binding")?;
        value.verify_local(value.local_account_id(), value.local_device_id())?;
        Ok(value)
    }

    pub fn verify_local(
        &self,
        expected_account_id: AccountId,
        expected_device_id: DeviceId,
    ) -> Result<()> {
        ensure!(
            self.content.version == VERSION
                && self.local_account_id() == expected_account_id
                && self.local_device_id() == expected_device_id
                && self.content.peer_account_id != self.local_account_id()
                && self.content.created_at_unix_seconds != 0,
            "runtime local mailbox binding metadata is invalid"
        );
        let sealed = SealedLocalMailboxBinding::decode(&self.content.sealed_binding)?;
        ensure!(
            sealed.binding_id() == self.binding_id(),
            "runtime local mailbox binding ID does not match its sealed capability"
        );
        self.local_device_id().verify(
            &signing_bytes(LOCAL_SIGNATURE_DOMAIN, &self.content)?,
            &self.signature,
        )?;
        Ok(())
    }

    pub fn open(
        &self,
        identity: &DeviceIdentity,
        encryption: &DeviceEncryptionIdentity,
        certificate: &DeviceCertificate,
    ) -> Result<LocalMailboxBinding> {
        self.verify_local(certificate.account_id(), certificate.device_id())?;
        ensure!(
            identity.device_id() == self.local_device_id(),
            "runtime local mailbox binding signer is not the active Device"
        );
        let opened = SealedLocalMailboxBinding::decode(&self.content.sealed_binding)?.open(
            identity,
            encryption,
            certificate,
        )?;
        ensure!(
            opened.binding_id()? == self.binding_id()
                && opened.owner_account_id() == self.local_account_id()
                && opened.owner_device_id() == self.local_device_id()
                && opened.peer_account_id() == self.peer_account_id()
                && opened.peer_device_id() == self.peer_device_id()
                && opened.scope() == mailbox_scope(self.conversation_id()),
            "opened local mailbox capability does not match runtime metadata"
        );
        Ok(opened)
    }

    pub fn local_account_id(&self) -> AccountId {
        self.content.local_account_id
    }

    pub fn local_device_id(&self) -> DeviceId {
        self.content.local_device_id
    }

    pub fn contact_id(&self) -> RuntimeContactId {
        self.content.contact_id
    }

    pub fn peer_account_id(&self) -> AccountId {
        self.content.peer_account_id
    }

    pub fn peer_device_id(&self) -> DeviceId {
        self.content.peer_device_id
    }

    pub fn conversation_id(&self) -> ConversationId {
        self.content.conversation_id
    }

    pub fn binding_id(&self) -> MailboxBindingId {
        self.content.binding_id
    }

    pub fn created_at_unix_seconds(&self) -> u64 {
        self.content.created_at_unix_seconds
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RuntimePeerMailboxBindingContent {
    version: u8,
    local_account_id: AccountId,
    local_device_id: DeviceId,
    contact_id: RuntimeContactId,
    peer_account_id: AccountId,
    peer_device_id: DeviceId,
    conversation_id: ConversationId,
    binding_id: MailboxBindingId,
    imported_at_unix_seconds: u64,
    encrypted_offer: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedRuntimePeerMailboxBinding {
    content: RuntimePeerMailboxBindingContent,
    signature: Vec<u8>,
}

impl SignedRuntimePeerMailboxBinding {
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        identity: &DeviceIdentity,
        local_account_id: AccountId,
        contact_id: RuntimeContactId,
        peer_account_id: AccountId,
        peer_device_id: DeviceId,
        conversation_id: ConversationId,
        imported_at_unix_seconds: u64,
        encrypted_offer: &EncryptedMailboxOffer,
    ) -> Result<Self> {
        let content = RuntimePeerMailboxBindingContent {
            version: VERSION,
            local_account_id,
            local_device_id: identity.device_id(),
            contact_id,
            peer_account_id,
            peer_device_id,
            conversation_id,
            binding_id: encrypted_offer.binding_id(),
            imported_at_unix_seconds,
            encrypted_offer: encrypted_offer.encode()?,
        };
        let signature = identity
            .sign(&signing_bytes(PEER_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let value = Self { content, signature };
        value.verify_local(local_account_id, identity.device_id())?;
        Ok(value)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify_local(self.local_account_id(), self.local_device_id())?;
        let bytes = postcard::to_allocvec(self).context("encode runtime peer mailbox binding")?;
        ensure!(
            bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "runtime peer mailbox binding is too large"
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "runtime peer mailbox binding size is invalid"
        );
        let value: Self =
            postcard::from_bytes(bytes).context("decode runtime peer mailbox binding")?;
        value.verify_local(value.local_account_id(), value.local_device_id())?;
        Ok(value)
    }

    pub fn verify_local(
        &self,
        expected_account_id: AccountId,
        expected_device_id: DeviceId,
    ) -> Result<()> {
        ensure!(
            self.content.version == VERSION
                && self.local_account_id() == expected_account_id
                && self.local_device_id() == expected_device_id
                && self.content.peer_account_id != self.local_account_id()
                && self.content.imported_at_unix_seconds != 0,
            "runtime peer mailbox binding metadata is invalid"
        );
        let offer = EncryptedMailboxOffer::decode(&self.content.encrypted_offer)?;
        ensure!(
            offer.binding_id() == self.binding_id(),
            "runtime peer mailbox offer does not match local metadata"
        );
        self.local_device_id().verify(
            &signing_bytes(PEER_SIGNATURE_DOMAIN, &self.content)?,
            &self.signature,
        )?;
        Ok(())
    }

    pub fn open(
        &self,
        recipient_encryption: &DeviceEncryptionIdentity,
        recipient_certificate: &DeviceCertificate,
        expected_owner_certificate: &DeviceCertificate,
        now_unix_seconds: u64,
    ) -> Result<PeerMailboxBinding> {
        self.verify_local(
            recipient_certificate.account_id(),
            recipient_certificate.device_id(),
        )?;
        let opened = EncryptedMailboxOffer::decode(&self.content.encrypted_offer)?.open(
            recipient_encryption,
            recipient_certificate,
            expected_owner_certificate,
            mailbox_scope(self.conversation_id()),
            now_unix_seconds,
        )?;
        ensure!(
            opened.binding_id()? == self.binding_id()
                && opened.owner_account_id() == self.peer_account_id()
                && opened.owner_device_id() == self.peer_device_id()
                && opened.recipient_account_id() == self.local_account_id()
                && opened.recipient_device_id() == self.local_device_id(),
            "opened peer mailbox capability does not match runtime metadata"
        );
        Ok(opened)
    }

    pub fn local_account_id(&self) -> AccountId {
        self.content.local_account_id
    }

    pub fn local_device_id(&self) -> DeviceId {
        self.content.local_device_id
    }

    pub fn contact_id(&self) -> RuntimeContactId {
        self.content.contact_id
    }

    pub fn peer_account_id(&self) -> AccountId {
        self.content.peer_account_id
    }

    pub fn peer_device_id(&self) -> DeviceId {
        self.content.peer_device_id
    }

    pub fn conversation_id(&self) -> ConversationId {
        self.content.conversation_id
    }

    pub fn binding_id(&self) -> MailboxBindingId {
        self.content.binding_id
    }

    pub fn imported_at_unix_seconds(&self) -> u64 {
        self.content.imported_at_unix_seconds
    }

    pub fn matches_offer(&self, offer: &EncryptedMailboxOffer) -> Result<bool> {
        Ok(self.content.encrypted_offer == offer.encode()?)
    }
}

pub fn mailbox_scope(conversation_id: ConversationId) -> MailboxScope {
    MailboxScope::from_bytes(*conversation_id.as_bytes())
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RuntimeMailboxPayloadContent {
    version: u8,
    binding_id: MailboxBindingId,
    source_account_id: AccountId,
    source_device_id: DeviceId,
    recipient_account_id: AccountId,
    recipient_device_id: DeviceId,
    conversation_id: ConversationId,
    event: AuthorizedEvent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeMailboxPayload(RuntimeMailboxPayloadContent);

impl RuntimeMailboxPayload {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        binding_id: MailboxBindingId,
        source_account_id: AccountId,
        source_device_id: DeviceId,
        recipient_account_id: AccountId,
        recipient_device_id: DeviceId,
        conversation_id: ConversationId,
        event: AuthorizedEvent,
    ) -> Result<Self> {
        let payload = Self(RuntimeMailboxPayloadContent {
            version: PAYLOAD_VERSION,
            binding_id,
            source_account_id,
            source_device_id,
            recipient_account_id,
            recipient_device_id,
            conversation_id,
            event,
        });
        payload.verify_for(
            binding_id,
            source_account_id,
            source_device_id,
            recipient_account_id,
            recipient_device_id,
            conversation_id,
        )?;
        Ok(payload)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = postcard::to_allocvec(self).context("encode runtime mailbox payload")?;
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_MAILBOX_PLAINTEXT_BYTES,
            "runtime mailbox payload size is invalid"
        );
        Ok(bytes)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn decode_and_verify(
        bytes: &[u8],
        binding_id: MailboxBindingId,
        source_account_id: AccountId,
        source_device_id: DeviceId,
        recipient_account_id: AccountId,
        recipient_device_id: DeviceId,
        conversation_id: ConversationId,
    ) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_MAILBOX_PLAINTEXT_BYTES,
            "runtime mailbox payload size is invalid"
        );
        let payload: Self =
            postcard::from_bytes(bytes).context("decode runtime mailbox payload")?;
        payload.verify_for(
            binding_id,
            source_account_id,
            source_device_id,
            recipient_account_id,
            recipient_device_id,
            conversation_id,
        )?;
        Ok(payload)
    }

    #[allow(clippy::too_many_arguments)]
    fn verify_for(
        &self,
        binding_id: MailboxBindingId,
        source_account_id: AccountId,
        source_device_id: DeviceId,
        recipient_account_id: AccountId,
        recipient_device_id: DeviceId,
        conversation_id: ConversationId,
    ) -> Result<()> {
        self.validate()?;
        ensure!(
            self.0.binding_id == binding_id
                && self.0.source_account_id == source_account_id
                && self.0.source_device_id == source_device_id
                && self.0.recipient_account_id == recipient_account_id
                && self.0.recipient_device_id == recipient_device_id
                && self.0.conversation_id == conversation_id,
            "runtime mailbox payload identity, binding, or conversation mismatch"
        );
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.0.version == PAYLOAD_VERSION
                && self.0.source_account_id != self.0.recipient_account_id,
            "runtime mailbox payload metadata is invalid"
        );
        self.0.event.verify_author()?;
        ensure!(
            self.0.event.author_account_id() == self.0.source_account_id
                && self.0.event.event().author_device_id() == self.0.source_device_id
                && self.0.event.event().conversation_id() == self.0.conversation_id,
            "runtime mailbox payload event does not match its authenticated source"
        );
        Ok(())
    }

    pub fn event(&self) -> &AuthorizedEvent {
        &self.0.event
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RuntimeMailboxDispatchContent {
    version: u8,
    local_account_id: AccountId,
    local_device_id: DeviceId,
    queue_id: crate::runtime_queue::RuntimeQueueId,
    contact_id: RuntimeContactId,
    peer_account_id: AccountId,
    peer_device_id: DeviceId,
    conversation_id: ConversationId,
    binding_id: MailboxBindingId,
    mailbox_id: MailboxId,
    item_id: MailboxItemId,
    event_id: EventId,
    created_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedRuntimeMailboxDispatch {
    content: RuntimeMailboxDispatchContent,
    signature: Vec<u8>,
}

impl SignedRuntimeMailboxDispatch {
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        identity: &DeviceIdentity,
        local_account_id: AccountId,
        queue_id: crate::runtime_queue::RuntimeQueueId,
        contact_id: RuntimeContactId,
        peer_account_id: AccountId,
        peer_device_id: DeviceId,
        conversation_id: ConversationId,
        binding_id: MailboxBindingId,
        mailbox_id: MailboxId,
        event_id: EventId,
        created_at_unix_seconds: u64,
        expires_at_unix_seconds: u64,
    ) -> Result<Self> {
        let item_id = runtime_mailbox_item_id(queue_id, event_id, binding_id);
        let content = RuntimeMailboxDispatchContent {
            version: VERSION,
            local_account_id,
            local_device_id: identity.device_id(),
            queue_id,
            contact_id,
            peer_account_id,
            peer_device_id,
            conversation_id,
            binding_id,
            mailbox_id,
            item_id,
            event_id,
            created_at_unix_seconds,
            expires_at_unix_seconds,
        };
        let signature = identity
            .sign(&signing_bytes(DISPATCH_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let dispatch = Self { content, signature };
        dispatch.verify_local(local_account_id, identity.device_id())?;
        Ok(dispatch)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify_local(self.local_account_id(), self.local_device_id())?;
        let bytes = postcard::to_allocvec(self).context("encode runtime mailbox dispatch")?;
        ensure!(
            bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "runtime mailbox dispatch is too large"
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "runtime mailbox dispatch size is invalid"
        );
        let dispatch: Self =
            postcard::from_bytes(bytes).context("decode runtime mailbox dispatch")?;
        dispatch.verify_local(dispatch.local_account_id(), dispatch.local_device_id())?;
        Ok(dispatch)
    }

    pub fn verify_local(
        &self,
        expected_account_id: AccountId,
        expected_device_id: DeviceId,
    ) -> Result<()> {
        let validity = self
            .expires_at_unix_seconds()
            .checked_sub(self.created_at_unix_seconds())
            .context("runtime mailbox dispatch expires before creation")?;
        ensure!(
            self.content.version == VERSION
                && self.local_account_id() == expected_account_id
                && self.local_device_id() == expected_device_id
                && self.peer_account_id() != self.local_account_id()
                && self.created_at_unix_seconds() != 0
                && (MIN_MAILBOX_TTL_SECONDS..=MAX_MAILBOX_TTL_SECONDS).contains(&validity)
                && self.item_id()
                    == runtime_mailbox_item_id(self.queue_id(), self.event_id(), self.binding_id()),
            "runtime mailbox dispatch metadata is invalid"
        );
        self.local_device_id().verify(
            &signing_bytes(DISPATCH_SIGNATURE_DOMAIN, &self.content)?,
            &self.signature,
        )?;
        Ok(())
    }

    pub fn local_account_id(&self) -> AccountId {
        self.content.local_account_id
    }

    pub fn local_device_id(&self) -> DeviceId {
        self.content.local_device_id
    }

    pub fn queue_id(&self) -> crate::runtime_queue::RuntimeQueueId {
        self.content.queue_id
    }

    pub fn contact_id(&self) -> RuntimeContactId {
        self.content.contact_id
    }

    pub fn peer_account_id(&self) -> AccountId {
        self.content.peer_account_id
    }

    pub fn peer_device_id(&self) -> DeviceId {
        self.content.peer_device_id
    }

    pub fn conversation_id(&self) -> ConversationId {
        self.content.conversation_id
    }

    pub fn binding_id(&self) -> MailboxBindingId {
        self.content.binding_id
    }

    pub fn mailbox_id(&self) -> MailboxId {
        self.content.mailbox_id
    }

    pub fn item_id(&self) -> MailboxItemId {
        self.content.item_id
    }

    pub fn event_id(&self) -> EventId {
        self.content.event_id
    }

    pub fn created_at_unix_seconds(&self) -> u64 {
        self.content.created_at_unix_seconds
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.content.expires_at_unix_seconds
    }
}

pub fn runtime_mailbox_item_id(
    queue_id: crate::runtime_queue::RuntimeQueueId,
    event_id: EventId,
    binding_id: MailboxBindingId,
) -> MailboxItemId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(ITEM_ID_DOMAIN);
    hasher.update(queue_id.as_bytes());
    hasher.update(event_id.as_bytes());
    hasher.update(binding_id.as_bytes());
    MailboxItemId::from_bytes(*hasher.finalize().as_bytes())
}

pub fn runtime_mailbox_event_item_id(
    event_id: EventId,
    binding_id: MailboxBindingId,
) -> MailboxItemId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(EVENT_ITEM_ID_DOMAIN);
    hasher.update(event_id.as_bytes());
    hasher.update(binding_id.as_bytes());
    MailboxItemId::from_bytes(*hasher.finalize().as_bytes())
}

fn signing_bytes<T: Serialize>(domain: &[u8], content: &T) -> Result<Vec<u8>> {
    let encoded =
        postcard::to_allocvec(content).context("encode runtime mailbox signed content")?;
    let mut bytes = Vec::with_capacity(domain.len() + encoded.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}
