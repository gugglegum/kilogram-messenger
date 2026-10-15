//! Inactive deterministic-CBOR contact and device-directory status model for
//! the local runtime IPC boundary.
//!
//! The active runtime remains Postcard IPC version 26. This candidate covers
//! contact enrollment and the read-only own-device-directory status command;
//! applying a new authority roster is deliberately a separate boundary.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::{
    ConversationId,
    canonical_account_v1::{AccountDecoder, signed_envelope},
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

pub const MAX_CANONICAL_RUNTIME_IPC_CONTACT_RESPONSE_BYTES: usize = 128 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_CONTACT_TEXT_BYTES: usize = 4 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_CONTACT_PATH_BYTES: usize = 4 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_CONTACT_ENDPOINTS: u8 = 4;
pub const MAX_CANONICAL_RUNTIME_IPC_ACCOUNT_DEVICES: u16 = 256;

pub const CANONICAL_RUNTIME_IPC_CONTACT_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-contact-request-digest:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_CONTACT_RESPONSE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-contact-response-signature:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_CONTACT_RESPONSE_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-contact-response-id:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_CONTACT_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-contact-id:cbor-v1\0";

const COMMAND_ADD_CONTACT: u64 = 0;
const COMMAND_OWN_DEVICE_DIRECTORY_STATUS: u64 = 1;
const RESULT_CONTACT_ADDED: u64 = 0;
const RESULT_OWN_DEVICE_DIRECTORY_STATUS: u64 = 1;
const RESULT_ERROR: u64 = 2;
const OPTION_NONE: u64 = 0;
const OPTION_SOME: u64 = 1;
const PROFILE_CURRENT: u64 = 0;
const PROFILE_CONVERGENCE_REQUIRED: u64 = 1;
const RESTART_AUTHENTICATED_RECEIPT_CURRENT: u64 = 0;
const RESTART_AUTHENTICATED_RECEIPT_RECOVERED: u64 = 1;
const RESTART_LAUNCH_PROFILE_DIRECTORY: u64 = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcContactCommandV1 {
    AddContact {
        conversation_label_utf8: Vec<u8>,
        expected_peer_account_id: [u8; 32],
        descriptor_file_utf8: Vec<u8>,
    },
    OwnDeviceDirectoryStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcContactRequestV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub bearer_token: [u8; 32],
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub command: CanonicalRuntimeIpcContactCommandV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcProfileConvergenceV1 {
    Current,
    Required,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcRestartRecoveryV1 {
    AuthenticatedReceiptCurrent,
    AuthenticatedReceiptRecovered,
    LaunchProfileDirectory,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcDeviceDirectoryStatusV1 {
    pub account_id: [u8; 32],
    pub local_device_id: [u8; 32],
    pub state_directory_utf8: Vec<u8>,
    pub authority_revision: u64,
    pub active_device_count: u16,
    pub receipt_id: Option<[u8; 32]>,
    pub receipt_generation: Option<u64>,
    pub device_list_digest: [u8; 32],
    pub applied_device_list_file_utf8: Vec<u8>,
    pub launch_device_list_file_utf8: Vec<u8>,
    pub profile_convergence: CanonicalRuntimeIpcProfileConvergenceV1,
    pub restart_recovery: CanonicalRuntimeIpcRestartRecoveryV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcContactResultV1 {
    ContactAdded {
        contact_id: [u8; 32],
        peer_account_id: [u8; 32],
        peer_device_id: [u8; 32],
        endpoint_candidate_count: u8,
        endpoint_candidate_added: bool,
        inserted: bool,
    },
    OwnDeviceDirectoryStatus(CanonicalRuntimeIpcDeviceDirectoryStatusV1),
    Error {
        code: u16,
        message_utf8: Vec<u8>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcContactResponseContentV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub request_digest: [u8; 32],
    pub responder_device_id: [u8; 32],
    pub result: CanonicalRuntimeIpcContactResultV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeIpcContactResponseV1 {
    pub content: CanonicalRuntimeIpcContactResponseContentV1,
    pub signature: [u8; 64],
}

pub fn encode_runtime_ipc_contact_request_v1(
    request: &CanonicalRuntimeIpcContactRequestV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_contact_request(request)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 7);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &request.descriptor_id);
    push_bytes(&mut output, &request.request_id);
    push_bytes(&mut output, &request.bearer_token);
    push_unsigned(&mut output, request.issued_at_unix_seconds);
    push_unsigned(&mut output, request.expires_at_unix_seconds);
    encode_contact_command(&mut output, &request.command);
    ensure_bound(
        &output,
        "contact request",
        MAX_CANONICAL_RUNTIME_IPC_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_runtime_ipc_contact_request_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcContactRequestV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "contact request",
        MAX_CANONICAL_RUNTIME_IPC_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("runtime-ipc-contact-request-v1", 7)?;
    decoder.expect_version("runtime-ipc-contact-request-v1.schema-version")?;
    let request = CanonicalRuntimeIpcContactRequestV1 {
        descriptor_id: decoder.fixed_bytes("runtime-ipc-contact-request-v1.descriptor-id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-contact-request-v1.request-id")?,
        bearer_token: decoder.fixed_bytes("runtime-ipc-contact-request-v1.bearer-token")?,
        issued_at_unix_seconds: decoder
            .unsigned("runtime-ipc-contact-request-v1.issued-at-unix-seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("runtime-ipc-contact-request-v1.expires-at-unix-seconds")?,
        command: decode_contact_command(&mut decoder)?,
    };
    decoder.finish()?;
    validate_contact_request(&request)?;
    Ok(request)
}

pub fn runtime_ipc_contact_request_digest_v1(
    request: &CanonicalRuntimeIpcContactRequestV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_CONTACT_REQUEST_DIGEST_DOMAIN,
        &encode_runtime_ipc_contact_request_v1(request)?,
    ))
}

pub fn canonical_runtime_ipc_contact_id_v1(
    local_account_id: &[u8; 32],
    peer_account_id: &[u8; 32],
    conversation_label_utf8: &[u8],
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    validate_text(
        conversation_label_utf8,
        1,
        MAX_CANONICAL_RUNTIME_IPC_CONTACT_TEXT_BYTES,
        CanonicalRuntimeIpcError::InvalidRequest,
    )?;
    if *local_account_id == [0; 32]
        || *peer_account_id == [0; 32]
        || local_account_id == peer_account_id
    {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    let label = std::str::from_utf8(conversation_label_utf8)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?;
    let conversation_id = ConversationId::from_label(label);
    let mut content = Vec::new();
    push_array_header(&mut content, 4);
    push_unsigned(&mut content, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut content, local_account_id);
    push_bytes(&mut content, peer_account_id);
    push_bytes(&mut content, conversation_id.as_bytes());
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_CONTACT_ID_DOMAIN,
        &content,
    ))
}

pub fn verify_runtime_ipc_contact_request_for_descriptor_v1(
    request: &CanonicalRuntimeIpcContactRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_runtime_ipc_descriptor_at_v1(descriptor, now_unix_seconds)?;
    validate_contact_request(request)?;
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
    if let CanonicalRuntimeIpcContactCommandV1::AddContact {
        expected_peer_account_id,
        ..
    } = &request.command
        && expected_peer_account_id == &descriptor.content.account_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    Ok(())
}

pub fn runtime_ipc_contact_response_signing_bytes_v1(
    content: &CanonicalRuntimeIpcContactResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    Ok(prefixed(
        CANONICAL_RUNTIME_IPC_CONTACT_RESPONSE_SIGNATURE_DOMAIN,
        &encode_contact_response_content(content)?,
    ))
}

pub fn sign_runtime_ipc_contact_response_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalRuntimeIpcContactRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
    result: CanonicalRuntimeIpcContactResultV1,
) -> Result<CanonicalSignedRuntimeIpcContactResponseV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_contact_request_for_descriptor_v1(request, descriptor, now_unix_seconds)?;
    if device_signing_key.verifying_key().to_bytes() != descriptor.content.device_id {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    let content = CanonicalRuntimeIpcContactResponseContentV1 {
        descriptor_id: request.descriptor_id,
        request_id: request.request_id,
        request_digest: runtime_ipc_contact_request_digest_v1(request)?,
        responder_device_id: descriptor.content.device_id,
        result,
    };
    let signature = device_signing_key
        .sign(&runtime_ipc_contact_response_signing_bytes_v1(&content)?)
        .to_bytes();
    let response = CanonicalSignedRuntimeIpcContactResponseV1 { content, signature };
    verify_runtime_ipc_contact_response_for_request_v1(&response, request, descriptor)?;
    Ok(response)
}

pub fn encode_signed_runtime_ipc_contact_response_v1(
    response: &CanonicalSignedRuntimeIpcContactResponseV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_contact_response_v1(response)?;
    let output = signed_envelope(
        &encode_contact_response_content(&response.content)?,
        &response.signature,
    );
    ensure_bound(
        &output,
        "contact response",
        MAX_CANONICAL_RUNTIME_IPC_CONTACT_RESPONSE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_runtime_ipc_contact_response_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeIpcContactResponseV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "contact response",
        MAX_CANONICAL_RUNTIME_IPC_CONTACT_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-runtime-ipc-contact-response-v1", 3)?;
    decoder.expect_version("signed-runtime-ipc-contact-response-v1.schema-version")?;
    let content = decode_contact_response_content(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-runtime-ipc-contact-response-v1.signature")?;
    decoder.finish()?;
    let response = CanonicalSignedRuntimeIpcContactResponseV1 { content, signature };
    verify_signed_runtime_ipc_contact_response_v1(&response)?;
    Ok(response)
}

pub fn verify_signed_runtime_ipc_contact_response_v1(
    response: &CanonicalSignedRuntimeIpcContactResponseV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_contact_response_content(&response.content)?;
    verify_ed25519(
        &response.content.responder_device_id,
        &runtime_ipc_contact_response_signing_bytes_v1(&response.content)?,
        &response.signature,
    )
}

pub fn verify_runtime_ipc_contact_response_for_request_v1(
    response: &CanonicalSignedRuntimeIpcContactResponseV1,
    request: &CanonicalRuntimeIpcContactRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_contact_response_v1(response)?;
    validate_contact_request(request)?;
    verify_signed_runtime_ipc_descriptor_v1(descriptor)?;
    if response.content.descriptor_id != runtime_ipc_descriptor_id_v1(descriptor)?
        || response.content.descriptor_id != request.descriptor_id
        || response.content.request_id != request.request_id
        || response.content.request_digest != runtime_ipc_contact_request_digest_v1(request)?
        || response.content.responder_device_id != descriptor.content.device_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    match (&request.command, &response.content.result) {
        (_, CanonicalRuntimeIpcContactResultV1::Error { .. }) => Ok(()),
        (
            CanonicalRuntimeIpcContactCommandV1::AddContact {
                conversation_label_utf8,
                expected_peer_account_id,
                ..
            },
            CanonicalRuntimeIpcContactResultV1::ContactAdded {
                contact_id,
                peer_account_id,
                ..
            },
        ) => {
            if peer_account_id != expected_peer_account_id
                || *contact_id
                    != canonical_runtime_ipc_contact_id_v1(
                        &descriptor.content.account_id,
                        expected_peer_account_id,
                        conversation_label_utf8,
                    )?
            {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            Ok(())
        }
        (
            CanonicalRuntimeIpcContactCommandV1::OwnDeviceDirectoryStatus,
            CanonicalRuntimeIpcContactResultV1::OwnDeviceDirectoryStatus(status),
        ) => {
            if status.account_id != descriptor.content.account_id
                || status.local_device_id != descriptor.content.device_id
            {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            Ok(())
        }
        _ => Err(CanonicalRuntimeIpcError::ContextMismatch),
    }
}

pub fn runtime_ipc_contact_response_id_v1(
    response: &CanonicalSignedRuntimeIpcContactResponseV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_CONTACT_RESPONSE_ID_DOMAIN,
        &encode_signed_runtime_ipc_contact_response_v1(response)?,
    ))
}

fn encode_contact_response_content(
    content: &CanonicalRuntimeIpcContactResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_contact_response_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 6);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.descriptor_id);
    push_bytes(&mut output, &content.request_id);
    push_bytes(&mut output, &content.request_digest);
    push_bytes(&mut output, &content.responder_device_id);
    encode_contact_result(&mut output, &content.result);
    Ok(output)
}

fn decode_contact_response_content(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcContactResponseContentV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-contact-response-content-v1", 6)?;
    decoder.expect_version("runtime-ipc-contact-response-content-v1.schema-version")?;
    let content = CanonicalRuntimeIpcContactResponseContentV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-contact-response-content-v1.descriptor-id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-contact-response-content-v1.request-id")?,
        request_digest: decoder
            .fixed_bytes("runtime-ipc-contact-response-content-v1.request-digest")?,
        responder_device_id: decoder
            .fixed_bytes("runtime-ipc-contact-response-content-v1.responder-device-id")?,
        result: decode_contact_result(decoder)?,
    };
    validate_contact_response_content(&content)?;
    Ok(content)
}

fn validate_contact_request(
    request: &CanonicalRuntimeIpcContactRequestV1,
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
        CanonicalRuntimeIpcContactCommandV1::AddContact {
            conversation_label_utf8,
            expected_peer_account_id,
            descriptor_file_utf8,
        } => {
            validate_text(
                conversation_label_utf8,
                1,
                MAX_CANONICAL_RUNTIME_IPC_CONTACT_TEXT_BYTES,
                CanonicalRuntimeIpcError::InvalidRequest,
            )?;
            validate_text(
                descriptor_file_utf8,
                1,
                MAX_CANONICAL_RUNTIME_IPC_CONTACT_PATH_BYTES,
                CanonicalRuntimeIpcError::InvalidRequest,
            )?;
            if *expected_peer_account_id == [0; 32] {
                return Err(CanonicalRuntimeIpcError::InvalidRequest);
            }
        }
        CanonicalRuntimeIpcContactCommandV1::OwnDeviceDirectoryStatus => {}
    }
    Ok(())
}

