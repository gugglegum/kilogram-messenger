use std::fmt;

use anyhow::{Context, Result, ensure};
use kilogram_identity::{AccountId, DeviceId, DeviceIdentity};
use serde::{Deserialize, Serialize};

use crate::runtime_publication::{SignedTicketPublicationObservation, TicketPublicationChannelId};

const CONFLICT_PROOF_VERSION: u8 = 1;
const CONFLICT_PROOF_SIGNATURE_DOMAIN: &[u8] = b"kilogram:ticket-publication-conflict-proof:v1\0";
const CONFLICT_PROOF_ID_DOMAIN: &[u8] = b"kilogram:ticket-publication-conflict-proof-id:v1\0";
pub const MAX_PUBLICATION_CONFLICT_PROOF_BYTES: usize = 128 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct PublicationConflictProofId([u8; 32]);

impl fmt::Display for PublicationConflictProofId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
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

/// A local Device-signed record that two authenticated own-device observations
/// disagree about one peer publication generation.
///
/// This proves what the detector observed; it is deliberately not described as
/// a Root-authorized accusation against the peer publisher.
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
        left.verify_signature()?;
        right.verify_signature()?;
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
        let signature = identity.sign(&signing_bytes(&content)?).to_vec();
        let proof = Self { content, signature };
        proof.verify_signature()?;
        Ok(proof)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify_signature()?;
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
        let proof: Self =
            postcard::from_bytes(bytes).context("decode publication conflict proof")?;
        proof.verify_signature()?;
        Ok(proof)
    }

    pub fn verify_local(
        &self,
        expected_account_id: AccountId,
        expected_device_id: DeviceId,
    ) -> Result<()> {
        self.verify_signature()?;
        ensure!(
            self.local_account_id() == expected_account_id
                && self.detector_device_id() == expected_device_id,
            "publication conflict proof belongs to another local identity"
        );
        Ok(())
    }

    fn verify_signature(&self) -> Result<()> {
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
        let first_id = self.content.first_observation.observation_id()?;
        let conflicting_id = self.content.conflicting_observation.observation_id()?;
        ensure!(
            first_id < conflicting_id,
            "publication conflict proof observations are not canonical"
        );
        self.content
            .detector_device_id
            .verify(&signing_bytes(&self.content)?, &self.signature)
            .context("verify publication conflict proof detector signature")
    }

    pub fn proof_id(&self) -> Result<PublicationConflictProofId> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(CONFLICT_PROOF_ID_DOMAIN);
        hasher.update(&self.encode()?);
        Ok(PublicationConflictProofId(*hasher.finalize().as_bytes()))
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

fn signing_bytes(content: &PublicationConflictProofContent) -> Result<Vec<u8>> {
    let encoded = postcard::to_allocvec(content).context("encode publication conflict proof")?;
    let mut bytes = Vec::with_capacity(CONFLICT_PROOF_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(CONFLICT_PROOF_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}
