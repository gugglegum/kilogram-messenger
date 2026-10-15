//! Inactive deterministic-CBOR durable message-queue insertion boundary for
//! local runtime IPC.
//!
//! The active runtime remains Postcard IPC version 26. The caller-generated
//! request ID is also the durable queue ID, making exact retries idempotent.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::{
    ConversationId,
    canonical_account_v1::{AccountDecoder, signed_envelope},
    canonical_runtime_ipc_contact_v1::canonical_runtime_ipc_contact_id_v1,
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

pub const MAX_CANONICAL_RUNTIME_IPC_QUEUE_REQUEST_BYTES: usize = 128 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_QUEUE_RESPONSE_BYTES: usize = 64 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_QUEUE_LABEL_BYTES: usize = 4 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_QUEUE_MESSAGE_BYTES: usize = 64 * 1024;

pub const CANONICAL_RUNTIME_IPC_QUEUE_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-queue-request-digest:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_QUEUE_RESPONSE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-queue-response-signature:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_QUEUE_RESPONSE_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-queue-response-id:cbor-v1\0";

const COMMAND_QUEUE_MESSAGE: u64 = 0;
const RESULT_MESSAGE_QUEUED: u64 = 0;
const RESULT_ERROR: u64 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcQueueCommandV1 {
    QueueMessage {
        conversation_label_utf8: Vec<u8>,
        peer_account_id: [u8; 32],
        message_utf8: Vec<u8>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcQueueRequestV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub bearer_token: [u8; 32],
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub command: CanonicalRuntimeIpcQueueCommandV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcQueueResultV1 {
    MessageQueued {
        queue_id: [u8; 32],
        contact_id: [u8; 32],
        conversation_id: [u8; 32],
        peer_account_id: [u8; 32],
        inserted: bool,
    },
    Error {
        code: u16,
        message_utf8: Vec<u8>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcQueueResponseContentV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub request_digest: [u8; 32],
    pub responder_device_id: [u8; 32],
    pub result: CanonicalRuntimeIpcQueueResultV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeIpcQueueResponseV1 {
    pub content: CanonicalRuntimeIpcQueueResponseContentV1,
    pub signature: [u8; 64],
}

pub fn encode_runtime_ipc_queue_request_v1(
    request: &CanonicalRuntimeIpcQueueRequestV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_queue_request(request)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 7);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &request.descriptor_id);
    push_bytes(&mut output, &request.request_id);
    push_bytes(&mut output, &request.bearer_token);
    push_unsigned(&mut output, request.issued_at_unix_seconds);
    push_unsigned(&mut output, request.expires_at_unix_seconds);
    encode_queue_command(&mut output, &request.command);
    ensure_bound(
        &output,
        "queue request",
        MAX_CANONICAL_RUNTIME_IPC_QUEUE_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_runtime_ipc_queue_request_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcQueueRequestV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "queue request",
        MAX_CANONICAL_RUNTIME_IPC_QUEUE_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("runtime-ipc-queue-request-v1", 7)?;
    decoder.expect_version("runtime-ipc-queue-request-v1.schema-version")?;
    let request = CanonicalRuntimeIpcQueueRequestV1 {
        descriptor_id: decoder.fixed_bytes("runtime-ipc-queue-request-v1.descriptor-id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-queue-request-v1.request-id")?,
        bearer_token: decoder.fixed_bytes("runtime-ipc-queue-request-v1.bearer-token")?,
        issued_at_unix_seconds: decoder
            .unsigned("runtime-ipc-queue-request-v1.issued-at-unix-seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("runtime-ipc-queue-request-v1.expires-at-unix-seconds")?,
        command: decode_queue_command(&mut decoder)?,
    };
    decoder.finish()?;
    validate_queue_request(&request)?;
    Ok(request)
}

pub fn runtime_ipc_queue_request_digest_v1(
    request: &CanonicalRuntimeIpcQueueRequestV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_QUEUE_REQUEST_DIGEST_DOMAIN,
        &encode_runtime_ipc_queue_request_v1(request)?,
    ))
}

pub fn verify_runtime_ipc_queue_request_for_descriptor_v1(
    request: &CanonicalRuntimeIpcQueueRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_runtime_ipc_descriptor_at_v1(descriptor, now_unix_seconds)?;
    validate_queue_request(request)?;
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
    match &request.command {
        CanonicalRuntimeIpcQueueCommandV1::QueueMessage {
            conversation_label_utf8,
            peer_account_id,
            ..
        } => {
            canonical_runtime_ipc_contact_id_v1(
                &descriptor.content.account_id,
                peer_account_id,
                conversation_label_utf8,
            )?;
        }
    }
    Ok(())
}

pub fn runtime_ipc_queue_response_signing_bytes_v1(
    content: &CanonicalRuntimeIpcQueueResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    Ok(prefixed(
        CANONICAL_RUNTIME_IPC_QUEUE_RESPONSE_SIGNATURE_DOMAIN,
        &encode_queue_response_content(content)?,
    ))
}

pub fn sign_runtime_ipc_queue_response_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalRuntimeIpcQueueRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
    result: CanonicalRuntimeIpcQueueResultV1,
) -> Result<CanonicalSignedRuntimeIpcQueueResponseV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_queue_request_for_descriptor_v1(request, descriptor, now_unix_seconds)?;
    if device_signing_key.verifying_key().to_bytes() != descriptor.content.device_id {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    let content = CanonicalRuntimeIpcQueueResponseContentV1 {
        descriptor_id: request.descriptor_id,
        request_id: request.request_id,
        request_digest: runtime_ipc_queue_request_digest_v1(request)?,
        responder_device_id: descriptor.content.device_id,
        result,
    };
    let signature = device_signing_key
        .sign(&runtime_ipc_queue_response_signing_bytes_v1(&content)?)
        .to_bytes();
    let response = CanonicalSignedRuntimeIpcQueueResponseV1 { content, signature };
    verify_runtime_ipc_queue_response_for_request_v1(&response, request, descriptor)?;
    Ok(response)
}

pub fn encode_signed_runtime_ipc_queue_response_v1(
    response: &CanonicalSignedRuntimeIpcQueueResponseV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_queue_response_v1(response)?;
    let output = signed_envelope(
        &encode_queue_response_content(&response.content)?,
        &response.signature,
    );
    ensure_bound(
        &output,
        "queue response",
        MAX_CANONICAL_RUNTIME_IPC_QUEUE_RESPONSE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_runtime_ipc_queue_response_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeIpcQueueResponseV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "queue response",
        MAX_CANONICAL_RUNTIME_IPC_QUEUE_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-runtime-ipc-queue-response-v1", 3)?;
    decoder.expect_version("signed-runtime-ipc-queue-response-v1.schema-version")?;
    let content = decode_queue_response_content(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-runtime-ipc-queue-response-v1.signature")?;
    decoder.finish()?;
    let response = CanonicalSignedRuntimeIpcQueueResponseV1 { content, signature };
    verify_signed_runtime_ipc_queue_response_v1(&response)?;
    Ok(response)
}

pub fn verify_signed_runtime_ipc_queue_response_v1(
    response: &CanonicalSignedRuntimeIpcQueueResponseV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_queue_response_content(&response.content)?;
    verify_ed25519(
        &response.content.responder_device_id,
        &runtime_ipc_queue_response_signing_bytes_v1(&response.content)?,
        &response.signature,
    )
}

pub fn verify_runtime_ipc_queue_response_for_request_v1(
    response: &CanonicalSignedRuntimeIpcQueueResponseV1,
    request: &CanonicalRuntimeIpcQueueRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_queue_response_v1(response)?;
    validate_queue_request(request)?;
    verify_signed_runtime_ipc_descriptor_v1(descriptor)?;
    if response.content.descriptor_id != runtime_ipc_descriptor_id_v1(descriptor)?
        || response.content.descriptor_id != request.descriptor_id
        || response.content.request_id != request.request_id
        || response.content.request_digest != runtime_ipc_queue_request_digest_v1(request)?
        || response.content.responder_device_id != descriptor.content.device_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    match (&request.command, &response.content.result) {
        (_, CanonicalRuntimeIpcQueueResultV1::Error { .. }) => Ok(()),
        (
            CanonicalRuntimeIpcQueueCommandV1::QueueMessage {
                conversation_label_utf8,
                peer_account_id,
                ..
            },
            CanonicalRuntimeIpcQueueResultV1::MessageQueued {
                queue_id,
                contact_id,
                conversation_id,
                peer_account_id: response_peer_account_id,
                ..
            },
        ) => {
            let label = std::str::from_utf8(conversation_label_utf8)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?;
            if *queue_id != request.request_id
                || response_peer_account_id != peer_account_id
                || *conversation_id != *ConversationId::from_label(label).as_bytes()
                || *contact_id
                    != canonical_runtime_ipc_contact_id_v1(
                        &descriptor.content.account_id,
                        peer_account_id,
                        conversation_label_utf8,
                    )?
            {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            Ok(())
        }
    }
}

pub fn runtime_ipc_queue_response_id_v1(
    response: &CanonicalSignedRuntimeIpcQueueResponseV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_QUEUE_RESPONSE_ID_DOMAIN,
        &encode_signed_runtime_ipc_queue_response_v1(response)?,
    ))
}

fn validate_queue_request(
    request: &CanonicalRuntimeIpcQueueRequestV1,
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
        CanonicalRuntimeIpcQueueCommandV1::QueueMessage {
            conversation_label_utf8,
            peer_account_id,
            message_utf8,
        } => {
            validate_text(
                conversation_label_utf8,
                MAX_CANONICAL_RUNTIME_IPC_QUEUE_LABEL_BYTES,
                CanonicalRuntimeIpcError::InvalidRequest,
            )?;
            validate_text(
                message_utf8,
                MAX_CANONICAL_RUNTIME_IPC_QUEUE_MESSAGE_BYTES,
                CanonicalRuntimeIpcError::InvalidRequest,
            )?;
            if *peer_account_id == [0; 32] {
                return Err(CanonicalRuntimeIpcError::InvalidRequest);
            }
        }
    }
    Ok(())
}

fn validate_queue_response_content(
    response: &CanonicalRuntimeIpcQueueResponseContentV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if response.descriptor_id == [0; 32]
        || response.request_id == [0; 32]
        || response.request_digest == [0; 32]
        || response.responder_device_id == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    match &response.result {
        CanonicalRuntimeIpcQueueResultV1::MessageQueued {
            queue_id,
            contact_id,
            conversation_id,
            peer_account_id,
            ..
        } => {
            if *queue_id == [0; 32]
                || *contact_id == [0; 32]
                || *conversation_id == [0; 32]
                || *peer_account_id == [0; 32]
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            Ok(())
        }
        CanonicalRuntimeIpcQueueResultV1::Error { code, message_utf8 } => {
            if *code == 0 {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            validate_text(
                message_utf8,
                MAX_CANONICAL_RUNTIME_IPC_ERROR_BYTES,
                CanonicalRuntimeIpcError::InvalidResponse,
            )
        }
    }
}

fn encode_queue_command(output: &mut Vec<u8>, command: &CanonicalRuntimeIpcQueueCommandV1) {
    match command {
        CanonicalRuntimeIpcQueueCommandV1::QueueMessage {
            conversation_label_utf8,
            peer_account_id,
            message_utf8,
        } => {
            push_array_header(output, 4);
            push_unsigned(output, COMMAND_QUEUE_MESSAGE);
            push_bytes(output, conversation_label_utf8);
            push_bytes(output, peer_account_id);
            push_bytes(output, message_utf8);
        }
    }
}

fn decode_queue_command(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcQueueCommandV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-queue-command-v1")?;
    match (
        decoder.unsigned("runtime-ipc-queue-command-v1.variant")?,
        length,
    ) {
        (COMMAND_QUEUE_MESSAGE, 4) => Ok(CanonicalRuntimeIpcQueueCommandV1::QueueMessage {
            conversation_label_utf8: decoder
                .bytes("runtime-ipc-queue-command-v1.conversation-label-utf8")?
                .to_vec(),
            peer_account_id: decoder.fixed_bytes("runtime-ipc-queue-command-v1.peer-account-id")?,
            message_utf8: decoder
                .bytes("runtime-ipc-queue-command-v1.message-utf8")?
                .to_vec(),
        }),
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_queue_response_content(
    content: &CanonicalRuntimeIpcQueueResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_queue_response_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 6);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.descriptor_id);
    push_bytes(&mut output, &content.request_id);
    push_bytes(&mut output, &content.request_digest);
    push_bytes(&mut output, &content.responder_device_id);
    encode_queue_result(&mut output, &content.result);
    Ok(output)
}

fn decode_queue_response_content(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcQueueResponseContentV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-queue-response-content-v1", 6)?;
    decoder.expect_version("runtime-ipc-queue-response-content-v1.schema-version")?;
    let content = CanonicalRuntimeIpcQueueResponseContentV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-queue-response-content-v1.descriptor-id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-queue-response-content-v1.request-id")?,
        request_digest: decoder
            .fixed_bytes("runtime-ipc-queue-response-content-v1.request-digest")?,
        responder_device_id: decoder
            .fixed_bytes("runtime-ipc-queue-response-content-v1.responder-device-id")?,
        result: decode_queue_result(decoder)?,
    };
    validate_queue_response_content(&content)?;
    Ok(content)
}

fn encode_queue_result(output: &mut Vec<u8>, result: &CanonicalRuntimeIpcQueueResultV1) {
    match result {
        CanonicalRuntimeIpcQueueResultV1::MessageQueued {
            queue_id,
            contact_id,
            conversation_id,
            peer_account_id,
            inserted,
        } => {
            push_array_header(output, 6);
            push_unsigned(output, RESULT_MESSAGE_QUEUED);
            push_bytes(output, queue_id);
            push_bytes(output, contact_id);
            push_bytes(output, conversation_id);
            push_bytes(output, peer_account_id);
            push_unsigned(output, u64::from(*inserted));
        }
        CanonicalRuntimeIpcQueueResultV1::Error { code, message_utf8 } => {
            push_array_header(output, 3);
            push_unsigned(output, RESULT_ERROR);
            push_unsigned(output, u64::from(*code));
            push_bytes(output, message_utf8);
        }
    }
}

fn decode_queue_result(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcQueueResultV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-queue-result-v1")?;
    match (
        decoder.unsigned("runtime-ipc-queue-result-v1.variant")?,
        length,
    ) {
        (RESULT_MESSAGE_QUEUED, 6) => Ok(CanonicalRuntimeIpcQueueResultV1::MessageQueued {
            queue_id: decoder.fixed_bytes("runtime-ipc-queue-result-v1.queue-id")?,
            contact_id: decoder.fixed_bytes("runtime-ipc-queue-result-v1.contact-id")?,
            conversation_id: decoder.fixed_bytes("runtime-ipc-queue-result-v1.conversation-id")?,
            peer_account_id: decoder.fixed_bytes("runtime-ipc-queue-result-v1.peer-account-id")?,
            inserted: decode_bool(decoder, "runtime-ipc-queue-result-v1.inserted")?,
        }),
        (RESULT_ERROR, 3) => Ok(CanonicalRuntimeIpcQueueResultV1::Error {
            code: u16::try_from(decoder.unsigned("runtime-ipc-queue-result-v1.error-code")?)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            message_utf8: decoder
                .bytes("runtime-ipc-queue-result-v1.error-message-utf8")?
                .to_vec(),
        }),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
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
    maximum: usize,
    error: CanonicalRuntimeIpcError,
) -> Result<(), CanonicalRuntimeIpcError> {
    if bytes.is_empty() || bytes.len() > maximum || std::str::from_utf8(bytes).is_err() {
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
    use crate::canonical_runtime_ipc_v1::decode_and_verify_signed_runtime_ipc_descriptor_v1;

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    struct Fixture {
        descriptor: CanonicalSignedRuntimeIpcDescriptorV1,
        device_key: SigningKey,
        request_id: [u8; 32],
        peer_account_id: [u8; 32],
        conversation_label: Vec<u8>,
        message: Vec<u8>,
        issued_at: u64,
        expires_at: u64,
    }

    fn documents() -> TestResult<(Value, Value, Value)> {
        Ok((
            serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../CANONICAL-RUNTIME-IPC-VECTORS.json"
            )))?,
            serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../CANONICAL-RUNTIME-IPC-CONTACT-VECTORS.json"
            )))?,
            serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../CANONICAL-RUNTIME-IPC-QUEUE-VECTORS.json"
            )))?,
        ))
    }

    fn bytes(document: &Value, pointer: &str) -> TestResult<Vec<u8>> {
        let value = document
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("missing string at {pointer}"))?;
        if value.len() % 2 != 0 {
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
        let (core, contact, _) = documents()?;
        Ok(Fixture {
            descriptor: decode_and_verify_signed_runtime_ipc_descriptor_v1(&bytes(
                &core,
                "/expected/descriptor/signed_cbor_hex",
            )?)?,
            device_key: SigningKey::from_bytes(&fixed(&core, "/input/device_secret_key_hex")?),
            request_id: [0x81; 32],
            peer_account_id: fixed(&contact, "/input/peer_account_id_hex")?,
            conversation_label: b"m1132-queue".to_vec(),
            message: b"hello from canonical durable queue".to_vec(),
            issued_at: 1_800_002_040,
            expires_at: 1_800_002_070,
        })
    }

    fn request(fixture: &Fixture) -> TestResult<CanonicalRuntimeIpcQueueRequestV1> {
        Ok(CanonicalRuntimeIpcQueueRequestV1 {
            descriptor_id: runtime_ipc_descriptor_id_v1(&fixture.descriptor)?,
            request_id: fixture.request_id,
            bearer_token: fixture.descriptor.content.bearer_token,
            issued_at_unix_seconds: fixture.issued_at,
            expires_at_unix_seconds: fixture.expires_at,
            command: CanonicalRuntimeIpcQueueCommandV1::QueueMessage {
                conversation_label_utf8: fixture.conversation_label.clone(),
                peer_account_id: fixture.peer_account_id,
                message_utf8: fixture.message.clone(),
            },
        })
    }

    fn result(fixture: &Fixture) -> TestResult<CanonicalRuntimeIpcQueueResultV1> {
        let label = std::str::from_utf8(&fixture.conversation_label)?;
        Ok(CanonicalRuntimeIpcQueueResultV1::MessageQueued {
            queue_id: fixture.request_id,
            contact_id: canonical_runtime_ipc_contact_id_v1(
                &fixture.descriptor.content.account_id,
                &fixture.peer_account_id,
                &fixture.conversation_label,
            )?,
            conversation_id: *ConversationId::from_label(label).as_bytes(),
            peer_account_id: fixture.peer_account_id,
            inserted: true,
        })
    }

    #[test]
    fn canonical_runtime_ipc_queue_round_trips_and_derives_ids() -> TestResult {
        let fixture = fixture()?;
        let request = request(&fixture)?;
        verify_runtime_ipc_queue_request_for_descriptor_v1(
            &request,
            &fixture.descriptor,
            fixture.issued_at,
        )?;
        let request_bytes = encode_runtime_ipc_queue_request_v1(&request)?;
        assert_eq!(
            decode_and_verify_runtime_ipc_queue_request_v1(&request_bytes)?,
            request
        );
        let response = sign_runtime_ipc_queue_response_v1(
            &fixture.device_key,
            &request,
            &fixture.descriptor,
            fixture.issued_at,
            result(&fixture)?,
        )?;
        let response_bytes = encode_signed_runtime_ipc_queue_response_v1(&response)?;
        assert_eq!(
            decode_and_verify_signed_runtime_ipc_queue_response_v1(&response_bytes)?,
            response
        );
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_queue_vectors_match() -> TestResult {
        let fixture = fixture()?;
        let (_, _, vectors) = documents()?;
        let request = request(&fixture)?;
        let response = sign_runtime_ipc_queue_response_v1(
            &fixture.device_key,
            &request,
            &fixture.descriptor,
            fixture.issued_at,
            result(&fixture)?,
        )?;
        assert_eq!(
            encode_runtime_ipc_queue_request_v1(&request)?,
            bytes(&vectors, "/expected/request/cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_queue_request_digest_v1(&request)?,
            fixed(&vectors, "/expected/request/object_id_hex")?
        );
        assert_eq!(
            encode_signed_runtime_ipc_queue_response_v1(&response)?,
            bytes(&vectors, "/expected/response/signed_cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_queue_response_id_v1(&response)?,
            fixed(&vectors, "/expected/response/object_id_hex")?
        );
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_queue_substitutions_fail_closed() -> TestResult {
        let fixture = fixture()?;
        let request = request(&fixture)?;
        let response = sign_runtime_ipc_queue_response_v1(
            &fixture.device_key,
            &request,
            &fixture.descriptor,
            fixture.issued_at,
            result(&fixture)?,
        )?;

        let mut changed_request = request.clone();
        let CanonicalRuntimeIpcQueueCommandV1::QueueMessage { message_utf8, .. } =
            &mut changed_request.command;
        message_utf8.push(b'!');
        assert!(
            verify_runtime_ipc_queue_response_for_request_v1(
                &response,
                &changed_request,
                &fixture.descriptor,
            )
            .is_err()
        );

        let mut changed_result = result(&fixture)?;
        let CanonicalRuntimeIpcQueueResultV1::MessageQueued { queue_id, .. } = &mut changed_result
        else {
            return Err("fixture result was not message-queued".into());
        };
        queue_id[0] ^= 1;
        assert!(
            sign_runtime_ipc_queue_response_v1(
                &fixture.device_key,
                &request,
                &fixture.descriptor,
                fixture.issued_at,
                changed_result,
            )
            .is_err()
        );

        let mut same_account = request.clone();
        let CanonicalRuntimeIpcQueueCommandV1::QueueMessage {
            peer_account_id, ..
        } = &mut same_account.command;
        *peer_account_id = fixture.descriptor.content.account_id;
        assert!(
            verify_runtime_ipc_queue_request_for_descriptor_v1(
                &same_account,
                &fixture.descriptor,
                fixture.issued_at,
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_queue_bounds_plaintext() -> TestResult {
        let fixture = fixture()?;
        let mut request = request(&fixture)?;
        let CanonicalRuntimeIpcQueueCommandV1::QueueMessage { message_utf8, .. } =
            &mut request.command;
        *message_utf8 = vec![b'x'; MAX_CANONICAL_RUNTIME_IPC_QUEUE_MESSAGE_BYTES + 1];
        assert!(matches!(
            encode_runtime_ipc_queue_request_v1(&request),
            Err(CanonicalRuntimeIpcError::InvalidRequest)
        ));
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
    fn regenerate_canonical_runtime_ipc_queue_vectors() -> TestResult {
        let fixture = fixture()?;
        let request = request(&fixture)?;
        let request_bytes = encode_runtime_ipc_queue_request_v1(&request)?;
        let request_digest = runtime_ipc_queue_request_digest_v1(&request)?;
        let response = sign_runtime_ipc_queue_response_v1(
            &fixture.device_key,
            &request,
            &fixture.descriptor,
            fixture.issued_at,
            result(&fixture)?,
        )?;
        let content_bytes = encode_queue_response_content(&response.content)?;
        let signed_bytes = encode_signed_runtime_ipc_queue_response_v1(&response)?;
        let response_id = runtime_ipc_queue_response_id_v1(&response)?;
        let label = std::str::from_utf8(&fixture.conversation_label)?;
        let contact_id = canonical_runtime_ipc_contact_id_v1(
            &fixture.descriptor.content.account_id,
            &fixture.peer_account_id,
            &fixture.conversation_label,
        )?;
        let document = json!({
            "schema_version": 1,
            "profile": "kilogram-deterministic-cbor-v1",
            "activation_status": "not-active",
            "dependencies": [
                "CANONICAL-RUNTIME-IPC-VECTORS.json",
                "CANONICAL-RUNTIME-IPC-CONTACT-VECTORS.json"
            ],
            "input": {
                "request_id_hex": lower_hex(&fixture.request_id),
                "request_issued_at_unix_seconds": fixture.issued_at,
                "request_expires_at_unix_seconds": fixture.expires_at,
                "conversation_label": label,
                "peer_account_id_hex": lower_hex(&fixture.peer_account_id),
                "message": std::str::from_utf8(&fixture.message)?,
                "inserted": true
            },
            "expected": {
                "account_id_hex": lower_hex(&fixture.descriptor.content.account_id),
                "device_id_hex": lower_hex(&fixture.descriptor.content.device_id),
                "conversation_id_hex": lower_hex(ConversationId::from_label(label).as_bytes()),
                "contact_id_hex": lower_hex(&contact_id),
                "request": {
                    "cbor_hex": lower_hex(&request_bytes),
                    "digest_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_QUEUE_REQUEST_DIGEST_DOMAIN),
                    "id_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_QUEUE_REQUEST_DIGEST_DOMAIN, &request_bytes)),
                    "object_id_hex": lower_hex(&request_digest)
                },
                "response": {
                    "content_cbor_hex": lower_hex(&content_bytes),
                    "signature_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_QUEUE_RESPONSE_SIGNATURE_DOMAIN),
                    "signing_preimage_hex": lower_hex(&runtime_ipc_queue_response_signing_bytes_v1(&response.content)?),
                    "signature_hex": lower_hex(&response.signature),
                    "signed_cbor_hex": lower_hex(&signed_bytes),
                    "id_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_QUEUE_RESPONSE_ID_DOMAIN, &signed_bytes)),
                    "object_id_hex": lower_hex(&response_id)
                },
                "context": {
                    "valid_at_unix_seconds": fixture.issued_at,
                    "queue_id_equals_request_id": true,
                    "response_binds_complete_request_digest": true,
                    "active_runtime_changed": false
                }
            }
        });
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../CANONICAL-RUNTIME-IPC-QUEUE-VECTORS.json");
        fs::write(
            path,
            format!("{}\n", serde_json::to_string_pretty(&document)?),
        )?;
        Ok(())
    }
}
