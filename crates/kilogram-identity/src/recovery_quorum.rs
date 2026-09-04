use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    AccountAuthoritySnapshot, AccountId, AccountRootRecoveryPackage, AccountRootRecoveryWitness,
    ConversationMembershipSnapshot, DeviceCapability, DeviceCertificate, DeviceId, DeviceIdentity,
    IdentityError, MAX_ACCOUNT_DEVICES, MAX_ACCOUNT_ROOT_RECOVERY_PACKAGE_BYTES,
    MAX_ACCOUNT_ROOT_RECOVERY_WITNESS_BYTES, verify_device_authorization_with_snapshot,
};

const REQUEST_MAGIC: &[u8; 16] = b"KILOGRAM-ARRQS01";
const APPROVAL_MAGIC: &[u8; 16] = b"KILOGRAM-ARAPR01";
const VERSION: u8 = 1;
const REQUEST_ID_DOMAIN: &str = "Kilogram Account Root recovery approval request ID v1";
const APPROVAL_ID_DOMAIN: &str = "Kilogram Account Root recovery approval ID v1";
const APPROVAL_SIGNATURE_DOMAIN: &[u8] = b"kilogram:account-root-recovery-approval:v1\0";
const CLOCK_SKEW_SECONDS: u64 = 120;

pub const DEFAULT_ACCOUNT_ROOT_RECOVERY_APPROVAL_VALIDITY_SECONDS: u64 = 600;
pub const MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_VALIDITY_SECONDS: u64 = 1_800;
pub const MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_REQUEST_BYTES: usize =
    MAX_ACCOUNT_ROOT_RECOVERY_PACKAGE_BYTES + MAX_ACCOUNT_ROOT_RECOVERY_WITNESS_BYTES + 64 * 1024;
