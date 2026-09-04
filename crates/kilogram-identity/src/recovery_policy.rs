use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    AccountId, AccountRootRecoveryPackage, AccountRootRecoveryWitness, DeviceCapability, DeviceId,
    DeviceIdentity, IdentityError, MAX_ACCOUNT_DEVICES, MAX_ACCOUNT_ROOT_RECOVERY_PACKAGE_BYTES,
    MAX_ACCOUNT_ROOT_RECOVERY_WITNESS_BYTES, verify_device_authorization_with_snapshot,
};

const POLICY_STATE_MAGIC: &[u8] = b"KILOGRAM-ARPST01";
const TRANSITION_REQUEST_MAGIC: &[u8] = b"KILOGRAM-ARPTQ01";
const TRANSITION_APPROVAL_MAGIC: &[u8] = b"KILOGRAM-ARPTA01";
const TRANSITION_CERTIFICATE_MAGIC: &[u8] = b"KILOGRAM-ARPTC01";
const VERSION: u8 = 1;
const REQUEST_ID_DOMAIN: &str = "Kilogram Account recovery policy transition request ID v1";
const APPROVAL_ID_DOMAIN: &str = "Kilogram Account recovery policy transition approval ID v1";
const CERTIFICATE_ID_DOMAIN: &str = "Kilogram Account recovery policy transition certificate ID v1";
const APPROVAL_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:account-recovery-policy-transition-approval:v1\0";
const CLOCK_SKEW_SECONDS: u64 = 120;

pub const MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_REQUEST_BYTES: usize = 2
    * (MAX_ACCOUNT_ROOT_RECOVERY_PACKAGE_BYTES + MAX_ACCOUNT_ROOT_RECOVERY_WITNESS_BYTES)
    + 64 * 1024;
pub const MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_APPROVAL_BYTES: usize = 16 * 1024;
pub const MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_APPROVALS: usize = MAX_ACCOUNT_DEVICES * 2;
pub const MAX_ACCOUNT_RECOVERY_POLICY_CERTIFICATE_BYTES: usize =
    MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_REQUEST_BYTES
        + MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_APPROVALS
            * MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_APPROVAL_BYTES
        + 64 * 1024;
const MAX_ACCOUNT_RECOVERY_POLICY_STATE_BYTES: usize = 4 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct PolicyStateContent {
    version: u8,
    account_id: AccountId,
    epoch: u64,
    recovery_roster_digest: [u8; 32],
    latest_transition_id: [u8; 32],
}

/// A DB-authenticated local anchor for the currently accepted recovery voter set.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountRecoveryPolicyState {
    content: PolicyStateContent,
}

impl AccountRecoveryPolicyState {
    pub fn genesis(package: &AccountRootRecoveryPackage) -> Result<Self, IdentityError> {
        package.verify()?;
        Ok(Self {
            content: PolicyStateContent {
                version: VERSION,
                account_id: package.account_id(),
                epoch: 0,
                recovery_roster_digest: package.recovery_roster_digest()?,
                latest_transition_id: [0_u8; 32],
            },
        })
    }

