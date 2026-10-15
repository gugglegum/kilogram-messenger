//! Inactive deterministic-CBOR history recovery checkpoint candidate for M1.1.4.
//!
//! The active recovery implementation remains Postcard encoded. This module
//! defines a cross-language checkpoint chain without changing runtime readers.

use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

use crate::canonical_v1::{
    CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
    push_unsigned, validate_canonical_profile_v1,
};

pub const MAX_CANONICAL_RECOVERY_PAGE_SIZE: u64 = 256;
pub const CANONICAL_HISTORY_RECOVERY_CHECKPOINT_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:history-recovery-checkpoint-signature:cbor-v1\0";
pub const CANONICAL_HISTORY_RECOVERY_CHECKPOINT_ID_DOMAIN: &[u8] =
    b"kilogram:history-recovery-checkpoint-id:cbor-v1\0";
pub const CANONICAL_HISTORY_RECOVERY_ID_DOMAIN: &[u8] = b"kilogram:history-recovery-id:cbor-v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRecoveryInventoryClaimV1 {
    pub event_count: u64,
    pub digest: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalHistoryRecoveryCheckpointContentV1 {
    pub account_id: [u8; 32],
    pub conversation_id: [u8; 32],
    pub source_device_id: [u8; 32],
    pub recipient_device_id: [u8; 32],
    pub sas: [u8; 32],
    pub approved_range_start: u64,
    pub approved_range_end: u64,
    pub page_size: u64,
    pub inventory_claim: Option<CanonicalRecoveryInventoryClaimV1>,
    pub next_range_start: u64,
    pub previous_checkpoint_id: Option<[u8; 32]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedHistoryRecoveryCheckpointV1 {
    pub content: CanonicalHistoryRecoveryCheckpointContentV1,
    pub signature: [u8; 64],
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum CanonicalRecoveryError {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),

    #[error("{object} array has {actual} items; expected {expected}")]
    WrongArrayLength {
        object: &'static str,
        expected: usize,
        actual: usize,
    },

    #[error("{field} has CBOR major type {actual}; expected {expected}")]
    WrongMajorType {
        field: &'static str,
        expected: u8,
        actual: u8,
    },

    #[error("{field} has {actual} bytes; expected {expected}")]
    WrongByteStringLength {
        field: &'static str,
        expected: usize,
        actual: usize,
    },

    #[error("{field} has value {actual}; expected {expected}")]
    WrongUnsignedValue {
        field: &'static str,
        expected: u64,
        actual: u64,
    },

    #[error("source and recipient Device IDs must differ")]
    SameDevice,

    #[error("approved or next recovery range is invalid")]
    InvalidRange,

    #[error("recovery page size {actual} is outside 1..={maximum}")]
    InvalidPageSize { actual: u64, maximum: u64 },

    #[error("inventory claim and checkpoint position are inconsistent")]
    InvalidInventoryClaim,

    #[error("previous checkpoint ID and checkpoint position are inconsistent")]
    InvalidPreviousCheckpoint,

    #[error("signed recovery checkpoint contains an invalid recipient Ed25519 key")]
    InvalidPublicKey,

    #[error("signed recovery checkpoint contains an invalid Ed25519 signature")]
    InvalidSignature,
}

pub fn encode_history_recovery_checkpoint_content_v1(
    content: &CanonicalHistoryRecoveryCheckpointContentV1,
) -> Result<Vec<u8>, CanonicalRecoveryError> {
    validate_content(content)?;
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 12);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut encoded, &content.account_id);
    push_bytes(&mut encoded, &content.conversation_id);
    push_bytes(&mut encoded, &content.source_device_id);
    push_bytes(&mut encoded, &content.recipient_device_id);
    push_bytes(&mut encoded, &content.sas);
    push_unsigned(&mut encoded, content.approved_range_start);
    push_unsigned(&mut encoded, content.approved_range_end);
    push_unsigned(&mut encoded, content.page_size);
    match &content.inventory_claim {
        None => push_array_header(&mut encoded, 0),
        Some(claim) => {
            push_array_header(&mut encoded, 2);
            push_unsigned(&mut encoded, claim.event_count);
            push_bytes(&mut encoded, &claim.digest);
        }
    }
    push_unsigned(&mut encoded, content.next_range_start);
    match &content.previous_checkpoint_id {
        None => push_array_header(&mut encoded, 0),
        Some(checkpoint_id) => {
            push_array_header(&mut encoded, 1);
            push_bytes(&mut encoded, checkpoint_id);
        }
    }
    Ok(encoded)
}

