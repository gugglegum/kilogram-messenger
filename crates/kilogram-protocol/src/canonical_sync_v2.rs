//! Inactive history-compatible deterministic-CBOR synchronization generation.
//!
//! Version 2 carries typed event keys and closed verified event carriers, so
//! installed Postcard history and native canonical events cannot collide or be
//! decoded through format sniffing. The codec is not registered by the active
//! runtime and is not yet advertised by a connection ticket capability.

use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

use crate::{
    canonical_account_v1::{AccountDecoder, CanonicalAccountError, signed_envelope},
    canonical_sync_history_v2::{
        CanonicalSyncEventCarrierV2, CanonicalSyncEventKeyV2, CanonicalSyncHistoryV2Error,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
        push_unsigned, validate_canonical_profile_v1,
    },
};

pub const CANONICAL_SYNC_INVENTORY_SIGNATURE_DOMAIN_V2: &[u8] =
    b"kilogram:sync-inventory-signature:cbor-v2\0";
pub const CANONICAL_SYNC_DIFF_SIGNATURE_DOMAIN_V2: &[u8] =
    b"kilogram:sync-diff-signature:cbor-v2\0";
pub const MAX_CANONICAL_SYNC_V2_INVENTORY_EVENT_KEYS: usize = 4096;
pub const MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH: usize = 64;
pub const MAX_CANONICAL_SYNC_V2_OBJECT_BYTES: usize = 8 * 1024 * 1024;

