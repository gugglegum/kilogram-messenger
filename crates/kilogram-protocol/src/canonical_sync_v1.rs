//! Inactive deterministic-CBOR synchronization candidate for M1.1.12.
//!
//! The active synchronization format remains Postcard v6. This module fixes a
//! portable representation of every request/response frame without assigning
//! it an active protocol identifier.

use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

use crate::{
    canonical_account_v1::{AccountDecoder, CanonicalAccountError, signed_envelope},
    canonical_event_v1::{
        CanonicalAuthorizedEventV1, CanonicalEventError, decode_and_verify_authorized_event_v1,
        encode_authorized_event_v1, event_id_v1,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CANONICAL_SYNC_INVENTORY_SIGNATURE_DOMAIN, CanonicalWireError,
        SyncInventoryContentV1, encode_sync_inventory_content_v1, prefixed, push_array_header,
        push_bytes, push_unsigned, validate_canonical_profile_v1,
    },
};

pub const CANONICAL_SYNC_DIFF_SIGNATURE_DOMAIN: &[u8] = b"kilogram:sync-diff-signature:cbor-v1\0";
pub const MAX_CANONICAL_SYNC_INVENTORY_EVENT_IDS: usize = 4096;
pub const MAX_CANONICAL_SYNC_EVENTS_PER_BATCH: usize = 64;
pub const MAX_CANONICAL_SYNC_OBJECT_BYTES: usize = 8 * 1024 * 1024;

