//! Inactive deterministic-CBOR ratchet bootstrap candidate for M1.1.3.
//!
//! Only public asynchronous bootstrap artifacts are represented here. Secret
//! Double Ratchet state and active Postcard codecs remain unchanged.

use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

use crate::canonical_authority_v1::{
    CanonicalAuthorityError, CanonicalSignedAccountAuthoritySnapshotV1,
    CanonicalSignedDeviceCertificateV1, decode_and_verify_signed_account_authority_snapshot_v1,
    decode_and_verify_signed_device_certificate_v1, encode_signed_account_authority_snapshot_v1,
    encode_signed_device_certificate_v1,
};
use crate::canonical_v1::{
    CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
    push_unsigned, validate_canonical_profile_v1,
};

pub const MAX_CANONICAL_PREKEY_POOL_SIZE: usize = 64;
pub const MAX_CANONICAL_PREKEY_DIRECTORY_DEVICES: usize = 32;
pub const MAX_CANONICAL_PREKEY_VALIDITY_SECONDS: u64 = 30 * 24 * 60 * 60;
pub const MAX_CANONICAL_NESTED_RATCHET_BYTES: usize = 512 * 1024;

pub const CANONICAL_RATCHET_IDENTITY_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:ratchet-identity-signature:cbor-v1\0";
pub const CANONICAL_PREKEY_BUNDLE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:prekey-bundle-signature:cbor-v1\0";
pub const CANONICAL_PREKEY_POOL_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:prekey-pool-signature:cbor-v1\0";
pub const CANONICAL_PREKEY_POOL_ID_DOMAIN: &[u8] = b"kilogram:prekey-pool-id:cbor-v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRatchetIdentityContentV1 {
    pub device_id: [u8; 32],
    pub curve25519_key: [u8; 32],
    pub ed25519_key: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRatchetIdentityV1 {
    pub content: CanonicalRatchetIdentityContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalPrekeyBundleContentV1 {
    pub ratchet_identity: CanonicalSignedRatchetIdentityV1,
    pub sequence: u64,
    pub one_time_key: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedPrekeyBundleV1 {
    pub content: CanonicalPrekeyBundleContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalPrekeyPoolContentV1 {
    pub ratchet_identity: CanonicalSignedRatchetIdentityV1,
    pub generation: u64,
    pub published_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub bundles: Vec<CanonicalSignedPrekeyBundleV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedPrekeyPoolV1 {
    pub content: CanonicalPrekeyPoolContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalAccountPrekeyDirectoryV1 {
    pub authority_snapshot: CanonicalSignedAccountAuthoritySnapshotV1,
    pub certificates: Vec<CanonicalSignedDeviceCertificateV1>,
    pub pools: Vec<CanonicalSignedPrekeyPoolV1>,
}

#[derive(Debug, Error)]
pub enum CanonicalRatchetError {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),

    #[error(transparent)]
    Authority(#[from] CanonicalAuthorityError),

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

    #[error("{field} contains {actual} bytes; maximum is {maximum}")]
    NestedEncodingTooLarge {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },

    #[error("canonical prekey pool must contain at least one bundle")]
    EmptyPrekeyPool,

    #[error("canonical prekey pool contains {actual} bundles; maximum is {maximum}")]
    TooManyPrekeys { actual: usize, maximum: usize },

    #[error("canonical prekey pool validity interval is invalid")]
    InvalidValidityInterval,

    #[error("canonical prekey pool validity is {actual} seconds; maximum is {maximum}")]
    ValidityTooLong { actual: u64, maximum: u64 },

    #[error("canonical prekey sequences must be contiguous")]
    NonContiguousPrekeySequence,

    #[error("canonical prekey pool contains a duplicate one-time key")]
    DuplicateOneTimeKey,

    #[error("{context} names a different ratchet identity")]
    RatchetIdentityMismatch { context: &'static str },

    #[error("canonical prekey directory must contain at least one device")]
    EmptyDirectory,

    #[error("canonical prekey directory contains {actual} devices; maximum is {maximum}")]
    TooManyDirectoryDevices { actual: usize, maximum: usize },

    #[error("canonical prekey directory certificates must be strictly ordered by Device ID")]
    NonCanonicalCertificateOrder,

    #[error("canonical prekey directory pools must be strictly ordered by Device ID")]
    NonCanonicalPoolOrder,

    #[error("canonical prekey directory certificate/pool coverage differs")]
    DirectoryCoverageMismatch,

    #[error("{context} names a different Account ID")]
    AccountMismatch { context: &'static str },

    #[error("certificate authority sequence is not before the directory authority revision")]
    CertificateOutsideAuthoritySnapshot,

    #[error("canonical prekey directory contains a revoked Device")]
    RevokedDirectoryDevice,

    #[error("{object} contains an invalid Ed25519 verifying key")]
    InvalidPublicKey { object: &'static str },

    #[error("{object} contains an invalid Ed25519 signature")]
    InvalidSignature { object: &'static str },
}

pub fn encode_ratchet_identity_content_v1(content: &CanonicalRatchetIdentityContentV1) -> Vec<u8> {
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 4);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut encoded, &content.device_id);
    push_bytes(&mut encoded, &content.curve25519_key);
    push_bytes(&mut encoded, &content.ed25519_key);
    encoded
}

pub fn ratchet_identity_signing_bytes_v1(content: &CanonicalRatchetIdentityContentV1) -> Vec<u8> {
    prefixed(
        CANONICAL_RATCHET_IDENTITY_SIGNATURE_DOMAIN,
        &encode_ratchet_identity_content_v1(content),
    )
}

pub fn encode_signed_ratchet_identity_v1(
    identity: &CanonicalSignedRatchetIdentityV1,
) -> Result<Vec<u8>, CanonicalRatchetError> {
    verify_signed_ratchet_identity_v1(identity)?;
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 3);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    encoded.extend_from_slice(&encode_ratchet_identity_content_v1(&identity.content));
    push_bytes(&mut encoded, &identity.signature);
    Ok(encoded)
}

pub fn decode_and_verify_signed_ratchet_identity_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRatchetIdentityV1, CanonicalRatchetError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = RatchetDecoder::new(bytes);
    let identity = decode_signed_ratchet_identity_from(&mut decoder)?;
    decoder.finish()?;
    verify_signed_ratchet_identity_v1(&identity)?;
    Ok(identity)
}

pub fn verify_signed_ratchet_identity_v1(
    identity: &CanonicalSignedRatchetIdentityV1,
) -> Result<(), CanonicalRatchetError> {
    verify_ed25519(
        &identity.content.device_id,
        &ratchet_identity_signing_bytes_v1(&identity.content),
        &identity.signature,
        "signed-ratchet-identity-v1",
    )
}

pub fn encode_prekey_bundle_content_v1(
    content: &CanonicalPrekeyBundleContentV1,
) -> Result<Vec<u8>, CanonicalRatchetError> {
    verify_signed_ratchet_identity_v1(&content.ratchet_identity)?;
    let identity = encode_signed_ratchet_identity_v1(&content.ratchet_identity)?;
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 4);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut encoded, &identity);
    push_unsigned(&mut encoded, content.sequence);
    push_bytes(&mut encoded, &content.one_time_key);
    Ok(encoded)
}

pub fn prekey_bundle_signing_bytes_v1(
    content: &CanonicalPrekeyBundleContentV1,
) -> Result<Vec<u8>, CanonicalRatchetError> {
    Ok(prefixed(
        CANONICAL_PREKEY_BUNDLE_SIGNATURE_DOMAIN,
        &encode_prekey_bundle_content_v1(content)?,
    ))
}

pub fn encode_signed_prekey_bundle_v1(
    bundle: &CanonicalSignedPrekeyBundleV1,
) -> Result<Vec<u8>, CanonicalRatchetError> {
    verify_signed_prekey_bundle_v1(bundle)?;
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 3);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    encoded.extend_from_slice(&encode_prekey_bundle_content_v1(&bundle.content)?);
    push_bytes(&mut encoded, &bundle.signature);
    Ok(encoded)
}

pub fn decode_and_verify_signed_prekey_bundle_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedPrekeyBundleV1, CanonicalRatchetError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = RatchetDecoder::new(bytes);
    let bundle = decode_signed_prekey_bundle_from(&mut decoder)?;
    decoder.finish()?;
    verify_signed_prekey_bundle_v1(&bundle)?;
    Ok(bundle)
}

pub fn verify_signed_prekey_bundle_v1(
    bundle: &CanonicalSignedPrekeyBundleV1,
) -> Result<(), CanonicalRatchetError> {
    verify_signed_ratchet_identity_v1(&bundle.content.ratchet_identity)?;
    verify_ed25519(
        &bundle.content.ratchet_identity.content.device_id,
        &prekey_bundle_signing_bytes_v1(&bundle.content)?,
        &bundle.signature,
        "signed-prekey-bundle-v1",
    )
}

pub fn encode_prekey_pool_content_v1(
    content: &CanonicalPrekeyPoolContentV1,
) -> Result<Vec<u8>, CanonicalRatchetError> {
    validate_pool_content(content)?;
    let identity = encode_signed_ratchet_identity_v1(&content.ratchet_identity)?;
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 6);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut encoded, &identity);
    push_unsigned(&mut encoded, content.generation);
    push_unsigned(&mut encoded, content.published_at_unix_seconds);
    push_unsigned(&mut encoded, content.expires_at_unix_seconds);
    push_array_header(&mut encoded, content.bundles.len() as u64);
    for bundle in &content.bundles {
        push_bytes(&mut encoded, &encode_signed_prekey_bundle_v1(bundle)?);
    }
    Ok(encoded)
}

