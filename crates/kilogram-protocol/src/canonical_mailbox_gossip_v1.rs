//! Inactive deterministic-CBOR blind-mailbox provider discovery artifacts.
//!
//! The active provider registry and gossip frames remain Postcard encoded.
//! This module defines the bounded cross-language target for provider offer
//! identifiers, deterministic selection, authenticated-session gossip frames,
//! and request/reply binding. Installation-local observer and path-domain
//! evidence deliberately remains outside the network artifact.

use std::collections::BTreeSet;

use thiserror::Error;

use crate::{
    canonical_account_v1::{AccountDecoder, CanonicalAccountError},
    canonical_mailbox_v1::{
        CanonicalMailboxError, CanonicalSignedMailboxStorageOfferV1,
        decode_and_verify_signed_mailbox_storage_offer_v1, encode_signed_mailbox_storage_offer_v1,
        mailbox_storage_offer_admission_work_bits_v1, verify_mailbox_storage_offer_at_v1,
        verify_signed_mailbox_storage_offer_v1,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CanonicalWireError, push_array_header, push_bytes,
        push_unsigned, validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_MAILBOX_PROVIDER_SELECTION: u64 = 8;
pub const MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_OFFERS: usize = 8;
pub const MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_HOPS: u64 = 2;
pub const MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_AGE_SECONDS: u64 = 15 * 60;
pub const MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_OFFER_BYTES: usize = 2 * 1024;
pub const MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_FRAME_VALIDITY_SECONDS: u64 = 60;
pub const MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_FRAME_BYTES: usize =
    MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_OFFER_BYTES
        * MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_OFFERS
        + 4 * 1024;
pub const DEFAULT_CANONICAL_MAILBOX_PROVIDER_ADMISSION_WORK_BITS: u64 = 8;

pub const CANONICAL_MAILBOX_PROVIDER_OFFER_ID_DOMAIN: &[u8] =
    b"kilogram:mailbox-provider-offer-id:cbor-v1\0";
pub const CANONICAL_MAILBOX_PROVIDER_SELECTION_DOMAIN: &[u8] =
    b"kilogram:mailbox-provider-selection:cbor-v1\0";
pub const CANONICAL_MAILBOX_PROVIDER_GOSSIP_SELECTION_DOMAIN: &[u8] =
    b"kilogram:mailbox-provider-gossip-selection:cbor-v1\0";
pub const CANONICAL_MAILBOX_PROVIDER_GOSSIP_FRAME_ID_DOMAIN: &[u8] =
    b"kilogram:mailbox-provider-gossip-frame-id:cbor-v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxProviderGossipEntryV1 {
    pub transmitted_hops: u64,
    pub offer: CanonicalSignedMailboxStorageOfferV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxProviderGossipFrameV1 {
    pub created_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub entropy: [u8; 32],
    pub reply_to: Option<[u8; 32]>,
    pub entries: Vec<CanonicalMailboxProviderGossipEntryV1>,
}

/// Local input to deterministic provider selection. The transport identity and
/// retained hop count are authenticated adapter state; neither is serialized
/// into a provider offer or gossip frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxProviderCandidateV1 {
    pub offer: CanonicalSignedMailboxStorageOfferV1,
    pub transport_identity: [u8; 32],
    pub observed_gossip_hops: u64,
}

#[derive(Debug, Error)]
pub enum CanonicalMailboxProviderGossipError {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),
    #[error(transparent)]
    Decode(#[from] CanonicalAccountError),
    #[error(transparent)]
    Mailbox(#[from] CanonicalMailboxError),
    #[error("canonical mailbox provider gossip {object} exceeds {maximum} bytes")]
    ObjectTooLarge {
        object: &'static str,
        maximum: usize,
    },
    #[error("canonical mailbox provider selection size is outside protocol bounds")]
    InvalidSelectionSize,
    #[error("canonical mailbox provider gossip hop count is outside protocol bounds")]
    InvalidHopCount,
    #[error("canonical mailbox provider gossip frame is outside its validity window")]
    InvalidFrameValidity,
    #[error("canonical mailbox provider gossip offer is too old to amplify")]
    StaleOffer,
    #[error("canonical mailbox provider gossip frame repeats a store key")]
    DuplicateStoreKey,
    #[error("canonical mailbox provider candidate set repeats a store key")]
    DuplicateCandidateStoreKey,
    #[error("canonical mailbox provider gossip option array is malformed")]
    InvalidOption,
    #[error("canonical mailbox provider gossip time arithmetic overflowed")]
    TimeOverflow,
    #[error("canonical mailbox provider gossip frame ID does not match the expected transcript")]
    FrameIdMismatch,
}

pub fn mailbox_provider_offer_id_v1(
    offer: &CanonicalSignedMailboxStorageOfferV1,
) -> Result<[u8; 32], CanonicalMailboxProviderGossipError> {
    Ok(domain_hash(
        CANONICAL_MAILBOX_PROVIDER_OFFER_ID_DOMAIN,
        &encode_signed_mailbox_storage_offer_v1(offer)?,
    ))
}

pub fn encode_mailbox_provider_selection_transcript_v1(
    offer: &CanonicalSignedMailboxStorageOfferV1,
    transport_identity: [u8; 32],
    selection_salt: [u8; 32],
) -> Result<Vec<u8>, CanonicalMailboxProviderGossipError> {
    verify_signed_mailbox_storage_offer_v1(offer)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 5);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &selection_salt);
    push_bytes(&mut output, &offer.content.store_key);
    push_bytes(&mut output, &transport_identity);
    push_bytes(&mut output, &mailbox_provider_offer_id_v1(offer)?);
    Ok(output)
}

