use std::fmt;

use anyhow::{Context, Result, ensure};
use kilogram_identity::{AccountId, DeviceId, DeviceIdentity};
pub use kilogram_publication_conflict::{
    MAX_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_BYTES,
    MAX_PUBLICATION_CONFLICT_RESOLUTION_RESPONSE_BYTES, PublicationConflictResolutionId,
    PublicationConflictResolutionResponse, RootSignedPublicationConflictResolution,
    SignedPublicationConflictResolutionRequest,
};
use serde::{Deserialize, Serialize};

const ROTATION_VERSION: u8 = 1;
const ROTATION_SIGNATURE_DOMAIN: &[u8] = b"kilogram:ticket-publication-channel-rotation:v1\0";
const ROTATION_ID_DOMAIN: &[u8] = b"kilogram:ticket-publication-channel-rotation-id:v1\0";
pub const MAX_PUBLICATION_ROTATION_BYTES: usize = 4 * 1024;

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

fn domain_bytes<T: Serialize>(domain: &[u8], content: &T) -> Result<Vec<u8>> {
    let encoded = postcard::to_allocvec(content).context("encode publication rotation content")?;
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