pub fn prekey_pool_signing_bytes_v1(
    content: &CanonicalPrekeyPoolContentV1,
) -> Result<Vec<u8>, CanonicalRatchetError> {
    Ok(prefixed(
        CANONICAL_PREKEY_POOL_SIGNATURE_DOMAIN,
        &encode_prekey_pool_content_v1(content)?,
    ))
}

pub fn encode_signed_prekey_pool_v1(
    pool: &CanonicalSignedPrekeyPoolV1,
) -> Result<Vec<u8>, CanonicalRatchetError> {
    verify_signed_prekey_pool_v1(pool)?;
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 3);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    encoded.extend_from_slice(&encode_prekey_pool_content_v1(&pool.content)?);
    push_bytes(&mut encoded, &pool.signature);
    Ok(encoded)
}

pub fn decode_and_verify_signed_prekey_pool_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedPrekeyPoolV1, CanonicalRatchetError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = RatchetDecoder::new(bytes);
    let pool = decode_signed_prekey_pool_from(&mut decoder)?;
    decoder.finish()?;
    verify_signed_prekey_pool_v1(&pool)?;
    Ok(pool)
}

pub fn verify_signed_prekey_pool_v1(
    pool: &CanonicalSignedPrekeyPoolV1,
) -> Result<(), CanonicalRatchetError> {
    validate_pool_content(&pool.content)?;
    verify_ed25519(
        &pool.content.ratchet_identity.content.device_id,
        &prekey_pool_signing_bytes_v1(&pool.content)?,
        &pool.signature,
        "signed-prekey-pool-v1",
    )
}

