//! Inactive deterministic-CBOR durable synchronization session for canonical sync v2.
//!
//! Checkpoints form a Device-signed hash-linked chain. They make a future
//! history-compatible multi-round sync implementation resumable without
//! trusting ambient process memory. Each checkpoint binds the exact `/cbor/2`
//! protocol identifier and preserves typed Event namespaces, while leaving the
//! active Postcard v6 runtime unchanged.

use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

use crate::{
    canonical_account_v1::{AccountDecoder, CanonicalAccountError, signed_envelope},
    canonical_sync_history_v2::{CanonicalSyncEventKeyV2, CanonicalSyncHistoryV2Error},
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
        push_unsigned, validate_canonical_profile_v1,
    },
};

pub const CANONICAL_SYNC_SESSION_PROTOCOL_IDENTIFIER_V2: &[u8; 23] = b"kilogram/m1/sync/cbor/2";
pub const CANONICAL_SYNC_SESSION_CHECKPOINT_SIGNATURE_DOMAIN_V2: &[u8] =
    b"kilogram:sync-session-checkpoint-signature:cbor-v2\0";
pub const CANONICAL_SYNC_SESSION_CHECKPOINT_ID_DOMAIN_V2: &[u8] =
    b"kilogram:sync-session-checkpoint-id:cbor-v2\0";
pub const MAX_CANONICAL_SYNC_SESSION_EVENT_KEYS_V2: usize = 4096;
pub const MAX_CANONICAL_SYNC_SESSION_CHECKPOINT_BYTES_V2: usize = 512 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum CanonicalSyncNextActionV2 {
    SendInventory = 1,
    ReceiveDiff = 2,
    SendEventBatch = 3,
    ReceiveComplete = 4,
    Complete = 5,
    Rejected = 6,
}

impl CanonicalSyncNextActionV2 {
    const fn code(self) -> u64 {
        self as u64
    }

    fn from_code(code: u64) -> Result<Self, CanonicalSyncSessionV2Error> {
        match code {
            1 => Ok(Self::SendInventory),
            2 => Ok(Self::ReceiveDiff),
            3 => Ok(Self::SendEventBatch),
            4 => Ok(Self::ReceiveComplete),
            5 => Ok(Self::Complete),
            6 => Ok(Self::Rejected),
            other => Err(CanonicalSyncSessionV2Error::UnknownNextAction(other)),
        }
    }

