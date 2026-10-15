//! Inactive deterministic-CBOR recipient-bound mailbox provisioning.
//!
//! The active provisioning crate remains Postcard v1/v2. This module defines
//! one mode-tagged migration target for legacy HTTPS and exact-volunteer
//! bindings, offers, replica commitments, ordered updates and recipient ACKs.

use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use thiserror::Error;
use url::Url;

use crate::{
    canonical_account_v1::{AccountDecoder, CanonicalAccountError},
    canonical_mailbox_v1::{
        CanonicalMailboxAddressV1, CanonicalMailboxError, decode_and_verify_mailbox_address_v1,
        encode_mailbox_address_v1,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
        push_unsigned, validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_MAILBOX_PROVISIONING_SERVICE_URL_BYTES: usize = 2_048;
pub const MAX_CANONICAL_MAILBOX_PROVISIONING_SEALED_CIPHERTEXT_BYTES: usize = 16 * 1024;
pub const MAX_CANONICAL_MAILBOX_CAPABILITY_UPDATE_BYTES: usize = 32 * 1024;
pub const MAX_CANONICAL_MAILBOX_CAPABILITY_ACKNOWLEDGEMENT_BYTES: usize = 4 * 1024;
pub const MIN_CANONICAL_MAILBOX_REPLICA_STORES: usize = 2;
pub const MAX_CANONICAL_MAILBOX_REPLICA_STORES: usize = 8;
pub const MIN_CANONICAL_MAILBOX_OFFER_VALIDITY_SECONDS: u64 = 60;
pub const MAX_CANONICAL_MAILBOX_OFFER_VALIDITY_SECONDS: u64 = 30 * 24 * 60 * 60;

pub const CANONICAL_MAILBOX_BINDING_ID_DOMAIN: &[u8] = b"kilogram:mailbox-binding-id:cbor-v1\0";
pub const CANONICAL_MAILBOX_LOCAL_BINDING_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:mailbox-local-binding:cbor-v1\0";
pub const CANONICAL_MAILBOX_LOCAL_BINDING_HPKE_INFO: &[u8] =
    b"kilogram:mailbox-local-binding-hpke:cbor-v1\0";
pub const CANONICAL_MAILBOX_PEER_OFFER_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:mailbox-peer-offer:cbor-v1\0";
pub const CANONICAL_MAILBOX_PEER_OFFER_HPKE_INFO: &[u8] =
    b"kilogram:mailbox-peer-offer-hpke:cbor-v1\0";
pub const CANONICAL_MAILBOX_REPLICA_SET_COMMITMENT_ID_DOMAIN: &[u8] =
    b"kilogram:mailbox-replica-set-commitment-id:cbor-v1\0";
pub const CANONICAL_MAILBOX_CAPABILITY_UPDATE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:mailbox-capability-update:cbor-v1\0";
pub const CANONICAL_MAILBOX_CAPABILITY_UPDATE_ID_DOMAIN: &[u8] =
    b"kilogram:mailbox-capability-update-id:cbor-v1\0";
pub const CANONICAL_MAILBOX_CAPABILITY_ACKNOWLEDGEMENT_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:mailbox-capability-acknowledgement:cbor-v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum CanonicalMailboxProvisioningModeV1 {
    LegacyHttps = 1,
    ExactVolunteer = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum CanonicalMailboxProvisioningEnvelopePurposeV1 {
    OwnerLocalBinding = 1,
    RecipientPeerOffer = 2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxServiceV1 {
    pub normalized_https_url: String,
    pub expected_store_key: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxProvisioningCoreV1 {
    pub mode: CanonicalMailboxProvisioningModeV1,
    pub owner_account_id: [u8; 32],
    pub owner_device_id: [u8; 32],
    pub peer_account_id: [u8; 32],
    pub peer_device_id: [u8; 32],
    pub scope: [u8; 32],
    pub address: CanonicalMailboxAddressV1,
    pub service: Option<CanonicalMailboxServiceV1>,
    pub created_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedLocalMailboxBindingV1 {
    pub core: CanonicalMailboxProvisioningCoreV1,
    pub read_secret: [u8; 32],
    pub write_secret: [u8; 32],
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedMailboxPeerOfferV1 {
    pub core: CanonicalMailboxProvisioningCoreV1,
    pub owner_encryption_public_key: [u8; 32],
    pub write_secret: [u8; 32],
    pub expires_at_unix_seconds: u64,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSealedMailboxProvisioningEnvelopeV1 {
    pub purpose: CanonicalMailboxProvisioningEnvelopePurposeV1,
    pub binding_id: [u8; 32],
    pub encapsulated_key: [u8; 32],
    pub ciphertext: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxReplicaSetV1 {
    pub store_keys: Vec<[u8; 32]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalMailboxCapabilityActionV1 {
    Activate {
        mode: CanonicalMailboxProvisioningModeV1,
        binding_id: [u8; 32],
        sealed_peer_offer: CanonicalSealedMailboxProvisioningEnvelopeV1,
        replica_set: Option<CanonicalMailboxReplicaSetV1>,
    },
    Revoke {
        binding_id: [u8; 32],
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxCapabilityUpdateContentV1 {
    pub owner_account_id: [u8; 32],
    pub owner_device_id: [u8; 32],
    pub recipient_account_id: [u8; 32],
    pub recipient_device_id: [u8; 32],
    pub scope: [u8; 32],
    pub generation: u64,
    pub previous_update_id: Option<[u8; 32]>,
    pub created_at_unix_seconds: u64,
    pub action: CanonicalMailboxCapabilityActionV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedMailboxCapabilityUpdateV1 {
    pub content: CanonicalMailboxCapabilityUpdateContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxCapabilityAcknowledgementContentV1 {
    pub session_binding: [u8; 32],
    pub update_id: [u8; 32],
    pub owner_account_id: [u8; 32],
    pub owner_device_id: [u8; 32],
    pub recipient_account_id: [u8; 32],
    pub recipient_device_id: [u8; 32],
    pub generation: u64,
    pub binding_id: [u8; 32],
    pub revoked: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedMailboxCapabilityAcknowledgementV1 {
    pub content: CanonicalMailboxCapabilityAcknowledgementContentV1,
    pub signature: [u8; 64],
}

#[derive(Debug, Error)]
pub enum CanonicalMailboxProvisioningError {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),
    #[error(transparent)]
    Decode(#[from] CanonicalAccountError),
    #[error(transparent)]
    Mailbox(#[from] CanonicalMailboxError),
    #[error("canonical mailbox provisioning {object} exceeds {maximum} bytes")]
    ObjectTooLarge {
        object: &'static str,
        maximum: usize,
    },
    #[error("canonical mailbox provisioning contains unknown mode {0}")]
    UnknownMode(u64),
    #[error("canonical mailbox provisioning contains unknown envelope purpose {0}")]
    UnknownEnvelopePurpose(u64),
    #[error("canonical mailbox provisioning contains unknown action {0}")]
    UnknownAction(u64),
    #[error("canonical mailbox provisioning option array is malformed")]
    InvalidOption,
    #[error("canonical mailbox provisioning service is invalid")]
    InvalidService,
    #[error("canonical mailbox provisioning core is invalid")]
    InvalidCore,
    #[error("canonical mailbox provisioning secret does not match its address")]
    SecretMismatch,
    #[error("canonical mailbox provisioning signature is invalid")]
    InvalidSignature,
    #[error("canonical mailbox provisioning envelope is invalid")]
    InvalidEnvelope,
    #[error("canonical mailbox provisioning replica set is invalid")]
    InvalidReplicaSet,
    #[error("canonical mailbox provisioning update is invalid")]
    InvalidUpdate,
    #[error("canonical mailbox provisioning update chain is invalid")]
    InvalidUpdateChain,
    #[error("canonical mailbox provisioning acknowledgement is invalid")]
    InvalidAcknowledgement,
}

pub fn encode_mailbox_provisioning_core_v1(
    core: &CanonicalMailboxProvisioningCoreV1,
) -> Result<Vec<u8>, CanonicalMailboxProvisioningError> {
    validate_core(core)?;
    let address = encode_mailbox_address_v1(&core.address)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 10);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_unsigned(&mut output, core.mode as u64);
    push_bytes(&mut output, &core.owner_account_id);
    push_bytes(&mut output, &core.owner_device_id);
    push_bytes(&mut output, &core.peer_account_id);
    push_bytes(&mut output, &core.peer_device_id);
    push_bytes(&mut output, &core.scope);
    push_bytes(&mut output, &address);
    encode_service_option(core.service.as_ref(), &mut output)?;
    push_unsigned(&mut output, core.created_at_unix_seconds);
    Ok(output)
}

pub fn decode_and_verify_mailbox_provisioning_core_v1(
    bytes: &[u8],
) -> Result<CanonicalMailboxProvisioningCoreV1, CanonicalMailboxProvisioningError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("mailbox-provisioning-core-v1", 10)?;
    decoder.expect_version("mailbox-provisioning-core-v1.schema_version")?;
    let core = CanonicalMailboxProvisioningCoreV1 {
        mode: decode_mode(decoder.unsigned("mailbox-provisioning-core-v1.mode")?)?,
        owner_account_id: decoder.fixed_bytes("mailbox-provisioning-core-v1.owner_account_id")?,
        owner_device_id: decoder.fixed_bytes("mailbox-provisioning-core-v1.owner_device_id")?,
        peer_account_id: decoder.fixed_bytes("mailbox-provisioning-core-v1.peer_account_id")?,
        peer_device_id: decoder.fixed_bytes("mailbox-provisioning-core-v1.peer_device_id")?,
        scope: decoder.fixed_bytes("mailbox-provisioning-core-v1.scope")?,
        address: decode_and_verify_mailbox_address_v1(
            decoder.bytes("mailbox-provisioning-core-v1.address")?,
        )?,
        service: decode_service_option(&mut decoder)?,
        created_at_unix_seconds: decoder
            .unsigned("mailbox-provisioning-core-v1.created_at_unix_seconds")?,
    };
    decoder.finish()?;
    validate_core(&core)?;
    if encode_mailbox_provisioning_core_v1(&core)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(core)
}

pub fn mailbox_binding_id_v1(
    core: &CanonicalMailboxProvisioningCoreV1,
) -> Result<[u8; 32], CanonicalMailboxProvisioningError> {
    Ok(domain_hash(
        CANONICAL_MAILBOX_BINDING_ID_DOMAIN,
        &encode_mailbox_provisioning_core_v1(core)?,
    ))
}

pub fn encode_signed_local_mailbox_binding_v1(
    binding: &CanonicalSignedLocalMailboxBindingV1,
) -> Result<Vec<u8>, CanonicalMailboxProvisioningError> {
    verify_signed_local_mailbox_binding_v1(binding)?;
    let content = encode_local_binding_content(binding)?;
    Ok(encode_signed(&content, &binding.signature))
}

pub fn decode_and_verify_signed_local_mailbox_binding_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedLocalMailboxBindingV1, CanonicalMailboxProvisioningError> {
    ensure_bound(
        bytes,
        "signed local binding",
        MAX_CANONICAL_MAILBOX_PROVISIONING_SEALED_CIPHERTEXT_BYTES,
    )?;
    let (content, signature) = decode_signed(bytes, "signed-local-mailbox-binding-v1")?;
    let mut decoder = AccountDecoder::new(content);
    decoder.expect_array("local-mailbox-binding-content-v1", 4)?;
    decoder.expect_version("local-mailbox-binding-content-v1.schema_version")?;
    let binding = CanonicalSignedLocalMailboxBindingV1 {
        core: decode_and_verify_mailbox_provisioning_core_v1(
            decoder.bytes("local-mailbox-binding-content-v1.core")?,
        )?,
        read_secret: decoder.fixed_bytes("local-mailbox-binding-content-v1.read_secret")?,
        write_secret: decoder.fixed_bytes("local-mailbox-binding-content-v1.write_secret")?,
        signature,
    };
    decoder.finish()?;
    verify_signed_local_mailbox_binding_v1(&binding)?;
    if encode_signed_local_mailbox_binding_v1(&binding)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(binding)
}

pub fn verify_signed_local_mailbox_binding_v1(
    binding: &CanonicalSignedLocalMailboxBindingV1,
) -> Result<(), CanonicalMailboxProvisioningError> {
    validate_core(&binding.core)?;
    if SigningKey::from_bytes(&binding.read_secret)
        .verifying_key()
        .to_bytes()
        != binding.core.address.read_key
        || SigningKey::from_bytes(&binding.write_secret)
            .verifying_key()
            .to_bytes()
            != binding.core.address.write_key
    {
        return Err(CanonicalMailboxProvisioningError::SecretMismatch);
    }
    verify_ed25519(
        &binding.core.owner_device_id,
        &prefixed(
            CANONICAL_MAILBOX_LOCAL_BINDING_SIGNATURE_DOMAIN,
            &encode_local_binding_content(binding)?,
        ),
        &binding.signature,
    )
}

pub fn encode_signed_mailbox_peer_offer_v1(
    offer: &CanonicalSignedMailboxPeerOfferV1,
) -> Result<Vec<u8>, CanonicalMailboxProvisioningError> {
    verify_signed_mailbox_peer_offer_v1(offer)?;
    let content = encode_peer_offer_content(offer)?;
    Ok(encode_signed(&content, &offer.signature))
}

pub fn decode_and_verify_signed_mailbox_peer_offer_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedMailboxPeerOfferV1, CanonicalMailboxProvisioningError> {
    ensure_bound(
        bytes,
        "signed peer offer",
        MAX_CANONICAL_MAILBOX_PROVISIONING_SEALED_CIPHERTEXT_BYTES,
    )?;
    let (content, signature) = decode_signed(bytes, "signed-mailbox-peer-offer-v1")?;
    let mut decoder = AccountDecoder::new(content);
    decoder.expect_array("mailbox-peer-offer-content-v1", 5)?;
    decoder.expect_version("mailbox-peer-offer-content-v1.schema_version")?;
    let offer = CanonicalSignedMailboxPeerOfferV1 {
        core: decode_and_verify_mailbox_provisioning_core_v1(
            decoder.bytes("mailbox-peer-offer-content-v1.core")?,
        )?,
        owner_encryption_public_key: decoder
            .fixed_bytes("mailbox-peer-offer-content-v1.owner_encryption_public_key")?,
        write_secret: decoder.fixed_bytes("mailbox-peer-offer-content-v1.write_secret")?,
        expires_at_unix_seconds: decoder
            .unsigned("mailbox-peer-offer-content-v1.expires_at_unix_seconds")?,
        signature,
    };
    decoder.finish()?;
    verify_signed_mailbox_peer_offer_v1(&offer)?;
    if encode_signed_mailbox_peer_offer_v1(&offer)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(offer)
}

pub fn verify_signed_mailbox_peer_offer_v1(
    offer: &CanonicalSignedMailboxPeerOfferV1,
) -> Result<(), CanonicalMailboxProvisioningError> {
    validate_core(&offer.core)?;
    if SigningKey::from_bytes(&offer.write_secret)
        .verifying_key()
        .to_bytes()
        != offer.core.address.write_key
        || offer
            .expires_at_unix_seconds
            .checked_sub(offer.core.created_at_unix_seconds)
            .is_none_or(|validity| {
                !(MIN_CANONICAL_MAILBOX_OFFER_VALIDITY_SECONDS
                    ..=MAX_CANONICAL_MAILBOX_OFFER_VALIDITY_SECONDS)
                    .contains(&validity)
            })
    {
        return Err(CanonicalMailboxProvisioningError::SecretMismatch);
    }
    verify_ed25519(
        &offer.core.owner_device_id,
        &prefixed(
            CANONICAL_MAILBOX_PEER_OFFER_SIGNATURE_DOMAIN,
            &encode_peer_offer_content(offer)?,
        ),
        &offer.signature,
    )
}

pub fn mailbox_provisioning_envelope_aad_v1(
    purpose: CanonicalMailboxProvisioningEnvelopePurposeV1,
    binding_id: [u8; 32],
) -> Vec<u8> {
    encode_envelope_header(purpose, binding_id)
}

pub fn mailbox_provisioning_envelope_hpke_info_v1(
    purpose: CanonicalMailboxProvisioningEnvelopePurposeV1,
) -> &'static [u8] {
    match purpose {
        CanonicalMailboxProvisioningEnvelopePurposeV1::OwnerLocalBinding => {
            CANONICAL_MAILBOX_LOCAL_BINDING_HPKE_INFO
        }
        CanonicalMailboxProvisioningEnvelopePurposeV1::RecipientPeerOffer => {
            CANONICAL_MAILBOX_PEER_OFFER_HPKE_INFO
        }
    }
}

pub fn encode_sealed_mailbox_provisioning_envelope_v1(
    envelope: &CanonicalSealedMailboxProvisioningEnvelopeV1,
) -> Result<Vec<u8>, CanonicalMailboxProvisioningError> {
    validate_envelope(envelope)?;
    let header = encode_envelope_header(envelope.purpose, envelope.binding_id);
    let mut output = Vec::new();
    push_array_header(&mut output, 4);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &header);
    push_bytes(&mut output, &envelope.encapsulated_key);
    push_bytes(&mut output, &envelope.ciphertext);
    ensure_bound(
        &output,
        "sealed envelope",
        MAX_CANONICAL_MAILBOX_PROVISIONING_SEALED_CIPHERTEXT_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_sealed_mailbox_provisioning_envelope_v1(
    bytes: &[u8],
) -> Result<CanonicalSealedMailboxProvisioningEnvelopeV1, CanonicalMailboxProvisioningError> {
    ensure_bound(
        bytes,
        "sealed envelope",
        MAX_CANONICAL_MAILBOX_PROVISIONING_SEALED_CIPHERTEXT_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("sealed-mailbox-provisioning-envelope-v1", 4)?;
    decoder.expect_version("sealed-mailbox-provisioning-envelope-v1.schema_version")?;
    let (purpose, binding_id) =
        decode_envelope_header(decoder.bytes("sealed-mailbox-provisioning-envelope-v1.header")?)?;
    let envelope = CanonicalSealedMailboxProvisioningEnvelopeV1 {
        purpose,
        binding_id,
        encapsulated_key: decoder
            .fixed_bytes("sealed-mailbox-provisioning-envelope-v1.encapsulated_key")?,
        ciphertext: decoder
            .bytes("sealed-mailbox-provisioning-envelope-v1.ciphertext")?
            .to_vec(),
    };
    decoder.finish()?;
    validate_envelope(&envelope)?;
    if encode_sealed_mailbox_provisioning_envelope_v1(&envelope)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(envelope)
}

pub fn encode_mailbox_replica_set_v1(
    replica_set: &CanonicalMailboxReplicaSetV1,
) -> Result<Vec<u8>, CanonicalMailboxProvisioningError> {
    validate_replica_set(replica_set)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 2);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_array_header(&mut output, replica_set.store_keys.len() as u64);
    for store_key in &replica_set.store_keys {
        push_bytes(&mut output, store_key);
    }
    Ok(output)
}

pub fn decode_and_verify_mailbox_replica_set_v1(
    bytes: &[u8],
) -> Result<CanonicalMailboxReplicaSetV1, CanonicalMailboxProvisioningError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("mailbox-replica-set-v1", 2)?;
    decoder.expect_version("mailbox-replica-set-v1.schema_version")?;
    let count = decoder.array("mailbox-replica-set-v1.store_keys")?;
    if !(MIN_CANONICAL_MAILBOX_REPLICA_STORES..=MAX_CANONICAL_MAILBOX_REPLICA_STORES)
        .contains(&count)
    {
        return Err(CanonicalMailboxProvisioningError::InvalidReplicaSet);
    }
    let mut store_keys = Vec::with_capacity(count);
    for _ in 0..count {
        store_keys.push(decoder.fixed_bytes("mailbox-replica-set-v1.store_key")?);
    }
    decoder.finish()?;
    let replica_set = CanonicalMailboxReplicaSetV1 { store_keys };
    validate_replica_set(&replica_set)?;
    if encode_mailbox_replica_set_v1(&replica_set)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(replica_set)
}

pub fn mailbox_replica_set_commitment_id_v1(
    binding_id: [u8; 32],
    replica_set: &CanonicalMailboxReplicaSetV1,
) -> Result<[u8; 32], CanonicalMailboxProvisioningError> {
    let replica_set = encode_mailbox_replica_set_v1(replica_set)?;
    let mut transcript = Vec::new();
    push_array_header(&mut transcript, 3);
    push_unsigned(&mut transcript, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut transcript, &binding_id);
    push_bytes(&mut transcript, &replica_set);
    Ok(domain_hash(
        CANONICAL_MAILBOX_REPLICA_SET_COMMITMENT_ID_DOMAIN,
        &transcript,
    ))
}

pub fn encode_signed_mailbox_capability_update_v1(
    update: &CanonicalSignedMailboxCapabilityUpdateV1,
) -> Result<Vec<u8>, CanonicalMailboxProvisioningError> {
    verify_signed_mailbox_capability_update_v1(update)?;
    let content = encode_capability_update_content(&update.content)?;
    let encoded = encode_signed(&content, &update.signature);
    ensure_bound(
        &encoded,
        "capability update",
        MAX_CANONICAL_MAILBOX_CAPABILITY_UPDATE_BYTES,
    )?;
    Ok(encoded)
}

pub fn decode_and_verify_signed_mailbox_capability_update_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedMailboxCapabilityUpdateV1, CanonicalMailboxProvisioningError> {
    ensure_bound(
        bytes,
        "capability update",
        MAX_CANONICAL_MAILBOX_CAPABILITY_UPDATE_BYTES,
    )?;
    let (content, signature) = decode_signed(bytes, "signed-mailbox-capability-update-v1")?;
    let update = CanonicalSignedMailboxCapabilityUpdateV1 {
        content: decode_capability_update_content(content)?,
        signature,
    };
    verify_signed_mailbox_capability_update_v1(&update)?;
    if encode_signed_mailbox_capability_update_v1(&update)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(update)
}

pub fn verify_signed_mailbox_capability_update_v1(
    update: &CanonicalSignedMailboxCapabilityUpdateV1,
) -> Result<(), CanonicalMailboxProvisioningError> {
    validate_update_content(&update.content)?;
    verify_ed25519(
        &update.content.owner_device_id,
        &prefixed(
            CANONICAL_MAILBOX_CAPABILITY_UPDATE_SIGNATURE_DOMAIN,
            &encode_capability_update_content(&update.content)?,
        ),
        &update.signature,
    )
}

pub fn mailbox_capability_update_id_v1(
    update: &CanonicalSignedMailboxCapabilityUpdateV1,
) -> Result<[u8; 32], CanonicalMailboxProvisioningError> {
    Ok(domain_hash(
        CANONICAL_MAILBOX_CAPABILITY_UPDATE_ID_DOMAIN,
        &encode_signed_mailbox_capability_update_v1(update)?,
    ))
}

pub fn verify_mailbox_capability_update_chain_link_v1(
    previous: &CanonicalSignedMailboxCapabilityUpdateV1,
    next: &CanonicalSignedMailboxCapabilityUpdateV1,
) -> Result<(), CanonicalMailboxProvisioningError> {
    verify_signed_mailbox_capability_update_v1(previous)?;
    verify_signed_mailbox_capability_update_v1(next)?;
    let previous_binding = action_binding_id(&previous.content.action);
    let next_binding = action_binding_id(&next.content.action);
    let identity_matches = previous.content.owner_account_id == next.content.owner_account_id
        && previous.content.owner_device_id == next.content.owner_device_id
        && previous.content.recipient_account_id == next.content.recipient_account_id
        && previous.content.recipient_device_id == next.content.recipient_device_id
        && previous.content.scope == next.content.scope;
    let action_matches = match &next.content.action {
        CanonicalMailboxCapabilityActionV1::Activate { .. } => next_binding != previous_binding,
        CanonicalMailboxCapabilityActionV1::Revoke { .. } => {
            !matches!(
                previous.content.action,
                CanonicalMailboxCapabilityActionV1::Revoke { .. }
            ) && next_binding == previous_binding
        }
    };
    if !identity_matches
        || next.content.generation != previous.content.generation.saturating_add(1)
        || next.content.previous_update_id != Some(mailbox_capability_update_id_v1(previous)?)
        || next.content.created_at_unix_seconds < previous.content.created_at_unix_seconds
        || !action_matches
    {
        return Err(CanonicalMailboxProvisioningError::InvalidUpdateChain);
    }
    Ok(())
}

pub fn encode_signed_mailbox_capability_acknowledgement_v1(
    acknowledgement: &CanonicalSignedMailboxCapabilityAcknowledgementV1,
) -> Result<Vec<u8>, CanonicalMailboxProvisioningError> {
    verify_signed_mailbox_capability_acknowledgement_v1(acknowledgement)?;
    let content = encode_acknowledgement_content(&acknowledgement.content);
    let encoded = encode_signed(&content, &acknowledgement.signature);
    ensure_bound(
        &encoded,
        "capability acknowledgement",
        MAX_CANONICAL_MAILBOX_CAPABILITY_ACKNOWLEDGEMENT_BYTES,
    )?;
    Ok(encoded)
}

pub fn decode_and_verify_signed_mailbox_capability_acknowledgement_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedMailboxCapabilityAcknowledgementV1, CanonicalMailboxProvisioningError> {
    ensure_bound(
        bytes,
        "capability acknowledgement",
        MAX_CANONICAL_MAILBOX_CAPABILITY_ACKNOWLEDGEMENT_BYTES,
    )?;
    let (content, signature) =
        decode_signed(bytes, "signed-mailbox-capability-acknowledgement-v1")?;
    let acknowledgement = CanonicalSignedMailboxCapabilityAcknowledgementV1 {
        content: decode_acknowledgement_content(content)?,
        signature,
    };
    verify_signed_mailbox_capability_acknowledgement_v1(&acknowledgement)?;
    if encode_signed_mailbox_capability_acknowledgement_v1(&acknowledgement)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(acknowledgement)
}

pub fn verify_signed_mailbox_capability_acknowledgement_v1(
    acknowledgement: &CanonicalSignedMailboxCapabilityAcknowledgementV1,
) -> Result<(), CanonicalMailboxProvisioningError> {
    let content = &acknowledgement.content;
    if content.owner_account_id == content.recipient_account_id
        || content.owner_device_id == content.recipient_device_id
        || content.generation == 0
    {
        return Err(CanonicalMailboxProvisioningError::InvalidAcknowledgement);
    }
    verify_ed25519(
        &content.recipient_device_id,
        &prefixed(
            CANONICAL_MAILBOX_CAPABILITY_ACKNOWLEDGEMENT_SIGNATURE_DOMAIN,
            &encode_acknowledgement_content(content),
        ),
        &acknowledgement.signature,
    )
}

pub fn verify_mailbox_capability_acknowledgement_for_v1(
    acknowledgement: &CanonicalSignedMailboxCapabilityAcknowledgementV1,
    session_binding: [u8; 32],
    update: &CanonicalSignedMailboxCapabilityUpdateV1,
) -> Result<(), CanonicalMailboxProvisioningError> {
    verify_signed_mailbox_capability_acknowledgement_v1(acknowledgement)?;
    verify_signed_mailbox_capability_update_v1(update)?;
    let content = &acknowledgement.content;
    if content.session_binding != session_binding
        || content.update_id != mailbox_capability_update_id_v1(update)?
        || content.owner_account_id != update.content.owner_account_id
        || content.owner_device_id != update.content.owner_device_id
        || content.recipient_account_id != update.content.recipient_account_id
        || content.recipient_device_id != update.content.recipient_device_id
        || content.generation != update.content.generation
        || content.binding_id != action_binding_id(&update.content.action)
        || content.revoked
            != matches!(
                update.content.action,
                CanonicalMailboxCapabilityActionV1::Revoke { .. }
            )
    {
        return Err(CanonicalMailboxProvisioningError::InvalidAcknowledgement);
    }
    Ok(())
}

fn encode_local_binding_content(
    binding: &CanonicalSignedLocalMailboxBindingV1,
) -> Result<Vec<u8>, CanonicalMailboxProvisioningError> {
    let core = encode_mailbox_provisioning_core_v1(&binding.core)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 4);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &core);
    push_bytes(&mut output, &binding.read_secret);
    push_bytes(&mut output, &binding.write_secret);
    Ok(output)
}

fn encode_peer_offer_content(
    offer: &CanonicalSignedMailboxPeerOfferV1,
) -> Result<Vec<u8>, CanonicalMailboxProvisioningError> {
    let core = encode_mailbox_provisioning_core_v1(&offer.core)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 5);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &core);
    push_bytes(&mut output, &offer.owner_encryption_public_key);
    push_bytes(&mut output, &offer.write_secret);
    push_unsigned(&mut output, offer.expires_at_unix_seconds);
    Ok(output)
}

fn encode_envelope_header(
    purpose: CanonicalMailboxProvisioningEnvelopePurposeV1,
    binding_id: [u8; 32],
) -> Vec<u8> {
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_unsigned(&mut output, purpose as u64);
    push_bytes(&mut output, &binding_id);
    output
}

fn decode_envelope_header(
    bytes: &[u8],
) -> Result<
    (CanonicalMailboxProvisioningEnvelopePurposeV1, [u8; 32]),
    CanonicalMailboxProvisioningError,
> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("mailbox-provisioning-envelope-header-v1", 3)?;
    decoder.expect_version("mailbox-provisioning-envelope-header-v1.schema_version")?;
    let purpose = decode_envelope_purpose(
        decoder.unsigned("mailbox-provisioning-envelope-header-v1.purpose")?,
    )?;
    let binding_id = decoder.fixed_bytes("mailbox-provisioning-envelope-header-v1.binding_id")?;
    decoder.finish()?;
    if encode_envelope_header(purpose, binding_id) != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok((purpose, binding_id))
}

fn encode_capability_update_content(
    content: &CanonicalMailboxCapabilityUpdateContentV1,
) -> Result<Vec<u8>, CanonicalMailboxProvisioningError> {
    validate_update_content(content)?;
    let action = encode_action(&content.action)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 10);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.owner_account_id);
    push_bytes(&mut output, &content.owner_device_id);
    push_bytes(&mut output, &content.recipient_account_id);
    push_bytes(&mut output, &content.recipient_device_id);
    push_bytes(&mut output, &content.scope);
    push_unsigned(&mut output, content.generation);
    encode_id_option(content.previous_update_id, &mut output);
    push_unsigned(&mut output, content.created_at_unix_seconds);
    push_bytes(&mut output, &action);
    Ok(output)
}

fn decode_capability_update_content(
    bytes: &[u8],
) -> Result<CanonicalMailboxCapabilityUpdateContentV1, CanonicalMailboxProvisioningError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("mailbox-capability-update-content-v1", 10)?;
    decoder.expect_version("mailbox-capability-update-content-v1.schema_version")?;
    let content = CanonicalMailboxCapabilityUpdateContentV1 {
        owner_account_id: decoder
            .fixed_bytes("mailbox-capability-update-content-v1.owner_account_id")?,
        owner_device_id: decoder
            .fixed_bytes("mailbox-capability-update-content-v1.owner_device_id")?,
        recipient_account_id: decoder
            .fixed_bytes("mailbox-capability-update-content-v1.recipient_account_id")?,
        recipient_device_id: decoder
            .fixed_bytes("mailbox-capability-update-content-v1.recipient_device_id")?,
        scope: decoder.fixed_bytes("mailbox-capability-update-content-v1.scope")?,
        generation: decoder.unsigned("mailbox-capability-update-content-v1.generation")?,
        previous_update_id: decode_id_option(&mut decoder)?,
        created_at_unix_seconds: decoder
            .unsigned("mailbox-capability-update-content-v1.created_at")?,
        action: decode_action(decoder.bytes("mailbox-capability-update-content-v1.action")?)?,
    };
    decoder.finish()?;
    validate_update_content(&content)?;
    if encode_capability_update_content(&content)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(content)
}

fn encode_action(
    action: &CanonicalMailboxCapabilityActionV1,
) -> Result<Vec<u8>, CanonicalMailboxProvisioningError> {
    let mut output = Vec::new();
    match action {
        CanonicalMailboxCapabilityActionV1::Activate {
            mode,
            binding_id,
            sealed_peer_offer,
            replica_set,
        } => {
            let offer = encode_sealed_mailbox_provisioning_envelope_v1(sealed_peer_offer)?;
            push_array_header(&mut output, 6);
            push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
            push_unsigned(&mut output, 1);
            push_unsigned(&mut output, *mode as u64);
            push_bytes(&mut output, binding_id);
            push_bytes(&mut output, &offer);
            match replica_set {
                None => push_array_header(&mut output, 0),
                Some(replica_set) => {
                    push_array_header(&mut output, 1);
                    push_bytes(&mut output, &encode_mailbox_replica_set_v1(replica_set)?);
                }
            }
        }
        CanonicalMailboxCapabilityActionV1::Revoke { binding_id } => {
            push_array_header(&mut output, 3);
            push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
            push_unsigned(&mut output, 2);
            push_bytes(&mut output, binding_id);
        }
    }
    Ok(output)
}

fn decode_action(
    bytes: &[u8],
) -> Result<CanonicalMailboxCapabilityActionV1, CanonicalMailboxProvisioningError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    let length = decoder.array("mailbox-capability-action-v1")?;
    decoder.expect_version("mailbox-capability-action-v1.schema_version")?;
    let variant = decoder.unsigned("mailbox-capability-action-v1.variant")?;
    let action = match (variant, length) {
        (1, 6) => {
            let mode = decode_mode(decoder.unsigned("mailbox-capability-action-v1.mode")?)?;
            let binding_id = decoder.fixed_bytes("mailbox-capability-action-v1.binding_id")?;
            let sealed_peer_offer = decode_and_verify_sealed_mailbox_provisioning_envelope_v1(
                decoder.bytes("mailbox-capability-action-v1.sealed_peer_offer")?,
            )?;
            let replica_set = match decoder.array("mailbox-capability-action-v1.replica_set")? {
                0 => None,
                1 => Some(decode_and_verify_mailbox_replica_set_v1(
                    decoder.bytes("mailbox-capability-action-v1.replica_set.value")?,
                )?),
                _ => return Err(CanonicalMailboxProvisioningError::InvalidOption),
            };
            CanonicalMailboxCapabilityActionV1::Activate {
                mode,
                binding_id,
                sealed_peer_offer,
                replica_set,
            }
        }
        (2, 3) => CanonicalMailboxCapabilityActionV1::Revoke {
            binding_id: decoder.fixed_bytes("mailbox-capability-action-v1.binding_id")?,
        },
        (1 | 2, _) => return Err(CanonicalMailboxProvisioningError::InvalidUpdate),
        _ => return Err(CanonicalMailboxProvisioningError::UnknownAction(variant)),
    };
    decoder.finish()?;
    if encode_action(&action)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(action)
}

fn encode_acknowledgement_content(
    content: &CanonicalMailboxCapabilityAcknowledgementContentV1,
) -> Vec<u8> {
    let mut output = Vec::new();
    push_array_header(&mut output, 10);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.session_binding);
    push_bytes(&mut output, &content.update_id);
    push_bytes(&mut output, &content.owner_account_id);
    push_bytes(&mut output, &content.owner_device_id);
    push_bytes(&mut output, &content.recipient_account_id);
    push_bytes(&mut output, &content.recipient_device_id);
    push_unsigned(&mut output, content.generation);
    push_bytes(&mut output, &content.binding_id);
    push_unsigned(&mut output, u64::from(content.revoked));
    output
}

fn decode_acknowledgement_content(
    bytes: &[u8],
) -> Result<CanonicalMailboxCapabilityAcknowledgementContentV1, CanonicalMailboxProvisioningError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("mailbox-capability-acknowledgement-content-v1", 10)?;
    decoder.expect_version("mailbox-capability-acknowledgement-content-v1.schema_version")?;
    let content = CanonicalMailboxCapabilityAcknowledgementContentV1 {
        session_binding: decoder
            .fixed_bytes("mailbox-capability-acknowledgement-content-v1.session_binding")?,
        update_id: decoder
            .fixed_bytes("mailbox-capability-acknowledgement-content-v1.update_id")?,
        owner_account_id: decoder
            .fixed_bytes("mailbox-capability-acknowledgement-content-v1.owner_account_id")?,
        owner_device_id: decoder
            .fixed_bytes("mailbox-capability-acknowledgement-content-v1.owner_device_id")?,
        recipient_account_id: decoder
            .fixed_bytes("mailbox-capability-acknowledgement-content-v1.recipient_account_id")?,
        recipient_device_id: decoder
            .fixed_bytes("mailbox-capability-acknowledgement-content-v1.recipient_device_id")?,
        generation: decoder.unsigned("mailbox-capability-acknowledgement-content-v1.generation")?,
        binding_id: decoder
            .fixed_bytes("mailbox-capability-acknowledgement-content-v1.binding_id")?,
        revoked: decode_bool(
            decoder.unsigned("mailbox-capability-acknowledgement-content-v1.revoked")?,
        )?,
    };
    decoder.finish()?;
    if encode_acknowledgement_content(&content) != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(content)
}

fn encode_service_option(
    service: Option<&CanonicalMailboxServiceV1>,
    output: &mut Vec<u8>,
) -> Result<(), CanonicalMailboxProvisioningError> {
    match service {
        None => push_array_header(output, 0),
        Some(service) => {
            validate_service(service)?;
            push_array_header(output, 1);
            push_array_header(output, 3);
            push_unsigned(output, CANONICAL_PROFILE_VERSION);
            push_bytes(output, service.normalized_https_url.as_bytes());
            push_bytes(output, &service.expected_store_key);
        }
    }
    Ok(())
}

fn decode_service_option(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Option<CanonicalMailboxServiceV1>, CanonicalMailboxProvisioningError> {
    match decoder.array("mailbox-service-option-v1")? {
        0 => Ok(None),
        1 => {
            decoder.expect_array("mailbox-service-v1", 3)?;
            decoder.expect_version("mailbox-service-v1.schema_version")?;
            let url = decoder.bytes("mailbox-service-v1.normalized_https_url_utf8")?;
            let normalized_https_url = std::str::from_utf8(url)
                .map_err(|_| CanonicalMailboxProvisioningError::InvalidService)?
                .to_owned();
            let service = CanonicalMailboxServiceV1 {
                normalized_https_url,
                expected_store_key: decoder.fixed_bytes("mailbox-service-v1.expected_store_key")?,
            };
            validate_service(&service)?;
            Ok(Some(service))
        }
        _ => Err(CanonicalMailboxProvisioningError::InvalidOption),
    }
}

fn validate_service(
    service: &CanonicalMailboxServiceV1,
) -> Result<(), CanonicalMailboxProvisioningError> {
    let value = service.normalized_https_url.as_bytes();
    if value.is_empty() || value.len() > MAX_CANONICAL_MAILBOX_PROVISIONING_SERVICE_URL_BYTES {
        return Err(CanonicalMailboxProvisioningError::InvalidService);
    }
    let parsed = Url::parse(&service.normalized_https_url)
        .map_err(|_| CanonicalMailboxProvisioningError::InvalidService)?;
    if parsed.scheme() != "https"
        || parsed.username() != ""
        || parsed.password().is_some()
        || parsed.host_str().is_none()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.path_segments().is_none()
        || parsed.to_string() != service.normalized_https_url
        || VerifyingKey::from_bytes(&service.expected_store_key).is_err()
    {
        return Err(CanonicalMailboxProvisioningError::InvalidService);
    }
    Ok(())
}

fn validate_core(
    core: &CanonicalMailboxProvisioningCoreV1,
) -> Result<(), CanonicalMailboxProvisioningError> {
    encode_mailbox_address_v1(&core.address)?;
    if core.owner_account_id == core.peer_account_id
        || core.owner_device_id == core.peer_device_id
        || core.created_at_unix_seconds == 0
        || VerifyingKey::from_bytes(&core.owner_device_id).is_err()
        || VerifyingKey::from_bytes(&core.peer_device_id).is_err()
        || !matches!(
            (core.mode, core.service.as_ref()),
            (CanonicalMailboxProvisioningModeV1::LegacyHttps, Some(_))
                | (CanonicalMailboxProvisioningModeV1::ExactVolunteer, None)
        )
    {
        return Err(CanonicalMailboxProvisioningError::InvalidCore);
    }
    if let Some(service) = &core.service {
        validate_service(service)?;
    }
    Ok(())
}

fn validate_envelope(
    envelope: &CanonicalSealedMailboxProvisioningEnvelopeV1,
) -> Result<(), CanonicalMailboxProvisioningError> {
    if envelope.ciphertext.is_empty()
        || envelope.ciphertext.len() > MAX_CANONICAL_MAILBOX_PROVISIONING_SEALED_CIPHERTEXT_BYTES
    {
        return Err(CanonicalMailboxProvisioningError::InvalidEnvelope);
    }
    Ok(())
}

fn validate_replica_set(
    replica_set: &CanonicalMailboxReplicaSetV1,
) -> Result<(), CanonicalMailboxProvisioningError> {
    if !(MIN_CANONICAL_MAILBOX_REPLICA_STORES..=MAX_CANONICAL_MAILBOX_REPLICA_STORES)
        .contains(&replica_set.store_keys.len())
        || replica_set
            .store_keys
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || replica_set
            .store_keys
            .iter()
            .any(|key| VerifyingKey::from_bytes(key).is_err())
    {
        return Err(CanonicalMailboxProvisioningError::InvalidReplicaSet);
    }
    Ok(())
}

fn validate_update_content(
    content: &CanonicalMailboxCapabilityUpdateContentV1,
) -> Result<(), CanonicalMailboxProvisioningError> {
    let predecessor_shape = (content.generation == 1 && content.previous_update_id.is_none())
        || (content.generation > 1 && content.previous_update_id.is_some());
    let action_shape = match &content.action {
        CanonicalMailboxCapabilityActionV1::Activate {
            mode,
            binding_id,
            sealed_peer_offer,
            replica_set,
        } => {
            sealed_peer_offer.purpose
                == CanonicalMailboxProvisioningEnvelopePurposeV1::RecipientPeerOffer
                && sealed_peer_offer.binding_id == *binding_id
                && match (mode, replica_set) {
                    (CanonicalMailboxProvisioningModeV1::LegacyHttps, None) => true,
                    (CanonicalMailboxProvisioningModeV1::LegacyHttps, Some(value)) => {
                        validate_replica_set(value).is_ok()
                    }
                    (CanonicalMailboxProvisioningModeV1::ExactVolunteer, Some(value)) => {
                        validate_replica_set(value).is_ok()
                    }
                    (CanonicalMailboxProvisioningModeV1::ExactVolunteer, None) => false,
                }
        }
        CanonicalMailboxCapabilityActionV1::Revoke { .. } => content.generation > 1,
    };
    if content.owner_account_id == content.recipient_account_id
        || content.owner_device_id == content.recipient_device_id
        || VerifyingKey::from_bytes(&content.owner_device_id).is_err()
        || VerifyingKey::from_bytes(&content.recipient_device_id).is_err()
        || content.created_at_unix_seconds == 0
        || !predecessor_shape
        || !action_shape
    {
        return Err(CanonicalMailboxProvisioningError::InvalidUpdate);
    }
    Ok(())
}

fn action_binding_id(action: &CanonicalMailboxCapabilityActionV1) -> [u8; 32] {
    match action {
        CanonicalMailboxCapabilityActionV1::Activate { binding_id, .. }
        | CanonicalMailboxCapabilityActionV1::Revoke { binding_id } => *binding_id,
    }
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
) -> Result<(&'a [u8], [u8; 64]), CanonicalMailboxProvisioningError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array(object, 3)?;
    decoder.expect_version("signed-mailbox-provisioning.schema_version")?;
    let content = decoder.bytes("signed-mailbox-provisioning.content")?;
    let signature = decoder.fixed_bytes("signed-mailbox-provisioning.signature")?;
    decoder.finish()?;
    Ok((content, signature))
}

fn encode_id_option(value: Option<[u8; 32]>, output: &mut Vec<u8>) {
    match value {
        None => push_array_header(output, 0),
        Some(value) => {
            push_array_header(output, 1);
            push_bytes(output, &value);
        }
    }
}

fn decode_id_option(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Option<[u8; 32]>, CanonicalMailboxProvisioningError> {
    match decoder.array("mailbox-id-option-v1")? {
        0 => Ok(None),
        1 => Ok(Some(decoder.fixed_bytes("mailbox-id-option-v1.value")?)),
        _ => Err(CanonicalMailboxProvisioningError::InvalidOption),
    }
}

fn decode_mode(
    value: u64,
) -> Result<CanonicalMailboxProvisioningModeV1, CanonicalMailboxProvisioningError> {
    match value {
        1 => Ok(CanonicalMailboxProvisioningModeV1::LegacyHttps),
        2 => Ok(CanonicalMailboxProvisioningModeV1::ExactVolunteer),
        _ => Err(CanonicalMailboxProvisioningError::UnknownMode(value)),
    }
}

fn decode_envelope_purpose(
    value: u64,
) -> Result<CanonicalMailboxProvisioningEnvelopePurposeV1, CanonicalMailboxProvisioningError> {
    match value {
        1 => Ok(CanonicalMailboxProvisioningEnvelopePurposeV1::OwnerLocalBinding),
        2 => Ok(CanonicalMailboxProvisioningEnvelopePurposeV1::RecipientPeerOffer),
        _ => Err(CanonicalMailboxProvisioningError::UnknownEnvelopePurpose(
            value,
        )),
    }
}

fn decode_bool(value: u64) -> Result<bool, CanonicalMailboxProvisioningError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(CanonicalMailboxProvisioningError::InvalidAcknowledgement),
    }
}

fn verify_ed25519(
    key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
) -> Result<(), CanonicalMailboxProvisioningError> {
    let key = VerifyingKey::from_bytes(key)
        .map_err(|_| CanonicalMailboxProvisioningError::InvalidSignature)?;
    key.verify_strict(message, &Signature::from_bytes(signature))
        .map_err(|_| CanonicalMailboxProvisioningError::InvalidSignature)
}

fn ensure_bound(
    bytes: &[u8],
    object: &'static str,
    maximum: usize,
) -> Result<(), CanonicalMailboxProvisioningError> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(CanonicalMailboxProvisioningError::ObjectTooLarge { object, maximum });
    }
    Ok(())
}

