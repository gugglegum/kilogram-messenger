//! Inactive deterministic-CBOR snapshot-bound outbox projection for local
//! runtime IPC.
//!
//! The active runtime remains Postcard IPC version 26. A continuation cursor
//! binds an exact content-derived snapshot ID and start index so a changing
//! outbox cannot silently create gaps or duplicates between pages.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::{
    canonical_account_v1::{AccountDecoder, signed_envelope},
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

pub const MAX_CANONICAL_RUNTIME_IPC_OUTBOX_REQUEST_BYTES: usize = 64 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_OUTBOX_RESPONSE_BYTES: usize = 128 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_OUTBOX_PAGE_SIZE: u16 = 128;
pub const MAX_CANONICAL_RUNTIME_IPC_OUTBOX_SNAPSHOT_ITEMS: u32 = 4_096;
pub const MAX_CANONICAL_RUNTIME_IPC_OUTBOX_CONTACTS: u32 = 256;
pub const MAX_CANONICAL_RUNTIME_IPC_OUTBOX_RETRY_RECORDS: u32 = 4_096;

pub const CANONICAL_RUNTIME_IPC_OUTBOX_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-outbox-request-digest:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_OUTBOX_RESPONSE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-outbox-response-signature:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_OUTBOX_RESPONSE_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-outbox-response-id:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_OUTBOX_SNAPSHOT_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-outbox-snapshot-id:cbor-v1\0";