    const fn is_terminal(self) -> bool {
        matches!(self, Self::Complete | Self::Rejected)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncSessionCheckpointContentV2 {
    pub protocol_identifier: [u8; 23],
    pub conversation_id: [u8; 32],
    pub local_device_id: [u8; 32],
    pub peer_device_id: [u8; 32],
    pub session_binding: [u8; 32],
    pub checkpoint_sequence: u64,
    pub sync_round: u64,
    pub predecessor_checkpoint_id: Option<[u8; 32]>,
    pub received_event_keys: Vec<CanonicalSyncEventKeyV2>,
    pub sent_event_keys: Vec<CanonicalSyncEventKeyV2>,
    pub local_more_available: bool,
    pub peer_more_available: bool,
    pub next_action: CanonicalSyncNextActionV2,
    pub paused: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedSyncSessionCheckpointV2 {
    pub content: CanonicalSyncSessionCheckpointContentV2,
    pub signature: [u8; 64],
}

#[derive(Debug, Error)]
pub enum CanonicalSyncSessionV2Error {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),

    #[error(transparent)]
    Account(#[from] CanonicalAccountError),

    #[error(transparent)]
    History(#[from] CanonicalSyncHistoryV2Error),

    #[error(
        "canonical sync session checkpoint exceeds {MAX_CANONICAL_SYNC_SESSION_CHECKPOINT_BYTES_V2} bytes"
    )]
    CheckpointTooLarge,

    #[error("checkpoint does not bind the exact canonical sync v2 protocol identifier")]
    WrongProtocolIdentifier,

    #[error("local and peer Device IDs must differ")]
    SameDevice,

    #[error("checkpoint sequence must start at one")]
    InvalidCheckpointSequence,

    #[error("sync round must be positive and no greater than checkpoint sequence")]
    InvalidSyncRound,

    #[error("checkpoint predecessor presence does not match its sequence")]
    InvalidPredecessorPresence,

    #[error("{field} contains {actual} items; maximum is {maximum}")]
    TooManyEventKeys {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },

    #[error("{field} must be strictly ordered by typed Event key")]
    NonCanonicalEventKeyOrder { field: &'static str },

    #[error("{field} contains integer {actual}; expected canonical boolean 0 or 1")]
    InvalidBoolean { field: &'static str, actual: u64 },

    #[error("unknown canonical sync next action {0}")]
    UnknownNextAction(u64),

    #[error("terminal checkpoint cannot be paused or advertise more events")]
    InvalidTerminalState,

    #[error("non-terminal checkpoint cannot clear both availability flags at receive-complete")]
    AmbiguousReceiveComplete,

    #[error("{object} contains an invalid Ed25519 verifying key")]
    InvalidPublicKey { object: &'static str },

    #[error("{object} contains an invalid Ed25519 signature")]
    InvalidSignature { object: &'static str },

    #[error("successor changes immutable session identity")]
    SessionIdentityChanged,

    #[error("successor checkpoint sequence is not exactly previous plus one")]
    CheckpointSequenceDiscontinuity,

    #[error("successor predecessor does not equal the exact previous checkpoint ID")]
    PredecessorMismatch,

    #[error("successor sync round rolls back or skips a round")]
    SyncRoundDiscontinuity,

    #[error("successor removes cumulative {field} evidence")]
    EventEvidenceRollback { field: &'static str },

    #[error("terminal checkpoint cannot have a successor")]
    TerminalCheckpointHasSuccessor,

    #[error("pause or resume transition changed state other than the paused flag")]
    InvalidPauseTransition,

    #[error("sync session next-action transition is invalid")]
    InvalidActionTransition,
}

pub fn encode_sync_session_checkpoint_content_v2(
    content: &CanonicalSyncSessionCheckpointContentV2,
) -> Result<Vec<u8>, CanonicalSyncSessionV2Error> {
    validate_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 15);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.protocol_identifier);
    push_bytes(&mut output, &content.conversation_id);
    push_bytes(&mut output, &content.local_device_id);
    push_bytes(&mut output, &content.peer_device_id);
    push_bytes(&mut output, &content.session_binding);
    push_unsigned(&mut output, content.checkpoint_sequence);
    push_unsigned(&mut output, content.sync_round);
    encode_optional_id(&mut output, content.predecessor_checkpoint_id);
    encode_keys(&mut output, &content.received_event_keys)?;
    encode_keys(&mut output, &content.sent_event_keys)?;
    push_unsigned(&mut output, u64::from(content.local_more_available));
    push_unsigned(&mut output, u64::from(content.peer_more_available));
    push_unsigned(&mut output, content.next_action.code());
    push_unsigned(&mut output, u64::from(content.paused));
    bounded(output)
}

pub fn sync_session_checkpoint_signing_bytes_v2(
    content: &CanonicalSyncSessionCheckpointContentV2,
) -> Result<Vec<u8>, CanonicalSyncSessionV2Error> {
    Ok(prefixed(
        CANONICAL_SYNC_SESSION_CHECKPOINT_SIGNATURE_DOMAIN_V2,
        &encode_sync_session_checkpoint_content_v2(content)?,
    ))
}

pub fn encode_signed_sync_session_checkpoint_v2(
    checkpoint: &CanonicalSignedSyncSessionCheckpointV2,
) -> Result<Vec<u8>, CanonicalSyncSessionV2Error> {
    verify_signed_sync_session_checkpoint_v2(checkpoint)?;
    let content = encode_sync_session_checkpoint_content_v2(&checkpoint.content)?;
    bounded(signed_envelope(&content, &checkpoint.signature))
}

pub fn decode_signed_sync_session_checkpoint_v2(
    bytes: &[u8],
) -> Result<CanonicalSignedSyncSessionCheckpointV2, CanonicalSyncSessionV2Error> {
    validate_input(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-sync-session-checkpoint-v2", 3)?;
    decoder.expect_version("signed-sync-session-checkpoint-v2.schema_version")?;
    let content = decode_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-sync-session-checkpoint-v2.signature")?;
    decoder.finish()?;
    Ok(CanonicalSignedSyncSessionCheckpointV2 { content, signature })
}

pub fn decode_and_verify_signed_sync_session_checkpoint_v2(
    bytes: &[u8],
) -> Result<CanonicalSignedSyncSessionCheckpointV2, CanonicalSyncSessionV2Error> {
    let checkpoint = decode_signed_sync_session_checkpoint_v2(bytes)?;
    verify_signed_sync_session_checkpoint_v2(&checkpoint)?;
    Ok(checkpoint)
}

pub fn verify_signed_sync_session_checkpoint_v2(
    checkpoint: &CanonicalSignedSyncSessionCheckpointV2,
) -> Result<(), CanonicalSyncSessionV2Error> {
    validate_content(&checkpoint.content)?;
    verify_ed25519(
        &checkpoint.content.local_device_id,
        &sync_session_checkpoint_signing_bytes_v2(&checkpoint.content)?,
        &checkpoint.signature,
        "signed-sync-session-checkpoint-v2",
    )
}

pub fn sync_session_checkpoint_id_v2(
    checkpoint: &CanonicalSignedSyncSessionCheckpointV2,
) -> Result<[u8; 32], CanonicalSyncSessionV2Error> {
    let encoded = encode_signed_sync_session_checkpoint_v2(checkpoint)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(CANONICAL_SYNC_SESSION_CHECKPOINT_ID_DOMAIN_V2);
    hasher.update(&encoded);
    Ok(*hasher.finalize().as_bytes())
}

pub fn verify_sync_session_checkpoint_successor_v2(
    previous: &CanonicalSignedSyncSessionCheckpointV2,
    successor: &CanonicalSignedSyncSessionCheckpointV2,
) -> Result<(), CanonicalSyncSessionV2Error> {
    verify_signed_sync_session_checkpoint_v2(previous)?;
    verify_signed_sync_session_checkpoint_v2(successor)?;
    let previous_content = &previous.content;
    let successor_content = &successor.content;
    if previous_content.next_action.is_terminal() {
        return Err(CanonicalSyncSessionV2Error::TerminalCheckpointHasSuccessor);
    }
    if previous_content.protocol_identifier != successor_content.protocol_identifier
        || previous_content.conversation_id != successor_content.conversation_id
        || previous_content.local_device_id != successor_content.local_device_id
        || previous_content.peer_device_id != successor_content.peer_device_id
        || previous_content.session_binding != successor_content.session_binding
    {
        return Err(CanonicalSyncSessionV2Error::SessionIdentityChanged);
    }
    if previous_content.checkpoint_sequence.checked_add(1)
        != Some(successor_content.checkpoint_sequence)
    {
        return Err(CanonicalSyncSessionV2Error::CheckpointSequenceDiscontinuity);
    }
    if successor_content.predecessor_checkpoint_id != Some(sync_session_checkpoint_id_v2(previous)?)
    {
        return Err(CanonicalSyncSessionV2Error::PredecessorMismatch);
    }
    if successor_content.sync_round < previous_content.sync_round
        || successor_content.sync_round > previous_content.sync_round.saturating_add(1)
    {
        return Err(CanonicalSyncSessionV2Error::SyncRoundDiscontinuity);
    }
    require_monotonic_evidence(
        "received_event_keys",
        &previous_content.received_event_keys,
        &successor_content.received_event_keys,
    )?;
    require_monotonic_evidence(
        "sent_event_keys",
        &previous_content.sent_event_keys,
        &successor_content.sent_event_keys,
    )?;

    if previous_content.paused || successor_content.paused {
        validate_pause_transition(previous_content, successor_content)?;
        return Ok(());
    }

    if previous_content.sync_round.checked_add(1) == Some(successor_content.sync_round) {
        if previous_content.next_action != CanonicalSyncNextActionV2::ReceiveComplete
            || successor_content.next_action != CanonicalSyncNextActionV2::SendInventory
            || (!previous_content.local_more_available && !previous_content.peer_more_available)
            || previous_content.received_event_keys != successor_content.received_event_keys
            || previous_content.sent_event_keys != successor_content.sent_event_keys
            || previous_content.local_more_available != successor_content.local_more_available
            || previous_content.peer_more_available != successor_content.peer_more_available
        {
            return Err(CanonicalSyncSessionV2Error::InvalidActionTransition);
        }
        return Ok(());
    }

    if successor_content.next_action == CanonicalSyncNextActionV2::Rejected {
        return Ok(());
    }
    let valid = matches!(
        (previous_content.next_action, successor_content.next_action),
        (
            CanonicalSyncNextActionV2::SendInventory,
            CanonicalSyncNextActionV2::ReceiveDiff
        ) | (
            CanonicalSyncNextActionV2::ReceiveDiff,
            CanonicalSyncNextActionV2::SendEventBatch
        ) | (
            CanonicalSyncNextActionV2::ReceiveDiff,
            CanonicalSyncNextActionV2::ReceiveComplete
        ) | (
            CanonicalSyncNextActionV2::SendEventBatch,
            CanonicalSyncNextActionV2::ReceiveComplete
        ) | (
            CanonicalSyncNextActionV2::ReceiveComplete,
            CanonicalSyncNextActionV2::Complete
        )
    );
    if valid {
        Ok(())
    } else {
        Err(CanonicalSyncSessionV2Error::InvalidActionTransition)
    }
}

fn validate_content(
    content: &CanonicalSyncSessionCheckpointContentV2,
) -> Result<(), CanonicalSyncSessionV2Error> {
    if content.protocol_identifier != *CANONICAL_SYNC_SESSION_PROTOCOL_IDENTIFIER_V2 {
        return Err(CanonicalSyncSessionV2Error::WrongProtocolIdentifier);
    }
    if content.local_device_id == content.peer_device_id {
        return Err(CanonicalSyncSessionV2Error::SameDevice);
    }
    if content.checkpoint_sequence == 0 {
        return Err(CanonicalSyncSessionV2Error::InvalidCheckpointSequence);
    }
    if content.sync_round == 0 || content.sync_round > content.checkpoint_sequence {
        return Err(CanonicalSyncSessionV2Error::InvalidSyncRound);
    }
    if (content.checkpoint_sequence == 1) != content.predecessor_checkpoint_id.is_none() {
        return Err(CanonicalSyncSessionV2Error::InvalidPredecessorPresence);
    }
    validate_keys("received_event_keys", &content.received_event_keys)?;
    validate_keys("sent_event_keys", &content.sent_event_keys)?;
    if content.next_action.is_terminal()
        && (content.paused || content.local_more_available || content.peer_more_available)
    {
        return Err(CanonicalSyncSessionV2Error::InvalidTerminalState);
    }
    if content.next_action == CanonicalSyncNextActionV2::ReceiveComplete
        && !content.local_more_available
        && !content.peer_more_available
    {
        return Err(CanonicalSyncSessionV2Error::AmbiguousReceiveComplete);
    }
    Ok(())
}

fn validate_pause_transition(
    previous: &CanonicalSyncSessionCheckpointContentV2,
    successor: &CanonicalSyncSessionCheckpointContentV2,
) -> Result<(), CanonicalSyncSessionV2Error> {
    if previous.next_action.is_terminal()
        || successor.next_action.is_terminal()
        || previous.paused == successor.paused
        || previous.sync_round != successor.sync_round
        || previous.next_action != successor.next_action
        || previous.received_event_keys != successor.received_event_keys
        || previous.sent_event_keys != successor.sent_event_keys
        || previous.local_more_available != successor.local_more_available
        || previous.peer_more_available != successor.peer_more_available
    {
        Err(CanonicalSyncSessionV2Error::InvalidPauseTransition)
    } else {
        Ok(())
    }
}

fn require_monotonic_evidence(
    field: &'static str,
    previous: &[CanonicalSyncEventKeyV2],
    successor: &[CanonicalSyncEventKeyV2],
) -> Result<(), CanonicalSyncSessionV2Error> {
    let mut successor_index = 0;
    for previous_key in previous {
        while successor_index < successor.len() && successor[successor_index] < *previous_key {
            successor_index += 1;
        }
        if successor.get(successor_index) != Some(previous_key) {
            return Err(CanonicalSyncSessionV2Error::EventEvidenceRollback { field });
        }
        successor_index += 1;
    }
    Ok(())
}

fn validate_keys(
    field: &'static str,
    keys: &[CanonicalSyncEventKeyV2],
) -> Result<(), CanonicalSyncSessionV2Error> {
    if keys.len() > MAX_CANONICAL_SYNC_SESSION_EVENT_KEYS_V2 {
        return Err(CanonicalSyncSessionV2Error::TooManyEventKeys {
            field,
            actual: keys.len(),
            maximum: MAX_CANONICAL_SYNC_SESSION_EVENT_KEYS_V2,
        });
    }
    if keys.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(CanonicalSyncSessionV2Error::NonCanonicalEventKeyOrder { field });
    }
    Ok(())
}

fn decode_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalSyncSessionCheckpointContentV2, CanonicalSyncSessionV2Error> {
    decoder.expect_array("sync-session-checkpoint-content-v2", 15)?;
    decoder.expect_version("sync-session-checkpoint-content-v2.schema_version")?;
    let protocol_identifier =
        decoder.fixed_bytes("sync-session-checkpoint-content-v2.protocol_identifier")?;
    let conversation_id =
        decoder.fixed_bytes("sync-session-checkpoint-content-v2.conversation_id")?;
    let local_device_id =
        decoder.fixed_bytes("sync-session-checkpoint-content-v2.local_device_id")?;
    let peer_device_id =
        decoder.fixed_bytes("sync-session-checkpoint-content-v2.peer_device_id")?;
    let session_binding =
        decoder.fixed_bytes("sync-session-checkpoint-content-v2.session_binding")?;
    let checkpoint_sequence =
        decoder.unsigned("sync-session-checkpoint-content-v2.checkpoint_sequence")?;
    let sync_round = decoder.unsigned("sync-session-checkpoint-content-v2.sync_round")?;
    let predecessor_checkpoint_id = decode_optional_id(
        decoder,
        "sync-session-checkpoint-content-v2.predecessor_checkpoint_id",
    )?;
    let received_event_keys = decode_keys(
        decoder,
        "sync-session-checkpoint-content-v2.received_event_keys",
    )?;
    let sent_event_keys = decode_keys(
        decoder,
        "sync-session-checkpoint-content-v2.sent_event_keys",
    )?;
    let local_more_available = decode_bool(
        decoder,
        "sync-session-checkpoint-content-v2.local_more_available",
    )?;
    let peer_more_available = decode_bool(
        decoder,
        "sync-session-checkpoint-content-v2.peer_more_available",
    )?;
    let next_action = CanonicalSyncNextActionV2::from_code(
        decoder.unsigned("sync-session-checkpoint-content-v2.next_action")?,
    )?;
    let paused = decode_bool(decoder, "sync-session-checkpoint-content-v2.paused")?;
    let content = CanonicalSyncSessionCheckpointContentV2 {
        protocol_identifier,
        conversation_id,
        local_device_id,
        peer_device_id,
        session_binding,
        checkpoint_sequence,
        sync_round,
        predecessor_checkpoint_id,
        received_event_keys,
        sent_event_keys,
        local_more_available,
        peer_more_available,
        next_action,
        paused,
    };
    validate_content(&content)?;
    Ok(content)
}

fn encode_optional_id(output: &mut Vec<u8>, id: Option<[u8; 32]>) {
    match id {
        None => push_array_header(output, 0),
        Some(id) => {
            push_array_header(output, 1);
            push_bytes(output, &id);
        }
    }
}

fn decode_optional_id(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
) -> Result<Option<[u8; 32]>, CanonicalSyncSessionV2Error> {
    match decoder.array(field)? {
        0 => Ok(None),
        1 => Ok(Some(decoder.fixed_bytes(field)?)),
        actual => Err(CanonicalAccountError::WrongArrayLength {
            object: field,
            expected: 1,
            actual,
        }
        .into()),
    }
}

fn encode_keys(
    output: &mut Vec<u8>,
    keys: &[CanonicalSyncEventKeyV2],
) -> Result<(), CanonicalSyncSessionV2Error> {
    push_array_header(output, keys.len() as u64);
    for key in keys {
        push_bytes(output, &key.encode()?);
    }
    Ok(())
}

fn decode_keys(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
) -> Result<Vec<CanonicalSyncEventKeyV2>, CanonicalSyncSessionV2Error> {
    let count = decoder.array(field)?;
    if count > MAX_CANONICAL_SYNC_SESSION_EVENT_KEYS_V2 {
        return Err(CanonicalSyncSessionV2Error::TooManyEventKeys {
            field,
            actual: count,
            maximum: MAX_CANONICAL_SYNC_SESSION_EVENT_KEYS_V2,
        });
    }
    let mut keys = Vec::with_capacity(count);
    for _ in 0..count {
        keys.push(CanonicalSyncEventKeyV2::decode(decoder.bytes(field)?)?);
    }
    validate_keys(field, &keys)?;
    Ok(keys)
}

fn decode_bool(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
) -> Result<bool, CanonicalSyncSessionV2Error> {
    match decoder.unsigned(field)? {
        0 => Ok(false),
        1 => Ok(true),
        actual => Err(CanonicalSyncSessionV2Error::InvalidBoolean { field, actual }),
    }
}

fn validate_input(bytes: &[u8]) -> Result<(), CanonicalSyncSessionV2Error> {
    ensure_bound(bytes)?;
    validate_canonical_profile_v1(bytes)?;
    Ok(())
}

fn ensure_bound(bytes: &[u8]) -> Result<(), CanonicalSyncSessionV2Error> {
    if bytes.len() > MAX_CANONICAL_SYNC_SESSION_CHECKPOINT_BYTES_V2 {
        Err(CanonicalSyncSessionV2Error::CheckpointTooLarge)
    } else {
        Ok(())
    }
}

fn bounded(bytes: Vec<u8>) -> Result<Vec<u8>, CanonicalSyncSessionV2Error> {
    ensure_bound(&bytes)?;
    Ok(bytes)
}

fn verify_ed25519(
    key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
    object: &'static str,
) -> Result<(), CanonicalSyncSessionV2Error> {
    let key = VerifyingKey::from_bytes(key)
        .map_err(|_| CanonicalSyncSessionV2Error::InvalidPublicKey { object })?;
    let signature = Signature::try_from(signature.as_slice())
        .map_err(|_| CanonicalSyncSessionV2Error::InvalidSignature { object })?;
    key.verify_strict(message, &signature)
        .map_err(|_| CanonicalSyncSessionV2Error::InvalidSignature { object })
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::Value;

    use super::*;
    use crate::canonical_sync_history_v2::CanonicalSyncEventNamespaceV2;

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

    fn fixed_hex<const N: usize>(value: &str) -> Result<[u8; N], Box<dyn std::error::Error>> {
        decode_hex(value)?
            .try_into()
            .map_err(|_| invalid_data(format!("expected {N} bytes")))
    }

    fn vectors() -> Result<Value, Box<dyn std::error::Error>> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-SYNC-SESSION-V2-VECTORS.json"
        )))?)
    }

