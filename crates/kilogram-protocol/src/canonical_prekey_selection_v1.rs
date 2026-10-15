//! Inactive deterministic-CBOR prekey selection transcript for M1.1.11.
//!
//! This candidate makes selection portable and auditable. It does not prove
//! control of the initiator Device key and does not change active Postcard.

use thiserror::Error;

use crate::{
    canonical_ratchet_v1::{
        CanonicalRatchetError, CanonicalSignedPrekeyBundleV1, CanonicalSignedPrekeyPoolV1,
        MAX_CANONICAL_NESTED_RATCHET_BYTES, RatchetDecoder,
        decode_and_verify_signed_prekey_pool_v1, encode_signed_prekey_pool_v1, prekey_pool_id_v1,
        verify_signed_prekey_pool_v1,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, push_array_header, push_bytes, push_unsigned,
        validate_canonical_profile_v1,
    },
};

pub const CANONICAL_PREKEY_SELECTION_DOMAIN: &[u8] = b"kilogram:prekey-selection:cbor-v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalPrekeySelectionTranscriptV1 {
    pub initiator_device_id: [u8; 32],
    pub pool: CanonicalSignedPrekeyPoolV1,
    pub selection_digest: [u8; 32],
    pub selected_index: u64,
    pub selected_sequence: u64,
}

#[derive(Debug, Error)]
pub enum CanonicalPrekeySelectionError {
    #[error(transparent)]
    Ratchet(#[from] CanonicalRatchetError),

    #[error("prekey selection digest does not match the exact initiator and pool")]
    SelectionDigestMismatch,

    #[error("prekey selection index is {actual}; expected {expected}")]
    SelectedIndexMismatch { expected: u64, actual: u64 },

    #[error("prekey selection sequence is {actual}; expected {expected}")]
    SelectedSequenceMismatch { expected: u64, actual: u64 },
}

pub fn prekey_selection_input_v1(initiator_device_id: &[u8; 32], pool_id: &[u8; 32]) -> Vec<u8> {
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 3);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut encoded, initiator_device_id);
    push_bytes(&mut encoded, pool_id);
    encoded
}

pub fn prekey_selection_digest_v1(
    initiator_device_id: &[u8; 32],
    pool: &CanonicalSignedPrekeyPoolV1,
) -> Result<[u8; 32], CanonicalPrekeySelectionError> {
    let pool_id = prekey_pool_id_v1(pool)?;
    let input = prekey_selection_input_v1(initiator_device_id, &pool_id);
    let mut hasher = blake3::Hasher::new();
    hasher.update(CANONICAL_PREKEY_SELECTION_DOMAIN);
    hasher.update(&input);
    Ok(*hasher.finalize().as_bytes())
}

pub fn derive_prekey_selection_transcript_v1(
    initiator_device_id: [u8; 32],
    pool: CanonicalSignedPrekeyPoolV1,
) -> Result<CanonicalPrekeySelectionTranscriptV1, CanonicalPrekeySelectionError> {
    verify_signed_prekey_pool_v1(&pool)?;
    let selection_digest = prekey_selection_digest_v1(&initiator_device_id, &pool)?;
    let selected_index = selection_index(&selection_digest, pool.content.bundles.len());
    let selected_sequence = pool.content.bundles[selected_index as usize]
        .content
        .sequence;
    Ok(CanonicalPrekeySelectionTranscriptV1 {
        initiator_device_id,
        pool,
        selection_digest,
        selected_index,
        selected_sequence,
    })
}

pub fn encode_prekey_selection_transcript_v1(
    transcript: &CanonicalPrekeySelectionTranscriptV1,
) -> Result<Vec<u8>, CanonicalPrekeySelectionError> {
    verify_prekey_selection_transcript_v1(transcript)?;
    let pool = encode_signed_prekey_pool_v1(&transcript.pool)?;
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 6);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut encoded, &transcript.initiator_device_id);
    push_bytes(&mut encoded, &pool);
    push_bytes(&mut encoded, &transcript.selection_digest);
    push_unsigned(&mut encoded, transcript.selected_index);
    push_unsigned(&mut encoded, transcript.selected_sequence);
    Ok(encoded)
}

pub fn decode_and_verify_prekey_selection_transcript_v1(
    bytes: &[u8],
) -> Result<CanonicalPrekeySelectionTranscriptV1, CanonicalPrekeySelectionError> {
    validate_canonical_profile_v1(bytes).map_err(CanonicalRatchetError::from)?;
    let mut decoder = RatchetDecoder::new(bytes);
    decoder.expect_array("prekey-selection-transcript-v1", 6)?;
    decoder.expect_version("prekey-selection-transcript-v1.schema_version")?;
    let initiator_device_id =
        decoder.fixed_bytes("prekey-selection-transcript-v1.initiator_device_id")?;
    let pool = decode_and_verify_signed_prekey_pool_v1(decoder.bytes(
        "prekey-selection-transcript-v1.pool",
        MAX_CANONICAL_NESTED_RATCHET_BYTES,
    )?)?;
    let selection_digest =
        decoder.fixed_bytes("prekey-selection-transcript-v1.selection_digest")?;
    let selected_index = decoder.unsigned("prekey-selection-transcript-v1.selected_index")?;
    let selected_sequence = decoder.unsigned("prekey-selection-transcript-v1.selected_sequence")?;
    decoder.finish()?;
    let transcript = CanonicalPrekeySelectionTranscriptV1 {
        initiator_device_id,
        pool,
        selection_digest,
        selected_index,
        selected_sequence,
    };
    verify_prekey_selection_transcript_v1(&transcript)?;
    Ok(transcript)
}