pub fn mailbox_provider_selection_rank_v1(
    offer: &CanonicalSignedMailboxStorageOfferV1,
    transport_identity: [u8; 32],
    selection_salt: [u8; 32],
) -> Result<[u8; 32], CanonicalMailboxProviderGossipError> {
    Ok(domain_hash(
        CANONICAL_MAILBOX_PROVIDER_SELECTION_DOMAIN,
        &encode_mailbox_provider_selection_transcript_v1(
            offer,
            transport_identity,
            selection_salt,
        )?,
    ))
}

pub fn mailbox_provider_gossip_selection_rank_v1(
    offer: &CanonicalSignedMailboxStorageOfferV1,
    transport_identity: [u8; 32],
    selection_salt: [u8; 32],
) -> Result<[u8; 32], CanonicalMailboxProviderGossipError> {
    Ok(domain_hash(
        CANONICAL_MAILBOX_PROVIDER_GOSSIP_SELECTION_DOMAIN,
        &encode_mailbox_provider_selection_transcript_v1(
            offer,
            transport_identity,
            selection_salt,
        )?,
    ))
}

pub fn select_mailbox_provider_candidates_v1(
    candidates: Vec<CanonicalMailboxProviderCandidateV1>,
    selection_salt: [u8; 32],
    requested: u64,
    now_unix_seconds: u64,
) -> Result<Vec<CanonicalMailboxProviderCandidateV1>, CanonicalMailboxProviderGossipError> {
    validate_requested(requested)?;
    validate_candidates(&candidates, now_unix_seconds)?;
    let mut ranked = candidates
        .into_iter()
        .map(|candidate| {
            Ok((
                mailbox_provider_selection_rank_v1(
                    &candidate.offer,
                    candidate.transport_identity,
                    selection_salt,
                )?,
                candidate,
            ))
        })
        .collect::<Result<Vec<_>, CanonicalMailboxProviderGossipError>>()?;
    ranked.sort_by(|left, right| {
        left.0.cmp(&right.0).then_with(|| {
            left.1
                .offer
                .content
                .store_key
                .cmp(&right.1.offer.content.store_key)
        })
    });
    Ok(select_distinct_transports(ranked, requested))
}