    fn vector_checkpoints(
        vectors: &Value,
    ) -> Result<Vec<CanonicalSignedSyncSessionCheckpointV2>, Box<dyn std::error::Error>> {
        vectors["expected"]["checkpoints"]
            .as_array()
            .ok_or_else(|| invalid_data("missing checkpoint vectors"))?
            .iter()
            .map(|value| {
                let encoded = decode_hex(
                    value["signed_cbor_hex"]
                        .as_str()
                        .ok_or_else(|| invalid_data("missing signed checkpoint"))?,
                )?;
                Ok(decode_and_verify_signed_sync_session_checkpoint_v2(
                    &encoded,
                )?)
            })
            .collect()
    }

    fn sign_checkpoint(
        content: CanonicalSyncSessionCheckpointContentV2,
        key: &SigningKey,
    ) -> Result<CanonicalSignedSyncSessionCheckpointV2, CanonicalSyncSessionV2Error> {
        let signature = key
            .sign(&sync_session_checkpoint_signing_bytes_v2(&content)?)
            .to_bytes();
        Ok(CanonicalSignedSyncSessionCheckpointV2 { content, signature })
    }

    #[test]
    fn golden_checkpoint_chain_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let vectors = vectors()?;
        assert_eq!(vectors["milestone"], "M1.1.55");
        assert_eq!(
            vectors["expected"]["protocol_identifier_utf8"],
            "kilogram/m1/sync/cbor/2"
        );
        let checkpoints = vector_checkpoints(&vectors)?;
        assert_eq!(checkpoints.len(), 4);

