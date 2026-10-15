//! Inactive deterministic-CBOR publication-conflict resolution ceremony.
//!
//! This module completes the canonical publication-conflict surface with a
//! Device request, an Account-Root resolution, a self-contained response and
//! a compact portable verification claim. The active Postcard/base64url
//! ceremony remains unchanged.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use thiserror::Error;

use crate::{
    canonical_account_v1::{
        AccountDecoder, CanonicalAccountError, CanonicalSignedAccountDeviceListV1,
        verify_signed_account_device_list_v1,
    },
    canonical_publication_conflict_v1::{
        CanonicalPublicationConflictError, CanonicalSignedPublicationConflictProofV1,
        decode_and_verify_signed_publication_conflict_proof_v1,
        encode_signed_publication_conflict_proof_v1, publication_conflict_evidence_id_v1,
        verify_signed_publication_conflict_proof_v1,
    },
    canonical_ticket_publication_v1::ticket_publication_channel_id_v1,
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
        push_unsigned, validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_BYTES: usize = 9 * 1024 * 1024;
pub const MAX_CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_BYTES: usize = 8 * 1024;
pub const MAX_CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_RESPONSE_BYTES: usize = 9 * 1024 * 1024;
pub const MAX_CANONICAL_PUBLICATION_CONFLICT_REPLACEMENT_TICKET_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_CANONICAL_PUBLICATION_CONFLICT_CLAIM_BYTES: usize = 1024;
pub const MAX_CANONICAL_PUBLICATION_CONFLICT_CLAIM_URI_BYTES: usize = 3072;
pub const CANONICAL_PUBLICATION_CONFLICT_CONFIRMATION_BYTES: usize = 12;

pub const CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:publication-conflict-resolution-request:cbor-v1\0";
pub const CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_ID_DOMAIN: &[u8] =
    b"kilogram:publication-conflict-resolution-request-id:cbor-v1\0";
pub const CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:publication-conflict-resolution-root-signature:cbor-v1\0";
pub const CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_ID_DOMAIN: &[u8] =
    b"kilogram:publication-conflict-resolution-id:cbor-v1\0";
pub const CANONICAL_PUBLICATION_CONFLICT_REPLACEMENT_TICKET_DIGEST_DOMAIN: &[u8] =
    b"kilogram:publication-conflict-replacement-ticket-digest:cbor-v1\0";
pub const CANONICAL_PUBLICATION_CONFLICT_CONFIRMATION_CODE_DOMAIN: &[u8] =
    b"kilogram:publication-conflict-confirmation:cbor-v1\0";
pub const CANONICAL_PUBLICATION_CONFLICT_CLAIM_URI_PREFIX: &str =
    "kilogram://publication-conflict/cbor-v1/";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum CanonicalPublicationConflictRoutePolicyV1 {
    Auto = 0,
    DirectOnly = 1,
    RelayOnly = 2,
}

impl CanonicalPublicationConflictRoutePolicyV1 {
    pub const fn code(self) -> u64 {
        self as u64
    }

    fn from_code(code: u64) -> Result<Self, CanonicalPublicationResolutionError> {
        match code {
            0 => Ok(Self::Auto),
            1 => Ok(Self::DirectOnly),
            2 => Ok(Self::RelayOnly),
            _ => Err(CanonicalPublicationResolutionError::InvalidRoutePolicy),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum CanonicalPublicationConflictClaimKindV1 {
    Request = 0,
    Response = 1,
}

impl CanonicalPublicationConflictClaimKindV1 {
    pub const fn code(self) -> u64 {
        self as u64
    }

    fn from_code(code: u64) -> Result<Self, CanonicalPublicationResolutionError> {
        match code {
            0 => Ok(Self::Request),
            1 => Ok(Self::Response),
            _ => Err(CanonicalPublicationResolutionError::InvalidClaim),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalPublicationConflictResolutionRequestContentV1 {
    pub local_account_id: [u8; 32],
    pub requester_device_id: [u8; 32],
    pub authority_revision: u64,
    pub conflict_proof: CanonicalSignedPublicationConflictProofV1,
    pub peer_account_id: [u8; 32],
    pub peer_device_id: [u8; 32],
    pub old_write_key: [u8; 32],
    pub new_write_key: [u8; 32],
    pub old_channel_epoch: u64,
    pub new_channel_epoch: u64,
    pub route_policy: CanonicalPublicationConflictRoutePolicyV1,
    pub replacement_ticket: Vec<u8>,
    pub requested_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedPublicationConflictResolutionRequestV1 {
    pub content: CanonicalPublicationConflictResolutionRequestContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalPublicationConflictResolutionContentV1 {
    pub request_id: [u8; 32],
    pub local_account_id: [u8; 32],
    pub authority_revision: u64,
    pub conflict_evidence_id: [u8; 32],
    pub peer_account_id: [u8; 32],
    pub peer_device_id: [u8; 32],
    pub old_write_key: [u8; 32],
    pub new_write_key: [u8; 32],
    pub old_channel_epoch: u64,
    pub new_channel_epoch: u64,
    pub route_policy: CanonicalPublicationConflictRoutePolicyV1,
    pub replacement_ticket_digest: [u8; 32],
    pub authorized_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRootSignedPublicationConflictResolutionV1 {
    pub content: CanonicalPublicationConflictResolutionContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalPublicationConflictResolutionResponseV1 {
    pub request_id: [u8; 32],
    pub resolution: CanonicalRootSignedPublicationConflictResolutionV1,
    pub replacement_ticket: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalPublicationConflictClaimV1 {
    pub kind: CanonicalPublicationConflictClaimKindV1,
    pub artifact_id: [u8; 32],
    pub request_id: [u8; 32],
    pub local_account_id: [u8; 32],
    pub authority_revision: u64,
    pub conflict_evidence_id: [u8; 32],
    pub peer_account_id: [u8; 32],
    pub peer_device_id: [u8; 32],
    pub old_publication_channel_id: [u8; 32],
    pub new_publication_channel_id: [u8; 32],
    pub old_channel_epoch: u64,
    pub new_channel_epoch: u64,
    pub route_policy: CanonicalPublicationConflictRoutePolicyV1,
    pub replacement_ticket_digest: [u8; 32],
    pub confirmation_code: [u8; CANONICAL_PUBLICATION_CONFLICT_CONFIRMATION_BYTES],
}

#[derive(Debug, Error)]
pub enum CanonicalPublicationResolutionError {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),
    #[error(transparent)]
    Decode(#[from] CanonicalAccountError),
    #[error(transparent)]
    Conflict(#[from] CanonicalPublicationConflictError),
    #[error("canonical publication-resolution {object} exceeds {maximum} bytes")]
    ObjectTooLarge {
        object: &'static str,
        maximum: usize,
    },
    #[error("canonical publication-resolution {object} contains an invalid Ed25519 key")]
    InvalidPublicKey { object: &'static str },
    #[error("canonical publication-resolution {object} contains an invalid signature")]
    InvalidSignature { object: &'static str },
    #[error("canonical publication-resolution request is invalid")]
    InvalidRequest,
    #[error("canonical publication-resolution request is not authorized by the supplied roster")]
    InvalidRequestAuthority,
    #[error("canonical publication-resolution route policy is unknown")]
    InvalidRoutePolicy,
    #[error("canonical publication-resolution does not match its request")]
    ResolutionRequestMismatch,
    #[error("canonical publication-resolution response is invalid")]
    InvalidResponse,
    #[error("canonical publication-resolution replacement ticket is not valid UTF-8")]
    InvalidTicketText,
    #[error("canonical publication-conflict claim is invalid")]
    InvalidClaim,
    #[error("canonical publication-conflict claim does not match its signed artifact")]
    ClaimMismatch,
    #[error("canonical publication-conflict claim URI is invalid")]
    InvalidClaimUri,
}

#[allow(clippy::too_many_arguments)]
pub fn sign_publication_conflict_resolution_request_v1(
    requester_signing_key: &SigningKey,
    local_account_id: [u8; 32],
    authority_revision: u64,
    conflict_proof: CanonicalSignedPublicationConflictProofV1,
    old_write_key: [u8; 32],
    new_write_key: [u8; 32],
    old_channel_epoch: u64,
    new_channel_epoch: u64,
    route_policy: CanonicalPublicationConflictRoutePolicyV1,
    replacement_ticket: Vec<u8>,
    requested_at_unix_seconds: u64,
) -> Result<
    CanonicalSignedPublicationConflictResolutionRequestV1,
    CanonicalPublicationResolutionError,
> {
    let content = CanonicalPublicationConflictResolutionRequestContentV1 {
        local_account_id,
        requester_device_id: requester_signing_key.verifying_key().to_bytes(),
        authority_revision,
        peer_account_id: conflict_proof
            .content
            .first_observation
            .content
            .publisher_account_id,
        peer_device_id: conflict_proof
            .content
            .first_observation
            .content
            .publisher_device_id,
        conflict_proof,
        old_write_key,
        new_write_key,
        old_channel_epoch,
        new_channel_epoch,
        route_policy,
        replacement_ticket,
        requested_at_unix_seconds,
    };
    let signature = requester_signing_key
        .sign(&publication_conflict_resolution_request_signing_bytes_v1(
            &content,
        )?)
        .to_bytes();
    let request = CanonicalSignedPublicationConflictResolutionRequestV1 { content, signature };
    verify_signed_publication_conflict_resolution_request_v1(&request)?;
    Ok(request)
}

pub fn encode_publication_conflict_resolution_request_content_v1(
    content: &CanonicalPublicationConflictResolutionRequestContentV1,
) -> Result<Vec<u8>, CanonicalPublicationResolutionError> {
    validate_request_content(content)?;
    let proof = encode_signed_publication_conflict_proof_v1(&content.conflict_proof)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 14);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.local_account_id);
    push_bytes(&mut output, &content.requester_device_id);
    push_unsigned(&mut output, content.authority_revision);
    push_bytes(&mut output, &proof);
    push_bytes(&mut output, &content.peer_account_id);
    push_bytes(&mut output, &content.peer_device_id);
    push_bytes(&mut output, &content.old_write_key);
    push_bytes(&mut output, &content.new_write_key);
    push_unsigned(&mut output, content.old_channel_epoch);
    push_unsigned(&mut output, content.new_channel_epoch);
    push_unsigned(&mut output, content.route_policy.code());
    push_bytes(&mut output, &content.replacement_ticket);
    push_unsigned(&mut output, content.requested_at_unix_seconds);
    Ok(output)
}

pub fn publication_conflict_resolution_request_signing_bytes_v1(
    content: &CanonicalPublicationConflictResolutionRequestContentV1,
) -> Result<Vec<u8>, CanonicalPublicationResolutionError> {
    Ok(prefixed(
        CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_SIGNATURE_DOMAIN,
        &encode_publication_conflict_resolution_request_content_v1(content)?,
    ))
}

pub fn encode_signed_publication_conflict_resolution_request_v1(
    request: &CanonicalSignedPublicationConflictResolutionRequestV1,
) -> Result<Vec<u8>, CanonicalPublicationResolutionError> {
    verify_signed_publication_conflict_resolution_request_v1(request)?;
    let content = encode_publication_conflict_resolution_request_content_v1(&request.content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content);
    push_bytes(&mut output, &request.signature);
    ensure_bound(
        &output,
        "signed resolution request",
        MAX_CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_publication_conflict_resolution_request_v1(
    bytes: &[u8],
) -> Result<
    CanonicalSignedPublicationConflictResolutionRequestV1,
    CanonicalPublicationResolutionError,
> {
    ensure_bound(
        bytes,
        "signed resolution request",
        MAX_CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_BYTES,
    )?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-publication-conflict-resolution-request-v1", 3)?;
    decoder.expect_version("signed-publication-conflict-resolution-request-v1.schema_version")?;
    let content = decode_publication_conflict_resolution_request_content_v1(
        decoder.bytes("signed-publication-conflict-resolution-request-v1.content")?,
    )?;
    let signature =
        decoder.fixed_bytes("signed-publication-conflict-resolution-request-v1.signature")?;
    decoder.finish()?;
    let request = CanonicalSignedPublicationConflictResolutionRequestV1 { content, signature };
    verify_signed_publication_conflict_resolution_request_v1(&request)?;
    if encode_signed_publication_conflict_resolution_request_v1(&request)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(request)
}

pub fn verify_signed_publication_conflict_resolution_request_v1(
    request: &CanonicalSignedPublicationConflictResolutionRequestV1,
) -> Result<(), CanonicalPublicationResolutionError> {
    validate_request_content(&request.content)?;
    verify_ed25519(
        &request.content.requester_device_id,
        &publication_conflict_resolution_request_signing_bytes_v1(&request.content)?,
        &request.signature,
        "signed-publication-conflict-resolution-request-v1",
    )
}

pub fn verify_publication_conflict_resolution_request_authority_v1(
    request: &CanonicalSignedPublicationConflictResolutionRequestV1,
    device_list: &CanonicalSignedAccountDeviceListV1,
) -> Result<(), CanonicalPublicationResolutionError> {
    verify_signed_publication_conflict_resolution_request_v1(request)?;
    verify_signed_account_device_list_v1(device_list)?;
    let authority = &device_list.content.authority_snapshot.content;
    let contains_device = |device_id: [u8; 32]| {
        device_list
            .content
            .devices
            .iter()
            .any(|certificate| certificate.content.device_id == device_id)
    };
    if authority.account_id != request.content.local_account_id
        || authority.revision != request.content.authority_revision
        || !contains_device(request.content.requester_device_id)
        || !contains_device(request.content.conflict_proof.content.detector_device_id)
    {
        return Err(CanonicalPublicationResolutionError::InvalidRequestAuthority);
    }
    Ok(())
}

pub fn publication_conflict_resolution_request_id_v1(
    request: &CanonicalSignedPublicationConflictResolutionRequestV1,
) -> Result<[u8; 32], CanonicalPublicationResolutionError> {
    Ok(domain_hash(
        CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_ID_DOMAIN,
        &encode_signed_publication_conflict_resolution_request_v1(request)?,
    ))
}

pub fn publication_conflict_replacement_ticket_digest_v1(ticket: &[u8]) -> [u8; 32] {
    domain_hash(
        CANONICAL_PUBLICATION_CONFLICT_REPLACEMENT_TICKET_DIGEST_DOMAIN,
        ticket,
    )
}

pub fn sign_publication_conflict_resolution_v1(
    root_signing_key: &SigningKey,
    request: &CanonicalSignedPublicationConflictResolutionRequestV1,
    device_list: &CanonicalSignedAccountDeviceListV1,
    authorized_at_unix_seconds: u64,
) -> Result<CanonicalRootSignedPublicationConflictResolutionV1, CanonicalPublicationResolutionError>
{
    verify_publication_conflict_resolution_request_authority_v1(request, device_list)?;
    if root_signing_key.verifying_key().to_bytes() != request.content.local_account_id {
        return Err(CanonicalPublicationResolutionError::InvalidRequestAuthority);
    }
    let content = CanonicalPublicationConflictResolutionContentV1 {
        request_id: publication_conflict_resolution_request_id_v1(request)?,
        local_account_id: request.content.local_account_id,
        authority_revision: request.content.authority_revision,
        conflict_evidence_id: publication_conflict_evidence_id_v1(&request.content.conflict_proof)?,
        peer_account_id: request.content.peer_account_id,
        peer_device_id: request.content.peer_device_id,
        old_write_key: request.content.old_write_key,
        new_write_key: request.content.new_write_key,
        old_channel_epoch: request.content.old_channel_epoch,
        new_channel_epoch: request.content.new_channel_epoch,
        route_policy: request.content.route_policy,
        replacement_ticket_digest: publication_conflict_replacement_ticket_digest_v1(
            &request.content.replacement_ticket,
        ),
        authorized_at_unix_seconds,
    };
    let signature = root_signing_key
        .sign(&publication_conflict_resolution_signing_bytes_v1(&content)?)
        .to_bytes();
    let resolution = CanonicalRootSignedPublicationConflictResolutionV1 { content, signature };
    verify_publication_conflict_resolution_for_request_v1(&resolution, request)?;
    Ok(resolution)
}

pub fn encode_publication_conflict_resolution_content_v1(
    content: &CanonicalPublicationConflictResolutionContentV1,
) -> Result<Vec<u8>, CanonicalPublicationResolutionError> {
    validate_resolution_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 14);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.request_id);
    push_bytes(&mut output, &content.local_account_id);
    push_unsigned(&mut output, content.authority_revision);
    push_bytes(&mut output, &content.conflict_evidence_id);
    push_bytes(&mut output, &content.peer_account_id);
    push_bytes(&mut output, &content.peer_device_id);
    push_bytes(&mut output, &content.old_write_key);
    push_bytes(&mut output, &content.new_write_key);
    push_unsigned(&mut output, content.old_channel_epoch);
    push_unsigned(&mut output, content.new_channel_epoch);
    push_unsigned(&mut output, content.route_policy.code());
    push_bytes(&mut output, &content.replacement_ticket_digest);
    push_unsigned(&mut output, content.authorized_at_unix_seconds);
    Ok(output)
}

pub fn publication_conflict_resolution_signing_bytes_v1(
    content: &CanonicalPublicationConflictResolutionContentV1,
) -> Result<Vec<u8>, CanonicalPublicationResolutionError> {
    Ok(prefixed(
        CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_SIGNATURE_DOMAIN,
        &encode_publication_conflict_resolution_content_v1(content)?,
    ))
}

pub fn encode_root_signed_publication_conflict_resolution_v1(
    resolution: &CanonicalRootSignedPublicationConflictResolutionV1,
) -> Result<Vec<u8>, CanonicalPublicationResolutionError> {
    verify_root_signed_publication_conflict_resolution_v1(resolution)?;
    let content = encode_publication_conflict_resolution_content_v1(&resolution.content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content);
    push_bytes(&mut output, &resolution.signature);
    ensure_bound(
        &output,
        "Root-signed resolution",
        MAX_CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_root_signed_publication_conflict_resolution_v1(
    bytes: &[u8],
) -> Result<CanonicalRootSignedPublicationConflictResolutionV1, CanonicalPublicationResolutionError>
{
    ensure_bound(
        bytes,
        "Root-signed resolution",
        MAX_CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("root-signed-publication-conflict-resolution-v1", 3)?;
    decoder.expect_version("root-signed-publication-conflict-resolution-v1.schema_version")?;
    let content = decode_publication_conflict_resolution_content_v1(
        decoder.bytes("root-signed-publication-conflict-resolution-v1.content")?,
    )?;
    let signature =
        decoder.fixed_bytes("root-signed-publication-conflict-resolution-v1.signature")?;
    decoder.finish()?;
    let resolution = CanonicalRootSignedPublicationConflictResolutionV1 { content, signature };
    verify_root_signed_publication_conflict_resolution_v1(&resolution)?;
    if encode_root_signed_publication_conflict_resolution_v1(&resolution)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(resolution)
}

pub fn verify_root_signed_publication_conflict_resolution_v1(
    resolution: &CanonicalRootSignedPublicationConflictResolutionV1,
) -> Result<(), CanonicalPublicationResolutionError> {
    validate_resolution_content(&resolution.content)?;
    verify_ed25519(
        &resolution.content.local_account_id,
        &publication_conflict_resolution_signing_bytes_v1(&resolution.content)?,
        &resolution.signature,
        "root-signed-publication-conflict-resolution-v1",
    )
}

pub fn verify_publication_conflict_resolution_for_request_v1(
    resolution: &CanonicalRootSignedPublicationConflictResolutionV1,
    request: &CanonicalSignedPublicationConflictResolutionRequestV1,
) -> Result<(), CanonicalPublicationResolutionError> {
    verify_root_signed_publication_conflict_resolution_v1(resolution)?;
    verify_signed_publication_conflict_resolution_request_v1(request)?;
    let expected = CanonicalPublicationConflictResolutionContentV1 {
        request_id: publication_conflict_resolution_request_id_v1(request)?,
        local_account_id: request.content.local_account_id,
        authority_revision: request.content.authority_revision,
        conflict_evidence_id: publication_conflict_evidence_id_v1(&request.content.conflict_proof)?,
        peer_account_id: request.content.peer_account_id,
        peer_device_id: request.content.peer_device_id,
        old_write_key: request.content.old_write_key,
        new_write_key: request.content.new_write_key,
        old_channel_epoch: request.content.old_channel_epoch,
        new_channel_epoch: request.content.new_channel_epoch,
        route_policy: request.content.route_policy,
        replacement_ticket_digest: publication_conflict_replacement_ticket_digest_v1(
            &request.content.replacement_ticket,
        ),
        authorized_at_unix_seconds: resolution.content.authorized_at_unix_seconds,
    };
    if resolution.content != expected {
        return Err(CanonicalPublicationResolutionError::ResolutionRequestMismatch);
    }
    Ok(())
}

pub fn publication_conflict_resolution_id_v1(
    resolution: &CanonicalRootSignedPublicationConflictResolutionV1,
) -> Result<[u8; 32], CanonicalPublicationResolutionError> {
    Ok(domain_hash(
        CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_ID_DOMAIN,
        &encode_root_signed_publication_conflict_resolution_v1(resolution)?,
    ))
}

pub fn create_publication_conflict_resolution_response_v1(
    request: &CanonicalSignedPublicationConflictResolutionRequestV1,
    resolution: CanonicalRootSignedPublicationConflictResolutionV1,
) -> Result<CanonicalPublicationConflictResolutionResponseV1, CanonicalPublicationResolutionError> {
    verify_publication_conflict_resolution_for_request_v1(&resolution, request)?;
    let response = CanonicalPublicationConflictResolutionResponseV1 {
        request_id: publication_conflict_resolution_request_id_v1(request)?,
        resolution,
        replacement_ticket: request.content.replacement_ticket.clone(),
    };
    verify_publication_conflict_resolution_response_for_request_v1(&response, request)?;
    Ok(response)
}

pub fn encode_publication_conflict_resolution_response_v1(
    response: &CanonicalPublicationConflictResolutionResponseV1,
) -> Result<Vec<u8>, CanonicalPublicationResolutionError> {
    verify_publication_conflict_resolution_response_v1(response)?;
    let resolution = encode_root_signed_publication_conflict_resolution_v1(&response.resolution)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 4);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &response.request_id);
    push_bytes(&mut output, &resolution);
    push_bytes(&mut output, &response.replacement_ticket);
    ensure_bound(
        &output,
        "resolution response",
        MAX_CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_RESPONSE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_publication_conflict_resolution_response_v1(
    bytes: &[u8],
) -> Result<CanonicalPublicationConflictResolutionResponseV1, CanonicalPublicationResolutionError> {
    ensure_bound(
        bytes,
        "resolution response",
        MAX_CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("publication-conflict-resolution-response-v1", 4)?;
    decoder.expect_version("publication-conflict-resolution-response-v1.schema_version")?;
    let request_id =
        decoder.fixed_bytes("publication-conflict-resolution-response-v1.request_id")?;
    let resolution = decode_and_verify_root_signed_publication_conflict_resolution_v1(
        decoder.bytes("publication-conflict-resolution-response-v1.resolution")?,
    )?;
    let replacement_ticket = decoder
        .bytes("publication-conflict-resolution-response-v1.replacement_ticket")?
        .to_vec();
    decoder.finish()?;
    let response = CanonicalPublicationConflictResolutionResponseV1 {
        request_id,
        resolution,
        replacement_ticket,
    };
    verify_publication_conflict_resolution_response_v1(&response)?;
    if encode_publication_conflict_resolution_response_v1(&response)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(response)
}

pub fn verify_publication_conflict_resolution_response_v1(
    response: &CanonicalPublicationConflictResolutionResponseV1,
) -> Result<(), CanonicalPublicationResolutionError> {
    verify_root_signed_publication_conflict_resolution_v1(&response.resolution)?;
    validate_replacement_ticket(&response.replacement_ticket)?;
    if response.request_id != response.resolution.content.request_id
        || publication_conflict_replacement_ticket_digest_v1(&response.replacement_ticket)
            != response.resolution.content.replacement_ticket_digest
    {
        return Err(CanonicalPublicationResolutionError::InvalidResponse);
    }
    Ok(())
}

pub fn verify_publication_conflict_resolution_response_for_request_v1(
    response: &CanonicalPublicationConflictResolutionResponseV1,
    request: &CanonicalSignedPublicationConflictResolutionRequestV1,
) -> Result<(), CanonicalPublicationResolutionError> {
    verify_publication_conflict_resolution_response_v1(response)?;
    verify_publication_conflict_resolution_for_request_v1(&response.resolution, request)?;
    if response.request_id != publication_conflict_resolution_request_id_v1(request)?
        || response.replacement_ticket != request.content.replacement_ticket
    {
        return Err(CanonicalPublicationResolutionError::ResolutionRequestMismatch);
    }
    Ok(())
}

pub fn publication_conflict_claim_for_request_v1(
    request: &CanonicalSignedPublicationConflictResolutionRequestV1,
) -> Result<CanonicalPublicationConflictClaimV1, CanonicalPublicationResolutionError> {
    verify_signed_publication_conflict_resolution_request_v1(request)?;
    let request_id = publication_conflict_resolution_request_id_v1(request)?;
    let common = claim_common_from_request(request)?;
    let mut claim = CanonicalPublicationConflictClaimV1 {
        kind: CanonicalPublicationConflictClaimKindV1::Request,
        artifact_id: request_id,
        request_id,
        local_account_id: common.local_account_id,
        authority_revision: common.authority_revision,
        conflict_evidence_id: common.conflict_evidence_id,
        peer_account_id: common.peer_account_id,
        peer_device_id: common.peer_device_id,
        old_publication_channel_id: common.old_publication_channel_id,
        new_publication_channel_id: common.new_publication_channel_id,
        old_channel_epoch: common.old_channel_epoch,
        new_channel_epoch: common.new_channel_epoch,
        route_policy: common.route_policy,
        replacement_ticket_digest: common.replacement_ticket_digest,
        confirmation_code: [0; CANONICAL_PUBLICATION_CONFLICT_CONFIRMATION_BYTES],
    };
    claim.confirmation_code = publication_conflict_confirmation_code_v1(&claim);
    verify_publication_conflict_claim_for_request_v1(&claim, request)?;
    Ok(claim)
}

pub fn publication_conflict_claim_for_response_v1(
    response: &CanonicalPublicationConflictResolutionResponseV1,
) -> Result<CanonicalPublicationConflictClaimV1, CanonicalPublicationResolutionError> {
    verify_publication_conflict_resolution_response_v1(response)?;
    let resolution = &response.resolution;
    let mut claim = CanonicalPublicationConflictClaimV1 {
        kind: CanonicalPublicationConflictClaimKindV1::Response,
        artifact_id: publication_conflict_resolution_id_v1(resolution)?,
        request_id: response.request_id,
        local_account_id: resolution.content.local_account_id,
        authority_revision: resolution.content.authority_revision,
        conflict_evidence_id: resolution.content.conflict_evidence_id,
        peer_account_id: resolution.content.peer_account_id,
        peer_device_id: resolution.content.peer_device_id,
        old_publication_channel_id: ticket_publication_channel_id_v1(
            &resolution.content.old_write_key,
        ),
        new_publication_channel_id: ticket_publication_channel_id_v1(
            &resolution.content.new_write_key,
        ),
        old_channel_epoch: resolution.content.old_channel_epoch,
        new_channel_epoch: resolution.content.new_channel_epoch,
        route_policy: resolution.content.route_policy,
        replacement_ticket_digest: resolution.content.replacement_ticket_digest,
        confirmation_code: [0; CANONICAL_PUBLICATION_CONFLICT_CONFIRMATION_BYTES],
    };
    claim.confirmation_code = publication_conflict_confirmation_code_v1(&claim);
    verify_publication_conflict_claim_for_response_v1(&claim, response)?;
    Ok(claim)
}

pub fn encode_publication_conflict_claim_v1(
    claim: &CanonicalPublicationConflictClaimV1,
) -> Result<Vec<u8>, CanonicalPublicationResolutionError> {
    verify_publication_conflict_claim_v1(claim)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 16);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_unsigned(&mut output, claim.kind.code());
    push_bytes(&mut output, &claim.artifact_id);
    push_bytes(&mut output, &claim.request_id);
    push_bytes(&mut output, &claim.local_account_id);
    push_unsigned(&mut output, claim.authority_revision);
    push_bytes(&mut output, &claim.conflict_evidence_id);
    push_bytes(&mut output, &claim.peer_account_id);
    push_bytes(&mut output, &claim.peer_device_id);
    push_bytes(&mut output, &claim.old_publication_channel_id);
    push_bytes(&mut output, &claim.new_publication_channel_id);
    push_unsigned(&mut output, claim.old_channel_epoch);
    push_unsigned(&mut output, claim.new_channel_epoch);
    push_unsigned(&mut output, claim.route_policy.code());
    push_bytes(&mut output, &claim.replacement_ticket_digest);
    push_bytes(&mut output, &claim.confirmation_code);
    ensure_bound(
        &output,
        "portable claim",
        MAX_CANONICAL_PUBLICATION_CONFLICT_CLAIM_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_publication_conflict_claim_v1(
    bytes: &[u8],
) -> Result<CanonicalPublicationConflictClaimV1, CanonicalPublicationResolutionError> {
    ensure_bound(
        bytes,
        "portable claim",
        MAX_CANONICAL_PUBLICATION_CONFLICT_CLAIM_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("publication-conflict-claim-v1", 16)?;
    decoder.expect_version("publication-conflict-claim-v1.schema_version")?;
    let claim = CanonicalPublicationConflictClaimV1 {
        kind: CanonicalPublicationConflictClaimKindV1::from_code(
            decoder.unsigned("publication-conflict-claim-v1.kind")?,
        )?,
        artifact_id: decoder.fixed_bytes("publication-conflict-claim-v1.artifact_id")?,
        request_id: decoder.fixed_bytes("publication-conflict-claim-v1.request_id")?,
        local_account_id: decoder.fixed_bytes("publication-conflict-claim-v1.local_account_id")?,
        authority_revision: decoder.unsigned("publication-conflict-claim-v1.authority_revision")?,
        conflict_evidence_id: decoder
            .fixed_bytes("publication-conflict-claim-v1.conflict_evidence_id")?,
        peer_account_id: decoder.fixed_bytes("publication-conflict-claim-v1.peer_account_id")?,
        peer_device_id: decoder.fixed_bytes("publication-conflict-claim-v1.peer_device_id")?,
        old_publication_channel_id: decoder
            .fixed_bytes("publication-conflict-claim-v1.old_publication_channel_id")?,
        new_publication_channel_id: decoder
            .fixed_bytes("publication-conflict-claim-v1.new_publication_channel_id")?,
        old_channel_epoch: decoder.unsigned("publication-conflict-claim-v1.old_channel_epoch")?,
        new_channel_epoch: decoder.unsigned("publication-conflict-claim-v1.new_channel_epoch")?,
        route_policy: CanonicalPublicationConflictRoutePolicyV1::from_code(
            decoder.unsigned("publication-conflict-claim-v1.route_policy")?,
        )?,
        replacement_ticket_digest: decoder
            .fixed_bytes("publication-conflict-claim-v1.replacement_ticket_digest")?,
        confirmation_code: decoder
            .fixed_bytes("publication-conflict-claim-v1.confirmation_code")?,
    };
    decoder.finish()?;
    verify_publication_conflict_claim_v1(&claim)?;
    if encode_publication_conflict_claim_v1(&claim)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(claim)
}

pub fn verify_publication_conflict_claim_v1(
    claim: &CanonicalPublicationConflictClaimV1,
) -> Result<(), CanonicalPublicationResolutionError> {
    validate_key(&claim.local_account_id, "claim local Account ID")?;
    validate_key(&claim.peer_account_id, "claim peer Account ID")?;
    validate_key(&claim.peer_device_id, "claim peer Device ID")?;
    if claim.authority_revision == 0
        || claim.old_publication_channel_id == claim.new_publication_channel_id
        || claim.new_channel_epoch <= claim.old_channel_epoch
        || claim.confirmation_code != publication_conflict_confirmation_code_v1(claim)
    {
        return Err(CanonicalPublicationResolutionError::InvalidClaim);
    }
    Ok(())
}

pub fn verify_publication_conflict_claim_for_request_v1(
    claim: &CanonicalPublicationConflictClaimV1,
    request: &CanonicalSignedPublicationConflictResolutionRequestV1,
) -> Result<(), CanonicalPublicationResolutionError> {
    verify_publication_conflict_claim_v1(claim)?;
    let expected = publication_conflict_claim_for_request_unchecked_v1(request)?;
    if claim != &expected {
        return Err(CanonicalPublicationResolutionError::ClaimMismatch);
    }
    Ok(())
}

pub fn verify_publication_conflict_claim_for_response_v1(
    claim: &CanonicalPublicationConflictClaimV1,
    response: &CanonicalPublicationConflictResolutionResponseV1,
) -> Result<(), CanonicalPublicationResolutionError> {
    verify_publication_conflict_claim_v1(claim)?;
    let expected = publication_conflict_claim_for_response_unchecked_v1(response)?;
    if claim != &expected {
        return Err(CanonicalPublicationResolutionError::ClaimMismatch);
    }
    Ok(())
}

pub fn encode_publication_conflict_claim_uri_v1(
    claim: &CanonicalPublicationConflictClaimV1,
) -> Result<String, CanonicalPublicationResolutionError> {
    let encoded = encode_publication_conflict_claim_v1(claim)?;
    let uri = format!(
        "{CANONICAL_PUBLICATION_CONFLICT_CLAIM_URI_PREFIX}{}",
        URL_SAFE_NO_PAD.encode(encoded)
    );
    if uri.len() > MAX_CANONICAL_PUBLICATION_CONFLICT_CLAIM_URI_BYTES {
        return Err(CanonicalPublicationResolutionError::InvalidClaimUri);
    }
    Ok(uri)
}

pub fn decode_and_verify_publication_conflict_claim_uri_v1(
    uri: &str,
) -> Result<CanonicalPublicationConflictClaimV1, CanonicalPublicationResolutionError> {
    if !uri.is_ascii()
        || !uri.starts_with(CANONICAL_PUBLICATION_CONFLICT_CLAIM_URI_PREFIX)
        || uri.len() > MAX_CANONICAL_PUBLICATION_CONFLICT_CLAIM_URI_BYTES
    {
        return Err(CanonicalPublicationResolutionError::InvalidClaimUri);
    }
    let payload = uri
        .strip_prefix(CANONICAL_PUBLICATION_CONFLICT_CLAIM_URI_PREFIX)
        .ok_or(CanonicalPublicationResolutionError::InvalidClaimUri)?;
    if payload.is_empty() || payload.contains('=') {
        return Err(CanonicalPublicationResolutionError::InvalidClaimUri);
    }
    let decoded = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| CanonicalPublicationResolutionError::InvalidClaimUri)?;
    let claim = decode_and_verify_publication_conflict_claim_v1(&decoded)?;
    if encode_publication_conflict_claim_uri_v1(&claim)? != uri {
        return Err(CanonicalPublicationResolutionError::InvalidClaimUri);
    }
    Ok(claim)
}

pub fn publication_conflict_confirmation_code_v1(
    claim: &CanonicalPublicationConflictClaimV1,
) -> [u8; CANONICAL_PUBLICATION_CONFLICT_CONFIRMATION_BYTES] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(CANONICAL_PUBLICATION_CONFLICT_CONFIRMATION_CODE_DOMAIN);
    let authority_revision = claim.authority_revision.to_be_bytes();
    let old_epoch = claim.old_channel_epoch.to_be_bytes();
    let new_epoch = claim.new_channel_epoch.to_be_bytes();
    let route = claim.route_policy.code().to_be_bytes();
    for value in [
        claim.request_id.as_slice(),
        claim.local_account_id.as_slice(),
        authority_revision.as_slice(),
        claim.conflict_evidence_id.as_slice(),
        claim.peer_account_id.as_slice(),
        claim.peer_device_id.as_slice(),
        claim.old_publication_channel_id.as_slice(),
        claim.new_publication_channel_id.as_slice(),
        old_epoch.as_slice(),
        new_epoch.as_slice(),
        route.as_slice(),
        claim.replacement_ticket_digest.as_slice(),
    ] {
        hasher.update(&(value.len() as u64).to_be_bytes());
        hasher.update(value);
    }
    let mut code = [0; CANONICAL_PUBLICATION_CONFLICT_CONFIRMATION_BYTES];
    let code_len = code.len();
    code.copy_from_slice(&hasher.finalize().as_bytes()[..code_len]);
    code
}

pub fn format_publication_conflict_confirmation_code_v1(
    code: &[u8; CANONICAL_PUBLICATION_CONFLICT_CONFIRMATION_BYTES],
) -> String {
    let mut output = String::from("KPC1");
    for chunk in code.as_chunks::<2>().0 {
        use std::fmt::Write as _;
        let _ = write!(output, "-{:02X}{:02X}", chunk[0], chunk[1]);
    }
    output
}

#[derive(Clone, Copy)]
struct ClaimCommon {
    local_account_id: [u8; 32],
    authority_revision: u64,
    conflict_evidence_id: [u8; 32],
    peer_account_id: [u8; 32],
    peer_device_id: [u8; 32],
    old_publication_channel_id: [u8; 32],
    new_publication_channel_id: [u8; 32],
    old_channel_epoch: u64,
    new_channel_epoch: u64,
    route_policy: CanonicalPublicationConflictRoutePolicyV1,
    replacement_ticket_digest: [u8; 32],
}

fn claim_common_from_request(
    request: &CanonicalSignedPublicationConflictResolutionRequestV1,
) -> Result<ClaimCommon, CanonicalPublicationResolutionError> {
    verify_signed_publication_conflict_resolution_request_v1(request)?;
    Ok(ClaimCommon {
        local_account_id: request.content.local_account_id,
        authority_revision: request.content.authority_revision,
        conflict_evidence_id: publication_conflict_evidence_id_v1(&request.content.conflict_proof)?,
        peer_account_id: request.content.peer_account_id,
        peer_device_id: request.content.peer_device_id,
        old_publication_channel_id: ticket_publication_channel_id_v1(
            &request.content.old_write_key,
        ),
        new_publication_channel_id: ticket_publication_channel_id_v1(
            &request.content.new_write_key,
        ),
        old_channel_epoch: request.content.old_channel_epoch,
        new_channel_epoch: request.content.new_channel_epoch,
        route_policy: request.content.route_policy,
        replacement_ticket_digest: publication_conflict_replacement_ticket_digest_v1(
            &request.content.replacement_ticket,
        ),
    })
}

fn publication_conflict_claim_for_request_unchecked_v1(
    request: &CanonicalSignedPublicationConflictResolutionRequestV1,
) -> Result<CanonicalPublicationConflictClaimV1, CanonicalPublicationResolutionError> {
    let request_id = publication_conflict_resolution_request_id_v1(request)?;
    let common = claim_common_from_request(request)?;
    let mut claim = CanonicalPublicationConflictClaimV1 {
        kind: CanonicalPublicationConflictClaimKindV1::Request,
        artifact_id: request_id,
        request_id,
        local_account_id: common.local_account_id,
        authority_revision: common.authority_revision,
        conflict_evidence_id: common.conflict_evidence_id,
        peer_account_id: common.peer_account_id,
        peer_device_id: common.peer_device_id,
        old_publication_channel_id: common.old_publication_channel_id,
        new_publication_channel_id: common.new_publication_channel_id,
        old_channel_epoch: common.old_channel_epoch,
        new_channel_epoch: common.new_channel_epoch,
        route_policy: common.route_policy,
        replacement_ticket_digest: common.replacement_ticket_digest,
        confirmation_code: [0; CANONICAL_PUBLICATION_CONFLICT_CONFIRMATION_BYTES],
    };
    claim.confirmation_code = publication_conflict_confirmation_code_v1(&claim);
    Ok(claim)
}

fn publication_conflict_claim_for_response_unchecked_v1(
    response: &CanonicalPublicationConflictResolutionResponseV1,
) -> Result<CanonicalPublicationConflictClaimV1, CanonicalPublicationResolutionError> {
    verify_publication_conflict_resolution_response_v1(response)?;
    let resolution = &response.resolution;
    let mut claim = CanonicalPublicationConflictClaimV1 {
        kind: CanonicalPublicationConflictClaimKindV1::Response,
        artifact_id: publication_conflict_resolution_id_v1(resolution)?,
        request_id: response.request_id,
        local_account_id: resolution.content.local_account_id,
        authority_revision: resolution.content.authority_revision,
        conflict_evidence_id: resolution.content.conflict_evidence_id,
        peer_account_id: resolution.content.peer_account_id,
        peer_device_id: resolution.content.peer_device_id,
        old_publication_channel_id: ticket_publication_channel_id_v1(
            &resolution.content.old_write_key,
        ),
        new_publication_channel_id: ticket_publication_channel_id_v1(
            &resolution.content.new_write_key,
        ),
        old_channel_epoch: resolution.content.old_channel_epoch,
        new_channel_epoch: resolution.content.new_channel_epoch,
        route_policy: resolution.content.route_policy,
        replacement_ticket_digest: resolution.content.replacement_ticket_digest,
        confirmation_code: [0; CANONICAL_PUBLICATION_CONFLICT_CONFIRMATION_BYTES],
    };
    claim.confirmation_code = publication_conflict_confirmation_code_v1(&claim);
    Ok(claim)
}

fn decode_publication_conflict_resolution_request_content_v1(
    bytes: &[u8],
) -> Result<
    CanonicalPublicationConflictResolutionRequestContentV1,
    CanonicalPublicationResolutionError,
> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("publication-conflict-resolution-request-content-v1", 14)?;
    decoder.expect_version("publication-conflict-resolution-request-content-v1.schema_version")?;
    let content = CanonicalPublicationConflictResolutionRequestContentV1 {
        local_account_id: decoder
            .fixed_bytes("publication-conflict-resolution-request-content-v1.local_account_id")?,
        requester_device_id: decoder.fixed_bytes(
            "publication-conflict-resolution-request-content-v1.requester_device_id",
        )?,
        authority_revision: decoder
            .unsigned("publication-conflict-resolution-request-content-v1.authority_revision")?,
        conflict_proof: decode_and_verify_signed_publication_conflict_proof_v1(
            decoder.bytes("publication-conflict-resolution-request-content-v1.conflict_proof")?,
        )?,
        peer_account_id: decoder
            .fixed_bytes("publication-conflict-resolution-request-content-v1.peer_account_id")?,
        peer_device_id: decoder
            .fixed_bytes("publication-conflict-resolution-request-content-v1.peer_device_id")?,
        old_write_key: decoder
            .fixed_bytes("publication-conflict-resolution-request-content-v1.old_write_key")?,
        new_write_key: decoder
            .fixed_bytes("publication-conflict-resolution-request-content-v1.new_write_key")?,
        old_channel_epoch: decoder
            .unsigned("publication-conflict-resolution-request-content-v1.old_channel_epoch")?,
        new_channel_epoch: decoder
            .unsigned("publication-conflict-resolution-request-content-v1.new_channel_epoch")?,
        route_policy: CanonicalPublicationConflictRoutePolicyV1::from_code(
            decoder.unsigned("publication-conflict-resolution-request-content-v1.route_policy")?,
        )?,
        replacement_ticket: decoder
            .bytes("publication-conflict-resolution-request-content-v1.replacement_ticket")?
            .to_vec(),
        requested_at_unix_seconds: decoder.unsigned(
            "publication-conflict-resolution-request-content-v1.requested_at_unix_seconds",
        )?,
    };
    decoder.finish()?;
    validate_request_content(&content)?;
    if encode_publication_conflict_resolution_request_content_v1(&content)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(content)
}

fn decode_publication_conflict_resolution_content_v1(
    bytes: &[u8],
) -> Result<CanonicalPublicationConflictResolutionContentV1, CanonicalPublicationResolutionError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("publication-conflict-resolution-content-v1", 14)?;
    decoder.expect_version("publication-conflict-resolution-content-v1.schema_version")?;
    let content = CanonicalPublicationConflictResolutionContentV1 {
        request_id: decoder.fixed_bytes("publication-conflict-resolution-content-v1.request_id")?,
        local_account_id: decoder
            .fixed_bytes("publication-conflict-resolution-content-v1.local_account_id")?,
        authority_revision: decoder
            .unsigned("publication-conflict-resolution-content-v1.authority_revision")?,
        conflict_evidence_id: decoder
            .fixed_bytes("publication-conflict-resolution-content-v1.conflict_evidence_id")?,
        peer_account_id: decoder
            .fixed_bytes("publication-conflict-resolution-content-v1.peer_account_id")?,
        peer_device_id: decoder
            .fixed_bytes("publication-conflict-resolution-content-v1.peer_device_id")?,
        old_write_key: decoder
            .fixed_bytes("publication-conflict-resolution-content-v1.old_write_key")?,
        new_write_key: decoder
            .fixed_bytes("publication-conflict-resolution-content-v1.new_write_key")?,
        old_channel_epoch: decoder
            .unsigned("publication-conflict-resolution-content-v1.old_channel_epoch")?,
        new_channel_epoch: decoder
            .unsigned("publication-conflict-resolution-content-v1.new_channel_epoch")?,
        route_policy: CanonicalPublicationConflictRoutePolicyV1::from_code(
            decoder.unsigned("publication-conflict-resolution-content-v1.route_policy")?,
        )?,
        replacement_ticket_digest: decoder
            .fixed_bytes("publication-conflict-resolution-content-v1.replacement_ticket_digest")?,
        authorized_at_unix_seconds: decoder
            .unsigned("publication-conflict-resolution-content-v1.authorized_at_unix_seconds")?,
    };
    decoder.finish()?;
    validate_resolution_content(&content)?;
    if encode_publication_conflict_resolution_content_v1(&content)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(content)
}

fn validate_request_content(
    content: &CanonicalPublicationConflictResolutionRequestContentV1,
) -> Result<(), CanonicalPublicationResolutionError> {
    verify_signed_publication_conflict_proof_v1(&content.conflict_proof)?;
    validate_key(&content.local_account_id, "request local Account ID")?;
    validate_key(&content.requester_device_id, "requester Device ID")?;
    validate_key(&content.peer_account_id, "request peer Account ID")?;
    validate_key(&content.peer_device_id, "request peer Device ID")?;
    validate_key(&content.old_write_key, "request old write key")?;
    validate_key(&content.new_write_key, "request new write key")?;
    validate_replacement_ticket(&content.replacement_ticket)?;
    let proof = &content.conflict_proof.content;
    if content.authority_revision == 0
        || content.requested_at_unix_seconds == 0
        || content.local_account_id != proof.local_account_id
        || content.peer_account_id != proof.first_observation.content.publisher_account_id
        || content.peer_device_id != proof.first_observation.content.publisher_device_id
        || ticket_publication_channel_id_v1(&content.old_write_key) != proof.channel_id
        || content.old_write_key == content.new_write_key
        || ticket_publication_channel_id_v1(&content.old_write_key)
            == ticket_publication_channel_id_v1(&content.new_write_key)
        || content.new_channel_epoch <= content.old_channel_epoch
    {
        return Err(CanonicalPublicationResolutionError::InvalidRequest);
    }
    Ok(())
}

fn validate_resolution_content(
    content: &CanonicalPublicationConflictResolutionContentV1,
) -> Result<(), CanonicalPublicationResolutionError> {
    validate_key(&content.local_account_id, "resolution local Account ID")?;
    validate_key(&content.peer_account_id, "resolution peer Account ID")?;
    validate_key(&content.peer_device_id, "resolution peer Device ID")?;
    validate_key(&content.old_write_key, "resolution old write key")?;
    validate_key(&content.new_write_key, "resolution new write key")?;
    if content.authority_revision == 0
        || content.authorized_at_unix_seconds == 0
        || content.old_write_key == content.new_write_key
        || ticket_publication_channel_id_v1(&content.old_write_key)
            == ticket_publication_channel_id_v1(&content.new_write_key)
        || content.new_channel_epoch <= content.old_channel_epoch
    {
        return Err(CanonicalPublicationResolutionError::ResolutionRequestMismatch);
    }
    Ok(())
}

fn validate_replacement_ticket(ticket: &[u8]) -> Result<(), CanonicalPublicationResolutionError> {
    if ticket.is_empty()
        || ticket.len() > MAX_CANONICAL_PUBLICATION_CONFLICT_REPLACEMENT_TICKET_BYTES
    {
        return Err(CanonicalPublicationResolutionError::InvalidRequest);
    }
    std::str::from_utf8(ticket)
        .map_err(|_| CanonicalPublicationResolutionError::InvalidTicketText)?;
    Ok(())
}

fn validate_key(
    key: &[u8; 32],
    object: &'static str,
) -> Result<(), CanonicalPublicationResolutionError> {
    VerifyingKey::from_bytes(key)
        .map(|_| ())
        .map_err(|_| CanonicalPublicationResolutionError::InvalidPublicKey { object })
}

fn verify_ed25519(
    key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
    object: &'static str,
) -> Result<(), CanonicalPublicationResolutionError> {
    let key = VerifyingKey::from_bytes(key)
        .map_err(|_| CanonicalPublicationResolutionError::InvalidPublicKey { object })?;
    let signature = Signature::try_from(signature.as_slice())
        .map_err(|_| CanonicalPublicationResolutionError::InvalidSignature { object })?;
    key.verify_strict(message, &signature)
        .map_err(|_| CanonicalPublicationResolutionError::InvalidSignature { object })
}

fn ensure_bound(
    bytes: &[u8],
    object: &'static str,
    maximum: usize,
) -> Result<(), CanonicalPublicationResolutionError> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(CanonicalPublicationResolutionError::ObjectTooLarge { object, maximum });
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
    use ed25519_dalek::{Signer, SigningKey};
    use serde::Deserialize;

    use super::*;
    use crate::{
        canonical_account_v1::{
            CanonicalAccountDeviceListContentV1, CanonicalSignedAccountDeviceListV1,
            account_device_list_signing_bytes_v1,
        },
        canonical_authority_v1::{
            CanonicalAccountAuthoritySnapshotContentV1, CanonicalDeviceCapabilityV1,
            CanonicalDeviceCertificateContentV1, CanonicalSignedAccountAuthoritySnapshotV1,
            CanonicalSignedDeviceCertificateV1, account_authority_snapshot_signing_bytes_v1,
            device_certificate_signing_bytes_v1,
        },
    };

    #[derive(Deserialize)]
    struct Vector {
        input: Input,
        expected: Expected,
    }

    #[derive(Deserialize)]
    struct Input {
        local_account_seed_hex: String,
        detector_seed_hex: String,
        old_write_seed_hex: String,
        new_write_seed_hex: String,
        authority_revision: u64,
        old_channel_epoch: u64,
        new_channel_epoch: u64,
        route_policy: u64,
        replacement_ticket: String,
        requested_at_unix_seconds: u64,
        authorized_at_unix_seconds: u64,
    }

    #[derive(Deserialize)]
    struct Expected {
        conflict_proof: SignedExpected,
        resolution_request: SignedExpected,
        resolution: SignedExpected,
        resolution_response: ResponseExpected,
        request_claim: ClaimExpected,
        response_claim: ClaimExpected,
        public_keys: PublicKeys,
    }

    #[derive(Deserialize)]
    struct PublicKeys {
        old_write_key_hex: String,
        new_write_key_hex: String,
    }

    #[derive(Deserialize)]
    struct SignedExpected {
        signed_cbor_hex: String,
        content_cbor_hex: String,
        signing_preimage_hex: String,
        signature_hex: String,
        #[serde(default)]
        request_id_hex: Option<String>,
        #[serde(default)]
        resolution_id_hex: Option<String>,
        #[serde(default)]
        replacement_ticket_digest_hex: Option<String>,
    }

    #[derive(Deserialize)]
    struct ResponseExpected {
        cbor_hex: String,
    }

    #[derive(Deserialize)]
    struct ClaimExpected {
        cbor_hex: String,
        uri: String,
        artifact_id_hex: String,
        confirmation_code_hex: String,
        confirmation_code: String,
    }

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    fn vectors() -> Result<Vector, serde_json::Error> {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-PUBLICATION-CONFLICT-VECTORS.json"
        )))
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn decode_hex(value: &str) -> TestResult<Vec<u8>> {
        if !value.len().is_multiple_of(2) {
            return Err("hex value has odd length".into());
        }
        Ok((0..value.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&value[index..index + 2], 16))
            .collect::<Result<Vec<_>, _>>()?)
    }

    fn fixed(value: &str) -> TestResult<[u8; 32]> {
        decode_hex(value)?
            .try_into()
            .map_err(|_| "expected 32 bytes".into())
    }

    fn authority_list(
        root: &SigningKey,
        device: &SigningKey,
        revision: u64,
    ) -> TestResult<CanonicalSignedAccountDeviceListV1> {
        let certificate_content = CanonicalDeviceCertificateContentV1 {
            account_id: root.verifying_key().to_bytes(),
            device_id: device.verifying_key().to_bytes(),
            encryption_public_key: [0xd0; 32],
            authority_sequence: revision - 1,
            capabilities: vec![
                CanonicalDeviceCapabilityV1::SignEvents,
                CanonicalDeviceCapabilityV1::SyncHistory,
            ],
        };
        let certificate = CanonicalSignedDeviceCertificateV1 {
            signature: root
                .sign(&device_certificate_signing_bytes_v1(&certificate_content)?)
                .to_bytes(),
            content: certificate_content,
        };
        let snapshot_content = CanonicalAccountAuthoritySnapshotContentV1 {
            account_id: root.verifying_key().to_bytes(),
            revision,
            revocations: Vec::new(),
        };
        let snapshot = CanonicalSignedAccountAuthoritySnapshotV1 {
            signature: root
                .sign(&account_authority_snapshot_signing_bytes_v1(
                    &snapshot_content,
                )?)
                .to_bytes(),
            content: snapshot_content,
        };
        let content = CanonicalAccountDeviceListContentV1 {
            authority_snapshot: snapshot,
            devices: vec![certificate],
        };
        Ok(CanonicalSignedAccountDeviceListV1 {
            signature: root
                .sign(&account_device_list_signing_bytes_v1(&content)?)
                .to_bytes(),
            content,
        })
    }

    fn objects() -> TestResult<(
        CanonicalSignedPublicationConflictResolutionRequestV1,
        CanonicalRootSignedPublicationConflictResolutionV1,
        CanonicalPublicationConflictResolutionResponseV1,
        CanonicalPublicationConflictClaimV1,
        CanonicalPublicationConflictClaimV1,
    )> {
        let vectors = vectors()?;
        let root = SigningKey::from_bytes(&fixed(&vectors.input.local_account_seed_hex)?);
        let requester = SigningKey::from_bytes(&fixed(&vectors.input.detector_seed_hex)?);
        let old_write = SigningKey::from_bytes(&fixed(&vectors.input.old_write_seed_hex)?);
        let new_write = SigningKey::from_bytes(&fixed(&vectors.input.new_write_seed_hex)?);
        assert_eq!(
            old_write.verifying_key().to_bytes(),
            fixed(&vectors.expected.public_keys.old_write_key_hex)?
        );
        assert_eq!(
            new_write.verifying_key().to_bytes(),
            fixed(&vectors.expected.public_keys.new_write_key_hex)?
        );
        let proof = decode_and_verify_signed_publication_conflict_proof_v1(&decode_hex(
            &vectors.expected.conflict_proof.signed_cbor_hex,
        )?)?;
        let request = sign_publication_conflict_resolution_request_v1(
            &requester,
            root.verifying_key().to_bytes(),
            vectors.input.authority_revision,
            proof,
            old_write.verifying_key().to_bytes(),
            new_write.verifying_key().to_bytes(),
            vectors.input.old_channel_epoch,
            vectors.input.new_channel_epoch,
            CanonicalPublicationConflictRoutePolicyV1::from_code(vectors.input.route_policy)?,
            vectors.input.replacement_ticket.as_bytes().to_vec(),
            vectors.input.requested_at_unix_seconds,
        )?;
        let list = authority_list(&root, &requester, vectors.input.authority_revision)?;
        let resolution = sign_publication_conflict_resolution_v1(
            &root,
            &request,
            &list,
            vectors.input.authorized_at_unix_seconds,
        )?;
        let response =
            create_publication_conflict_resolution_response_v1(&request, resolution.clone())?;
        let request_claim = publication_conflict_claim_for_request_v1(&request)?;
        let response_claim = publication_conflict_claim_for_response_v1(&response)?;
        Ok((request, resolution, response, request_claim, response_claim))
    }

    #[test]
    fn request_resolution_response_and_claims_match_golden_vectors() -> TestResult {
        let vectors = vectors()?;
        let (request, resolution, response, request_claim, response_claim) = objects()?;
        let expected_request = &vectors.expected.resolution_request;
        let request_content =
            encode_publication_conflict_resolution_request_content_v1(&request.content)?;
        assert_eq!(hex(&request_content), expected_request.content_cbor_hex);
        let request_preimage =
            publication_conflict_resolution_request_signing_bytes_v1(&request.content)?;
        assert_eq!(
            hex(&request_preimage),
            expected_request.signing_preimage_hex
        );
        assert_eq!(hex(&request.signature), expected_request.signature_hex);
        let request_bytes = encode_signed_publication_conflict_resolution_request_v1(&request)?;
        assert_eq!(hex(&request_bytes), expected_request.signed_cbor_hex);
        assert_eq!(
            hex(&publication_conflict_resolution_request_id_v1(&request)?),
            expected_request
                .request_id_hex
                .as_deref()
                .ok_or("request ID vector missing")?
        );

        let expected_resolution = &vectors.expected.resolution;
        let resolution_content =
            encode_publication_conflict_resolution_content_v1(&resolution.content)?;
        assert_eq!(
            hex(&resolution_content),
            expected_resolution.content_cbor_hex
        );
        let resolution_preimage =
            publication_conflict_resolution_signing_bytes_v1(&resolution.content)?;
        assert_eq!(
            hex(&resolution_preimage),
            expected_resolution.signing_preimage_hex
        );
        assert_eq!(
            hex(&resolution.signature),
            expected_resolution.signature_hex
        );
        let resolution_bytes = encode_root_signed_publication_conflict_resolution_v1(&resolution)?;
        assert_eq!(hex(&resolution_bytes), expected_resolution.signed_cbor_hex);
        assert_eq!(
            hex(&publication_conflict_resolution_id_v1(&resolution)?),
            expected_resolution
                .resolution_id_hex
                .as_deref()
                .ok_or("resolution ID vector missing")?
        );
        assert_eq!(
            hex(&resolution.content.replacement_ticket_digest),
            expected_resolution
                .replacement_ticket_digest_hex
                .as_deref()
                .ok_or("replacement digest vector missing")?
        );
        let response_bytes = encode_publication_conflict_resolution_response_v1(&response)?;
        assert_eq!(
            hex(&response_bytes),
            vectors.expected.resolution_response.cbor_hex
        );

        for (actual, expected) in [
            (&request_claim, &vectors.expected.request_claim),
            (&response_claim, &vectors.expected.response_claim),
        ] {
            assert_eq!(hex(&actual.artifact_id), expected.artifact_id_hex);
            assert_eq!(
                hex(&actual.confirmation_code),
                expected.confirmation_code_hex
            );
            assert_eq!(
                format_publication_conflict_confirmation_code_v1(&actual.confirmation_code),
                expected.confirmation_code
            );
            let claim_bytes = encode_publication_conflict_claim_v1(actual)?;
            assert_eq!(hex(&claim_bytes), expected.cbor_hex);
            assert_eq!(
                encode_publication_conflict_claim_uri_v1(actual)?,
                expected.uri
            );
            assert_eq!(
                decode_and_verify_publication_conflict_claim_uri_v1(&expected.uri)?,
                *actual
            );
        }
        assert_eq!(
            request_claim.confirmation_code,
            response_claim.confirmation_code
        );
        assert_ne!(request_claim.artifact_id, response_claim.artifact_id);
        Ok(())
    }

    #[test]
    fn authority_substitution_and_cross_artifact_claims_fail_closed() -> TestResult {
        let vectors = vectors()?;
        let (request, resolution, response, request_claim, response_claim) = objects()?;
        let root = SigningKey::from_bytes(&fixed(&vectors.input.local_account_seed_hex)?);
        let requester = SigningKey::from_bytes(&fixed(&vectors.input.detector_seed_hex)?);
        let wrong_device = SigningKey::from_bytes(&[0xee; 32]);
        let wrong_list = authority_list(&root, &wrong_device, vectors.input.authority_revision)?;
        assert!(
            verify_publication_conflict_resolution_request_authority_v1(&request, &wrong_list)
                .is_err()
        );

        let mut changed_resolution = resolution.clone();
        changed_resolution.content.new_channel_epoch += 1;
        assert!(
            verify_publication_conflict_resolution_for_request_v1(&changed_resolution, &request)
                .is_err()
        );
        let mut changed_response = response.clone();
        changed_response.replacement_ticket.push(b'!');
        assert!(verify_publication_conflict_resolution_response_v1(&changed_response).is_err());
        assert!(
            verify_publication_conflict_claim_for_request_v1(&response_claim, &request).is_err()
        );
        assert!(
            verify_publication_conflict_claim_for_response_v1(&request_claim, &response).is_err()
        );

        let list = authority_list(&root, &requester, vectors.input.authority_revision)?;
        assert!(
            sign_publication_conflict_resolution_v1(
                &SigningKey::from_bytes(&[0xef; 32]),
                &request,
                &list,
                vectors.input.authorized_at_unix_seconds,
            )
            .is_err()
        );
        Ok(())
    }
}