pub fn prekey_pool_id_v1(
    pool: &CanonicalSignedPrekeyPoolV1,
) -> Result<[u8; 32], CanonicalRatchetError> {
    let encoded = encode_signed_prekey_pool_v1(pool)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(CANONICAL_PREKEY_POOL_ID_DOMAIN);
    hasher.update(&encoded);
    Ok(*hasher.finalize().as_bytes())
}

pub fn encode_account_prekey_directory_v1(
    directory: &CanonicalAccountPrekeyDirectoryV1,
) -> Result<Vec<u8>, CanonicalRatchetError> {
    verify_account_prekey_directory_v1(directory)?;
    let snapshot = encode_signed_account_authority_snapshot_v1(&directory.authority_snapshot)?;
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 4);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut encoded, &snapshot);
    push_array_header(&mut encoded, directory.certificates.len() as u64);
    for certificate in &directory.certificates {
        push_bytes(
            &mut encoded,
            &encode_signed_device_certificate_v1(certificate)?,
        );
    }
    push_array_header(&mut encoded, directory.pools.len() as u64);
    for pool in &directory.pools {
        push_bytes(&mut encoded, &encode_signed_prekey_pool_v1(pool)?);
    }
    Ok(encoded)
}

pub fn decode_and_verify_account_prekey_directory_v1(
    bytes: &[u8],
) -> Result<CanonicalAccountPrekeyDirectoryV1, CanonicalRatchetError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = RatchetDecoder::new(bytes);
    decoder.expect_array("account-prekey-directory-v1", 4)?;
    decoder.expect_version("account-prekey-directory-v1.schema_version")?;
    let snapshot_bytes = decoder.bytes(
        "account-prekey-directory-v1.authority_snapshot",
        MAX_CANONICAL_NESTED_RATCHET_BYTES,
    )?;
    let authority_snapshot =
        decode_and_verify_signed_account_authority_snapshot_v1(snapshot_bytes)?;
    let certificate_count = decoder.array("account-prekey-directory-v1.certificates")?;
    validate_directory_count(certificate_count)?;
    let mut certificates = Vec::with_capacity(certificate_count);
    for _ in 0..certificate_count {
        certificates.push(decode_and_verify_signed_device_certificate_v1(
            decoder.bytes(
                "account-prekey-directory-v1.certificate",
                MAX_CANONICAL_NESTED_RATCHET_BYTES,
            )?,
        )?);
    }
    let pool_count = decoder.array("account-prekey-directory-v1.pools")?;
    validate_directory_count(pool_count)?;
    let mut pools = Vec::with_capacity(pool_count);
    for _ in 0..pool_count {
        pools.push(decode_and_verify_signed_prekey_pool_v1(decoder.bytes(
            "account-prekey-directory-v1.pool",
            MAX_CANONICAL_NESTED_RATCHET_BYTES,
        )?)?)
    }
    decoder.finish()?;
    let directory = CanonicalAccountPrekeyDirectoryV1 {
        authority_snapshot,
        certificates,
        pools,
    };
    verify_account_prekey_directory_v1(&directory)?;
    Ok(directory)
}

pub fn verify_account_prekey_directory_v1(
    directory: &CanonicalAccountPrekeyDirectoryV1,
) -> Result<(), CanonicalRatchetError> {
    let count = directory.certificates.len();
    validate_directory_count(count)?;
    if directory.pools.len() != count {
        return Err(CanonicalRatchetError::DirectoryCoverageMismatch);
    }
    // Re-encoding performs all nested signature checks.
    let snapshot_bytes =
        encode_signed_account_authority_snapshot_v1(&directory.authority_snapshot)?;
    if snapshot_bytes.len() > MAX_CANONICAL_NESTED_RATCHET_BYTES {
        return Err(CanonicalRatchetError::NestedEncodingTooLarge {
            field: "authority_snapshot",
            actual: snapshot_bytes.len(),
            maximum: MAX_CANONICAL_NESTED_RATCHET_BYTES,
        });
    }
    let account_id = directory.authority_snapshot.content.account_id;
    let revision = directory.authority_snapshot.content.revision;
    if directory
        .certificates
        .windows(2)
        .any(|pair| pair[0].content.device_id >= pair[1].content.device_id)
    {
        return Err(CanonicalRatchetError::NonCanonicalCertificateOrder);
    }
    if directory.pools.windows(2).any(|pair| {
        pair[0].content.ratchet_identity.content.device_id
            >= pair[1].content.ratchet_identity.content.device_id
    }) {
        return Err(CanonicalRatchetError::NonCanonicalPoolOrder);
    }
    for (certificate, pool) in directory.certificates.iter().zip(&directory.pools) {
        encode_signed_device_certificate_v1(certificate)?;
        verify_signed_prekey_pool_v1(pool)?;
        if certificate.content.account_id != account_id {
            return Err(CanonicalRatchetError::AccountMismatch {
                context: "directory certificate",
            });
        }
        if certificate.content.authority_sequence >= revision {
            return Err(CanonicalRatchetError::CertificateOutsideAuthoritySnapshot);
        }
        let device_id = certificate.content.device_id;
        if directory
            .authority_snapshot
            .content
            .revocations
            .iter()
            .any(|revocation| revocation.content.device_id == device_id)
        {
            return Err(CanonicalRatchetError::RevokedDirectoryDevice);
        }
        if pool.content.ratchet_identity.content.device_id != device_id {
            return Err(CanonicalRatchetError::DirectoryCoverageMismatch);
        }
    }
    Ok(())
}