    pub fn transitioned(
        certificate: &AccountRecoveryPolicyTransitionCertificate,
    ) -> Result<Self, IdentityError> {
        certificate.verify()?;
        Ok(Self {
            content: PolicyStateContent {
                version: VERSION,
                account_id: certificate.account_id(),
                epoch: certificate.new_epoch(),
                recovery_roster_digest: certificate.new_recovery_roster_digest()?,
                latest_transition_id: certificate.certificate_id()?,
            },
        })
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, IdentityError> {
        ensure_size(bytes, MAX_ACCOUNT_RECOVERY_POLICY_STATE_BYTES)?;
        let payload = bytes
            .strip_prefix(POLICY_STATE_MAGIC)
            .ok_or(IdentityError::InvalidAccountRecoveryPolicyStateMagic)?;
        let state: Self = postcard::from_bytes(payload)?;
        state.verify()?;
        Ok(state)
    }

    pub fn encode(&self) -> Result<Vec<u8>, IdentityError> {
        self.verify()?;
        encode_with_magic(
            self,
            POLICY_STATE_MAGIC,
            MAX_ACCOUNT_RECOVERY_POLICY_STATE_BYTES,
        )
    }

    pub fn verify(&self) -> Result<(), IdentityError> {
        if self.content.version != VERSION {
            return Err(IdentityError::UnsupportedAccountRecoveryPolicyVersion(
                self.content.version,
            ));
        }
        if self.content.epoch == 0 && self.content.latest_transition_id != [0_u8; 32] {
            return Err(IdentityError::InvalidAccountRecoveryPolicyEpoch);
        }
        if self.content.epoch > 0 && self.content.latest_transition_id == [0_u8; 32] {
            return Err(IdentityError::InvalidAccountRecoveryPolicyEpoch);
        }
        Ok(())
    }

    pub fn account_id(&self) -> AccountId {
        self.content.account_id
    }

    pub fn epoch(&self) -> u64 {
        self.content.epoch
    }

    pub fn recovery_roster_digest(&self) -> &[u8; 32] {
        &self.content.recovery_roster_digest
    }

    pub fn latest_transition_id(&self) -> &[u8; 32] {
        &self.content.latest_transition_id
    }

    pub fn verify_transition_anchor(
        &self,
        request: &AccountRecoveryPolicyTransitionRequest,
    ) -> Result<(), IdentityError> {
        self.verify()?;
        request.verify_static()?;
        if self.account_id() != request.account_id()
            || self.epoch() != request.old_epoch()
            || self.recovery_roster_digest() != &request.old_recovery_roster_digest()?
            || self.latest_transition_id() != request.previous_transition_id()
        {
            return Err(IdentityError::AccountRecoveryPolicyAnchorMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct TransitionRequestContent {
    version: u8,
    old_epoch: u64,
    previous_transition_id: [u8; 32],
    old_package: AccountRootRecoveryPackage,
    old_witness: AccountRootRecoveryWitness,
    new_package: AccountRootRecoveryPackage,
    new_witness: AccountRootRecoveryWitness,
    challenge: [u8; 32],
    issued_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountRecoveryPolicyTransitionRequest {
    content: TransitionRequestContent,
}

impl AccountRecoveryPolicyTransitionRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        old_epoch: u64,
        previous_transition_id: [u8; 32],
        old_package: AccountRootRecoveryPackage,
        old_witness: AccountRootRecoveryWitness,
        new_package: AccountRootRecoveryPackage,
        new_witness: AccountRootRecoveryWitness,
        now_unix_seconds: u64,
        validity_seconds: u64,
    ) -> Result<Self, IdentityError> {
        let mut challenge = [0_u8; 32];
        getrandom::fill(&mut challenge).map_err(IdentityError::SecureRandom)?;
        Self::issue_with_challenge(
            old_epoch,
            previous_transition_id,
            old_package,
            old_witness,
            new_package,
            new_witness,
            challenge,
            now_unix_seconds,
            validity_seconds,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn issue_with_challenge(
        old_epoch: u64,
        previous_transition_id: [u8; 32],
        old_package: AccountRootRecoveryPackage,
        old_witness: AccountRootRecoveryWitness,
        new_package: AccountRootRecoveryPackage,
        new_witness: AccountRootRecoveryWitness,
        challenge: [u8; 32],
        now_unix_seconds: u64,
        validity_seconds: u64,
    ) -> Result<Self, IdentityError> {
        if !(1..=crate::MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_VALIDITY_SECONDS)
            .contains(&validity_seconds)
        {
            return Err(IdentityError::InvalidAccountRootRecoveryApprovalValidity);
        }
        let expires_at_unix_seconds = now_unix_seconds
            .checked_add(validity_seconds)
            .ok_or(IdentityError::InvalidAccountRootRecoveryApprovalValidity)?;
        let request = Self {
            content: TransitionRequestContent {
                version: VERSION,
                old_epoch,
                previous_transition_id,
                old_package,
                old_witness,
                new_package,
                new_witness,
                challenge,
                issued_at_unix_seconds: now_unix_seconds,
                expires_at_unix_seconds,
            },
        };
        request.verify_at(now_unix_seconds)?;
        Ok(request)
    }

    pub fn decode_and_verify(bytes: &[u8], now_unix_seconds: u64) -> Result<Self, IdentityError> {
        ensure_size(bytes, MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_REQUEST_BYTES)?;
        let payload = bytes
            .strip_prefix(TRANSITION_REQUEST_MAGIC)
            .ok_or(IdentityError::InvalidAccountRecoveryPolicyTransitionRequestMagic)?;
        let request: Self = postcard::from_bytes(payload)?;
        request.verify_at(now_unix_seconds)?;
        Ok(request)
    }

    pub fn encode(&self) -> Result<Vec<u8>, IdentityError> {
        self.verify_static()?;
        encode_with_magic(
            self,
            TRANSITION_REQUEST_MAGIC,
            MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_REQUEST_BYTES,
        )
    }

    pub fn verify_at(&self, now_unix_seconds: u64) -> Result<(), IdentityError> {
        self.verify_static()?;
        if self.content.issued_at_unix_seconds > now_unix_seconds.saturating_add(CLOCK_SKEW_SECONDS)
            || self.content.expires_at_unix_seconds < now_unix_seconds
        {
            return Err(IdentityError::AccountRecoveryPolicyTransitionRequestNotCurrent);
        }
        Ok(())
    }

    fn verify_static(&self) -> Result<(), IdentityError> {
        if self.content.version != VERSION {
            return Err(IdentityError::UnsupportedAccountRecoveryPolicyVersion(
                self.content.version,
            ));
        }
        self.content.old_package.verify()?;
        self.content
            .old_witness
            .verify_package(&self.content.old_package)?;
        self.content.new_package.verify()?;
        self.content
            .new_witness
            .verify_package(&self.content.new_package)?;
        if self.content.old_epoch == u64::MAX {
            return Err(IdentityError::InvalidAccountRecoveryPolicyEpoch);
        }
        if self.content.old_epoch == 0 && self.content.previous_transition_id != [0_u8; 32] {
            return Err(IdentityError::InvalidAccountRecoveryPolicyEpoch);
        }
        if self.content.old_epoch > 0 && self.content.previous_transition_id == [0_u8; 32] {
            return Err(IdentityError::InvalidAccountRecoveryPolicyEpoch);
        }
        verify_recovery_package_successor(
            &self.content.old_package,
            &self.content.new_package,
            true,
        )?;
        let validity = self
            .content
            .expires_at_unix_seconds
            .checked_sub(self.content.issued_at_unix_seconds)
            .ok_or(IdentityError::InvalidAccountRootRecoveryApprovalValidity)?;
        if !(1..=crate::MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_VALIDITY_SECONDS).contains(&validity) {
            return Err(IdentityError::InvalidAccountRootRecoveryApprovalValidity);
        }
        Ok(())
    }

    pub fn request_id(&self) -> Result<[u8; 32], IdentityError> {
        Ok(blake3::derive_key(REQUEST_ID_DOMAIN, &self.encode()?))
    }

    pub fn account_id(&self) -> AccountId {
        self.content.old_package.account_id()
    }

    pub fn old_epoch(&self) -> u64 {
        self.content.old_epoch
    }

    pub fn new_epoch(&self) -> u64 {
        self.content.old_epoch + 1
    }

    pub fn previous_transition_id(&self) -> &[u8; 32] {
        &self.content.previous_transition_id
    }

    pub fn old_package(&self) -> &AccountRootRecoveryPackage {
        &self.content.old_package
    }

    pub fn new_package(&self) -> &AccountRootRecoveryPackage {
        &self.content.new_package
    }

    pub fn old_recovery_roster_digest(&self) -> Result<[u8; 32], IdentityError> {
        self.old_package().recovery_roster_digest()
    }

    pub fn new_recovery_roster_digest(&self) -> Result<[u8; 32], IdentityError> {
        self.new_package().recovery_roster_digest()
    }

    pub fn old_roster_count(&self) -> usize {
        self.old_package().device_count()
    }

    pub fn new_roster_count(&self) -> usize {
        self.new_package().device_count()
    }

    pub fn old_required_approvals(&self) -> usize {
        self.old_roster_count() / 2 + 1
    }

    pub fn new_required_approvals(&self) -> usize {
        self.new_roster_count() / 2 + 1
    }

    pub fn challenge(&self) -> &[u8; 32] {
        &self.content.challenge
    }

    pub fn issued_at_unix_seconds(&self) -> u64 {
        self.content.issued_at_unix_seconds
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.content.expires_at_unix_seconds
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct TransitionApprovalContent {
    version: u8,
    account_id: AccountId,
    request_id: [u8; 32],
    old_epoch: u64,
    new_epoch: u64,
    previous_transition_id: [u8; 32],
    old_recovery_roster_digest: [u8; 32],
    new_recovery_roster_digest: [u8; 32],
    challenge: [u8; 32],
    request_expires_at_unix_seconds: u64,
    approver_device_id: DeviceId,
    previous_transition_approval_id: [u8; 32],
    approved_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountRecoveryPolicyTransitionApproval {
    content: TransitionApprovalContent,
    signature: Vec<u8>,
}

impl AccountRecoveryPolicyTransitionApproval {
    pub fn issue(
        identity: &DeviceIdentity,
        request: &AccountRecoveryPolicyTransitionRequest,
        previous_transition_approval_id: [u8; 32],
        now_unix_seconds: u64,
    ) -> Result<Self, IdentityError> {
        request.verify_at(now_unix_seconds)?;
        let approver_device_id = identity.device_id();
        if !is_active_voter(request.old_package(), approver_device_id)
            && !is_active_voter(request.new_package(), approver_device_id)
        {
            return Err(
                IdentityError::AccountRecoveryPolicyApproverNotInEitherRoster(approver_device_id),
            );
        }
        let content = TransitionApprovalContent {
            version: VERSION,
            account_id: request.account_id(),
            request_id: request.request_id()?,
            old_epoch: request.old_epoch(),
            new_epoch: request.new_epoch(),
            previous_transition_id: *request.previous_transition_id(),
            old_recovery_roster_digest: request.old_recovery_roster_digest()?,
            new_recovery_roster_digest: request.new_recovery_roster_digest()?,
            challenge: *request.challenge(),
            request_expires_at_unix_seconds: request.expires_at_unix_seconds(),
            approver_device_id,
            previous_transition_approval_id,
            approved_at_unix_seconds: now_unix_seconds,
        };
        let signature = identity
            .sign(&transition_approval_signing_bytes(&content)?)
            .to_vec();
        let approval = Self { content, signature };
        approval.verify_for_request(request)?;
        Ok(approval)
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, IdentityError> {
        ensure_size(bytes, MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_APPROVAL_BYTES)?;
        let payload = bytes
            .strip_prefix(TRANSITION_APPROVAL_MAGIC)
            .ok_or(IdentityError::InvalidAccountRecoveryPolicyTransitionApprovalMagic)?;
        let approval: Self = postcard::from_bytes(payload)?;
        approval.verify_signature()?;
        Ok(approval)
    }

    pub fn encode(&self) -> Result<Vec<u8>, IdentityError> {
        self.verify_signature()?;
        encode_with_magic(
            self,
            TRANSITION_APPROVAL_MAGIC,
            MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_APPROVAL_BYTES,
        )
    }

    pub fn verify_for_request(
        &self,
        request: &AccountRecoveryPolicyTransitionRequest,
    ) -> Result<(), IdentityError> {
        self.verify_signature()?;
        request.verify_static()?;
        if self.content.account_id != request.account_id()
            || self.content.request_id != request.request_id()?
            || self.content.old_epoch != request.old_epoch()
            || self.content.new_epoch != request.new_epoch()
            || self.content.previous_transition_id != *request.previous_transition_id()
            || self.content.old_recovery_roster_digest != request.old_recovery_roster_digest()?
            || self.content.new_recovery_roster_digest != request.new_recovery_roster_digest()?
            || self.content.challenge != *request.challenge()
            || self.content.request_expires_at_unix_seconds != request.expires_at_unix_seconds()
        {
            return Err(IdentityError::AccountRecoveryPolicyTransitionApprovalMismatch);
        }
        if self.content.approved_at_unix_seconds
            < request
                .issued_at_unix_seconds()
                .saturating_sub(CLOCK_SKEW_SECONDS)
            || self.content.approved_at_unix_seconds > request.expires_at_unix_seconds()
        {
            return Err(IdentityError::AccountRecoveryPolicyTransitionApprovalTimeInvalid);
        }
        if !is_active_voter(request.old_package(), self.approver_device_id())
            && !is_active_voter(request.new_package(), self.approver_device_id())
        {
            return Err(
                IdentityError::AccountRecoveryPolicyApproverNotInEitherRoster(
                    self.approver_device_id(),
                ),
            );
        }
        Ok(())
    }

    fn verify_signature(&self) -> Result<(), IdentityError> {
        if self.content.version != VERSION {
            return Err(IdentityError::UnsupportedAccountRecoveryPolicyVersion(
                self.content.version,
            ));
        }
        self.content.approver_device_id.verify(
            &transition_approval_signing_bytes(&self.content)?,
            &self.signature,
        )
    }

    pub fn approval_id(&self) -> Result<[u8; 32], IdentityError> {
        Ok(blake3::derive_key(APPROVAL_ID_DOMAIN, &self.encode()?))
    }

    pub fn account_id(&self) -> AccountId {
        self.content.account_id
    }

    pub fn request_id(&self) -> &[u8; 32] {
        &self.content.request_id
    }

    pub fn old_epoch(&self) -> u64 {
        self.content.old_epoch
    }

    pub fn new_epoch(&self) -> u64 {
        self.content.new_epoch
    }

    pub fn approver_device_id(&self) -> DeviceId {
        self.content.approver_device_id
    }

    pub fn previous_transition_approval_id(&self) -> &[u8; 32] {
        &self.content.previous_transition_approval_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountRecoveryPolicyTransitionReport {
    old_observed: usize,
    old_required: usize,
    new_observed: usize,
    new_required: usize,
}

impl AccountRecoveryPolicyTransitionReport {
    pub fn old_observed(&self) -> usize {
        self.old_observed
    }
    pub fn old_required(&self) -> usize {
        self.old_required
    }
    pub fn new_observed(&self) -> usize {
        self.new_observed
    }
    pub fn new_required(&self) -> usize {
        self.new_required
    }
    pub fn joint_majority_satisfied(&self) -> bool {
        self.old_observed >= self.old_required && self.new_observed >= self.new_required
    }
    pub fn require_joint_majority(&self) -> Result<(), IdentityError> {
        if !self.joint_majority_satisfied() {
            return Err(
                IdentityError::InsufficientAccountRecoveryPolicyJointQuorum {
                    old_observed: self.old_observed,
                    old_required: self.old_required,
                    new_observed: self.new_observed,
                    new_required: self.new_required,
                },
            );
        }
        Ok(())
    }
}

pub fn verify_account_recovery_policy_transition(
    request: &AccountRecoveryPolicyTransitionRequest,
    approvals: &[AccountRecoveryPolicyTransitionApproval],
) -> Result<AccountRecoveryPolicyTransitionReport, IdentityError> {
    request.verify_static()?;
    if approvals.len() > MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_APPROVALS {
        return Err(IdentityError::TooManyAccountRecoveryPolicyApprovals(
            approvals.len(),
        ));
    }
    let mut distinct = BTreeSet::new();
    let mut old_observed = 0;
    let mut new_observed = 0;
    for approval in approvals {
        approval.verify_for_request(request)?;
        let device_id = approval.approver_device_id();
        if !distinct.insert(device_id) {
            return Err(IdentityError::DuplicateAccountRecoveryPolicyApprover(
                device_id,
            ));
        }
        old_observed += usize::from(is_active_voter(request.old_package(), device_id));
        new_observed += usize::from(is_active_voter(request.new_package(), device_id));
    }
    Ok(AccountRecoveryPolicyTransitionReport {
        old_observed,
        old_required: request.old_required_approvals(),
        new_observed,
        new_required: request.new_required_approvals(),
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct TransitionCertificateContent {
    version: u8,
    request: AccountRecoveryPolicyTransitionRequest,
    approvals: Vec<AccountRecoveryPolicyTransitionApproval>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountRecoveryPolicyTransitionCertificate {
    content: TransitionCertificateContent,
}

impl AccountRecoveryPolicyTransitionCertificate {
    pub fn issue(
        request: AccountRecoveryPolicyTransitionRequest,
        mut approvals: Vec<AccountRecoveryPolicyTransitionApproval>,
        now_unix_seconds: u64,
    ) -> Result<Self, IdentityError> {
        request.verify_at(now_unix_seconds)?;
        approvals.sort_by_key(AccountRecoveryPolicyTransitionApproval::approver_device_id);
        let certificate = Self {
            content: TransitionCertificateContent {
                version: VERSION,
                request,
                approvals,
            },
        };
        certificate.verify()?;
        Ok(certificate)
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, IdentityError> {
        ensure_size(bytes, MAX_ACCOUNT_RECOVERY_POLICY_CERTIFICATE_BYTES)?;
        let payload = bytes
            .strip_prefix(TRANSITION_CERTIFICATE_MAGIC)
            .ok_or(IdentityError::InvalidAccountRecoveryPolicyTransitionCertificateMagic)?;
        let certificate: Self = postcard::from_bytes(payload)?;
        certificate.verify()?;
        Ok(certificate)
    }

    pub fn encode(&self) -> Result<Vec<u8>, IdentityError> {
        self.verify()?;
        encode_with_magic(
            self,
            TRANSITION_CERTIFICATE_MAGIC,
            MAX_ACCOUNT_RECOVERY_POLICY_CERTIFICATE_BYTES,
        )
    }

    pub fn verify(&self) -> Result<(), IdentityError> {
        if self.content.version != VERSION {
            return Err(IdentityError::UnsupportedAccountRecoveryPolicyVersion(
                self.content.version,
            ));
        }
        if !self
            .content
            .approvals
            .windows(2)
            .all(|pair| pair[0].approver_device_id() < pair[1].approver_device_id())
        {
            return Err(IdentityError::NonCanonicalAccountRecoveryPolicyApprovals);
        }
        verify_account_recovery_policy_transition(&self.content.request, &self.content.approvals)?
            .require_joint_majority()
    }

    pub fn certificate_id(&self) -> Result<[u8; 32], IdentityError> {
        Ok(blake3::derive_key(CERTIFICATE_ID_DOMAIN, &self.encode()?))
    }

    pub fn request(&self) -> &AccountRecoveryPolicyTransitionRequest {
        &self.content.request
    }

    pub fn account_id(&self) -> AccountId {
        self.request().account_id()
    }
    pub fn old_epoch(&self) -> u64 {
        self.request().old_epoch()
    }
    pub fn new_epoch(&self) -> u64 {
        self.request().new_epoch()
    }
    pub fn previous_transition_id(&self) -> &[u8; 32] {
        self.request().previous_transition_id()
    }
    pub fn old_package(&self) -> &AccountRootRecoveryPackage {
        self.request().old_package()
    }
    pub fn new_package(&self) -> &AccountRootRecoveryPackage {
        self.request().new_package()
    }
    pub fn old_recovery_roster_digest(&self) -> Result<[u8; 32], IdentityError> {
        self.request().old_recovery_roster_digest()
    }
    pub fn new_recovery_roster_digest(&self) -> Result<[u8; 32], IdentityError> {
        self.request().new_recovery_roster_digest()
    }
    pub fn approvals(&self) -> &[AccountRecoveryPolicyTransitionApproval] {
        &self.content.approvals
    }
}

/// Verifies that `new` is a monotonic successor of `old`; roster changes are
/// optionally required. This is also used to validate later same-roster
/// recovery checkpoints against a certified policy transition.
pub fn verify_recovery_package_successor(
    old: &AccountRootRecoveryPackage,
    new: &AccountRootRecoveryPackage,
    require_roster_change: bool,
) -> Result<(), IdentityError> {
    old.verify()?;
    new.verify()?;
    if old.account_id() != new.account_id() || new.authority_revision() < old.authority_revision() {
        return Err(IdentityError::AccountRecoveryPolicyStateRollback);
    }
    if new.authority_revision() == old.authority_revision()
        && new.authority_snapshot() != old.authority_snapshot()
    {
        return Err(IdentityError::AccountRecoveryPolicyStateRollback);
    }
    if old
        .authority_snapshot()
        .revocations()
        .iter()
        .any(|old_revocation| {
            !new.authority_snapshot()
                .revocations()
                .iter()
                .any(|candidate| candidate == old_revocation)
        })
    {
        return Err(IdentityError::AccountRecoveryPolicyStateRollback);
    }
    for old_membership in old.conversation_memberships() {
        let Some(new_membership) = new
            .conversation_memberships()
            .iter()
            .find(|candidate| candidate.conversation_id() == old_membership.conversation_id())
        else {
            return Err(IdentityError::AccountRecoveryPolicyStateRollback);
        };
        if new_membership.revision() < old_membership.revision()
            || (new_membership.revision() == old_membership.revision()
                && new_membership != old_membership)
            || (new_membership.revision() > old_membership.revision()
                && old_membership
                    .members()
                    .iter()
                    .any(|member| !new_membership.members().contains(member)))
        {
            return Err(IdentityError::AccountRecoveryPolicyStateRollback);
        }
    }
    for old_certificate in old.device_list().devices() {
        match new
            .device_list()
            .certificate_for(old_certificate.device_id())
        {
            Some(new_certificate) if new_certificate == old_certificate => {}
            Some(_) => return Err(IdentityError::AccountRecoveryPolicyStateRollback),
            None => {
                if !new
                    .authority_snapshot()
                    .revocations()
                    .iter()
                    .any(|revocation| revocation.device_id() == old_certificate.device_id())
                {
                    return Err(IdentityError::AccountRecoveryPolicyStateRollback);
                }
            }
        }
    }
    let old_digest = old.recovery_roster_digest()?;
    let new_digest = new.recovery_roster_digest()?;
    if require_roster_change && old_digest == new_digest {
        return Err(IdentityError::AccountRecoveryPolicyRosterUnchanged);
    }
    if require_roster_change && new.authority_revision() <= old.authority_revision() {
        return Err(IdentityError::AccountRecoveryPolicyStateRollback);
    }
    Ok(())
}

fn is_active_voter(package: &AccountRootRecoveryPackage, device_id: DeviceId) -> bool {
    package
        .device_list()
        .certificate_for(device_id)
        .is_some_and(|certificate| {
            verify_device_authorization_with_snapshot(
                package.account_id(),
                certificate,
                package.authority_snapshot(),
                &DeviceCapability::MESSAGING,
            )
            .is_ok()
        })
}

fn transition_approval_signing_bytes(
    content: &TransitionApprovalContent,
) -> Result<Vec<u8>, IdentityError> {
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(APPROVAL_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(APPROVAL_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn ensure_size(bytes: &[u8], maximum: usize) -> Result<(), IdentityError> {
    if bytes.len() > maximum {
        return Err(IdentityError::AccountRecoveryPolicyArtifactTooLarge {
            actual: bytes.len(),
            maximum,
        });
    }
    Ok(())
}

fn encode_with_magic<T: Serialize>(
    value: &T,
    magic: &[u8],
    maximum: usize,
) -> Result<Vec<u8>, IdentityError> {
    let payload = postcard::to_allocvec(value)?;
    let total = magic.len() + payload.len();
    if total > maximum {
        return Err(IdentityError::AccountRecoveryPolicyArtifactTooLarge {
            actual: total,
            maximum,
        });
    }
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(magic);
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::{AccountRootState, DeviceState};

    #[test]
    fn joint_majority_counts_overlap_in_both_rosters() -> Result<(), IdentityError> {
        let root_dir = tempdir()?;
        let root = AccountRootState::create(root_dir.path())?;
        let mut devices = Vec::new();
        let mut certificates = Vec::new();
        for _ in 0..2 {
            let dir = tempdir()?;
            let device = DeviceState::load_or_create(dir.path())?;
            certificates.push(root.issue_device_certificate(
                device.identity().device_id(),
                device.encryption().public_key(),
                &DeviceCapability::MESSAGING,
            )?);
            devices.push(device);
        }
        root.publish_device_list(&certificates)?;
        let (old_package, old_witness) = root.export_recovery()?;
        let dir = tempdir()?;
        let device = DeviceState::load_or_create(dir.path())?;
        certificates.push(root.issue_device_certificate(
            device.identity().device_id(),
            device.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?);
        devices.push(device);
        root.publish_device_list(&certificates)?;
        let (new_package, new_witness) = root.export_recovery()?;
        let request = AccountRecoveryPolicyTransitionRequest::issue_with_challenge(
            0,
            [0_u8; 32],
            old_package,
            old_witness,
            new_package,
            new_witness,
            [9_u8; 32],
            1_000,
            600,
        )?;
        let first = AccountRecoveryPolicyTransitionApproval::issue(
            devices[0].identity(),
            &request,
            [0_u8; 32],
            1_100,
        )?;
        let partial =
            verify_account_recovery_policy_transition(&request, std::slice::from_ref(&first))?;
        assert_eq!((partial.old_observed(), partial.new_observed()), (1, 1));
        assert!(!partial.joint_majority_satisfied());
        let second = AccountRecoveryPolicyTransitionApproval::issue(
            devices[1].identity(),
            &request,
            [0_u8; 32],
            1_101,
        )?;
        let certificate = AccountRecoveryPolicyTransitionCertificate::issue(
            request.clone(),
            vec![second, first],
            1_200,
        )?;
        assert_eq!(certificate.old_epoch(), 0);
        assert_eq!(certificate.new_epoch(), 1);
        assert_eq!(
            AccountRecoveryPolicyState::transitioned(&certificate)?.epoch(),
            1
        );
        assert_eq!(
            AccountRecoveryPolicyTransitionCertificate::decode_and_verify(&certificate.encode()?)?,
            certificate
        );
        Ok(())
    }

    #[test]
    fn new_only_signature_does_not_replace_old_majority() -> Result<(), IdentityError> {
        let root_dir = tempdir()?;
        let root = AccountRootState::create(root_dir.path())?;
        let mut devices = Vec::new();
        let mut certificates = Vec::new();
        for _ in 0..2 {
            let dir = tempdir()?;
            let device = DeviceState::load_or_create(dir.path())?;
            certificates.push(root.issue_device_certificate(
                device.identity().device_id(),
                device.encryption().public_key(),
                &DeviceCapability::MESSAGING,
            )?);
            devices.push(device);
        }
        root.publish_device_list(&certificates)?;
        let (old_package, old_witness) = root.export_recovery()?;
        let dir = tempdir()?;
        let device = DeviceState::load_or_create(dir.path())?;
        certificates.push(root.issue_device_certificate(
            device.identity().device_id(),
            device.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?);
        devices.push(device);
        root.publish_device_list(&certificates)?;
        let (new_package, new_witness) = root.export_recovery()?;
        let request = AccountRecoveryPolicyTransitionRequest::issue_with_challenge(
            0,
            [0_u8; 32],
            old_package,
            old_witness,
            new_package,
            new_witness,
            [3_u8; 32],
            1_000,
            600,
        )?;
        let new_only = AccountRecoveryPolicyTransitionApproval::issue(
            devices[2].identity(),
            &request,
            [0_u8; 32],
            1_100,
        )?;
        let report = verify_account_recovery_policy_transition(&request, &[new_only])?;
        assert_eq!((report.old_observed(), report.new_observed()), (0, 1));
        assert!(report.require_joint_majority().is_err());
        Ok(())
    }
}
