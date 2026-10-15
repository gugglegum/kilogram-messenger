//! Inactive deterministic-CBOR history-rewrap candidate for M1.1.25.
//!
//! The active runtime remains the Postcard v1 history-rewrap protocol. This
//! module specifies a portable recipient-only HPKE representation without
//! assigning it an active wire identifier.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use kilogram_crypto::{DeviceEncryptionIdentity, EncryptionPublicKey, SealedMessage};
use thiserror::Error;

use crate::{
    canonical_account_v1::{
        AccountDecoder, CanonicalAccountError, CanonicalSignedAccountDeviceListV1,
        CanonicalSignedConversationMembershipV1, decode_and_verify_signed_account_device_list_v1,
        decode_and_verify_signed_conversation_membership_v1, encode_signed_account_device_list_v1,
        encode_signed_conversation_membership_v1, signed_envelope,
        verify_signed_account_device_list_v1, verify_signed_conversation_membership_v1,
    },
    canonical_event_v1::{
        CanonicalAuthorizedEventV1, CanonicalEventError, CanonicalEventPayloadV1,
        decode_and_verify_authorized_event_v1, encode_authorized_event_v1, event_id_v1,
        verify_authorized_event_for_membership_v1,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
        push_unsigned, validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_HISTORY_REWRAP_ENTRIES: usize = 256;
pub const MAX_CANONICAL_HISTORY_REWRAP_INVENTORY_EVENTS: usize = 4096;
pub const MAX_CANONICAL_HISTORY_REWRAP_PLAINTEXT_BYTES: usize = 64 * 1024;
pub const MAX_CANONICAL_HISTORY_REWRAP_CIPHERTEXT_BYTES: usize =
    MAX_CANONICAL_HISTORY_REWRAP_PLAINTEXT_BYTES + 16;
pub const MAX_CANONICAL_HISTORY_REWRAP_NESTED_BYTES: usize = 1024 * 1024;
pub const MAX_CANONICAL_HISTORY_REWRAP_BUNDLE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_CANONICAL_HISTORY_REWRAP_TRANSFER_BYTES: usize = 9 * 1024 * 1024;

pub const CANONICAL_HISTORY_REWRAP_SAS_DOMAIN: &[u8] = b"kilogram:history-rewrap-sas:cbor-v1\0";
pub const CANONICAL_HISTORY_REWRAP_INVENTORY_DOMAIN: &[u8] =
    b"kilogram:history-rewrap-inventory:cbor-v1\0";
pub const CANONICAL_HISTORY_REWRAP_REQUEST_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:history-rewrap-request-signature:cbor-v1\0";
pub const CANONICAL_HISTORY_REWRAP_MANIFEST_ID_DOMAIN: &[u8] =
    b"kilogram:history-rewrap-manifest-id:cbor-v1\0";
pub const CANONICAL_HISTORY_REWRAP_ENTRY_HPKE_INFO: &[u8] =
    b"kilogram:history-rewrap-entry-hpke:cbor-v1\0";
pub const CANONICAL_HISTORY_REWRAP_ENTRY_AAD_DOMAIN: &[u8] =
    b"kilogram:history-rewrap-entry-aad:cbor-v1\0";
pub const CANONICAL_HISTORY_REWRAP_ENTRY_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:history-rewrap-entry-signature:cbor-v1\0";
pub const CANONICAL_HISTORY_REWRAP_BUNDLE_ID_DOMAIN: &[u8] =
    b"kilogram:history-rewrap-bundle-id:cbor-v1\0";
pub const CANONICAL_HISTORY_REWRAP_TRANSFER_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:history-rewrap-transfer-signature:cbor-v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalHistoryRewrapRequestContentV1 {
    pub conversation_id: [u8; 32],
    pub source_device_id: [u8; 32],
    pub recipient_device_id: [u8; 32],
    pub session_binding: [u8; 32],
    pub range_start: u64,
    pub max_event_count: u64,
    pub sas: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedHistoryRewrapRequestV1 {
    pub content: CanonicalHistoryRewrapRequestContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalHistoryRewrapManifestV1 {
    pub conversation_id: [u8; 32],
    pub account_device_list: CanonicalSignedAccountDeviceListV1,
    pub conversation_membership: CanonicalSignedConversationMembershipV1,
    pub source_device_id: [u8; 32],
    pub recipient_device_id: [u8; 32],
    pub inventory_event_count: u64,
    pub inventory_digest: [u8; 32],
    pub range_start: u64,
    pub range_end: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalHistoryRewrapEntryContentV1 {
    pub manifest_id: [u8; 32],
    pub source_device_id: [u8; 32],
    pub inventory_index: u64,
    pub event: CanonicalAuthorizedEventV1,
    pub encapsulated_key: [u8; 32],
    pub ciphertext: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedHistoryRewrapEntryV1 {
    pub content: CanonicalHistoryRewrapEntryContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalHistoryRewrapBundleV1 {
    pub manifest: CanonicalHistoryRewrapManifestV1,
    pub entries: Vec<CanonicalSignedHistoryRewrapEntryV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalHistoryRewrapTransferContentV1 {
    pub request: CanonicalSignedHistoryRewrapRequestV1,
    pub bundle: CanonicalHistoryRewrapBundleV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedHistoryRewrapTransferV1 {
    pub content: CanonicalHistoryRewrapTransferContentV1,
    pub signature: [u8; 64],
}

#[derive(Debug, Error)]
pub enum CanonicalHistoryRewrapError {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),
    #[error(transparent)]
    Account(#[from] CanonicalAccountError),
    #[error(transparent)]
    Event(#[from] CanonicalEventError),
    #[error(transparent)]
    Crypto(#[from] kilogram_crypto::CryptoError),
    #[error("canonical history-rewrap {object} exceeds {maximum} bytes")]
    ObjectTooLarge {
        object: &'static str,
        maximum: usize,
    },
    #[error("canonical history-rewrap request is invalid")]
    InvalidRequest,
    #[error("canonical history-rewrap manifest is invalid")]
    InvalidManifest,
    #[error("canonical history-rewrap entry is invalid")]
    InvalidEntry,
    #[error("canonical history-rewrap bundle is invalid")]
    InvalidBundle,
    #[error("canonical history-rewrap transfer is invalid")]
    InvalidTransfer,
    #[error("canonical history-rewrap contains an invalid Ed25519 key")]
    InvalidPublicKey,
    #[error("canonical history-rewrap signature is invalid")]
    InvalidSignature,
    #[error("canonical history-rewrap recipient encryption key does not match the roster")]
    RecipientEncryptionKeyMismatch,
    #[error("canonical history-rewrap plaintext is not valid UTF-8")]
    InvalidPlaintext,
}

pub fn history_rewrap_sas_input_v1(
    account_device_list: &CanonicalSignedAccountDeviceListV1,
    source_device_id: [u8; 32],
    recipient_device_id: [u8; 32],
) -> Result<Vec<u8>, CanonicalHistoryRewrapError> {
    verify_signed_account_device_list_v1(account_device_list)?;
    validate_rewrap_devices(account_device_list, source_device_id, recipient_device_id)?;
    let list = encode_signed_account_device_list_v1(account_device_list)?;
    ensure_bound(
        &list,
        "account device list",
        MAX_CANONICAL_HISTORY_REWRAP_NESTED_BYTES,
    )?;
    let mut output = Vec::new();
    push_array_header(&mut output, 4);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &list);
    push_bytes(&mut output, &source_device_id);
    push_bytes(&mut output, &recipient_device_id);
    Ok(output)
}

pub fn history_rewrap_sas_v1(
    account_device_list: &CanonicalSignedAccountDeviceListV1,
    source_device_id: [u8; 32],
    recipient_device_id: [u8; 32],
) -> Result<[u8; 32], CanonicalHistoryRewrapError> {
    Ok(domain_hash(
        CANONICAL_HISTORY_REWRAP_SAS_DOMAIN,
        &history_rewrap_sas_input_v1(account_device_list, source_device_id, recipient_device_id)?,
    ))
}

pub fn history_rewrap_inventory_input_v1(
    conversation_id: [u8; 32],
    event_ids: &[[u8; 32]],
) -> Result<Vec<u8>, CanonicalHistoryRewrapError> {
    if conversation_id == [0; 32]
        || event_ids.is_empty()
        || event_ids.len() > MAX_CANONICAL_HISTORY_REWRAP_INVENTORY_EVENTS
        || event_ids.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(CanonicalHistoryRewrapError::InvalidManifest);
    }
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &conversation_id);
    push_array_header(&mut output, event_ids.len() as u64);
    for event_id in event_ids {
        push_bytes(&mut output, event_id);
    }
    Ok(output)
}

pub fn history_rewrap_inventory_digest_v1(
    conversation_id: [u8; 32],
    event_ids: &[[u8; 32]],
) -> Result<[u8; 32], CanonicalHistoryRewrapError> {
    Ok(domain_hash(
        CANONICAL_HISTORY_REWRAP_INVENTORY_DOMAIN,
        &history_rewrap_inventory_input_v1(conversation_id, event_ids)?,
    ))
}

pub fn encode_history_rewrap_request_content_v1(
    content: &CanonicalHistoryRewrapRequestContentV1,
) -> Result<Vec<u8>, CanonicalHistoryRewrapError> {
    validate_request_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 8);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.conversation_id);
    push_bytes(&mut output, &content.source_device_id);
    push_bytes(&mut output, &content.recipient_device_id);
    push_bytes(&mut output, &content.session_binding);
    push_unsigned(&mut output, content.range_start);
    push_unsigned(&mut output, content.max_event_count);
    push_bytes(&mut output, &content.sas);
    Ok(output)
}

pub fn history_rewrap_request_signing_bytes_v1(
    content: &CanonicalHistoryRewrapRequestContentV1,
) -> Result<Vec<u8>, CanonicalHistoryRewrapError> {
    Ok(prefixed(
        CANONICAL_HISTORY_REWRAP_REQUEST_SIGNATURE_DOMAIN,
        &encode_history_rewrap_request_content_v1(content)?,
    ))
}

pub fn sign_history_rewrap_request_v1(
    recipient_signing_key: &SigningKey,
    mut content: CanonicalHistoryRewrapRequestContentV1,
) -> Result<CanonicalSignedHistoryRewrapRequestV1, CanonicalHistoryRewrapError> {
    content.recipient_device_id = recipient_signing_key.verifying_key().to_bytes();
    let signature = recipient_signing_key
        .sign(&history_rewrap_request_signing_bytes_v1(&content)?)
        .to_bytes();
    let request = CanonicalSignedHistoryRewrapRequestV1 { content, signature };
    verify_signed_history_rewrap_request_v1(&request)?;
    Ok(request)
}

pub fn encode_signed_history_rewrap_request_v1(
    request: &CanonicalSignedHistoryRewrapRequestV1,
) -> Result<Vec<u8>, CanonicalHistoryRewrapError> {
    verify_signed_history_rewrap_request_v1(request)?;
    Ok(signed_envelope(
        &encode_history_rewrap_request_content_v1(&request.content)?,
        &request.signature,
    ))
}

pub fn decode_and_verify_signed_history_rewrap_request_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedHistoryRewrapRequestV1, CanonicalHistoryRewrapError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-history-rewrap-request-v1", 3)?;
    decoder.expect_version("signed-history-rewrap-request-v1.schema_version")?;
    let content = decode_history_rewrap_request_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-history-rewrap-request-v1.signature")?;
    decoder.finish()?;
    let request = CanonicalSignedHistoryRewrapRequestV1 { content, signature };
    verify_signed_history_rewrap_request_v1(&request)?;
    if encode_signed_history_rewrap_request_v1(&request)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(request)
}

pub fn verify_signed_history_rewrap_request_v1(
    request: &CanonicalSignedHistoryRewrapRequestV1,
) -> Result<(), CanonicalHistoryRewrapError> {
    validate_request_content(&request.content)?;
    verify_ed25519(
        &request.content.recipient_device_id,
        &history_rewrap_request_signing_bytes_v1(&request.content)?,
        &request.signature,
    )
}

pub fn encode_history_rewrap_manifest_v1(
    manifest: &CanonicalHistoryRewrapManifestV1,
) -> Result<Vec<u8>, CanonicalHistoryRewrapError> {
    validate_manifest(manifest)?;
    let list = encode_signed_account_device_list_v1(&manifest.account_device_list)?;
    let membership = encode_signed_conversation_membership_v1(&manifest.conversation_membership)?;
    for nested in [&list, &membership] {
        ensure_bound(
            nested,
            "manifest nested object",
            MAX_CANONICAL_HISTORY_REWRAP_NESTED_BYTES,
        )?;
    }
    let mut output = Vec::new();
    push_array_header(&mut output, 10);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &manifest.conversation_id);
    push_bytes(&mut output, &list);
    push_bytes(&mut output, &membership);
    push_bytes(&mut output, &manifest.source_device_id);
    push_bytes(&mut output, &manifest.recipient_device_id);
    push_unsigned(&mut output, manifest.inventory_event_count);
    push_bytes(&mut output, &manifest.inventory_digest);
    push_unsigned(&mut output, manifest.range_start);
    push_unsigned(&mut output, manifest.range_end);
    Ok(output)
}

pub fn decode_and_verify_history_rewrap_manifest_v1(
    bytes: &[u8],
) -> Result<CanonicalHistoryRewrapManifestV1, CanonicalHistoryRewrapError> {
    ensure_bound(bytes, "manifest", MAX_CANONICAL_HISTORY_REWRAP_NESTED_BYTES)?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("history-rewrap-manifest-v1", 10)?;
    decoder.expect_version("history-rewrap-manifest-v1.schema_version")?;
    let conversation_id = decoder.fixed_bytes("history-rewrap-manifest-v1.conversation_id")?;
    let list_bytes = decoder.bytes("history-rewrap-manifest-v1.account_device_list")?;
    let membership_bytes = decoder.bytes("history-rewrap-manifest-v1.conversation_membership")?;
    for nested in [list_bytes, membership_bytes] {
        ensure_bound(
            nested,
            "manifest nested object",
            MAX_CANONICAL_HISTORY_REWRAP_NESTED_BYTES,
        )?;
    }
    let manifest = CanonicalHistoryRewrapManifestV1 {
        conversation_id,
        account_device_list: decode_and_verify_signed_account_device_list_v1(list_bytes)?,
        conversation_membership: decode_and_verify_signed_conversation_membership_v1(
            membership_bytes,
        )?,
        source_device_id: decoder.fixed_bytes("history-rewrap-manifest-v1.source_device_id")?,
        recipient_device_id: decoder
            .fixed_bytes("history-rewrap-manifest-v1.recipient_device_id")?,
        inventory_event_count: decoder
            .unsigned("history-rewrap-manifest-v1.inventory_event_count")?,
        inventory_digest: decoder.fixed_bytes("history-rewrap-manifest-v1.inventory_digest")?,
        range_start: decoder.unsigned("history-rewrap-manifest-v1.range_start")?,
        range_end: decoder.unsigned("history-rewrap-manifest-v1.range_end")?,
    };
    decoder.finish()?;
    validate_manifest(&manifest)?;
    if encode_history_rewrap_manifest_v1(&manifest)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(manifest)
}

pub fn history_rewrap_manifest_id_v1(
    manifest: &CanonicalHistoryRewrapManifestV1,
) -> Result<[u8; 32], CanonicalHistoryRewrapError> {
    Ok(domain_hash(
        CANONICAL_HISTORY_REWRAP_MANIFEST_ID_DOMAIN,
        &encode_history_rewrap_manifest_v1(manifest)?,
    ))
}

pub fn verify_history_rewrap_request_for_manifest_v1(
    request: &CanonicalSignedHistoryRewrapRequestV1,
    expected_session_binding: [u8; 32],
    manifest: &CanonicalHistoryRewrapManifestV1,
) -> Result<(), CanonicalHistoryRewrapError> {
    verify_signed_history_rewrap_request_v1(request)?;
    validate_manifest(manifest)?;
    let maximum_end = request
        .content
        .range_start
        .checked_add(request.content.max_event_count)
        .ok_or(CanonicalHistoryRewrapError::InvalidRequest)?;
    if request.content.session_binding != expected_session_binding
        || request.content.conversation_id != manifest.conversation_id
        || request.content.source_device_id != manifest.source_device_id
        || request.content.recipient_device_id != manifest.recipient_device_id
        || request.content.sas
            != history_rewrap_sas_v1(
                &manifest.account_device_list,
                manifest.source_device_id,
                manifest.recipient_device_id,
            )?
        || request.content.range_start != manifest.range_start
        || manifest.range_end > maximum_end
    {
        return Err(CanonicalHistoryRewrapError::InvalidRequest);
    }
    Ok(())
}

pub fn encode_history_rewrap_entry_content_v1(
    content: &CanonicalHistoryRewrapEntryContentV1,
) -> Result<Vec<u8>, CanonicalHistoryRewrapError> {
    validate_entry_content(content)?;
    let event = encode_authorized_event_v1(&content.event)?;
    ensure_bound(
        &event,
        "authorized event",
        MAX_CANONICAL_HISTORY_REWRAP_NESTED_BYTES,
    )?;
    let mut output = Vec::new();
    push_array_header(&mut output, 7);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.manifest_id);
    push_bytes(&mut output, &content.source_device_id);
    push_unsigned(&mut output, content.inventory_index);
    push_bytes(&mut output, &event);
    push_bytes(&mut output, &content.encapsulated_key);
    push_bytes(&mut output, &content.ciphertext);
    Ok(output)
}

pub fn history_rewrap_entry_signing_bytes_v1(
    content: &CanonicalHistoryRewrapEntryContentV1,
) -> Result<Vec<u8>, CanonicalHistoryRewrapError> {
    Ok(prefixed(
        CANONICAL_HISTORY_REWRAP_ENTRY_SIGNATURE_DOMAIN,
        &encode_history_rewrap_entry_content_v1(content)?,
    ))
}

pub fn sign_history_rewrap_entry_v1(
    source_signing_key: &SigningKey,
    content: CanonicalHistoryRewrapEntryContentV1,
) -> Result<CanonicalSignedHistoryRewrapEntryV1, CanonicalHistoryRewrapError> {
    if content.source_device_id != source_signing_key.verifying_key().to_bytes() {
        return Err(CanonicalHistoryRewrapError::InvalidEntry);
    }
    let signature = source_signing_key
        .sign(&history_rewrap_entry_signing_bytes_v1(&content)?)
        .to_bytes();
    let entry = CanonicalSignedHistoryRewrapEntryV1 { content, signature };
    verify_signed_history_rewrap_entry_v1(&entry)?;
    Ok(entry)
}

pub fn seal_history_rewrap_entry_v1(
    source_signing_key: &SigningKey,
    manifest: &CanonicalHistoryRewrapManifestV1,
    inventory_index: u64,
    event: CanonicalAuthorizedEventV1,
    plaintext: &str,
) -> Result<CanonicalSignedHistoryRewrapEntryV1, CanonicalHistoryRewrapError> {
    validate_manifest(manifest)?;
    validate_entry_event_for_manifest(&event, inventory_index, manifest)?;
    if plaintext.len() > MAX_CANONICAL_HISTORY_REWRAP_PLAINTEXT_BYTES {
        return Err(CanonicalHistoryRewrapError::InvalidPlaintext);
    }
    let manifest_id = history_rewrap_manifest_id_v1(manifest)?;
    let event_id = event_id_v1(&event.event)?;
    let aad = history_rewrap_entry_aad_v1(manifest_id, inventory_index, event_id);
    let recipient_key = recipient_encryption_key(manifest)?;
    let sealed = recipient_key.seal(
        plaintext.as_bytes(),
        CANONICAL_HISTORY_REWRAP_ENTRY_HPKE_INFO,
        &aad,
    )?;
    let encapsulated_key: [u8; 32] = sealed
        .encapsulated_key
        .try_into()
        .map_err(|_| CanonicalHistoryRewrapError::InvalidEntry)?;
    sign_history_rewrap_entry_v1(
        source_signing_key,
        CanonicalHistoryRewrapEntryContentV1 {
            manifest_id,
            source_device_id: manifest.source_device_id,
            inventory_index,
            event,
            encapsulated_key,
            ciphertext: sealed.ciphertext,
        },
    )
}

pub fn encode_signed_history_rewrap_entry_v1(
    entry: &CanonicalSignedHistoryRewrapEntryV1,
) -> Result<Vec<u8>, CanonicalHistoryRewrapError> {
    verify_signed_history_rewrap_entry_v1(entry)?;
    let encoded = signed_envelope(
        &encode_history_rewrap_entry_content_v1(&entry.content)?,
        &entry.signature,
    );
    ensure_bound(
        &encoded,
        "signed entry",
        MAX_CANONICAL_HISTORY_REWRAP_NESTED_BYTES,
    )?;
    Ok(encoded)
}

pub fn decode_and_verify_signed_history_rewrap_entry_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedHistoryRewrapEntryV1, CanonicalHistoryRewrapError> {
    ensure_bound(
        bytes,
        "signed entry",
        MAX_CANONICAL_HISTORY_REWRAP_NESTED_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-history-rewrap-entry-v1", 3)?;
    decoder.expect_version("signed-history-rewrap-entry-v1.schema_version")?;
    let content = decode_history_rewrap_entry_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-history-rewrap-entry-v1.signature")?;
    decoder.finish()?;
    let entry = CanonicalSignedHistoryRewrapEntryV1 { content, signature };
    verify_signed_history_rewrap_entry_v1(&entry)?;
    if encode_signed_history_rewrap_entry_v1(&entry)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(entry)
}

pub fn verify_signed_history_rewrap_entry_v1(
    entry: &CanonicalSignedHistoryRewrapEntryV1,
) -> Result<(), CanonicalHistoryRewrapError> {
    validate_entry_content(&entry.content)?;
    verify_ed25519(
        &entry.content.source_device_id,
        &history_rewrap_entry_signing_bytes_v1(&entry.content)?,
        &entry.signature,
    )
}

pub fn verify_history_rewrap_entry_for_manifest_v1(
    entry: &CanonicalSignedHistoryRewrapEntryV1,
    manifest: &CanonicalHistoryRewrapManifestV1,
) -> Result<(), CanonicalHistoryRewrapError> {
    verify_signed_history_rewrap_entry_v1(entry)?;
    validate_manifest(manifest)?;
    if entry.content.manifest_id != history_rewrap_manifest_id_v1(manifest)?
        || entry.content.source_device_id != manifest.source_device_id
    {
        return Err(CanonicalHistoryRewrapError::InvalidEntry);
    }
    validate_entry_event_for_manifest(
        &entry.content.event,
        entry.content.inventory_index,
        manifest,
    )
}

pub fn history_rewrap_entry_aad_input_v1(
    manifest_id: [u8; 32],
    inventory_index: u64,
    event_id: [u8; 32],
) -> Vec<u8> {
    let mut output = Vec::new();
    push_array_header(&mut output, 4);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &manifest_id);
    push_unsigned(&mut output, inventory_index);
    push_bytes(&mut output, &event_id);
    output
}

pub fn history_rewrap_entry_aad_v1(
    manifest_id: [u8; 32],
    inventory_index: u64,
    event_id: [u8; 32],
) -> Vec<u8> {
    prefixed(
        CANONICAL_HISTORY_REWRAP_ENTRY_AAD_DOMAIN,
        &history_rewrap_entry_aad_input_v1(manifest_id, inventory_index, event_id),
    )
}

pub fn open_history_rewrap_entry_v1(
    entry: &CanonicalSignedHistoryRewrapEntryV1,
    manifest: &CanonicalHistoryRewrapManifestV1,
    recipient_encryption: &DeviceEncryptionIdentity,
) -> Result<String, CanonicalHistoryRewrapError> {
    verify_history_rewrap_entry_for_manifest_v1(entry, manifest)?;
    let expected_key = recipient_encryption_key(manifest)?;
    if recipient_encryption.public_key().as_bytes() != expected_key.as_bytes() {
        return Err(CanonicalHistoryRewrapError::RecipientEncryptionKeyMismatch);
    }
    let event_id = event_id_v1(&entry.content.event.event)?;
    let plaintext = recipient_encryption.open(
        &SealedMessage {
            encapsulated_key: entry.content.encapsulated_key.to_vec(),
            ciphertext: entry.content.ciphertext.clone(),
        },
        CANONICAL_HISTORY_REWRAP_ENTRY_HPKE_INFO,
        &history_rewrap_entry_aad_v1(
            entry.content.manifest_id,
            entry.content.inventory_index,
            event_id,
        ),
    )?;
    if plaintext.len() > MAX_CANONICAL_HISTORY_REWRAP_PLAINTEXT_BYTES {
        return Err(CanonicalHistoryRewrapError::InvalidPlaintext);
    }
    String::from_utf8(plaintext).map_err(|_| CanonicalHistoryRewrapError::InvalidPlaintext)
}

pub fn encode_history_rewrap_bundle_v1(
    bundle: &CanonicalHistoryRewrapBundleV1,
) -> Result<Vec<u8>, CanonicalHistoryRewrapError> {
    verify_history_rewrap_bundle_v1(bundle)?;
    let manifest = encode_history_rewrap_manifest_v1(&bundle.manifest)?;
    let mut entries = Vec::with_capacity(bundle.entries.len());
    for entry in &bundle.entries {
        entries.push(encode_signed_history_rewrap_entry_v1(entry)?);
    }
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &manifest);
    push_array_header(&mut output, entries.len() as u64);
    for entry in entries {
        push_bytes(&mut output, &entry);
    }
    ensure_bound(&output, "bundle", MAX_CANONICAL_HISTORY_REWRAP_BUNDLE_BYTES)?;
    Ok(output)
}

pub fn decode_and_verify_history_rewrap_bundle_v1(
    bytes: &[u8],
) -> Result<CanonicalHistoryRewrapBundleV1, CanonicalHistoryRewrapError> {
    ensure_bound(bytes, "bundle", MAX_CANONICAL_HISTORY_REWRAP_BUNDLE_BYTES)?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("history-rewrap-bundle-v1", 3)?;
    decoder.expect_version("history-rewrap-bundle-v1.schema_version")?;
    let manifest = decode_and_verify_history_rewrap_manifest_v1(
        decoder.bytes("history-rewrap-bundle-v1.manifest")?,
    )?;
    let count = decoder.array("history-rewrap-bundle-v1.entries")?;
    if count == 0 || count > MAX_CANONICAL_HISTORY_REWRAP_ENTRIES {
        return Err(CanonicalHistoryRewrapError::InvalidBundle);
    }
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        entries.push(decode_and_verify_signed_history_rewrap_entry_v1(
            decoder.bytes("history-rewrap-bundle-v1.entry")?,
        )?);
    }
    decoder.finish()?;
    let bundle = CanonicalHistoryRewrapBundleV1 { manifest, entries };
    verify_history_rewrap_bundle_v1(&bundle)?;
    if encode_history_rewrap_bundle_v1(&bundle)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(bundle)
}

pub fn verify_history_rewrap_bundle_v1(
    bundle: &CanonicalHistoryRewrapBundleV1,
) -> Result<(), CanonicalHistoryRewrapError> {
    validate_manifest(&bundle.manifest)?;
    let expected_count = usize::try_from(bundle.manifest.range_end - bundle.manifest.range_start)
        .map_err(|_| CanonicalHistoryRewrapError::InvalidBundle)?;
    if bundle.entries.len() != expected_count {
        return Err(CanonicalHistoryRewrapError::InvalidBundle);
    }
    let mut event_ids = Vec::with_capacity(bundle.entries.len());
    for (offset, entry) in bundle.entries.iter().enumerate() {
        let expected_index = bundle.manifest.range_start
            + u64::try_from(offset).map_err(|_| CanonicalHistoryRewrapError::InvalidBundle)?;
        if entry.content.inventory_index != expected_index {
            return Err(CanonicalHistoryRewrapError::InvalidBundle);
        }
        verify_history_rewrap_entry_for_manifest_v1(entry, &bundle.manifest)?;
        event_ids.push(event_id_v1(&entry.content.event.event)?);
    }
    if event_ids.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(CanonicalHistoryRewrapError::InvalidBundle);
    }
    if bundle.manifest.range_start == 0
        && bundle.manifest.range_end == bundle.manifest.inventory_event_count
        && history_rewrap_inventory_digest_v1(bundle.manifest.conversation_id, &event_ids)?
            != bundle.manifest.inventory_digest
    {
        return Err(CanonicalHistoryRewrapError::InvalidBundle);
    }
    Ok(())
}

