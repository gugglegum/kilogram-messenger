//! Inactive deterministic-CBOR account collaboration artifacts for M1.1.8.
//!
//! These objects extend the canonical authority foundation with Root-signed
//! device-list and conversation-membership snapshots. They are deliberately
//! disconnected from the active Postcard runtime.

use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

use crate::{
    canonical_authority_v1::{
        CanonicalAuthorityError, CanonicalDeviceCapabilityV1,
        CanonicalSignedAccountAuthoritySnapshotV1, CanonicalSignedDeviceCertificateV1,
        decode_signed_account_authority_snapshot_v1, decode_signed_device_certificate_v1,
        encode_signed_account_authority_snapshot_v1, encode_signed_device_certificate_v1,
        verify_signed_account_authority_snapshot_v1, verify_signed_device_certificate_v1,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
        push_unsigned, validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_ACCOUNT_DEVICES: usize = 32;
pub const MAX_CANONICAL_CONVERSATION_MEMBERS: usize = 4096;
pub const MAX_CANONICAL_NESTED_ACCOUNT_BYTES: usize = 512 * 1024;

pub const CANONICAL_ACCOUNT_DEVICE_LIST_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:account-device-list-signature:cbor-v1\0";
pub const CANONICAL_CONVERSATION_MEMBERSHIP_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:conversation-membership-signature:cbor-v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalAccountDeviceListContentV1 {
    pub authority_snapshot: CanonicalSignedAccountAuthoritySnapshotV1,
    pub devices: Vec<CanonicalSignedDeviceCertificateV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedAccountDeviceListV1 {
    pub content: CanonicalAccountDeviceListContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalConversationMembershipContentV1 {
    pub conversation_id: [u8; 32],
    pub revision: u64,
    pub owner_account_id: [u8; 32],
    pub members: Vec<[u8; 32]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedConversationMembershipV1 {
    pub content: CanonicalConversationMembershipContentV1,
    pub signature: [u8; 64],
}

#[derive(Debug, Error)]
pub enum CanonicalAccountError {
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

    #[error("canonical account device list must not be empty")]
    EmptyDeviceList,

    #[error("canonical account device list must be strictly ordered by Device ID")]
    NonCanonicalDeviceOrder,

    #[error("canonical conversation membership revision must be greater than zero")]
    InvalidMembershipRevision,

    #[error("canonical conversation members must be strictly ordered by Account ID")]
    NonCanonicalMemberOrder,

    #[error("conversation owner is not present in the membership")]
    OwnerNotMember,

    #[error("{context} names a different Account ID")]
    AccountMismatch { context: &'static str },

    #[error("device certificate sequence is outside the authority snapshot")]
    CertificateOutsideSnapshot,

    #[error("device list contains a revoked certificate")]
    RevokedDevice,

    #[error("device certificate lacks the complete messaging capability set")]
    MissingMessagingCapabilities,

    #[error("nested canonical object exceeds {MAX_CANONICAL_NESTED_ACCOUNT_BYTES} bytes")]
    NestedObjectTooLarge,

    #[error("{object} contains an invalid Ed25519 verifying key")]
    InvalidPublicKey { object: &'static str },

    #[error("{object} contains an invalid Ed25519 signature")]
    InvalidSignature { object: &'static str },
}

pub fn encode_account_device_list_content_v1(
    content: &CanonicalAccountDeviceListContentV1,
) -> Result<Vec<u8>, CanonicalAccountError> {
    validate_device_list_content(content)?;
    let snapshot = encode_signed_account_authority_snapshot_v1(&content.authority_snapshot)?;
    ensure_nested_bound(&snapshot)?;
    let mut certificates = Vec::with_capacity(content.devices.len());
    for certificate in &content.devices {
        let encoded = encode_signed_device_certificate_v1(certificate)?;
        ensure_nested_bound(&encoded)?;
        certificates.push(encoded);
    }
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &snapshot);
    push_array_header(&mut output, certificates.len() as u64);
    for certificate in certificates {
        push_bytes(&mut output, &certificate);
    }
    Ok(output)
}

pub fn account_device_list_signing_bytes_v1(
    content: &CanonicalAccountDeviceListContentV1,
) -> Result<Vec<u8>, CanonicalAccountError> {
    Ok(prefixed(
        CANONICAL_ACCOUNT_DEVICE_LIST_SIGNATURE_DOMAIN,
        &encode_account_device_list_content_v1(content)?,
    ))
}

pub fn encode_signed_account_device_list_v1(
    list: &CanonicalSignedAccountDeviceListV1,
) -> Result<Vec<u8>, CanonicalAccountError> {
    verify_signed_account_device_list_v1(list)?;
    let content = encode_account_device_list_content_v1(&list.content)?;
    Ok(signed_envelope(&content, &list.signature))
}

pub fn decode_signed_account_device_list_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedAccountDeviceListV1, CanonicalAccountError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-account-device-list-v1", 3)?;
    decoder.expect_version("signed-account-device-list-v1.schema_version")?;
    let content = decode_account_device_list_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-account-device-list-v1.signature")?;
    decoder.finish()?;
    Ok(CanonicalSignedAccountDeviceListV1 { content, signature })
}

pub fn decode_and_verify_signed_account_device_list_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedAccountDeviceListV1, CanonicalAccountError> {
    let list = decode_signed_account_device_list_v1(bytes)?;
    verify_signed_account_device_list_v1(&list)?;
    Ok(list)
}

pub fn verify_signed_account_device_list_v1(
    list: &CanonicalSignedAccountDeviceListV1,
) -> Result<(), CanonicalAccountError> {
    validate_device_list_content(&list.content)?;
    verify_signed_account_authority_snapshot_v1(&list.content.authority_snapshot)?;
    for certificate in &list.content.devices {
        verify_signed_device_certificate_v1(certificate)?;
    }
    verify_ed25519(
        &list.content.authority_snapshot.content.account_id,
        &account_device_list_signing_bytes_v1(&list.content)?,
        &list.signature,
        "signed-account-device-list-v1",
    )
}

pub fn encode_conversation_membership_content_v1(
    content: &CanonicalConversationMembershipContentV1,
) -> Result<Vec<u8>, CanonicalAccountError> {
    validate_membership_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 5);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.conversation_id);
    push_unsigned(&mut output, content.revision);
    push_bytes(&mut output, &content.owner_account_id);
    push_array_header(&mut output, content.members.len() as u64);
    for member in &content.members {
        push_bytes(&mut output, member);
    }
    Ok(output)
}

pub fn conversation_membership_signing_bytes_v1(
    content: &CanonicalConversationMembershipContentV1,
) -> Result<Vec<u8>, CanonicalAccountError> {
    Ok(prefixed(
        CANONICAL_CONVERSATION_MEMBERSHIP_SIGNATURE_DOMAIN,
        &encode_conversation_membership_content_v1(content)?,
    ))
}

pub fn encode_signed_conversation_membership_v1(
    membership: &CanonicalSignedConversationMembershipV1,
) -> Result<Vec<u8>, CanonicalAccountError> {
    verify_signed_conversation_membership_v1(membership)?;
    let content = encode_conversation_membership_content_v1(&membership.content)?;
    Ok(signed_envelope(&content, &membership.signature))
}

pub fn decode_signed_conversation_membership_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedConversationMembershipV1, CanonicalAccountError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-conversation-membership-v1", 3)?;
    decoder.expect_version("signed-conversation-membership-v1.schema_version")?;
    let content = decode_conversation_membership_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-conversation-membership-v1.signature")?;
    decoder.finish()?;
    Ok(CanonicalSignedConversationMembershipV1 { content, signature })
}

pub fn decode_and_verify_signed_conversation_membership_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedConversationMembershipV1, CanonicalAccountError> {
    let membership = decode_signed_conversation_membership_v1(bytes)?;
    verify_signed_conversation_membership_v1(&membership)?;
    Ok(membership)
}

pub fn verify_signed_conversation_membership_v1(
    membership: &CanonicalSignedConversationMembershipV1,
) -> Result<(), CanonicalAccountError> {
    validate_membership_content(&membership.content)?;
    verify_ed25519(
        &membership.content.owner_account_id,
        &conversation_membership_signing_bytes_v1(&membership.content)?,
        &membership.signature,
        "signed-conversation-membership-v1",
    )
}

pub(crate) fn signed_envelope(content: &[u8], signature: &[u8; 64]) -> Vec<u8> {
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    output.extend_from_slice(content);
    push_bytes(&mut output, signature);
    output
}

fn validate_device_list_content(
    content: &CanonicalAccountDeviceListContentV1,
) -> Result<(), CanonicalAccountError> {
    if content.devices.is_empty() {
        return Err(CanonicalAccountError::EmptyDeviceList);
    }
    if content.devices.len() > MAX_CANONICAL_ACCOUNT_DEVICES {
        return Err(CanonicalWireError::TooManyItems {
            field: "devices",
            actual: content.devices.len(),
            maximum: MAX_CANONICAL_ACCOUNT_DEVICES,
        }
        .into());
    }
    let account_id = content.authority_snapshot.content.account_id;
    let revision = content.authority_snapshot.content.revision;
    for certificate in &content.devices {
        if certificate.content.account_id != account_id {
            return Err(CanonicalAccountError::AccountMismatch {
                context: "device-list certificate",
            });
        }
        if certificate.content.authority_sequence >= revision {
            return Err(CanonicalAccountError::CertificateOutsideSnapshot);
        }
        if certificate.content.capabilities
            != [
                CanonicalDeviceCapabilityV1::SignEvents,
                CanonicalDeviceCapabilityV1::SyncHistory,
            ]
        {
            return Err(CanonicalAccountError::MissingMessagingCapabilities);
        }
        if content
            .authority_snapshot
            .content
            .revocations
            .iter()
            .any(|revocation| revocation.content.device_id == certificate.content.device_id)
        {
            return Err(CanonicalAccountError::RevokedDevice);
        }
    }
    if content
        .devices
        .windows(2)
        .any(|pair| pair[0].content.device_id >= pair[1].content.device_id)
    {
        return Err(CanonicalAccountError::NonCanonicalDeviceOrder);
    }
    Ok(())
}

fn validate_membership_content(
    content: &CanonicalConversationMembershipContentV1,
) -> Result<(), CanonicalAccountError> {
    if content.revision == 0 {
        return Err(CanonicalAccountError::InvalidMembershipRevision);
    }
    if content.members.len() > MAX_CANONICAL_CONVERSATION_MEMBERS {
        return Err(CanonicalWireError::TooManyItems {
            field: "members",
            actual: content.members.len(),
            maximum: MAX_CANONICAL_CONVERSATION_MEMBERS,
        }
        .into());
    }
    if content.members.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(CanonicalAccountError::NonCanonicalMemberOrder);
    }
    if content
        .members
        .binary_search(&content.owner_account_id)
        .is_err()
    {
        return Err(CanonicalAccountError::OwnerNotMember);
    }
    Ok(())
}

fn ensure_nested_bound(bytes: &[u8]) -> Result<(), CanonicalAccountError> {
    if bytes.len() > MAX_CANONICAL_NESTED_ACCOUNT_BYTES {
        return Err(CanonicalAccountError::NestedObjectTooLarge);
    }
    Ok(())
}

fn verify_ed25519(
    key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
    object: &'static str,
) -> Result<(), CanonicalAccountError> {
    let key = VerifyingKey::from_bytes(key)
        .map_err(|_| CanonicalAccountError::InvalidPublicKey { object })?;
    let signature = Signature::try_from(signature.as_slice())
        .map_err(|_| CanonicalAccountError::InvalidSignature { object })?;
    key.verify_strict(message, &signature)
        .map_err(|_| CanonicalAccountError::InvalidSignature { object })
}

fn decode_account_device_list_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalAccountDeviceListContentV1, CanonicalAccountError> {
    decoder.expect_array("account-device-list-content-v1", 3)?;
    decoder.expect_version("account-device-list-content-v1.schema_version")?;
    let snapshot_bytes = decoder.bytes("account-device-list-content-v1.authority_snapshot")?;
    ensure_nested_bound(snapshot_bytes)?;
    let authority_snapshot = decode_signed_account_authority_snapshot_v1(snapshot_bytes)?;
    let count = decoder.array("account-device-list-content-v1.devices")?;
    if count > MAX_CANONICAL_ACCOUNT_DEVICES {
        return Err(CanonicalWireError::TooManyItems {
            field: "devices",
            actual: count,
            maximum: MAX_CANONICAL_ACCOUNT_DEVICES,
        }
        .into());
    }
    let mut devices = Vec::with_capacity(count);
    for _ in 0..count {
        let bytes = decoder.bytes("account-device-list-content-v1.device")?;
        ensure_nested_bound(bytes)?;
        devices.push(decode_signed_device_certificate_v1(bytes)?);
    }
    let content = CanonicalAccountDeviceListContentV1 {
        authority_snapshot,
        devices,
    };
    validate_device_list_content(&content)?;
    Ok(content)
}

fn decode_conversation_membership_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalConversationMembershipContentV1, CanonicalAccountError> {
    decoder.expect_array("conversation-membership-content-v1", 5)?;
    decoder.expect_version("conversation-membership-content-v1.schema_version")?;
    let conversation_id =
        decoder.fixed_bytes("conversation-membership-content-v1.conversation_id")?;
    let revision = decoder.unsigned("conversation-membership-content-v1.revision")?;
    let owner_account_id =
        decoder.fixed_bytes("conversation-membership-content-v1.owner_account_id")?;
    let count = decoder.array("conversation-membership-content-v1.members")?;
    if count > MAX_CANONICAL_CONVERSATION_MEMBERS {
        return Err(CanonicalWireError::TooManyItems {
            field: "members",
            actual: count,
            maximum: MAX_CANONICAL_CONVERSATION_MEMBERS,
        }
        .into());
    }
    let mut members = Vec::with_capacity(count);
    for _ in 0..count {
        members.push(decoder.fixed_bytes("conversation-membership-content-v1.member")?);
    }
    let content = CanonicalConversationMembershipContentV1 {
        conversation_id,
        revision,
        owner_account_id,
        members,
    };
    validate_membership_content(&content)?;
    Ok(content)
}

pub(crate) struct AccountDecoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> AccountDecoder<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    pub(crate) fn finish(self) -> Result<(), CanonicalAccountError> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(CanonicalWireError::TrailingData.into())
        }
    }

    pub(crate) fn expect_array(
        &mut self,
        object: &'static str,
        expected: usize,
    ) -> Result<(), CanonicalAccountError> {
        let actual = self.array(object)?;
        if actual == expected {
            Ok(())
        } else {
            Err(CanonicalAccountError::WrongArrayLength {
                object,
                expected,
                actual,
            })
        }
    }

    pub(crate) fn expect_version(
        &mut self,
        field: &'static str,
    ) -> Result<(), CanonicalAccountError> {
        let actual = self.unsigned(field)?;
        if actual == CANONICAL_PROFILE_VERSION {
            Ok(())
        } else {
            Err(CanonicalAccountError::WrongUnsignedValue {
                field,
                expected: CANONICAL_PROFILE_VERSION,
                actual,
            })
        }
    }

    fn array(&mut self, field: &'static str) -> Result<usize, CanonicalAccountError> {
        usize::try_from(self.head(4, field)?).map_err(|_| CanonicalWireError::LengthOverflow.into())
    }

    fn unsigned(&mut self, field: &'static str) -> Result<u64, CanonicalAccountError> {
        self.head(0, field)
    }

    pub(crate) fn fixed_bytes<const N: usize>(
        &mut self,
        field: &'static str,
    ) -> Result<[u8; N], CanonicalAccountError> {
        let value = self.bytes(field)?;
        if value.len() != N {
            return Err(CanonicalAccountError::WrongByteStringLength {
                field,
                expected: N,
                actual: value.len(),
            });
        }
        value
            .try_into()
            .map_err(|_| CanonicalWireError::UnexpectedEnd.into())
    }

    pub(crate) fn bytes(&mut self, field: &'static str) -> Result<&'a [u8], CanonicalAccountError> {
        let length = usize::try_from(self.head(2, field)?)
            .map_err(|_| CanonicalWireError::LengthOverflow)?;
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
    ) -> Result<u64, CanonicalAccountError> {
        let initial = self.byte()?;
        let actual_major = initial >> 5;
        if actual_major != expected_major {
            return Err(CanonicalAccountError::WrongMajorType {
                field,
                expected: expected_major,
                actual: actual_major,
            });
        }
        self.argument(initial & 0x1f)
    }

    fn argument(&mut self, additional: u8) -> Result<u64, CanonicalAccountError> {
        match additional {
            value @ 0..=23 => Ok(u64::from(value)),
            24 => Ok(u64::from(self.byte()?)),
            25 => Ok(u64::from(u16::from_be_bytes(self.array_bytes()?))),
            26 => Ok(u64::from(u32::from_be_bytes(self.array_bytes()?))),
            27 => Ok(u64::from_be_bytes(self.array_bytes()?)),
            31 => Err(CanonicalWireError::IndefiniteLength.into()),
            value => Err(CanonicalWireError::ReservedAdditionalInformation(value).into()),
        }
    }

    fn byte(&mut self) -> Result<u8, CanonicalAccountError> {
        let value = self
            .bytes
            .get(self.cursor)
            .copied()
            .ok_or(CanonicalWireError::UnexpectedEnd)?;
        self.cursor += 1;
        Ok(value)
    }

    fn array_bytes<const N: usize>(&mut self) -> Result<[u8; N], CanonicalAccountError> {
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
    struct AccountVectors {
        schema_version: u64,
        profile: String,
        activation_status: String,
        input: AccountInput,
        expected: AccountExpected,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct AccountInput {
        root_secret_key_hex: String,
        other_member_secret_key_hex: String,
        conversation_id_hex: String,
        membership_revision: u64,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct AccountExpected {
        account_id_hex: String,
        other_member_account_id_hex: String,
        ordered_members_hex: Vec<String>,
        account_device_list: SignedExpected,
        conversation_membership: SignedExpected,
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
    #[serde(deny_unknown_fields)]
    struct AuthorityExpected {
        account_id_hex: String,
        device_id_hex: String,
        revoked_device_id_hex: String,
        certificate: AuthoritySignedExpected,
        revocation: AuthoritySignedExpected,
        authority_snapshot: AuthoritySignedExpected,
        session_authorization: AuthoritySignedExpected,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct AuthoritySignedExpected {
        content_cbor_hex: String,
        signature_domain_hex: String,
        signing_preimage_hex: String,
        signature_hex: String,
        signed_cbor_hex: String,
    }

    fn load_vectors() -> Result<(AccountVectors, AuthorityVectors), Box<dyn std::error::Error>> {
        let account = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-ACCOUNT-VECTORS.json"
        )))?;
        let authority = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-AUTHORITY-VECTORS.json"
        )))?;
        Ok((account, authority))
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
            return Err(invalid_data("account vector contains invalid hexadecimal"));
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

    fn assert_expected(
        expected: &SignedExpected,
        domain: &[u8],
        content: &[u8],
        preimage: &[u8],
        signature: &[u8; 64],
        signed: &[u8],
    ) {
        assert_eq!(to_hex(content), expected.content_cbor_hex);
        assert_eq!(to_hex(domain), expected.signature_domain_hex);
        assert_eq!(to_hex(preimage), expected.signing_preimage_hex);
        assert_eq!(to_hex(signature), expected.signature_hex);
        assert_eq!(to_hex(signed), expected.signed_cbor_hex);
    }

    #[test]
    fn account_collaboration_vectors_match_bytes_and_signatures()
    -> Result<(), Box<dyn std::error::Error>> {
        let (vectors, authority) = load_vectors()?;
        assert_eq!(vectors.schema_version, CANONICAL_PROFILE_VERSION);
        assert_eq!(vectors.profile, "kilogram-deterministic-cbor-v1");
        assert_eq!(vectors.activation_status, "not-active");
        let root_seed = decode_fixed(&vectors.input.root_secret_key_hex)?;
        let root = SigningKey::from_bytes(&root_seed);
        let account_id = root.verifying_key().to_bytes();
        assert_eq!(to_hex(&account_id), vectors.expected.account_id_hex);
        assert_eq!(
            vectors.expected.account_id_hex,
            authority.expected.account_id_hex
        );

        let snapshot_bytes = decode_hex(&authority.expected.authority_snapshot.signed_cbor_hex)?;
        let certificate_bytes = decode_hex(&authority.expected.certificate.signed_cbor_hex)?;
        let device_list_content = CanonicalAccountDeviceListContentV1 {
            authority_snapshot: decode_signed_account_authority_snapshot_v1(&snapshot_bytes)?,
            devices: vec![decode_signed_device_certificate_v1(&certificate_bytes)?],
        };
        let list_content_bytes = encode_account_device_list_content_v1(&device_list_content)?;
        let list_preimage = account_device_list_signing_bytes_v1(&device_list_content)?;
        let list_signature = root.sign(&list_preimage).to_bytes();
        let list = CanonicalSignedAccountDeviceListV1 {
            content: device_list_content,
            signature: list_signature,
        };
        let list_bytes = encode_signed_account_device_list_v1(&list)?;
        assert_expected(
            &vectors.expected.account_device_list,
            CANONICAL_ACCOUNT_DEVICE_LIST_SIGNATURE_DOMAIN,
            &list_content_bytes,
            &list_preimage,
            &list_signature,
            &list_bytes,
        );
        assert_eq!(
            decode_and_verify_signed_account_device_list_v1(&list_bytes)?,
            list
        );

        let other_seed = decode_fixed(&vectors.input.other_member_secret_key_hex)?;
        let other_id = SigningKey::from_bytes(&other_seed)
            .verifying_key()
            .to_bytes();
        assert_eq!(
            to_hex(&other_id),
            vectors.expected.other_member_account_id_hex
        );
        let members = vectors
            .expected
            .ordered_members_hex
            .iter()
            .map(|member| decode_fixed(member))
            .collect::<Result<Vec<[u8; 32]>, _>>()?;
        let membership_content = CanonicalConversationMembershipContentV1 {
            conversation_id: decode_fixed(&vectors.input.conversation_id_hex)?,
            revision: vectors.input.membership_revision,
            owner_account_id: account_id,
            members,
        };
        let membership_content_bytes =
            encode_conversation_membership_content_v1(&membership_content)?;
        let membership_preimage = conversation_membership_signing_bytes_v1(&membership_content)?;
        let membership_signature = root.sign(&membership_preimage).to_bytes();
        let membership = CanonicalSignedConversationMembershipV1 {
            content: membership_content,
            signature: membership_signature,
        };
        let membership_bytes = encode_signed_conversation_membership_v1(&membership)?;
        assert_expected(
            &vectors.expected.conversation_membership,
            CANONICAL_CONVERSATION_MEMBERSHIP_SIGNATURE_DOMAIN,
            &membership_content_bytes,
            &membership_preimage,
            &membership_signature,
            &membership_bytes,
        );
        assert_eq!(
            decode_and_verify_signed_conversation_membership_v1(&membership_bytes)?,
            membership
        );
        Ok(())
    }

    #[test]
    fn account_collaboration_rejects_noncanonical_and_tampered_inputs()
    -> Result<(), Box<dyn std::error::Error>> {
        let (vectors, _) = load_vectors()?;
        let owner = decode_fixed(&vectors.expected.account_id_hex)?;
        let other = decode_fixed(&vectors.expected.other_member_account_id_hex)?;
        let conversation = decode_fixed(&vectors.input.conversation_id_hex)?;
        assert!(
            encode_conversation_membership_content_v1(&CanonicalConversationMembershipContentV1 {
                conversation_id: conversation,
                revision: 1,
                owner_account_id: owner,
                members: vec![owner, other],
            })
            .is_err()
        );
        assert!(
            encode_conversation_membership_content_v1(&CanonicalConversationMembershipContentV1 {
                conversation_id: conversation,
                revision: 1,
                owner_account_id: owner,
                members: vec![other],
            })
            .is_err()
        );
        let mut tampered = decode_hex(&vectors.expected.conversation_membership.signed_cbor_hex)?;
        let last = tampered
            .last_mut()
            .ok_or_else(|| invalid_data("empty fixture"))?;
        *last ^= 1;
        assert!(decode_and_verify_signed_conversation_membership_v1(&tampered).is_err());
        Ok(())
    }

    #[test]
    fn authority_fixture_shape_remains_closed() -> Result<(), Box<dyn std::error::Error>> {
        let (_, authority) = load_vectors()?;
        for value in [
            authority.expected.device_id_hex,
            authority.expected.revoked_device_id_hex,
            authority.expected.certificate.content_cbor_hex,
            authority.expected.certificate.signature_domain_hex,
            authority.expected.certificate.signing_preimage_hex,
            authority.expected.certificate.signature_hex,
            authority.expected.revocation.content_cbor_hex,
            authority.expected.revocation.signature_domain_hex,
            authority.expected.revocation.signing_preimage_hex,
            authority.expected.revocation.signature_hex,
            authority.expected.revocation.signed_cbor_hex,
            authority.expected.authority_snapshot.content_cbor_hex,
            authority.expected.authority_snapshot.signature_domain_hex,
            authority.expected.authority_snapshot.signing_preimage_hex,
            authority.expected.authority_snapshot.signature_hex,
            authority.expected.session_authorization.content_cbor_hex,
            authority
                .expected
                .session_authorization
                .signature_domain_hex,
            authority
                .expected
                .session_authorization
                .signing_preimage_hex,
            authority.expected.session_authorization.signature_hex,
            authority.expected.session_authorization.signed_cbor_hex,
        ] {
            assert!(!value.is_empty());
        }
        Ok(())
    }
}
