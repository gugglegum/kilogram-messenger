//! Inactive deterministic-CBOR Account Root recovery candidate for M1.1.26.
//!
//! The active portable recovery artifacts remain their versioned Postcard v1
//! forms. This module covers the recovery package, independent witness and
//! current-device freshness quorum without assigning a runtime identifier.

use std::collections::BTreeSet;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use thiserror::Error;

use crate::{
    canonical_account_v1::{
        AccountDecoder, CanonicalAccountError, CanonicalSignedAccountDeviceListV1,
        CanonicalSignedConversationMembershipV1, decode_and_verify_signed_account_device_list_v1,
        decode_and_verify_signed_conversation_membership_v1, encode_signed_account_device_list_v1,
        encode_signed_conversation_membership_v1, signed_envelope,
        verify_signed_account_device_list_v1, verify_signed_conversation_membership_v1,
    },
    canonical_authority_v1::{
        CanonicalAuthorityError, CanonicalSignedAccountAuthoritySnapshotV1,
        decode_and_verify_signed_account_authority_snapshot_v1,
        encode_signed_account_authority_snapshot_v1, verify_signed_account_authority_snapshot_v1,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
        push_unsigned, validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_ACCOUNT_RECOVERY_MEMBERSHIPS: usize = 16 * 1024;
pub const MAX_CANONICAL_ACCOUNT_RECOVERY_NESTED_BYTES: usize = 512 * 1024;
pub const MAX_CANONICAL_ACCOUNT_RECOVERY_PACKAGE_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_CANONICAL_ACCOUNT_RECOVERY_WITNESS_BYTES: usize = 4 * 1024;
pub const MAX_CANONICAL_ACCOUNT_RECOVERY_REQUEST_BYTES: usize =
    MAX_CANONICAL_ACCOUNT_RECOVERY_PACKAGE_BYTES
        + MAX_CANONICAL_ACCOUNT_RECOVERY_WITNESS_BYTES
        + 64 * 1024;
pub const MAX_CANONICAL_ACCOUNT_RECOVERY_APPROVAL_BYTES: usize = 16 * 1024;
pub const MAX_CANONICAL_ACCOUNT_RECOVERY_APPROVALS: usize = 32;
pub const MAX_CANONICAL_ACCOUNT_RECOVERY_VALIDITY_SECONDS: u64 = 1_800;
pub const CANONICAL_ACCOUNT_RECOVERY_CLOCK_SKEW_SECONDS: u64 = 120;

pub const CANONICAL_ACCOUNT_RECOVERY_PACKAGE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:account-root-recovery-package-signature:cbor-v1\0";
pub const CANONICAL_ACCOUNT_RECOVERY_PACKAGE_ID_DOMAIN: &[u8] =
    b"kilogram:account-root-recovery-package-id:cbor-v1\0";
pub const CANONICAL_ACCOUNT_RECOVERY_STATE_VECTOR_DOMAIN: &[u8] =
    b"kilogram:account-root-recovery-state-vector:cbor-v1\0";
pub const CANONICAL_ACCOUNT_RECOVERY_ROSTER_DOMAIN: &[u8] =
    b"kilogram:account-root-recovery-voter-roster:cbor-v1\0";
pub const CANONICAL_ACCOUNT_RECOVERY_WITNESS_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:account-root-recovery-witness-signature:cbor-v1\0";
pub const CANONICAL_ACCOUNT_RECOVERY_REQUEST_ID_DOMAIN: &[u8] =
    b"kilogram:account-root-recovery-approval-request-id:cbor-v1\0";
pub const CANONICAL_ACCOUNT_RECOVERY_APPROVAL_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:account-root-recovery-approval-signature:cbor-v1\0";
pub const CANONICAL_ACCOUNT_RECOVERY_APPROVAL_ID_DOMAIN: &[u8] =
    b"kilogram:account-root-recovery-approval-id:cbor-v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalAccountRecoveryPackageContentV1 {
    pub account_id: [u8; 32],
    pub authority_snapshot: CanonicalSignedAccountAuthoritySnapshotV1,
    pub device_list: CanonicalSignedAccountDeviceListV1,
    pub conversation_memberships: Vec<CanonicalSignedConversationMembershipV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedAccountRecoveryPackageV1 {
    pub content: CanonicalAccountRecoveryPackageContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalAccountRecoveryWitnessContentV1 {
    pub account_id: [u8; 32],
    pub authority_revision: u64,
    pub package_id: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedAccountRecoveryWitnessV1 {
    pub content: CanonicalAccountRecoveryWitnessContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalAccountRecoveryApprovalRequestV1 {
    pub package: CanonicalSignedAccountRecoveryPackageV1,
    pub witness: CanonicalSignedAccountRecoveryWitnessV1,
    pub challenge: [u8; 32],
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalAccountRecoveryApprovalContentV1 {
    pub account_id: [u8; 32],
    pub request_id: [u8; 32],
    pub package_id: [u8; 32],
    pub authority_revision: u64,
    pub state_vector_digest: [u8; 32],
    pub recovery_roster_digest: [u8; 32],
    pub challenge: [u8; 32],
    pub request_expires_at_unix_seconds: u64,
    pub approver_device_id: [u8; 32],
    pub previous_approval_head_digest: [u8; 32],
    pub approved_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedAccountRecoveryApprovalV1 {
    pub content: CanonicalAccountRecoveryApprovalContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalAccountRecoveryFreshnessClaimV1 {
    ArtifactIntegrityOnly,
    SingleCurrentDeviceObserved,
    CurrentDeviceMajorityObserved,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalAccountRecoveryQuorumReportV1 {
    pub claim: CanonicalAccountRecoveryFreshnessClaimV1,
    pub account_id: [u8; 32],
    pub request_id: [u8; 32],
    pub package_id: [u8; 32],
    pub recovery_roster_digest: [u8; 32],
    pub roster_count: usize,
    pub observed_approvals: usize,
    pub required_approvals: usize,
}

#[derive(Debug, Error)]
pub enum CanonicalAccountRecoveryError {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),
    #[error(transparent)]
    Account(#[from] CanonicalAccountError),
    #[error(transparent)]
    Authority(#[from] CanonicalAuthorityError),
    #[error("canonical Account recovery {object} exceeds {maximum} bytes")]
    ObjectTooLarge {
        object: &'static str,
        maximum: usize,
    },
    #[error("canonical Account recovery package is invalid")]
    InvalidPackage,
    #[error("canonical Account recovery witness is invalid")]
    InvalidWitness,
    #[error("canonical Account recovery request is invalid")]
    InvalidRequest,
    #[error("canonical Account recovery request is not current")]
    RequestNotCurrent,
    #[error("canonical Account recovery approval is invalid")]
    InvalidApproval,
    #[error("canonical Account recovery approval time is invalid")]
    ApprovalTimeInvalid,
    #[error("canonical Account recovery approvals must be strictly ordered by Device ID")]
    NonCanonicalApprovals,
    #[error("canonical Account recovery contains an invalid Ed25519 key")]
    InvalidPublicKey,
    #[error("canonical Account recovery signature is invalid")]
    InvalidSignature,
}

pub fn encode_account_recovery_package_content_v1(
    content: &CanonicalAccountRecoveryPackageContentV1,
) -> Result<Vec<u8>, CanonicalAccountRecoveryError> {
    validate_package_content(content)?;
    let snapshot = encode_signed_account_authority_snapshot_v1(&content.authority_snapshot)?;
    let device_list = encode_signed_account_device_list_v1(&content.device_list)?;
    ensure_nested_bound(&snapshot, "authority snapshot")?;
    ensure_nested_bound(&device_list, "device list")?;
    let mut memberships = Vec::with_capacity(content.conversation_memberships.len());
    for membership in &content.conversation_memberships {
        let encoded = encode_signed_conversation_membership_v1(membership)?;
        ensure_nested_bound(&encoded, "conversation membership")?;
        memberships.push(encoded);
    }
    let mut output = Vec::new();
    push_array_header(&mut output, 5);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.account_id);
    push_bytes(&mut output, &snapshot);
    push_bytes(&mut output, &device_list);
    push_array_header(&mut output, memberships.len() as u64);
    for membership in memberships {
        push_bytes(&mut output, &membership);
    }
    ensure_bound(
        &output,
        "package content",
        MAX_CANONICAL_ACCOUNT_RECOVERY_PACKAGE_BYTES,
    )?;
    Ok(output)
}

pub fn account_recovery_package_signing_bytes_v1(
    content: &CanonicalAccountRecoveryPackageContentV1,
) -> Result<Vec<u8>, CanonicalAccountRecoveryError> {
    Ok(prefixed(
        CANONICAL_ACCOUNT_RECOVERY_PACKAGE_SIGNATURE_DOMAIN,
        &encode_account_recovery_package_content_v1(content)?,
    ))
}

pub fn sign_account_recovery_package_v1(
    root_signing_key: &SigningKey,
    mut content: CanonicalAccountRecoveryPackageContentV1,
) -> Result<CanonicalSignedAccountRecoveryPackageV1, CanonicalAccountRecoveryError> {
    content.account_id = root_signing_key.verifying_key().to_bytes();
    let signature = root_signing_key
        .sign(&account_recovery_package_signing_bytes_v1(&content)?)
        .to_bytes();
    let package = CanonicalSignedAccountRecoveryPackageV1 { content, signature };
    verify_signed_account_recovery_package_v1(&package)?;
    Ok(package)
}

pub fn encode_signed_account_recovery_package_v1(
    package: &CanonicalSignedAccountRecoveryPackageV1,
) -> Result<Vec<u8>, CanonicalAccountRecoveryError> {
    verify_signed_account_recovery_package_v1(package)?;
    let encoded = signed_envelope(
        &encode_account_recovery_package_content_v1(&package.content)?,
        &package.signature,
    );
    ensure_bound(
        &encoded,
        "package",
        MAX_CANONICAL_ACCOUNT_RECOVERY_PACKAGE_BYTES,
    )?;
    Ok(encoded)
}

pub fn decode_and_verify_signed_account_recovery_package_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedAccountRecoveryPackageV1, CanonicalAccountRecoveryError> {
    ensure_bound(
        bytes,
        "package",
        MAX_CANONICAL_ACCOUNT_RECOVERY_PACKAGE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-account-recovery-package-v1", 3)?;
    decoder.expect_version("signed-account-recovery-package-v1.schema_version")?;
    let content = decode_package_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-account-recovery-package-v1.signature")?;
    decoder.finish()?;
    let package = CanonicalSignedAccountRecoveryPackageV1 { content, signature };
    verify_signed_account_recovery_package_v1(&package)?;
    Ok(package)
}

pub fn verify_signed_account_recovery_package_v1(
    package: &CanonicalSignedAccountRecoveryPackageV1,
) -> Result<(), CanonicalAccountRecoveryError> {
    validate_package_content(&package.content)?;
    verify_signed_account_authority_snapshot_v1(&package.content.authority_snapshot)?;
    verify_signed_account_device_list_v1(&package.content.device_list)?;
    for membership in &package.content.conversation_memberships {
        verify_signed_conversation_membership_v1(membership)?;
    }
    verify_ed25519(
        &package.content.account_id,
        &account_recovery_package_signing_bytes_v1(&package.content)?,
        &package.signature,
    )
}

pub fn account_recovery_package_id_v1(
    package: &CanonicalSignedAccountRecoveryPackageV1,
) -> Result<[u8; 32], CanonicalAccountRecoveryError> {
    Ok(domain_hash(
        CANONICAL_ACCOUNT_RECOVERY_PACKAGE_ID_DOMAIN,
        &encode_signed_account_recovery_package_v1(package)?,
    ))
}

pub fn account_recovery_state_vector_input_v1(
    package: &CanonicalSignedAccountRecoveryPackageV1,
) -> Result<Vec<u8>, CanonicalAccountRecoveryError> {
    verify_signed_account_recovery_package_v1(package)?;
    let snapshot =
        encode_signed_account_authority_snapshot_v1(&package.content.authority_snapshot)?;
    let device_list = encode_signed_account_device_list_v1(&package.content.device_list)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 4);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &snapshot);
    push_bytes(&mut output, &device_list);
    push_array_header(
        &mut output,
        package.content.conversation_memberships.len() as u64,
    );
    for membership in &package.content.conversation_memberships {
        push_bytes(
            &mut output,
            &encode_signed_conversation_membership_v1(membership)?,
        );
    }
    Ok(output)
}

pub fn account_recovery_state_vector_digest_v1(
    package: &CanonicalSignedAccountRecoveryPackageV1,
) -> Result<[u8; 32], CanonicalAccountRecoveryError> {
    Ok(domain_hash(
        CANONICAL_ACCOUNT_RECOVERY_STATE_VECTOR_DOMAIN,
        &account_recovery_state_vector_input_v1(package)?,
    ))
}

pub fn account_recovery_roster_digest_v1(
    package: &CanonicalSignedAccountRecoveryPackageV1,
) -> Result<[u8; 32], CanonicalAccountRecoveryError> {
    verify_signed_account_recovery_package_v1(package)?;
    Ok(domain_hash(
        CANONICAL_ACCOUNT_RECOVERY_ROSTER_DOMAIN,
        &encode_signed_account_device_list_v1(&package.content.device_list)?,
    ))
}

pub fn encode_account_recovery_witness_content_v1(
    content: &CanonicalAccountRecoveryWitnessContentV1,
) -> Result<Vec<u8>, CanonicalAccountRecoveryError> {
    validate_witness_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 4);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.account_id);
    push_unsigned(&mut output, content.authority_revision);
    push_bytes(&mut output, &content.package_id);
    Ok(output)
}

pub fn account_recovery_witness_signing_bytes_v1(
    content: &CanonicalAccountRecoveryWitnessContentV1,
) -> Result<Vec<u8>, CanonicalAccountRecoveryError> {
    Ok(prefixed(
        CANONICAL_ACCOUNT_RECOVERY_WITNESS_SIGNATURE_DOMAIN,
        &encode_account_recovery_witness_content_v1(content)?,
    ))
}

pub fn issue_account_recovery_witness_v1(
    root_signing_key: &SigningKey,
    package: &CanonicalSignedAccountRecoveryPackageV1,
) -> Result<CanonicalSignedAccountRecoveryWitnessV1, CanonicalAccountRecoveryError> {
    verify_signed_account_recovery_package_v1(package)?;
    if root_signing_key.verifying_key().to_bytes() != package.content.account_id {
        return Err(CanonicalAccountRecoveryError::InvalidWitness);
    }
    let content = CanonicalAccountRecoveryWitnessContentV1 {
        account_id: package.content.account_id,
        authority_revision: package.content.authority_snapshot.content.revision,
        package_id: account_recovery_package_id_v1(package)?,
    };
    let signature = root_signing_key
        .sign(&account_recovery_witness_signing_bytes_v1(&content)?)
        .to_bytes();
    Ok(CanonicalSignedAccountRecoveryWitnessV1 { content, signature })
}

pub fn encode_signed_account_recovery_witness_v1(
    witness: &CanonicalSignedAccountRecoveryWitnessV1,
) -> Result<Vec<u8>, CanonicalAccountRecoveryError> {
    verify_signed_account_recovery_witness_v1(witness)?;
    let encoded = signed_envelope(
        &encode_account_recovery_witness_content_v1(&witness.content)?,
        &witness.signature,
    );
    ensure_bound(
        &encoded,
        "witness",
        MAX_CANONICAL_ACCOUNT_RECOVERY_WITNESS_BYTES,
    )?;
    Ok(encoded)
}

pub fn decode_and_verify_signed_account_recovery_witness_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedAccountRecoveryWitnessV1, CanonicalAccountRecoveryError> {
    ensure_bound(
        bytes,
        "witness",
        MAX_CANONICAL_ACCOUNT_RECOVERY_WITNESS_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-account-recovery-witness-v1", 3)?;
    decoder.expect_version("signed-account-recovery-witness-v1.schema_version")?;
    let content = decode_witness_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-account-recovery-witness-v1.signature")?;
    decoder.finish()?;
    let witness = CanonicalSignedAccountRecoveryWitnessV1 { content, signature };
    verify_signed_account_recovery_witness_v1(&witness)?;
    Ok(witness)
}

pub fn verify_signed_account_recovery_witness_v1(
    witness: &CanonicalSignedAccountRecoveryWitnessV1,
) -> Result<(), CanonicalAccountRecoveryError> {
    validate_witness_content(&witness.content)?;
    verify_ed25519(
        &witness.content.account_id,
        &account_recovery_witness_signing_bytes_v1(&witness.content)?,
        &witness.signature,
    )
}

pub fn verify_account_recovery_witness_for_package_v1(
    witness: &CanonicalSignedAccountRecoveryWitnessV1,
    package: &CanonicalSignedAccountRecoveryPackageV1,
) -> Result<(), CanonicalAccountRecoveryError> {
    verify_signed_account_recovery_witness_v1(witness)?;
    verify_signed_account_recovery_package_v1(package)?;
    if witness.content.account_id != package.content.account_id
        || witness.content.authority_revision != package.content.authority_snapshot.content.revision
        || witness.content.package_id != account_recovery_package_id_v1(package)?
    {
        return Err(CanonicalAccountRecoveryError::InvalidWitness);
    }
    Ok(())
}

pub fn encode_account_recovery_approval_request_v1(
    request: &CanonicalAccountRecoveryApprovalRequestV1,
) -> Result<Vec<u8>, CanonicalAccountRecoveryError> {
    validate_request(request)?;
    let package = encode_signed_account_recovery_package_v1(&request.package)?;
    let witness = encode_signed_account_recovery_witness_v1(&request.witness)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 6);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &package);
    push_bytes(&mut output, &witness);
    push_bytes(&mut output, &request.challenge);
    push_unsigned(&mut output, request.issued_at_unix_seconds);
    push_unsigned(&mut output, request.expires_at_unix_seconds);
    ensure_bound(
        &output,
        "approval request",
        MAX_CANONICAL_ACCOUNT_RECOVERY_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_account_recovery_approval_request_v1(
    bytes: &[u8],
) -> Result<CanonicalAccountRecoveryApprovalRequestV1, CanonicalAccountRecoveryError> {
    ensure_bound(
        bytes,
        "approval request",
        MAX_CANONICAL_ACCOUNT_RECOVERY_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("account-recovery-approval-request-v1", 6)?;
    decoder.expect_version("account-recovery-approval-request-v1.schema_version")?;
    let package_bytes = decoder.bytes("account-recovery-approval-request-v1.package")?;
    let witness_bytes = decoder.bytes("account-recovery-approval-request-v1.witness")?;
    let challenge = decoder.fixed_bytes("account-recovery-approval-request-v1.challenge")?;
    let issued_at_unix_seconds =
        decoder.unsigned("account-recovery-approval-request-v1.issued_at_unix_seconds")?;
    let expires_at_unix_seconds =
        decoder.unsigned("account-recovery-approval-request-v1.expires_at_unix_seconds")?;
    decoder.finish()?;
    let request = CanonicalAccountRecoveryApprovalRequestV1 {
        package: decode_and_verify_signed_account_recovery_package_v1(package_bytes)?,
        witness: decode_and_verify_signed_account_recovery_witness_v1(witness_bytes)?,
        challenge,
        issued_at_unix_seconds,
        expires_at_unix_seconds,
    };
    validate_request(&request)?;
    Ok(request)
}

pub fn verify_account_recovery_approval_request_at_v1(
    request: &CanonicalAccountRecoveryApprovalRequestV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalAccountRecoveryError> {
    validate_request(request)?;
    if request.issued_at_unix_seconds
        > now_unix_seconds.saturating_add(CANONICAL_ACCOUNT_RECOVERY_CLOCK_SKEW_SECONDS)
        || request.expires_at_unix_seconds < now_unix_seconds
    {
        return Err(CanonicalAccountRecoveryError::RequestNotCurrent);
    }
    Ok(())
}

pub fn account_recovery_request_id_v1(
    request: &CanonicalAccountRecoveryApprovalRequestV1,
) -> Result<[u8; 32], CanonicalAccountRecoveryError> {
    Ok(domain_hash(
        CANONICAL_ACCOUNT_RECOVERY_REQUEST_ID_DOMAIN,
        &encode_account_recovery_approval_request_v1(request)?,
    ))
}

pub fn encode_account_recovery_approval_content_v1(
    content: &CanonicalAccountRecoveryApprovalContentV1,
) -> Result<Vec<u8>, CanonicalAccountRecoveryError> {
    validate_approval_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 12);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.account_id);
    push_bytes(&mut output, &content.request_id);
    push_bytes(&mut output, &content.package_id);
    push_unsigned(&mut output, content.authority_revision);
    push_bytes(&mut output, &content.state_vector_digest);
    push_bytes(&mut output, &content.recovery_roster_digest);
    push_bytes(&mut output, &content.challenge);
    push_unsigned(&mut output, content.request_expires_at_unix_seconds);
    push_bytes(&mut output, &content.approver_device_id);
    push_bytes(&mut output, &content.previous_approval_head_digest);
    push_unsigned(&mut output, content.approved_at_unix_seconds);
    Ok(output)
}

pub fn account_recovery_approval_signing_bytes_v1(
    content: &CanonicalAccountRecoveryApprovalContentV1,
) -> Result<Vec<u8>, CanonicalAccountRecoveryError> {
    Ok(prefixed(
        CANONICAL_ACCOUNT_RECOVERY_APPROVAL_SIGNATURE_DOMAIN,
        &encode_account_recovery_approval_content_v1(content)?,
    ))
}

pub fn sign_account_recovery_approval_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalAccountRecoveryApprovalRequestV1,
    previous_approval_head_digest: [u8; 32],
    approved_at_unix_seconds: u64,
) -> Result<CanonicalSignedAccountRecoveryApprovalV1, CanonicalAccountRecoveryError> {
    validate_request(request)?;
    let content = CanonicalAccountRecoveryApprovalContentV1 {
        account_id: request.package.content.account_id,
        request_id: account_recovery_request_id_v1(request)?,
        package_id: account_recovery_package_id_v1(&request.package)?,
        authority_revision: request.package.content.authority_snapshot.content.revision,
        state_vector_digest: account_recovery_state_vector_digest_v1(&request.package)?,
        recovery_roster_digest: account_recovery_roster_digest_v1(&request.package)?,
        challenge: request.challenge,
        request_expires_at_unix_seconds: request.expires_at_unix_seconds,
        approver_device_id: device_signing_key.verifying_key().to_bytes(),
        previous_approval_head_digest,
        approved_at_unix_seconds,
    };
    let signature = device_signing_key
        .sign(&account_recovery_approval_signing_bytes_v1(&content)?)
        .to_bytes();
    let approval = CanonicalSignedAccountRecoveryApprovalV1 { content, signature };
    verify_account_recovery_approval_for_request_v1(&approval, request, approved_at_unix_seconds)?;
    Ok(approval)
}

pub fn encode_signed_account_recovery_approval_v1(
    approval: &CanonicalSignedAccountRecoveryApprovalV1,
) -> Result<Vec<u8>, CanonicalAccountRecoveryError> {
    verify_signed_account_recovery_approval_v1(approval)?;
    let encoded = signed_envelope(
        &encode_account_recovery_approval_content_v1(&approval.content)?,
        &approval.signature,
    );
    ensure_bound(
        &encoded,
        "approval",
        MAX_CANONICAL_ACCOUNT_RECOVERY_APPROVAL_BYTES,
    )?;
    Ok(encoded)
}

pub fn decode_and_verify_signed_account_recovery_approval_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedAccountRecoveryApprovalV1, CanonicalAccountRecoveryError> {
    ensure_bound(
        bytes,
        "approval",
        MAX_CANONICAL_ACCOUNT_RECOVERY_APPROVAL_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-account-recovery-approval-v1", 3)?;
    decoder.expect_version("signed-account-recovery-approval-v1.schema_version")?;
    let content = decode_approval_content_from(&mut decoder)?;
    let signature = decoder.fixed_bytes("signed-account-recovery-approval-v1.signature")?;
    decoder.finish()?;
    let approval = CanonicalSignedAccountRecoveryApprovalV1 { content, signature };
    verify_signed_account_recovery_approval_v1(&approval)?;
    Ok(approval)
}

pub fn verify_signed_account_recovery_approval_v1(
    approval: &CanonicalSignedAccountRecoveryApprovalV1,
) -> Result<(), CanonicalAccountRecoveryError> {
    validate_approval_content(&approval.content)?;
    verify_ed25519(
        &approval.content.approver_device_id,
        &account_recovery_approval_signing_bytes_v1(&approval.content)?,
        &approval.signature,
    )
}

pub fn account_recovery_approval_id_v1(
    approval: &CanonicalSignedAccountRecoveryApprovalV1,
) -> Result<[u8; 32], CanonicalAccountRecoveryError> {
    Ok(domain_hash(
        CANONICAL_ACCOUNT_RECOVERY_APPROVAL_ID_DOMAIN,
        &encode_signed_account_recovery_approval_v1(approval)?,
    ))
}

pub fn verify_account_recovery_approval_for_request_v1(
    approval: &CanonicalSignedAccountRecoveryApprovalV1,
    request: &CanonicalAccountRecoveryApprovalRequestV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalAccountRecoveryError> {
    verify_signed_account_recovery_approval_v1(approval)?;
    verify_account_recovery_approval_request_at_v1(request, now_unix_seconds)?;
    let package = &request.package;
    if approval.content.account_id != package.content.account_id
        || approval.content.request_id != account_recovery_request_id_v1(request)?
        || approval.content.package_id != account_recovery_package_id_v1(package)?
        || approval.content.authority_revision
            != package.content.authority_snapshot.content.revision
        || approval.content.state_vector_digest != account_recovery_state_vector_digest_v1(package)?
        || approval.content.recovery_roster_digest != account_recovery_roster_digest_v1(package)?
        || approval.content.challenge != request.challenge
        || approval.content.request_expires_at_unix_seconds != request.expires_at_unix_seconds
    {
        return Err(CanonicalAccountRecoveryError::InvalidApproval);
    }
    if approval.content.approved_at_unix_seconds
        < request
            .issued_at_unix_seconds
            .saturating_sub(CANONICAL_ACCOUNT_RECOVERY_CLOCK_SKEW_SECONDS)
        || approval.content.approved_at_unix_seconds > request.expires_at_unix_seconds
        || approval.content.approved_at_unix_seconds
            > now_unix_seconds.saturating_add(CANONICAL_ACCOUNT_RECOVERY_CLOCK_SKEW_SECONDS)
    {
        return Err(CanonicalAccountRecoveryError::ApprovalTimeInvalid);
    }
    let certificate = package
        .content
        .device_list
        .content
        .devices
        .iter()
        .find(|certificate| certificate.content.device_id == approval.content.approver_device_id)
        .ok_or(CanonicalAccountRecoveryError::InvalidApproval)?;
    if certificate.content.capabilities.len() != 2 {
        return Err(CanonicalAccountRecoveryError::InvalidApproval);
    }
    Ok(())
}

pub fn verify_account_recovery_quorum_v1(
    request: &CanonicalAccountRecoveryApprovalRequestV1,
    approvals: &[CanonicalSignedAccountRecoveryApprovalV1],
    now_unix_seconds: u64,
) -> Result<CanonicalAccountRecoveryQuorumReportV1, CanonicalAccountRecoveryError> {
    verify_account_recovery_approval_request_at_v1(request, now_unix_seconds)?;
    if approvals.len() > MAX_CANONICAL_ACCOUNT_RECOVERY_APPROVALS {
        return Err(CanonicalAccountRecoveryError::InvalidApproval);
    }
    if approvals
        .windows(2)
        .any(|pair| pair[0].content.approver_device_id >= pair[1].content.approver_device_id)
    {
        return Err(CanonicalAccountRecoveryError::NonCanonicalApprovals);
    }
    let mut approvers = BTreeSet::new();
    for approval in approvals {
        verify_account_recovery_approval_for_request_v1(approval, request, now_unix_seconds)?;
        if !approvers.insert(approval.content.approver_device_id) {
            return Err(CanonicalAccountRecoveryError::NonCanonicalApprovals);
        }
    }
    let roster_count = request.package.content.device_list.content.devices.len();
    let observed_approvals = approvers.len();
    let required_approvals = roster_count / 2 + 1;
    let claim = if observed_approvals >= required_approvals {
        CanonicalAccountRecoveryFreshnessClaimV1::CurrentDeviceMajorityObserved
    } else if observed_approvals == 1 {
        CanonicalAccountRecoveryFreshnessClaimV1::SingleCurrentDeviceObserved
    } else {
        CanonicalAccountRecoveryFreshnessClaimV1::ArtifactIntegrityOnly
    };
    Ok(CanonicalAccountRecoveryQuorumReportV1 {
        claim,
        account_id: request.package.content.account_id,
        request_id: account_recovery_request_id_v1(request)?,
        package_id: account_recovery_package_id_v1(&request.package)?,
        recovery_roster_digest: account_recovery_roster_digest_v1(&request.package)?,
        roster_count,
        observed_approvals,
        required_approvals,
    })
}

fn decode_package_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalAccountRecoveryPackageContentV1, CanonicalAccountRecoveryError> {
    decoder.expect_array("account-recovery-package-content-v1", 5)?;
    decoder.expect_version("account-recovery-package-content-v1.schema_version")?;
    let account_id = decoder.fixed_bytes("account-recovery-package-content-v1.account_id")?;
    let snapshot_bytes = decoder.bytes("account-recovery-package-content-v1.authority_snapshot")?;
    ensure_nested_bound(snapshot_bytes, "authority snapshot")?;
    let device_list_bytes = decoder.bytes("account-recovery-package-content-v1.device_list")?;
    ensure_nested_bound(device_list_bytes, "device list")?;
    let count = decoder.array("account-recovery-package-content-v1.memberships")?;
    if count > MAX_CANONICAL_ACCOUNT_RECOVERY_MEMBERSHIPS {
        return Err(CanonicalAccountRecoveryError::InvalidPackage);
    }
    let mut conversation_memberships = Vec::with_capacity(count);
    for _ in 0..count {
        let bytes = decoder.bytes("account-recovery-package-content-v1.membership")?;
        ensure_nested_bound(bytes, "conversation membership")?;
        conversation_memberships.push(decode_and_verify_signed_conversation_membership_v1(bytes)?);
    }
    let content = CanonicalAccountRecoveryPackageContentV1 {
        account_id,
        authority_snapshot: decode_and_verify_signed_account_authority_snapshot_v1(snapshot_bytes)?,
        device_list: decode_and_verify_signed_account_device_list_v1(device_list_bytes)?,
        conversation_memberships,
    };
    validate_package_content(&content)?;
    Ok(content)
}

fn decode_witness_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalAccountRecoveryWitnessContentV1, CanonicalAccountRecoveryError> {
    decoder.expect_array("account-recovery-witness-content-v1", 4)?;
    decoder.expect_version("account-recovery-witness-content-v1.schema_version")?;
    let content = CanonicalAccountRecoveryWitnessContentV1 {
        account_id: decoder.fixed_bytes("account-recovery-witness-content-v1.account_id")?,
        authority_revision: decoder
            .unsigned("account-recovery-witness-content-v1.authority_revision")?,
        package_id: decoder.fixed_bytes("account-recovery-witness-content-v1.package_id")?,
    };
    validate_witness_content(&content)?;
    Ok(content)
}

fn decode_approval_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalAccountRecoveryApprovalContentV1, CanonicalAccountRecoveryError> {
    decoder.expect_array("account-recovery-approval-content-v1", 12)?;
    decoder.expect_version("account-recovery-approval-content-v1.schema_version")?;
    let content = CanonicalAccountRecoveryApprovalContentV1 {
        account_id: decoder.fixed_bytes("account-recovery-approval-content-v1.account_id")?,
        request_id: decoder.fixed_bytes("account-recovery-approval-content-v1.request_id")?,
        package_id: decoder.fixed_bytes("account-recovery-approval-content-v1.package_id")?,
        authority_revision: decoder
            .unsigned("account-recovery-approval-content-v1.authority_revision")?,
        state_vector_digest: decoder
            .fixed_bytes("account-recovery-approval-content-v1.state_vector_digest")?,
        recovery_roster_digest: decoder
            .fixed_bytes("account-recovery-approval-content-v1.recovery_roster_digest")?,
        challenge: decoder.fixed_bytes("account-recovery-approval-content-v1.challenge")?,
        request_expires_at_unix_seconds: decoder
            .unsigned("account-recovery-approval-content-v1.request_expires_at_unix_seconds")?,
        approver_device_id: decoder
            .fixed_bytes("account-recovery-approval-content-v1.approver_device_id")?,
        previous_approval_head_digest: decoder
            .fixed_bytes("account-recovery-approval-content-v1.previous_approval_head_digest")?,
        approved_at_unix_seconds: decoder
            .unsigned("account-recovery-approval-content-v1.approved_at_unix_seconds")?,
    };
    validate_approval_content(&content)?;
    Ok(content)
}

fn validate_package_content(
    content: &CanonicalAccountRecoveryPackageContentV1,
) -> Result<(), CanonicalAccountRecoveryError> {
    if content.account_id == [0; 32]
        || content.account_id != content.authority_snapshot.content.account_id
        || content.account_id
            != content
                .device_list
                .content
                .authority_snapshot
                .content
                .account_id
        || content.authority_snapshot != content.device_list.content.authority_snapshot
        || content.conversation_memberships.len() > MAX_CANONICAL_ACCOUNT_RECOVERY_MEMBERSHIPS
        || content
            .conversation_memberships
            .windows(2)
            .any(|pair| pair[0].content.conversation_id >= pair[1].content.conversation_id)
        || content
            .conversation_memberships
            .iter()
            .any(|membership| membership.content.owner_account_id != content.account_id)
    {
        return Err(CanonicalAccountRecoveryError::InvalidPackage);
    }
    Ok(())
}

fn validate_witness_content(
    content: &CanonicalAccountRecoveryWitnessContentV1,
) -> Result<(), CanonicalAccountRecoveryError> {
    if content.account_id == [0; 32]
        || content.authority_revision == 0
        || content.package_id == [0; 32]
    {
        return Err(CanonicalAccountRecoveryError::InvalidWitness);
    }
    Ok(())
}

fn validate_request(
    request: &CanonicalAccountRecoveryApprovalRequestV1,
) -> Result<(), CanonicalAccountRecoveryError> {
    verify_signed_account_recovery_package_v1(&request.package)?;
    verify_account_recovery_witness_for_package_v1(&request.witness, &request.package)?;
    let validity = request
        .expires_at_unix_seconds
        .checked_sub(request.issued_at_unix_seconds)
        .ok_or(CanonicalAccountRecoveryError::InvalidRequest)?;
    if request.challenge == [0; 32]
        || !(1..=MAX_CANONICAL_ACCOUNT_RECOVERY_VALIDITY_SECONDS).contains(&validity)
    {
        return Err(CanonicalAccountRecoveryError::InvalidRequest);
    }
    Ok(())
}

fn validate_approval_content(
    content: &CanonicalAccountRecoveryApprovalContentV1,
) -> Result<(), CanonicalAccountRecoveryError> {
    if content.account_id == [0; 32]
        || content.request_id == [0; 32]
        || content.package_id == [0; 32]
        || content.authority_revision == 0
        || content.state_vector_digest == [0; 32]
        || content.recovery_roster_digest == [0; 32]
        || content.challenge == [0; 32]
        || content.request_expires_at_unix_seconds == 0
        || content.approver_device_id == [0; 32]
        || content.approved_at_unix_seconds == 0
    {
        return Err(CanonicalAccountRecoveryError::InvalidApproval);
    }
    Ok(())
}

fn ensure_nested_bound(
    bytes: &[u8],
    object: &'static str,
) -> Result<(), CanonicalAccountRecoveryError> {
    ensure_bound(bytes, object, MAX_CANONICAL_ACCOUNT_RECOVERY_NESTED_BYTES)
}

fn ensure_bound(
    bytes: &[u8],
    object: &'static str,
    maximum: usize,
) -> Result<(), CanonicalAccountRecoveryError> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(CanonicalAccountRecoveryError::ObjectTooLarge { object, maximum });
    }
    Ok(())
}

fn verify_ed25519(
    key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
) -> Result<(), CanonicalAccountRecoveryError> {
    let key = VerifyingKey::from_bytes(key)
        .map_err(|_| CanonicalAccountRecoveryError::InvalidPublicKey)?;
    let signature = Signature::try_from(signature.as_slice())
        .map_err(|_| CanonicalAccountRecoveryError::InvalidSignature)?;
    key.verify_strict(message, &signature)
        .map_err(|_| CanonicalAccountRecoveryError::InvalidSignature)
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

    fn vectors() -> TestResult<Value> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-ACCOUNT-RECOVERY-VECTORS.json"
        )))?)
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
    fn canonical_account_recovery_vectors_round_trip_and_reach_majority() -> TestResult {
        let vectors = vectors()?;
        let package = decode_and_verify_signed_account_recovery_package_v1(&hex_at(
            &vectors,
            "/expected/package/signed_cbor_hex",
        )?)?;
        let witness = decode_and_verify_signed_account_recovery_witness_v1(&hex_at(
            &vectors,
            "/expected/witness/signed_cbor_hex",
        )?)?;
        let request = decode_and_verify_account_recovery_approval_request_v1(&hex_at(
            &vectors,
            "/expected/request/cbor_hex",
        )?)?;
        let approval = decode_and_verify_signed_account_recovery_approval_v1(&hex_at(
            &vectors,
            "/expected/approval/signed_cbor_hex",
        )?)?;
        verify_account_recovery_witness_for_package_v1(&witness, &package)?;
        assert_eq!(request.package, package);
        assert_eq!(request.witness, witness);
        assert_eq!(
            account_recovery_package_id_v1(&package)?,
            fixed::<32>(&vectors, "/expected/package/object_id_hex")?
        );
        assert_eq!(
            account_recovery_state_vector_digest_v1(&package)?,
            fixed::<32>(&vectors, "/expected/state_vector/digest_hex")?
        );
        assert_eq!(
            account_recovery_roster_digest_v1(&package)?,
            fixed::<32>(&vectors, "/expected/roster/digest_hex")?
        );
        assert_eq!(
            account_recovery_request_id_v1(&request)?,
            fixed::<32>(&vectors, "/expected/request/object_id_hex")?
        );
        assert_eq!(
            account_recovery_approval_id_v1(&approval)?,
            fixed::<32>(&vectors, "/expected/approval/object_id_hex")?
        );
        let now = request.issued_at_unix_seconds + 1;
        let report = verify_account_recovery_quorum_v1(&request, &[approval], now)?;
        assert_eq!(
            report.claim,
            CanonicalAccountRecoveryFreshnessClaimV1::CurrentDeviceMajorityObserved
        );
        assert_eq!(report.observed_approvals, 1);
        assert_eq!(report.required_approvals, 1);
        Ok(())
    }

    #[test]
    fn canonical_account_recovery_substitutions_fail_closed() -> TestResult {
        let vectors = vectors()?;
        let request = decode_and_verify_account_recovery_approval_request_v1(&hex_at(
            &vectors,
            "/expected/request/cbor_hex",
        )?)?;
        let approval = decode_and_verify_signed_account_recovery_approval_v1(&hex_at(
            &vectors,
            "/expected/approval/signed_cbor_hex",
        )?)?;
        assert!(
            verify_account_recovery_approval_request_at_v1(
                &request,
                request.expires_at_unix_seconds + 1,
            )
            .is_err()
        );
        let mut wrong_request = request.clone();
        wrong_request.challenge[0] ^= 1;
        assert!(
            verify_account_recovery_approval_for_request_v1(
                &approval,
                &wrong_request,
                request.issued_at_unix_seconds + 1,
            )
            .is_err()
        );
        let mut changed_approval = approval;
        changed_approval.content.previous_approval_head_digest[0] ^= 1;
        assert!(verify_signed_account_recovery_approval_v1(&changed_approval).is_err());
        Ok(())
    }
}
