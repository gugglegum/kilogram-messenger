use anyhow::{Context, Result, ensure};
use kilogram_crypto::DeviceEncryptionIdentity;
use kilogram_identity::{AccountId, DeviceCertificate, DeviceId, DeviceIdentity};
use kilogram_mailbox_provisioning::{
    EncryptedMailboxOffer, LocalMailboxBinding, MailboxBindingId, MailboxScope, PeerMailboxBinding,
    SealedLocalMailboxBinding,
};
use kilogram_protocol::ConversationId;
use serde::{Deserialize, Serialize};

use crate::runtime_queue::{MAX_RUNTIME_RECORD_BYTES, RuntimeContactId};

const VERSION: u8 = 1;
const LOCAL_SIGNATURE_DOMAIN: &[u8] = b"kilogram:runtime-local-mailbox-binding:v1\0";
const PEER_SIGNATURE_DOMAIN: &[u8] = b"kilogram:runtime-peer-mailbox-binding:v1\0";

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

fn signing_bytes<T: Serialize>(domain: &[u8], content: &T) -> Result<Vec<u8>> {
    let encoded =
        postcard::to_allocvec(content).context("encode runtime mailbox signed content")?;
    let mut bytes = Vec::with_capacity(domain.len() + encoded.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}