pub fn history_recovery_checkpoint_signing_bytes_v1(
    content: &CanonicalHistoryRecoveryCheckpointContentV1,
) -> Result<Vec<u8>, CanonicalRecoveryError> {
    Ok(prefixed(
        CANONICAL_HISTORY_RECOVERY_CHECKPOINT_SIGNATURE_DOMAIN,
        &encode_history_recovery_checkpoint_content_v1(content)?,
    ))
}

pub fn encode_signed_history_recovery_checkpoint_v1(
    checkpoint: &CanonicalSignedHistoryRecoveryCheckpointV1,
) -> Result<Vec<u8>, CanonicalRecoveryError> {
    verify_signed_history_recovery_checkpoint_v1(checkpoint)?;
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 3);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    encoded.extend_from_slice(&encode_history_recovery_checkpoint_content_v1(
        &checkpoint.content,
    )?);
    push_bytes(&mut encoded, &checkpoint.signature);
    Ok(encoded)
}

pub fn decode_and_verify_signed_history_recovery_checkpoint_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedHistoryRecoveryCheckpointV1, CanonicalRecoveryError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = RecoveryDecoder::new(bytes);
    decoder.expect_array("signed-history-recovery-checkpoint-v1", 3)?;
    decoder.expect_version("signed-history-recovery-checkpoint-v1.schema_version")?;
    let content = decode_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-history-recovery-checkpoint-v1.signature")?;
    decoder.finish()?;
    let checkpoint = CanonicalSignedHistoryRecoveryCheckpointV1 { content, signature };
    verify_signed_history_recovery_checkpoint_v1(&checkpoint)?;
    Ok(checkpoint)
}

pub fn verify_signed_history_recovery_checkpoint_v1(
    checkpoint: &CanonicalSignedHistoryRecoveryCheckpointV1,
) -> Result<(), CanonicalRecoveryError> {
    validate_content(&checkpoint.content)?;
    let key = VerifyingKey::from_bytes(&checkpoint.content.recipient_device_id)
        .map_err(|_| CanonicalRecoveryError::InvalidPublicKey)?;
    let signature = Signature::try_from(checkpoint.signature.as_slice())
        .map_err(|_| CanonicalRecoveryError::InvalidSignature)?;
    key.verify_strict(
        &history_recovery_checkpoint_signing_bytes_v1(&checkpoint.content)?,
        &signature,
    )
    .map_err(|_| CanonicalRecoveryError::InvalidSignature)
}

pub fn history_recovery_identity_content_v1(
    content: &CanonicalHistoryRecoveryCheckpointContentV1,
) -> Result<Vec<u8>, CanonicalRecoveryError> {
    validate_content(content)?;
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 9);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut encoded, &content.account_id);
    push_bytes(&mut encoded, &content.conversation_id);
    push_bytes(&mut encoded, &content.source_device_id);
    push_bytes(&mut encoded, &content.recipient_device_id);
    push_bytes(&mut encoded, &content.sas);
    push_unsigned(&mut encoded, content.approved_range_start);
    push_unsigned(&mut encoded, content.approved_range_end);
    push_unsigned(&mut encoded, content.page_size);
    Ok(encoded)
}

pub fn history_recovery_id_v1(
    content: &CanonicalHistoryRecoveryCheckpointContentV1,
) -> Result<[u8; 32], CanonicalRecoveryError> {
    let identity = history_recovery_identity_content_v1(content)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(CANONICAL_HISTORY_RECOVERY_ID_DOMAIN);
    hasher.update(&identity);
    Ok(*hasher.finalize().as_bytes())
}

pub fn history_recovery_checkpoint_id_v1(
    checkpoint: &CanonicalSignedHistoryRecoveryCheckpointV1,
) -> Result<[u8; 32], CanonicalRecoveryError> {
    let encoded = encode_signed_history_recovery_checkpoint_v1(checkpoint)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(CANONICAL_HISTORY_RECOVERY_CHECKPOINT_ID_DOMAIN);
    hasher.update(&encoded);
    Ok(*hasher.finalize().as_bytes())
}

