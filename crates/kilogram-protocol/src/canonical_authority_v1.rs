//! Inactive deterministic-CBOR authority and session candidate for M1.1.2.
//!
//! The active messenger continues to use the versioned Postcard objects in
//! `wire` and `kilogram-identity`. This module reserves a cross-language shape,
//! provides strict encode/decode/verify operations, and is not connected to a
//! runtime protocol identifier.

use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

use crate::canonical_v1::{
    CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
    push_unsigned, validate_canonical_profile_v1,
};

pub const MAX_CANONICAL_DEVICE_CAPABILITIES: usize = 2;
pub const MAX_CANONICAL_AUTHORITY_REVOCATIONS: usize = 32;

pub const CANONICAL_DEVICE_CERTIFICATE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:device-certificate-signature:cbor-v1\0";
pub const CANONICAL_DEVICE_REVOCATION_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:device-revocation-signature:cbor-v1\0";
pub const CANONICAL_AUTHORITY_SNAPSHOT_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:account-authority-snapshot-signature:cbor-v1\0";
pub const CANONICAL_DEVICE_SESSION_AUTHORIZATION_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:device-session-authorization-signature:cbor-v1\0";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u64)]
pub enum CanonicalDeviceCapabilityV1 {
    SignEvents = 1,
    SyncHistory = 2,
}

impl CanonicalDeviceCapabilityV1 {
    pub const fn code(self) -> u64 {
        self as u64
    }

