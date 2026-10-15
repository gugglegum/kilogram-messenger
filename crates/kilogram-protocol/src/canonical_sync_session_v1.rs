//! Inactive deterministic-CBOR durable synchronization session candidate.
//!
//! Checkpoints form a Device-signed hash-linked chain. They make a future
//! multi-round sync implementation resumable without trusting ambient process
//! memory, while leaving the active Postcard v6 runtime unchanged.

use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

use crate::{
    canonical_account_v1::{AccountDecoder, CanonicalAccountError, signed_envelope},
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
        push_unsigned, validate_canonical_profile_v1,
    },
};

pub const CANONICAL_SYNC_SESSION_CHECKPOINT_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:sync-session-checkpoint-signature:cbor-v1\0";
pub const CANONICAL_SYNC_SESSION_CHECKPOINT_ID_DOMAIN: &[u8] =
    b"kilogram:sync-session-checkpoint-id:cbor-v1\0";
pub const MAX_CANONICAL_SYNC_SESSION_EVENT_IDS: usize = 4096;
pub const MAX_CANONICAL_SYNC_SESSION_CHECKPOINT_BYTES: usize = 512 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum CanonicalSyncNextActionV1 {
    SendInventory = 1,
    ReceiveDiff = 2,
    SendEventBatch = 3,
    ReceiveComplete = 4,
    Complete = 5,
    Rejected = 6,
}

impl CanonicalSyncNextActionV1 {
    const fn code(self) -> u64 {
        self as u64
    }

    fn from_code(code: u64) -> Result<Self, CanonicalSyncSessionError> {
        match code {
            1 => Ok(Self::SendInventory),
            2 => Ok(Self::ReceiveDiff),
            3 => Ok(Self::SendEventBatch),
            4 => Ok(Self::ReceiveComplete),
            5 => Ok(Self::Complete),
            6 => Ok(Self::Rejected),
            other => Err(CanonicalSyncSessionError::UnknownNextAction(other)),
        }
    }

