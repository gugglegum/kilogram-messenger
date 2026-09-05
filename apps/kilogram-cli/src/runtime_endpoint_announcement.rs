use std::fmt;

use anyhow::{Context, Result, ensure};
use kilogram_crypto::SealedMessage;
use kilogram_identity::{
    AccountDeviceListSnapshot, AccountId, DeviceEncryptionIdentity, DeviceId, DeviceIdentity,
};
use kilogram_protocol::{ConversationId, SyncSessionBinding};
use kilogram_ticket_publication::TicketPublicationWriteKey;
use kilogram_transport_iroh::RoutePolicy;
use serde::{Deserialize, Serialize};

use crate::runtime_publication::{
    SignedTicketPublicationObservation, TicketPublicationChannelId, TicketPublicationId,
};

const BUNDLE_VERSION: u8 = 1;
const ENVELOPE_VERSION: u8 = 1;
const EVIDENCE_VERSION: u8 = 1;
const ACKNOWLEDGEMENT_VERSION: u8 = 1;
const BUNDLE_SIGNATURE_DOMAIN: &[u8] = b"kilogram:endpoint-announcement-bundle:v1\0";
const BUNDLE_ID_DOMAIN: &[u8] = b"kilogram:endpoint-announcement-bundle-id:v1\0";
const ENVELOPE_HPKE_INFO: &[u8] = b"kilogram:endpoint-announcement-envelope:v1";
const EVIDENCE_SIGNATURE_DOMAIN: &[u8] = b"kilogram:accepted-endpoint-observation:v1\0";
const EVIDENCE_ID_DOMAIN: &[u8] = b"kilogram:accepted-endpoint-observation-id:v1\0";
const ACKNOWLEDGEMENT_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:endpoint-announcement-acknowledgement:v1\0";
const MAX_CLOCK_SKEW_SECONDS: u64 = 5 * 60;
pub const DEFAULT_ENDPOINT_ANNOUNCEMENT_VALIDITY_SECONDS: u64 = 15 * 60;
pub const MAX_ENDPOINT_ANNOUNCEMENT_VALIDITY_SECONDS: u64 = 60 * 60;
pub const MAX_ENDPOINT_ANNOUNCEMENT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_ENDPOINT_ANNOUNCEMENT_ACKNOWLEDGEMENT_BYTES: usize = 4 * 1024;
pub const MAX_ENDPOINT_ANNOUNCEMENT_CONTACTS: usize = 256;
pub const MAX_ENDPOINTS_PER_ANNOUNCEMENT_CONTACT: usize = 4;
const MAX_TICKET_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct EndpointAnnouncementBundleId([u8; 32]);

impl fmt::Display for EndpointAnnouncementBundleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct AcceptedEndpointObservationId([u8; 32]);

impl fmt::Display for AcceptedEndpointObservationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct EndpointAnnouncementAcknowledgementContent {
    version: u8,
    session_binding: SyncSessionBinding,
    bundle_id: EndpointAnnouncementBundleId,
    source_device_id: DeviceId,
    recipient_device_id: DeviceId,
    authority_revision: u64,
    contact_count: usize,
    contact_added_count: usize,
    endpoint_count: usize,
    endpoint_added_count: usize,
    publication_binding_added_count: usize,
    observation_evidence_count: usize,
    observation_evidence_added_count: usize,
}

/// Recipient-signed proof that one exact announcement bundle passed the local import gate.
///
/// The transport-session binding makes a captured acknowledgement unusable for a later push.
/// Retrying the encrypted bundle itself remains safe because the import operation is idempotent.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedEndpointAnnouncementAcknowledgement {
    content: EndpointAnnouncementAcknowledgementContent,
    signature: Vec<u8>,
}