    fn from_code(code: u64) -> Result<Self, CanonicalAuthorityError> {
        match code {
            1 => Ok(Self::SignEvents),
            2 => Ok(Self::SyncHistory),
            value => Err(CanonicalAuthorityError::UnknownCapability(value)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalDeviceCertificateContentV1 {
    pub account_id: [u8; 32],
    pub device_id: [u8; 32],
    pub encryption_public_key: [u8; 32],
    pub authority_sequence: u64,
    pub capabilities: Vec<CanonicalDeviceCapabilityV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedDeviceCertificateV1 {
    pub content: CanonicalDeviceCertificateContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalDeviceRevocationContentV1 {
    pub account_id: [u8; 32],
    pub device_id: [u8; 32],
    pub authority_sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedDeviceRevocationV1 {
    pub content: CanonicalDeviceRevocationContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalAccountAuthoritySnapshotContentV1 {
    pub account_id: [u8; 32],
    pub revision: u64,
    pub revocations: Vec<CanonicalSignedDeviceRevocationV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedAccountAuthoritySnapshotV1 {
    pub content: CanonicalAccountAuthoritySnapshotContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalDeviceSessionAuthorizationContentV1 {
    pub session_binding: [u8; 32],
    pub certificate: CanonicalSignedDeviceCertificateV1,
    pub authority_snapshot: CanonicalSignedAccountAuthoritySnapshotV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedDeviceSessionAuthorizationV1 {
    pub content: CanonicalDeviceSessionAuthorizationContentV1,
    pub signature: [u8; 64],
}

#[derive(Debug, Error)]
pub enum CanonicalAuthorityError {
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

    #[error("unknown canonical device capability {0}")]
    UnknownCapability(u64),

    #[error("canonical device capabilities must not be empty")]
    EmptyCapabilities,

    #[error("canonical device capabilities must be strictly increasing")]
    NonCanonicalCapabilities,

    #[error("canonical authority revocations must be strictly ordered by device ID")]
    NonCanonicalRevocations,

    #[error("{context} names a different Account ID")]
    AccountMismatch { context: &'static str },

    #[error(
        "certificate authority sequence {certificate_sequence} is not before snapshot revision {snapshot_revision}"
    )]
    CertificateOutsideSnapshot {
        certificate_sequence: u64,
        snapshot_revision: u64,
    },

    #[error(
        "revocation authority sequence {revocation_sequence} is not before snapshot revision {snapshot_revision}"
    )]
    RevocationOutsideSnapshot {
        revocation_sequence: u64,
        snapshot_revision: u64,
    },

    #[error("the session certificate's Device ID is revoked by its authority snapshot")]
    SessionDeviceRevoked,

    #[error("{object} contains an invalid Ed25519 verifying key")]
    InvalidPublicKey { object: &'static str },

    #[error("{object} contains an invalid Ed25519 signature")]
    InvalidSignature { object: &'static str },
}

pub fn encode_device_certificate_content_v1(
    content: &CanonicalDeviceCertificateContentV1,
) -> Result<Vec<u8>, CanonicalAuthorityError> {
    validate_certificate_content(content)?;
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 6);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut encoded, &content.account_id);
    push_bytes(&mut encoded, &content.device_id);
    push_bytes(&mut encoded, &content.encryption_public_key);
    push_unsigned(&mut encoded, content.authority_sequence);
    push_array_header(&mut encoded, content.capabilities.len() as u64);
    for capability in &content.capabilities {
        push_unsigned(&mut encoded, capability.code());
    }
    Ok(encoded)
}

pub fn device_certificate_signing_bytes_v1(
    content: &CanonicalDeviceCertificateContentV1,
) -> Result<Vec<u8>, CanonicalAuthorityError> {
    Ok(prefixed(
        CANONICAL_DEVICE_CERTIFICATE_SIGNATURE_DOMAIN,
        &encode_device_certificate_content_v1(content)?,
    ))
}

pub fn encode_signed_device_certificate_v1(
    certificate: &CanonicalSignedDeviceCertificateV1,
) -> Result<Vec<u8>, CanonicalAuthorityError> {
    verify_signed_device_certificate_v1(certificate)?;
    encode_signed_device_certificate_unverified(certificate)
}

pub fn decode_signed_device_certificate_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedDeviceCertificateV1, CanonicalAuthorityError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AuthorityDecoder::new(bytes);
    let certificate = decode_signed_device_certificate_from(&mut decoder)?;
    decoder.finish()?;
    Ok(certificate)
}

pub fn decode_and_verify_signed_device_certificate_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedDeviceCertificateV1, CanonicalAuthorityError> {
    let certificate = decode_signed_device_certificate_v1(bytes)?;
    verify_signed_device_certificate_v1(&certificate)?;
    Ok(certificate)
}

pub fn verify_signed_device_certificate_v1(
    certificate: &CanonicalSignedDeviceCertificateV1,
) -> Result<(), CanonicalAuthorityError> {
    verify_ed25519(
        &certificate.content.account_id,
        &device_certificate_signing_bytes_v1(&certificate.content)?,
        &certificate.signature,
        "signed-device-certificate-v1",
    )
}

pub fn encode_device_revocation_content_v1(
    content: &CanonicalDeviceRevocationContentV1,
) -> Vec<u8> {
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 4);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut encoded, &content.account_id);
    push_bytes(&mut encoded, &content.device_id);
    push_unsigned(&mut encoded, content.authority_sequence);
    encoded
}

pub fn device_revocation_signing_bytes_v1(content: &CanonicalDeviceRevocationContentV1) -> Vec<u8> {
    prefixed(
        CANONICAL_DEVICE_REVOCATION_SIGNATURE_DOMAIN,
        &encode_device_revocation_content_v1(content),
    )
}

pub fn encode_signed_device_revocation_v1(
    revocation: &CanonicalSignedDeviceRevocationV1,
) -> Result<Vec<u8>, CanonicalAuthorityError> {
    verify_signed_device_revocation_v1(revocation)?;
    Ok(encode_signed_device_revocation_unverified(revocation))
}

pub fn decode_signed_device_revocation_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedDeviceRevocationV1, CanonicalAuthorityError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AuthorityDecoder::new(bytes);
    let revocation = decode_signed_device_revocation_from(&mut decoder)?;
    decoder.finish()?;
    Ok(revocation)
}

pub fn decode_and_verify_signed_device_revocation_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedDeviceRevocationV1, CanonicalAuthorityError> {
    let revocation = decode_signed_device_revocation_v1(bytes)?;
    verify_signed_device_revocation_v1(&revocation)?;
    Ok(revocation)
}

pub fn verify_signed_device_revocation_v1(
    revocation: &CanonicalSignedDeviceRevocationV1,
) -> Result<(), CanonicalAuthorityError> {
    verify_ed25519(
        &revocation.content.account_id,
        &device_revocation_signing_bytes_v1(&revocation.content),
        &revocation.signature,
        "signed-device-revocation-v1",
    )
}

pub fn encode_account_authority_snapshot_content_v1(
    content: &CanonicalAccountAuthoritySnapshotContentV1,
) -> Result<Vec<u8>, CanonicalAuthorityError> {
    validate_snapshot_content(content)?;
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 4);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut encoded, &content.account_id);
    push_unsigned(&mut encoded, content.revision);
    push_array_header(&mut encoded, content.revocations.len() as u64);
    for revocation in &content.revocations {
        encoded.extend_from_slice(&encode_signed_device_revocation_v1(revocation)?);
    }
    Ok(encoded)
}

pub fn account_authority_snapshot_signing_bytes_v1(
    content: &CanonicalAccountAuthoritySnapshotContentV1,
) -> Result<Vec<u8>, CanonicalAuthorityError> {
    Ok(prefixed(
        CANONICAL_AUTHORITY_SNAPSHOT_SIGNATURE_DOMAIN,
        &encode_account_authority_snapshot_content_v1(content)?,
    ))
}

pub fn encode_signed_account_authority_snapshot_v1(
    snapshot: &CanonicalSignedAccountAuthoritySnapshotV1,
) -> Result<Vec<u8>, CanonicalAuthorityError> {
    verify_signed_account_authority_snapshot_v1(snapshot)?;
    encode_signed_account_authority_snapshot_unverified(snapshot)
}

pub fn decode_signed_account_authority_snapshot_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedAccountAuthoritySnapshotV1, CanonicalAuthorityError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AuthorityDecoder::new(bytes);
    let snapshot = decode_signed_account_authority_snapshot_from(&mut decoder)?;
    decoder.finish()?;
    Ok(snapshot)
}

pub fn decode_and_verify_signed_account_authority_snapshot_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedAccountAuthoritySnapshotV1, CanonicalAuthorityError> {
    let snapshot = decode_signed_account_authority_snapshot_v1(bytes)?;
    verify_signed_account_authority_snapshot_v1(&snapshot)?;
    Ok(snapshot)
}

pub fn verify_signed_account_authority_snapshot_v1(
    snapshot: &CanonicalSignedAccountAuthoritySnapshotV1,
) -> Result<(), CanonicalAuthorityError> {
    validate_snapshot_content(&snapshot.content)?;
    for revocation in &snapshot.content.revocations {
        verify_signed_device_revocation_v1(revocation)?;
    }
    verify_ed25519(
        &snapshot.content.account_id,
        &account_authority_snapshot_signing_bytes_v1(&snapshot.content)?,
        &snapshot.signature,
        "signed-account-authority-snapshot-v1",
    )
}

pub fn encode_device_session_authorization_content_v1(
    content: &CanonicalDeviceSessionAuthorizationContentV1,
) -> Result<Vec<u8>, CanonicalAuthorityError> {
    validate_session_content(content)?;
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 4);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut encoded, &content.session_binding);
    encoded.extend_from_slice(&encode_signed_device_certificate_v1(&content.certificate)?);
    encoded.extend_from_slice(&encode_signed_account_authority_snapshot_v1(
        &content.authority_snapshot,
    )?);
    Ok(encoded)
}