pub const CLIENT_SYNC_V2_INVENTORY_VARIANT: u64 = 1;
pub const CLIENT_SYNC_V2_EVENTS_VARIANT: u64 = 2;
pub const CLIENT_SYNC_V2_PAUSE_VARIANT: u64 = 3;
pub const SERVER_SYNC_V2_DIFF_VARIANT: u64 = 1;
pub const SERVER_SYNC_V2_COMPLETE_VARIANT: u64 = 2;
pub const SERVER_SYNC_V2_REJECTED_VARIANT: u64 = 3;
pub const SERVER_SYNC_V2_PAUSED_VARIANT: u64 = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncInventoryContentV2 {
    pub conversation_id: [u8; 32],
    pub requester_device_id: [u8; 32],
    pub session_binding: [u8; 32],
    pub event_keys: Vec<CanonicalSyncEventKeyV2>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedSyncInventoryV2 {
    pub content: CanonicalSyncInventoryContentV2,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncEventBatchV2 {
    pub conversation_id: [u8; 32],
    pub events: Vec<CanonicalSyncEventCarrierV2>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncDiffContentV2 {
    pub conversation_id: [u8; 32],
    pub responder_device_id: [u8; 32],
    pub session_binding: [u8; 32],
    pub requested_event_keys: Vec<CanonicalSyncEventKeyV2>,
    pub events: Vec<CanonicalSyncEventCarrierV2>,
    pub more_available: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedSyncDiffV2 {
    pub content: CanonicalSyncDiffContentV2,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncCompleteV2 {
    pub conversation_id: [u8; 32],
    pub stored_event_keys: Vec<CanonicalSyncEventKeyV2>,
    pub more_available: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncPauseV2 {
    pub conversation_id: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncPausedV2 {
    pub conversation_id: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum CanonicalSyncRejectionReasonV2 {
    RequesterNotAllowed = 1,
}

impl CanonicalSyncRejectionReasonV2 {
    const fn code(self) -> u64 {
        self as u64
    }

    fn from_code(code: u64) -> Result<Self, CanonicalSyncV2Error> {
        match code {
            1 => Ok(Self::RequesterNotAllowed),
            other => Err(CanonicalSyncV2Error::UnknownRejectionReason(other)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncRejectedV2 {
    pub conversation_id: [u8; 32],
    pub reason: CanonicalSyncRejectionReasonV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalSyncClientFrameV2 {
    Inventory(CanonicalSignedSyncInventoryV2),
    Events(CanonicalSyncEventBatchV2),
    Pause(CanonicalSyncPauseV2),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalSyncServerFrameV2 {
    Diff(CanonicalSignedSyncDiffV2),
    Complete(CanonicalSyncCompleteV2),
    Rejected(CanonicalSyncRejectedV2),
    Paused(CanonicalSyncPausedV2),
}

#[derive(Debug, Error)]
pub enum CanonicalSyncV2Error {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),

    #[error(transparent)]
    Account(#[from] CanonicalAccountError),

    #[error(transparent)]
    History(#[from] CanonicalSyncHistoryV2Error),

    #[error("canonical sync v2 object exceeds {MAX_CANONICAL_SYNC_V2_OBJECT_BYTES} bytes")]
    ObjectTooLarge,

    #[error("{field} must be strictly ordered by typed Event key")]
    NonCanonicalEventKeyOrder { field: &'static str },

    #[error("canonical sync v2 event batch must be strictly ordered by derived typed Event key")]
    NonCanonicalEventOrder,

    #[error("canonical sync v2 event belongs to a different conversation")]
    EventConversationMismatch,

    #[error("canonical sync v2 inventory and diff name different conversations")]
    FrameConversationMismatch,

    #[error("canonical sync v2 inventory and diff carry different session bindings")]
    SessionBindingMismatch,

    #[error("{field} contains {actual} items; maximum is {maximum}")]
    TooManyItems {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },

    #[error("{field} contains integer {actual}; expected canonical boolean 0 or 1")]
    InvalidBoolean { field: &'static str, actual: u64 },

    #[error("unknown canonical sync v2 rejection reason {0}")]
    UnknownRejectionReason(u64),

    #[error("unknown canonical client sync v2 frame variant {0}")]
    UnknownClientFrameVariant(u64),

    #[error("unknown canonical server sync v2 frame variant {0}")]
    UnknownServerFrameVariant(u64),

    #[error("{object} contains an invalid Ed25519 verifying key")]
    InvalidPublicKey { object: &'static str },

    #[error("{object} contains an invalid Ed25519 signature")]
    InvalidSignature { object: &'static str },
}

pub fn encode_sync_inventory_content_v2(
    content: &CanonicalSyncInventoryContentV2,
) -> Result<Vec<u8>, CanonicalSyncV2Error> {
    validate_ordered_keys(
        "inventory_event_keys",
        &content.event_keys,
        MAX_CANONICAL_SYNC_V2_INVENTORY_EVENT_KEYS,
    )?;
    let mut output = Vec::new();
    push_array_header(&mut output, 5);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.conversation_id);
    push_bytes(&mut output, &content.requester_device_id);
    push_bytes(&mut output, &content.session_binding);
    encode_keys_into(&mut output, &content.event_keys)?;
    bounded(output)
}

pub fn sync_inventory_signing_bytes_v2(
    content: &CanonicalSyncInventoryContentV2,
) -> Result<Vec<u8>, CanonicalSyncV2Error> {
    Ok(prefixed(
        CANONICAL_SYNC_INVENTORY_SIGNATURE_DOMAIN_V2,
        &encode_sync_inventory_content_v2(content)?,
    ))
}

pub fn encode_signed_sync_inventory_v2(
    inventory: &CanonicalSignedSyncInventoryV2,
) -> Result<Vec<u8>, CanonicalSyncV2Error> {
    verify_signed_sync_inventory_v2(inventory)?;
    let content = encode_sync_inventory_content_v2(&inventory.content)?;
    bounded(signed_envelope(&content, &inventory.signature))
}

pub fn decode_signed_sync_inventory_v2(
    bytes: &[u8],
) -> Result<CanonicalSignedSyncInventoryV2, CanonicalSyncV2Error> {
    validate_input(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-sync-inventory-v2", 3)?;
    decoder.expect_version("signed-sync-inventory-v2.schema_version")?;
    let content = decode_inventory_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-sync-inventory-v2.signature")?;
    decoder.finish()?;
    Ok(CanonicalSignedSyncInventoryV2 { content, signature })
}

pub fn decode_and_verify_signed_sync_inventory_v2(
    bytes: &[u8],
) -> Result<CanonicalSignedSyncInventoryV2, CanonicalSyncV2Error> {
    let inventory = decode_signed_sync_inventory_v2(bytes)?;
    verify_signed_sync_inventory_v2(&inventory)?;
    Ok(inventory)
}

pub fn verify_signed_sync_inventory_v2(
    inventory: &CanonicalSignedSyncInventoryV2,
) -> Result<(), CanonicalSyncV2Error> {
    verify_ed25519(
        &inventory.content.requester_device_id,
        &sync_inventory_signing_bytes_v2(&inventory.content)?,
        &inventory.signature,
        "signed-sync-inventory-v2",
    )
}

pub fn encode_sync_event_batch_v2(
    batch: &CanonicalSyncEventBatchV2,
) -> Result<Vec<u8>, CanonicalSyncV2Error> {
    validate_events("batch_events", batch.conversation_id, &batch.events)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &batch.conversation_id);
    encode_events_into(&mut output, &batch.events)?;
    bounded(output)
}

pub fn decode_and_verify_sync_event_batch_v2(
    bytes: &[u8],
) -> Result<CanonicalSyncEventBatchV2, CanonicalSyncV2Error> {
    validate_input(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("sync-event-batch-v2", 3)?;
    decoder.expect_version("sync-event-batch-v2.schema_version")?;
    let conversation_id = decoder.fixed_bytes("sync-event-batch-v2.conversation_id")?;
    let events = decode_events_from(&mut decoder, "sync-event-batch-v2.events")?;
    decoder.finish()?;
    let batch = CanonicalSyncEventBatchV2 {
        conversation_id,
        events,
    };
    validate_events("batch_events", batch.conversation_id, &batch.events)?;
    Ok(batch)
}

pub fn encode_sync_diff_content_v2(
    content: &CanonicalSyncDiffContentV2,
) -> Result<Vec<u8>, CanonicalSyncV2Error> {
    validate_diff_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 7);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.conversation_id);
    push_bytes(&mut output, &content.responder_device_id);
    push_bytes(&mut output, &content.session_binding);
    encode_keys_into(&mut output, &content.requested_event_keys)?;
    encode_events_into(&mut output, &content.events)?;
    push_unsigned(&mut output, u64::from(content.more_available));
    bounded(output)
}

pub fn sync_diff_signing_bytes_v2(
    content: &CanonicalSyncDiffContentV2,
) -> Result<Vec<u8>, CanonicalSyncV2Error> {
    Ok(prefixed(
        CANONICAL_SYNC_DIFF_SIGNATURE_DOMAIN_V2,
        &encode_sync_diff_content_v2(content)?,
    ))
}

pub fn encode_signed_sync_diff_v2(
    diff: &CanonicalSignedSyncDiffV2,
) -> Result<Vec<u8>, CanonicalSyncV2Error> {
    verify_signed_sync_diff_v2(diff)?;
    let content = encode_sync_diff_content_v2(&diff.content)?;
    bounded(signed_envelope(&content, &diff.signature))
}

pub fn decode_signed_sync_diff_v2(
    bytes: &[u8],
) -> Result<CanonicalSignedSyncDiffV2, CanonicalSyncV2Error> {
    validate_input(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-sync-diff-v2", 3)?;
    decoder.expect_version("signed-sync-diff-v2.schema_version")?;
    let content = decode_sync_diff_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-sync-diff-v2.signature")?;
    decoder.finish()?;
    Ok(CanonicalSignedSyncDiffV2 { content, signature })
}

pub fn decode_and_verify_signed_sync_diff_v2(
    bytes: &[u8],
) -> Result<CanonicalSignedSyncDiffV2, CanonicalSyncV2Error> {
    let diff = decode_signed_sync_diff_v2(bytes)?;
    verify_signed_sync_diff_v2(&diff)?;
    Ok(diff)
}

pub fn verify_signed_sync_diff_v2(
    diff: &CanonicalSignedSyncDiffV2,
) -> Result<(), CanonicalSyncV2Error> {
    validate_diff_content(&diff.content)?;
    verify_ed25519(
        &diff.content.responder_device_id,
        &sync_diff_signing_bytes_v2(&diff.content)?,
        &diff.signature,
        "signed-sync-diff-v2",
    )
}

pub fn encode_sync_complete_v2(
    complete: &CanonicalSyncCompleteV2,
) -> Result<Vec<u8>, CanonicalSyncV2Error> {
    validate_ordered_keys(
        "stored_event_keys",
        &complete.stored_event_keys,
        MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH,
    )?;
    let mut output = Vec::new();
    push_array_header(&mut output, 4);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &complete.conversation_id);
    encode_keys_into(&mut output, &complete.stored_event_keys)?;
    push_unsigned(&mut output, u64::from(complete.more_available));
    bounded(output)
}

pub fn decode_sync_complete_v2(
    bytes: &[u8],
) -> Result<CanonicalSyncCompleteV2, CanonicalSyncV2Error> {
    validate_input(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("sync-complete-v2", 4)?;
    decoder.expect_version("sync-complete-v2.schema_version")?;
    let conversation_id = decoder.fixed_bytes("sync-complete-v2.conversation_id")?;
    let stored_event_keys = decode_keys_from(
        &mut decoder,
        "sync-complete-v2.stored_event_keys",
        MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH,
    )?;
    let more_available = decode_bool(&mut decoder, "sync-complete-v2.more_available")?;
    decoder.finish()?;
    Ok(CanonicalSyncCompleteV2 {
        conversation_id,
        stored_event_keys,
        more_available,
    })
}

pub fn encode_sync_pause_v2(pause: &CanonicalSyncPauseV2) -> Result<Vec<u8>, CanonicalSyncV2Error> {
    encode_conversation_only(pause.conversation_id)
}

pub fn decode_sync_pause_v2(bytes: &[u8]) -> Result<CanonicalSyncPauseV2, CanonicalSyncV2Error> {
    Ok(CanonicalSyncPauseV2 {
        conversation_id: decode_conversation_only(bytes, "sync-pause-v2")?,
    })
}

pub fn encode_sync_paused_v2(
    paused: &CanonicalSyncPausedV2,
) -> Result<Vec<u8>, CanonicalSyncV2Error> {
    encode_conversation_only(paused.conversation_id)
}

pub fn decode_sync_paused_v2(bytes: &[u8]) -> Result<CanonicalSyncPausedV2, CanonicalSyncV2Error> {
    Ok(CanonicalSyncPausedV2 {
        conversation_id: decode_conversation_only(bytes, "sync-paused-v2")?,
    })
}

pub fn encode_sync_rejected_v2(
    rejected: &CanonicalSyncRejectedV2,
) -> Result<Vec<u8>, CanonicalSyncV2Error> {
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &rejected.conversation_id);
    push_unsigned(&mut output, rejected.reason.code());
    bounded(output)
}

pub fn decode_sync_rejected_v2(
    bytes: &[u8],
) -> Result<CanonicalSyncRejectedV2, CanonicalSyncV2Error> {
    validate_input(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("sync-rejected-v2", 3)?;
    decoder.expect_version("sync-rejected-v2.schema_version")?;
    let conversation_id = decoder.fixed_bytes("sync-rejected-v2.conversation_id")?;
    let reason =
        CanonicalSyncRejectionReasonV2::from_code(decoder.unsigned("sync-rejected-v2.reason")?)?;
    decoder.finish()?;
    Ok(CanonicalSyncRejectedV2 {
        conversation_id,
        reason,
    })
}

pub fn encode_sync_client_frame_v2(
    frame: &CanonicalSyncClientFrameV2,
) -> Result<Vec<u8>, CanonicalSyncV2Error> {
    let (variant, payload) = match frame {
        CanonicalSyncClientFrameV2::Inventory(value) => (
            CLIENT_SYNC_V2_INVENTORY_VARIANT,
            encode_signed_sync_inventory_v2(value)?,
        ),
        CanonicalSyncClientFrameV2::Events(value) => (
            CLIENT_SYNC_V2_EVENTS_VARIANT,
            encode_sync_event_batch_v2(value)?,
        ),
        CanonicalSyncClientFrameV2::Pause(value) => {
            (CLIENT_SYNC_V2_PAUSE_VARIANT, encode_sync_pause_v2(value)?)
        }
    };
    encode_frame(variant, &payload)
}

pub fn decode_and_verify_sync_client_frame_v2(
    bytes: &[u8],
) -> Result<CanonicalSyncClientFrameV2, CanonicalSyncV2Error> {
    let (variant, payload) = decode_frame(bytes, "sync-client-frame-v2")?;
    match variant {
        CLIENT_SYNC_V2_INVENTORY_VARIANT => Ok(CanonicalSyncClientFrameV2::Inventory(
            decode_and_verify_signed_sync_inventory_v2(payload)?,
        )),
        CLIENT_SYNC_V2_EVENTS_VARIANT => Ok(CanonicalSyncClientFrameV2::Events(
            decode_and_verify_sync_event_batch_v2(payload)?,
        )),
        CLIENT_SYNC_V2_PAUSE_VARIANT => Ok(CanonicalSyncClientFrameV2::Pause(
            decode_sync_pause_v2(payload)?,
        )),
        other => Err(CanonicalSyncV2Error::UnknownClientFrameVariant(other)),
    }
}

pub fn encode_sync_server_frame_v2(
    frame: &CanonicalSyncServerFrameV2,
) -> Result<Vec<u8>, CanonicalSyncV2Error> {
    let (variant, payload) = match frame {
        CanonicalSyncServerFrameV2::Diff(value) => (
            SERVER_SYNC_V2_DIFF_VARIANT,
            encode_signed_sync_diff_v2(value)?,
        ),
        CanonicalSyncServerFrameV2::Complete(value) => (
            SERVER_SYNC_V2_COMPLETE_VARIANT,
            encode_sync_complete_v2(value)?,
        ),
        CanonicalSyncServerFrameV2::Rejected(value) => (
            SERVER_SYNC_V2_REJECTED_VARIANT,
            encode_sync_rejected_v2(value)?,
        ),
        CanonicalSyncServerFrameV2::Paused(value) => {
            (SERVER_SYNC_V2_PAUSED_VARIANT, encode_sync_paused_v2(value)?)
        }
    };
    encode_frame(variant, &payload)
}

pub fn decode_and_verify_sync_server_frame_v2(
    bytes: &[u8],
) -> Result<CanonicalSyncServerFrameV2, CanonicalSyncV2Error> {
    let (variant, payload) = decode_frame(bytes, "sync-server-frame-v2")?;
    match variant {
        SERVER_SYNC_V2_DIFF_VARIANT => Ok(CanonicalSyncServerFrameV2::Diff(
            decode_and_verify_signed_sync_diff_v2(payload)?,
        )),
        SERVER_SYNC_V2_COMPLETE_VARIANT => Ok(CanonicalSyncServerFrameV2::Complete(
            decode_sync_complete_v2(payload)?,
        )),
        SERVER_SYNC_V2_REJECTED_VARIANT => Ok(CanonicalSyncServerFrameV2::Rejected(
            decode_sync_rejected_v2(payload)?,
        )),
        SERVER_SYNC_V2_PAUSED_VARIANT => Ok(CanonicalSyncServerFrameV2::Paused(
            decode_sync_paused_v2(payload)?,
        )),
        other => Err(CanonicalSyncV2Error::UnknownServerFrameVariant(other)),
    }
}

pub fn verify_sync_frame_session_v2(
    inventory: &CanonicalSignedSyncInventoryV2,
    diff: &CanonicalSignedSyncDiffV2,
) -> Result<(), CanonicalSyncV2Error> {
    verify_signed_sync_inventory_v2(inventory)?;
    verify_signed_sync_diff_v2(diff)?;
    if inventory.content.conversation_id != diff.content.conversation_id {
        return Err(CanonicalSyncV2Error::FrameConversationMismatch);
    }
    if inventory.content.session_binding != diff.content.session_binding {
        return Err(CanonicalSyncV2Error::SessionBindingMismatch);
    }
    Ok(())
}

fn decode_inventory_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalSyncInventoryContentV2, CanonicalSyncV2Error> {
    decoder.expect_array("sync-inventory-content-v2", 5)?;
    decoder.expect_version("sync-inventory-content-v2.schema_version")?;
    let conversation_id = decoder.fixed_bytes("sync-inventory-content-v2.conversation_id")?;
    let requester_device_id =
        decoder.fixed_bytes("sync-inventory-content-v2.requester_device_id")?;
    let session_binding = decoder.fixed_bytes("sync-inventory-content-v2.session_binding")?;
    let event_keys = decode_keys_from(
        decoder,
        "sync-inventory-content-v2.event_keys",
        MAX_CANONICAL_SYNC_V2_INVENTORY_EVENT_KEYS,
    )?;
    Ok(CanonicalSyncInventoryContentV2 {
        conversation_id,
        requester_device_id,
        session_binding,
        event_keys,
    })
}

fn decode_sync_diff_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalSyncDiffContentV2, CanonicalSyncV2Error> {
    decoder.expect_array("sync-diff-content-v2", 7)?;
    decoder.expect_version("sync-diff-content-v2.schema_version")?;
    let conversation_id = decoder.fixed_bytes("sync-diff-content-v2.conversation_id")?;
    let responder_device_id = decoder.fixed_bytes("sync-diff-content-v2.responder_device_id")?;
    let session_binding = decoder.fixed_bytes("sync-diff-content-v2.session_binding")?;
    let requested_event_keys = decode_keys_from(
        decoder,
        "sync-diff-content-v2.requested_event_keys",
        MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH,
    )?;
    let events = decode_events_from(decoder, "sync-diff-content-v2.events")?;
    let more_available = decode_bool(decoder, "sync-diff-content-v2.more_available")?;
    let content = CanonicalSyncDiffContentV2 {
        conversation_id,
        responder_device_id,
        session_binding,
        requested_event_keys,
        events,
        more_available,
    };
    validate_diff_content(&content)?;
    Ok(content)
}

fn validate_diff_content(content: &CanonicalSyncDiffContentV2) -> Result<(), CanonicalSyncV2Error> {
    validate_ordered_keys(
        "requested_event_keys",
        &content.requested_event_keys,
        MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH,
    )?;
    validate_events("diff_events", content.conversation_id, &content.events)
}

fn validate_events(
    field: &'static str,
    conversation_id: [u8; 32],
    events: &[CanonicalSyncEventCarrierV2],
) -> Result<(), CanonicalSyncV2Error> {
    if events.len() > MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH {
        return Err(CanonicalSyncV2Error::TooManyItems {
            field,
            actual: events.len(),
            maximum: MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH,
        });
    }
    let mut previous = None;
    for event in events {
        if event.conversation_id() != &conversation_id {
            return Err(CanonicalSyncV2Error::EventConversationMismatch);
        }
        let key = event.key()?;
        if previous.is_some_and(|value| value >= key) {
            return Err(CanonicalSyncV2Error::NonCanonicalEventOrder);
        }
        previous = Some(key);
        event.encode()?;
    }
    Ok(())
}

fn validate_ordered_keys(
    field: &'static str,
    keys: &[CanonicalSyncEventKeyV2],
    maximum: usize,
) -> Result<(), CanonicalSyncV2Error> {
    if keys.len() > maximum {
        return Err(CanonicalSyncV2Error::TooManyItems {
            field,
            actual: keys.len(),
            maximum,
        });
    }
    if keys.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(CanonicalSyncV2Error::NonCanonicalEventKeyOrder { field });
    }
    Ok(())
}

fn encode_keys_into(
    output: &mut Vec<u8>,
    keys: &[CanonicalSyncEventKeyV2],
) -> Result<(), CanonicalSyncV2Error> {
    push_array_header(output, keys.len() as u64);
    for key in keys {
        push_bytes(output, &key.encode()?);
    }
    Ok(())
}

fn decode_keys_from(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
    maximum: usize,
) -> Result<Vec<CanonicalSyncEventKeyV2>, CanonicalSyncV2Error> {
    let count = decoder.array(field)?;
    if count > maximum {
        return Err(CanonicalSyncV2Error::TooManyItems {
            field,
            actual: count,
            maximum,
        });
    }
    let mut keys = Vec::with_capacity(count);
    for _ in 0..count {
        let nested = decoder.bytes(field)?;
        ensure_object_bound(nested)?;
        keys.push(CanonicalSyncEventKeyV2::decode(nested)?);
    }
    validate_ordered_keys(field, &keys, maximum)?;
    Ok(keys)
}

fn encode_events_into(
    output: &mut Vec<u8>,
    events: &[CanonicalSyncEventCarrierV2],
) -> Result<(), CanonicalSyncV2Error> {
    push_array_header(output, events.len() as u64);
    for event in events {
        push_bytes(output, &event.encode()?);
    }
    Ok(())
}

fn decode_events_from(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
) -> Result<Vec<CanonicalSyncEventCarrierV2>, CanonicalSyncV2Error> {
    let count = decoder.array(field)?;
    if count > MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH {
        return Err(CanonicalSyncV2Error::TooManyItems {
            field,
            actual: count,
            maximum: MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH,
        });
    }
    let mut events = Vec::with_capacity(count);
    for _ in 0..count {
        let nested = decoder.bytes(field)?;
        ensure_object_bound(nested)?;
        events.push(CanonicalSyncEventCarrierV2::decode_and_verify(nested)?);
    }
    Ok(events)
}

fn decode_bool(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
) -> Result<bool, CanonicalSyncV2Error> {
    match decoder.unsigned(field)? {
        0 => Ok(false),
        1 => Ok(true),
        actual => Err(CanonicalSyncV2Error::InvalidBoolean { field, actual }),
    }
}

fn encode_conversation_only(conversation_id: [u8; 32]) -> Result<Vec<u8>, CanonicalSyncV2Error> {
    let mut output = Vec::new();
    push_array_header(&mut output, 2);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &conversation_id);
    bounded(output)
}

fn decode_conversation_only(
    bytes: &[u8],
    object: &'static str,
) -> Result<[u8; 32], CanonicalSyncV2Error> {
    validate_input(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array(object, 2)?;
    decoder.expect_version("sync-conversation-only-v2.schema_version")?;
    let conversation_id = decoder.fixed_bytes("sync-conversation-only-v2.conversation_id")?;
    decoder.finish()?;
    Ok(conversation_id)
}

fn encode_frame(variant: u64, payload: &[u8]) -> Result<Vec<u8>, CanonicalSyncV2Error> {
    ensure_object_bound(payload)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_unsigned(&mut output, variant);
    push_bytes(&mut output, payload);
    bounded(output)
}

fn decode_frame<'a>(
    bytes: &'a [u8],
    object: &'static str,
) -> Result<(u64, &'a [u8]), CanonicalSyncV2Error> {
    validate_input(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array(object, 3)?;
    decoder.expect_version("sync-frame-v2.schema_version")?;
    let variant = decoder.unsigned("sync-frame-v2.variant")?;
    let payload = decoder.bytes("sync-frame-v2.payload")?;
    ensure_object_bound(payload)?;
    decoder.finish()?;
    Ok((variant, payload))
}

fn validate_input(bytes: &[u8]) -> Result<(), CanonicalSyncV2Error> {
    ensure_object_bound(bytes)?;
    validate_canonical_profile_v1(bytes)?;
    Ok(())
}

fn ensure_object_bound(bytes: &[u8]) -> Result<(), CanonicalSyncV2Error> {
    if bytes.len() > MAX_CANONICAL_SYNC_V2_OBJECT_BYTES {
        Err(CanonicalSyncV2Error::ObjectTooLarge)
    } else {
        Ok(())
    }
}

fn bounded(bytes: Vec<u8>) -> Result<Vec<u8>, CanonicalSyncV2Error> {
    ensure_object_bound(&bytes)?;
    Ok(bytes)
}

fn verify_ed25519(
    key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
    object: &'static str,
) -> Result<(), CanonicalSyncV2Error> {
    let key = VerifyingKey::from_bytes(key)
        .map_err(|_| CanonicalSyncV2Error::InvalidPublicKey { object })?;
    let signature = Signature::try_from(signature.as_slice())
        .map_err(|_| CanonicalSyncV2Error::InvalidSignature { object })?;
    key.verify_strict(message, &signature)
        .map_err(|_| CanonicalSyncV2Error::InvalidSignature { object })
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
        if !value.len().is_multiple_of(2)
            || !value.bytes().all(|value| value.is_ascii_hexdigit())
            || value.bytes().any(|value| value.is_ascii_uppercase())
        {
            return Err(invalid_data("invalid lowercase vector hexadecimal"));
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

    fn pointer_usize(document: &Value, pointer: &str) -> Result<usize, Box<dyn std::error::Error>> {
        usize::try_from(
            document
                .pointer(pointer)
                .and_then(Value::as_u64)
                .ok_or_else(|| invalid_data(format!("missing vector pointer {pointer}")))?,
        )
        .map_err(|error| error.into())
    }

    fn vectors(path: &str) -> Result<Value, Box<dyn std::error::Error>> {
        let source = match path {
            "sync" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../CANONICAL-SYNC-V2-VECTORS.json"
            )),
            "history" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../CANONICAL-SYNC-HISTORY-V2-VECTORS.json"
            )),
            _ => return Err(invalid_data("unknown vector document")),
        };
        Ok(serde_json::from_str(source)?)
    }

    fn reconstructed_event_batch(
        sync: &Value,
        history: &Value,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut bytes = pointer_hex(sync, "/expected/event_batch/prefix_hex")?;
        bytes.extend(pointer_hex(history, "/expected/legacy_carrier_cbor_hex")?);
        assert_eq!(
            bytes.len(),
            pointer_usize(sync, "/expected/event_batch/encoded_length")?
        );
        Ok(bytes)
    }

    fn reconstructed_event_frame(
        sync: &Value,
        history: &Value,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut bytes = pointer_hex(sync, "/expected/client_frames/events/prefix_hex")?;
        bytes.extend(reconstructed_event_batch(sync, history)?);
        assert_eq!(
            bytes.len(),
            pointer_usize(sync, "/expected/client_frames/events/encoded_length")?
        );
        Ok(bytes)
    }

    #[test]
    fn golden_history_compatible_objects_and_frames_round_trip()
    -> Result<(), Box<dyn std::error::Error>> {
        let sync = vectors("sync")?;
        let history = vectors("history")?;
        assert_eq!(sync["milestone"], "M1.1.54");
        assert_eq!(sync["activation_status"], "not-active");

        let inventory_bytes = pointer_hex(&sync, "/expected/inventory/signed_cbor_hex")?;
        let inventory = decode_and_verify_signed_sync_inventory_v2(&inventory_bytes)?;
        assert_eq!(
            encode_signed_sync_inventory_v2(&inventory)?,
            inventory_bytes
        );

        let batch_bytes = reconstructed_event_batch(&sync, &history)?;
        let batch = decode_and_verify_sync_event_batch_v2(&batch_bytes)?;
        assert_eq!(encode_sync_event_batch_v2(&batch)?, batch_bytes);

        let diff_bytes = pointer_hex(&sync, "/expected/diff/signed_cbor_hex")?;
        let diff = decode_and_verify_signed_sync_diff_v2(&diff_bytes)?;
        assert_eq!(encode_signed_sync_diff_v2(&diff)?, diff_bytes);
        verify_sync_frame_session_v2(&inventory, &diff)?;

        let complete_bytes = pointer_hex(&sync, "/expected/complete/cbor_hex")?;
        let complete = decode_sync_complete_v2(&complete_bytes)?;
        assert_eq!(encode_sync_complete_v2(&complete)?, complete_bytes);

        let pause_bytes = pointer_hex(&sync, "/expected/pause/cbor_hex")?;
        let pause = decode_sync_pause_v2(&pause_bytes)?;
        assert_eq!(encode_sync_pause_v2(&pause)?, pause_bytes);
        let paused_bytes = pointer_hex(&sync, "/expected/paused/cbor_hex")?;
        let paused = decode_sync_paused_v2(&paused_bytes)?;
        assert_eq!(encode_sync_paused_v2(&paused)?, paused_bytes);
        let rejected_bytes = pointer_hex(&sync, "/expected/rejected/cbor_hex")?;
        let rejected = decode_sync_rejected_v2(&rejected_bytes)?;
        assert_eq!(encode_sync_rejected_v2(&rejected)?, rejected_bytes);

        for pointer in [
            "/expected/client_frames/inventory_cbor_hex",
            "/expected/client_frames/pause_cbor_hex",
        ] {
            let bytes = pointer_hex(&sync, pointer)?;
            let frame = decode_and_verify_sync_client_frame_v2(&bytes)?;
            assert_eq!(encode_sync_client_frame_v2(&frame)?, bytes);
        }
        let event_frame_bytes = reconstructed_event_frame(&sync, &history)?;
        let event_frame = decode_and_verify_sync_client_frame_v2(&event_frame_bytes)?;
        assert_eq!(
            encode_sync_client_frame_v2(&event_frame)?,
            event_frame_bytes
        );

        for pointer in [
            "/expected/server_frames/diff_cbor_hex",
            "/expected/server_frames/complete_cbor_hex",
            "/expected/server_frames/rejected_cbor_hex",
            "/expected/server_frames/paused_cbor_hex",
        ] {
            let bytes = pointer_hex(&sync, pointer)?;
            let frame = decode_and_verify_sync_server_frame_v2(&bytes)?;
            assert_eq!(encode_sync_server_frame_v2(&frame)?, bytes);
        }
        Ok(())
    }

    #[test]
    fn both_closed_event_carriers_integrate_without_format_sniffing()
    -> Result<(), Box<dyn std::error::Error>> {
        let history = vectors("history")?;
        for pointer in [
            "/expected/legacy_carrier_cbor_hex",
            "/expected/canonical_carrier_cbor_hex",
        ] {
            let carrier =
                CanonicalSyncEventCarrierV2::decode_and_verify(&pointer_hex(&history, pointer)?)?;
            let batch = CanonicalSyncEventBatchV2 {
                conversation_id: *carrier.conversation_id(),
                events: vec![carrier.clone()],
            };
            let encoded = encode_sync_event_batch_v2(&batch)?;
            assert_eq!(decode_and_verify_sync_event_batch_v2(&encoded)?, batch);

            let frame = CanonicalSyncClientFrameV2::Events(batch);
            let encoded_frame = encode_sync_client_frame_v2(&frame)?;
            assert_eq!(
                decode_and_verify_sync_client_frame_v2(&encoded_frame)?,
                frame
            );
        }
        Ok(())
    }

    #[test]
    fn malformed_identity_signatures_booleans_and_variants_fail_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let sync = vectors("sync")?;
        let history = vectors("history")?;

        let mut inventory_bytes = pointer_hex(&sync, "/expected/inventory/signed_cbor_hex")?;
        let last = inventory_bytes.len() - 1;
        inventory_bytes[last] ^= 1;
        assert!(decode_and_verify_signed_sync_inventory_v2(&inventory_bytes).is_err());

        let inventory_bytes = pointer_hex(&sync, "/expected/inventory/signed_cbor_hex")?;
        let inventory = decode_and_verify_signed_sync_inventory_v2(&inventory_bytes)?;
        let key = inventory.content.event_keys[0];
        let duplicate = CanonicalSyncInventoryContentV2 {
            event_keys: vec![key, key],
            ..inventory.content.clone()
        };
        assert!(matches!(
            encode_sync_inventory_content_v2(&duplicate),
            Err(CanonicalSyncV2Error::NonCanonicalEventKeyOrder { .. })
        ));

        let legacy = CanonicalSyncEventCarrierV2::decode_and_verify(&pointer_hex(
            &history,
            "/expected/legacy_carrier_cbor_hex",
        )?)?;
        let wrong_conversation = CanonicalSyncEventBatchV2 {
            conversation_id: [0_u8; 32],
            events: vec![legacy],
        };
        assert!(matches!(
            encode_sync_event_batch_v2(&wrong_conversation),
            Err(CanonicalSyncV2Error::EventConversationMismatch)
        ));

        let mut complete = pointer_hex(&sync, "/expected/complete/cbor_hex")?;
        *complete
            .last_mut()
            .ok_or_else(|| invalid_data("empty complete"))? = 2;
        assert!(matches!(
            decode_sync_complete_v2(&complete),
            Err(CanonicalSyncV2Error::InvalidBoolean { .. })
        ));

        let mut client = pointer_hex(&sync, "/expected/client_frames/pause_cbor_hex")?;
        client[2] = 4;
        assert!(matches!(
            decode_and_verify_sync_client_frame_v2(&client),
            Err(CanonicalSyncV2Error::UnknownClientFrameVariant(4))
        ));
        let mut server = pointer_hex(&sync, "/expected/server_frames/paused_cbor_hex")?;
        server[2] = 5;
        assert!(matches!(
            decode_and_verify_sync_server_frame_v2(&server),
            Err(CanonicalSyncV2Error::UnknownServerFrameVariant(5))
        ));

        let mut trailing = reconstructed_event_batch(&sync, &history)?;
        trailing.push(0);
        assert!(decode_and_verify_sync_event_batch_v2(&trailing).is_err());
        Ok(())
    }
}