pub fn history_rewrap_bundle_id_v1(
    bundle: &CanonicalHistoryRewrapBundleV1,
) -> Result<[u8; 32], CanonicalHistoryRewrapError> {
    Ok(domain_hash(
        CANONICAL_HISTORY_REWRAP_BUNDLE_ID_DOMAIN,
        &encode_history_rewrap_bundle_v1(bundle)?,
    ))
}

pub fn encode_history_rewrap_transfer_content_v1(
    content: &CanonicalHistoryRewrapTransferContentV1,
) -> Result<Vec<u8>, CanonicalHistoryRewrapError> {
    validate_transfer_content(content)?;
    let request = encode_signed_history_rewrap_request_v1(&content.request)?;
    let bundle = encode_history_rewrap_bundle_v1(&content.bundle)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &request);
    push_bytes(&mut output, &bundle);
    Ok(output)
}

pub fn history_rewrap_transfer_signing_bytes_v1(
    content: &CanonicalHistoryRewrapTransferContentV1,
) -> Result<Vec<u8>, CanonicalHistoryRewrapError> {
    Ok(prefixed(
        CANONICAL_HISTORY_REWRAP_TRANSFER_SIGNATURE_DOMAIN,
        &encode_history_rewrap_transfer_content_v1(content)?,
    ))
}