pub fn device_session_authorization_signing_bytes_v1(
    content: &CanonicalDeviceSessionAuthorizationContentV1,
) -> Result<Vec<u8>, CanonicalAuthorityError> {
    Ok(prefixed(
        CANONICAL_DEVICE_SESSION_AUTHORIZATION_SIGNATURE_DOMAIN,
        &encode_device_session_authorization_content_v1(content)?,
    ))
}

pub fn encode_signed_device_session_authorization_v1(
    authorization: &CanonicalSignedDeviceSessionAuthorizationV1,
) -> Result<Vec<u8>, CanonicalAuthorityError> {
    verify_signed_device_session_authorization_v1(authorization)?;
    let content = encode_device_session_authorization_content_v1(&authorization.content)?;
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 3);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    encoded.extend_from_slice(&content);
    push_bytes(&mut encoded, &authorization.signature);
    Ok(encoded)
}

pub fn decode_signed_device_session_authorization_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedDeviceSessionAuthorizationV1, CanonicalAuthorityError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AuthorityDecoder::new(bytes);
    decoder.expect_array("signed-device-session-authorization-v1", 3)?;
    decoder.expect_version("signed-device-session-authorization-v1.schema_version")?;
    let content = decode_device_session_authorization_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-device-session-authorization-v1.signature")?;
    decoder.finish()?;
    let authorization = CanonicalSignedDeviceSessionAuthorizationV1 { content, signature };
    validate_session_content(&authorization.content)?;
    Ok(authorization)
}

pub fn decode_and_verify_signed_device_session_authorization_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedDeviceSessionAuthorizationV1, CanonicalAuthorityError> {
    let authorization = decode_signed_device_session_authorization_v1(bytes)?;
    verify_signed_device_session_authorization_v1(&authorization)?;
    Ok(authorization)
}

pub fn verify_signed_device_session_authorization_v1(
    authorization: &CanonicalSignedDeviceSessionAuthorizationV1,
) -> Result<(), CanonicalAuthorityError> {
    validate_session_content(&authorization.content)?;
    verify_signed_device_certificate_v1(&authorization.content.certificate)?;
    verify_signed_account_authority_snapshot_v1(&authorization.content.authority_snapshot)?;
    verify_ed25519(
        &authorization.content.certificate.content.device_id,
        &device_session_authorization_signing_bytes_v1(&authorization.content)?,
        &authorization.signature,
        "signed-device-session-authorization-v1",
    )
}

fn encode_signed_device_certificate_unverified(
    certificate: &CanonicalSignedDeviceCertificateV1,
) -> Result<Vec<u8>, CanonicalAuthorityError> {
    let content = encode_device_certificate_content_v1(&certificate.content)?;
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 3);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    encoded.extend_from_slice(&content);
    push_bytes(&mut encoded, &certificate.signature);
    Ok(encoded)
}

fn encode_signed_device_revocation_unverified(
    revocation: &CanonicalSignedDeviceRevocationV1,
) -> Vec<u8> {
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 3);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    encoded.extend_from_slice(&encode_device_revocation_content_v1(&revocation.content));
    push_bytes(&mut encoded, &revocation.signature);
    encoded
}

fn encode_signed_account_authority_snapshot_unverified(
    snapshot: &CanonicalSignedAccountAuthoritySnapshotV1,
) -> Result<Vec<u8>, CanonicalAuthorityError> {
    let content = encode_account_authority_snapshot_content_v1(&snapshot.content)?;
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 3);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    encoded.extend_from_slice(&content);
    push_bytes(&mut encoded, &snapshot.signature);
    Ok(encoded)
}

