//! Inactive deterministic-CBOR runtime mailbox records.
//!
//! The active CLI runtime continues to use Postcard v1. This module composes
//! the canonical provisioning and authorized-event objects into strict local
//! binding records, imported peer records, mailbox payloads, durable dispatch
//! records, and deterministic recovery identifiers. It is a migration target
//! only and is not imported by the active runtime.

use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

use crate::{
    canonical_account_v1::{AccountDecoder, CanonicalAccountError},
    canonical_event_v1::{
        CanonicalAuthorizedEventV1, CanonicalEventError, decode_and_verify_authorized_event_v1,
        encode_authorized_event_v1, event_id_v1,
    },
    canonical_mailbox_provisioning_v1::{
        CanonicalMailboxProvisioningError, CanonicalSignedLocalMailboxBindingV1,
        CanonicalSignedMailboxPeerOfferV1, decode_and_verify_signed_local_mailbox_binding_v1,
        decode_and_verify_signed_mailbox_peer_offer_v1, encode_signed_local_mailbox_binding_v1,
        encode_signed_mailbox_peer_offer_v1, mailbox_binding_id_v1,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
        push_unsigned, validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_RUNTIME_MAILBOX_BINDING_BYTES: usize = 64 * 1024;
pub const MAX_CANONICAL_RUNTIME_MAILBOX_PAYLOAD_BYTES: usize = 1024 * 1024;
pub const MAX_CANONICAL_RUNTIME_MAILBOX_DISPATCH_BYTES: usize = 8 * 1024;
pub const MIN_CANONICAL_RUNTIME_MAILBOX_DISPATCH_TTL_SECONDS: u64 = 60;
pub const MAX_CANONICAL_RUNTIME_MAILBOX_DISPATCH_TTL_SECONDS: u64 = 7 * 24 * 60 * 60;

pub const CANONICAL_RUNTIME_LOCAL_MAILBOX_BINDING_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-local-mailbox-binding:cbor-v1\0";
pub const CANONICAL_RUNTIME_PEER_MAILBOX_BINDING_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-peer-mailbox-binding:cbor-v1\0";
pub const CANONICAL_RUNTIME_MAILBOX_DISPATCH_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-mailbox-dispatch:cbor-v1\0";
pub const CANONICAL_RUNTIME_MAILBOX_ITEM_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-mailbox-item-id:cbor-v1\0";
pub const CANONICAL_RUNTIME_MAILBOX_EVENT_ITEM_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-mailbox-event-item-id:cbor-v1\0";
pub const CANONICAL_RUNTIME_MAILBOX_DISPATCH_REPLICATION_BINDING_DOMAIN: &[u8] =
    b"kilogram:runtime-mailbox-dispatch-replication-binding:cbor-v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeLocalMailboxBindingContentV1 {
    pub local_account_id: [u8; 32],
    pub local_device_id: [u8; 32],
    pub contact_id: [u8; 32],
    pub peer_account_id: [u8; 32],
    pub peer_device_id: [u8; 32],
    pub conversation_id: [u8; 32],
    pub binding_id: [u8; 32],
    pub created_at_unix_seconds: u64,
    pub binding: CanonicalSignedLocalMailboxBindingV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeLocalMailboxBindingV1 {
    pub content: CanonicalRuntimeLocalMailboxBindingContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimePeerMailboxBindingContentV1 {
    pub local_account_id: [u8; 32],
    pub local_device_id: [u8; 32],
    pub contact_id: [u8; 32],
    pub peer_account_id: [u8; 32],
    pub peer_device_id: [u8; 32],
    pub conversation_id: [u8; 32],
    pub binding_id: [u8; 32],
    pub imported_at_unix_seconds: u64,
    pub offer: CanonicalSignedMailboxPeerOfferV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimePeerMailboxBindingV1 {
    pub content: CanonicalRuntimePeerMailboxBindingContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeMailboxPayloadV1 {
    pub binding_id: [u8; 32],
    pub source_account_id: [u8; 32],
    pub source_device_id: [u8; 32],
    pub recipient_account_id: [u8; 32],
    pub recipient_device_id: [u8; 32],
    pub conversation_id: [u8; 32],
    pub event: CanonicalAuthorizedEventV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeMailboxDispatchContentV1 {
    pub local_account_id: [u8; 32],
    pub local_device_id: [u8; 32],
    pub queue_id: [u8; 32],
    pub contact_id: [u8; 32],
    pub peer_account_id: [u8; 32],
    pub peer_device_id: [u8; 32],
    pub conversation_id: [u8; 32],
    pub binding_id: [u8; 32],
    pub mailbox_id: [u8; 32],
    pub item_id: [u8; 32],
    pub event_id: [u8; 32],
    pub created_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeMailboxDispatchV1 {
    pub content: CanonicalRuntimeMailboxDispatchContentV1,
    pub signature: [u8; 64],
}

#[derive(Debug, Error)]
pub enum CanonicalRuntimeMailboxError {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),
    #[error(transparent)]
    Decode(#[from] CanonicalAccountError),
    #[error(transparent)]
    Event(#[from] CanonicalEventError),
    #[error(transparent)]
    Provisioning(#[from] CanonicalMailboxProvisioningError),
    #[error("canonical runtime mailbox {object} exceeds {maximum} bytes")]
    ObjectTooLarge {
        object: &'static str,
        maximum: usize,
    },
    #[error("canonical runtime mailbox binding metadata does not match its nested capability")]
    BindingMismatch,
    #[error("canonical runtime mailbox payload metadata does not match its authorized event")]
    PayloadMismatch,
    #[error("canonical runtime mailbox dispatch metadata is invalid")]
    InvalidDispatch,
    #[error("canonical runtime mailbox payload does not match its signed dispatch")]
    DispatchMismatch,
    #[error("canonical runtime mailbox {object} contains an invalid Ed25519 public key")]
    InvalidPublicKey { object: &'static str },
    #[error("canonical runtime mailbox {object} contains an invalid Ed25519 signature")]
    InvalidSignature { object: &'static str },
}

pub fn runtime_local_mailbox_binding_signing_bytes_v1(
    content: &CanonicalRuntimeLocalMailboxBindingContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeMailboxError> {
    validate_local_binding_content(content)?;
    Ok(prefixed(
        CANONICAL_RUNTIME_LOCAL_MAILBOX_BINDING_SIGNATURE_DOMAIN,
        &encode_local_binding_content(content)?,
    ))
}

pub fn encode_signed_runtime_local_mailbox_binding_v1(
    binding: &CanonicalSignedRuntimeLocalMailboxBindingV1,
) -> Result<Vec<u8>, CanonicalRuntimeMailboxError> {
    verify_signed_runtime_local_mailbox_binding_v1(binding)?;
    let content = encode_local_binding_content(&binding.content)?;
    let output = encode_signed(&content, &binding.signature);
    ensure_bound(
        &output,
        "signed local binding",
        MAX_CANONICAL_RUNTIME_MAILBOX_BINDING_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_runtime_local_mailbox_binding_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeLocalMailboxBindingV1, CanonicalRuntimeMailboxError> {
    ensure_bound(
        bytes,
        "signed local binding",
        MAX_CANONICAL_RUNTIME_MAILBOX_BINDING_BYTES,
    )?;
    let (content_bytes, signature) = decode_signed(bytes, "signed-runtime-local-binding-v1")?;
    let content = decode_local_binding_content(content_bytes)?;
    let binding = CanonicalSignedRuntimeLocalMailboxBindingV1 { content, signature };
    verify_signed_runtime_local_mailbox_binding_v1(&binding)?;
    if encode_signed_runtime_local_mailbox_binding_v1(&binding)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(binding)
}

pub fn verify_signed_runtime_local_mailbox_binding_v1(
    binding: &CanonicalSignedRuntimeLocalMailboxBindingV1,
) -> Result<(), CanonicalRuntimeMailboxError> {
    verify_ed25519(
        &binding.content.local_device_id,
        &runtime_local_mailbox_binding_signing_bytes_v1(&binding.content)?,
        &binding.signature,
        "signed local binding",
    )
}

pub fn runtime_peer_mailbox_binding_signing_bytes_v1(
    content: &CanonicalRuntimePeerMailboxBindingContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeMailboxError> {
    validate_peer_binding_content(content)?;
    Ok(prefixed(
        CANONICAL_RUNTIME_PEER_MAILBOX_BINDING_SIGNATURE_DOMAIN,
        &encode_peer_binding_content(content)?,
    ))
}

pub fn encode_signed_runtime_peer_mailbox_binding_v1(
    binding: &CanonicalSignedRuntimePeerMailboxBindingV1,
) -> Result<Vec<u8>, CanonicalRuntimeMailboxError> {
    verify_signed_runtime_peer_mailbox_binding_v1(binding)?;
    let content = encode_peer_binding_content(&binding.content)?;
    let output = encode_signed(&content, &binding.signature);
    ensure_bound(
        &output,
        "signed peer binding",
        MAX_CANONICAL_RUNTIME_MAILBOX_BINDING_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_runtime_peer_mailbox_binding_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimePeerMailboxBindingV1, CanonicalRuntimeMailboxError> {
    ensure_bound(
        bytes,
        "signed peer binding",
        MAX_CANONICAL_RUNTIME_MAILBOX_BINDING_BYTES,
    )?;
    let (content_bytes, signature) = decode_signed(bytes, "signed-runtime-peer-binding-v1")?;
    let content = decode_peer_binding_content(content_bytes)?;
    let binding = CanonicalSignedRuntimePeerMailboxBindingV1 { content, signature };
    verify_signed_runtime_peer_mailbox_binding_v1(&binding)?;
    if encode_signed_runtime_peer_mailbox_binding_v1(&binding)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(binding)
}

pub fn verify_signed_runtime_peer_mailbox_binding_v1(
    binding: &CanonicalSignedRuntimePeerMailboxBindingV1,
) -> Result<(), CanonicalRuntimeMailboxError> {
    verify_ed25519(
        &binding.content.local_device_id,
        &runtime_peer_mailbox_binding_signing_bytes_v1(&binding.content)?,
        &binding.signature,
        "signed peer binding",
    )
}

pub fn encode_runtime_mailbox_payload_v1(
    payload: &CanonicalRuntimeMailboxPayloadV1,
) -> Result<Vec<u8>, CanonicalRuntimeMailboxError> {
    validate_payload(payload)?;
    let event = encode_authorized_event_v1(&payload.event)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 8);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &payload.binding_id);
    push_bytes(&mut output, &payload.source_account_id);
    push_bytes(&mut output, &payload.source_device_id);
    push_bytes(&mut output, &payload.recipient_account_id);
    push_bytes(&mut output, &payload.recipient_device_id);
    push_bytes(&mut output, &payload.conversation_id);
    push_bytes(&mut output, &event);
    ensure_bound(
        &output,
        "payload",
        MAX_CANONICAL_RUNTIME_MAILBOX_PAYLOAD_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_runtime_mailbox_payload_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeMailboxPayloadV1, CanonicalRuntimeMailboxError> {
    ensure_bound(
        bytes,
        "payload",
        MAX_CANONICAL_RUNTIME_MAILBOX_PAYLOAD_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("runtime-mailbox-payload-v1", 8)?;
    decoder.expect_version("runtime-mailbox-payload-v1.schema_version")?;
    let binding_id = decoder.fixed_bytes("runtime-mailbox-payload-v1.binding_id")?;
    let source_account_id = decoder.fixed_bytes("runtime-mailbox-payload-v1.source_account_id")?;
    let source_device_id = decoder.fixed_bytes("runtime-mailbox-payload-v1.source_device_id")?;
    let recipient_account_id =
        decoder.fixed_bytes("runtime-mailbox-payload-v1.recipient_account_id")?;
    let recipient_device_id =
        decoder.fixed_bytes("runtime-mailbox-payload-v1.recipient_device_id")?;
    let conversation_id = decoder.fixed_bytes("runtime-mailbox-payload-v1.conversation_id")?;
    let event_bytes = decoder.bytes("runtime-mailbox-payload-v1.authorized_event")?;
    let event = decode_and_verify_authorized_event_v1(event_bytes)?;
    decoder.finish()?;
    let payload = CanonicalRuntimeMailboxPayloadV1 {
        binding_id,
        source_account_id,
        source_device_id,
        recipient_account_id,
        recipient_device_id,
        conversation_id,
        event,
    };
    validate_payload(&payload)?;
    if encode_runtime_mailbox_payload_v1(&payload)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(payload)
}

pub fn runtime_mailbox_dispatch_signing_bytes_v1(
    content: &CanonicalRuntimeMailboxDispatchContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeMailboxError> {
    validate_dispatch_content(content)?;
    Ok(prefixed(
        CANONICAL_RUNTIME_MAILBOX_DISPATCH_SIGNATURE_DOMAIN,
        &encode_dispatch_content(content)?,
    ))
}

pub fn encode_signed_runtime_mailbox_dispatch_v1(
    dispatch: &CanonicalSignedRuntimeMailboxDispatchV1,
) -> Result<Vec<u8>, CanonicalRuntimeMailboxError> {
    verify_signed_runtime_mailbox_dispatch_v1(dispatch)?;
    let content = encode_dispatch_content(&dispatch.content)?;
    let output = encode_signed(&content, &dispatch.signature);
    ensure_bound(
        &output,
        "signed dispatch",
        MAX_CANONICAL_RUNTIME_MAILBOX_DISPATCH_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_runtime_mailbox_dispatch_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeMailboxDispatchV1, CanonicalRuntimeMailboxError> {
    ensure_bound(
        bytes,
        "signed dispatch",
        MAX_CANONICAL_RUNTIME_MAILBOX_DISPATCH_BYTES,
    )?;
    let (content_bytes, signature) = decode_signed(bytes, "signed-runtime-mailbox-dispatch-v1")?;
    let content = decode_dispatch_content(content_bytes)?;
    let dispatch = CanonicalSignedRuntimeMailboxDispatchV1 { content, signature };
    verify_signed_runtime_mailbox_dispatch_v1(&dispatch)?;
    if encode_signed_runtime_mailbox_dispatch_v1(&dispatch)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(dispatch)
}

pub fn verify_signed_runtime_mailbox_dispatch_v1(
    dispatch: &CanonicalSignedRuntimeMailboxDispatchV1,
) -> Result<(), CanonicalRuntimeMailboxError> {
    verify_ed25519(
        &dispatch.content.local_device_id,
        &runtime_mailbox_dispatch_signing_bytes_v1(&dispatch.content)?,
        &dispatch.signature,
        "signed dispatch",
    )
}

pub fn runtime_mailbox_item_id_v1(
    queue_id: &[u8; 32],
    event_id: &[u8; 32],
    binding_id: &[u8; 32],
) -> [u8; 32] {
    domain_hash_parts(
        CANONICAL_RUNTIME_MAILBOX_ITEM_ID_DOMAIN,
        &[queue_id, event_id, binding_id],
    )
}

pub fn runtime_mailbox_event_item_id_v1(event_id: &[u8; 32], binding_id: &[u8; 32]) -> [u8; 32] {
    domain_hash_parts(
        CANONICAL_RUNTIME_MAILBOX_EVENT_ITEM_ID_DOMAIN,
        &[event_id, binding_id],
    )
}

pub fn runtime_mailbox_dispatch_replication_binding_v1(
    dispatch: &CanonicalSignedRuntimeMailboxDispatchV1,
) -> Result<[u8; 32], CanonicalRuntimeMailboxError> {
    Ok(domain_hash_parts(
        CANONICAL_RUNTIME_MAILBOX_DISPATCH_REPLICATION_BINDING_DOMAIN,
        &[&encode_signed_runtime_mailbox_dispatch_v1(dispatch)?],
    ))
}

pub fn verify_runtime_mailbox_payload_for_dispatch_v1(
    payload: &CanonicalRuntimeMailboxPayloadV1,
    dispatch: &CanonicalSignedRuntimeMailboxDispatchV1,
) -> Result<(), CanonicalRuntimeMailboxError> {
    validate_payload(payload)?;
    verify_signed_runtime_mailbox_dispatch_v1(dispatch)?;
    let content = &dispatch.content;
    if payload.binding_id != content.binding_id
        || payload.source_account_id != content.local_account_id
        || payload.source_device_id != content.local_device_id
        || payload.recipient_account_id != content.peer_account_id
        || payload.recipient_device_id != content.peer_device_id
        || payload.conversation_id != content.conversation_id
        || event_id_v1(&payload.event.event)? != content.event_id
    {
        return Err(CanonicalRuntimeMailboxError::DispatchMismatch);
    }
    Ok(())
}

fn encode_local_binding_content(
    content: &CanonicalRuntimeLocalMailboxBindingContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeMailboxError> {
    validate_local_binding_content(content)?;
    let nested = encode_signed_local_mailbox_binding_v1(&content.binding)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 10);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.local_account_id);
    push_bytes(&mut output, &content.local_device_id);
    push_bytes(&mut output, &content.contact_id);
    push_bytes(&mut output, &content.peer_account_id);
    push_bytes(&mut output, &content.peer_device_id);
    push_bytes(&mut output, &content.conversation_id);
    push_bytes(&mut output, &content.binding_id);
    push_unsigned(&mut output, content.created_at_unix_seconds);
    push_bytes(&mut output, &nested);
    Ok(output)
}

fn decode_local_binding_content(
    bytes: &[u8],
) -> Result<CanonicalRuntimeLocalMailboxBindingContentV1, CanonicalRuntimeMailboxError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("runtime-local-mailbox-binding-content-v1", 10)?;
    decoder.expect_version("runtime-local-mailbox-binding-content-v1.schema_version")?;
    let local_account_id = decoder.fixed_bytes("runtime-local-binding.local_account_id")?;
    let local_device_id = decoder.fixed_bytes("runtime-local-binding.local_device_id")?;
    let contact_id = decoder.fixed_bytes("runtime-local-binding.contact_id")?;
    let peer_account_id = decoder.fixed_bytes("runtime-local-binding.peer_account_id")?;
    let peer_device_id = decoder.fixed_bytes("runtime-local-binding.peer_device_id")?;
    let conversation_id = decoder.fixed_bytes("runtime-local-binding.conversation_id")?;
    let binding_id = decoder.fixed_bytes("runtime-local-binding.binding_id")?;
    let created_at_unix_seconds = decoder.unsigned("runtime-local-binding.created_at")?;
    let nested = decoder.bytes("runtime-local-binding.binding")?;
    let binding = decode_and_verify_signed_local_mailbox_binding_v1(nested)?;
    decoder.finish()?;
    let content = CanonicalRuntimeLocalMailboxBindingContentV1 {
        local_account_id,
        local_device_id,
        contact_id,
        peer_account_id,
        peer_device_id,
        conversation_id,
        binding_id,
        created_at_unix_seconds,
        binding,
    };
    validate_local_binding_content(&content)?;
    if encode_local_binding_content(&content)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(content)
}

fn encode_peer_binding_content(
    content: &CanonicalRuntimePeerMailboxBindingContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeMailboxError> {
    validate_peer_binding_content(content)?;
    let nested = encode_signed_mailbox_peer_offer_v1(&content.offer)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 10);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.local_account_id);
    push_bytes(&mut output, &content.local_device_id);
    push_bytes(&mut output, &content.contact_id);
    push_bytes(&mut output, &content.peer_account_id);
    push_bytes(&mut output, &content.peer_device_id);
    push_bytes(&mut output, &content.conversation_id);
    push_bytes(&mut output, &content.binding_id);
    push_unsigned(&mut output, content.imported_at_unix_seconds);
    push_bytes(&mut output, &nested);
    Ok(output)
}

fn decode_peer_binding_content(
    bytes: &[u8],
) -> Result<CanonicalRuntimePeerMailboxBindingContentV1, CanonicalRuntimeMailboxError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("runtime-peer-mailbox-binding-content-v1", 10)?;
    decoder.expect_version("runtime-peer-mailbox-binding-content-v1.schema_version")?;
    let local_account_id = decoder.fixed_bytes("runtime-peer-binding.local_account_id")?;
    let local_device_id = decoder.fixed_bytes("runtime-peer-binding.local_device_id")?;
    let contact_id = decoder.fixed_bytes("runtime-peer-binding.contact_id")?;
    let peer_account_id = decoder.fixed_bytes("runtime-peer-binding.peer_account_id")?;
    let peer_device_id = decoder.fixed_bytes("runtime-peer-binding.peer_device_id")?;
    let conversation_id = decoder.fixed_bytes("runtime-peer-binding.conversation_id")?;
    let binding_id = decoder.fixed_bytes("runtime-peer-binding.binding_id")?;
    let imported_at_unix_seconds = decoder.unsigned("runtime-peer-binding.imported_at")?;
    let nested = decoder.bytes("runtime-peer-binding.offer")?;
    let offer = decode_and_verify_signed_mailbox_peer_offer_v1(nested)?;
    decoder.finish()?;
    let content = CanonicalRuntimePeerMailboxBindingContentV1 {
        local_account_id,
        local_device_id,
        contact_id,
        peer_account_id,
        peer_device_id,
        conversation_id,
        binding_id,
        imported_at_unix_seconds,
        offer,
    };
    validate_peer_binding_content(&content)?;
    if encode_peer_binding_content(&content)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(content)
}

fn encode_dispatch_content(
    content: &CanonicalRuntimeMailboxDispatchContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeMailboxError> {
    validate_dispatch_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 14);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.local_account_id);
    push_bytes(&mut output, &content.local_device_id);
    push_bytes(&mut output, &content.queue_id);
    push_bytes(&mut output, &content.contact_id);
    push_bytes(&mut output, &content.peer_account_id);
    push_bytes(&mut output, &content.peer_device_id);
    push_bytes(&mut output, &content.conversation_id);
    push_bytes(&mut output, &content.binding_id);
    push_bytes(&mut output, &content.mailbox_id);
    push_bytes(&mut output, &content.item_id);
    push_bytes(&mut output, &content.event_id);
    push_unsigned(&mut output, content.created_at_unix_seconds);
    push_unsigned(&mut output, content.expires_at_unix_seconds);
    Ok(output)
}

fn decode_dispatch_content(
    bytes: &[u8],
) -> Result<CanonicalRuntimeMailboxDispatchContentV1, CanonicalRuntimeMailboxError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("runtime-mailbox-dispatch-content-v1", 14)?;
    decoder.expect_version("runtime-mailbox-dispatch-content-v1.schema_version")?;
    let content = CanonicalRuntimeMailboxDispatchContentV1 {
        local_account_id: decoder.fixed_bytes("runtime-dispatch.local_account_id")?,
        local_device_id: decoder.fixed_bytes("runtime-dispatch.local_device_id")?,
        queue_id: decoder.fixed_bytes("runtime-dispatch.queue_id")?,
        contact_id: decoder.fixed_bytes("runtime-dispatch.contact_id")?,
        peer_account_id: decoder.fixed_bytes("runtime-dispatch.peer_account_id")?,
        peer_device_id: decoder.fixed_bytes("runtime-dispatch.peer_device_id")?,
        conversation_id: decoder.fixed_bytes("runtime-dispatch.conversation_id")?,
        binding_id: decoder.fixed_bytes("runtime-dispatch.binding_id")?,
        mailbox_id: decoder.fixed_bytes("runtime-dispatch.mailbox_id")?,
        item_id: decoder.fixed_bytes("runtime-dispatch.item_id")?,
        event_id: decoder.fixed_bytes("runtime-dispatch.event_id")?,
        created_at_unix_seconds: decoder.unsigned("runtime-dispatch.created_at")?,
        expires_at_unix_seconds: decoder.unsigned("runtime-dispatch.expires_at")?,
    };
    decoder.finish()?;
    validate_dispatch_content(&content)?;
    if encode_dispatch_content(&content)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(content)
}

fn validate_local_binding_content(
    content: &CanonicalRuntimeLocalMailboxBindingContentV1,
) -> Result<(), CanonicalRuntimeMailboxError> {
    let core = &content.binding.core;
    if content.created_at_unix_seconds == 0
        || content.local_account_id == content.peer_account_id
        || content.local_account_id != core.owner_account_id
        || content.local_device_id != core.owner_device_id
        || content.peer_account_id != core.peer_account_id
        || content.peer_device_id != core.peer_device_id
        || content.conversation_id != core.scope
        || content.binding_id != mailbox_binding_id_v1(core)?
    {
        return Err(CanonicalRuntimeMailboxError::BindingMismatch);
    }
    Ok(())
}

fn validate_peer_binding_content(
    content: &CanonicalRuntimePeerMailboxBindingContentV1,
) -> Result<(), CanonicalRuntimeMailboxError> {
    let core = &content.offer.core;
    if content.imported_at_unix_seconds == 0
        || content.local_account_id == content.peer_account_id
        || content.local_account_id != core.peer_account_id
        || content.local_device_id != core.peer_device_id
        || content.peer_account_id != core.owner_account_id
        || content.peer_device_id != core.owner_device_id
        || content.conversation_id != core.scope
        || content.binding_id != mailbox_binding_id_v1(core)?
    {
        return Err(CanonicalRuntimeMailboxError::BindingMismatch);
    }
    Ok(())
}

fn validate_payload(
    payload: &CanonicalRuntimeMailboxPayloadV1,
) -> Result<(), CanonicalRuntimeMailboxError> {
    let event = &payload.event;
    let certificate = &event.author_certificate.content;
    if payload.source_account_id == payload.recipient_account_id
        || payload.source_account_id != certificate.account_id
        || payload.source_device_id != event.event.content.author_device_id
        || payload.source_device_id != certificate.device_id
        || payload.conversation_id != event.event.content.conversation_id
    {
        return Err(CanonicalRuntimeMailboxError::PayloadMismatch);
    }
    encode_authorized_event_v1(event)?;
    Ok(())
}

fn validate_dispatch_content(
    content: &CanonicalRuntimeMailboxDispatchContentV1,
) -> Result<(), CanonicalRuntimeMailboxError> {
    let Some(ttl) = content
        .expires_at_unix_seconds
        .checked_sub(content.created_at_unix_seconds)
    else {
        return Err(CanonicalRuntimeMailboxError::InvalidDispatch);
    };
    if content.created_at_unix_seconds == 0
        || content.local_account_id == content.peer_account_id
        || !(MIN_CANONICAL_RUNTIME_MAILBOX_DISPATCH_TTL_SECONDS
            ..=MAX_CANONICAL_RUNTIME_MAILBOX_DISPATCH_TTL_SECONDS)
            .contains(&ttl)
        || content.item_id
            != runtime_mailbox_item_id_v1(&content.queue_id, &content.event_id, &content.binding_id)
    {
        return Err(CanonicalRuntimeMailboxError::InvalidDispatch);
    }
    Ok(())
}

fn encode_signed(content: &[u8], signature: &[u8; 64]) -> Vec<u8> {
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, content);
    push_bytes(&mut output, signature);
    output
}

fn decode_signed<'a>(
    bytes: &'a [u8],
    object: &'static str,
) -> Result<(&'a [u8], [u8; 64]), CanonicalRuntimeMailboxError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array(object, 3)?;
    decoder.expect_version("signed-runtime-mailbox.schema_version")?;
    let content = decoder.bytes("signed-runtime-mailbox.content")?;
    let signature = decoder.fixed_bytes("signed-runtime-mailbox.signature")?;
    decoder.finish()?;
    Ok((content, signature))
}

fn ensure_bound(
    bytes: &[u8],
    object: &'static str,
    maximum: usize,
) -> Result<(), CanonicalRuntimeMailboxError> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(CanonicalRuntimeMailboxError::ObjectTooLarge { object, maximum });
    }
    Ok(())
}

fn verify_ed25519(
    public_key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
    object: &'static str,
) -> Result<(), CanonicalRuntimeMailboxError> {
    let verifying_key = VerifyingKey::from_bytes(public_key)
        .map_err(|_| CanonicalRuntimeMailboxError::InvalidPublicKey { object })?;
    verifying_key
        .verify_strict(message, &Signature::from_bytes(signature))
        .map_err(|_| CanonicalRuntimeMailboxError::InvalidSignature { object })
}

fn domain_hash_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    for part in parts {
        hasher.update(part);
    }
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct Vectors {
        expected: Expected,
    }

    #[derive(Deserialize)]
    struct Expected {
        signed_local_binding: SignedObject,
        signed_peer_binding: SignedObject,
        payload: PayloadObject,
        signed_dispatch: DispatchObject,
    }

    #[derive(Deserialize)]
    struct SignedObject {
        signed_cbor_hex: String,
    }

    #[derive(Deserialize)]
    struct PayloadObject {
        cbor_hex: String,
    }

    #[derive(Deserialize)]
    struct DispatchObject {
        signed_cbor_hex: String,
        item_id_hex: String,
        event_item_id_hex: String,
        replication_binding_hex: String,
    }

    fn bytes(value: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if !value.len().is_multiple_of(2) {
            return Err("odd-length hexadecimal".into());
        }
        (0..value.len())
            .step_by(2)
            .map(|index| Ok(u8::from_str_radix(&value[index..index + 2], 16)?))
            .collect()
    }

    fn fixed<const N: usize>(value: &str) -> Result<[u8; N], Box<dyn std::error::Error>> {
        Ok(bytes(value)?
            .try_into()
            .map_err(|_| "unexpected vector length")?)
    }

    fn vectors() -> Result<Vectors, serde_json::Error> {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-RUNTIME-MAILBOX-VECTORS.json"
        )))
    }

    #[test]
    fn golden_runtime_mailbox_objects_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let vectors = vectors()?;
        let local_bytes = bytes(&vectors.expected.signed_local_binding.signed_cbor_hex)?;
        let local = decode_and_verify_signed_runtime_local_mailbox_binding_v1(&local_bytes)?;
        assert_eq!(
            encode_signed_runtime_local_mailbox_binding_v1(&local)?,
            local_bytes
        );

        let peer_bytes = bytes(&vectors.expected.signed_peer_binding.signed_cbor_hex)?;
        let peer = decode_and_verify_signed_runtime_peer_mailbox_binding_v1(&peer_bytes)?;
        assert_eq!(
            encode_signed_runtime_peer_mailbox_binding_v1(&peer)?,
            peer_bytes
        );

        let payload_bytes = bytes(&vectors.expected.payload.cbor_hex)?;
        let payload = decode_and_verify_runtime_mailbox_payload_v1(&payload_bytes)?;
        assert_eq!(encode_runtime_mailbox_payload_v1(&payload)?, payload_bytes);

        let dispatch_bytes = bytes(&vectors.expected.signed_dispatch.signed_cbor_hex)?;
        let dispatch = decode_and_verify_signed_runtime_mailbox_dispatch_v1(&dispatch_bytes)?;
        assert_eq!(
            encode_signed_runtime_mailbox_dispatch_v1(&dispatch)?,
            dispatch_bytes
        );
        assert_eq!(
            dispatch.content.item_id,
            fixed(&vectors.expected.signed_dispatch.item_id_hex)?
        );
        assert_eq!(
            runtime_mailbox_event_item_id_v1(
                &dispatch.content.event_id,
                &dispatch.content.binding_id
            ),
            fixed(&vectors.expected.signed_dispatch.event_item_id_hex)?
        );
        assert_eq!(
            runtime_mailbox_dispatch_replication_binding_v1(&dispatch)?,
            fixed(&vectors.expected.signed_dispatch.replication_binding_hex)?
        );
        verify_runtime_mailbox_payload_for_dispatch_v1(&payload, &dispatch)?;
        Ok(())
    }

    #[test]
    fn tampering_and_cross_dispatch_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let vectors = vectors()?;
        let mut local = bytes(&vectors.expected.signed_local_binding.signed_cbor_hex)?;
        let last = local.len() - 1;
        local[last] ^= 1;
        assert!(decode_and_verify_signed_runtime_local_mailbox_binding_v1(&local).is_err());

        let payload = decode_and_verify_runtime_mailbox_payload_v1(&bytes(
            &vectors.expected.payload.cbor_hex,
        )?)?;
        let mut dispatch = decode_and_verify_signed_runtime_mailbox_dispatch_v1(&bytes(
            &vectors.expected.signed_dispatch.signed_cbor_hex,
        )?)?;
        dispatch.content.peer_device_id[0] ^= 1;
        assert!(verify_runtime_mailbox_payload_for_dispatch_v1(&payload, &dispatch).is_err());
        Ok(())
    }
}
