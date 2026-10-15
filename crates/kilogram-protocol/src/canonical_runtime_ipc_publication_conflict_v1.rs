//! Inactive deterministic-CBOR publication-conflict boundary for local runtime IPC.
//!
//! The portable conflict request and Root-signed response intentionally contain
//! publication write capabilities and replacement-ticket plaintext. They are
//! therefore kept in a trusted, content-addressed staging area. Only bounded
//! artifact references and compact public verification claims cross this IPC
//! boundary. Local paths, artifact plaintext, publication write keys, and the
//! Account Root secret are never encoded here.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::{
    canonical_account_v1::{AccountDecoder, signed_envelope},
    canonical_publication_resolution_v1::{
        CanonicalPublicationConflictClaimKindV1, CanonicalPublicationConflictClaimV1,
        MAX_CANONICAL_PUBLICATION_CONFLICT_REPLACEMENT_TICKET_BYTES,
        MAX_CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_BYTES,
        MAX_CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_RESPONSE_BYTES,
        decode_and_verify_publication_conflict_claim_v1, encode_publication_conflict_claim_v1,
        verify_publication_conflict_claim_v1,
    },
    canonical_runtime_ipc_v1::{
        CANONICAL_RUNTIME_IPC_CLOCK_SKEW_SECONDS, CanonicalRuntimeIpcError,
        CanonicalSignedRuntimeIpcDescriptorV1, MAX_CANONICAL_RUNTIME_IPC_ERROR_BYTES,
        MAX_CANONICAL_RUNTIME_IPC_REQUEST_VALIDITY_SECONDS, runtime_ipc_descriptor_id_v1,
        verify_runtime_ipc_descriptor_at_v1, verify_signed_runtime_ipc_descriptor_v1,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, prefixed, push_array_header, push_bytes, push_unsigned,
        validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_CONFLICT_REQUEST_BYTES: usize = 128 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_CONFLICT_RESPONSE_BYTES: usize = 128 * 1024;

pub const CANONICAL_RUNTIME_IPC_PUBLICATION_CONFLICT_ARTIFACT_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-publication-conflict-artifact-digest:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_PUBLICATION_CONFLICT_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-publication-conflict-request-digest:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_PUBLICATION_CONFLICT_RESPONSE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-publication-conflict-response-signature:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_PUBLICATION_CONFLICT_RESPONSE_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-publication-conflict-response-id:cbor-v1\0";

const COMMAND_CREATE_REQUEST: u64 = 0;
const COMMAND_APPLY_RESPONSE: u64 = 1;
const RESULT_REQUEST_CREATED: u64 = 0;
const RESULT_RESPONSE_APPLIED: u64 = 1;
const RESULT_ERROR: u64 = 2;
const OUTCOME_COMMITTED: u64 = 0;
const OUTCOME_ALREADY_COMMITTED: u64 = 1;
const OPTION_NONE: u64 = 0;
const OPTION_SOME: u64 = 1;
const ARTIFACT_STAGED: u64 = 0;
const PROOF_RETAINED: u64 = 0;
const ROOT_AUTHORIZATION_PENDING: u64 = 0;
const ROOT_AUTHORIZATION_VERIFIED: u64 = 1;
const ENDPOINT_ATOMICALLY_REPLACED: u64 = 0;
const RESOLUTION_RETAINED: u64 = 0;
const QUARANTINE_RELEASED: u64 = 0;
const SIBLING_PROPAGATION_PENDING: u64 = 0;
const RESTART_NOT_REQUIRED: u64 = 0;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcPublicationConflictPendingRequestV1 {
    pub claim: CanonicalPublicationConflictClaimV1,
    pub requester_device_id: [u8; 32],
    pub requested_at_unix_seconds: u64,
    pub request_artifact_bytes: u64,
    pub request_artifact_digest: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcPublicationConflictCurrentV1 {
    pub authority_revision: u64,
    pub device_list_digest: [u8; 32],
    pub conflict_proof_id: [u8; 32],
    pub conflict_evidence_id: [u8; 32],
    pub peer_account_id: [u8; 32],
    pub peer_device_id: [u8; 32],
    pub old_publication_channel_id: [u8; 32],
    pub old_channel_epoch: u64,
    pub endpoint_ticket_bytes: u64,
    pub endpoint_ticket_artifact_digest: [u8; 32],
    pub pending_request: Option<Box<CanonicalRuntimeIpcPublicationConflictPendingRequestV1>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationConflictCommandV1 {
    CreatePublicationConflictRequest {
        expected_current: Box<CanonicalRuntimeIpcPublicationConflictCurrentV1>,
        staged_replacement_ticket_bytes: u64,
        staged_replacement_ticket_artifact_digest: [u8; 32],
    },
    ApplyPublicationConflictResponse {
        expected_current: Box<CanonicalRuntimeIpcPublicationConflictCurrentV1>,
        staged_response_bytes: u64,
        staged_response_artifact_digest: [u8; 32],
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcPublicationConflictRequestV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub bearer_token: [u8; 32],
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub command: CanonicalRuntimeIpcPublicationConflictCommandV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationConflictOutcomeV1 {
    Committed,
    AlreadyCommitted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationConflictArtifactStateV1 {
    StagedForOfflineRootExport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationConflictProofStateV1 {
    Retained,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationConflictRootStateV1 {
    AwaitingOfflineAuthorization,
    VerifiedExactCurrentAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationConflictEndpointStateV1 {
    AtomicallyReplaced,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationConflictResolutionStateV1 {
    Retained,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationConflictQuarantineStateV1 {
    Released,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationConflictSiblingStateV1 {
    PendingPropagation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationConflictRestartStateV1 {
    NotRequired,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcPublicationConflictRequestCreatedV1 {
    pub outcome: CanonicalRuntimeIpcPublicationConflictOutcomeV1,
    pub request_claim: CanonicalPublicationConflictClaimV1,
    pub conflict_proof_id: [u8; 32],
    pub requester_device_id: [u8; 32],
    pub requested_at_unix_seconds: u64,
    pub request_artifact_bytes: u64,
    pub request_artifact_digest: [u8; 32],
    pub replacement_ticket_artifact_bytes: u64,
    pub replacement_ticket_artifact_digest: [u8; 32],
    pub artifact_state: CanonicalRuntimeIpcPublicationConflictArtifactStateV1,
    pub proof_state: CanonicalRuntimeIpcPublicationConflictProofStateV1,
    pub root_state: CanonicalRuntimeIpcPublicationConflictRootStateV1,
    pub restart_state: CanonicalRuntimeIpcPublicationConflictRestartStateV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcPublicationConflictResolutionAppliedV1 {
    pub outcome: CanonicalRuntimeIpcPublicationConflictOutcomeV1,
    pub response_claim: CanonicalPublicationConflictClaimV1,
    pub conflict_proof_id: [u8; 32],
    pub authorized_at_unix_seconds: u64,
    pub response_artifact_bytes: u64,
    pub response_artifact_digest: [u8; 32],
    pub root_state: CanonicalRuntimeIpcPublicationConflictRootStateV1,
    pub endpoint_state: CanonicalRuntimeIpcPublicationConflictEndpointStateV1,
    pub resolution_state: CanonicalRuntimeIpcPublicationConflictResolutionStateV1,
    pub proof_state: CanonicalRuntimeIpcPublicationConflictProofStateV1,
    pub quarantine_state: CanonicalRuntimeIpcPublicationConflictQuarantineStateV1,
    pub sibling_state: CanonicalRuntimeIpcPublicationConflictSiblingStateV1,
    pub restart_state: CanonicalRuntimeIpcPublicationConflictRestartStateV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationConflictResultV1 {
    PublicationConflictRequestCreated(Box<CanonicalRuntimeIpcPublicationConflictRequestCreatedV1>),
    PublicationConflictResponseApplied(
        Box<CanonicalRuntimeIpcPublicationConflictResolutionAppliedV1>,
    ),
    Error {
        code: u16,
        message_utf8: Vec<u8>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcPublicationConflictResponseContentV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub request_digest: [u8; 32],
    pub responder_device_id: [u8; 32],
    pub result: CanonicalRuntimeIpcPublicationConflictResultV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeIpcPublicationConflictResponseV1 {
    pub content: CanonicalRuntimeIpcPublicationConflictResponseContentV1,
    pub signature: [u8; 64],
}

pub fn runtime_ipc_publication_conflict_artifact_digest_v1(bytes: &[u8]) -> [u8; 32] {
    domain_hash(
        CANONICAL_RUNTIME_IPC_PUBLICATION_CONFLICT_ARTIFACT_DIGEST_DOMAIN,
        bytes,
    )
}

pub fn encode_runtime_ipc_publication_conflict_request_v1(
    request: &CanonicalRuntimeIpcPublicationConflictRequestV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_request(request)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 7);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &request.descriptor_id);
    push_bytes(&mut output, &request.request_id);
    push_bytes(&mut output, &request.bearer_token);
    push_unsigned(&mut output, request.issued_at_unix_seconds);
    push_unsigned(&mut output, request.expires_at_unix_seconds);
    encode_command(&mut output, &request.command)?;
    ensure_bound(
        &output,
        "publication conflict request",
        MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_CONFLICT_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_runtime_ipc_publication_conflict_request_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcPublicationConflictRequestV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "publication conflict request",
        MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_CONFLICT_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("runtime-ipc-publication-conflict-request-v1", 7)?;
    decoder.expect_version("runtime-ipc-publication-conflict-request-v1.schema-version")?;
    let request = CanonicalRuntimeIpcPublicationConflictRequestV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-publication-conflict-request-v1.descriptor-id")?,
        request_id: decoder
            .fixed_bytes("runtime-ipc-publication-conflict-request-v1.request-id")?,
        bearer_token: decoder
            .fixed_bytes("runtime-ipc-publication-conflict-request-v1.bearer-token")?,
        issued_at_unix_seconds: decoder
            .unsigned("runtime-ipc-publication-conflict-request-v1.issued-at-unix-seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("runtime-ipc-publication-conflict-request-v1.expires-at-unix-seconds")?,
        command: decode_command(&mut decoder)?,
    };
    decoder.finish()?;
    validate_request(&request)?;
    if encode_runtime_ipc_publication_conflict_request_v1(&request)? != bytes {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    Ok(request)
}

pub fn runtime_ipc_publication_conflict_request_digest_v1(
    request: &CanonicalRuntimeIpcPublicationConflictRequestV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_PUBLICATION_CONFLICT_REQUEST_DIGEST_DOMAIN,
        &encode_runtime_ipc_publication_conflict_request_v1(request)?,
    ))
}

pub fn verify_runtime_ipc_publication_conflict_request_for_descriptor_v1(
    request: &CanonicalRuntimeIpcPublicationConflictRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_runtime_ipc_descriptor_at_v1(descriptor, now_unix_seconds)?;
    validate_request(request)?;
    if request.descriptor_id != runtime_ipc_descriptor_id_v1(descriptor)?
        || request.bearer_token != descriptor.content.bearer_token
        || request.issued_at_unix_seconds < descriptor.content.issued_at_unix_seconds
        || request.expires_at_unix_seconds > descriptor.content.expires_at_unix_seconds
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    if request.issued_at_unix_seconds
        > now_unix_seconds.saturating_add(CANONICAL_RUNTIME_IPC_CLOCK_SKEW_SECONDS)
        || request.expires_at_unix_seconds < now_unix_seconds
    {
        return Err(CanonicalRuntimeIpcError::RequestNotCurrent);
    }
    let current = request_current(request);
    if current.peer_account_id == descriptor.content.account_id {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    if let Some(pending) = &current.pending_request
        && (pending.claim.local_account_id != descriptor.content.account_id
            || pending.requester_device_id != descriptor.content.device_id)
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    Ok(())
}

pub fn runtime_ipc_publication_conflict_response_signing_bytes_v1(
    content: &CanonicalRuntimeIpcPublicationConflictResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    Ok(prefixed(
        CANONICAL_RUNTIME_IPC_PUBLICATION_CONFLICT_RESPONSE_SIGNATURE_DOMAIN,
        &encode_response_content(content)?,
    ))
}

pub fn sign_runtime_ipc_publication_conflict_response_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalRuntimeIpcPublicationConflictRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
    result: CanonicalRuntimeIpcPublicationConflictResultV1,
) -> Result<CanonicalSignedRuntimeIpcPublicationConflictResponseV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_publication_conflict_request_for_descriptor_v1(
        request,
        descriptor,
        now_unix_seconds,
    )?;
    if device_signing_key.verifying_key().to_bytes() != descriptor.content.device_id {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    verify_result_for_request(&result, request, descriptor)?;
    let content = CanonicalRuntimeIpcPublicationConflictResponseContentV1 {
        descriptor_id: request.descriptor_id,
        request_id: request.request_id,
        request_digest: runtime_ipc_publication_conflict_request_digest_v1(request)?,
        responder_device_id: descriptor.content.device_id,
        result,
    };
    let signature = device_signing_key
        .sign(&runtime_ipc_publication_conflict_response_signing_bytes_v1(
            &content,
        )?)
        .to_bytes();
    let response = CanonicalSignedRuntimeIpcPublicationConflictResponseV1 { content, signature };
    verify_runtime_ipc_publication_conflict_response_for_request_v1(
        &response, request, descriptor,
    )?;
    Ok(response)
}

pub fn encode_signed_runtime_ipc_publication_conflict_response_v1(
    response: &CanonicalSignedRuntimeIpcPublicationConflictResponseV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_publication_conflict_response_v1(response)?;
    let output = signed_envelope(
        &encode_response_content(&response.content)?,
        &response.signature,
    );
    ensure_bound(
        &output,
        "publication conflict response",
        MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_CONFLICT_RESPONSE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_runtime_ipc_publication_conflict_response_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeIpcPublicationConflictResponseV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "publication conflict response",
        MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_CONFLICT_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-runtime-ipc-publication-conflict-response-v1", 3)?;
    decoder.expect_version("signed-runtime-ipc-publication-conflict-response-v1.schema-version")?;
    let content = decode_response_content(&mut decoder)?;
    let signature =
        decoder.fixed_bytes("signed-runtime-ipc-publication-conflict-response-v1.signature")?;
    decoder.finish()?;
    let response = CanonicalSignedRuntimeIpcPublicationConflictResponseV1 { content, signature };
    verify_signed_runtime_ipc_publication_conflict_response_v1(&response)?;
    if encode_signed_runtime_ipc_publication_conflict_response_v1(&response)? != bytes {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(response)
}

pub fn verify_signed_runtime_ipc_publication_conflict_response_v1(
    response: &CanonicalSignedRuntimeIpcPublicationConflictResponseV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_response_content(&response.content)?;
    verify_ed25519(
        &response.content.responder_device_id,
        &runtime_ipc_publication_conflict_response_signing_bytes_v1(&response.content)?,
        &response.signature,
    )
}

pub fn verify_runtime_ipc_publication_conflict_response_for_request_v1(
    response: &CanonicalSignedRuntimeIpcPublicationConflictResponseV1,
    request: &CanonicalRuntimeIpcPublicationConflictRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_publication_conflict_response_v1(response)?;
    validate_request(request)?;
    verify_signed_runtime_ipc_descriptor_v1(descriptor)?;
    if response.content.descriptor_id != runtime_ipc_descriptor_id_v1(descriptor)?
        || response.content.descriptor_id != request.descriptor_id
        || response.content.request_id != request.request_id
        || response.content.request_digest
            != runtime_ipc_publication_conflict_request_digest_v1(request)?
        || response.content.responder_device_id != descriptor.content.device_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    verify_result_for_request(&response.content.result, request, descriptor)
}

pub fn runtime_ipc_publication_conflict_response_id_v1(
    response: &CanonicalSignedRuntimeIpcPublicationConflictResponseV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_PUBLICATION_CONFLICT_RESPONSE_ID_DOMAIN,
        &encode_signed_runtime_ipc_publication_conflict_response_v1(response)?,
    ))
}

pub fn encode_runtime_ipc_publication_conflict_current_v1(
    current: &CanonicalRuntimeIpcPublicationConflictCurrentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_current(current, false)?;
    let mut output = Vec::new();
    encode_current(&mut output, current)?;
    Ok(output)
}

pub fn decode_and_verify_runtime_ipc_publication_conflict_current_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcPublicationConflictCurrentV1, CanonicalRuntimeIpcError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    let current = decode_current(&mut decoder)?;
    decoder.finish()?;
    if encode_runtime_ipc_publication_conflict_current_v1(&current)? != bytes {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    Ok(current)
}

pub fn encode_runtime_ipc_publication_conflict_request_created_v1(
    created: &CanonicalRuntimeIpcPublicationConflictRequestCreatedV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_request_created(created)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 14);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_unsigned(&mut output, encode_outcome(created.outcome));
    push_bytes(
        &mut output,
        &encode_publication_conflict_claim_v1(&created.request_claim)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
    );
    push_bytes(&mut output, &created.conflict_proof_id);
    push_bytes(&mut output, &created.requester_device_id);
    push_unsigned(&mut output, created.requested_at_unix_seconds);
    push_unsigned(&mut output, created.request_artifact_bytes);
    push_bytes(&mut output, &created.request_artifact_digest);
    push_unsigned(&mut output, created.replacement_ticket_artifact_bytes);
    push_bytes(&mut output, &created.replacement_ticket_artifact_digest);
    push_unsigned(&mut output, encode_artifact_state(created.artifact_state));
    push_unsigned(&mut output, encode_proof_state(created.proof_state));
    push_unsigned(&mut output, encode_root_state(created.root_state));
    push_unsigned(&mut output, encode_restart_state(created.restart_state));
    Ok(output)
}

pub fn encode_runtime_ipc_publication_conflict_resolution_applied_v1(
    applied: &CanonicalRuntimeIpcPublicationConflictResolutionAppliedV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_resolution_applied(applied)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 14);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_unsigned(&mut output, encode_outcome(applied.outcome));
    push_bytes(
        &mut output,
        &encode_publication_conflict_claim_v1(&applied.response_claim)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
    );
    push_bytes(&mut output, &applied.conflict_proof_id);
    push_unsigned(&mut output, applied.authorized_at_unix_seconds);
    push_unsigned(&mut output, applied.response_artifact_bytes);
    push_bytes(&mut output, &applied.response_artifact_digest);
    push_unsigned(&mut output, encode_root_state(applied.root_state));
    push_unsigned(&mut output, encode_endpoint_state(applied.endpoint_state));
    push_unsigned(
        &mut output,
        encode_resolution_state(applied.resolution_state),
    );
    push_unsigned(&mut output, encode_proof_state(applied.proof_state));
    push_unsigned(
        &mut output,
        encode_quarantine_state(applied.quarantine_state),
    );
    push_unsigned(&mut output, encode_sibling_state(applied.sibling_state));
    push_unsigned(&mut output, encode_restart_state(applied.restart_state));
    Ok(output)
}

fn validate_request(
    request: &CanonicalRuntimeIpcPublicationConflictRequestV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    let validity = request
        .expires_at_unix_seconds
        .checked_sub(request.issued_at_unix_seconds)
        .ok_or(CanonicalRuntimeIpcError::InvalidRequest)?;
    if request.descriptor_id == [0; 32]
        || request.request_id == [0; 32]
        || request.bearer_token == [0; 32]
        || !(1..=MAX_CANONICAL_RUNTIME_IPC_REQUEST_VALIDITY_SECONDS).contains(&validity)
    {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    match &request.command {
        CanonicalRuntimeIpcPublicationConflictCommandV1::CreatePublicationConflictRequest {
            expected_current,
            staged_replacement_ticket_bytes,
            staged_replacement_ticket_artifact_digest,
        } => {
            validate_current(expected_current, false)?;
            if expected_current.pending_request.is_some()
                || !valid_artifact_reference(
                    *staged_replacement_ticket_bytes,
                    staged_replacement_ticket_artifact_digest,
                    MAX_CANONICAL_PUBLICATION_CONFLICT_REPLACEMENT_TICKET_BYTES,
                )
            {
                return Err(CanonicalRuntimeIpcError::InvalidRequest);
            }
        }
        CanonicalRuntimeIpcPublicationConflictCommandV1::ApplyPublicationConflictResponse {
            expected_current,
            staged_response_bytes,
            staged_response_artifact_digest,
        } => {
            validate_current(expected_current, false)?;
            if expected_current.pending_request.is_none()
                || !valid_artifact_reference(
                    *staged_response_bytes,
                    staged_response_artifact_digest,
                    MAX_CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_RESPONSE_BYTES,
                )
            {
                return Err(CanonicalRuntimeIpcError::InvalidRequest);
            }
        }
    }
    Ok(())
}

fn validate_current(
    current: &CanonicalRuntimeIpcPublicationConflictCurrentV1,
    response: bool,
) -> Result<(), CanonicalRuntimeIpcError> {
    let invalid = || invalid_for(response);
    if current.authority_revision == 0
        || current.device_list_digest == [0; 32]
        || current.conflict_proof_id == [0; 32]
        || current.conflict_evidence_id == [0; 32]
        || current.peer_account_id == [0; 32]
        || current.peer_device_id == [0; 32]
        || current.old_publication_channel_id == [0; 32]
        || !valid_artifact_reference(
            current.endpoint_ticket_bytes,
            &current.endpoint_ticket_artifact_digest,
            MAX_CANONICAL_PUBLICATION_CONFLICT_REPLACEMENT_TICKET_BYTES,
        )
    {
        return Err(invalid());
    }
    VerifyingKey::from_bytes(&current.peer_device_id)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidPublicKey)?;
    if let Some(pending) = &current.pending_request {
        validate_pending_request(pending, current, response)?;
    }
    Ok(())
}

fn validate_pending_request(
    pending: &CanonicalRuntimeIpcPublicationConflictPendingRequestV1,
    current: &CanonicalRuntimeIpcPublicationConflictCurrentV1,
    response: bool,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_publication_conflict_claim_v1(&pending.claim).map_err(|_| invalid_for(response))?;
    if pending.claim.kind != CanonicalPublicationConflictClaimKindV1::Request
        || pending.claim.artifact_id != pending.claim.request_id
        || pending.requester_device_id == [0; 32]
        || pending.requested_at_unix_seconds == 0
        || !valid_artifact_reference(
            pending.request_artifact_bytes,
            &pending.request_artifact_digest,
            MAX_CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_BYTES,
        )
        || !claim_matches_current(&pending.claim, current)
    {
        return Err(invalid_for(response));
    }
    VerifyingKey::from_bytes(&pending.requester_device_id)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidPublicKey)?;
    Ok(())
}

fn validate_response_content(
    content: &CanonicalRuntimeIpcPublicationConflictResponseContentV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if content.descriptor_id == [0; 32]
        || content.request_id == [0; 32]
        || content.request_digest == [0; 32]
        || content.responder_device_id == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    match &content.result {
        CanonicalRuntimeIpcPublicationConflictResultV1::PublicationConflictRequestCreated(
            created,
        ) => validate_request_created(created),
        CanonicalRuntimeIpcPublicationConflictResultV1::PublicationConflictResponseApplied(
            applied,
        ) => validate_resolution_applied(applied),
        CanonicalRuntimeIpcPublicationConflictResultV1::Error { code, message_utf8 } => {
            if *code == 0
                || message_utf8.is_empty()
                || message_utf8.len() > MAX_CANONICAL_RUNTIME_IPC_ERROR_BYTES
                || std::str::from_utf8(message_utf8).is_err()
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            Ok(())
        }
    }
}

fn validate_request_created(
    created: &CanonicalRuntimeIpcPublicationConflictRequestCreatedV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_publication_conflict_claim_v1(&created.request_claim)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    if created.request_claim.kind != CanonicalPublicationConflictClaimKindV1::Request
        || created.request_claim.artifact_id != created.request_claim.request_id
        || created.conflict_proof_id == [0; 32]
        || created.requester_device_id == [0; 32]
        || created.requested_at_unix_seconds == 0
        || !valid_artifact_reference(
            created.request_artifact_bytes,
            &created.request_artifact_digest,
            MAX_CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_BYTES,
        )
        || !valid_artifact_reference(
            created.replacement_ticket_artifact_bytes,
            &created.replacement_ticket_artifact_digest,
            MAX_CANONICAL_PUBLICATION_CONFLICT_REPLACEMENT_TICKET_BYTES,
        )
        || created.root_state
            != CanonicalRuntimeIpcPublicationConflictRootStateV1::AwaitingOfflineAuthorization
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    VerifyingKey::from_bytes(&created.requester_device_id)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidPublicKey)?;
    Ok(())
}

fn validate_resolution_applied(
    applied: &CanonicalRuntimeIpcPublicationConflictResolutionAppliedV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_publication_conflict_claim_v1(&applied.response_claim)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    if applied.response_claim.kind != CanonicalPublicationConflictClaimKindV1::Response
        || applied.conflict_proof_id == [0; 32]
        || applied.authorized_at_unix_seconds == 0
        || !valid_artifact_reference(
            applied.response_artifact_bytes,
            &applied.response_artifact_digest,
            MAX_CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_RESPONSE_BYTES,
        )
        || applied.root_state
            != CanonicalRuntimeIpcPublicationConflictRootStateV1::VerifiedExactCurrentAuthority
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn verify_result_for_request(
    result: &CanonicalRuntimeIpcPublicationConflictResultV1,
    request: &CanonicalRuntimeIpcPublicationConflictRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match (result, &request.command) {
        (
            CanonicalRuntimeIpcPublicationConflictResultV1::PublicationConflictRequestCreated(
                created,
            ),
            CanonicalRuntimeIpcPublicationConflictCommandV1::CreatePublicationConflictRequest {
                expected_current,
                staged_replacement_ticket_bytes,
                staged_replacement_ticket_artifact_digest,
            },
        ) => {
            validate_request_created(created)?;
            if created.request_claim.local_account_id != descriptor.content.account_id
                || created.requester_device_id != descriptor.content.device_id
                || created.requested_at_unix_seconds != request.issued_at_unix_seconds
                || created.conflict_proof_id != expected_current.conflict_proof_id
                || !claim_matches_current(&created.request_claim, expected_current)
                || created.replacement_ticket_artifact_bytes != *staged_replacement_ticket_bytes
                || created.replacement_ticket_artifact_digest
                    != *staged_replacement_ticket_artifact_digest
            {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            Ok(())
        }
        (
            CanonicalRuntimeIpcPublicationConflictResultV1::PublicationConflictResponseApplied(
                applied,
            ),
            CanonicalRuntimeIpcPublicationConflictCommandV1::ApplyPublicationConflictResponse {
                expected_current,
                staged_response_bytes,
                staged_response_artifact_digest,
            },
        ) => {
            validate_resolution_applied(applied)?;
            let pending = expected_current
                .pending_request
                .as_ref()
                .ok_or(CanonicalRuntimeIpcError::ContextMismatch)?;
            if applied.response_claim.local_account_id != descriptor.content.account_id
                || applied.conflict_proof_id != expected_current.conflict_proof_id
                || applied.authorized_at_unix_seconds < pending.requested_at_unix_seconds
                || applied.response_artifact_bytes != *staged_response_bytes
                || applied.response_artifact_digest != *staged_response_artifact_digest
                || !claims_share_ceremony(&pending.claim, &applied.response_claim)
            {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            Ok(())
        }
        (CanonicalRuntimeIpcPublicationConflictResultV1::Error { .. }, _) => Ok(()),
        _ => Err(CanonicalRuntimeIpcError::ContextMismatch),
    }
}

fn claim_matches_current(
    claim: &CanonicalPublicationConflictClaimV1,
    current: &CanonicalRuntimeIpcPublicationConflictCurrentV1,
) -> bool {
    claim.authority_revision == current.authority_revision
        && claim.conflict_evidence_id == current.conflict_evidence_id
        && claim.peer_account_id == current.peer_account_id
        && claim.peer_device_id == current.peer_device_id
        && claim.old_publication_channel_id == current.old_publication_channel_id
        && claim.old_channel_epoch == current.old_channel_epoch
}

fn claims_share_ceremony(
    request: &CanonicalPublicationConflictClaimV1,
    response: &CanonicalPublicationConflictClaimV1,
) -> bool {
    request.kind == CanonicalPublicationConflictClaimKindV1::Request
        && response.kind == CanonicalPublicationConflictClaimKindV1::Response
        && request.request_id == response.request_id
        && request.local_account_id == response.local_account_id
        && request.authority_revision == response.authority_revision
        && request.conflict_evidence_id == response.conflict_evidence_id
        && request.peer_account_id == response.peer_account_id
        && request.peer_device_id == response.peer_device_id
        && request.old_publication_channel_id == response.old_publication_channel_id
        && request.new_publication_channel_id == response.new_publication_channel_id
        && request.old_channel_epoch == response.old_channel_epoch
        && request.new_channel_epoch == response.new_channel_epoch
        && request.route_policy == response.route_policy
        && request.replacement_ticket_digest == response.replacement_ticket_digest
        && request.confirmation_code == response.confirmation_code
}

fn valid_artifact_reference(bytes: u64, digest: &[u8; 32], maximum: usize) -> bool {
    bytes > 0 && bytes <= maximum as u64 && *digest != [0; 32]
}

fn invalid_for(response: bool) -> CanonicalRuntimeIpcError {
    if response {
        CanonicalRuntimeIpcError::InvalidResponse
    } else {
        CanonicalRuntimeIpcError::InvalidRequest
    }
}

fn request_current(
    request: &CanonicalRuntimeIpcPublicationConflictRequestV1,
) -> &CanonicalRuntimeIpcPublicationConflictCurrentV1 {
    match &request.command {
        CanonicalRuntimeIpcPublicationConflictCommandV1::CreatePublicationConflictRequest {
            expected_current,
            ..
        }
        | CanonicalRuntimeIpcPublicationConflictCommandV1::ApplyPublicationConflictResponse {
            expected_current,
            ..
        } => expected_current,
    }
}

fn encode_command(
    output: &mut Vec<u8>,
    command: &CanonicalRuntimeIpcPublicationConflictCommandV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match command {
        CanonicalRuntimeIpcPublicationConflictCommandV1::CreatePublicationConflictRequest {
            expected_current,
            staged_replacement_ticket_bytes,
            staged_replacement_ticket_artifact_digest,
        } => {
            push_array_header(output, 4);
            push_unsigned(output, COMMAND_CREATE_REQUEST);
            push_bytes(
                output,
                &encode_runtime_ipc_publication_conflict_current_v1(expected_current)?,
            );
            push_unsigned(output, *staged_replacement_ticket_bytes);
            push_bytes(output, staged_replacement_ticket_artifact_digest);
        }
        CanonicalRuntimeIpcPublicationConflictCommandV1::ApplyPublicationConflictResponse {
            expected_current,
            staged_response_bytes,
            staged_response_artifact_digest,
        } => {
            push_array_header(output, 4);
            push_unsigned(output, COMMAND_APPLY_RESPONSE);
            push_bytes(
                output,
                &encode_runtime_ipc_publication_conflict_current_v1(expected_current)?,
            );
            push_unsigned(output, *staged_response_bytes);
            push_bytes(output, staged_response_artifact_digest);
        }
    }
    Ok(())
}

fn decode_command(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcPublicationConflictCommandV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-publication-conflict-command-v1")?;
    let variant = decoder.unsigned("runtime-ipc-publication-conflict-command-v1.variant")?;
    if length != 4 {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    let expected_current = Box::new(
        decode_and_verify_runtime_ipc_publication_conflict_current_v1(
            decoder.bytes("runtime-ipc-publication-conflict-command-v1.expected-current")?,
        )?,
    );
    let artifact_bytes =
        decoder.unsigned("runtime-ipc-publication-conflict-command-v1.artifact-bytes")?;
    let artifact_digest =
        decoder.fixed_bytes("runtime-ipc-publication-conflict-command-v1.artifact-digest")?;
    match variant {
        COMMAND_CREATE_REQUEST => Ok(
            CanonicalRuntimeIpcPublicationConflictCommandV1::CreatePublicationConflictRequest {
                expected_current,
                staged_replacement_ticket_bytes: artifact_bytes,
                staged_replacement_ticket_artifact_digest: artifact_digest,
            },
        ),
        COMMAND_APPLY_RESPONSE => Ok(
            CanonicalRuntimeIpcPublicationConflictCommandV1::ApplyPublicationConflictResponse {
                expected_current,
                staged_response_bytes: artifact_bytes,
                staged_response_artifact_digest: artifact_digest,
            },
        ),
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_current(
    output: &mut Vec<u8>,
    current: &CanonicalRuntimeIpcPublicationConflictCurrentV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    push_array_header(output, 12);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_unsigned(output, current.authority_revision);
    push_bytes(output, &current.device_list_digest);
    push_bytes(output, &current.conflict_proof_id);
    push_bytes(output, &current.conflict_evidence_id);
    push_bytes(output, &current.peer_account_id);
    push_bytes(output, &current.peer_device_id);
    push_bytes(output, &current.old_publication_channel_id);
    push_unsigned(output, current.old_channel_epoch);
    push_unsigned(output, current.endpoint_ticket_bytes);
    push_bytes(output, &current.endpoint_ticket_artifact_digest);
    encode_optional_pending(output, current.pending_request.as_deref())?;
    Ok(())
}

fn decode_current(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcPublicationConflictCurrentV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-publication-conflict-current-v1", 12)?;
    decoder.expect_version("runtime-ipc-publication-conflict-current-v1.schema-version")?;
    let current = CanonicalRuntimeIpcPublicationConflictCurrentV1 {
        authority_revision: decoder
            .unsigned("runtime-ipc-publication-conflict-current-v1.authority-revision")?,
        device_list_digest: decoder
            .fixed_bytes("runtime-ipc-publication-conflict-current-v1.device-list-digest")?,
        conflict_proof_id: decoder
            .fixed_bytes("runtime-ipc-publication-conflict-current-v1.conflict-proof-id")?,
        conflict_evidence_id: decoder
            .fixed_bytes("runtime-ipc-publication-conflict-current-v1.conflict-evidence-id")?,
        peer_account_id: decoder
            .fixed_bytes("runtime-ipc-publication-conflict-current-v1.peer-account-id")?,
        peer_device_id: decoder
            .fixed_bytes("runtime-ipc-publication-conflict-current-v1.peer-device-id")?,
        old_publication_channel_id: decoder.fixed_bytes(
            "runtime-ipc-publication-conflict-current-v1.old-publication-channel-id",
        )?,
        old_channel_epoch: decoder
            .unsigned("runtime-ipc-publication-conflict-current-v1.old-channel-epoch")?,
        endpoint_ticket_bytes: decoder
            .unsigned("runtime-ipc-publication-conflict-current-v1.endpoint-ticket-bytes")?,
        endpoint_ticket_artifact_digest: decoder.fixed_bytes(
            "runtime-ipc-publication-conflict-current-v1.endpoint-ticket-artifact-digest",
        )?,
        pending_request: decode_optional_pending(decoder)?,
    };
    validate_current(&current, false)?;
    Ok(current)
}

fn encode_optional_pending(
    output: &mut Vec<u8>,
    pending: Option<&CanonicalRuntimeIpcPublicationConflictPendingRequestV1>,
) -> Result<(), CanonicalRuntimeIpcError> {
    match pending {
        None => {
            push_array_header(output, 1);
            push_unsigned(output, OPTION_NONE);
        }
        Some(pending) => {
            push_array_header(output, 2);
            push_unsigned(output, OPTION_SOME);
            let mut nested = Vec::new();
            push_array_header(&mut nested, 6);
            push_unsigned(&mut nested, CANONICAL_PROFILE_VERSION);
            push_bytes(
                &mut nested,
                &encode_publication_conflict_claim_v1(&pending.claim)
                    .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?,
            );
            push_bytes(&mut nested, &pending.requester_device_id);
            push_unsigned(&mut nested, pending.requested_at_unix_seconds);
            push_unsigned(&mut nested, pending.request_artifact_bytes);
            push_bytes(&mut nested, &pending.request_artifact_digest);
            push_bytes(output, &nested);
        }
    }
    Ok(())
}

fn decode_optional_pending(
    decoder: &mut AccountDecoder<'_>,
) -> Result<
    Option<Box<CanonicalRuntimeIpcPublicationConflictPendingRequestV1>>,
    CanonicalRuntimeIpcError,
> {
    let length = decoder.array("runtime-ipc-publication-conflict-current-v1.pending-request")?;
    match (
        decoder.unsigned("runtime-ipc-publication-conflict-current-v1.pending-request.variant")?,
        length,
    ) {
        (OPTION_NONE, 1) => Ok(None),
        (OPTION_SOME, 2) => {
            let bytes = decoder
                .bytes("runtime-ipc-publication-conflict-current-v1.pending-request.value")?;
            validate_canonical_profile_v1(bytes)?;
            let mut nested = AccountDecoder::new(bytes);
            nested.expect_array("runtime-ipc-publication-conflict-pending-request-v1", 6)?;
            nested.expect_version(
                "runtime-ipc-publication-conflict-pending-request-v1.schema-version",
            )?;
            let pending = CanonicalRuntimeIpcPublicationConflictPendingRequestV1 {
                claim: decode_and_verify_publication_conflict_claim_v1(
                    nested.bytes("runtime-ipc-publication-conflict-pending-request-v1.claim")?,
                )
                .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?,
                requester_device_id: nested.fixed_bytes(
                    "runtime-ipc-publication-conflict-pending-request-v1.requester-device-id",
                )?,
                requested_at_unix_seconds: nested.unsigned(
                    "runtime-ipc-publication-conflict-pending-request-v1.requested-at-unix-seconds",
                )?,
                request_artifact_bytes: nested.unsigned(
                    "runtime-ipc-publication-conflict-pending-request-v1.request-artifact-bytes",
                )?,
                request_artifact_digest: nested.fixed_bytes(
                    "runtime-ipc-publication-conflict-pending-request-v1.request-artifact-digest",
                )?,
            };
            nested.finish()?;
            Ok(Some(Box::new(pending)))
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_response_content(
    content: &CanonicalRuntimeIpcPublicationConflictResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_response_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 6);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.descriptor_id);
    push_bytes(&mut output, &content.request_id);
    push_bytes(&mut output, &content.request_digest);
    push_bytes(&mut output, &content.responder_device_id);
    encode_result(&mut output, &content.result)?;
    Ok(output)
}

fn decode_response_content(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcPublicationConflictResponseContentV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-publication-conflict-response-content-v1", 6)?;
    decoder
        .expect_version("runtime-ipc-publication-conflict-response-content-v1.schema-version")?;
    let content = CanonicalRuntimeIpcPublicationConflictResponseContentV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-publication-conflict-response-content-v1.descriptor-id")?,
        request_id: decoder
            .fixed_bytes("runtime-ipc-publication-conflict-response-content-v1.request-id")?,
        request_digest: decoder
            .fixed_bytes("runtime-ipc-publication-conflict-response-content-v1.request-digest")?,
        responder_device_id: decoder.fixed_bytes(
            "runtime-ipc-publication-conflict-response-content-v1.responder-device-id",
        )?,
        result: decode_result(decoder)?,
    };
    validate_response_content(&content)?;
    Ok(content)
}

fn encode_result(
    output: &mut Vec<u8>,
    result: &CanonicalRuntimeIpcPublicationConflictResultV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match result {
        CanonicalRuntimeIpcPublicationConflictResultV1::PublicationConflictRequestCreated(
            created,
        ) => {
            push_array_header(output, 2);
            push_unsigned(output, RESULT_REQUEST_CREATED);
            push_bytes(
                output,
                &encode_runtime_ipc_publication_conflict_request_created_v1(created)?,
            );
        }
        CanonicalRuntimeIpcPublicationConflictResultV1::PublicationConflictResponseApplied(
            applied,
        ) => {
            push_array_header(output, 2);
            push_unsigned(output, RESULT_RESPONSE_APPLIED);
            push_bytes(
                output,
                &encode_runtime_ipc_publication_conflict_resolution_applied_v1(applied)?,
            );
        }
        CanonicalRuntimeIpcPublicationConflictResultV1::Error { code, message_utf8 } => {
            push_array_header(output, 3);
            push_unsigned(output, RESULT_ERROR);
            push_unsigned(output, u64::from(*code));
            push_bytes(output, message_utf8);
        }
    }
    Ok(())
}

fn decode_result(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcPublicationConflictResultV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-publication-conflict-result-v1")?;
    match (
        decoder.unsigned("runtime-ipc-publication-conflict-result-v1.variant")?,
        length,
    ) {
        (RESULT_REQUEST_CREATED, 2) => {
            let bytes =
                decoder.bytes("runtime-ipc-publication-conflict-result-v1.request-created")?;
            validate_canonical_profile_v1(bytes)?;
            let mut nested = AccountDecoder::new(bytes);
            let created = decode_request_created(&mut nested)?;
            nested.finish()?;
            if encode_runtime_ipc_publication_conflict_request_created_v1(&created)? != bytes {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            Ok(
                CanonicalRuntimeIpcPublicationConflictResultV1::PublicationConflictRequestCreated(
                    Box::new(created),
                ),
            )
        }
        (RESULT_RESPONSE_APPLIED, 2) => {
            let bytes =
                decoder.bytes("runtime-ipc-publication-conflict-result-v1.response-applied")?;
            validate_canonical_profile_v1(bytes)?;
            let mut nested = AccountDecoder::new(bytes);
            let applied = decode_resolution_applied(&mut nested)?;
            nested.finish()?;
            if encode_runtime_ipc_publication_conflict_resolution_applied_v1(&applied)? != bytes {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            Ok(
                CanonicalRuntimeIpcPublicationConflictResultV1::PublicationConflictResponseApplied(
                    Box::new(applied),
                ),
            )
        }
        (RESULT_ERROR, 3) => Ok(CanonicalRuntimeIpcPublicationConflictResultV1::Error {
            code: u16::try_from(
                decoder.unsigned("runtime-ipc-publication-conflict-result-v1.error-code")?,
            )
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            message_utf8: decoder
                .bytes("runtime-ipc-publication-conflict-result-v1.error-message-utf8")?
                .to_vec(),
        }),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn decode_request_created(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcPublicationConflictRequestCreatedV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-publication-conflict-request-created-v1", 14)?;
    decoder.expect_version("runtime-ipc-publication-conflict-request-created-v1.schema-version")?;
    let created = CanonicalRuntimeIpcPublicationConflictRequestCreatedV1 {
        outcome: decode_outcome(
            decoder.unsigned("runtime-ipc-publication-conflict-request-created-v1.outcome")?,
        )?,
        request_claim: decode_and_verify_publication_conflict_claim_v1(
            decoder.bytes("runtime-ipc-publication-conflict-request-created-v1.request-claim")?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
        conflict_proof_id: decoder.fixed_bytes(
            "runtime-ipc-publication-conflict-request-created-v1.conflict-proof-id",
        )?,
        requester_device_id: decoder.fixed_bytes(
            "runtime-ipc-publication-conflict-request-created-v1.requester-device-id",
        )?,
        requested_at_unix_seconds: decoder.unsigned(
            "runtime-ipc-publication-conflict-request-created-v1.requested-at-unix-seconds",
        )?,
        request_artifact_bytes: decoder.unsigned(
            "runtime-ipc-publication-conflict-request-created-v1.request-artifact-bytes",
        )?,
        request_artifact_digest: decoder.fixed_bytes(
            "runtime-ipc-publication-conflict-request-created-v1.request-artifact-digest",
        )?,
        replacement_ticket_artifact_bytes: decoder.unsigned(
            "runtime-ipc-publication-conflict-request-created-v1.replacement-ticket-artifact-bytes",
        )?,
        replacement_ticket_artifact_digest: decoder.fixed_bytes(
            "runtime-ipc-publication-conflict-request-created-v1.replacement-ticket-artifact-digest",
        )?,
        artifact_state: decode_artifact_state(
            decoder.unsigned("runtime-ipc-publication-conflict-request-created-v1.artifact-state")?,
        )?,
        proof_state: decode_proof_state(
            decoder.unsigned("runtime-ipc-publication-conflict-request-created-v1.proof-state")?,
        )?,
        root_state: decode_root_state(
            decoder.unsigned("runtime-ipc-publication-conflict-request-created-v1.root-state")?,
        )?,
        restart_state: decode_restart_state(
            decoder.unsigned("runtime-ipc-publication-conflict-request-created-v1.restart-state")?,
        )?,
    };
    validate_request_created(&created)?;
    Ok(created)
}

fn decode_resolution_applied(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcPublicationConflictResolutionAppliedV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-publication-conflict-resolution-applied-v1", 14)?;
    decoder
        .expect_version("runtime-ipc-publication-conflict-resolution-applied-v1.schema-version")?;
    let applied = CanonicalRuntimeIpcPublicationConflictResolutionAppliedV1 {
        outcome: decode_outcome(
            decoder.unsigned("runtime-ipc-publication-conflict-resolution-applied-v1.outcome")?,
        )?,
        response_claim: decode_and_verify_publication_conflict_claim_v1(
            decoder
                .bytes("runtime-ipc-publication-conflict-resolution-applied-v1.response-claim")?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
        conflict_proof_id: decoder.fixed_bytes(
            "runtime-ipc-publication-conflict-resolution-applied-v1.conflict-proof-id",
        )?,
        authorized_at_unix_seconds: decoder.unsigned(
            "runtime-ipc-publication-conflict-resolution-applied-v1.authorized-at-unix-seconds",
        )?,
        response_artifact_bytes: decoder.unsigned(
            "runtime-ipc-publication-conflict-resolution-applied-v1.response-artifact-bytes",
        )?,
        response_artifact_digest: decoder.fixed_bytes(
            "runtime-ipc-publication-conflict-resolution-applied-v1.response-artifact-digest",
        )?,
        root_state: decode_root_state(
            decoder
                .unsigned("runtime-ipc-publication-conflict-resolution-applied-v1.root-state")?,
        )?,
        endpoint_state: decode_endpoint_state(
            decoder.unsigned(
                "runtime-ipc-publication-conflict-resolution-applied-v1.endpoint-state",
            )?,
        )?,
        resolution_state: decode_resolution_state(decoder.unsigned(
            "runtime-ipc-publication-conflict-resolution-applied-v1.resolution-state",
        )?)?,
        proof_state: decode_proof_state(
            decoder
                .unsigned("runtime-ipc-publication-conflict-resolution-applied-v1.proof-state")?,
        )?,
        quarantine_state: decode_quarantine_state(decoder.unsigned(
            "runtime-ipc-publication-conflict-resolution-applied-v1.quarantine-state",
        )?)?,
        sibling_state: decode_sibling_state(
            decoder
                .unsigned("runtime-ipc-publication-conflict-resolution-applied-v1.sibling-state")?,
        )?,
        restart_state: decode_restart_state(
            decoder
                .unsigned("runtime-ipc-publication-conflict-resolution-applied-v1.restart-state")?,
        )?,
    };
    validate_resolution_applied(&applied)?;
    Ok(applied)
}

fn encode_outcome(value: CanonicalRuntimeIpcPublicationConflictOutcomeV1) -> u64 {
    match value {
        CanonicalRuntimeIpcPublicationConflictOutcomeV1::Committed => OUTCOME_COMMITTED,
        CanonicalRuntimeIpcPublicationConflictOutcomeV1::AlreadyCommitted => {
            OUTCOME_ALREADY_COMMITTED
        }
    }
}

fn decode_outcome(
    value: u64,
) -> Result<CanonicalRuntimeIpcPublicationConflictOutcomeV1, CanonicalRuntimeIpcError> {
    match value {
        OUTCOME_COMMITTED => Ok(CanonicalRuntimeIpcPublicationConflictOutcomeV1::Committed),
        OUTCOME_ALREADY_COMMITTED => {
            Ok(CanonicalRuntimeIpcPublicationConflictOutcomeV1::AlreadyCommitted)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

macro_rules! fixed_state_codec {
    ($encode:ident, $decode:ident, $ty:ty, $variant:path, $wire:expr) => {
        fn $encode(value: $ty) -> u64 {
            match value {
                $variant => $wire,
            }
        }
        fn $decode(value: u64) -> Result<$ty, CanonicalRuntimeIpcError> {
            match value {
                candidate if candidate == $wire => Ok($variant),
                _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
            }
        }
    };
}

fixed_state_codec!(
    encode_artifact_state,
    decode_artifact_state,
    CanonicalRuntimeIpcPublicationConflictArtifactStateV1,
    CanonicalRuntimeIpcPublicationConflictArtifactStateV1::StagedForOfflineRootExport,
    ARTIFACT_STAGED
);
fixed_state_codec!(
    encode_proof_state,
    decode_proof_state,
    CanonicalRuntimeIpcPublicationConflictProofStateV1,
    CanonicalRuntimeIpcPublicationConflictProofStateV1::Retained,
    PROOF_RETAINED
);
fixed_state_codec!(
    encode_endpoint_state,
    decode_endpoint_state,
    CanonicalRuntimeIpcPublicationConflictEndpointStateV1,
    CanonicalRuntimeIpcPublicationConflictEndpointStateV1::AtomicallyReplaced,
    ENDPOINT_ATOMICALLY_REPLACED
);
fixed_state_codec!(
    encode_resolution_state,
    decode_resolution_state,
    CanonicalRuntimeIpcPublicationConflictResolutionStateV1,
    CanonicalRuntimeIpcPublicationConflictResolutionStateV1::Retained,
    RESOLUTION_RETAINED
);
fixed_state_codec!(
    encode_quarantine_state,
    decode_quarantine_state,
    CanonicalRuntimeIpcPublicationConflictQuarantineStateV1,
    CanonicalRuntimeIpcPublicationConflictQuarantineStateV1::Released,
    QUARANTINE_RELEASED
);
fixed_state_codec!(
    encode_sibling_state,
    decode_sibling_state,
    CanonicalRuntimeIpcPublicationConflictSiblingStateV1,
    CanonicalRuntimeIpcPublicationConflictSiblingStateV1::PendingPropagation,
    SIBLING_PROPAGATION_PENDING
);
fixed_state_codec!(
    encode_restart_state,
    decode_restart_state,
    CanonicalRuntimeIpcPublicationConflictRestartStateV1,
    CanonicalRuntimeIpcPublicationConflictRestartStateV1::NotRequired,
    RESTART_NOT_REQUIRED
);

fn encode_root_state(value: CanonicalRuntimeIpcPublicationConflictRootStateV1) -> u64 {
    match value {
        CanonicalRuntimeIpcPublicationConflictRootStateV1::AwaitingOfflineAuthorization => {
            ROOT_AUTHORIZATION_PENDING
        }
        CanonicalRuntimeIpcPublicationConflictRootStateV1::VerifiedExactCurrentAuthority => {
            ROOT_AUTHORIZATION_VERIFIED
        }
    }
}

fn decode_root_state(
    value: u64,
) -> Result<CanonicalRuntimeIpcPublicationConflictRootStateV1, CanonicalRuntimeIpcError> {
    match value {
        ROOT_AUTHORIZATION_PENDING => {
            Ok(CanonicalRuntimeIpcPublicationConflictRootStateV1::AwaitingOfflineAuthorization)
        }
        ROOT_AUTHORIZATION_VERIFIED => {
            Ok(CanonicalRuntimeIpcPublicationConflictRootStateV1::VerifiedExactCurrentAuthority)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn ensure_bound(
    bytes: &[u8],
    object: &'static str,
    maximum: usize,
) -> Result<(), CanonicalRuntimeIpcError> {
    if bytes.len() > maximum {
        return Err(CanonicalRuntimeIpcError::ObjectTooLarge { object, maximum });
    }
    Ok(())
}

fn verify_ed25519(
    public_key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
) -> Result<(), CanonicalRuntimeIpcError> {
    let key = VerifyingKey::from_bytes(public_key)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidPublicKey)?;
    key.verify_strict(message, &Signature::from_bytes(signature))
        .map_err(|_| CanonicalRuntimeIpcError::InvalidSignature)
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut input = Vec::with_capacity(domain.len() + bytes.len());
    input.extend_from_slice(domain);
    input.extend_from_slice(bytes);
    *blake3::hash(&input).as_bytes()
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde_json::{Value, json};

    use super::*;
    use crate::{
        canonical_publication_resolution_v1::decode_and_verify_publication_conflict_claim_v1,
        canonical_runtime_ipc_v1::{
            CanonicalRuntimeIpcDescriptorContentV1, CanonicalRuntimeIpcLoopbackAddressV1,
            encode_signed_runtime_ipc_descriptor_v1, sign_runtime_ipc_descriptor_v1,
        },
    };

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    struct Fixture {
        descriptor: CanonicalSignedRuntimeIpcDescriptorV1,
        device_key: SigningKey,
        request_artifact: Vec<u8>,
        response_artifact: Vec<u8>,
        endpoint_ticket: Vec<u8>,
        replacement_ticket: Vec<u8>,
        create_request: CanonicalRuntimeIpcPublicationConflictRequestV1,
        create_result: CanonicalRuntimeIpcPublicationConflictRequestCreatedV1,
        apply_request: CanonicalRuntimeIpcPublicationConflictRequestV1,
        apply_result: CanonicalRuntimeIpcPublicationConflictResolutionAppliedV1,
    }

    fn conflict_vectors() -> TestResult<Value> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-PUBLICATION-CONFLICT-VECTORS.json"
        )))?)
    }

    fn vectors() -> TestResult<Value> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-RUNTIME-IPC-PUBLICATION-CONFLICT-VECTORS.json"
        )))?)
    }

    fn bytes(document: &Value, pointer: &str) -> TestResult<Vec<u8>> {
        let value = document
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("missing string at {pointer}"))?;
        if !value.len().is_multiple_of(2) {
            return Err(format!("odd hex length at {pointer}").into());
        }
        (0..value.len())
            .step_by(2)
            .map(|index| Ok(u8::from_str_radix(&value[index..index + 2], 16)?))
            .collect()
    }

    fn fixed<const N: usize>(document: &Value, pointer: &str) -> TestResult<[u8; N]> {
        Ok(bytes(document, pointer)?
            .try_into()
            .map_err(|_| format!("wrong byte length at {pointer}"))?)
    }

    fn text(document: &Value, pointer: &str) -> TestResult<Vec<u8>> {
        Ok(document
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("missing string at {pointer}"))?
            .as_bytes()
            .to_vec())
    }

    fn number(document: &Value, pointer: &str) -> TestResult<u64> {
        document
            .pointer(pointer)
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("missing unsigned integer at {pointer}").into())
    }

    fn fixture() -> TestResult<Fixture> {
        let conflict = conflict_vectors()?;
        let device_secret = fixed(&conflict, "/input/detector_seed_hex")?;
        let device_key = SigningKey::from_bytes(&device_secret);
        let local_account_id = fixed(&conflict, "/expected/public_keys/local_account_id_hex")?;
        let peer_account_id = fixed(&conflict, "/expected/public_keys/publisher_account_id_hex")?;
        let peer_device_id = fixed(&conflict, "/expected/public_keys/publisher_device_id_hex")?;
        let issued_at = number(&conflict, "/input/requested_at_unix_seconds")?;
        let descriptor = sign_runtime_ipc_descriptor_v1(
            &device_key,
            CanonicalRuntimeIpcDescriptorContentV1 {
                address: CanonicalRuntimeIpcLoopbackAddressV1::Ipv4([127, 0, 0, 1]),
                port: 49_639,
                bearer_token: [0xb6; 32],
                account_id: local_account_id,
                device_id: device_key.verifying_key().to_bytes(),
                server_instance_id: [0xb7; 32],
                issued_at_unix_seconds: issued_at - 10,
                expires_at_unix_seconds: issued_at + 3_600,
            },
        )?;
        let request_claim = decode_and_verify_publication_conflict_claim_v1(&bytes(
            &conflict,
            "/expected/request_claim/cbor_hex",
        )?)?;
        let response_claim = decode_and_verify_publication_conflict_claim_v1(&bytes(
            &conflict,
            "/expected/response_claim/cbor_hex",
        )?)?;
        let request_artifact = bytes(&conflict, "/expected/resolution_request/signed_cbor_hex")?;
        let response_artifact = bytes(&conflict, "/expected/resolution_response/cbor_hex")?;
        let endpoint_ticket = text(&conflict, "/input/first_ticket")?;
        let replacement_ticket = text(&conflict, "/input/replacement_ticket")?;
        let current = CanonicalRuntimeIpcPublicationConflictCurrentV1 {
            authority_revision: number(&conflict, "/input/authority_revision")?,
            device_list_digest: [0xb5; 32],
            conflict_proof_id: fixed(&conflict, "/expected/conflict_proof/proof_id_hex")?,
            conflict_evidence_id: fixed(&conflict, "/expected/conflict_proof/evidence_id_hex")?,
            peer_account_id,
            peer_device_id,
            old_publication_channel_id: fixed(
                &conflict,
                "/expected/public_keys/old_channel_id_hex",
            )?,
            old_channel_epoch: number(&conflict, "/input/old_channel_epoch")?,
            endpoint_ticket_bytes: u64::try_from(endpoint_ticket.len())?,
            endpoint_ticket_artifact_digest: runtime_ipc_publication_conflict_artifact_digest_v1(
                &endpoint_ticket,
            ),
            pending_request: None,
        };
        let replacement_digest =
            runtime_ipc_publication_conflict_artifact_digest_v1(&replacement_ticket);
        let create_request = CanonicalRuntimeIpcPublicationConflictRequestV1 {
            descriptor_id: runtime_ipc_descriptor_id_v1(&descriptor)?,
            request_id: [0xb8; 32],
            bearer_token: descriptor.content.bearer_token,
            issued_at_unix_seconds: issued_at,
            expires_at_unix_seconds: issued_at + 30,
            command:
                CanonicalRuntimeIpcPublicationConflictCommandV1::CreatePublicationConflictRequest {
                    expected_current: Box::new(current.clone()),
                    staged_replacement_ticket_bytes: u64::try_from(replacement_ticket.len())?,
                    staged_replacement_ticket_artifact_digest: replacement_digest,
                },
        };
        let request_artifact_digest =
            runtime_ipc_publication_conflict_artifact_digest_v1(&request_artifact);
        let create_result = CanonicalRuntimeIpcPublicationConflictRequestCreatedV1 {
            outcome: CanonicalRuntimeIpcPublicationConflictOutcomeV1::Committed,
            request_claim: request_claim.clone(),
            conflict_proof_id: current.conflict_proof_id,
            requester_device_id: descriptor.content.device_id,
            requested_at_unix_seconds: issued_at,
            request_artifact_bytes: u64::try_from(request_artifact.len())?,
            request_artifact_digest,
            replacement_ticket_artifact_bytes: u64::try_from(replacement_ticket.len())?,
            replacement_ticket_artifact_digest: replacement_digest,
            artifact_state:
                CanonicalRuntimeIpcPublicationConflictArtifactStateV1::StagedForOfflineRootExport,
            proof_state: CanonicalRuntimeIpcPublicationConflictProofStateV1::Retained,
            root_state:
                CanonicalRuntimeIpcPublicationConflictRootStateV1::AwaitingOfflineAuthorization,
            restart_state: CanonicalRuntimeIpcPublicationConflictRestartStateV1::NotRequired,
        };
        let mut apply_current = current;
        apply_current.pending_request = Some(Box::new(
            CanonicalRuntimeIpcPublicationConflictPendingRequestV1 {
                claim: request_claim,
                requester_device_id: descriptor.content.device_id,
                requested_at_unix_seconds: issued_at,
                request_artifact_bytes: u64::try_from(request_artifact.len())?,
                request_artifact_digest,
            },
        ));
        let authorized_at = number(&conflict, "/input/authorized_at_unix_seconds")?;
        let response_artifact_digest =
            runtime_ipc_publication_conflict_artifact_digest_v1(&response_artifact);
        let apply_request = CanonicalRuntimeIpcPublicationConflictRequestV1 {
            descriptor_id: runtime_ipc_descriptor_id_v1(&descriptor)?,
            request_id: [0xb9; 32],
            bearer_token: descriptor.content.bearer_token,
            issued_at_unix_seconds: authorized_at,
            expires_at_unix_seconds: authorized_at + 30,
            command:
                CanonicalRuntimeIpcPublicationConflictCommandV1::ApplyPublicationConflictResponse {
                    expected_current: Box::new(apply_current),
                    staged_response_bytes: u64::try_from(response_artifact.len())?,
                    staged_response_artifact_digest: response_artifact_digest,
                },
        };
        let apply_result = CanonicalRuntimeIpcPublicationConflictResolutionAppliedV1 {
            outcome: CanonicalRuntimeIpcPublicationConflictOutcomeV1::Committed,
            response_claim,
            conflict_proof_id: create_result.conflict_proof_id,
            authorized_at_unix_seconds: authorized_at,
            response_artifact_bytes: u64::try_from(response_artifact.len())?,
            response_artifact_digest,
            root_state:
                CanonicalRuntimeIpcPublicationConflictRootStateV1::VerifiedExactCurrentAuthority,
            endpoint_state:
                CanonicalRuntimeIpcPublicationConflictEndpointStateV1::AtomicallyReplaced,
            resolution_state: CanonicalRuntimeIpcPublicationConflictResolutionStateV1::Retained,
            proof_state: CanonicalRuntimeIpcPublicationConflictProofStateV1::Retained,
            quarantine_state: CanonicalRuntimeIpcPublicationConflictQuarantineStateV1::Released,
            sibling_state: CanonicalRuntimeIpcPublicationConflictSiblingStateV1::PendingPropagation,
            restart_state: CanonicalRuntimeIpcPublicationConflictRestartStateV1::NotRequired,
        };
        Ok(Fixture {
            descriptor,
            device_key,
            request_artifact,
            response_artifact,
            endpoint_ticket,
            replacement_ticket,
            create_request,
            create_result,
            apply_request,
            apply_result,
        })
    }

    fn create_response(
        fixture: &Fixture,
    ) -> Result<CanonicalSignedRuntimeIpcPublicationConflictResponseV1, CanonicalRuntimeIpcError>
    {
        sign_runtime_ipc_publication_conflict_response_v1(
            &fixture.device_key,
            &fixture.create_request,
            &fixture.descriptor,
            fixture.create_request.issued_at_unix_seconds,
            CanonicalRuntimeIpcPublicationConflictResultV1::PublicationConflictRequestCreated(
                Box::new(fixture.create_result.clone()),
            ),
        )
    }

    fn apply_response(
        fixture: &Fixture,
    ) -> Result<CanonicalSignedRuntimeIpcPublicationConflictResponseV1, CanonicalRuntimeIpcError>
    {
        sign_runtime_ipc_publication_conflict_response_v1(
            &fixture.device_key,
            &fixture.apply_request,
            &fixture.descriptor,
            fixture.apply_request.issued_at_unix_seconds,
            CanonicalRuntimeIpcPublicationConflictResultV1::PublicationConflictResponseApplied(
                Box::new(fixture.apply_result.clone()),
            ),
        )
    }

    #[test]
    fn canonical_runtime_ipc_publication_conflict_round_trips() -> TestResult {
        let fixture = fixture()?;
        for request in [&fixture.create_request, &fixture.apply_request] {
            verify_runtime_ipc_publication_conflict_request_for_descriptor_v1(
                request,
                &fixture.descriptor,
                request.issued_at_unix_seconds,
            )?;
            let encoded = encode_runtime_ipc_publication_conflict_request_v1(request)?;
            assert_eq!(
                decode_and_verify_runtime_ipc_publication_conflict_request_v1(&encoded)?,
                *request
            );
        }
        for response in [create_response(&fixture)?, apply_response(&fixture)?] {
            let encoded = encode_signed_runtime_ipc_publication_conflict_response_v1(&response)?;
            assert_eq!(
                decode_and_verify_signed_runtime_ipc_publication_conflict_response_v1(&encoded)?,
                response
            );
        }
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_publication_conflict_vectors_match() -> TestResult {
        let fixture = fixture()?;
        let vectors = vectors()?;
        let create_response = create_response(&fixture)?;
        let apply_response = apply_response(&fixture)?;
        assert_eq!(
            encode_runtime_ipc_publication_conflict_request_v1(&fixture.create_request)?,
            bytes(&vectors, "/expected/create/request/cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_publication_conflict_request_digest_v1(&fixture.create_request)?,
            fixed(&vectors, "/expected/create/request/object_id_hex")?
        );
        assert_eq!(
            encode_signed_runtime_ipc_publication_conflict_response_v1(&create_response)?,
            bytes(&vectors, "/expected/create/response/signed_cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_publication_conflict_response_id_v1(&create_response)?,
            fixed(&vectors, "/expected/create/response/object_id_hex")?
        );
        assert_eq!(
            encode_runtime_ipc_publication_conflict_request_v1(&fixture.apply_request)?,
            bytes(&vectors, "/expected/apply/request/cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_publication_conflict_request_digest_v1(&fixture.apply_request)?,
            fixed(&vectors, "/expected/apply/request/object_id_hex")?
        );
        assert_eq!(
            encode_signed_runtime_ipc_publication_conflict_response_v1(&apply_response)?,
            bytes(&vectors, "/expected/apply/response/signed_cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_publication_conflict_response_id_v1(&apply_response)?,
            fixed(&vectors, "/expected/apply/response/object_id_hex")?
        );
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_publication_conflict_substitutions_fail_closed() -> TestResult {
        let fixture = fixture()?;
        let create_response = create_response(&fixture)?;
        let apply_response = apply_response(&fixture)?;

        let mut changed_create = fixture.create_request.clone();
        let CanonicalRuntimeIpcPublicationConflictCommandV1::CreatePublicationConflictRequest {
            staged_replacement_ticket_artifact_digest,
            ..
        } = &mut changed_create.command
        else {
            unreachable!();
        };
        staged_replacement_ticket_artifact_digest[0] ^= 1;
        assert!(
            verify_runtime_ipc_publication_conflict_response_for_request_v1(
                &create_response,
                &changed_create,
                &fixture.descriptor,
            )
            .is_err()
        );

        let mut wrong_claim = fixture.apply_result.clone();
        wrong_claim.response_claim.request_id[0] ^= 1;
        assert!(
            sign_runtime_ipc_publication_conflict_response_v1(
                &fixture.device_key,
                &fixture.apply_request,
                &fixture.descriptor,
                fixture.apply_request.issued_at_unix_seconds,
                CanonicalRuntimeIpcPublicationConflictResultV1::PublicationConflictResponseApplied(
                    Box::new(wrong_claim),
                ),
            )
            .is_err()
        );

        assert!(
            verify_runtime_ipc_publication_conflict_response_for_request_v1(
                &apply_response,
                &fixture.create_request,
                &fixture.descriptor,
            )
            .is_err()
        );
        assert_ne!(
            runtime_ipc_publication_conflict_artifact_digest_v1(&fixture.request_artifact),
            runtime_ipc_publication_conflict_artifact_digest_v1(&fixture.response_artifact)
        );

        let mut all_ipc =
            encode_runtime_ipc_publication_conflict_request_v1(&fixture.create_request)?;
        all_ipc.extend_from_slice(&encode_signed_runtime_ipc_publication_conflict_response_v1(
            &create_response,
        )?);
        all_ipc.extend_from_slice(&encode_runtime_ipc_publication_conflict_request_v1(
            &fixture.apply_request,
        )?);
        all_ipc.extend_from_slice(&encode_signed_runtime_ipc_publication_conflict_response_v1(
            &apply_response,
        )?);
        for secret in [
            fixture.request_artifact.as_slice(),
            fixture.response_artifact.as_slice(),
            fixture.endpoint_ticket.as_slice(),
            fixture.replacement_ticket.as_slice(),
        ] {
            assert!(!all_ipc.windows(secret.len()).any(|window| window == secret));
        }
        Ok(())
    }

    fn lower_hex(bytes: &[u8]) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            output.push(char::from(DIGITS[usize::from(byte >> 4)]));
            output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
        }
        output
    }

    fn request_json(
        request: &CanonicalRuntimeIpcPublicationConflictRequestV1,
    ) -> TestResult<Value> {
        let encoded = encode_runtime_ipc_publication_conflict_request_v1(request)?;
        Ok(json!({
            "cbor_hex": lower_hex(&encoded),
            "digest_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_PUBLICATION_CONFLICT_REQUEST_DIGEST_DOMAIN),
            "id_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_PUBLICATION_CONFLICT_REQUEST_DIGEST_DOMAIN, &encoded)),
            "object_id_hex": lower_hex(&runtime_ipc_publication_conflict_request_digest_v1(request)?)
        }))
    }

    fn response_json(
        response: &CanonicalSignedRuntimeIpcPublicationConflictResponseV1,
    ) -> TestResult<Value> {
        let content = encode_response_content(&response.content)?;
        let encoded = encode_signed_runtime_ipc_publication_conflict_response_v1(response)?;
        Ok(json!({
            "content_cbor_hex": lower_hex(&content),
            "signature_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_PUBLICATION_CONFLICT_RESPONSE_SIGNATURE_DOMAIN),
            "signing_preimage_hex": lower_hex(&runtime_ipc_publication_conflict_response_signing_bytes_v1(&response.content)?),
            "signature_hex": lower_hex(&response.signature),
            "signed_cbor_hex": lower_hex(&encoded),
            "id_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_PUBLICATION_CONFLICT_RESPONSE_ID_DOMAIN, &encoded)),
            "object_id_hex": lower_hex(&runtime_ipc_publication_conflict_response_id_v1(response)?)
        }))
    }

    #[test]
    #[ignore = "explicit deterministic vector regeneration"]
    fn regenerate_canonical_runtime_ipc_publication_conflict_vectors() -> TestResult {
        let fixture = fixture()?;
        let create_response = create_response(&fixture)?;
        let apply_response = apply_response(&fixture)?;
        let create_current = match &fixture.create_request.command {
            CanonicalRuntimeIpcPublicationConflictCommandV1::CreatePublicationConflictRequest {
                expected_current,
                ..
            } => expected_current,
            _ => unreachable!(),
        };
        let apply_current = match &fixture.apply_request.command {
            CanonicalRuntimeIpcPublicationConflictCommandV1::ApplyPublicationConflictResponse {
                expected_current,
                ..
            } => expected_current,
            _ => unreachable!(),
        };
        let document = json!({
            "schema_version": 1,
            "profile": "kilogram-deterministic-cbor-v1",
            "activation_status": "not-active",
            "dependencies": [
                "CANONICAL-PUBLICATION-CONFLICT-VECTORS.json"
            ],
            "input": {
                "device_secret_hex": lower_hex(&fixture.device_key.to_bytes()),
                "descriptor_ipv4_hex": "7f000001",
                "descriptor_port": fixture.descriptor.content.port,
                "descriptor_issued_at_unix_seconds": fixture.descriptor.content.issued_at_unix_seconds,
                "descriptor_expires_at_unix_seconds": fixture.descriptor.content.expires_at_unix_seconds,
                "bearer_token_hex": lower_hex(&fixture.descriptor.content.bearer_token),
                "server_instance_id_hex": lower_hex(&fixture.descriptor.content.server_instance_id),
                "device_list_digest_hex": lower_hex(&create_current.device_list_digest),
                "create_request_id_hex": lower_hex(&fixture.create_request.request_id),
                "create_issued_at_unix_seconds": fixture.create_request.issued_at_unix_seconds,
                "create_expires_at_unix_seconds": fixture.create_request.expires_at_unix_seconds,
                "apply_request_id_hex": lower_hex(&fixture.apply_request.request_id),
                "apply_issued_at_unix_seconds": fixture.apply_request.issued_at_unix_seconds,
                "apply_expires_at_unix_seconds": fixture.apply_request.expires_at_unix_seconds,
                "endpoint_ticket_utf8_hex": lower_hex(&fixture.endpoint_ticket),
                "replacement_ticket_utf8_hex": lower_hex(&fixture.replacement_ticket),
                "request_artifact_fixture_hex": lower_hex(&fixture.request_artifact),
                "response_artifact_fixture_hex": lower_hex(&fixture.response_artifact)
            },
            "expected": {
                "account_id_hex": lower_hex(&fixture.descriptor.content.account_id),
                "device_id_hex": lower_hex(&fixture.descriptor.content.device_id),
                "descriptor_id_hex": lower_hex(&runtime_ipc_descriptor_id_v1(&fixture.descriptor)?),
                "descriptor_signed_cbor_hex": lower_hex(&encode_signed_runtime_ipc_descriptor_v1(&fixture.descriptor)?),
                "artifact_digest_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_PUBLICATION_CONFLICT_ARTIFACT_DIGEST_DOMAIN),
                "endpoint_ticket_artifact_digest_hex": lower_hex(&create_current.endpoint_ticket_artifact_digest),
                "replacement_ticket_artifact_digest_hex": lower_hex(&fixture.create_result.replacement_ticket_artifact_digest),
                "request_artifact_digest_hex": lower_hex(&fixture.create_result.request_artifact_digest),
                "response_artifact_digest_hex": lower_hex(&fixture.apply_result.response_artifact_digest),
                "create": {
                    "current_cbor_hex": lower_hex(&encode_runtime_ipc_publication_conflict_current_v1(create_current)?),
                    "result_cbor_hex": lower_hex(&encode_runtime_ipc_publication_conflict_request_created_v1(&fixture.create_result)?),
                    "request": request_json(&fixture.create_request)?,
                    "response": response_json(&create_response)?
                },
                "apply": {
                    "current_cbor_hex": lower_hex(&encode_runtime_ipc_publication_conflict_current_v1(apply_current)?),
                    "result_cbor_hex": lower_hex(&encode_runtime_ipc_publication_conflict_resolution_applied_v1(&fixture.apply_result)?),
                    "request": request_json(&fixture.apply_request)?,
                    "response": response_json(&apply_response)?
                },
                "context": {
                    "active_runtime_changed": false,
                    "network_io_executed": false,
                    "artifact_broker_activated": false,
                    "root_secret_in_ipc": false,
                    "publication_write_key_in_ipc": false,
                    "replacement_ticket_plaintext_in_ipc": false,
                    "root_resolution_plaintext_in_ipc": false,
                    "local_path_in_ipc": false,
                    "request_artifact_staged_before_success": true,
                    "response_verified_before_atomic_install": true,
                    "conflict_proof_retained_after_resolution": true,
                    "sibling_propagation_still_pending": true
                }
            }
        });
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../CANONICAL-RUNTIME-IPC-PUBLICATION-CONFLICT-VECTORS.json");
        fs::write(
            path,
            format!("{}\n", serde_json::to_string_pretty(&document)?),
        )?;
        Ok(())
    }
}
