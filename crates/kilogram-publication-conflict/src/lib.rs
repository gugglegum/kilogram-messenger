#![forbid(unsafe_code)]

use std::fmt;

use anyhow::{Context, Result, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use kilogram_identity::{AccountId, AccountRootState, DeviceId, DeviceIdentity};
use kilogram_ticket_publication::{TicketPublicationChannelId, TicketPublicationWriteKey};
use serde::{Deserialize, Serialize};

const PUBLICATION_VERSION: u8 = 1;
const OBSERVATION_VERSION: u8 = 1;
const CONFLICT_PROOF_VERSION: u8 = 1;
const RESOLUTION_REQUEST_VERSION: u8 = 1;
const RESOLUTION_VERSION: u8 = 2;
const RESOLUTION_RESPONSE_VERSION: u8 = 1;
const CLAIM_VERSION: u8 = 1;
const PUBLICATION_SIGNATURE_DOMAIN: &[u8] = b"kilogram:ticket-publication:v1\0";
const OBSERVATION_SIGNATURE_DOMAIN: &[u8] = b"kilogram:ticket-publication-observation:v1\0";
const CONFLICT_PROOF_SIGNATURE_DOMAIN: &[u8] = b"kilogram:ticket-publication-conflict-proof:v1\0";
const RESOLUTION_REQUEST_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:publication-conflict-resolution-request:v1\0";
const PUBLICATION_ID_DOMAIN: &[u8] = b"kilogram:ticket-publication-id:v1\0";
const OBSERVATION_ID_DOMAIN: &[u8] = b"kilogram:ticket-publication-observation-id:v1\0";
const CONFLICT_PROOF_ID_DOMAIN: &[u8] = b"kilogram:ticket-publication-conflict-proof-id:v1\0";
const CONFLICT_EVIDENCE_ID_DOMAIN: &[u8] = b"kilogram:ticket-publication-conflict-evidence-id:v1\0";
const RESOLUTION_REQUEST_ID_DOMAIN: &[u8] =
    b"kilogram:publication-conflict-resolution-request-id:v1\0";
const RESOLUTION_ID_DOMAIN: &[u8] = b"kilogram:publication-conflict-resolution-id:v2\0";
const TICKET_DIGEST_DOMAIN: &[u8] = b"kilogram:ticket-publication-ticket-digest:v1\0";
const CONFIRMATION_CODE_DOMAIN: &[u8] = b"kilogram:publication-conflict-confirmation:v1\0";
pub const PUBLICATION_CONFLICT_CLAIM_URI_PREFIX: &str = "kilogram://publication-conflict/v1/";
pub const MAX_PUBLICATION_CONFLICT_CLAIM_URI_BYTES: usize = 3_072;
pub const MIN_TICKET_PUBLICATION_TTL_SECONDS: u64 = 30;
pub const DEFAULT_TICKET_PUBLICATION_TTL_SECONDS: u64 = 15 * 60;
pub const MAX_TICKET_PUBLICATION_TTL_SECONDS: u64 = 60 * 60;
pub const MAX_TICKET_PUBLICATION_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_PUBLICATION_CONFLICT_PROOF_BYTES: usize = 128 * 1024;
pub const MAX_PUBLICATION_CONFLICT_RESOLUTION_BYTES: usize = 8 * 1024;
pub const MAX_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_BYTES: usize = 9 * 1024 * 1024;
pub const MAX_PUBLICATION_CONFLICT_RESOLUTION_RESPONSE_BYTES: usize = 9 * 1024 * 1024;
const MAX_TICKET_TEXT_BYTES: usize = 8 * 1024 * 1024;
const MAX_CLOCK_SKEW_SECONDS: u64 = 5 * 60;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PublicationConflictRoutePolicy {
    Auto,
    DirectOnly,
    RelayOnly,
}

impl PublicationConflictRoutePolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::DirectOnly => "direct-only",
            Self::RelayOnly => "relay-only",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TicketPublicationId([u8; 32]);

impl fmt::Display for TicketPublicationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TicketPublicationObservationId([u8; 32]);

impl fmt::Display for TicketPublicationObservationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct TicketPublicationContent {
    version: u8,
    channel_id: TicketPublicationChannelId,
    publisher_account_id: AccountId,
    publisher_device_id: DeviceId,
    recipient_account_id: AccountId,
    generation: u64,
    previous_publication_id: Option<TicketPublicationId>,
    published_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
    ticket_digest: [u8; 32],
    ticket: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedTicketPublication {
    content: TicketPublicationContent,
    signature: Vec<u8>,
}

impl SignedTicketPublication {
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        identity: &DeviceIdentity,
        channel_id: TicketPublicationChannelId,
        publisher_account_id: AccountId,
        recipient_account_id: AccountId,
        ticket: String,
        published_at_unix_seconds: u64,
        ttl_seconds: u64,
        previous: Option<&Self>,
    ) -> Result<Self> {
        ensure!(
            (MIN_TICKET_PUBLICATION_TTL_SECONDS..=MAX_TICKET_PUBLICATION_TTL_SECONDS)
                .contains(&ttl_seconds),
            "ticket publication TTL must be between {MIN_TICKET_PUBLICATION_TTL_SECONDS} and {MAX_TICKET_PUBLICATION_TTL_SECONDS} seconds"
        );
        ensure!(
            !ticket.is_empty() && ticket.len() <= MAX_TICKET_TEXT_BYTES,
            "ticket publication contains an invalid ticket size"
        );
        let expires_at_unix_seconds = published_at_unix_seconds
            .checked_add(ttl_seconds)
            .context("ticket publication expiry overflow")?;
        let (generation, previous_publication_id) = match previous {
            Some(previous) => {
                previous.verify_signature()?;
                ensure!(
                    previous.channel_id() == channel_id
                        && previous.publisher_account_id() == publisher_account_id
                        && previous.publisher_device_id() == identity.device_id()
                        && previous.recipient_account_id() == recipient_account_id,
                    "ticket publication chain changes channel identity"
                );
                ensure!(
                    published_at_unix_seconds >= previous.published_at_unix_seconds(),
                    "ticket publication chain moves publication time backwards"
                );
                (
                    previous
                        .generation()
                        .checked_add(1)
                        .context("ticket publication generation overflow")?,
                    Some(previous.publication_id()?),
                )
            }
            None => (1, None),
        };
        let content = TicketPublicationContent {
            version: PUBLICATION_VERSION,
            channel_id,
            publisher_account_id,
            publisher_device_id: identity.device_id(),
            recipient_account_id,
            generation,
            previous_publication_id,
            published_at_unix_seconds,
            expires_at_unix_seconds,
            ticket_digest: ticket_digest(ticket.as_bytes()),
            ticket,
        };
        let signature = identity
            .sign(&domain_bytes(PUBLICATION_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let value = Self { content, signature };
        value.verify(previous)?;
        Ok(value)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify_signature()?;
        postcard::to_allocvec(self).context("encode signed ticket publication")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_TICKET_PUBLICATION_BYTES,
            "signed ticket publication size is invalid"
        );
        let value: Self =
            postcard::from_bytes(bytes).context("decode signed ticket publication")?;
        value.verify_signature()?;
        Ok(value)
    }

    pub fn verify(&self, previous: Option<&Self>) -> Result<()> {
        self.verify_signature()?;
        match previous {
            Some(previous) => {
                previous.verify_signature()?;
                ensure!(
                    self.channel_id() == previous.channel_id()
                        && self.publisher_account_id() == previous.publisher_account_id()
                        && self.publisher_device_id() == previous.publisher_device_id()
                        && self.recipient_account_id() == previous.recipient_account_id(),
                    "ticket publication chain changes channel identity"
                );
                ensure!(
                    self.generation()
                        == previous
                            .generation()
                            .checked_add(1)
                            .context("ticket publication generation overflow")?
                        && self.content.previous_publication_id == Some(previous.publication_id()?),
                    "ticket publication chain is not contiguous"
                );
                ensure!(
                    self.published_at_unix_seconds() >= previous.published_at_unix_seconds(),
                    "ticket publication chain moves publication time backwards"
                );
            }
            None => ensure!(
                self.generation() == 1 && self.content.previous_publication_id.is_none(),
                "ticket publication chain has an invalid first entry"
            ),
        }
        Ok(())
    }

    pub fn verify_at(&self, now_unix_seconds: u64) -> Result<()> {
        self.verify_signature()?;
        ensure!(
            self.published_at_unix_seconds()
                <= now_unix_seconds.saturating_add(MAX_CLOCK_SKEW_SECONDS),
            "ticket publication time is too far in the future"
        );
        ensure!(
            self.expires_at_unix_seconds() > now_unix_seconds,
            "ticket publication has expired"
        );
        Ok(())
    }

    pub fn verify_signature(&self) -> Result<()> {
        ensure!(
            self.content.version == PUBLICATION_VERSION,
            "unsupported ticket publication version"
        );
        ensure!(
            !self.content.ticket.is_empty()
                && self.content.ticket.len() <= MAX_TICKET_TEXT_BYTES
                && self.content.ticket_digest == ticket_digest(self.content.ticket.as_bytes()),
            "ticket publication ticket digest is invalid"
        );
        let lifetime = self
            .content
            .expires_at_unix_seconds
            .checked_sub(self.content.published_at_unix_seconds)
            .context("ticket publication expiry precedes publication time")?;
        ensure!(
            (MIN_TICKET_PUBLICATION_TTL_SECONDS..=MAX_TICKET_PUBLICATION_TTL_SECONDS)
                .contains(&lifetime),
            "ticket publication lifetime is outside protocol bounds"
        );
        ensure!(
            self.content.generation != 0,
            "ticket publication generation must be non-zero"
        );
        self.content
            .publisher_device_id
            .verify(
                &domain_bytes(PUBLICATION_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify ticket publication device signature")
    }

    pub fn publication_id(&self) -> Result<TicketPublicationId> {
        Ok(TicketPublicationId(domain_hash(
            PUBLICATION_ID_DOMAIN,
            &self.encode()?,
        )))
    }

    pub fn channel_id(&self) -> TicketPublicationChannelId {
        self.content.channel_id
    }

    pub fn publisher_account_id(&self) -> AccountId {
        self.content.publisher_account_id
    }

    pub fn publisher_device_id(&self) -> DeviceId {
        self.content.publisher_device_id
    }

    pub fn recipient_account_id(&self) -> AccountId {
        self.content.recipient_account_id
    }

    pub fn generation(&self) -> u64 {
        self.content.generation
    }

    pub fn published_at_unix_seconds(&self) -> u64 {
        self.content.published_at_unix_seconds
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.content.expires_at_unix_seconds
    }

    pub fn ticket_digest(&self) -> [u8; 32] {
        self.content.ticket_digest
    }

    pub fn ticket(&self) -> &str {
        &self.content.ticket
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct TicketPublicationObservationContent {
    version: u8,
    local_account_id: AccountId,
    local_device_id: DeviceId,
    channel_id: TicketPublicationChannelId,
    publisher_account_id: AccountId,
    publisher_device_id: DeviceId,
    observation_generation: u64,
    previous_observation_id: Option<TicketPublicationObservationId>,
    publication_generation: u64,
    publication_id: TicketPublicationId,
    ticket_digest: [u8; 32],
    observed_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedTicketPublicationObservation {
    content: TicketPublicationObservationContent,
    signature: Vec<u8>,
}

impl SignedTicketPublicationObservation {
    pub fn sign(
        identity: &DeviceIdentity,
        local_account_id: AccountId,
        publication: &SignedTicketPublication,
        observed_at_unix_seconds: u64,
        previous: Option<&Self>,
    ) -> Result<Self> {
        publication.verify_at(observed_at_unix_seconds)?;
        ensure!(
            publication.recipient_account_id() == local_account_id,
            "ticket publication is addressed to another account"
        );
        let (observation_generation, previous_observation_id) = match previous {
            Some(previous) => {
                previous.verify_signature()?;
                ensure!(
                    previous.local_account_id() == local_account_id
                        && previous.local_device_id() == identity.device_id()
                        && previous.channel_id() == publication.channel_id()
                        && previous.publisher_account_id() == publication.publisher_account_id()
                        && previous.publisher_device_id() == publication.publisher_device_id(),
                    "ticket publication observation chain changes identity"
                );
                ensure!(
                    publication.generation() > previous.publication_generation(),
                    "ticket publication observation does not advance the peer high-water mark"
                );
                (
                    previous
                        .observation_generation()
                        .checked_add(1)
                        .context("ticket publication observation generation overflow")?,
                    Some(previous.observation_id()?),
                )
            }
            None => (1, None),
        };
        let content = TicketPublicationObservationContent {
            version: OBSERVATION_VERSION,
            local_account_id,
            local_device_id: identity.device_id(),
            channel_id: publication.channel_id(),
            publisher_account_id: publication.publisher_account_id(),
            publisher_device_id: publication.publisher_device_id(),
            observation_generation,
            previous_observation_id,
            publication_generation: publication.generation(),
            publication_id: publication.publication_id()?,
            ticket_digest: publication.ticket_digest(),
            observed_at_unix_seconds,
        };
        let signature = identity
            .sign(&domain_bytes(OBSERVATION_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let value = Self { content, signature };
        value.verify(previous)?;
        Ok(value)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify_signature()?;
        postcard::to_allocvec(self).context("encode ticket publication observation")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_TICKET_PUBLICATION_BYTES,
            "ticket publication observation size is invalid"
        );
        let value: Self =
            postcard::from_bytes(bytes).context("decode ticket publication observation")?;
        value.verify_signature()?;
        Ok(value)
    }

    pub fn verify(&self, previous: Option<&Self>) -> Result<()> {
        self.verify_signature()?;
        match previous {
            Some(previous) => {
                previous.verify_signature()?;
                ensure!(
                    self.local_account_id() == previous.local_account_id()
                        && self.local_device_id() == previous.local_device_id()
                        && self.channel_id() == previous.channel_id()
                        && self.publisher_account_id() == previous.publisher_account_id()
                        && self.publisher_device_id() == previous.publisher_device_id(),
                    "ticket publication observation chain changes identity"
                );
                ensure!(
                    self.observation_generation()
                        == previous
                            .observation_generation()
                            .checked_add(1)
                            .context("ticket publication observation generation overflow")?
                        && self.content.previous_observation_id == Some(previous.observation_id()?),
                    "ticket publication observation chain is not contiguous"
                );
                ensure!(
                    self.publication_generation() > previous.publication_generation()
                        && self.observed_at_unix_seconds() >= previous.observed_at_unix_seconds(),
                    "ticket publication observation rolls its high-water mark back"
                );
            }
            None => ensure!(
                self.observation_generation() == 1
                    && self.content.previous_observation_id.is_none()
                    && self.publication_generation() != 0,
                "ticket publication observation chain has an invalid first entry"
            ),
        }
        Ok(())
    }

    pub fn verify_signature(&self) -> Result<()> {
        ensure!(
            self.content.version == OBSERVATION_VERSION,
            "unsupported ticket publication observation version"
        );
        ensure!(
            self.content.observation_generation != 0 && self.content.publication_generation != 0,
            "ticket publication observation generation must be non-zero"
        );
        self.content
            .local_device_id
            .verify(
                &domain_bytes(OBSERVATION_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify ticket publication observation signature")
    }

    pub fn observation_id(&self) -> Result<TicketPublicationObservationId> {
        Ok(TicketPublicationObservationId(domain_hash(
            OBSERVATION_ID_DOMAIN,
            &self.encode()?,
        )))
    }

    pub fn local_account_id(&self) -> AccountId {
        self.content.local_account_id
    }

    pub fn local_device_id(&self) -> DeviceId {
        self.content.local_device_id
    }

    pub fn channel_id(&self) -> TicketPublicationChannelId {
        self.content.channel_id
    }

    pub fn publisher_account_id(&self) -> AccountId {
        self.content.publisher_account_id
    }

    pub fn publisher_device_id(&self) -> DeviceId {
        self.content.publisher_device_id
    }

    pub fn observation_generation(&self) -> u64 {
        self.content.observation_generation
    }

    pub fn publication_generation(&self) -> u64 {
        self.content.publication_generation
    }

    pub fn publication_id(&self) -> TicketPublicationId {
        self.content.publication_id
    }

    pub fn ticket_digest(&self) -> [u8; 32] {
        self.content.ticket_digest
    }

    pub fn observed_at_unix_seconds(&self) -> u64 {
        self.content.observed_at_unix_seconds
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct PublicationConflictProofId([u8; 32]);

impl fmt::Display for PublicationConflictProofId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct PublicationConflictEvidenceId([u8; 32]);

impl fmt::Display for PublicationConflictEvidenceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PublicationConflictProofContent {
    version: u8,
    local_account_id: AccountId,
    detector_device_id: DeviceId,
    detected_at_unix_seconds: u64,
    channel_id: TicketPublicationChannelId,
    publication_generation: u64,
    first_observation: SignedTicketPublicationObservation,
    conflicting_observation: SignedTicketPublicationObservation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedPublicationConflictProof {
    content: PublicationConflictProofContent,
    signature: Vec<u8>,
}

impl SignedPublicationConflictProof {
    pub fn sign(
        identity: &DeviceIdentity,
        local_account_id: AccountId,
        detected_at_unix_seconds: u64,
        left: SignedTicketPublicationObservation,
        right: SignedTicketPublicationObservation,
    ) -> Result<Self> {
        let (first_observation, conflicting_observation) = canonical_observation_pair(left, right)?;
        verify_conflict_pair(
            local_account_id,
            &first_observation,
            &conflicting_observation,
        )?;
        let content = PublicationConflictProofContent {
            version: CONFLICT_PROOF_VERSION,
            local_account_id,
            detector_device_id: identity.device_id(),
            detected_at_unix_seconds,
            channel_id: first_observation.channel_id(),
            publication_generation: first_observation.publication_generation(),
            first_observation,
            conflicting_observation,
        };
        let signature = identity
            .sign(&domain_bytes(CONFLICT_PROOF_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let value = Self { content, signature };
        value.verify()?;
        Ok(value)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify()?;
        let bytes = postcard::to_allocvec(self).context("encode publication conflict proof")?;
        ensure!(
            bytes.len() <= MAX_PUBLICATION_CONFLICT_PROOF_BYTES,
            "publication conflict proof is too large"
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_PUBLICATION_CONFLICT_PROOF_BYTES,
            "publication conflict proof size is invalid"
        );
        let value: Self =
            postcard::from_bytes(bytes).context("decode publication conflict proof")?;
        value.verify()?;
        Ok(value)
    }

    pub fn verify_local(
        &self,
        expected_account_id: AccountId,
        expected_device_id: DeviceId,
    ) -> Result<()> {
        self.verify()?;
        ensure!(
            self.local_account_id() == expected_account_id
                && self.detector_device_id() == expected_device_id,
            "publication conflict proof belongs to another local identity"
        );
        Ok(())
    }

    pub fn verify(&self) -> Result<()> {
        ensure!(
            self.content.version == CONFLICT_PROOF_VERSION,
            "unsupported publication conflict proof version"
        );
        ensure!(
            self.content.detected_at_unix_seconds != 0,
            "publication conflict detection time must be non-zero"
        );
        verify_conflict_pair(
            self.content.local_account_id,
            &self.content.first_observation,
            &self.content.conflicting_observation,
        )?;
        ensure!(
            self.content.channel_id == self.content.first_observation.channel_id()
                && self.content.publication_generation
                    == self.content.first_observation.publication_generation(),
            "publication conflict proof summary does not match its observations"
        );
        ensure!(
            self.content.first_observation.observation_id()?
                < self.content.conflicting_observation.observation_id()?,
            "publication conflict proof observations are not canonical"
        );
        self.content
            .detector_device_id
            .verify(
                &domain_bytes(CONFLICT_PROOF_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify publication conflict proof detector signature")
    }

    pub fn proof_id(&self) -> Result<PublicationConflictProofId> {
        Ok(PublicationConflictProofId(domain_hash(
            CONFLICT_PROOF_ID_DOMAIN,
            &self.encode()?,
        )))
    }

    pub fn evidence_id(&self) -> Result<PublicationConflictEvidenceId> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(CONFLICT_EVIDENCE_ID_DOMAIN);
        for observation in [
            &self.content.first_observation,
            &self.content.conflicting_observation,
        ] {
            let encoded = observation.encode()?;
            hasher.update(&(encoded.len() as u64).to_be_bytes());
            hasher.update(&encoded);
        }
        Ok(PublicationConflictEvidenceId(*hasher.finalize().as_bytes()))
    }

    pub fn local_account_id(&self) -> AccountId {
        self.content.local_account_id
    }

    pub fn detector_device_id(&self) -> DeviceId {
        self.content.detector_device_id
    }

    pub fn detected_at_unix_seconds(&self) -> u64 {
        self.content.detected_at_unix_seconds
    }

    pub fn channel_id(&self) -> TicketPublicationChannelId {
        self.content.channel_id
    }

    pub fn publication_generation(&self) -> u64 {
        self.content.publication_generation
    }

    pub fn publisher_account_id(&self) -> AccountId {
        self.content.first_observation.publisher_account_id()
    }

    pub fn publisher_device_id(&self) -> DeviceId {
        self.content.first_observation.publisher_device_id()
    }

    pub fn observations(
        &self,
    ) -> (
        &SignedTicketPublicationObservation,
        &SignedTicketPublicationObservation,
    ) {
        (
            &self.content.first_observation,
            &self.content.conflicting_observation,
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct PublicationConflictResolutionRequestId([u8; 32]);

impl fmt::Display for PublicationConflictResolutionRequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct PublicationConflictResolutionId([u8; 32]);

impl fmt::Display for PublicationConflictResolutionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PublicationConflictResolutionRequestContent {
    version: u8,
    local_account_id: AccountId,
    requester_device_id: DeviceId,
    authority_revision: u64,
    conflict_proof: SignedPublicationConflictProof,
    peer_account_id: AccountId,
    peer_device_id: DeviceId,
    old_write_key: TicketPublicationWriteKey,
    new_write_key: TicketPublicationWriteKey,
    old_channel_epoch: u64,
    new_channel_epoch: u64,
    route_policy: PublicationConflictRoutePolicy,
    replacement_ticket: Vec<u8>,
    requested_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedPublicationConflictResolutionRequest {
    content: PublicationConflictResolutionRequestContent,
    signature: Vec<u8>,
}

impl SignedPublicationConflictResolutionRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        identity: &DeviceIdentity,
        local_account_id: AccountId,
        authority_revision: u64,
        conflict_proof: SignedPublicationConflictProof,
        old_write_key: TicketPublicationWriteKey,
        new_write_key: TicketPublicationWriteKey,
        old_channel_epoch: u64,
        new_channel_epoch: u64,
        route_policy: PublicationConflictRoutePolicy,
        replacement_ticket: Vec<u8>,
        requested_at_unix_seconds: u64,
    ) -> Result<Self> {
        let content = PublicationConflictResolutionRequestContent {
            version: RESOLUTION_REQUEST_VERSION,
            local_account_id,
            requester_device_id: identity.device_id(),
            authority_revision,
            peer_account_id: conflict_proof.publisher_account_id(),
            peer_device_id: conflict_proof.publisher_device_id(),
            conflict_proof,
            old_write_key,
            new_write_key,
            old_channel_epoch,
            new_channel_epoch,
            route_policy,
            replacement_ticket,
            requested_at_unix_seconds,
        };
        let signature = identity
            .sign(&domain_bytes(
                RESOLUTION_REQUEST_SIGNATURE_DOMAIN,
                &content,
            )?)
            .to_vec();
        let value = Self { content, signature };
        value.verify()?;
        Ok(value)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify()?;
        let bytes = postcard::to_allocvec(self)
            .context("encode publication conflict resolution request")?;
        ensure!(
            bytes.len() <= MAX_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_BYTES,
            "publication conflict resolution request is too large"
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_BYTES,
            "publication conflict resolution request size is invalid"
        );
        let value: Self = postcard::from_bytes(bytes)
            .context("decode publication conflict resolution request")?;
        value.verify()?;
        Ok(value)
    }

    pub fn verify(&self) -> Result<()> {
        ensure!(
            self.content.version == RESOLUTION_REQUEST_VERSION,
            "unsupported publication conflict resolution request version"
        );
        self.content.conflict_proof.verify()?;
        self.content.old_write_key.verify()?;
        self.content.new_write_key.verify()?;
        ensure!(
            self.content.local_account_id == self.content.conflict_proof.local_account_id()
                && self.content.peer_account_id
                    == self.content.conflict_proof.publisher_account_id()
                && self.content.peer_device_id == self.content.conflict_proof.publisher_device_id()
                && self.content.old_write_key.channel_id()
                    == self.content.conflict_proof.channel_id(),
            "publication conflict resolution request does not match its evidence"
        );
        ensure!(
            self.content.old_write_key != self.content.new_write_key
                && self.content.new_channel_epoch > self.content.old_channel_epoch,
            "publication conflict resolution request is not a forward channel rotation"
        );
        ensure!(
            !self.content.replacement_ticket.is_empty()
                && self.content.replacement_ticket.len()
                    <= MAX_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_BYTES,
            "publication conflict resolution request replacement ticket size is invalid"
        );
        ensure!(
            self.content.requested_at_unix_seconds != 0,
            "publication conflict resolution request time must be non-zero"
        );
        self.content
            .requester_device_id
            .verify(
                &domain_bytes(RESOLUTION_REQUEST_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify publication conflict resolution request Device signature")
    }

    pub fn request_id(&self) -> Result<PublicationConflictResolutionRequestId> {
        Ok(PublicationConflictResolutionRequestId(domain_hash(
            RESOLUTION_REQUEST_ID_DOMAIN,
            &self.encode()?,
        )))
    }

    pub fn local_account_id(&self) -> AccountId {
        self.content.local_account_id
    }

    pub fn requester_device_id(&self) -> DeviceId {
        self.content.requester_device_id
    }

    pub fn authority_revision(&self) -> u64 {
        self.content.authority_revision
    }

    pub fn conflict_proof(&self) -> &SignedPublicationConflictProof {
        &self.content.conflict_proof
    }

    pub fn peer_account_id(&self) -> AccountId {
        self.content.peer_account_id
    }

    pub fn peer_device_id(&self) -> DeviceId {
        self.content.peer_device_id
    }

    pub fn old_write_key(&self) -> TicketPublicationWriteKey {
        self.content.old_write_key
    }

    pub fn new_write_key(&self) -> TicketPublicationWriteKey {
        self.content.new_write_key
    }

    pub fn replacement_ticket(&self) -> &[u8] {
        &self.content.replacement_ticket
    }

    pub fn old_channel_epoch(&self) -> u64 {
        self.content.old_channel_epoch
    }

    pub fn new_channel_epoch(&self) -> u64 {
        self.content.new_channel_epoch
    }

    pub fn route_policy(&self) -> PublicationConflictRoutePolicy {
        self.content.route_policy
    }

    pub fn replacement_ticket_digest(&self) -> [u8; 32] {
        *blake3::hash(&self.content.replacement_ticket).as_bytes()
    }

    pub fn requested_at_unix_seconds(&self) -> u64 {
        self.content.requested_at_unix_seconds
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PublicationConflictResolutionContent {
    version: u8,
    request_id: PublicationConflictResolutionRequestId,
    local_account_id: AccountId,
    authority_revision: u64,
    conflict_evidence_id: PublicationConflictEvidenceId,
    peer_account_id: AccountId,
    peer_device_id: DeviceId,
    old_write_key: TicketPublicationWriteKey,
    new_write_key: TicketPublicationWriteKey,
    replacement_ticket_digest: [u8; 32],
    authorized_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RootSignedPublicationConflictResolution {
    content: PublicationConflictResolutionContent,
    signature: Vec<u8>,
}

impl RootSignedPublicationConflictResolution {
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        root: &AccountRootState,
        request_id: PublicationConflictResolutionRequestId,
        authority_revision: u64,
        conflict_evidence_id: PublicationConflictEvidenceId,
        peer_account_id: AccountId,
        peer_device_id: DeviceId,
        old_write_key: TicketPublicationWriteKey,
        new_write_key: TicketPublicationWriteKey,
        replacement_ticket_digest: [u8; 32],
        authorized_at_unix_seconds: u64,
    ) -> Result<Self> {
        let content = PublicationConflictResolutionContent {
            version: RESOLUTION_VERSION,
            request_id,
            local_account_id: root.account_id(),
            authority_revision,
            conflict_evidence_id,
            peer_account_id,
            peer_device_id,
            old_write_key,
            new_write_key,
            replacement_ticket_digest,
            authorized_at_unix_seconds,
        };
        let signature = root.sign_publication_conflict_resolution(&content_bytes(&content)?);
        let value = Self { content, signature };
        value.verify()?;
        Ok(value)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify()?;
        let bytes =
            postcard::to_allocvec(self).context("encode publication conflict resolution")?;
        ensure!(
            bytes.len() <= MAX_PUBLICATION_CONFLICT_RESOLUTION_BYTES,
            "publication conflict resolution is too large"
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_PUBLICATION_CONFLICT_RESOLUTION_BYTES,
            "publication conflict resolution size is invalid"
        );
        let value: Self =
            postcard::from_bytes(bytes).context("decode publication conflict resolution")?;
        value.verify()?;
        Ok(value)
    }

    pub fn verify(&self) -> Result<()> {
        ensure!(
            self.content.version == RESOLUTION_VERSION,
            "unsupported publication conflict resolution version"
        );
        self.content.old_write_key.verify()?;
        self.content.new_write_key.verify()?;
        ensure!(
            self.content.old_write_key != self.content.new_write_key,
            "publication conflict resolution does not rotate the channel"
        );
        ensure!(
            self.content.authorized_at_unix_seconds != 0,
            "publication conflict resolution time must be non-zero"
        );
        self.content
            .local_account_id
            .verify_publication_conflict_resolution(&content_bytes(&self.content)?, &self.signature)
            .context("verify publication conflict resolution Account Root signature")
    }

    pub fn resolution_id(&self) -> Result<PublicationConflictResolutionId> {
        Ok(PublicationConflictResolutionId(domain_hash(
            RESOLUTION_ID_DOMAIN,
            &self.encode()?,
        )))
    }

    pub fn local_account_id(&self) -> AccountId {
        self.content.local_account_id
    }

    pub fn request_id(&self) -> PublicationConflictResolutionRequestId {
        self.content.request_id
    }

    pub fn authority_revision(&self) -> u64 {
        self.content.authority_revision
    }

    pub fn conflict_evidence_id(&self) -> PublicationConflictEvidenceId {
        self.content.conflict_evidence_id
    }

    pub fn peer_account_id(&self) -> AccountId {
        self.content.peer_account_id
    }

    pub fn peer_device_id(&self) -> DeviceId {
        self.content.peer_device_id
    }

    pub fn old_write_key(&self) -> TicketPublicationWriteKey {
        self.content.old_write_key
    }

    pub fn new_write_key(&self) -> TicketPublicationWriteKey {
        self.content.new_write_key
    }

    pub fn replacement_ticket_digest(&self) -> [u8; 32] {
        self.content.replacement_ticket_digest
    }

    pub fn authorized_at_unix_seconds(&self) -> u64 {
        self.content.authorized_at_unix_seconds
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PublicationConflictResolutionResponseContent {
    version: u8,
    request_id: PublicationConflictResolutionRequestId,
    resolution: RootSignedPublicationConflictResolution,
    replacement_ticket: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PublicationConflictResolutionResponse {
    content: PublicationConflictResolutionResponseContent,
}

impl PublicationConflictResolutionResponse {
    pub fn new(
        request: &SignedPublicationConflictResolutionRequest,
        resolution: RootSignedPublicationConflictResolution,
    ) -> Result<Self> {
        let value = Self {
            content: PublicationConflictResolutionResponseContent {
                version: RESOLUTION_RESPONSE_VERSION,
                request_id: request.request_id()?,
                resolution,
                replacement_ticket: request.replacement_ticket().to_vec(),
            },
        };
        value.verify()?;
        Ok(value)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify()?;
        let bytes = postcard::to_allocvec(self)
            .context("encode publication conflict resolution response")?;
        ensure!(
            bytes.len() <= MAX_PUBLICATION_CONFLICT_RESOLUTION_RESPONSE_BYTES,
            "publication conflict resolution response is too large"
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_PUBLICATION_CONFLICT_RESOLUTION_RESPONSE_BYTES,
            "publication conflict resolution response size is invalid"
        );
        let value: Self = postcard::from_bytes(bytes)
            .context("decode publication conflict resolution response")?;
        value.verify()?;
        Ok(value)
    }

    pub fn verify(&self) -> Result<()> {
        ensure!(
            self.content.version == RESOLUTION_RESPONSE_VERSION,
            "unsupported publication conflict resolution response version"
        );
        self.content.resolution.verify()?;
        ensure!(
            self.content.request_id == self.content.resolution.request_id()
                && !self.content.replacement_ticket.is_empty()
                && self.content.replacement_ticket.len()
                    <= MAX_PUBLICATION_CONFLICT_RESOLUTION_RESPONSE_BYTES
                && *blake3::hash(&self.content.replacement_ticket).as_bytes()
                    == self.content.resolution.replacement_ticket_digest(),
            "publication conflict resolution response does not match its Root authorization"
        );
        Ok(())
    }

    pub fn request_id(&self) -> PublicationConflictResolutionRequestId {
        self.content.request_id
    }

    pub fn resolution(&self) -> &RootSignedPublicationConflictResolution {
        &self.content.resolution
    }

    pub fn replacement_ticket(&self) -> &[u8] {
        &self.content.replacement_ticket
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum PublicationConflictClaimKind {
    Request,
    Response,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PublicationConflictQrClaim {
    version: u8,
    kind: PublicationConflictClaimKind,
    artifact_id: String,
    request_id: String,
    local_account_id: AccountId,
    authority_revision: u64,
    conflict_evidence_id: String,
    peer_account_id: AccountId,
    peer_device_id: DeviceId,
    old_publication_channel_id: String,
    new_publication_channel_id: String,
    replacement_ticket_digest: String,
    confirmation_code: String,
}

impl PublicationConflictQrClaim {
    pub fn for_request(request: &SignedPublicationConflictResolutionRequest) -> Result<Self> {
        let request_id = request.request_id()?.to_string();
        let mut claim = Self {
            version: CLAIM_VERSION,
            kind: PublicationConflictClaimKind::Request,
            artifact_id: request_id.clone(),
            request_id,
            local_account_id: request.local_account_id(),
            authority_revision: request.authority_revision(),
            conflict_evidence_id: request.conflict_proof().evidence_id()?.to_string(),
            peer_account_id: request.peer_account_id(),
            peer_device_id: request.peer_device_id(),
            old_publication_channel_id: request.old_write_key().channel_id().to_string(),
            new_publication_channel_id: request.new_write_key().channel_id().to_string(),
            replacement_ticket_digest: encode_hex(&request.replacement_ticket_digest()),
            confirmation_code: String::new(),
        };
        claim.confirmation_code = confirmation_code(&claim);
        claim.verify()?;
        Ok(claim)
    }

    pub fn for_response(response: &PublicationConflictResolutionResponse) -> Result<Self> {
        let resolution = response.resolution();
        let mut claim = Self {
            version: CLAIM_VERSION,
            kind: PublicationConflictClaimKind::Response,
            artifact_id: resolution.resolution_id()?.to_string(),
            request_id: response.request_id().to_string(),
            local_account_id: resolution.local_account_id(),
            authority_revision: resolution.authority_revision(),
            conflict_evidence_id: resolution.conflict_evidence_id().to_string(),
            peer_account_id: resolution.peer_account_id(),
            peer_device_id: resolution.peer_device_id(),
            old_publication_channel_id: resolution.old_write_key().channel_id().to_string(),
            new_publication_channel_id: resolution.new_write_key().channel_id().to_string(),
            replacement_ticket_digest: encode_hex(&resolution.replacement_ticket_digest()),
            confirmation_code: String::new(),
        };
        claim.confirmation_code = confirmation_code(&claim);
        claim.verify()?;
        Ok(claim)
    }

    pub fn verify(&self) -> Result<()> {
        ensure!(
            self.version == CLAIM_VERSION,
            "unsupported publication-conflict QR claim version"
        );
        ensure!(
            !self.artifact_id.is_empty()
                && !self.request_id.is_empty()
                && !self.conflict_evidence_id.is_empty()
                && !self.old_publication_channel_id.is_empty()
                && !self.new_publication_channel_id.is_empty()
                && !self.replacement_ticket_digest.is_empty(),
            "publication-conflict QR claim has an empty security field"
        );
        ensure!(
            self.confirmation_code == confirmation_code(self),
            "publication-conflict QR confirmation code is invalid"
        );
        Ok(())
    }

    pub fn encode_uri(&self) -> Result<String> {
        self.verify()?;
        let encoded = postcard::to_allocvec(self)
            .context("encode publication-conflict QR verification claim")?;
        let uri = format!(
            "{PUBLICATION_CONFLICT_CLAIM_URI_PREFIX}{}",
            URL_SAFE_NO_PAD.encode(encoded)
        );
        ensure!(
            uri.len() <= MAX_PUBLICATION_CONFLICT_CLAIM_URI_BYTES,
            "publication-conflict QR verification claim is too large"
        );
        Ok(uri)
    }

    pub fn decode_uri(uri: &str) -> Result<Self> {
        ensure!(
            uri.is_ascii()
                && uri.starts_with(PUBLICATION_CONFLICT_CLAIM_URI_PREFIX)
                && uri.len() <= MAX_PUBLICATION_CONFLICT_CLAIM_URI_BYTES,
            "publication-conflict QR URI shape is invalid"
        );
        let bytes = URL_SAFE_NO_PAD
            .decode(&uri[PUBLICATION_CONFLICT_CLAIM_URI_PREFIX.len()..])
            .context("decode publication-conflict QR claim as base64url")?;
        let claim: Self = postcard::from_bytes(&bytes)
            .context("decode publication-conflict QR verification claim")?;
        claim.verify()?;
        Ok(claim)
    }

    pub fn confirmation_code(&self) -> &str {
        &self.confirmation_code
    }
}

fn canonical_observation_pair(
    left: SignedTicketPublicationObservation,
    right: SignedTicketPublicationObservation,
) -> Result<(
    SignedTicketPublicationObservation,
    SignedTicketPublicationObservation,
)> {
    let left_id = left.observation_id()?;
    let right_id = right.observation_id()?;
    ensure!(
        left_id != right_id,
        "publication conflict proof requires two distinct observations"
    );
    Ok(if left_id < right_id {
        (left, right)
    } else {
        (right, left)
    })
}

fn verify_conflict_pair(
    local_account_id: AccountId,
    first: &SignedTicketPublicationObservation,
    conflicting: &SignedTicketPublicationObservation,
) -> Result<()> {
    first.verify_signature()?;
    conflicting.verify_signature()?;
    ensure!(
        first.local_account_id() == local_account_id
            && conflicting.local_account_id() == local_account_id
            && first.channel_id() == conflicting.channel_id()
            && first.publisher_account_id() == conflicting.publisher_account_id()
            && first.publisher_device_id() == conflicting.publisher_device_id()
            && first.publication_generation() == conflicting.publication_generation(),
        "publication conflict proof observations do not describe one publication generation"
    );
    ensure!(
        first.publication_id() != conflicting.publication_id()
            || first.ticket_digest() != conflicting.ticket_digest(),
        "publication conflict proof observations agree"
    );
    Ok(())
}

fn confirmation_code(claim: &PublicationConflictQrClaim) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(CONFIRMATION_CODE_DOMAIN);
    let authority_revision = claim.authority_revision.to_be_bytes();
    for value in [
        claim.request_id.as_bytes(),
        claim.local_account_id.as_bytes(),
        authority_revision.as_slice(),
        claim.conflict_evidence_id.as_bytes(),
        claim.peer_account_id.as_bytes(),
        claim.peer_device_id.as_bytes(),
        claim.old_publication_channel_id.as_bytes(),
        claim.new_publication_channel_id.as_bytes(),
        claim.replacement_ticket_digest.as_bytes(),
    ] {
        hasher.update(&(value.len() as u64).to_be_bytes());
        hasher.update(value);
    }
    let digest = hasher.finalize();
    let mut code = String::from("KPC1");
    for chunk in digest.as_bytes()[..12].as_chunks::<2>().0 {
        use fmt::Write as _;
        let _ = write!(code, "-{:02X}{:02X}", chunk[0], chunk[1]);
    }
    code
}

fn content_bytes<T: Serialize>(content: &T) -> Result<Vec<u8>> {
    postcard::to_allocvec(content).context("encode publication artifact content")
}

fn domain_bytes<T: Serialize>(domain: &[u8], content: &T) -> Result<Vec<u8>> {
    let encoded = content_bytes(content)?;
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

fn ticket_digest(ticket: &[u8]) -> [u8; 32] {
    domain_hash(TICKET_DIGEST_DOMAIN, ticket)
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
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
    use kilogram_ticket_publication::TicketPublicationWriteCapability;

    #[test]
    fn request_and_response_claims_share_confirmation_but_not_payload() -> Result<()> {
        let root_directory = tempfile::tempdir()?;
        let root = AccountRootState::create(root_directory.path())?;
        let publisher = DeviceIdentity::generate()?;
        let detector = DeviceIdentity::generate()?;
        let old_write_key = TicketPublicationWriteCapability::derive(
            publisher.secret_bytes(),
            root.account_id().as_bytes(),
        )
        .write_key();
        let channel = old_write_key.channel_id();
        let first = SignedTicketPublication::sign(
            &publisher,
            channel,
            root.account_id(),
            root.account_id(),
            "one".to_owned(),
            100,
            MIN_TICKET_PUBLICATION_TTL_SECONDS,
            None,
        )?;
        let conflicting = SignedTicketPublication::sign(
            &publisher,
            channel,
            root.account_id(),
            root.account_id(),
            "two".to_owned(),
            100,
            MIN_TICKET_PUBLICATION_TTL_SECONDS,
            None,
        )?;
        let first_observation = SignedTicketPublicationObservation::sign(
            &detector,
            root.account_id(),
            &first,
            101,
            None,
        )?;
        let conflicting_observation = SignedTicketPublicationObservation::sign(
            &detector,
            root.account_id(),
            &conflicting,
            101,
            None,
        )?;
        let proof = SignedPublicationConflictProof::sign(
            &detector,
            root.account_id(),
            102,
            first_observation,
            conflicting_observation,
        )?;
        let replacement_publisher = DeviceIdentity::generate()?;
        let new_write_key = TicketPublicationWriteCapability::derive(
            replacement_publisher.secret_bytes(),
            root.account_id().as_bytes(),
        )
        .write_key();
        let request = SignedPublicationConflictResolutionRequest::sign(
            &detector,
            root.account_id(),
            1,
            proof,
            old_write_key,
            new_write_key,
            0,
            1,
            PublicationConflictRoutePolicy::Auto,
            b"replacement-ticket".to_vec(),
            103,
        )?;
        let resolution = RootSignedPublicationConflictResolution::sign(
            &root,
            request.request_id()?,
            request.authority_revision(),
            request.conflict_proof().evidence_id()?,
            request.peer_account_id(),
            request.peer_device_id(),
            request.old_write_key(),
            request.new_write_key(),
            request.replacement_ticket_digest(),
            104,
        )?;
        let response = PublicationConflictResolutionResponse::new(&request, resolution)?;
        let request_claim = PublicationConflictQrClaim::for_request(&request)?;
        let response_claim = PublicationConflictQrClaim::for_response(&response)?;
        assert_eq!(
            request_claim.confirmation_code(),
            response_claim.confirmation_code()
        );
        assert_ne!(request_claim.encode_uri()?, response_claim.encode_uri()?);
        assert_eq!(
            PublicationConflictQrClaim::decode_uri(&request_claim.encode_uri()?)?,
            request_claim
        );
        Ok(())
    }
}
