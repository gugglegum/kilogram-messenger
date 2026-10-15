//! Inactive deterministic-CBOR blind-mailbox capability artifacts.
//!
//! The active mailbox implementation remains Postcard encoded. This module
//! defines a future cross-language target for portable mailbox addresses,
//! opaque encrypted envelopes, capability authorizations, volunteer storage
//! offers, and store receipts. Provisioning, provider gossip, transport, and
//! runtime dispatch envelopes are deliberately separate compatibility surfaces.

use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

use crate::{
    canonical_account_v1::{AccountDecoder, CanonicalAccountError, signed_envelope},
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
        push_unsigned, validate_canonical_profile_v1,
    },
};

pub const MIN_CANONICAL_MAILBOX_TTL_SECONDS: u64 = 60;
pub const MAX_CANONICAL_MAILBOX_TTL_SECONDS: u64 = 604_800;
pub const MAX_CANONICAL_MAILBOX_PAGE_ITEMS: u64 = 8;
pub const MAX_CANONICAL_MAILBOX_ADDRESS_BYTES: usize = 256;
pub const MAX_CANONICAL_MAILBOX_AUTHORIZATION_BYTES: usize = 1024;
pub const MAX_CANONICAL_MAILBOX_RECEIPT_BYTES: usize = 1024;
pub const MAX_CANONICAL_MAILBOX_ENVELOPE_BYTES: usize = 1024 * 1024;
pub const MAX_CANONICAL_MAILBOX_STORAGE_ENDPOINT_BYTES: usize = 16 * 1024;
pub const MAX_CANONICAL_MAILBOX_STORAGE_OFFER_BYTES: usize = 24 * 1024;
pub const MIN_CANONICAL_MAILBOX_STORAGE_OFFER_VALIDITY_SECONDS: u64 = 30;
pub const MAX_CANONICAL_MAILBOX_STORAGE_OFFER_VALIDITY_SECONDS: u64 = 3600;
pub const MAX_CANONICAL_MAILBOX_STORAGE_OFFER_ADMISSION_WORK_BITS: u64 = 20;

pub const CANONICAL_MAILBOX_ID_DOMAIN: &[u8] = b"kilogram:blind-mailbox-id:cbor-v1\0";
pub const CANONICAL_MAILBOX_ENVELOPE_HPKE_INFO: &[u8] =
    b"kilogram:blind-mailbox-envelope:cbor-v1\0";
pub const CANONICAL_MAILBOX_ENVELOPE_DIGEST_DOMAIN: &[u8] =
    b"kilogram:blind-mailbox-envelope-digest:cbor-v1\0";
pub const CANONICAL_MAILBOX_WRITE_AUTHORIZATION_DOMAIN: &[u8] =
    b"kilogram:blind-mailbox-write:cbor-v1\0";
pub const CANONICAL_MAILBOX_READ_AUTHORIZATION_DOMAIN: &[u8] =
    b"kilogram:blind-mailbox-read:cbor-v1\0";
pub const CANONICAL_MAILBOX_STORAGE_OFFER_DOMAIN: &[u8] =
    b"kilogram:blind-mailbox-storage-offer:cbor-v1\0";
pub const CANONICAL_MAILBOX_STORAGE_OFFER_ADMISSION_WORK_DOMAIN: &[u8] =
    b"kilogram:blind-mailbox-storage-offer-admission-work:cbor-v1\0";
pub const CANONICAL_MAILBOX_STORED_RECEIPT_DOMAIN: &[u8] =
    b"kilogram:blind-mailbox-stored-receipt:cbor-v1\0";
pub const CANONICAL_MAILBOX_RECEIPT_ID_DOMAIN: &[u8] =
    b"kilogram:blind-mailbox-receipt-id:cbor-v1\0";
pub const CANONICAL_MAILBOX_DELETE_RECEIPT_DOMAIN: &[u8] =
    b"kilogram:blind-mailbox-delete-receipt:cbor-v1\0";
