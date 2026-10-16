//! Inactive Device-signed responder checkpoint chain for canonical sync v2.
//!
//! The initiator checkpoint cannot represent responder actions without
//! changing their meaning. This sibling format has separate signature and ID
//! domains and remains local durable state until `/cbor/2` activation.

use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

use crate::{
    canonical_account_v1::{AccountDecoder, CanonicalAccountError, signed_envelope},
    canonical_sync_history_v2::{CanonicalSyncEventKeyV2, CanonicalSyncHistoryV2Error},
    canonical_sync_session_v2::{
        CANONICAL_SYNC_SESSION_PROTOCOL_IDENTIFIER_V2,
        MAX_CANONICAL_SYNC_SESSION_CHECKPOINT_BYTES_V2, MAX_CANONICAL_SYNC_SESSION_EVENT_KEYS_V2,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
        push_unsigned, validate_canonical_profile_v1,
    },
};

pub const CANONICAL_SYNC_RESPONDER_CHECKPOINT_SIGNATURE_DOMAIN_V2: &[u8] =
    b"kilogram:sync-responder-checkpoint-signature:cbor-v2\0";
pub const CANONICAL_SYNC_RESPONDER_CHECKPOINT_ID_DOMAIN_V2: &[u8] =
    b"kilogram:sync-responder-checkpoint-id:cbor-v2\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum CanonicalSyncResponderNextActionV2 {
    ReceiveInventory = 1,
    SendDiff = 2,
    ReceiveEventBatch = 3,
    SendComplete = 4,
    Complete = 5,
    Rejected = 6,
}

impl CanonicalSyncResponderNextActionV2 {
    const fn code(self) -> u64 {
        self as u64
    }

    fn from_code(code: u64) -> Result<Self, CanonicalSyncResponderSessionV2Error> {
        match code {
            1 => Ok(Self::ReceiveInventory),
            2 => Ok(Self::SendDiff),
            3 => Ok(Self::ReceiveEventBatch),
            4 => Ok(Self::SendComplete),
            5 => Ok(Self::Complete),
            6 => Ok(Self::Rejected),
            other => Err(CanonicalSyncResponderSessionV2Error::UnknownNextAction(
                other,
            )),
        }
    }