const COMMAND_OUTBOX_STATUS_PAGE: u64 = 0;
const RESULT_OUTBOX_STATUS_PAGE: u64 = 0;
const RESULT_ERROR: u64 = 1;
const OPTION_NONE: u64 = 0;
const OPTION_SOME: u64 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOutboxCursorV1 {
    pub snapshot_id: [u8; 32],
    pub start_index: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcOutboxCommandV1 {
    OutboxStatusPage {
        cursor: Option<CanonicalRuntimeIpcOutboxCursorV1>,
        limit: u16,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOutboxRequestV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub bearer_token: [u8; 32],
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub command: CanonicalRuntimeIpcOutboxCommandV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcOutboxStateV1 {
    Queued,
    Materialized,
    Delivered,
    MailboxPending,
    MailboxStored,
    MailboxExpired,
    MailboxFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOutboxCountsV1 {
    pub contact_count: u32,
    pub queue_count: u32,
    pub pending_count: u32,
    pub materialized_count: u32,
    pub delivered_count: u32,
    pub mailbox_pending_count: u32,
    pub mailbox_stored_count: u32,
    pub mailbox_expired_count: u32,
    pub mailbox_failed_count: u32,
    pub retry_record_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOutboxItemV1 {
    pub queue_id: [u8; 32],
    pub contact_id: [u8; 32],
    pub peer_account_id: [u8; 32],
    pub conversation_id: [u8; 32],
    pub state: CanonicalRuntimeIpcOutboxStateV1,
    pub acknowledgement_event_id: Option<[u8; 32]>,
    pub retry_record_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOutboxPageV1 {
    pub snapshot_id: [u8; 32],
    pub counts: CanonicalRuntimeIpcOutboxCountsV1,
    pub items: Vec<CanonicalRuntimeIpcOutboxItemV1>,
    pub next_cursor: Option<CanonicalRuntimeIpcOutboxCursorV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcOutboxResultV1 {
    OutboxStatusPage(CanonicalRuntimeIpcOutboxPageV1),
    Error { code: u16, message_utf8: Vec<u8> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOutboxResponseContentV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub request_digest: [u8; 32],
    pub responder_device_id: [u8; 32],
    pub result: CanonicalRuntimeIpcOutboxResultV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeIpcOutboxResponseV1 {
    pub content: CanonicalRuntimeIpcOutboxResponseContentV1,
    pub signature: [u8; 64],
}

pub fn encode_runtime_ipc_outbox_request_v1(
    request: &CanonicalRuntimeIpcOutboxRequestV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_outbox_request(request)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 7);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &request.descriptor_id);
    push_bytes(&mut output, &request.request_id);
    push_bytes(&mut output, &request.bearer_token);
    push_unsigned(&mut output, request.issued_at_unix_seconds);
    push_unsigned(&mut output, request.expires_at_unix_seconds);
    encode_outbox_command(&mut output, &request.command);
    ensure_bound(
        &output,
        "outbox request",
        MAX_CANONICAL_RUNTIME_IPC_OUTBOX_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_runtime_ipc_outbox_request_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcOutboxRequestV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "outbox request",
        MAX_CANONICAL_RUNTIME_IPC_OUTBOX_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("runtime-ipc-outbox-request-v1", 7)?;
    decoder.expect_version("runtime-ipc-outbox-request-v1.schema-version")?;
    let request = CanonicalRuntimeIpcOutboxRequestV1 {
        descriptor_id: decoder.fixed_bytes("runtime-ipc-outbox-request-v1.descriptor-id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-outbox-request-v1.request-id")?,
        bearer_token: decoder.fixed_bytes("runtime-ipc-outbox-request-v1.bearer-token")?,
        issued_at_unix_seconds: decoder
            .unsigned("runtime-ipc-outbox-request-v1.issued-at-unix-seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("runtime-ipc-outbox-request-v1.expires-at-unix-seconds")?,
        command: decode_outbox_command(&mut decoder)?,
    };
    decoder.finish()?;
    validate_outbox_request(&request)?;
    Ok(request)
}

pub fn runtime_ipc_outbox_request_digest_v1(
    request: &CanonicalRuntimeIpcOutboxRequestV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_OUTBOX_REQUEST_DIGEST_DOMAIN,
        &encode_runtime_ipc_outbox_request_v1(request)?,
    ))
}

pub fn verify_runtime_ipc_outbox_request_for_descriptor_v1(
    request: &CanonicalRuntimeIpcOutboxRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_runtime_ipc_descriptor_at_v1(descriptor, now_unix_seconds)?;
    validate_outbox_request(request)?;
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

pub fn runtime_ipc_outbox_snapshot_id_v1(
    counts: &CanonicalRuntimeIpcOutboxCountsV1,
    items: &[CanonicalRuntimeIpcOutboxItemV1],
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    validate_complete_outbox_snapshot(counts, items)?;
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 3);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    encode_outbox_counts(&mut encoded, counts);
    encode_outbox_items(&mut encoded, items);
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_OUTBOX_SNAPSHOT_ID_DOMAIN,
        &encoded,
    ))
}

pub fn runtime_ipc_outbox_response_signing_bytes_v1(
    content: &CanonicalRuntimeIpcOutboxResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    Ok(prefixed(
        CANONICAL_RUNTIME_IPC_OUTBOX_RESPONSE_SIGNATURE_DOMAIN,
        &encode_outbox_response_content(content)?,
    ))
}

pub fn sign_runtime_ipc_outbox_response_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalRuntimeIpcOutboxRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
    result: CanonicalRuntimeIpcOutboxResultV1,
) -> Result<CanonicalSignedRuntimeIpcOutboxResponseV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_outbox_request_for_descriptor_v1(request, descriptor, now_unix_seconds)?;
    if device_signing_key.verifying_key().to_bytes() != descriptor.content.device_id {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    let content = CanonicalRuntimeIpcOutboxResponseContentV1 {
        descriptor_id: request.descriptor_id,
        request_id: request.request_id,
        request_digest: runtime_ipc_outbox_request_digest_v1(request)?,
        responder_device_id: descriptor.content.device_id,
        result,
    };
    let signature = device_signing_key
        .sign(&runtime_ipc_outbox_response_signing_bytes_v1(&content)?)
        .to_bytes();
    let response = CanonicalSignedRuntimeIpcOutboxResponseV1 { content, signature };
    verify_runtime_ipc_outbox_response_for_request_v1(&response, request, descriptor)?;
    Ok(response)
}

pub fn encode_signed_runtime_ipc_outbox_response_v1(
    response: &CanonicalSignedRuntimeIpcOutboxResponseV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_outbox_response_v1(response)?;
    let output = signed_envelope(
        &encode_outbox_response_content(&response.content)?,
        &response.signature,
    );
    ensure_bound(
        &output,
        "outbox response",
        MAX_CANONICAL_RUNTIME_IPC_OUTBOX_RESPONSE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_runtime_ipc_outbox_response_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeIpcOutboxResponseV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "outbox response",
        MAX_CANONICAL_RUNTIME_IPC_OUTBOX_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-runtime-ipc-outbox-response-v1", 3)?;
    decoder.expect_version("signed-runtime-ipc-outbox-response-v1.schema-version")?;
    let content = decode_outbox_response_content(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-runtime-ipc-outbox-response-v1.signature")?;
    decoder.finish()?;
    let response = CanonicalSignedRuntimeIpcOutboxResponseV1 { content, signature };
    verify_signed_runtime_ipc_outbox_response_v1(&response)?;
    Ok(response)
}

pub fn verify_signed_runtime_ipc_outbox_response_v1(
    response: &CanonicalSignedRuntimeIpcOutboxResponseV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_outbox_response_content(&response.content)?;
    verify_ed25519(
        &response.content.responder_device_id,
        &runtime_ipc_outbox_response_signing_bytes_v1(&response.content)?,
        &response.signature,
    )
}

pub fn verify_runtime_ipc_outbox_response_for_request_v1(
    response: &CanonicalSignedRuntimeIpcOutboxResponseV1,
    request: &CanonicalRuntimeIpcOutboxRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_outbox_response_v1(response)?;
    validate_outbox_request(request)?;
    verify_signed_runtime_ipc_descriptor_v1(descriptor)?;
    if response.content.descriptor_id != runtime_ipc_descriptor_id_v1(descriptor)?
        || response.content.descriptor_id != request.descriptor_id
        || response.content.request_id != request.request_id
        || response.content.request_digest != runtime_ipc_outbox_request_digest_v1(request)?
        || response.content.responder_device_id != descriptor.content.device_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    match (&request.command, &response.content.result) {
        (_, CanonicalRuntimeIpcOutboxResultV1::Error { .. }) => Ok(()),
        (
            CanonicalRuntimeIpcOutboxCommandV1::OutboxStatusPage { cursor, limit },
            CanonicalRuntimeIpcOutboxResultV1::OutboxStatusPage(page),
        ) => verify_outbox_page_pair(cursor.as_ref(), *limit, page),
    }
}

pub fn runtime_ipc_outbox_response_id_v1(
    response: &CanonicalSignedRuntimeIpcOutboxResponseV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_OUTBOX_RESPONSE_ID_DOMAIN,
        &encode_signed_runtime_ipc_outbox_response_v1(response)?,
    ))
}

fn validate_outbox_request(
    request: &CanonicalRuntimeIpcOutboxRequestV1,
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
        CanonicalRuntimeIpcOutboxCommandV1::OutboxStatusPage { cursor, limit } => {
            if !(1..=MAX_CANONICAL_RUNTIME_IPC_OUTBOX_PAGE_SIZE).contains(limit) {
                return Err(CanonicalRuntimeIpcError::InvalidRequest);
            }
            if cursor.is_some_and(|value| {
                value.snapshot_id == [0; 32]
                    || value.start_index == 0
                    || value.start_index >= MAX_CANONICAL_RUNTIME_IPC_OUTBOX_SNAPSHOT_ITEMS
            }) {
                return Err(CanonicalRuntimeIpcError::InvalidRequest);
            }
        }
    }
    Ok(())
}

fn validate_outbox_response_content(
    content: &CanonicalRuntimeIpcOutboxResponseContentV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if content.descriptor_id == [0; 32]
        || content.request_id == [0; 32]
        || content.request_digest == [0; 32]
        || content.responder_device_id == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    match &content.result {
        CanonicalRuntimeIpcOutboxResultV1::OutboxStatusPage(page) => validate_outbox_page(page),
        CanonicalRuntimeIpcOutboxResultV1::Error { code, message_utf8 } => {
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

fn validate_outbox_counts(
    counts: &CanonicalRuntimeIpcOutboxCountsV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    let mailbox_count = counts
        .mailbox_pending_count
        .checked_add(counts.mailbox_stored_count)
        .and_then(|value| value.checked_add(counts.mailbox_expired_count))
        .and_then(|value| value.checked_add(counts.mailbox_failed_count))
        .ok_or(CanonicalRuntimeIpcError::InvalidResponse)?;
    if counts.contact_count > MAX_CANONICAL_RUNTIME_IPC_OUTBOX_CONTACTS
        || counts.queue_count > MAX_CANONICAL_RUNTIME_IPC_OUTBOX_SNAPSHOT_ITEMS
        || counts.retry_record_count > MAX_CANONICAL_RUNTIME_IPC_OUTBOX_RETRY_RECORDS
        || counts.pending_count.checked_add(counts.delivered_count) != Some(counts.queue_count)
        || counts.delivered_count > counts.materialized_count
        || counts.materialized_count > counts.queue_count
        || mailbox_count > counts.pending_count
        || mailbox_count > counts.materialized_count
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn validate_outbox_item(
    item: &CanonicalRuntimeIpcOutboxItemV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if item.queue_id == [0; 32]
        || item.contact_id == [0; 32]
        || item.peer_account_id == [0; 32]
        || item.conversation_id == [0; 32]
        || item.retry_record_count > MAX_CANONICAL_RUNTIME_IPC_OUTBOX_RETRY_RECORDS
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    match (item.state, item.acknowledgement_event_id) {
        (CanonicalRuntimeIpcOutboxStateV1::Delivered, Some(id)) if id != [0; 32] => Ok(()),
        (CanonicalRuntimeIpcOutboxStateV1::Delivered, _) => {
            Err(CanonicalRuntimeIpcError::InvalidResponse)
        }
        (_, None) => Ok(()),
        (_, Some(_)) => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn validate_outbox_items(
    items: &[CanonicalRuntimeIpcOutboxItemV1],
) -> Result<(), CanonicalRuntimeIpcError> {
    let maximum = usize::try_from(MAX_CANONICAL_RUNTIME_IPC_OUTBOX_SNAPSHOT_ITEMS)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    if items.len() > maximum {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    let mut previous = None;
    for item in items {
        validate_outbox_item(item)?;
        if previous.is_some_and(|queue_id: [u8; 32]| queue_id >= item.queue_id) {
            return Err(CanonicalRuntimeIpcError::InvalidResponse);
        }
        previous = Some(item.queue_id);
    }
    Ok(())
}

fn validate_complete_outbox_snapshot(
    counts: &CanonicalRuntimeIpcOutboxCountsV1,
    items: &[CanonicalRuntimeIpcOutboxItemV1],
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_outbox_counts(counts)?;
    validate_outbox_items(items)?;
    if usize::try_from(counts.queue_count).ok() != Some(items.len()) {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    let mut pending = 0_u32;
    let mut materialized = 0_u32;
    let mut delivered = 0_u32;
    let mut mailbox_pending = 0_u32;
    let mut mailbox_stored = 0_u32;
    let mut mailbox_expired = 0_u32;
    let mut mailbox_failed = 0_u32;
    let mut retries = 0_u32;
    for item in items {
        retries = retries
            .checked_add(item.retry_record_count)
            .ok_or(CanonicalRuntimeIpcError::InvalidResponse)?;
        if item.state != CanonicalRuntimeIpcOutboxStateV1::Delivered {
            pending += 1;
        }
        if item.state != CanonicalRuntimeIpcOutboxStateV1::Queued {
            materialized += 1;
        }
        match item.state {
            CanonicalRuntimeIpcOutboxStateV1::Delivered => delivered += 1,
            CanonicalRuntimeIpcOutboxStateV1::MailboxPending => mailbox_pending += 1,
            CanonicalRuntimeIpcOutboxStateV1::MailboxStored => mailbox_stored += 1,
            CanonicalRuntimeIpcOutboxStateV1::MailboxExpired => mailbox_expired += 1,
            CanonicalRuntimeIpcOutboxStateV1::MailboxFailed => mailbox_failed += 1,
            CanonicalRuntimeIpcOutboxStateV1::Queued
            | CanonicalRuntimeIpcOutboxStateV1::Materialized => {}
        }
    }
    if pending != counts.pending_count
        || materialized != counts.materialized_count
        || delivered != counts.delivered_count
        || mailbox_pending != counts.mailbox_pending_count
        || mailbox_stored != counts.mailbox_stored_count
        || mailbox_expired != counts.mailbox_expired_count
        || mailbox_failed != counts.mailbox_failed_count
        || retries != counts.retry_record_count
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn validate_outbox_page(
    page: &CanonicalRuntimeIpcOutboxPageV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_outbox_counts(&page.counts)?;
    validate_outbox_items(&page.items)?;
    if page.snapshot_id == [0; 32]
        || page.items.len() > usize::from(MAX_CANONICAL_RUNTIME_IPC_OUTBOX_PAGE_SIZE)
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    if let Some(cursor) = page.next_cursor
        && (cursor.snapshot_id != page.snapshot_id
            || cursor.start_index == 0
            || cursor.start_index >= page.counts.queue_count)
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn verify_outbox_page_pair(
    request_cursor: Option<&CanonicalRuntimeIpcOutboxCursorV1>,
    limit: u16,
    page: &CanonicalRuntimeIpcOutboxPageV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_outbox_page(page)?;
    let start_index = request_cursor.map_or(0, |cursor| cursor.start_index);
    if request_cursor.is_some_and(|cursor| cursor.snapshot_id != page.snapshot_id)
        || start_index > page.counts.queue_count
        || page.items.len() > usize::from(limit)
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    let page_length =
        u32::try_from(page.items.len()).map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    let end_index = start_index
        .checked_add(page_length)
        .ok_or(CanonicalRuntimeIpcError::InvalidResponse)?;
    if end_index > page.counts.queue_count
        || (start_index < page.counts.queue_count && page.items.is_empty())
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    if end_index < page.counts.queue_count {
        if page.next_cursor
            != Some(CanonicalRuntimeIpcOutboxCursorV1 {
                snapshot_id: page.snapshot_id,
                start_index: end_index,
            })
        {
            return Err(CanonicalRuntimeIpcError::ContextMismatch);
        }
    } else if page.next_cursor.is_some() {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    Ok(())
}

fn encode_outbox_command(output: &mut Vec<u8>, command: &CanonicalRuntimeIpcOutboxCommandV1) {
    match command {
        CanonicalRuntimeIpcOutboxCommandV1::OutboxStatusPage { cursor, limit } => {
            push_array_header(output, 3);
            push_unsigned(output, COMMAND_OUTBOX_STATUS_PAGE);
            encode_outbox_cursor_option(output, cursor.as_ref());
            push_unsigned(output, u64::from(*limit));
        }
    }
}

fn decode_outbox_command(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOutboxCommandV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-outbox-command-v1")?;
    match (
        decoder.unsigned("runtime-ipc-outbox-command-v1.variant")?,
        length,
    ) {
        (COMMAND_OUTBOX_STATUS_PAGE, 3) => {
            let cursor = decode_outbox_cursor_option(decoder)?;
            let limit = u16::try_from(decoder.unsigned("runtime-ipc-outbox-command-v1.limit")?)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?;
            Ok(CanonicalRuntimeIpcOutboxCommandV1::OutboxStatusPage { cursor, limit })
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_outbox_cursor_option(
    output: &mut Vec<u8>,
    cursor: Option<&CanonicalRuntimeIpcOutboxCursorV1>,
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
            push_unsigned(output, u64::from(cursor.start_index));
        }
    }
}

fn decode_outbox_cursor_option(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Option<CanonicalRuntimeIpcOutboxCursorV1>, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-outbox-cursor-option-v1")?;
    match (
        decoder.unsigned("runtime-ipc-outbox-cursor-option-v1.variant")?,
        length,
    ) {
        (OPTION_NONE, 1) => Ok(None),
        (OPTION_SOME, 2) => {
            decoder.expect_array("runtime-ipc-outbox-cursor-v1", 2)?;
            Ok(Some(CanonicalRuntimeIpcOutboxCursorV1 {
                snapshot_id: decoder.fixed_bytes("runtime-ipc-outbox-cursor-v1.snapshot-id")?,
                start_index: u32::try_from(
                    decoder.unsigned("runtime-ipc-outbox-cursor-v1.start-index")?,
                )
                .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?,
            }))
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_outbox_response_content(
    content: &CanonicalRuntimeIpcOutboxResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_outbox_response_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 6);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.descriptor_id);
    push_bytes(&mut output, &content.request_id);
    push_bytes(&mut output, &content.request_digest);
    push_bytes(&mut output, &content.responder_device_id);
    encode_outbox_result(&mut output, &content.result);
    Ok(output)
}

fn decode_outbox_response_content(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOutboxResponseContentV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-outbox-response-content-v1", 6)?;
    decoder.expect_version("runtime-ipc-outbox-response-content-v1.schema-version")?;
    let content = CanonicalRuntimeIpcOutboxResponseContentV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-outbox-response-content-v1.descriptor-id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-outbox-response-content-v1.request-id")?,
        request_digest: decoder
            .fixed_bytes("runtime-ipc-outbox-response-content-v1.request-digest")?,
        responder_device_id: decoder
            .fixed_bytes("runtime-ipc-outbox-response-content-v1.responder-device-id")?,
        result: decode_outbox_result(decoder)?,
    };
    validate_outbox_response_content(&content)?;
    Ok(content)
}

fn encode_outbox_result(output: &mut Vec<u8>, result: &CanonicalRuntimeIpcOutboxResultV1) {
    match result {
        CanonicalRuntimeIpcOutboxResultV1::OutboxStatusPage(page) => {
            push_array_header(output, 2);
            push_unsigned(output, RESULT_OUTBOX_STATUS_PAGE);
            encode_outbox_page(output, page);
        }
        CanonicalRuntimeIpcOutboxResultV1::Error { code, message_utf8 } => {
            push_array_header(output, 3);
            push_unsigned(output, RESULT_ERROR);
            push_unsigned(output, u64::from(*code));
            push_bytes(output, message_utf8);
        }
    }
}

fn decode_outbox_result(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOutboxResultV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-outbox-result-v1")?;
    match (
        decoder.unsigned("runtime-ipc-outbox-result-v1.variant")?,
        length,
    ) {
        (RESULT_OUTBOX_STATUS_PAGE, 2) => Ok(CanonicalRuntimeIpcOutboxResultV1::OutboxStatusPage(
            decode_outbox_page(decoder)?,
        )),
        (RESULT_ERROR, 3) => Ok(CanonicalRuntimeIpcOutboxResultV1::Error {
            code: u16::try_from(decoder.unsigned("runtime-ipc-outbox-result-v1.error-code")?)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            message_utf8: decoder
                .bytes("runtime-ipc-outbox-result-v1.error-message-utf8")?
                .to_vec(),
        }),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_outbox_page(output: &mut Vec<u8>, page: &CanonicalRuntimeIpcOutboxPageV1) {
    push_array_header(output, 4);
    push_bytes(output, &page.snapshot_id);
    encode_outbox_counts(output, &page.counts);
    encode_outbox_items(output, &page.items);
    encode_outbox_cursor_option(output, page.next_cursor.as_ref());
}

fn decode_outbox_page(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOutboxPageV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-outbox-page-v1", 4)?;
    Ok(CanonicalRuntimeIpcOutboxPageV1 {
        snapshot_id: decoder.fixed_bytes("runtime-ipc-outbox-page-v1.snapshot-id")?,
        counts: decode_outbox_counts(decoder)?,
        items: decode_outbox_items(decoder)?,
        next_cursor: decode_outbox_cursor_option(decoder)?,
    })
}

fn encode_outbox_counts(output: &mut Vec<u8>, counts: &CanonicalRuntimeIpcOutboxCountsV1) {
    push_array_header(output, 10);
    for value in [
        counts.contact_count,
        counts.queue_count,
        counts.pending_count,
        counts.materialized_count,
        counts.delivered_count,
        counts.mailbox_pending_count,
        counts.mailbox_stored_count,
        counts.mailbox_expired_count,
        counts.mailbox_failed_count,
        counts.retry_record_count,
    ] {
        push_unsigned(output, u64::from(value));
    }
}

fn decode_outbox_counts(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOutboxCountsV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-outbox-counts-v1", 10)?;
    let mut next = |field| {
        u32::try_from(decoder.unsigned(field)?)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)
    };
    Ok(CanonicalRuntimeIpcOutboxCountsV1 {
        contact_count: next("runtime-ipc-outbox-counts-v1.contact-count")?,
        queue_count: next("runtime-ipc-outbox-counts-v1.queue-count")?,
        pending_count: next("runtime-ipc-outbox-counts-v1.pending-count")?,
        materialized_count: next("runtime-ipc-outbox-counts-v1.materialized-count")?,
        delivered_count: next("runtime-ipc-outbox-counts-v1.delivered-count")?,
        mailbox_pending_count: next("runtime-ipc-outbox-counts-v1.mailbox-pending-count")?,
        mailbox_stored_count: next("runtime-ipc-outbox-counts-v1.mailbox-stored-count")?,
        mailbox_expired_count: next("runtime-ipc-outbox-counts-v1.mailbox-expired-count")?,
        mailbox_failed_count: next("runtime-ipc-outbox-counts-v1.mailbox-failed-count")?,
        retry_record_count: next("runtime-ipc-outbox-counts-v1.retry-record-count")?,
    })
}

fn encode_outbox_items(output: &mut Vec<u8>, items: &[CanonicalRuntimeIpcOutboxItemV1]) {
    push_array_header(output, items.len() as u64);
    for item in items {
        encode_outbox_item(output, item);
    }
}

fn decode_outbox_items(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Vec<CanonicalRuntimeIpcOutboxItemV1>, CanonicalRuntimeIpcError> {
    let count = decoder.array("runtime-ipc-outbox-items-v1")?;
    let maximum = usize::try_from(MAX_CANONICAL_RUNTIME_IPC_OUTBOX_SNAPSHOT_ITEMS)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    if count > maximum {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    (0..count).map(|_| decode_outbox_item(decoder)).collect()
}

fn encode_outbox_item(output: &mut Vec<u8>, item: &CanonicalRuntimeIpcOutboxItemV1) {
    push_array_header(output, 7);
    push_bytes(output, &item.queue_id);
    push_bytes(output, &item.contact_id);
    push_bytes(output, &item.peer_account_id);
    push_bytes(output, &item.conversation_id);
    push_unsigned(output, encode_outbox_state(item.state));
    encode_digest_option(output, item.acknowledgement_event_id.as_ref());
    push_unsigned(output, u64::from(item.retry_record_count));
}

fn decode_outbox_item(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOutboxItemV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-outbox-item-v1", 7)?;
    Ok(CanonicalRuntimeIpcOutboxItemV1 {
        queue_id: decoder.fixed_bytes("runtime-ipc-outbox-item-v1.queue-id")?,
        contact_id: decoder.fixed_bytes("runtime-ipc-outbox-item-v1.contact-id")?,
        peer_account_id: decoder.fixed_bytes("runtime-ipc-outbox-item-v1.peer-account-id")?,
        conversation_id: decoder.fixed_bytes("runtime-ipc-outbox-item-v1.conversation-id")?,
        state: decode_outbox_state(decoder)?,
        acknowledgement_event_id: decode_digest_option(decoder)?,
        retry_record_count: u32::try_from(
            decoder.unsigned("runtime-ipc-outbox-item-v1.retry-record-count")?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
    })
}

fn encode_outbox_state(state: CanonicalRuntimeIpcOutboxStateV1) -> u64 {
    match state {
        CanonicalRuntimeIpcOutboxStateV1::Queued => 0,
        CanonicalRuntimeIpcOutboxStateV1::Materialized => 1,
        CanonicalRuntimeIpcOutboxStateV1::Delivered => 2,
        CanonicalRuntimeIpcOutboxStateV1::MailboxPending => 3,
        CanonicalRuntimeIpcOutboxStateV1::MailboxStored => 4,
        CanonicalRuntimeIpcOutboxStateV1::MailboxExpired => 5,
        CanonicalRuntimeIpcOutboxStateV1::MailboxFailed => 6,
    }
}

fn decode_outbox_state(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOutboxStateV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-outbox-item-v1.state")? {
        0 => Ok(CanonicalRuntimeIpcOutboxStateV1::Queued),
        1 => Ok(CanonicalRuntimeIpcOutboxStateV1::Materialized),
        2 => Ok(CanonicalRuntimeIpcOutboxStateV1::Delivered),
        3 => Ok(CanonicalRuntimeIpcOutboxStateV1::MailboxPending),
        4 => Ok(CanonicalRuntimeIpcOutboxStateV1::MailboxStored),
        5 => Ok(CanonicalRuntimeIpcOutboxStateV1::MailboxExpired),
        6 => Ok(CanonicalRuntimeIpcOutboxStateV1::MailboxFailed),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_digest_option(output: &mut Vec<u8>, digest: Option<&[u8; 32]>) {
    match digest {
        None => {
            push_array_header(output, 1);
            push_unsigned(output, OPTION_NONE);
        }
        Some(digest) => {
            push_array_header(output, 2);
            push_unsigned(output, OPTION_SOME);
            push_bytes(output, digest);
        }
    }
}

fn decode_digest_option(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Option<[u8; 32]>, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-outbox-digest-option-v1")?;
    match (
        decoder.unsigned("runtime-ipc-outbox-digest-option-v1.variant")?,
        length,
    ) {
        (OPTION_NONE, 1) => Ok(None),
        (OPTION_SOME, 2) => Ok(Some(
            decoder.fixed_bytes("runtime-ipc-outbox-digest-option-v1.digest")?,
        )),
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
        issued_at: u64,
        expires_at: u64,
        counts: CanonicalRuntimeIpcOutboxCountsV1,
        items: Vec<CanonicalRuntimeIpcOutboxItemV1>,
    }

    fn documents() -> TestResult<(Value, Value, Value)> {
        Ok((
            serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../CANONICAL-RUNTIME-IPC-VECTORS.json"
            )))?,
            serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../CANONICAL-RUNTIME-IPC-QUEUE-VECTORS.json"
            )))?,
            serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../CANONICAL-RUNTIME-IPC-OUTBOX-VECTORS.json"
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
        let (core, queue, _) = documents()?;
        let items = vec![
            CanonicalRuntimeIpcOutboxItemV1 {
                queue_id: fixed(&queue, "/input/request_id_hex")?,
                contact_id: fixed(&queue, "/expected/contact_id_hex")?,
                peer_account_id: fixed(&queue, "/input/peer_account_id_hex")?,
                conversation_id: fixed(&queue, "/expected/conversation_id_hex")?,
                state: CanonicalRuntimeIpcOutboxStateV1::Queued,
                acknowledgement_event_id: None,
                retry_record_count: 0,
            },
            CanonicalRuntimeIpcOutboxItemV1 {
                queue_id: [0x82; 32],
                contact_id: [0x32; 32],
                peer_account_id: [0x42; 32],
                conversation_id: [0x52; 32],
                state: CanonicalRuntimeIpcOutboxStateV1::MailboxStored,
                acknowledgement_event_id: None,
                retry_record_count: 2,
            },
            CanonicalRuntimeIpcOutboxItemV1 {
                queue_id: [0x83; 32],
                contact_id: [0x33; 32],
                peer_account_id: [0x43; 32],
                conversation_id: [0x53; 32],
                state: CanonicalRuntimeIpcOutboxStateV1::Delivered,
                acknowledgement_event_id: Some([0x73; 32]),
                retry_record_count: 1,
            },
        ];
        Ok(Fixture {
            descriptor: decode_and_verify_signed_runtime_ipc_descriptor_v1(&bytes(
                &core,
                "/expected/descriptor/signed_cbor_hex",
            )?)?,
            device_key: SigningKey::from_bytes(&fixed(&core, "/input/device_secret_key_hex")?),
            request_id: [0x91; 32],
            issued_at: 1_800_002_100,
            expires_at: 1_800_002_130,
            counts: CanonicalRuntimeIpcOutboxCountsV1 {
                contact_count: 3,
                queue_count: 3,
                pending_count: 2,
                materialized_count: 2,
                delivered_count: 1,
                mailbox_pending_count: 0,
                mailbox_stored_count: 1,
                mailbox_expired_count: 0,
                mailbox_failed_count: 0,
                retry_record_count: 3,
            },
            items,
        })
    }

    fn request(
        fixture: &Fixture,
        cursor: Option<CanonicalRuntimeIpcOutboxCursorV1>,
        limit: u16,
    ) -> TestResult<CanonicalRuntimeIpcOutboxRequestV1> {
        Ok(CanonicalRuntimeIpcOutboxRequestV1 {
            descriptor_id: runtime_ipc_descriptor_id_v1(&fixture.descriptor)?,
            request_id: fixture.request_id,
            bearer_token: fixture.descriptor.content.bearer_token,
            issued_at_unix_seconds: fixture.issued_at,
            expires_at_unix_seconds: fixture.expires_at,
            command: CanonicalRuntimeIpcOutboxCommandV1::OutboxStatusPage { cursor, limit },
        })
    }

    fn first_page(fixture: &Fixture) -> TestResult<CanonicalRuntimeIpcOutboxPageV1> {
        let snapshot_id = runtime_ipc_outbox_snapshot_id_v1(&fixture.counts, &fixture.items)?;
        Ok(CanonicalRuntimeIpcOutboxPageV1 {
            snapshot_id,
            counts: fixture.counts,
            items: fixture.items[..2].to_vec(),
            next_cursor: Some(CanonicalRuntimeIpcOutboxCursorV1 {
                snapshot_id,
                start_index: 2,
            }),
        })
    }

    #[test]
    fn canonical_runtime_ipc_outbox_round_trips_and_binds_snapshot() -> TestResult {
        let fixture = fixture()?;
        let initial_request = request(&fixture, None, 2)?;
        let page = first_page(&fixture)?;
        verify_runtime_ipc_outbox_request_for_descriptor_v1(
            &initial_request,
            &fixture.descriptor,
            fixture.issued_at,
        )?;
        let request_bytes = encode_runtime_ipc_outbox_request_v1(&initial_request)?;
        assert_eq!(
            decode_and_verify_runtime_ipc_outbox_request_v1(&request_bytes)?,
            initial_request
        );
        let response = sign_runtime_ipc_outbox_response_v1(
            &fixture.device_key,
            &initial_request,
            &fixture.descriptor,
            fixture.issued_at,
            CanonicalRuntimeIpcOutboxResultV1::OutboxStatusPage(page.clone()),
        )?;
        let response_bytes = encode_signed_runtime_ipc_outbox_response_v1(&response)?;
        assert_eq!(
            decode_and_verify_signed_runtime_ipc_outbox_response_v1(&response_bytes)?,
            response
        );

        let continuation = request(
            &fixture,
            Some(page.next_cursor.ok_or("missing continuation")?),
            2,
        )?;
        let last_page = CanonicalRuntimeIpcOutboxPageV1 {
            snapshot_id: page.snapshot_id,
            counts: fixture.counts,
            items: fixture.items[2..].to_vec(),
            next_cursor: None,
        };
        sign_runtime_ipc_outbox_response_v1(
            &fixture.device_key,
            &continuation,
            &fixture.descriptor,
            fixture.issued_at,
            CanonicalRuntimeIpcOutboxResultV1::OutboxStatusPage(last_page),
        )?;
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_outbox_vectors_match() -> TestResult {
        let fixture = fixture()?;
        let (_, _, vectors) = documents()?;
        let request = request(&fixture, None, 2)?;
        let response = sign_runtime_ipc_outbox_response_v1(
            &fixture.device_key,
            &request,
            &fixture.descriptor,
            fixture.issued_at,
            CanonicalRuntimeIpcOutboxResultV1::OutboxStatusPage(first_page(&fixture)?),
        )?;
        assert_eq!(
            encode_runtime_ipc_outbox_request_v1(&request)?,
            bytes(&vectors, "/expected/request/cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_outbox_snapshot_id_v1(&fixture.counts, &fixture.items)?,
            fixed(&vectors, "/expected/snapshot/object_id_hex")?
        );
        assert_eq!(
            encode_signed_runtime_ipc_outbox_response_v1(&response)?,
            bytes(&vectors, "/expected/response/signed_cbor_hex")?
        );
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_outbox_pagination_substitutions_fail_closed() -> TestResult {
        let fixture = fixture()?;
        let request = request(&fixture, None, 2)?;
        let mut page = first_page(&fixture)?;
        page.next_cursor
            .as_mut()
            .ok_or("missing cursor")?
            .start_index = 1;
        assert!(
            sign_runtime_ipc_outbox_response_v1(
                &fixture.device_key,
                &request,
                &fixture.descriptor,
                fixture.issued_at,
                CanonicalRuntimeIpcOutboxResultV1::OutboxStatusPage(page),
            )
            .is_err()
        );

        let mut wrong_order = fixture.items.clone();
        wrong_order.swap(0, 1);
        assert!(runtime_ipc_outbox_snapshot_id_v1(&fixture.counts, &wrong_order).is_err());

        let mut wrong_counts = fixture.counts;
        wrong_counts.retry_record_count += 1;
        assert!(runtime_ipc_outbox_snapshot_id_v1(&wrong_counts, &fixture.items).is_err());
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_outbox_state_invariants_fail_closed() -> TestResult {
        let fixture = fixture()?;
        let mut items = fixture.items.clone();
        items[0].acknowledgement_event_id = Some([0x74; 32]);
        assert!(runtime_ipc_outbox_snapshot_id_v1(&fixture.counts, &items).is_err());

        let mut items = fixture.items.clone();
        items[2].acknowledgement_event_id = None;
        assert!(runtime_ipc_outbox_snapshot_id_v1(&fixture.counts, &items).is_err());
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

    fn state_name(state: CanonicalRuntimeIpcOutboxStateV1) -> &'static str {
        match state {
            CanonicalRuntimeIpcOutboxStateV1::Queued => "queued",
            CanonicalRuntimeIpcOutboxStateV1::Materialized => "materialized",
            CanonicalRuntimeIpcOutboxStateV1::Delivered => "delivered",
            CanonicalRuntimeIpcOutboxStateV1::MailboxPending => "mailbox-pending",
            CanonicalRuntimeIpcOutboxStateV1::MailboxStored => "mailbox-stored",
            CanonicalRuntimeIpcOutboxStateV1::MailboxExpired => "mailbox-expired",
            CanonicalRuntimeIpcOutboxStateV1::MailboxFailed => "mailbox-failed",
        }
    }

    fn item_json(item: &CanonicalRuntimeIpcOutboxItemV1) -> Value {
        json!({
            "queue_id_hex": lower_hex(&item.queue_id),
            "contact_id_hex": lower_hex(&item.contact_id),
            "peer_account_id_hex": lower_hex(&item.peer_account_id),
            "conversation_id_hex": lower_hex(&item.conversation_id),
            "state": state_name(item.state),
            "acknowledgement_event_id_hex": item.acknowledgement_event_id.map(|id| lower_hex(&id)),
            "retry_record_count": item.retry_record_count
        })
    }

    #[test]
    #[ignore = "explicit deterministic vector regeneration"]
    fn regenerate_canonical_runtime_ipc_outbox_vectors() -> TestResult {
        let fixture = fixture()?;
        let request = request(&fixture, None, 2)?;
        let request_bytes = encode_runtime_ipc_outbox_request_v1(&request)?;
        let request_digest = runtime_ipc_outbox_request_digest_v1(&request)?;
        let page = first_page(&fixture)?;
        let response = sign_runtime_ipc_outbox_response_v1(
            &fixture.device_key,
            &request,
            &fixture.descriptor,
            fixture.issued_at,
            CanonicalRuntimeIpcOutboxResultV1::OutboxStatusPage(page.clone()),
        )?;
        let content_bytes = encode_outbox_response_content(&response.content)?;
        let signed_bytes = encode_signed_runtime_ipc_outbox_response_v1(&response)?;
        let response_id = runtime_ipc_outbox_response_id_v1(&response)?;
        let mut snapshot_bytes = Vec::new();
        push_array_header(&mut snapshot_bytes, 3);
        push_unsigned(&mut snapshot_bytes, CANONICAL_PROFILE_VERSION);
        encode_outbox_counts(&mut snapshot_bytes, &fixture.counts);
        encode_outbox_items(&mut snapshot_bytes, &fixture.items);
        let document = json!({
            "schema_version": 1,
            "profile": "kilogram-deterministic-cbor-v1",
            "activation_status": "not-active",
            "dependencies": [
                "CANONICAL-RUNTIME-IPC-VECTORS.json",
                "CANONICAL-RUNTIME-IPC-QUEUE-VECTORS.json"
            ],
            "input": {
                "request_id_hex": lower_hex(&fixture.request_id),
                "request_issued_at_unix_seconds": fixture.issued_at,
                "request_expires_at_unix_seconds": fixture.expires_at,
                "page_limit": 2,
                "counts": {
                    "contact_count": fixture.counts.contact_count,
                    "queue_count": fixture.counts.queue_count,
                    "pending_count": fixture.counts.pending_count,
                    "materialized_count": fixture.counts.materialized_count,
                    "delivered_count": fixture.counts.delivered_count,
                    "mailbox_pending_count": fixture.counts.mailbox_pending_count,
                    "mailbox_stored_count": fixture.counts.mailbox_stored_count,
                    "mailbox_expired_count": fixture.counts.mailbox_expired_count,
                    "mailbox_failed_count": fixture.counts.mailbox_failed_count,
                    "retry_record_count": fixture.counts.retry_record_count
                },
                "items": fixture.items.iter().map(item_json).collect::<Vec<_>>()
            },
            "expected": {
                "account_id_hex": lower_hex(&fixture.descriptor.content.account_id),
                "device_id_hex": lower_hex(&fixture.descriptor.content.device_id),
                "snapshot": {
                    "cbor_hex": lower_hex(&snapshot_bytes),
                    "id_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_OUTBOX_SNAPSHOT_ID_DOMAIN),
                    "id_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_OUTBOX_SNAPSHOT_ID_DOMAIN, &snapshot_bytes)),
                    "object_id_hex": lower_hex(&page.snapshot_id)
                },
                "request": {
                    "cbor_hex": lower_hex(&request_bytes),
                    "digest_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_OUTBOX_REQUEST_DIGEST_DOMAIN),
                    "id_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_OUTBOX_REQUEST_DIGEST_DOMAIN, &request_bytes)),
                    "object_id_hex": lower_hex(&request_digest)
                },
                "response": {
                    "content_cbor_hex": lower_hex(&content_bytes),
                    "signature_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_OUTBOX_RESPONSE_SIGNATURE_DOMAIN),
                    "signing_preimage_hex": lower_hex(&runtime_ipc_outbox_response_signing_bytes_v1(&response.content)?),
                    "signature_hex": lower_hex(&response.signature),
                    "signed_cbor_hex": lower_hex(&signed_bytes),
                    "id_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_OUTBOX_RESPONSE_ID_DOMAIN, &signed_bytes)),
                    "object_id_hex": lower_hex(&response_id)
                },
                "page": {
                    "item_count": page.items.len(),
                    "next_start_index": page.next_cursor.map(|cursor| cursor.start_index),
                    "snapshot_id_hex": lower_hex(&page.snapshot_id)
                },
                "context": {
                    "valid_at_unix_seconds": fixture.issued_at,
                    "snapshot_is_content_derived": true,
                    "pagination_is_snapshot_bound": true,
                    "response_binds_complete_request_digest": true,
                    "active_runtime_changed": false
                }
            }
        });
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../CANONICAL-RUNTIME-IPC-OUTBOX-VECTORS.json");
        fs::write(
            path,
            format!("{}\n", serde_json::to_string_pretty(&document)?),
        )?;
        Ok(())
    }
}