fn validate_certificate_content(
    content: &CanonicalDeviceCertificateContentV1,
) -> Result<(), CanonicalAuthorityError> {
    if content.capabilities.is_empty() {
        return Err(CanonicalAuthorityError::EmptyCapabilities);
    }
    if content.capabilities.len() > MAX_CANONICAL_DEVICE_CAPABILITIES {
        return Err(CanonicalWireError::TooManyItems {
            field: "capabilities",
            actual: content.capabilities.len(),
            maximum: MAX_CANONICAL_DEVICE_CAPABILITIES,
        }
        .into());
    }
    if content
        .capabilities
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(CanonicalAuthorityError::NonCanonicalCapabilities);
    }
    Ok(())
}

fn validate_snapshot_content(
    content: &CanonicalAccountAuthoritySnapshotContentV1,
) -> Result<(), CanonicalAuthorityError> {
    if content.revocations.len() > MAX_CANONICAL_AUTHORITY_REVOCATIONS {
        return Err(CanonicalWireError::TooManyItems {
            field: "revocations",
            actual: content.revocations.len(),
            maximum: MAX_CANONICAL_AUTHORITY_REVOCATIONS,
        }
        .into());
    }
    for revocation in &content.revocations {
        if revocation.content.account_id != content.account_id {
            return Err(CanonicalAuthorityError::AccountMismatch {
                context: "authority snapshot revocation",
            });
        }
        if revocation.content.authority_sequence >= content.revision {
            return Err(CanonicalAuthorityError::RevocationOutsideSnapshot {
                revocation_sequence: revocation.content.authority_sequence,
                snapshot_revision: content.revision,
            });
        }
    }
    if content
        .revocations
        .windows(2)
        .any(|pair| pair[0].content.device_id >= pair[1].content.device_id)
    {
        return Err(CanonicalAuthorityError::NonCanonicalRevocations);
    }
    Ok(())
}

fn validate_session_content(
    content: &CanonicalDeviceSessionAuthorizationContentV1,
) -> Result<(), CanonicalAuthorityError> {
    validate_certificate_content(&content.certificate.content)?;
    validate_snapshot_content(&content.authority_snapshot.content)?;
    if content.certificate.content.account_id != content.authority_snapshot.content.account_id {
        return Err(CanonicalAuthorityError::AccountMismatch {
            context: "session certificate and authority snapshot",
        });
    }
    if content.certificate.content.authority_sequence >= content.authority_snapshot.content.revision
    {
        return Err(CanonicalAuthorityError::CertificateOutsideSnapshot {
            certificate_sequence: content.certificate.content.authority_sequence,
            snapshot_revision: content.authority_snapshot.content.revision,
        });
    }
    if content
        .authority_snapshot
        .content
        .revocations
        .iter()
        .any(|revocation| revocation.content.device_id == content.certificate.content.device_id)
    {
        return Err(CanonicalAuthorityError::SessionDeviceRevoked);
    }
    Ok(())
}

fn verify_ed25519(
    verifying_key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
    object: &'static str,
) -> Result<(), CanonicalAuthorityError> {
    let verifying_key = VerifyingKey::from_bytes(verifying_key)
        .map_err(|_| CanonicalAuthorityError::InvalidPublicKey { object })?;
    let signature = Signature::try_from(signature.as_slice())
        .map_err(|_| CanonicalAuthorityError::InvalidSignature { object })?;
    verifying_key
        .verify_strict(message, &signature)
        .map_err(|_| CanonicalAuthorityError::InvalidSignature { object })
}

fn decode_signed_device_certificate_from(
    decoder: &mut AuthorityDecoder<'_>,
) -> Result<CanonicalSignedDeviceCertificateV1, CanonicalAuthorityError> {
    decoder.expect_array("signed-device-certificate-v1", 3)?;
    decoder.expect_version("signed-device-certificate-v1.schema_version")?;
    let content = decode_device_certificate_content_from(decoder)?;
    let signature = decoder.fixed_bytes("signed-device-certificate-v1.signature")?;
    let certificate = CanonicalSignedDeviceCertificateV1 { content, signature };
    validate_certificate_content(&certificate.content)?;
    Ok(certificate)
}

fn decode_device_certificate_content_from(
    decoder: &mut AuthorityDecoder<'_>,
) -> Result<CanonicalDeviceCertificateContentV1, CanonicalAuthorityError> {
    decoder.expect_array("device-certificate-content-v1", 6)?;
    decoder.expect_version("device-certificate-content-v1.schema_version")?;
    let account_id = decoder.fixed_bytes("device-certificate-content-v1.account_id")?;
    let device_id = decoder.fixed_bytes("device-certificate-content-v1.device_id")?;
    let encryption_public_key =
        decoder.fixed_bytes("device-certificate-content-v1.encryption_public_key")?;
    let authority_sequence =
        decoder.unsigned("device-certificate-content-v1.authority_sequence")?;
    let capability_count = decoder.array("device-certificate-content-v1.capabilities")?;
    if capability_count > MAX_CANONICAL_DEVICE_CAPABILITIES {
        return Err(CanonicalWireError::TooManyItems {
            field: "capabilities",
            actual: capability_count,
            maximum: MAX_CANONICAL_DEVICE_CAPABILITIES,
        }
        .into());
    }
    let mut capabilities = Vec::with_capacity(capability_count);
    for _ in 0..capability_count {
        capabilities.push(CanonicalDeviceCapabilityV1::from_code(
            decoder.unsigned("device-certificate-content-v1.capability")?,
        )?);
    }
    let content = CanonicalDeviceCertificateContentV1 {
        account_id,
        device_id,
        encryption_public_key,
        authority_sequence,
        capabilities,
    };
    validate_certificate_content(&content)?;
    Ok(content)
}