pub const CANONICAL_MAILBOX_DELETE_RECEIPT_ID_DOMAIN: &[u8] =
    b"kilogram:blind-mailbox-delete-id:cbor-v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxAddressV1 {
    pub read_key: [u8; 32],
    pub write_key: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxEnvelopeContextV1 {
    pub mailbox_id: [u8; 32],
    pub item_id: [u8; 32],
    pub created_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxEnvelopeV1 {
    pub context: CanonicalMailboxEnvelopeContextV1,
    pub encapsulated_key: [u8; 32],
    pub ciphertext: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxWriteAuthorizationContentV1 {
    pub mailbox_id: [u8; 32],
    pub item_id: [u8; 32],
    pub write_key: [u8; 32],
    pub requested_ttl_seconds: u64,
    pub envelope_bytes: u64,
    pub envelope_digest: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedMailboxWriteAuthorizationV1 {
    pub content: CanonicalMailboxWriteAuthorizationContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalMailboxReadOperationV1 {
    List {
        nonce: [u8; 32],
        after_item_id: Option<[u8; 32]>,
        limit: u64,
    },
    Delete {
        item_id: [u8; 32],
        receipt_id: [u8; 32],
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxReadAuthorizationContentV1 {
    pub mailbox_id: [u8; 32],
    pub read_key: [u8; 32],
    pub operation: CanonicalMailboxReadOperationV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedMailboxReadAuthorizationV1 {
    pub content: CanonicalMailboxReadAuthorizationContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum CanonicalMailboxStoragePolicyClassV1 {
    BoundedVolunteer = 1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxStorageOfferContentV1 {
    pub provider_endpoint: Vec<u8>,
    pub store_key: [u8; 32],
    pub policy_class: CanonicalMailboxStoragePolicyClassV1,
    pub capacity_hint_bytes: u64,
    pub max_record_bytes: u64,
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub nonce: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedMailboxStorageOfferV1 {
    pub content: CanonicalMailboxStorageOfferContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxStoredReceiptContentV1 {
    pub store_key: [u8; 32],
    pub mailbox_id: [u8; 32],
    pub item_id: [u8; 32],
    pub envelope_bytes: u64,
    pub envelope_digest: [u8; 32],
    pub stored_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedMailboxStoredReceiptV1 {
    pub content: CanonicalMailboxStoredReceiptContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxDeleteReceiptContentV1 {
    pub store_key: [u8; 32],
    pub mailbox_id: [u8; 32],
    pub item_id: [u8; 32],
    pub stored_receipt_id: [u8; 32],
    pub deleted_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedMailboxDeleteReceiptV1 {
    pub content: CanonicalMailboxDeleteReceiptContentV1,
    pub signature: [u8; 64],
}

#[derive(Debug, Error)]
pub enum CanonicalMailboxError {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),
    #[error(transparent)]
    Decode(#[from] CanonicalAccountError),
    #[error("canonical mailbox {object} exceeds {maximum} bytes")]
    ObjectTooLarge {
        object: &'static str,
        maximum: usize,
    },
    #[error("canonical mailbox {object} contains an invalid Ed25519 key")]
    InvalidPublicKey { object: &'static str },
    #[error("canonical mailbox {object} contains an invalid Ed25519 signature")]
    InvalidSignature { object: &'static str },
    #[error("canonical mailbox TTL is outside protocol bounds")]
    InvalidTtl,
    #[error("canonical mailbox envelope is empty or malformed")]
    InvalidEnvelope,
    #[error("canonical mailbox authorization does not match its address or envelope")]
    AuthorizationMismatch,
    #[error("unknown canonical mailbox read operation {0}")]
    UnknownReadOperation(u64),
    #[error("canonical mailbox list page limit is outside protocol bounds")]
    InvalidPageLimit,
    #[error("canonical mailbox option array is malformed")]
    InvalidOption,
    #[error("unknown canonical mailbox storage policy class {0}")]
    UnknownStoragePolicy(u64),
    #[error("canonical mailbox storage offer is malformed")]
    InvalidStorageOffer,
    #[error("canonical mailbox storage offer admission-work requirement is invalid")]
    InvalidAdmissionWork,
    #[error("canonical mailbox receipt is malformed")]
    InvalidReceipt,
}

pub fn encode_mailbox_address_v1(
    address: &CanonicalMailboxAddressV1,
) -> Result<Vec<u8>, CanonicalMailboxError> {
    validate_address(address)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &address.read_key);
    push_bytes(&mut output, &address.write_key);
    ensure_bound(&output, "address", MAX_CANONICAL_MAILBOX_ADDRESS_BYTES)?;
    Ok(output)
}

pub fn decode_and_verify_mailbox_address_v1(
    bytes: &[u8],
) -> Result<CanonicalMailboxAddressV1, CanonicalMailboxError> {
    ensure_bound(bytes, "address", MAX_CANONICAL_MAILBOX_ADDRESS_BYTES)?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("mailbox-address-v1", 3)?;
    decoder.expect_version("mailbox-address-v1.schema_version")?;
    let address = CanonicalMailboxAddressV1 {
        read_key: decoder.fixed_bytes("mailbox-address-v1.read_key")?,
        write_key: decoder.fixed_bytes("mailbox-address-v1.write_key")?,
    };
    decoder.finish()?;
    validate_address(&address)?;
    Ok(address)
}

pub fn mailbox_id_v1(
    address: &CanonicalMailboxAddressV1,
) -> Result<[u8; 32], CanonicalMailboxError> {
    let encoded = encode_mailbox_address_v1(address)?;
    Ok(domain_hash(CANONICAL_MAILBOX_ID_DOMAIN, &encoded))
}

pub fn encode_mailbox_envelope_context_v1(
    context: &CanonicalMailboxEnvelopeContextV1,
) -> Result<Vec<u8>, CanonicalMailboxError> {
    validate_envelope_context(context)?;
    let mut output = Vec::new();
    encode_envelope_context_into(context, &mut output);
    Ok(output)
}

pub fn mailbox_envelope_aad_v1(
    context: &CanonicalMailboxEnvelopeContextV1,
) -> Result<Vec<u8>, CanonicalMailboxError> {
    encode_mailbox_envelope_context_v1(context)
}

pub fn encode_mailbox_envelope_v1(
    envelope: &CanonicalMailboxEnvelopeV1,
) -> Result<Vec<u8>, CanonicalMailboxError> {
    validate_envelope(envelope)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 4);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    encode_envelope_context_into(&envelope.context, &mut output);
    push_bytes(&mut output, &envelope.encapsulated_key);
    push_bytes(&mut output, &envelope.ciphertext);
    ensure_bound(&output, "envelope", MAX_CANONICAL_MAILBOX_ENVELOPE_BYTES)?;
    Ok(output)
}

pub fn decode_and_verify_mailbox_envelope_v1(
    bytes: &[u8],
) -> Result<CanonicalMailboxEnvelopeV1, CanonicalMailboxError> {
    ensure_bound(bytes, "envelope", MAX_CANONICAL_MAILBOX_ENVELOPE_BYTES)?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("mailbox-envelope-v1", 4)?;
    decoder.expect_version("mailbox-envelope-v1.schema_version")?;
    let context = decode_envelope_context_from(&mut decoder)?;
    let encapsulated_key = decoder.fixed_bytes("mailbox-envelope-v1.encapsulated_key")?;
    let ciphertext = decoder.bytes("mailbox-envelope-v1.ciphertext")?.to_vec();
    decoder.finish()?;
    let envelope = CanonicalMailboxEnvelopeV1 {
        context,
        encapsulated_key,
        ciphertext,
    };
    validate_envelope(&envelope)?;
    Ok(envelope)
}

pub fn mailbox_envelope_digest_v1(
    envelope: &CanonicalMailboxEnvelopeV1,
) -> Result<[u8; 32], CanonicalMailboxError> {
    Ok(domain_hash(
        CANONICAL_MAILBOX_ENVELOPE_DIGEST_DOMAIN,
        &encode_mailbox_envelope_v1(envelope)?,
    ))
}

pub fn encode_mailbox_write_authorization_content_v1(
    content: &CanonicalMailboxWriteAuthorizationContentV1,
) -> Result<Vec<u8>, CanonicalMailboxError> {
    validate_write_content(content)?;
    let mut output = Vec::new();
    encode_write_content_into(content, &mut output);
    Ok(output)
}

pub fn mailbox_write_authorization_signing_bytes_v1(
    content: &CanonicalMailboxWriteAuthorizationContentV1,
) -> Result<Vec<u8>, CanonicalMailboxError> {
    Ok(prefixed(
        CANONICAL_MAILBOX_WRITE_AUTHORIZATION_DOMAIN,
        &encode_mailbox_write_authorization_content_v1(content)?,
    ))
}

pub fn encode_signed_mailbox_write_authorization_v1(
    authorization: &CanonicalSignedMailboxWriteAuthorizationV1,
) -> Result<Vec<u8>, CanonicalMailboxError> {
    verify_signed_mailbox_write_authorization_v1(authorization)?;
    let content = encode_mailbox_write_authorization_content_v1(&authorization.content)?;
    let encoded = signed_envelope(&content, &authorization.signature);
    ensure_bound(
        &encoded,
        "write authorization",
        MAX_CANONICAL_MAILBOX_AUTHORIZATION_BYTES,
    )?;
    Ok(encoded)
}

pub fn decode_and_verify_signed_mailbox_write_authorization_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedMailboxWriteAuthorizationV1, CanonicalMailboxError> {
    ensure_bound(
        bytes,
        "write authorization",
        MAX_CANONICAL_MAILBOX_AUTHORIZATION_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-mailbox-write-authorization-v1", 3)?;
    decoder.expect_version("signed-mailbox-write-authorization-v1.schema_version")?;
    let content = decode_write_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-mailbox-write-authorization-v1.signature")?;
    decoder.finish()?;
    let authorization = CanonicalSignedMailboxWriteAuthorizationV1 { content, signature };
    verify_signed_mailbox_write_authorization_v1(&authorization)?;
    Ok(authorization)
}

pub fn verify_signed_mailbox_write_authorization_v1(
    authorization: &CanonicalSignedMailboxWriteAuthorizationV1,
) -> Result<(), CanonicalMailboxError> {
    validate_write_content(&authorization.content)?;
    verify_ed25519(
        &authorization.content.write_key,
        &mailbox_write_authorization_signing_bytes_v1(&authorization.content)?,
        &authorization.signature,
        "write authorization",
    )
}

pub fn verify_mailbox_write_authorization_for_v1(
    authorization: &CanonicalSignedMailboxWriteAuthorizationV1,
    address: &CanonicalMailboxAddressV1,
    envelope: &CanonicalMailboxEnvelopeV1,
) -> Result<(), CanonicalMailboxError> {
    verify_signed_mailbox_write_authorization_v1(authorization)?;
    let encoded = encode_mailbox_envelope_v1(envelope)?;
    if authorization.content.mailbox_id != mailbox_id_v1(address)?
        || authorization.content.mailbox_id != envelope.context.mailbox_id
        || authorization.content.item_id != envelope.context.item_id
        || authorization.content.write_key != address.write_key
        || authorization.content.envelope_bytes != encoded.len() as u64
        || authorization.content.envelope_digest != mailbox_envelope_digest_v1(envelope)?
    {
        return Err(CanonicalMailboxError::AuthorizationMismatch);
    }
    Ok(())
}

pub fn encode_mailbox_read_authorization_content_v1(
    content: &CanonicalMailboxReadAuthorizationContentV1,
) -> Result<Vec<u8>, CanonicalMailboxError> {
    validate_read_content(content)?;
    let mut output = Vec::new();
    encode_read_content_into(content, &mut output);
    Ok(output)
}

pub fn mailbox_read_authorization_signing_bytes_v1(
    content: &CanonicalMailboxReadAuthorizationContentV1,
) -> Result<Vec<u8>, CanonicalMailboxError> {
    Ok(prefixed(
        CANONICAL_MAILBOX_READ_AUTHORIZATION_DOMAIN,
        &encode_mailbox_read_authorization_content_v1(content)?,
    ))
}

pub fn encode_signed_mailbox_read_authorization_v1(
    authorization: &CanonicalSignedMailboxReadAuthorizationV1,
) -> Result<Vec<u8>, CanonicalMailboxError> {
    verify_signed_mailbox_read_authorization_v1(authorization)?;
    let content = encode_mailbox_read_authorization_content_v1(&authorization.content)?;
    let encoded = signed_envelope(&content, &authorization.signature);
    ensure_bound(
        &encoded,
        "read authorization",
        MAX_CANONICAL_MAILBOX_AUTHORIZATION_BYTES,
    )?;
    Ok(encoded)
}

pub fn decode_and_verify_signed_mailbox_read_authorization_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedMailboxReadAuthorizationV1, CanonicalMailboxError> {
    ensure_bound(
        bytes,
        "read authorization",
        MAX_CANONICAL_MAILBOX_AUTHORIZATION_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-mailbox-read-authorization-v1", 3)?;
    decoder.expect_version("signed-mailbox-read-authorization-v1.schema_version")?;
    let content = decode_read_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-mailbox-read-authorization-v1.signature")?;
    decoder.finish()?;
    let authorization = CanonicalSignedMailboxReadAuthorizationV1 { content, signature };
    verify_signed_mailbox_read_authorization_v1(&authorization)?;
    Ok(authorization)
}

pub fn verify_signed_mailbox_read_authorization_v1(
    authorization: &CanonicalSignedMailboxReadAuthorizationV1,
) -> Result<(), CanonicalMailboxError> {
    validate_read_content(&authorization.content)?;
    verify_ed25519(
        &authorization.content.read_key,
        &mailbox_read_authorization_signing_bytes_v1(&authorization.content)?,
        &authorization.signature,
        "read authorization",
    )
}

pub fn verify_mailbox_read_authorization_for_v1(
    authorization: &CanonicalSignedMailboxReadAuthorizationV1,
    address: &CanonicalMailboxAddressV1,
) -> Result<(), CanonicalMailboxError> {
    verify_signed_mailbox_read_authorization_v1(authorization)?;
    if authorization.content.mailbox_id != mailbox_id_v1(address)?
        || authorization.content.read_key != address.read_key
    {
        return Err(CanonicalMailboxError::AuthorizationMismatch);
    }
    Ok(())
}

pub fn encode_mailbox_storage_offer_content_v1(
    content: &CanonicalMailboxStorageOfferContentV1,
) -> Result<Vec<u8>, CanonicalMailboxError> {
    validate_storage_offer_content(content)?;
    let mut output = Vec::new();
    encode_storage_offer_content_into(content, &mut output);
    Ok(output)
}

pub fn mailbox_storage_offer_signing_bytes_v1(
    content: &CanonicalMailboxStorageOfferContentV1,
) -> Result<Vec<u8>, CanonicalMailboxError> {
    Ok(prefixed(
        CANONICAL_MAILBOX_STORAGE_OFFER_DOMAIN,
        &encode_mailbox_storage_offer_content_v1(content)?,
    ))
}

pub fn encode_signed_mailbox_storage_offer_v1(
    offer: &CanonicalSignedMailboxStorageOfferV1,
) -> Result<Vec<u8>, CanonicalMailboxError> {
    verify_signed_mailbox_storage_offer_v1(offer)?;
    let content = encode_mailbox_storage_offer_content_v1(&offer.content)?;
    let encoded = signed_envelope(&content, &offer.signature);
    ensure_bound(
        &encoded,
        "storage offer",
        MAX_CANONICAL_MAILBOX_STORAGE_OFFER_BYTES,
    )?;
    Ok(encoded)
}

pub fn decode_and_verify_signed_mailbox_storage_offer_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedMailboxStorageOfferV1, CanonicalMailboxError> {
    ensure_bound(
        bytes,
        "storage offer",
        MAX_CANONICAL_MAILBOX_STORAGE_OFFER_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-mailbox-storage-offer-v1", 3)?;
    decoder.expect_version("signed-mailbox-storage-offer-v1.schema_version")?;
    let content = decode_storage_offer_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-mailbox-storage-offer-v1.signature")?;
    decoder.finish()?;
    let offer = CanonicalSignedMailboxStorageOfferV1 { content, signature };
    verify_signed_mailbox_storage_offer_v1(&offer)?;
    Ok(offer)
}

pub fn verify_signed_mailbox_storage_offer_v1(
    offer: &CanonicalSignedMailboxStorageOfferV1,
) -> Result<(), CanonicalMailboxError> {
    validate_storage_offer_content(&offer.content)?;
    verify_ed25519(
        &offer.content.store_key,
        &mailbox_storage_offer_signing_bytes_v1(&offer.content)?,
        &offer.signature,
        "storage offer",
    )
}

pub fn verify_mailbox_storage_offer_at_v1(
    offer: &CanonicalSignedMailboxStorageOfferV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalMailboxError> {
    verify_signed_mailbox_storage_offer_v1(offer)?;
    if now_unix_seconds < offer.content.issued_at_unix_seconds
        || now_unix_seconds >= offer.content.expires_at_unix_seconds
    {
        return Err(CanonicalMailboxError::InvalidStorageOffer);
    }
    Ok(())
}

pub fn mailbox_storage_offer_admission_work_bits_v1(
    content: &CanonicalMailboxStorageOfferContentV1,
) -> Result<u16, CanonicalMailboxError> {
    let encoded = encode_mailbox_storage_offer_content_v1(content)?;
    let digest = domain_hash(
        CANONICAL_MAILBOX_STORAGE_OFFER_ADMISSION_WORK_DOMAIN,
        &encoded,
    );
    Ok(leading_zero_bits(&digest))
}

pub fn verify_mailbox_storage_offer_admission_work_v1(
    offer: &CanonicalSignedMailboxStorageOfferV1,
    minimum_work_bits: u64,
) -> Result<(), CanonicalMailboxError> {
    verify_signed_mailbox_storage_offer_v1(offer)?;
    if !(1..=MAX_CANONICAL_MAILBOX_STORAGE_OFFER_ADMISSION_WORK_BITS).contains(&minimum_work_bits)
        || u64::from(mailbox_storage_offer_admission_work_bits_v1(
            &offer.content,
        )?) < minimum_work_bits
    {
        return Err(CanonicalMailboxError::InvalidAdmissionWork);
    }
    Ok(())
}

pub fn encode_mailbox_stored_receipt_content_v1(
    content: &CanonicalMailboxStoredReceiptContentV1,
) -> Result<Vec<u8>, CanonicalMailboxError> {
    validate_stored_receipt_content(content)?;
    let mut output = Vec::new();
    encode_stored_receipt_content_into(content, &mut output);
    Ok(output)
}

pub fn mailbox_stored_receipt_signing_bytes_v1(
    content: &CanonicalMailboxStoredReceiptContentV1,
) -> Result<Vec<u8>, CanonicalMailboxError> {
    Ok(prefixed(
        CANONICAL_MAILBOX_STORED_RECEIPT_DOMAIN,
        &encode_mailbox_stored_receipt_content_v1(content)?,
    ))
}

pub fn encode_signed_mailbox_stored_receipt_v1(
    receipt: &CanonicalSignedMailboxStoredReceiptV1,
) -> Result<Vec<u8>, CanonicalMailboxError> {
    verify_signed_mailbox_stored_receipt_v1(receipt)?;
    let content = encode_mailbox_stored_receipt_content_v1(&receipt.content)?;
    let encoded = signed_envelope(&content, &receipt.signature);
    ensure_bound(
        &encoded,
        "stored receipt",
        MAX_CANONICAL_MAILBOX_RECEIPT_BYTES,
    )?;
    Ok(encoded)
}

pub fn decode_and_verify_signed_mailbox_stored_receipt_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedMailboxStoredReceiptV1, CanonicalMailboxError> {
    ensure_bound(bytes, "stored receipt", MAX_CANONICAL_MAILBOX_RECEIPT_BYTES)?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-mailbox-stored-receipt-v1", 3)?;
    decoder.expect_version("signed-mailbox-stored-receipt-v1.schema_version")?;
    let content = decode_stored_receipt_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-mailbox-stored-receipt-v1.signature")?;
    decoder.finish()?;
    let receipt = CanonicalSignedMailboxStoredReceiptV1 { content, signature };
    verify_signed_mailbox_stored_receipt_v1(&receipt)?;
    Ok(receipt)
}

pub fn verify_signed_mailbox_stored_receipt_v1(
    receipt: &CanonicalSignedMailboxStoredReceiptV1,
) -> Result<(), CanonicalMailboxError> {
    validate_stored_receipt_content(&receipt.content)?;
    verify_ed25519(
        &receipt.content.store_key,
        &mailbox_stored_receipt_signing_bytes_v1(&receipt.content)?,
        &receipt.signature,
        "stored receipt",
    )
}

pub fn verify_mailbox_stored_receipt_for_v1(
    receipt: &CanonicalSignedMailboxStoredReceiptV1,
    envelope: &CanonicalMailboxEnvelopeV1,
) -> Result<(), CanonicalMailboxError> {
    verify_signed_mailbox_stored_receipt_v1(receipt)?;
    let encoded = encode_mailbox_envelope_v1(envelope)?;
    if receipt.content.mailbox_id != envelope.context.mailbox_id
        || receipt.content.item_id != envelope.context.item_id
        || receipt.content.envelope_bytes != encoded.len() as u64
        || receipt.content.envelope_digest != mailbox_envelope_digest_v1(envelope)?
    {
        return Err(CanonicalMailboxError::InvalidReceipt);
    }
    Ok(())
}

pub fn mailbox_stored_receipt_id_v1(
    receipt: &CanonicalSignedMailboxStoredReceiptV1,
) -> Result<[u8; 32], CanonicalMailboxError> {
    Ok(domain_hash(
        CANONICAL_MAILBOX_RECEIPT_ID_DOMAIN,
        &encode_signed_mailbox_stored_receipt_v1(receipt)?,
    ))
}

pub fn encode_mailbox_delete_receipt_content_v1(
    content: &CanonicalMailboxDeleteReceiptContentV1,
) -> Result<Vec<u8>, CanonicalMailboxError> {
    validate_delete_receipt_content(content)?;
    let mut output = Vec::new();
    encode_delete_receipt_content_into(content, &mut output);
    Ok(output)
}

pub fn mailbox_delete_receipt_signing_bytes_v1(
    content: &CanonicalMailboxDeleteReceiptContentV1,
) -> Result<Vec<u8>, CanonicalMailboxError> {
    Ok(prefixed(
        CANONICAL_MAILBOX_DELETE_RECEIPT_DOMAIN,
        &encode_mailbox_delete_receipt_content_v1(content)?,
    ))
}

pub fn encode_signed_mailbox_delete_receipt_v1(
    receipt: &CanonicalSignedMailboxDeleteReceiptV1,
) -> Result<Vec<u8>, CanonicalMailboxError> {
    verify_signed_mailbox_delete_receipt_v1(receipt)?;
    let content = encode_mailbox_delete_receipt_content_v1(&receipt.content)?;
    let encoded = signed_envelope(&content, &receipt.signature);
    ensure_bound(
        &encoded,
        "delete receipt",
        MAX_CANONICAL_MAILBOX_RECEIPT_BYTES,
    )?;
    Ok(encoded)
}

pub fn decode_and_verify_signed_mailbox_delete_receipt_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedMailboxDeleteReceiptV1, CanonicalMailboxError> {
    ensure_bound(bytes, "delete receipt", MAX_CANONICAL_MAILBOX_RECEIPT_BYTES)?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-mailbox-delete-receipt-v1", 3)?;
    decoder.expect_version("signed-mailbox-delete-receipt-v1.schema_version")?;
    let content = decode_delete_receipt_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-mailbox-delete-receipt-v1.signature")?;
    decoder.finish()?;
    let receipt = CanonicalSignedMailboxDeleteReceiptV1 { content, signature };
    verify_signed_mailbox_delete_receipt_v1(&receipt)?;
    Ok(receipt)
}

pub fn verify_signed_mailbox_delete_receipt_v1(
    receipt: &CanonicalSignedMailboxDeleteReceiptV1,
) -> Result<(), CanonicalMailboxError> {
    validate_delete_receipt_content(&receipt.content)?;
    verify_ed25519(
        &receipt.content.store_key,
        &mailbox_delete_receipt_signing_bytes_v1(&receipt.content)?,
        &receipt.signature,
        "delete receipt",
    )
}

pub fn verify_mailbox_delete_receipt_for_v1(
    receipt: &CanonicalSignedMailboxDeleteReceiptV1,
    stored_receipt: &CanonicalSignedMailboxStoredReceiptV1,
) -> Result<(), CanonicalMailboxError> {
    verify_signed_mailbox_delete_receipt_v1(receipt)?;
    verify_signed_mailbox_stored_receipt_v1(stored_receipt)?;
    if receipt.content.store_key != stored_receipt.content.store_key
        || receipt.content.mailbox_id != stored_receipt.content.mailbox_id
        || receipt.content.item_id != stored_receipt.content.item_id
        || receipt.content.stored_receipt_id != mailbox_stored_receipt_id_v1(stored_receipt)?
        || receipt.content.expires_at_unix_seconds != stored_receipt.content.expires_at_unix_seconds
    {
        return Err(CanonicalMailboxError::InvalidReceipt);
    }
    Ok(())
}

pub fn mailbox_delete_receipt_id_v1(
    receipt: &CanonicalSignedMailboxDeleteReceiptV1,
) -> Result<[u8; 32], CanonicalMailboxError> {
    Ok(domain_hash(
        CANONICAL_MAILBOX_DELETE_RECEIPT_ID_DOMAIN,
        &encode_signed_mailbox_delete_receipt_v1(receipt)?,
    ))
}

fn encode_envelope_context_into(context: &CanonicalMailboxEnvelopeContextV1, output: &mut Vec<u8>) {
    push_array_header(output, 5);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_bytes(output, &context.mailbox_id);
    push_bytes(output, &context.item_id);
    push_unsigned(output, context.created_at_unix_seconds);
    push_unsigned(output, context.expires_at_unix_seconds);
}

fn decode_envelope_context_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalMailboxEnvelopeContextV1, CanonicalMailboxError> {
    decoder.expect_array("mailbox-envelope-context-v1", 5)?;
    decoder.expect_version("mailbox-envelope-context-v1.schema_version")?;
    let context = CanonicalMailboxEnvelopeContextV1 {
        mailbox_id: decoder.fixed_bytes("mailbox-envelope-context-v1.mailbox_id")?,
        item_id: decoder.fixed_bytes("mailbox-envelope-context-v1.item_id")?,
        created_at_unix_seconds: decoder
            .unsigned("mailbox-envelope-context-v1.created_at_unix_seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("mailbox-envelope-context-v1.expires_at_unix_seconds")?,
    };
    validate_envelope_context(&context)?;
    Ok(context)
}

fn encode_write_content_into(
    content: &CanonicalMailboxWriteAuthorizationContentV1,
    output: &mut Vec<u8>,
) {
    push_array_header(output, 7);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_bytes(output, &content.mailbox_id);
    push_bytes(output, &content.item_id);
    push_bytes(output, &content.write_key);
    push_unsigned(output, content.requested_ttl_seconds);
    push_unsigned(output, content.envelope_bytes);
    push_bytes(output, &content.envelope_digest);
}

fn decode_write_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalMailboxWriteAuthorizationContentV1, CanonicalMailboxError> {
    decoder.expect_array("mailbox-write-authorization-content-v1", 7)?;
    decoder.expect_version("mailbox-write-authorization-content-v1.schema_version")?;
    let content = CanonicalMailboxWriteAuthorizationContentV1 {
        mailbox_id: decoder.fixed_bytes("mailbox-write-authorization-content-v1.mailbox_id")?,
        item_id: decoder.fixed_bytes("mailbox-write-authorization-content-v1.item_id")?,
        write_key: decoder.fixed_bytes("mailbox-write-authorization-content-v1.write_key")?,
        requested_ttl_seconds: decoder
            .unsigned("mailbox-write-authorization-content-v1.requested_ttl_seconds")?,
        envelope_bytes: decoder
            .unsigned("mailbox-write-authorization-content-v1.envelope_bytes")?,
        envelope_digest: decoder
            .fixed_bytes("mailbox-write-authorization-content-v1.envelope_digest")?,
    };
    validate_write_content(&content)?;
    Ok(content)
}

fn encode_read_content_into(
    content: &CanonicalMailboxReadAuthorizationContentV1,
    output: &mut Vec<u8>,
) {
    push_array_header(output, 4);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_bytes(output, &content.mailbox_id);
    push_bytes(output, &content.read_key);
    match content.operation {
        CanonicalMailboxReadOperationV1::List {
            nonce,
            after_item_id,
            limit,
        } => {
            push_array_header(output, 5);
            push_unsigned(output, CANONICAL_PROFILE_VERSION);
            push_unsigned(output, 1);
            push_bytes(output, &nonce);
            match after_item_id {
                None => push_array_header(output, 0),
                Some(item_id) => {
                    push_array_header(output, 1);
                    push_bytes(output, &item_id);
                }
            }
            push_unsigned(output, limit);
        }
        CanonicalMailboxReadOperationV1::Delete {
            item_id,
            receipt_id,
        } => {
            push_array_header(output, 4);
            push_unsigned(output, CANONICAL_PROFILE_VERSION);
            push_unsigned(output, 2);
            push_bytes(output, &item_id);
            push_bytes(output, &receipt_id);
        }
    }
}

fn decode_read_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalMailboxReadAuthorizationContentV1, CanonicalMailboxError> {
    decoder.expect_array("mailbox-read-authorization-content-v1", 4)?;
    decoder.expect_version("mailbox-read-authorization-content-v1.schema_version")?;
    let mailbox_id = decoder.fixed_bytes("mailbox-read-authorization-content-v1.mailbox_id")?;
    let read_key = decoder.fixed_bytes("mailbox-read-authorization-content-v1.read_key")?;
    let operation_length = decoder.array("mailbox-read-operation-v1")?;
    let operation_version = decoder.unsigned("mailbox-read-operation-v1.schema_version")?;
    if operation_version != CANONICAL_PROFILE_VERSION {
        return Err(CanonicalMailboxError::UnknownReadOperation(
            operation_version,
        ));
    }
    let variant = decoder.unsigned("mailbox-read-operation-v1.variant")?;
    let operation = match variant {
        1 if operation_length == 5 => {
            let nonce = decoder.fixed_bytes("mailbox-read-operation-v1.nonce")?;
            let option_length = decoder.array("mailbox-read-operation-v1.after_item_id")?;
            let after_item_id = match option_length {
                0 => None,
                1 => Some(decoder.fixed_bytes("mailbox-read-operation-v1.after_item_id.value")?),
                _ => return Err(CanonicalMailboxError::InvalidOption),
            };
            let limit = decoder.unsigned("mailbox-read-operation-v1.limit")?;
            CanonicalMailboxReadOperationV1::List {
                nonce,
                after_item_id,
                limit,
            }
        }
        2 if operation_length == 4 => CanonicalMailboxReadOperationV1::Delete {
            item_id: decoder.fixed_bytes("mailbox-read-operation-v1.item_id")?,
            receipt_id: decoder.fixed_bytes("mailbox-read-operation-v1.receipt_id")?,
        },
        other => return Err(CanonicalMailboxError::UnknownReadOperation(other)),
    };
    let content = CanonicalMailboxReadAuthorizationContentV1 {
        mailbox_id,
        read_key,
        operation,
    };
    validate_read_content(&content)?;
    Ok(content)
}

fn encode_storage_offer_content_into(
    content: &CanonicalMailboxStorageOfferContentV1,
    output: &mut Vec<u8>,
) {
    push_array_header(output, 9);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_bytes(output, &content.provider_endpoint);
    push_bytes(output, &content.store_key);
    push_unsigned(output, content.policy_class as u64);
    push_unsigned(output, content.capacity_hint_bytes);
    push_unsigned(output, content.max_record_bytes);
    push_unsigned(output, content.issued_at_unix_seconds);
    push_unsigned(output, content.expires_at_unix_seconds);
    push_bytes(output, &content.nonce);
}

fn decode_storage_offer_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalMailboxStorageOfferContentV1, CanonicalMailboxError> {
    decoder.expect_array("mailbox-storage-offer-content-v1", 9)?;
    decoder.expect_version("mailbox-storage-offer-content-v1.schema_version")?;
    let provider_endpoint = decoder
        .bytes("mailbox-storage-offer-content-v1.provider_endpoint")?
        .to_vec();
    let store_key = decoder.fixed_bytes("mailbox-storage-offer-content-v1.store_key")?;
    let policy_code = decoder.unsigned("mailbox-storage-offer-content-v1.policy_class")?;
    let policy_class = match policy_code {
        1 => CanonicalMailboxStoragePolicyClassV1::BoundedVolunteer,
        other => return Err(CanonicalMailboxError::UnknownStoragePolicy(other)),
    };
    let content = CanonicalMailboxStorageOfferContentV1 {
        provider_endpoint,
        store_key,
        policy_class,
        capacity_hint_bytes: decoder
            .unsigned("mailbox-storage-offer-content-v1.capacity_hint_bytes")?,
        max_record_bytes: decoder.unsigned("mailbox-storage-offer-content-v1.max_record_bytes")?,
        issued_at_unix_seconds: decoder
            .unsigned("mailbox-storage-offer-content-v1.issued_at_unix_seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("mailbox-storage-offer-content-v1.expires_at_unix_seconds")?,
        nonce: decoder.fixed_bytes("mailbox-storage-offer-content-v1.nonce")?,
    };
    validate_storage_offer_content(&content)?;
    Ok(content)
}

fn encode_stored_receipt_content_into(
    content: &CanonicalMailboxStoredReceiptContentV1,
    output: &mut Vec<u8>,
) {
    push_array_header(output, 8);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_bytes(output, &content.store_key);
    push_bytes(output, &content.mailbox_id);
    push_bytes(output, &content.item_id);
    push_unsigned(output, content.envelope_bytes);
    push_bytes(output, &content.envelope_digest);
    push_unsigned(output, content.stored_at_unix_seconds);
    push_unsigned(output, content.expires_at_unix_seconds);
}

fn decode_stored_receipt_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalMailboxStoredReceiptContentV1, CanonicalMailboxError> {
    decoder.expect_array("mailbox-stored-receipt-content-v1", 8)?;
    decoder.expect_version("mailbox-stored-receipt-content-v1.schema_version")?;
    let content = CanonicalMailboxStoredReceiptContentV1 {
        store_key: decoder.fixed_bytes("mailbox-stored-receipt-content-v1.store_key")?,
        mailbox_id: decoder.fixed_bytes("mailbox-stored-receipt-content-v1.mailbox_id")?,
        item_id: decoder.fixed_bytes("mailbox-stored-receipt-content-v1.item_id")?,
        envelope_bytes: decoder.unsigned("mailbox-stored-receipt-content-v1.envelope_bytes")?,
        envelope_digest: decoder
            .fixed_bytes("mailbox-stored-receipt-content-v1.envelope_digest")?,
        stored_at_unix_seconds: decoder
            .unsigned("mailbox-stored-receipt-content-v1.stored_at_unix_seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("mailbox-stored-receipt-content-v1.expires_at_unix_seconds")?,
    };
    validate_stored_receipt_content(&content)?;
    Ok(content)
}

fn encode_delete_receipt_content_into(
    content: &CanonicalMailboxDeleteReceiptContentV1,
    output: &mut Vec<u8>,
) {
    push_array_header(output, 7);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_bytes(output, &content.store_key);
    push_bytes(output, &content.mailbox_id);
    push_bytes(output, &content.item_id);
    push_bytes(output, &content.stored_receipt_id);
    push_unsigned(output, content.deleted_at_unix_seconds);
    push_unsigned(output, content.expires_at_unix_seconds);
}

fn decode_delete_receipt_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalMailboxDeleteReceiptContentV1, CanonicalMailboxError> {
    decoder.expect_array("mailbox-delete-receipt-content-v1", 7)?;
    decoder.expect_version("mailbox-delete-receipt-content-v1.schema_version")?;
    let content = CanonicalMailboxDeleteReceiptContentV1 {
        store_key: decoder.fixed_bytes("mailbox-delete-receipt-content-v1.store_key")?,
        mailbox_id: decoder.fixed_bytes("mailbox-delete-receipt-content-v1.mailbox_id")?,
        item_id: decoder.fixed_bytes("mailbox-delete-receipt-content-v1.item_id")?,
        stored_receipt_id: decoder
            .fixed_bytes("mailbox-delete-receipt-content-v1.stored_receipt_id")?,
        deleted_at_unix_seconds: decoder
            .unsigned("mailbox-delete-receipt-content-v1.deleted_at_unix_seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("mailbox-delete-receipt-content-v1.expires_at_unix_seconds")?,
    };
    validate_delete_receipt_content(&content)?;
    Ok(content)
}

fn validate_address(address: &CanonicalMailboxAddressV1) -> Result<(), CanonicalMailboxError> {
    validate_public_key(&address.read_key, "read key")?;
    validate_public_key(&address.write_key, "write key")
}

fn validate_envelope_context(
    context: &CanonicalMailboxEnvelopeContextV1,
) -> Result<(), CanonicalMailboxError> {
    validate_ttl(
        context.created_at_unix_seconds,
        context.expires_at_unix_seconds,
    )
}

fn validate_envelope(envelope: &CanonicalMailboxEnvelopeV1) -> Result<(), CanonicalMailboxError> {
    validate_envelope_context(&envelope.context)?;
    if envelope.ciphertext.is_empty() {
        return Err(CanonicalMailboxError::InvalidEnvelope);
    }
    Ok(())
}

fn validate_write_content(
    content: &CanonicalMailboxWriteAuthorizationContentV1,
) -> Result<(), CanonicalMailboxError> {
    validate_public_key(&content.write_key, "write authorization")?;
    if !(MIN_CANONICAL_MAILBOX_TTL_SECONDS..=MAX_CANONICAL_MAILBOX_TTL_SECONDS)
        .contains(&content.requested_ttl_seconds)
        || content.envelope_bytes == 0
        || content.envelope_bytes > MAX_CANONICAL_MAILBOX_ENVELOPE_BYTES as u64
    {
        return Err(CanonicalMailboxError::AuthorizationMismatch);
    }
    Ok(())
}

fn validate_read_content(
    content: &CanonicalMailboxReadAuthorizationContentV1,
) -> Result<(), CanonicalMailboxError> {
    validate_public_key(&content.read_key, "read authorization")?;
    if let CanonicalMailboxReadOperationV1::List { limit, .. } = content.operation
        && !(1..=MAX_CANONICAL_MAILBOX_PAGE_ITEMS).contains(&limit)
    {
        return Err(CanonicalMailboxError::InvalidPageLimit);
    }
    Ok(())
}

fn validate_storage_offer_content(
    content: &CanonicalMailboxStorageOfferContentV1,
) -> Result<(), CanonicalMailboxError> {
    validate_public_key(&content.store_key, "storage offer")?;
    let validity = content
        .expires_at_unix_seconds
        .checked_sub(content.issued_at_unix_seconds)
        .ok_or(CanonicalMailboxError::InvalidStorageOffer)?;
    if content.provider_endpoint.is_empty()
        || content.provider_endpoint.len() > MAX_CANONICAL_MAILBOX_STORAGE_ENDPOINT_BYTES
        || !(MIN_CANONICAL_MAILBOX_STORAGE_OFFER_VALIDITY_SECONDS
            ..=MAX_CANONICAL_MAILBOX_STORAGE_OFFER_VALIDITY_SECONDS)
            .contains(&validity)
        || content.capacity_hint_bytes == 0
        || content.max_record_bytes == 0
        || content.max_record_bytes > content.capacity_hint_bytes
    {
        return Err(CanonicalMailboxError::InvalidStorageOffer);
    }
    Ok(())
}

fn validate_stored_receipt_content(
    content: &CanonicalMailboxStoredReceiptContentV1,
) -> Result<(), CanonicalMailboxError> {
    validate_public_key(&content.store_key, "stored receipt")?;
    if content.envelope_bytes == 0
        || content.envelope_bytes > MAX_CANONICAL_MAILBOX_ENVELOPE_BYTES as u64
        || content.stored_at_unix_seconds >= content.expires_at_unix_seconds
    {
        return Err(CanonicalMailboxError::InvalidReceipt);
    }
    Ok(())
}

fn validate_delete_receipt_content(
    content: &CanonicalMailboxDeleteReceiptContentV1,
) -> Result<(), CanonicalMailboxError> {
    validate_public_key(&content.store_key, "delete receipt")?;
    if content.deleted_at_unix_seconds >= content.expires_at_unix_seconds {
        return Err(CanonicalMailboxError::InvalidReceipt);
    }
    Ok(())
}

fn validate_ttl(created: u64, expires: u64) -> Result<(), CanonicalMailboxError> {
    let ttl = expires
        .checked_sub(created)
        .ok_or(CanonicalMailboxError::InvalidTtl)?;
    if !(MIN_CANONICAL_MAILBOX_TTL_SECONDS..=MAX_CANONICAL_MAILBOX_TTL_SECONDS).contains(&ttl) {
        return Err(CanonicalMailboxError::InvalidTtl);
    }
    Ok(())
}

fn validate_public_key(key: &[u8; 32], object: &'static str) -> Result<(), CanonicalMailboxError> {
    VerifyingKey::from_bytes(key)
        .map(|_| ())
        .map_err(|_| CanonicalMailboxError::InvalidPublicKey { object })
}

fn verify_ed25519(
    key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
    object: &'static str,
) -> Result<(), CanonicalMailboxError> {
    let key = VerifyingKey::from_bytes(key)
        .map_err(|_| CanonicalMailboxError::InvalidPublicKey { object })?;
    key.verify_strict(message, &Signature::from_bytes(signature))
        .map_err(|_| CanonicalMailboxError::InvalidSignature { object })
}

fn ensure_bound(
    bytes: &[u8],
    object: &'static str,
    maximum: usize,
) -> Result<(), CanonicalMailboxError> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(CanonicalMailboxError::ObjectTooLarge { object, maximum });
    }
    Ok(())
}

fn domain_hash(domain: &[u8], encoded: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(encoded);
    *hasher.finalize().as_bytes()
}

fn leading_zero_bits(digest: &[u8; 32]) -> u16 {
    let mut bits = 0_u16;
    for byte in digest {
        if *byte == 0 {
            bits += 8;
        } else {
            bits += byte.leading_zeros() as u16;
            break;
        }
    }
    bits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        (0..value.len())
            .step_by(2)
            .map(|index| Ok(u8::from_str_radix(&value[index..index + 2], 16)?))
            .collect()
    }

    fn vector_bytes(
        expected: &serde_json::Value,
        pointer: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        decode_hex(
            expected
                .pointer(pointer)
                .and_then(serde_json::Value::as_str)
                .ok_or("canonical mailbox vector field is missing")?,
        )
    }

    #[test]
    fn golden_mailbox_vectors_decode_and_bind() -> Result<(), Box<dyn std::error::Error>> {
        let vectors: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-MAILBOX-VECTORS.json"
        )))?;
        let expected = &vectors["expected"];

        let address =
            decode_and_verify_mailbox_address_v1(&vector_bytes(expected, "/address_cbor_hex")?)?;
        assert_eq!(
            mailbox_id_v1(&address)?.to_vec(),
            vector_bytes(expected, "/mailbox_id_hex")?
        );
        let envelope =
            decode_and_verify_mailbox_envelope_v1(&vector_bytes(expected, "/envelope/cbor_hex")?)?;
        assert_eq!(
            mailbox_envelope_aad_v1(&envelope.context)?,
            vector_bytes(expected, "/envelope/aad_cbor_hex")?
        );
        assert_eq!(
            mailbox_envelope_digest_v1(&envelope)?.to_vec(),
            vector_bytes(expected, "/envelope/digest_hex")?
        );

        let write = decode_and_verify_signed_mailbox_write_authorization_v1(&vector_bytes(
            expected,
            "/write_authorization/signed_cbor_hex",
        )?)?;
        verify_mailbox_write_authorization_for_v1(&write, &address, &envelope)?;
        for pointer in [
            "/read_list_authorization/signed_cbor_hex",
            "/read_delete_authorization/signed_cbor_hex",
        ] {
            let read = decode_and_verify_signed_mailbox_read_authorization_v1(&vector_bytes(
                expected, pointer,
            )?)?;
            verify_mailbox_read_authorization_for_v1(&read, &address)?;
        }
        let offer = decode_and_verify_signed_mailbox_storage_offer_v1(&vector_bytes(
            expected,
            "/storage_offer/signed_cbor_hex",
        )?)?;
        verify_mailbox_storage_offer_at_v1(&offer, 1_800_000_100)?;
        assert_eq!(
            mailbox_storage_offer_admission_work_bits_v1(&offer.content)? as u64,
            expected["storage_offer"]["admission_work_bits"]
                .as_u64()
                .ok_or("canonical mailbox work-bits vector is missing")?
        );
        let stored = decode_and_verify_signed_mailbox_stored_receipt_v1(&vector_bytes(
            expected,
            "/stored_receipt/signed_cbor_hex",
        )?)?;
        verify_mailbox_stored_receipt_for_v1(&stored, &envelope)?;
        assert_eq!(
            mailbox_stored_receipt_id_v1(&stored)?.to_vec(),
            vector_bytes(expected, "/stored_receipt/id_hex")?
        );
        let deleted = decode_and_verify_signed_mailbox_delete_receipt_v1(&vector_bytes(
            expected,
            "/delete_receipt/signed_cbor_hex",
        )?)?;
        verify_mailbox_delete_receipt_for_v1(&deleted, &stored)?;
        assert_eq!(
            mailbox_delete_receipt_id_v1(&deleted)?.to_vec(),
            vector_bytes(expected, "/delete_receipt/id_hex")?
        );
        Ok(())
    }

    #[test]
    fn semantic_substitutions_fail_closed() {
        let context = CanonicalMailboxEnvelopeContextV1 {
            mailbox_id: [1; 32],
            item_id: [2; 32],
            created_at_unix_seconds: 100,
            expires_at_unix_seconds: 100,
        };
        assert!(encode_mailbox_envelope_context_v1(&context).is_err());
        let read_content = CanonicalMailboxReadAuthorizationContentV1 {
            mailbox_id: [3; 32],
            read_key: [4; 32],
            operation: CanonicalMailboxReadOperationV1::List {
                nonce: [5; 32],
                after_item_id: None,
                limit: MAX_CANONICAL_MAILBOX_PAGE_ITEMS + 1,
            },
        };
        assert!(encode_mailbox_read_authorization_content_v1(&read_content).is_err());
    }
}