pub fn verify_prekey_selection_transcript_v1(
    transcript: &CanonicalPrekeySelectionTranscriptV1,
) -> Result<(), CanonicalPrekeySelectionError> {
    verify_signed_prekey_pool_v1(&transcript.pool)?;
    let expected_digest =
        prekey_selection_digest_v1(&transcript.initiator_device_id, &transcript.pool)?;
    if transcript.selection_digest != expected_digest {
        return Err(CanonicalPrekeySelectionError::SelectionDigestMismatch);
    }
    let expected_index = selection_index(&expected_digest, transcript.pool.content.bundles.len());
    if transcript.selected_index != expected_index {
        return Err(CanonicalPrekeySelectionError::SelectedIndexMismatch {
            expected: expected_index,
            actual: transcript.selected_index,
        });
    }
    let expected_sequence = transcript.pool.content.bundles[expected_index as usize]
        .content
        .sequence;
    if transcript.selected_sequence != expected_sequence {
        return Err(CanonicalPrekeySelectionError::SelectedSequenceMismatch {
            expected: expected_sequence,
            actual: transcript.selected_sequence,
        });
    }
    Ok(())
}

pub fn selected_prekey_bundle_v1(
    transcript: &CanonicalPrekeySelectionTranscriptV1,
) -> Result<&CanonicalSignedPrekeyBundleV1, CanonicalPrekeySelectionError> {
    verify_prekey_selection_transcript_v1(transcript)?;
    Ok(&transcript.pool.content.bundles[transcript.selected_index as usize])
}

fn selection_index(digest: &[u8; 32], count: usize) -> u64 {
    let mut prefix = [0_u8; 8];
    prefix.copy_from_slice(&digest[..8]);
    u64::from_be_bytes(prefix) % count as u64
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Vectors {
        schema_version: u64,
        profile: String,
        activation_status: String,
        input: Input,
        expected: Expected,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        initiator_device_id_hex: String,
        signed_prekey_pool_cbor_hex: String,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Expected {
        pool_id_hex: String,
        selection_domain_hex: String,
        selection_input_cbor_hex: String,
        selection_digest_hex: String,
        digest_prefix_u64_be: String,
        selected_index: u64,
        selected_sequence: u64,
        selected_bundle_cbor_hex: String,
        transcript_cbor_hex: String,
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

    fn fixed_hex<const N: usize>(value: &str) -> Result<[u8; N], Box<dyn std::error::Error>> {
        Ok(decode_hex(value)?.try_into().map_err(|_| "wrong length")?)
    }

    #[test]
    fn selection_vector_matches_every_derived_byte() -> Result<(), Box<dyn std::error::Error>> {
        let vectors: Vectors = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-PREKEY-SELECTION-VECTORS.json"
        )))?;
        assert_eq!(vectors.schema_version, 1);
        assert_eq!(vectors.profile, "kilogram-deterministic-cbor-v1");
        assert_eq!(vectors.activation_status, "not-active");
        let initiator = fixed_hex(&vectors.input.initiator_device_id_hex)?;
        let pool = decode_and_verify_signed_prekey_pool_v1(&decode_hex(
            &vectors.input.signed_prekey_pool_cbor_hex,
        )?)?;
        let transcript = derive_prekey_selection_transcript_v1(initiator, pool)?;
        assert_eq!(
            prekey_pool_id_v1(&transcript.pool)?,
            fixed_hex(&vectors.expected.pool_id_hex)?
        );
        assert_eq!(
            CANONICAL_PREKEY_SELECTION_DOMAIN,
            decode_hex(&vectors.expected.selection_domain_hex)?
        );
        assert_eq!(
            prekey_selection_input_v1(
                &transcript.initiator_device_id,
                &prekey_pool_id_v1(&transcript.pool)?
            ),
            decode_hex(&vectors.expected.selection_input_cbor_hex)?
        );
        assert_eq!(
            transcript.selection_digest,
            fixed_hex(&vectors.expected.selection_digest_hex)?
        );
        assert_eq!(
            u64::from_be_bytes(transcript.selection_digest[..8].try_into()?),
            vectors.expected.digest_prefix_u64_be.parse::<u64>()?
        );
        assert_eq!(transcript.selected_index, vectors.expected.selected_index);
        assert_eq!(
            transcript.selected_sequence,
            vectors.expected.selected_sequence
        );
        assert_eq!(
            encode_signed_prekey_bundle_for_test(selected_prekey_bundle_v1(&transcript)?)?,
            decode_hex(&vectors.expected.selected_bundle_cbor_hex)?
        );
        let encoded = encode_prekey_selection_transcript_v1(&transcript)?;
        assert_eq!(encoded, decode_hex(&vectors.expected.transcript_cbor_hex)?);
        assert_eq!(
            decode_and_verify_prekey_selection_transcript_v1(&encoded)?,
            transcript
        );
        Ok(())
    }

    #[test]
    fn selection_transcript_rejects_tampered_derivations() -> Result<(), Box<dyn std::error::Error>>
    {
        let vectors: Vectors = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-PREKEY-SELECTION-VECTORS.json"
        )))?;
        let mut encoded = decode_hex(&vectors.expected.transcript_cbor_hex)?;
        assert_eq!(
            decode_and_verify_prekey_selection_transcript_v1(&encoded)?.selected_sequence,
            vectors.expected.selected_sequence
        );
        let last = encoded.last_mut().ok_or("empty transcript")?;
        *last ^= 1;
        assert!(decode_and_verify_prekey_selection_transcript_v1(&encoded).is_err());
        Ok(())
    }

    fn encode_signed_prekey_bundle_for_test(
        bundle: &CanonicalSignedPrekeyBundleV1,
    ) -> Result<Vec<u8>, CanonicalRatchetError> {
        crate::canonical_ratchet_v1::encode_signed_prekey_bundle_v1(bundle)
    }
}