pub fn select_mailbox_provider_gossip_candidates_v1(
    candidates: Vec<CanonicalMailboxProviderCandidateV1>,
    selection_salt: [u8; 32],
    requested: u64,
    now_unix_seconds: u64,
) -> Result<Vec<CanonicalMailboxProviderCandidateV1>, CanonicalMailboxProviderGossipError> {
    validate_requested(requested)?;
    validate_candidates(&candidates, now_unix_seconds)?;
    let mut ranked = candidates
        .into_iter()
        .filter_map(|candidate| {
            let encoded_offer = encode_signed_mailbox_storage_offer_v1(&candidate.offer).ok()?;
            let issued_at = candidate.offer.content.issued_at_unix_seconds;
            let work =
                mailbox_storage_offer_admission_work_bits_v1(&candidate.offer.content).ok()?;
            (candidate.observed_gossip_hops < MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_HOPS
                && encoded_offer.len() <= MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_OFFER_BYTES
                && now_unix_seconds.saturating_sub(issued_at)
                    <= MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_AGE_SECONDS
                && u64::from(work) >= DEFAULT_CANONICAL_MAILBOX_PROVIDER_ADMISSION_WORK_BITS)
                .then_some(candidate)
        })
        .map(|candidate| {
            Ok((
                mailbox_provider_gossip_selection_rank_v1(
                    &candidate.offer,
                    candidate.transport_identity,
                    selection_salt,
                )?,
                candidate,
            ))
        })
        .collect::<Result<Vec<_>, CanonicalMailboxProviderGossipError>>()?;
    ranked.sort_by(|left, right| {
        left.0.cmp(&right.0).then_with(|| {
            left.1
                .offer
                .content
                .store_key
                .cmp(&right.1.offer.content.store_key)
        })
    });
    Ok(select_distinct_transports(ranked, requested))
}

pub fn build_mailbox_provider_gossip_frame_v1(
    candidates: Vec<CanonicalMailboxProviderCandidateV1>,
    entropy: [u8; 32],
    reply_to: Option<[u8; 32]>,
    now_unix_seconds: u64,
) -> Result<CanonicalMailboxProviderGossipFrameV1, CanonicalMailboxProviderGossipError> {
    let selected = select_mailbox_provider_gossip_candidates_v1(
        candidates,
        entropy,
        MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_OFFERS as u64,
        now_unix_seconds,
    )?;
    let frame = CanonicalMailboxProviderGossipFrameV1 {
        created_at_unix_seconds: now_unix_seconds,
        expires_at_unix_seconds: now_unix_seconds
            .checked_add(MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_FRAME_VALIDITY_SECONDS)
            .ok_or(CanonicalMailboxProviderGossipError::TimeOverflow)?,
        entropy,
        reply_to,
        entries: selected
            .into_iter()
            .map(|candidate| CanonicalMailboxProviderGossipEntryV1 {
                transmitted_hops: candidate.observed_gossip_hops + 1,
                offer: candidate.offer,
            })
            .collect(),
    };
    validate_frame(&frame, now_unix_seconds)?;
    Ok(frame)
}

pub fn encode_mailbox_provider_gossip_entry_v1(
    entry: &CanonicalMailboxProviderGossipEntryV1,
) -> Result<Vec<u8>, CanonicalMailboxProviderGossipError> {
    validate_entry(entry)?;
    let encoded_offer = encode_signed_mailbox_storage_offer_v1(&entry.offer)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_unsigned(&mut output, entry.transmitted_hops);
    push_bytes(&mut output, &encoded_offer);
    Ok(output)
}

