//! Inactive deterministic-CBOR event and authorization candidate for M1.1.10.
//!
//! The active event format remains Postcard v5. This module completes a future
//! portable representation without assigning it an active protocol identifier.

use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

use crate::{
    canonical_account_v1::{
        AccountDecoder, CanonicalAccountError, CanonicalSignedAccountDeviceListV1,
        CanonicalSignedConversationMembershipV1, decode_signed_account_device_list_v1,
        encode_signed_account_device_list_v1, signed_envelope,
        verify_signed_account_device_list_v1, verify_signed_conversation_membership_v1,
    },
    canonical_authority_v1::{
        CanonicalAuthorityError, CanonicalDeviceCapabilityV1,
        CanonicalSignedAccountAuthoritySnapshotV1, CanonicalSignedDeviceCertificateV1,
        decode_signed_account_authority_snapshot_v1, decode_signed_device_certificate_v1,
        encode_signed_account_authority_snapshot_v1, encode_signed_device_certificate_v1,
        verify_signed_account_authority_snapshot_v1, verify_signed_device_certificate_v1,
    },
    canonical_ratchet_v1::{
        CanonicalRatchetError, CanonicalSignedRatchetIdentityV1,
        decode_and_verify_signed_ratchet_identity_v1, encode_signed_ratchet_identity_v1,
        verify_signed_ratchet_identity_v1,
    },
    canonical_v1::{
        ACKNOWLEDGEMENT_PAYLOAD_VARIANT, CANONICAL_EVENT_ID_DOMAIN,
        CANONICAL_EVENT_SIGNATURE_DOMAIN, CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed,
        push_array_header, push_bytes, push_unsigned, validate_canonical_profile_v1,
    },
};

pub const RATCHET_TEXT_PAYLOAD_VARIANT: u64 = 2;
pub const MAX_CANONICAL_EVENT_PARENTS: usize = 64;
pub const MAX_CANONICAL_EVENT_RECIPIENTS: usize = 32;
pub const MAX_CANONICAL_RATCHET_CIPHERTEXT_BYTES: usize = 128 * 1024;
pub const MAX_CANONICAL_EVENT_NESTED_BYTES: usize = 512 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum CanonicalRatchetMessageKindV1 {
    PreKey = 0,
    Normal = 1,
}

impl CanonicalRatchetMessageKindV1 {
    const fn code(self) -> u64 {
        self as u64
    }

