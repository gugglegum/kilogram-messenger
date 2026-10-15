//! Inactive deterministic-CBOR read model for the local runtime IPC boundary.
//!
//! This module extends the separately versioned candidate descriptor from
//! `canonical_runtime_ipc_v1` with bounded, paginated conversation and history
//! reads. The active runtime remains Postcard IPC version 26.

use std::cmp::Ordering;

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

pub const MAX_CANONICAL_RUNTIME_IPC_READ_RESPONSE_BYTES: usize = 256 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_CONVERSATION_PAGE_SIZE: u16 = 64;
pub const MAX_CANONICAL_RUNTIME_IPC_ENDPOINT_CANDIDATES: usize = 4;
pub const MAX_CANONICAL_RUNTIME_IPC_HISTORY_PAGE_SIZE: u16 = 100;
pub const MAX_CANONICAL_RUNTIME_IPC_HISTORY_PAGE_BODY_BYTES: usize = 192 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_PREVIEW_BYTES: usize = 96;
pub const MAX_CANONICAL_RUNTIME_IPC_TEXT_BYTES: usize = 4 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_PATH_BYTES: usize = 4 * 1024;

pub const CANONICAL_RUNTIME_IPC_READ_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-read-request-digest:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_READ_RESPONSE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-read-response-signature:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_READ_RESPONSE_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-read-response-id:cbor-v1\0";

