use std::{fmt, path::PathBuf};

use anyhow::{Context, Result, ensure};
use kilogram_crypto::SealedMessage;
use kilogram_identity::{AccountId, DeviceEncryptionIdentity, DeviceId, DeviceIdentity};
use kilogram_protocol::{AuthorizedEvent, ConversationId, EventId};
use kilogram_transport_iroh::RoutePolicy;
use serde::{Deserialize, Serialize};

const CONTACT_VERSION: u8 = 1;
const QUEUED_MESSAGE_VERSION: u8 = 1;
const MATERIALIZATION_VERSION: u8 = 1;
const DELIVERY_VERSION: u8 = 1;
const RETRY_VERSION: u8 = 1;
const CONTACT_SIGNATURE_DOMAIN: &[u8] = b"kilogram:runtime-contact:v1\0";
const QUEUE_SIGNATURE_DOMAIN: &[u8] = b"kilogram:runtime-queue:v1\0";
const QUEUE_HPKE_INFO: &[u8] = b"kilogram:runtime-queue-body:v1";
const MATERIALIZATION_SIGNATURE_DOMAIN: &[u8] = b"kilogram:runtime-materialized:v1\0";
const DELIVERY_SIGNATURE_DOMAIN: &[u8] = b"kilogram:runtime-delivered:v1\0";
const RETRY_SIGNATURE_DOMAIN: &[u8] = b"kilogram:runtime-retry:v1\0";
pub const MAX_RUNTIME_MESSAGE_BYTES: usize = 64 * 1024;
pub const MAX_RUNTIME_RECORD_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeContactId([u8; 32]);

impl fmt::Display for RuntimeContactId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeQueueId([u8; 32]);