pub fn validate_history_recovery_successor_v1(
    previous: &CanonicalSignedHistoryRecoveryCheckpointV1,
    successor: &CanonicalSignedHistoryRecoveryCheckpointV1,
) -> Result<(), CanonicalRecoveryError> {
    verify_signed_history_recovery_checkpoint_v1(previous)?;
    verify_signed_history_recovery_checkpoint_v1(successor)?;
    if previous.content.account_id != successor.content.account_id
        || previous.content.conversation_id != successor.content.conversation_id
        || previous.content.source_device_id != successor.content.source_device_id
        || previous.content.recipient_device_id != successor.content.recipient_device_id
        || previous.content.sas != successor.content.sas
        || previous.content.approved_range_start != successor.content.approved_range_start
        || previous.content.approved_range_end != successor.content.approved_range_end
        || previous.content.page_size != successor.content.page_size
        || successor.content.previous_checkpoint_id
            != Some(history_recovery_checkpoint_id_v1(previous)?)
        || successor.content.next_range_start <= previous.content.next_range_start
    {
        return Err(CanonicalRecoveryError::InvalidPreviousCheckpoint);
    }
    match (
        &previous.content.inventory_claim,
        &successor.content.inventory_claim,
    ) {
        (None, Some(_)) => {}
        (Some(previous_claim), Some(successor_claim)) if previous_claim == successor_claim => {}
        _ => return Err(CanonicalRecoveryError::InvalidInventoryClaim),
    }
    Ok(())
}

fn validate_content(
    content: &CanonicalHistoryRecoveryCheckpointContentV1,
) -> Result<(), CanonicalRecoveryError> {
    if content.source_device_id == content.recipient_device_id {
        return Err(CanonicalRecoveryError::SameDevice);
    }
    if content.approved_range_start >= content.approved_range_end
        || content.next_range_start < content.approved_range_start
        || content.next_range_start > content.approved_range_end
    {
        return Err(CanonicalRecoveryError::InvalidRange);
    }
    if content.page_size == 0 || content.page_size > MAX_CANONICAL_RECOVERY_PAGE_SIZE {
        return Err(CanonicalRecoveryError::InvalidPageSize {
            actual: content.page_size,
            maximum: MAX_CANONICAL_RECOVERY_PAGE_SIZE,
        });
    }
    match &content.inventory_claim {
        None if content.next_range_start == content.approved_range_start => {}
        Some(claim) if content.next_range_start <= claim.event_count => {}
        _ => return Err(CanonicalRecoveryError::InvalidInventoryClaim),
    }
    match (&content.inventory_claim, content.previous_checkpoint_id) {
        (None, None) => {}
        (Some(_), Some(_)) => {}
        _ => return Err(CanonicalRecoveryError::InvalidPreviousCheckpoint),
    }
    Ok(())
}

fn decode_content_from(
    decoder: &mut RecoveryDecoder<'_>,
) -> Result<CanonicalHistoryRecoveryCheckpointContentV1, CanonicalRecoveryError> {
    decoder.expect_array("history-recovery-checkpoint-content-v1", 12)?;
    decoder.expect_version("history-recovery-checkpoint-content-v1.schema_version")?;
    let account_id = decoder.fixed_bytes("history-recovery-checkpoint-content-v1.account_id")?;
    let conversation_id =
        decoder.fixed_bytes("history-recovery-checkpoint-content-v1.conversation_id")?;
    let source_device_id =
        decoder.fixed_bytes("history-recovery-checkpoint-content-v1.source_device_id")?;
    let recipient_device_id =
        decoder.fixed_bytes("history-recovery-checkpoint-content-v1.recipient_device_id")?;
    let sas = decoder.fixed_bytes("history-recovery-checkpoint-content-v1.sas")?;
    let approved_range_start =
        decoder.unsigned("history-recovery-checkpoint-content-v1.approved_range_start")?;
    let approved_range_end =
        decoder.unsigned("history-recovery-checkpoint-content-v1.approved_range_end")?;
    let page_size = decoder.unsigned("history-recovery-checkpoint-content-v1.page_size")?;
    let claim_items = decoder.array("history-recovery-checkpoint-content-v1.inventory_claim")?;
    let inventory_claim = match claim_items {
        0 => None,
        2 => Some(CanonicalRecoveryInventoryClaimV1 {
            event_count: decoder
                .unsigned("history-recovery-checkpoint-content-v1.inventory_event_count")?,
            digest: decoder
                .fixed_bytes("history-recovery-checkpoint-content-v1.inventory_digest")?,
        }),
        actual => {
            return Err(CanonicalRecoveryError::WrongArrayLength {
                object: "history-recovery-checkpoint-content-v1.inventory_claim",
                expected: 2,
                actual,
            });
        }
    };
    let next_range_start =
        decoder.unsigned("history-recovery-checkpoint-content-v1.next_range_start")?;
    let previous_items =
        decoder.array("history-recovery-checkpoint-content-v1.previous_checkpoint")?;
    let previous_checkpoint_id = match previous_items {
        0 => None,
        1 => Some(
            decoder.fixed_bytes("history-recovery-checkpoint-content-v1.previous_checkpoint_id")?,
        ),
        actual => {
            return Err(CanonicalRecoveryError::WrongArrayLength {
                object: "history-recovery-checkpoint-content-v1.previous_checkpoint",
                expected: 1,
                actual,
            });
        }
    };
    let content = CanonicalHistoryRecoveryCheckpointContentV1 {
        account_id,
        conversation_id,
        source_device_id,
        recipient_device_id,
        sas,
        approved_range_start,
        approved_range_end,
        page_size,
        inventory_claim,
        next_range_start,
        previous_checkpoint_id,
    };
    validate_content(&content)?;
    Ok(content)
}