fn validate_directory_count(count: usize) -> Result<(), CanonicalRatchetError> {
    if count == 0 {
        return Err(CanonicalRatchetError::EmptyDirectory);
    }
    if count > MAX_CANONICAL_PREKEY_DIRECTORY_DEVICES {
        return Err(CanonicalRatchetError::TooManyDirectoryDevices {
            actual: count,
            maximum: MAX_CANONICAL_PREKEY_DIRECTORY_DEVICES,
        });
    }
    Ok(())
}

fn validate_pool_content(
    content: &CanonicalPrekeyPoolContentV1,
) -> Result<(), CanonicalRatchetError> {
    verify_signed_ratchet_identity_v1(&content.ratchet_identity)?;
    if content.bundles.is_empty() {
        return Err(CanonicalRatchetError::EmptyPrekeyPool);
    }
    if content.bundles.len() > MAX_CANONICAL_PREKEY_POOL_SIZE {
        return Err(CanonicalRatchetError::TooManyPrekeys {
            actual: content.bundles.len(),
            maximum: MAX_CANONICAL_PREKEY_POOL_SIZE,
        });
    }
    if content.published_at_unix_seconds >= content.expires_at_unix_seconds {
        return Err(CanonicalRatchetError::InvalidValidityInterval);
    }
    let validity = content
        .expires_at_unix_seconds
        .checked_sub(content.published_at_unix_seconds)
        .ok_or(CanonicalRatchetError::InvalidValidityInterval)?;
    if validity > MAX_CANONICAL_PREKEY_VALIDITY_SECONDS {
        return Err(CanonicalRatchetError::ValidityTooLong {
            actual: validity,
            maximum: MAX_CANONICAL_PREKEY_VALIDITY_SECONDS,
        });
    }
    for (index, bundle) in content.bundles.iter().enumerate() {
        verify_signed_prekey_bundle_v1(bundle)?;
        if bundle.content.ratchet_identity != content.ratchet_identity {
            return Err(CanonicalRatchetError::RatchetIdentityMismatch {
                context: "prekey pool bundle",
            });
        }
        if index > 0 {
            let expected = content.bundles[index - 1]
                .content
                .sequence
                .checked_add(1)
                .ok_or(CanonicalRatchetError::NonContiguousPrekeySequence)?;
            if bundle.content.sequence != expected {
                return Err(CanonicalRatchetError::NonContiguousPrekeySequence);
            }
        }
        if content.bundles[..index]
            .iter()
            .any(|previous| previous.content.one_time_key == bundle.content.one_time_key)
        {
            return Err(CanonicalRatchetError::DuplicateOneTimeKey);
        }
    }
    Ok(())
}

fn verify_ed25519(
    verifying_key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
    object: &'static str,
) -> Result<(), CanonicalRatchetError> {
    let verifying_key = VerifyingKey::from_bytes(verifying_key)
        .map_err(|_| CanonicalRatchetError::InvalidPublicKey { object })?;
    let signature = Signature::try_from(signature.as_slice())
        .map_err(|_| CanonicalRatchetError::InvalidSignature { object })?;
    verifying_key
        .verify_strict(message, &signature)
        .map_err(|_| CanonicalRatchetError::InvalidSignature { object })
}

fn decode_signed_ratchet_identity_from(
    decoder: &mut RatchetDecoder<'_>,
) -> Result<CanonicalSignedRatchetIdentityV1, CanonicalRatchetError> {
    decoder.expect_array("signed-ratchet-identity-v1", 3)?;
    decoder.expect_version("signed-ratchet-identity-v1.schema_version")?;
    let content = decode_ratchet_identity_content_from(decoder)?;
    let signature = decoder.fixed_bytes("signed-ratchet-identity-v1.signature")?;
    Ok(CanonicalSignedRatchetIdentityV1 { content, signature })
}

fn decode_ratchet_identity_content_from(
    decoder: &mut RatchetDecoder<'_>,
) -> Result<CanonicalRatchetIdentityContentV1, CanonicalRatchetError> {
    decoder.expect_array("ratchet-identity-content-v1", 4)?;
    decoder.expect_version("ratchet-identity-content-v1.schema_version")?;
    Ok(CanonicalRatchetIdentityContentV1 {
        device_id: decoder.fixed_bytes("ratchet-identity-content-v1.device_id")?,
        curve25519_key: decoder.fixed_bytes("ratchet-identity-content-v1.curve25519_key")?,
        ed25519_key: decoder.fixed_bytes("ratchet-identity-content-v1.ed25519_key")?,
    })
}

fn decode_signed_prekey_bundle_from(
    decoder: &mut RatchetDecoder<'_>,
) -> Result<CanonicalSignedPrekeyBundleV1, CanonicalRatchetError> {
    decoder.expect_array("signed-prekey-bundle-v1", 3)?;
    decoder.expect_version("signed-prekey-bundle-v1.schema_version")?;
    let content = decode_prekey_bundle_content_from(decoder)?;
    let signature = decoder.fixed_bytes("signed-prekey-bundle-v1.signature")?;
    Ok(CanonicalSignedPrekeyBundleV1 { content, signature })
}

