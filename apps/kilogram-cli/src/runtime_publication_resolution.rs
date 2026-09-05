use std::fmt;

use anyhow::{Context, Result, ensure};
use kilogram_identity::{AccountId, AccountRootState, DeviceId, DeviceIdentity};
use kilogram_ticket_publication::TicketPublicationWriteKey;
use kilogram_transport_iroh::RoutePolicy;
use serde::{Deserialize, Serialize};

use crate::runtime_publication_conflict::PublicationConflictEvidenceId;

const ROTATION_VERSION: u8 = 1;
const ROTATION_SIGNATURE_DOMAIN: &[u8] = b"kilogram:ticket-publication-channel-rotation:v1\0";
const ROTATION_ID_DOMAIN: &[u8] = b"kilogram:ticket-publication-channel-rotation-id:v1\0";
const RESOLUTION_REQUEST_VERSION: u8 = 1;
const RESOLUTION_REQUEST_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:publication-conflict-resolution-request:v1\0";
const RESOLUTION_REQUEST_ID_DOMAIN: &[u8] =
    b"kilogram:publication-conflict-resolution-request-id:v1\0";
const RESOLUTION_VERSION: u8 = 2;
const RESOLUTION_ID_DOMAIN: &[u8] = b"kilogram:publication-conflict-resolution-id:v2\0";
const RESOLUTION_RESPONSE_VERSION: u8 = 1;
pub const MAX_PUBLICATION_ROTATION_BYTES: usize = 4 * 1024;
pub const MAX_PUBLICATION_CONFLICT_RESOLUTION_BYTES: usize = 8 * 1024;
pub const MAX_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_BYTES: usize = 9 * 1024 * 1024;
pub const MAX_PUBLICATION_CONFLICT_RESOLUTION_RESPONSE_BYTES: usize = 9 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct PublicationChannelRotationId([u8; 32]);

impl fmt::Display for PublicationChannelRotationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PublicationChannelRotationContent {
    version: u8,
    local_account_id: AccountId,
    local_device_id: DeviceId,
    peer_account_id: AccountId,
    epoch: u64,
    rotated_at_unix_seconds: u64,
}

/// Device-signed append-only selection of a new outgoing publication
/// capability. It changes only this Device's pseudonymous store channel.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedPublicationChannelRotation {
    content: PublicationChannelRotationContent,
    signature: Vec<u8>,
}

impl SignedPublicationChannelRotation {
    pub fn sign(
        identity: &DeviceIdentity,
        local_account_id: AccountId,
        peer_account_id: AccountId,
        epoch: u64,
        rotated_at_unix_seconds: u64,
    ) -> Result<Self> {
        ensure!(
            epoch != 0,
            "publication channel rotation epoch must be non-zero"
        );
        ensure!(
            rotated_at_unix_seconds != 0,
            "publication channel rotation time must be non-zero"
        );
        let content = PublicationChannelRotationContent {
            version: ROTATION_VERSION,
            local_account_id,
            local_device_id: identity.device_id(),
            peer_account_id,
            epoch,
            rotated_at_unix_seconds,
        };
        let signature = identity
            .sign(&domain_bytes(ROTATION_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let value = Self { content, signature };
        value.verify()?;
        Ok(value)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify()?;
        let bytes = postcard::to_allocvec(self).context("encode publication channel rotation")?;
        ensure!(
            bytes.len() <= MAX_PUBLICATION_ROTATION_BYTES,
            "publication channel rotation is too large"
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_PUBLICATION_ROTATION_BYTES,
            "publication channel rotation size is invalid"
        );
        let value: Self =
            postcard::from_bytes(bytes).context("decode publication channel rotation")?;
        value.verify()?;
        Ok(value)
    }

    pub fn verify(&self) -> Result<()> {
        ensure!(
            self.content.version == ROTATION_VERSION,
            "unsupported publication channel rotation version"
        );
        ensure!(
            self.content.epoch != 0 && self.content.rotated_at_unix_seconds != 0,
            "publication channel rotation has invalid epoch or time"
        );
        self.content
            .local_device_id
            .verify(
                &domain_bytes(ROTATION_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify publication channel rotation Device signature")
    }

    pub fn verify_local(
        &self,
        local_account_id: AccountId,
        local_device_id: DeviceId,
    ) -> Result<()> {
        self.verify()?;
        ensure!(
            self.content.local_account_id == local_account_id
                && self.content.local_device_id == local_device_id,
            "publication channel rotation belongs to another local identity"
        );
        Ok(())
    }

    pub fn rotation_id(&self) -> Result<PublicationChannelRotationId> {
        Ok(PublicationChannelRotationId(domain_hash(
            ROTATION_ID_DOMAIN,
            &self.encode()?,
        )))
    }

    pub fn peer_account_id(&self) -> AccountId {
        self.content.peer_account_id
    }

    pub fn epoch(&self) -> u64 {
        self.content.epoch
    }

    pub fn rotated_at_unix_seconds(&self) -> u64 {
        self.content.rotated_at_unix_seconds
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct PublicationConflictResolutionId([u8; 32]);

impl fmt::Display for PublicationConflictResolutionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct PublicationConflictResolutionRequestId([u8; 32]);

impl fmt::Display for PublicationConflictResolutionRequestId {
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
    conflict_proof: crate::runtime_publication_conflict::SignedPublicationConflictProof,
    peer_account_id: AccountId,
    peer_device_id: DeviceId,
    old_write_key: TicketPublicationWriteKey,
    new_write_key: TicketPublicationWriteKey,
    old_channel_epoch: u64,
    new_channel_epoch: u64,
    route_policy: RoutePolicy,
    replacement_ticket: Vec<u8>,
    requested_at_unix_seconds: u64,
}

/// Bounded Device-signed request that can cross an air gap to an offline Root
/// signer. It contains public evidence and one exact peer-signed replacement;
/// it never contains a local Device or Account Root secret.
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
        conflict_proof: crate::runtime_publication_conflict::SignedPublicationConflictProof,
        old_write_key: TicketPublicationWriteKey,
        new_write_key: TicketPublicationWriteKey,
        old_channel_epoch: u64,
        new_channel_epoch: u64,
        route_policy: RoutePolicy,
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

    pub fn conflict_proof(
        &self,
    ) -> &crate::runtime_publication_conflict::SignedPublicationConflictProof {
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

    pub fn route_policy(&self) -> RoutePolicy {
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

/// Portable Account-Root authorization to resolve one exact retained proof by
/// accepting one exact peer-signed replacement descriptor on a new channel.
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

/// Self-contained offline Root response. The resolution authenticates the
/// request ID and ticket digest; the included public ticket lets any sibling
/// apply the response without a second unbound file.
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

fn content_bytes<T: Serialize>(content: &T) -> Result<Vec<u8>> {
    postcard::to_allocvec(content).context("encode publication resolution content")
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

fn write_hex(formatter: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}