pub fn encode_mailbox_provider_gossip_frame_v1(
    frame: &CanonicalMailboxProviderGossipFrameV1,
    now_unix_seconds: u64,
) -> Result<Vec<u8>, CanonicalMailboxProviderGossipError> {
    validate_frame(frame, now_unix_seconds)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 6);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_unsigned(&mut output, frame.created_at_unix_seconds);
    push_unsigned(&mut output, frame.expires_at_unix_seconds);
    push_bytes(&mut output, &frame.entropy);
    match frame.reply_to {
        None => push_array_header(&mut output, 0),
        Some(frame_id) => {
            push_array_header(&mut output, 1);
            push_bytes(&mut output, &frame_id);
        }
    }
    push_array_header(&mut output, frame.entries.len() as u64);
    for entry in &frame.entries {
        output.extend_from_slice(&encode_mailbox_provider_gossip_entry_v1(entry)?);
    }
    ensure_bound(
        &output,
        "gossip frame",
        MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_FRAME_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_mailbox_provider_gossip_frame_v1(
    bytes: &[u8],
    now_unix_seconds: u64,
) -> Result<CanonicalMailboxProviderGossipFrameV1, CanonicalMailboxProviderGossipError> {
    ensure_bound(
        bytes,
        "gossip frame",
        MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_FRAME_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("mailbox-provider-gossip-frame-v1", 6)?;
    decoder.expect_version("mailbox-provider-gossip-frame-v1.schema_version")?;
    let created_at_unix_seconds =
        decoder.unsigned("mailbox-provider-gossip-frame-v1.created_at_unix_seconds")?;
    let expires_at_unix_seconds =
        decoder.unsigned("mailbox-provider-gossip-frame-v1.expires_at_unix_seconds")?;
    let entropy = decoder.fixed_bytes("mailbox-provider-gossip-frame-v1.entropy")?;
    let reply_length = decoder.array("mailbox-provider-gossip-frame-v1.reply_to")?;
    let reply_to = match reply_length {
        0 => None,
        1 => Some(decoder.fixed_bytes("mailbox-provider-gossip-frame-v1.reply_to.value")?),
        _ => return Err(CanonicalMailboxProviderGossipError::InvalidOption),
    };
    let entry_count = decoder.array("mailbox-provider-gossip-frame-v1.entries")?;
    if entry_count > MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_OFFERS {
        return Err(CanonicalWireError::TooManyItems {
            field: "mailbox-provider-gossip-frame-v1.entries",
            actual: entry_count,
            maximum: MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_OFFERS,
        }
        .into());
    }
    let mut entries = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        decoder.expect_array("mailbox-provider-gossip-entry-v1", 3)?;
        decoder.expect_version("mailbox-provider-gossip-entry-v1.schema_version")?;
        let transmitted_hops =
            decoder.unsigned("mailbox-provider-gossip-entry-v1.transmitted_hops")?;
        let encoded_offer = decoder
            .bytes("mailbox-provider-gossip-entry-v1.offer")?
            .to_vec();
        ensure_bound(
            &encoded_offer,
            "gossip offer",
            MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_OFFER_BYTES,
        )?;
        let offer = decode_and_verify_signed_mailbox_storage_offer_v1(&encoded_offer)?;
        let entry = CanonicalMailboxProviderGossipEntryV1 {
            transmitted_hops,
            offer,
        };
        validate_entry_at(&entry, now_unix_seconds)?;
        entries.push(entry);
    }
    decoder.finish()?;
    let frame = CanonicalMailboxProviderGossipFrameV1 {
        created_at_unix_seconds,
        expires_at_unix_seconds,
        entropy,
        reply_to,
        entries,
    };
    validate_frame(&frame, now_unix_seconds)?;
    if encode_mailbox_provider_gossip_frame_v1(&frame, now_unix_seconds)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(frame)
}

pub fn mailbox_provider_gossip_frame_id_v1(
    frame: &CanonicalMailboxProviderGossipFrameV1,
) -> Result<[u8; 32], CanonicalMailboxProviderGossipError> {
    Ok(domain_hash(
        CANONICAL_MAILBOX_PROVIDER_GOSSIP_FRAME_ID_DOMAIN,
        &encode_mailbox_provider_gossip_frame_v1(frame, frame.created_at_unix_seconds)?,
    ))
}

pub fn decode_and_verify_mailbox_provider_gossip_frame_for_id_v1(
    bytes: &[u8],
    now_unix_seconds: u64,
    expected_frame_id: [u8; 32],
) -> Result<CanonicalMailboxProviderGossipFrameV1, CanonicalMailboxProviderGossipError> {
    let frame = decode_and_verify_mailbox_provider_gossip_frame_v1(bytes, now_unix_seconds)?;
    if mailbox_provider_gossip_frame_id_v1(&frame)? != expected_frame_id {
        return Err(CanonicalMailboxProviderGossipError::FrameIdMismatch);
    }
    Ok(frame)
}

pub fn verify_mailbox_provider_gossip_reply_v1(
    request: &CanonicalMailboxProviderGossipFrameV1,
    reply: &CanonicalMailboxProviderGossipFrameV1,
) -> Result<(), CanonicalMailboxProviderGossipError> {
    if reply.reply_to != Some(mailbox_provider_gossip_frame_id_v1(request)?) {
        return Err(CanonicalMailboxProviderGossipError::InvalidOption);
    }
    Ok(())
}

fn validate_requested(requested: u64) -> Result<(), CanonicalMailboxProviderGossipError> {
    if !(1..=MAX_CANONICAL_MAILBOX_PROVIDER_SELECTION).contains(&requested) {
        return Err(CanonicalMailboxProviderGossipError::InvalidSelectionSize);
    }
    Ok(())
}

fn validate_candidates(
    candidates: &[CanonicalMailboxProviderCandidateV1],
    now_unix_seconds: u64,
) -> Result<(), CanonicalMailboxProviderGossipError> {
    let mut store_keys = BTreeSet::new();
    for candidate in candidates {
        verify_mailbox_storage_offer_at_v1(&candidate.offer, now_unix_seconds)?;
        if candidate.observed_gossip_hops > MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_HOPS {
            return Err(CanonicalMailboxProviderGossipError::InvalidHopCount);
        }
        if !store_keys.insert(candidate.offer.content.store_key) {
            return Err(CanonicalMailboxProviderGossipError::DuplicateCandidateStoreKey);
        }
    }
    Ok(())
}

fn select_distinct_transports(
    ranked: Vec<([u8; 32], CanonicalMailboxProviderCandidateV1)>,
    requested: u64,
) -> Vec<CanonicalMailboxProviderCandidateV1> {
    let mut identities = BTreeSet::new();
    let mut selected = Vec::new();
    for (_, candidate) in ranked {
        if identities.insert(candidate.transport_identity) {
            selected.push(candidate);
            if selected.len() as u64 == requested {
                break;
            }
        }
    }
    selected
}

fn validate_entry(
    entry: &CanonicalMailboxProviderGossipEntryV1,
) -> Result<(), CanonicalMailboxProviderGossipError> {
    if !(1..=MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_HOPS).contains(&entry.transmitted_hops) {
        return Err(CanonicalMailboxProviderGossipError::InvalidHopCount);
    }
    verify_signed_mailbox_storage_offer_v1(&entry.offer)?;
    ensure_bound(
        &encode_signed_mailbox_storage_offer_v1(&entry.offer)?,
        "gossip offer",
        MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_OFFER_BYTES,
    )?;
    Ok(())
}

fn validate_entry_at(
    entry: &CanonicalMailboxProviderGossipEntryV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalMailboxProviderGossipError> {
    validate_entry(entry)?;
    verify_mailbox_storage_offer_at_v1(&entry.offer, now_unix_seconds)?;
    if now_unix_seconds.saturating_sub(entry.offer.content.issued_at_unix_seconds)
        > MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_AGE_SECONDS
    {
        return Err(CanonicalMailboxProviderGossipError::StaleOffer);
    }
    Ok(())
}

fn validate_frame(
    frame: &CanonicalMailboxProviderGossipFrameV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalMailboxProviderGossipError> {
    if frame.created_at_unix_seconds > now_unix_seconds
        || now_unix_seconds >= frame.expires_at_unix_seconds
        || frame
            .expires_at_unix_seconds
            .checked_sub(frame.created_at_unix_seconds)
            .is_none_or(|validity| {
                validity == 0
                    || validity > MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_FRAME_VALIDITY_SECONDS
            })
    {
        return Err(CanonicalMailboxProviderGossipError::InvalidFrameValidity);
    }
    if frame.entries.len() > MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_OFFERS {
        return Err(CanonicalWireError::TooManyItems {
            field: "mailbox-provider-gossip-frame-v1.entries",
            actual: frame.entries.len(),
            maximum: MAX_CANONICAL_MAILBOX_PROVIDER_GOSSIP_OFFERS,
        }
        .into());
    }
    let mut store_keys = BTreeSet::new();
    for entry in &frame.entries {
        validate_entry_at(entry, now_unix_seconds)?;
        if !store_keys.insert(entry.offer.content.store_key) {
            return Err(CanonicalMailboxProviderGossipError::DuplicateStoreKey);
        }
    }
    Ok(())
}

fn ensure_bound(
    bytes: &[u8],
    object: &'static str,
    maximum: usize,
) -> Result<(), CanonicalMailboxProviderGossipError> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(CanonicalMailboxProviderGossipError::ObjectTooLarge { object, maximum });
    }
    Ok(())
}

fn domain_hash(domain: &[u8], value: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(value);
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::Value;

    use super::*;
    use crate::canonical_mailbox_v1::{
        CANONICAL_MAILBOX_STORAGE_OFFER_DOMAIN, CanonicalMailboxStorageOfferContentV1,
        CanonicalMailboxStoragePolicyClassV1, mailbox_storage_offer_signing_bytes_v1,
    };

    fn offer(
        seed: u8,
        endpoint: u8,
        issued_at: u64,
    ) -> Result<CanonicalSignedMailboxStorageOfferV1, Box<dyn Error>> {
        let signing = SigningKey::from_bytes(&[seed; 32]);
        let mut content = CanonicalMailboxStorageOfferContentV1 {
            provider_endpoint: vec![endpoint; 16],
            store_key: signing.verifying_key().to_bytes(),
            policy_class: CanonicalMailboxStoragePolicyClassV1::BoundedVolunteer,
            capacity_hint_bytes: 200 * 1024 * 1024,
            max_record_bytes: 1024 * 1024,
            issued_at_unix_seconds: issued_at,
            expires_at_unix_seconds: issued_at + 3_600,
            nonce: [0; 32],
        };
        while u64::from(mailbox_storage_offer_admission_work_bits_v1(&content)?)
            < DEFAULT_CANONICAL_MAILBOX_PROVIDER_ADMISSION_WORK_BITS
        {
            for byte in content.nonce.iter_mut().rev() {
                *byte = byte.wrapping_add(1);
                if *byte != 0 {
                    break;
                }
            }
        }
        let signature = signing
            .sign(&mailbox_storage_offer_signing_bytes_v1(&content)?)
            .to_bytes();
        assert_eq!(
            CANONICAL_MAILBOX_STORAGE_OFFER_DOMAIN,
            b"kilogram:blind-mailbox-storage-offer:cbor-v1\0"
        );
        Ok(CanonicalSignedMailboxStorageOfferV1 { content, signature })
    }

    fn candidate(
        seed: u8,
        endpoint: u8,
        hops: u64,
    ) -> Result<CanonicalMailboxProviderCandidateV1, Box<dyn Error>> {
        Ok(CanonicalMailboxProviderCandidateV1 {
            offer: offer(seed, endpoint, 1_800_000_000)?,
            transport_identity: [endpoint; 32],
            observed_gossip_hops: hops,
        })
    }

    fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        if !value.len().is_multiple_of(2) {
            return Err("hex value has odd length".into());
        }
        (0..value.len())
            .step_by(2)
            .map(|index| Ok(u8::from_str_radix(&value[index..index + 2], 16)?))
            .collect::<Result<Vec<_>, Box<dyn Error>>>()
    }

    fn pointer_hex(document: &Value, pointer: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        decode_hex(
            document
                .pointer(pointer)
                .and_then(Value::as_str)
                .ok_or("vector JSON pointer is missing")?,
        )
    }

    fn fixed<const N: usize>(bytes: Vec<u8>) -> Result<[u8; N], Box<dyn Error>> {
        bytes
            .try_into()
            .map_err(|_| "vector byte string length changed".into())
    }

    #[test]
    fn golden_gossip_vectors_match_exact_bytes_ids_and_ranks() -> Result<(), Box<dyn Error>> {
        let vectors: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-MAILBOX-GOSSIP-VECTORS.json"
        )))?;
        let mailbox: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-MAILBOX-VECTORS.json"
        )))?;
        let offer = decode_and_verify_signed_mailbox_storage_offer_v1(&pointer_hex(
            &mailbox,
            "/expected/storage_offer/signed_cbor_hex",
        )?)?;
        let transport_identity = fixed(pointer_hex(&vectors, "/input/transport_identity_hex")?)?;
        let selection_salt = fixed(pointer_hex(&vectors, "/input/selection_salt_hex")?)?;
        assert_eq!(
            mailbox_provider_offer_id_v1(&offer)?.to_vec(),
            pointer_hex(&vectors, "/expected/offer_id_hex")?
        );
        assert_eq!(
            encode_mailbox_provider_selection_transcript_v1(
                &offer,
                transport_identity,
                selection_salt,
            )?,
            pointer_hex(&vectors, "/expected/selection_transcript_cbor_hex")?
        );
        assert_eq!(
            mailbox_provider_selection_rank_v1(&offer, transport_identity, selection_salt)?
                .to_vec(),
            pointer_hex(&vectors, "/expected/selection_rank_hex")?
        );
        assert_eq!(
            mailbox_provider_gossip_selection_rank_v1(&offer, transport_identity, selection_salt,)?
                .to_vec(),
            pointer_hex(&vectors, "/expected/gossip_selection_rank_hex")?
        );

        let request_time = vectors["input"]["request_created_at"]
            .as_u64()
            .ok_or("request time is missing")?;
        let request_bytes = pointer_hex(&vectors, "/expected/request/frame_cbor_hex")?;
        let request =
            decode_and_verify_mailbox_provider_gossip_frame_v1(&request_bytes, request_time)?;
        assert_eq!(
            encode_mailbox_provider_gossip_frame_v1(&request, request_time)?,
            request_bytes
        );
        assert_eq!(
            mailbox_provider_gossip_frame_id_v1(&request)?.to_vec(),
            pointer_hex(&vectors, "/expected/request/frame_id_hex")?
        );

        let reply_time = vectors["input"]["reply_created_at"]
            .as_u64()
            .ok_or("reply time is missing")?;
        let reply_bytes = pointer_hex(&vectors, "/expected/reply/frame_cbor_hex")?;
        let reply = decode_and_verify_mailbox_provider_gossip_frame_v1(&reply_bytes, reply_time)?;
        verify_mailbox_provider_gossip_reply_v1(&request, &reply)?;
        assert_eq!(
            mailbox_provider_gossip_frame_id_v1(&reply)?.to_vec(),
            pointer_hex(&vectors, "/expected/reply/frame_id_hex")?
        );
        Ok(())
    }

    #[test]
    fn frame_round_trip_and_reply_binding() -> Result<(), Box<dyn Error>> {
        let now = 1_800_000_030;
        let request = build_mailbox_provider_gossip_frame_v1(
            vec![candidate(1, 11, 0)?, candidate(2, 22, 1)?],
            [31; 32],
            None,
            now,
        )?;
        let encoded = encode_mailbox_provider_gossip_frame_v1(&request, now)?;
        assert_eq!(
            decode_and_verify_mailbox_provider_gossip_frame_v1(&encoded, now)?,
            request
        );
        let reply = build_mailbox_provider_gossip_frame_v1(
            vec![candidate(3, 33, 0)?],
            [41; 32],
            Some(mailbox_provider_gossip_frame_id_v1(&request)?),
            now,
        )?;
        verify_mailbox_provider_gossip_reply_v1(&request, &reply)?;
        assert!(decode_and_verify_mailbox_provider_gossip_frame_v1(&encoded, now + 60).is_err());
        Ok(())
    }

    #[test]
    fn selection_is_deterministic_and_transport_distinct() -> Result<(), Box<dyn Error>> {
        let now = 1_800_000_030;
        let mut duplicate_transport = candidate(3, 33, 0)?;
        duplicate_transport.transport_identity = [11; 32];
        let candidates = vec![
            candidate(1, 11, 0)?,
            candidate(2, 22, 0)?,
            duplicate_transport,
        ];
        let first = select_mailbox_provider_candidates_v1(candidates.clone(), [7; 32], 3, now)?;
        let second = select_mailbox_provider_candidates_v1(candidates, [7; 32], 3, now)?;
        assert_eq!(first, second);
        assert_eq!(first.len(), 2);
        assert_ne!(first[0].transport_identity, first[1].transport_identity);
        Ok(())
    }

    #[test]
    fn malformed_hops_duplicates_and_reply_are_rejected() -> Result<(), Box<dyn Error>> {
        let now = 1_800_000_030;
        let offer = offer(4, 44, 1_800_000_000)?;
        let duplicate = CanonicalMailboxProviderGossipFrameV1 {
            created_at_unix_seconds: now,
            expires_at_unix_seconds: now + 60,
            entropy: [5; 32],
            reply_to: None,
            entries: vec![
                CanonicalMailboxProviderGossipEntryV1 {
                    transmitted_hops: 1,
                    offer: offer.clone(),
                },
                CanonicalMailboxProviderGossipEntryV1 {
                    transmitted_hops: 2,
                    offer,
                },
            ],
        };
        assert!(encode_mailbox_provider_gossip_frame_v1(&duplicate, now).is_err());
        let request =
            build_mailbox_provider_gossip_frame_v1(vec![candidate(5, 55, 0)?], [8; 32], None, now)?;
        let unrelated = build_mailbox_provider_gossip_frame_v1(
            vec![candidate(6, 66, 0)?],
            [9; 32],
            Some([0; 32]),
            now,
        )?;
        assert!(verify_mailbox_provider_gossip_reply_v1(&request, &unrelated).is_err());
        Ok(())
    }
}