fn decode_prekey_bundle_content_from(
    decoder: &mut RatchetDecoder<'_>,
) -> Result<CanonicalPrekeyBundleContentV1, CanonicalRatchetError> {
    decoder.expect_array("prekey-bundle-content-v1", 4)?;
    decoder.expect_version("prekey-bundle-content-v1.schema_version")?;
    let identity = decode_and_verify_signed_ratchet_identity_v1(decoder.bytes(
        "prekey-bundle-content-v1.ratchet_identity",
        MAX_CANONICAL_NESTED_RATCHET_BYTES,
    )?)?;
    Ok(CanonicalPrekeyBundleContentV1 {
        ratchet_identity: identity,
        sequence: decoder.unsigned("prekey-bundle-content-v1.sequence")?,
        one_time_key: decoder.fixed_bytes("prekey-bundle-content-v1.one_time_key")?,
    })
}

fn decode_signed_prekey_pool_from(
    decoder: &mut RatchetDecoder<'_>,
) -> Result<CanonicalSignedPrekeyPoolV1, CanonicalRatchetError> {
    decoder.expect_array("signed-prekey-pool-v1", 3)?;
    decoder.expect_version("signed-prekey-pool-v1.schema_version")?;
    let content = decode_prekey_pool_content_from(decoder)?;
    let signature = decoder.fixed_bytes("signed-prekey-pool-v1.signature")?;
    Ok(CanonicalSignedPrekeyPoolV1 { content, signature })
}

fn decode_prekey_pool_content_from(
    decoder: &mut RatchetDecoder<'_>,
) -> Result<CanonicalPrekeyPoolContentV1, CanonicalRatchetError> {
    decoder.expect_array("prekey-pool-content-v1", 6)?;
    decoder.expect_version("prekey-pool-content-v1.schema_version")?;
    let ratchet_identity = decode_and_verify_signed_ratchet_identity_v1(decoder.bytes(
        "prekey-pool-content-v1.ratchet_identity",
        MAX_CANONICAL_NESTED_RATCHET_BYTES,
    )?)?;
    let generation = decoder.unsigned("prekey-pool-content-v1.generation")?;
    let published_at_unix_seconds =
        decoder.unsigned("prekey-pool-content-v1.published_at_unix_seconds")?;
    let expires_at_unix_seconds =
        decoder.unsigned("prekey-pool-content-v1.expires_at_unix_seconds")?;
    let count = decoder.array("prekey-pool-content-v1.bundles")?;
    if count == 0 {
        return Err(CanonicalRatchetError::EmptyPrekeyPool);
    }
    if count > MAX_CANONICAL_PREKEY_POOL_SIZE {
        return Err(CanonicalRatchetError::TooManyPrekeys {
            actual: count,
            maximum: MAX_CANONICAL_PREKEY_POOL_SIZE,
        });
    }
    let mut bundles = Vec::with_capacity(count);
    for _ in 0..count {
        bundles.push(decode_and_verify_signed_prekey_bundle_v1(decoder.bytes(
            "prekey-pool-content-v1.bundle",
            MAX_CANONICAL_NESTED_RATCHET_BYTES,
        )?)?);
    }
    let content = CanonicalPrekeyPoolContentV1 {
        ratchet_identity,
        generation,
        published_at_unix_seconds,
        expires_at_unix_seconds,
        bundles,
    };
    validate_pool_content(&content)?;
    Ok(content)
}