const COMMAND_CONVERSATION_LIST_PAGE: u64 = 0;
const COMMAND_HISTORY_PAGE: u64 = 1;
const RESULT_CONVERSATION_LIST_PAGE: u64 = 0;
const RESULT_HISTORY_PAGE: u64 = 1;
const RESULT_ERROR: u64 = 2;
const ROUTE_AUTO: u64 = 0;
const ROUTE_DIRECT_ONLY: u64 = 1;
const ROUTE_RELAY_ONLY: u64 = 2;
const ENDPOINT_USABLE: u64 = 0;
const ENDPOINT_STALE: u64 = 1;
const ENDPOINT_QUARANTINED: u64 = 2;
const OPTION_NONE: u64 = 0;
const OPTION_SOME: u64 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcConversationCursorV1 {
    pub conversation_label_utf8: Vec<u8>,
    pub peer_account_id: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcHistoryCursorV1 {
    pub snapshot_id: [u8; 32],
    pub before_index: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcReadCommandV1 {
    ConversationListPage {
        cursor: Option<CanonicalRuntimeIpcConversationCursorV1>,
        limit: u16,
    },
    HistoryPage {
        conversation_label_utf8: Vec<u8>,
        cursor: Option<CanonicalRuntimeIpcHistoryCursorV1>,
        limit: u16,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcReadRequestV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub bearer_token: [u8; 32],
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub command: CanonicalRuntimeIpcReadCommandV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcRoutePolicyV1 {
    Auto,
    DirectOnly,
    RelayOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcEndpointStateV1 {
    Usable,
    Stale,
    Quarantined,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcEndpointCandidateV1 {
    pub peer_device_id: [u8; 32],
    pub primary: bool,
    pub route_policy: CanonicalRuntimeIpcRoutePolicyV1,
    pub descriptor_path_utf8: Vec<u8>,
    pub state: CanonicalRuntimeIpcEndpointStateV1,
    pub authority_revision: Option<u64>,
    pub publication_channel_id_utf8: Option<Vec<u8>>,
    pub observed_publication_generation: Option<u64>,
    pub observed_at_unix_seconds: Option<u64>,
    pub publication_conflict_generation: Option<u64>,
    pub publication_conflict_proof_id_utf8: Option<Vec<u8>>,
    pub publication_conflict_detected_at_unix_seconds: Option<u64>,
    pub detail_utf8: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcMessagePreviewV1 {
    pub event_id: [u8; 32],
    pub author_account_id: [u8; 32],
    pub body_utf8: Vec<u8>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcConversationSummaryV1 {
    pub contact_id_utf8: Vec<u8>,
    pub conversation_label_utf8: Vec<u8>,
    pub conversation_id: [u8; 32],
    pub peer_account_id: [u8; 32],
    pub peer_device_id: [u8; 32],
    pub endpoint_candidate_count: u8,
    pub usable_endpoint_candidate_count: u8,
    pub stale_endpoint_candidate_count: u8,
    pub quarantined_endpoint_candidate_count: u8,
    pub endpoint_candidates: Vec<CanonicalRuntimeIpcEndpointCandidateV1>,
    pub route_policy: CanonicalRuntimeIpcRoutePolicyV1,
    pub message_count: u32,
    pub latest_message: Option<CanonicalRuntimeIpcMessagePreviewV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcHistoryMessageV1 {
    pub event_id: [u8; 32],
    pub author_account_id: [u8; 32],
    pub author_device_id: [u8; 32],
    pub author_sequence: u64,
    pub body_utf8: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcHistoryPageV1 {
    pub conversation_id: [u8; 32],
    pub snapshot_id: [u8; 32],
    pub total_messages: u32,
    pub messages: Vec<CanonicalRuntimeIpcHistoryMessageV1>,
    pub next_cursor: Option<CanonicalRuntimeIpcHistoryCursorV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcReadResultV1 {
    ConversationListPage {
        conversations: Vec<CanonicalRuntimeIpcConversationSummaryV1>,
        next_cursor: Option<CanonicalRuntimeIpcConversationCursorV1>,
    },
    HistoryPage(CanonicalRuntimeIpcHistoryPageV1),
    Error {
        code: u16,
        message_utf8: Vec<u8>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcReadResponseContentV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub request_digest: [u8; 32],
    pub responder_device_id: [u8; 32],
    pub result: CanonicalRuntimeIpcReadResultV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeIpcReadResponseV1 {
    pub content: CanonicalRuntimeIpcReadResponseContentV1,
    pub signature: [u8; 64],
}

pub fn encode_runtime_ipc_read_request_v1(
    request: &CanonicalRuntimeIpcReadRequestV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_read_request(request)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 7);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &request.descriptor_id);
    push_bytes(&mut output, &request.request_id);
    push_bytes(&mut output, &request.bearer_token);
    push_unsigned(&mut output, request.issued_at_unix_seconds);
    push_unsigned(&mut output, request.expires_at_unix_seconds);
    encode_read_command(&mut output, &request.command);
    ensure_bound(
        &output,
        "read request",
        MAX_CANONICAL_RUNTIME_IPC_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_runtime_ipc_read_request_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcReadRequestV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "read request",
        MAX_CANONICAL_RUNTIME_IPC_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("runtime-ipc-read-request-v1", 7)?;
    decoder.expect_version("runtime-ipc-read-request-v1.schema_version")?;
    let request = CanonicalRuntimeIpcReadRequestV1 {
        descriptor_id: decoder.fixed_bytes("runtime-ipc-read-request-v1.descriptor_id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-read-request-v1.request_id")?,
        bearer_token: decoder.fixed_bytes("runtime-ipc-read-request-v1.bearer_token")?,
        issued_at_unix_seconds: decoder
            .unsigned("runtime-ipc-read-request-v1.issued_at_unix_seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("runtime-ipc-read-request-v1.expires_at_unix_seconds")?,
        command: decode_read_command(&mut decoder)?,
    };
    decoder.finish()?;
    validate_read_request(&request)?;
    Ok(request)
}

pub fn runtime_ipc_read_request_digest_v1(
    request: &CanonicalRuntimeIpcReadRequestV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_READ_REQUEST_DIGEST_DOMAIN,
        &encode_runtime_ipc_read_request_v1(request)?,
    ))
}

pub fn verify_runtime_ipc_read_request_for_descriptor_v1(
    request: &CanonicalRuntimeIpcReadRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_runtime_ipc_descriptor_at_v1(descriptor, now_unix_seconds)?;
    validate_read_request(request)?;
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

pub fn runtime_ipc_read_response_signing_bytes_v1(
    content: &CanonicalRuntimeIpcReadResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    Ok(prefixed(
        CANONICAL_RUNTIME_IPC_READ_RESPONSE_SIGNATURE_DOMAIN,
        &encode_read_response_content(content)?,
    ))
}

pub fn sign_runtime_ipc_read_response_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalRuntimeIpcReadRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
    result: CanonicalRuntimeIpcReadResultV1,
) -> Result<CanonicalSignedRuntimeIpcReadResponseV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_read_request_for_descriptor_v1(request, descriptor, now_unix_seconds)?;
    if device_signing_key.verifying_key().to_bytes() != descriptor.content.device_id {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    let content = CanonicalRuntimeIpcReadResponseContentV1 {
        descriptor_id: request.descriptor_id,
        request_id: request.request_id,
        request_digest: runtime_ipc_read_request_digest_v1(request)?,
        responder_device_id: descriptor.content.device_id,
        result,
    };
    let signature = device_signing_key
        .sign(&runtime_ipc_read_response_signing_bytes_v1(&content)?)
        .to_bytes();
    let response = CanonicalSignedRuntimeIpcReadResponseV1 { content, signature };
    verify_runtime_ipc_read_response_for_request_v1(&response, request, descriptor)?;
    Ok(response)
}

pub fn encode_signed_runtime_ipc_read_response_v1(
    response: &CanonicalSignedRuntimeIpcReadResponseV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_read_response_v1(response)?;
    let output = signed_envelope(
        &encode_read_response_content(&response.content)?,
        &response.signature,
    );
    ensure_bound(
        &output,
        "read response",
        MAX_CANONICAL_RUNTIME_IPC_READ_RESPONSE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_runtime_ipc_read_response_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeIpcReadResponseV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "read response",
        MAX_CANONICAL_RUNTIME_IPC_READ_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-runtime-ipc-read-response-v1", 3)?;
    decoder.expect_version("signed-runtime-ipc-read-response-v1.schema_version")?;
    let content = decode_read_response_content(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-runtime-ipc-read-response-v1.signature")?;
    decoder.finish()?;
    let response = CanonicalSignedRuntimeIpcReadResponseV1 { content, signature };
    verify_signed_runtime_ipc_read_response_v1(&response)?;
    Ok(response)
}

pub fn verify_signed_runtime_ipc_read_response_v1(
    response: &CanonicalSignedRuntimeIpcReadResponseV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_read_response_content(&response.content)?;
    verify_ed25519(
        &response.content.responder_device_id,
        &runtime_ipc_read_response_signing_bytes_v1(&response.content)?,
        &response.signature,
    )
}

pub fn verify_runtime_ipc_read_response_for_request_v1(
    response: &CanonicalSignedRuntimeIpcReadResponseV1,
    request: &CanonicalRuntimeIpcReadRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_read_response_v1(response)?;
    validate_read_request(request)?;
    verify_signed_runtime_ipc_descriptor_v1(descriptor)?;
    if response.content.descriptor_id != runtime_ipc_descriptor_id_v1(descriptor)?
        || response.content.descriptor_id != request.descriptor_id
        || response.content.request_id != request.request_id
        || response.content.request_digest != runtime_ipc_read_request_digest_v1(request)?
        || response.content.responder_device_id != descriptor.content.device_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    match (&request.command, &response.content.result) {
        (_, CanonicalRuntimeIpcReadResultV1::Error { .. }) => Ok(()),
        (
            CanonicalRuntimeIpcReadCommandV1::ConversationListPage { cursor, limit },
            CanonicalRuntimeIpcReadResultV1::ConversationListPage {
                conversations,
                next_cursor,
            },
        ) => verify_conversation_page_pair(cursor.as_ref(), *limit, conversations, next_cursor),
        (
            CanonicalRuntimeIpcReadCommandV1::HistoryPage {
                conversation_label_utf8,
                cursor,
                limit,
            },
            CanonicalRuntimeIpcReadResultV1::HistoryPage(page),
        ) => verify_history_page_pair(conversation_label_utf8, cursor.as_ref(), *limit, page),
        _ => Err(CanonicalRuntimeIpcError::ContextMismatch),
    }
}

pub fn runtime_ipc_read_response_id_v1(
    response: &CanonicalSignedRuntimeIpcReadResponseV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_READ_RESPONSE_ID_DOMAIN,
        &encode_signed_runtime_ipc_read_response_v1(response)?,
    ))
}

fn encode_read_response_content(
    content: &CanonicalRuntimeIpcReadResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_read_response_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 6);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.descriptor_id);
    push_bytes(&mut output, &content.request_id);
    push_bytes(&mut output, &content.request_digest);
    push_bytes(&mut output, &content.responder_device_id);
    encode_read_result(&mut output, &content.result);
    Ok(output)
}

fn decode_read_response_content(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcReadResponseContentV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-read-response-content-v1", 6)?;
    decoder.expect_version("runtime-ipc-read-response-content-v1.schema_version")?;
    let content = CanonicalRuntimeIpcReadResponseContentV1 {
        descriptor_id: decoder.fixed_bytes("runtime-ipc-read-response-content-v1.descriptor_id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-read-response-content-v1.request_id")?,
        request_digest: decoder
            .fixed_bytes("runtime-ipc-read-response-content-v1.request_digest")?,
        responder_device_id: decoder
            .fixed_bytes("runtime-ipc-read-response-content-v1.responder_device_id")?,
        result: decode_read_result(decoder)?,
    };
    validate_read_response_content(&content)?;
    Ok(content)
}

fn validate_read_request(
    request: &CanonicalRuntimeIpcReadRequestV1,
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
        CanonicalRuntimeIpcReadCommandV1::ConversationListPage { cursor, limit } => {
            if !(1..=MAX_CANONICAL_RUNTIME_IPC_CONVERSATION_PAGE_SIZE).contains(limit) {
                return Err(CanonicalRuntimeIpcError::InvalidRequest);
            }
            if let Some(cursor) = cursor {
                validate_conversation_cursor(cursor)?;
            }
        }
        CanonicalRuntimeIpcReadCommandV1::HistoryPage {
            conversation_label_utf8,
            cursor,
            limit,
        } => {
            validate_text(
                conversation_label_utf8,
                1,
                MAX_CANONICAL_RUNTIME_IPC_TEXT_BYTES,
                CanonicalRuntimeIpcError::InvalidRequest,
            )?;
            if !(1..=MAX_CANONICAL_RUNTIME_IPC_HISTORY_PAGE_SIZE).contains(limit) {
                return Err(CanonicalRuntimeIpcError::InvalidRequest);
            }
            if cursor.is_some_and(|value| value.snapshot_id == [0; 32] || value.before_index == 0) {
                return Err(CanonicalRuntimeIpcError::InvalidRequest);
            }
        }
    }
    Ok(())
}

fn validate_read_response_content(
    response: &CanonicalRuntimeIpcReadResponseContentV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if response.descriptor_id == [0; 32]
        || response.request_id == [0; 32]
        || response.request_digest == [0; 32]
        || response.responder_device_id == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    match &response.result {
        CanonicalRuntimeIpcReadResultV1::ConversationListPage {
            conversations,
            next_cursor,
        } => validate_conversation_page(conversations, next_cursor.as_ref()),
        CanonicalRuntimeIpcReadResultV1::HistoryPage(page) => validate_history_page(page),
        CanonicalRuntimeIpcReadResultV1::Error { code, message_utf8 } => {
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

fn validate_conversation_cursor(
    cursor: &CanonicalRuntimeIpcConversationCursorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_text(
        &cursor.conversation_label_utf8,
        1,
        MAX_CANONICAL_RUNTIME_IPC_TEXT_BYTES,
        CanonicalRuntimeIpcError::InvalidRequest,
    )?;
    if cursor.peer_account_id == [0; 32] {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    Ok(())
}

fn validate_conversation_page(
    conversations: &[CanonicalRuntimeIpcConversationSummaryV1],
    next_cursor: Option<&CanonicalRuntimeIpcConversationCursorV1>,
) -> Result<(), CanonicalRuntimeIpcError> {
    if conversations.len() > usize::from(MAX_CANONICAL_RUNTIME_IPC_CONVERSATION_PAGE_SIZE) {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    for conversation in conversations {
        validate_conversation_summary(conversation)?;
    }
    if conversations.windows(2).any(|pair| {
        compare_conversation_key(
            &pair[0].conversation_label_utf8,
            &pair[0].peer_account_id,
            &pair[1].conversation_label_utf8,
            &pair[1].peer_account_id,
        ) != Ordering::Less
    }) {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    if let Some(cursor) = next_cursor {
        validate_conversation_cursor(cursor)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
        let Some(last) = conversations.last() else {
            return Err(CanonicalRuntimeIpcError::InvalidResponse);
        };
        if cursor.conversation_label_utf8 != last.conversation_label_utf8
            || cursor.peer_account_id != last.peer_account_id
        {
            return Err(CanonicalRuntimeIpcError::InvalidResponse);
        }
    }
    Ok(())
}

fn validate_conversation_summary(
    summary: &CanonicalRuntimeIpcConversationSummaryV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_text(
        &summary.contact_id_utf8,
        1,
        MAX_CANONICAL_RUNTIME_IPC_TEXT_BYTES,
        CanonicalRuntimeIpcError::InvalidResponse,
    )?;
    validate_text(
        &summary.conversation_label_utf8,
        1,
        MAX_CANONICAL_RUNTIME_IPC_TEXT_BYTES,
        CanonicalRuntimeIpcError::InvalidResponse,
    )?;
    let label = std::str::from_utf8(&summary.conversation_label_utf8)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    if summary.conversation_id != *ConversationId::from_label(label).as_bytes()
        || summary.peer_account_id == [0; 32]
        || summary.peer_device_id == [0; 32]
        || summary.endpoint_candidates.is_empty()
        || summary.endpoint_candidates.len() > MAX_CANONICAL_RUNTIME_IPC_ENDPOINT_CANDIDATES
        || usize::from(summary.endpoint_candidate_count) != summary.endpoint_candidates.len()
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    let usable = summary
        .endpoint_candidates
        .iter()
        .filter(|candidate| candidate.state == CanonicalRuntimeIpcEndpointStateV1::Usable)
        .count();
    let stale = summary
        .endpoint_candidates
        .iter()
        .filter(|candidate| candidate.state == CanonicalRuntimeIpcEndpointStateV1::Stale)
        .count();
    let quarantined = summary
        .endpoint_candidates
        .iter()
        .filter(|candidate| candidate.state == CanonicalRuntimeIpcEndpointStateV1::Quarantined)
        .count();
    if usize::from(summary.usable_endpoint_candidate_count) != usable
        || usize::from(summary.stale_endpoint_candidate_count) != stale
        || usize::from(summary.quarantined_endpoint_candidate_count) != quarantined
        || usable + stale + quarantined != summary.endpoint_candidates.len()
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    for candidate in &summary.endpoint_candidates {
        validate_endpoint_candidate(candidate)?;
    }
    if !summary.endpoint_candidates[0].primary
        || summary
            .endpoint_candidates
            .iter()
            .skip(1)
            .any(|candidate| candidate.primary)
        || summary.endpoint_candidates[0].peer_device_id != summary.peer_device_id
        || summary.endpoint_candidates[0].route_policy != summary.route_policy
        || summary.endpoint_candidates.windows(2).any(|pair| {
            (!pair[0].primary && pair[1].primary)
                || (pair[0].primary == pair[1].primary
                    && pair[0].peer_device_id >= pair[1].peer_device_id)
        })
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    match (summary.message_count, &summary.latest_message) {
        (0, None) => {}
        (0, Some(_)) | (_, None) => return Err(CanonicalRuntimeIpcError::InvalidResponse),
        (_, Some(preview)) => validate_message_preview(preview)?,
    }
    Ok(())
}

fn validate_endpoint_candidate(
    candidate: &CanonicalRuntimeIpcEndpointCandidateV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if candidate.peer_device_id == [0; 32] {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    validate_text(
        &candidate.descriptor_path_utf8,
        1,
        MAX_CANONICAL_RUNTIME_IPC_PATH_BYTES,
        CanonicalRuntimeIpcError::InvalidResponse,
    )?;
    validate_text(
        &candidate.detail_utf8,
        1,
        MAX_CANONICAL_RUNTIME_IPC_TEXT_BYTES,
        CanonicalRuntimeIpcError::InvalidResponse,
    )?;
    for value in [
        candidate.publication_channel_id_utf8.as_deref(),
        candidate.publication_conflict_proof_id_utf8.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        validate_text(
            value,
            1,
            MAX_CANONICAL_RUNTIME_IPC_TEXT_BYTES,
            CanonicalRuntimeIpcError::InvalidResponse,
        )?;
    }
    if candidate.observed_publication_generation.is_some()
        != candidate.observed_at_unix_seconds.is_some()
        || (candidate.observed_publication_generation.is_some()
            && candidate.publication_channel_id_utf8.is_none())
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    let conflict_fields = [
        candidate.publication_conflict_generation.is_some(),
        candidate.publication_conflict_proof_id_utf8.is_some(),
        candidate
            .publication_conflict_detected_at_unix_seconds
            .is_some(),
    ];
    let has_conflict = conflict_fields.iter().all(|value| *value);
    if conflict_fields.iter().any(|value| *value) != has_conflict
        || has_conflict != (candidate.state == CanonicalRuntimeIpcEndpointStateV1::Quarantined)
        || (candidate.state == CanonicalRuntimeIpcEndpointStateV1::Usable
            && (candidate.authority_revision.is_none()
                || candidate.publication_channel_id_utf8.is_none()))
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn validate_message_preview(
    preview: &CanonicalRuntimeIpcMessagePreviewV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if preview.event_id == [0; 32] || preview.author_account_id == [0; 32] {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    validate_text(
        &preview.body_utf8,
        1,
        MAX_CANONICAL_RUNTIME_IPC_PREVIEW_BYTES,
        CanonicalRuntimeIpcError::InvalidResponse,
    )
}

fn validate_history_page(
    page: &CanonicalRuntimeIpcHistoryPageV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if page.conversation_id == [0; 32]
        || page.snapshot_id == [0; 32]
        || page.messages.len() > usize::from(MAX_CANONICAL_RUNTIME_IPC_HISTORY_PAGE_SIZE)
        || usize::try_from(page.total_messages).map_or(true, |total| total < page.messages.len())
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    let mut aggregate = 0_usize;
    let mut event_ids = Vec::with_capacity(page.messages.len());
    for message in &page.messages {
        if message.event_id == [0; 32]
            || message.author_account_id == [0; 32]
            || message.author_device_id == [0; 32]
        {
            return Err(CanonicalRuntimeIpcError::InvalidResponse);
        }
        validate_text(
            &message.body_utf8,
            1,
            MAX_CANONICAL_RUNTIME_IPC_HISTORY_PAGE_BODY_BYTES,
            CanonicalRuntimeIpcError::InvalidResponse,
        )?;
        aggregate = aggregate
            .checked_add(message.body_utf8.len())
            .ok_or(CanonicalRuntimeIpcError::InvalidResponse)?;
        event_ids.push(message.event_id);
    }
    event_ids.sort_unstable();
    if aggregate > MAX_CANONICAL_RUNTIME_IPC_HISTORY_PAGE_BODY_BYTES
        || event_ids.windows(2).any(|pair| pair[0] == pair[1])
        || page.next_cursor.is_some_and(|cursor| {
            cursor.snapshot_id != page.snapshot_id
                || cursor.before_index == 0
                || cursor.before_index >= page.total_messages
        })
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn verify_conversation_page_pair(
    request_cursor: Option<&CanonicalRuntimeIpcConversationCursorV1>,
    limit: u16,
    conversations: &[CanonicalRuntimeIpcConversationSummaryV1],
    next_cursor: &Option<CanonicalRuntimeIpcConversationCursorV1>,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_conversation_page(conversations, next_cursor.as_ref())?;
    if conversations.len() > usize::from(limit)
        || (next_cursor.is_some() && conversations.len() != usize::from(limit))
        || request_cursor.is_some_and(|cursor| {
            conversations.first().is_some_and(|first| {
                compare_conversation_key(
                    &cursor.conversation_label_utf8,
                    &cursor.peer_account_id,
                    &first.conversation_label_utf8,
                    &first.peer_account_id,
                ) != Ordering::Less
            })
        })
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    Ok(())
}

fn verify_history_page_pair(
    conversation_label_utf8: &[u8],
    request_cursor: Option<&CanonicalRuntimeIpcHistoryCursorV1>,
    limit: u16,
    page: &CanonicalRuntimeIpcHistoryPageV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_history_page(page)?;
    let label = std::str::from_utf8(conversation_label_utf8)
        .map_err(|_| CanonicalRuntimeIpcError::ContextMismatch)?;
    let end = request_cursor.map_or(page.total_messages, |cursor| cursor.before_index);
    let message_count = u32::try_from(page.messages.len())
        .map_err(|_| CanonicalRuntimeIpcError::ContextMismatch)?;
    if page.conversation_id != *ConversationId::from_label(label).as_bytes()
        || page.messages.len() > usize::from(limit)
        || request_cursor.is_some_and(|cursor| {
            cursor.snapshot_id != page.snapshot_id || cursor.before_index > page.total_messages
        })
        || match page.next_cursor {
            Some(next) => {
                page.messages.is_empty()
                    || next.before_index.checked_add(message_count) != Some(end)
            }
            None => message_count != end,
        }
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    Ok(())
}

fn compare_conversation_key(
    left_label: &[u8],
    left_account: &[u8; 32],
    right_label: &[u8],
    right_account: &[u8; 32],
) -> Ordering {
    left_label
        .cmp(right_label)
        .then_with(|| left_account.cmp(right_account))
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

fn encode_read_command(output: &mut Vec<u8>, command: &CanonicalRuntimeIpcReadCommandV1) {
    match command {
        CanonicalRuntimeIpcReadCommandV1::ConversationListPage { cursor, limit } => {
            push_array_header(output, 3);
            push_unsigned(output, COMMAND_CONVERSATION_LIST_PAGE);
            encode_optional_conversation_cursor(output, cursor.as_ref());
            push_unsigned(output, u64::from(*limit));
        }
        CanonicalRuntimeIpcReadCommandV1::HistoryPage {
            conversation_label_utf8,
            cursor,
            limit,
        } => {
            push_array_header(output, 4);
            push_unsigned(output, COMMAND_HISTORY_PAGE);
            push_bytes(output, conversation_label_utf8);
            encode_optional_history_cursor(output, cursor.as_ref());
            push_unsigned(output, u64::from(*limit));
        }
    }
}

fn decode_read_command(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcReadCommandV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-read-command-v1")?;
    match (
        decoder.unsigned("runtime-ipc-read-command-v1.variant")?,
        length,
    ) {
        (COMMAND_CONVERSATION_LIST_PAGE, 3) => {
            Ok(CanonicalRuntimeIpcReadCommandV1::ConversationListPage {
                cursor: decode_optional_conversation_cursor(decoder)?,
                limit: u16::try_from(
                    decoder.unsigned("runtime-ipc-read-command-v1.conversation-limit")?,
                )
                .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?,
            })
        }
        (COMMAND_HISTORY_PAGE, 4) => Ok(CanonicalRuntimeIpcReadCommandV1::HistoryPage {
            conversation_label_utf8: decoder
                .bytes("runtime-ipc-read-command-v1.conversation-label-utf8")?
                .to_vec(),
            cursor: decode_optional_history_cursor(decoder)?,
            limit: u16::try_from(decoder.unsigned("runtime-ipc-read-command-v1.history-limit")?)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?,
        }),
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_read_result(output: &mut Vec<u8>, result: &CanonicalRuntimeIpcReadResultV1) {
    match result {
        CanonicalRuntimeIpcReadResultV1::ConversationListPage {
            conversations,
            next_cursor,
        } => {
            push_array_header(output, 3);
            push_unsigned(output, RESULT_CONVERSATION_LIST_PAGE);
            push_array_header(output, conversations.len() as u64);
            for conversation in conversations {
                encode_conversation_summary(output, conversation);
            }
            encode_optional_conversation_cursor(output, next_cursor.as_ref());
        }
        CanonicalRuntimeIpcReadResultV1::HistoryPage(page) => {
            push_array_header(output, 2);
            push_unsigned(output, RESULT_HISTORY_PAGE);
            encode_history_page(output, page);
        }
        CanonicalRuntimeIpcReadResultV1::Error { code, message_utf8 } => {
            push_array_header(output, 3);
            push_unsigned(output, RESULT_ERROR);
            push_unsigned(output, u64::from(*code));
            push_bytes(output, message_utf8);
        }
    }
}

fn decode_read_result(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcReadResultV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-read-result-v1")?;
    match (
        decoder.unsigned("runtime-ipc-read-result-v1.variant")?,
        length,
    ) {
        (RESULT_CONVERSATION_LIST_PAGE, 3) => {
            let count = decoder.array("runtime-ipc-read-result-v1.conversations")?;
            if count > usize::from(MAX_CANONICAL_RUNTIME_IPC_CONVERSATION_PAGE_SIZE) {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            let mut conversations = Vec::with_capacity(count);
            for _ in 0..count {
                conversations.push(decode_conversation_summary(decoder)?);
            }
            Ok(CanonicalRuntimeIpcReadResultV1::ConversationListPage {
                conversations,
                next_cursor: decode_optional_conversation_cursor(decoder)?,
            })
        }
        (RESULT_HISTORY_PAGE, 2) => Ok(CanonicalRuntimeIpcReadResultV1::HistoryPage(
            decode_history_page(decoder)?,
        )),
        (RESULT_ERROR, 3) => Ok(CanonicalRuntimeIpcReadResultV1::Error {
            code: u16::try_from(decoder.unsigned("runtime-ipc-read-result-v1.error-code")?)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            message_utf8: decoder
                .bytes("runtime-ipc-read-result-v1.error-message-utf8")?
                .to_vec(),
        }),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_conversation_summary(
    output: &mut Vec<u8>,
    summary: &CanonicalRuntimeIpcConversationSummaryV1,
) {
    push_array_header(output, 13);
    push_bytes(output, &summary.contact_id_utf8);
    push_bytes(output, &summary.conversation_label_utf8);
    push_bytes(output, &summary.conversation_id);
    push_bytes(output, &summary.peer_account_id);
    push_bytes(output, &summary.peer_device_id);
    push_unsigned(output, u64::from(summary.endpoint_candidate_count));
    push_unsigned(output, u64::from(summary.usable_endpoint_candidate_count));
    push_unsigned(output, u64::from(summary.stale_endpoint_candidate_count));
    push_unsigned(
        output,
        u64::from(summary.quarantined_endpoint_candidate_count),
    );
    push_array_header(output, summary.endpoint_candidates.len() as u64);
    for candidate in &summary.endpoint_candidates {
        encode_endpoint_candidate(output, candidate);
    }
    encode_route_policy(output, summary.route_policy);
    push_unsigned(output, u64::from(summary.message_count));
    encode_optional_message_preview(output, summary.latest_message.as_ref());
}

fn decode_conversation_summary(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcConversationSummaryV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-conversation-summary-v1", 13)?;
    let contact_id_utf8 = decoder
        .bytes("runtime-ipc-conversation-summary-v1.contact-id-utf8")?
        .to_vec();
    let conversation_label_utf8 = decoder
        .bytes("runtime-ipc-conversation-summary-v1.conversation-label-utf8")?
        .to_vec();
    let conversation_id = decoder.fixed_bytes("runtime-ipc-conversation-summary-v1.id")?;
    let peer_account_id =
        decoder.fixed_bytes("runtime-ipc-conversation-summary-v1.peer-account-id")?;
    let peer_device_id =
        decoder.fixed_bytes("runtime-ipc-conversation-summary-v1.peer-device-id")?;
    let endpoint_candidate_count = decode_u8(
        decoder,
        "runtime-ipc-conversation-summary-v1.endpoint-count",
    )?;
    let usable_endpoint_candidate_count = decode_u8(
        decoder,
        "runtime-ipc-conversation-summary-v1.usable-endpoint-count",
    )?;
    let stale_endpoint_candidate_count = decode_u8(
        decoder,
        "runtime-ipc-conversation-summary-v1.stale-endpoint-count",
    )?;
    let quarantined_endpoint_candidate_count = decode_u8(
        decoder,
        "runtime-ipc-conversation-summary-v1.quarantined-endpoint-count",
    )?;
    let count = decoder.array("runtime-ipc-conversation-summary-v1.endpoint-candidates")?;
    if count > MAX_CANONICAL_RUNTIME_IPC_ENDPOINT_CANDIDATES {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    let mut endpoint_candidates = Vec::with_capacity(count);
    for _ in 0..count {
        endpoint_candidates.push(decode_endpoint_candidate(decoder)?);
    }
    Ok(CanonicalRuntimeIpcConversationSummaryV1 {
        contact_id_utf8,
        conversation_label_utf8,
        conversation_id,
        peer_account_id,
        peer_device_id,
        endpoint_candidate_count,
        usable_endpoint_candidate_count,
        stale_endpoint_candidate_count,
        quarantined_endpoint_candidate_count,
        endpoint_candidates,
        route_policy: decode_route_policy(decoder)?,
        message_count: u32::try_from(
            decoder.unsigned("runtime-ipc-conversation-summary-v1.message-count")?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
        latest_message: decode_optional_message_preview(decoder)?,
    })
}

fn encode_endpoint_candidate(
    output: &mut Vec<u8>,
    candidate: &CanonicalRuntimeIpcEndpointCandidateV1,
) {
    push_array_header(output, 13);
    push_bytes(output, &candidate.peer_device_id);
    encode_bool(output, candidate.primary);
    encode_route_policy(output, candidate.route_policy);
    push_bytes(output, &candidate.descriptor_path_utf8);
    encode_endpoint_state(output, candidate.state);
    encode_optional_u64(output, candidate.authority_revision);
    encode_optional_bytes(output, candidate.publication_channel_id_utf8.as_deref());
    encode_optional_u64(output, candidate.observed_publication_generation);
    encode_optional_u64(output, candidate.observed_at_unix_seconds);
    encode_optional_u64(output, candidate.publication_conflict_generation);
    encode_optional_bytes(
        output,
        candidate.publication_conflict_proof_id_utf8.as_deref(),
    );
    encode_optional_u64(
        output,
        candidate.publication_conflict_detected_at_unix_seconds,
    );
    push_bytes(output, &candidate.detail_utf8);
}

fn decode_endpoint_candidate(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcEndpointCandidateV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-endpoint-candidate-v1", 13)?;
    Ok(CanonicalRuntimeIpcEndpointCandidateV1 {
        peer_device_id: decoder.fixed_bytes("runtime-ipc-endpoint-candidate-v1.peer-device-id")?,
        primary: decode_bool(decoder, "runtime-ipc-endpoint-candidate-v1.primary")?,
        route_policy: decode_route_policy(decoder)?,
        descriptor_path_utf8: decoder
            .bytes("runtime-ipc-endpoint-candidate-v1.descriptor-path-utf8")?
            .to_vec(),
        state: decode_endpoint_state(decoder)?,
        authority_revision: decode_optional_u64(decoder)?,
        publication_channel_id_utf8: decode_optional_bytes(decoder)?,
        observed_publication_generation: decode_optional_u64(decoder)?,
        observed_at_unix_seconds: decode_optional_u64(decoder)?,
        publication_conflict_generation: decode_optional_u64(decoder)?,
        publication_conflict_proof_id_utf8: decode_optional_bytes(decoder)?,
        publication_conflict_detected_at_unix_seconds: decode_optional_u64(decoder)?,
        detail_utf8: decoder
            .bytes("runtime-ipc-endpoint-candidate-v1.detail-utf8")?
            .to_vec(),
    })
}

fn encode_history_page(output: &mut Vec<u8>, page: &CanonicalRuntimeIpcHistoryPageV1) {
    push_array_header(output, 5);
    push_bytes(output, &page.conversation_id);
    push_bytes(output, &page.snapshot_id);
    push_unsigned(output, u64::from(page.total_messages));
    push_array_header(output, page.messages.len() as u64);
    for message in &page.messages {
        push_array_header(output, 5);
        push_bytes(output, &message.event_id);
        push_bytes(output, &message.author_account_id);
        push_bytes(output, &message.author_device_id);
        push_unsigned(output, message.author_sequence);
        push_bytes(output, &message.body_utf8);
    }
    encode_optional_history_cursor(output, page.next_cursor.as_ref());
}

fn decode_history_page(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcHistoryPageV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-history-page-v1", 5)?;
    let conversation_id = decoder.fixed_bytes("runtime-ipc-history-page-v1.conversation-id")?;
    let snapshot_id = decoder.fixed_bytes("runtime-ipc-history-page-v1.snapshot-id")?;
    let total_messages =
        u32::try_from(decoder.unsigned("runtime-ipc-history-page-v1.total-messages")?)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    let count = decoder.array("runtime-ipc-history-page-v1.messages")?;
    if count > usize::from(MAX_CANONICAL_RUNTIME_IPC_HISTORY_PAGE_SIZE) {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    let mut messages = Vec::with_capacity(count);
    for _ in 0..count {
        decoder.expect_array("runtime-ipc-history-message-v1", 5)?;
        messages.push(CanonicalRuntimeIpcHistoryMessageV1 {
            event_id: decoder.fixed_bytes("runtime-ipc-history-message-v1.event-id")?,
            author_account_id: decoder
                .fixed_bytes("runtime-ipc-history-message-v1.author-account-id")?,
            author_device_id: decoder
                .fixed_bytes("runtime-ipc-history-message-v1.author-device-id")?,
            author_sequence: decoder.unsigned("runtime-ipc-history-message-v1.author-sequence")?,
            body_utf8: decoder
                .bytes("runtime-ipc-history-message-v1.body-utf8")?
                .to_vec(),
        });
    }
    Ok(CanonicalRuntimeIpcHistoryPageV1 {
        conversation_id,
        snapshot_id,
        total_messages,
        messages,
        next_cursor: decode_optional_history_cursor(decoder)?,
    })
}

fn encode_optional_conversation_cursor(
    output: &mut Vec<u8>,
    cursor: Option<&CanonicalRuntimeIpcConversationCursorV1>,
) {
    match cursor {
        None => {
            push_array_header(output, 1);
            push_unsigned(output, OPTION_NONE);
        }
        Some(cursor) => {
            push_array_header(output, 2);
            push_unsigned(output, OPTION_SOME);
            push_array_header(output, 2);
            push_bytes(output, &cursor.conversation_label_utf8);
            push_bytes(output, &cursor.peer_account_id);
        }
    }
}

fn decode_optional_conversation_cursor(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Option<CanonicalRuntimeIpcConversationCursorV1>, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-conversation-cursor-option-v1")?;
    match (
        decoder.unsigned("runtime-ipc-conversation-cursor-option-v1.variant")?,
        length,
    ) {
        (OPTION_NONE, 1) => Ok(None),
        (OPTION_SOME, 2) => {
            decoder.expect_array("runtime-ipc-conversation-cursor-v1", 2)?;
            Ok(Some(CanonicalRuntimeIpcConversationCursorV1 {
                conversation_label_utf8: decoder
                    .bytes("runtime-ipc-conversation-cursor-v1.label-utf8")?
                    .to_vec(),
                peer_account_id: decoder
                    .fixed_bytes("runtime-ipc-conversation-cursor-v1.peer-account-id")?,
            }))
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_optional_history_cursor(
    output: &mut Vec<u8>,
    cursor: Option<&CanonicalRuntimeIpcHistoryCursorV1>,
) {
    match cursor {
        None => {
            push_array_header(output, 1);
            push_unsigned(output, OPTION_NONE);
        }
        Some(cursor) => {
            push_array_header(output, 2);
            push_unsigned(output, OPTION_SOME);
            push_array_header(output, 2);
            push_bytes(output, &cursor.snapshot_id);
            push_unsigned(output, u64::from(cursor.before_index));
        }
    }
}

fn decode_optional_history_cursor(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Option<CanonicalRuntimeIpcHistoryCursorV1>, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-history-cursor-option-v1")?;
    match (
        decoder.unsigned("runtime-ipc-history-cursor-option-v1.variant")?,
        length,
    ) {
        (OPTION_NONE, 1) => Ok(None),
        (OPTION_SOME, 2) => {
            decoder.expect_array("runtime-ipc-history-cursor-v1", 2)?;
            Ok(Some(CanonicalRuntimeIpcHistoryCursorV1 {
                snapshot_id: decoder.fixed_bytes("runtime-ipc-history-cursor-v1.snapshot-id")?,
                before_index: u32::try_from(
                    decoder.unsigned("runtime-ipc-history-cursor-v1.before-index")?,
                )
                .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?,
            }))
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_optional_message_preview(
    output: &mut Vec<u8>,
    preview: Option<&CanonicalRuntimeIpcMessagePreviewV1>,
) {
    match preview {
        None => {
            push_array_header(output, 1);
            push_unsigned(output, OPTION_NONE);
        }
        Some(preview) => {
            push_array_header(output, 2);
            push_unsigned(output, OPTION_SOME);
            push_array_header(output, 4);
            push_bytes(output, &preview.event_id);
            push_bytes(output, &preview.author_account_id);
            push_bytes(output, &preview.body_utf8);
            encode_bool(output, preview.truncated);
        }
    }
}

fn decode_optional_message_preview(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Option<CanonicalRuntimeIpcMessagePreviewV1>, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-message-preview-option-v1")?;
    match (
        decoder.unsigned("runtime-ipc-message-preview-option-v1.variant")?,
        length,
    ) {
        (OPTION_NONE, 1) => Ok(None),
        (OPTION_SOME, 2) => {
            decoder.expect_array("runtime-ipc-message-preview-v1", 4)?;
            Ok(Some(CanonicalRuntimeIpcMessagePreviewV1 {
                event_id: decoder.fixed_bytes("runtime-ipc-message-preview-v1.event-id")?,
                author_account_id: decoder
                    .fixed_bytes("runtime-ipc-message-preview-v1.author-account-id")?,
                body_utf8: decoder
                    .bytes("runtime-ipc-message-preview-v1.body-utf8")?
                    .to_vec(),
                truncated: decode_bool(decoder, "runtime-ipc-message-preview-v1.truncated")?,
            }))
        }
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

fn encode_optional_bytes(output: &mut Vec<u8>, value: Option<&[u8]>) {
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

fn decode_optional_bytes(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Option<Vec<u8>>, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-optional-bytes-v1")?;
    match (
        decoder.unsigned("runtime-ipc-optional-bytes-v1.variant")?,
        length,
    ) {
        (OPTION_NONE, 1) => Ok(None),
        (OPTION_SOME, 2) => Ok(Some(
            decoder
                .bytes("runtime-ipc-optional-bytes-v1.value")?
                .to_vec(),
        )),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_route_policy(output: &mut Vec<u8>, policy: CanonicalRuntimeIpcRoutePolicyV1) {
    push_unsigned(
        output,
        match policy {
            CanonicalRuntimeIpcRoutePolicyV1::Auto => ROUTE_AUTO,
            CanonicalRuntimeIpcRoutePolicyV1::DirectOnly => ROUTE_DIRECT_ONLY,
            CanonicalRuntimeIpcRoutePolicyV1::RelayOnly => ROUTE_RELAY_ONLY,
        },
    );
}

fn decode_route_policy(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcRoutePolicyV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-route-policy-v1")? {
        ROUTE_AUTO => Ok(CanonicalRuntimeIpcRoutePolicyV1::Auto),
        ROUTE_DIRECT_ONLY => Ok(CanonicalRuntimeIpcRoutePolicyV1::DirectOnly),
        ROUTE_RELAY_ONLY => Ok(CanonicalRuntimeIpcRoutePolicyV1::RelayOnly),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_endpoint_state(output: &mut Vec<u8>, state: CanonicalRuntimeIpcEndpointStateV1) {
    push_unsigned(
        output,
        match state {
            CanonicalRuntimeIpcEndpointStateV1::Usable => ENDPOINT_USABLE,
            CanonicalRuntimeIpcEndpointStateV1::Stale => ENDPOINT_STALE,
            CanonicalRuntimeIpcEndpointStateV1::Quarantined => ENDPOINT_QUARANTINED,
        },
    );
}

fn decode_endpoint_state(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcEndpointStateV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-endpoint-state-v1")? {
        ENDPOINT_USABLE => Ok(CanonicalRuntimeIpcEndpointStateV1::Usable),
        ENDPOINT_STALE => Ok(CanonicalRuntimeIpcEndpointStateV1::Stale),
        ENDPOINT_QUARANTINED => Ok(CanonicalRuntimeIpcEndpointStateV1::Quarantined),
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

fn decode_u8(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
) -> Result<u8, CanonicalRuntimeIpcError> {
    u8::try_from(decoder.unsigned(field)?).map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)
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
                "/../../CANONICAL-RUNTIME-IPC-READ-VECTORS.json"
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
    fn canonical_runtime_ipc_read_vectors_bind_exact_requests() -> TestResult {
        let (vectors, core) = vectors()?;
        let descriptor = decode_and_verify_signed_runtime_ipc_descriptor_v1(&hex_at(
            &core,
            "/expected/descriptor/signed_cbor_hex",
        )?)?;
        let signing_key = SigningKey::from_bytes(&fixed(&core, "/input/device_secret_key_hex")?);
        for surface in ["conversation_list", "history_page"] {
            let request_path = format!("/expected/{surface}/request/cbor_hex");
            let response_path = format!("/expected/{surface}/response/signed_cbor_hex");
            let request =
                decode_and_verify_runtime_ipc_read_request_v1(&hex_at(&vectors, &request_path)?)?;
            let response = decode_and_verify_signed_runtime_ipc_read_response_v1(&hex_at(
                &vectors,
                &response_path,
            )?)?;
            verify_runtime_ipc_read_request_for_descriptor_v1(
                &request,
                &descriptor,
                request.issued_at_unix_seconds,
            )?;
            verify_runtime_ipc_read_response_for_request_v1(&response, &request, &descriptor)?;
            assert_eq!(
                encode_runtime_ipc_read_request_v1(&request)?,
                hex_at(&vectors, &request_path)?
            );
            assert_eq!(
                encode_signed_runtime_ipc_read_response_v1(&sign_runtime_ipc_read_response_v1(
                    &signing_key,
                    &request,
                    &descriptor,
                    request.issued_at_unix_seconds,
                    response.content.result.clone(),
                )?)?,
                hex_at(&vectors, &response_path)?
            );
        }
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_read_vector_ids_match() -> TestResult {
        let (vectors, _) = vectors()?;
        for surface in ["conversation_list", "history_page"] {
            let request = decode_and_verify_runtime_ipc_read_request_v1(&hex_at(
                &vectors,
                &format!("/expected/{surface}/request/cbor_hex"),
            )?)?;
            let response = decode_and_verify_signed_runtime_ipc_read_response_v1(&hex_at(
                &vectors,
                &format!("/expected/{surface}/response/signed_cbor_hex"),
            )?)?;
            assert_eq!(
                runtime_ipc_read_request_digest_v1(&request)?,
                fixed(
                    &vectors,
                    &format!("/expected/{surface}/request/object_id_hex"),
                )?
            );
            assert_eq!(
                runtime_ipc_read_response_id_v1(&response)?,
                fixed(
                    &vectors,
                    &format!("/expected/{surface}/response/object_id_hex"),
                )?
            );
        }
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_read_substitutions_fail_closed() -> TestResult {
        let (vectors, core) = vectors()?;
        let descriptor = decode_and_verify_signed_runtime_ipc_descriptor_v1(&hex_at(
            &core,
            "/expected/descriptor/signed_cbor_hex",
        )?)?;
        let request = decode_and_verify_runtime_ipc_read_request_v1(&hex_at(
            &vectors,
            "/expected/history_page/request/cbor_hex",
        )?)?;
        let response = decode_and_verify_signed_runtime_ipc_read_response_v1(&hex_at(
            &vectors,
            "/expected/history_page/response/signed_cbor_hex",
        )?)?;
        let mut other_request = request.clone();
        if let CanonicalRuntimeIpcReadCommandV1::HistoryPage {
            conversation_label_utf8,
            ..
        } = &mut other_request.command
        {
            conversation_label_utf8.push(b'x');
        }
        assert!(
            verify_runtime_ipc_read_response_for_request_v1(
                &response,
                &other_request,
                &descriptor,
            )
            .is_err()
        );
        let mut other_response = response;
        if let CanonicalRuntimeIpcReadResultV1::HistoryPage(page) =
            &mut other_response.content.result
        {
            page.snapshot_id[0] ^= 1;
        }
        assert!(verify_signed_runtime_ipc_read_response_v1(&other_response).is_err());
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_read_pagination_fails_closed() -> TestResult {
        let (vectors, core) = vectors()?;
        let descriptor = decode_and_verify_signed_runtime_ipc_descriptor_v1(&hex_at(
            &core,
            "/expected/descriptor/signed_cbor_hex",
        )?)?;
        let signing_key = SigningKey::from_bytes(&fixed(&core, "/input/device_secret_key_hex")?);
        let request = decode_and_verify_runtime_ipc_read_request_v1(&hex_at(
            &vectors,
            "/expected/history_page/request/cbor_hex",
        )?)?;
        let response = decode_and_verify_signed_runtime_ipc_read_response_v1(&hex_at(
            &vectors,
            "/expected/history_page/response/signed_cbor_hex",
        )?)?;
        let mut wrong_result = response.content.result;
        if let CanonicalRuntimeIpcReadResultV1::HistoryPage(page) = &mut wrong_result
            && let Some(cursor) = &mut page.next_cursor
        {
            cursor.before_index = 2;
        }
        assert!(
            sign_runtime_ipc_read_response_v1(
                &signing_key,
                &request,
                &descriptor,
                request.issued_at_unix_seconds,
                wrong_result,
            )
            .is_err()
        );
        Ok(())
    }
}
