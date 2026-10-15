//! Inactive deterministic-CBOR transport wrapper for endpoint announcements.
//!
//! The active runtime remains Postcard v4/HPKE v1/acknowledgement v2. This
//! module specifies the recipient-only HPKE envelope and recipient-signed,
//! session-bound acknowledgement needed to complete the canonical candidate.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use kilogram_crypto::{DeviceEncryptionIdentity, EncryptionPublicKey, SealedMessage};
use thiserror::Error;

use crate::{
    canonical_account_v1::{AccountDecoder, CanonicalAccountError},
    canonical_endpoint_announcement_v1::{
        CanonicalEndpointAnnouncementError, CanonicalEndpointAnnouncementStateV1,
        CanonicalSignedEndpointAnnouncementBundleV1, MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_BYTES,
        MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_CONTACTS,
        MAX_CANONICAL_ENDPOINTS_PER_ANNOUNCEMENT_CONTACT,
        decode_and_verify_signed_endpoint_announcement_bundle_v1,
        encode_signed_endpoint_announcement_bundle_v1, endpoint_announcement_bundle_id_v1,
        verify_endpoint_announcement_bundle_at_v1, verify_signed_endpoint_announcement_bundle_v1,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
        push_unsigned, validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_CIPHERTEXT_BYTES: usize =
    MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_BYTES + 16;
pub const MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_ENVELOPE_BYTES: usize =
    MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_CIPHERTEXT_BYTES + 512;
pub const MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_ACKNOWLEDGEMENT_BYTES: usize = 4 * 1024;

pub const CANONICAL_ENDPOINT_ANNOUNCEMENT_ENVELOPE_HPKE_INFO: &[u8] =
    b"kilogram:endpoint-announcement-envelope:cbor-v1\0";
pub const CANONICAL_ENDPOINT_ANNOUNCEMENT_ACKNOWLEDGEMENT_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:endpoint-announcement-acknowledgement:cbor-v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalEndpointAnnouncementEnvelopeHeaderV1 {
    pub account_id: [u8; 32],
    pub source_device_id: [u8; 32],
    pub recipient_device_id: [u8; 32],
    pub bundle_id: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalEncryptedEndpointAnnouncementBundleV1 {
    pub header: CanonicalEndpointAnnouncementEnvelopeHeaderV1,
    pub encapsulated_key: [u8; 32],
    pub ciphertext: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CanonicalEndpointAnnouncementAppliedCountsV1 {
    pub contact_added_count: u64,
    pub endpoint_added_count: u64,
    pub publication_binding_added_count: u64,
    pub observation_evidence_added_count: u64,
    pub publication_conflict_added_count: u64,
    pub publication_resolution_added_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalEndpointAnnouncementAcknowledgementContentV1 {
    pub session_binding: [u8; 32],
    pub bundle_id: [u8; 32],
    pub source_device_id: [u8; 32],
    pub recipient_device_id: [u8; 32],
    pub authority_revision: u64,
    pub contact_count: u64,
    pub contact_added_count: u64,
    pub endpoint_count: u64,
    pub endpoint_added_count: u64,
    pub publication_binding_added_count: u64,
    pub observation_evidence_count: u64,
    pub observation_evidence_added_count: u64,
    pub publication_conflict_count: u64,
    pub publication_conflict_added_count: u64,
    pub publication_resolution_count: u64,
    pub publication_resolution_added_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedEndpointAnnouncementAcknowledgementV1 {
    pub content: CanonicalEndpointAnnouncementAcknowledgementContentV1,
    pub signature: [u8; 64],
}

#[derive(Debug, Error)]
pub enum CanonicalEndpointAnnouncementTransportError {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),
    #[error(transparent)]
    Decode(#[from] CanonicalAccountError),
    #[error(transparent)]
    Announcement(#[from] CanonicalEndpointAnnouncementError),
    #[error(transparent)]
    Crypto(#[from] kilogram_crypto::CryptoError),
    #[error("canonical endpoint-announcement transport {object} exceeds {maximum} bytes")]
    ObjectTooLarge {
        object: &'static str,
        maximum: usize,
    },
    #[error("canonical endpoint-announcement envelope is invalid")]
    InvalidEnvelope,
    #[error("canonical endpoint-announcement envelope does not match its plaintext bundle")]
    EnvelopeBundleMismatch,
    #[error("canonical endpoint-announcement recipient encryption key does not match the roster")]
    RecipientEncryptionKeyMismatch,
    #[error("canonical endpoint-announcement acknowledgement is invalid")]
    InvalidAcknowledgement,
    #[error("canonical endpoint-announcement acknowledgement contains an invalid Ed25519 key")]
    InvalidPublicKey,
    #[error("canonical endpoint-announcement acknowledgement signature is invalid")]
    InvalidSignature,
}

pub fn endpoint_announcement_envelope_header_for_bundle_v1(
    bundle: &CanonicalSignedEndpointAnnouncementBundleV1,
) -> Result<
    CanonicalEndpointAnnouncementEnvelopeHeaderV1,
    CanonicalEndpointAnnouncementTransportError,
> {
    verify_signed_endpoint_announcement_bundle_v1(bundle)?;
    Ok(CanonicalEndpointAnnouncementEnvelopeHeaderV1 {
        account_id: bundle
            .content
            .account_device_list
            .content
            .authority_snapshot
            .content
            .account_id,
        source_device_id: bundle.content.source_device_id,
        recipient_device_id: bundle.content.recipient_device_id,
        bundle_id: endpoint_announcement_bundle_id_v1(bundle)?,
    })
}

pub fn encode_endpoint_announcement_envelope_header_v1(
    header: &CanonicalEndpointAnnouncementEnvelopeHeaderV1,
) -> Result<Vec<u8>, CanonicalEndpointAnnouncementTransportError> {
    validate_envelope_header(header)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 5);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &header.account_id);
    push_bytes(&mut output, &header.source_device_id);
    push_bytes(&mut output, &header.recipient_device_id);
    push_bytes(&mut output, &header.bundle_id);
    Ok(output)
}

pub fn decode_and_verify_endpoint_announcement_envelope_header_v1(
    bytes: &[u8],
) -> Result<
    CanonicalEndpointAnnouncementEnvelopeHeaderV1,
    CanonicalEndpointAnnouncementTransportError,
> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("endpoint-announcement-envelope-header-v1", 5)?;
    decoder.expect_version("endpoint-announcement-envelope-header-v1.schema_version")?;
    let header = CanonicalEndpointAnnouncementEnvelopeHeaderV1 {
        account_id: decoder.fixed_bytes("endpoint-announcement-envelope-header-v1.account_id")?,
        source_device_id: decoder
            .fixed_bytes("endpoint-announcement-envelope-header-v1.source_device_id")?,
        recipient_device_id: decoder
            .fixed_bytes("endpoint-announcement-envelope-header-v1.recipient_device_id")?,
        bundle_id: decoder.fixed_bytes("endpoint-announcement-envelope-header-v1.bundle_id")?,
    };
    decoder.finish()?;
    validate_envelope_header(&header)?;
    if encode_endpoint_announcement_envelope_header_v1(&header)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(header)
}

pub fn endpoint_announcement_envelope_aad_v1(
    header: &CanonicalEndpointAnnouncementEnvelopeHeaderV1,
) -> Result<Vec<u8>, CanonicalEndpointAnnouncementTransportError> {
    encode_endpoint_announcement_envelope_header_v1(header)
}

pub const fn endpoint_announcement_envelope_hpke_info_v1() -> &'static [u8] {
    CANONICAL_ENDPOINT_ANNOUNCEMENT_ENVELOPE_HPKE_INFO
}

pub fn encode_encrypted_endpoint_announcement_bundle_v1(
    envelope: &CanonicalEncryptedEndpointAnnouncementBundleV1,
) -> Result<Vec<u8>, CanonicalEndpointAnnouncementTransportError> {
    validate_envelope(envelope)?;
    let header = encode_endpoint_announcement_envelope_header_v1(&envelope.header)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 4);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &header);
    push_bytes(&mut output, &envelope.encapsulated_key);
    push_bytes(&mut output, &envelope.ciphertext);
    ensure_bound(
        &output,
        "encrypted bundle",
        MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_ENVELOPE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_encrypted_endpoint_announcement_bundle_v1(
    bytes: &[u8],
) -> Result<
    CanonicalEncryptedEndpointAnnouncementBundleV1,
    CanonicalEndpointAnnouncementTransportError,
> {
    ensure_bound(
        bytes,
        "encrypted bundle",
        MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_ENVELOPE_BYTES,
    )?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("encrypted-endpoint-announcement-bundle-v1", 4)?;
    decoder.expect_version("encrypted-endpoint-announcement-bundle-v1.schema_version")?;
    let header = decode_and_verify_endpoint_announcement_envelope_header_v1(
        decoder.bytes("encrypted-endpoint-announcement-bundle-v1.header")?,
    )?;
    let encapsulated_key =
        decoder.fixed_bytes("encrypted-endpoint-announcement-bundle-v1.encapsulated_key")?;
    let ciphertext = decoder
        .bytes("encrypted-endpoint-announcement-bundle-v1.ciphertext")?
        .to_vec();
    decoder.finish()?;
    let envelope = CanonicalEncryptedEndpointAnnouncementBundleV1 {
        header,
        encapsulated_key,
        ciphertext,
    };
    validate_envelope(&envelope)?;
    if encode_encrypted_endpoint_announcement_bundle_v1(&envelope)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(envelope)
}

pub fn seal_endpoint_announcement_bundle_v1(
    bundle: &CanonicalSignedEndpointAnnouncementBundleV1,
) -> Result<
    CanonicalEncryptedEndpointAnnouncementBundleV1,
    CanonicalEndpointAnnouncementTransportError,
> {
    verify_signed_endpoint_announcement_bundle_v1(bundle)?;
    let header = endpoint_announcement_envelope_header_for_bundle_v1(bundle)?;
    let certificate = bundle
        .content
        .account_device_list
        .content
        .devices
        .iter()
        .find(|value| value.content.device_id == bundle.content.recipient_device_id)
        .ok_or(CanonicalEndpointAnnouncementTransportError::InvalidEnvelope)?;
    let recipient_key = EncryptionPublicKey::from_bytes(certificate.content.encryption_public_key)?;
    let aad = endpoint_announcement_envelope_aad_v1(&header)?;
    let sealed = recipient_key.seal(
        &encode_signed_endpoint_announcement_bundle_v1(bundle)?,
        endpoint_announcement_envelope_hpke_info_v1(),
        &aad,
    )?;
    let encapsulated_key: [u8; 32] = sealed
        .encapsulated_key
        .try_into()
        .map_err(|_| CanonicalEndpointAnnouncementTransportError::InvalidEnvelope)?;
    let envelope = CanonicalEncryptedEndpointAnnouncementBundleV1 {
        header,
        encapsulated_key,
        ciphertext: sealed.ciphertext,
    };
    validate_envelope(&envelope)?;
    Ok(envelope)
}

pub fn open_endpoint_announcement_bundle_v1(
    envelope: &CanonicalEncryptedEndpointAnnouncementBundleV1,
    recipient_encryption: &DeviceEncryptionIdentity,
    now_unix_seconds: u64,
) -> Result<CanonicalSignedEndpointAnnouncementBundleV1, CanonicalEndpointAnnouncementTransportError>
{
    validate_envelope(envelope)?;
    let aad = endpoint_announcement_envelope_aad_v1(&envelope.header)?;
    let plaintext = recipient_encryption.open(
        &SealedMessage {
            encapsulated_key: envelope.encapsulated_key.to_vec(),
            ciphertext: envelope.ciphertext.clone(),
        },
        endpoint_announcement_envelope_hpke_info_v1(),
        &aad,
    )?;
    let bundle = decode_and_verify_signed_endpoint_announcement_bundle_v1(&plaintext)?;
    verify_endpoint_announcement_bundle_at_v1(&bundle, now_unix_seconds)?;
    if endpoint_announcement_envelope_header_for_bundle_v1(&bundle)? != envelope.header {
        return Err(CanonicalEndpointAnnouncementTransportError::EnvelopeBundleMismatch);
    }
    let certificate = bundle
        .content
        .account_device_list
        .content
        .devices
        .iter()
        .find(|value| value.content.device_id == bundle.content.recipient_device_id)
        .ok_or(CanonicalEndpointAnnouncementTransportError::InvalidEnvelope)?;
    if recipient_encryption.public_key().as_bytes() != &certificate.content.encryption_public_key {
        return Err(CanonicalEndpointAnnouncementTransportError::RecipientEncryptionKeyMismatch);
    }
    Ok(bundle)
}

pub fn sign_endpoint_announcement_acknowledgement_v1(
    signing_key: &SigningKey,
    session_binding: [u8; 32],
    bundle: &CanonicalSignedEndpointAnnouncementBundleV1,
    applied: CanonicalEndpointAnnouncementAppliedCountsV1,
) -> Result<
    CanonicalSignedEndpointAnnouncementAcknowledgementV1,
    CanonicalEndpointAnnouncementTransportError,
> {
    verify_signed_endpoint_announcement_bundle_v1(bundle)?;
    let recipient_device_id = signing_key.verifying_key().to_bytes();
    if recipient_device_id != bundle.content.recipient_device_id {
        return Err(CanonicalEndpointAnnouncementTransportError::InvalidAcknowledgement);
    }
    let statistics = bundle_statistics(bundle)?;
    let content = CanonicalEndpointAnnouncementAcknowledgementContentV1 {
        session_binding,
        bundle_id: endpoint_announcement_bundle_id_v1(bundle)?,
        source_device_id: bundle.content.source_device_id,
        recipient_device_id,
        authority_revision: bundle
            .content
            .account_device_list
            .content
            .authority_snapshot
            .content
            .revision,
        contact_count: statistics.contact_count,
        contact_added_count: applied.contact_added_count,
        endpoint_count: statistics.endpoint_count,
        endpoint_added_count: applied.endpoint_added_count,
        publication_binding_added_count: applied.publication_binding_added_count,
        observation_evidence_count: statistics.observation_evidence_count,
        observation_evidence_added_count: applied.observation_evidence_added_count,
        publication_conflict_count: statistics.publication_conflict_count,
        publication_conflict_added_count: applied.publication_conflict_added_count,
        publication_resolution_count: statistics.publication_resolution_count,
        publication_resolution_added_count: applied.publication_resolution_added_count,
    };
    validate_acknowledgement_content(&content)?;
    let signature = signing_key
        .sign(&endpoint_announcement_acknowledgement_signing_bytes_v1(
            &content,
        )?)
        .to_bytes();
    let acknowledgement =
        CanonicalSignedEndpointAnnouncementAcknowledgementV1 { content, signature };
    verify_endpoint_announcement_acknowledgement_for_v1(&acknowledgement, session_binding, bundle)?;
    Ok(acknowledgement)
}

pub fn encode_endpoint_announcement_acknowledgement_content_v1(
    content: &CanonicalEndpointAnnouncementAcknowledgementContentV1,
) -> Result<Vec<u8>, CanonicalEndpointAnnouncementTransportError> {
    validate_acknowledgement_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 17);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.session_binding);
    push_bytes(&mut output, &content.bundle_id);
    push_bytes(&mut output, &content.source_device_id);
    push_bytes(&mut output, &content.recipient_device_id);
    push_unsigned(&mut output, content.authority_revision);
    push_unsigned(&mut output, content.contact_count);
    push_unsigned(&mut output, content.contact_added_count);
    push_unsigned(&mut output, content.endpoint_count);
    push_unsigned(&mut output, content.endpoint_added_count);
    push_unsigned(&mut output, content.publication_binding_added_count);
    push_unsigned(&mut output, content.observation_evidence_count);
    push_unsigned(&mut output, content.observation_evidence_added_count);
    push_unsigned(&mut output, content.publication_conflict_count);
    push_unsigned(&mut output, content.publication_conflict_added_count);
    push_unsigned(&mut output, content.publication_resolution_count);
    push_unsigned(&mut output, content.publication_resolution_added_count);
    Ok(output)
}

pub fn endpoint_announcement_acknowledgement_signing_bytes_v1(
    content: &CanonicalEndpointAnnouncementAcknowledgementContentV1,
) -> Result<Vec<u8>, CanonicalEndpointAnnouncementTransportError> {
    Ok(prefixed(
        CANONICAL_ENDPOINT_ANNOUNCEMENT_ACKNOWLEDGEMENT_SIGNATURE_DOMAIN,
        &encode_endpoint_announcement_acknowledgement_content_v1(content)?,
    ))
}

pub fn encode_signed_endpoint_announcement_acknowledgement_v1(
    acknowledgement: &CanonicalSignedEndpointAnnouncementAcknowledgementV1,
) -> Result<Vec<u8>, CanonicalEndpointAnnouncementTransportError> {
    verify_signed_endpoint_announcement_acknowledgement_v1(acknowledgement)?;
    let content =
        encode_endpoint_announcement_acknowledgement_content_v1(&acknowledgement.content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content);
    push_bytes(&mut output, &acknowledgement.signature);
    ensure_bound(
        &output,
        "signed acknowledgement",
        MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_ACKNOWLEDGEMENT_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_endpoint_announcement_acknowledgement_v1(
    bytes: &[u8],
) -> Result<
    CanonicalSignedEndpointAnnouncementAcknowledgementV1,
    CanonicalEndpointAnnouncementTransportError,
> {
    ensure_bound(
        bytes,
        "signed acknowledgement",
        MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_ACKNOWLEDGEMENT_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-endpoint-announcement-acknowledgement-v1", 3)?;
    decoder.expect_version("signed-endpoint-announcement-acknowledgement-v1.schema_version")?;
    let content = decode_endpoint_announcement_acknowledgement_content_v1(
        decoder.bytes("signed-endpoint-announcement-acknowledgement-v1.content")?,
    )?;
    let signature =
        decoder.fixed_bytes("signed-endpoint-announcement-acknowledgement-v1.signature")?;
    decoder.finish()?;
    let acknowledgement =
        CanonicalSignedEndpointAnnouncementAcknowledgementV1 { content, signature };
    verify_signed_endpoint_announcement_acknowledgement_v1(&acknowledgement)?;
    if encode_signed_endpoint_announcement_acknowledgement_v1(&acknowledgement)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(acknowledgement)
}

pub fn verify_signed_endpoint_announcement_acknowledgement_v1(
    acknowledgement: &CanonicalSignedEndpointAnnouncementAcknowledgementV1,
) -> Result<(), CanonicalEndpointAnnouncementTransportError> {
    validate_acknowledgement_content(&acknowledgement.content)?;
    let key = VerifyingKey::from_bytes(&acknowledgement.content.recipient_device_id)
        .map_err(|_| CanonicalEndpointAnnouncementTransportError::InvalidPublicKey)?;
    key.verify_strict(
        &endpoint_announcement_acknowledgement_signing_bytes_v1(&acknowledgement.content)?,
        &Signature::from_bytes(&acknowledgement.signature),
    )
    .map_err(|_| CanonicalEndpointAnnouncementTransportError::InvalidSignature)
}

pub fn verify_endpoint_announcement_acknowledgement_for_v1(
    acknowledgement: &CanonicalSignedEndpointAnnouncementAcknowledgementV1,
    session_binding: [u8; 32],
    bundle: &CanonicalSignedEndpointAnnouncementBundleV1,
) -> Result<(), CanonicalEndpointAnnouncementTransportError> {
    verify_signed_endpoint_announcement_acknowledgement_v1(acknowledgement)?;
    verify_signed_endpoint_announcement_bundle_v1(bundle)?;
    let statistics = bundle_statistics(bundle)?;
    let content = &acknowledgement.content;
    if content.session_binding != session_binding
        || content.bundle_id != endpoint_announcement_bundle_id_v1(bundle)?
        || content.source_device_id != bundle.content.source_device_id
        || content.recipient_device_id != bundle.content.recipient_device_id
        || content.authority_revision
            != bundle
                .content
                .account_device_list
                .content
                .authority_snapshot
                .content
                .revision
        || content.contact_count != statistics.contact_count
        || content.endpoint_count != statistics.endpoint_count
        || content.observation_evidence_count != statistics.observation_evidence_count
        || content.publication_conflict_count != statistics.publication_conflict_count
        || content.publication_resolution_count != statistics.publication_resolution_count
    {
        return Err(CanonicalEndpointAnnouncementTransportError::InvalidAcknowledgement);
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct BundleStatistics {
    contact_count: u64,
    endpoint_count: u64,
    observation_evidence_count: u64,
    publication_conflict_count: u64,
    publication_resolution_count: u64,
}

fn bundle_statistics(
    bundle: &CanonicalSignedEndpointAnnouncementBundleV1,
) -> Result<BundleStatistics, CanonicalEndpointAnnouncementTransportError> {
    verify_signed_endpoint_announcement_bundle_v1(bundle)?;
    let mut statistics = BundleStatistics {
        contact_count: bundle.content.contacts.len() as u64,
        endpoint_count: 0,
        observation_evidence_count: 0,
        publication_conflict_count: 0,
        publication_resolution_count: 0,
    };
    for contact in &bundle.content.contacts {
        statistics.endpoint_count = statistics
            .endpoint_count
            .checked_add(contact.endpoints.len() as u64)
            .ok_or(CanonicalEndpointAnnouncementTransportError::InvalidAcknowledgement)?;
        for endpoint in &contact.endpoints {
            match &endpoint.state {
                CanonicalEndpointAnnouncementStateV1::DirectObservation(_)
                | CanonicalEndpointAnnouncementStateV1::AcceptedObservation(_) => {
                    statistics.observation_evidence_count += 1;
                }
                CanonicalEndpointAnnouncementStateV1::PublicationConflict(_) => {
                    statistics.publication_conflict_count += 1;
                }
                CanonicalEndpointAnnouncementStateV1::PublicationResolution(_) => {
                    statistics.publication_resolution_count += 1;
                }
                CanonicalEndpointAnnouncementStateV1::None => {}
            }
        }
    }
    Ok(statistics)
}

fn decode_endpoint_announcement_acknowledgement_content_v1(
    bytes: &[u8],
) -> Result<
    CanonicalEndpointAnnouncementAcknowledgementContentV1,
    CanonicalEndpointAnnouncementTransportError,
> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("endpoint-announcement-acknowledgement-content-v1", 17)?;
    decoder.expect_version("endpoint-announcement-acknowledgement-content-v1.schema_version")?;
    let content = CanonicalEndpointAnnouncementAcknowledgementContentV1 {
        session_binding: decoder
            .fixed_bytes("endpoint-announcement-acknowledgement-content-v1.session_binding")?,
        bundle_id: decoder
            .fixed_bytes("endpoint-announcement-acknowledgement-content-v1.bundle_id")?,
        source_device_id: decoder
            .fixed_bytes("endpoint-announcement-acknowledgement-content-v1.source_device_id")?,
        recipient_device_id: decoder
            .fixed_bytes("endpoint-announcement-acknowledgement-content-v1.recipient_device_id")?,
        authority_revision: decoder
            .unsigned("endpoint-announcement-acknowledgement-content-v1.authority_revision")?,
        contact_count: decoder
            .unsigned("endpoint-announcement-acknowledgement-content-v1.contact_count")?,
        contact_added_count: decoder
            .unsigned("endpoint-announcement-acknowledgement-content-v1.contact_added_count")?,
        endpoint_count: decoder
            .unsigned("endpoint-announcement-acknowledgement-content-v1.endpoint_count")?,
        endpoint_added_count: decoder
            .unsigned("endpoint-announcement-acknowledgement-content-v1.endpoint_added_count")?,
        publication_binding_added_count: decoder.unsigned(
            "endpoint-announcement-acknowledgement-content-v1.publication_binding_added_count",
        )?,
        observation_evidence_count: decoder.unsigned(
            "endpoint-announcement-acknowledgement-content-v1.observation_evidence_count",
        )?,
        observation_evidence_added_count: decoder.unsigned(
            "endpoint-announcement-acknowledgement-content-v1.observation_evidence_added_count",
        )?,
        publication_conflict_count: decoder.unsigned(
            "endpoint-announcement-acknowledgement-content-v1.publication_conflict_count",
        )?,
        publication_conflict_added_count: decoder.unsigned(
            "endpoint-announcement-acknowledgement-content-v1.publication_conflict_added_count",
        )?,
        publication_resolution_count: decoder.unsigned(
            "endpoint-announcement-acknowledgement-content-v1.publication_resolution_count",
        )?,
        publication_resolution_added_count: decoder.unsigned(
            "endpoint-announcement-acknowledgement-content-v1.publication_resolution_added_count",
        )?,
    };
    decoder.finish()?;
    validate_acknowledgement_content(&content)?;
    if encode_endpoint_announcement_acknowledgement_content_v1(&content)? != bytes {
        return Err(CanonicalWireError::NonMinimalArgument.into());
    }
    Ok(content)
}

fn validate_envelope_header(
    header: &CanonicalEndpointAnnouncementEnvelopeHeaderV1,
) -> Result<(), CanonicalEndpointAnnouncementTransportError> {
    if header.account_id == [0; 32]
        || header.source_device_id == [0; 32]
        || header.recipient_device_id == [0; 32]
        || header.bundle_id == [0; 32]
        || header.source_device_id == header.recipient_device_id
    {
        return Err(CanonicalEndpointAnnouncementTransportError::InvalidEnvelope);
    }
    Ok(())
}

fn validate_envelope(
    envelope: &CanonicalEncryptedEndpointAnnouncementBundleV1,
) -> Result<(), CanonicalEndpointAnnouncementTransportError> {
    validate_envelope_header(&envelope.header)?;
    if envelope.encapsulated_key == [0; 32]
        || envelope.ciphertext.is_empty()
        || envelope.ciphertext.len() > MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_CIPHERTEXT_BYTES
    {
        return Err(CanonicalEndpointAnnouncementTransportError::InvalidEnvelope);
    }
    Ok(())
}

fn validate_acknowledgement_content(
    content: &CanonicalEndpointAnnouncementAcknowledgementContentV1,
) -> Result<(), CanonicalEndpointAnnouncementTransportError> {
    let maximum_endpoints = (MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_CONTACTS
        * MAX_CANONICAL_ENDPOINTS_PER_ANNOUNCEMENT_CONTACT) as u64;
    if content.session_binding == [0; 32]
        || content.bundle_id == [0; 32]
        || content.source_device_id == [0; 32]
        || content.recipient_device_id == [0; 32]
        || content.source_device_id == content.recipient_device_id
        || content.authority_revision == 0
        || content.contact_count > MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_CONTACTS as u64
        || content.endpoint_count > maximum_endpoints
        || content.contact_added_count > content.contact_count
        || content.endpoint_added_count > content.endpoint_count
        || content.publication_binding_added_count > content.endpoint_count
        || content.observation_evidence_count > content.endpoint_count
        || content.observation_evidence_added_count > content.observation_evidence_count
        || content.publication_conflict_count > content.endpoint_count
        || content.publication_conflict_added_count > content.publication_conflict_count
        || content.publication_resolution_count > content.endpoint_count
        || content.publication_resolution_added_count > content.publication_resolution_count
    {
        return Err(CanonicalEndpointAnnouncementTransportError::InvalidAcknowledgement);
    }
    Ok(())
}

fn ensure_bound(
    bytes: &[u8],
    object: &'static str,
    maximum: usize,
) -> Result<(), CanonicalEndpointAnnouncementTransportError> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(
            CanonicalEndpointAnnouncementTransportError::ObjectTooLarge { object, maximum },
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct TransportVectors {
        schema_version: u64,
        profile: String,
        activation_status: String,
        dependency: String,
        input: TransportInput,
        expected: TransportExpected,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct TransportInput {
        session_binding_hex: String,
        applied_counts: AppliedCounts,
        fixed_hpke_seal: FixedSeal,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct AppliedCounts {
        contact_added_count: u64,
        endpoint_added_count: u64,
        publication_binding_added_count: u64,
        observation_evidence_added_count: u64,
        publication_conflict_added_count: u64,
        publication_resolution_added_count: u64,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct FixedSeal {
        encapsulated_key_hex: String,
        ciphertext_hex: String,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct TransportExpected {
        domains: Domains,
        bundle_statistics: Statistics,
        envelope_header: HeaderExpected,
        encrypted_bundle: EncryptedExpected,
        acknowledgement: AcknowledgementExpected,
        transport_binding_hash_hex: String,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Domains {
        hpke_info_hex: String,
        acknowledgement_signature_domain_hex: String,
        transport_binding_domain_hex: String,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Statistics {
        authority_revision: u64,
        contact_count: u64,
        endpoint_count: u64,
        observation_evidence_count: u64,
        publication_conflict_count: u64,
        publication_resolution_count: u64,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct HeaderExpected {
        cbor_hex: String,
        aad_hex: String,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct EncryptedExpected {
        encapsulated_key_hex: String,
        ciphertext_hex: String,
        cbor_hex: String,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct AcknowledgementExpected {
        content_cbor_hex: String,
        signing_preimage_hex: String,
        signature_hex: String,
        signed_cbor_hex: String,
    }

    #[test]
    fn endpoint_announcement_transport_vectors_open_acknowledge_and_round_trip()
    -> Result<(), Box<dyn std::error::Error>> {
        let vectors = transport_vectors()?;
        assert_eq!(vectors.schema_version, 1);
        assert_eq!(vectors.profile, "kilogram-deterministic-cbor-v1");
        assert_eq!(vectors.activation_status, "not-active");
        assert_eq!(
            vectors.dependency,
            "CANONICAL-ENDPOINT-ANNOUNCEMENT-VECTORS.json"
        );
        assert_eq!(
            vectors.expected.domains.hpke_info_hex,
            encode_hex(endpoint_announcement_envelope_hpke_info_v1())
        );
        assert_eq!(
            vectors
                .expected
                .domains
                .acknowledgement_signature_domain_hex,
            encode_hex(CANONICAL_ENDPOINT_ANNOUNCEMENT_ACKNOWLEDGEMENT_SIGNATURE_DOMAIN)
        );
        assert_eq!(
            vectors.expected.domains.transport_binding_domain_hex,
            encode_hex(b"kilogram:endpoint-announcement-transport-vector:cbor-v1\0")
        );
        assert_eq!(vectors.expected.transport_binding_hash_hex.len(), 64);

        let endpoint_vectors = endpoint_vectors()?;
        let bundle = endpoint_bundle(&endpoint_vectors)?;
        let header = endpoint_announcement_envelope_header_for_bundle_v1(&bundle)?;
        let header_bytes = encode_endpoint_announcement_envelope_header_v1(&header)?;
        assert_eq!(
            encode_hex(&header_bytes),
            vectors.expected.envelope_header.cbor_hex
        );
        assert_eq!(
            endpoint_announcement_envelope_aad_v1(&header)?,
            header_bytes
        );
        assert_eq!(
            vectors.expected.envelope_header.aad_hex,
            vectors.expected.envelope_header.cbor_hex
        );

        let envelope_bytes = decode_hex(&vectors.expected.encrypted_bundle.cbor_hex)?;
        let envelope =
            decode_and_verify_encrypted_endpoint_announcement_bundle_v1(&envelope_bytes)?;
        assert_eq!(
            encode_encrypted_endpoint_announcement_bundle_v1(&envelope)?,
            envelope_bytes
        );
        assert_eq!(
            encode_hex(&envelope.encapsulated_key),
            vectors.expected.encrypted_bundle.encapsulated_key_hex
        );
        assert_eq!(
            encode_hex(&envelope.ciphertext),
            vectors.expected.encrypted_bundle.ciphertext_hex
        );
        assert_eq!(
            vectors.input.fixed_hpke_seal.encapsulated_key_hex,
            vectors.expected.encrypted_bundle.encapsulated_key_hex
        );
        assert_eq!(
            vectors.input.fixed_hpke_seal.ciphertext_hex,
            vectors.expected.encrypted_bundle.ciphertext_hex
        );
        let recipient_encryption =
            DeviceEncryptionIdentity::from_secret_bytes(fixed_hex(text_at(
                &endpoint_vectors,
                &["input", "recipient_encryption_secret_seed_hex"],
            )?)?);
        assert_eq!(
            encode_hex(recipient_encryption.public_key().as_bytes()),
            text_at(
                &endpoint_vectors,
                &["input", "recipient_encryption_public_key_hex"],
            )?
        );
        let opened = open_endpoint_announcement_bundle_v1(
            &envelope,
            &recipient_encryption,
            unsigned_at(
                &endpoint_vectors,
                &["expected", "signed_bundle", "valid_at_unix_seconds"],
            )?,
        )?;
        assert_eq!(opened, bundle);

        let acknowledgement_bytes = decode_hex(&vectors.expected.acknowledgement.signed_cbor_hex)?;
        let acknowledgement = decode_and_verify_signed_endpoint_announcement_acknowledgement_v1(
            &acknowledgement_bytes,
        )?;
        assert_eq!(
            encode_signed_endpoint_announcement_acknowledgement_v1(&acknowledgement)?,
            acknowledgement_bytes
        );
        let session_binding = fixed_hex(&vectors.input.session_binding_hex)?;
        verify_endpoint_announcement_acknowledgement_for_v1(
            &acknowledgement,
            session_binding,
            &bundle,
        )?;
        let recipient_signing = SigningKey::from_bytes(&fixed_hex(text_at(
            &endpoint_vectors,
            &["input", "recipient_device_seed_hex"],
        )?)?);
        let generated = sign_endpoint_announcement_acknowledgement_v1(
            &recipient_signing,
            session_binding,
            &bundle,
            CanonicalEndpointAnnouncementAppliedCountsV1 {
                contact_added_count: vectors.input.applied_counts.contact_added_count,
                endpoint_added_count: vectors.input.applied_counts.endpoint_added_count,
                publication_binding_added_count: vectors
                    .input
                    .applied_counts
                    .publication_binding_added_count,
                observation_evidence_added_count: vectors
                    .input
                    .applied_counts
                    .observation_evidence_added_count,
                publication_conflict_added_count: vectors
                    .input
                    .applied_counts
                    .publication_conflict_added_count,
                publication_resolution_added_count: vectors
                    .input
                    .applied_counts
                    .publication_resolution_added_count,
            },
        )?;
        assert_eq!(generated, acknowledgement);
        assert_eq!(
            encode_hex(&encode_endpoint_announcement_acknowledgement_content_v1(
                &acknowledgement.content,
            )?),
            vectors.expected.acknowledgement.content_cbor_hex
        );
        assert_eq!(
            encode_hex(&endpoint_announcement_acknowledgement_signing_bytes_v1(
                &acknowledgement.content,
            )?),
            vectors.expected.acknowledgement.signing_preimage_hex
        );
        assert_eq!(
            encode_hex(&acknowledgement.signature),
            vectors.expected.acknowledgement.signature_hex
        );
        assert_statistics(
            &acknowledgement.content,
            &vectors.expected.bundle_statistics,
        );
        Ok(())
    }

    #[test]
    fn endpoint_announcement_transport_rejects_wrong_recipient_tampering_and_replay()
    -> Result<(), Box<dyn std::error::Error>> {
        let vectors = transport_vectors()?;
        let endpoint_vectors = endpoint_vectors()?;
        let bundle = endpoint_bundle(&endpoint_vectors)?;
        let envelope_bytes = decode_hex(&vectors.expected.encrypted_bundle.cbor_hex)?;
        let envelope =
            decode_and_verify_encrypted_endpoint_announcement_bundle_v1(&envelope_bytes)?;
        let valid_at = unsigned_at(
            &endpoint_vectors,
            &["expected", "signed_bundle", "valid_at_unix_seconds"],
        )?;
        let recipient = DeviceEncryptionIdentity::from_secret_bytes(fixed_hex(text_at(
            &endpoint_vectors,
            &["input", "recipient_encryption_secret_seed_hex"],
        )?)?);
        let wrong_recipient = DeviceEncryptionIdentity::from_secret_bytes([0xee; 32]);
        assert!(
            open_endpoint_announcement_bundle_v1(&envelope, &wrong_recipient, valid_at).is_err()
        );

        let mut tampered_ciphertext = envelope.clone();
        tampered_ciphertext.ciphertext[0] ^= 1;
        assert!(
            open_endpoint_announcement_bundle_v1(&tampered_ciphertext, &recipient, valid_at)
                .is_err()
        );
        let mut tampered_header = envelope;
        tampered_header.header.bundle_id[0] ^= 1;
        assert!(
            open_endpoint_announcement_bundle_v1(&tampered_header, &recipient, valid_at).is_err()
        );

        let acknowledgement = decode_and_verify_signed_endpoint_announcement_acknowledgement_v1(
            &decode_hex(&vectors.expected.acknowledgement.signed_cbor_hex)?,
        )?;
        let session_binding = fixed_hex(&vectors.input.session_binding_hex)?;
        let mut wrong_session = session_binding;
        wrong_session[0] ^= 1;
        assert!(
            verify_endpoint_announcement_acknowledgement_for_v1(
                &acknowledgement,
                wrong_session,
                &bundle,
            )
            .is_err()
        );
        let mut changed_count = acknowledgement;
        changed_count.content.contact_added_count -= 1;
        assert!(verify_signed_endpoint_announcement_acknowledgement_v1(&changed_count).is_err());
        assert!(
            sign_endpoint_announcement_acknowledgement_v1(
                &SigningKey::from_bytes(&[0xef; 32]),
                session_binding,
                &bundle,
                CanonicalEndpointAnnouncementAppliedCountsV1::default(),
            )
            .is_err()
        );
        Ok(())
    }

    fn assert_statistics(
        content: &CanonicalEndpointAnnouncementAcknowledgementContentV1,
        expected: &Statistics,
    ) {
        assert_eq!(content.authority_revision, expected.authority_revision);
        assert_eq!(content.contact_count, expected.contact_count);
        assert_eq!(content.endpoint_count, expected.endpoint_count);
        assert_eq!(
            content.observation_evidence_count,
            expected.observation_evidence_count
        );
        assert_eq!(
            content.publication_conflict_count,
            expected.publication_conflict_count
        );
        assert_eq!(
            content.publication_resolution_count,
            expected.publication_resolution_count
        );
    }

    fn transport_vectors() -> Result<TransportVectors, Box<dyn std::error::Error>> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-ENDPOINT-ANNOUNCEMENT-TRANSPORT-VECTORS.json"
        )))?)
    }

    fn endpoint_vectors() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-ENDPOINT-ANNOUNCEMENT-VECTORS.json"
        )))?)
    }

    fn endpoint_bundle(
        vectors: &serde_json::Value,
    ) -> Result<CanonicalSignedEndpointAnnouncementBundleV1, Box<dyn std::error::Error>> {
        Ok(decode_and_verify_signed_endpoint_announcement_bundle_v1(
            &decode_hex(text_at(
                vectors,
                &["expected", "signed_bundle", "signed_cbor_hex"],
            )?)?,
        )?)
    }

    fn text_at<'a>(
        value: &'a serde_json::Value,
        path: &[&str],
    ) -> Result<&'a str, Box<dyn std::error::Error>> {
        let mut current = value;
        for segment in path {
            current = current.get(segment).ok_or("missing JSON field")?;
        }
        current
            .as_str()
            .ok_or_else(|| "JSON field is not text".into())
    }

    fn unsigned_at(
        value: &serde_json::Value,
        path: &[&str],
    ) -> Result<u64, Box<dyn std::error::Error>> {
        let mut current = value;
        for segment in path {
            current = current.get(segment).ok_or("missing JSON field")?;
        }
        current
            .as_u64()
            .ok_or_else(|| "JSON field is not unsigned".into())
    }

    fn fixed_hex(value: &str) -> Result<[u8; 32], Box<dyn std::error::Error>> {
        Ok(decode_hex(value)?
            .try_into()
            .map_err(|_| "hex value is not 32 bytes")?)
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

    fn encode_hex(bytes: &[u8]) -> String {
        use std::fmt::Write as _;
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            let _ = write!(output, "{byte:02x}");
        }
        output
    }
}
