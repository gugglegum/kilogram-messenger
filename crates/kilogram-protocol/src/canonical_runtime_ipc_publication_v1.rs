//! Inactive deterministic-CBOR publication-channel rotation boundary for local
//! runtime IPC.
//!
//! The caller commits the exact current public capability and next epoch. The
//! runtime derives the successor from its Device secret internally and returns
//! only public capability material in a Device-signed transition. Network
//! publication, peer delivery and conflict resolution are separate surfaces.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::{
    canonical_account_v1::{AccountDecoder, signed_envelope},
    canonical_runtime_ipc_v1::{
        CANONICAL_RUNTIME_IPC_CLOCK_SKEW_SECONDS, CanonicalRuntimeIpcError,
        CanonicalSignedRuntimeIpcDescriptorV1, MAX_CANONICAL_RUNTIME_IPC_ERROR_BYTES,
        MAX_CANONICAL_RUNTIME_IPC_REQUEST_VALIDITY_SECONDS, runtime_ipc_descriptor_id_v1,
        verify_runtime_ipc_descriptor_at_v1, verify_signed_runtime_ipc_descriptor_v1,
    },
    canonical_ticket_publication_v1::{
        CanonicalTicketPublicationCapabilityV1, decode_and_verify_ticket_publication_capability_v1,
        derive_ticket_publication_capability_v1, encode_ticket_publication_capability_v1,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, prefixed, push_array_header, push_bytes, push_unsigned,
        validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_REQUEST_BYTES: usize = 64 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_RESPONSE_BYTES: usize = 64 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_PEER_SCOPE_BYTES: usize = 4 * 1024;

pub const CANONICAL_RUNTIME_IPC_PUBLICATION_PEER_SCOPE_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-publication-peer-scope-digest:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_PUBLICATION_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-publication-request-digest:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_PUBLICATION_ROTATION_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-publication-rotation-id:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_PUBLICATION_RESPONSE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-publication-response-signature:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_PUBLICATION_RESPONSE_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-publication-response-id:cbor-v1\0";

const COMMAND_ROTATE_PUBLICATION_CHANNEL: u64 = 0;
const RESULT_PUBLICATION_CHANNEL_ROTATED: u64 = 0;
const RESULT_ERROR: u64 = 1;
const OUTCOME_COMMITTED: u64 = 0;
const OUTCOME_ALREADY_COMMITTED: u64 = 1;
const DELIVERY_PENDING_PEER_DELIVERY: u64 = 0;
const PUBLICATION_AWAITING_FIRST_PUBLICATION: u64 = 0;
const RESTART_NOT_REQUIRED: u64 = 0;
const OPTION_NONE: u64 = 0;
const OPTION_SOME: u64 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcPublicationChannelStateV1 {
    pub contact_id: [u8; 32],
    pub peer_account_id: [u8; 32],
    pub peer_scope_digest: [u8; 32],
    pub channel_epoch: u64,
    pub capability: CanonicalTicketPublicationCapabilityV1,
    pub latest_publication_id: Option<[u8; 32]>,
    pub latest_publication_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationCommandV1 {
    RotatePublicationChannel {
        expected_current: CanonicalRuntimeIpcPublicationChannelStateV1,
        next_channel_epoch: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcPublicationRequestV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub bearer_token: [u8; 32],
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub command: CanonicalRuntimeIpcPublicationCommandV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationRotationOutcomeV1 {
    Committed,
    AlreadyCommitted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationDeliveryStateV1 {
    PendingPeerDelivery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationStateV1 {
    AwaitingFirstPublication,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationRestartStateV1 {
    NotRequired,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcPublicationChannelRotationV1 {
    pub outcome: CanonicalRuntimeIpcPublicationRotationOutcomeV1,
    pub contact_id: [u8; 32],
    pub peer_account_id: [u8; 32],
    pub peer_scope_digest: [u8; 32],
    pub previous_capability: CanonicalTicketPublicationCapabilityV1,
    pub previous_channel_epoch: u64,
    pub capability: CanonicalTicketPublicationCapabilityV1,
    pub channel_epoch: u64,
    pub rotation_id: [u8; 32],
    pub previous_latest_publication_id: Option<[u8; 32]>,
    pub previous_latest_publication_generation: u64,
    pub publication_state: CanonicalRuntimeIpcPublicationStateV1,
    pub delivery_state: CanonicalRuntimeIpcPublicationDeliveryStateV1,
    pub restart_state: CanonicalRuntimeIpcPublicationRestartStateV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationResultV1 {
    PublicationChannelRotated(Box<CanonicalRuntimeIpcPublicationChannelRotationV1>),
    Error { code: u16, message_utf8: Vec<u8> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcPublicationResponseContentV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub request_digest: [u8; 32],
    pub responder_device_id: [u8; 32],
    pub result: CanonicalRuntimeIpcPublicationResultV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeIpcPublicationResponseV1 {
    pub content: CanonicalRuntimeIpcPublicationResponseContentV1,
    pub signature: [u8; 64],
}

pub fn canonical_runtime_ipc_publication_peer_scope_digest_v1(
    peer_scope: &[u8],
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    if peer_scope.is_empty()
        || peer_scope.len() > MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_PEER_SCOPE_BYTES
    {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_PUBLICATION_PEER_SCOPE_DIGEST_DOMAIN,
        peer_scope,
    ))
}

pub fn encode_runtime_ipc_publication_request_v1(
    request: &CanonicalRuntimeIpcPublicationRequestV1,
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
        "publication request",
        MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_runtime_ipc_publication_request_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcPublicationRequestV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "publication request",
        MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("runtime-ipc-publication-request-v1", 7)?;
    decoder.expect_version("runtime-ipc-publication-request-v1.schema-version")?;
    let request = CanonicalRuntimeIpcPublicationRequestV1 {
        descriptor_id: decoder.fixed_bytes("runtime-ipc-publication-request-v1.descriptor-id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-publication-request-v1.request-id")?,
        bearer_token: decoder.fixed_bytes("runtime-ipc-publication-request-v1.bearer-token")?,
        issued_at_unix_seconds: decoder
            .unsigned("runtime-ipc-publication-request-v1.issued-at-unix-seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("runtime-ipc-publication-request-v1.expires-at-unix-seconds")?,
        command: decode_command(&mut decoder)?,
    };
    decoder.finish()?;
    validate_request(&request)?;
    Ok(request)
}

pub fn runtime_ipc_publication_request_digest_v1(
    request: &CanonicalRuntimeIpcPublicationRequestV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_PUBLICATION_REQUEST_DIGEST_DOMAIN,
        &encode_runtime_ipc_publication_request_v1(request)?,
    ))
}

pub fn verify_runtime_ipc_publication_request_for_descriptor_v1(
    request: &CanonicalRuntimeIpcPublicationRequestV1,
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
    let CanonicalRuntimeIpcPublicationCommandV1::RotatePublicationChannel {
        expected_current, ..
    } = &request.command;
    if expected_current.peer_account_id == descriptor.content.account_id {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    Ok(())
}

pub fn runtime_ipc_publication_rotation_id_v1(
    request: &CanonicalRuntimeIpcPublicationRequestV1,
    capability: &CanonicalTicketPublicationCapabilityV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    let CanonicalRuntimeIpcPublicationCommandV1::RotatePublicationChannel {
        next_channel_epoch,
        ..
    } = &request.command;
    let capability = encode_ticket_publication_capability_v1(capability)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    let mut context = Vec::new();
    push_array_header(&mut context, 4);
    push_unsigned(&mut context, CANONICAL_PROFILE_VERSION);
    push_bytes(
        &mut context,
        &runtime_ipc_publication_request_digest_v1(request)?,
    );
    push_bytes(&mut context, &capability);
    push_unsigned(&mut context, *next_channel_epoch);
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_PUBLICATION_ROTATION_ID_DOMAIN,
        &context,
    ))
}

/// Derives the successor entirely inside the trusted runtime boundary.
///
/// Neither `device_secret` nor the plaintext durable `peer_scope` is encoded
/// in the request or returned transition. This helper authenticates the
/// current request, re-derives the public predecessor and successor, and
/// requires the exact scope digest and descriptor Device before the caller
/// commits the returned transition under one durable state lock.
pub fn derive_runtime_ipc_publication_rotation_v1(
    request: &CanonicalRuntimeIpcPublicationRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
    device_secret: [u8; 32],
    peer_scope: &[u8],
    outcome: CanonicalRuntimeIpcPublicationRotationOutcomeV1,
) -> Result<CanonicalRuntimeIpcPublicationChannelRotationV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_publication_request_for_descriptor_v1(
        request,
        descriptor,
        now_unix_seconds,
    )?;
    if runtime_ipc_descriptor_id_v1(descriptor)? != request.descriptor_id
        || SigningKey::from_bytes(&device_secret)
            .verifying_key()
            .to_bytes()
            != descriptor.content.device_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    let CanonicalRuntimeIpcPublicationCommandV1::RotatePublicationChannel {
        expected_current,
        next_channel_epoch,
    } = &request.command;
    if canonical_runtime_ipc_publication_peer_scope_digest_v1(peer_scope)?
        != expected_current.peer_scope_digest
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    let previous_capability = derive_ticket_publication_capability_v1(
        device_secret,
        peer_scope,
        expected_current.channel_epoch,
    )
    .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?;
    if previous_capability != expected_current.capability {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    let capability =
        derive_ticket_publication_capability_v1(device_secret, peer_scope, *next_channel_epoch)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?;
    let rotation = CanonicalRuntimeIpcPublicationChannelRotationV1 {
        outcome,
        contact_id: expected_current.contact_id,
        peer_account_id: expected_current.peer_account_id,
        peer_scope_digest: expected_current.peer_scope_digest,
        previous_capability: expected_current.capability.clone(),
        previous_channel_epoch: expected_current.channel_epoch,
        rotation_id: runtime_ipc_publication_rotation_id_v1(request, &capability)?,
        capability,
        channel_epoch: *next_channel_epoch,
        previous_latest_publication_id: expected_current.latest_publication_id,
        previous_latest_publication_generation: expected_current.latest_publication_generation,
        publication_state: CanonicalRuntimeIpcPublicationStateV1::AwaitingFirstPublication,
        delivery_state: CanonicalRuntimeIpcPublicationDeliveryStateV1::PendingPeerDelivery,
        restart_state: CanonicalRuntimeIpcPublicationRestartStateV1::NotRequired,
    };
    verify_rotation_for_request(&rotation, request)?;
    Ok(rotation)
}

pub fn runtime_ipc_publication_response_signing_bytes_v1(
    content: &CanonicalRuntimeIpcPublicationResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    Ok(prefixed(
        CANONICAL_RUNTIME_IPC_PUBLICATION_RESPONSE_SIGNATURE_DOMAIN,
        &encode_response_content(content)?,
    ))
}

pub fn sign_runtime_ipc_publication_response_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalRuntimeIpcPublicationRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
    result: CanonicalRuntimeIpcPublicationResultV1,
) -> Result<CanonicalSignedRuntimeIpcPublicationResponseV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_publication_request_for_descriptor_v1(
        request,
        descriptor,
        now_unix_seconds,
    )?;
    if device_signing_key.verifying_key().to_bytes() != descriptor.content.device_id {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    if let CanonicalRuntimeIpcPublicationResultV1::PublicationChannelRotated(rotation) = &result {
        verify_rotation_for_request(rotation, request)?;
    }
    let content = CanonicalRuntimeIpcPublicationResponseContentV1 {
        descriptor_id: request.descriptor_id,
        request_id: request.request_id,
        request_digest: runtime_ipc_publication_request_digest_v1(request)?,
        responder_device_id: descriptor.content.device_id,
        result,
    };
    let signature = device_signing_key
        .sign(&runtime_ipc_publication_response_signing_bytes_v1(
            &content,
        )?)
        .to_bytes();
    let response = CanonicalSignedRuntimeIpcPublicationResponseV1 { content, signature };
    verify_runtime_ipc_publication_response_for_request_v1(&response, request, descriptor)?;
    Ok(response)
}

pub fn encode_signed_runtime_ipc_publication_response_v1(
    response: &CanonicalSignedRuntimeIpcPublicationResponseV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_publication_response_v1(response)?;
    let output = signed_envelope(
        &encode_response_content(&response.content)?,
        &response.signature,
    );
    ensure_bound(
        &output,
        "publication response",
        MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_RESPONSE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_runtime_ipc_publication_response_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeIpcPublicationResponseV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "publication response",
        MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-runtime-ipc-publication-response-v1", 3)?;
    decoder.expect_version("signed-runtime-ipc-publication-response-v1.schema-version")?;
    let content = decode_response_content(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-runtime-ipc-publication-response-v1.signature")?;
    decoder.finish()?;
    let response = CanonicalSignedRuntimeIpcPublicationResponseV1 { content, signature };
    verify_signed_runtime_ipc_publication_response_v1(&response)?;
    Ok(response)
}

pub fn verify_signed_runtime_ipc_publication_response_v1(
    response: &CanonicalSignedRuntimeIpcPublicationResponseV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_response_content(&response.content)?;
    verify_ed25519(
        &response.content.responder_device_id,
        &runtime_ipc_publication_response_signing_bytes_v1(&response.content)?,
        &response.signature,
    )
}

pub fn verify_runtime_ipc_publication_response_for_request_v1(
    response: &CanonicalSignedRuntimeIpcPublicationResponseV1,
    request: &CanonicalRuntimeIpcPublicationRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_publication_response_v1(response)?;
    validate_request(request)?;
    verify_signed_runtime_ipc_descriptor_v1(descriptor)?;
    if response.content.descriptor_id != runtime_ipc_descriptor_id_v1(descriptor)?
        || response.content.descriptor_id != request.descriptor_id
        || response.content.request_id != request.request_id
        || response.content.request_digest != runtime_ipc_publication_request_digest_v1(request)?
        || response.content.responder_device_id != descriptor.content.device_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    match &response.content.result {
        CanonicalRuntimeIpcPublicationResultV1::PublicationChannelRotated(rotation) => {
            verify_rotation_for_request(rotation, request)
        }
        CanonicalRuntimeIpcPublicationResultV1::Error { .. } => Ok(()),
    }
}

pub fn runtime_ipc_publication_response_id_v1(
    response: &CanonicalSignedRuntimeIpcPublicationResponseV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_PUBLICATION_RESPONSE_ID_DOMAIN,
        &encode_signed_runtime_ipc_publication_response_v1(response)?,
    ))
}

fn validate_request(
    request: &CanonicalRuntimeIpcPublicationRequestV1,
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
    let CanonicalRuntimeIpcPublicationCommandV1::RotatePublicationChannel {
        expected_current,
        next_channel_epoch,
    } = &request.command;
    validate_channel_state(expected_current)?;
    if expected_current
        .channel_epoch
        .checked_add(1)
        .is_none_or(|expected| expected != *next_channel_epoch)
    {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    Ok(())
}

fn validate_channel_state(
    state: &CanonicalRuntimeIpcPublicationChannelStateV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if state.contact_id == [0; 32]
        || state.peer_account_id == [0; 32]
        || state.peer_scope_digest == [0; 32]
        || state.channel_epoch == 0
        || (state.latest_publication_id.is_none() && state.latest_publication_generation != 0)
        || (state.latest_publication_id.is_some() && state.latest_publication_generation == 0)
    {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    encode_ticket_publication_capability_v1(&state.capability)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?;
    Ok(())
}

fn validate_response_content(
    content: &CanonicalRuntimeIpcPublicationResponseContentV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if content.descriptor_id == [0; 32]
        || content.request_id == [0; 32]
        || content.request_digest == [0; 32]
        || content.responder_device_id == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    match &content.result {
        CanonicalRuntimeIpcPublicationResultV1::PublicationChannelRotated(rotation) => {
            validate_rotation(rotation)
        }
        CanonicalRuntimeIpcPublicationResultV1::Error { code, message_utf8 } => {
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

fn validate_rotation(
    rotation: &CanonicalRuntimeIpcPublicationChannelRotationV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if rotation.contact_id == [0; 32]
        || rotation.peer_account_id == [0; 32]
        || rotation.peer_scope_digest == [0; 32]
        || rotation.rotation_id == [0; 32]
        || rotation.previous_channel_epoch == 0
        || rotation
            .previous_channel_epoch
            .checked_add(1)
            .is_none_or(|expected| expected != rotation.channel_epoch)
        || rotation.previous_capability == rotation.capability
        || (rotation.previous_latest_publication_id.is_none()
            && rotation.previous_latest_publication_generation != 0)
        || (rotation.previous_latest_publication_id.is_some()
            && rotation.previous_latest_publication_generation == 0)
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    encode_ticket_publication_capability_v1(&rotation.previous_capability)
        .and_then(|_| encode_ticket_publication_capability_v1(&rotation.capability))
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    Ok(())
}

fn verify_rotation_for_request(
    rotation: &CanonicalRuntimeIpcPublicationChannelRotationV1,
    request: &CanonicalRuntimeIpcPublicationRequestV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_rotation(rotation)?;
    let CanonicalRuntimeIpcPublicationCommandV1::RotatePublicationChannel {
        expected_current,
        next_channel_epoch,
    } = &request.command;
    if rotation.contact_id != expected_current.contact_id
        || rotation.peer_account_id != expected_current.peer_account_id
        || rotation.peer_scope_digest != expected_current.peer_scope_digest
        || rotation.previous_capability != expected_current.capability
        || rotation.previous_channel_epoch != expected_current.channel_epoch
        || rotation.channel_epoch != *next_channel_epoch
        || rotation.previous_latest_publication_id != expected_current.latest_publication_id
        || rotation.previous_latest_publication_generation
            != expected_current.latest_publication_generation
        || rotation.rotation_id
            != runtime_ipc_publication_rotation_id_v1(request, &rotation.capability)?
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    Ok(())
}

fn encode_command(
    output: &mut Vec<u8>,
    command: &CanonicalRuntimeIpcPublicationCommandV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    let CanonicalRuntimeIpcPublicationCommandV1::RotatePublicationChannel {
        expected_current,
        next_channel_epoch,
    } = command;
    push_array_header(output, 3);
    push_unsigned(output, COMMAND_ROTATE_PUBLICATION_CHANNEL);
    encode_channel_state(output, expected_current)?;
    push_unsigned(output, *next_channel_epoch);
    Ok(())
}

fn decode_command(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcPublicationCommandV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-publication-command-v1")?;
    match (
        decoder.unsigned("runtime-ipc-publication-command-v1.variant")?,
        length,
    ) {
        (COMMAND_ROTATE_PUBLICATION_CHANNEL, 3) => Ok(
            CanonicalRuntimeIpcPublicationCommandV1::RotatePublicationChannel {
                expected_current: decode_channel_state(decoder)?,
                next_channel_epoch: decoder
                    .unsigned("runtime-ipc-publication-command-v1.next-channel-epoch")?,
            },
        ),
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_channel_state(
    output: &mut Vec<u8>,
    state: &CanonicalRuntimeIpcPublicationChannelStateV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    push_array_header(output, 8);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_bytes(output, &state.contact_id);
    push_bytes(output, &state.peer_account_id);
    push_bytes(output, &state.peer_scope_digest);
    push_unsigned(output, state.channel_epoch);
    push_bytes(
        output,
        &encode_ticket_publication_capability_v1(&state.capability)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?,
    );
    encode_optional_digest(output, state.latest_publication_id);
    push_unsigned(output, state.latest_publication_generation);
    Ok(())
}

fn decode_channel_state(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcPublicationChannelStateV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-publication-channel-state-v1", 8)?;
    decoder.expect_version("runtime-ipc-publication-channel-state-v1.schema-version")?;
    let state = CanonicalRuntimeIpcPublicationChannelStateV1 {
        contact_id: decoder.fixed_bytes("runtime-ipc-publication-channel-state-v1.contact-id")?,
        peer_account_id: decoder
            .fixed_bytes("runtime-ipc-publication-channel-state-v1.peer-account-id")?,
        peer_scope_digest: decoder
            .fixed_bytes("runtime-ipc-publication-channel-state-v1.peer-scope-digest")?,
        channel_epoch: decoder
            .unsigned("runtime-ipc-publication-channel-state-v1.channel-epoch")?,
        capability: decode_and_verify_ticket_publication_capability_v1(
            decoder.bytes("runtime-ipc-publication-channel-state-v1.capability")?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?,
        latest_publication_id: decode_optional_digest(
            decoder,
            "runtime-ipc-publication-channel-state-v1.latest-publication-id",
            CanonicalRuntimeIpcError::InvalidRequest,
        )?,
        latest_publication_generation: decoder
            .unsigned("runtime-ipc-publication-channel-state-v1.latest-publication-generation")?,
    };
    validate_channel_state(&state)?;
    Ok(state)
}

pub fn encode_runtime_ipc_publication_rotation_v1(
    rotation: &CanonicalRuntimeIpcPublicationChannelRotationV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_rotation(rotation)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 15);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_unsigned(&mut output, encode_outcome(rotation.outcome));
    push_bytes(&mut output, &rotation.contact_id);
    push_bytes(&mut output, &rotation.peer_account_id);
    push_bytes(&mut output, &rotation.peer_scope_digest);
    push_bytes(
        &mut output,
        &encode_ticket_publication_capability_v1(&rotation.previous_capability)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
    );
    push_unsigned(&mut output, rotation.previous_channel_epoch);
    push_bytes(
        &mut output,
        &encode_ticket_publication_capability_v1(&rotation.capability)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
    );
    push_unsigned(&mut output, rotation.channel_epoch);
    push_bytes(&mut output, &rotation.rotation_id);
    encode_optional_digest(&mut output, rotation.previous_latest_publication_id);
    push_unsigned(&mut output, rotation.previous_latest_publication_generation);
    push_unsigned(
        &mut output,
        encode_publication_state(rotation.publication_state),
    );
    push_unsigned(&mut output, encode_delivery_state(rotation.delivery_state));
    push_unsigned(&mut output, encode_restart_state(rotation.restart_state));
    Ok(output)
}

fn decode_rotation(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcPublicationChannelRotationV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-publication-channel-rotation-v1", 15)?;
    decoder.expect_version("runtime-ipc-publication-channel-rotation-v1.schema-version")?;
    let rotation = CanonicalRuntimeIpcPublicationChannelRotationV1 {
        outcome: decode_outcome(
            decoder.unsigned("runtime-ipc-publication-channel-rotation-v1.outcome")?,
        )?,
        contact_id: decoder
            .fixed_bytes("runtime-ipc-publication-channel-rotation-v1.contact-id")?,
        peer_account_id: decoder
            .fixed_bytes("runtime-ipc-publication-channel-rotation-v1.peer-account-id")?,
        peer_scope_digest: decoder
            .fixed_bytes("runtime-ipc-publication-channel-rotation-v1.peer-scope-digest")?,
        previous_capability: decode_and_verify_ticket_publication_capability_v1(
            decoder.bytes("runtime-ipc-publication-channel-rotation-v1.previous-capability")?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
        previous_channel_epoch: decoder
            .unsigned("runtime-ipc-publication-channel-rotation-v1.previous-channel-epoch")?,
        capability: decode_and_verify_ticket_publication_capability_v1(
            decoder.bytes("runtime-ipc-publication-channel-rotation-v1.capability")?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
        channel_epoch: decoder
            .unsigned("runtime-ipc-publication-channel-rotation-v1.channel-epoch")?,
        rotation_id: decoder
            .fixed_bytes("runtime-ipc-publication-channel-rotation-v1.rotation-id")?,
        previous_latest_publication_id: decode_optional_digest(
            decoder,
            "runtime-ipc-publication-channel-rotation-v1.previous-latest-publication-id",
            CanonicalRuntimeIpcError::InvalidResponse,
        )?,
        previous_latest_publication_generation: decoder.unsigned(
            "runtime-ipc-publication-channel-rotation-v1.previous-latest-publication-generation",
        )?,
        publication_state: decode_publication_state(
            decoder.unsigned("runtime-ipc-publication-channel-rotation-v1.publication-state")?,
        )?,
        delivery_state: decode_delivery_state(
            decoder.unsigned("runtime-ipc-publication-channel-rotation-v1.delivery-state")?,
        )?,
        restart_state: decode_restart_state(
            decoder.unsigned("runtime-ipc-publication-channel-rotation-v1.restart-state")?,
        )?,
    };
    validate_rotation(&rotation)?;
    Ok(rotation)
}

fn encode_response_content(
    content: &CanonicalRuntimeIpcPublicationResponseContentV1,
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
) -> Result<CanonicalRuntimeIpcPublicationResponseContentV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-publication-response-content-v1", 6)?;
    decoder.expect_version("runtime-ipc-publication-response-content-v1.schema-version")?;
    let content = CanonicalRuntimeIpcPublicationResponseContentV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-publication-response-content-v1.descriptor-id")?,
        request_id: decoder
            .fixed_bytes("runtime-ipc-publication-response-content-v1.request-id")?,
        request_digest: decoder
            .fixed_bytes("runtime-ipc-publication-response-content-v1.request-digest")?,
        responder_device_id: decoder
            .fixed_bytes("runtime-ipc-publication-response-content-v1.responder-device-id")?,
        result: decode_result(decoder)?,
    };
    validate_response_content(&content)?;
    Ok(content)
}

fn encode_result(
    output: &mut Vec<u8>,
    result: &CanonicalRuntimeIpcPublicationResultV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match result {
        CanonicalRuntimeIpcPublicationResultV1::PublicationChannelRotated(rotation) => {
            push_array_header(output, 2);
            push_unsigned(output, RESULT_PUBLICATION_CHANNEL_ROTATED);
            let rotation = encode_runtime_ipc_publication_rotation_v1(rotation)?;
            push_bytes(output, &rotation);
        }
        CanonicalRuntimeIpcPublicationResultV1::Error { code, message_utf8 } => {
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
) -> Result<CanonicalRuntimeIpcPublicationResultV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-publication-result-v1")?;
    match (
        decoder.unsigned("runtime-ipc-publication-result-v1.variant")?,
        length,
    ) {
        (RESULT_PUBLICATION_CHANNEL_ROTATED, 2) => {
            let bytes = decoder.bytes("runtime-ipc-publication-result-v1.rotation")?;
            validate_canonical_profile_v1(bytes)?;
            let mut nested = AccountDecoder::new(bytes);
            let rotation = decode_rotation(&mut nested)?;
            nested.finish()?;
            if encode_runtime_ipc_publication_rotation_v1(&rotation)? != bytes {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            Ok(
                CanonicalRuntimeIpcPublicationResultV1::PublicationChannelRotated(Box::new(
                    rotation,
                )),
            )
        }
        (RESULT_ERROR, 3) => Ok(CanonicalRuntimeIpcPublicationResultV1::Error {
            code: u16::try_from(decoder.unsigned("runtime-ipc-publication-result-v1.error-code")?)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            message_utf8: decoder
                .bytes("runtime-ipc-publication-result-v1.error-message-utf8")?
                .to_vec(),
        }),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_optional_digest(output: &mut Vec<u8>, value: Option<[u8; 32]>) {
    match value {
        None => {
            push_array_header(output, 1);
            push_unsigned(output, OPTION_NONE);
        }
        Some(value) => {
            push_array_header(output, 2);
            push_unsigned(output, OPTION_SOME);
            push_bytes(output, &value);
        }
    }
}

fn decode_optional_digest(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
    invalid_variant: CanonicalRuntimeIpcError,
) -> Result<Option<[u8; 32]>, CanonicalRuntimeIpcError> {
    let length = decoder.array(field)?;
    match (decoder.unsigned(field)?, length) {
        (OPTION_NONE, 1) => Ok(None),
        (OPTION_SOME, 2) => Ok(Some(decoder.fixed_bytes(field)?)),
        _ => Err(invalid_variant),
    }
}

fn encode_outcome(value: CanonicalRuntimeIpcPublicationRotationOutcomeV1) -> u64 {
    match value {
        CanonicalRuntimeIpcPublicationRotationOutcomeV1::Committed => OUTCOME_COMMITTED,
        CanonicalRuntimeIpcPublicationRotationOutcomeV1::AlreadyCommitted => {
            OUTCOME_ALREADY_COMMITTED
        }
    }
}

fn decode_outcome(
    value: u64,
) -> Result<CanonicalRuntimeIpcPublicationRotationOutcomeV1, CanonicalRuntimeIpcError> {
    match value {
        OUTCOME_COMMITTED => Ok(CanonicalRuntimeIpcPublicationRotationOutcomeV1::Committed),
        OUTCOME_ALREADY_COMMITTED => {
            Ok(CanonicalRuntimeIpcPublicationRotationOutcomeV1::AlreadyCommitted)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_publication_state(value: CanonicalRuntimeIpcPublicationStateV1) -> u64 {
    match value {
        CanonicalRuntimeIpcPublicationStateV1::AwaitingFirstPublication => {
            PUBLICATION_AWAITING_FIRST_PUBLICATION
        }
    }
}

fn decode_publication_state(
    value: u64,
) -> Result<CanonicalRuntimeIpcPublicationStateV1, CanonicalRuntimeIpcError> {
    match value {
        PUBLICATION_AWAITING_FIRST_PUBLICATION => {
            Ok(CanonicalRuntimeIpcPublicationStateV1::AwaitingFirstPublication)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_delivery_state(value: CanonicalRuntimeIpcPublicationDeliveryStateV1) -> u64 {
    match value {
        CanonicalRuntimeIpcPublicationDeliveryStateV1::PendingPeerDelivery => {
            DELIVERY_PENDING_PEER_DELIVERY
        }
    }
}

fn decode_delivery_state(
    value: u64,
) -> Result<CanonicalRuntimeIpcPublicationDeliveryStateV1, CanonicalRuntimeIpcError> {
    match value {
        DELIVERY_PENDING_PEER_DELIVERY => {
            Ok(CanonicalRuntimeIpcPublicationDeliveryStateV1::PendingPeerDelivery)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_restart_state(value: CanonicalRuntimeIpcPublicationRestartStateV1) -> u64 {
    match value {
        CanonicalRuntimeIpcPublicationRestartStateV1::NotRequired => RESTART_NOT_REQUIRED,
    }
}

fn decode_restart_state(
    value: u64,
) -> Result<CanonicalRuntimeIpcPublicationRestartStateV1, CanonicalRuntimeIpcError> {
    match value {
        RESTART_NOT_REQUIRED => Ok(CanonicalRuntimeIpcPublicationRestartStateV1::NotRequired),
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
        canonical_runtime_ipc_contact_v1::canonical_runtime_ipc_contact_id_v1,
        canonical_runtime_ipc_v1::decode_and_verify_signed_runtime_ipc_descriptor_v1,
        canonical_ticket_publication_v1::derive_ticket_publication_capability_v1,
    };

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    struct Fixture {
        descriptor: CanonicalSignedRuntimeIpcDescriptorV1,
        device_key: SigningKey,
        device_secret: [u8; 32],
        peer_scope: Vec<u8>,
        request: CanonicalRuntimeIpcPublicationRequestV1,
    }

    fn dependencies() -> TestResult<(Value, Value)> {
        Ok((
            serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../CANONICAL-RUNTIME-IPC-VECTORS.json"
            )))?,
            serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../CANONICAL-RUNTIME-IPC-CONTACT-VECTORS.json"
            )))?,
        ))
    }

    fn vectors() -> TestResult<Value> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-RUNTIME-IPC-PUBLICATION-VECTORS.json"
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

    fn fixture() -> TestResult<Fixture> {
        let (core, contact) = dependencies()?;
        let descriptor = decode_and_verify_signed_runtime_ipc_descriptor_v1(&bytes(
            &core,
            "/expected/descriptor/signed_cbor_hex",
        )?)?;
        let device_secret = fixed(&core, "/input/device_secret_key_hex")?;
        let device_key = SigningKey::from_bytes(&device_secret);
        let peer_account_id = fixed(&contact, "/input/peer_account_id_hex")?;
        let label = b"m1137-publication";
        let contact_id = canonical_runtime_ipc_contact_id_v1(
            &descriptor.content.account_id,
            &peer_account_id,
            label,
        )?;
        let peer_scope = b"m1137-publication-peer-scope".to_vec();
        let current_capability =
            derive_ticket_publication_capability_v1(device_secret, &peer_scope, 3)?;
        let request = CanonicalRuntimeIpcPublicationRequestV1 {
            descriptor_id: runtime_ipc_descriptor_id_v1(&descriptor)?,
            request_id: [0x93; 32],
            bearer_token: descriptor.content.bearer_token,
            issued_at_unix_seconds: 1_800_002_100,
            expires_at_unix_seconds: 1_800_002_130,
            command: CanonicalRuntimeIpcPublicationCommandV1::RotatePublicationChannel {
                expected_current: CanonicalRuntimeIpcPublicationChannelStateV1 {
                    contact_id,
                    peer_account_id,
                    peer_scope_digest: canonical_runtime_ipc_publication_peer_scope_digest_v1(
                        &peer_scope,
                    )?,
                    channel_epoch: 3,
                    capability: current_capability,
                    latest_publication_id: Some([0x92; 32]),
                    latest_publication_generation: 7,
                },
                next_channel_epoch: 4,
            },
        };
        Ok(Fixture {
            descriptor,
            device_key,
            device_secret,
            peer_scope,
            request,
        })
    }

    fn rotation(fixture: &Fixture) -> TestResult<CanonicalRuntimeIpcPublicationChannelRotationV1> {
        Ok(derive_runtime_ipc_publication_rotation_v1(
            &fixture.request,
            &fixture.descriptor,
            fixture.request.issued_at_unix_seconds,
            fixture.device_secret,
            &fixture.peer_scope,
            CanonicalRuntimeIpcPublicationRotationOutcomeV1::Committed,
        )?)
    }

    fn response(
        fixture: &Fixture,
    ) -> Result<CanonicalSignedRuntimeIpcPublicationResponseV1, CanonicalRuntimeIpcError> {
        sign_runtime_ipc_publication_response_v1(
            &fixture.device_key,
            &fixture.request,
            &fixture.descriptor,
            fixture.request.issued_at_unix_seconds,
            CanonicalRuntimeIpcPublicationResultV1::PublicationChannelRotated(Box::new(
                rotation(fixture).map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            )),
        )
    }

    #[test]
    fn canonical_runtime_ipc_publication_round_trips() -> TestResult {
        let fixture = fixture()?;
        verify_runtime_ipc_publication_request_for_descriptor_v1(
            &fixture.request,
            &fixture.descriptor,
            fixture.request.issued_at_unix_seconds,
        )?;
        let request_bytes = encode_runtime_ipc_publication_request_v1(&fixture.request)?;
        assert_eq!(
            decode_and_verify_runtime_ipc_publication_request_v1(&request_bytes)?,
            fixture.request
        );
        let response = response(&fixture)?;
        let response_bytes = encode_signed_runtime_ipc_publication_response_v1(&response)?;
        assert_eq!(
            decode_and_verify_signed_runtime_ipc_publication_response_v1(&response_bytes)?,
            response
        );
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_publication_vectors_match() -> TestResult {
        let fixture = fixture()?;
        let vectors = vectors()?;
        let response = response(&fixture)?;
        assert_eq!(
            encode_runtime_ipc_publication_request_v1(&fixture.request)?,
            bytes(&vectors, "/expected/request/cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_publication_request_digest_v1(&fixture.request)?,
            fixed(&vectors, "/expected/request/object_id_hex")?
        );
        assert_eq!(
            encode_signed_runtime_ipc_publication_response_v1(&response)?,
            bytes(&vectors, "/expected/response/signed_cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_publication_response_id_v1(&response)?,
            fixed(&vectors, "/expected/response/object_id_hex")?
        );
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_publication_substitutions_fail_closed() -> TestResult {
        let fixture = fixture()?;
        let response = response(&fixture)?;

        let mut changed_epoch = fixture.request.clone();
        let CanonicalRuntimeIpcPublicationCommandV1::RotatePublicationChannel {
            next_channel_epoch,
            ..
        } = &mut changed_epoch.command;
        *next_channel_epoch += 1;
        assert!(encode_runtime_ipc_publication_request_v1(&changed_epoch).is_err());

        assert!(
            derive_runtime_ipc_publication_rotation_v1(
                &fixture.request,
                &fixture.descriptor,
                fixture.request.issued_at_unix_seconds,
                fixture.device_secret,
                b"substituted-peer-scope",
                CanonicalRuntimeIpcPublicationRotationOutcomeV1::Committed,
            )
            .is_err()
        );
        let mut substituted_device_secret = fixture.device_secret;
        substituted_device_secret[0] ^= 1;
        assert!(
            derive_runtime_ipc_publication_rotation_v1(
                &fixture.request,
                &fixture.descriptor,
                fixture.request.issued_at_unix_seconds,
                substituted_device_secret,
                &fixture.peer_scope,
                CanonicalRuntimeIpcPublicationRotationOutcomeV1::Committed,
            )
            .is_err()
        );

        let mut substituted_bearer = fixture.request.clone();
        substituted_bearer.bearer_token[0] ^= 1;
        assert!(
            derive_runtime_ipc_publication_rotation_v1(
                &substituted_bearer,
                &fixture.descriptor,
                fixture.request.issued_at_unix_seconds,
                fixture.device_secret,
                &fixture.peer_scope,
                CanonicalRuntimeIpcPublicationRotationOutcomeV1::Committed,
            )
            .is_err()
        );

        let mut substituted_predecessor = fixture.request.clone();
        let CanonicalRuntimeIpcPublicationCommandV1::RotatePublicationChannel {
            expected_current,
            ..
        } = &mut substituted_predecessor.command;
        expected_current.capability = derive_ticket_publication_capability_v1(
            fixture.device_secret,
            &fixture.peer_scope,
            expected_current.channel_epoch - 1,
        )?;
        assert!(
            derive_runtime_ipc_publication_rotation_v1(
                &substituted_predecessor,
                &fixture.descriptor,
                fixture.request.issued_at_unix_seconds,
                fixture.device_secret,
                &fixture.peer_scope,
                CanonicalRuntimeIpcPublicationRotationOutcomeV1::Committed,
            )
            .is_err()
        );

        let mut changed_request = fixture.request.clone();
        changed_request.request_id[0] ^= 1;
        assert!(
            verify_runtime_ipc_publication_response_for_request_v1(
                &response,
                &changed_request,
                &fixture.descriptor,
            )
            .is_err()
        );

        let mut reused_capability = rotation(&fixture)?;
        reused_capability.capability = reused_capability.previous_capability.clone();
        assert!(
            sign_runtime_ipc_publication_response_v1(
                &fixture.device_key,
                &fixture.request,
                &fixture.descriptor,
                fixture.request.issued_at_unix_seconds,
                CanonicalRuntimeIpcPublicationResultV1::PublicationChannelRotated(Box::new(
                    reused_capability,
                )),
            )
            .is_err()
        );
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

    #[test]
    #[ignore = "explicit deterministic vector regeneration"]
    fn regenerate_canonical_runtime_ipc_publication_vectors() -> TestResult {
        let fixture = fixture()?;
        let request_bytes = encode_runtime_ipc_publication_request_v1(&fixture.request)?;
        let request_digest = runtime_ipc_publication_request_digest_v1(&fixture.request)?;
        let response = response(&fixture)?;
        let response_content = encode_response_content(&response.content)?;
        let response_bytes = encode_signed_runtime_ipc_publication_response_v1(&response)?;
        let response_id = runtime_ipc_publication_response_id_v1(&response)?;
        let rotation = rotation(&fixture)?;
        let CanonicalRuntimeIpcPublicationCommandV1::RotatePublicationChannel {
            expected_current,
            next_channel_epoch,
        } = &fixture.request.command;
        let document = json!({
            "schema_version": 1,
            "profile": "kilogram-deterministic-cbor-v1",
            "activation_status": "not-active",
            "dependencies": [
                "CANONICAL-RUNTIME-IPC-VECTORS.json",
                "CANONICAL-RUNTIME-IPC-CONTACT-VECTORS.json",
                "CANONICAL-TICKET-PUBLICATION-VECTORS.json"
            ],
            "input": {
                "device_secret_hex": lower_hex(&fixture.device_secret),
                "request_id_hex": lower_hex(&fixture.request.request_id),
                "request_issued_at_unix_seconds": fixture.request.issued_at_unix_seconds,
                "request_expires_at_unix_seconds": fixture.request.expires_at_unix_seconds,
                "peer_scope_hex": lower_hex(&fixture.peer_scope),
                "contact_id_hex": lower_hex(&expected_current.contact_id),
                "peer_account_id_hex": lower_hex(&expected_current.peer_account_id),
                "current_channel_epoch": expected_current.channel_epoch,
                "next_channel_epoch": next_channel_epoch,
                "latest_publication_id_hex": lower_hex(&expected_current.latest_publication_id.ok_or("missing publication ID")?),
                "latest_publication_generation": expected_current.latest_publication_generation,
                "outcome": "committed"
            },
            "expected": {
                "account_id_hex": lower_hex(&fixture.descriptor.content.account_id),
                "device_id_hex": lower_hex(&fixture.descriptor.content.device_id),
                "peer_scope_digest_hex": lower_hex(&expected_current.peer_scope_digest),
                "previous_capability": {
                    "write_key_hex": lower_hex(&expected_current.capability.write_key),
                    "channel_id_hex": lower_hex(&expected_current.capability.channel_id),
                    "cbor_hex": lower_hex(&encode_ticket_publication_capability_v1(&expected_current.capability)?)
                },
                "capability": {
                    "write_key_hex": lower_hex(&rotation.capability.write_key),
                    "channel_id_hex": lower_hex(&rotation.capability.channel_id),
                    "cbor_hex": lower_hex(&encode_ticket_publication_capability_v1(&rotation.capability)?)
                },
                "rotation_id_hex": lower_hex(&rotation.rotation_id),
                "rotation_cbor_hex": lower_hex(&encode_runtime_ipc_publication_rotation_v1(&rotation)?),
                "request": {
                    "cbor_hex": lower_hex(&request_bytes),
                    "digest_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_PUBLICATION_REQUEST_DIGEST_DOMAIN),
                    "id_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_PUBLICATION_REQUEST_DIGEST_DOMAIN, &request_bytes)),
                    "object_id_hex": lower_hex(&request_digest)
                },
                "response": {
                    "content_cbor_hex": lower_hex(&response_content),
                    "signature_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_PUBLICATION_RESPONSE_SIGNATURE_DOMAIN),
                    "signing_preimage_hex": lower_hex(&runtime_ipc_publication_response_signing_bytes_v1(&response.content)?),
                    "signature_hex": lower_hex(&response.signature),
                    "signed_cbor_hex": lower_hex(&response_bytes),
                    "id_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_PUBLICATION_RESPONSE_ID_DOMAIN, &response_bytes)),
                    "object_id_hex": lower_hex(&response_id)
                },
                "context": {
                    "valid_at_unix_seconds": fixture.request.issued_at_unix_seconds,
                    "successor_derived_inside_runtime": true,
                    "local_device_secret_in_payload": false,
                    "network_publication_executed": false,
                    "peer_delivery_executed": false,
                    "active_runtime_changed": false
                }
            }
        });
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../CANONICAL-RUNTIME-IPC-PUBLICATION-VECTORS.json");
        fs::write(
            path,
            format!("{}\n", serde_json::to_string_pretty(&document)?),
        )?;
        Ok(())
    }
}