pub fn sign_history_rewrap_transfer_v1(
    source_signing_key: &SigningKey,
    content: CanonicalHistoryRewrapTransferContentV1,
) -> Result<CanonicalSignedHistoryRewrapTransferV1, CanonicalHistoryRewrapError> {
    if source_signing_key.verifying_key().to_bytes() != content.bundle.manifest.source_device_id {
        return Err(CanonicalHistoryRewrapError::InvalidTransfer);
    }
    let signature = source_signing_key
        .sign(&history_rewrap_transfer_signing_bytes_v1(&content)?)
        .to_bytes();
    let transfer = CanonicalSignedHistoryRewrapTransferV1 { content, signature };
    verify_signed_history_rewrap_transfer_v1(&transfer)?;
    Ok(transfer)
}

pub fn encode_signed_history_rewrap_transfer_v1(
    transfer: &CanonicalSignedHistoryRewrapTransferV1,
) -> Result<Vec<u8>, CanonicalHistoryRewrapError> {
    verify_signed_history_rewrap_transfer_v1(transfer)?;
    let encoded = signed_envelope(
        &encode_history_rewrap_transfer_content_v1(&transfer.content)?,
        &transfer.signature,
    );
    ensure_bound(
        &encoded,
        "signed transfer",
        MAX_CANONICAL_HISTORY_REWRAP_TRANSFER_BYTES,
    )?;
    Ok(encoded)
}