        for (index, checkpoint) in checkpoints.iter().enumerate() {
            let expected = &vectors["expected"]["checkpoints"][index];
            assert_eq!(
                encode_sync_session_checkpoint_content_v2(&checkpoint.content)?,
                decode_hex(
                    expected["content_cbor_hex"]
                        .as_str()
                        .ok_or_else(|| invalid_data("missing content checkpoint"))?,
                )?
            );
            assert_eq!(
                encode_signed_sync_session_checkpoint_v2(checkpoint)?,
                decode_hex(
                    expected["signed_cbor_hex"]
                        .as_str()
                        .ok_or_else(|| invalid_data("missing signed checkpoint"))?,
                )?
            );
            assert_eq!(
                sync_session_checkpoint_id_v2(checkpoint)?,
                fixed_hex(
                    expected["checkpoint_id_hex"]
                        .as_str()
                        .ok_or_else(|| invalid_data("missing checkpoint ID"))?,
                )?
            );
        }
        for pair in checkpoints.windows(2) {
            verify_sync_session_checkpoint_successor_v2(&pair[0], &pair[1])?;
        }
        Ok(())
    }

    #[test]
    fn signatures_chain_and_cumulative_evidence_fail_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let vectors = vectors()?;
        let checkpoints = vector_checkpoints(&vectors)?;
        let key = SigningKey::from_bytes(&fixed_hex(
            vectors["input"]["local_device_secret_key_hex"]
                .as_str()
                .ok_or_else(|| invalid_data("missing local Device key"))?,
        )?);

        let mut invalid_signature = checkpoints[0].clone();
        invalid_signature.signature[0] ^= 1;
        assert!(matches!(
            verify_signed_sync_session_checkpoint_v2(&invalid_signature),
            Err(CanonicalSyncSessionV2Error::InvalidSignature { .. })
        ));

        let mut wrong_predecessor = checkpoints[1].content.clone();
        wrong_predecessor.predecessor_checkpoint_id = Some([0x55; 32]);
        let wrong_predecessor = sign_checkpoint(wrong_predecessor, &key)?;
        assert!(matches!(
            verify_sync_session_checkpoint_successor_v2(&checkpoints[0], &wrong_predecessor),
            Err(CanonicalSyncSessionV2Error::PredecessorMismatch)
        ));

        let mut rollback = checkpoints[1].content.clone();
        rollback.received_event_keys.clear();
        let rollback = sign_checkpoint(rollback, &key)?;
        assert!(matches!(
            verify_sync_session_checkpoint_successor_v2(&checkpoints[0], &rollback),
            Err(CanonicalSyncSessionV2Error::EventEvidenceRollback {
                field: "received_event_keys"
            })
        ));

        let mut changed_session = checkpoints[1].content.clone();
        changed_session.session_binding[0] ^= 1;
        let changed_session = sign_checkpoint(changed_session, &key)?;
        assert!(matches!(
            verify_sync_session_checkpoint_successor_v2(&checkpoints[0], &changed_session),
            Err(CanonicalSyncSessionV2Error::SessionIdentityChanged)
        ));

        let mut wrong_protocol = checkpoints[0].content.clone();
        wrong_protocol.protocol_identifier[22] = b'1';
        assert!(matches!(
            encode_sync_session_checkpoint_content_v2(&wrong_protocol),
            Err(CanonicalSyncSessionV2Error::WrongProtocolIdentifier)
        ));
        Ok(())
    }

    #[test]
    fn typed_evidence_preserves_namespaces_and_order() -> Result<(), Box<dyn std::error::Error>> {
        let vectors = vectors()?;
        let checkpoints = vector_checkpoints(&vectors)?;
        let key = SigningKey::from_bytes(&fixed_hex(
            vectors["input"]["local_device_secret_key_hex"]
                .as_str()
                .ok_or_else(|| invalid_data("missing local Device key"))?,
        )?);
        let raw_id = [0x77; 32];
        let legacy =
            CanonicalSyncEventKeyV2::new(CanonicalSyncEventNamespaceV2::LegacyPostcardV6, raw_id);
        let canonical =
            CanonicalSyncEventKeyV2::new(CanonicalSyncEventNamespaceV2::CanonicalCborV1, raw_id);
        assert_ne!(legacy, canonical);
        assert!(legacy < canonical);

        let mut ordered = checkpoints[0].content.clone();
        ordered.received_event_keys = vec![legacy, canonical];
        let ordered = sign_checkpoint(ordered, &key)?;
        assert_eq!(
            decode_and_verify_signed_sync_session_checkpoint_v2(
                &encode_signed_sync_session_checkpoint_v2(&ordered)?
            )?,
            ordered
        );

        let mut reversed = checkpoints[0].content.clone();
        reversed.received_event_keys = vec![canonical, legacy];
        assert!(matches!(
            encode_sync_session_checkpoint_content_v2(&reversed),
            Err(CanonicalSyncSessionV2Error::NonCanonicalEventKeyOrder {
                field: "received_event_keys"
            })
        ));
        Ok(())
    }

    #[test]
    fn round_pause_and_terminal_transitions_fail_closed() -> Result<(), Box<dyn std::error::Error>>
    {
        let vectors = vectors()?;
        let checkpoints = vector_checkpoints(&vectors)?;
        let key = SigningKey::from_bytes(&fixed_hex(
            vectors["input"]["local_device_secret_key_hex"]
                .as_str()
                .ok_or_else(|| invalid_data("missing local Device key"))?,
        )?);

        let mut invalid_round_action = checkpoints[1].content.clone();
        invalid_round_action.next_action = CanonicalSyncNextActionV2::ReceiveDiff;
        let invalid_round_action = sign_checkpoint(invalid_round_action, &key)?;
        assert!(matches!(
            verify_sync_session_checkpoint_successor_v2(&checkpoints[0], &invalid_round_action),
            Err(CanonicalSyncSessionV2Error::InvalidActionTransition)
        ));

        let mut changed_while_pausing = checkpoints[2].content.clone();
        changed_while_pausing.peer_more_available = false;
        let changed_while_pausing = sign_checkpoint(changed_while_pausing, &key)?;
        assert!(matches!(
            verify_sync_session_checkpoint_successor_v2(&checkpoints[1], &changed_while_pausing),
            Err(CanonicalSyncSessionV2Error::InvalidPauseTransition)
        ));

        let mut terminal_content = checkpoints[0].content.clone();
        terminal_content.checkpoint_sequence = 2;
        terminal_content.predecessor_checkpoint_id =
            Some(sync_session_checkpoint_id_v2(&checkpoints[0])?);
        terminal_content.local_more_available = false;
        terminal_content.peer_more_available = false;
        terminal_content.next_action = CanonicalSyncNextActionV2::Complete;
        let terminal = sign_checkpoint(terminal_content, &key)?;
        verify_sync_session_checkpoint_successor_v2(&checkpoints[0], &terminal)?;

        let mut after_terminal_content = terminal.content.clone();
        after_terminal_content.checkpoint_sequence = 3;
        after_terminal_content.predecessor_checkpoint_id =
            Some(sync_session_checkpoint_id_v2(&terminal)?);
        after_terminal_content.next_action = CanonicalSyncNextActionV2::Rejected;
        let after_terminal = sign_checkpoint(after_terminal_content, &key)?;
        assert!(matches!(
            verify_sync_session_checkpoint_successor_v2(&terminal, &after_terminal),
            Err(CanonicalSyncSessionV2Error::TerminalCheckpointHasSuccessor)
        ));

        let mut ambiguous = checkpoints[0].content.clone();
        ambiguous.local_more_available = false;
        ambiguous.peer_more_available = false;
        assert!(matches!(
            encode_sync_session_checkpoint_content_v2(&ambiguous),
            Err(CanonicalSyncSessionV2Error::AmbiguousReceiveComplete)
        ));
        Ok(())
    }
}
