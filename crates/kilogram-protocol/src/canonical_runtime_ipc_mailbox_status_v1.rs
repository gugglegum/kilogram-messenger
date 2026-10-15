//! Inactive deterministic-CBOR mailbox-capability status for local runtime IPC.
//!
//! The active runtime remains Postcard IPC version 26. This projection carries
//! only signed capability update heads and their optional signed recipient ACKs;
//! local read/write secrets and decrypted peer offers are never serialized.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::{
    canonical_account_v1::{AccountDecoder, signed_envelope},
    canonical_mailbox_provisioning_v1::{
        CanonicalMailboxCapabilityActionV1, CanonicalSignedMailboxCapabilityAcknowledgementV1,
        CanonicalSignedMailboxCapabilityUpdateV1,
        decode_and_verify_signed_mailbox_capability_acknowledgement_v1,
        decode_and_verify_signed_mailbox_capability_update_v1,
        encode_signed_mailbox_capability_acknowledgement_v1,
        encode_signed_mailbox_capability_update_v1,
        verify_mailbox_capability_acknowledgement_for_v1,
        verify_signed_mailbox_capability_update_v1,
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

pub const MAX_CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_REQUEST_BYTES: usize = 64 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_RESPONSE_BYTES: usize = 640 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_HEADS: u32 = 1_024;
pub const MAX_CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_PAGE_SIZE: u16 = 16;

pub const CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-mailbox-status-request-digest:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_RESPONSE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-mailbox-status-response-signature:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_RESPONSE_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-mailbox-status-response-id:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_SNAPSHOT_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-mailbox-status-snapshot-id:cbor-v1\0";

const COMMAND_STATUS_PAGE: u64 = 0;
const RESULT_STATUS_PAGE: u64 = 0;
const RESULT_ERROR: u64 = 1;
const OPTION_NONE: u64 = 0;
const OPTION_SOME: u64 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcMailboxStatusCursorV1 {
    pub snapshot_id: [u8; 32],
    pub start_index: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcMailboxStatusCommandV1 {
    MailboxCapabilityStatusPage {
        cursor: Option<CanonicalRuntimeIpcMailboxStatusCursorV1>,
        limit: u16,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcMailboxStatusRequestV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub bearer_token: [u8; 32],
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub command: CanonicalRuntimeIpcMailboxStatusCommandV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcMailboxCapabilityHeadV1 {
    pub update: CanonicalSignedMailboxCapabilityUpdateV1,
    pub acknowledgement: Option<CanonicalSignedMailboxCapabilityAcknowledgementV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcMailboxStatusCountsV1 {
    pub head_count: u32,
    pub active_head_count: u32,
    pub revoked_head_count: u32,
    pub acknowledged_head_count: u32,
    pub unacknowledged_head_count: u32,
    pub local_owner_head_count: u32,
    pub local_recipient_head_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcMailboxStatusPageV1 {
    pub snapshot_id: [u8; 32],
    pub counts: CanonicalRuntimeIpcMailboxStatusCountsV1,
    pub heads: Vec<CanonicalRuntimeIpcMailboxCapabilityHeadV1>,
    pub next_cursor: Option<CanonicalRuntimeIpcMailboxStatusCursorV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcMailboxStatusResultV1 {
    MailboxCapabilityStatusPage(CanonicalRuntimeIpcMailboxStatusPageV1),
    Error { code: u16, message_utf8: Vec<u8> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcMailboxStatusResponseContentV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub request_digest: [u8; 32],
    pub responder_device_id: [u8; 32],
    pub result: CanonicalRuntimeIpcMailboxStatusResultV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeIpcMailboxStatusResponseV1 {
    pub content: CanonicalRuntimeIpcMailboxStatusResponseContentV1,
    pub signature: [u8; 64],
}

pub fn encode_runtime_ipc_mailbox_status_request_v1(
    request: &CanonicalRuntimeIpcMailboxStatusRequestV1,
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
        "mailbox status request",
        MAX_CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_runtime_ipc_mailbox_status_request_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcMailboxStatusRequestV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "mailbox status request",
        MAX_CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("runtime-ipc-mailbox-status-request-v1", 7)?;
    decoder.expect_version("runtime-ipc-mailbox-status-request-v1.schema-version")?;
    let request = CanonicalRuntimeIpcMailboxStatusRequestV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-mailbox-status-request-v1.descriptor-id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-mailbox-status-request-v1.request-id")?,
        bearer_token: decoder.fixed_bytes("runtime-ipc-mailbox-status-request-v1.bearer-token")?,
        issued_at_unix_seconds: decoder
            .unsigned("runtime-ipc-mailbox-status-request-v1.issued-at-unix-seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("runtime-ipc-mailbox-status-request-v1.expires-at-unix-seconds")?,
        command: decode_command(&mut decoder)?,
    };
    decoder.finish()?;
    validate_request(&request)?;
    Ok(request)
}

pub fn runtime_ipc_mailbox_status_request_digest_v1(
    request: &CanonicalRuntimeIpcMailboxStatusRequestV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_REQUEST_DIGEST_DOMAIN,
        &encode_runtime_ipc_mailbox_status_request_v1(request)?,
    ))
}

pub fn verify_runtime_ipc_mailbox_status_request_for_descriptor_v1(
    request: &CanonicalRuntimeIpcMailboxStatusRequestV1,
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

pub fn runtime_ipc_mailbox_status_counts_v1(
    local_account_id: [u8; 32],
    local_device_id: [u8; 32],
    heads: &[CanonicalRuntimeIpcMailboxCapabilityHeadV1],
) -> Result<CanonicalRuntimeIpcMailboxStatusCountsV1, CanonicalRuntimeIpcError> {
    validate_complete_heads(local_account_id, local_device_id, heads)?;
    let mut counts = CanonicalRuntimeIpcMailboxStatusCountsV1 {
        head_count: u32::try_from(heads.len())
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
        active_head_count: 0,
        revoked_head_count: 0,
        acknowledged_head_count: 0,
        unacknowledged_head_count: 0,
        local_owner_head_count: 0,
        local_recipient_head_count: 0,
    };
    for head in heads {
        if is_revoked(head) {
            counts.revoked_head_count += 1;
        } else {
            counts.active_head_count += 1;
        }
        if head.acknowledgement.is_some() {
            counts.acknowledged_head_count += 1;
        } else {
            counts.unacknowledged_head_count += 1;
        }
        if is_local_owner(head, local_account_id, local_device_id) {
            counts.local_owner_head_count += 1;
        } else {
            counts.local_recipient_head_count += 1;
        }
    }
    Ok(counts)
}

pub fn runtime_ipc_mailbox_status_snapshot_id_v1(
    local_account_id: [u8; 32],
    local_device_id: [u8; 32],
    heads: &[CanonicalRuntimeIpcMailboxCapabilityHeadV1],
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    let counts = runtime_ipc_mailbox_status_counts_v1(local_account_id, local_device_id, heads)?;
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 5);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut encoded, &local_account_id);
    push_bytes(&mut encoded, &local_device_id);
    encode_counts(&mut encoded, &counts);
    encode_heads(&mut encoded, heads)?;
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_SNAPSHOT_ID_DOMAIN,
        &encoded,
    ))
}

pub fn runtime_ipc_mailbox_status_response_signing_bytes_v1(
    content: &CanonicalRuntimeIpcMailboxStatusResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    Ok(prefixed(
        CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_RESPONSE_SIGNATURE_DOMAIN,
        &encode_response_content(content)?,
    ))
}

pub fn sign_runtime_ipc_mailbox_status_response_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalRuntimeIpcMailboxStatusRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
    result: CanonicalRuntimeIpcMailboxStatusResultV1,
) -> Result<CanonicalSignedRuntimeIpcMailboxStatusResponseV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_mailbox_status_request_for_descriptor_v1(
        request,
        descriptor,
        now_unix_seconds,
    )?;
    if device_signing_key.verifying_key().to_bytes() != descriptor.content.device_id {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    let content = CanonicalRuntimeIpcMailboxStatusResponseContentV1 {
        descriptor_id: request.descriptor_id,
        request_id: request.request_id,
        request_digest: runtime_ipc_mailbox_status_request_digest_v1(request)?,
        responder_device_id: descriptor.content.device_id,
        result,
    };
    let signature = device_signing_key
        .sign(&runtime_ipc_mailbox_status_response_signing_bytes_v1(
            &content,
        )?)
        .to_bytes();
    let response = CanonicalSignedRuntimeIpcMailboxStatusResponseV1 { content, signature };
    verify_runtime_ipc_mailbox_status_response_for_request_v1(&response, request, descriptor)?;
    Ok(response)
}

pub fn encode_signed_runtime_ipc_mailbox_status_response_v1(
    response: &CanonicalSignedRuntimeIpcMailboxStatusResponseV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_mailbox_status_response_v1(response)?;
    let output = signed_envelope(
        &encode_response_content(&response.content)?,
        &response.signature,
    );
    ensure_bound(
        &output,
        "mailbox status response",
        MAX_CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_RESPONSE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_runtime_ipc_mailbox_status_response_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeIpcMailboxStatusResponseV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "mailbox status response",
        MAX_CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-runtime-ipc-mailbox-status-response-v1", 3)?;
    decoder.expect_version("signed-runtime-ipc-mailbox-status-response-v1.schema-version")?;
    let content = decode_response_content(&mut decoder)?;
    let signature =
        decoder.fixed_bytes("signed-runtime-ipc-mailbox-status-response-v1.signature")?;
    decoder.finish()?;
    let response = CanonicalSignedRuntimeIpcMailboxStatusResponseV1 { content, signature };
    verify_signed_runtime_ipc_mailbox_status_response_v1(&response)?;
    Ok(response)
}

pub fn verify_signed_runtime_ipc_mailbox_status_response_v1(
    response: &CanonicalSignedRuntimeIpcMailboxStatusResponseV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_response_content(&response.content)?;
    verify_ed25519(
        &response.content.responder_device_id,
        &runtime_ipc_mailbox_status_response_signing_bytes_v1(&response.content)?,
        &response.signature,
    )
}

pub fn verify_runtime_ipc_mailbox_status_response_for_request_v1(
    response: &CanonicalSignedRuntimeIpcMailboxStatusResponseV1,
    request: &CanonicalRuntimeIpcMailboxStatusRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_mailbox_status_response_v1(response)?;
    validate_request(request)?;
    verify_signed_runtime_ipc_descriptor_v1(descriptor)?;
    if response.content.descriptor_id != runtime_ipc_descriptor_id_v1(descriptor)?
        || response.content.descriptor_id != request.descriptor_id
        || response.content.request_id != request.request_id
        || response.content.request_digest != runtime_ipc_mailbox_status_request_digest_v1(request)?
        || response.content.responder_device_id != descriptor.content.device_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    match (&request.command, &response.content.result) {
        (_, CanonicalRuntimeIpcMailboxStatusResultV1::Error { .. }) => Ok(()),
        (
            CanonicalRuntimeIpcMailboxStatusCommandV1::MailboxCapabilityStatusPage {
                cursor,
                limit,
            },
            CanonicalRuntimeIpcMailboxStatusResultV1::MailboxCapabilityStatusPage(page),
        ) => verify_page_pair(
            cursor.as_ref(),
            *limit,
            page,
            descriptor.content.account_id,
            descriptor.content.device_id,
        ),
    }
}

pub fn runtime_ipc_mailbox_status_response_id_v1(
    response: &CanonicalSignedRuntimeIpcMailboxStatusResponseV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_RESPONSE_ID_DOMAIN,
        &encode_signed_runtime_ipc_mailbox_status_response_v1(response)?,
    ))
}

fn validate_request(
    request: &CanonicalRuntimeIpcMailboxStatusRequestV1,
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
        CanonicalRuntimeIpcMailboxStatusCommandV1::MailboxCapabilityStatusPage {
            cursor,
            limit,
        } => {
            if !(1..=MAX_CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_PAGE_SIZE).contains(limit)
                || cursor.is_some_and(|value| {
                    value.snapshot_id == [0; 32]
                        || value.start_index == 0
                        || value.start_index >= MAX_CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_HEADS
                })
            {
                return Err(CanonicalRuntimeIpcError::InvalidRequest);
            }
        }
    }
    Ok(())
}

fn validate_response_content(
    content: &CanonicalRuntimeIpcMailboxStatusResponseContentV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if content.descriptor_id == [0; 32]
        || content.request_id == [0; 32]
        || content.request_digest == [0; 32]
        || content.responder_device_id == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    match &content.result {
        CanonicalRuntimeIpcMailboxStatusResultV1::MailboxCapabilityStatusPage(page) => {
            validate_page(page)
        }
        CanonicalRuntimeIpcMailboxStatusResultV1::Error { code, message_utf8 } => {
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

fn validate_counts(
    counts: &CanonicalRuntimeIpcMailboxStatusCountsV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if counts.head_count > MAX_CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_HEADS
        || counts
            .active_head_count
            .saturating_add(counts.revoked_head_count)
            != counts.head_count
        || counts
            .acknowledged_head_count
            .saturating_add(counts.unacknowledged_head_count)
            != counts.head_count
        || counts
            .local_owner_head_count
            .saturating_add(counts.local_recipient_head_count)
            != counts.head_count
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn validate_head(
    head: &CanonicalRuntimeIpcMailboxCapabilityHeadV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_mailbox_capability_update_v1(&head.update)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    if let Some(acknowledgement) = &head.acknowledgement {
        verify_mailbox_capability_acknowledgement_for_v1(
            acknowledgement,
            acknowledgement.content.session_binding,
            &head.update,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    }
    Ok(())
}

fn validate_heads(
    heads: &[CanonicalRuntimeIpcMailboxCapabilityHeadV1],
) -> Result<(), CanonicalRuntimeIpcError> {
    if heads.len() > usize::from(MAX_CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_PAGE_SIZE) {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    validate_head_order_and_signatures(heads)
}

fn validate_complete_heads(
    local_account_id: [u8; 32],
    local_device_id: [u8; 32],
    heads: &[CanonicalRuntimeIpcMailboxCapabilityHeadV1],
) -> Result<(), CanonicalRuntimeIpcError> {
    if heads.len()
        > usize::try_from(MAX_CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_HEADS)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    validate_head_order_and_signatures(heads)?;
    for head in heads {
        validate_head_for_local(head, local_account_id, local_device_id)?;
    }
    Ok(())
}

fn validate_head_order_and_signatures(
    heads: &[CanonicalRuntimeIpcMailboxCapabilityHeadV1],
) -> Result<(), CanonicalRuntimeIpcError> {
    let mut previous = None;
    for head in heads {
        validate_head(head)?;
        let key = head_key(head);
        if previous.is_some_and(|value| value >= key) {
            return Err(CanonicalRuntimeIpcError::InvalidResponse);
        }
        previous = Some(key);
    }
    Ok(())
}

fn validate_head_for_local(
    head: &CanonicalRuntimeIpcMailboxCapabilityHeadV1,
    local_account_id: [u8; 32],
    local_device_id: [u8; 32],
) -> Result<(), CanonicalRuntimeIpcError> {
    if !is_local_owner(head, local_account_id, local_device_id)
        && !is_local_recipient(head, local_account_id, local_device_id)
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    Ok(())
}

fn validate_page(
    page: &CanonicalRuntimeIpcMailboxStatusPageV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_counts(&page.counts)?;
    validate_heads(&page.heads)?;
    if page.snapshot_id == [0; 32]
        || page.heads.len() > usize::from(MAX_CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_PAGE_SIZE)
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    if let Some(cursor) = page.next_cursor
        && (cursor.snapshot_id != page.snapshot_id
            || cursor.start_index == 0
            || cursor.start_index >= page.counts.head_count)
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn verify_page_pair(
    request_cursor: Option<&CanonicalRuntimeIpcMailboxStatusCursorV1>,
    limit: u16,
    page: &CanonicalRuntimeIpcMailboxStatusPageV1,
    local_account_id: [u8; 32],
    local_device_id: [u8; 32],
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_page(page)?;
    for head in &page.heads {
        validate_head_for_local(head, local_account_id, local_device_id)?;
    }
    let start_index = request_cursor.map_or(0, |cursor| cursor.start_index);
    if request_cursor.is_some_and(|cursor| cursor.snapshot_id != page.snapshot_id)
        || start_index > page.counts.head_count
        || page.heads.len() > usize::from(limit)
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    let page_length =
        u32::try_from(page.heads.len()).map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    let end_index = start_index
        .checked_add(page_length)
        .ok_or(CanonicalRuntimeIpcError::InvalidResponse)?;
    if end_index > page.counts.head_count
        || (start_index < page.counts.head_count && page.heads.is_empty())
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    let page_counts =
        runtime_ipc_mailbox_status_counts_v1(local_account_id, local_device_id, &page.heads)?;
    if page_counts.active_head_count > page.counts.active_head_count
        || page_counts.revoked_head_count > page.counts.revoked_head_count
        || page_counts.acknowledged_head_count > page.counts.acknowledged_head_count
        || page_counts.unacknowledged_head_count > page.counts.unacknowledged_head_count
        || page_counts.local_owner_head_count > page.counts.local_owner_head_count
        || page_counts.local_recipient_head_count > page.counts.local_recipient_head_count
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    if start_index == 0
        && end_index == page.counts.head_count
        && (page_counts != page.counts
            || runtime_ipc_mailbox_status_snapshot_id_v1(
                local_account_id,
                local_device_id,
                &page.heads,
            )? != page.snapshot_id)
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    if end_index < page.counts.head_count {
        if page.next_cursor
            != Some(CanonicalRuntimeIpcMailboxStatusCursorV1 {
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

fn head_key(head: &CanonicalRuntimeIpcMailboxCapabilityHeadV1) -> [[u8; 32]; 5] {
    let content = &head.update.content;
    [
        content.owner_account_id,
        content.owner_device_id,
        content.recipient_account_id,
        content.recipient_device_id,
        content.scope,
    ]
}

fn is_revoked(head: &CanonicalRuntimeIpcMailboxCapabilityHeadV1) -> bool {
    matches!(
        head.update.content.action,
        CanonicalMailboxCapabilityActionV1::Revoke { .. }
    )
}

fn is_local_owner(
    head: &CanonicalRuntimeIpcMailboxCapabilityHeadV1,
    account_id: [u8; 32],
    device_id: [u8; 32],
) -> bool {
    head.update.content.owner_account_id == account_id
        && head.update.content.owner_device_id == device_id
}

fn is_local_recipient(
    head: &CanonicalRuntimeIpcMailboxCapabilityHeadV1,
    account_id: [u8; 32],
    device_id: [u8; 32],
) -> bool {
    head.update.content.recipient_account_id == account_id
        && head.update.content.recipient_device_id == device_id
}

fn encode_command(output: &mut Vec<u8>, command: &CanonicalRuntimeIpcMailboxStatusCommandV1) {
    match command {
        CanonicalRuntimeIpcMailboxStatusCommandV1::MailboxCapabilityStatusPage {
            cursor,
            limit,
        } => {
            push_array_header(output, 3);
            push_unsigned(output, COMMAND_STATUS_PAGE);
            encode_cursor_option(output, cursor.as_ref());
            push_unsigned(output, u64::from(*limit));
        }
    }
}

fn decode_command(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcMailboxStatusCommandV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-mailbox-status-command-v1", 3)?;
    match decoder.unsigned("runtime-ipc-mailbox-status-command-v1.variant")? {
        COMMAND_STATUS_PAGE => Ok(
            CanonicalRuntimeIpcMailboxStatusCommandV1::MailboxCapabilityStatusPage {
                cursor: decode_cursor_option(decoder)?,
                limit: u16::try_from(
                    decoder.unsigned("runtime-ipc-mailbox-status-command-v1.limit")?,
                )
                .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?,
            },
        ),
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_cursor_option(
    output: &mut Vec<u8>,
    cursor: Option<&CanonicalRuntimeIpcMailboxStatusCursorV1>,
) {
    match cursor {
        None => {
            push_array_header(output, 1);
            push_unsigned(output, OPTION_NONE);
        }
        Some(cursor) => {
            push_array_header(output, 2);
            push_unsigned(output, OPTION_SOME);
            push_array_header(output, 3);
            push_unsigned(output, CANONICAL_PROFILE_VERSION);
            push_bytes(output, &cursor.snapshot_id);
            push_unsigned(output, u64::from(cursor.start_index));
        }
    }
}

fn decode_cursor_option(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Option<CanonicalRuntimeIpcMailboxStatusCursorV1>, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-mailbox-status-cursor-option-v1")?;
    match (
        decoder.unsigned("runtime-ipc-mailbox-status-cursor-option-v1.variant")?,
        length,
    ) {
        (OPTION_NONE, 1) => Ok(None),
        (OPTION_SOME, 2) => {
            decoder.expect_array("runtime-ipc-mailbox-status-cursor-v1", 3)?;
            decoder.expect_version("runtime-ipc-mailbox-status-cursor-v1.schema-version")?;
            Ok(Some(CanonicalRuntimeIpcMailboxStatusCursorV1 {
                snapshot_id: decoder
                    .fixed_bytes("runtime-ipc-mailbox-status-cursor-v1.snapshot-id")?,
                start_index: u32::try_from(
                    decoder.unsigned("runtime-ipc-mailbox-status-cursor-v1.start-index")?,
                )
                .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?,
            }))
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_response_content(
    content: &CanonicalRuntimeIpcMailboxStatusResponseContentV1,
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
) -> Result<CanonicalRuntimeIpcMailboxStatusResponseContentV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-mailbox-status-response-content-v1", 6)?;
    decoder.expect_version("runtime-ipc-mailbox-status-response-content-v1.schema-version")?;
    Ok(CanonicalRuntimeIpcMailboxStatusResponseContentV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-mailbox-status-response-content-v1.descriptor-id")?,
        request_id: decoder
            .fixed_bytes("runtime-ipc-mailbox-status-response-content-v1.request-id")?,
        request_digest: decoder
            .fixed_bytes("runtime-ipc-mailbox-status-response-content-v1.request-digest")?,
        responder_device_id: decoder
            .fixed_bytes("runtime-ipc-mailbox-status-response-content-v1.responder-device-id")?,
        result: decode_result(decoder)?,
    })
}

fn encode_result(
    output: &mut Vec<u8>,
    result: &CanonicalRuntimeIpcMailboxStatusResultV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match result {
        CanonicalRuntimeIpcMailboxStatusResultV1::MailboxCapabilityStatusPage(page) => {
            push_array_header(output, 2);
            push_unsigned(output, RESULT_STATUS_PAGE);
            encode_page(output, page)?;
        }
        CanonicalRuntimeIpcMailboxStatusResultV1::Error { code, message_utf8 } => {
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
) -> Result<CanonicalRuntimeIpcMailboxStatusResultV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-mailbox-status-result-v1")?;
    match (
        decoder.unsigned("runtime-ipc-mailbox-status-result-v1.variant")?,
        length,
    ) {
        (RESULT_STATUS_PAGE, 2) => Ok(
            CanonicalRuntimeIpcMailboxStatusResultV1::MailboxCapabilityStatusPage(decode_page(
                decoder,
            )?),
        ),
        (RESULT_ERROR, 3) => Ok(CanonicalRuntimeIpcMailboxStatusResultV1::Error {
            code: u16::try_from(decoder.unsigned("runtime-ipc-mailbox-status-result-v1.code")?)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            message_utf8: decoder
                .bytes("runtime-ipc-mailbox-status-result-v1.message")?
                .to_vec(),
        }),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_page(
    output: &mut Vec<u8>,
    page: &CanonicalRuntimeIpcMailboxStatusPageV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    push_array_header(output, 5);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_bytes(output, &page.snapshot_id);
    encode_counts(output, &page.counts);
    encode_heads(output, &page.heads)?;
    encode_cursor_option(output, page.next_cursor.as_ref());
    Ok(())
}

fn decode_page(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcMailboxStatusPageV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-mailbox-status-page-v1", 5)?;
    decoder.expect_version("runtime-ipc-mailbox-status-page-v1.schema-version")?;
    Ok(CanonicalRuntimeIpcMailboxStatusPageV1 {
        snapshot_id: decoder.fixed_bytes("runtime-ipc-mailbox-status-page-v1.snapshot-id")?,
        counts: decode_counts(decoder)?,
        heads: decode_heads(decoder)?,
        next_cursor: decode_cursor_option(decoder)?,
    })
}

fn encode_counts(output: &mut Vec<u8>, counts: &CanonicalRuntimeIpcMailboxStatusCountsV1) {
    push_array_header(output, 8);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    for value in [
        counts.head_count,
        counts.active_head_count,
        counts.revoked_head_count,
        counts.acknowledged_head_count,
        counts.unacknowledged_head_count,
        counts.local_owner_head_count,
        counts.local_recipient_head_count,
    ] {
        push_unsigned(output, u64::from(value));
    }
}

fn decode_counts(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcMailboxStatusCountsV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-mailbox-status-counts-v1", 8)?;
    decoder.expect_version("runtime-ipc-mailbox-status-counts-v1.schema-version")?;
    let mut next = |field| {
        u32::try_from(decoder.unsigned(field)?)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)
    };
    Ok(CanonicalRuntimeIpcMailboxStatusCountsV1 {
        head_count: next("runtime-ipc-mailbox-status-counts-v1.head-count")?,
        active_head_count: next("runtime-ipc-mailbox-status-counts-v1.active-head-count")?,
        revoked_head_count: next("runtime-ipc-mailbox-status-counts-v1.revoked-head-count")?,
        acknowledged_head_count: next(
            "runtime-ipc-mailbox-status-counts-v1.acknowledged-head-count",
        )?,
        unacknowledged_head_count: next(
            "runtime-ipc-mailbox-status-counts-v1.unacknowledged-head-count",
        )?,
        local_owner_head_count: next(
            "runtime-ipc-mailbox-status-counts-v1.local-owner-head-count",
        )?,
        local_recipient_head_count: next(
            "runtime-ipc-mailbox-status-counts-v1.local-recipient-head-count",
        )?,
    })
}

fn encode_heads(
    output: &mut Vec<u8>,
    heads: &[CanonicalRuntimeIpcMailboxCapabilityHeadV1],
) -> Result<(), CanonicalRuntimeIpcError> {
    push_array_header(output, heads.len() as u64);
    for head in heads {
        push_array_header(output, 3);
        push_unsigned(output, CANONICAL_PROFILE_VERSION);
        push_bytes(
            output,
            &encode_signed_mailbox_capability_update_v1(&head.update)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
        );
        encode_acknowledgement_option(output, head.acknowledgement.as_ref())?;
    }
    Ok(())
}

fn decode_heads(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Vec<CanonicalRuntimeIpcMailboxCapabilityHeadV1>, CanonicalRuntimeIpcError> {
    let count = decoder.array("runtime-ipc-mailbox-status-heads-v1")?;
    if count > usize::from(MAX_CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_PAGE_SIZE) {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    (0..count)
        .map(|_| {
            decoder.expect_array("runtime-ipc-mailbox-capability-head-v1", 3)?;
            decoder.expect_version("runtime-ipc-mailbox-capability-head-v1.schema-version")?;
            let update = decode_and_verify_signed_mailbox_capability_update_v1(
                decoder.bytes("runtime-ipc-mailbox-capability-head-v1.update")?,
            )
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
            let acknowledgement = decode_acknowledgement_option(decoder)?;
            Ok(CanonicalRuntimeIpcMailboxCapabilityHeadV1 {
                update,
                acknowledgement,
            })
        })
        .collect()
}

fn encode_acknowledgement_option(
    output: &mut Vec<u8>,
    acknowledgement: Option<&CanonicalSignedMailboxCapabilityAcknowledgementV1>,
) -> Result<(), CanonicalRuntimeIpcError> {
    match acknowledgement {
        None => {
            push_array_header(output, 1);
            push_unsigned(output, OPTION_NONE);
        }
        Some(acknowledgement) => {
            push_array_header(output, 2);
            push_unsigned(output, OPTION_SOME);
            push_bytes(
                output,
                &encode_signed_mailbox_capability_acknowledgement_v1(acknowledgement)
                    .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            );
        }
    }
    Ok(())
}

fn decode_acknowledgement_option(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Option<CanonicalSignedMailboxCapabilityAcknowledgementV1>, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-mailbox-status-acknowledgement-option-v1")?;
    match (
        decoder.unsigned("runtime-ipc-mailbox-status-acknowledgement-option-v1.variant")?,
        length,
    ) {
        (OPTION_NONE, 1) => Ok(None),
        (OPTION_SOME, 2) => Ok(Some(
            decode_and_verify_signed_mailbox_capability_acknowledgement_v1(
                decoder.bytes("runtime-ipc-mailbox-status-acknowledgement-option-v1.value")?,
            )
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
        )),
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
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::{Value, json};

    use super::*;
    use crate::canonical_runtime_ipc_v1::{
        CanonicalRuntimeIpcDescriptorContentV1, CanonicalRuntimeIpcLoopbackAddressV1,
        encode_signed_runtime_ipc_descriptor_v1, sign_runtime_ipc_descriptor_v1,
    };

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    struct Fixture {
        signing_key: SigningKey,
        descriptor: CanonicalSignedRuntimeIpcDescriptorV1,
        request: CanonicalRuntimeIpcMailboxStatusRequestV1,
        response: CanonicalSignedRuntimeIpcMailboxStatusResponseV1,
        heads: Vec<CanonicalRuntimeIpcMailboxCapabilityHeadV1>,
        valid_at: u64,
    }

    fn vectors() -> TestResult<Value> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-RUNTIME-IPC-MAILBOX-STATUS-VECTORS.json"
        )))?)
    }

    fn provisioning_vectors() -> TestResult<Value> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-MAILBOX-PROVISIONING-VECTORS.json"
        )))?)
    }

    fn string<'a>(value: &'a Value, pointer: &str) -> TestResult<&'a str> {
        match value.pointer(pointer).and_then(Value::as_str) {
            Some(found) => Ok(found),
            None => Err(format!("missing string at {pointer}").into()),
        }
    }

    fn bytes(value: &Value, pointer: &str) -> TestResult<Vec<u8>> {
        let encoded = string(value, pointer)?;
        let (pairs, remainder) = encoded.as_bytes().as_chunks::<2>();
        if !remainder.is_empty() {
            return Err(format!("odd hex length at {pointer}").into());
        }
        pairs
            .iter()
            .map(|pair| {
                let pair = std::str::from_utf8(pair)?;
                Ok(u8::from_str_radix(pair, 16)?)
            })
            .collect()
    }

    fn fixed<const N: usize>(value: &Value, pointer: &str) -> TestResult<[u8; N]> {
        bytes(value, pointer)?
            .try_into()
            .map_err(|_| format!("wrong byte length at {pointer}").into())
    }

    fn unsigned(value: &Value, pointer: &str) -> TestResult<u64> {
        match value.pointer(pointer).and_then(Value::as_u64) {
            Some(found) => Ok(found),
            None => Err(format!("missing unsigned integer at {pointer}").into()),
        }
    }

    fn lower_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn fixture() -> TestResult<Fixture> {
        let vectors = vectors()?;
        let provisioning = provisioning_vectors()?;
        let input = &vectors["input"];
        let signing_key =
            SigningKey::from_bytes(&fixed(&provisioning, "/input/owner_secret_key_hex")?);
        let account_id = fixed(&provisioning, "/input/owner_account_id_hex")?;
        let descriptor = sign_runtime_ipc_descriptor_v1(
            &signing_key,
            CanonicalRuntimeIpcDescriptorContentV1 {
                address: CanonicalRuntimeIpcLoopbackAddressV1::Ipv4([127, 0, 0, 1]),
                port: u16::try_from(unsigned(input, "/port")?)?,
                bearer_token: fixed(input, "/bearer_token_hex")?,
                account_id,
                device_id: signing_key.verifying_key().to_bytes(),
                server_instance_id: fixed(input, "/server_instance_id_hex")?,
                issued_at_unix_seconds: unsigned(input, "/descriptor_issued_at_unix_seconds")?,
                expires_at_unix_seconds: unsigned(input, "/descriptor_expires_at_unix_seconds")?,
            },
        )?;
        let update = decode_and_verify_signed_mailbox_capability_update_v1(&bytes(
            &provisioning,
            "/expected/activation_update/signed_cbor_hex",
        )?)?;
        let acknowledgement =
            decode_and_verify_signed_mailbox_capability_acknowledgement_v1(&bytes(
                &provisioning,
                "/expected/activation_acknowledgement/signed_cbor_hex",
            )?)?;
        let heads = vec![CanonicalRuntimeIpcMailboxCapabilityHeadV1 {
            update,
            acknowledgement: Some(acknowledgement),
        }];
        let counts = runtime_ipc_mailbox_status_counts_v1(
            account_id,
            signing_key.verifying_key().to_bytes(),
            &heads,
        )?;
        let snapshot_id = runtime_ipc_mailbox_status_snapshot_id_v1(
            account_id,
            signing_key.verifying_key().to_bytes(),
            &heads,
        )?;
        let request = CanonicalRuntimeIpcMailboxStatusRequestV1 {
            descriptor_id: runtime_ipc_descriptor_id_v1(&descriptor)?,
            request_id: fixed(input, "/request_id_hex")?,
            bearer_token: descriptor.content.bearer_token,
            issued_at_unix_seconds: unsigned(input, "/request_issued_at_unix_seconds")?,
            expires_at_unix_seconds: unsigned(input, "/request_expires_at_unix_seconds")?,
            command: CanonicalRuntimeIpcMailboxStatusCommandV1::MailboxCapabilityStatusPage {
                cursor: None,
                limit: u16::try_from(unsigned(input, "/page_limit")?)?,
            },
        };
        let valid_at = unsigned(input, "/valid_at_unix_seconds")?;
        let response = sign_runtime_ipc_mailbox_status_response_v1(
            &signing_key,
            &request,
            &descriptor,
            valid_at,
            CanonicalRuntimeIpcMailboxStatusResultV1::MailboxCapabilityStatusPage(
                CanonicalRuntimeIpcMailboxStatusPageV1 {
                    snapshot_id,
                    counts,
                    heads: heads.clone(),
                    next_cursor: None,
                },
            ),
        )?;
        Ok(Fixture {
            signing_key,
            descriptor,
            request,
            response,
            heads,
            valid_at,
        })
    }

    #[test]
    fn canonical_runtime_ipc_mailbox_status_round_trips_and_binds_snapshot() -> TestResult {
        let fixture = fixture()?;
        let request = encode_runtime_ipc_mailbox_status_request_v1(&fixture.request)?;
        assert_eq!(
            decode_and_verify_runtime_ipc_mailbox_status_request_v1(&request)?,
            fixture.request
        );
        verify_runtime_ipc_mailbox_status_request_for_descriptor_v1(
            &fixture.request,
            &fixture.descriptor,
            fixture.valid_at,
        )?;
        let response = encode_signed_runtime_ipc_mailbox_status_response_v1(&fixture.response)?;
        assert_eq!(
            decode_and_verify_signed_runtime_ipc_mailbox_status_response_v1(&response)?,
            fixture.response
        );
        verify_runtime_ipc_mailbox_status_response_for_request_v1(
            &fixture.response,
            &fixture.request,
            &fixture.descriptor,
        )?;
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_mailbox_status_substitutions_fail_closed() -> TestResult {
        let fixture = fixture()?;
        let mut changed = fixture.response.clone();
        if let CanonicalRuntimeIpcMailboxStatusResultV1::MailboxCapabilityStatusPage(page) =
            &mut changed.content.result
        {
            page.counts.active_head_count = 0;
            page.counts.revoked_head_count = 1;
        }
        assert!(
            sign_runtime_ipc_mailbox_status_response_v1(
                &fixture.signing_key,
                &fixture.request,
                &fixture.descriptor,
                fixture.valid_at,
                changed.content.result,
            )
            .is_err()
        );

        let mut changed = fixture.response.clone();
        if let CanonicalRuntimeIpcMailboxStatusResultV1::MailboxCapabilityStatusPage(page) =
            &mut changed.content.result
        {
            page.heads
                .first_mut()
                .ok_or("fixture mailbox status page has no head")?
                .acknowledgement = None;
        }
        assert!(
            verify_runtime_ipc_mailbox_status_response_for_request_v1(
                &changed,
                &fixture.request,
                &fixture.descriptor,
            )
            .is_err()
        );

        let other_account = [0x55; 32];
        assert!(
            runtime_ipc_mailbox_status_snapshot_id_v1(
                other_account,
                fixture.descriptor.content.device_id,
                &fixture.heads,
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_mailbox_status_vectors_match() -> TestResult {
        let vectors = vectors()?;
        let fixture = fixture()?;
        let request = encode_runtime_ipc_mailbox_status_request_v1(&fixture.request)?;
        let response = encode_signed_runtime_ipc_mailbox_status_response_v1(&fixture.response)?;
        assert_eq!(
            lower_hex(&request),
            string(&vectors, "/expected/request/cbor_hex")?
        );
        assert_eq!(
            lower_hex(&response),
            string(&vectors, "/expected/response/signed_cbor_hex")?
        );
        assert_eq!(
            lower_hex(&runtime_ipc_mailbox_status_response_id_v1(
                &fixture.response
            )?),
            string(&vectors, "/expected/response/object_id_hex")?
        );
        Ok(())
    }

    #[test]
    #[ignore = "explicit deterministic vector regeneration"]
    fn regenerate_canonical_runtime_ipc_mailbox_status_vectors() -> TestResult {
        let old = vectors()?;
        let fixture = fixture()?;
        let request = encode_runtime_ipc_mailbox_status_request_v1(&fixture.request)?;
        let request_digest = runtime_ipc_mailbox_status_request_digest_v1(&fixture.request)?;
        let response_content = encode_response_content(&fixture.response.content)?;
        let response = encode_signed_runtime_ipc_mailbox_status_response_v1(&fixture.response)?;
        let response_id = runtime_ipc_mailbox_status_response_id_v1(&fixture.response)?;
        let descriptor = encode_signed_runtime_ipc_descriptor_v1(&fixture.descriptor)?;
        let snapshot_id = match &fixture.response.content.result {
            CanonicalRuntimeIpcMailboxStatusResultV1::MailboxCapabilityStatusPage(page) => {
                page.snapshot_id
            }
            CanonicalRuntimeIpcMailboxStatusResultV1::Error { .. } => {
                return Err("fixture response is not a mailbox status page".into());
            }
        };
        let signing_preimage =
            runtime_ipc_mailbox_status_response_signing_bytes_v1(&fixture.response.content)?;
        let output = json!({
            "schema_version": 1,
            "profile": "kilogram-deterministic-cbor-v1",
            "activation_status": "not-active",
            "dependencies": ["CANONICAL-MAILBOX-PROVISIONING-VECTORS.json"],
            "input": old["input"].clone(),
            "expected": {
                "account_id_hex": lower_hex(&fixture.descriptor.content.account_id),
                "device_id_hex": lower_hex(&fixture.descriptor.content.device_id),
                "descriptor": {
                    "signed_cbor_hex": lower_hex(&descriptor),
                    "object_id_hex": lower_hex(&fixture.request.descriptor_id)
                },
                "snapshot_id_hex": lower_hex(&snapshot_id),
                "request": {
                    "cbor_hex": lower_hex(&request),
                    "digest_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_REQUEST_DIGEST_DOMAIN),
                    "id_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_REQUEST_DIGEST_DOMAIN, &request)),
                    "object_id_hex": lower_hex(&request_digest)
                },
                "response": {
                    "content_cbor_hex": lower_hex(&response_content),
                    "signature_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_RESPONSE_SIGNATURE_DOMAIN),
                    "signing_preimage_hex": lower_hex(&signing_preimage),
                    "signature_hex": lower_hex(&fixture.response.signature),
                    "signed_cbor_hex": lower_hex(&response),
                    "id_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_RESPONSE_ID_DOMAIN, &response)),
                    "object_id_hex": lower_hex(&response_id)
                },
                "context": {
                    "valid_at_unix_seconds": fixture.valid_at,
                    "head_count": fixture.heads.len(),
                    "signed_updates_and_acknowledgements_only": true,
                    "read_write_secrets_in_payload": false,
                    "response_binds_complete_request_digest": true,
                    "active_runtime_changed": false
                }
            }
        });
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../CANONICAL-RUNTIME-IPC-MAILBOX-STATUS-VECTORS.json");
        std::fs::write(
            path,
            format!("{}\n", serde_json::to_string_pretty(&output)?),
        )?;
        Ok(())
    }
}