pub const MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ApprovalRequestContent {
    version: u8,
    package: AccountRootRecoveryPackage,
    witness: AccountRootRecoveryWitness,
    challenge: [u8; 32],
    issued_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountRootRecoveryApprovalRequest {
    content: ApprovalRequestContent,
}

impl AccountRootRecoveryApprovalRequest {
    pub fn issue(
        package: AccountRootRecoveryPackage,
        witness: AccountRootRecoveryWitness,
        now_unix_seconds: u64,
        validity_seconds: u64,
    ) -> Result<Self, IdentityError> {
        let mut challenge = [0_u8; 32];
        getrandom::fill(&mut challenge).map_err(IdentityError::SecureRandom)?;
        Self::issue_with_challenge(
            package,
            witness,
            challenge,
            now_unix_seconds,
            validity_seconds,
        )
    }

    fn issue_with_challenge(
        package: AccountRootRecoveryPackage,
        witness: AccountRootRecoveryWitness,
        challenge: [u8; 32],
        now_unix_seconds: u64,
        validity_seconds: u64,
    ) -> Result<Self, IdentityError> {
        if !(1..=MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_VALIDITY_SECONDS).contains(&validity_seconds) {
            return Err(IdentityError::InvalidAccountRootRecoveryApprovalValidity);
        }
        let expires_at_unix_seconds = now_unix_seconds
            .checked_add(validity_seconds)
            .ok_or(IdentityError::InvalidAccountRootRecoveryApprovalValidity)?;
        let request = Self {
            content: ApprovalRequestContent {
                version: VERSION,
                package,
                witness,
                challenge,
                issued_at_unix_seconds: now_unix_seconds,
                expires_at_unix_seconds,
            },
        };
        request.verify_at(now_unix_seconds)?;
        Ok(request)
    }

    pub fn decode_and_verify(bytes: &[u8], now_unix_seconds: u64) -> Result<Self, IdentityError> {
        if bytes.len() > MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_REQUEST_BYTES {
            return Err(IdentityError::AccountRootRecoveryApprovalRequestTooLarge(
                bytes.len(),
            ));
        }
        let payload = bytes
            .strip_prefix(REQUEST_MAGIC)
            .ok_or(IdentityError::InvalidAccountRootRecoveryApprovalRequestMagic)?;
        let request: Self = postcard::from_bytes(payload)?;
        request.verify_at(now_unix_seconds)?;
        Ok(request)
    }

    pub fn encode(&self) -> Result<Vec<u8>, IdentityError> {
        self.verify_static()?;
        let payload = postcard::to_allocvec(self)?;
        let total_len = REQUEST_MAGIC.len() + payload.len();
        if total_len > MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_REQUEST_BYTES {
            return Err(IdentityError::AccountRootRecoveryApprovalRequestTooLarge(
                total_len,
            ));
        }
        let mut bytes = Vec::with_capacity(total_len);
        bytes.extend_from_slice(REQUEST_MAGIC);
        bytes.extend_from_slice(&payload);
        Ok(bytes)
    }

    pub fn verify_at(&self, now_unix_seconds: u64) -> Result<(), IdentityError> {
        self.verify_static()?;
        if self.content.issued_at_unix_seconds > now_unix_seconds.saturating_add(CLOCK_SKEW_SECONDS)
            || self.content.expires_at_unix_seconds < now_unix_seconds
        {
            return Err(IdentityError::AccountRootRecoveryApprovalRequestNotCurrent);
        }
        Ok(())
    }

    fn verify_static(&self) -> Result<(), IdentityError> {
        if self.content.version != VERSION {
            return Err(
                IdentityError::UnsupportedAccountRootRecoveryApprovalVersion(self.content.version),
            );
        }
        self.content.package.verify()?;
        self.content.witness.verify_package(&self.content.package)?;
        let validity = self
            .content
            .expires_at_unix_seconds
            .checked_sub(self.content.issued_at_unix_seconds)
            .ok_or(IdentityError::InvalidAccountRootRecoveryApprovalValidity)?;
        if !(1..=MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_VALIDITY_SECONDS).contains(&validity) {
            return Err(IdentityError::InvalidAccountRootRecoveryApprovalValidity);
        }
        Ok(())
    }

    pub fn request_id(&self) -> Result<[u8; 32], IdentityError> {
        Ok(blake3::derive_key(REQUEST_ID_DOMAIN, &self.encode()?))
    }

    pub fn account_id(&self) -> AccountId {
        self.content.package.account_id()
    }

    pub fn package(&self) -> &AccountRootRecoveryPackage {
        &self.content.package
    }

    pub fn witness(&self) -> &AccountRootRecoveryWitness {
        &self.content.witness
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

    pub fn roster_count(&self) -> usize {
        self.content.package.device_count()
    }

    pub fn required_approvals(&self) -> usize {
        self.roster_count() / 2 + 1
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ApprovalContent {
    version: u8,
    account_id: AccountId,
    request_id: [u8; 32],
    package_id: [u8; 32],
    authority_revision: u64,
    state_vector_digest: [u8; 32],
    recovery_roster_digest: [u8; 32],
    challenge: [u8; 32],
    request_expires_at_unix_seconds: u64,
    approver_device_id: DeviceId,
    previous_approval_head_digest: [u8; 32],
    approved_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountRootRecoveryApproval {
    content: ApprovalContent,
    signature: Vec<u8>,
}

impl AccountRootRecoveryApproval {
    pub fn issue(
        identity: &DeviceIdentity,
        request: &AccountRootRecoveryApprovalRequest,
        previous_approval_head_digest: [u8; 32],
        now_unix_seconds: u64,
    ) -> Result<Self, IdentityError> {
        request.verify_at(now_unix_seconds)?;
        let approver_device_id = identity.device_id();
        request
            .package()
            .device_list()
            .certificate_for(approver_device_id)
            .ok_or(IdentityError::AccountRootRecoveryApproverNotInRoster(
                approver_device_id,
            ))?;
        let content = ApprovalContent {
            version: VERSION,
            account_id: request.account_id(),
            request_id: request.request_id()?,
            package_id: request.package().package_id()?,
            authority_revision: request.package().authority_revision(),
            state_vector_digest: request.package().state_vector_digest()?,
            recovery_roster_digest: request.package().recovery_roster_digest()?,
            challenge: *request.challenge(),
            request_expires_at_unix_seconds: request.expires_at_unix_seconds(),
            approver_device_id,
            previous_approval_head_digest,
            approved_at_unix_seconds: now_unix_seconds,
        };
        let signature = identity.sign(&approval_signing_bytes(&content)?).to_vec();
        let approval = Self { content, signature };
        approval.verify_for_request(request, now_unix_seconds)?;
        Ok(approval)
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, IdentityError> {
        if bytes.len() > MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_BYTES {
            return Err(IdentityError::AccountRootRecoveryApprovalTooLarge(
                bytes.len(),
            ));
        }
        let payload = bytes
            .strip_prefix(APPROVAL_MAGIC)
            .ok_or(IdentityError::InvalidAccountRootRecoveryApprovalMagic)?;
        let approval: Self = postcard::from_bytes(payload)?;
        approval.verify_signature()?;
        Ok(approval)
    }

    pub fn encode(&self) -> Result<Vec<u8>, IdentityError> {
        self.verify_signature()?;
        let payload = postcard::to_allocvec(self)?;
        let total_len = APPROVAL_MAGIC.len() + payload.len();
        if total_len > MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_BYTES {
            return Err(IdentityError::AccountRootRecoveryApprovalTooLarge(
                total_len,
            ));
        }
        let mut bytes = Vec::with_capacity(total_len);
        bytes.extend_from_slice(APPROVAL_MAGIC);
        bytes.extend_from_slice(&payload);
        Ok(bytes)
    }

    pub fn verify_signature(&self) -> Result<(), IdentityError> {
        if self.content.version != VERSION {
            return Err(
                IdentityError::UnsupportedAccountRootRecoveryApprovalVersion(self.content.version),
            );
        }
        self.content
            .approver_device_id
            .verify(&approval_signing_bytes(&self.content)?, &self.signature)
    }

    pub fn verify_for_request(
        &self,
        request: &AccountRootRecoveryApprovalRequest,
        now_unix_seconds: u64,
    ) -> Result<(), IdentityError> {
        self.verify_signature()?;
        request.verify_at(now_unix_seconds)?;
        let package = request.package();
        if self.content.account_id != request.account_id()
            || self.content.request_id != request.request_id()?
            || self.content.package_id != package.package_id()?
            || self.content.authority_revision != package.authority_revision()
            || self.content.state_vector_digest != package.state_vector_digest()?
            || self.content.recovery_roster_digest != package.recovery_roster_digest()?
            || self.content.challenge != *request.challenge()
            || self.content.request_expires_at_unix_seconds != request.expires_at_unix_seconds()
        {
            return Err(IdentityError::AccountRootRecoveryApprovalMismatch);
        }
        if self.content.approved_at_unix_seconds
            < request
                .issued_at_unix_seconds()
                .saturating_sub(CLOCK_SKEW_SECONDS)
            || self.content.approved_at_unix_seconds > request.expires_at_unix_seconds()
            || self.content.approved_at_unix_seconds
                > now_unix_seconds.saturating_add(CLOCK_SKEW_SECONDS)
        {
            return Err(IdentityError::AccountRootRecoveryApprovalTimeInvalid);
        }
        let certificate = package
            .device_list()
            .certificate_for(self.content.approver_device_id)
            .ok_or(IdentityError::AccountRootRecoveryApproverNotInRoster(
                self.content.approver_device_id,
            ))?;
        verify_device_authorization_with_snapshot(
            request.account_id(),
            certificate,
            package.authority_snapshot(),
            &DeviceCapability::MESSAGING,
        )?;
        Ok(())
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

    pub fn package_id(&self) -> &[u8; 32] {
        &self.content.package_id
    }

    pub fn authority_revision(&self) -> u64 {
        self.content.authority_revision
    }

    pub fn recovery_roster_digest(&self) -> &[u8; 32] {
        &self.content.recovery_roster_digest
    }

    pub fn approver_device_id(&self) -> DeviceId {
        self.content.approver_device_id
    }

    pub fn previous_approval_head_digest(&self) -> &[u8; 32] {
        &self.content.previous_approval_head_digest
    }

    pub fn approved_at_unix_seconds(&self) -> u64 {
        self.content.approved_at_unix_seconds
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountRootRecoveryFreshnessClaim {
    ArtifactIntegrityOnly,
    SingleCurrentDeviceObserved,
    CurrentDeviceMajorityObserved,
}

impl AccountRootRecoveryFreshnessClaim {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ArtifactIntegrityOnly => "artifact-integrity-only",
            Self::SingleCurrentDeviceObserved => "single-current-device-observed",
            Self::CurrentDeviceMajorityObserved => "current-device-majority-observed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountRootRecoveryQuorumReport {
    claim: AccountRootRecoveryFreshnessClaim,
    account_id: AccountId,
    request_id: [u8; 32],
    package_id: [u8; 32],
    recovery_roster_digest: [u8; 32],
    roster_count: usize,
    observed_approvals: usize,
    required_approvals: usize,
}

impl AccountRootRecoveryQuorumReport {
    pub fn claim(&self) -> AccountRootRecoveryFreshnessClaim {
        self.claim
    }

    pub fn account_id(&self) -> AccountId {
        self.account_id
    }

    pub fn request_id(&self) -> &[u8; 32] {
        &self.request_id
    }

    pub fn package_id(&self) -> &[u8; 32] {
        &self.package_id
    }

    pub fn recovery_roster_digest(&self) -> &[u8; 32] {
        &self.recovery_roster_digest
    }

    pub fn roster_count(&self) -> usize {
        self.roster_count
    }

    pub fn observed_approvals(&self) -> usize {
        self.observed_approvals
    }

    pub fn required_approvals(&self) -> usize {
        self.required_approvals
    }

    pub fn require_majority(&self) -> Result<(), IdentityError> {
        if self.observed_approvals < self.required_approvals {
            return Err(IdentityError::InsufficientAccountRootRecoveryQuorum {
                observed: self.observed_approvals,
                required: self.required_approvals,
            });
        }
        Ok(())
    }
}

pub fn verify_account_root_recovery_quorum(
    request: &AccountRootRecoveryApprovalRequest,
    approvals: &[AccountRootRecoveryApproval],
    now_unix_seconds: u64,
) -> Result<AccountRootRecoveryQuorumReport, IdentityError> {
    request.verify_at(now_unix_seconds)?;
    if approvals.len() > MAX_ACCOUNT_DEVICES {
        return Err(IdentityError::TooManyAccountDevices(approvals.len()));
    }
    let mut approvers = BTreeSet::new();
    for approval in approvals {
        approval.verify_for_request(request, now_unix_seconds)?;
        if !approvers.insert(approval.approver_device_id()) {
            return Err(IdentityError::DuplicateAccountRootRecoveryApprover(
                approval.approver_device_id(),
            ));
        }
    }
    let observed_approvals = approvers.len();
    let required_approvals = request.required_approvals();
    let claim = if observed_approvals >= required_approvals {
        AccountRootRecoveryFreshnessClaim::CurrentDeviceMajorityObserved
    } else if observed_approvals == 1 {
        AccountRootRecoveryFreshnessClaim::SingleCurrentDeviceObserved
    } else {
        AccountRootRecoveryFreshnessClaim::ArtifactIntegrityOnly
    };
    Ok(AccountRootRecoveryQuorumReport {
        claim,
        account_id: request.account_id(),
        request_id: request.request_id()?,
        package_id: request.package().package_id()?,
        recovery_roster_digest: request.package().recovery_roster_digest()?,
        roster_count: request.roster_count(),
        observed_approvals,
        required_approvals,
    })
}

impl AccountRootRecoveryPackage {
    pub fn verify_dominates_device_state(
        &self,
        local_certificate: &DeviceCertificate,
        local_authority: &AccountAuthoritySnapshot,
        local_owned_memberships: &[ConversationMembershipSnapshot],
    ) -> Result<(), IdentityError> {
        self.verify()?;
        local_certificate.verify_for_account(self.account_id())?;
        local_authority.verify_for_account(self.account_id())?;
        let candidate_certificate = self
            .device_list()
            .certificate_for(local_certificate.device_id())
            .ok_or(IdentityError::AccountRootRecoveryApproverNotInRoster(
                local_certificate.device_id(),
            ))?;
        if candidate_certificate != local_certificate {
            return Err(IdentityError::AccountRootRecoveryApproverCertificateMismatch);
        }
        verify_device_authorization_with_snapshot(
            self.account_id(),
            candidate_certificate,
            self.authority_snapshot(),
            &DeviceCapability::MESSAGING,
        )?;

        if self.authority_revision() < local_authority.revision() {
            return Err(IdentityError::AuthoritySnapshotRollback {
                account_id: self.account_id(),
                stored_revision: local_authority.revision(),
                received_revision: self.authority_revision(),
            });
        }
        if self.authority_revision() == local_authority.revision() {
            if self.authority_snapshot() != local_authority {
                return Err(IdentityError::AuthoritySnapshotEquivocation {
                    account_id: self.account_id(),
                    revision: self.authority_revision(),
                });
            }
        } else if local_authority.revocations().iter().any(|stored| {
            !self
                .authority_snapshot()
                .revocations()
                .iter()
                .any(|candidate| candidate == stored)
        }) {
            return Err(IdentityError::AuthoritySnapshotRollback {
                account_id: self.account_id(),
                stored_revision: local_authority.revision(),
                received_revision: self.authority_revision(),
            });
        }

        for local in local_owned_memberships {
            local.verify_for_owner(self.account_id())?;
            let candidate = self
                .conversation_memberships()
                .iter()
                .find(|membership| membership.conversation_id() == local.conversation_id())
                .ok_or(IdentityError::AccountRootRecoveryMissingLocalMembership(
                    local.conversation_id(),
                ))?;
            if candidate.revision() < local.revision() {
                return Err(IdentityError::ConversationMembershipRollback {
                    conversation_id: local.conversation_id(),
                    stored_revision: local.revision(),
                    received_revision: candidate.revision(),
                });
            }
            if candidate.revision() == local.revision() && candidate != local {
                return Err(IdentityError::ConversationMembershipEquivocation {
                    conversation_id: local.conversation_id(),
                    revision: local.revision(),
                });
            }
            if candidate.revision() > local.revision()
                && local
                    .members()
                    .iter()
                    .any(|member| !candidate.members().contains(member))
            {
                return Err(IdentityError::ConversationMembershipNotAddOnly(
                    local.conversation_id(),
                ));
            }
        }
        Ok(())
    }
}

fn approval_signing_bytes(content: &ApprovalContent) -> Result<Vec<u8>, IdentityError> {
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(APPROVAL_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(APPROVAL_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::{AccountRootState, DeviceState};

    fn fixture(
        device_count: usize,
    ) -> Result<
        (
            AccountRootRecoveryApprovalRequest,
            Vec<DeviceState>,
            Vec<DeviceCertificate>,
        ),
        IdentityError,
    > {
        let root_dir = tempdir()?;
        let root = AccountRootState::create(root_dir.path())?;
        let mut devices = Vec::new();
        let mut certificates = Vec::new();
        for _ in 0..device_count {
            let directory = tempdir()?;
            let device = DeviceState::load_or_create(directory.path())?;
            let certificate = root.issue_device_certificate(
                device.identity().device_id(),
                device.encryption().public_key(),
                &DeviceCapability::MESSAGING,
            )?;
            certificates.push(certificate);
            devices.push(device);
        }
        root.publish_device_list(&certificates)?;
        let (package, witness) = root.export_recovery()?;
        let request = AccountRootRecoveryApprovalRequest::issue_with_challenge(
            package, witness, [7_u8; 32], 1_000, 600,
        )?;
        Ok((request, devices, certificates))
    }

    #[test]
    fn request_is_bounded_exact_and_expires() -> Result<(), IdentityError> {
        let (request, _, _) = fixture(1)?;
        let encoded = request.encode()?;
        assert_eq!(
            AccountRootRecoveryApprovalRequest::decode_and_verify(&encoded, 1_300)?,
            request
        );
        assert!(matches!(
            AccountRootRecoveryApprovalRequest::decode_and_verify(&encoded, 1_601),
            Err(IdentityError::AccountRootRecoveryApprovalRequestNotCurrent)
        ));
        assert_eq!(request.required_approvals(), 1);
        Ok(())
    }

    #[test]
    fn exact_roster_majority_rejects_duplicates_and_replay() -> Result<(), IdentityError> {
        let (request, devices, _) = fixture(3)?;
        let first =
            AccountRootRecoveryApproval::issue(devices[0].identity(), &request, [0_u8; 32], 1_100)?;
        let single =
            verify_account_root_recovery_quorum(&request, std::slice::from_ref(&first), 1_200)?;
        assert_eq!(
            single.claim(),
            AccountRootRecoveryFreshnessClaim::SingleCurrentDeviceObserved
        );
        assert!(matches!(
            single.require_majority(),
            Err(IdentityError::InsufficientAccountRootRecoveryQuorum {
                observed: 1,
                required: 2
            })
        ));
        let second =
            AccountRootRecoveryApproval::issue(devices[1].identity(), &request, [0_u8; 32], 1_101)?;
        let majority =
            verify_account_root_recovery_quorum(&request, &[first.clone(), second], 1_200)?;
        assert_eq!(
            majority.claim(),
            AccountRootRecoveryFreshnessClaim::CurrentDeviceMajorityObserved
        );
        majority.require_majority()?;
        assert!(matches!(
            verify_account_root_recovery_quorum(&request, &[first.clone(), first], 1_200),
            Err(IdentityError::DuplicateAccountRootRecoveryApprover(_))
        ));

        let replay = AccountRootRecoveryApprovalRequest::issue_with_challenge(
            request.package().clone(),
            request.witness().clone(),
            [8_u8; 32],
            1_000,
            600,
        )?;
        assert!(matches!(
            verify_account_root_recovery_quorum(
                &replay,
                &[AccountRootRecoveryApproval::issue(
                    devices[0].identity(),
                    &request,
                    [0_u8; 32],
                    1_100,
                )?],
                1_200,
            ),
            Err(IdentityError::AccountRootRecoveryApprovalMismatch)
        ));
        Ok(())
    }
}