    fn from_code(code: u64) -> Result<Self, CanonicalEventError> {
        match code {
            0 => Ok(Self::PreKey),
            1 => Ok(Self::Normal),
            other => Err(CanonicalEventError::UnknownRatchetMessageKind(other)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRatchetCiphertextV1 {
    pub kind: CanonicalRatchetMessageKindV1,
    pub ciphertext: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRatchetRecipientV1 {
    pub device_id: [u8; 32],
    pub ciphertext: CanonicalRatchetCiphertextV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRatchetTextPayloadV1 {
    pub recipient_device_list: CanonicalSignedAccountDeviceListV1,
    pub sender_ratchet_identity: CanonicalSignedRatchetIdentityV1,
    pub recipients: Vec<CanonicalRatchetRecipientV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalEventPayloadV1 {
    Acknowledgement { acknowledged_event_id: [u8; 32] },
    RatchetText(Box<CanonicalRatchetTextPayloadV1>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalEventContentV1 {
    pub conversation_id: [u8; 32],
    pub author_device_id: [u8; 32],
    pub author_sequence: u64,
    pub parents: Vec<[u8; 32]>,
    pub payload: CanonicalEventPayloadV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedEventV1 {
    pub content: CanonicalEventContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalAuthorizedEventV1 {
    pub event: CanonicalSignedEventV1,
    pub author_certificate: CanonicalSignedDeviceCertificateV1,
    pub author_authority_snapshot: CanonicalSignedAccountAuthoritySnapshotV1,
}

#[derive(Debug, Error)]
pub enum CanonicalEventError {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),

    #[error(transparent)]
    Account(#[from] CanonicalAccountError),

    #[error(transparent)]
    Authority(#[from] CanonicalAuthorityError),

    #[error(transparent)]
    Ratchet(#[from] CanonicalRatchetError),

    #[error("{object} array has {actual} items; expected {expected}")]
    WrongArrayLength {
        object: &'static str,
        expected: usize,
        actual: usize,
    },

    #[error("unknown canonical event payload variant {0}")]
    UnknownPayloadVariant(u64),

    #[error("unknown canonical ratchet message kind {0}")]
    UnknownRatchetMessageKind(u64),

    #[error("canonical event parents must be strictly ordered by Event ID")]
    NonCanonicalParentOrder,

    #[error("canonical ratchet text must contain at least one recipient")]
    EmptyRecipients,

    #[error("canonical ratchet recipients must be strictly ordered by Device ID")]
    NonCanonicalRecipientOrder,

    #[error("canonical ratchet recipient is the event author")]
    RecipientIsAuthor,

    #[error("canonical recipient list does not match the exact account device list")]
    RecipientDeviceListMismatch,

    #[error("canonical sender ratchet identity does not match the event author")]
    RatchetIdentityAuthorMismatch,

    #[error("canonical ratchet ciphertext has {actual} bytes; expected 1..={maximum}")]
    InvalidCiphertextLength { actual: usize, maximum: usize },

    #[error("nested canonical object exceeds {MAX_CANONICAL_EVENT_NESTED_BYTES} bytes")]
    NestedObjectTooLarge,

    #[error("authorized event certificate does not match the event author")]
    AuthorCertificateMismatch,

    #[error("authorized event certificate and authority snapshot name different Accounts")]
    AuthorAccountMismatch,

    #[error("authorized event certificate sequence is outside the authority snapshot")]
    AuthorCertificateOutsideSnapshot,

    #[error("authorized event author is revoked")]
    AuthorRevoked,

    #[error("authorized event certificate lacks SignEvents capability")]
    AuthorCannotSignEvents,

    #[error("event and conversation membership refer to different conversations")]
    MembershipConversationMismatch,

    #[error("event author Account is absent from conversation membership")]
    AuthorNotMember,

    #[error("{object} contains an invalid Ed25519 verifying key")]
    InvalidPublicKey { object: &'static str },

    #[error("{object} contains an invalid Ed25519 signature")]
    InvalidSignature { object: &'static str },
}

pub fn encode_event_content_v1(
    content: &CanonicalEventContentV1,
) -> Result<Vec<u8>, CanonicalEventError> {
    validate_event_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 6);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.conversation_id);
    push_bytes(&mut output, &content.author_device_id);
    push_unsigned(&mut output, content.author_sequence);
    push_array_header(&mut output, content.parents.len() as u64);
    for parent in &content.parents {
        push_bytes(&mut output, parent);
    }
    encode_payload_into(&mut output, &content.payload)?;
    Ok(output)
}

pub fn event_signing_bytes_v1(
    content: &CanonicalEventContentV1,
) -> Result<Vec<u8>, CanonicalEventError> {
    Ok(prefixed(
        CANONICAL_EVENT_SIGNATURE_DOMAIN,
        &encode_event_content_v1(content)?,
    ))
}

pub fn encode_signed_event_v1(
    event: &CanonicalSignedEventV1,
) -> Result<Vec<u8>, CanonicalEventError> {
    verify_signed_event_v1(event)?;
    let content = encode_event_content_v1(&event.content)?;
    Ok(signed_envelope(&content, &event.signature))
}

pub fn decode_signed_event_v1(bytes: &[u8]) -> Result<CanonicalSignedEventV1, CanonicalEventError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    let event = decode_signed_event_from(&mut decoder)?;
    decoder.finish()?;
    Ok(event)
}

pub fn decode_and_verify_signed_event_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedEventV1, CanonicalEventError> {
    let event = decode_signed_event_v1(bytes)?;
    verify_signed_event_v1(&event)?;
    Ok(event)
}

pub fn verify_signed_event_v1(event: &CanonicalSignedEventV1) -> Result<(), CanonicalEventError> {
    validate_event_content(&event.content)?;
    verify_ed25519(
        &event.content.author_device_id,
        &event_signing_bytes_v1(&event.content)?,
        &event.signature,
        "signed-event-v1",
    )
}

pub fn event_id_v1(event: &CanonicalSignedEventV1) -> Result<[u8; 32], CanonicalEventError> {
    let encoded = encode_signed_event_v1(event)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(CANONICAL_EVENT_ID_DOMAIN);
    hasher.update(&encoded);
    Ok(*hasher.finalize().as_bytes())
}

pub fn encode_authorized_event_v1(
    authorized: &CanonicalAuthorizedEventV1,
) -> Result<Vec<u8>, CanonicalEventError> {
    verify_authorized_event_v1(authorized)?;
    let event = encode_signed_event_v1(&authorized.event)?;
    let certificate = encode_signed_device_certificate_v1(&authorized.author_certificate)?;
    let snapshot =
        encode_signed_account_authority_snapshot_v1(&authorized.author_authority_snapshot)?;
    for nested in [&event, &certificate, &snapshot] {
        ensure_nested_bound(nested)?;
    }
    let mut output = Vec::new();
    push_array_header(&mut output, 4);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &event);
    push_bytes(&mut output, &certificate);
    push_bytes(&mut output, &snapshot);
    Ok(output)
}

pub fn decode_authorized_event_v1(
    bytes: &[u8],
) -> Result<CanonicalAuthorizedEventV1, CanonicalEventError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("authorized-event-v1", 4)?;
    decoder.expect_version("authorized-event-v1.schema_version")?;
    let event_bytes = decoder.bytes("authorized-event-v1.event")?;
    let certificate_bytes = decoder.bytes("authorized-event-v1.author_certificate")?;
    let snapshot_bytes = decoder.bytes("authorized-event-v1.author_authority_snapshot")?;
    for nested in [event_bytes, certificate_bytes, snapshot_bytes] {
        ensure_nested_bound(nested)?;
    }
    let authorized = CanonicalAuthorizedEventV1 {
        event: decode_signed_event_v1(event_bytes)?,
        author_certificate: decode_signed_device_certificate_v1(certificate_bytes)?,
        author_authority_snapshot: decode_signed_account_authority_snapshot_v1(snapshot_bytes)?,
    };
    decoder.finish()?;
    validate_authorization_links(&authorized)?;
    Ok(authorized)
}

pub fn decode_and_verify_authorized_event_v1(
    bytes: &[u8],
) -> Result<CanonicalAuthorizedEventV1, CanonicalEventError> {
    let authorized = decode_authorized_event_v1(bytes)?;
    verify_authorized_event_v1(&authorized)?;
    Ok(authorized)
}

pub fn verify_authorized_event_v1(
    authorized: &CanonicalAuthorizedEventV1,
) -> Result<(), CanonicalEventError> {
    validate_authorization_links(authorized)?;
    verify_signed_event_v1(&authorized.event)?;
    verify_signed_device_certificate_v1(&authorized.author_certificate)?;
    verify_signed_account_authority_snapshot_v1(&authorized.author_authority_snapshot)?;
    Ok(())
}

pub fn verify_authorized_event_for_membership_v1(
    authorized: &CanonicalAuthorizedEventV1,
    membership: &CanonicalSignedConversationMembershipV1,
) -> Result<(), CanonicalEventError> {
    verify_authorized_event_v1(authorized)?;
    verify_signed_conversation_membership_v1(membership)?;
    if membership.content.conversation_id != authorized.event.content.conversation_id {
        return Err(CanonicalEventError::MembershipConversationMismatch);
    }
    let account_id = authorized.author_certificate.content.account_id;
    if membership
        .content
        .members
        .binary_search(&account_id)
        .is_err()
    {
        return Err(CanonicalEventError::AuthorNotMember);
    }
    Ok(())
}

fn encode_payload_into(
    output: &mut Vec<u8>,
    payload: &CanonicalEventPayloadV1,
) -> Result<(), CanonicalEventError> {
    match payload {
        CanonicalEventPayloadV1::Acknowledgement {
            acknowledged_event_id,
        } => {
            push_array_header(output, 2);
            push_unsigned(output, ACKNOWLEDGEMENT_PAYLOAD_VARIANT);
            push_bytes(output, acknowledged_event_id);
        }
        CanonicalEventPayloadV1::RatchetText(payload) => {
            let device_list = encode_signed_account_device_list_v1(&payload.recipient_device_list)?;
            let identity = encode_signed_ratchet_identity_v1(&payload.sender_ratchet_identity)?;
            ensure_nested_bound(&device_list)?;
            ensure_nested_bound(&identity)?;
            push_array_header(output, 4);
            push_unsigned(output, RATCHET_TEXT_PAYLOAD_VARIANT);
            push_bytes(output, &device_list);
            push_bytes(output, &identity);
            push_array_header(output, payload.recipients.len() as u64);
            for recipient in &payload.recipients {
                push_array_header(output, 4);
                push_unsigned(output, CANONICAL_PROFILE_VERSION);
                push_bytes(output, &recipient.device_id);
                push_unsigned(output, recipient.ciphertext.kind.code());
                push_bytes(output, &recipient.ciphertext.ciphertext);
            }
        }
    }
    Ok(())
}

fn validate_event_content(content: &CanonicalEventContentV1) -> Result<(), CanonicalEventError> {
    if content.parents.len() > MAX_CANONICAL_EVENT_PARENTS {
        return Err(CanonicalWireError::TooManyItems {
            field: "parents",
            actual: content.parents.len(),
            maximum: MAX_CANONICAL_EVENT_PARENTS,
        }
        .into());
    }
    if content.parents.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(CanonicalEventError::NonCanonicalParentOrder);
    }
    if let CanonicalEventPayloadV1::RatchetText(payload) = &content.payload {
        verify_signed_account_device_list_v1(&payload.recipient_device_list)?;
        verify_signed_ratchet_identity_v1(&payload.sender_ratchet_identity)?;
        if payload.sender_ratchet_identity.content.device_id != content.author_device_id {
            return Err(CanonicalEventError::RatchetIdentityAuthorMismatch);
        }
        if payload.recipients.is_empty() {
            return Err(CanonicalEventError::EmptyRecipients);
        }
        if payload.recipients.len() > MAX_CANONICAL_EVENT_RECIPIENTS {
            return Err(CanonicalWireError::TooManyItems {
                field: "recipients",
                actual: payload.recipients.len(),
                maximum: MAX_CANONICAL_EVENT_RECIPIENTS,
            }
            .into());
        }
        for recipient in &payload.recipients {
            if recipient.device_id == content.author_device_id {
                return Err(CanonicalEventError::RecipientIsAuthor);
            }
            let length = recipient.ciphertext.ciphertext.len();
            if length == 0 || length > MAX_CANONICAL_RATCHET_CIPHERTEXT_BYTES {
                return Err(CanonicalEventError::InvalidCiphertextLength {
                    actual: length,
                    maximum: MAX_CANONICAL_RATCHET_CIPHERTEXT_BYTES,
                });
            }
        }
        if payload
            .recipients
            .windows(2)
            .any(|pair| pair[0].device_id >= pair[1].device_id)
        {
            return Err(CanonicalEventError::NonCanonicalRecipientOrder);
        }
        let listed = &payload.recipient_device_list.content.devices;
        if listed.len() != payload.recipients.len()
            || listed
                .iter()
                .zip(&payload.recipients)
                .any(|(certificate, recipient)| {
                    certificate.content.device_id != recipient.device_id
                })
        {
            return Err(CanonicalEventError::RecipientDeviceListMismatch);
        }
    }
    Ok(())
}

fn validate_authorization_links(
    authorized: &CanonicalAuthorizedEventV1,
) -> Result<(), CanonicalEventError> {
    let certificate = &authorized.author_certificate.content;
    let snapshot = &authorized.author_authority_snapshot.content;
    if certificate.device_id != authorized.event.content.author_device_id {
        return Err(CanonicalEventError::AuthorCertificateMismatch);
    }
    if certificate.account_id != snapshot.account_id {
        return Err(CanonicalEventError::AuthorAccountMismatch);
    }
    if certificate.authority_sequence >= snapshot.revision {
        return Err(CanonicalEventError::AuthorCertificateOutsideSnapshot);
    }
    if snapshot
        .revocations
        .iter()
        .any(|revocation| revocation.content.device_id == certificate.device_id)
    {
        return Err(CanonicalEventError::AuthorRevoked);
    }
    if !certificate
        .capabilities
        .contains(&CanonicalDeviceCapabilityV1::SignEvents)
    {
        return Err(CanonicalEventError::AuthorCannotSignEvents);
    }
    Ok(())
}

fn decode_signed_event_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalSignedEventV1, CanonicalEventError> {
    decoder.expect_array("signed-event-v1", 3)?;
    decoder.expect_version("signed-event-v1.schema_version")?;
    let content = decode_event_content_from(decoder)?;
    let signature = decoder.fixed_bytes("signed-event-v1.signature")?;
    Ok(CanonicalSignedEventV1 { content, signature })
}

fn decode_event_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalEventContentV1, CanonicalEventError> {
    decoder.expect_array("event-content-v1", 6)?;
    decoder.expect_version("event-content-v1.schema_version")?;
    let conversation_id = decoder.fixed_bytes("event-content-v1.conversation_id")?;
    let author_device_id = decoder.fixed_bytes("event-content-v1.author_device_id")?;
    let author_sequence = decoder.unsigned("event-content-v1.author_sequence")?;
    let parent_count = decoder.array("event-content-v1.parents")?;
    if parent_count > MAX_CANONICAL_EVENT_PARENTS {
        return Err(CanonicalWireError::TooManyItems {
            field: "parents",
            actual: parent_count,
            maximum: MAX_CANONICAL_EVENT_PARENTS,
        }
        .into());
    }
    let mut parents = Vec::with_capacity(parent_count);
    for _ in 0..parent_count {
        parents.push(decoder.fixed_bytes("event-content-v1.parent")?);
    }
    let payload = decode_payload_from(decoder)?;
    let content = CanonicalEventContentV1 {
        conversation_id,
        author_device_id,
        author_sequence,
        parents,
        payload,
    };
    validate_event_content(&content)?;
    Ok(content)
}

fn decode_payload_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalEventPayloadV1, CanonicalEventError> {
    let length = decoder.array("event-payload-v1")?;
    let variant = decoder.unsigned("event-payload-v1.variant")?;
    match variant {
        ACKNOWLEDGEMENT_PAYLOAD_VARIANT => {
            require_array_length("acknowledgement-payload-v1", length, 2)?;
            Ok(CanonicalEventPayloadV1::Acknowledgement {
                acknowledged_event_id: decoder
                    .fixed_bytes("acknowledgement-payload-v1.acknowledged_event_id")?,
            })
        }
        RATCHET_TEXT_PAYLOAD_VARIANT => {
            require_array_length("ratchet-text-payload-v1", length, 4)?;
            let device_list_bytes = decoder.bytes("ratchet-text-payload-v1.device_list")?;
            let identity_bytes = decoder.bytes("ratchet-text-payload-v1.sender_identity")?;
            ensure_nested_bound(device_list_bytes)?;
            ensure_nested_bound(identity_bytes)?;
            let count = decoder.array("ratchet-text-payload-v1.recipients")?;
            if count > MAX_CANONICAL_EVENT_RECIPIENTS {
                return Err(CanonicalWireError::TooManyItems {
                    field: "recipients",
                    actual: count,
                    maximum: MAX_CANONICAL_EVENT_RECIPIENTS,
                }
                .into());
            }
            let mut recipients = Vec::with_capacity(count);
            for _ in 0..count {
                decoder.expect_array("ratchet-recipient-v1", 4)?;
                decoder.expect_version("ratchet-recipient-v1.schema_version")?;
                let device_id = decoder.fixed_bytes("ratchet-recipient-v1.device_id")?;
                let kind = CanonicalRatchetMessageKindV1::from_code(
                    decoder.unsigned("ratchet-recipient-v1.message_kind")?,
                )?;
                let ciphertext = decoder.bytes("ratchet-recipient-v1.ciphertext")?.to_vec();
                recipients.push(CanonicalRatchetRecipientV1 {
                    device_id,
                    ciphertext: CanonicalRatchetCiphertextV1 { kind, ciphertext },
                });
            }
            Ok(CanonicalEventPayloadV1::RatchetText(Box::new(
                CanonicalRatchetTextPayloadV1 {
                    recipient_device_list: decode_signed_account_device_list_v1(device_list_bytes)?,
                    sender_ratchet_identity: decode_and_verify_signed_ratchet_identity_v1(
                        identity_bytes,
                    )?,
                    recipients,
                },
            )))
        }
        other => Err(CanonicalEventError::UnknownPayloadVariant(other)),
    }
}

fn require_array_length(
    object: &'static str,
    actual: usize,
    expected: usize,
) -> Result<(), CanonicalEventError> {
    if actual == expected {
        Ok(())
    } else {
        Err(CanonicalEventError::WrongArrayLength {
            object,
            expected,
            actual,
        })
    }
}

fn ensure_nested_bound(bytes: &[u8]) -> Result<(), CanonicalEventError> {
    if bytes.len() > MAX_CANONICAL_EVENT_NESTED_BYTES {
        return Err(CanonicalEventError::NestedObjectTooLarge);
    }
    Ok(())
}

fn verify_ed25519(
    key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
    object: &'static str,
) -> Result<(), CanonicalEventError> {
    let key = VerifyingKey::from_bytes(key)
        .map_err(|_| CanonicalEventError::InvalidPublicKey { object })?;
    let signature = Signature::try_from(signature.as_slice())
        .map_err(|_| CanonicalEventError::InvalidSignature { object })?;
    key.verify_strict(message, &signature)
        .map_err(|_| CanonicalEventError::InvalidSignature { object })
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct EventVectors {
        schema_version: u64,
        profile: String,
        activation_status: String,
        input: EventInput,
        expected: EventExpected,
    }

    #[derive(Deserialize)]
    struct EventInput {
        author_root_secret_key_hex: String,
        author_device_secret_key_hex: String,
        conversation_id_hex: String,
        text_author_sequence: u64,
        acknowledgement_author_sequence: u64,
        parents_hex: Vec<String>,
        recipient_ciphertext_hex: String,
        recipient_message_kind: u64,
    }

    #[derive(Deserialize)]
    struct EventExpected {
        author_account_id_hex: String,
        author_device_id_hex: String,
        recipient_account_id_hex: String,
        recipient_device_id_hex: String,
        membership: SignedExpected,
        text_event: EventObjectExpected,
        authorized_text_event_cbor_hex: String,
        acknowledgement_event: EventObjectExpected,
        authorized_acknowledgement_event_cbor_hex: String,
    }

    #[derive(Deserialize)]
    struct EventObjectExpected {
        content_cbor_hex: String,
        signature_domain_hex: String,
        signing_preimage_hex: String,
        signature_hex: String,
        signed_cbor_hex: String,
        event_id_domain_hex: String,
        event_id_hex: String,
    }

    #[derive(Deserialize)]
    struct SignedExpected {
        signed_cbor_hex: String,
    }

    #[derive(Deserialize)]
    struct RatchetVectors {
        expected: RatchetExpected,
    }

    #[derive(Deserialize)]
    struct RatchetExpected {
        account_id_hex: String,
        device_id_hex: String,
        certificate: SignedExpected,
        authority_snapshot: SignedExpected,
        ratchet_identity: SignedExpected,
    }

    #[derive(Deserialize)]
    struct AccountVectors {
        expected: AccountExpected,
    }

    #[derive(Deserialize)]
    struct AccountExpected {
        account_id_hex: String,
        account_device_list: SignedExpected,
    }

    fn invalid_data(message: impl Into<String>) -> Box<dyn std::error::Error> {
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message.into(),
        ))
    }

    fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if !value.len().is_multiple_of(2)
            || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
            || value.bytes().any(|byte| byte.is_ascii_uppercase())
        {
            return Err(invalid_data("event vector contains invalid hexadecimal"));
        }
        (0..value.len())
            .step_by(2)
            .map(|offset| Ok(u8::from_str_radix(&value[offset..offset + 2], 16)?))
            .collect()
    }

    fn decode_fixed<const N: usize>(value: &str) -> Result<[u8; N], Box<dyn std::error::Error>> {
        let bytes = decode_hex(value)?;
        bytes.try_into().map_err(|bytes: Vec<u8>| {
            invalid_data(format!("vector has {} bytes; expected {N}", bytes.len()))
        })
    }

    fn to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn assert_event_expected(
        expected: &EventObjectExpected,
        event: &CanonicalSignedEventV1,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let content = encode_event_content_v1(&event.content)?;
        let preimage = event_signing_bytes_v1(&event.content)?;
        let signed = encode_signed_event_v1(event)?;
        assert_eq!(to_hex(&content), expected.content_cbor_hex);
        assert_eq!(
            to_hex(CANONICAL_EVENT_SIGNATURE_DOMAIN),
            expected.signature_domain_hex
        );
        assert_eq!(to_hex(&preimage), expected.signing_preimage_hex);
        assert_eq!(to_hex(&event.signature), expected.signature_hex);
        assert_eq!(to_hex(&signed), expected.signed_cbor_hex);
        assert_eq!(
            to_hex(CANONICAL_EVENT_ID_DOMAIN),
            expected.event_id_domain_hex
        );
        assert_eq!(to_hex(&event_id_v1(event)?), expected.event_id_hex);
        assert_eq!(decode_and_verify_signed_event_v1(&signed)?, *event);
        Ok(())
    }

    #[test]
    fn event_vectors_match_all_payload_authorization_and_membership_bytes()
    -> Result<(), Box<dyn std::error::Error>> {
        let vectors: EventVectors = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-EVENT-VECTORS.json"
        )))?;
        let ratchet: RatchetVectors = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-RATCHET-VECTORS.json"
        )))?;
        let account: AccountVectors = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-ACCOUNT-VECTORS.json"
        )))?;
        assert_eq!(vectors.schema_version, CANONICAL_PROFILE_VERSION);
        assert_eq!(vectors.profile, "kilogram-deterministic-cbor-v1");
        assert_eq!(vectors.activation_status, "not-active");

        let author_root =
            SigningKey::from_bytes(&decode_fixed(&vectors.input.author_root_secret_key_hex)?);
        let author_device =
            SigningKey::from_bytes(&decode_fixed(&vectors.input.author_device_secret_key_hex)?);
        let author_account_id = author_root.verifying_key().to_bytes();
        let author_device_id = author_device.verifying_key().to_bytes();
        assert_eq!(
            to_hex(&author_account_id),
            vectors.expected.author_account_id_hex
        );
        assert_eq!(
            to_hex(&author_device_id),
            vectors.expected.author_device_id_hex
        );
        assert_eq!(
            ratchet.expected.account_id_hex,
            vectors.expected.author_account_id_hex
        );
        assert_eq!(
            ratchet.expected.device_id_hex,
            vectors.expected.author_device_id_hex
        );
        assert_eq!(
            account.expected.account_id_hex,
            vectors.expected.recipient_account_id_hex
        );

        let membership = crate::canonical_account_v1::decode_signed_conversation_membership_v1(
            &decode_hex(&vectors.expected.membership.signed_cbor_hex)?,
        )?;
        let device_list = decode_signed_account_device_list_v1(&decode_hex(
            &account.expected.account_device_list.signed_cbor_hex,
        )?)?;
        let recipient_device_id = device_list.content.devices[0].content.device_id;
        assert_eq!(
            to_hex(&recipient_device_id),
            vectors.expected.recipient_device_id_hex
        );
        let sender_identity = decode_and_verify_signed_ratchet_identity_v1(&decode_hex(
            &ratchet.expected.ratchet_identity.signed_cbor_hex,
        )?)?;
        let parents = vectors
            .input
            .parents_hex
            .iter()
            .map(|value| decode_fixed(value))
            .collect::<Result<Vec<_>, _>>()?;
        let text_content = CanonicalEventContentV1 {
            conversation_id: decode_fixed(&vectors.input.conversation_id_hex)?,
            author_device_id,
            author_sequence: vectors.input.text_author_sequence,
            parents,
            payload: CanonicalEventPayloadV1::RatchetText(Box::new(
                CanonicalRatchetTextPayloadV1 {
                    recipient_device_list: device_list,
                    sender_ratchet_identity: sender_identity,
                    recipients: vec![CanonicalRatchetRecipientV1 {
                        device_id: recipient_device_id,
                        ciphertext: CanonicalRatchetCiphertextV1 {
                            kind: CanonicalRatchetMessageKindV1::from_code(
                                vectors.input.recipient_message_kind,
                            )?,
                            ciphertext: decode_hex(&vectors.input.recipient_ciphertext_hex)?,
                        },
                    }],
                },
            )),
        };
        let text_signature = author_device
            .sign(&event_signing_bytes_v1(&text_content)?)
            .to_bytes();
        let text_event = CanonicalSignedEventV1 {
            content: text_content,
            signature: text_signature,
        };
        assert_event_expected(&vectors.expected.text_event, &text_event)?;

        let certificate = decode_signed_device_certificate_v1(&decode_hex(
            &ratchet.expected.certificate.signed_cbor_hex,
        )?)?;
        let snapshot = decode_signed_account_authority_snapshot_v1(&decode_hex(
            &ratchet.expected.authority_snapshot.signed_cbor_hex,
        )?)?;
        let authorized_text = CanonicalAuthorizedEventV1 {
            event: text_event.clone(),
            author_certificate: certificate.clone(),
            author_authority_snapshot: snapshot.clone(),
        };
        assert_eq!(
            to_hex(&encode_authorized_event_v1(&authorized_text)?),
            vectors.expected.authorized_text_event_cbor_hex
        );
        verify_authorized_event_for_membership_v1(&authorized_text, &membership)?;

        let acknowledgement_content = CanonicalEventContentV1 {
            conversation_id: decode_fixed(&vectors.input.conversation_id_hex)?,
            author_device_id,
            author_sequence: vectors.input.acknowledgement_author_sequence,
            parents: vec![event_id_v1(&text_event)?],
            payload: CanonicalEventPayloadV1::Acknowledgement {
                acknowledged_event_id: event_id_v1(&text_event)?,
            },
        };
        let acknowledgement = CanonicalSignedEventV1 {
            signature: author_device
                .sign(&event_signing_bytes_v1(&acknowledgement_content)?)
                .to_bytes(),
            content: acknowledgement_content,
        };
        assert_event_expected(&vectors.expected.acknowledgement_event, &acknowledgement)?;
        let authorized_acknowledgement = CanonicalAuthorizedEventV1 {
            event: acknowledgement,
            author_certificate: certificate,
            author_authority_snapshot: snapshot,
        };
        assert_eq!(
            to_hex(&encode_authorized_event_v1(&authorized_acknowledgement)?),
            vectors.expected.authorized_acknowledgement_event_cbor_hex
        );
        verify_authorized_event_for_membership_v1(&authorized_acknowledgement, &membership)?;
        Ok(())
    }

    #[test]
    fn event_semantics_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
        let wire: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-WIRE-VECTORS.json"
        )))?;
        let bytes = decode_hex(
            wire.pointer("/acknowledgement_event/expected/signed_event_cbor_hex")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| invalid_data("wire acknowledgement fixture is missing"))?,
        )?;
        let decoded = decode_and_verify_signed_event_v1(&bytes)?;
        assert!(matches!(
            decoded.content.payload,
            CanonicalEventPayloadV1::Acknowledgement { .. }
        ));
        let mut trailing = bytes;
        trailing.push(0);
        assert!(decode_and_verify_signed_event_v1(&trailing).is_err());
        Ok(())
    }
}
