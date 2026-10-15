//! Inactive deterministic-CBOR Account recovery policy-transition candidate.
//!
//! This module completes the portable Account recovery surface started by
//! `canonical_account_recovery_v1`. It deliberately does not encode the local
//! DB-authenticated policy anchor: a portable transition certificate is valid
//! only after the caller checks its old epoch, old roster digest and previous
//! transition ID against that local monotonic anchor.

use std::collections::BTreeSet;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use thiserror::Error;

use crate::{
    canonical_account_recovery_v1::{
        CANONICAL_ACCOUNT_RECOVERY_CLOCK_SKEW_SECONDS, CanonicalAccountRecoveryError,
        CanonicalSignedAccountRecoveryPackageV1, CanonicalSignedAccountRecoveryWitnessV1,
        MAX_CANONICAL_ACCOUNT_RECOVERY_APPROVAL_BYTES, MAX_CANONICAL_ACCOUNT_RECOVERY_APPROVALS,
        MAX_CANONICAL_ACCOUNT_RECOVERY_PACKAGE_BYTES,
        MAX_CANONICAL_ACCOUNT_RECOVERY_VALIDITY_SECONDS,
        MAX_CANONICAL_ACCOUNT_RECOVERY_WITNESS_BYTES, account_recovery_roster_digest_v1,
        decode_and_verify_signed_account_recovery_package_v1,
        decode_and_verify_signed_account_recovery_witness_v1,
        encode_signed_account_recovery_package_v1, encode_signed_account_recovery_witness_v1,
        verify_account_recovery_witness_for_package_v1, verify_signed_account_recovery_package_v1,
    },
    canonical_account_v1::{AccountDecoder, CanonicalAccountError, signed_envelope},
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CanonicalWireError, prefixed, push_array_header, push_bytes,
        push_unsigned, validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_ACCOUNT_RECOVERY_POLICY_REQUEST_BYTES: usize = 2
    * (MAX_CANONICAL_ACCOUNT_RECOVERY_PACKAGE_BYTES + MAX_CANONICAL_ACCOUNT_RECOVERY_WITNESS_BYTES)
    + 64 * 1024;
pub const MAX_CANONICAL_ACCOUNT_RECOVERY_POLICY_APPROVAL_BYTES: usize =
    MAX_CANONICAL_ACCOUNT_RECOVERY_APPROVAL_BYTES;
pub const MAX_CANONICAL_ACCOUNT_RECOVERY_POLICY_APPROVALS: usize =
    MAX_CANONICAL_ACCOUNT_RECOVERY_APPROVALS * 2;
pub const MAX_CANONICAL_ACCOUNT_RECOVERY_POLICY_CERTIFICATE_BYTES: usize =
    MAX_CANONICAL_ACCOUNT_RECOVERY_POLICY_REQUEST_BYTES
        + MAX_CANONICAL_ACCOUNT_RECOVERY_POLICY_APPROVALS
            * MAX_CANONICAL_ACCOUNT_RECOVERY_POLICY_APPROVAL_BYTES
        + 64 * 1024;

pub const CANONICAL_ACCOUNT_RECOVERY_POLICY_REQUEST_ID_DOMAIN: &[u8] =
    b"kilogram:account-recovery-policy-transition-request-id:cbor-v1\0";
pub const CANONICAL_ACCOUNT_RECOVERY_POLICY_APPROVAL_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:account-recovery-policy-transition-approval-signature:cbor-v1\0";
pub const CANONICAL_ACCOUNT_RECOVERY_POLICY_APPROVAL_ID_DOMAIN: &[u8] =
    b"kilogram:account-recovery-policy-transition-approval-id:cbor-v1\0";
pub const CANONICAL_ACCOUNT_RECOVERY_POLICY_CERTIFICATE_ID_DOMAIN: &[u8] =
    b"kilogram:account-recovery-policy-transition-certificate-id:cbor-v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalAccountRecoveryPolicyTransitionRequestV1 {
    pub old_epoch: u64,
    pub previous_transition_id: [u8; 32],
    pub old_package: CanonicalSignedAccountRecoveryPackageV1,
    pub old_witness: CanonicalSignedAccountRecoveryWitnessV1,
    pub new_package: CanonicalSignedAccountRecoveryPackageV1,
    pub new_witness: CanonicalSignedAccountRecoveryWitnessV1,
    pub challenge: [u8; 32],
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalAccountRecoveryPolicyTransitionApprovalContentV1 {
    pub account_id: [u8; 32],
    pub request_id: [u8; 32],
    pub old_epoch: u64,
    pub new_epoch: u64,
    pub previous_transition_id: [u8; 32],
    pub old_recovery_roster_digest: [u8; 32],
    pub new_recovery_roster_digest: [u8; 32],
    pub challenge: [u8; 32],
    pub request_expires_at_unix_seconds: u64,
    pub approver_device_id: [u8; 32],
    pub previous_transition_approval_id: [u8; 32],
    pub approved_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedAccountRecoveryPolicyTransitionApprovalV1 {
    pub content: CanonicalAccountRecoveryPolicyTransitionApprovalContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalAccountRecoveryPolicyTransitionCertificateV1 {
    pub request: CanonicalAccountRecoveryPolicyTransitionRequestV1,
    pub approvals: Vec<CanonicalSignedAccountRecoveryPolicyTransitionApprovalV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalAccountRecoveryPolicyTransitionReportV1 {
    pub account_id: [u8; 32],
    pub request_id: [u8; 32],
    pub old_epoch: u64,
    pub new_epoch: u64,
    pub old_recovery_roster_digest: [u8; 32],
    pub new_recovery_roster_digest: [u8; 32],
    pub old_roster_count: usize,
    pub old_observed_approvals: usize,
    pub old_required_approvals: usize,
    pub new_roster_count: usize,
    pub new_observed_approvals: usize,
    pub new_required_approvals: usize,
}

impl CanonicalAccountRecoveryPolicyTransitionReportV1 {
    pub fn joint_majority_satisfied(&self) -> bool {
        self.old_observed_approvals >= self.old_required_approvals
            && self.new_observed_approvals >= self.new_required_approvals
    }

    pub fn require_joint_majority(&self) -> Result<(), CanonicalAccountRecoveryPolicyError> {
        if self.joint_majority_satisfied() {
            Ok(())
        } else {
            Err(
                CanonicalAccountRecoveryPolicyError::InsufficientJointQuorum {
                    old_observed: self.old_observed_approvals,
                    old_required: self.old_required_approvals,
                    new_observed: self.new_observed_approvals,
                    new_required: self.new_required_approvals,
                },
            )
        }
    }
}

#[derive(Debug, Error)]
pub enum CanonicalAccountRecoveryPolicyError {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),
    #[error(transparent)]
    Account(#[from] CanonicalAccountError),
    #[error(transparent)]
    Recovery(#[from] CanonicalAccountRecoveryError),
    #[error("canonical Account recovery policy {object} exceeds {maximum} bytes")]
    ObjectTooLarge {
        object: &'static str,
        maximum: usize,
    },
    #[error("canonical Account recovery policy transition request is invalid")]
    InvalidRequest,
    #[error("canonical Account recovery policy transition request is not current")]
    RequestNotCurrent,
    #[error("canonical Account recovery policy successor is not monotonic")]
    StateRollback,
    #[error("canonical Account recovery policy transition did not change the roster")]
    RosterUnchanged,
    #[error("canonical Account recovery policy transition approval is invalid")]
    InvalidApproval,
    #[error("canonical Account recovery policy transition approval time is invalid")]
    ApprovalTimeInvalid,
    #[error("canonical Account recovery policy approvals must be strictly ordered by Device ID")]
    NonCanonicalApprovals,
    #[error("canonical Account recovery policy transition certificate is invalid")]
    InvalidCertificate,
    #[error(
        "canonical Account recovery policy joint quorum is insufficient: old {old_observed}/{old_required}, new {new_observed}/{new_required}"
    )]
    InsufficientJointQuorum {
        old_observed: usize,
        old_required: usize,
        new_observed: usize,
        new_required: usize,
    },
    #[error("canonical Account recovery policy contains an invalid Ed25519 key")]
    InvalidPublicKey,
    #[error("canonical Account recovery policy signature is invalid")]
    InvalidSignature,
}

pub fn encode_account_recovery_policy_transition_request_v1(
    request: &CanonicalAccountRecoveryPolicyTransitionRequestV1,
) -> Result<Vec<u8>, CanonicalAccountRecoveryPolicyError> {
    verify_account_recovery_policy_transition_request_static_v1(request)?;
    let old_package = encode_signed_account_recovery_package_v1(&request.old_package)?;
    let old_witness = encode_signed_account_recovery_witness_v1(&request.old_witness)?;
    let new_package = encode_signed_account_recovery_package_v1(&request.new_package)?;
    let new_witness = encode_signed_account_recovery_witness_v1(&request.new_witness)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 10);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_unsigned(&mut output, request.old_epoch);
    push_bytes(&mut output, &request.previous_transition_id);
    push_bytes(&mut output, &old_package);
    push_bytes(&mut output, &old_witness);
    push_bytes(&mut output, &new_package);
    push_bytes(&mut output, &new_witness);
    push_bytes(&mut output, &request.challenge);
    push_unsigned(&mut output, request.issued_at_unix_seconds);
    push_unsigned(&mut output, request.expires_at_unix_seconds);
    ensure_bound(
        &output,
        "transition request",
        MAX_CANONICAL_ACCOUNT_RECOVERY_POLICY_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_account_recovery_policy_transition_request_v1(
    bytes: &[u8],
) -> Result<CanonicalAccountRecoveryPolicyTransitionRequestV1, CanonicalAccountRecoveryPolicyError>
{
    ensure_bound(
        bytes,
        "transition request",
        MAX_CANONICAL_ACCOUNT_RECOVERY_POLICY_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("account-recovery-policy-transition-request-v1", 10)?;
    decoder.expect_version("account-recovery-policy-transition-request-v1.schema_version")?;
    let old_epoch = decoder.unsigned("account-recovery-policy-transition-request-v1.old_epoch")?;
    let previous_transition_id = decoder
        .fixed_bytes("account-recovery-policy-transition-request-v1.previous_transition_id")?;
    let old_package = decoder.bytes("account-recovery-policy-transition-request-v1.old_package")?;
    let old_witness = decoder.bytes("account-recovery-policy-transition-request-v1.old_witness")?;
    let new_package = decoder.bytes("account-recovery-policy-transition-request-v1.new_package")?;
    let new_witness = decoder.bytes("account-recovery-policy-transition-request-v1.new_witness")?;
    let challenge =
        decoder.fixed_bytes("account-recovery-policy-transition-request-v1.challenge")?;
    let issued_at_unix_seconds =
        decoder.unsigned("account-recovery-policy-transition-request-v1.issued_at_unix_seconds")?;
    let expires_at_unix_seconds = decoder
        .unsigned("account-recovery-policy-transition-request-v1.expires_at_unix_seconds")?;
    decoder.finish()?;
    let request = CanonicalAccountRecoveryPolicyTransitionRequestV1 {
        old_epoch,
        previous_transition_id,
        old_package: decode_and_verify_signed_account_recovery_package_v1(old_package)?,
        old_witness: decode_and_verify_signed_account_recovery_witness_v1(old_witness)?,
        new_package: decode_and_verify_signed_account_recovery_package_v1(new_package)?,
        new_witness: decode_and_verify_signed_account_recovery_witness_v1(new_witness)?,
        challenge,
        issued_at_unix_seconds,
        expires_at_unix_seconds,
    };
    verify_account_recovery_policy_transition_request_static_v1(&request)?;
    Ok(request)
}

pub fn verify_account_recovery_policy_transition_request_static_v1(
    request: &CanonicalAccountRecoveryPolicyTransitionRequestV1,
) -> Result<(), CanonicalAccountRecoveryPolicyError> {
    verify_account_recovery_witness_for_package_v1(&request.old_witness, &request.old_package)?;
    verify_account_recovery_witness_for_package_v1(&request.new_witness, &request.new_package)?;
    if request.old_epoch == u64::MAX
        || (request.old_epoch == 0 && request.previous_transition_id != [0; 32])
        || (request.old_epoch > 0 && request.previous_transition_id == [0; 32])
        || request.challenge == [0; 32]
    {
        return Err(CanonicalAccountRecoveryPolicyError::InvalidRequest);
    }
    let validity = request
        .expires_at_unix_seconds
        .checked_sub(request.issued_at_unix_seconds)
        .ok_or(CanonicalAccountRecoveryPolicyError::InvalidRequest)?;
    if !(1..=MAX_CANONICAL_ACCOUNT_RECOVERY_VALIDITY_SECONDS).contains(&validity) {
        return Err(CanonicalAccountRecoveryPolicyError::InvalidRequest);
    }
    verify_account_recovery_package_successor_v1(&request.old_package, &request.new_package)
}

pub fn verify_account_recovery_policy_transition_request_at_v1(
    request: &CanonicalAccountRecoveryPolicyTransitionRequestV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalAccountRecoveryPolicyError> {
    verify_account_recovery_policy_transition_request_static_v1(request)?;
    if request.issued_at_unix_seconds
        > now_unix_seconds.saturating_add(CANONICAL_ACCOUNT_RECOVERY_CLOCK_SKEW_SECONDS)
        || request.expires_at_unix_seconds < now_unix_seconds
    {
        return Err(CanonicalAccountRecoveryPolicyError::RequestNotCurrent);
    }
    Ok(())
}

pub fn account_recovery_policy_transition_request_id_v1(
    request: &CanonicalAccountRecoveryPolicyTransitionRequestV1,
) -> Result<[u8; 32], CanonicalAccountRecoveryPolicyError> {
    Ok(domain_hash(
        CANONICAL_ACCOUNT_RECOVERY_POLICY_REQUEST_ID_DOMAIN,
        &encode_account_recovery_policy_transition_request_v1(request)?,
    ))
}

pub fn verify_account_recovery_policy_transition_anchor_v1(
    request: &CanonicalAccountRecoveryPolicyTransitionRequestV1,
    expected_account_id: [u8; 32],
    expected_old_epoch: u64,
    expected_old_recovery_roster_digest: [u8; 32],
    expected_previous_transition_id: [u8; 32],
) -> Result<(), CanonicalAccountRecoveryPolicyError> {
    verify_account_recovery_policy_transition_request_static_v1(request)?;
    if request.old_package.content.account_id != expected_account_id
        || request.old_epoch != expected_old_epoch
        || account_recovery_roster_digest_v1(&request.old_package)?
            != expected_old_recovery_roster_digest
        || request.previous_transition_id != expected_previous_transition_id
    {
        return Err(CanonicalAccountRecoveryPolicyError::InvalidRequest);
    }
    Ok(())
}

pub fn encode_account_recovery_policy_transition_approval_content_v1(
    content: &CanonicalAccountRecoveryPolicyTransitionApprovalContentV1,
) -> Result<Vec<u8>, CanonicalAccountRecoveryPolicyError> {
    validate_approval_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 13);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.account_id);
    push_bytes(&mut output, &content.request_id);
    push_unsigned(&mut output, content.old_epoch);
    push_unsigned(&mut output, content.new_epoch);
    push_bytes(&mut output, &content.previous_transition_id);
    push_bytes(&mut output, &content.old_recovery_roster_digest);
    push_bytes(&mut output, &content.new_recovery_roster_digest);
    push_bytes(&mut output, &content.challenge);
    push_unsigned(&mut output, content.request_expires_at_unix_seconds);
    push_bytes(&mut output, &content.approver_device_id);
    push_bytes(&mut output, &content.previous_transition_approval_id);
    push_unsigned(&mut output, content.approved_at_unix_seconds);
    Ok(output)
}

pub fn account_recovery_policy_transition_approval_signing_bytes_v1(
    content: &CanonicalAccountRecoveryPolicyTransitionApprovalContentV1,
) -> Result<Vec<u8>, CanonicalAccountRecoveryPolicyError> {
    Ok(prefixed(
        CANONICAL_ACCOUNT_RECOVERY_POLICY_APPROVAL_SIGNATURE_DOMAIN,
        &encode_account_recovery_policy_transition_approval_content_v1(content)?,
    ))
}

pub fn sign_account_recovery_policy_transition_approval_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalAccountRecoveryPolicyTransitionRequestV1,
    previous_transition_approval_id: [u8; 32],
    approved_at_unix_seconds: u64,
) -> Result<
    CanonicalSignedAccountRecoveryPolicyTransitionApprovalV1,
    CanonicalAccountRecoveryPolicyError,
> {
    verify_account_recovery_policy_transition_request_at_v1(request, approved_at_unix_seconds)?;
    let approver_device_id = device_signing_key.verifying_key().to_bytes();
    if !is_active_voter(&request.old_package, approver_device_id)
        && !is_active_voter(&request.new_package, approver_device_id)
    {
        return Err(CanonicalAccountRecoveryPolicyError::InvalidApproval);
    }
    let content = CanonicalAccountRecoveryPolicyTransitionApprovalContentV1 {
        account_id: request.old_package.content.account_id,
        request_id: account_recovery_policy_transition_request_id_v1(request)?,
        old_epoch: request.old_epoch,
        new_epoch: request.old_epoch + 1,
        previous_transition_id: request.previous_transition_id,
        old_recovery_roster_digest: account_recovery_roster_digest_v1(&request.old_package)?,
        new_recovery_roster_digest: account_recovery_roster_digest_v1(&request.new_package)?,
        challenge: request.challenge,
        request_expires_at_unix_seconds: request.expires_at_unix_seconds,
        approver_device_id,
        previous_transition_approval_id,
        approved_at_unix_seconds,
    };
    let signature = device_signing_key
        .sign(&account_recovery_policy_transition_approval_signing_bytes_v1(&content)?)
        .to_bytes();
    let approval = CanonicalSignedAccountRecoveryPolicyTransitionApprovalV1 { content, signature };
    verify_account_recovery_policy_transition_approval_for_request_v1(&approval, request)?;
    Ok(approval)
}

pub fn encode_signed_account_recovery_policy_transition_approval_v1(
    approval: &CanonicalSignedAccountRecoveryPolicyTransitionApprovalV1,
) -> Result<Vec<u8>, CanonicalAccountRecoveryPolicyError> {
    verify_signed_account_recovery_policy_transition_approval_v1(approval)?;
    let encoded = signed_envelope(
        &encode_account_recovery_policy_transition_approval_content_v1(&approval.content)?,
        &approval.signature,
    );
    ensure_bound(
        &encoded,
        "transition approval",
        MAX_CANONICAL_ACCOUNT_RECOVERY_POLICY_APPROVAL_BYTES,
    )?;
    Ok(encoded)
}

pub fn decode_and_verify_signed_account_recovery_policy_transition_approval_v1(
    bytes: &[u8],
) -> Result<
    CanonicalSignedAccountRecoveryPolicyTransitionApprovalV1,
    CanonicalAccountRecoveryPolicyError,
> {
    ensure_bound(
        bytes,
        "transition approval",
        MAX_CANONICAL_ACCOUNT_RECOVERY_POLICY_APPROVAL_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-account-recovery-policy-transition-approval-v1", 3)?;
    decoder
        .expect_version("signed-account-recovery-policy-transition-approval-v1.schema_version")?;
    let content = decode_approval_content_from(&mut decoder)?;
    let signature =
        decoder.fixed_bytes("signed-account-recovery-policy-transition-approval-v1.signature")?;
    decoder.finish()?;
    let approval = CanonicalSignedAccountRecoveryPolicyTransitionApprovalV1 { content, signature };
    verify_signed_account_recovery_policy_transition_approval_v1(&approval)?;
    Ok(approval)
}

pub fn verify_signed_account_recovery_policy_transition_approval_v1(
    approval: &CanonicalSignedAccountRecoveryPolicyTransitionApprovalV1,
) -> Result<(), CanonicalAccountRecoveryPolicyError> {
    validate_approval_content(&approval.content)?;
    verify_ed25519(
        &approval.content.approver_device_id,
        &account_recovery_policy_transition_approval_signing_bytes_v1(&approval.content)?,
        &approval.signature,
    )
}

pub fn account_recovery_policy_transition_approval_id_v1(
    approval: &CanonicalSignedAccountRecoveryPolicyTransitionApprovalV1,
) -> Result<[u8; 32], CanonicalAccountRecoveryPolicyError> {
    Ok(domain_hash(
        CANONICAL_ACCOUNT_RECOVERY_POLICY_APPROVAL_ID_DOMAIN,
        &encode_signed_account_recovery_policy_transition_approval_v1(approval)?,
    ))
}

pub fn verify_account_recovery_policy_transition_approval_for_request_v1(
    approval: &CanonicalSignedAccountRecoveryPolicyTransitionApprovalV1,
    request: &CanonicalAccountRecoveryPolicyTransitionRequestV1,
) -> Result<(), CanonicalAccountRecoveryPolicyError> {
    verify_signed_account_recovery_policy_transition_approval_v1(approval)?;
    verify_account_recovery_policy_transition_request_static_v1(request)?;
    let expected_new_epoch = request
        .old_epoch
        .checked_add(1)
        .ok_or(CanonicalAccountRecoveryPolicyError::InvalidRequest)?;
    if approval.content.account_id != request.old_package.content.account_id
        || approval.content.request_id != account_recovery_policy_transition_request_id_v1(request)?
        || approval.content.old_epoch != request.old_epoch
        || approval.content.new_epoch != expected_new_epoch
        || approval.content.previous_transition_id != request.previous_transition_id
        || approval.content.old_recovery_roster_digest
            != account_recovery_roster_digest_v1(&request.old_package)?
        || approval.content.new_recovery_roster_digest
            != account_recovery_roster_digest_v1(&request.new_package)?
        || approval.content.challenge != request.challenge
        || approval.content.request_expires_at_unix_seconds != request.expires_at_unix_seconds
    {
        return Err(CanonicalAccountRecoveryPolicyError::InvalidApproval);
    }
    if approval.content.approved_at_unix_seconds
        < request
            .issued_at_unix_seconds
            .saturating_sub(CANONICAL_ACCOUNT_RECOVERY_CLOCK_SKEW_SECONDS)
        || approval.content.approved_at_unix_seconds > request.expires_at_unix_seconds
    {
        return Err(CanonicalAccountRecoveryPolicyError::ApprovalTimeInvalid);
    }
    if !is_active_voter(&request.old_package, approval.content.approver_device_id)
        && !is_active_voter(&request.new_package, approval.content.approver_device_id)
    {
        return Err(CanonicalAccountRecoveryPolicyError::InvalidApproval);
    }
    Ok(())
}

pub fn verify_account_recovery_policy_transition_v1(
    request: &CanonicalAccountRecoveryPolicyTransitionRequestV1,
    approvals: &[CanonicalSignedAccountRecoveryPolicyTransitionApprovalV1],
) -> Result<CanonicalAccountRecoveryPolicyTransitionReportV1, CanonicalAccountRecoveryPolicyError> {
    verify_account_recovery_policy_transition_request_static_v1(request)?;
    if approvals.len() > MAX_CANONICAL_ACCOUNT_RECOVERY_POLICY_APPROVALS {
        return Err(CanonicalAccountRecoveryPolicyError::InvalidApproval);
    }
    if approvals
        .windows(2)
        .any(|pair| pair[0].content.approver_device_id >= pair[1].content.approver_device_id)
    {
        return Err(CanonicalAccountRecoveryPolicyError::NonCanonicalApprovals);
    }
    let mut distinct = BTreeSet::new();
    let mut old_observed_approvals = 0;
    let mut new_observed_approvals = 0;
    for approval in approvals {
        verify_account_recovery_policy_transition_approval_for_request_v1(approval, request)?;
        if !distinct.insert(approval.content.approver_device_id) {
            return Err(CanonicalAccountRecoveryPolicyError::NonCanonicalApprovals);
        }
        old_observed_approvals += usize::from(is_active_voter(
            &request.old_package,
            approval.content.approver_device_id,
        ));
        new_observed_approvals += usize::from(is_active_voter(
            &request.new_package,
            approval.content.approver_device_id,
        ));
    }
    let old_roster_count = request
        .old_package
        .content
        .device_list
        .content
        .devices
        .len();
    let new_roster_count = request
        .new_package
        .content
        .device_list
        .content
        .devices
        .len();
    Ok(CanonicalAccountRecoveryPolicyTransitionReportV1 {
        account_id: request.old_package.content.account_id,
        request_id: account_recovery_policy_transition_request_id_v1(request)?,
        old_epoch: request.old_epoch,
        new_epoch: request.old_epoch + 1,
        old_recovery_roster_digest: account_recovery_roster_digest_v1(&request.old_package)?,
        new_recovery_roster_digest: account_recovery_roster_digest_v1(&request.new_package)?,
        old_roster_count,
        old_observed_approvals,
        old_required_approvals: old_roster_count / 2 + 1,
        new_roster_count,
        new_observed_approvals,
        new_required_approvals: new_roster_count / 2 + 1,
    })
}

pub fn build_account_recovery_policy_transition_certificate_v1(
    request: CanonicalAccountRecoveryPolicyTransitionRequestV1,
    mut approvals: Vec<CanonicalSignedAccountRecoveryPolicyTransitionApprovalV1>,
    now_unix_seconds: u64,
) -> Result<
    CanonicalAccountRecoveryPolicyTransitionCertificateV1,
    CanonicalAccountRecoveryPolicyError,
> {
    verify_account_recovery_policy_transition_request_at_v1(&request, now_unix_seconds)?;
    approvals.sort_by_key(|approval| approval.content.approver_device_id);
    let certificate = CanonicalAccountRecoveryPolicyTransitionCertificateV1 { request, approvals };
    verify_account_recovery_policy_transition_certificate_v1(&certificate)?;
    Ok(certificate)
}

pub fn encode_account_recovery_policy_transition_certificate_v1(
    certificate: &CanonicalAccountRecoveryPolicyTransitionCertificateV1,
) -> Result<Vec<u8>, CanonicalAccountRecoveryPolicyError> {
    verify_account_recovery_policy_transition_certificate_v1(certificate)?;
    let request = encode_account_recovery_policy_transition_request_v1(&certificate.request)?;
    let mut approvals = Vec::with_capacity(certificate.approvals.len());
    for approval in &certificate.approvals {
        approvals.push(encode_signed_account_recovery_policy_transition_approval_v1(approval)?);
    }
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &request);
    push_array_header(&mut output, approvals.len() as u64);
    for approval in approvals {
        push_bytes(&mut output, &approval);
    }
    ensure_bound(
        &output,
        "transition certificate",
        MAX_CANONICAL_ACCOUNT_RECOVERY_POLICY_CERTIFICATE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_account_recovery_policy_transition_certificate_v1(
    bytes: &[u8],
) -> Result<
    CanonicalAccountRecoveryPolicyTransitionCertificateV1,
    CanonicalAccountRecoveryPolicyError,
> {
    ensure_bound(
        bytes,
        "transition certificate",
        MAX_CANONICAL_ACCOUNT_RECOVERY_POLICY_CERTIFICATE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("account-recovery-policy-transition-certificate-v1", 3)?;
    decoder.expect_version("account-recovery-policy-transition-certificate-v1.schema_version")?;
    let request = decoder.bytes("account-recovery-policy-transition-certificate-v1.request")?;
    let approval_count =
        decoder.array("account-recovery-policy-transition-certificate-v1.approvals")?;
    if approval_count > MAX_CANONICAL_ACCOUNT_RECOVERY_POLICY_APPROVALS {
        return Err(CanonicalAccountRecoveryPolicyError::InvalidCertificate);
    }
    let mut approvals = Vec::with_capacity(approval_count);
    for _ in 0..approval_count {
        let approval =
            decoder.bytes("account-recovery-policy-transition-certificate-v1.approval")?;
        approvals.push(
            decode_and_verify_signed_account_recovery_policy_transition_approval_v1(approval)?,
        );
    }
    decoder.finish()?;
    let certificate = CanonicalAccountRecoveryPolicyTransitionCertificateV1 {
        request: decode_and_verify_account_recovery_policy_transition_request_v1(request)?,
        approvals,
    };
    verify_account_recovery_policy_transition_certificate_v1(&certificate)?;
    Ok(certificate)
}

pub fn verify_account_recovery_policy_transition_certificate_v1(
    certificate: &CanonicalAccountRecoveryPolicyTransitionCertificateV1,
) -> Result<(), CanonicalAccountRecoveryPolicyError> {
    if certificate.approvals.is_empty() {
        return Err(CanonicalAccountRecoveryPolicyError::InvalidCertificate);
    }
    verify_account_recovery_policy_transition_v1(&certificate.request, &certificate.approvals)?
        .require_joint_majority()
}

pub fn account_recovery_policy_transition_certificate_id_v1(
    certificate: &CanonicalAccountRecoveryPolicyTransitionCertificateV1,
) -> Result<[u8; 32], CanonicalAccountRecoveryPolicyError> {
    Ok(domain_hash(
        CANONICAL_ACCOUNT_RECOVERY_POLICY_CERTIFICATE_ID_DOMAIN,
        &encode_account_recovery_policy_transition_certificate_v1(certificate)?,
    ))
}

pub fn verify_account_recovery_package_successor_v1(
    old: &CanonicalSignedAccountRecoveryPackageV1,
    new: &CanonicalSignedAccountRecoveryPackageV1,
) -> Result<(), CanonicalAccountRecoveryPolicyError> {
    verify_signed_account_recovery_package_v1(old)?;
    verify_signed_account_recovery_package_v1(new)?;
    if old.content.account_id != new.content.account_id
        || new.content.authority_snapshot.content.revision
            <= old.content.authority_snapshot.content.revision
    {
        return Err(CanonicalAccountRecoveryPolicyError::StateRollback);
    }
    if old
        .content
        .authority_snapshot
        .content
        .revocations
        .iter()
        .any(|old_revocation| {
            !new.content
                .authority_snapshot
                .content
                .revocations
                .contains(old_revocation)
        })
    {
        return Err(CanonicalAccountRecoveryPolicyError::StateRollback);
    }
    for old_membership in &old.content.conversation_memberships {
        let Some(new_membership) = new
            .content
            .conversation_memberships
            .iter()
            .find(|candidate| {
                candidate.content.conversation_id == old_membership.content.conversation_id
            })
        else {
            return Err(CanonicalAccountRecoveryPolicyError::StateRollback);
        };
        if new_membership.content.revision < old_membership.content.revision
            || (new_membership.content.revision == old_membership.content.revision
                && new_membership != old_membership)
            || (new_membership.content.revision > old_membership.content.revision
                && old_membership
                    .content
                    .members
                    .iter()
                    .any(|member| !new_membership.content.members.contains(member)))
        {
            return Err(CanonicalAccountRecoveryPolicyError::StateRollback);
        }
    }
    for old_certificate in &old.content.device_list.content.devices {
        match new
            .content
            .device_list
            .content
            .devices
            .iter()
            .find(|candidate| candidate.content.device_id == old_certificate.content.device_id)
        {
            Some(new_certificate) if new_certificate == old_certificate => {}
            Some(_) => return Err(CanonicalAccountRecoveryPolicyError::StateRollback),
            None => {
                if !new
                    .content
                    .authority_snapshot
                    .content
                    .revocations
                    .iter()
                    .any(|revocation| {
                        revocation.content.device_id == old_certificate.content.device_id
                    })
                {
                    return Err(CanonicalAccountRecoveryPolicyError::StateRollback);
                }
            }
        }
    }
    if account_recovery_roster_digest_v1(old)? == account_recovery_roster_digest_v1(new)? {
        return Err(CanonicalAccountRecoveryPolicyError::RosterUnchanged);
    }
    Ok(())
}

fn decode_approval_content_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<
    CanonicalAccountRecoveryPolicyTransitionApprovalContentV1,
    CanonicalAccountRecoveryPolicyError,
> {
    decoder.expect_array("account-recovery-policy-transition-approval-content-v1", 13)?;
    decoder
        .expect_version("account-recovery-policy-transition-approval-content-v1.schema_version")?;
    let content = CanonicalAccountRecoveryPolicyTransitionApprovalContentV1 {
        account_id: decoder
            .fixed_bytes("account-recovery-policy-transition-approval-content-v1.account_id")?,
        request_id: decoder
            .fixed_bytes("account-recovery-policy-transition-approval-content-v1.request_id")?,
        old_epoch: decoder
            .unsigned("account-recovery-policy-transition-approval-content-v1.old_epoch")?,
        new_epoch: decoder
            .unsigned("account-recovery-policy-transition-approval-content-v1.new_epoch")?,
        previous_transition_id: decoder.fixed_bytes(
            "account-recovery-policy-transition-approval-content-v1.previous_transition_id",
        )?,
        old_recovery_roster_digest: decoder.fixed_bytes(
            "account-recovery-policy-transition-approval-content-v1.old_recovery_roster_digest",
        )?,
        new_recovery_roster_digest: decoder.fixed_bytes(
            "account-recovery-policy-transition-approval-content-v1.new_recovery_roster_digest",
        )?,
        challenge: decoder
            .fixed_bytes("account-recovery-policy-transition-approval-content-v1.challenge")?,
        request_expires_at_unix_seconds: decoder.unsigned(
            "account-recovery-policy-transition-approval-content-v1.request_expires_at_unix_seconds",
        )?,
        approver_device_id: decoder.fixed_bytes(
            "account-recovery-policy-transition-approval-content-v1.approver_device_id",
        )?,
        previous_transition_approval_id: decoder.fixed_bytes(
            "account-recovery-policy-transition-approval-content-v1.previous_transition_approval_id",
        )?,
        approved_at_unix_seconds: decoder.unsigned(
            "account-recovery-policy-transition-approval-content-v1.approved_at_unix_seconds",
        )?,
    };
    validate_approval_content(&content)?;
    Ok(content)
}

fn validate_approval_content(
    content: &CanonicalAccountRecoveryPolicyTransitionApprovalContentV1,
) -> Result<(), CanonicalAccountRecoveryPolicyError> {
    let expected_new_epoch = content
        .old_epoch
        .checked_add(1)
        .ok_or(CanonicalAccountRecoveryPolicyError::InvalidApproval)?;
    if content.account_id == [0; 32]
        || content.request_id == [0; 32]
        || content.new_epoch != expected_new_epoch
        || (content.old_epoch == 0 && content.previous_transition_id != [0; 32])
        || (content.old_epoch > 0 && content.previous_transition_id == [0; 32])
        || content.old_recovery_roster_digest == [0; 32]
        || content.new_recovery_roster_digest == [0; 32]
        || content.old_recovery_roster_digest == content.new_recovery_roster_digest
        || content.challenge == [0; 32]
        || content.request_expires_at_unix_seconds == 0
        || content.approver_device_id == [0; 32]
        || content.approved_at_unix_seconds == 0
    {
        return Err(CanonicalAccountRecoveryPolicyError::InvalidApproval);
    }
    Ok(())
}

fn is_active_voter(package: &CanonicalSignedAccountRecoveryPackageV1, device_id: [u8; 32]) -> bool {
    package
        .content
        .device_list
        .content
        .devices
        .iter()
        .any(|certificate| certificate.content.device_id == device_id)
}

fn ensure_bound(
    bytes: &[u8],
    object: &'static str,
    maximum: usize,
) -> Result<(), CanonicalAccountRecoveryPolicyError> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(CanonicalAccountRecoveryPolicyError::ObjectTooLarge { object, maximum });
    }
    Ok(())
}

fn verify_ed25519(
    key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
) -> Result<(), CanonicalAccountRecoveryPolicyError> {
    let key = VerifyingKey::from_bytes(key)
        .map_err(|_| CanonicalAccountRecoveryPolicyError::InvalidPublicKey)?;
    let signature = Signature::try_from(signature.as_slice())
        .map_err(|_| CanonicalAccountRecoveryPolicyError::InvalidSignature)?;
    key.verify_strict(message, &signature)
        .map_err(|_| CanonicalAccountRecoveryPolicyError::InvalidSignature)
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
            "/../../CANONICAL-ACCOUNT-RECOVERY-POLICY-VECTORS.json"
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
    fn canonical_policy_transition_vectors_reach_joint_majority() -> TestResult {
        let vectors = vectors()?;
        let request = decode_and_verify_account_recovery_policy_transition_request_v1(&hex_at(
            &vectors,
            "/expected/request/cbor_hex",
        )?)?;
        let first = decode_and_verify_signed_account_recovery_policy_transition_approval_v1(
            &hex_at(&vectors, "/expected/approvals/0/signed_cbor_hex")?,
        )?;
        let second = decode_and_verify_signed_account_recovery_policy_transition_approval_v1(
            &hex_at(&vectors, "/expected/approvals/1/signed_cbor_hex")?,
        )?;
        let certificate = decode_and_verify_account_recovery_policy_transition_certificate_v1(
            &hex_at(&vectors, "/expected/certificate/cbor_hex")?,
        )?;
        assert_eq!(
            account_recovery_policy_transition_request_id_v1(&request)?,
            fixed::<32>(&vectors, "/expected/request/object_id_hex")?
        );
        assert_eq!(
            account_recovery_policy_transition_approval_id_v1(&first)?,
            fixed::<32>(&vectors, "/expected/approvals/0/object_id_hex")?
        );
        assert_eq!(
            account_recovery_policy_transition_approval_id_v1(&second)?,
            fixed::<32>(&vectors, "/expected/approvals/1/object_id_hex")?
        );
        assert_eq!(
            account_recovery_policy_transition_certificate_id_v1(&certificate)?,
            fixed::<32>(&vectors, "/expected/certificate/object_id_hex")?
        );
        let report = verify_account_recovery_policy_transition_v1(
            &request,
            &[first.clone(), second.clone()],
        )?;
        assert_eq!(
            (
                report.old_observed_approvals,
                report.old_required_approvals,
                report.new_observed_approvals,
                report.new_required_approvals,
            ),
            (1, 1, 2, 2)
        );
        assert!(report.joint_majority_satisfied());
        assert_eq!(certificate.request, request);
        assert_eq!(certificate.approvals, vec![first, second]);
        Ok(())
    }

    #[test]
    fn canonical_policy_transition_substitutions_fail_closed() -> TestResult {
        let vectors = vectors()?;
        let request = decode_and_verify_account_recovery_policy_transition_request_v1(&hex_at(
            &vectors,
            "/expected/request/cbor_hex",
        )?)?;
        let first = decode_and_verify_signed_account_recovery_policy_transition_approval_v1(
            &hex_at(&vectors, "/expected/approvals/0/signed_cbor_hex")?,
        )?;
        let second = decode_and_verify_signed_account_recovery_policy_transition_approval_v1(
            &hex_at(&vectors, "/expected/approvals/1/signed_cbor_hex")?,
        )?;
        assert!(
            verify_account_recovery_policy_transition_request_at_v1(
                &request,
                request.expires_at_unix_seconds + 1,
            )
            .is_err()
        );
        let partial =
            verify_account_recovery_policy_transition_v1(&request, std::slice::from_ref(&first))?;
        assert_eq!(
            (
                partial.old_observed_approvals,
                partial.new_observed_approvals
            ),
            (1, 1)
        );
        assert!(partial.require_joint_majority().is_err());
        let new_only = if is_active_voter(&request.old_package, first.content.approver_device_id) {
            second
        } else {
            first
        };
        let report = verify_account_recovery_policy_transition_v1(&request, &[new_only])?;
        assert_eq!(report.old_observed_approvals, 0);
        assert!(report.require_joint_majority().is_err());
        let mut same_roster = request.new_package.clone();
        same_roster.content.device_list = request.old_package.content.device_list.clone();
        assert!(
            verify_account_recovery_package_successor_v1(&request.old_package, &same_roster)
                .is_err()
        );
        Ok(())
    }
}
