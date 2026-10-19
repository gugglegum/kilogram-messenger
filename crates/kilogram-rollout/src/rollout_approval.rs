//! Device-signed approval bridge for dormant rollout-controller proposals.
//!
//! A release policy may produce an advisory proposal, but it cannot sign or
//! persist canonical-sync activation authority. This module requires an
//! explicit local approval, signs both the exact proposal and the resulting
//! activation record with the current Device identity, and remains free of
//! storage, IPC and network side effects.

use kilogram_identity::{DeviceId, DeviceIdentity, IdentityError};
use kilogram_session::{
    canonical_sync_v2_activation::{
        CanonicalSyncV2ActivationActor, CanonicalSyncV2ActivationError,
        SignedCanonicalSyncV2ActivationRecord,
    },
    canonical_sync_v2_adapter::{
        CanonicalSyncV2ActivationDisposition, CanonicalSyncV2MigrationState,
    },
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ReleasePolicyId, RolloutActivationDisposition, RolloutActivationTarget,
    RolloutControllerDecision, trusted_policy::TrustedPolicyBundleId,
};

const APPROVAL_VERSION: u8 = 1;
const APPROVAL_SIGNATURE_DOMAIN: &[u8] = b"kilogram:rollout:device-approval:v1\0";
const APPROVAL_ID_DOMAIN: &[u8] = b"kilogram:rollout:device-approval-id:v1\0";
pub const MAX_DEVICE_ROLLOUT_APPROVAL_BYTES: usize = 2_048;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplicitDeviceRolloutApproval {
    NotAuthorized,
    UserOrTestOperatorApproved,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct DeviceRolloutApprovalContent {
    version: u8,
    trusted_policy_bundle_id: [u8; 32],
    policy_id: [u8; 32],
    policy_epoch: u64,
    target: u8,
    disposition: u8,
    local_device_id: [u8; 32],
    expected_previous_record_id: Option<[u8; 32]>,
    expected_previous_epoch: u64,
    candidate_activation_record: Vec<u8>,
    nonce: [u8; 32],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SignedDeviceRolloutApprovalWire {
    content: DeviceRolloutApprovalContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedDeviceRolloutApproval {
    content: DeviceRolloutApprovalContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeviceRolloutApprovalApplyOutcome {
    Ready(SignedCanonicalSyncV2ActivationRecord),
    AlreadyApplied(SignedCanonicalSyncV2ActivationRecord),
}

impl SignedDeviceRolloutApproval {
    pub fn prepare(
        identity: &DeviceIdentity,
        trusted_policy_bundle_id: TrustedPolicyBundleId,
        decision: RolloutControllerDecision,
        current: Option<&SignedCanonicalSyncV2ActivationRecord>,
        authorization: ExplicitDeviceRolloutApproval,
    ) -> Result<Self, DeviceRolloutApprovalError> {
        if authorization != ExplicitDeviceRolloutApproval::UserOrTestOperatorApproved {
            return Err(DeviceRolloutApprovalError::ExplicitApprovalRequired);
        }
        let proposal = proposal_from_decision(decision)?;
        validate_current_device(identity.device_id(), current)?;
        let evidence =
            current.and_then(SignedCanonicalSyncV2ActivationRecord::canonical_evidence_id);
        if proposal.disposition == RolloutActivationDisposition::EmergencyContainment
            && evidence.is_none()
        {
            return Err(DeviceRolloutApprovalError::ContainmentEvidenceRequired);
        }
        let reason = format!(
            "device-approved rollout policy {} bundle {}",
            hex_lower(&proposal.policy_id.as_bytes()),
            hex_lower(&trusted_policy_bundle_id.as_bytes())
        );
        let candidate = SignedCanonicalSyncV2ActivationRecord::issue(
            identity,
            current,
            migration_state(proposal.target),
            activation_disposition(proposal.disposition),
            CanonicalSyncV2ActivationActor::ReleasePolicy,
            evidence,
            reason,
        )?;
        let mut nonce = [0_u8; 32];
        getrandom::fill(&mut nonce).map_err(DeviceRolloutApprovalError::SecureRandom)?;
        Self::issue_with_nonce(
            identity,
            trusted_policy_bundle_id,
            proposal,
            current,
            candidate,
            nonce,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn issue_with_nonce(
        identity: &DeviceIdentity,
        trusted_policy_bundle_id: TrustedPolicyBundleId,
        proposal: RolloutProposal,
        current: Option<&SignedCanonicalSyncV2ActivationRecord>,
        candidate: SignedCanonicalSyncV2ActivationRecord,
        nonce: [u8; 32],
    ) -> Result<Self, DeviceRolloutApprovalError> {
        let expected_previous_record_id = current
            .map(SignedCanonicalSyncV2ActivationRecord::record_id)
            .transpose()?;
        let expected_previous_epoch =
            current.map_or(0, SignedCanonicalSyncV2ActivationRecord::migration_epoch);
        let content = DeviceRolloutApprovalContent {
            version: APPROVAL_VERSION,
            trusted_policy_bundle_id: trusted_policy_bundle_id.as_bytes(),
            policy_id: proposal.policy_id.as_bytes(),
            policy_epoch: proposal.policy_epoch,
            target: target_code(proposal.target),
            disposition: disposition_code(proposal.disposition),
            local_device_id: *identity.device_id().as_bytes(),
            expected_previous_record_id,
            expected_previous_epoch,
            candidate_activation_record: candidate.encode()?,
            nonce,
        };
        validate_content(&content)?;
        let signature = identity.sign(&approval_signing_bytes(&content)?).to_vec();
        let approval = Self { content, signature };
        approval.verify_signature()?;
        Ok(approval)
    }

    pub fn decode_and_verify(
        encoded: &[u8],
        expected_device_id: DeviceId,
    ) -> Result<Self, DeviceRolloutApprovalError> {
        if encoded.is_empty() || encoded.len() > MAX_DEVICE_ROLLOUT_APPROVAL_BYTES {
            return Err(DeviceRolloutApprovalError::ApprovalLength(encoded.len()));
        }
        let wire: SignedDeviceRolloutApprovalWire =
            postcard::from_bytes(encoded).map_err(DeviceRolloutApprovalError::ApprovalCodec)?;
        let approval = Self {
            content: wire.content,
            signature: wire.signature,
        };
        if DeviceId::from_bytes(approval.content.local_device_id) != expected_device_id {
            return Err(DeviceRolloutApprovalError::WrongDevice);
        }
        approval.verify_signature()?;
        if approval.encode()? != encoded {
            return Err(DeviceRolloutApprovalError::NonCanonicalApproval);
        }
        Ok(approval)
    }

    pub fn encode(&self) -> Result<Vec<u8>, DeviceRolloutApprovalError> {
        validate_content(&self.content)?;
        self.verify_signature()?;
        let encoded = postcard::to_allocvec(&SignedDeviceRolloutApprovalWire {
            content: self.content.clone(),
            signature: self.signature.clone(),
        })
        .map_err(DeviceRolloutApprovalError::ApprovalCodec)?;
        if encoded.is_empty() || encoded.len() > MAX_DEVICE_ROLLOUT_APPROVAL_BYTES {
            return Err(DeviceRolloutApprovalError::ApprovalLength(encoded.len()));
        }
        Ok(encoded)
    }

    pub fn approval_id(&self) -> Result<[u8; 32], DeviceRolloutApprovalError> {
        let mut hasher = Sha256::new();
        hasher.update(APPROVAL_ID_DOMAIN);
        hasher.update(self.encode()?);
        Ok(hasher.finalize().into())
    }

    pub fn apply(
        &self,
        trusted_policy_bundle_id: TrustedPolicyBundleId,
        decision: RolloutControllerDecision,
        current: Option<&SignedCanonicalSyncV2ActivationRecord>,
    ) -> Result<DeviceRolloutApprovalApplyOutcome, DeviceRolloutApprovalError> {
        self.verify_signature()?;
        if self.content.trusted_policy_bundle_id != trusted_policy_bundle_id.as_bytes() {
            return Err(DeviceRolloutApprovalError::ProposalBinding);
        }
        let expected_device_id = DeviceId::from_bytes(self.content.local_device_id);
        let candidate = SignedCanonicalSyncV2ActivationRecord::decode_and_verify(
            &self.content.candidate_activation_record,
            expected_device_id,
        )?;
        if candidate.migration_state() != migration_state(target_from_code(self.content.target)?)
            || candidate.disposition()
                != activation_disposition(disposition_from_code(self.content.disposition)?)
            || candidate.actor() != CanonicalSyncV2ActivationActor::ReleasePolicy
        {
            return Err(DeviceRolloutApprovalError::CandidateBinding);
        }

        if let Some(current) = current
            && current.record_id()? == candidate.record_id()?
        {
            if current.encode()? != self.content.candidate_activation_record {
                return Err(DeviceRolloutApprovalError::CandidateBinding);
            }
            return Ok(DeviceRolloutApprovalApplyOutcome::AlreadyApplied(candidate));
        }

        let proposal = proposal_from_decision(decision)?;
        if self.content.policy_id != proposal.policy_id.as_bytes()
            || self.content.policy_epoch != proposal.policy_epoch
            || target_from_code(self.content.target)? != proposal.target
            || disposition_from_code(self.content.disposition)? != proposal.disposition
        {
            return Err(DeviceRolloutApprovalError::ProposalBinding);
        }
        if candidate.migration_state() != migration_state(proposal.target)
            || candidate.disposition() != activation_disposition(proposal.disposition)
            || candidate.actor() != CanonicalSyncV2ActivationActor::ReleasePolicy
        {
            return Err(DeviceRolloutApprovalError::CandidateBinding);
        }

        validate_current_device(expected_device_id, current)?;
        let current_record_id = current
            .map(SignedCanonicalSyncV2ActivationRecord::record_id)
            .transpose()?;
        let current_epoch =
            current.map_or(0, SignedCanonicalSyncV2ActivationRecord::migration_epoch);
        if current_record_id != self.content.expected_previous_record_id
            || current_epoch != self.content.expected_previous_epoch
        {
            return Err(DeviceRolloutApprovalError::StaleActivationHead);
        }
        candidate.verify_transition(current)?;
        Ok(DeviceRolloutApprovalApplyOutcome::Ready(candidate))
    }

    pub fn local_device_id(&self) -> DeviceId {
        DeviceId::from_bytes(self.content.local_device_id)
    }

    pub fn trusted_policy_bundle_id(&self) -> TrustedPolicyBundleId {
        TrustedPolicyBundleId::from_bytes_for_store(self.content.trusted_policy_bundle_id)
    }

    pub fn candidate_activation_record(
        &self,
    ) -> Result<SignedCanonicalSyncV2ActivationRecord, DeviceRolloutApprovalError> {
        SignedCanonicalSyncV2ActivationRecord::decode_and_verify(
            &self.content.candidate_activation_record,
            self.local_device_id(),
        )
        .map_err(DeviceRolloutApprovalError::Activation)
    }

    fn verify_signature(&self) -> Result<(), DeviceRolloutApprovalError> {
        validate_content(&self.content)?;
        self.local_device_id()
            .verify(&approval_signing_bytes(&self.content)?, &self.signature)
            .map_err(DeviceRolloutApprovalError::Signature)
    }
}

#[derive(Clone, Copy)]
struct RolloutProposal {
    policy_id: ReleasePolicyId,
    policy_epoch: u64,
    target: RolloutActivationTarget,
    disposition: RolloutActivationDisposition,
}

fn proposal_from_decision(
    decision: RolloutControllerDecision,
) -> Result<RolloutProposal, DeviceRolloutApprovalError> {
    match decision {
        RolloutControllerDecision::ProposeDeviceActivation {
            policy_id,
            policy_epoch,
            target,
            disposition,
            requires_explicit_device_approval: true,
            release_policy_can_sign_activation: false,
        } => Ok(RolloutProposal {
            policy_id,
            policy_epoch,
            target,
            disposition,
        }),
        _ => Err(DeviceRolloutApprovalError::ControllerDidNotPropose),
    }
}

fn validate_content(
    content: &DeviceRolloutApprovalContent,
) -> Result<(), DeviceRolloutApprovalError> {
    if content.version != APPROVAL_VERSION {
        return Err(DeviceRolloutApprovalError::ApprovalVersion(content.version));
    }
    if content.trusted_policy_bundle_id == [0_u8; 32]
        || content.policy_id == [0_u8; 32]
        || content.policy_epoch == 0
        || content.local_device_id == [0_u8; 32]
        || content.nonce == [0_u8; 32]
    {
        return Err(DeviceRolloutApprovalError::InvalidContent);
    }
    let target = target_from_code(content.target)?;
    let disposition = disposition_from_code(content.disposition)?;
    if target == RolloutActivationTarget::LegacyOnly
        && disposition == RolloutActivationDisposition::Normal
    {
        return Err(DeviceRolloutApprovalError::InvalidContent);
    }
    if content.candidate_activation_record.is_empty()
        || content.candidate_activation_record.len()
            > kilogram_session::canonical_sync_v2_activation::MAX_CANONICAL_SYNC_V2_ACTIVATION_RECORD_BYTES
    {
        return Err(DeviceRolloutApprovalError::CandidateLength(
            content.candidate_activation_record.len(),
        ));
    }
    Ok(())
}

fn approval_signing_bytes(
    content: &DeviceRolloutApprovalContent,
) -> Result<Vec<u8>, DeviceRolloutApprovalError> {
    validate_content(content)?;
    let encoded =
        postcard::to_allocvec(content).map_err(DeviceRolloutApprovalError::ApprovalCodec)?;
    let mut signing = Vec::with_capacity(APPROVAL_SIGNATURE_DOMAIN.len() + encoded.len());
    signing.extend_from_slice(APPROVAL_SIGNATURE_DOMAIN);
    signing.extend_from_slice(&encoded);
    Ok(signing)
}

fn validate_current_device(
    expected: DeviceId,
    current: Option<&SignedCanonicalSyncV2ActivationRecord>,
) -> Result<(), DeviceRolloutApprovalError> {
    if let Some(current) = current
        && current.local_device_id() != expected
    {
        return Err(DeviceRolloutApprovalError::WrongDevice);
    }
    Ok(())
}

const fn migration_state(target: RolloutActivationTarget) -> CanonicalSyncV2MigrationState {
    match target {
        RolloutActivationTarget::LegacyOnly => CanonicalSyncV2MigrationState::LegacyOnly,
        RolloutActivationTarget::AdvertiseCanonicalV2 => {
            CanonicalSyncV2MigrationState::AdvertiseCanonicalV2
        }
        RolloutActivationTarget::DualReaderPreferLegacy => {
            CanonicalSyncV2MigrationState::DualReaderPreferLegacy
        }
        RolloutActivationTarget::DualReaderPreferCanonical => {
            CanonicalSyncV2MigrationState::DualReaderPreferCanonical
        }
    }
}

const fn activation_disposition(
    disposition: RolloutActivationDisposition,
) -> CanonicalSyncV2ActivationDisposition {
    match disposition {
        RolloutActivationDisposition::Normal => CanonicalSyncV2ActivationDisposition::Normal,
        RolloutActivationDisposition::EmergencyContainment => {
            CanonicalSyncV2ActivationDisposition::EmergencyContainment
        }
    }
}

const fn target_code(target: RolloutActivationTarget) -> u8 {
    match target {
        RolloutActivationTarget::LegacyOnly => 1,
        RolloutActivationTarget::AdvertiseCanonicalV2 => 2,
        RolloutActivationTarget::DualReaderPreferLegacy => 3,
        RolloutActivationTarget::DualReaderPreferCanonical => 4,
    }
}

fn target_from_code(code: u8) -> Result<RolloutActivationTarget, DeviceRolloutApprovalError> {
    match code {
        1 => Ok(RolloutActivationTarget::LegacyOnly),
        2 => Ok(RolloutActivationTarget::AdvertiseCanonicalV2),
        3 => Ok(RolloutActivationTarget::DualReaderPreferLegacy),
        4 => Ok(RolloutActivationTarget::DualReaderPreferCanonical),
        _ => Err(DeviceRolloutApprovalError::InvalidContent),
    }
}

const fn disposition_code(disposition: RolloutActivationDisposition) -> u8 {
    match disposition {
        RolloutActivationDisposition::Normal => 1,
        RolloutActivationDisposition::EmergencyContainment => 2,
    }
}

fn disposition_from_code(
    code: u8,
) -> Result<RolloutActivationDisposition, DeviceRolloutApprovalError> {
    match code {
        1 => Ok(RolloutActivationDisposition::Normal),
        2 => Ok(RolloutActivationDisposition::EmergencyContainment),
        _ => Err(DeviceRolloutApprovalError::InvalidContent),
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[derive(Debug, Error)]
pub enum DeviceRolloutApprovalError {
    #[error("explicit local Device rollout approval is required")]
    ExplicitApprovalRequired,
    #[error("rollout controller did not produce an eligible Device activation proposal")]
    ControllerDidNotPropose,
    #[error("rollout containment approval requires authenticated canonical evidence")]
    ContainmentEvidenceRequired,
    #[error("rollout approval is bound to another Device")]
    WrongDevice,
    #[error("rollout approval no longer matches the exact trusted policy proposal")]
    ProposalBinding,
    #[error("rollout approval candidate does not match the proposal")]
    CandidateBinding,
    #[error("rollout approval was prepared against a stale activation head")]
    StaleActivationHead,
    #[error("rollout approval has invalid length {0}")]
    ApprovalLength(usize),
    #[error("rollout approval candidate has invalid length {0}")]
    CandidateLength(usize),
    #[error("rollout approval uses unsupported version {0}")]
    ApprovalVersion(u8),
    #[error("rollout approval content is invalid")]
    InvalidContent,
    #[error("rollout approval encoding is invalid: {0}")]
    ApprovalCodec(postcard::Error),
    #[error("rollout approval is not canonically encoded")]
    NonCanonicalApproval,
    #[error("rollout approval signature is invalid: {0}")]
    Signature(IdentityError),
    #[error("secure random generation failed: {0}")]
    SecureRandom(getrandom::Error),
    #[error(transparent)]
    Activation(#[from] CanonicalSyncV2ActivationError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RolloutControllerCurrentState, RolloutControllerHoldReason};

    fn test_identity(byte: u8) -> DeviceIdentity {
        DeviceIdentity::from_secret_bytes([byte; 32])
    }

    fn bundle(byte: u8) -> TrustedPolicyBundleId {
        TrustedPolicyBundleId::from_bytes_for_store([byte; 32])
    }

    fn proposal(
        target: RolloutActivationTarget,
        disposition: RolloutActivationDisposition,
    ) -> RolloutControllerDecision {
        RolloutControllerDecision::ProposeDeviceActivation {
            policy_id: ReleasePolicyId([7_u8; 32]),
            policy_epoch: 4,
            target,
            disposition,
            requires_explicit_device_approval: true,
            release_policy_can_sign_activation: false,
        }
    }

    fn first_activation(
        identity: &DeviceIdentity,
    ) -> Result<SignedCanonicalSyncV2ActivationRecord, CanonicalSyncV2ActivationError> {
        SignedCanonicalSyncV2ActivationRecord::issue(
            identity,
            None,
            CanonicalSyncV2MigrationState::AdvertiseCanonicalV2,
            CanonicalSyncV2ActivationDisposition::Normal,
            CanonicalSyncV2ActivationActor::ExplicitLocal,
            None,
            "test predecessor",
        )
    }

    #[test]
    fn explicit_device_approval_yields_exact_replay_safe_activation()
    -> Result<(), DeviceRolloutApprovalError> {
        let identity = test_identity(1);
        let current = first_activation(&identity)?;
        let decision = proposal(
            RolloutActivationTarget::DualReaderPreferLegacy,
            RolloutActivationDisposition::Normal,
        );
        let approval = SignedDeviceRolloutApproval::prepare(
            &identity,
            bundle(2),
            decision,
            Some(&current),
            ExplicitDeviceRolloutApproval::UserOrTestOperatorApproved,
        )?;
        let encoded = approval.encode()?;
        let approval =
            SignedDeviceRolloutApproval::decode_and_verify(&encoded, identity.device_id())?;
        let candidate = if let DeviceRolloutApprovalApplyOutcome::Ready(candidate) =
            approval.apply(bundle(2), decision, Some(&current))?
        {
            candidate
        } else {
            return Err(DeviceRolloutApprovalError::CandidateBinding);
        };
        assert_eq!(candidate.migration_epoch(), 2);
        assert_eq!(
            candidate.actor(),
            CanonicalSyncV2ActivationActor::ReleasePolicy
        );
        assert!(matches!(
            approval.apply(bundle(2), decision, Some(&candidate))?,
            DeviceRolloutApprovalApplyOutcome::AlreadyApplied(_)
        ));
        Ok(())
    }

    #[test]
    fn hold_and_missing_explicit_approval_cannot_prepare() -> Result<(), DeviceRolloutApprovalError>
    {
        let identity = test_identity(3);
        let hold = RolloutControllerDecision::Hold {
            reason: RolloutControllerHoldReason::MissingSignedPolicy,
            current: RolloutControllerCurrentState {
                target: RolloutActivationTarget::LegacyOnly,
                disposition: RolloutActivationDisposition::Normal,
                has_authenticated_canonical_evidence: false,
            },
        };
        assert!(matches!(
            SignedDeviceRolloutApproval::prepare(
                &identity,
                bundle(4),
                hold,
                None,
                ExplicitDeviceRolloutApproval::UserOrTestOperatorApproved,
            ),
            Err(DeviceRolloutApprovalError::ControllerDidNotPropose)
        ));
        assert!(matches!(
            SignedDeviceRolloutApproval::prepare(
                &identity,
                bundle(4),
                proposal(
                    RolloutActivationTarget::AdvertiseCanonicalV2,
                    RolloutActivationDisposition::Normal,
                ),
                None,
                ExplicitDeviceRolloutApproval::NotAuthorized,
            ),
            Err(DeviceRolloutApprovalError::ExplicitApprovalRequired)
        ));
        Ok(())
    }

    #[test]
    fn wrong_device_tamper_and_stale_head_fail_closed() -> Result<(), DeviceRolloutApprovalError> {
        let identity = test_identity(5);
        let other = test_identity(6);
        let current = first_activation(&identity)?;
        let decision = proposal(
            RolloutActivationTarget::DualReaderPreferLegacy,
            RolloutActivationDisposition::Normal,
        );
        let approval = SignedDeviceRolloutApproval::prepare(
            &identity,
            bundle(7),
            decision,
            Some(&current),
            ExplicitDeviceRolloutApproval::UserOrTestOperatorApproved,
        )?;
        assert!(matches!(
            SignedDeviceRolloutApproval::decode_and_verify(&approval.encode()?, other.device_id()),
            Err(DeviceRolloutApprovalError::WrongDevice)
        ));
        let mut tampered = approval.encode()?;
        let middle = tampered.len() / 2;
        tampered[middle] ^= 1;
        assert!(
            SignedDeviceRolloutApproval::decode_and_verify(&tampered, identity.device_id())
                .is_err()
        );
        let stale = first_activation(&identity)?;
        let advanced = SignedCanonicalSyncV2ActivationRecord::issue(
            &identity,
            Some(&stale),
            CanonicalSyncV2MigrationState::DualReaderPreferLegacy,
            CanonicalSyncV2ActivationDisposition::Normal,
            CanonicalSyncV2ActivationActor::ExplicitLocal,
            None,
            "independent advance",
        )?;
        assert!(matches!(
            approval.apply(bundle(7), decision, Some(&advanced)),
            Err(DeviceRolloutApprovalError::StaleActivationHead)
        ));
        Ok(())
    }

    #[test]
    fn containment_preserves_evidence_and_never_uses_release_signature()
    -> Result<(), DeviceRolloutApprovalError> {
        let identity = test_identity(8);
        let advertise = first_activation(&identity)?;
        let dual_reader = SignedCanonicalSyncV2ActivationRecord::issue(
            &identity,
            Some(&advertise),
            CanonicalSyncV2MigrationState::DualReaderPreferLegacy,
            CanonicalSyncV2ActivationDisposition::Normal,
            CanonicalSyncV2ActivationActor::ExplicitLocal,
            None,
            "dual reader enabled",
        )?;
        let selected = SignedCanonicalSyncV2ActivationRecord::issue(
            &identity,
            Some(&dual_reader),
            CanonicalSyncV2MigrationState::DualReaderPreferLegacy,
            CanonicalSyncV2ActivationDisposition::Normal,
            CanonicalSyncV2ActivationActor::ExplicitLocal,
            Some([9_u8; 32]),
            "authenticated canonical evidence",
        )?;
        let decision = proposal(
            RolloutActivationTarget::DualReaderPreferLegacy,
            RolloutActivationDisposition::EmergencyContainment,
        );
        let approval = SignedDeviceRolloutApproval::prepare(
            &identity,
            bundle(10),
            decision,
            Some(&selected),
            ExplicitDeviceRolloutApproval::UserOrTestOperatorApproved,
        )?;
        let candidate = if let DeviceRolloutApprovalApplyOutcome::Ready(candidate) =
            approval.apply(bundle(10), decision, Some(&selected))?
        {
            candidate
        } else {
            return Err(DeviceRolloutApprovalError::CandidateBinding);
        };
        assert_eq!(candidate.canonical_evidence_id(), Some([9_u8; 32]));
        assert_eq!(
            candidate.disposition(),
            CanonicalSyncV2ActivationDisposition::EmergencyContainment
        );
        assert!(
            identity
                .device_id()
                .verify(
                    &approval_signing_bytes(&approval.content)?,
                    &approval.signature
                )
                .is_ok()
        );
        Ok(())
    }
}