fn validate_contact_response_content(
    response: &CanonicalRuntimeIpcContactResponseContentV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if response.descriptor_id == [0; 32]
        || response.request_id == [0; 32]
        || response.request_digest == [0; 32]
        || response.responder_device_id == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    match &response.result {
        CanonicalRuntimeIpcContactResultV1::ContactAdded {
            contact_id,
            peer_account_id,
            peer_device_id,
            endpoint_candidate_count,
            endpoint_candidate_added,
            inserted,
        } => {
            if *contact_id == [0; 32]
                || *peer_account_id == [0; 32]
                || *peer_device_id == [0; 32]
                || !(1..=MAX_CANONICAL_RUNTIME_IPC_CONTACT_ENDPOINTS)
                    .contains(endpoint_candidate_count)
                || (*inserted && !*endpoint_candidate_added)
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            Ok(())
        }
        CanonicalRuntimeIpcContactResultV1::OwnDeviceDirectoryStatus(status) => {
            validate_device_directory_status(status)
        }
        CanonicalRuntimeIpcContactResultV1::Error { code, message_utf8 } => {
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

fn validate_device_directory_status(
    status: &CanonicalRuntimeIpcDeviceDirectoryStatusV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    for path in [
        status.state_directory_utf8.as_slice(),
        status.applied_device_list_file_utf8.as_slice(),
        status.launch_device_list_file_utf8.as_slice(),
    ] {
        validate_text(
            path,
            1,
            MAX_CANONICAL_RUNTIME_IPC_CONTACT_PATH_BYTES,
            CanonicalRuntimeIpcError::InvalidResponse,
        )?;
    }
    let receipt_pair_valid = match (status.receipt_id, status.receipt_generation) {
        (None, None) => true,
        (Some(id), Some(generation)) => id != [0; 32] && generation > 0,
        _ => false,
    };
    let convergence_valid = match status.profile_convergence {
        CanonicalRuntimeIpcProfileConvergenceV1::Current => {
            status.applied_device_list_file_utf8 == status.launch_device_list_file_utf8
        }
        CanonicalRuntimeIpcProfileConvergenceV1::Required => {
            status.applied_device_list_file_utf8 != status.launch_device_list_file_utf8
        }
    };
    let restart_valid = match status.restart_recovery {
        CanonicalRuntimeIpcRestartRecoveryV1::AuthenticatedReceiptCurrent
        | CanonicalRuntimeIpcRestartRecoveryV1::AuthenticatedReceiptRecovered => {
            status.receipt_id.is_some()
        }
        CanonicalRuntimeIpcRestartRecoveryV1::LaunchProfileDirectory => status.receipt_id.is_none(),
    };
    if status.account_id == [0; 32]
        || status.local_device_id == [0; 32]
        || status.authority_revision == 0
        || !(1..=MAX_CANONICAL_RUNTIME_IPC_ACCOUNT_DEVICES).contains(&status.active_device_count)
        || status.device_list_digest == [0; 32]
        || !receipt_pair_valid
        || !convergence_valid
        || !restart_valid
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn encode_contact_command(output: &mut Vec<u8>, command: &CanonicalRuntimeIpcContactCommandV1) {
    match command {
        CanonicalRuntimeIpcContactCommandV1::AddContact {
            conversation_label_utf8,
            expected_peer_account_id,
            descriptor_file_utf8,
        } => {
            push_array_header(output, 4);
            push_unsigned(output, COMMAND_ADD_CONTACT);
            push_bytes(output, conversation_label_utf8);
            push_bytes(output, expected_peer_account_id);
            push_bytes(output, descriptor_file_utf8);
        }
        CanonicalRuntimeIpcContactCommandV1::OwnDeviceDirectoryStatus => {
            push_array_header(output, 1);
            push_unsigned(output, COMMAND_OWN_DEVICE_DIRECTORY_STATUS);
        }
    }
}

fn decode_contact_command(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcContactCommandV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-contact-command-v1")?;
    match (
        decoder.unsigned("runtime-ipc-contact-command-v1.variant")?,
        length,
    ) {
        (COMMAND_ADD_CONTACT, 4) => Ok(CanonicalRuntimeIpcContactCommandV1::AddContact {
            conversation_label_utf8: decoder
                .bytes("runtime-ipc-contact-command-v1.conversation-label-utf8")?
                .to_vec(),
            expected_peer_account_id: decoder
                .fixed_bytes("runtime-ipc-contact-command-v1.expected-peer-account-id")?,
            descriptor_file_utf8: decoder
                .bytes("runtime-ipc-contact-command-v1.descriptor-file-utf8")?
                .to_vec(),
        }),
        (COMMAND_OWN_DEVICE_DIRECTORY_STATUS, 1) => {
            Ok(CanonicalRuntimeIpcContactCommandV1::OwnDeviceDirectoryStatus)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_contact_result(output: &mut Vec<u8>, result: &CanonicalRuntimeIpcContactResultV1) {
    match result {
        CanonicalRuntimeIpcContactResultV1::ContactAdded {
            contact_id,
            peer_account_id,
            peer_device_id,
            endpoint_candidate_count,
            endpoint_candidate_added,
            inserted,
        } => {
            push_array_header(output, 7);
            push_unsigned(output, RESULT_CONTACT_ADDED);
            push_bytes(output, contact_id);
            push_bytes(output, peer_account_id);
            push_bytes(output, peer_device_id);
            push_unsigned(output, u64::from(*endpoint_candidate_count));
            encode_bool(output, *endpoint_candidate_added);
            encode_bool(output, *inserted);
        }
        CanonicalRuntimeIpcContactResultV1::OwnDeviceDirectoryStatus(status) => {
            push_array_header(output, 2);
            push_unsigned(output, RESULT_OWN_DEVICE_DIRECTORY_STATUS);
            encode_device_directory_status(output, status);
        }
        CanonicalRuntimeIpcContactResultV1::Error { code, message_utf8 } => {
            push_array_header(output, 3);
            push_unsigned(output, RESULT_ERROR);
            push_unsigned(output, u64::from(*code));
            push_bytes(output, message_utf8);
        }
    }
}

fn decode_contact_result(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcContactResultV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-contact-result-v1")?;
    match (
        decoder.unsigned("runtime-ipc-contact-result-v1.variant")?,
        length,
    ) {
        (RESULT_CONTACT_ADDED, 7) => Ok(CanonicalRuntimeIpcContactResultV1::ContactAdded {
            contact_id: decoder.fixed_bytes("runtime-ipc-contact-result-v1.contact-id")?,
            peer_account_id: decoder
                .fixed_bytes("runtime-ipc-contact-result-v1.peer-account-id")?,
            peer_device_id: decoder.fixed_bytes("runtime-ipc-contact-result-v1.peer-device-id")?,
            endpoint_candidate_count: u8::try_from(
                decoder.unsigned("runtime-ipc-contact-result-v1.endpoint-candidate-count")?,
            )
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            endpoint_candidate_added: decode_bool(
                decoder,
                "runtime-ipc-contact-result-v1.endpoint-candidate-added",
            )?,
            inserted: decode_bool(decoder, "runtime-ipc-contact-result-v1.inserted")?,
        }),
        (RESULT_OWN_DEVICE_DIRECTORY_STATUS, 2) => Ok(
            CanonicalRuntimeIpcContactResultV1::OwnDeviceDirectoryStatus(
                decode_device_directory_status(decoder)?,
            ),
        ),
        (RESULT_ERROR, 3) => Ok(CanonicalRuntimeIpcContactResultV1::Error {
            code: u16::try_from(decoder.unsigned("runtime-ipc-contact-result-v1.error-code")?)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            message_utf8: decoder
                .bytes("runtime-ipc-contact-result-v1.error-message-utf8")?
                .to_vec(),
        }),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_device_directory_status(
    output: &mut Vec<u8>,
    status: &CanonicalRuntimeIpcDeviceDirectoryStatusV1,
) {
    push_array_header(output, 13);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_bytes(output, &status.account_id);
    push_bytes(output, &status.local_device_id);
    push_bytes(output, &status.state_directory_utf8);
    push_unsigned(output, status.authority_revision);
    push_unsigned(output, u64::from(status.active_device_count));
    encode_optional_fixed(output, status.receipt_id.as_ref());
    encode_optional_u64(output, status.receipt_generation);
    push_bytes(output, &status.device_list_digest);
    push_bytes(output, &status.applied_device_list_file_utf8);
    push_bytes(output, &status.launch_device_list_file_utf8);
    encode_profile_convergence(output, status.profile_convergence);
    encode_restart_recovery(output, status.restart_recovery);
}

fn decode_device_directory_status(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcDeviceDirectoryStatusV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-device-directory-status-v1", 13)?;
    decoder.expect_version("runtime-ipc-device-directory-status-v1.schema-version")?;
    let status = CanonicalRuntimeIpcDeviceDirectoryStatusV1 {
        account_id: decoder.fixed_bytes("runtime-ipc-device-directory-status-v1.account-id")?,
        local_device_id: decoder
            .fixed_bytes("runtime-ipc-device-directory-status-v1.local-device-id")?,
        state_directory_utf8: decoder
            .bytes("runtime-ipc-device-directory-status-v1.state-directory-utf8")?
            .to_vec(),
        authority_revision: decoder
            .unsigned("runtime-ipc-device-directory-status-v1.authority-revision")?,
        active_device_count: u16::try_from(
            decoder.unsigned("runtime-ipc-device-directory-status-v1.active-device-count")?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
        receipt_id: decode_optional_fixed(decoder)?,
        receipt_generation: decode_optional_u64(decoder)?,
        device_list_digest: decoder
            .fixed_bytes("runtime-ipc-device-directory-status-v1.device-list-digest")?,
        applied_device_list_file_utf8: decoder
            .bytes("runtime-ipc-device-directory-status-v1.applied-device-list-file-utf8")?
            .to_vec(),
        launch_device_list_file_utf8: decoder
            .bytes("runtime-ipc-device-directory-status-v1.launch-device-list-file-utf8")?
            .to_vec(),
        profile_convergence: decode_profile_convergence(decoder)?,
        restart_recovery: decode_restart_recovery(decoder)?,
    };
    validate_device_directory_status(&status)?;
    Ok(status)
}

fn encode_optional_fixed(output: &mut Vec<u8>, value: Option<&[u8; 32]>) {
    match value {
        None => {
            push_array_header(output, 1);
            push_unsigned(output, OPTION_NONE);
        }
        Some(value) => {
            push_array_header(output, 2);
            push_unsigned(output, OPTION_SOME);
            push_bytes(output, value);
        }
    }
}

fn decode_optional_fixed(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Option<[u8; 32]>, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-optional-fixed-bytes-v1")?;
    match (
        decoder.unsigned("runtime-ipc-optional-fixed-bytes-v1.variant")?,
        length,
    ) {
        (OPTION_NONE, 1) => Ok(None),
        (OPTION_SOME, 2) => Ok(Some(
            decoder.fixed_bytes("runtime-ipc-optional-fixed-bytes-v1.value")?,
        )),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_optional_u64(output: &mut Vec<u8>, value: Option<u64>) {
    match value {
        None => {
            push_array_header(output, 1);
            push_unsigned(output, OPTION_NONE);
        }
        Some(value) => {
            push_array_header(output, 2);
            push_unsigned(output, OPTION_SOME);
            push_unsigned(output, value);
        }
    }
}

fn decode_optional_u64(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Option<u64>, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-optional-uint-v1")?;
    match (
        decoder.unsigned("runtime-ipc-optional-uint-v1.variant")?,
        length,
    ) {
        (OPTION_NONE, 1) => Ok(None),
        (OPTION_SOME, 2) => Ok(Some(
            decoder.unsigned("runtime-ipc-optional-uint-v1.value")?,
        )),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_profile_convergence(
    output: &mut Vec<u8>,
    value: CanonicalRuntimeIpcProfileConvergenceV1,
) {
    push_unsigned(
        output,
        match value {
            CanonicalRuntimeIpcProfileConvergenceV1::Current => PROFILE_CURRENT,
            CanonicalRuntimeIpcProfileConvergenceV1::Required => PROFILE_CONVERGENCE_REQUIRED,
        },
    );
}

fn decode_profile_convergence(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcProfileConvergenceV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-profile-convergence-v1")? {
        PROFILE_CURRENT => Ok(CanonicalRuntimeIpcProfileConvergenceV1::Current),
        PROFILE_CONVERGENCE_REQUIRED => Ok(CanonicalRuntimeIpcProfileConvergenceV1::Required),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_restart_recovery(output: &mut Vec<u8>, value: CanonicalRuntimeIpcRestartRecoveryV1) {
    push_unsigned(
        output,
        match value {
            CanonicalRuntimeIpcRestartRecoveryV1::AuthenticatedReceiptCurrent => {
                RESTART_AUTHENTICATED_RECEIPT_CURRENT
            }
            CanonicalRuntimeIpcRestartRecoveryV1::AuthenticatedReceiptRecovered => {
                RESTART_AUTHENTICATED_RECEIPT_RECOVERED
            }
            CanonicalRuntimeIpcRestartRecoveryV1::LaunchProfileDirectory => {
                RESTART_LAUNCH_PROFILE_DIRECTORY
            }
        },
    );
}

fn decode_restart_recovery(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcRestartRecoveryV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-restart-recovery-v1")? {
        RESTART_AUTHENTICATED_RECEIPT_CURRENT => {
            Ok(CanonicalRuntimeIpcRestartRecoveryV1::AuthenticatedReceiptCurrent)
        }
        RESTART_AUTHENTICATED_RECEIPT_RECOVERED => {
            Ok(CanonicalRuntimeIpcRestartRecoveryV1::AuthenticatedReceiptRecovered)
        }
        RESTART_LAUNCH_PROFILE_DIRECTORY => {
            Ok(CanonicalRuntimeIpcRestartRecoveryV1::LaunchProfileDirectory)
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
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(CanonicalRuntimeIpcError::ObjectTooLarge { object, maximum });
    }
    Ok(())
}

fn verify_ed25519(
    key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
) -> Result<(), CanonicalRuntimeIpcError> {
    let key =
        VerifyingKey::from_bytes(key).map_err(|_| CanonicalRuntimeIpcError::InvalidPublicKey)?;
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
    use serde_json::Value;

    use super::*;
    use crate::canonical_runtime_ipc_v1::decode_and_verify_signed_runtime_ipc_descriptor_v1;

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    fn vectors() -> TestResult<(Value, Value)> {
        Ok((
            serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../CANONICAL-RUNTIME-IPC-CONTACT-VECTORS.json"
            )))?,
            serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../CANONICAL-RUNTIME-IPC-VECTORS.json"
            )))?,
        ))
    }

    fn hex_at(document: &Value, pointer: &str) -> TestResult<Vec<u8>> {
        let value = document
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("missing vector pointer {pointer}"))?;
        Ok((0..value.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&value[index..index + 2], 16))
            .collect::<Result<Vec<_>, _>>()?)
    }

    fn fixed<const N: usize>(document: &Value, pointer: &str) -> TestResult<[u8; N]> {
        Ok(hex_at(document, pointer)?
            .try_into()
            .map_err(|bytes: Vec<u8>| {
                format!("{pointer} has {} bytes; expected {N}", bytes.len())
            })?)
    }

    #[test]
    fn canonical_runtime_ipc_contact_vectors_bind_exact_requests() -> TestResult {
        let (vectors, core) = vectors()?;
        let descriptor = decode_and_verify_signed_runtime_ipc_descriptor_v1(&hex_at(
            &core,
            "/expected/descriptor/signed_cbor_hex",
        )?)?;
        let signing_key = SigningKey::from_bytes(&fixed(&core, "/input/device_secret_key_hex")?);
        for surface in ["add_contact", "directory_status"] {
            let request_path = format!("/expected/{surface}/request/cbor_hex");
            let response_path = format!("/expected/{surface}/response/signed_cbor_hex");
            let request = decode_and_verify_runtime_ipc_contact_request_v1(&hex_at(
                &vectors,
                &request_path,
            )?)?;
            let response = decode_and_verify_signed_runtime_ipc_contact_response_v1(&hex_at(
                &vectors,
                &response_path,
            )?)?;
            verify_runtime_ipc_contact_request_for_descriptor_v1(
                &request,
                &descriptor,
                request.issued_at_unix_seconds,
            )?;
            verify_runtime_ipc_contact_response_for_request_v1(&response, &request, &descriptor)?;
            assert_eq!(
                encode_runtime_ipc_contact_request_v1(&request)?,
                hex_at(&vectors, &request_path)?
            );
            assert_eq!(
                encode_signed_runtime_ipc_contact_response_v1(
                    &sign_runtime_ipc_contact_response_v1(
                        &signing_key,
                        &request,
                        &descriptor,
                        request.issued_at_unix_seconds,
                        response.content.result.clone(),
                    )?
                )?,
                hex_at(&vectors, &response_path)?
            );
        }
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_contact_vector_ids_match() -> TestResult {
        let (vectors, _) = vectors()?;
        for surface in ["add_contact", "directory_status"] {
            let request = decode_and_verify_runtime_ipc_contact_request_v1(&hex_at(
                &vectors,
                &format!("/expected/{surface}/request/cbor_hex"),
            )?)?;
            let response = decode_and_verify_signed_runtime_ipc_contact_response_v1(&hex_at(
                &vectors,
                &format!("/expected/{surface}/response/signed_cbor_hex"),
            )?)?;
            assert_eq!(
                runtime_ipc_contact_request_digest_v1(&request)?,
                fixed(
                    &vectors,
                    &format!("/expected/{surface}/request/object_id_hex")
                )?
            );
            assert_eq!(
                runtime_ipc_contact_response_id_v1(&response)?,
                fixed(
                    &vectors,
                    &format!("/expected/{surface}/response/object_id_hex")
                )?
            );
        }
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_contact_substitutions_fail_closed() -> TestResult {
        let (vectors, core) = vectors()?;
        let descriptor = decode_and_verify_signed_runtime_ipc_descriptor_v1(&hex_at(
            &core,
            "/expected/descriptor/signed_cbor_hex",
        )?)?;
        let request = decode_and_verify_runtime_ipc_contact_request_v1(&hex_at(
            &vectors,
            "/expected/add_contact/request/cbor_hex",
        )?)?;
        let response = decode_and_verify_signed_runtime_ipc_contact_response_v1(&hex_at(
            &vectors,
            "/expected/add_contact/response/signed_cbor_hex",
        )?)?;
        let mut changed = request.clone();
        if let CanonicalRuntimeIpcContactCommandV1::AddContact {
            conversation_label_utf8,
            ..
        } = &mut changed.command
        {
            conversation_label_utf8.push(b'x');
        }
        assert!(
            verify_runtime_ipc_contact_response_for_request_v1(&response, &changed, &descriptor,)
                .is_err()
        );
        let mut tampered = response;
        tampered.signature[0] ^= 1;
        assert!(verify_signed_runtime_ipc_contact_response_v1(&tampered).is_err());
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_directory_status_invariants_fail_closed() -> TestResult {
        let (vectors, core) = vectors()?;
        let descriptor = decode_and_verify_signed_runtime_ipc_descriptor_v1(&hex_at(
            &core,
            "/expected/descriptor/signed_cbor_hex",
        )?)?;
        let signing_key = SigningKey::from_bytes(&fixed(&core, "/input/device_secret_key_hex")?);
        let request = decode_and_verify_runtime_ipc_contact_request_v1(&hex_at(
            &vectors,
            "/expected/directory_status/request/cbor_hex",
        )?)?;
        let response = decode_and_verify_signed_runtime_ipc_contact_response_v1(&hex_at(
            &vectors,
            "/expected/directory_status/response/signed_cbor_hex",
        )?)?;
        let mut result = response.content.result;
        if let CanonicalRuntimeIpcContactResultV1::OwnDeviceDirectoryStatus(status) = &mut result {
            status.profile_convergence = CanonicalRuntimeIpcProfileConvergenceV1::Current;
        }
        assert!(
            sign_runtime_ipc_contact_response_v1(
                &signing_key,
                &request,
                &descriptor,
                request.issued_at_unix_seconds,
                result,
            )
            .is_err()
        );
        Ok(())
    }
}
