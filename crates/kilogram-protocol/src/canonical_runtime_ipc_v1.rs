//! Inactive deterministic-CBOR core for the local runtime IPC trust boundary.
//!
//! The active runtime continues to use Postcard IPC version 26 and its JSON
//! descriptor. This candidate reserves a separately versioned descriptor,
//! request and response contract with explicit correlation and freshness.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use thiserror::Error;

use crate::{
    canonical_account_v1::{AccountDecoder, CanonicalAccountError, signed_envelope},
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
        push_unsigned, validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_RUNTIME_IPC_DESCRIPTOR_BYTES: usize = 2 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_REQUEST_BYTES: usize = 64 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_RESPONSE_BYTES: usize = 64 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_ERROR_BYTES: usize = 4 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_DESCRIPTOR_VALIDITY_SECONDS: u64 = 24 * 60 * 60;
pub const MAX_CANONICAL_RUNTIME_IPC_REQUEST_VALIDITY_SECONDS: u64 = 30;
pub const MAX_CANONICAL_RUNTIME_IPC_WAIT_MILLISECONDS: u32 = 120_000;
pub const CANONICAL_RUNTIME_IPC_CLOCK_SKEW_SECONDS: u64 = 30;

pub const CANONICAL_RUNTIME_IPC_DESCRIPTOR_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-descriptor-signature:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_DESCRIPTOR_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-descriptor-id:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-request-digest:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_RESPONSE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-response-signature:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_RESPONSE_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-response-id:cbor-v1\0";

const COMMAND_PING: u64 = 0;
const COMMAND_WAIT_FOR_CHANGE: u64 = 1;
const COMMAND_SHUTDOWN: u64 = 2;
const RESULT_PONG: u64 = 0;
const RESULT_CHANGE_STATE: u64 = 1;
const RESULT_SHUTDOWN_ACCEPTED: u64 = 2;
const RESULT_ERROR: u64 = 3;
const ADDRESS_IPV4: u64 = 0;
const ADDRESS_IPV6: u64 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcLoopbackAddressV1 {
    Ipv4([u8; 4]),
    Ipv6([u8; 16]),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcDescriptorContentV1 {
    pub address: CanonicalRuntimeIpcLoopbackAddressV1,
    pub port: u16,
    pub bearer_token: [u8; 32],
    pub account_id: [u8; 32],
    pub device_id: [u8; 32],
    pub server_instance_id: [u8; 32],
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeIpcDescriptorV1 {
    pub content: CanonicalRuntimeIpcDescriptorContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcCoreCommandV1 {
    Ping,
    WaitForChange {
        after_revision: u64,
        timeout_milliseconds: u32,
    },
    Shutdown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcCoreRequestV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub bearer_token: [u8; 32],
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub command: CanonicalRuntimeIpcCoreCommandV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcCoreResultV1 {
    Pong {
        account_id: [u8; 32],
        device_id: [u8; 32],
    },
    ChangeState {
        revision: u64,
        changed: bool,
    },
    ShutdownAccepted,
    Error {
        code: u16,
        message_utf8: Vec<u8>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcCoreResponseContentV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub request_digest: [u8; 32],
    pub responder_device_id: [u8; 32],
    pub result: CanonicalRuntimeIpcCoreResultV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeIpcCoreResponseV1 {
    pub content: CanonicalRuntimeIpcCoreResponseContentV1,
    pub signature: [u8; 64],
}

#[derive(Debug, Error)]
pub enum CanonicalRuntimeIpcError {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),
    #[error(transparent)]
    Account(#[from] CanonicalAccountError),
    #[error("canonical runtime IPC {object} exceeds {maximum} bytes")]
    ObjectTooLarge {
        object: &'static str,
        maximum: usize,
    },
    #[error("canonical runtime IPC descriptor is invalid")]
    InvalidDescriptor,
    #[error("canonical runtime IPC descriptor is not current")]
    DescriptorNotCurrent,
    #[error("canonical runtime IPC request is invalid")]
    InvalidRequest,
    #[error("canonical runtime IPC request is not current")]
    RequestNotCurrent,
    #[error("canonical runtime IPC response is invalid")]
    InvalidResponse,
    #[error("canonical runtime IPC context does not match")]
    ContextMismatch,
    #[error("canonical runtime IPC contains an invalid Ed25519 key")]
    InvalidPublicKey,
    #[error("canonical runtime IPC signature is invalid")]
    InvalidSignature,
}

pub fn encode_runtime_ipc_descriptor_content_v1(
    content: &CanonicalRuntimeIpcDescriptorContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_descriptor_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 9);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    encode_loopback_address(&mut output, content.address);
    push_unsigned(&mut output, u64::from(content.port));
    push_bytes(&mut output, &content.bearer_token);
    push_bytes(&mut output, &content.account_id);
    push_bytes(&mut output, &content.device_id);
    push_bytes(&mut output, &content.server_instance_id);
    push_unsigned(&mut output, content.issued_at_unix_seconds);
    push_unsigned(&mut output, content.expires_at_unix_seconds);
    Ok(output)
}

pub fn runtime_ipc_descriptor_signing_bytes_v1(
    content: &CanonicalRuntimeIpcDescriptorContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    Ok(prefixed(
        CANONICAL_RUNTIME_IPC_DESCRIPTOR_SIGNATURE_DOMAIN,
        &encode_runtime_ipc_descriptor_content_v1(content)?,
    ))
}

pub fn sign_runtime_ipc_descriptor_v1(
    device_signing_key: &SigningKey,
    content: CanonicalRuntimeIpcDescriptorContentV1,
) -> Result<CanonicalSignedRuntimeIpcDescriptorV1, CanonicalRuntimeIpcError> {
    if content.device_id != device_signing_key.verifying_key().to_bytes() {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    let signature = device_signing_key
        .sign(&runtime_ipc_descriptor_signing_bytes_v1(&content)?)
        .to_bytes();
    let descriptor = CanonicalSignedRuntimeIpcDescriptorV1 { content, signature };
    verify_signed_runtime_ipc_descriptor_v1(&descriptor)?;
    Ok(descriptor)
}

pub fn encode_signed_runtime_ipc_descriptor_v1(
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_descriptor_v1(descriptor)?;
    let encoded = signed_envelope(
        &encode_runtime_ipc_descriptor_content_v1(&descriptor.content)?,
        &descriptor.signature,
    );
    ensure_bound(
        &encoded,
        "descriptor",
        MAX_CANONICAL_RUNTIME_IPC_DESCRIPTOR_BYTES,
    )?;
    Ok(encoded)
}

pub fn decode_and_verify_signed_runtime_ipc_descriptor_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeIpcDescriptorV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "descriptor",
        MAX_CANONICAL_RUNTIME_IPC_DESCRIPTOR_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-runtime-ipc-descriptor-v1", 3)?;
    decoder.expect_version("signed-runtime-ipc-descriptor-v1.schema_version")?;
    let content = decode_descriptor_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-runtime-ipc-descriptor-v1.signature")?;
    decoder.finish()?;
    let descriptor = CanonicalSignedRuntimeIpcDescriptorV1 { content, signature };
    verify_signed_runtime_ipc_descriptor_v1(&descriptor)?;
    Ok(descriptor)
}

pub fn verify_signed_runtime_ipc_descriptor_v1(
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_descriptor_content(&descriptor.content)?;
    verify_ed25519(
        &descriptor.content.device_id,
        &runtime_ipc_descriptor_signing_bytes_v1(&descriptor.content)?,
        &descriptor.signature,
    )
}

pub fn verify_runtime_ipc_descriptor_at_v1(
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_descriptor_v1(descriptor)?;
    if descriptor.content.issued_at_unix_seconds
        > now_unix_seconds.saturating_add(CANONICAL_RUNTIME_IPC_CLOCK_SKEW_SECONDS)
        || descriptor.content.expires_at_unix_seconds < now_unix_seconds
    {
        return Err(CanonicalRuntimeIpcError::DescriptorNotCurrent);
    }
    Ok(())
}

pub fn runtime_ipc_descriptor_id_v1(
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_DESCRIPTOR_ID_DOMAIN,
        &encode_signed_runtime_ipc_descriptor_v1(descriptor)?,
    ))
}

pub fn encode_runtime_ipc_core_request_v1(
    request: &CanonicalRuntimeIpcCoreRequestV1,
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
    encode_command(&mut output, &request.command);
    ensure_bound(
        &output,
        "core request",
        MAX_CANONICAL_RUNTIME_IPC_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_runtime_ipc_core_request_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcCoreRequestV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "core request",
        MAX_CANONICAL_RUNTIME_IPC_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("runtime-ipc-core-request-v1", 7)?;
    decoder.expect_version("runtime-ipc-core-request-v1.schema_version")?;
    let request = CanonicalRuntimeIpcCoreRequestV1 {
        descriptor_id: decoder.fixed_bytes("runtime-ipc-core-request-v1.descriptor_id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-core-request-v1.request_id")?,
        bearer_token: decoder.fixed_bytes("runtime-ipc-core-request-v1.bearer_token")?,
        issued_at_unix_seconds: decoder
            .unsigned("runtime-ipc-core-request-v1.issued_at_unix_seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("runtime-ipc-core-request-v1.expires_at_unix_seconds")?,
        command: decode_command(&mut decoder)?,
    };
    decoder.finish()?;
    validate_request(&request)?;
    Ok(request)
}

pub fn runtime_ipc_core_request_digest_v1(
    request: &CanonicalRuntimeIpcCoreRequestV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_REQUEST_DIGEST_DOMAIN,
        &encode_runtime_ipc_core_request_v1(request)?,
    ))
}

pub fn verify_runtime_ipc_core_request_for_descriptor_v1(
    request: &CanonicalRuntimeIpcCoreRequestV1,
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
    Ok(())
}

pub fn runtime_ipc_core_response_signing_bytes_v1(
    content: &CanonicalRuntimeIpcCoreResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    Ok(prefixed(
        CANONICAL_RUNTIME_IPC_RESPONSE_SIGNATURE_DOMAIN,
        &encode_runtime_ipc_core_response_content_v1(content)?,
    ))
}

pub fn sign_runtime_ipc_core_response_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalRuntimeIpcCoreRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
    result: CanonicalRuntimeIpcCoreResultV1,
) -> Result<CanonicalSignedRuntimeIpcCoreResponseV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_core_request_for_descriptor_v1(request, descriptor, now_unix_seconds)?;
    if device_signing_key.verifying_key().to_bytes() != descriptor.content.device_id
        || request.descriptor_id != runtime_ipc_descriptor_id_v1(descriptor)?
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    let content = CanonicalRuntimeIpcCoreResponseContentV1 {
        descriptor_id: request.descriptor_id,
        request_id: request.request_id,
        request_digest: runtime_ipc_core_request_digest_v1(request)?,
        responder_device_id: descriptor.content.device_id,
        result,
    };
    let signature = device_signing_key
        .sign(&runtime_ipc_core_response_signing_bytes_v1(&content)?)
        .to_bytes();
    let response = CanonicalSignedRuntimeIpcCoreResponseV1 { content, signature };
    verify_runtime_ipc_core_response_for_request_v1(&response, request, descriptor)?;
    Ok(response)
}

pub fn encode_signed_runtime_ipc_core_response_v1(
    response: &CanonicalSignedRuntimeIpcCoreResponseV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_core_response_v1(response)?;
    let output = signed_envelope(
        &encode_runtime_ipc_core_response_content_v1(&response.content)?,
        &response.signature,
    );
    ensure_bound(
        &output,
        "core response",
        MAX_CANONICAL_RUNTIME_IPC_RESPONSE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_runtime_ipc_core_response_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeIpcCoreResponseV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "core response",
        MAX_CANONICAL_RUNTIME_IPC_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-runtime-ipc-core-response-v1", 3)?;
    decoder.expect_version("signed-runtime-ipc-core-response-v1.schema_version")?;
    let content = decode_response_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-runtime-ipc-core-response-v1.signature")?;
    decoder.finish()?;
    let response = CanonicalSignedRuntimeIpcCoreResponseV1 { content, signature };
    verify_signed_runtime_ipc_core_response_v1(&response)?;
    Ok(response)
}

pub fn verify_signed_runtime_ipc_core_response_v1(
    response: &CanonicalSignedRuntimeIpcCoreResponseV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_response_content(&response.content)?;
    verify_ed25519(
        &response.content.responder_device_id,
        &runtime_ipc_core_response_signing_bytes_v1(&response.content)?,
        &response.signature,
    )
}

pub fn verify_runtime_ipc_core_response_for_request_v1(
    response: &CanonicalSignedRuntimeIpcCoreResponseV1,
    request: &CanonicalRuntimeIpcCoreRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_core_response_v1(response)?;
    validate_request(request)?;
    verify_signed_runtime_ipc_descriptor_v1(descriptor)?;
    if response.content.descriptor_id != runtime_ipc_descriptor_id_v1(descriptor)?
        || response.content.descriptor_id != request.descriptor_id
        || response.content.request_id != request.request_id
        || response.content.request_digest != runtime_ipc_core_request_digest_v1(request)?
        || response.content.responder_device_id != descriptor.content.device_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    match (&request.command, &response.content.result) {
        (_, CanonicalRuntimeIpcCoreResultV1::Error { .. }) => Ok(()),
        (
            CanonicalRuntimeIpcCoreCommandV1::Ping,
            CanonicalRuntimeIpcCoreResultV1::Pong {
                account_id,
                device_id,
            },
        ) if *account_id == descriptor.content.account_id
            && *device_id == descriptor.content.device_id =>
        {
            Ok(())
        }
        (
            CanonicalRuntimeIpcCoreCommandV1::WaitForChange { after_revision, .. },
            CanonicalRuntimeIpcCoreResultV1::ChangeState { revision, changed },
        ) if (*changed && *revision > *after_revision)
            || (!*changed && *revision == *after_revision) =>
        {
            Ok(())
        }
        (
            CanonicalRuntimeIpcCoreCommandV1::Shutdown,
            CanonicalRuntimeIpcCoreResultV1::ShutdownAccepted,
        ) => Ok(()),
        _ => Err(CanonicalRuntimeIpcError::ContextMismatch),
    }
}

pub fn runtime_ipc_core_response_id_v1(
    response: &CanonicalSignedRuntimeIpcCoreResponseV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_RESPONSE_ID_DOMAIN,
        &encode_signed_runtime_ipc_core_response_v1(response)?,
    ))
}

fn encode_runtime_ipc_core_response_content_v1(
    content: &CanonicalRuntimeIpcCoreResponseContentV1,
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

fn decode_response_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcCoreResponseContentV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-core-response-content-v1", 6)?;
    decoder.expect_version("runtime-ipc-core-response-content-v1.schema_version")?;
    let content = CanonicalRuntimeIpcCoreResponseContentV1 {
        descriptor_id: decoder.fixed_bytes("runtime-ipc-core-response-content-v1.descriptor_id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-core-response-content-v1.request_id")?,
        request_digest: decoder
            .fixed_bytes("runtime-ipc-core-response-content-v1.request_digest")?,
        responder_device_id: decoder
            .fixed_bytes("runtime-ipc-core-response-content-v1.responder_device_id")?,
        result: decode_result(decoder)?,
    };
    validate_response_content(&content)?;
    Ok(content)
}

fn decode_descriptor_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcDescriptorContentV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-descriptor-content-v1", 9)?;
    decoder.expect_version("runtime-ipc-descriptor-content-v1.schema_version")?;
    let content = CanonicalRuntimeIpcDescriptorContentV1 {
        address: decode_loopback_address(decoder)?,
        port: u16::try_from(decoder.unsigned("runtime-ipc-descriptor-content-v1.port")?)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidDescriptor)?,
        bearer_token: decoder.fixed_bytes("runtime-ipc-descriptor-content-v1.bearer_token")?,
        account_id: decoder.fixed_bytes("runtime-ipc-descriptor-content-v1.account_id")?,
        device_id: decoder.fixed_bytes("runtime-ipc-descriptor-content-v1.device_id")?,
        server_instance_id: decoder
            .fixed_bytes("runtime-ipc-descriptor-content-v1.server_instance_id")?,
        issued_at_unix_seconds: decoder
            .unsigned("runtime-ipc-descriptor-content-v1.issued_at_unix_seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("runtime-ipc-descriptor-content-v1.expires_at_unix_seconds")?,
    };
    validate_descriptor_content(&content)?;
    Ok(content)
}

