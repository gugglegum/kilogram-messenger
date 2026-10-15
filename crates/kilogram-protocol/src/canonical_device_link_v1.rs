//! Inactive deterministic-CBOR device-link authorization candidate for M1.1.9.
//!
//! The active device-link ceremony continues to use its versioned Postcard
//! artifacts. This module only defines and exercises a future portable shape.

use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

use crate::{
    canonical_account_v1::{
        AccountDecoder, CanonicalAccountError, CanonicalSignedAccountDeviceListV1,
        decode_signed_account_device_list_v1, encode_signed_account_device_list_v1,
        signed_envelope, verify_signed_account_device_list_v1,
    },
    canonical_authority_v1::{
        CanonicalAuthorityError, CanonicalSignedDeviceCertificateV1,
        decode_signed_device_certificate_v1, encode_signed_device_certificate_v1,
        verify_signed_device_certificate_v1,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
        push_unsigned, validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_DEVICE_LINK_NESTED_BYTES: usize = 512 * 1024;

pub const CANONICAL_DEVICE_LINK_AUTHORIZATION_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:device-link-authorization-signature:cbor-v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalDeviceLinkAuthorizationContentV1 {
    pub account_id: [u8; 32],
    pub request_id: [u8; 32],
    pub device_certificate: CanonicalSignedDeviceCertificateV1,
    pub device_list: CanonicalSignedAccountDeviceListV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedDeviceLinkAuthorizationV1 {
    pub content: CanonicalDeviceLinkAuthorizationContentV1,
    pub signature: [u8; 64],
}

#[derive(Debug, Error)]
pub enum CanonicalDeviceLinkError {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),

    #[error(transparent)]
    Authority(#[from] CanonicalAuthorityError),

    #[error(transparent)]
    Account(#[from] CanonicalAccountError),

    #[error("{context} names a different Account ID")]
    AccountMismatch { context: &'static str },

    #[error("device-link authorization list does not contain the exact requested certificate")]
    RequestedCertificateMissingFromList,

    #[error("nested canonical object exceeds {MAX_CANONICAL_DEVICE_LINK_NESTED_BYTES} bytes")]
    NestedObjectTooLarge,

    #[error("{object} contains an invalid Ed25519 verifying key")]
    InvalidPublicKey { object: &'static str },

    #[error("{object} contains an invalid Ed25519 signature")]
    InvalidSignature { object: &'static str },
}

pub fn encode_device_link_authorization_content_v1(
    content: &CanonicalDeviceLinkAuthorizationContentV1,
) -> Result<Vec<u8>, CanonicalDeviceLinkError> {
    validate_content(content)?;
    let certificate = encode_signed_device_certificate_v1(&content.device_certificate)?;
    let device_list = encode_signed_account_device_list_v1(&content.device_list)?;
    ensure_nested_bound(&certificate)?;
    ensure_nested_bound(&device_list)?;

    let mut output = Vec::new();
    push_array_header(&mut output, 5);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.account_id);
    push_bytes(&mut output, &content.request_id);
    push_bytes(&mut output, &certificate);
    push_bytes(&mut output, &device_list);
    Ok(output)
}

pub fn device_link_authorization_signing_bytes_v1(
    content: &CanonicalDeviceLinkAuthorizationContentV1,
) -> Result<Vec<u8>, CanonicalDeviceLinkError> {
    Ok(prefixed(
        CANONICAL_DEVICE_LINK_AUTHORIZATION_SIGNATURE_DOMAIN,
        &encode_device_link_authorization_content_v1(content)?,
    ))
}

pub fn encode_signed_device_link_authorization_v1(
    authorization: &CanonicalSignedDeviceLinkAuthorizationV1,
) -> Result<Vec<u8>, CanonicalDeviceLinkError> {
    verify_signed_device_link_authorization_v1(authorization)?;
    let content = encode_device_link_authorization_content_v1(&authorization.content)?;
    Ok(signed_envelope(&content, &authorization.signature))
}

pub fn decode_signed_device_link_authorization_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedDeviceLinkAuthorizationV1, CanonicalDeviceLinkError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-device-link-authorization-v1", 3)?;
    decoder.expect_version("signed-device-link-authorization-v1.schema_version")?;
    let content = decode_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-device-link-authorization-v1.signature")?;
    decoder.finish()?;
    Ok(CanonicalSignedDeviceLinkAuthorizationV1 { content, signature })
}

pub fn decode_and_verify_signed_device_link_authorization_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedDeviceLinkAuthorizationV1, CanonicalDeviceLinkError> {
    let authorization = decode_signed_device_link_authorization_v1(bytes)?;
    verify_signed_device_link_authorization_v1(&authorization)?;
    Ok(authorization)
}

pub fn verify_signed_device_link_authorization_v1(
    authorization: &CanonicalSignedDeviceLinkAuthorizationV1,
) -> Result<(), CanonicalDeviceLinkError> {
    validate_content(&authorization.content)?;
    verify_signed_device_certificate_v1(&authorization.content.device_certificate)?;
    verify_signed_account_device_list_v1(&authorization.content.device_list)?;
    verify_ed25519(
        &authorization.content.account_id,
        &device_link_authorization_signing_bytes_v1(&authorization.content)?,
        &authorization.signature,
        "signed-device-link-authorization-v1",
    )
}

fn validate_content(
    content: &CanonicalDeviceLinkAuthorizationContentV1,
) -> Result<(), CanonicalDeviceLinkError> {
    if content.device_certificate.content.account_id != content.account_id {
        return Err(CanonicalDeviceLinkError::AccountMismatch {
            context: "device-link certificate",
        });
    }
    if content
        .device_list
        .content
        .authority_snapshot
        .content
        .account_id
        != content.account_id
    {
        return Err(CanonicalDeviceLinkError::AccountMismatch {
            context: "device-link device list",
        });
    }
    if !content
        .device_list
        .content
        .devices
        .contains(&content.device_certificate)
    {
        return Err(CanonicalDeviceLinkError::RequestedCertificateMissingFromList);
    }
    Ok(())
}

fn decode_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalDeviceLinkAuthorizationContentV1, CanonicalDeviceLinkError> {
    decoder.expect_array("device-link-authorization-content-v1", 5)?;
    decoder.expect_version("device-link-authorization-content-v1.schema_version")?;
    let account_id = decoder.fixed_bytes("device-link-authorization-content-v1.account_id")?;
    let request_id = decoder.fixed_bytes("device-link-authorization-content-v1.request_id")?;
    let certificate_bytes =
        decoder.bytes("device-link-authorization-content-v1.device_certificate")?;
    ensure_nested_bound(certificate_bytes)?;
    let device_list_bytes = decoder.bytes("device-link-authorization-content-v1.device_list")?;
    ensure_nested_bound(device_list_bytes)?;
    let content = CanonicalDeviceLinkAuthorizationContentV1 {
        account_id,
        request_id,
        device_certificate: decode_signed_device_certificate_v1(certificate_bytes)?,
        device_list: decode_signed_account_device_list_v1(device_list_bytes)?,
    };
    validate_content(&content)?;
    Ok(content)
}

fn ensure_nested_bound(bytes: &[u8]) -> Result<(), CanonicalDeviceLinkError> {
    if bytes.len() > MAX_CANONICAL_DEVICE_LINK_NESTED_BYTES {
        return Err(CanonicalDeviceLinkError::NestedObjectTooLarge);
    }
    Ok(())
}

fn verify_ed25519(
    key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
    object: &'static str,
) -> Result<(), CanonicalDeviceLinkError> {
    let key = VerifyingKey::from_bytes(key)
        .map_err(|_| CanonicalDeviceLinkError::InvalidPublicKey { object })?;
    let signature = Signature::try_from(signature.as_slice())
        .map_err(|_| CanonicalDeviceLinkError::InvalidSignature { object })?;
    key.verify_strict(message, &signature)
        .map_err(|_| CanonicalDeviceLinkError::InvalidSignature { object })
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct DeviceLinkVectors {
        schema_version: u64,
        profile: String,
        activation_status: String,
        input: DeviceLinkInput,
        expected: DeviceLinkExpected,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct DeviceLinkInput {
        root_secret_key_hex: String,
        request_id_hex: String,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct DeviceLinkExpected {
        account_id_hex: String,
        device_id_hex: String,
        authorization: SignedExpected,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SignedExpected {
        content_cbor_hex: String,
        signature_domain_hex: String,
        signing_preimage_hex: String,
        signature_hex: String,
        signed_cbor_hex: String,
    }

    #[derive(Deserialize)]
    struct AuthorityVectors {
        expected: AuthorityExpected,
    }

    #[derive(Deserialize)]
    struct AuthorityExpected {
        certificate: NestedExpected,
        device_id_hex: String,
    }

    #[derive(Deserialize)]
    struct AccountVectors {
        expected: AccountExpected,
    }

    #[derive(Deserialize)]
    struct AccountExpected {
        account_device_list: NestedExpected,
    }

    #[derive(Deserialize)]
    struct NestedExpected {
        signed_cbor_hex: String,
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
                "device-link vector contains invalid hexadecimal",
            ));
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

    fn fixture() -> Result<
        (
            DeviceLinkVectors,
            CanonicalDeviceLinkAuthorizationContentV1,
            SigningKey,
        ),
        Box<dyn std::error::Error>,
    > {
        let vectors: DeviceLinkVectors = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-DEVICE-LINK-VECTORS.json"
        )))?;
        let authority: AuthorityVectors = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-AUTHORITY-VECTORS.json"
        )))?;
        let account: AccountVectors = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-ACCOUNT-VECTORS.json"
        )))?;
        let root = SigningKey::from_bytes(&decode_fixed(&vectors.input.root_secret_key_hex)?);
        let certificate = decode_signed_device_certificate_v1(&decode_hex(
            &authority.expected.certificate.signed_cbor_hex,
        )?)?;
        let device_list = decode_signed_account_device_list_v1(&decode_hex(
            &account.expected.account_device_list.signed_cbor_hex,
        )?)?;
        let content = CanonicalDeviceLinkAuthorizationContentV1 {
            account_id: root.verifying_key().to_bytes(),
            request_id: decode_fixed(&vectors.input.request_id_hex)?,
            device_certificate: certificate,
            device_list,
        };
        assert_eq!(
            vectors.expected.device_id_hex,
            authority.expected.device_id_hex
        );
        Ok((vectors, content, root))
    }

    #[test]
    fn device_link_vector_matches_exact_bytes_and_signature()
    -> Result<(), Box<dyn std::error::Error>> {
        let (vectors, content, root) = fixture()?;
        assert_eq!(vectors.schema_version, CANONICAL_PROFILE_VERSION);
        assert_eq!(vectors.profile, "kilogram-deterministic-cbor-v1");
        assert_eq!(vectors.activation_status, "not-active");
        assert_eq!(to_hex(&content.account_id), vectors.expected.account_id_hex);
        let encoded_content = encode_device_link_authorization_content_v1(&content)?;
        let preimage = device_link_authorization_signing_bytes_v1(&content)?;
        let signature = root.sign(&preimage).to_bytes();
        let authorization = CanonicalSignedDeviceLinkAuthorizationV1 { content, signature };
        let encoded = encode_signed_device_link_authorization_v1(&authorization)?;
        let expected = &vectors.expected.authorization;
        assert_eq!(to_hex(&encoded_content), expected.content_cbor_hex);
        assert_eq!(
            to_hex(CANONICAL_DEVICE_LINK_AUTHORIZATION_SIGNATURE_DOMAIN),
            expected.signature_domain_hex
        );
        assert_eq!(to_hex(&preimage), expected.signing_preimage_hex);
        assert_eq!(to_hex(&signature), expected.signature_hex);
        assert_eq!(to_hex(&encoded), expected.signed_cbor_hex);
        assert_eq!(
            decode_and_verify_signed_device_link_authorization_v1(&encoded)?,
            authorization
        );
        Ok(())
    }

    #[test]
    fn authorization_rejects_wrong_request_signature_and_trailing_data()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_, content, root) = fixture()?;
        let signature = root
            .sign(&device_link_authorization_signing_bytes_v1(&content)?)
            .to_bytes();
        let authorization = CanonicalSignedDeviceLinkAuthorizationV1 { content, signature };
        let mut encoded = encode_signed_device_link_authorization_v1(&authorization)?;
        encoded.push(0);
        assert!(decode_and_verify_signed_device_link_authorization_v1(&encoded).is_err());

        let mut wrong_request = authorization.clone();
        wrong_request.content.request_id[0] ^= 1;
        assert!(verify_signed_device_link_authorization_v1(&wrong_request).is_err());

        let mut wrong_list = authorization;
        wrong_list.content.device_list.content.devices.clear();
        assert!(matches!(
            verify_signed_device_link_authorization_v1(&wrong_list),
            Err(CanonicalDeviceLinkError::RequestedCertificateMissingFromList)
        ));
        Ok(())
    }
}