struct RecoveryDecoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> RecoveryDecoder<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn finish(self) -> Result<(), CanonicalRecoveryError> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(CanonicalWireError::TrailingData.into())
        }
    }

    fn expect_array(
        &mut self,
        object: &'static str,
        expected: usize,
    ) -> Result<(), CanonicalRecoveryError> {
        let actual = self.array(object)?;
        if actual != expected {
            return Err(CanonicalRecoveryError::WrongArrayLength {
                object,
                expected,
                actual,
            });
        }
        Ok(())
    }

    fn expect_version(&mut self, field: &'static str) -> Result<(), CanonicalRecoveryError> {
        let actual = self.unsigned(field)?;
        if actual != CANONICAL_PROFILE_VERSION {
            return Err(CanonicalRecoveryError::WrongUnsignedValue {
                field,
                expected: CANONICAL_PROFILE_VERSION,
                actual,
            });
        }
        Ok(())
    }

    fn array(&mut self, field: &'static str) -> Result<usize, CanonicalRecoveryError> {
        usize::try_from(self.head(4, field)?).map_err(|_| CanonicalWireError::LengthOverflow.into())
    }

    fn unsigned(&mut self, field: &'static str) -> Result<u64, CanonicalRecoveryError> {
        self.head(0, field)
    }

    fn fixed_bytes<const N: usize>(
        &mut self,
        field: &'static str,
    ) -> Result<[u8; N], CanonicalRecoveryError> {
        let length = usize::try_from(self.head(2, field)?)
            .map_err(|_| CanonicalWireError::LengthOverflow)?;
        if length != N {
            return Err(CanonicalRecoveryError::WrongByteStringLength {
                field,
                expected: N,
                actual: length,
            });
        }
        self.bytes_array()
    }

    fn head(
        &mut self,
        expected_major: u8,
        field: &'static str,
    ) -> Result<u64, CanonicalRecoveryError> {
        let initial = self.byte()?;
        let actual_major = initial >> 5;
        if actual_major != expected_major {
            return Err(CanonicalRecoveryError::WrongMajorType {
                field,
                expected: expected_major,
                actual: actual_major,
            });
        }
        self.argument(initial & 0x1f)
    }

    fn argument(&mut self, additional: u8) -> Result<u64, CanonicalRecoveryError> {
        match additional {
            value @ 0..=23 => Ok(u64::from(value)),
            24 => {
                let value = u64::from(self.byte()?);
                if value < 24 {
                    return Err(CanonicalWireError::NonMinimalArgument.into());
                }
                Ok(value)
            }
            25 => {
                let value = u64::from(u16::from_be_bytes(self.bytes_array()?));
                if value <= u64::from(u8::MAX) {
                    return Err(CanonicalWireError::NonMinimalArgument.into());
                }
                Ok(value)
            }
            26 => {
                let value = u64::from(u32::from_be_bytes(self.bytes_array()?));
                if value <= u64::from(u16::MAX) {
                    return Err(CanonicalWireError::NonMinimalArgument.into());
                }
                Ok(value)
            }
            27 => {
                let value = u64::from_be_bytes(self.bytes_array()?);
                if value <= u64::from(u32::MAX) {
                    return Err(CanonicalWireError::NonMinimalArgument.into());
                }
                Ok(value)
            }
            31 => Err(CanonicalWireError::IndefiniteLength.into()),
            value => Err(CanonicalWireError::ReservedAdditionalInformation(value).into()),
        }
    }

    fn byte(&mut self) -> Result<u8, CanonicalRecoveryError> {
        let value = self
            .bytes
            .get(self.cursor)
            .copied()
            .ok_or(CanonicalWireError::UnexpectedEnd)?;
        self.cursor += 1;
        Ok(value)
    }

    fn bytes_array<const N: usize>(&mut self) -> Result<[u8; N], CanonicalRecoveryError> {
        let end = self
            .cursor
            .checked_add(N)
            .ok_or(CanonicalWireError::LengthOverflow)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(CanonicalWireError::UnexpectedEnd)?;
        self.cursor = end;
        value
            .try_into()
            .map_err(|_| CanonicalWireError::UnexpectedEnd.into())
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::Value;

    use super::*;

    fn signed(
        key: &SigningKey,
        content: CanonicalHistoryRecoveryCheckpointContentV1,
    ) -> Result<CanonicalSignedHistoryRecoveryCheckpointV1, CanonicalRecoveryError> {
        Ok(CanonicalSignedHistoryRecoveryCheckpointV1 {
            signature: key
                .sign(&history_recovery_checkpoint_signing_bytes_v1(&content)?)
                .to_bytes(),
            content,
        })
    }

    fn initial_content(key: &SigningKey) -> CanonicalHistoryRecoveryCheckpointContentV1 {
        CanonicalHistoryRecoveryCheckpointContentV1 {
            account_id: [1; 32],
            conversation_id: [2; 32],
            source_device_id: [3; 32],
            recipient_device_id: key.verifying_key().to_bytes(),
            sas: [4; 32],
            approved_range_start: 10,
            approved_range_end: 30,
            page_size: 8,
            inventory_claim: None,
            next_range_start: 10,
            previous_checkpoint_id: None,
        }
    }

    fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if !value.len().is_multiple_of(2) {
            return Err("hex value has odd length".into());
        }
        (0..value.len())
            .step_by(2)
            .map(|index| Ok(u8::from_str_radix(&value[index..index + 2], 16)?))
            .collect()
    }

    fn fixed_hex<const N: usize>(value: &Value) -> Result<[u8; N], Box<dyn std::error::Error>> {
        Ok(decode_hex(value.as_str().ok_or("expected hex string")?)?
            .try_into()
            .map_err(|_| "hex value has wrong length")?)
    }

    fn vector_checkpoint(
        value: &Value,
        content: CanonicalHistoryRecoveryCheckpointContentV1,
        key: &SigningKey,
    ) -> Result<CanonicalSignedHistoryRecoveryCheckpointV1, Box<dyn std::error::Error>> {
        let checkpoint = signed(key, content)?;
        assert_eq!(
            encode_history_recovery_checkpoint_content_v1(&checkpoint.content)?,
            decode_hex(
                value["content_cbor_hex"]
                    .as_str()
                    .ok_or("missing content")?
            )?
        );
        assert_eq!(
            history_recovery_checkpoint_signing_bytes_v1(&checkpoint.content)?,
            decode_hex(
                value["signing_preimage_hex"]
                    .as_str()
                    .ok_or("missing preimage")?
            )?
        );
        assert_eq!(checkpoint.signature, fixed_hex(&value["signature_hex"])?);
        assert_eq!(
            encode_signed_history_recovery_checkpoint_v1(&checkpoint)?,
            decode_hex(
                value["signed_cbor_hex"]
                    .as_str()
                    .ok_or("missing signed CBOR")?
            )?
        );
        assert_eq!(
            history_recovery_checkpoint_id_v1(&checkpoint)?,
            fixed_hex(&value["checkpoint_id_hex"])?
        );
        Ok(checkpoint)
    }

    #[test]
    fn checkpoint_chain_round_trips_and_links() -> Result<(), Box<dyn std::error::Error>> {
        let key = SigningKey::from_bytes(&[7; 32]);
        let initial = signed(&key, initial_content(&key))?;
        let initial_bytes = encode_signed_history_recovery_checkpoint_v1(&initial)?;
        assert_eq!(
            decode_and_verify_signed_history_recovery_checkpoint_v1(&initial_bytes)?,
            initial
        );
        let successor = signed(
            &key,
            CanonicalHistoryRecoveryCheckpointContentV1 {
                inventory_claim: Some(CanonicalRecoveryInventoryClaimV1 {
                    event_count: 30,
                    digest: [5; 32],
                }),
                next_range_start: 18,
                previous_checkpoint_id: Some(history_recovery_checkpoint_id_v1(&initial)?),
                ..initial.content.clone()
            },
        )?;
        validate_history_recovery_successor_v1(&initial, &successor)?;
        assert_eq!(
            history_recovery_id_v1(&initial.content)?,
            history_recovery_id_v1(&successor.content)?
        );
        Ok(())
    }

    #[test]
    fn malformed_optional_state_and_signature_are_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        let key = SigningKey::from_bytes(&[7; 32]);
        let mut content = initial_content(&key);
        content.inventory_claim = Some(CanonicalRecoveryInventoryClaimV1 {
            event_count: 30,
            digest: [5; 32],
        });
        assert_eq!(
            validate_content(&content),
            Err(CanonicalRecoveryError::InvalidPreviousCheckpoint)
        );

        let mut encoded =
            encode_signed_history_recovery_checkpoint_v1(&signed(&key, initial_content(&key))?)?;
        let last = encoded
            .last_mut()
            .ok_or_else(|| std::io::Error::other("empty recovery checkpoint"))?;
        *last ^= 1;
        assert_eq!(
            decode_and_verify_signed_history_recovery_checkpoint_v1(&encoded),
            Err(CanonicalRecoveryError::InvalidSignature)
        );
        Ok(())
    }

    #[test]
    fn golden_recovery_vector_matches_exact_bytes_signatures_and_ids()
    -> Result<(), Box<dyn std::error::Error>> {
        let vector: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-RECOVERY-VECTORS.json"
        )))?;
        let input = &vector["input"];
        let expected = &vector["expected"];
        let key = SigningKey::from_bytes(&fixed_hex(&input["recipient_secret_key_hex"])?);
        assert_eq!(
            key.verifying_key().to_bytes(),
            fixed_hex(&expected["recipient_device_id_hex"])?
        );
        let initial_content = CanonicalHistoryRecoveryCheckpointContentV1 {
            account_id: fixed_hex(&input["account_id_hex"])?,
            conversation_id: fixed_hex(&input["conversation_id_hex"])?,
            source_device_id: fixed_hex(&input["source_device_id_hex"])?,
            recipient_device_id: key.verifying_key().to_bytes(),
            sas: fixed_hex(&input["sas_hex"])?,
            approved_range_start: input["approved_range_start"]
                .as_u64()
                .ok_or("missing start")?,
            approved_range_end: input["approved_range_end"].as_u64().ok_or("missing end")?,
            page_size: input["page_size"].as_u64().ok_or("missing page size")?,
            inventory_claim: None,
            next_range_start: input["approved_range_start"]
                .as_u64()
                .ok_or("missing start")?,
            previous_checkpoint_id: None,
        };
        let initial = vector_checkpoint(&expected["initial_checkpoint"], initial_content, &key)?;
        assert_eq!(
            history_recovery_identity_content_v1(&initial.content)?,
            decode_hex(
                expected["recovery_identity_cbor_hex"]
                    .as_str()
                    .ok_or("missing identity")?
            )?
        );
        assert_eq!(
            history_recovery_id_v1(&initial.content)?,
            fixed_hex(&expected["recovery_id_hex"])?
        );

        let successor = vector_checkpoint(
            &expected["successor_checkpoint"],
            CanonicalHistoryRecoveryCheckpointContentV1 {
                inventory_claim: Some(CanonicalRecoveryInventoryClaimV1 {
                    event_count: input["inventory_event_count"]
                        .as_u64()
                        .ok_or("missing count")?,
                    digest: fixed_hex(&input["inventory_digest_hex"])?,
                }),
                next_range_start: input["successor_next_range_start"]
                    .as_u64()
                    .ok_or("missing next")?,
                previous_checkpoint_id: Some(history_recovery_checkpoint_id_v1(&initial)?),
                ..initial.content.clone()
            },
            &key,
        )?;
        validate_history_recovery_successor_v1(&initial, &successor)?;
        assert_eq!(
            history_recovery_id_v1(&initial.content)?,
            history_recovery_id_v1(&successor.content)?
        );
        Ok(())
    }
}
