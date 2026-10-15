//! Inactive deterministic-CBOR authority-changing device-directory boundary
//! for local runtime IPC.
//!
//! The active runtime remains Postcard IPC version 26. This candidate embeds
//! the exact Root-signed successor roster in the request so a pathname cannot
//! become a time-of-check/time-of-use authority substitution.

use std::collections::BTreeSet;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::{
    canonical_account_v1::{
        AccountDecoder, CanonicalSignedAccountDeviceListV1,
        decode_and_verify_signed_account_device_list_v1, encode_signed_account_device_list_v1,
        signed_envelope, verify_signed_account_device_list_v1,
    },
    canonical_runtime_ipc_contact_v1::{
        CanonicalRuntimeIpcDeviceDirectoryStatusV1, CanonicalRuntimeIpcProfileConvergenceV1,
        decode_device_directory_status, encode_device_directory_status,
        validate_device_directory_status,
    },
    canonical_runtime_ipc_v1::{
        CANONICAL_RUNTIME_IPC_CLOCK_SKEW_SECONDS, CanonicalRuntimeIpcError,
        CanonicalSignedRuntimeIpcDescriptorV1, MAX_CANONICAL_RUNTIME_IPC_ERROR_BYTES,
        MAX_CANONICAL_RUNTIME_IPC_REQUEST_BYTES,
        MAX_CANONICAL_RUNTIME_IPC_REQUEST_VALIDITY_SECONDS, runtime_ipc_descriptor_id_v1,
        verify_runtime_ipc_descriptor_at_v1, verify_signed_runtime_ipc_descriptor_v1,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, prefixed, push_array_header, push_bytes, push_unsigned,
        validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_RUNTIME_IPC_DIRECTORY_RESPONSE_BYTES: usize = 128 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_DIRECTORY_PATH_BYTES: usize = 4 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_DIRECTORY_DEVICES: usize = 32;

pub const CANONICAL_RUNTIME_IPC_DIRECTORY_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-directory-request-digest:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_DIRECTORY_RESPONSE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-directory-response-signature:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_DIRECTORY_RESPONSE_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-directory-response-id:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_DEVICE_LIST_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-device-list-digest:cbor-v1\0";

const COMMAND_APPLY_OWN_DEVICE_DIRECTORY: u64 = 0;
const RESULT_OWN_DEVICE_DIRECTORY_APPLIED: u64 = 0;
const RESULT_ERROR: u64 = 1;
const STATUS_REMOVED_DEVICES_EXCLUDED: u64 = 0;
const STATUS_IMMUTABLE_PREEXISTING_RECIPIENTS: u64 = 0;
const STATUS_EXISTING_COPIES_REMAIN_READABLE: u64 = 0;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcDirectoryCommandV1 {
    ApplyOwnDeviceDirectory {
        device_list_file_utf8: Vec<u8>,
        expected_current_authority_revision: u64,
        expected_current_device_list_digest: [u8; 32],
        candidate_device_list: CanonicalSignedAccountDeviceListV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcDirectoryRequestV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub bearer_token: [u8; 32],
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub command: CanonicalRuntimeIpcDirectoryCommandV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcFutureRecipientStatusV1 {
    RemovedDevicesExcludedFromFutureRecipientSets,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPreexistingRecipientStatusV1 {
    ImmutableCannotBeRemotelyRewrittenOrErased,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcHistoryAvailabilityStatusV1 {
    ExistingCopiesRemainReadable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcDeviceDirectoryAppliedV1 {
    pub account_id: [u8; 32],
    pub local_device_id: [u8; 32],
    pub previous_authority_revision: u64,
    pub authority_revision: u64,
    pub previous_device_list_digest: [u8; 32],
    pub device_list_digest: [u8; 32],
    pub active_device_count: u16,
    pub removed_device_ids: Vec<[u8; 32]>,
    pub ratchet_session_records_retired: u64,
    pub prekey_observations_retired: u64,
    pub pending_unmaterialized_messages: u64,
    pub pending_materialized_messages: u64,
    pub ticket_published: bool,
    pub launch_profile_update_required: bool,
    pub future_recipient_status: CanonicalRuntimeIpcFutureRecipientStatusV1,
    pub preexisting_recipient_status: CanonicalRuntimeIpcPreexistingRecipientStatusV1,
    pub history_availability_status: CanonicalRuntimeIpcHistoryAvailabilityStatusV1,
    pub directory_status: CanonicalRuntimeIpcDeviceDirectoryStatusV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcDirectoryResultV1 {
    OwnDeviceDirectoryApplied(Box<CanonicalRuntimeIpcDeviceDirectoryAppliedV1>),
    Error { code: u16, message_utf8: Vec<u8> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcDirectoryResponseContentV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub request_digest: [u8; 32],
    pub responder_device_id: [u8; 32],
    pub result: CanonicalRuntimeIpcDirectoryResultV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeIpcDirectoryResponseV1 {
    pub content: CanonicalRuntimeIpcDirectoryResponseContentV1,
    pub signature: [u8; 64],
}

pub fn encode_runtime_ipc_directory_request_v1(
    request: &CanonicalRuntimeIpcDirectoryRequestV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_directory_request(request)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 7);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &request.descriptor_id);
    push_bytes(&mut output, &request.request_id);
    push_bytes(&mut output, &request.bearer_token);
    push_unsigned(&mut output, request.issued_at_unix_seconds);
    push_unsigned(&mut output, request.expires_at_unix_seconds);
    encode_directory_command(&mut output, &request.command)?;
    ensure_bound(
        &output,
        "directory request",
        MAX_CANONICAL_RUNTIME_IPC_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_runtime_ipc_directory_request_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcDirectoryRequestV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "directory request",
        MAX_CANONICAL_RUNTIME_IPC_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("runtime-ipc-directory-request-v1", 7)?;
    decoder.expect_version("runtime-ipc-directory-request-v1.schema-version")?;
    let request = CanonicalRuntimeIpcDirectoryRequestV1 {
        descriptor_id: decoder.fixed_bytes("runtime-ipc-directory-request-v1.descriptor-id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-directory-request-v1.request-id")?,
        bearer_token: decoder.fixed_bytes("runtime-ipc-directory-request-v1.bearer-token")?,
        issued_at_unix_seconds: decoder
            .unsigned("runtime-ipc-directory-request-v1.issued-at-unix-seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("runtime-ipc-directory-request-v1.expires-at-unix-seconds")?,
        command: decode_directory_command(&mut decoder)?,
    };
    decoder.finish()?;
    validate_directory_request(&request)?;
    Ok(request)
}

pub fn runtime_ipc_directory_request_digest_v1(
    request: &CanonicalRuntimeIpcDirectoryRequestV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_DIRECTORY_REQUEST_DIGEST_DOMAIN,
        &encode_runtime_ipc_directory_request_v1(request)?,
    ))
}

pub fn runtime_ipc_device_list_digest_v1(
    device_list: &CanonicalSignedAccountDeviceListV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_DEVICE_LIST_DIGEST_DOMAIN,
        &encode_signed_account_device_list_v1(device_list)?,
    ))
}

pub fn verify_runtime_ipc_device_directory_transition_v1(
    current: &CanonicalSignedAccountDeviceListV1,
    candidate: &CanonicalSignedAccountDeviceListV1,
    local_device_id: &[u8; 32],
) -> Result<Vec<[u8; 32]>, CanonicalRuntimeIpcError> {
    verify_signed_account_device_list_v1(current)?;
    verify_signed_account_device_list_v1(candidate)?;
    if *local_device_id == [0; 32]
        || current.content.authority_snapshot.content.account_id
            != candidate.content.authority_snapshot.content.account_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    let current_revision = current.content.authority_snapshot.content.revision;
    let candidate_revision = candidate.content.authority_snapshot.content.revision;
    if candidate_revision < current_revision
        || (candidate_revision == current_revision && candidate != current)
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    if current
        .content
        .authority_snapshot
        .content
        .revocations
        .iter()
        .any(|revocation| {
            !candidate
                .content
                .authority_snapshot
                .content
                .revocations
                .contains(revocation)
        })
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }

    let current_local = current
        .content
        .devices
        .iter()
        .find(|certificate| certificate.content.device_id == *local_device_id)
        .ok_or(CanonicalRuntimeIpcError::ContextMismatch)?;
    let candidate_local = candidate
        .content
        .devices
        .iter()
        .find(|certificate| certificate.content.device_id == *local_device_id)
        .ok_or(CanonicalRuntimeIpcError::ContextMismatch)?;
    if candidate_local != current_local {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    for certificate in &candidate.content.devices {
        let Some(previous) = current
            .content
            .devices
            .iter()
            .find(|previous| previous.content.device_id == certificate.content.device_id)
        else {
            return Err(CanonicalRuntimeIpcError::ContextMismatch);
        };
        if previous != certificate {
            return Err(CanonicalRuntimeIpcError::ContextMismatch);
        }
    }

    let candidate_ids = candidate
        .content
        .devices
        .iter()
        .map(|certificate| certificate.content.device_id)
        .collect::<BTreeSet<_>>();
    let removed = current
        .content
        .devices
        .iter()
        .map(|certificate| certificate.content.device_id)
        .filter(|device_id| !candidate_ids.contains(device_id))
        .collect::<Vec<_>>();
    for removed_device_id in &removed {
        if !candidate
            .content
            .authority_snapshot
            .content
            .revocations
            .iter()
            .any(|revocation| revocation.content.device_id == *removed_device_id)
        {
            return Err(CanonicalRuntimeIpcError::ContextMismatch);
        }
    }
    Ok(removed)
}

pub fn verify_runtime_ipc_directory_request_for_descriptor_v1(
    request: &CanonicalRuntimeIpcDirectoryRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    current_device_list: &CanonicalSignedAccountDeviceListV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_runtime_ipc_descriptor_at_v1(descriptor, now_unix_seconds)?;
    validate_directory_request(request)?;
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
    verify_signed_account_device_list_v1(current_device_list)?;
    if current_device_list
        .content
        .authority_snapshot
        .content
        .account_id
        != descriptor.content.account_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    match &request.command {
        CanonicalRuntimeIpcDirectoryCommandV1::ApplyOwnDeviceDirectory {
            expected_current_authority_revision,
            expected_current_device_list_digest,
            candidate_device_list,
            ..
        } => {
            if *expected_current_authority_revision
                != current_device_list
                    .content
                    .authority_snapshot
                    .content
                    .revision
                || *expected_current_device_list_digest
                    != runtime_ipc_device_list_digest_v1(current_device_list)?
                || candidate_device_list
                    .content
                    .authority_snapshot
                    .content
                    .account_id
                    != descriptor.content.account_id
            {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            verify_runtime_ipc_device_directory_transition_v1(
                current_device_list,
                candidate_device_list,
                &descriptor.content.device_id,
            )?;
        }
    }
    Ok(())
}

pub fn runtime_ipc_directory_response_signing_bytes_v1(
    content: &CanonicalRuntimeIpcDirectoryResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    Ok(prefixed(
        CANONICAL_RUNTIME_IPC_DIRECTORY_RESPONSE_SIGNATURE_DOMAIN,
        &encode_directory_response_content(content)?,
    ))
}

pub fn sign_runtime_ipc_directory_response_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalRuntimeIpcDirectoryRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    current_device_list: &CanonicalSignedAccountDeviceListV1,
    now_unix_seconds: u64,
    result: CanonicalRuntimeIpcDirectoryResultV1,
) -> Result<CanonicalSignedRuntimeIpcDirectoryResponseV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_directory_request_for_descriptor_v1(
        request,
        descriptor,
        current_device_list,
        now_unix_seconds,
    )?;
    if device_signing_key.verifying_key().to_bytes() != descriptor.content.device_id {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    let content = CanonicalRuntimeIpcDirectoryResponseContentV1 {
        descriptor_id: request.descriptor_id,
        request_id: request.request_id,
        request_digest: runtime_ipc_directory_request_digest_v1(request)?,
        responder_device_id: descriptor.content.device_id,
        result,
    };
    let signature = device_signing_key
        .sign(&runtime_ipc_directory_response_signing_bytes_v1(&content)?)
        .to_bytes();
    let response = CanonicalSignedRuntimeIpcDirectoryResponseV1 { content, signature };
    verify_runtime_ipc_directory_response_for_request_v1(
        &response,
        request,
        descriptor,
        current_device_list,
    )?;
    Ok(response)
}

pub fn encode_signed_runtime_ipc_directory_response_v1(
    response: &CanonicalSignedRuntimeIpcDirectoryResponseV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_directory_response_v1(response)?;
    let output = signed_envelope(
        &encode_directory_response_content(&response.content)?,
        &response.signature,
    );
    ensure_bound(
        &output,
        "directory response",
        MAX_CANONICAL_RUNTIME_IPC_DIRECTORY_RESPONSE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_runtime_ipc_directory_response_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeIpcDirectoryResponseV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "directory response",
        MAX_CANONICAL_RUNTIME_IPC_DIRECTORY_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-runtime-ipc-directory-response-v1", 3)?;
    decoder.expect_version("signed-runtime-ipc-directory-response-v1.schema-version")?;
    let content = decode_directory_response_content(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-runtime-ipc-directory-response-v1.signature")?;
    decoder.finish()?;
    let response = CanonicalSignedRuntimeIpcDirectoryResponseV1 { content, signature };
    verify_signed_runtime_ipc_directory_response_v1(&response)?;
    Ok(response)
}

pub fn verify_signed_runtime_ipc_directory_response_v1(
    response: &CanonicalSignedRuntimeIpcDirectoryResponseV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_directory_response_content(&response.content)?;
    verify_ed25519(
        &response.content.responder_device_id,
        &runtime_ipc_directory_response_signing_bytes_v1(&response.content)?,
        &response.signature,
    )
}

pub fn verify_runtime_ipc_directory_response_for_request_v1(
    response: &CanonicalSignedRuntimeIpcDirectoryResponseV1,
    request: &CanonicalRuntimeIpcDirectoryRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    current_device_list: &CanonicalSignedAccountDeviceListV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_directory_response_v1(response)?;
    validate_directory_request(request)?;
    verify_signed_runtime_ipc_descriptor_v1(descriptor)?;
    if response.content.descriptor_id != runtime_ipc_descriptor_id_v1(descriptor)?
        || response.content.descriptor_id != request.descriptor_id
        || response.content.request_id != request.request_id
        || response.content.request_digest != runtime_ipc_directory_request_digest_v1(request)?
        || response.content.responder_device_id != descriptor.content.device_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    match (&request.command, &response.content.result) {
        (_, CanonicalRuntimeIpcDirectoryResultV1::Error { .. }) => Ok(()),
        (
            CanonicalRuntimeIpcDirectoryCommandV1::ApplyOwnDeviceDirectory {
                device_list_file_utf8,
                expected_current_authority_revision,
                expected_current_device_list_digest,
                candidate_device_list,
            },
            CanonicalRuntimeIpcDirectoryResultV1::OwnDeviceDirectoryApplied(applied),
        ) => {
            let removed = verify_runtime_ipc_device_directory_transition_v1(
                current_device_list,
                candidate_device_list,
                &descriptor.content.device_id,
            )?;
            if applied.account_id != descriptor.content.account_id
                || applied.local_device_id != descriptor.content.device_id
                || *expected_current_authority_revision
                    != current_device_list
                        .content
                        .authority_snapshot
                        .content
                        .revision
                || *expected_current_device_list_digest
                    != runtime_ipc_device_list_digest_v1(current_device_list)?
                || candidate_device_list
                    .content
                    .authority_snapshot
                    .content
                    .account_id
                    != descriptor.content.account_id
                || applied.previous_authority_revision
                    != current_device_list
                        .content
                        .authority_snapshot
                        .content
                        .revision
                || applied.authority_revision
                    != candidate_device_list
                        .content
                        .authority_snapshot
                        .content
                        .revision
                || applied.previous_device_list_digest
                    != runtime_ipc_device_list_digest_v1(current_device_list)?
                || applied.device_list_digest
                    != runtime_ipc_device_list_digest_v1(candidate_device_list)?
                || usize::from(applied.active_device_count)
                    != candidate_device_list.content.devices.len()
                || applied.removed_device_ids != removed
                || applied.directory_status.applied_device_list_file_utf8 != *device_list_file_utf8
            {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            Ok(())
        }
    }
}

pub fn runtime_ipc_directory_response_id_v1(
    response: &CanonicalSignedRuntimeIpcDirectoryResponseV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_DIRECTORY_RESPONSE_ID_DOMAIN,
        &encode_signed_runtime_ipc_directory_response_v1(response)?,
    ))
}

fn validate_directory_request(
    request: &CanonicalRuntimeIpcDirectoryRequestV1,
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
        CanonicalRuntimeIpcDirectoryCommandV1::ApplyOwnDeviceDirectory {
            device_list_file_utf8,
            expected_current_authority_revision,
            expected_current_device_list_digest,
            candidate_device_list,
        } => {
            validate_text(
                device_list_file_utf8,
                1,
                MAX_CANONICAL_RUNTIME_IPC_DIRECTORY_PATH_BYTES,
                CanonicalRuntimeIpcError::InvalidRequest,
            )?;
            if *expected_current_authority_revision == 0
                || *expected_current_device_list_digest == [0; 32]
            {
                return Err(CanonicalRuntimeIpcError::InvalidRequest);
            }
            verify_signed_account_device_list_v1(candidate_device_list)?;
        }
    }
    Ok(())
}

fn validate_directory_response_content(
    response: &CanonicalRuntimeIpcDirectoryResponseContentV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if response.descriptor_id == [0; 32]
        || response.request_id == [0; 32]
        || response.request_digest == [0; 32]
        || response.responder_device_id == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    match &response.result {
        CanonicalRuntimeIpcDirectoryResultV1::OwnDeviceDirectoryApplied(applied) => {
            validate_directory_applied(applied)
        }
        CanonicalRuntimeIpcDirectoryResultV1::Error { code, message_utf8 } => {
            if *code == 0 {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            validate_text(
                message_utf8,
                1,
                MAX_CANONICAL_RUNTIME_IPC_ERROR_BYTES,
                CanonicalRuntimeIpcError::InvalidResponse,
            )
        }
    }
}

fn validate_directory_applied(
    applied: &CanonicalRuntimeIpcDeviceDirectoryAppliedV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_device_directory_status(&applied.directory_status)?;
    let removed_ordered = applied
        .removed_device_ids
        .windows(2)
        .all(|pair| pair[0] < pair[1]);
    let idempotence_valid = if applied.previous_authority_revision == applied.authority_revision {
        applied.previous_device_list_digest == applied.device_list_digest
            && applied.removed_device_ids.is_empty()
    } else {
        applied.previous_authority_revision < applied.authority_revision
            && applied.previous_device_list_digest != applied.device_list_digest
    };
    let idempotent_effects_valid = applied.previous_authority_revision
        != applied.authority_revision
        || (applied.ratchet_session_records_retired == 0
            && applied.prekey_observations_retired == 0
            && !applied.ticket_published);
    if applied.account_id == [0; 32]
        || applied.local_device_id == [0; 32]
        || applied.previous_authority_revision == 0
        || applied.authority_revision == 0
        || applied.previous_device_list_digest == [0; 32]
        || applied.device_list_digest == [0; 32]
        || !(1..=MAX_CANONICAL_RUNTIME_IPC_DIRECTORY_DEVICES as u16)
            .contains(&applied.active_device_count)
        || applied.removed_device_ids.len() > MAX_CANONICAL_RUNTIME_IPC_DIRECTORY_DEVICES
        || applied
            .removed_device_ids
            .contains(&applied.local_device_id)
        || !removed_ordered
        || !idempotence_valid
        || !idempotent_effects_valid
        || applied.directory_status.account_id != applied.account_id
        || applied.directory_status.local_device_id != applied.local_device_id
        || applied.directory_status.authority_revision != applied.authority_revision
        || applied.directory_status.active_device_count != applied.active_device_count
        || applied.directory_status.device_list_digest != applied.device_list_digest
        || applied.launch_profile_update_required
            != matches!(
                applied.directory_status.profile_convergence,
                CanonicalRuntimeIpcProfileConvergenceV1::Required
            )
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn encode_directory_command(
    output: &mut Vec<u8>,
    command: &CanonicalRuntimeIpcDirectoryCommandV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match command {
        CanonicalRuntimeIpcDirectoryCommandV1::ApplyOwnDeviceDirectory {
            device_list_file_utf8,
            expected_current_authority_revision,
            expected_current_device_list_digest,
            candidate_device_list,
        } => {
            push_array_header(output, 5);
            push_unsigned(output, COMMAND_APPLY_OWN_DEVICE_DIRECTORY);
            push_bytes(output, device_list_file_utf8);
            push_unsigned(output, *expected_current_authority_revision);
            push_bytes(output, expected_current_device_list_digest);
            push_bytes(
                output,
                &encode_signed_account_device_list_v1(candidate_device_list)?,
            );
        }
    }
    Ok(())
}

fn decode_directory_command(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcDirectoryCommandV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-directory-command-v1")?;
    match (
        decoder.unsigned("runtime-ipc-directory-command-v1.variant")?,
        length,
    ) {
        (COMMAND_APPLY_OWN_DEVICE_DIRECTORY, 5) => {
            let device_list_file_utf8 = decoder
                .bytes("runtime-ipc-directory-command-v1.device-list-file-utf8")?
                .to_vec();
            let expected_current_authority_revision = decoder
                .unsigned("runtime-ipc-directory-command-v1.expected-current-authority-revision")?;
            let expected_current_device_list_digest = decoder.fixed_bytes(
                "runtime-ipc-directory-command-v1.expected-current-device-list-digest",
            )?;
            let candidate = decoder
                .bytes("runtime-ipc-directory-command-v1.candidate-device-list")?
                .to_vec();
            Ok(
                CanonicalRuntimeIpcDirectoryCommandV1::ApplyOwnDeviceDirectory {
                    device_list_file_utf8,
                    expected_current_authority_revision,
                    expected_current_device_list_digest,
                    candidate_device_list: decode_and_verify_signed_account_device_list_v1(
                        &candidate,
                    )?,
                },
            )
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_directory_response_content(
    content: &CanonicalRuntimeIpcDirectoryResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_directory_response_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 6);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.descriptor_id);
    push_bytes(&mut output, &content.request_id);
    push_bytes(&mut output, &content.request_digest);
    push_bytes(&mut output, &content.responder_device_id);
    encode_directory_result(&mut output, &content.result);
    Ok(output)
}

fn decode_directory_response_content(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcDirectoryResponseContentV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-directory-response-content-v1", 6)?;
    decoder.expect_version("runtime-ipc-directory-response-content-v1.schema-version")?;
    let content = CanonicalRuntimeIpcDirectoryResponseContentV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-directory-response-content-v1.descriptor-id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-directory-response-content-v1.request-id")?,
        request_digest: decoder
            .fixed_bytes("runtime-ipc-directory-response-content-v1.request-digest")?,
        responder_device_id: decoder
            .fixed_bytes("runtime-ipc-directory-response-content-v1.responder-device-id")?,
        result: decode_directory_result(decoder)?,
    };
    validate_directory_response_content(&content)?;
    Ok(content)
}

fn encode_directory_result(output: &mut Vec<u8>, result: &CanonicalRuntimeIpcDirectoryResultV1) {
    match result {
        CanonicalRuntimeIpcDirectoryResultV1::OwnDeviceDirectoryApplied(applied) => {
            push_array_header(output, 2);
            push_unsigned(output, RESULT_OWN_DEVICE_DIRECTORY_APPLIED);
            encode_directory_applied(output, applied);
        }
        CanonicalRuntimeIpcDirectoryResultV1::Error { code, message_utf8 } => {
            push_array_header(output, 3);
            push_unsigned(output, RESULT_ERROR);
            push_unsigned(output, u64::from(*code));
            push_bytes(output, message_utf8);
        }
    }
}

fn decode_directory_result(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcDirectoryResultV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-directory-result-v1")?;
    match (
        decoder.unsigned("runtime-ipc-directory-result-v1.variant")?,
        length,
    ) {
        (RESULT_OWN_DEVICE_DIRECTORY_APPLIED, 2) => Ok(
            CanonicalRuntimeIpcDirectoryResultV1::OwnDeviceDirectoryApplied(Box::new(
                decode_directory_applied(decoder)?,
            )),
        ),
        (RESULT_ERROR, 3) => Ok(CanonicalRuntimeIpcDirectoryResultV1::Error {
            code: u16::try_from(decoder.unsigned("runtime-ipc-directory-result-v1.error-code")?)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            message_utf8: decoder
                .bytes("runtime-ipc-directory-result-v1.error-message-utf8")?
                .to_vec(),
        }),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_directory_applied(
    output: &mut Vec<u8>,
    applied: &CanonicalRuntimeIpcDeviceDirectoryAppliedV1,
) {
    push_array_header(output, 19);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_bytes(output, &applied.account_id);
    push_bytes(output, &applied.local_device_id);
    push_unsigned(output, applied.previous_authority_revision);
    push_unsigned(output, applied.authority_revision);
    push_bytes(output, &applied.previous_device_list_digest);
    push_bytes(output, &applied.device_list_digest);
    push_unsigned(output, u64::from(applied.active_device_count));
    push_array_header(output, applied.removed_device_ids.len() as u64);
    for device_id in &applied.removed_device_ids {
        push_bytes(output, device_id);
    }
    push_unsigned(output, applied.ratchet_session_records_retired);
    push_unsigned(output, applied.prekey_observations_retired);
    push_unsigned(output, applied.pending_unmaterialized_messages);
    push_unsigned(output, applied.pending_materialized_messages);
    encode_bool(output, applied.ticket_published);
    encode_bool(output, applied.launch_profile_update_required);
    encode_future_recipient_status(output, applied.future_recipient_status);
    encode_preexisting_recipient_status(output, applied.preexisting_recipient_status);
    encode_history_availability_status(output, applied.history_availability_status);
    encode_device_directory_status(output, &applied.directory_status);
}

fn decode_directory_applied(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcDeviceDirectoryAppliedV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-device-directory-applied-v1", 19)?;
    decoder.expect_version("runtime-ipc-device-directory-applied-v1.schema-version")?;
    let account_id = decoder.fixed_bytes("runtime-ipc-device-directory-applied-v1.account-id")?;
    let local_device_id =
        decoder.fixed_bytes("runtime-ipc-device-directory-applied-v1.local-device-id")?;
    let previous_authority_revision =
        decoder.unsigned("runtime-ipc-device-directory-applied-v1.previous-authority-revision")?;
    let authority_revision =
        decoder.unsigned("runtime-ipc-device-directory-applied-v1.authority-revision")?;
    let previous_device_list_digest = decoder
        .fixed_bytes("runtime-ipc-device-directory-applied-v1.previous-device-list-digest")?;
    let device_list_digest =
        decoder.fixed_bytes("runtime-ipc-device-directory-applied-v1.device-list-digest")?;
    let active_device_count = u16::try_from(
        decoder.unsigned("runtime-ipc-device-directory-applied-v1.active-device-count")?,
    )
    .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    let removed_count =
        decoder.array("runtime-ipc-device-directory-applied-v1.removed-device-ids")?;
    if removed_count > MAX_CANONICAL_RUNTIME_IPC_DIRECTORY_DEVICES {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    let mut removed_device_ids = Vec::with_capacity(removed_count);
    for _ in 0..removed_count {
        removed_device_ids.push(
            decoder.fixed_bytes("runtime-ipc-device-directory-applied-v1.removed-device-id")?,
        );
    }
    let applied = CanonicalRuntimeIpcDeviceDirectoryAppliedV1 {
        account_id,
        local_device_id,
        previous_authority_revision,
        authority_revision,
        previous_device_list_digest,
        device_list_digest,
        active_device_count,
        removed_device_ids,
        ratchet_session_records_retired: decoder
            .unsigned("runtime-ipc-device-directory-applied-v1.ratchet-session-records-retired")?,
        prekey_observations_retired: decoder
            .unsigned("runtime-ipc-device-directory-applied-v1.prekey-observations-retired")?,
        pending_unmaterialized_messages: decoder
            .unsigned("runtime-ipc-device-directory-applied-v1.pending-unmaterialized-messages")?,
        pending_materialized_messages: decoder
            .unsigned("runtime-ipc-device-directory-applied-v1.pending-materialized-messages")?,
        ticket_published: decode_bool(
            decoder,
            "runtime-ipc-device-directory-applied-v1.ticket-published",
        )?,
        launch_profile_update_required: decode_bool(
            decoder,
            "runtime-ipc-device-directory-applied-v1.launch-profile-update-required",
        )?,
        future_recipient_status: decode_future_recipient_status(decoder)?,
        preexisting_recipient_status: decode_preexisting_recipient_status(decoder)?,
        history_availability_status: decode_history_availability_status(decoder)?,
        directory_status: decode_device_directory_status(decoder)?,
    };
    validate_directory_applied(&applied)?;
    Ok(applied)
}

fn encode_future_recipient_status(
    output: &mut Vec<u8>,
    value: CanonicalRuntimeIpcFutureRecipientStatusV1,
) {
    push_unsigned(
        output,
        match value {
            CanonicalRuntimeIpcFutureRecipientStatusV1::RemovedDevicesExcludedFromFutureRecipientSets => {
                STATUS_REMOVED_DEVICES_EXCLUDED
            }
        },
    );
}

fn decode_future_recipient_status(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcFutureRecipientStatusV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-future-recipient-status-v1")? {
        STATUS_REMOVED_DEVICES_EXCLUDED => {
            Ok(CanonicalRuntimeIpcFutureRecipientStatusV1::RemovedDevicesExcludedFromFutureRecipientSets)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_preexisting_recipient_status(
    output: &mut Vec<u8>,
    value: CanonicalRuntimeIpcPreexistingRecipientStatusV1,
) {
    push_unsigned(
        output,
        match value {
            CanonicalRuntimeIpcPreexistingRecipientStatusV1::ImmutableCannotBeRemotelyRewrittenOrErased => {
                STATUS_IMMUTABLE_PREEXISTING_RECIPIENTS
            }
        },
    );
}

fn decode_preexisting_recipient_status(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcPreexistingRecipientStatusV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-preexisting-recipient-status-v1")? {
        STATUS_IMMUTABLE_PREEXISTING_RECIPIENTS => Ok(
            CanonicalRuntimeIpcPreexistingRecipientStatusV1::ImmutableCannotBeRemotelyRewrittenOrErased,
        ),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_history_availability_status(
    output: &mut Vec<u8>,
    value: CanonicalRuntimeIpcHistoryAvailabilityStatusV1,
) {
    push_unsigned(
        output,
        match value {
            CanonicalRuntimeIpcHistoryAvailabilityStatusV1::ExistingCopiesRemainReadable => {
                STATUS_EXISTING_COPIES_REMAIN_READABLE
            }
        },
    );
}

fn decode_history_availability_status(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcHistoryAvailabilityStatusV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-history-availability-status-v1")? {
        STATUS_EXISTING_COPIES_REMAIN_READABLE => {
            Ok(CanonicalRuntimeIpcHistoryAvailabilityStatusV1::ExistingCopiesRemainReadable)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_bool(output: &mut Vec<u8>, value: bool) {
    push_unsigned(output, u64::from(value));
}

fn decode_bool(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
) -> Result<bool, CanonicalRuntimeIpcError> {
    match decoder.unsigned(field)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn validate_text(
    bytes: &[u8],
    minimum: usize,
    maximum: usize,
    error: CanonicalRuntimeIpcError,
) -> Result<(), CanonicalRuntimeIpcError> {
    if !(minimum..=maximum).contains(&bytes.len())
        || bytes.contains(&0)
        || std::str::from_utf8(bytes).is_err()
    {
        return Err(error);
    }
    Ok(())
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
    verifying_key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
) -> Result<(), CanonicalRuntimeIpcError> {
    let key = VerifyingKey::from_bytes(verifying_key)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidPublicKey)?;
    let signature = Signature::try_from(signature.as_slice())
        .map_err(|_| CanonicalRuntimeIpcError::InvalidSignature)?;
    key.verify_strict(message, &signature)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidSignature)
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::Signer;
    use serde_json::Value;

    use super::*;
    use crate::{
        canonical_account_v1::{
            CanonicalAccountDeviceListContentV1, account_device_list_signing_bytes_v1,
        },
        canonical_authority_v1::{
            CanonicalAccountAuthoritySnapshotContentV1, CanonicalDeviceCertificateContentV1,
            CanonicalDeviceRevocationContentV1, CanonicalSignedAccountAuthoritySnapshotV1,
            CanonicalSignedDeviceCertificateV1, CanonicalSignedDeviceRevocationV1,
            account_authority_snapshot_signing_bytes_v1, device_certificate_signing_bytes_v1,
            device_revocation_signing_bytes_v1,
        },
        canonical_runtime_ipc_contact_v1::{
            CanonicalRuntimeIpcProfileConvergenceV1, CanonicalRuntimeIpcRestartRecoveryV1,
        },
        canonical_runtime_ipc_v1::decode_and_verify_signed_runtime_ipc_descriptor_v1,
    };

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    fn json_documents() -> TestResult<(Value, Value, Value)> {
        Ok((
            serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../CANONICAL-RUNTIME-IPC-VECTORS.json"
            )))?,
            serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../CANONICAL-ACCOUNT-RECOVERY-POLICY-VECTORS.json"
            )))?,
            serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../CANONICAL-ACCOUNT-VECTORS.json"
            )))?,
        ))
    }

    fn bytes(document: &Value, pointer: &str) -> TestResult<Vec<u8>> {
        let value = document
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("missing vector pointer {pointer}"))?;
        if value.len() % 2 != 0 {
            return Err(format!("vector pointer {pointer} contains odd-length hex").into());
        }
        Ok((0..value.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&value[index..index + 2], 16))
            .collect::<Result<Vec<_>, _>>()?)
    }

    fn fixed<const N: usize>(document: &Value, pointer: &str) -> TestResult<[u8; N]> {
        Ok(bytes(document, pointer)?
            .try_into()
            .map_err(|value: Vec<u8>| {
                format!("vector pointer {pointer} has {} bytes", value.len())
            })?)
    }

    fn sign_revocation(
        root: &SigningKey,
        account_id: [u8; 32],
        device_id: [u8; 32],
        authority_sequence: u64,
    ) -> CanonicalSignedDeviceRevocationV1 {
        let content = CanonicalDeviceRevocationContentV1 {
            account_id,
            device_id,
            authority_sequence,
        };
        let signature = root
            .sign(&device_revocation_signing_bytes_v1(&content))
            .to_bytes();
        CanonicalSignedDeviceRevocationV1 { content, signature }
    }

    fn sign_certificate(
        root: &SigningKey,
        content: CanonicalDeviceCertificateContentV1,
    ) -> TestResult<CanonicalSignedDeviceCertificateV1> {
        let signature = root
            .sign(&device_certificate_signing_bytes_v1(&content)?)
            .to_bytes();
        Ok(CanonicalSignedDeviceCertificateV1 { content, signature })
    }

    fn sign_list(
        root: &SigningKey,
        current: &CanonicalSignedAccountDeviceListV1,
        revision: u64,
        mut revocations: Vec<CanonicalSignedDeviceRevocationV1>,
        mut devices: Vec<CanonicalSignedDeviceCertificateV1>,
    ) -> TestResult<CanonicalSignedAccountDeviceListV1> {
        revocations.sort_by_key(|revocation| revocation.content.device_id);
        devices.sort_by_key(|certificate| certificate.content.device_id);
        let snapshot_content = CanonicalAccountAuthoritySnapshotContentV1 {
            account_id: current.content.authority_snapshot.content.account_id,
            revision,
            revocations,
        };
        let snapshot_signature = root
            .sign(&account_authority_snapshot_signing_bytes_v1(
                &snapshot_content,
            )?)
            .to_bytes();
        let authority_snapshot = CanonicalSignedAccountAuthoritySnapshotV1 {
            content: snapshot_content,
            signature: snapshot_signature,
        };
        let content = CanonicalAccountDeviceListContentV1 {
            authority_snapshot,
            devices,
        };
        let signature = root
            .sign(&account_device_list_signing_bytes_v1(&content)?)
            .to_bytes();
        Ok(CanonicalSignedAccountDeviceListV1 { content, signature })
    }

    struct Fixture {
        descriptor: CanonicalSignedRuntimeIpcDescriptorV1,
        device_key: SigningKey,
        root_key: SigningKey,
        current: CanonicalSignedAccountDeviceListV1,
        candidate: CanonicalSignedAccountDeviceListV1,
        removed_device_id: [u8; 32],
    }

    fn fixture() -> TestResult<Fixture> {
        let (core, recovery, account) = json_documents()?;
        let descriptor = decode_and_verify_signed_runtime_ipc_descriptor_v1(&bytes(
            &core,
            "/expected/descriptor/signed_cbor_hex",
        )?)?;
        let device_key = SigningKey::from_bytes(&fixed(&core, "/input/device_secret_key_hex")?);
        let root_key = SigningKey::from_bytes(&fixed(&account, "/input/root_secret_key_hex")?);
        let current = decode_and_verify_signed_account_device_list_v1(&bytes(
            &recovery,
            "/expected/dependencies/new_account_device_list/signed_cbor_hex",
        )?)?;
        let removed_device_id = current.content.devices[1].content.device_id;
        let mut revocations = current
            .content
            .authority_snapshot
            .content
            .revocations
            .clone();
        revocations.push(sign_revocation(
            &root_key,
            descriptor.content.account_id,
            removed_device_id,
            current.content.authority_snapshot.content.revision,
        ));
        let candidate = sign_list(
            &root_key,
            &current,
            current.content.authority_snapshot.content.revision + 1,
            revocations,
            vec![current.content.devices[0].clone()],
        )?;
        Ok(Fixture {
            descriptor,
            device_key,
            root_key,
            current,
            candidate,
            removed_device_id,
        })
    }

    fn request(fixture: &Fixture) -> TestResult<CanonicalRuntimeIpcDirectoryRequestV1> {
        Ok(CanonicalRuntimeIpcDirectoryRequestV1 {
            descriptor_id: runtime_ipc_descriptor_id_v1(&fixture.descriptor)?,
            request_id: [0x41; 32],
            bearer_token: fixture.descriptor.content.bearer_token,
            issued_at_unix_seconds: 1_800_002_020,
            expires_at_unix_seconds: 1_800_002_050,
            command: CanonicalRuntimeIpcDirectoryCommandV1::ApplyOwnDeviceDirectory {
                device_list_file_utf8: b"C:\\Kilogram\\state\\active-devices-v11.bin".to_vec(),
                expected_current_authority_revision: fixture
                    .current
                    .content
                    .authority_snapshot
                    .content
                    .revision,
                expected_current_device_list_digest: runtime_ipc_device_list_digest_v1(
                    &fixture.current,
                )?,
                candidate_device_list: fixture.candidate.clone(),
            },
        })
    }

    fn applied_result(fixture: &Fixture) -> TestResult<CanonicalRuntimeIpcDirectoryResultV1> {
        let candidate_digest = runtime_ipc_device_list_digest_v1(&fixture.candidate)?;
        Ok(
            CanonicalRuntimeIpcDirectoryResultV1::OwnDeviceDirectoryApplied(
                Box::new(CanonicalRuntimeIpcDeviceDirectoryAppliedV1 {
                    account_id: fixture.descriptor.content.account_id,
                    local_device_id: fixture.descriptor.content.device_id,
                    previous_authority_revision: fixture
                        .current
                        .content
                        .authority_snapshot
                        .content
                        .revision,
                    authority_revision: fixture
                        .candidate
                        .content
                        .authority_snapshot
                        .content
                        .revision,
                    previous_device_list_digest: runtime_ipc_device_list_digest_v1(
                        &fixture.current,
                    )?,
                    device_list_digest: candidate_digest,
                    active_device_count: 1,
                    removed_device_ids: vec![fixture.removed_device_id],
                    ratchet_session_records_retired: 1,
                    prekey_observations_retired: 1,
                    pending_unmaterialized_messages: 2,
                    pending_materialized_messages: 3,
                    ticket_published: true,
                    launch_profile_update_required: true,
                    future_recipient_status:
                        CanonicalRuntimeIpcFutureRecipientStatusV1::RemovedDevicesExcludedFromFutureRecipientSets,
                    preexisting_recipient_status:
                        CanonicalRuntimeIpcPreexistingRecipientStatusV1::ImmutableCannotBeRemotelyRewrittenOrErased,
                    history_availability_status:
                        CanonicalRuntimeIpcHistoryAvailabilityStatusV1::ExistingCopiesRemainReadable,
                    directory_status: CanonicalRuntimeIpcDeviceDirectoryStatusV1 {
                        account_id: fixture.descriptor.content.account_id,
                        local_device_id: fixture.descriptor.content.device_id,
                        state_directory_utf8: b"C:\\Kilogram\\state".to_vec(),
                        authority_revision: fixture
                            .candidate
                            .content
                            .authority_snapshot
                            .content
                            .revision,
                        active_device_count: 1,
                        receipt_id: Some([0x90; 32]),
                        receipt_generation: Some(4),
                        device_list_digest: candidate_digest,
                        applied_device_list_file_utf8:
                            b"C:\\Kilogram\\state\\active-devices-v11.bin".to_vec(),
                        launch_device_list_file_utf8:
                            b"C:\\Kilogram\\profile\\launch-devices.bin".to_vec(),
                        profile_convergence: CanonicalRuntimeIpcProfileConvergenceV1::Required,
                        restart_recovery:
                            CanonicalRuntimeIpcRestartRecoveryV1::AuthenticatedReceiptCurrent,
                    },
                }),
            ),
        )
    }

    #[test]
    fn canonical_runtime_ipc_directory_round_trips_exact_transition() -> TestResult {
        let fixture = fixture()?;
        let request = request(&fixture)?;
        let encoded_request = encode_runtime_ipc_directory_request_v1(&request)?;
        assert_eq!(
            decode_and_verify_runtime_ipc_directory_request_v1(&encoded_request)?,
            request
        );
        verify_runtime_ipc_directory_request_for_descriptor_v1(
            &request,
            &fixture.descriptor,
            &fixture.current,
            request.issued_at_unix_seconds,
        )?;
        assert_eq!(
            verify_runtime_ipc_device_directory_transition_v1(
                &fixture.current,
                &fixture.candidate,
                &fixture.descriptor.content.device_id,
            )?,
            vec![fixture.removed_device_id]
        );
        let response = sign_runtime_ipc_directory_response_v1(
            &fixture.device_key,
            &request,
            &fixture.descriptor,
            &fixture.current,
            request.issued_at_unix_seconds,
            applied_result(&fixture)?,
        )?;
        let encoded_response = encode_signed_runtime_ipc_directory_response_v1(&response)?;
        assert_eq!(
            decode_and_verify_signed_runtime_ipc_directory_response_v1(&encoded_response)?,
            response
        );
        verify_runtime_ipc_directory_response_for_request_v1(
            &response,
            &request,
            &fixture.descriptor,
            &fixture.current,
        )?;
        assert_ne!(runtime_ipc_directory_response_id_v1(&response)?, [0; 32]);
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_directory_distinguishes_expired_request() -> TestResult {
        let fixture = fixture()?;
        let request = request(&fixture)?;
        assert!(matches!(
            verify_runtime_ipc_directory_request_for_descriptor_v1(
                &request,
                &fixture.descriptor,
                &fixture.current,
                request.expires_at_unix_seconds + 1,
            ),
            Err(CanonicalRuntimeIpcError::RequestNotCurrent)
        ));
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_directory_vectors_match() -> TestResult {
        let fixture = fixture()?;
        let request = request(&fixture)?;
        let response = sign_runtime_ipc_directory_response_v1(
            &fixture.device_key,
            &request,
            &fixture.descriptor,
            &fixture.current,
            request.issued_at_unix_seconds,
            applied_result(&fixture)?,
        )?;
        let vectors: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-RUNTIME-IPC-DIRECTORY-VECTORS.json"
        )))?;
        assert_eq!(
            encode_signed_account_device_list_v1(&fixture.current)?,
            bytes(&vectors, "/expected/current_device_list/signed_cbor_hex")?
        );
        assert_eq!(
            encode_signed_account_device_list_v1(&fixture.candidate)?,
            bytes(&vectors, "/expected/candidate_device_list/signed_cbor_hex")?
        );
        assert_eq!(
            encode_runtime_ipc_directory_request_v1(&request)?,
            bytes(&vectors, "/expected/request/cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_directory_request_digest_v1(&request)?,
            fixed(&vectors, "/expected/request/object_id_hex")?
        );
        assert_eq!(
            encode_signed_runtime_ipc_directory_response_v1(&response)?,
            bytes(&vectors, "/expected/response/signed_cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_directory_response_id_v1(&response)?,
            fixed(&vectors, "/expected/response/object_id_hex")?
        );
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_directory_rejects_unrevoked_and_local_removal() -> TestResult {
        let fixture = fixture()?;
        let revision = fixture.current.content.authority_snapshot.content.revision + 1;
        let old_revocations = fixture
            .current
            .content
            .authority_snapshot
            .content
            .revocations
            .clone();
        let unrevoked = sign_list(
            &fixture.root_key,
            &fixture.current,
            revision,
            old_revocations.clone(),
            vec![fixture.current.content.devices[0].clone()],
        )?;
        assert!(
            verify_runtime_ipc_device_directory_transition_v1(
                &fixture.current,
                &unrevoked,
                &fixture.descriptor.content.device_id,
            )
            .is_err()
        );

        let local_revocation = sign_revocation(
            &fixture.root_key,
            fixture.descriptor.content.account_id,
            fixture.descriptor.content.device_id,
            fixture.current.content.authority_snapshot.content.revision,
        );
        let mut revocations = old_revocations;
        revocations.push(local_revocation);
        let local_removed = sign_list(
            &fixture.root_key,
            &fixture.current,
            revision,
            revocations,
            vec![fixture.current.content.devices[1].clone()],
        )?;
        assert!(
            verify_runtime_ipc_device_directory_transition_v1(
                &fixture.current,
                &local_removed,
                &fixture.descriptor.content.device_id,
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_directory_rejects_added_or_replaced_device() -> TestResult {
        let fixture = fixture()?;
        let revision = fixture.current.content.authority_snapshot.content.revision + 1;
        let mut revocations = fixture
            .current
            .content
            .authority_snapshot
            .content
            .revocations
            .clone();
        revocations.push(sign_revocation(
            &fixture.root_key,
            fixture.descriptor.content.account_id,
            fixture.removed_device_id,
            fixture.current.content.authority_snapshot.content.revision,
        ));
        let added_key = SigningKey::from_bytes(&[0x71; 32]);
        let added = sign_certificate(
            &fixture.root_key,
            CanonicalDeviceCertificateContentV1 {
                account_id: fixture.descriptor.content.account_id,
                device_id: added_key.verifying_key().to_bytes(),
                encryption_public_key: [0x72; 32],
                authority_sequence: fixture.current.content.authority_snapshot.content.revision,
                capabilities: fixture.current.content.devices[0]
                    .content
                    .capabilities
                    .clone(),
            },
        )?;
        let with_added = sign_list(
            &fixture.root_key,
            &fixture.current,
            revision,
            revocations.clone(),
            vec![fixture.current.content.devices[0].clone(), added],
        )?;
        assert!(
            verify_runtime_ipc_device_directory_transition_v1(
                &fixture.current,
                &with_added,
                &fixture.descriptor.content.device_id,
            )
            .is_err()
        );

        let mut replacement_content = fixture.current.content.devices[0].content.clone();
        replacement_content.encryption_public_key = [0x73; 32];
        let replacement = sign_certificate(&fixture.root_key, replacement_content)?;
        let with_replacement = sign_list(
            &fixture.root_key,
            &fixture.current,
            revision,
            revocations,
            vec![replacement],
        )?;
        assert!(
            verify_runtime_ipc_device_directory_transition_v1(
                &fixture.current,
                &with_replacement,
                &fixture.descriptor.content.device_id,
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_directory_response_substitutions_fail_closed() -> TestResult {
        let fixture = fixture()?;
        let request = request(&fixture)?;
        let response = sign_runtime_ipc_directory_response_v1(
            &fixture.device_key,
            &request,
            &fixture.descriptor,
            &fixture.current,
            request.issued_at_unix_seconds,
            applied_result(&fixture)?,
        )?;
        let mut changed = response.clone();
        changed.content.request_id[0] ^= 1;
        assert!(verify_signed_runtime_ipc_directory_response_v1(&changed).is_err());

        let mut false_idempotent = match applied_result(&fixture)? {
            CanonicalRuntimeIpcDirectoryResultV1::OwnDeviceDirectoryApplied(applied) => applied,
            CanonicalRuntimeIpcDirectoryResultV1::Error { .. } => {
                return Err("fixture result was not own-device-directory-applied".into());
            }
        };
        false_idempotent.previous_authority_revision = false_idempotent.authority_revision;
        false_idempotent.previous_device_list_digest = false_idempotent.device_list_digest;
        false_idempotent.removed_device_ids.clear();
        assert!(matches!(
            validate_directory_applied(&false_idempotent),
            Err(CanonicalRuntimeIpcError::InvalidResponse)
        ));
        false_idempotent.ratchet_session_records_retired = 0;
        false_idempotent.prekey_observations_retired = 0;
        false_idempotent.ticket_published = false;
        validate_directory_applied(&false_idempotent)?;

        let mut changed_result = applied_result(&fixture)?;
        let CanonicalRuntimeIpcDirectoryResultV1::OwnDeviceDirectoryApplied(applied) =
            &mut changed_result
        else {
            return Err("fixture result was not own-device-directory-applied".into());
        };
        applied.removed_device_ids.clear();
        assert!(
            sign_runtime_ipc_directory_response_v1(
                &fixture.device_key,
                &request,
                &fixture.descriptor,
                &fixture.current,
                request.issued_at_unix_seconds,
                changed_result,
            )
            .is_err()
        );

        let mut stale_request = request.clone();
        let CanonicalRuntimeIpcDirectoryCommandV1::ApplyOwnDeviceDirectory {
            expected_current_authority_revision,
            ..
        } = &mut stale_request.command;
        *expected_current_authority_revision -= 1;
        let stale_content = CanonicalRuntimeIpcDirectoryResponseContentV1 {
            descriptor_id: stale_request.descriptor_id,
            request_id: stale_request.request_id,
            request_digest: runtime_ipc_directory_request_digest_v1(&stale_request)?,
            responder_device_id: fixture.descriptor.content.device_id,
            result: applied_result(&fixture)?,
        };
        let stale_response = CanonicalSignedRuntimeIpcDirectoryResponseV1 {
            signature: fixture
                .device_key
                .sign(&runtime_ipc_directory_response_signing_bytes_v1(
                    &stale_content,
                )?)
                .to_bytes(),
            content: stale_content,
        };
        assert!(
            verify_runtime_ipc_directory_response_for_request_v1(
                &stale_response,
                &stale_request,
                &fixture.descriptor,
                &fixture.current,
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
    fn regenerate_canonical_runtime_ipc_directory_vectors() -> TestResult {
        let fixture = fixture()?;
        let request = request(&fixture)?;
        let request_cbor = encode_runtime_ipc_directory_request_v1(&request)?;
        let request_digest = runtime_ipc_directory_request_digest_v1(&request)?;
        let response = sign_runtime_ipc_directory_response_v1(
            &fixture.device_key,
            &request,
            &fixture.descriptor,
            &fixture.current,
            request.issued_at_unix_seconds,
            applied_result(&fixture)?,
        )?;
        let response_content = encode_directory_response_content(&response.content)?;
        let response_cbor = encode_signed_runtime_ipc_directory_response_v1(&response)?;
        let response_id = runtime_ipc_directory_response_id_v1(&response)?;
        let current_cbor = encode_signed_account_device_list_v1(&fixture.current)?;
        let candidate_cbor = encode_signed_account_device_list_v1(&fixture.candidate)?;
        let current_digest = runtime_ipc_device_list_digest_v1(&fixture.current)?;
        let candidate_digest = runtime_ipc_device_list_digest_v1(&fixture.candidate)?;
        let path = match &request.command {
            CanonicalRuntimeIpcDirectoryCommandV1::ApplyOwnDeviceDirectory {
                device_list_file_utf8,
                ..
            } => String::from_utf8(device_list_file_utf8.clone())?,
        };
        let document = serde_json::json!({
            "schema_version": 1,
            "profile": "kilogram-deterministic-cbor-v1",
            "activation_status": "not-active",
            "dependencies": [
                "CANONICAL-RUNTIME-IPC-VECTORS.json",
                "CANONICAL-ACCOUNT-RECOVERY-POLICY-VECTORS.json",
                "CANONICAL-ACCOUNT-VECTORS.json",
                "CANONICAL-AUTHORITY-VECTORS.json"
            ],
            "input": {
                "request_issued_at_unix_seconds": request.issued_at_unix_seconds,
                "request_expires_at_unix_seconds": request.expires_at_unix_seconds,
                "request_id_hex": lower_hex(&request.request_id),
                "device_list_file": path,
                "ratchet_session_records_retired": 1,
                "prekey_observations_retired": 1,
                "pending_unmaterialized_messages": 2,
                "pending_materialized_messages": 3,
                "ticket_published": true,
                "launch_profile_update_required": true,
                "receipt_id_hex": lower_hex(&[0x90; 32]),
                "receipt_generation": 4,
                "state_directory": "C:\\Kilogram\\state",
                "launch_device_list_file": "C:\\Kilogram\\profile\\launch-devices.bin"
            },
            "expected": {
                "account_id_hex": lower_hex(&fixture.descriptor.content.account_id),
                "local_device_id_hex": lower_hex(&fixture.descriptor.content.device_id),
                "removed_device_id_hex": lower_hex(&fixture.removed_device_id),
                "previous_authority_revision": fixture.current.content.authority_snapshot.content.revision,
                "authority_revision": fixture.candidate.content.authority_snapshot.content.revision,
                "active_device_count": fixture.candidate.content.devices.len(),
                "current_device_list": {
                    "signed_cbor_hex": lower_hex(&current_cbor),
                    "digest_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_DEVICE_LIST_DIGEST_DOMAIN),
                    "digest_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_DEVICE_LIST_DIGEST_DOMAIN, &current_cbor)),
                    "digest_hex": lower_hex(&current_digest)
                },
                "candidate_device_list": {
                    "signed_cbor_hex": lower_hex(&candidate_cbor),
                    "digest_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_DEVICE_LIST_DIGEST_DOMAIN),
                    "digest_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_DEVICE_LIST_DIGEST_DOMAIN, &candidate_cbor)),
                    "digest_hex": lower_hex(&candidate_digest)
                },
                "request": {
                    "cbor_hex": lower_hex(&request_cbor),
                    "digest_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_DIRECTORY_REQUEST_DIGEST_DOMAIN),
                    "digest_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_DIRECTORY_REQUEST_DIGEST_DOMAIN, &request_cbor)),
                    "object_id_hex": lower_hex(&request_digest)
                },
                "response": {
                    "content_cbor_hex": lower_hex(&response_content),
                    "signature_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_DIRECTORY_RESPONSE_SIGNATURE_DOMAIN),
                    "signing_preimage_hex": lower_hex(&runtime_ipc_directory_response_signing_bytes_v1(&response.content)?),
                    "signature_hex": lower_hex(&response.signature),
                    "signed_cbor_hex": lower_hex(&response_cbor),
                    "id_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_DIRECTORY_RESPONSE_ID_DOMAIN, &response_cbor)),
                    "object_id_hex": lower_hex(&response_id)
                },
                "context": {
                    "valid_at_unix_seconds": request.issued_at_unix_seconds,
                    "command_result_pair": "apply-own-device-directory/own-device-directory-applied",
                    "exact_current_anchor_required": true,
                    "candidate_embedded_in_request": true,
                    "only_existing_root_revoked_devices_may_disappear": true,
                    "running_device_must_remain_byte_identical": true,
                    "active_runtime_changed": false
                }
            }
        });
        let output = serde_json::to_string_pretty(&document)? + "\n";
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../CANONICAL-RUNTIME-IPC-DIRECTORY-VECTORS.json");
        std::fs::write(path, output)?;
        Ok(())
    }
}