fn decode_signed_device_revocation_from(
    decoder: &mut AuthorityDecoder<'_>,
) -> Result<CanonicalSignedDeviceRevocationV1, CanonicalAuthorityError> {
    decoder.expect_array("signed-device-revocation-v1", 3)?;
    decoder.expect_version("signed-device-revocation-v1.schema_version")?;
    let content = decode_device_revocation_content_from(decoder)?;
    let signature = decoder.fixed_bytes("signed-device-revocation-v1.signature")?;
    Ok(CanonicalSignedDeviceRevocationV1 { content, signature })
}

fn decode_device_revocation_content_from(
    decoder: &mut AuthorityDecoder<'_>,
) -> Result<CanonicalDeviceRevocationContentV1, CanonicalAuthorityError> {
    decoder.expect_array("device-revocation-content-v1", 4)?;
    decoder.expect_version("device-revocation-content-v1.schema_version")?;
    Ok(CanonicalDeviceRevocationContentV1 {
        account_id: decoder.fixed_bytes("device-revocation-content-v1.account_id")?,
        device_id: decoder.fixed_bytes("device-revocation-content-v1.device_id")?,
        authority_sequence: decoder.unsigned("device-revocation-content-v1.authority_sequence")?,
    })
}

fn decode_signed_account_authority_snapshot_from(
    decoder: &mut AuthorityDecoder<'_>,
) -> Result<CanonicalSignedAccountAuthoritySnapshotV1, CanonicalAuthorityError> {
    decoder.expect_array("signed-account-authority-snapshot-v1", 3)?;
    decoder.expect_version("signed-account-authority-snapshot-v1.schema_version")?;
    let content = decode_account_authority_snapshot_content_from(decoder)?;
    let signature = decoder.fixed_bytes("signed-account-authority-snapshot-v1.signature")?;
    let snapshot = CanonicalSignedAccountAuthoritySnapshotV1 { content, signature };
    validate_snapshot_content(&snapshot.content)?;
    Ok(snapshot)
}

fn decode_account_authority_snapshot_content_from(
    decoder: &mut AuthorityDecoder<'_>,
) -> Result<CanonicalAccountAuthoritySnapshotContentV1, CanonicalAuthorityError> {
    decoder.expect_array("account-authority-snapshot-content-v1", 4)?;
    decoder.expect_version("account-authority-snapshot-content-v1.schema_version")?;
    let account_id = decoder.fixed_bytes("account-authority-snapshot-content-v1.account_id")?;
    let revision = decoder.unsigned("account-authority-snapshot-content-v1.revision")?;
    let revocation_count = decoder.array("account-authority-snapshot-content-v1.revocations")?;
    if revocation_count > MAX_CANONICAL_AUTHORITY_REVOCATIONS {
        return Err(CanonicalWireError::TooManyItems {
            field: "revocations",
            actual: revocation_count,
            maximum: MAX_CANONICAL_AUTHORITY_REVOCATIONS,
        }
        .into());
    }
    let mut revocations = Vec::with_capacity(revocation_count);
    for _ in 0..revocation_count {
        revocations.push(decode_signed_device_revocation_from(decoder)?);
    }
    let content = CanonicalAccountAuthoritySnapshotContentV1 {
        account_id,
        revision,
        revocations,
    };
    validate_snapshot_content(&content)?;
    Ok(content)
}

fn decode_device_session_authorization_content_from(
    decoder: &mut AuthorityDecoder<'_>,
) -> Result<CanonicalDeviceSessionAuthorizationContentV1, CanonicalAuthorityError> {
    decoder.expect_array("device-session-authorization-content-v1", 4)?;
    decoder.expect_version("device-session-authorization-content-v1.schema_version")?;
    let session_binding =
        decoder.fixed_bytes("device-session-authorization-content-v1.session_binding")?;
    let certificate = decode_signed_device_certificate_from(decoder)?;
    let authority_snapshot = decode_signed_account_authority_snapshot_from(decoder)?;
    let content = CanonicalDeviceSessionAuthorizationContentV1 {
        session_binding,
        certificate,
        authority_snapshot,
    };
    validate_session_content(&content)?;
    Ok(content)
}