struct RatchetDecoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> RatchetDecoder<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn finish(self) -> Result<(), CanonicalRatchetError> {
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
    ) -> Result<(), CanonicalRatchetError> {
        let actual = self.array(object)?;
        if actual != expected {
            return Err(CanonicalRatchetError::WrongArrayLength {
                object,
                expected,
                actual,
            });
        }
        Ok(())
    }

    fn expect_version(&mut self, field: &'static str) -> Result<(), CanonicalRatchetError> {
        let actual = self.unsigned(field)?;
        if actual != CANONICAL_PROFILE_VERSION {
            return Err(CanonicalRatchetError::WrongUnsignedValue {
                field,
                expected: CANONICAL_PROFILE_VERSION,
                actual,
            });
        }
        Ok(())
    }

    fn array(&mut self, field: &'static str) -> Result<usize, CanonicalRatchetError> {
        let length = self.head(4, field)?;
        usize::try_from(length).map_err(|_| CanonicalWireError::LengthOverflow.into())
    }

    fn unsigned(&mut self, field: &'static str) -> Result<u64, CanonicalRatchetError> {
        self.head(0, field)
    }

    fn fixed_bytes<const N: usize>(
        &mut self,
        field: &'static str,
    ) -> Result<[u8; N], CanonicalRatchetError> {
        let value = self.bytes(field, N)?;
        if value.len() != N {
            return Err(CanonicalRatchetError::WrongByteStringLength {
                field,
                expected: N,
                actual: value.len(),
            });
        }
        value
            .try_into()
            .map_err(|_| CanonicalWireError::UnexpectedEnd.into())
    }

    fn bytes(
        &mut self,
        field: &'static str,
        maximum: usize,
    ) -> Result<&'a [u8], CanonicalRatchetError> {
        let length = self.head(2, field)?;
        let length = usize::try_from(length).map_err(|_| CanonicalWireError::LengthOverflow)?;
        if length > maximum {
            return Err(CanonicalRatchetError::NestedEncodingTooLarge {
                field,
                actual: length,
                maximum,
            });
        }
        let end = self
            .cursor
            .checked_add(length)
            .ok_or(CanonicalWireError::LengthOverflow)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(CanonicalWireError::UnexpectedEnd)?;
        self.cursor = end;
        Ok(value)
    }

    fn head(
        &mut self,
        expected_major: u8,
        field: &'static str,
    ) -> Result<u64, CanonicalRatchetError> {
        let initial = self.byte()?;
        let actual_major = initial >> 5;
        if actual_major != expected_major {
            return Err(CanonicalRatchetError::WrongMajorType {
                field,
                expected: expected_major,
                actual: actual_major,
            });
        }
        self.argument(initial & 0x1f)
    }

    fn argument(&mut self, additional: u8) -> Result<u64, CanonicalRatchetError> {
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

    fn byte(&mut self) -> Result<u8, CanonicalRatchetError> {
        let value = self
            .bytes
            .get(self.cursor)
            .copied()
            .ok_or(CanonicalWireError::UnexpectedEnd)?;
        self.cursor += 1;
        Ok(value)
    }

    fn bytes_array<const N: usize>(&mut self) -> Result<[u8; N], CanonicalRatchetError> {
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
    use crate::canonical_authority_v1::{
        CANONICAL_AUTHORITY_SNAPSHOT_SIGNATURE_DOMAIN,
        CANONICAL_DEVICE_CERTIFICATE_SIGNATURE_DOMAIN, CanonicalAccountAuthoritySnapshotContentV1,
        CanonicalDeviceCapabilityV1, CanonicalDeviceCertificateContentV1,
        CanonicalSignedAccountAuthoritySnapshotV1, CanonicalSignedDeviceCertificateV1,
        account_authority_snapshot_signing_bytes_v1, device_certificate_signing_bytes_v1,
        encode_signed_account_authority_snapshot_v1, encode_signed_device_certificate_v1,
    };

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RatchetVectors {
        schema_version: u64,
        profile: String,
        activation_status: String,
        input: RatchetVectorInput,
        expected: RatchetVectorExpected,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RatchetVectorInput {
        root_secret_key_hex: String,
        device_secret_key_hex: String,
        encryption_public_key_hex: String,
        curve25519_key_hex: String,
        olm_ed25519_key_hex: String,
        one_time_key_hexes: Vec<String>,
        certificate_authority_sequence: u64,
        authority_revision: u64,
        prekey_sequences: Vec<u64>,
        pool_generation: u64,
        published_at_unix_seconds: u64,
        expires_at_unix_seconds: u64,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RatchetVectorExpected {
        account_id_hex: String,
        device_id_hex: String,
        certificate: SignedObjectExpected,
        authority_snapshot: SignedObjectExpected,
        ratchet_identity: SignedObjectExpected,
        prekey_bundles: Vec<SignedObjectExpected>,
        prekey_pool: SignedObjectExpected,
        prekey_pool_id_domain_hex: String,
        prekey_pool_id_hex: String,
        account_prekey_directory_cbor_hex: String,
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

    fn invalid_data(message: impl Into<String>) -> Box<dyn std::error::Error> {
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message.into(),
        ))
    }

    fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if value.is_empty()
            || !value.len().is_multiple_of(2)
            || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
            || value.bytes().any(|byte| byte.is_ascii_uppercase())
        {
            return Err(invalid_data(
                "ratchet vector contains non-canonical hexadecimal",
            ));
        }
        let mut decoded = Vec::with_capacity(value.len() / 2);
        for offset in (0..value.len()).step_by(2) {
            decoded.push(u8::from_str_radix(&value[offset..offset + 2], 16)?);
        }
        Ok(decoded)
    }

    fn fixed_hex<const N: usize>(value: &str) -> Result<[u8; N], Box<dyn std::error::Error>> {
        decode_hex(value)?.try_into().map_err(|value: Vec<u8>| {
            invalid_data(format!("expected {N} bytes, got {}", value.len()))
        })
    }

    fn assert_signed_expected(
        expected: &SignedObjectExpected,
        content: &[u8],
        domain: &[u8],
        preimage: &[u8],
        signature: &[u8; 64],
        signed: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(decode_hex(&expected.content_cbor_hex)?, content);
        assert_eq!(decode_hex(&expected.signature_domain_hex)?, domain);
        assert_eq!(decode_hex(&expected.signing_preimage_hex)?, preimage);
        assert_eq!(fixed_hex::<64>(&expected.signature_hex)?, *signature);
        assert_eq!(decode_hex(&expected.signed_cbor_hex)?, signed);
        Ok(())
    }

    fn signed_fixture() -> Result<CanonicalSignedPrekeyPoolV1, CanonicalRatchetError> {
        let device_key = SigningKey::from_bytes(&[9; 32]);
        let identity_content = CanonicalRatchetIdentityContentV1 {
            device_id: device_key.verifying_key().to_bytes(),
            curve25519_key: [0x31; 32],
            ed25519_key: [0x41; 32],
        };
        let identity = CanonicalSignedRatchetIdentityV1 {
            signature: device_key
                .sign(&ratchet_identity_signing_bytes_v1(&identity_content))
                .to_bytes(),
            content: identity_content,
        };
        let mut bundles = Vec::new();
        for (sequence, byte) in [(7, 0x51), (8, 0x52)] {
            let content = CanonicalPrekeyBundleContentV1 {
                ratchet_identity: identity.clone(),
                sequence,
                one_time_key: [byte; 32],
            };
            bundles.push(CanonicalSignedPrekeyBundleV1 {
                signature: device_key
                    .sign(&prekey_bundle_signing_bytes_v1(&content)?)
                    .to_bytes(),
                content,
            });
        }
        let content = CanonicalPrekeyPoolContentV1 {
            ratchet_identity: identity,
            generation: 3,
            published_at_unix_seconds: 1_700_000_000,
            expires_at_unix_seconds: 1_700_086_400,
            bundles,
        };
        Ok(CanonicalSignedPrekeyPoolV1 {
            signature: device_key
                .sign(&prekey_pool_signing_bytes_v1(&content)?)
                .to_bytes(),
            content,
        })
    }

    #[test]
    fn signed_pool_round_trips_and_rejects_mutation() -> Result<(), Box<dyn std::error::Error>> {
        let pool = signed_fixture()?;
        let encoded = encode_signed_prekey_pool_v1(&pool)?;
        assert_eq!(decode_and_verify_signed_prekey_pool_v1(&encoded)?, pool);
        assert_ne!(prekey_pool_id_v1(&pool)?, [0; 32]);

        let mut tampered = encoded;
        let last = tampered
            .last_mut()
            .ok_or_else(|| std::io::Error::other("empty encoded pool"))?;
        *last ^= 1;
        assert!(decode_and_verify_signed_prekey_pool_v1(&tampered).is_err());
        Ok(())
    }

    #[test]
    fn pool_rejects_non_contiguous_and_duplicate_prekeys() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut pool = signed_fixture()?;
        pool.content.bundles[1].content.sequence = 10;
        assert!(matches!(
            verify_signed_prekey_pool_v1(&pool),
            Err(CanonicalRatchetError::NonContiguousPrekeySequence)
                | Err(CanonicalRatchetError::InvalidSignature { .. })
        ));

        let mut pool = signed_fixture()?;
        pool.content.bundles[1].content.one_time_key = pool.content.bundles[0].content.one_time_key;
        assert!(verify_signed_prekey_pool_v1(&pool).is_err());
        Ok(())
    }

    #[test]
    fn directory_round_trips_with_authority_linkage() -> Result<(), Box<dyn std::error::Error>> {
        let root_key = SigningKey::from_bytes(&[7; 32]);
        let pool = signed_fixture()?;
        let device_id = pool.content.ratchet_identity.content.device_id;
        let certificate_content = CanonicalDeviceCertificateContentV1 {
            account_id: root_key.verifying_key().to_bytes(),
            device_id,
            encryption_public_key: [0x61; 32],
            authority_sequence: 4,
            capabilities: vec![
                CanonicalDeviceCapabilityV1::SignEvents,
                CanonicalDeviceCapabilityV1::SyncHistory,
            ],
        };
        let certificate = CanonicalSignedDeviceCertificateV1 {
            signature: root_key
                .sign(&device_certificate_signing_bytes_v1(&certificate_content)?)
                .to_bytes(),
            content: certificate_content,
        };
        let snapshot_content = CanonicalAccountAuthoritySnapshotContentV1 {
            account_id: root_key.verifying_key().to_bytes(),
            revision: 5,
            revocations: Vec::new(),
        };
        let snapshot = CanonicalSignedAccountAuthoritySnapshotV1 {
            signature: root_key
                .sign(&account_authority_snapshot_signing_bytes_v1(
                    &snapshot_content,
                )?)
                .to_bytes(),
            content: snapshot_content,
        };
        let directory = CanonicalAccountPrekeyDirectoryV1 {
            authority_snapshot: snapshot,
            certificates: vec![certificate],
            pools: vec![pool],
        };
        let encoded = encode_account_prekey_directory_v1(&directory)?;
        assert_eq!(
            decode_and_verify_account_prekey_directory_v1(&encoded)?,
            directory
        );
        Ok(())
    }

    #[test]
    fn golden_ratchet_vector_matches_exact_bytes_and_signatures()
    -> Result<(), Box<dyn std::error::Error>> {
        let vectors: RatchetVectors =
            serde_json::from_str(include_str!("../../../CANONICAL-RATCHET-VECTORS.json"))?;
        assert_eq!(vectors.schema_version, 1);
        assert_eq!(vectors.profile, "kilogram-deterministic-cbor-v1");
        assert_eq!(vectors.activation_status, "not-active");
        let input = vectors.input;
        let expected = vectors.expected;
        let root_key = SigningKey::from_bytes(&fixed_hex(&input.root_secret_key_hex)?);
        let device_key = SigningKey::from_bytes(&fixed_hex(&input.device_secret_key_hex)?);
        let account_id = root_key.verifying_key().to_bytes();
        let device_id = device_key.verifying_key().to_bytes();
        assert_eq!(fixed_hex::<32>(&expected.account_id_hex)?, account_id);
        assert_eq!(fixed_hex::<32>(&expected.device_id_hex)?, device_id);

        let certificate_content = CanonicalDeviceCertificateContentV1 {
            account_id,
            device_id,
            encryption_public_key: fixed_hex(&input.encryption_public_key_hex)?,
            authority_sequence: input.certificate_authority_sequence,
            capabilities: vec![
                CanonicalDeviceCapabilityV1::SignEvents,
                CanonicalDeviceCapabilityV1::SyncHistory,
            ],
        };
        let certificate_preimage = device_certificate_signing_bytes_v1(&certificate_content)?;
        let certificate_signature = root_key.sign(&certificate_preimage).to_bytes();
        let certificate = CanonicalSignedDeviceCertificateV1 {
            content: certificate_content,
            signature: certificate_signature,
        };
        let certificate_cbor = encode_signed_device_certificate_v1(&certificate)?;
        assert_signed_expected(
            &expected.certificate,
            &certificate_preimage[CANONICAL_DEVICE_CERTIFICATE_SIGNATURE_DOMAIN.len()..],
            CANONICAL_DEVICE_CERTIFICATE_SIGNATURE_DOMAIN,
            &certificate_preimage,
            &certificate_signature,
            &certificate_cbor,
        )?;

        let snapshot_content = CanonicalAccountAuthoritySnapshotContentV1 {
            account_id,
            revision: input.authority_revision,
            revocations: Vec::new(),
        };
        let snapshot_preimage = account_authority_snapshot_signing_bytes_v1(&snapshot_content)?;
        let snapshot_signature = root_key.sign(&snapshot_preimage).to_bytes();
        let snapshot = CanonicalSignedAccountAuthoritySnapshotV1 {
            content: snapshot_content,
            signature: snapshot_signature,
        };
        let snapshot_cbor = encode_signed_account_authority_snapshot_v1(&snapshot)?;
        assert_signed_expected(
            &expected.authority_snapshot,
            &snapshot_preimage[CANONICAL_AUTHORITY_SNAPSHOT_SIGNATURE_DOMAIN.len()..],
            CANONICAL_AUTHORITY_SNAPSHOT_SIGNATURE_DOMAIN,
            &snapshot_preimage,
            &snapshot_signature,
            &snapshot_cbor,
        )?;

        let identity_content = CanonicalRatchetIdentityContentV1 {
            device_id,
            curve25519_key: fixed_hex(&input.curve25519_key_hex)?,
            ed25519_key: fixed_hex(&input.olm_ed25519_key_hex)?,
        };
        let identity_content_cbor = encode_ratchet_identity_content_v1(&identity_content);
        let identity_preimage = ratchet_identity_signing_bytes_v1(&identity_content);
        let identity_signature = device_key.sign(&identity_preimage).to_bytes();
        let identity = CanonicalSignedRatchetIdentityV1 {
            content: identity_content,
            signature: identity_signature,
        };
        let identity_cbor = encode_signed_ratchet_identity_v1(&identity)?;
        assert_signed_expected(
            &expected.ratchet_identity,
            &identity_content_cbor,
            CANONICAL_RATCHET_IDENTITY_SIGNATURE_DOMAIN,
            &identity_preimage,
            &identity_signature,
            &identity_cbor,
        )?;

        if input.prekey_sequences.len() != input.one_time_key_hexes.len()
            || expected.prekey_bundles.len() != input.prekey_sequences.len()
        {
            return Err(invalid_data("prekey vector lengths differ"));
        }
        let mut bundles = Vec::new();
        for index in 0..input.prekey_sequences.len() {
            let content = CanonicalPrekeyBundleContentV1 {
                ratchet_identity: identity.clone(),
                sequence: input.prekey_sequences[index],
                one_time_key: fixed_hex(&input.one_time_key_hexes[index])?,
            };
            let content_cbor = encode_prekey_bundle_content_v1(&content)?;
            let preimage = prekey_bundle_signing_bytes_v1(&content)?;
            let signature = device_key.sign(&preimage).to_bytes();
            let bundle = CanonicalSignedPrekeyBundleV1 { content, signature };
            let signed = encode_signed_prekey_bundle_v1(&bundle)?;
            assert_signed_expected(
                &expected.prekey_bundles[index],
                &content_cbor,
                CANONICAL_PREKEY_BUNDLE_SIGNATURE_DOMAIN,
                &preimage,
                &signature,
                &signed,
            )?;
            bundles.push(bundle);
        }

        let pool_content = CanonicalPrekeyPoolContentV1 {
            ratchet_identity: identity,
            generation: input.pool_generation,
            published_at_unix_seconds: input.published_at_unix_seconds,
            expires_at_unix_seconds: input.expires_at_unix_seconds,
            bundles,
        };
        let pool_content_cbor = encode_prekey_pool_content_v1(&pool_content)?;
        let pool_preimage = prekey_pool_signing_bytes_v1(&pool_content)?;
        let pool_signature = device_key.sign(&pool_preimage).to_bytes();
        let pool = CanonicalSignedPrekeyPoolV1 {
            content: pool_content,
            signature: pool_signature,
        };
        let pool_cbor = encode_signed_prekey_pool_v1(&pool)?;
        assert_signed_expected(
            &expected.prekey_pool,
            &pool_content_cbor,
            CANONICAL_PREKEY_POOL_SIGNATURE_DOMAIN,
            &pool_preimage,
            &pool_signature,
            &pool_cbor,
        )?;
        assert_eq!(
            decode_hex(&expected.prekey_pool_id_domain_hex)?,
            CANONICAL_PREKEY_POOL_ID_DOMAIN
        );
        assert_eq!(
            fixed_hex::<32>(&expected.prekey_pool_id_hex)?,
            prekey_pool_id_v1(&pool)?
        );

        let directory = CanonicalAccountPrekeyDirectoryV1 {
            authority_snapshot: snapshot,
            certificates: vec![certificate],
            pools: vec![pool],
        };
        let directory_cbor = encode_account_prekey_directory_v1(&directory)?;
        assert_eq!(
            decode_hex(&expected.account_prekey_directory_cbor_hex)?,
            directory_cbor
        );
        assert_eq!(
            decode_and_verify_account_prekey_directory_v1(&directory_cbor)?,
            directory
        );
        Ok(())
    }
}