impl RuntimeQueueId {
    pub fn generate() -> Result<Self> {
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes).context("generate runtime queue ID")?;
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl fmt::Display for RuntimeQueueId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RuntimeContactContent {
    version: u8,
    local_account_id: AccountId,
    local_device_id: DeviceId,
    peer_account_id: AccountId,
    peer_device_id: DeviceId,
    conversation_label: String,
    conversation_id: ConversationId,
    route_policy: RoutePolicy,
    descriptor_file: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedRuntimeContact {
    content: RuntimeContactContent,
    signature: Vec<u8>,
}

impl SignedRuntimeContact {
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        identity: &DeviceIdentity,
        local_account_id: AccountId,
        peer_account_id: AccountId,
        peer_device_id: DeviceId,
        conversation_label: String,
        conversation_id: ConversationId,
        route_policy: RoutePolicy,
        descriptor_file: PathBuf,
    ) -> Result<Self> {
        ensure!(
            descriptor_file.is_absolute(),
            "runtime descriptor path must be absolute"
        );
        ensure!(
            !conversation_label.is_empty() && conversation_label.len() <= 4_096,
            "runtime conversation label must contain between 1 and 4096 bytes"
        );
        ensure!(
            ConversationId::from_label(&conversation_label) == conversation_id,
            "runtime conversation label does not match its conversation ID"
        );
        let content = RuntimeContactContent {
            version: CONTACT_VERSION,
            local_account_id,
            local_device_id: identity.device_id(),
            peer_account_id,
            peer_device_id,
            conversation_label,
            conversation_id,
            route_policy,
            descriptor_file,
        };
        let signature = identity
            .sign(&signing_bytes(CONTACT_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let contact = Self { content, signature };
        contact.verify()?;
        Ok(contact)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify()?;
        postcard::to_allocvec(self).context("encode signed runtime contact")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "runtime contact record is too large"
        );
        let contact: Self = postcard::from_bytes(bytes).context("decode signed runtime contact")?;
        contact.verify()?;
        Ok(contact)
    }

    pub fn verify(&self) -> Result<()> {
        ensure!(
            self.content.version == CONTACT_VERSION,
            "unsupported runtime contact version"
        );
        ensure!(
            self.content.descriptor_file.is_absolute(),
            "runtime descriptor path must be absolute"
        );
        ensure!(
            !self.content.conversation_label.is_empty()
                && self.content.conversation_label.len() <= 4_096
                && ConversationId::from_label(&self.content.conversation_label)
                    == self.content.conversation_id,
            "runtime contact has an invalid conversation label"
        );
        self.content
            .local_device_id
            .verify(
                &signing_bytes(CONTACT_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify runtime contact signature")
    }

    pub fn verify_local(&self, account_id: AccountId, device_id: DeviceId) -> Result<()> {
        self.verify()?;
        ensure!(
            self.local_account_id() == account_id,
            "runtime contact belongs to another local account"
        );
        ensure!(
            self.local_device_id() == device_id,
            "runtime contact belongs to another local device"
        );
        Ok(())
    }

    pub fn contact_id(&self) -> RuntimeContactId {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"kilogram:runtime-contact-id:v1\0");
        hasher.update(self.content.local_account_id.as_bytes());
        hasher.update(self.content.peer_account_id.as_bytes());
        hasher.update(self.content.conversation_id.as_bytes());
        RuntimeContactId(*hasher.finalize().as_bytes())
    }

    pub fn local_account_id(&self) -> AccountId {
        self.content.local_account_id
    }

    pub fn local_device_id(&self) -> DeviceId {
        self.content.local_device_id
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

    pub fn conversation_label(&self) -> &str {
        &self.content.conversation_label
    }

    pub fn route_policy(&self) -> RoutePolicy {
        self.content.route_policy
    }

    pub fn descriptor_file(&self) -> &PathBuf {
        &self.content.descriptor_file
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct QueuedMessageContent {
    version: u8,
    queue_id: RuntimeQueueId,
    contact_id: RuntimeContactId,
    local_account_id: AccountId,
    local_device_id: DeviceId,
    peer_account_id: AccountId,
    conversation_id: ConversationId,
    created_at_unix_seconds: u64,
    sealed_body: SealedMessage,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedQueuedMessage {
    content: QueuedMessageContent,
    signature: Vec<u8>,
}

impl SignedQueuedMessage {
    #[cfg(test)]
    pub fn seal(
        identity: &DeviceIdentity,
        encryption: &DeviceEncryptionIdentity,
        contact: &SignedRuntimeContact,
        body: &str,
        created_at_unix_seconds: u64,
    ) -> Result<Self> {
        Self::seal_with_queue_id(
            identity,
            encryption,
            contact,
            RuntimeQueueId::generate()?,
            body,
            created_at_unix_seconds,
        )
    }

    pub fn seal_with_queue_id(
        identity: &DeviceIdentity,
        encryption: &DeviceEncryptionIdentity,
        contact: &SignedRuntimeContact,
        queue_id: RuntimeQueueId,
        body: &str,
        created_at_unix_seconds: u64,
    ) -> Result<Self> {
        ensure!(!body.is_empty(), "queued message must not be empty");
        ensure!(
            body.len() <= MAX_RUNTIME_MESSAGE_BYTES,
            "queued message is too large"
        );
        let aad = queue_body_aad(
            queue_id,
            contact.contact_id(),
            identity.device_id(),
            contact.peer_account_id(),
            contact.conversation_id(),
            created_at_unix_seconds,
        )?;
        let sealed_body = encryption
            .public_key()
            .seal(body.as_bytes(), QUEUE_HPKE_INFO, &aad)
            .context("seal runtime queued message to the local device")?;
        let content = QueuedMessageContent {
            version: QUEUED_MESSAGE_VERSION,
            queue_id,
            contact_id: contact.contact_id(),
            local_account_id: contact.local_account_id(),
            local_device_id: identity.device_id(),
            peer_account_id: contact.peer_account_id(),
            conversation_id: contact.conversation_id(),
            created_at_unix_seconds,
            sealed_body,
        };
        let signature = identity
            .sign(&signing_bytes(QUEUE_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let queued = Self { content, signature };
        queued.verify()?;
        Ok(queued)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify()?;
        postcard::to_allocvec(self).context("encode signed queued message")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "queued message record is too large"
        );
        let queued: Self = postcard::from_bytes(bytes).context("decode signed queued message")?;
        queued.verify()?;
        Ok(queued)
    }

    pub fn verify(&self) -> Result<()> {
        ensure!(
            self.content.version == QUEUED_MESSAGE_VERSION,
            "unsupported queued message version"
        );
        self.content
            .local_device_id
            .verify(
                &signing_bytes(QUEUE_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify queued message signature")
    }

    pub fn open(&self, encryption: &DeviceEncryptionIdentity) -> Result<String> {
        self.verify()?;
        let aad = queue_body_aad(
            self.queue_id(),
            self.contact_id(),
            self.local_device_id(),
            self.peer_account_id(),
            self.conversation_id(),
            self.created_at_unix_seconds(),
        )?;
        let plaintext = encryption
            .open(&self.content.sealed_body, QUEUE_HPKE_INFO, &aad)
            .context("open runtime queued message on its local device")?;
        ensure!(
            plaintext.len() <= MAX_RUNTIME_MESSAGE_BYTES,
            "opened queued message is too large"
        );
        String::from_utf8(plaintext).context("queued message is not UTF-8")
    }

    pub fn queue_id(&self) -> RuntimeQueueId {
        self.content.queue_id
    }
    pub fn contact_id(&self) -> RuntimeContactId {
        self.content.contact_id
    }
    pub fn local_account_id(&self) -> AccountId {
        self.content.local_account_id
    }
    pub fn local_device_id(&self) -> DeviceId {
        self.content.local_device_id
    }
    pub fn peer_account_id(&self) -> AccountId {
        self.content.peer_account_id
    }
    pub fn conversation_id(&self) -> ConversationId {
        self.content.conversation_id
    }
    pub fn created_at_unix_seconds(&self) -> u64 {
        self.content.created_at_unix_seconds
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct MaterializedMessageContent {
    version: u8,
    queue_id: RuntimeQueueId,
    local_device_id: DeviceId,
    event: AuthorizedEvent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedMaterializedMessage {
    content: MaterializedMessageContent,
    signature: Vec<u8>,
}

impl SignedMaterializedMessage {
    pub fn sign(
        identity: &DeviceIdentity,
        queue_id: RuntimeQueueId,
        event: AuthorizedEvent,
    ) -> Result<Self> {
        let content = MaterializedMessageContent {
            version: MATERIALIZATION_VERSION,
            queue_id,
            local_device_id: identity.device_id(),
            event,
        };
        let signature = identity
            .sign(&signing_bytes(MATERIALIZATION_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let materialized = Self { content, signature };
        materialized.verify()?;
        Ok(materialized)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify()?;
        postcard::to_allocvec(self).context("encode runtime materialization")
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "runtime materialization is too large"
        );
        let value: Self = postcard::from_bytes(bytes).context("decode runtime materialization")?;
        value.verify()?;
        Ok(value)
    }
    pub fn verify(&self) -> Result<()> {
        ensure!(
            self.content.version == MATERIALIZATION_VERSION,
            "unsupported runtime materialization version"
        );
        self.content
            .event
            .verify_author()
            .context("verify materialized event author")?;
        self.content
            .local_device_id
            .verify(
                &signing_bytes(MATERIALIZATION_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify runtime materialization signature")
    }
    pub fn queue_id(&self) -> RuntimeQueueId {
        self.content.queue_id
    }
    pub fn event(&self) -> &AuthorizedEvent {
        &self.content.event
    }
    pub fn local_device_id(&self) -> DeviceId {
        self.content.local_device_id
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct DeliveredMessageContent {
    version: u8,
    queue_id: RuntimeQueueId,
    local_device_id: DeviceId,
    event_id: EventId,
    acknowledgement_event_id: EventId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedDeliveredMessage {
    content: DeliveredMessageContent,
    signature: Vec<u8>,
}

impl SignedDeliveredMessage {
    pub fn sign(
        identity: &DeviceIdentity,
        queue_id: RuntimeQueueId,
        event_id: EventId,
        acknowledgement_event_id: EventId,
    ) -> Result<Self> {
        let content = DeliveredMessageContent {
            version: DELIVERY_VERSION,
            queue_id,
            local_device_id: identity.device_id(),
            event_id,
            acknowledgement_event_id,
        };
        let signature = identity
            .sign(&signing_bytes(DELIVERY_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let value = Self { content, signature };
        value.verify()?;
        Ok(value)
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify()?;
        postcard::to_allocvec(self).context("encode runtime delivery marker")
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "runtime delivery marker is too large"
        );
        let value: Self = postcard::from_bytes(bytes).context("decode runtime delivery marker")?;
        value.verify()?;
        Ok(value)
    }
    pub fn verify(&self) -> Result<()> {
        ensure!(
            self.content.version == DELIVERY_VERSION,
            "unsupported runtime delivery marker version"
        );
        self.content
            .local_device_id
            .verify(
                &signing_bytes(DELIVERY_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify runtime delivery marker signature")
    }
    pub fn queue_id(&self) -> RuntimeQueueId {
        self.content.queue_id
    }
    pub fn event_id(&self) -> EventId {
        self.content.event_id
    }
    pub fn acknowledgement_event_id(&self) -> EventId {
        self.content.acknowledgement_event_id
    }
    pub fn local_device_id(&self) -> DeviceId {
        self.content.local_device_id
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RetryStateContent {
    version: u8,
    queue_id: RuntimeQueueId,
    local_device_id: DeviceId,
    generation: u32,
    previous_state_id: Option<[u8; 32]>,
    not_before_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedRuntimeRetryState {
    content: RetryStateContent,
    signature: Vec<u8>,
}

impl SignedRuntimeRetryState {
    pub fn sign(
        identity: &DeviceIdentity,
        queue_id: RuntimeQueueId,
        previous: Option<&Self>,
        not_before_unix_seconds: u64,
    ) -> Result<Self> {
        let (generation, previous_state_id) = match previous {
            Some(previous) => (
                previous
                    .generation()
                    .checked_add(1)
                    .context("runtime retry generation overflow")?,
                Some(previous.state_id()?),
            ),
            None => (1, None),
        };
        let content = RetryStateContent {
            version: RETRY_VERSION,
            queue_id,
            local_device_id: identity.device_id(),
            generation,
            previous_state_id,
            not_before_unix_seconds,
        };
        let signature = identity
            .sign(&signing_bytes(RETRY_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let value = Self { content, signature };
        value.verify(previous)?;
        Ok(value)
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        postcard::to_allocvec(self).context("encode runtime retry state")
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "runtime retry state is too large"
        );
        let value: Self = postcard::from_bytes(bytes).context("decode runtime retry state")?;
        value.verify_signature()?;
        Ok(value)
    }
    pub fn verify(&self, previous: Option<&Self>) -> Result<()> {
        self.verify_signature()?;
        match previous {
            Some(previous) => {
                ensure!(
                    self.generation() == previous.generation() + 1,
                    "runtime retry generation is not contiguous"
                );
                ensure!(
                    self.content.previous_state_id == Some(previous.state_id()?),
                    "runtime retry previous state ID mismatch"
                );
            }
            None => {
                ensure!(
                    self.generation() == 1 && self.content.previous_state_id.is_none(),
                    "runtime retry chain has an invalid first state"
                );
            }
        }
        Ok(())
    }
    fn verify_signature(&self) -> Result<()> {
        ensure!(
            self.content.version == RETRY_VERSION,
            "unsupported runtime retry state version"
        );
        self.content
            .local_device_id
            .verify(
                &signing_bytes(RETRY_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify runtime retry state signature")
    }
    pub fn state_id(&self) -> Result<[u8; 32]> {
        Ok(*blake3::hash(&self.encode()?).as_bytes())
    }
    pub fn queue_id(&self) -> RuntimeQueueId {
        self.content.queue_id
    }
    pub fn generation(&self) -> u32 {
        self.content.generation
    }
    pub fn not_before_unix_seconds(&self) -> u64 {
        self.content.not_before_unix_seconds
    }
    pub fn local_device_id(&self) -> DeviceId {
        self.content.local_device_id
    }
}

fn signing_bytes<T: Serialize>(domain: &[u8], content: &T) -> Result<Vec<u8>> {
    let encoded = postcard::to_allocvec(content).context("encode runtime signed content")?;
    let mut bytes = Vec::with_capacity(domain.len() + encoded.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
fn queue_body_aad(
    queue_id: RuntimeQueueId,
    contact_id: RuntimeContactId,
    local_device_id: DeviceId,
    peer_account_id: AccountId,
    conversation_id: ConversationId,
    created_at_unix_seconds: u64,
) -> Result<Vec<u8>> {
    postcard::to_allocvec(&(
        QUEUE_HPKE_INFO,
        queue_id,
        contact_id,
        local_device_id,
        peer_account_id,
        conversation_id,
        created_at_unix_seconds,
    ))
    .context("encode runtime queue body AAD")
}

fn write_hex(formatter: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use kilogram_identity::AccountRootState;

    use super::*;

    #[test]
    fn contact_queue_and_retry_are_authenticated_and_restart_safe() -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        let local_root = AccountRootState::create(directory.path().join("local-root"))?;
        let peer_root = AccountRootState::create(directory.path().join("peer-root"))?;
        let local_identity = DeviceIdentity::generate()?;
        let local_encryption = DeviceEncryptionIdentity::generate()?;
        let peer_identity = DeviceIdentity::generate()?;
        let conversation_label = "runtime-queue-test";
        let conversation_id = ConversationId::from_label(conversation_label);
        let descriptor = directory.path().join("peer.ticket");
        let contact = SignedRuntimeContact::sign(
            &local_identity,
            local_root.account_id(),
            peer_root.account_id(),
            peer_identity.device_id(),
            conversation_label.to_owned(),
            conversation_id,
            RoutePolicy::RelayOnly,
            descriptor,
        )?;
        let decoded_contact = SignedRuntimeContact::decode(&contact.encode()?)?;
        assert_eq!(decoded_contact, contact);
        assert_eq!(decoded_contact.contact_id(), contact.contact_id());

        let body = "plaintext that must not be retained in the queue record";
        let queued =
            SignedQueuedMessage::seal(&local_identity, &local_encryption, &contact, body, 42)?;
        let encoded = queued.encode()?;
        assert!(
            !encoded
                .windows(body.len())
                .any(|window| window == body.as_bytes())
        );
        let restarted = SignedQueuedMessage::decode(&encoded)?;
        assert_eq!(restarted.open(&local_encryption)?, body);
        assert!(
            restarted
                .open(&DeviceEncryptionIdentity::generate()?)
                .is_err()
        );

        let first = SignedRuntimeRetryState::sign(&local_identity, restarted.queue_id(), None, 50)?;
        let first_restarted = SignedRuntimeRetryState::decode(&first.encode()?)?;
        first_restarted.verify(None)?;
        let second = SignedRuntimeRetryState::sign(
            &local_identity,
            restarted.queue_id(),
            Some(&first_restarted),
            75,
        )?;
        let second_restarted = SignedRuntimeRetryState::decode(&second.encode()?)?;
        second_restarted.verify(Some(&first_restarted))?;
        assert_eq!(second_restarted.generation(), 2);

        let mut tampered = encoded;
        let last = tampered.len() - 1;
        tampered[last] ^= 1;
        assert!(SignedQueuedMessage::decode(&tampered).is_err());
        Ok(())
    }
}