struct AuthorityDecoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> AuthorityDecoder<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn finish(self) -> Result<(), CanonicalAuthorityError> {
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
    ) -> Result<(), CanonicalAuthorityError> {
        let actual = self.array(object)?;
        if actual != expected {
            return Err(CanonicalAuthorityError::WrongArrayLength {
                object,
                expected,
                actual,
            });
        }
        Ok(())
    }

    fn expect_version(&mut self, field: &'static str) -> Result<(), CanonicalAuthorityError> {
        let actual = self.unsigned(field)?;
        if actual != CANONICAL_PROFILE_VERSION {
            return Err(CanonicalAuthorityError::WrongUnsignedValue {
                field,
                expected: CANONICAL_PROFILE_VERSION,
                actual,
            });
        }
        Ok(())
    }

    fn array(&mut self, field: &'static str) -> Result<usize, CanonicalAuthorityError> {
        let length = self.head(4, field)?;
        usize::try_from(length).map_err(|_| CanonicalWireError::LengthOverflow.into())
    }

    fn unsigned(&mut self, field: &'static str) -> Result<u64, CanonicalAuthorityError> {
        self.head(0, field)
    }

    fn fixed_bytes<const N: usize>(
        &mut self,
        field: &'static str,
    ) -> Result<[u8; N], CanonicalAuthorityError> {
        let length = self.head(2, field)?;
        let length = usize::try_from(length).map_err(|_| CanonicalWireError::LengthOverflow)?;
        if length != N {
            return Err(CanonicalAuthorityError::WrongByteStringLength {
                field,
                expected: N,
                actual: length,
            });
        }
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

    fn head(
        &mut self,
        expected_major: u8,
        field: &'static str,
    ) -> Result<u64, CanonicalAuthorityError> {
        let initial = self.byte()?;
        let actual_major = initial >> 5;
        if actual_major != expected_major {
            return Err(CanonicalAuthorityError::WrongMajorType {
                field,
                expected: expected_major,
                actual: actual_major,
            });
        }
        self.argument(initial & 0x1f)
    }

    fn argument(&mut self, additional: u8) -> Result<u64, CanonicalAuthorityError> {
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

    fn byte(&mut self) -> Result<u8, CanonicalAuthorityError> {
        let value = self
            .bytes
            .get(self.cursor)
            .copied()
            .ok_or(CanonicalWireError::UnexpectedEnd)?;
        self.cursor += 1;
        Ok(value)
    }

    fn bytes_array<const N: usize>(&mut self) -> Result<[u8; N], CanonicalAuthorityError> {
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
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct AuthorityVectors {
        schema_version: u64,
        profile: String,
        activation_status: String,
        input: AuthorityVectorInput,
        expected: AuthorityVectorExpected,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct AuthorityVectorInput {
        root_secret_key_hex: String,
        device_secret_key_hex: String,
        revoked_device_secret_key_hex: String,
        encryption_public_key_hex: String,
        certificate_authority_sequence: u64,
        capabilities: Vec<u64>,
        revocation_authority_sequence: u64,
        authority_revision: u64,
        session_binding_hex: String,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct AuthorityVectorExpected {
        account_id_hex: String,
        device_id_hex: String,
        revoked_device_id_hex: String,
        certificate: SignedObjectExpected,
        revocation: SignedObjectExpected,
        authority_snapshot: SignedObjectExpected,
        session_authorization: SignedObjectExpected,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SignedObjectExpected {
        content_cbor_hex: String,
        signature_domain_hex: String,
        signing_preimage_hex: String,
        signature_hex: String,
        signed_cbor_hex: String,
    }

    fn sequential<const N: usize>(start: u8) -> [u8; N] {
        std::array::from_fn(|index| start.wrapping_add(index as u8))
    }

    fn to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
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
            return Err(invalid_data(
                "authority golden vector contains non-canonical hexadecimal",
            ));
        }
        let mut decoded = Vec::with_capacity(value.len() / 2);
        for offset in (0..value.len()).step_by(2) {
            decoded.push(u8::from_str_radix(&value[offset..offset + 2], 16)?);
        }
        Ok(decoded)
    }

    fn decode_fixed<const N: usize>(value: &str) -> Result<[u8; N], Box<dyn std::error::Error>> {
        let bytes = decode_hex(value)?;
        bytes.try_into().map_err(|value: Vec<u8>| {
            invalid_data(format!(
                "authority golden vector contains {} bytes; expected {N}",
                value.len()
            ))
        })
    }

    fn load_vectors() -> Result<AuthorityVectors, Box<dyn std::error::Error>> {
        let vectors: AuthorityVectors = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-AUTHORITY-VECTORS.json"
        )))?;
        if vectors.schema_version != CANONICAL_PROFILE_VERSION
            || vectors.profile != "kilogram-deterministic-cbor-v1"
            || vectors.activation_status != "not-active"
        {
            return Err(invalid_data(
                "authority golden vector header does not match profile v1",
            ));
        }
        Ok(vectors)
    }

    fn signed_fixture() -> CanonicalSignedDeviceSessionAuthorizationV1 {
        let root = SigningKey::from_bytes(&sequential(0x20));
        let device = SigningKey::from_bytes(&sequential(0x00));
        let revoked_device = SigningKey::from_bytes(&sequential(0x40));
        let account_id = root.verifying_key().to_bytes();

        let certificate_content = CanonicalDeviceCertificateContentV1 {
            account_id,
            device_id: device.verifying_key().to_bytes(),
            encryption_public_key: sequential(0xa0),
            authority_sequence: 7,
            capabilities: vec![
                CanonicalDeviceCapabilityV1::SignEvents,
                CanonicalDeviceCapabilityV1::SyncHistory,
            ],
        };
        let certificate = CanonicalSignedDeviceCertificateV1 {
            signature: root
                .sign(&device_certificate_signing_bytes_v1(&certificate_content).unwrap())
                .to_bytes(),
            content: certificate_content,
        };

        let revocation_content = CanonicalDeviceRevocationContentV1 {
            account_id,
            device_id: revoked_device.verifying_key().to_bytes(),
            authority_sequence: 3,
        };
        let revocation = CanonicalSignedDeviceRevocationV1 {
            signature: root
                .sign(&device_revocation_signing_bytes_v1(&revocation_content))
                .to_bytes(),
            content: revocation_content,
        };
        let snapshot_content = CanonicalAccountAuthoritySnapshotContentV1 {
            account_id,
            revision: 8,
            revocations: vec![revocation],
        };
        let authority_snapshot = CanonicalSignedAccountAuthoritySnapshotV1 {
            signature: root
                .sign(&account_authority_snapshot_signing_bytes_v1(&snapshot_content).unwrap())
                .to_bytes(),
            content: snapshot_content,
        };
        let content = CanonicalDeviceSessionAuthorizationContentV1 {
            session_binding: sequential(0xc0),
            certificate,
            authority_snapshot,
        };
        CanonicalSignedDeviceSessionAuthorizationV1 {
            signature: device
                .sign(&device_session_authorization_signing_bytes_v1(&content).unwrap())
                .to_bytes(),
            content,
        }
    }

    #[test]
    fn authority_session_round_trip_and_signatures_are_strict() {
        let fixture = signed_fixture();
        let encoded = encode_signed_device_session_authorization_v1(&fixture).unwrap();
        assert_eq!(
            decode_and_verify_signed_device_session_authorization_v1(&encoded).unwrap(),
            fixture
        );

        let mut tampered = encoded;
        *tampered.last_mut().unwrap() ^= 1;
        assert!(matches!(
            decode_and_verify_signed_device_session_authorization_v1(&tampered),
            Err(CanonicalAuthorityError::InvalidSignature {
                object: "signed-device-session-authorization-v1"
            })
        ));
    }

    #[test]
    fn decoder_rejects_non_minimal_version_and_trailing_data() {
        let encoded = encode_signed_device_session_authorization_v1(&signed_fixture()).unwrap();
        let mut non_minimal = vec![encoded[0], 0x18, 0x01];
        non_minimal.extend_from_slice(&encoded[2..]);
        assert!(matches!(
            decode_signed_device_session_authorization_v1(&non_minimal),
            Err(CanonicalAuthorityError::Wire(
                CanonicalWireError::NonMinimalArgument
            ))
        ));

        let mut trailing = encoded;
        trailing.push(0);
        assert!(matches!(
            decode_signed_device_session_authorization_v1(&trailing),
            Err(CanonicalAuthorityError::Wire(
                CanonicalWireError::TrailingData
            ))
        ));
    }

    #[test]
    fn structural_rules_reject_ambiguous_or_incoherent_authority() {
        let mut fixture = signed_fixture();
        fixture.content.certificate.content.capabilities.reverse();
        assert!(matches!(
            device_session_authorization_signing_bytes_v1(&fixture.content),
            Err(CanonicalAuthorityError::NonCanonicalCapabilities)
        ));

        let mut fixture = signed_fixture();
        fixture.content.certificate.content.account_id[0] ^= 1;
        assert!(matches!(
            device_session_authorization_signing_bytes_v1(&fixture.content),
            Err(CanonicalAuthorityError::AccountMismatch { .. })
        ));

        let mut fixture = signed_fixture();
        fixture.content.certificate.content.device_id =
            fixture.content.authority_snapshot.content.revocations[0]
                .content
                .device_id;
        assert!(matches!(
            device_session_authorization_signing_bytes_v1(&fixture.content),
            Err(CanonicalAuthorityError::SessionDeviceRevoked)
        ));
    }

    #[test]
    fn golden_authority_vector_matches_bytes_and_signatures()
    -> Result<(), Box<dyn std::error::Error>> {
        let vector = load_vectors()?;
        let root = SigningKey::from_bytes(&decode_fixed(&vector.input.root_secret_key_hex)?);
        let device = SigningKey::from_bytes(&decode_fixed(&vector.input.device_secret_key_hex)?);
        let revoked =
            SigningKey::from_bytes(&decode_fixed(&vector.input.revoked_device_secret_key_hex)?);
        let account_id = root.verifying_key().to_bytes();
        let device_id = device.verifying_key().to_bytes();
        let revoked_device_id = revoked.verifying_key().to_bytes();
        assert_eq!(to_hex(&account_id), vector.expected.account_id_hex);
        assert_eq!(to_hex(&device_id), vector.expected.device_id_hex);
        assert_eq!(
            to_hex(&revoked_device_id),
            vector.expected.revoked_device_id_hex
        );

        let capabilities = vector
            .input
            .capabilities
            .iter()
            .copied()
            .map(CanonicalDeviceCapabilityV1::from_code)
            .collect::<Result<Vec<_>, _>>()?;
        let certificate_content = CanonicalDeviceCertificateContentV1 {
            account_id,
            device_id,
            encryption_public_key: decode_fixed(&vector.input.encryption_public_key_hex)?,
            authority_sequence: vector.input.certificate_authority_sequence,
            capabilities,
        };
        let certificate_content_bytes = encode_device_certificate_content_v1(&certificate_content)?;
        let certificate_preimage = device_certificate_signing_bytes_v1(&certificate_content)?;
        let certificate = CanonicalSignedDeviceCertificateV1 {
            signature: root.sign(&certificate_preimage).to_bytes(),
            content: certificate_content,
        };
        let certificate_bytes = encode_signed_device_certificate_v1(&certificate)?;
        assert_signed_object(
            &vector.expected.certificate,
            CANONICAL_DEVICE_CERTIFICATE_SIGNATURE_DOMAIN,
            &certificate_content_bytes,
            &certificate_preimage,
            &certificate.signature,
            &certificate_bytes,
        );
        assert_eq!(
            decode_and_verify_signed_device_certificate_v1(&certificate_bytes)?,
            certificate
        );

        let revocation_content = CanonicalDeviceRevocationContentV1 {
            account_id,
            device_id: revoked_device_id,
            authority_sequence: vector.input.revocation_authority_sequence,
        };
        let revocation_content_bytes = encode_device_revocation_content_v1(&revocation_content);
        let revocation_preimage = device_revocation_signing_bytes_v1(&revocation_content);
        let revocation = CanonicalSignedDeviceRevocationV1 {
            signature: root.sign(&revocation_preimage).to_bytes(),
            content: revocation_content,
        };
        let revocation_bytes = encode_signed_device_revocation_v1(&revocation)?;
        assert_signed_object(
            &vector.expected.revocation,
            CANONICAL_DEVICE_REVOCATION_SIGNATURE_DOMAIN,
            &revocation_content_bytes,
            &revocation_preimage,
            &revocation.signature,
            &revocation_bytes,
        );
        assert_eq!(
            decode_and_verify_signed_device_revocation_v1(&revocation_bytes)?,
            revocation
        );

        let snapshot_content = CanonicalAccountAuthoritySnapshotContentV1 {
            account_id,
            revision: vector.input.authority_revision,
            revocations: vec![revocation],
        };
        let snapshot_content_bytes =
            encode_account_authority_snapshot_content_v1(&snapshot_content)?;
        let snapshot_preimage = account_authority_snapshot_signing_bytes_v1(&snapshot_content)?;
        let snapshot = CanonicalSignedAccountAuthoritySnapshotV1 {
            signature: root.sign(&snapshot_preimage).to_bytes(),
            content: snapshot_content,
        };
        let snapshot_bytes = encode_signed_account_authority_snapshot_v1(&snapshot)?;
        assert_signed_object(
            &vector.expected.authority_snapshot,
            CANONICAL_AUTHORITY_SNAPSHOT_SIGNATURE_DOMAIN,
            &snapshot_content_bytes,
            &snapshot_preimage,
            &snapshot.signature,
            &snapshot_bytes,
        );
        assert_eq!(
            decode_and_verify_signed_account_authority_snapshot_v1(&snapshot_bytes)?,
            snapshot
        );

        let session_content = CanonicalDeviceSessionAuthorizationContentV1 {
            session_binding: decode_fixed(&vector.input.session_binding_hex)?,
            certificate,
            authority_snapshot: snapshot,
        };
        let session_content_bytes =
            encode_device_session_authorization_content_v1(&session_content)?;
        let session_preimage = device_session_authorization_signing_bytes_v1(&session_content)?;
        let authorization = CanonicalSignedDeviceSessionAuthorizationV1 {
            signature: device.sign(&session_preimage).to_bytes(),
            content: session_content,
        };
        let authorization_bytes = encode_signed_device_session_authorization_v1(&authorization)?;
        assert_signed_object(
            &vector.expected.session_authorization,
            CANONICAL_DEVICE_SESSION_AUTHORIZATION_SIGNATURE_DOMAIN,
            &session_content_bytes,
            &session_preimage,
            &authorization.signature,
            &authorization_bytes,
        );
        assert_eq!(
            decode_and_verify_signed_device_session_authorization_v1(&authorization_bytes)?,
            authorization
        );
        Ok(())
    }

    fn assert_signed_object(
        expected: &SignedObjectExpected,
        domain: &[u8],
        content: &[u8],
        preimage: &[u8],
        signature: &[u8],
        signed: &[u8],
    ) {
        assert_eq!(to_hex(content), expected.content_cbor_hex);
        assert_eq!(to_hex(domain), expected.signature_domain_hex);
        assert_eq!(to_hex(preimage), expected.signing_preimage_hex);
        assert_eq!(to_hex(signature), expected.signature_hex);
        assert_eq!(to_hex(signed), expected.signed_cbor_hex);
    }
}