pub const CLIENT_SYNC_INVENTORY_VARIANT: u64 = 1;
pub const CLIENT_SYNC_EVENTS_VARIANT: u64 = 2;
pub const CLIENT_SYNC_PAUSE_VARIANT: u64 = 3;
pub const SERVER_SYNC_DIFF_VARIANT: u64 = 1;
pub const SERVER_SYNC_COMPLETE_VARIANT: u64 = 2;
pub const SERVER_SYNC_REJECTED_VARIANT: u64 = 3;
pub const SERVER_SYNC_PAUSED_VARIANT: u64 = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncInventoryContentV1 {
    pub conversation_id: [u8; 32],
    pub requester_device_id: [u8; 32],
    pub session_binding: [u8; 32],
    pub event_ids: Vec<[u8; 32]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedSyncInventoryV1 {
    pub content: CanonicalSyncInventoryContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncEventBatchV1 {
    pub conversation_id: [u8; 32],
    pub events: Vec<CanonicalAuthorizedEventV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncDiffContentV1 {
    pub conversation_id: [u8; 32],
    pub responder_device_id: [u8; 32],
    pub session_binding: [u8; 32],
    pub requested_event_ids: Vec<[u8; 32]>,
    pub events: Vec<CanonicalAuthorizedEventV1>,
    pub more_available: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedSyncDiffV1 {
    pub content: CanonicalSyncDiffContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncCompleteV1 {
    pub conversation_id: [u8; 32],
    pub stored_event_ids: Vec<[u8; 32]>,
    pub more_available: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncPauseV1 {
    pub conversation_id: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncPausedV1 {
    pub conversation_id: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum CanonicalSyncRejectionReasonV1 {
    RequesterNotAllowed = 1,
}

impl CanonicalSyncRejectionReasonV1 {
    const fn code(self) -> u64 {
        self as u64
    }

    fn from_code(code: u64) -> Result<Self, CanonicalSyncError> {
        match code {
            1 => Ok(Self::RequesterNotAllowed),
            other => Err(CanonicalSyncError::UnknownRejectionReason(other)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncRejectedV1 {
    pub conversation_id: [u8; 32],
    pub reason: CanonicalSyncRejectionReasonV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalSyncClientFrameV1 {
    Inventory(CanonicalSignedSyncInventoryV1),
    Events(CanonicalSyncEventBatchV1),
    Pause(CanonicalSyncPauseV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalSyncServerFrameV1 {
    Diff(CanonicalSignedSyncDiffV1),
    Complete(CanonicalSyncCompleteV1),
    Rejected(CanonicalSyncRejectedV1),
    Paused(CanonicalSyncPausedV1),
}

#[derive(Debug, Error)]
pub enum CanonicalSyncError {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),

    #[error(transparent)]
    Account(#[from] CanonicalAccountError),

    #[error(transparent)]
    Event(#[from] CanonicalEventError),

    #[error("canonical sync object exceeds {MAX_CANONICAL_SYNC_OBJECT_BYTES} bytes")]
    ObjectTooLarge,

    #[error("{field} must be strictly ordered by Event ID")]
    NonCanonicalEventIdOrder { field: &'static str },

    #[error("canonical sync event batch must be strictly ordered by derived Event ID")]
    NonCanonicalEventOrder,

    #[error("canonical sync event belongs to a different conversation")]
    EventConversationMismatch,

    #[error("canonical inventory and diff name different conversations")]
    FrameConversationMismatch,

    #[error("canonical inventory and diff carry different session bindings")]
    SessionBindingMismatch,

    #[error("{field} contains {actual} items; maximum is {maximum}")]
    TooManyItems {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },

    #[error("{field} contains integer {actual}; expected canonical boolean 0 or 1")]
    InvalidBoolean { field: &'static str, actual: u64 },

    #[error("unknown canonical sync rejection reason {0}")]
    UnknownRejectionReason(u64),

    #[error("unknown canonical client sync frame variant {0}")]
    UnknownClientFrameVariant(u64),

    #[error("unknown canonical server sync frame variant {0}")]
    UnknownServerFrameVariant(u64),

    #[error("{object} contains an invalid Ed25519 verifying key")]
    InvalidPublicKey { object: &'static str },

    #[error("{object} contains an invalid Ed25519 signature")]
    InvalidSignature { object: &'static str },
}

pub fn encode_sync_inventory_content_v1_candidate(
    content: &CanonicalSyncInventoryContentV1,
) -> Result<Vec<u8>, CanonicalSyncError> {
    validate_ordered_ids(
        "inventory_event_ids",
        &content.event_ids,
        MAX_CANONICAL_SYNC_INVENTORY_EVENT_IDS,
    )?;
    let encoded = encode_sync_inventory_content_v1(SyncInventoryContentV1 {
        conversation_id: &content.conversation_id,
        requester_device_id: &content.requester_device_id,
        session_binding: &content.session_binding,
        event_ids: &content.event_ids,
    })?;
    ensure_object_bound(&encoded)?;
    Ok(encoded)
}

pub fn sync_inventory_signing_bytes_v1_candidate(
    content: &CanonicalSyncInventoryContentV1,
) -> Result<Vec<u8>, CanonicalSyncError> {
    Ok(prefixed(
        CANONICAL_SYNC_INVENTORY_SIGNATURE_DOMAIN,
        &encode_sync_inventory_content_v1_candidate(content)?,
    ))
}

pub fn encode_signed_sync_inventory_v1_candidate(
    inventory: &CanonicalSignedSyncInventoryV1,
) -> Result<Vec<u8>, CanonicalSyncError> {
    verify_signed_sync_inventory_v1(inventory)?;
    let content = encode_sync_inventory_content_v1_candidate(&inventory.content)?;
    bounded(signed_envelope(&content, &inventory.signature))
}

pub fn decode_signed_sync_inventory_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedSyncInventoryV1, CanonicalSyncError> {
    validate_input(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-sync-inventory-v1", 3)?;
    decoder.expect_version("signed-sync-inventory-v1.schema_version")?;
    let content = decode_inventory_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-sync-inventory-v1.signature")?;
    decoder.finish()?;
    Ok(CanonicalSignedSyncInventoryV1 { content, signature })
}

pub fn decode_and_verify_signed_sync_inventory_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedSyncInventoryV1, CanonicalSyncError> {
    let inventory = decode_signed_sync_inventory_v1(bytes)?;
    verify_signed_sync_inventory_v1(&inventory)?;
    Ok(inventory)
}

pub fn verify_signed_sync_inventory_v1(
    inventory: &CanonicalSignedSyncInventoryV1,
) -> Result<(), CanonicalSyncError> {
    verify_ed25519(
        &inventory.content.requester_device_id,
        &sync_inventory_signing_bytes_v1_candidate(&inventory.content)?,
        &inventory.signature,
        "signed-sync-inventory-v1",
    )
}

pub fn encode_sync_event_batch_v1(
    batch: &CanonicalSyncEventBatchV1,
) -> Result<Vec<u8>, CanonicalSyncError> {
    validate_events("batch_events", batch.conversation_id, &batch.events)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &batch.conversation_id);
    push_array_header(&mut output, batch.events.len() as u64);
    for event in &batch.events {
        push_bytes(&mut output, &encode_authorized_event_v1(event)?);
    }
    bounded(output)
}

pub fn decode_and_verify_sync_event_batch_v1(
    bytes: &[u8],
) -> Result<CanonicalSyncEventBatchV1, CanonicalSyncError> {
    validate_input(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("sync-event-batch-v1", 3)?;
    decoder.expect_version("sync-event-batch-v1.schema_version")?;
    let conversation_id = decoder.fixed_bytes("sync-event-batch-v1.conversation_id")?;
    let events = decode_events_from(&mut decoder, "sync-event-batch-v1.events")?;
    decoder.finish()?;
    let batch = CanonicalSyncEventBatchV1 {
        conversation_id,
        events,
    };
    validate_events("batch_events", batch.conversation_id, &batch.events)?;
    Ok(batch)
}

pub fn encode_sync_diff_content_v1(
    content: &CanonicalSyncDiffContentV1,
) -> Result<Vec<u8>, CanonicalSyncError> {
    validate_diff_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 7);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.conversation_id);
    push_bytes(&mut output, &content.responder_device_id);
    push_bytes(&mut output, &content.session_binding);
    encode_ids_into(&mut output, &content.requested_event_ids);
    encode_events_into(&mut output, &content.events)?;
    push_unsigned(&mut output, u64::from(content.more_available));
    bounded(output)
}

pub fn sync_diff_signing_bytes_v1(
    content: &CanonicalSyncDiffContentV1,
) -> Result<Vec<u8>, CanonicalSyncError> {
    Ok(prefixed(
        CANONICAL_SYNC_DIFF_SIGNATURE_DOMAIN,
        &encode_sync_diff_content_v1(content)?,
    ))
}

pub fn encode_signed_sync_diff_v1(
    diff: &CanonicalSignedSyncDiffV1,
) -> Result<Vec<u8>, CanonicalSyncError> {
    verify_signed_sync_diff_v1(diff)?;
    let content = encode_sync_diff_content_v1(&diff.content)?;
    bounded(signed_envelope(&content, &diff.signature))
}

pub fn decode_signed_sync_diff_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedSyncDiffV1, CanonicalSyncError> {
    validate_input(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-sync-diff-v1", 3)?;
    decoder.expect_version("signed-sync-diff-v1.schema_version")?;
    let content = decode_sync_diff_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-sync-diff-v1.signature")?;
    decoder.finish()?;
    Ok(CanonicalSignedSyncDiffV1 { content, signature })
}

pub fn decode_and_verify_signed_sync_diff_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedSyncDiffV1, CanonicalSyncError> {
    let diff = decode_signed_sync_diff_v1(bytes)?;
    verify_signed_sync_diff_v1(&diff)?;
    Ok(diff)
}

pub fn verify_signed_sync_diff_v1(
    diff: &CanonicalSignedSyncDiffV1,
) -> Result<(), CanonicalSyncError> {
    validate_diff_content(&diff.content)?;
    verify_ed25519(
        &diff.content.responder_device_id,
        &sync_diff_signing_bytes_v1(&diff.content)?,
        &diff.signature,
        "signed-sync-diff-v1",
    )
}

pub fn encode_sync_complete_v1(
    complete: &CanonicalSyncCompleteV1,
) -> Result<Vec<u8>, CanonicalSyncError> {
    validate_ordered_ids(
        "stored_event_ids",
        &complete.stored_event_ids,
        MAX_CANONICAL_SYNC_EVENTS_PER_BATCH,
    )?;
    let mut output = Vec::new();
    push_array_header(&mut output, 4);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &complete.conversation_id);
    encode_ids_into(&mut output, &complete.stored_event_ids);
    push_unsigned(&mut output, u64::from(complete.more_available));
    bounded(output)
}

pub fn decode_sync_complete_v1(
    bytes: &[u8],
) -> Result<CanonicalSyncCompleteV1, CanonicalSyncError> {
    validate_input(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("sync-complete-v1", 4)?;
    decoder.expect_version("sync-complete-v1.schema_version")?;
    let conversation_id = decoder.fixed_bytes("sync-complete-v1.conversation_id")?;
    let stored_event_ids = decode_ids_from(
        &mut decoder,
        "sync-complete-v1.stored_event_ids",
        MAX_CANONICAL_SYNC_EVENTS_PER_BATCH,
    )?;
    let more_available = decode_bool(&mut decoder, "sync-complete-v1.more_available")?;
    decoder.finish()?;
    Ok(CanonicalSyncCompleteV1 {
        conversation_id,
        stored_event_ids,
        more_available,
    })
}

pub fn encode_sync_pause_v1(pause: &CanonicalSyncPauseV1) -> Result<Vec<u8>, CanonicalSyncError> {
    encode_conversation_only("sync-pause-v1", pause.conversation_id)
}

pub fn decode_sync_pause_v1(bytes: &[u8]) -> Result<CanonicalSyncPauseV1, CanonicalSyncError> {
    Ok(CanonicalSyncPauseV1 {
        conversation_id: decode_conversation_only(bytes, "sync-pause-v1")?,
    })
}

pub fn encode_sync_paused_v1(
    paused: &CanonicalSyncPausedV1,
) -> Result<Vec<u8>, CanonicalSyncError> {
    encode_conversation_only("sync-paused-v1", paused.conversation_id)
}

pub fn decode_sync_paused_v1(bytes: &[u8]) -> Result<CanonicalSyncPausedV1, CanonicalSyncError> {
    Ok(CanonicalSyncPausedV1 {
        conversation_id: decode_conversation_only(bytes, "sync-paused-v1")?,
    })
}

pub fn encode_sync_rejected_v1(
    rejected: &CanonicalSyncRejectedV1,
) -> Result<Vec<u8>, CanonicalSyncError> {
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &rejected.conversation_id);
    push_unsigned(&mut output, rejected.reason.code());
    bounded(output)
}

pub fn decode_sync_rejected_v1(
    bytes: &[u8],
) -> Result<CanonicalSyncRejectedV1, CanonicalSyncError> {
    validate_input(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("sync-rejected-v1", 3)?;
    decoder.expect_version("sync-rejected-v1.schema_version")?;
    let conversation_id = decoder.fixed_bytes("sync-rejected-v1.conversation_id")?;
    let reason =
        CanonicalSyncRejectionReasonV1::from_code(decoder.unsigned("sync-rejected-v1.reason")?)?;
    decoder.finish()?;
    Ok(CanonicalSyncRejectedV1 {
        conversation_id,
        reason,
    })
}

pub fn encode_sync_client_frame_v1(
    frame: &CanonicalSyncClientFrameV1,
) -> Result<Vec<u8>, CanonicalSyncError> {
    let (variant, nested) = match frame {
        CanonicalSyncClientFrameV1::Inventory(value) => (
            CLIENT_SYNC_INVENTORY_VARIANT,
            encode_signed_sync_inventory_v1_candidate(value)?,
        ),
        CanonicalSyncClientFrameV1::Events(value) => (
            CLIENT_SYNC_EVENTS_VARIANT,
            encode_sync_event_batch_v1(value)?,
        ),
        CanonicalSyncClientFrameV1::Pause(value) => {
            (CLIENT_SYNC_PAUSE_VARIANT, encode_sync_pause_v1(value)?)
        }
    };
    encode_frame(variant, &nested)
}

pub fn decode_and_verify_sync_client_frame_v1(
    bytes: &[u8],
) -> Result<CanonicalSyncClientFrameV1, CanonicalSyncError> {
    let (variant, nested) = decode_frame(bytes, "sync-client-frame-v1")?;
    match variant {
        CLIENT_SYNC_INVENTORY_VARIANT => Ok(CanonicalSyncClientFrameV1::Inventory(
            decode_and_verify_signed_sync_inventory_v1(nested)?,
        )),
        CLIENT_SYNC_EVENTS_VARIANT => Ok(CanonicalSyncClientFrameV1::Events(
            decode_and_verify_sync_event_batch_v1(nested)?,
        )),
        CLIENT_SYNC_PAUSE_VARIANT => Ok(CanonicalSyncClientFrameV1::Pause(decode_sync_pause_v1(
            nested,
        )?)),
        other => Err(CanonicalSyncError::UnknownClientFrameVariant(other)),
    }
}

pub fn encode_sync_server_frame_v1(
    frame: &CanonicalSyncServerFrameV1,
) -> Result<Vec<u8>, CanonicalSyncError> {
    let (variant, nested) = match frame {
        CanonicalSyncServerFrameV1::Diff(value) => {
            (SERVER_SYNC_DIFF_VARIANT, encode_signed_sync_diff_v1(value)?)
        }
        CanonicalSyncServerFrameV1::Complete(value) => (
            SERVER_SYNC_COMPLETE_VARIANT,
            encode_sync_complete_v1(value)?,
        ),
        CanonicalSyncServerFrameV1::Rejected(value) => (
            SERVER_SYNC_REJECTED_VARIANT,
            encode_sync_rejected_v1(value)?,
        ),
        CanonicalSyncServerFrameV1::Paused(value) => {
            (SERVER_SYNC_PAUSED_VARIANT, encode_sync_paused_v1(value)?)
        }
    };
    encode_frame(variant, &nested)
}

pub fn decode_and_verify_sync_server_frame_v1(
    bytes: &[u8],
) -> Result<CanonicalSyncServerFrameV1, CanonicalSyncError> {
    let (variant, nested) = decode_frame(bytes, "sync-server-frame-v1")?;
    match variant {
        SERVER_SYNC_DIFF_VARIANT => Ok(CanonicalSyncServerFrameV1::Diff(
            decode_and_verify_signed_sync_diff_v1(nested)?,
        )),
        SERVER_SYNC_COMPLETE_VARIANT => Ok(CanonicalSyncServerFrameV1::Complete(
            decode_sync_complete_v1(nested)?,
        )),
        SERVER_SYNC_REJECTED_VARIANT => Ok(CanonicalSyncServerFrameV1::Rejected(
            decode_sync_rejected_v1(nested)?,
        )),
        SERVER_SYNC_PAUSED_VARIANT => Ok(CanonicalSyncServerFrameV1::Paused(
            decode_sync_paused_v1(nested)?,
        )),
        other => Err(CanonicalSyncError::UnknownServerFrameVariant(other)),
    }
}

pub fn verify_sync_frame_session_v1(
    inventory: &CanonicalSignedSyncInventoryV1,
    diff: &CanonicalSignedSyncDiffV1,
) -> Result<(), CanonicalSyncError> {
    verify_signed_sync_inventory_v1(inventory)?;
    verify_signed_sync_diff_v1(diff)?;
    if inventory.content.conversation_id != diff.content.conversation_id {
        return Err(CanonicalSyncError::FrameConversationMismatch);
    }
    if inventory.content.session_binding != diff.content.session_binding {
        return Err(CanonicalSyncError::SessionBindingMismatch);
    }
    Ok(())
}

fn decode_inventory_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalSyncInventoryContentV1, CanonicalSyncError> {
    decoder.expect_array("sync-inventory-content-v1", 5)?;
    decoder.expect_version("sync-inventory-content-v1.schema_version")?;
    let conversation_id = decoder.fixed_bytes("sync-inventory-content-v1.conversation_id")?;
    let requester_device_id =
        decoder.fixed_bytes("sync-inventory-content-v1.requester_device_id")?;
    let session_binding = decoder.fixed_bytes("sync-inventory-content-v1.session_binding")?;
    let event_ids = decode_ids_from(
        decoder,
        "sync-inventory-content-v1.event_ids",
        MAX_CANONICAL_SYNC_INVENTORY_EVENT_IDS,
    )?;
    Ok(CanonicalSyncInventoryContentV1 {
        conversation_id,
        requester_device_id,
        session_binding,
        event_ids,
    })
}

fn decode_sync_diff_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalSyncDiffContentV1, CanonicalSyncError> {
    decoder.expect_array("sync-diff-content-v1", 7)?;
    decoder.expect_version("sync-diff-content-v1.schema_version")?;
    let conversation_id = decoder.fixed_bytes("sync-diff-content-v1.conversation_id")?;
    let responder_device_id = decoder.fixed_bytes("sync-diff-content-v1.responder_device_id")?;
    let session_binding = decoder.fixed_bytes("sync-diff-content-v1.session_binding")?;
    let requested_event_ids = decode_ids_from(
        decoder,
        "sync-diff-content-v1.requested_event_ids",
        MAX_CANONICAL_SYNC_EVENTS_PER_BATCH,
    )?;
    let events = decode_events_from(decoder, "sync-diff-content-v1.events")?;
    let more_available = decode_bool(decoder, "sync-diff-content-v1.more_available")?;
    let content = CanonicalSyncDiffContentV1 {
        conversation_id,
        responder_device_id,
        session_binding,
        requested_event_ids,
        events,
        more_available,
    };
    validate_diff_content(&content)?;
    Ok(content)
}

fn validate_diff_content(content: &CanonicalSyncDiffContentV1) -> Result<(), CanonicalSyncError> {
    validate_ordered_ids(
        "requested_event_ids",
        &content.requested_event_ids,
        MAX_CANONICAL_SYNC_EVENTS_PER_BATCH,
    )?;
    validate_events("diff_events", content.conversation_id, &content.events)
}

fn validate_events(
    field: &'static str,
    conversation_id: [u8; 32],
    events: &[CanonicalAuthorizedEventV1],
) -> Result<(), CanonicalSyncError> {
    if events.len() > MAX_CANONICAL_SYNC_EVENTS_PER_BATCH {
        return Err(CanonicalSyncError::TooManyItems {
            field,
            actual: events.len(),
            maximum: MAX_CANONICAL_SYNC_EVENTS_PER_BATCH,
        });
    }
    let mut previous = None;
    for event in events {
        if event.event.content.conversation_id != conversation_id {
            return Err(CanonicalSyncError::EventConversationMismatch);
        }
        let event_id = event_id_v1(&event.event)?;
        if previous.is_some_and(|value| value >= event_id) {
            return Err(CanonicalSyncError::NonCanonicalEventOrder);
        }
        previous = Some(event_id);
        encode_authorized_event_v1(event)?;
    }
    Ok(())
}

fn validate_ordered_ids(
    field: &'static str,
    ids: &[[u8; 32]],
    maximum: usize,
) -> Result<(), CanonicalSyncError> {
    if ids.len() > maximum {
        return Err(CanonicalSyncError::TooManyItems {
            field,
            actual: ids.len(),
            maximum,
        });
    }
    if ids.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(CanonicalSyncError::NonCanonicalEventIdOrder { field });
    }
    Ok(())
}

fn encode_ids_into(output: &mut Vec<u8>, ids: &[[u8; 32]]) {
    push_array_header(output, ids.len() as u64);
    for id in ids {
        push_bytes(output, id);
    }
}

fn decode_ids_from(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
    maximum: usize,
) -> Result<Vec<[u8; 32]>, CanonicalSyncError> {
    let count = decoder.array(field)?;
    if count > maximum {
        return Err(CanonicalSyncError::TooManyItems {
            field,
            actual: count,
            maximum,
        });
    }
    let mut ids = Vec::with_capacity(count);
    for _ in 0..count {
        ids.push(decoder.fixed_bytes(field)?);
    }
    validate_ordered_ids(field, &ids, maximum)?;
    Ok(ids)
}

fn encode_events_into(
    output: &mut Vec<u8>,
    events: &[CanonicalAuthorizedEventV1],
) -> Result<(), CanonicalSyncError> {
    push_array_header(output, events.len() as u64);
    for event in events {
        push_bytes(output, &encode_authorized_event_v1(event)?);
    }
    Ok(())
}

fn decode_events_from(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
) -> Result<Vec<CanonicalAuthorizedEventV1>, CanonicalSyncError> {
    let count = decoder.array(field)?;
    if count > MAX_CANONICAL_SYNC_EVENTS_PER_BATCH {
        return Err(CanonicalSyncError::TooManyItems {
            field,
            actual: count,
            maximum: MAX_CANONICAL_SYNC_EVENTS_PER_BATCH,
        });
    }
    let mut events = Vec::with_capacity(count);
    for _ in 0..count {
        let nested = decoder.bytes(field)?;
        ensure_object_bound(nested)?;
        events.push(decode_and_verify_authorized_event_v1(nested)?);
    }
    Ok(events)
}

fn decode_bool(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
) -> Result<bool, CanonicalSyncError> {
    match decoder.unsigned(field)? {
        0 => Ok(false),
        1 => Ok(true),
        actual => Err(CanonicalSyncError::InvalidBoolean { field, actual }),
    }
}

fn encode_conversation_only(
    _object: &'static str,
    conversation_id: [u8; 32],
) -> Result<Vec<u8>, CanonicalSyncError> {
    let mut output = Vec::new();
    push_array_header(&mut output, 2);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &conversation_id);
    bounded(output)
}

fn decode_conversation_only(
    bytes: &[u8],
    object: &'static str,
) -> Result<[u8; 32], CanonicalSyncError> {
    validate_input(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array(object, 2)?;
    decoder.expect_version("sync-conversation-only-v1.schema_version")?;
    let conversation_id = decoder.fixed_bytes("sync-conversation-only-v1.conversation_id")?;
    decoder.finish()?;
    Ok(conversation_id)
}

fn encode_frame(variant: u64, nested: &[u8]) -> Result<Vec<u8>, CanonicalSyncError> {
    ensure_object_bound(nested)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_unsigned(&mut output, variant);
    push_bytes(&mut output, nested);
    bounded(output)
}

fn decode_frame<'a>(
    bytes: &'a [u8],
    object: &'static str,
) -> Result<(u64, &'a [u8]), CanonicalSyncError> {
    validate_input(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array(object, 3)?;
    decoder.expect_version("sync-frame-v1.schema_version")?;
    let variant = decoder.unsigned("sync-frame-v1.variant")?;
    let nested = decoder.bytes("sync-frame-v1.payload")?;
    ensure_object_bound(nested)?;
    decoder.finish()?;
    Ok((variant, nested))
}

fn validate_input(bytes: &[u8]) -> Result<(), CanonicalSyncError> {
    ensure_object_bound(bytes)?;
    validate_canonical_profile_v1(bytes)?;
    Ok(())
}

fn ensure_object_bound(bytes: &[u8]) -> Result<(), CanonicalSyncError> {
    if bytes.len() > MAX_CANONICAL_SYNC_OBJECT_BYTES {
        Err(CanonicalSyncError::ObjectTooLarge)
    } else {
        Ok(())
    }
}

fn bounded(bytes: Vec<u8>) -> Result<Vec<u8>, CanonicalSyncError> {
    ensure_object_bound(&bytes)?;
    Ok(bytes)
}

fn verify_ed25519(
    key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
    object: &'static str,
) -> Result<(), CanonicalSyncError> {
    let key = VerifyingKey::from_bytes(key)
        .map_err(|_| CanonicalSyncError::InvalidPublicKey { object })?;
    let signature = Signature::try_from(signature.as_slice())
        .map_err(|_| CanonicalSyncError::InvalidSignature { object })?;
    key.verify_strict(message, &signature)
        .map_err(|_| CanonicalSyncError::InvalidSignature { object })
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    fn invalid_data(message: impl Into<String>) -> Box<dyn std::error::Error> {
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message.into(),
        ))
    }

    fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if !value.len().is_multiple_of(2) || !value.bytes().all(|value| value.is_ascii_hexdigit()) {
            return Err(invalid_data("invalid vector hexadecimal"));
        }
        (0..value.len())
            .step_by(2)
            .map(|index| Ok(u8::from_str_radix(&value[index..index + 2], 16)?))
            .collect()
    }

    fn pointer_hex(document: &Value, pointer: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        decode_hex(
            document
                .pointer(pointer)
                .and_then(Value::as_str)
                .ok_or_else(|| invalid_data(format!("missing vector pointer {pointer}")))?,
        )
    }

    fn sync_vectors() -> Result<Value, Box<dyn std::error::Error>> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-SYNC-VECTORS.json"
        )))?)
    }

    fn event_vectors() -> Result<Value, Box<dyn std::error::Error>> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-EVENT-VECTORS.json"
        )))?)
    }

    #[test]
    fn golden_sync_objects_and_frames_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let vectors = sync_vectors()?;

        let inventory_bytes = pointer_hex(&vectors, "/expected/inventory/signed_cbor_hex")?;
        let inventory = decode_and_verify_signed_sync_inventory_v1(&inventory_bytes)?;
        assert_eq!(
            encode_signed_sync_inventory_v1_candidate(&inventory)?,
            inventory_bytes
        );

        let batch_bytes = pointer_hex(&vectors, "/expected/event_batch/cbor_hex")?;
        let batch = decode_and_verify_sync_event_batch_v1(&batch_bytes)?;
        assert_eq!(encode_sync_event_batch_v1(&batch)?, batch_bytes);

        let diff_bytes = pointer_hex(&vectors, "/expected/diff/signed_cbor_hex")?;
        let diff = decode_and_verify_signed_sync_diff_v1(&diff_bytes)?;
        assert_eq!(encode_signed_sync_diff_v1(&diff)?, diff_bytes);
        verify_sync_frame_session_v1(&inventory, &diff)?;

        let complete_bytes = pointer_hex(&vectors, "/expected/complete/cbor_hex")?;
        let complete = decode_sync_complete_v1(&complete_bytes)?;
        assert_eq!(encode_sync_complete_v1(&complete)?, complete_bytes);

        let pause_bytes = pointer_hex(&vectors, "/expected/pause/cbor_hex")?;
        let pause = decode_sync_pause_v1(&pause_bytes)?;
        assert_eq!(encode_sync_pause_v1(&pause)?, pause_bytes);

        let paused_bytes = pointer_hex(&vectors, "/expected/paused/cbor_hex")?;
        let paused = decode_sync_paused_v1(&paused_bytes)?;
        assert_eq!(encode_sync_paused_v1(&paused)?, paused_bytes);

        let rejected_bytes = pointer_hex(&vectors, "/expected/rejected/cbor_hex")?;
        let rejected = decode_sync_rejected_v1(&rejected_bytes)?;
        assert_eq!(encode_sync_rejected_v1(&rejected)?, rejected_bytes);

        for pointer in [
            "/expected/client_frames/inventory_cbor_hex",
            "/expected/client_frames/events_cbor_hex",
            "/expected/client_frames/pause_cbor_hex",
        ] {
            let bytes = pointer_hex(&vectors, pointer)?;
            let frame = decode_and_verify_sync_client_frame_v1(&bytes)?;
            assert_eq!(encode_sync_client_frame_v1(&frame)?, bytes);
        }
        for pointer in [
            "/expected/server_frames/diff_cbor_hex",
            "/expected/server_frames/complete_cbor_hex",
            "/expected/server_frames/rejected_cbor_hex",
            "/expected/server_frames/paused_cbor_hex",
        ] {
            let bytes = pointer_hex(&vectors, pointer)?;
            let frame = decode_and_verify_sync_server_frame_v1(&bytes)?;
            assert_eq!(encode_sync_server_frame_v1(&frame)?, bytes);
        }
        Ok(())
    }

    #[test]
    fn nonempty_batches_verify_nested_authorization_and_order()
    -> Result<(), Box<dyn std::error::Error>> {
        let events = event_vectors()?;
        let conversation_id: [u8; 32] = pointer_hex(&events, "/input/conversation_id_hex")?
            .try_into()
            .map_err(|_| invalid_data("invalid conversation ID"))?;
        let text = decode_and_verify_authorized_event_v1(&pointer_hex(
            &events,
            "/expected/authorized_text_event_cbor_hex",
        )?)?;
        let acknowledgement = decode_and_verify_authorized_event_v1(&pointer_hex(
            &events,
            "/expected/authorized_acknowledgement_event_cbor_hex",
        )?)?;
        assert!(event_id_v1(&text.event)? < event_id_v1(&acknowledgement.event)?);

        let batch = CanonicalSyncEventBatchV1 {
            conversation_id,
            events: vec![text.clone(), acknowledgement.clone()],
        };
        let encoded = encode_sync_event_batch_v1(&batch)?;
        assert_eq!(decode_and_verify_sync_event_batch_v1(&encoded)?, batch);

        let reversed = CanonicalSyncEventBatchV1 {
            conversation_id,
            events: vec![acknowledgement, text.clone()],
        };
        assert!(matches!(
            encode_sync_event_batch_v1(&reversed),
            Err(CanonicalSyncError::NonCanonicalEventOrder)
        ));
        let duplicate = CanonicalSyncEventBatchV1 {
            conversation_id,
            events: vec![text.clone(), text],
        };
        assert!(matches!(
            encode_sync_event_batch_v1(&duplicate),
            Err(CanonicalSyncError::NonCanonicalEventOrder)
        ));
        Ok(())
    }

    #[test]
    fn malformed_signatures_booleans_and_variants_fail_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let vectors = sync_vectors()?;

        let mut signed_diff = pointer_hex(&vectors, "/expected/diff/signed_cbor_hex")?;
        let last = signed_diff.len() - 1;
        signed_diff[last] ^= 1;
        assert!(decode_and_verify_signed_sync_diff_v1(&signed_diff).is_err());

        let mut complete = pointer_hex(&vectors, "/expected/complete/cbor_hex")?;
        *complete
            .last_mut()
            .ok_or_else(|| invalid_data("empty complete"))? = 2;
        assert!(matches!(
            decode_sync_complete_v1(&complete),
            Err(CanonicalSyncError::InvalidBoolean { .. })
        ));

        let mut client = pointer_hex(&vectors, "/expected/client_frames/pause_cbor_hex")?;
        client[2] = 4;
        assert!(matches!(
            decode_and_verify_sync_client_frame_v1(&client),
            Err(CanonicalSyncError::UnknownClientFrameVariant(4))
        ));

        let mut server = pointer_hex(&vectors, "/expected/server_frames/paused_cbor_hex")?;
        server[2] = 5;
        assert!(matches!(
            decode_and_verify_sync_server_frame_v1(&server),
            Err(CanonicalSyncError::UnknownServerFrameVariant(5))
        ));
        Ok(())
    }
}
