//! Inactive deterministic-CBOR ticket-publication conflict evidence.
//!
//! This module covers publication, observation and local conflict-proof
//! artifacts. Root-authorized resolution remains a separate follow-up, so the
//! source compatibility surface is intentionally only partially covered.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use thiserror::Error;

use crate::{
    canonical_account_v1::{AccountDecoder, CanonicalAccountError},
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
        push_unsigned, validate_canonical_profile_v1,
    },
};

pub const MIN_CANONICAL_TICKET_PUBLICATION_TTL_SECONDS: u64 = 30;
pub const MAX_CANONICAL_TICKET_PUBLICATION_TTL_SECONDS: u64 = 60 * 60;
pub const MAX_CANONICAL_TICKET_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_CANONICAL_SIGNED_TICKET_PUBLICATION_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_CANONICAL_TICKET_PUBLICATION_OBSERVATION_BYTES: usize = 4096;
pub const MAX_CANONICAL_PUBLICATION_CONFLICT_PROOF_BYTES: usize = 128 * 1024;
pub const MAX_CANONICAL_PUBLICATION_CLOCK_SKEW_SECONDS: u64 = 5 * 60;

pub const CANONICAL_TICKET_PUBLICATION_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:ticket-publication:cbor-v1\0";
pub const CANONICAL_TICKET_PUBLICATION_ID_DOMAIN: &[u8] =
    b"kilogram:ticket-publication-id:cbor-v1\0";
pub const CANONICAL_TICKET_PUBLICATION_TICKET_DIGEST_DOMAIN: &[u8] =
    b"kilogram:ticket-publication-ticket-digest:v1\0";
pub const CANONICAL_TICKET_PUBLICATION_OBSERVATION_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:ticket-publication-observation:cbor-v1\0";
pub const CANONICAL_TICKET_PUBLICATION_OBSERVATION_ID_DOMAIN: &[u8] =
    b"kilogram:ticket-publication-observation-id:cbor-v1\0";
pub const CANONICAL_PUBLICATION_CONFLICT_PROOF_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:ticket-publication-conflict-proof:cbor-v1\0";
pub const CANONICAL_PUBLICATION_CONFLICT_PROOF_ID_DOMAIN: &[u8] =
    b"kilogram:ticket-publication-conflict-proof-id:cbor-v1\0";