fn validate_descriptor_content(
    content: &CanonicalRuntimeIpcDescriptorContentV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    let validity = content
        .expires_at_unix_seconds
        .checked_sub(content.issued_at_unix_seconds)
        .ok_or(CanonicalRuntimeIpcError::InvalidDescriptor)?;
    if !is_loopback(content.address)
        || content.port == 0
        || content.bearer_token == [0; 32]
        || content.account_id == [0; 32]
        || content.device_id == [0; 32]
        || content.server_instance_id == [0; 32]
        || !(1..=MAX_CANONICAL_RUNTIME_IPC_DESCRIPTOR_VALIDITY_SECONDS).contains(&validity)
    {
        return Err(CanonicalRuntimeIpcError::InvalidDescriptor);
    }
    Ok(())
}

fn validate_request(
    request: &CanonicalRuntimeIpcCoreRequestV1,
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
    if let CanonicalRuntimeIpcCoreCommandV1::WaitForChange {
        timeout_milliseconds,
        ..
    } = request.command
        && !(1..=MAX_CANONICAL_RUNTIME_IPC_WAIT_MILLISECONDS).contains(&timeout_milliseconds)
    {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    Ok(())
}

fn validate_response_content(
    response: &CanonicalRuntimeIpcCoreResponseContentV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if response.descriptor_id == [0; 32]
        || response.request_id == [0; 32]
        || response.request_digest == [0; 32]
        || response.responder_device_id == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    match &response.result {
        CanonicalRuntimeIpcCoreResultV1::Pong {
            account_id,
            device_id,
        } if *account_id != [0; 32] && *device_id != [0; 32] => Ok(()),
        CanonicalRuntimeIpcCoreResultV1::ChangeState { .. }
        | CanonicalRuntimeIpcCoreResultV1::ShutdownAccepted => Ok(()),
        CanonicalRuntimeIpcCoreResultV1::Error { code, message_utf8 }
            if *code > 0
                && !message_utf8.is_empty()
                && message_utf8.len() <= MAX_CANONICAL_RUNTIME_IPC_ERROR_BYTES
                && std::str::from_utf8(message_utf8).is_ok() =>
        {
            Ok(())
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_loopback_address(output: &mut Vec<u8>, address: CanonicalRuntimeIpcLoopbackAddressV1) {
    push_array_header(output, 2);
    match address {
        CanonicalRuntimeIpcLoopbackAddressV1::Ipv4(bytes) => {
            push_unsigned(output, ADDRESS_IPV4);
            push_bytes(output, &bytes);
        }
        CanonicalRuntimeIpcLoopbackAddressV1::Ipv6(bytes) => {
            push_unsigned(output, ADDRESS_IPV6);
            push_bytes(output, &bytes);
        }
    }
}

fn decode_loopback_address(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcLoopbackAddressV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-loopback-address-v1", 2)?;
    let address = match decoder.unsigned("runtime-ipc-loopback-address-v1.variant")? {
        ADDRESS_IPV4 => CanonicalRuntimeIpcLoopbackAddressV1::Ipv4(
            decoder.fixed_bytes("runtime-ipc-loopback-address-v1.ipv4")?,
        ),
        ADDRESS_IPV6 => CanonicalRuntimeIpcLoopbackAddressV1::Ipv6(
            decoder.fixed_bytes("runtime-ipc-loopback-address-v1.ipv6")?,
        ),
        _ => return Err(CanonicalRuntimeIpcError::InvalidDescriptor),
    };
    Ok(address)
}

fn is_loopback(address: CanonicalRuntimeIpcLoopbackAddressV1) -> bool {
    match address {
        CanonicalRuntimeIpcLoopbackAddressV1::Ipv4(bytes) => bytes == [127, 0, 0, 1],
        CanonicalRuntimeIpcLoopbackAddressV1::Ipv6(bytes) => {
            bytes[..15] == [0; 15] && bytes[15] == 1
        }
    }
}

fn encode_command(output: &mut Vec<u8>, command: &CanonicalRuntimeIpcCoreCommandV1) {
    match command {
        CanonicalRuntimeIpcCoreCommandV1::Ping => {
            push_array_header(output, 1);
            push_unsigned(output, COMMAND_PING);
        }
        CanonicalRuntimeIpcCoreCommandV1::WaitForChange {
            after_revision,
            timeout_milliseconds,
        } => {
            push_array_header(output, 3);
            push_unsigned(output, COMMAND_WAIT_FOR_CHANGE);
            push_unsigned(output, *after_revision);
            push_unsigned(output, u64::from(*timeout_milliseconds));
        }
        CanonicalRuntimeIpcCoreCommandV1::Shutdown => {
            push_array_header(output, 1);
            push_unsigned(output, COMMAND_SHUTDOWN);
        }
    }
}

fn decode_command(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcCoreCommandV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-core-command-v1")?;
    let variant = decoder.unsigned("runtime-ipc-core-command-v1.variant")?;
    match (variant, length) {
        (COMMAND_PING, 1) => Ok(CanonicalRuntimeIpcCoreCommandV1::Ping),
        (COMMAND_WAIT_FOR_CHANGE, 3) => {
            let after_revision = decoder.unsigned("runtime-ipc-core-command-v1.after_revision")?;
            let timeout_milliseconds = u32::try_from(
                decoder.unsigned("runtime-ipc-core-command-v1.timeout_milliseconds")?,
            )
            .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?;
            Ok(CanonicalRuntimeIpcCoreCommandV1::WaitForChange {
                after_revision,
                timeout_milliseconds,
            })
        }
        (COMMAND_SHUTDOWN, 1) => Ok(CanonicalRuntimeIpcCoreCommandV1::Shutdown),
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_result(
    output: &mut Vec<u8>,
    result: &CanonicalRuntimeIpcCoreResultV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match result {
        CanonicalRuntimeIpcCoreResultV1::Pong {
            account_id,
            device_id,
        } => {
            push_array_header(output, 3);
            push_unsigned(output, RESULT_PONG);
            push_bytes(output, account_id);
            push_bytes(output, device_id);
        }
        CanonicalRuntimeIpcCoreResultV1::ChangeState { revision, changed } => {
            push_array_header(output, 3);
            push_unsigned(output, RESULT_CHANGE_STATE);
            push_unsigned(output, *revision);
            push_unsigned(output, u64::from(*changed));
        }
        CanonicalRuntimeIpcCoreResultV1::ShutdownAccepted => {
            push_array_header(output, 1);
            push_unsigned(output, RESULT_SHUTDOWN_ACCEPTED);
        }
        CanonicalRuntimeIpcCoreResultV1::Error { code, message_utf8 } => {
            push_array_header(output, 3);
            push_unsigned(output, RESULT_ERROR);
            push_unsigned(output, u64::from(*code));
            push_bytes(output, message_utf8);
        }
    }
    validate_result(result)
}

fn decode_result(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcCoreResultV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-core-result-v1")?;
    let variant = decoder.unsigned("runtime-ipc-core-result-v1.variant")?;
    let result = match (variant, length) {
        (RESULT_PONG, 3) => CanonicalRuntimeIpcCoreResultV1::Pong {
            account_id: decoder.fixed_bytes("runtime-ipc-core-result-v1.account_id")?,
            device_id: decoder.fixed_bytes("runtime-ipc-core-result-v1.device_id")?,
        },
        (RESULT_CHANGE_STATE, 3) => {
            let revision = decoder.unsigned("runtime-ipc-core-result-v1.revision")?;
            let changed = match decoder.unsigned("runtime-ipc-core-result-v1.changed")? {
                0 => false,
                1 => true,
                _ => return Err(CanonicalRuntimeIpcError::InvalidResponse),
            };
            CanonicalRuntimeIpcCoreResultV1::ChangeState { revision, changed }
        }
        (RESULT_SHUTDOWN_ACCEPTED, 1) => CanonicalRuntimeIpcCoreResultV1::ShutdownAccepted,
        (RESULT_ERROR, 3) => CanonicalRuntimeIpcCoreResultV1::Error {
            code: u16::try_from(decoder.unsigned("runtime-ipc-core-result-v1.error_code")?)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            message_utf8: decoder
                .bytes("runtime-ipc-core-result-v1.error_message_utf8")?
                .to_vec(),
        },
        _ => return Err(CanonicalRuntimeIpcError::InvalidResponse),
    };
    validate_result(&result)?;
    Ok(result)
}

fn validate_result(
    result: &CanonicalRuntimeIpcCoreResultV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    let placeholder = CanonicalRuntimeIpcCoreResponseContentV1 {
        descriptor_id: [1; 32],
        request_id: [1; 32],
        request_digest: [1; 32],
        responder_device_id: [1; 32],
        result: result.clone(),
    };
    validate_response_content(&placeholder)
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

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    fn vectors() -> TestResult<Value> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-RUNTIME-IPC-VECTORS.json"
        )))?)
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
    fn canonical_runtime_ipc_core_vectors_bind_context() -> TestResult {
        let vectors = vectors()?;
        let signing_key = SigningKey::from_bytes(&fixed(&vectors, "/input/device_secret_key_hex")?);
        let descriptor = decode_and_verify_signed_runtime_ipc_descriptor_v1(&hex_at(
            &vectors,
            "/expected/descriptor/signed_cbor_hex",
        )?)?;
        let request = decode_and_verify_runtime_ipc_core_request_v1(&hex_at(
            &vectors,
            "/expected/request/cbor_hex",
        )?)?;
        let response = decode_and_verify_signed_runtime_ipc_core_response_v1(&hex_at(
            &vectors,
            "/expected/response/signed_cbor_hex",
        )?)?;
        assert_eq!(
            runtime_ipc_descriptor_id_v1(&descriptor)?,
            fixed(&vectors, "/expected/descriptor/object_id_hex")?
        );
        assert_eq!(
            runtime_ipc_core_request_digest_v1(&request)?,
            fixed(&vectors, "/expected/request/object_id_hex")?
        );
        assert_eq!(
            runtime_ipc_core_response_id_v1(&response)?,
            fixed(&vectors, "/expected/response/object_id_hex")?
        );
        verify_runtime_ipc_core_request_for_descriptor_v1(
            &request,
            &descriptor,
            request.issued_at_unix_seconds,
        )?;
        verify_runtime_ipc_core_response_for_request_v1(&response, &request, &descriptor)?;
        assert_eq!(
            encode_signed_runtime_ipc_descriptor_v1(&sign_runtime_ipc_descriptor_v1(
                &signing_key,
                descriptor.content.clone(),
            )?)?,
            hex_at(&vectors, "/expected/descriptor/signed_cbor_hex")?,
        );
        assert_eq!(
            encode_signed_runtime_ipc_core_response_v1(&sign_runtime_ipc_core_response_v1(
                &signing_key,
                &request,
                &descriptor,
                request.issued_at_unix_seconds,
                response.content.result.clone(),
            )?)?,
            hex_at(&vectors, "/expected/response/signed_cbor_hex")?,
        );
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_core_substitutions_fail_closed() -> TestResult {
        let vectors = vectors()?;
        let descriptor = decode_and_verify_signed_runtime_ipc_descriptor_v1(&hex_at(
            &vectors,
            "/expected/descriptor/signed_cbor_hex",
        )?)?;
        let request = decode_and_verify_runtime_ipc_core_request_v1(&hex_at(
            &vectors,
            "/expected/request/cbor_hex",
        )?)?;
        let response = decode_and_verify_signed_runtime_ipc_core_response_v1(&hex_at(
            &vectors,
            "/expected/response/signed_cbor_hex",
        )?)?;
        let mut wrong_token = request.clone();
        wrong_token.bearer_token[0] ^= 1;
        assert!(
            verify_runtime_ipc_core_request_for_descriptor_v1(
                &wrong_token,
                &descriptor,
                request.issued_at_unix_seconds,
            )
            .is_err()
        );
        assert!(
            verify_runtime_ipc_descriptor_at_v1(
                &descriptor,
                descriptor.content.expires_at_unix_seconds + 1,
            )
            .is_err()
        );
        let mut wrong_request = request.clone();
        wrong_request.request_id[0] ^= 1;
        assert!(
            verify_runtime_ipc_core_response_for_request_v1(
                &response,
                &wrong_request,
                &descriptor,
            )
            .is_err()
        );
        let mut wrong_result = response;
        wrong_result.content.result = CanonicalRuntimeIpcCoreResultV1::ShutdownAccepted;
        assert!(
            verify_runtime_ipc_core_response_for_request_v1(&wrong_result, &request, &descriptor,)
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_core_command_result_matrix_fails_closed() -> TestResult {
        let vectors = vectors()?;
        let signing_key = SigningKey::from_bytes(&fixed(&vectors, "/input/device_secret_key_hex")?);
        let descriptor = decode_and_verify_signed_runtime_ipc_descriptor_v1(&hex_at(
            &vectors,
            "/expected/descriptor/signed_cbor_hex",
        )?)?;
        let base = decode_and_verify_runtime_ipc_core_request_v1(&hex_at(
            &vectors,
            "/expected/request/cbor_hex",
        )?)?;

        let mut ping = base.clone();
        ping.request_id = [7; 32];
        ping.command = CanonicalRuntimeIpcCoreCommandV1::Ping;
        let pong = sign_runtime_ipc_core_response_v1(
            &signing_key,
            &ping,
            &descriptor,
            ping.issued_at_unix_seconds,
            CanonicalRuntimeIpcCoreResultV1::Pong {
                account_id: descriptor.content.account_id,
                device_id: descriptor.content.device_id,
            },
        )?;
        verify_runtime_ipc_core_response_for_request_v1(&pong, &ping, &descriptor)?;
        assert!(
            sign_runtime_ipc_core_response_v1(
                &signing_key,
                &ping,
                &descriptor,
                ping.issued_at_unix_seconds,
                CanonicalRuntimeIpcCoreResultV1::Pong {
                    account_id: [9; 32],
                    device_id: descriptor.content.device_id,
                },
            )
            .is_err()
        );

        let mut shutdown = base.clone();
        shutdown.request_id = [8; 32];
        shutdown.command = CanonicalRuntimeIpcCoreCommandV1::Shutdown;
        sign_runtime_ipc_core_response_v1(
            &signing_key,
            &shutdown,
            &descriptor,
            shutdown.issued_at_unix_seconds,
            CanonicalRuntimeIpcCoreResultV1::ShutdownAccepted,
        )?;
        sign_runtime_ipc_core_response_v1(
            &signing_key,
            &shutdown,
            &descriptor,
            shutdown.issued_at_unix_seconds,
            CanonicalRuntimeIpcCoreResultV1::Error {
                code: 1,
                message_utf8: b"shutdown rejected".to_vec(),
            },
        )?;

        let unchanged = sign_runtime_ipc_core_response_v1(
            &signing_key,
            &base,
            &descriptor,
            base.issued_at_unix_seconds,
            CanonicalRuntimeIpcCoreResultV1::ChangeState {
                revision: 41,
                changed: false,
            },
        )?;
        verify_runtime_ipc_core_response_for_request_v1(&unchanged, &base, &descriptor)?;
        assert!(
            sign_runtime_ipc_core_response_v1(
                &signing_key,
                &base,
                &descriptor,
                base.issued_at_unix_seconds,
                CanonicalRuntimeIpcCoreResultV1::ChangeState {
                    revision: 40,
                    changed: false,
                },
            )
            .is_err()
        );

        let mut wrong_bearer = base.clone();
        wrong_bearer.bearer_token[0] ^= 1;
        assert!(
            sign_runtime_ipc_core_response_v1(
                &signing_key,
                &wrong_bearer,
                &descriptor,
                base.issued_at_unix_seconds,
                CanonicalRuntimeIpcCoreResultV1::ChangeState {
                    revision: 42,
                    changed: true,
                },
            )
            .is_err()
        );
        assert!(
            sign_runtime_ipc_core_response_v1(
                &signing_key,
                &base,
                &descriptor,
                base.expires_at_unix_seconds + 1,
                CanonicalRuntimeIpcCoreResultV1::ChangeState {
                    revision: 42,
                    changed: true,
                },
            )
            .is_err()
        );
        Ok(())
    }
}