pub fn decode_and_verify_signed_history_rewrap_transfer_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedHistoryRewrapTransferV1, CanonicalHistoryRewrapError> {
    ensure_bound(
        bytes,
        "signed transfer",
        MAX_CANONICAL_HISTORY_REWRAP_TRANSFER_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-history-rewrap-transfer-v1", 3)?;
    decoder.expect_version("signed-history-rewrap-transfer-v1.schema_version")?;
    let content = decode_history_rewrap_transfer_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-history-rewrap-transfer-v1.signature")?;
    decoder.finish()?;
    let transfer = CanonicalSignedHistoryRewrapTransferV1 { content, signature };
    verify_signed_history_rewrap_transfer_v1(&transfer)?;
    if encode_signed_history_rewrap_transfer_v1(&transfer)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(transfer)
}

pub fn verify_signed_history_rewrap_transfer_v1(
    transfer: &CanonicalSignedHistoryRewrapTransferV1,
) -> Result<(), CanonicalHistoryRewrapError> {
    validate_transfer_content(&transfer.content)?;
    verify_ed25519(
        &transfer.content.bundle.manifest.source_device_id,
        &history_rewrap_transfer_signing_bytes_v1(&transfer.content)?,
        &transfer.signature,
    )
}

pub fn verify_history_rewrap_transfer_for_request_v1(
    transfer: &CanonicalSignedHistoryRewrapTransferV1,
    expected_request: &CanonicalSignedHistoryRewrapRequestV1,
    expected_session_binding: [u8; 32],
) -> Result<(), CanonicalHistoryRewrapError> {
    verify_signed_history_rewrap_transfer_v1(transfer)?;
    if &transfer.content.request != expected_request {
        return Err(CanonicalHistoryRewrapError::InvalidTransfer);
    }
    verify_history_rewrap_request_for_manifest_v1(
        expected_request,
        expected_session_binding,
        &transfer.content.bundle.manifest,
    )
}

fn decode_history_rewrap_request_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalHistoryRewrapRequestContentV1, CanonicalHistoryRewrapError> {
    decoder.expect_array("history-rewrap-request-content-v1", 8)?;
    decoder.expect_version("history-rewrap-request-content-v1.schema_version")?;
    let content = CanonicalHistoryRewrapRequestContentV1 {
        conversation_id: decoder
            .fixed_bytes("history-rewrap-request-content-v1.conversation_id")?,
        source_device_id: decoder
            .fixed_bytes("history-rewrap-request-content-v1.source_device_id")?,
        recipient_device_id: decoder
            .fixed_bytes("history-rewrap-request-content-v1.recipient_device_id")?,
        session_binding: decoder
            .fixed_bytes("history-rewrap-request-content-v1.session_binding")?,
        range_start: decoder.unsigned("history-rewrap-request-content-v1.range_start")?,
        max_event_count: decoder.unsigned("history-rewrap-request-content-v1.max_event_count")?,
        sas: decoder.fixed_bytes("history-rewrap-request-content-v1.sas")?,
    };
    validate_request_content(&content)?;
    Ok(content)
}

fn decode_history_rewrap_entry_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalHistoryRewrapEntryContentV1, CanonicalHistoryRewrapError> {
    decoder.expect_array("history-rewrap-entry-content-v1", 7)?;
    decoder.expect_version("history-rewrap-entry-content-v1.schema_version")?;
    let manifest_id = decoder.fixed_bytes("history-rewrap-entry-content-v1.manifest_id")?;
    let source_device_id =
        decoder.fixed_bytes("history-rewrap-entry-content-v1.source_device_id")?;
    let inventory_index = decoder.unsigned("history-rewrap-entry-content-v1.inventory_index")?;
    let event_bytes = decoder.bytes("history-rewrap-entry-content-v1.event")?;
    ensure_bound(
        event_bytes,
        "authorized event",
        MAX_CANONICAL_HISTORY_REWRAP_NESTED_BYTES,
    )?;
    let content = CanonicalHistoryRewrapEntryContentV1 {
        manifest_id,
        source_device_id,
        inventory_index,
        event: decode_and_verify_authorized_event_v1(event_bytes)?,
        encapsulated_key: decoder
            .fixed_bytes("history-rewrap-entry-content-v1.encapsulated_key")?,
        ciphertext: decoder
            .bytes("history-rewrap-entry-content-v1.ciphertext")?
            .to_vec(),
    };
    validate_entry_content(&content)?;
    Ok(content)
}

fn decode_history_rewrap_transfer_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalHistoryRewrapTransferContentV1, CanonicalHistoryRewrapError> {
    decoder.expect_array("history-rewrap-transfer-content-v1", 3)?;
    decoder.expect_version("history-rewrap-transfer-content-v1.schema_version")?;
    let request = decode_and_verify_signed_history_rewrap_request_v1(
        decoder.bytes("history-rewrap-transfer-content-v1.request")?,
    )?;
    let bundle = decode_and_verify_history_rewrap_bundle_v1(
        decoder.bytes("history-rewrap-transfer-content-v1.bundle")?,
    )?;
    let content = CanonicalHistoryRewrapTransferContentV1 { request, bundle };
    validate_transfer_content(&content)?;
    Ok(content)
}

fn validate_request_content(
    content: &CanonicalHistoryRewrapRequestContentV1,
) -> Result<(), CanonicalHistoryRewrapError> {
    let end = content
        .range_start
        .checked_add(content.max_event_count)
        .ok_or(CanonicalHistoryRewrapError::InvalidRequest)?;
    if content.conversation_id == [0; 32]
        || content.source_device_id == [0; 32]
        || content.recipient_device_id == [0; 32]
        || content.source_device_id == content.recipient_device_id
        || content.session_binding == [0; 32]
        || content.sas == [0; 32]
        || content.max_event_count == 0
        || content.max_event_count > MAX_CANONICAL_HISTORY_REWRAP_ENTRIES as u64
        || end > MAX_CANONICAL_HISTORY_REWRAP_INVENTORY_EVENTS as u64
    {
        return Err(CanonicalHistoryRewrapError::InvalidRequest);
    }
    Ok(())
}

fn validate_manifest(
    manifest: &CanonicalHistoryRewrapManifestV1,
) -> Result<(), CanonicalHistoryRewrapError> {
    verify_signed_account_device_list_v1(&manifest.account_device_list)?;
    verify_signed_conversation_membership_v1(&manifest.conversation_membership)?;
    validate_rewrap_devices(
        &manifest.account_device_list,
        manifest.source_device_id,
        manifest.recipient_device_id,
    )?;
    let account_id = manifest
        .account_device_list
        .content
        .authority_snapshot
        .content
        .account_id;
    if manifest.conversation_id == [0; 32]
        || manifest.inventory_digest == [0; 32]
        || manifest.conversation_membership.content.conversation_id != manifest.conversation_id
        || manifest
            .conversation_membership
            .content
            .members
            .binary_search(&account_id)
            .is_err()
        || manifest.inventory_event_count == 0
        || manifest.inventory_event_count > MAX_CANONICAL_HISTORY_REWRAP_INVENTORY_EVENTS as u64
        || manifest.range_start >= manifest.range_end
        || manifest.range_end > manifest.inventory_event_count
        || manifest.range_end - manifest.range_start > MAX_CANONICAL_HISTORY_REWRAP_ENTRIES as u64
    {
        return Err(CanonicalHistoryRewrapError::InvalidManifest);
    }
    Ok(())
}

fn validate_rewrap_devices(
    list: &CanonicalSignedAccountDeviceListV1,
    source_device_id: [u8; 32],
    recipient_device_id: [u8; 32],
) -> Result<(), CanonicalHistoryRewrapError> {
    if source_device_id == [0; 32]
        || recipient_device_id == [0; 32]
        || source_device_id == recipient_device_id
        || list
            .content
            .devices
            .binary_search_by_key(&source_device_id, |certificate| {
                certificate.content.device_id
            })
            .is_err()
        || list
            .content
            .devices
            .binary_search_by_key(&recipient_device_id, |certificate| {
                certificate.content.device_id
            })
            .is_err()
    {
        return Err(CanonicalHistoryRewrapError::InvalidManifest);
    }
    Ok(())
}

fn validate_entry_content(
    content: &CanonicalHistoryRewrapEntryContentV1,
) -> Result<(), CanonicalHistoryRewrapError> {
    if content.manifest_id == [0; 32]
        || content.source_device_id == [0; 32]
        || content.encapsulated_key == [0; 32]
        || content.ciphertext.is_empty()
        || content.ciphertext.len() > MAX_CANONICAL_HISTORY_REWRAP_CIPHERTEXT_BYTES
        || !matches!(
            content.event.event.content.payload,
            CanonicalEventPayloadV1::RatchetText(_)
        )
    {
        return Err(CanonicalHistoryRewrapError::InvalidEntry);
    }
    Ok(())
}

fn validate_entry_event_for_manifest(
    event: &CanonicalAuthorizedEventV1,
    inventory_index: u64,
    manifest: &CanonicalHistoryRewrapManifestV1,
) -> Result<(), CanonicalHistoryRewrapError> {
    if inventory_index < manifest.range_start
        || inventory_index >= manifest.range_end
        || event.event.content.conversation_id != manifest.conversation_id
        || !matches!(
            event.event.content.payload,
            CanonicalEventPayloadV1::RatchetText(_)
        )
    {
        return Err(CanonicalHistoryRewrapError::InvalidEntry);
    }
    verify_authorized_event_for_membership_v1(event, &manifest.conversation_membership)?;
    Ok(())
}

fn validate_transfer_content(
    content: &CanonicalHistoryRewrapTransferContentV1,
) -> Result<(), CanonicalHistoryRewrapError> {
    verify_signed_history_rewrap_request_v1(&content.request)?;
    verify_history_rewrap_bundle_v1(&content.bundle)?;
    verify_history_rewrap_request_for_manifest_v1(
        &content.request,
        content.request.content.session_binding,
        &content.bundle.manifest,
    )
    .map_err(|_| CanonicalHistoryRewrapError::InvalidTransfer)
}

fn recipient_encryption_key(
    manifest: &CanonicalHistoryRewrapManifestV1,
) -> Result<EncryptionPublicKey, CanonicalHistoryRewrapError> {
    let certificate = manifest
        .account_device_list
        .content
        .devices
        .iter()
        .find(|value| value.content.device_id == manifest.recipient_device_id)
        .ok_or(CanonicalHistoryRewrapError::InvalidManifest)?;
    Ok(EncryptionPublicKey::from_bytes(
        certificate.content.encryption_public_key,
    )?)
}

fn verify_ed25519(
    public_key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
) -> Result<(), CanonicalHistoryRewrapError> {
    let key = VerifyingKey::from_bytes(public_key)
        .map_err(|_| CanonicalHistoryRewrapError::InvalidPublicKey)?;
    key.verify_strict(message, &Signature::from_bytes(signature))
        .map_err(|_| CanonicalHistoryRewrapError::InvalidSignature)
}

fn ensure_bound(
    bytes: &[u8],
    object: &'static str,
    maximum: usize,
) -> Result<(), CanonicalHistoryRewrapError> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(CanonicalHistoryRewrapError::ObjectTooLarge { object, maximum });
    }
    Ok(())
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    fn document(name: &str) -> TestResult<Value> {
        let json = match name {
            "rewrap" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../CANONICAL-HISTORY-REWRAP-VECTORS.json"
            )),
            "endpoint" => include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../CANONICAL-ENDPOINT-ANNOUNCEMENT-VECTORS.json"
            )),
            _ => return Err("unknown vector document".into()),
        };
        Ok(serde_json::from_str(json)?)
    }

    fn hex_at(document: &Value, pointer: &str) -> TestResult<Vec<u8>> {
        let value = document
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("missing vector pointer {pointer}"))?;
        if !value.len().is_multiple_of(2) {
            return Err(format!("odd-length hex at {pointer}").into());
        }
        Ok((0..value.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&value[index..index + 2], 16))
            .collect::<Result<Vec<_>, _>>()?)
    }

    fn fixed<const N: usize>(document: &Value, pointer: &str) -> TestResult<[u8; N]> {
        hex_at(document, pointer)?
            .try_into()
            .map_err(|_| format!("{pointer} length changed").into())
    }

    #[test]
    fn canonical_history_rewrap_vector_round_trips_and_opens_real_hpke() -> TestResult {
        let vectors = document("rewrap")?;
        let endpoint = document("endpoint")?;
        let request_bytes = hex_at(&vectors, "/expected/request/signed_cbor_hex")?;
        let manifest_bytes = hex_at(&vectors, "/expected/manifest/cbor_hex")?;
        let entry_bytes = hex_at(&vectors, "/expected/entry/signed_cbor_hex")?;
        let bundle_bytes = hex_at(&vectors, "/expected/bundle/cbor_hex")?;
        let transfer_bytes = hex_at(&vectors, "/expected/transfer/signed_cbor_hex")?;
        let request = decode_and_verify_signed_history_rewrap_request_v1(&request_bytes)?;
        let manifest = decode_and_verify_history_rewrap_manifest_v1(&manifest_bytes)?;
        let entry = decode_and_verify_signed_history_rewrap_entry_v1(&entry_bytes)?;
        let bundle = decode_and_verify_history_rewrap_bundle_v1(&bundle_bytes)?;
        let transfer = decode_and_verify_signed_history_rewrap_transfer_v1(&transfer_bytes)?;
        let session = fixed::<32>(&vectors, "/input/session_binding_hex")?;

        assert_eq!(request, transfer.content.request);
        assert_eq!(manifest, bundle.manifest);
        assert_eq!(entry, bundle.entries[0]);
        assert_eq!(bundle, transfer.content.bundle);
        assert_eq!(
            history_rewrap_sas_input_v1(
                &manifest.account_device_list,
                manifest.source_device_id,
                manifest.recipient_device_id,
            )?,
            hex_at(&vectors, "/expected/sas/input_cbor_hex")?
        );
        assert_eq!(
            request.content.sas,
            fixed::<32>(&vectors, "/expected/sas/digest_hex")?
        );
        assert_eq!(
            history_rewrap_inventory_digest_v1(
                manifest.conversation_id,
                &[fixed::<32>(&vectors, "/expected/event_id_hex")?],
            )?,
            fixed::<32>(&vectors, "/expected/inventory/digest_hex")?
        );
        assert_eq!(
            history_rewrap_manifest_id_v1(&manifest)?,
            fixed::<32>(&vectors, "/expected/manifest/object_id_hex")?
        );
        assert_eq!(
            history_rewrap_bundle_id_v1(&bundle)?,
            fixed::<32>(&vectors, "/expected/bundle/object_id_hex")?
        );
        assert_eq!(
            history_rewrap_entry_aad_v1(
                entry.content.manifest_id,
                entry.content.inventory_index,
                event_id_v1(&entry.content.event.event)?,
            ),
            hex_at(&vectors, "/expected/hpke/aad_hex")?
        );
        verify_history_rewrap_transfer_for_request_v1(&transfer, &request, session)?;

        let recipient = DeviceEncryptionIdentity::from_secret_bytes(fixed::<32>(
            &endpoint,
            "/input/recipient_encryption_secret_seed_hex",
        )?);
        let expected_plaintext = vectors
            .pointer("/input/plaintext_utf8")
            .and_then(Value::as_str)
            .ok_or("missing plaintext")?;
        assert_eq!(
            open_history_rewrap_entry_v1(&entry, &manifest, &recipient)?,
            expected_plaintext
        );
        Ok(())
    }

    #[test]
    fn canonical_history_rewrap_context_substitutions_fail_closed() -> TestResult {
        let vectors = document("rewrap")?;
        let endpoint = document("endpoint")?;
        let request = decode_and_verify_signed_history_rewrap_request_v1(&hex_at(
            &vectors,
            "/expected/request/signed_cbor_hex",
        )?)?;
        let manifest = decode_and_verify_history_rewrap_manifest_v1(&hex_at(
            &vectors,
            "/expected/manifest/cbor_hex",
        )?)?;
        let entry = decode_and_verify_signed_history_rewrap_entry_v1(&hex_at(
            &vectors,
            "/expected/entry/signed_cbor_hex",
        )?)?;
        let transfer = decode_and_verify_signed_history_rewrap_transfer_v1(&hex_at(
            &vectors,
            "/expected/transfer/signed_cbor_hex",
        )?)?;
        let session = fixed::<32>(&vectors, "/input/session_binding_hex")?;

        let mut wrong_session = session;
        wrong_session[0] ^= 1;
        assert!(
            verify_history_rewrap_transfer_for_request_v1(&transfer, &request, wrong_session,)
                .is_err()
        );
        let mut wrong_request = request.clone();
        wrong_request.signature[0] ^= 1;
        assert!(
            verify_history_rewrap_transfer_for_request_v1(&transfer, &wrong_request, session,)
                .is_err()
        );
        let wrong_recipient = DeviceEncryptionIdentity::from_secret_bytes([7; 32]);
        assert!(open_history_rewrap_entry_v1(&entry, &manifest, &wrong_recipient).is_err());
        let mut changed_entry = entry.clone();
        changed_entry.content.ciphertext[0] ^= 1;
        assert!(
            open_history_rewrap_entry_v1(
                &changed_entry,
                &manifest,
                &DeviceEncryptionIdentity::from_secret_bytes(fixed::<32>(
                    &endpoint,
                    "/input/recipient_encryption_secret_seed_hex",
                )?),
            )
            .is_err()
        );
        Ok(())
    }
}