    const fn is_terminal(self) -> bool {
        matches!(self, Self::Complete | Self::Rejected)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncSessionCheckpointContentV1 {
    pub conversation_id: [u8; 32],
    pub local_device_id: [u8; 32],
    pub peer_device_id: [u8; 32],
    pub session_binding: [u8; 32],
    pub checkpoint_sequence: u64,
    pub sync_round: u64,
    pub predecessor_checkpoint_id: Option<[u8; 32]>,
    pub received_event_ids: Vec<[u8; 32]>,
    pub sent_event_ids: Vec<[u8; 32]>,
    pub local_more_available: bool,
    pub peer_more_available: bool,
    pub next_action: CanonicalSyncNextActionV1,
    pub paused: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedSyncSessionCheckpointV1 {
    pub content: CanonicalSyncSessionCheckpointContentV1,
    pub signature: [u8; 64],
}

#[derive(Debug, Error)]
pub enum CanonicalSyncSessionError {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),

    #[error(transparent)]
    Account(#[from] CanonicalAccountError),

    #[error(
        "canonical sync session checkpoint exceeds {MAX_CANONICAL_SYNC_SESSION_CHECKPOINT_BYTES} bytes"
    )]
    CheckpointTooLarge,

    #[error("local and peer Device IDs must differ")]
    SameDevice,

    #[error("checkpoint sequence must start at one")]
    InvalidCheckpointSequence,

    #[error("sync round must be positive and no greater than checkpoint sequence")]
    InvalidSyncRound,

    #[error("checkpoint predecessor presence does not match its sequence")]
    InvalidPredecessorPresence,

    #[error("{field} contains {actual} items; maximum is {maximum}")]
    TooManyEventIds {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },

    #[error("{field} must be strictly ordered by Event ID")]
    NonCanonicalEventIdOrder { field: &'static str },

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

pub fn encode_sync_session_checkpoint_content_v1(
    content: &CanonicalSyncSessionCheckpointContentV1,
) -> Result<Vec<u8>, CanonicalSyncSessionError> {
    validate_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 14);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.conversation_id);
    push_bytes(&mut output, &content.local_device_id);
    push_bytes(&mut output, &content.peer_device_id);
    push_bytes(&mut output, &content.session_binding);
    push_unsigned(&mut output, content.checkpoint_sequence);
    push_unsigned(&mut output, content.sync_round);
    encode_optional_id(&mut output, content.predecessor_checkpoint_id);
    encode_ids(&mut output, &content.received_event_ids);
    encode_ids(&mut output, &content.sent_event_ids);
    push_unsigned(&mut output, u64::from(content.local_more_available));
    push_unsigned(&mut output, u64::from(content.peer_more_available));
    push_unsigned(&mut output, content.next_action.code());
    push_unsigned(&mut output, u64::from(content.paused));
    bounded(output)
}

pub fn sync_session_checkpoint_signing_bytes_v1(
    content: &CanonicalSyncSessionCheckpointContentV1,
) -> Result<Vec<u8>, CanonicalSyncSessionError> {
    Ok(prefixed(
        CANONICAL_SYNC_SESSION_CHECKPOINT_SIGNATURE_DOMAIN,
        &encode_sync_session_checkpoint_content_v1(content)?,
    ))
}

pub fn encode_signed_sync_session_checkpoint_v1(
    checkpoint: &CanonicalSignedSyncSessionCheckpointV1,
) -> Result<Vec<u8>, CanonicalSyncSessionError> {
    verify_signed_sync_session_checkpoint_v1(checkpoint)?;
    let content = encode_sync_session_checkpoint_content_v1(&checkpoint.content)?;
    bounded(signed_envelope(&content, &checkpoint.signature))
}

pub fn decode_signed_sync_session_checkpoint_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedSyncSessionCheckpointV1, CanonicalSyncSessionError> {
    validate_input(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-sync-session-checkpoint-v1", 3)?;
    decoder.expect_version("signed-sync-session-checkpoint-v1.schema_version")?;
    let content = decode_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-sync-session-checkpoint-v1.signature")?;
    decoder.finish()?;
    Ok(CanonicalSignedSyncSessionCheckpointV1 { content, signature })
}

pub fn decode_and_verify_signed_sync_session_checkpoint_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedSyncSessionCheckpointV1, CanonicalSyncSessionError> {
    let checkpoint = decode_signed_sync_session_checkpoint_v1(bytes)?;
    verify_signed_sync_session_checkpoint_v1(&checkpoint)?;
    Ok(checkpoint)
}

pub fn verify_signed_sync_session_checkpoint_v1(
    checkpoint: &CanonicalSignedSyncSessionCheckpointV1,
) -> Result<(), CanonicalSyncSessionError> {
    validate_content(&checkpoint.content)?;
    verify_ed25519(
        &checkpoint.content.local_device_id,
        &sync_session_checkpoint_signing_bytes_v1(&checkpoint.content)?,
        &checkpoint.signature,
        "signed-sync-session-checkpoint-v1",
    )
}

pub fn sync_session_checkpoint_id_v1(
    checkpoint: &CanonicalSignedSyncSessionCheckpointV1,
) -> Result<[u8; 32], CanonicalSyncSessionError> {
    let encoded = encode_signed_sync_session_checkpoint_v1(checkpoint)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(CANONICAL_SYNC_SESSION_CHECKPOINT_ID_DOMAIN);
    hasher.update(&encoded);
    Ok(*hasher.finalize().as_bytes())
}

pub fn verify_sync_session_checkpoint_successor_v1(
    previous: &CanonicalSignedSyncSessionCheckpointV1,
    successor: &CanonicalSignedSyncSessionCheckpointV1,
) -> Result<(), CanonicalSyncSessionError> {
    verify_signed_sync_session_checkpoint_v1(previous)?;
    verify_signed_sync_session_checkpoint_v1(successor)?;
    let previous_content = &previous.content;
    let successor_content = &successor.content;
    if previous_content.next_action.is_terminal() {
        return Err(CanonicalSyncSessionError::TerminalCheckpointHasSuccessor);
    }
    if previous_content.conversation_id != successor_content.conversation_id
        || previous_content.local_device_id != successor_content.local_device_id
        || previous_content.peer_device_id != successor_content.peer_device_id
        || previous_content.session_binding != successor_content.session_binding
    {
        return Err(CanonicalSyncSessionError::SessionIdentityChanged);
    }
    if previous_content.checkpoint_sequence.checked_add(1)
        != Some(successor_content.checkpoint_sequence)
    {
        return Err(CanonicalSyncSessionError::CheckpointSequenceDiscontinuity);
    }
    if successor_content.predecessor_checkpoint_id != Some(sync_session_checkpoint_id_v1(previous)?)
    {
        return Err(CanonicalSyncSessionError::PredecessorMismatch);
    }
    if successor_content.sync_round < previous_content.sync_round
        || successor_content.sync_round > previous_content.sync_round.saturating_add(1)
    {
        return Err(CanonicalSyncSessionError::SyncRoundDiscontinuity);
    }
    require_monotonic_evidence(
        "received_event_ids",
        &previous_content.received_event_ids,
        &successor_content.received_event_ids,
    )?;
    require_monotonic_evidence(
        "sent_event_ids",
        &previous_content.sent_event_ids,
        &successor_content.sent_event_ids,
    )?;

    if previous_content.paused || successor_content.paused {
        validate_pause_transition(previous_content, successor_content)?;
        return Ok(());
    }

    if previous_content.sync_round.checked_add(1) == Some(successor_content.sync_round) {
        if previous_content.next_action != CanonicalSyncNextActionV1::ReceiveComplete
            || successor_content.next_action != CanonicalSyncNextActionV1::SendInventory
            || (!previous_content.local_more_available && !previous_content.peer_more_available)
            || previous_content.received_event_ids != successor_content.received_event_ids
            || previous_content.sent_event_ids != successor_content.sent_event_ids
            || previous_content.local_more_available != successor_content.local_more_available
            || previous_content.peer_more_available != successor_content.peer_more_available
        {
            return Err(CanonicalSyncSessionError::InvalidActionTransition);
        }
        return Ok(());
    }

    if successor_content.next_action == CanonicalSyncNextActionV1::Rejected {
        return Ok(());
    }
    let valid = matches!(
        (previous_content.next_action, successor_content.next_action),
        (
            CanonicalSyncNextActionV1::SendInventory,
            CanonicalSyncNextActionV1::ReceiveDiff
        ) | (
            CanonicalSyncNextActionV1::ReceiveDiff,
            CanonicalSyncNextActionV1::SendEventBatch
        ) | (
            CanonicalSyncNextActionV1::ReceiveDiff,
            CanonicalSyncNextActionV1::ReceiveComplete
        ) | (
            CanonicalSyncNextActionV1::SendEventBatch,
            CanonicalSyncNextActionV1::ReceiveComplete
        ) | (
            CanonicalSyncNextActionV1::ReceiveComplete,
            CanonicalSyncNextActionV1::Complete
        )
    );
    if valid {
        Ok(())
    } else {
        Err(CanonicalSyncSessionError::InvalidActionTransition)
    }
}

fn validate_content(
    content: &CanonicalSyncSessionCheckpointContentV1,
) -> Result<(), CanonicalSyncSessionError> {
    if content.local_device_id == content.peer_device_id {
        return Err(CanonicalSyncSessionError::SameDevice);
    }
    if content.checkpoint_sequence == 0 {
        return Err(CanonicalSyncSessionError::InvalidCheckpointSequence);
    }
    if content.sync_round == 0 || content.sync_round > content.checkpoint_sequence {
        return Err(CanonicalSyncSessionError::InvalidSyncRound);
    }
    if (content.checkpoint_sequence == 1) != content.predecessor_checkpoint_id.is_none() {
        return Err(CanonicalSyncSessionError::InvalidPredecessorPresence);
    }
    validate_ids("received_event_ids", &content.received_event_ids)?;
    validate_ids("sent_event_ids", &content.sent_event_ids)?;
    if content.next_action.is_terminal()
        && (content.paused || content.local_more_available || content.peer_more_available)
    {
        return Err(CanonicalSyncSessionError::InvalidTerminalState);
    }
    if content.next_action == CanonicalSyncNextActionV1::ReceiveComplete
        && !content.local_more_available
        && !content.peer_more_available
    {
        return Err(CanonicalSyncSessionError::AmbiguousReceiveComplete);
    }
    Ok(())
}

fn validate_pause_transition(
    previous: &CanonicalSyncSessionCheckpointContentV1,
    successor: &CanonicalSyncSessionCheckpointContentV1,
) -> Result<(), CanonicalSyncSessionError> {
    if previous.next_action.is_terminal()
        || successor.next_action.is_terminal()
        || previous.paused == successor.paused
        || previous.sync_round != successor.sync_round
        || previous.next_action != successor.next_action
        || previous.received_event_ids != successor.received_event_ids
        || previous.sent_event_ids != successor.sent_event_ids
        || previous.local_more_available != successor.local_more_available
        || previous.peer_more_available != successor.peer_more_available
    {
        Err(CanonicalSyncSessionError::InvalidPauseTransition)
    } else {
        Ok(())
    }
}

fn require_monotonic_evidence(
    field: &'static str,
    previous: &[[u8; 32]],
    successor: &[[u8; 32]],
) -> Result<(), CanonicalSyncSessionError> {
    let mut successor_index = 0;
    for previous_id in previous {
        while successor_index < successor.len() && successor[successor_index] < *previous_id {
            successor_index += 1;
        }
        if successor.get(successor_index) != Some(previous_id) {
            return Err(CanonicalSyncSessionError::EventEvidenceRollback { field });
        }
        successor_index += 1;
    }
    Ok(())
}

fn validate_ids(field: &'static str, ids: &[[u8; 32]]) -> Result<(), CanonicalSyncSessionError> {
    if ids.len() > MAX_CANONICAL_SYNC_SESSION_EVENT_IDS {
        return Err(CanonicalSyncSessionError::TooManyEventIds {
            field,
            actual: ids.len(),
            maximum: MAX_CANONICAL_SYNC_SESSION_EVENT_IDS,
        });
    }
    if ids.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(CanonicalSyncSessionError::NonCanonicalEventIdOrder { field });
    }
    Ok(())
}

fn decode_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalSyncSessionCheckpointContentV1, CanonicalSyncSessionError> {
    decoder.expect_array("sync-session-checkpoint-content-v1", 14)?;
    decoder.expect_version("sync-session-checkpoint-content-v1.schema_version")?;
    let conversation_id =
        decoder.fixed_bytes("sync-session-checkpoint-content-v1.conversation_id")?;
    let local_device_id =
        decoder.fixed_bytes("sync-session-checkpoint-content-v1.local_device_id")?;
    let peer_device_id =
        decoder.fixed_bytes("sync-session-checkpoint-content-v1.peer_device_id")?;
    let session_binding =
        decoder.fixed_bytes("sync-session-checkpoint-content-v1.session_binding")?;
    let checkpoint_sequence =
        decoder.unsigned("sync-session-checkpoint-content-v1.checkpoint_sequence")?;
    let sync_round = decoder.unsigned("sync-session-checkpoint-content-v1.sync_round")?;
    let predecessor_checkpoint_id = decode_optional_id(
        decoder,
        "sync-session-checkpoint-content-v1.predecessor_checkpoint_id",
    )?;
    let received_event_ids = decode_ids(
        decoder,
        "sync-session-checkpoint-content-v1.received_event_ids",
    )?;
    let sent_event_ids = decode_ids(decoder, "sync-session-checkpoint-content-v1.sent_event_ids")?;
    let local_more_available = decode_bool(
        decoder,
        "sync-session-checkpoint-content-v1.local_more_available",
    )?;
    let peer_more_available = decode_bool(
        decoder,
        "sync-session-checkpoint-content-v1.peer_more_available",
    )?;
    let next_action = CanonicalSyncNextActionV1::from_code(
        decoder.unsigned("sync-session-checkpoint-content-v1.next_action")?,
    )?;
    let paused = decode_bool(decoder, "sync-session-checkpoint-content-v1.paused")?;
    let content = CanonicalSyncSessionCheckpointContentV1 {
        conversation_id,
        local_device_id,
        peer_device_id,
        session_binding,
        checkpoint_sequence,
        sync_round,
        predecessor_checkpoint_id,
        received_event_ids,
        sent_event_ids,
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
) -> Result<Option<[u8; 32]>, CanonicalSyncSessionError> {
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

fn encode_ids(output: &mut Vec<u8>, ids: &[[u8; 32]]) {
    push_array_header(output, ids.len() as u64);
    for id in ids {
        push_bytes(output, id);
    }
}

fn decode_ids(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
) -> Result<Vec<[u8; 32]>, CanonicalSyncSessionError> {
    let count = decoder.array(field)?;
    if count > MAX_CANONICAL_SYNC_SESSION_EVENT_IDS {
        return Err(CanonicalSyncSessionError::TooManyEventIds {
            field,
            actual: count,
            maximum: MAX_CANONICAL_SYNC_SESSION_EVENT_IDS,
        });
    }
    let mut ids = Vec::with_capacity(count);
    for _ in 0..count {
        ids.push(decoder.fixed_bytes(field)?);
    }
    validate_ids(field, &ids)?;
    Ok(ids)
}

fn decode_bool(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
) -> Result<bool, CanonicalSyncSessionError> {
    match decoder.unsigned(field)? {
        0 => Ok(false),
        1 => Ok(true),
        actual => Err(CanonicalSyncSessionError::InvalidBoolean { field, actual }),
    }
}

fn validate_input(bytes: &[u8]) -> Result<(), CanonicalSyncSessionError> {
    ensure_bound(bytes)?;
    validate_canonical_profile_v1(bytes)?;
    Ok(())
}

fn ensure_bound(bytes: &[u8]) -> Result<(), CanonicalSyncSessionError> {
    if bytes.len() > MAX_CANONICAL_SYNC_SESSION_CHECKPOINT_BYTES {
        Err(CanonicalSyncSessionError::CheckpointTooLarge)
    } else {
        Ok(())
    }
}

fn bounded(bytes: Vec<u8>) -> Result<Vec<u8>, CanonicalSyncSessionError> {
    ensure_bound(&bytes)?;
    Ok(bytes)
}

fn verify_ed25519(
    key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
    object: &'static str,
) -> Result<(), CanonicalSyncSessionError> {
    let key = VerifyingKey::from_bytes(key)
        .map_err(|_| CanonicalSyncSessionError::InvalidPublicKey { object })?;
    let signature = Signature::try_from(signature.as_slice())
        .map_err(|_| CanonicalSyncSessionError::InvalidSignature { object })?;
    key.verify_strict(message, &signature)
        .map_err(|_| CanonicalSyncSessionError::InvalidSignature { object })
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};
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

    fn fixed_hex<const N: usize>(value: &str) -> Result<[u8; N], Box<dyn std::error::Error>> {
        decode_hex(value)?
            .try_into()
            .map_err(|_| invalid_data(format!("expected {N} bytes")))
    }

    fn vectors() -> Result<Value, Box<dyn std::error::Error>> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-SYNC-SESSION-VECTORS.json"
        )))?)
    }

    fn vector_checkpoints(
        vectors: &Value,
    ) -> Result<Vec<CanonicalSignedSyncSessionCheckpointV1>, Box<dyn std::error::Error>> {
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
                Ok(decode_and_verify_signed_sync_session_checkpoint_v1(
                    &encoded,
                )?)
            })
            .collect()
    }

    fn sign_checkpoint(
        content: CanonicalSyncSessionCheckpointContentV1,
        key: &SigningKey,
    ) -> Result<CanonicalSignedSyncSessionCheckpointV1, CanonicalSyncSessionError> {
        let signature = key
            .sign(&sync_session_checkpoint_signing_bytes_v1(&content)?)
            .to_bytes();
        Ok(CanonicalSignedSyncSessionCheckpointV1 { content, signature })
    }

    #[test]
    fn golden_checkpoint_chain_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let vectors = vectors()?;
        let checkpoints = vector_checkpoints(&vectors)?;
        assert_eq!(checkpoints.len(), 4);

        for (index, checkpoint) in checkpoints.iter().enumerate() {
            let expected = &vectors["expected"]["checkpoints"][index];
            assert_eq!(
                encode_sync_session_checkpoint_content_v1(&checkpoint.content)?,
                decode_hex(
                    expected["content_cbor_hex"]
                        .as_str()
                        .ok_or_else(|| invalid_data("missing content checkpoint"))?,
                )?
            );
            assert_eq!(
                encode_signed_sync_session_checkpoint_v1(checkpoint)?,
                decode_hex(
                    expected["signed_cbor_hex"]
                        .as_str()
                        .ok_or_else(|| invalid_data("missing signed checkpoint"))?,
                )?
            );
            assert_eq!(
                sync_session_checkpoint_id_v1(checkpoint)?,
                fixed_hex(
                    expected["checkpoint_id_hex"]
                        .as_str()
                        .ok_or_else(|| invalid_data("missing checkpoint ID"))?,
                )?
            );
        }
        for pair in checkpoints.windows(2) {
            verify_sync_session_checkpoint_successor_v1(&pair[0], &pair[1])?;
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
            verify_signed_sync_session_checkpoint_v1(&invalid_signature),
            Err(CanonicalSyncSessionError::InvalidSignature { .. })
        ));

        let mut wrong_predecessor = checkpoints[1].content.clone();
        wrong_predecessor.predecessor_checkpoint_id = Some([0x55; 32]);
        let wrong_predecessor = sign_checkpoint(wrong_predecessor, &key)?;
        assert!(matches!(
            verify_sync_session_checkpoint_successor_v1(&checkpoints[0], &wrong_predecessor),
            Err(CanonicalSyncSessionError::PredecessorMismatch)
        ));

        let mut rollback = checkpoints[1].content.clone();
        rollback.sent_event_ids.clear();
        let rollback = sign_checkpoint(rollback, &key)?;
        assert!(matches!(
            verify_sync_session_checkpoint_successor_v1(&checkpoints[0], &rollback),
            Err(CanonicalSyncSessionError::EventEvidenceRollback {
                field: "sent_event_ids"
            })
        ));

        let mut changed_session = checkpoints[1].content.clone();
        changed_session.session_binding[0] ^= 1;
        let changed_session = sign_checkpoint(changed_session, &key)?;
        assert!(matches!(
            verify_sync_session_checkpoint_successor_v1(&checkpoints[0], &changed_session),
            Err(CanonicalSyncSessionError::SessionIdentityChanged)
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
        invalid_round_action.next_action = CanonicalSyncNextActionV1::ReceiveDiff;
        let invalid_round_action = sign_checkpoint(invalid_round_action, &key)?;
        assert!(matches!(
            verify_sync_session_checkpoint_successor_v1(&checkpoints[0], &invalid_round_action),
            Err(CanonicalSyncSessionError::InvalidActionTransition)
        ));

        let mut changed_while_pausing = checkpoints[2].content.clone();
        changed_while_pausing.peer_more_available = false;
        let changed_while_pausing = sign_checkpoint(changed_while_pausing, &key)?;
        assert!(matches!(
            verify_sync_session_checkpoint_successor_v1(&checkpoints[1], &changed_while_pausing),
            Err(CanonicalSyncSessionError::InvalidPauseTransition)
        ));

        let mut terminal_content = checkpoints[0].content.clone();
        terminal_content.checkpoint_sequence = 2;
        terminal_content.predecessor_checkpoint_id =
            Some(sync_session_checkpoint_id_v1(&checkpoints[0])?);
        terminal_content.local_more_available = false;
        terminal_content.peer_more_available = false;
        terminal_content.next_action = CanonicalSyncNextActionV1::Complete;
        let terminal = sign_checkpoint(terminal_content, &key)?;
        verify_sync_session_checkpoint_successor_v1(&checkpoints[0], &terminal)?;

        let mut after_terminal_content = terminal.content.clone();
        after_terminal_content.checkpoint_sequence = 3;
        after_terminal_content.predecessor_checkpoint_id =
            Some(sync_session_checkpoint_id_v1(&terminal)?);
        after_terminal_content.next_action = CanonicalSyncNextActionV1::Rejected;
        let after_terminal = sign_checkpoint(after_terminal_content, &key)?;
        assert!(matches!(
            verify_sync_session_checkpoint_successor_v1(&terminal, &after_terminal),
            Err(CanonicalSyncSessionError::TerminalCheckpointHasSuccessor)
        ));

        let mut ambiguous = checkpoints[0].content.clone();
        ambiguous.local_more_available = false;
        ambiguous.peer_more_available = false;
        assert!(matches!(
            encode_sync_session_checkpoint_content_v1(&ambiguous),
            Err(CanonicalSyncSessionError::AmbiguousReceiveComplete)
        ));
        Ok(())
    }
}