fn domain_hash(domain: &[u8], encoded: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(encoded);
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct Vectors {
        schema_version: u64,
        profile: String,
        activation_status: String,
        input: Input,
        expected: Expected,
    }

    #[derive(Deserialize)]
    struct Input {
        session_binding_hex: String,
    }

    #[derive(Deserialize)]
    struct Expected {
        legacy_core_cbor_hex: String,
        legacy_binding_id_hex: String,
        exact_core_cbor_hex: String,
        exact_binding_id_hex: String,
        signed_local_binding: Signed,
        local_envelope: Envelope,
        signed_legacy_peer_offer: Signed,
        signed_exact_peer_offer: Signed,
        offer_envelope: Envelope,
        replica_set: Replica,
        activation_update: Update,
        revocation_update: Update,
        activation_acknowledgement: Signed,
    }

    #[derive(Deserialize)]
    struct Signed {
        signed_cbor_hex: String,
    }
    #[derive(Deserialize)]
    struct Envelope {
        cbor_hex: String,
        aad_cbor_hex: String,
        hpke_info_hex: String,
    }
    #[derive(Deserialize)]
    struct Replica {
        cbor_hex: String,
        commitment_id_hex: String,
    }
    #[derive(Deserialize)]
    struct Update {
        signed_cbor_hex: String,
        update_id_hex: String,
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

    fn vectors() -> Result<Vectors, Box<dyn std::error::Error>> {
        Ok(serde_json::from_str(include_str!(
            "../../../CANONICAL-MAILBOX-PROVISIONING-VECTORS.json"
        ))?)
    }

    #[test]
    fn golden_provisioning_lifecycle_verifies() -> Result<(), Box<dyn std::error::Error>> {
        let vectors = vectors()?;
        assert_eq!(vectors.schema_version, 1);
        assert_eq!(vectors.profile, "kilogram-deterministic-cbor-v1");
        assert_eq!(vectors.activation_status, "not-active");

        let legacy_core = decode_and_verify_mailbox_provisioning_core_v1(&bytes(
            &vectors.expected.legacy_core_cbor_hex,
        )?)?;
        assert_eq!(
            mailbox_binding_id_v1(&legacy_core)?,
            fixed(&vectors.expected.legacy_binding_id_hex)?
        );
        let exact_core = decode_and_verify_mailbox_provisioning_core_v1(&bytes(
            &vectors.expected.exact_core_cbor_hex,
        )?)?;
        assert_eq!(
            mailbox_binding_id_v1(&exact_core)?,
            fixed(&vectors.expected.exact_binding_id_hex)?
        );

        decode_and_verify_signed_local_mailbox_binding_v1(&bytes(
            &vectors.expected.signed_local_binding.signed_cbor_hex,
        )?)?;
        let local_envelope = decode_and_verify_sealed_mailbox_provisioning_envelope_v1(&bytes(
            &vectors.expected.local_envelope.cbor_hex,
        )?)?;
        assert_eq!(
            mailbox_provisioning_envelope_aad_v1(local_envelope.purpose, local_envelope.binding_id),
            bytes(&vectors.expected.local_envelope.aad_cbor_hex)?,
        );
        assert_eq!(
            mailbox_provisioning_envelope_hpke_info_v1(local_envelope.purpose),
            bytes(&vectors.expected.local_envelope.hpke_info_hex)?,
        );
        decode_and_verify_signed_mailbox_peer_offer_v1(&bytes(
            &vectors.expected.signed_legacy_peer_offer.signed_cbor_hex,
        )?)?;
        decode_and_verify_signed_mailbox_peer_offer_v1(&bytes(
            &vectors.expected.signed_exact_peer_offer.signed_cbor_hex,
        )?)?;
        let offer_envelope = decode_and_verify_sealed_mailbox_provisioning_envelope_v1(&bytes(
            &vectors.expected.offer_envelope.cbor_hex,
        )?)?;
        assert_eq!(
            mailbox_provisioning_envelope_aad_v1(offer_envelope.purpose, offer_envelope.binding_id),
            bytes(&vectors.expected.offer_envelope.aad_cbor_hex)?,
        );
        assert_eq!(
            mailbox_provisioning_envelope_hpke_info_v1(offer_envelope.purpose),
            bytes(&vectors.expected.offer_envelope.hpke_info_hex)?,
        );
        let replica_set = decode_and_verify_mailbox_replica_set_v1(&bytes(
            &vectors.expected.replica_set.cbor_hex,
        )?)?;
        assert_eq!(
            mailbox_replica_set_commitment_id_v1(
                mailbox_binding_id_v1(&exact_core)?,
                &replica_set
            )?,
            fixed(&vectors.expected.replica_set.commitment_id_hex)?,
        );

        let activation = decode_and_verify_signed_mailbox_capability_update_v1(&bytes(
            &vectors.expected.activation_update.signed_cbor_hex,
        )?)?;
        assert_eq!(
            mailbox_capability_update_id_v1(&activation)?,
            fixed(&vectors.expected.activation_update.update_id_hex)?,
        );
        let revocation = decode_and_verify_signed_mailbox_capability_update_v1(&bytes(
            &vectors.expected.revocation_update.signed_cbor_hex,
        )?)?;
        assert_eq!(
            mailbox_capability_update_id_v1(&revocation)?,
            fixed(&vectors.expected.revocation_update.update_id_hex)?,
        );
        verify_mailbox_capability_update_chain_link_v1(&activation, &revocation)?;
        let acknowledgement = decode_and_verify_signed_mailbox_capability_acknowledgement_v1(
            &bytes(&vectors.expected.activation_acknowledgement.signed_cbor_hex)?,
        )?;
        verify_mailbox_capability_acknowledgement_for_v1(
            &acknowledgement,
            fixed(&vectors.input.session_binding_hex)?,
            &activation,
        )?;
        Ok(())
    }

    #[test]
    fn tampering_and_wrong_session_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let vectors = vectors()?;
        let mut activation_bytes = bytes(&vectors.expected.activation_update.signed_cbor_hex)?;
        let last = activation_bytes
            .len()
            .checked_sub(1)
            .ok_or("empty activation")?;
        activation_bytes[last] ^= 1;
        assert!(decode_and_verify_signed_mailbox_capability_update_v1(&activation_bytes).is_err());
        let activation = decode_and_verify_signed_mailbox_capability_update_v1(&bytes(
            &vectors.expected.activation_update.signed_cbor_hex,
        )?)?;
        let acknowledgement = decode_and_verify_signed_mailbox_capability_acknowledgement_v1(
            &bytes(&vectors.expected.activation_acknowledgement.signed_cbor_hex)?,
        )?;
        assert!(
            verify_mailbox_capability_acknowledgement_for_v1(
                &acknowledgement,
                [0xff; 32],
                &activation,
            )
            .is_err()
        );
        Ok(())
    }
}