pub const CANONICAL_PUBLICATION_CONFLICT_EVIDENCE_ID_DOMAIN: &[u8] =
    b"kilogram:ticket-publication-conflict-evidence-id:cbor-v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalTicketPublicationContentV1 {
    pub channel_id: [u8; 32],
    pub publisher_account_id: [u8; 32],
    pub publisher_device_id: [u8; 32],
    pub recipient_account_id: [u8; 32],
    pub generation: u64,
    pub previous_publication_id: Option<[u8; 32]>,
    pub published_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub ticket_digest: [u8; 32],
    pub ticket: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedTicketPublicationV1 {
    pub content: CanonicalTicketPublicationContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalTicketPublicationObservationContentV1 {
    pub local_account_id: [u8; 32],
    pub local_device_id: [u8; 32],
    pub channel_id: [u8; 32],
    pub publisher_account_id: [u8; 32],
    pub publisher_device_id: [u8; 32],
    pub observation_generation: u64,
    pub previous_observation_id: Option<[u8; 32]>,
    pub publication_generation: u64,
    pub publication_id: [u8; 32],
    pub ticket_digest: [u8; 32],
    pub observed_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedTicketPublicationObservationV1 {
    pub content: CanonicalTicketPublicationObservationContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalPublicationConflictProofContentV1 {
    pub local_account_id: [u8; 32],
    pub detector_device_id: [u8; 32],
    pub detected_at_unix_seconds: u64,
    pub channel_id: [u8; 32],
    pub publication_generation: u64,
    pub first_observation: CanonicalSignedTicketPublicationObservationV1,
    pub conflicting_observation: CanonicalSignedTicketPublicationObservationV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedPublicationConflictProofV1 {
    pub content: CanonicalPublicationConflictProofContentV1,
    pub signature: [u8; 64],
}

#[derive(Debug, Error)]
pub enum CanonicalPublicationConflictError {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),
    #[error(transparent)]
    Decode(#[from] CanonicalAccountError),
    #[error("canonical publication-conflict {object} exceeds {maximum} bytes")]
    ObjectTooLarge {
        object: &'static str,
        maximum: usize,
    },
    #[error("canonical publication-conflict {object} contains an invalid Ed25519 key")]
    InvalidPublicKey { object: &'static str },
    #[error("canonical publication-conflict {object} contains an invalid signature")]
    InvalidSignature { object: &'static str },
    #[error("canonical ticket publication metadata is outside protocol bounds")]
    InvalidPublication,
    #[error("canonical ticket publication is not valid UTF-8")]
    InvalidTicketText,
    #[error("canonical ticket publication chain is not contiguous")]
    InvalidPublicationChain,
    #[error("canonical ticket publication is not valid at the supplied time")]
    PublicationNotCurrent,
    #[error("canonical ticket publication observation metadata is outside protocol bounds")]
    InvalidObservation,
    #[error("canonical ticket publication observation chain is not contiguous")]
    InvalidObservationChain,
    #[error("canonical publication conflict observations do not prove one conflict")]
    InvalidConflictPair,
    #[error("canonical publication conflict proof summary or ordering is invalid")]
    InvalidConflictProof,
}

#[allow(clippy::too_many_arguments)]
pub fn sign_ticket_publication_v1(
    signing_key: &SigningKey,
    channel_id: [u8; 32],
    publisher_account_id: [u8; 32],
    recipient_account_id: [u8; 32],
    ticket: Vec<u8>,
    published_at_unix_seconds: u64,
    ttl_seconds: u64,
    previous: Option<&CanonicalSignedTicketPublicationV1>,
) -> Result<CanonicalSignedTicketPublicationV1, CanonicalPublicationConflictError> {
    if !(MIN_CANONICAL_TICKET_PUBLICATION_TTL_SECONDS
        ..=MAX_CANONICAL_TICKET_PUBLICATION_TTL_SECONDS)
        .contains(&ttl_seconds)
    {
        return Err(CanonicalPublicationConflictError::InvalidPublication);
    }
    let expires_at_unix_seconds = published_at_unix_seconds
        .checked_add(ttl_seconds)
        .ok_or(CanonicalPublicationConflictError::InvalidPublication)?;
    let (generation, previous_publication_id) = match previous {
        Some(previous) => {
            verify_signed_ticket_publication_v1(previous)?;
            if previous.content.channel_id != channel_id
                || previous.content.publisher_account_id != publisher_account_id
                || previous.content.publisher_device_id != signing_key.verifying_key().to_bytes()
                || previous.content.recipient_account_id != recipient_account_id
                || published_at_unix_seconds < previous.content.published_at_unix_seconds
            {
                return Err(CanonicalPublicationConflictError::InvalidPublicationChain);
            }
            (
                previous
                    .content
                    .generation
                    .checked_add(1)
                    .ok_or(CanonicalPublicationConflictError::InvalidPublicationChain)?,
                Some(ticket_publication_id_v1(previous)?),
            )
        }
        None => (1, None),
    };
    let content = CanonicalTicketPublicationContentV1 {
        channel_id,
        publisher_account_id,
        publisher_device_id: signing_key.verifying_key().to_bytes(),
        recipient_account_id,
        generation,
        previous_publication_id,
        published_at_unix_seconds,
        expires_at_unix_seconds,
        ticket_digest: ticket_publication_ticket_digest_v1(&ticket),
        ticket,
    };
    let signature = signing_key
        .sign(&ticket_publication_signing_bytes_v1(&content)?)
        .to_bytes();
    let publication = CanonicalSignedTicketPublicationV1 { content, signature };
    verify_signed_ticket_publication_v1(&publication)?;
    Ok(publication)
}

pub fn encode_ticket_publication_content_v1(
    content: &CanonicalTicketPublicationContentV1,
) -> Result<Vec<u8>, CanonicalPublicationConflictError> {
    validate_publication_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 11);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.channel_id);
    push_bytes(&mut output, &content.publisher_account_id);
    push_bytes(&mut output, &content.publisher_device_id);
    push_bytes(&mut output, &content.recipient_account_id);
    push_unsigned(&mut output, content.generation);
    encode_optional_id(&mut output, content.previous_publication_id);
    push_unsigned(&mut output, content.published_at_unix_seconds);
    push_unsigned(&mut output, content.expires_at_unix_seconds);
    push_bytes(&mut output, &content.ticket_digest);
    push_bytes(&mut output, &content.ticket);
    Ok(output)
}

pub fn ticket_publication_signing_bytes_v1(
    content: &CanonicalTicketPublicationContentV1,
) -> Result<Vec<u8>, CanonicalPublicationConflictError> {
    Ok(prefixed(
        CANONICAL_TICKET_PUBLICATION_SIGNATURE_DOMAIN,
        &encode_ticket_publication_content_v1(content)?,
    ))
}

pub fn encode_signed_ticket_publication_v1(
    publication: &CanonicalSignedTicketPublicationV1,
) -> Result<Vec<u8>, CanonicalPublicationConflictError> {
    verify_signed_ticket_publication_v1(publication)?;
    let content = encode_ticket_publication_content_v1(&publication.content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content);
    push_bytes(&mut output, &publication.signature);
    ensure_bound(
        &output,
        "signed publication",
        MAX_CANONICAL_SIGNED_TICKET_PUBLICATION_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_ticket_publication_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedTicketPublicationV1, CanonicalPublicationConflictError> {
    ensure_bound(
        bytes,
        "signed publication",
        MAX_CANONICAL_SIGNED_TICKET_PUBLICATION_BYTES,
    )?;
    // The nested canonical content may be slightly larger than the profile's
    // 8 MiB single-byte-string limit when the ticket itself is exactly 8 MiB.
    // Strict outer minimality is still enforced by exact re-encoding below.
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-ticket-publication-v1", 3)?;
    decoder.expect_version("signed-ticket-publication-v1.schema_version")?;
    let content = decode_ticket_publication_content_v1(
        decoder.bytes("signed-ticket-publication-v1.content")?,
    )?;
    let signature = decoder.fixed_bytes("signed-ticket-publication-v1.signature")?;
    decoder.finish()?;
    let publication = CanonicalSignedTicketPublicationV1 { content, signature };
    verify_signed_ticket_publication_v1(&publication)?;
    if encode_signed_ticket_publication_v1(&publication)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(publication)
}

pub fn verify_signed_ticket_publication_v1(
    publication: &CanonicalSignedTicketPublicationV1,
) -> Result<(), CanonicalPublicationConflictError> {
    validate_publication_content(&publication.content)?;
    verify_ed25519(
        &publication.content.publisher_device_id,
        &ticket_publication_signing_bytes_v1(&publication.content)?,
        &publication.signature,
        "signed-ticket-publication-v1",
    )
}

pub fn verify_ticket_publication_chain_link_v1(
    previous: &CanonicalSignedTicketPublicationV1,
    successor: &CanonicalSignedTicketPublicationV1,
) -> Result<(), CanonicalPublicationConflictError> {
    verify_signed_ticket_publication_v1(previous)?;
    verify_signed_ticket_publication_v1(successor)?;
    if successor.content.channel_id != previous.content.channel_id
        || successor.content.publisher_account_id != previous.content.publisher_account_id
        || successor.content.publisher_device_id != previous.content.publisher_device_id
        || successor.content.recipient_account_id != previous.content.recipient_account_id
        || successor.content.generation != previous.content.generation.checked_add(1).unwrap_or(0)
        || successor.content.previous_publication_id != Some(ticket_publication_id_v1(previous)?)
        || successor.content.published_at_unix_seconds < previous.content.published_at_unix_seconds
    {
        return Err(CanonicalPublicationConflictError::InvalidPublicationChain);
    }
    Ok(())
}

pub fn verify_ticket_publication_at_v1(
    publication: &CanonicalSignedTicketPublicationV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalPublicationConflictError> {
    verify_signed_ticket_publication_v1(publication)?;
    if publication.content.published_at_unix_seconds
        > now_unix_seconds.saturating_add(MAX_CANONICAL_PUBLICATION_CLOCK_SKEW_SECONDS)
        || publication.content.expires_at_unix_seconds <= now_unix_seconds
    {
        return Err(CanonicalPublicationConflictError::PublicationNotCurrent);
    }
    Ok(())
}

pub fn ticket_publication_id_v1(
    publication: &CanonicalSignedTicketPublicationV1,
) -> Result<[u8; 32], CanonicalPublicationConflictError> {
    Ok(domain_hash(
        CANONICAL_TICKET_PUBLICATION_ID_DOMAIN,
        &encode_signed_ticket_publication_v1(publication)?,
    ))
}

pub fn ticket_publication_ticket_digest_v1(ticket: &[u8]) -> [u8; 32] {
    domain_hash(CANONICAL_TICKET_PUBLICATION_TICKET_DIGEST_DOMAIN, ticket)
}

pub fn sign_ticket_publication_observation_v1(
    signing_key: &SigningKey,
    local_account_id: [u8; 32],
    publication: &CanonicalSignedTicketPublicationV1,
    observed_at_unix_seconds: u64,
    previous: Option<&CanonicalSignedTicketPublicationObservationV1>,
) -> Result<CanonicalSignedTicketPublicationObservationV1, CanonicalPublicationConflictError> {
    verify_ticket_publication_at_v1(publication, observed_at_unix_seconds)?;
    if publication.content.recipient_account_id != local_account_id {
        return Err(CanonicalPublicationConflictError::InvalidObservation);
    }
    let local_device_id = signing_key.verifying_key().to_bytes();
    let (observation_generation, previous_observation_id) = match previous {
        Some(previous) => {
            verify_signed_ticket_publication_observation_v1(previous)?;
            if previous.content.local_account_id != local_account_id
                || previous.content.local_device_id != local_device_id
                || previous.content.channel_id != publication.content.channel_id
                || previous.content.publisher_account_id != publication.content.publisher_account_id
                || previous.content.publisher_device_id != publication.content.publisher_device_id
                || publication.content.generation <= previous.content.publication_generation
                || observed_at_unix_seconds < previous.content.observed_at_unix_seconds
            {
                return Err(CanonicalPublicationConflictError::InvalidObservationChain);
            }
            (
                previous
                    .content
                    .observation_generation
                    .checked_add(1)
                    .ok_or(CanonicalPublicationConflictError::InvalidObservationChain)?,
                Some(ticket_publication_observation_id_v1(previous)?),
            )
        }
        None => (1, None),
    };
    let content = CanonicalTicketPublicationObservationContentV1 {
        local_account_id,
        local_device_id,
        channel_id: publication.content.channel_id,
        publisher_account_id: publication.content.publisher_account_id,
        publisher_device_id: publication.content.publisher_device_id,
        observation_generation,
        previous_observation_id,
        publication_generation: publication.content.generation,
        publication_id: ticket_publication_id_v1(publication)?,
        ticket_digest: publication.content.ticket_digest,
        observed_at_unix_seconds,
    };
    let signature = signing_key
        .sign(&ticket_publication_observation_signing_bytes_v1(&content)?)
        .to_bytes();
    let observation = CanonicalSignedTicketPublicationObservationV1 { content, signature };
    verify_signed_ticket_publication_observation_v1(&observation)?;
    Ok(observation)
}

pub fn encode_ticket_publication_observation_content_v1(
    content: &CanonicalTicketPublicationObservationContentV1,
) -> Result<Vec<u8>, CanonicalPublicationConflictError> {
    validate_observation_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 12);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.local_account_id);
    push_bytes(&mut output, &content.local_device_id);
    push_bytes(&mut output, &content.channel_id);
    push_bytes(&mut output, &content.publisher_account_id);
    push_bytes(&mut output, &content.publisher_device_id);
    push_unsigned(&mut output, content.observation_generation);
    encode_optional_id(&mut output, content.previous_observation_id);
    push_unsigned(&mut output, content.publication_generation);
    push_bytes(&mut output, &content.publication_id);
    push_bytes(&mut output, &content.ticket_digest);
    push_unsigned(&mut output, content.observed_at_unix_seconds);
    Ok(output)
}

pub fn ticket_publication_observation_signing_bytes_v1(
    content: &CanonicalTicketPublicationObservationContentV1,
) -> Result<Vec<u8>, CanonicalPublicationConflictError> {
    Ok(prefixed(
        CANONICAL_TICKET_PUBLICATION_OBSERVATION_SIGNATURE_DOMAIN,
        &encode_ticket_publication_observation_content_v1(content)?,
    ))
}

pub fn encode_signed_ticket_publication_observation_v1(
    observation: &CanonicalSignedTicketPublicationObservationV1,
) -> Result<Vec<u8>, CanonicalPublicationConflictError> {
    verify_signed_ticket_publication_observation_v1(observation)?;
    let content = encode_ticket_publication_observation_content_v1(&observation.content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content);
    push_bytes(&mut output, &observation.signature);
    ensure_bound(
        &output,
        "signed observation",
        MAX_CANONICAL_TICKET_PUBLICATION_OBSERVATION_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_ticket_publication_observation_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedTicketPublicationObservationV1, CanonicalPublicationConflictError> {
    ensure_bound(
        bytes,
        "signed observation",
        MAX_CANONICAL_TICKET_PUBLICATION_OBSERVATION_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-ticket-publication-observation-v1", 3)?;
    decoder.expect_version("signed-ticket-publication-observation-v1.schema_version")?;
    let content = decode_ticket_publication_observation_content_v1(
        decoder.bytes("signed-ticket-publication-observation-v1.content")?,
    )?;
    let signature = decoder.fixed_bytes("signed-ticket-publication-observation-v1.signature")?;
    decoder.finish()?;
    let observation = CanonicalSignedTicketPublicationObservationV1 { content, signature };
    verify_signed_ticket_publication_observation_v1(&observation)?;
    if encode_signed_ticket_publication_observation_v1(&observation)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(observation)
}

pub fn verify_signed_ticket_publication_observation_v1(
    observation: &CanonicalSignedTicketPublicationObservationV1,
) -> Result<(), CanonicalPublicationConflictError> {
    validate_observation_content(&observation.content)?;
    verify_ed25519(
        &observation.content.local_device_id,
        &ticket_publication_observation_signing_bytes_v1(&observation.content)?,
        &observation.signature,
        "signed-ticket-publication-observation-v1",
    )
}

pub fn verify_ticket_publication_observation_chain_link_v1(
    previous: &CanonicalSignedTicketPublicationObservationV1,
    successor: &CanonicalSignedTicketPublicationObservationV1,
) -> Result<(), CanonicalPublicationConflictError> {
    verify_signed_ticket_publication_observation_v1(previous)?;
    verify_signed_ticket_publication_observation_v1(successor)?;
    if successor.content.local_account_id != previous.content.local_account_id
        || successor.content.local_device_id != previous.content.local_device_id
        || successor.content.channel_id != previous.content.channel_id
        || successor.content.publisher_account_id != previous.content.publisher_account_id
        || successor.content.publisher_device_id != previous.content.publisher_device_id
        || successor.content.observation_generation
            != previous
                .content
                .observation_generation
                .checked_add(1)
                .unwrap_or(0)
        || successor.content.previous_observation_id
            != Some(ticket_publication_observation_id_v1(previous)?)
        || successor.content.publication_generation <= previous.content.publication_generation
        || successor.content.observed_at_unix_seconds < previous.content.observed_at_unix_seconds
    {
        return Err(CanonicalPublicationConflictError::InvalidObservationChain);
    }
    Ok(())
}

pub fn ticket_publication_observation_id_v1(
    observation: &CanonicalSignedTicketPublicationObservationV1,
) -> Result<[u8; 32], CanonicalPublicationConflictError> {
    Ok(domain_hash(
        CANONICAL_TICKET_PUBLICATION_OBSERVATION_ID_DOMAIN,
        &encode_signed_ticket_publication_observation_v1(observation)?,
    ))
}

pub fn sign_publication_conflict_proof_v1(
    signing_key: &SigningKey,
    local_account_id: [u8; 32],
    detected_at_unix_seconds: u64,
    left: CanonicalSignedTicketPublicationObservationV1,
    right: CanonicalSignedTicketPublicationObservationV1,
) -> Result<CanonicalSignedPublicationConflictProofV1, CanonicalPublicationConflictError> {
    let (first_observation, conflicting_observation) = canonical_observation_pair(left, right)?;
    verify_conflict_pair(
        local_account_id,
        &first_observation,
        &conflicting_observation,
    )?;
    let content = CanonicalPublicationConflictProofContentV1 {
        local_account_id,
        detector_device_id: signing_key.verifying_key().to_bytes(),
        detected_at_unix_seconds,
        channel_id: first_observation.content.channel_id,
        publication_generation: first_observation.content.publication_generation,
        first_observation,
        conflicting_observation,
    };
    let signature = signing_key
        .sign(&publication_conflict_proof_signing_bytes_v1(&content)?)
        .to_bytes();
    let proof = CanonicalSignedPublicationConflictProofV1 { content, signature };
    verify_signed_publication_conflict_proof_v1(&proof)?;
    Ok(proof)
}

pub fn encode_publication_conflict_proof_content_v1(
    content: &CanonicalPublicationConflictProofContentV1,
) -> Result<Vec<u8>, CanonicalPublicationConflictError> {
    validate_conflict_proof_content(content)?;
    let first = encode_signed_ticket_publication_observation_v1(&content.first_observation)?;
    let conflicting =
        encode_signed_ticket_publication_observation_v1(&content.conflicting_observation)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 8);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.local_account_id);
    push_bytes(&mut output, &content.detector_device_id);
    push_unsigned(&mut output, content.detected_at_unix_seconds);
    push_bytes(&mut output, &content.channel_id);
    push_unsigned(&mut output, content.publication_generation);
    push_bytes(&mut output, &first);
    push_bytes(&mut output, &conflicting);
    Ok(output)
}

pub fn publication_conflict_proof_signing_bytes_v1(
    content: &CanonicalPublicationConflictProofContentV1,
) -> Result<Vec<u8>, CanonicalPublicationConflictError> {
    Ok(prefixed(
        CANONICAL_PUBLICATION_CONFLICT_PROOF_SIGNATURE_DOMAIN,
        &encode_publication_conflict_proof_content_v1(content)?,
    ))
}

pub fn encode_signed_publication_conflict_proof_v1(
    proof: &CanonicalSignedPublicationConflictProofV1,
) -> Result<Vec<u8>, CanonicalPublicationConflictError> {
    verify_signed_publication_conflict_proof_v1(proof)?;
    let content = encode_publication_conflict_proof_content_v1(&proof.content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content);
    push_bytes(&mut output, &proof.signature);
    ensure_bound(
        &output,
        "signed conflict proof",
        MAX_CANONICAL_PUBLICATION_CONFLICT_PROOF_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_publication_conflict_proof_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedPublicationConflictProofV1, CanonicalPublicationConflictError> {
    ensure_bound(
        bytes,
        "signed conflict proof",
        MAX_CANONICAL_PUBLICATION_CONFLICT_PROOF_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-publication-conflict-proof-v1", 3)?;
    decoder.expect_version("signed-publication-conflict-proof-v1.schema_version")?;
    let content = decode_publication_conflict_proof_content_v1(
        decoder.bytes("signed-publication-conflict-proof-v1.content")?,
    )?;
    let signature = decoder.fixed_bytes("signed-publication-conflict-proof-v1.signature")?;
    decoder.finish()?;
    let proof = CanonicalSignedPublicationConflictProofV1 { content, signature };
    verify_signed_publication_conflict_proof_v1(&proof)?;
    if encode_signed_publication_conflict_proof_v1(&proof)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(proof)
}

pub fn verify_signed_publication_conflict_proof_v1(
    proof: &CanonicalSignedPublicationConflictProofV1,
) -> Result<(), CanonicalPublicationConflictError> {
    validate_conflict_proof_content(&proof.content)?;
    verify_ed25519(
        &proof.content.detector_device_id,
        &publication_conflict_proof_signing_bytes_v1(&proof.content)?,
        &proof.signature,
        "signed-publication-conflict-proof-v1",
    )
}

pub fn publication_conflict_proof_id_v1(
    proof: &CanonicalSignedPublicationConflictProofV1,
) -> Result<[u8; 32], CanonicalPublicationConflictError> {
    Ok(domain_hash(
        CANONICAL_PUBLICATION_CONFLICT_PROOF_ID_DOMAIN,
        &encode_signed_publication_conflict_proof_v1(proof)?,
    ))
}

pub fn publication_conflict_evidence_id_v1(
    proof: &CanonicalSignedPublicationConflictProofV1,
) -> Result<[u8; 32], CanonicalPublicationConflictError> {
    verify_signed_publication_conflict_proof_v1(proof)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(CANONICAL_PUBLICATION_CONFLICT_EVIDENCE_ID_DOMAIN);
    for observation in [
        &proof.content.first_observation,
        &proof.content.conflicting_observation,
    ] {
        let encoded = encode_signed_ticket_publication_observation_v1(observation)?;
        hasher.update(&(encoded.len() as u64).to_be_bytes());
        hasher.update(&encoded);
    }
    Ok(*hasher.finalize().as_bytes())
}

fn decode_ticket_publication_content_v1(
    bytes: &[u8],
) -> Result<CanonicalTicketPublicationContentV1, CanonicalPublicationConflictError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("ticket-publication-content-v1", 11)?;
    decoder.expect_version("ticket-publication-content-v1.schema_version")?;
    let content = CanonicalTicketPublicationContentV1 {
        channel_id: decoder.fixed_bytes("ticket-publication-content-v1.channel_id")?,
        publisher_account_id: decoder
            .fixed_bytes("ticket-publication-content-v1.publisher_account_id")?,
        publisher_device_id: decoder
            .fixed_bytes("ticket-publication-content-v1.publisher_device_id")?,
        recipient_account_id: decoder
            .fixed_bytes("ticket-publication-content-v1.recipient_account_id")?,
        generation: decoder.unsigned("ticket-publication-content-v1.generation")?,
        previous_publication_id: decode_optional_id(
            &mut decoder,
            "ticket-publication-content-v1.previous_publication_id",
        )?,
        published_at_unix_seconds: decoder
            .unsigned("ticket-publication-content-v1.published_at_unix_seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("ticket-publication-content-v1.expires_at_unix_seconds")?,
        ticket_digest: decoder.fixed_bytes("ticket-publication-content-v1.ticket_digest")?,
        ticket: decoder
            .bytes("ticket-publication-content-v1.ticket")?
            .to_vec(),
    };
    decoder.finish()?;
    validate_publication_content(&content)?;
    if encode_ticket_publication_content_v1(&content)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(content)
}

fn decode_ticket_publication_observation_content_v1(
    bytes: &[u8],
) -> Result<CanonicalTicketPublicationObservationContentV1, CanonicalPublicationConflictError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("ticket-publication-observation-content-v1", 12)?;
    decoder.expect_version("ticket-publication-observation-content-v1.schema_version")?;
    let content = CanonicalTicketPublicationObservationContentV1 {
        local_account_id: decoder
            .fixed_bytes("ticket-publication-observation-content-v1.local_account_id")?,
        local_device_id: decoder
            .fixed_bytes("ticket-publication-observation-content-v1.local_device_id")?,
        channel_id: decoder.fixed_bytes("ticket-publication-observation-content-v1.channel_id")?,
        publisher_account_id: decoder
            .fixed_bytes("ticket-publication-observation-content-v1.publisher_account_id")?,
        publisher_device_id: decoder
            .fixed_bytes("ticket-publication-observation-content-v1.publisher_device_id")?,
        observation_generation: decoder
            .unsigned("ticket-publication-observation-content-v1.observation_generation")?,
        previous_observation_id: decode_optional_id(
            &mut decoder,
            "ticket-publication-observation-content-v1.previous_observation_id",
        )?,
        publication_generation: decoder
            .unsigned("ticket-publication-observation-content-v1.publication_generation")?,
        publication_id: decoder
            .fixed_bytes("ticket-publication-observation-content-v1.publication_id")?,
        ticket_digest: decoder
            .fixed_bytes("ticket-publication-observation-content-v1.ticket_digest")?,
        observed_at_unix_seconds: decoder
            .unsigned("ticket-publication-observation-content-v1.observed_at_unix_seconds")?,
    };
    decoder.finish()?;
    validate_observation_content(&content)?;
    if encode_ticket_publication_observation_content_v1(&content)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(content)
}

fn decode_publication_conflict_proof_content_v1(
    bytes: &[u8],
) -> Result<CanonicalPublicationConflictProofContentV1, CanonicalPublicationConflictError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("publication-conflict-proof-content-v1", 8)?;
    decoder.expect_version("publication-conflict-proof-content-v1.schema_version")?;
    let content = CanonicalPublicationConflictProofContentV1 {
        local_account_id: decoder
            .fixed_bytes("publication-conflict-proof-content-v1.local_account_id")?,
        detector_device_id: decoder
            .fixed_bytes("publication-conflict-proof-content-v1.detector_device_id")?,
        detected_at_unix_seconds: decoder
            .unsigned("publication-conflict-proof-content-v1.detected_at_unix_seconds")?,
        channel_id: decoder.fixed_bytes("publication-conflict-proof-content-v1.channel_id")?,
        publication_generation: decoder
            .unsigned("publication-conflict-proof-content-v1.publication_generation")?,
        first_observation: decode_and_verify_signed_ticket_publication_observation_v1(
            decoder.bytes("publication-conflict-proof-content-v1.first_observation")?,
        )?,
        conflicting_observation: decode_and_verify_signed_ticket_publication_observation_v1(
            decoder.bytes("publication-conflict-proof-content-v1.conflicting_observation")?,
        )?,
    };
    decoder.finish()?;
    validate_conflict_proof_content(&content)?;
    if encode_publication_conflict_proof_content_v1(&content)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(content)
}

fn validate_publication_content(
    content: &CanonicalTicketPublicationContentV1,
) -> Result<(), CanonicalPublicationConflictError> {
    validate_key(&content.publisher_account_id, "publisher Account ID")?;
    validate_key(&content.publisher_device_id, "publisher Device ID")?;
    validate_key(&content.recipient_account_id, "recipient Account ID")?;
    let predecessor_shape = (content.generation == 1 && content.previous_publication_id.is_none())
        || (content.generation > 1 && content.previous_publication_id.is_some());
    let lifetime = content
        .expires_at_unix_seconds
        .checked_sub(content.published_at_unix_seconds)
        .ok_or(CanonicalPublicationConflictError::InvalidPublication)?;
    if !predecessor_shape
        || !(MIN_CANONICAL_TICKET_PUBLICATION_TTL_SECONDS
            ..=MAX_CANONICAL_TICKET_PUBLICATION_TTL_SECONDS)
            .contains(&lifetime)
        || content.ticket.is_empty()
        || content.ticket.len() > MAX_CANONICAL_TICKET_BYTES
        || content.ticket_digest != ticket_publication_ticket_digest_v1(&content.ticket)
    {
        return Err(CanonicalPublicationConflictError::InvalidPublication);
    }
    std::str::from_utf8(&content.ticket)
        .map_err(|_| CanonicalPublicationConflictError::InvalidTicketText)?;
    Ok(())
}

fn validate_observation_content(
    content: &CanonicalTicketPublicationObservationContentV1,
) -> Result<(), CanonicalPublicationConflictError> {
    validate_key(&content.local_account_id, "local Account ID")?;
    validate_key(&content.local_device_id, "local Device ID")?;
    validate_key(&content.publisher_account_id, "publisher Account ID")?;
    validate_key(&content.publisher_device_id, "publisher Device ID")?;
    let predecessor_shape = (content.observation_generation == 1
        && content.previous_observation_id.is_none())
        || (content.observation_generation > 1 && content.previous_observation_id.is_some());
    if !predecessor_shape || content.publication_generation == 0 {
        return Err(CanonicalPublicationConflictError::InvalidObservation);
    }
    Ok(())
}

fn validate_conflict_proof_content(
    content: &CanonicalPublicationConflictProofContentV1,
) -> Result<(), CanonicalPublicationConflictError> {
    validate_key(&content.local_account_id, "local Account ID")?;
    validate_key(&content.detector_device_id, "detector Device ID")?;
    if content.detected_at_unix_seconds == 0 {
        return Err(CanonicalPublicationConflictError::InvalidConflictProof);
    }
    verify_conflict_pair(
        content.local_account_id,
        &content.first_observation,
        &content.conflicting_observation,
    )?;
    if content.channel_id != content.first_observation.content.channel_id
        || content.publication_generation
            != content.first_observation.content.publication_generation
        || ticket_publication_observation_id_v1(&content.first_observation)?
            >= ticket_publication_observation_id_v1(&content.conflicting_observation)?
    {
        return Err(CanonicalPublicationConflictError::InvalidConflictProof);
    }
    Ok(())
}

fn canonical_observation_pair(
    left: CanonicalSignedTicketPublicationObservationV1,
    right: CanonicalSignedTicketPublicationObservationV1,
) -> Result<
    (
        CanonicalSignedTicketPublicationObservationV1,
        CanonicalSignedTicketPublicationObservationV1,
    ),
    CanonicalPublicationConflictError,
> {
    let left_id = ticket_publication_observation_id_v1(&left)?;
    let right_id = ticket_publication_observation_id_v1(&right)?;
    if left_id == right_id {
        return Err(CanonicalPublicationConflictError::InvalidConflictPair);
    }
    Ok(if left_id < right_id {
        (left, right)
    } else {
        (right, left)
    })
}

fn verify_conflict_pair(
    local_account_id: [u8; 32],
    first: &CanonicalSignedTicketPublicationObservationV1,
    conflicting: &CanonicalSignedTicketPublicationObservationV1,
) -> Result<(), CanonicalPublicationConflictError> {
    verify_signed_ticket_publication_observation_v1(first)?;
    verify_signed_ticket_publication_observation_v1(conflicting)?;
    if first.content.local_account_id != local_account_id
        || conflicting.content.local_account_id != local_account_id
        || first.content.channel_id != conflicting.content.channel_id
        || first.content.publisher_account_id != conflicting.content.publisher_account_id
        || first.content.publisher_device_id != conflicting.content.publisher_device_id
        || first.content.publication_generation != conflicting.content.publication_generation
        || (first.content.publication_id == conflicting.content.publication_id
            && first.content.ticket_digest == conflicting.content.ticket_digest)
    {
        return Err(CanonicalPublicationConflictError::InvalidConflictPair);
    }
    Ok(())
}

fn encode_optional_id(output: &mut Vec<u8>, id: Option<[u8; 32]>) {
    match id {
        None => push_array_header(output, 0),
        Some(id) => {
            push_array_header(output, 1);
            push_bytes(output, &id);
        }
    }
}

fn decode_optional_id(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
) -> Result<Option<[u8; 32]>, CanonicalPublicationConflictError> {
    match decoder.array(field)? {
        0 => Ok(None),
        1 => Ok(Some(decoder.fixed_bytes(field)?)),
        actual => Err(CanonicalAccountError::WrongArrayLength {
            object: field,
            expected: 1,
            actual,
        }
        .into()),
    }
}

fn validate_key(
    bytes: &[u8; 32],
    object: &'static str,
) -> Result<(), CanonicalPublicationConflictError> {
    VerifyingKey::from_bytes(bytes)
        .map(|_| ())
        .map_err(|_| CanonicalPublicationConflictError::InvalidPublicKey { object })
}

fn verify_ed25519(
    public_key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
    object: &'static str,
) -> Result<(), CanonicalPublicationConflictError> {
    let key = VerifyingKey::from_bytes(public_key)
        .map_err(|_| CanonicalPublicationConflictError::InvalidPublicKey { object })?;
    key.verify_strict(message, &Signature::from_bytes(signature))
        .map_err(|_| CanonicalPublicationConflictError::InvalidSignature { object })
}

fn ensure_bound(
    bytes: &[u8],
    object: &'static str,
    maximum: usize,
) -> Result<(), CanonicalPublicationConflictError> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(CanonicalPublicationConflictError::ObjectTooLarge { object, maximum });
    }
    Ok(())
}

fn domain_hash(domain: &[u8], value: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(value);
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Vectors {
        input: Input,
        expected: Expected,
    }

    #[derive(Deserialize)]
    struct Input {
        publisher_account_seed_hex: String,
        local_account_seed_hex: String,
        publisher_device_seed_hex: String,
        observer_1_seed_hex: String,
        observer_2_seed_hex: String,
        detector_seed_hex: String,
        channel_id_hex: String,
        first_ticket: String,
        conflicting_ticket: String,
        published_at_unix_seconds: u64,
        expires_at_unix_seconds: u64,
        observed_at_unix_seconds: u64,
        detected_at_unix_seconds: u64,
    }

    #[derive(Deserialize)]
    struct Expected {
        public_keys: PublicKeys,
        first_publication: SignedExpected,
        conflicting_publication: SignedExpected,
        first_observation: SignedExpected,
        conflicting_observation: SignedExpected,
        conflict_proof: ProofExpected,
    }

    #[derive(Deserialize)]
    struct PublicKeys {
        publisher_account_id_hex: String,
        local_account_id_hex: String,
        publisher_device_id_hex: String,
        observer_1_device_id_hex: String,
        observer_2_device_id_hex: String,
        detector_device_id_hex: String,
    }

    #[derive(Deserialize)]
    struct SignedExpected {
        content_cbor_hex: String,
        signing_preimage_hex: String,
        signature_hex: String,
        signed_cbor_hex: String,
        publication_id_hex: Option<String>,
        ticket_digest_hex: Option<String>,
        observation_id_hex: Option<String>,
    }

    #[derive(Deserialize)]
    struct ProofExpected {
        content_cbor_hex: String,
        signing_preimage_hex: String,
        signature_hex: String,
        signed_cbor_hex: String,
        proof_id_hex: String,
        evidence_id_hex: String,
        first_observation_id_hex: String,
        conflicting_observation_id_hex: String,
    }

    fn vectors() -> Result<Vectors, serde_json::Error> {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-PUBLICATION-CONFLICT-VECTORS.json"
        )))
    }

    fn bytes(value: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if !value.len().is_multiple_of(2) {
            return Err("odd hexadecimal length".into());
        }
        (0..value.len())
            .step_by(2)
            .map(|index| Ok(u8::from_str_radix(&value[index..index + 2], 16)?))
            .collect()
    }

    fn fixed<const N: usize>(value: &str) -> Result<[u8; N], Box<dyn std::error::Error>> {
        Ok(bytes(value)?
            .try_into()
            .map_err(|_| "unexpected vector length")?)
    }

    fn key(value: &str) -> Result<SigningKey, Box<dyn std::error::Error>> {
        Ok(SigningKey::from_bytes(&fixed(value)?))
    }

    fn hex(value: &[u8]) -> String {
        value.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn assert_signed(
        content: &[u8],
        preimage: &[u8],
        signature: &[u8],
        signed: &[u8],
        expected: &SignedExpected,
    ) {
        assert_eq!(hex(content), expected.content_cbor_hex);
        assert_eq!(hex(preimage), expected.signing_preimage_hex);
        assert_eq!(hex(signature), expected.signature_hex);
        assert_eq!(hex(signed), expected.signed_cbor_hex);
    }

    #[test]
    fn publication_observation_and_proof_match_golden_vectors()
    -> Result<(), Box<dyn std::error::Error>> {
        let vectors = vectors()?;
        let publisher_account = key(&vectors.input.publisher_account_seed_hex)?;
        let local_account = key(&vectors.input.local_account_seed_hex)?;
        let publisher = key(&vectors.input.publisher_device_seed_hex)?;
        let observer_1 = key(&vectors.input.observer_1_seed_hex)?;
        let observer_2 = key(&vectors.input.observer_2_seed_hex)?;
        let detector = key(&vectors.input.detector_seed_hex)?;
        assert_eq!(
            hex(&publisher_account.verifying_key().to_bytes()),
            vectors.expected.public_keys.publisher_account_id_hex
        );
        assert_eq!(
            hex(&local_account.verifying_key().to_bytes()),
            vectors.expected.public_keys.local_account_id_hex
        );
        for (actual, expected) in [
            (
                publisher.verifying_key().to_bytes(),
                &vectors.expected.public_keys.publisher_device_id_hex,
            ),
            (
                observer_1.verifying_key().to_bytes(),
                &vectors.expected.public_keys.observer_1_device_id_hex,
            ),
            (
                observer_2.verifying_key().to_bytes(),
                &vectors.expected.public_keys.observer_2_device_id_hex,
            ),
            (
                detector.verifying_key().to_bytes(),
                &vectors.expected.public_keys.detector_device_id_hex,
            ),
        ] {
            assert_eq!(hex(&actual), *expected);
        }
        let ttl = vectors
            .input
            .expires_at_unix_seconds
            .checked_sub(vectors.input.published_at_unix_seconds)
            .ok_or("invalid vector lifetime")?;
        let channel_id = fixed(&vectors.input.channel_id_hex)?;
        let publisher_account_id = publisher_account.verifying_key().to_bytes();
        let local_account_id = local_account.verifying_key().to_bytes();
        let first = sign_ticket_publication_v1(
            &publisher,
            channel_id,
            publisher_account_id,
            local_account_id,
            vectors.input.first_ticket.as_bytes().to_vec(),
            vectors.input.published_at_unix_seconds,
            ttl,
            None,
        )?;
        let conflicting = sign_ticket_publication_v1(
            &publisher,
            channel_id,
            publisher_account_id,
            local_account_id,
            vectors.input.conflicting_ticket.as_bytes().to_vec(),
            vectors.input.published_at_unix_seconds,
            ttl,
            None,
        )?;
        for (actual, expected) in [
            (&first, &vectors.expected.first_publication),
            (&conflicting, &vectors.expected.conflicting_publication),
        ] {
            let content = encode_ticket_publication_content_v1(&actual.content)?;
            let preimage = ticket_publication_signing_bytes_v1(&actual.content)?;
            let signed = encode_signed_ticket_publication_v1(actual)?;
            assert_signed(&content, &preimage, &actual.signature, &signed, expected);
            assert_eq!(
                hex(&ticket_publication_id_v1(actual)?),
                expected
                    .publication_id_hex
                    .as_deref()
                    .ok_or("publication ID missing")?
            );
            assert_eq!(
                hex(&actual.content.ticket_digest),
                expected
                    .ticket_digest_hex
                    .as_deref()
                    .ok_or("ticket digest missing")?
            );
            assert_eq!(
                decode_and_verify_signed_ticket_publication_v1(&signed)?,
                *actual
            );
        }
        let first_observation = sign_ticket_publication_observation_v1(
            &observer_1,
            local_account_id,
            &first,
            vectors.input.observed_at_unix_seconds,
            None,
        )?;
        let conflicting_observation = sign_ticket_publication_observation_v1(
            &observer_2,
            local_account_id,
            &conflicting,
            vectors.input.observed_at_unix_seconds,
            None,
        )?;
        for (actual, expected) in [
            (&first_observation, &vectors.expected.first_observation),
            (
                &conflicting_observation,
                &vectors.expected.conflicting_observation,
            ),
        ] {
            let content = encode_ticket_publication_observation_content_v1(&actual.content)?;
            let preimage = ticket_publication_observation_signing_bytes_v1(&actual.content)?;
            let signed = encode_signed_ticket_publication_observation_v1(actual)?;
            assert_signed(&content, &preimage, &actual.signature, &signed, expected);
            assert_eq!(
                hex(&ticket_publication_observation_id_v1(actual)?),
                expected
                    .observation_id_hex
                    .as_deref()
                    .ok_or("observation ID missing")?
            );
            assert_eq!(
                decode_and_verify_signed_ticket_publication_observation_v1(&signed)?,
                *actual
            );
        }
        let proof = sign_publication_conflict_proof_v1(
            &detector,
            local_account_id,
            vectors.input.detected_at_unix_seconds,
            first_observation,
            conflicting_observation,
        )?;
        let content = encode_publication_conflict_proof_content_v1(&proof.content)?;
        let preimage = publication_conflict_proof_signing_bytes_v1(&proof.content)?;
        let signed = encode_signed_publication_conflict_proof_v1(&proof)?;
        assert_eq!(
            hex(&content),
            vectors.expected.conflict_proof.content_cbor_hex
        );
        assert_eq!(
            hex(&preimage),
            vectors.expected.conflict_proof.signing_preimage_hex
        );
        assert_eq!(
            hex(&proof.signature),
            vectors.expected.conflict_proof.signature_hex
        );
        assert_eq!(
            hex(&signed),
            vectors.expected.conflict_proof.signed_cbor_hex
        );
        assert_eq!(
            hex(&publication_conflict_proof_id_v1(&proof)?),
            vectors.expected.conflict_proof.proof_id_hex
        );
        assert_eq!(
            hex(&publication_conflict_evidence_id_v1(&proof)?),
            vectors.expected.conflict_proof.evidence_id_hex
        );
        assert_eq!(
            hex(&ticket_publication_observation_id_v1(
                &proof.content.first_observation
            )?),
            vectors.expected.conflict_proof.first_observation_id_hex
        );
        assert_eq!(
            hex(&ticket_publication_observation_id_v1(
                &proof.content.conflicting_observation
            )?),
            vectors
                .expected
                .conflict_proof
                .conflicting_observation_id_hex
        );
        assert_eq!(
            decode_and_verify_signed_publication_conflict_proof_v1(&signed)?,
            proof
        );
        Ok(())
    }

    #[test]
    fn chains_conflicts_and_substitutions_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
        let publisher = SigningKey::from_bytes(&[1; 32]);
        let local_root = SigningKey::from_bytes(&[2; 32]);
        let observer = SigningKey::from_bytes(&[3; 32]);
        let first = sign_ticket_publication_v1(
            &publisher,
            [4; 32],
            SigningKey::from_bytes(&[5; 32]).verifying_key().to_bytes(),
            local_root.verifying_key().to_bytes(),
            b"first".to_vec(),
            100,
            30,
            None,
        )?;
        let successor = sign_ticket_publication_v1(
            &publisher,
            [4; 32],
            first.content.publisher_account_id,
            first.content.recipient_account_id,
            b"successor".to_vec(),
            110,
            30,
            Some(&first),
        )?;
        verify_ticket_publication_chain_link_v1(&first, &successor)?;
        let observation = sign_ticket_publication_observation_v1(
            &observer,
            first.content.recipient_account_id,
            &first,
            101,
            None,
        )?;
        let next_observation = sign_ticket_publication_observation_v1(
            &observer,
            first.content.recipient_account_id,
            &successor,
            111,
            Some(&observation),
        )?;
        verify_ticket_publication_observation_chain_link_v1(&observation, &next_observation)?;
        let mut changed = successor.clone();
        changed.content.previous_publication_id = Some([0; 32]);
        assert!(verify_signed_ticket_publication_v1(&changed).is_err());
        let mut changed = next_observation;
        changed.content.publication_generation = first.content.generation;
        assert!(verify_signed_ticket_publication_observation_v1(&changed).is_err());
        assert!(
            sign_publication_conflict_proof_v1(
                &observer,
                first.content.recipient_account_id,
                120,
                observation.clone(),
                observation,
            )
            .is_err()
        );
        Ok(())
    }
}