impl SignedEndpointAnnouncementAcknowledgement {
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        identity: &DeviceIdentity,
        session_binding: SyncSessionBinding,
        bundle_id: EndpointAnnouncementBundleId,
        source_device_id: DeviceId,
        authority_revision: u64,
        contact_count: usize,
        contact_added_count: usize,
        endpoint_count: usize,
        endpoint_added_count: usize,
        publication_binding_added_count: usize,
        observation_evidence_count: usize,
        observation_evidence_added_count: usize,
    ) -> Result<Self> {
        let content = EndpointAnnouncementAcknowledgementContent {
            version: ACKNOWLEDGEMENT_VERSION,
            session_binding,
            bundle_id,
            source_device_id,
            recipient_device_id: identity.device_id(),
            authority_revision,
            contact_count,
            contact_added_count,
            endpoint_count,
            endpoint_added_count,
            publication_binding_added_count,
            observation_evidence_count,
            observation_evidence_added_count,
        };
        let signature = identity
            .sign(&signing_bytes(ACKNOWLEDGEMENT_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let acknowledgement = Self { content, signature };
        acknowledgement.verify_signature()?;
        Ok(acknowledgement)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify_signature()?;
        let bytes =
            postcard::to_allocvec(self).context("encode endpoint announcement acknowledgement")?;
        ensure!(
            bytes.len() <= MAX_ENDPOINT_ANNOUNCEMENT_ACKNOWLEDGEMENT_BYTES,
            "endpoint announcement acknowledgement is too large"
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_ENDPOINT_ANNOUNCEMENT_ACKNOWLEDGEMENT_BYTES,
            "endpoint announcement acknowledgement size is invalid"
        );
        let acknowledgement: Self =
            postcard::from_bytes(bytes).context("decode endpoint announcement acknowledgement")?;
        acknowledgement.verify_signature()?;
        Ok(acknowledgement)
    }

    pub fn verify_for_session(
        &self,
        session_binding: SyncSessionBinding,
        bundle_id: EndpointAnnouncementBundleId,
        source_device_id: DeviceId,
        recipient_device_id: DeviceId,
    ) -> Result<()> {
        self.verify_signature()?;
        ensure!(
            self.content.session_binding == session_binding
                && self.content.bundle_id == bundle_id
                && self.content.source_device_id == source_device_id
                && self.content.recipient_device_id == recipient_device_id,
            "endpoint announcement acknowledgement does not match this transfer session"
        );
        Ok(())
    }

    fn verify_signature(&self) -> Result<()> {
        ensure!(
            self.content.version == ACKNOWLEDGEMENT_VERSION,
            "unsupported endpoint announcement acknowledgement version"
        );
        ensure!(
            self.content.source_device_id != self.content.recipient_device_id,
            "endpoint announcement acknowledgement names the same source and recipient"
        );
        ensure!(
            self.content.contact_added_count <= self.content.contact_count
                && self.content.endpoint_added_count <= self.content.endpoint_count
                && self.content.publication_binding_added_count <= self.content.endpoint_count
                && self.content.observation_evidence_count <= self.content.endpoint_count
                && self.content.observation_evidence_added_count
                    <= self.content.observation_evidence_count,
            "endpoint announcement acknowledgement contains inconsistent counts"
        );
        ensure!(
            self.content.contact_count <= MAX_ENDPOINT_ANNOUNCEMENT_CONTACTS
                && self.content.endpoint_count
                    <= MAX_ENDPOINT_ANNOUNCEMENT_CONTACTS
                        .saturating_mul(MAX_ENDPOINTS_PER_ANNOUNCEMENT_CONTACT),
            "endpoint announcement acknowledgement counts exceed protocol bounds"
        );
        self.content
            .recipient_device_id
            .verify(
                &signing_bytes(ACKNOWLEDGEMENT_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify endpoint announcement acknowledgement signature")
    }

    pub fn authority_revision(&self) -> u64 {
        self.content.authority_revision
    }

    pub fn contact_count(&self) -> usize {
        self.content.contact_count
    }

    pub fn contact_added_count(&self) -> usize {
        self.content.contact_added_count
    }

    pub fn endpoint_count(&self) -> usize {
        self.content.endpoint_count
    }

    pub fn endpoint_added_count(&self) -> usize {
        self.content.endpoint_added_count
    }

    pub fn publication_binding_added_count(&self) -> usize {
        self.content.publication_binding_added_count
    }

    pub fn observation_evidence_count(&self) -> usize {
        self.content.observation_evidence_count
    }

    pub fn observation_evidence_added_count(&self) -> usize {
        self.content.observation_evidence_added_count
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EndpointCandidateAnnouncement {
    peer_device_id: DeviceId,
    primary: bool,
    route_policy: RoutePolicy,
    ticket_publication_write_key: TicketPublicationWriteKey,
    ticket: String,
    latest_observation: Option<SignedTicketPublicationObservation>,
}

impl EndpointCandidateAnnouncement {
    pub fn new(
        peer_device_id: DeviceId,
        primary: bool,
        route_policy: RoutePolicy,
        ticket_publication_write_key: TicketPublicationWriteKey,
        ticket: String,
        latest_observation: Option<SignedTicketPublicationObservation>,
    ) -> Result<Self> {
        let value = Self {
            peer_device_id,
            primary,
            route_policy,
            ticket_publication_write_key,
            ticket,
            latest_observation,
        };
        value.verify()?;
        Ok(value)
    }

    fn verify(&self) -> Result<()> {
        ensure!(
            !self.ticket.is_empty() && self.ticket.len() <= MAX_TICKET_BYTES,
            "endpoint announcement contains an invalid ticket size"
        );
        self.ticket_publication_write_key
            .verify()
            .context("verify endpoint announcement publication key")?;
        if let Some(observation) = &self.latest_observation {
            observation.verify_signature()?;
            ensure!(
                observation.channel_id() == self.ticket_publication_write_key.channel_id()
                    && observation.publisher_device_id() == self.peer_device_id,
                "endpoint announcement observation does not match its endpoint"
            );
        }
        Ok(())
    }

    pub fn peer_device_id(&self) -> DeviceId {
        self.peer_device_id
    }

    pub fn primary(&self) -> bool {
        self.primary
    }

    pub fn route_policy(&self) -> RoutePolicy {
        self.route_policy
    }

    pub fn ticket_publication_write_key(&self) -> TicketPublicationWriteKey {
        self.ticket_publication_write_key
    }

    pub fn ticket(&self) -> &str {
        &self.ticket
    }

    pub fn latest_observation(&self) -> Option<&SignedTicketPublicationObservation> {
        self.latest_observation.as_ref()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContactEndpointAnnouncement {
    conversation_label: String,
    conversation_id: ConversationId,
    peer_account_id: AccountId,
    endpoints: Vec<EndpointCandidateAnnouncement>,
}

impl ContactEndpointAnnouncement {
    pub fn new(
        conversation_label: String,
        peer_account_id: AccountId,
        mut endpoints: Vec<EndpointCandidateAnnouncement>,
    ) -> Result<Self> {
        endpoints.sort_by_key(EndpointCandidateAnnouncement::peer_device_id);
        let value = Self {
            conversation_id: ConversationId::from_label(&conversation_label),
            conversation_label,
            peer_account_id,
            endpoints,
        };
        value.verify()?;
        Ok(value)
    }

    fn verify(&self) -> Result<()> {
        ensure!(
            !self.conversation_label.is_empty()
                && self.conversation_label.len() <= 4_096
                && ConversationId::from_label(&self.conversation_label) == self.conversation_id,
            "endpoint announcement has an invalid conversation label"
        );
        ensure!(
            !self.endpoints.is_empty()
                && self.endpoints.len() <= MAX_ENDPOINTS_PER_ANNOUNCEMENT_CONTACT,
            "endpoint announcement candidate count is outside protocol bounds"
        );
        ensure!(
            self.endpoints
                .iter()
                .filter(|endpoint| endpoint.primary)
                .count()
                == 1,
            "endpoint announcement contact must have exactly one primary endpoint"
        );
        let mut previous = None;
        for endpoint in &self.endpoints {
            endpoint.verify()?;
            ensure!(
                previous.is_none_or(|value| value < endpoint.peer_device_id),
                "endpoint announcement candidates are duplicated or non-canonical"
            );
            if let Some(observation) = endpoint.latest_observation() {
                ensure!(
                    observation.publisher_account_id() == self.peer_account_id,
                    "endpoint announcement observation names another peer account"
                );
            }
            previous = Some(endpoint.peer_device_id);
        }
        Ok(())
    }

    pub fn conversation_label(&self) -> &str {
        &self.conversation_label
    }

    pub fn conversation_id(&self) -> ConversationId {
        self.conversation_id
    }

    pub fn peer_account_id(&self) -> AccountId {
        self.peer_account_id
    }

    pub fn endpoints(&self) -> &[EndpointCandidateAnnouncement] {
        &self.endpoints
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct EndpointAnnouncementBundleContent {
    version: u8,
    account_device_list: AccountDeviceListSnapshot,
    source_device_id: DeviceId,
    recipient_device_id: DeviceId,
    created_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
    contacts: Vec<ContactEndpointAnnouncement>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedEndpointAnnouncementBundle {
    content: EndpointAnnouncementBundleContent,
    signature: Vec<u8>,
}

impl SignedEndpointAnnouncementBundle {
    pub fn sign(
        identity: &DeviceIdentity,
        account_device_list: AccountDeviceListSnapshot,
        recipient_device_id: DeviceId,
        created_at_unix_seconds: u64,
        validity_seconds: u64,
        mut contacts: Vec<ContactEndpointAnnouncement>,
    ) -> Result<Self> {
        ensure!(
            (1..=MAX_ENDPOINT_ANNOUNCEMENT_VALIDITY_SECONDS).contains(&validity_seconds),
            "endpoint announcement validity is outside protocol bounds"
        );
        contacts.sort_by(|left, right| {
            left.peer_account_id
                .as_bytes()
                .cmp(right.peer_account_id.as_bytes())
                .then_with(|| left.conversation_id.cmp(&right.conversation_id))
        });
        let content = EndpointAnnouncementBundleContent {
            version: BUNDLE_VERSION,
            account_device_list,
            source_device_id: identity.device_id(),
            recipient_device_id,
            created_at_unix_seconds,
            expires_at_unix_seconds: created_at_unix_seconds
                .checked_add(validity_seconds)
                .context("endpoint announcement expiry overflow")?,
            contacts,
        };
        let signature = identity
            .sign(&signing_bytes(BUNDLE_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let bundle = Self { content, signature };
        bundle.verify_signature()?;
        Ok(bundle)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify_signature()?;
        let bytes = postcard::to_allocvec(self).context("encode endpoint announcement bundle")?;
        ensure!(
            bytes.len() <= MAX_ENDPOINT_ANNOUNCEMENT_BYTES,
            "endpoint announcement bundle is too large"
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_ENDPOINT_ANNOUNCEMENT_BYTES,
            "endpoint announcement bundle is too large"
        );
        let bundle: Self =
            postcard::from_bytes(bytes).context("decode endpoint announcement bundle")?;
        bundle.verify_signature()?;
        Ok(bundle)
    }

    pub fn verify_at(&self, now_unix_seconds: u64) -> Result<()> {
        self.verify_signature()?;
        ensure!(
            self.content.created_at_unix_seconds
                <= now_unix_seconds.saturating_add(MAX_CLOCK_SKEW_SECONDS),
            "endpoint announcement was created too far in the future"
        );
        ensure!(
            now_unix_seconds <= self.content.expires_at_unix_seconds,
            "endpoint announcement has expired"
        );
        Ok(())
    }

    fn verify_signature(&self) -> Result<()> {
        ensure!(
            self.content.version == BUNDLE_VERSION,
            "unsupported endpoint announcement bundle version"
        );
        self.content
            .account_device_list
            .verify()
            .context("verify endpoint announcement account device list")?;
        ensure!(
            self.content
                .account_device_list
                .certificate_for(self.content.source_device_id)
                .is_some()
                && self
                    .content
                    .account_device_list
                    .certificate_for(self.content.recipient_device_id)
                    .is_some(),
            "endpoint announcement source or recipient is absent from its device list"
        );
        ensure!(
            self.content.expires_at_unix_seconds > self.content.created_at_unix_seconds
                && self
                    .content
                    .expires_at_unix_seconds
                    .saturating_sub(self.content.created_at_unix_seconds)
                    <= MAX_ENDPOINT_ANNOUNCEMENT_VALIDITY_SECONDS,
            "endpoint announcement validity is outside protocol bounds"
        );
        ensure!(
            self.content.contacts.len() <= MAX_ENDPOINT_ANNOUNCEMENT_CONTACTS,
            "endpoint announcement contact count is outside protocol bounds"
        );
        let mut previous = None;
        for contact in &self.content.contacts {
            contact.verify()?;
            for endpoint in contact.endpoints() {
                if let Some(observation) = endpoint.latest_observation() {
                    ensure!(
                        observation.local_account_id()
                            == self.content.account_device_list.account_id()
                            && observation.local_device_id() == self.content.source_device_id,
                        "endpoint announcement observation was not made by its source device"
                    );
                }
            }
            let key = (*contact.peer_account_id.as_bytes(), contact.conversation_id);
            ensure!(
                previous.is_none_or(|value| value < key),
                "endpoint announcement contacts are duplicated or non-canonical"
            );
            previous = Some(key);
        }
        self.content
            .source_device_id
            .verify(
                &signing_bytes(BUNDLE_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify endpoint announcement source signature")
    }

    pub fn bundle_id(&self) -> Result<EndpointAnnouncementBundleId> {
        Ok(EndpointAnnouncementBundleId(domain_hash(
            BUNDLE_ID_DOMAIN,
            &self.encode()?,
        )))
    }

    pub fn account_device_list(&self) -> &AccountDeviceListSnapshot {
        &self.content.account_device_list
    }

    pub fn account_id(&self) -> AccountId {
        self.content.account_device_list.account_id()
    }

    pub fn source_device_id(&self) -> DeviceId {
        self.content.source_device_id
    }

    pub fn recipient_device_id(&self) -> DeviceId {
        self.content.recipient_device_id
    }

    pub fn created_at_unix_seconds(&self) -> u64 {
        self.content.created_at_unix_seconds
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.content.expires_at_unix_seconds
    }

    pub fn contacts(&self) -> &[ContactEndpointAnnouncement] {
        &self.content.contacts
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct EndpointAnnouncementEnvelopeAad {
    version: u8,
    account_id: AccountId,
    source_device_id: DeviceId,
    recipient_device_id: DeviceId,
    bundle_id: EndpointAnnouncementBundleId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EncryptedEndpointAnnouncementBundle {
    aad: EndpointAnnouncementEnvelopeAad,
    sealed_bundle: SealedMessage,
}

impl EncryptedEndpointAnnouncementBundle {
    pub fn seal(bundle: &SignedEndpointAnnouncementBundle) -> Result<Self> {
        bundle.verify_signature()?;
        let recipient = bundle
            .account_device_list()
            .certificate_for(bundle.recipient_device_id())
            .context("endpoint announcement recipient certificate is absent")?;
        let aad = EndpointAnnouncementEnvelopeAad {
            version: ENVELOPE_VERSION,
            account_id: bundle.account_id(),
            source_device_id: bundle.source_device_id(),
            recipient_device_id: bundle.recipient_device_id(),
            bundle_id: bundle.bundle_id()?,
        };
        let aad_bytes = postcard::to_allocvec(&aad)
            .context("encode endpoint announcement envelope metadata")?;
        let sealed_bundle = recipient
            .encryption_public_key()
            .seal(&bundle.encode()?, ENVELOPE_HPKE_INFO, &aad_bytes)
            .context("encrypt endpoint announcement for recipient device")?;
        let envelope = Self { aad, sealed_bundle };
        envelope.verify_outer()?;
        Ok(envelope)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify_outer()?;
        let bytes = postcard::to_allocvec(self).context("encode endpoint announcement envelope")?;
        ensure!(
            bytes.len() <= MAX_ENDPOINT_ANNOUNCEMENT_BYTES,
            "endpoint announcement envelope is too large"
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_ENDPOINT_ANNOUNCEMENT_BYTES,
            "endpoint announcement envelope is too large"
        );
        let envelope: Self =
            postcard::from_bytes(bytes).context("decode endpoint announcement envelope")?;
        envelope.verify_outer()?;
        Ok(envelope)
    }

    pub fn open(
        &self,
        recipient_device_id: DeviceId,
        recipient_encryption: &DeviceEncryptionIdentity,
        now_unix_seconds: u64,
    ) -> Result<SignedEndpointAnnouncementBundle> {
        self.verify_outer()?;
        ensure!(
            self.aad.recipient_device_id == recipient_device_id,
            "endpoint announcement is addressed to another device"
        );
        let aad_bytes = postcard::to_allocvec(&self.aad)
            .context("encode endpoint announcement envelope metadata")?;
        let plaintext = recipient_encryption
            .open(&self.sealed_bundle, ENVELOPE_HPKE_INFO, &aad_bytes)
            .context("decrypt endpoint announcement for local device")?;
        let bundle = SignedEndpointAnnouncementBundle::decode(&plaintext)?;
        bundle.verify_at(now_unix_seconds)?;
        ensure!(
            bundle.account_id() == self.aad.account_id
                && bundle.source_device_id() == self.aad.source_device_id
                && bundle.recipient_device_id() == self.aad.recipient_device_id
                && bundle.bundle_id()? == self.aad.bundle_id,
            "endpoint announcement envelope metadata is inconsistent"
        );
        Ok(bundle)
    }

    pub fn bundle_id(&self) -> EndpointAnnouncementBundleId {
        self.aad.bundle_id
    }

    fn verify_outer(&self) -> Result<()> {
        ensure!(
            self.aad.version == ENVELOPE_VERSION,
            "unsupported endpoint announcement envelope version"
        );
        ensure!(
            self.sealed_bundle.encapsulated_key.len() <= 128
                && !self.sealed_bundle.ciphertext.is_empty()
                && self.sealed_bundle.ciphertext.len() <= MAX_ENDPOINT_ANNOUNCEMENT_BYTES,
            "endpoint announcement ciphertext is outside protocol bounds"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AcceptedEndpointObservationContent {
    version: u8,
    local_account_id: AccountId,
    local_device_id: DeviceId,
    source_device_id: DeviceId,
    bundle_id: EndpointAnnouncementBundleId,
    imported_at_unix_seconds: u64,
    source_observation: SignedTicketPublicationObservation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedAcceptedEndpointObservation {
    content: AcceptedEndpointObservationContent,
    signature: Vec<u8>,
}

impl SignedAcceptedEndpointObservation {
    pub fn sign(
        identity: &DeviceIdentity,
        local_account_id: AccountId,
        source_device_id: DeviceId,
        bundle_id: EndpointAnnouncementBundleId,
        imported_at_unix_seconds: u64,
        source_observation: SignedTicketPublicationObservation,
    ) -> Result<Self> {
        let content = AcceptedEndpointObservationContent {
            version: EVIDENCE_VERSION,
            local_account_id,
            local_device_id: identity.device_id(),
            source_device_id,
            bundle_id,
            imported_at_unix_seconds,
            source_observation,
        };
        let signature = identity
            .sign(&signing_bytes(EVIDENCE_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let evidence = Self { content, signature };
        evidence.verify()?;
        Ok(evidence)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify()?;
        postcard::to_allocvec(self).context("encode accepted endpoint observation")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let evidence: Self =
            postcard::from_bytes(bytes).context("decode accepted endpoint observation")?;
        evidence.verify()?;
        Ok(evidence)
    }

    pub fn verify(&self) -> Result<()> {
        ensure!(
            self.content.version == EVIDENCE_VERSION,
            "unsupported accepted endpoint observation version"
        );
        self.content.source_observation.verify_signature()?;
        ensure!(
            self.content.source_observation.local_account_id() == self.content.local_account_id
                && self.content.source_observation.local_device_id()
                    == self.content.source_device_id,
            "accepted endpoint observation source identity is inconsistent"
        );
        self.content
            .local_device_id
            .verify(
                &signing_bytes(EVIDENCE_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify accepted endpoint observation signature")
    }

    pub fn verify_local(&self, account_id: AccountId, device_id: DeviceId) -> Result<()> {
        self.verify()?;
        ensure!(
            self.content.local_account_id == account_id
                && self.content.local_device_id == device_id,
            "accepted endpoint observation belongs to another local identity"
        );
        Ok(())
    }

    pub fn evidence_id(&self) -> Result<AcceptedEndpointObservationId> {
        Ok(AcceptedEndpointObservationId(domain_hash(
            EVIDENCE_ID_DOMAIN,
            &self.encode()?,
        )))
    }

    pub fn channel_id(&self) -> TicketPublicationChannelId {
        self.content.source_observation.channel_id()
    }

    pub fn publisher_account_id(&self) -> AccountId {
        self.content.source_observation.publisher_account_id()
    }

    pub fn publisher_device_id(&self) -> DeviceId {
        self.content.source_observation.publisher_device_id()
    }

    pub fn publication_generation(&self) -> u64 {
        self.content.source_observation.publication_generation()
    }

    pub fn publication_id(&self) -> TicketPublicationId {
        self.content.source_observation.publication_id()
    }

    pub fn ticket_digest(&self) -> [u8; 32] {
        self.content.source_observation.ticket_digest()
    }
}

fn signing_bytes<T: Serialize>(domain: &[u8], content: &T) -> Result<Vec<u8>> {
    let encoded = postcard::to_allocvec(content).context("encode endpoint announcement signing")?;
    let mut bytes = Vec::with_capacity(domain.len() + encoded.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

fn write_hex(formatter: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kilogram_identity::{AccountRootState, DeviceCapability};

    fn fixture() -> Result<(
        DeviceIdentity,
        DeviceEncryptionIdentity,
        DeviceIdentity,
        DeviceEncryptionIdentity,
        AccountDeviceListSnapshot,
    )> {
        let source = DeviceIdentity::generate()?;
        let source_encryption = DeviceEncryptionIdentity::generate()?;
        let recipient = DeviceIdentity::generate()?;
        let recipient_encryption = DeviceEncryptionIdentity::generate()?;
        let root_directory = tempfile::tempdir()?;
        let root = AccountRootState::create(root_directory.path())?;
        let source_certificate = root.issue_device_certificate(
            source.device_id(),
            source_encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let recipient_certificate = root.issue_device_certificate(
            recipient.device_id(),
            recipient_encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let list = root.publish_device_list(&[source_certificate, recipient_certificate])?;
        Ok((
            source,
            source_encryption,
            recipient,
            recipient_encryption,
            list,
        ))
    }

    #[test]
    fn recipient_encrypted_bundle_round_trips_and_expires() -> Result<()> {
        let (source, _, recipient, recipient_encryption, list) = fixture()?;
        let bundle = SignedEndpointAnnouncementBundle::sign(
            &source,
            list,
            recipient.device_id(),
            1_000,
            60,
            Vec::new(),
        )?;
        let envelope = EncryptedEndpointAnnouncementBundle::seal(&bundle)?;
        let decoded = EncryptedEndpointAnnouncementBundle::decode(&envelope.encode()?)?;
        assert_eq!(
            decoded.open(recipient.device_id(), &recipient_encryption, 1_030)?,
            bundle
        );
        assert!(
            decoded
                .open(recipient.device_id(), &recipient_encryption, 1_061)
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn acknowledgement_is_recipient_signed_and_session_bound() -> Result<()> {
        let (source, _, recipient, _, list) = fixture()?;
        let bundle = SignedEndpointAnnouncementBundle::sign(
            &source,
            list.clone(),
            recipient.device_id(),
            1_000,
            300,
            Vec::new(),
        )?;
        let session = SyncSessionBinding::from_transport_label("recipient-endpoint");
        let acknowledgement = SignedEndpointAnnouncementAcknowledgement::sign(
            &recipient,
            session,
            bundle.bundle_id()?,
            source.device_id(),
            list.revision(),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        )?;
        let decoded =
            SignedEndpointAnnouncementAcknowledgement::decode(&acknowledgement.encode()?)?;
        decoded.verify_for_session(
            session,
            bundle.bundle_id()?,
            source.device_id(),
            recipient.device_id(),
        )?;
        assert!(
            decoded
                .verify_for_session(
                    SyncSessionBinding::from_transport_label("replayed-session"),
                    bundle.bundle_id()?,
                    source.device_id(),
                    recipient.device_id(),
                )
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn wrong_recipient_and_tampering_are_rejected() -> Result<()> {
        let (source, source_encryption, recipient, recipient_encryption, list) = fixture()?;
        let bundle = SignedEndpointAnnouncementBundle::sign(
            &source,
            list,
            recipient.device_id(),
            1_000,
            60,
            Vec::new(),
        )?;
        let mut envelope = EncryptedEndpointAnnouncementBundle::seal(&bundle)?;
        assert!(
            envelope
                .open(source.device_id(), &source_encryption, 1_030)
                .is_err()
        );
        envelope.sealed_bundle.ciphertext[0] ^= 1;
        assert!(
            envelope
                .open(recipient.device_id(), &recipient_encryption, 1_030)
                .is_err()
        );
        Ok(())
    }
}