    const fn is_terminal(self) -> bool {
        matches!(self, Self::Complete | Self::Rejected)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncResponderCheckpointContentV2 {
    pub protocol_identifier: [u8; 23],
    pub conversation_id: [u8; 32],
    pub responder_device_id: [u8; 32],
    pub requester_device_id: [u8; 32],
    pub session_binding: [u8; 32],
    pub checkpoint_sequence: u64,
    pub sync_round: u64,
    pub predecessor_checkpoint_id: Option<[u8; 32]>,
    pub received_event_keys: Vec<CanonicalSyncEventKeyV2>,
    pub sent_event_keys: Vec<CanonicalSyncEventKeyV2>,
    pub continuation_required: bool,
    pub next_action: CanonicalSyncResponderNextActionV2,
    pub paused: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedSyncResponderCheckpointV2 {
    pub content: CanonicalSyncResponderCheckpointContentV2,
    pub signature: [u8; 64],
}

#[derive(Debug, Error)]
pub enum CanonicalSyncResponderSessionV2Error {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),

    #[error(transparent)]
    Account(#[from] CanonicalAccountError),

    #[error(transparent)]
    History(#[from] CanonicalSyncHistoryV2Error),

    #[error("canonical sync responder checkpoint is too large")]
    CheckpointTooLarge,

    #[error("responder checkpoint does not bind canonical sync v2")]
    WrongProtocolIdentifier,

    #[error("responder and requester Device IDs must differ")]
    SameDevice,

    #[error("responder checkpoint sequence must start at one")]
    InvalidCheckpointSequence,

    #[error("responder sync round must be positive and no greater than checkpoint sequence")]
    InvalidSyncRound,

    #[error("responder checkpoint predecessor presence does not match its sequence")]
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

    #[error("unknown canonical sync responder next action {0}")]
    UnknownNextAction(u64),

    #[error("terminal responder checkpoint cannot be paused or require continuation")]
    InvalidTerminalState,

    #[error("responder checkpoint contains an invalid Ed25519 verifying key")]
    InvalidPublicKey,

    #[error("responder checkpoint contains an invalid Ed25519 signature")]
    InvalidSignature,

    #[error("responder checkpoint successor changes immutable session identity")]
    SessionIdentityChanged,

    #[error("responder checkpoint sequence is not exactly previous plus one")]
    CheckpointSequenceDiscontinuity,

    #[error("responder checkpoint predecessor does not equal the exact previous checkpoint ID")]
    PredecessorMismatch,

    #[error("responder sync round rolls back or skips a round")]
    SyncRoundDiscontinuity,

    #[error("responder successor removes cumulative {field} evidence")]
    EventEvidenceRollback { field: &'static str },

    #[error("terminal responder checkpoint cannot have a successor")]
    TerminalCheckpointHasSuccessor,

    #[error("responder pause or resume changed state other than the paused flag")]
    InvalidPauseTransition,

    #[error("responder checkpoint next-action transition is invalid")]
    InvalidActionTransition,
}

pub fn encode_sync_responder_checkpoint_content_v2(
    content: &CanonicalSyncResponderCheckpointContentV2,
) -> Result<Vec<u8>, CanonicalSyncResponderSessionV2Error> {
    validate_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 14);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.protocol_identifier);
    push_bytes(&mut output, &content.conversation_id);
    push_bytes(&mut output, &content.responder_device_id);
    push_bytes(&mut output, &content.requester_device_id);
    push_bytes(&mut output, &content.session_binding);
    push_unsigned(&mut output, content.checkpoint_sequence);
    push_unsigned(&mut output, content.sync_round);
    encode_optional_id(&mut output, content.predecessor_checkpoint_id);
    encode_keys(&mut output, &content.received_event_keys)?;
    encode_keys(&mut output, &content.sent_event_keys)?;
    push_unsigned(&mut output, u64::from(content.continuation_required));
    push_unsigned(&mut output, content.next_action.code());
    push_unsigned(&mut output, u64::from(content.paused));
    bounded(output)
}

pub fn sync_responder_checkpoint_signing_bytes_v2(
    content: &CanonicalSyncResponderCheckpointContentV2,
) -> Result<Vec<u8>, CanonicalSyncResponderSessionV2Error> {
    Ok(prefixed(
        CANONICAL_SYNC_RESPONDER_CHECKPOINT_SIGNATURE_DOMAIN_V2,
        &encode_sync_responder_checkpoint_content_v2(content)?,
    ))
}

pub fn encode_signed_sync_responder_checkpoint_v2(
    checkpoint: &CanonicalSignedSyncResponderCheckpointV2,
) -> Result<Vec<u8>, CanonicalSyncResponderSessionV2Error> {
    verify_signed_sync_responder_checkpoint_v2(checkpoint)?;
    let content = encode_sync_responder_checkpoint_content_v2(&checkpoint.content)?;
    bounded(signed_envelope(&content, &checkpoint.signature))
}

pub fn decode_signed_sync_responder_checkpoint_v2(
    bytes: &[u8],
) -> Result<CanonicalSignedSyncResponderCheckpointV2, CanonicalSyncResponderSessionV2Error> {
    validate_input(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-sync-responder-checkpoint-v2", 3)?;
    decoder.expect_version("signed-sync-responder-checkpoint-v2.schema_version")?;
    let content = decode_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-sync-responder-checkpoint-v2.signature")?;
    decoder.finish()?;
    Ok(CanonicalSignedSyncResponderCheckpointV2 { content, signature })
}

pub fn decode_and_verify_signed_sync_responder_checkpoint_v2(
    bytes: &[u8],
) -> Result<CanonicalSignedSyncResponderCheckpointV2, CanonicalSyncResponderSessionV2Error> {
    let checkpoint = decode_signed_sync_responder_checkpoint_v2(bytes)?;
    verify_signed_sync_responder_checkpoint_v2(&checkpoint)?;
    Ok(checkpoint)
}

pub fn verify_signed_sync_responder_checkpoint_v2(
    checkpoint: &CanonicalSignedSyncResponderCheckpointV2,
) -> Result<(), CanonicalSyncResponderSessionV2Error> {
    validate_content(&checkpoint.content)?;
    let key = VerifyingKey::from_bytes(&checkpoint.content.responder_device_id)
        .map_err(|_| CanonicalSyncResponderSessionV2Error::InvalidPublicKey)?;
    let signature = Signature::try_from(checkpoint.signature.as_slice())
        .map_err(|_| CanonicalSyncResponderSessionV2Error::InvalidSignature)?;
    key.verify_strict(
        &sync_responder_checkpoint_signing_bytes_v2(&checkpoint.content)?,
        &signature,
    )
    .map_err(|_| CanonicalSyncResponderSessionV2Error::InvalidSignature)
}

pub fn sync_responder_checkpoint_id_v2(
    checkpoint: &CanonicalSignedSyncResponderCheckpointV2,
) -> Result<[u8; 32], CanonicalSyncResponderSessionV2Error> {
    let encoded = encode_signed_sync_responder_checkpoint_v2(checkpoint)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(CANONICAL_SYNC_RESPONDER_CHECKPOINT_ID_DOMAIN_V2);
    hasher.update(&encoded);
    Ok(*hasher.finalize().as_bytes())
}

pub fn verify_sync_responder_checkpoint_successor_v2(
    previous: &CanonicalSignedSyncResponderCheckpointV2,
    successor: &CanonicalSignedSyncResponderCheckpointV2,
) -> Result<(), CanonicalSyncResponderSessionV2Error> {
    verify_signed_sync_responder_checkpoint_v2(previous)?;
    verify_signed_sync_responder_checkpoint_v2(successor)?;
    let previous_content = &previous.content;
    let successor_content = &successor.content;
    if previous_content.next_action.is_terminal() {
        return Err(CanonicalSyncResponderSessionV2Error::TerminalCheckpointHasSuccessor);
    }
    if previous_content.protocol_identifier != successor_content.protocol_identifier
        || previous_content.conversation_id != successor_content.conversation_id
        || previous_content.responder_device_id != successor_content.responder_device_id
        || previous_content.requester_device_id != successor_content.requester_device_id
        || previous_content.session_binding != successor_content.session_binding
    {
        return Err(CanonicalSyncResponderSessionV2Error::SessionIdentityChanged);
    }
    if previous_content.checkpoint_sequence.checked_add(1)
        != Some(successor_content.checkpoint_sequence)
    {
        return Err(CanonicalSyncResponderSessionV2Error::CheckpointSequenceDiscontinuity);
    }
    if successor_content.predecessor_checkpoint_id
        != Some(sync_responder_checkpoint_id_v2(previous)?)
    {
        return Err(CanonicalSyncResponderSessionV2Error::PredecessorMismatch);
    }
    if successor_content.sync_round < previous_content.sync_round
        || successor_content.sync_round > previous_content.sync_round.saturating_add(1)
    {
        return Err(CanonicalSyncResponderSessionV2Error::SyncRoundDiscontinuity);
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
        if previous_content.next_action != CanonicalSyncResponderNextActionV2::SendComplete
            || !previous_content.continuation_required
            || successor_content.next_action != CanonicalSyncResponderNextActionV2::ReceiveInventory
            || successor_content.continuation_required
            || previous_content.received_event_keys != successor_content.received_event_keys
            || previous_content.sent_event_keys != successor_content.sent_event_keys
        {
            return Err(CanonicalSyncResponderSessionV2Error::InvalidActionTransition);
        }
        return Ok(());
    }

    if successor_content.next_action == CanonicalSyncResponderNextActionV2::Rejected {
        return Ok(());
    }
    let valid = matches!(
        (previous_content.next_action, successor_content.next_action),
        (
            CanonicalSyncResponderNextActionV2::ReceiveInventory,
            CanonicalSyncResponderNextActionV2::SendDiff
        ) | (
            CanonicalSyncResponderNextActionV2::SendDiff,
            CanonicalSyncResponderNextActionV2::ReceiveEventBatch
        ) | (
            CanonicalSyncResponderNextActionV2::ReceiveEventBatch,
            CanonicalSyncResponderNextActionV2::SendComplete
        ) | (
            CanonicalSyncResponderNextActionV2::SendComplete,
            CanonicalSyncResponderNextActionV2::Complete
        )
    );
    if !valid
        || (successor_content.next_action == CanonicalSyncResponderNextActionV2::Complete
            && previous_content.continuation_required)
    {
        Err(CanonicalSyncResponderSessionV2Error::InvalidActionTransition)
    } else {
        Ok(())
    }
}

fn validate_content(
    content: &CanonicalSyncResponderCheckpointContentV2,
) -> Result<(), CanonicalSyncResponderSessionV2Error> {
    if content.protocol_identifier != *CANONICAL_SYNC_SESSION_PROTOCOL_IDENTIFIER_V2 {
        return Err(CanonicalSyncResponderSessionV2Error::WrongProtocolIdentifier);
    }
    if content.responder_device_id == content.requester_device_id {
        return Err(CanonicalSyncResponderSessionV2Error::SameDevice);
    }
    if content.checkpoint_sequence == 0 {
        return Err(CanonicalSyncResponderSessionV2Error::InvalidCheckpointSequence);
    }
    if content.sync_round == 0 || content.sync_round > content.checkpoint_sequence {
        return Err(CanonicalSyncResponderSessionV2Error::InvalidSyncRound);
    }
    if (content.checkpoint_sequence == 1) != content.predecessor_checkpoint_id.is_none() {
        return Err(CanonicalSyncResponderSessionV2Error::InvalidPredecessorPresence);
    }
    validate_keys("received_event_keys", &content.received_event_keys)?;
    validate_keys("sent_event_keys", &content.sent_event_keys)?;
    if content.next_action.is_terminal() && (content.paused || content.continuation_required) {
        return Err(CanonicalSyncResponderSessionV2Error::InvalidTerminalState);
    }
    Ok(())
}

fn validate_pause_transition(
    previous: &CanonicalSyncResponderCheckpointContentV2,
    successor: &CanonicalSyncResponderCheckpointContentV2,
) -> Result<(), CanonicalSyncResponderSessionV2Error> {
    if previous.next_action.is_terminal()
        || successor.next_action.is_terminal()
        || previous.paused == successor.paused
        || previous.sync_round != successor.sync_round
        || previous.next_action != successor.next_action
        || previous.received_event_keys != successor.received_event_keys
        || previous.sent_event_keys != successor.sent_event_keys
        || previous.continuation_required != successor.continuation_required
    {
        Err(CanonicalSyncResponderSessionV2Error::InvalidPauseTransition)
    } else {
        Ok(())
    }
}

fn require_monotonic_evidence(
    field: &'static str,
    previous: &[CanonicalSyncEventKeyV2],
    successor: &[CanonicalSyncEventKeyV2],
) -> Result<(), CanonicalSyncResponderSessionV2Error> {
    let mut successor_index = 0;
    for previous_key in previous {
        while successor_index < successor.len() && successor[successor_index] < *previous_key {
            successor_index += 1;
        }
        if successor.get(successor_index) != Some(previous_key) {
            return Err(CanonicalSyncResponderSessionV2Error::EventEvidenceRollback { field });
        }
        successor_index += 1;
    }
    Ok(())
}

fn validate_keys(
    field: &'static str,
    keys: &[CanonicalSyncEventKeyV2],
) -> Result<(), CanonicalSyncResponderSessionV2Error> {
    if keys.len() > MAX_CANONICAL_SYNC_SESSION_EVENT_KEYS_V2 {
        return Err(CanonicalSyncResponderSessionV2Error::TooManyEventKeys {
            field,
            actual: keys.len(),
            maximum: MAX_CANONICAL_SYNC_SESSION_EVENT_KEYS_V2,
        });
    }
    if keys.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(CanonicalSyncResponderSessionV2Error::NonCanonicalEventKeyOrder { field });
    }
    Ok(())
}

fn decode_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalSyncResponderCheckpointContentV2, CanonicalSyncResponderSessionV2Error> {
    decoder.expect_array("sync-responder-checkpoint-content-v2", 14)?;
    decoder.expect_version("sync-responder-checkpoint-content-v2.schema_version")?;
    let protocol_identifier =
        decoder.fixed_bytes("sync-responder-checkpoint-content-v2.protocol_identifier")?;
    let conversation_id =
        decoder.fixed_bytes("sync-responder-checkpoint-content-v2.conversation_id")?;
    let responder_device_id =
        decoder.fixed_bytes("sync-responder-checkpoint-content-v2.responder_device_id")?;
    let requester_device_id =
        decoder.fixed_bytes("sync-responder-checkpoint-content-v2.requester_device_id")?;
    let session_binding =
        decoder.fixed_bytes("sync-responder-checkpoint-content-v2.session_binding")?;
    let checkpoint_sequence =
        decoder.unsigned("sync-responder-checkpoint-content-v2.checkpoint_sequence")?;
    let sync_round = decoder.unsigned("sync-responder-checkpoint-content-v2.sync_round")?;
    let predecessor_checkpoint_id = decode_optional_id(
        decoder,
        "sync-responder-checkpoint-content-v2.predecessor_checkpoint_id",
    )?;
    let received_event_keys = decode_keys(
        decoder,
        "sync-responder-checkpoint-content-v2.received_event_keys",
    )?;
    let sent_event_keys = decode_keys(
        decoder,
        "sync-responder-checkpoint-content-v2.sent_event_keys",
    )?;
    let continuation_required = decode_bool(
        decoder,
        "sync-responder-checkpoint-content-v2.continuation_required",
    )?;
    let next_action = CanonicalSyncResponderNextActionV2::from_code(
        decoder.unsigned("sync-responder-checkpoint-content-v2.next_action")?,
    )?;
    let paused = decode_bool(decoder, "sync-responder-checkpoint-content-v2.paused")?;
    let content = CanonicalSyncResponderCheckpointContentV2 {
        protocol_identifier,
        conversation_id,
        responder_device_id,
        requester_device_id,
        session_binding,
        checkpoint_sequence,
        sync_round,
        predecessor_checkpoint_id,
        received_event_keys,
        sent_event_keys,
        continuation_required,
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
) -> Result<Option<[u8; 32]>, CanonicalSyncResponderSessionV2Error> {
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
) -> Result<(), CanonicalSyncResponderSessionV2Error> {
    push_array_header(output, keys.len() as u64);
    for key in keys {
        push_bytes(output, &key.encode()?);
    }
    Ok(())
}

fn decode_keys(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
) -> Result<Vec<CanonicalSyncEventKeyV2>, CanonicalSyncResponderSessionV2Error> {
    let count = decoder.array(field)?;
    if count > MAX_CANONICAL_SYNC_SESSION_EVENT_KEYS_V2 {
        return Err(CanonicalSyncResponderSessionV2Error::TooManyEventKeys {
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
) -> Result<bool, CanonicalSyncResponderSessionV2Error> {
    match decoder.unsigned(field)? {
        0 => Ok(false),
        1 => Ok(true),
        actual => Err(CanonicalSyncResponderSessionV2Error::InvalidBoolean { field, actual }),
    }
}

fn validate_input(bytes: &[u8]) -> Result<(), CanonicalSyncResponderSessionV2Error> {
    ensure_bound(bytes)?;
    validate_canonical_profile_v1(bytes)?;
    Ok(())
}

fn ensure_bound(bytes: &[u8]) -> Result<(), CanonicalSyncResponderSessionV2Error> {
    if bytes.len() > MAX_CANONICAL_SYNC_SESSION_CHECKPOINT_BYTES_V2 {
        Err(CanonicalSyncResponderSessionV2Error::CheckpointTooLarge)
    } else {
        Ok(())
    }
}

fn bounded(bytes: Vec<u8>) -> Result<Vec<u8>, CanonicalSyncResponderSessionV2Error> {
    ensure_bound(&bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;
    use crate::canonical_sync_history_v2::CanonicalSyncEventNamespaceV2;
    use crate::canonical_sync_session_v2::CANONICAL_SYNC_SESSION_CHECKPOINT_SIGNATURE_DOMAIN_V2;

    fn key(namespace: CanonicalSyncEventNamespaceV2, value: u8) -> CanonicalSyncEventKeyV2 {
        CanonicalSyncEventKeyV2::new(namespace, [value; 32])
    }

    fn sign(
        content: CanonicalSyncResponderCheckpointContentV2,
        key: &SigningKey,
    ) -> Result<CanonicalSignedSyncResponderCheckpointV2, CanonicalSyncResponderSessionV2Error>
    {
        let signature = key
            .sign(&sync_responder_checkpoint_signing_bytes_v2(&content)?)
            .to_bytes();
        Ok(CanonicalSignedSyncResponderCheckpointV2 { content, signature })
    }

    fn initial(key: &SigningKey) -> CanonicalSyncResponderCheckpointContentV2 {
        CanonicalSyncResponderCheckpointContentV2 {
            protocol_identifier: *CANONICAL_SYNC_SESSION_PROTOCOL_IDENTIFIER_V2,
            conversation_id: [0x31; 32],
            responder_device_id: key.verifying_key().to_bytes(),
            requester_device_id: [0x42; 32],
            session_binding: [0x53; 32],
            checkpoint_sequence: 1,
            sync_round: 1,
            predecessor_checkpoint_id: None,
            received_event_keys: Vec::new(),
            sent_event_keys: Vec::new(),
            continuation_required: false,
            next_action: CanonicalSyncResponderNextActionV2::ReceiveInventory,
            paused: false,
        }
    }

    fn successor_content(
        previous: &CanonicalSignedSyncResponderCheckpointV2,
        next_action: CanonicalSyncResponderNextActionV2,
    ) -> Result<CanonicalSyncResponderCheckpointContentV2, CanonicalSyncResponderSessionV2Error>
    {
        let mut content = previous.content.clone();
        content.checkpoint_sequence += 1;
        content.predecessor_checkpoint_id = Some(sync_responder_checkpoint_id_v2(previous)?);
        content.next_action = next_action;
        Ok(content)
    }

    #[test]
    fn responder_checkpoint_chain_round_trips_and_is_domain_separated()
    -> Result<(), Box<dyn std::error::Error>> {
        let signing_key = SigningKey::from_bytes(&[0x17; 32]);
        let receive_inventory = sign(initial(&signing_key), &signing_key)?;
        let mut send_diff = successor_content(
            &receive_inventory,
            CanonicalSyncResponderNextActionV2::SendDiff,
        )?;
        send_diff.sent_event_keys = vec![key(CanonicalSyncEventNamespaceV2::CanonicalCborV1, 1)];
        send_diff.continuation_required = true;
        let send_diff = sign(send_diff, &signing_key)?;
        let receive_batch = sign(
            successor_content(
                &send_diff,
                CanonicalSyncResponderNextActionV2::ReceiveEventBatch,
            )?,
            &signing_key,
        )?;
        let mut send_complete = successor_content(
            &receive_batch,
            CanonicalSyncResponderNextActionV2::SendComplete,
        )?;
        send_complete.received_event_keys =
            vec![key(CanonicalSyncEventNamespaceV2::LegacyPostcardV6, 2)];
        let send_complete = sign(send_complete, &signing_key)?;
        let mut next_round = successor_content(
            &send_complete,
            CanonicalSyncResponderNextActionV2::ReceiveInventory,
        )?;
        next_round.sync_round += 1;
        next_round.continuation_required = false;
        let next_round = sign(next_round, &signing_key)?;

        let chain = [
            receive_inventory,
            send_diff,
            receive_batch,
            send_complete,
            next_round,
        ];
        assert_ne!(
            CANONICAL_SYNC_RESPONDER_CHECKPOINT_SIGNATURE_DOMAIN_V2,
            CANONICAL_SYNC_SESSION_CHECKPOINT_SIGNATURE_DOMAIN_V2
        );
        for checkpoint in &chain {
            let encoded = encode_signed_sync_responder_checkpoint_v2(checkpoint)?;
            assert_eq!(
                decode_and_verify_signed_sync_responder_checkpoint_v2(&encoded)?,
                *checkpoint
            );
        }
        for pair in chain.windows(2) {
            verify_sync_responder_checkpoint_successor_v2(&pair[0], &pair[1])?;
        }
        Ok(())
    }

    #[test]
    fn responder_checkpoint_identity_evidence_and_actions_fail_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let signing_key = SigningKey::from_bytes(&[0x27; 32]);
        let initial = sign(initial(&signing_key), &signing_key)?;
        let mut send_diff_content =
            successor_content(&initial, CanonicalSyncResponderNextActionV2::SendDiff)?;
        send_diff_content.sent_event_keys =
            vec![key(CanonicalSyncEventNamespaceV2::LegacyPostcardV6, 3)];
        let send_diff = sign(send_diff_content, &signing_key)?;
        verify_sync_responder_checkpoint_successor_v2(&initial, &send_diff)?;

        let mut wrong_identity = send_diff.content.clone();
        wrong_identity.requester_device_id[0] ^= 1;
        let wrong_identity = sign(wrong_identity, &signing_key)?;
        assert!(matches!(
            verify_sync_responder_checkpoint_successor_v2(&initial, &wrong_identity),
            Err(CanonicalSyncResponderSessionV2Error::SessionIdentityChanged)
        ));

        let mut rollback = successor_content(
            &send_diff,
            CanonicalSyncResponderNextActionV2::ReceiveEventBatch,
        )?;
        rollback.sent_event_keys.clear();
        let rollback = sign(rollback, &signing_key)?;
        assert!(matches!(
            verify_sync_responder_checkpoint_successor_v2(&send_diff, &rollback),
            Err(CanonicalSyncResponderSessionV2Error::EventEvidenceRollback { .. })
        ));

        let invalid_action = sign(
            successor_content(&initial, CanonicalSyncResponderNextActionV2::SendComplete)?,
            &signing_key,
        )?;
        assert!(matches!(
            verify_sync_responder_checkpoint_successor_v2(&initial, &invalid_action),
            Err(CanonicalSyncResponderSessionV2Error::InvalidActionTransition)
        ));

        let mut invalid_signature = send_diff;
        invalid_signature.signature[0] ^= 1;
        assert!(matches!(
            verify_signed_sync_responder_checkpoint_v2(&invalid_signature),
            Err(CanonicalSyncResponderSessionV2Error::InvalidSignature)
        ));
        Ok(())
    }

    #[test]
    fn responder_pause_terminal_and_typed_order_fail_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let signing_key = SigningKey::from_bytes(&[0x37; 32]);
        let initial = sign(initial(&signing_key), &signing_key)?;
        let mut paused_content = successor_content(
            &initial,
            CanonicalSyncResponderNextActionV2::ReceiveInventory,
        )?;
        paused_content.paused = true;
        let paused = sign(paused_content, &signing_key)?;
        verify_sync_responder_checkpoint_successor_v2(&initial, &paused)?;

        let mut resume_content = successor_content(
            &paused,
            CanonicalSyncResponderNextActionV2::ReceiveInventory,
        )?;
        resume_content.paused = false;
        let resumed = sign(resume_content, &signing_key)?;
        verify_sync_responder_checkpoint_successor_v2(&paused, &resumed)?;

        let mut noncanonical = initial.content.clone();
        noncanonical.received_event_keys = vec![
            key(CanonicalSyncEventNamespaceV2::CanonicalCborV1, 9),
            key(CanonicalSyncEventNamespaceV2::LegacyPostcardV6, 9),
        ];
        assert!(matches!(
            encode_sync_responder_checkpoint_content_v2(&noncanonical),
            Err(CanonicalSyncResponderSessionV2Error::NonCanonicalEventKeyOrder { .. })
        ));

        let mut terminal =
            successor_content(&initial, CanonicalSyncResponderNextActionV2::Rejected)?;
        terminal.continuation_required = false;
        let terminal = sign(terminal, &signing_key)?;
        verify_sync_responder_checkpoint_successor_v2(&initial, &terminal)?;
        let after_terminal = sign(
            successor_content(&terminal, CanonicalSyncResponderNextActionV2::Rejected)?,
            &signing_key,
        )?;
        assert!(matches!(
            verify_sync_responder_checkpoint_successor_v2(&terminal, &after_terminal),
            Err(CanonicalSyncResponderSessionV2Error::TerminalCheckpointHasSuccessor)
        ));
        Ok(())
    }
}
