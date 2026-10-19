//! Explicit Device-signed recovery from canonical-sync emergency containment.
//!
//! Healthy release policy and local health are prerequisites, never authority.
//! Recovery preserves the exact migration target and authenticated evidence and
//! requires a distinct, explicit, Device-signed ceremony.

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
    RolloutActivationTarget, RolloutDisposition, RolloutError, RolloutHealthCounters,
    RolloutHealthSnapshot, RolloutPhase, rollout_phase_target,
    trust::{ReleaseTrustError, ReleaseTrustRoot},
    trusted_policy::{TrustedPolicyBundleId, TrustedPolicyError, TrustedReleasePolicyBundle},
};

const RECOVERY_VERSION: u8 = 1;
const RECOVERY_SIGNATURE_DOMAIN: &[u8] = b"kilogram:rollout:containment-recovery:v1\0";
const RECOVERY_ID_DOMAIN: &[u8] = b"kilogram:rollout:containment-recovery-id:v1\0";
pub const MAX_CONTAINMENT_RECOVERY_APPROVAL_BYTES: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplicitContainmentRecoveryApproval {
    NotAuthorized,
    UserOrTestOperatorApproved,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RecoveryHealthBinding {
    policy_id: [u8; 32],
    release_artifact_id: [u8; 32],
    phase: RolloutPhase,
    cohort_upper_bound_basis_points: u16,
    counters: RolloutHealthCounters,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ContainmentRecoveryContent {
    version: u8,
    trusted_policy_bundle_id: [u8; 32],
    health: RecoveryHealthBinding,
    local_device_id: [u8; 32],
    containment_record_id: [u8; 32],
    containment_epoch: u64,
    canonical_evidence_id: [u8; 32],
    candidate_activation_record: Vec<u8>,
    nonce: [u8; 32],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SignedContainmentRecoveryApprovalWire {
    content: ContainmentRecoveryContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedContainmentRecoveryApproval {
    content: ContainmentRecoveryContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContainmentRecoveryApplyOutcome {
    Ready(SignedCanonicalSyncV2ActivationRecord),
    AlreadyApplied(SignedCanonicalSyncV2ActivationRecord),
}

impl SignedContainmentRecoveryApproval {
    pub fn prepare(
        identity: &DeviceIdentity,
        trust_root: ReleaseTrustRoot,
        bundle: &TrustedReleasePolicyBundle,
        health: RolloutHealthSnapshot,
        containment: &SignedCanonicalSyncV2ActivationRecord,
        authorization: ExplicitContainmentRecoveryApproval,
    ) -> Result<Self, ContainmentRecoveryError> {
        if authorization != ExplicitContainmentRecoveryApproval::UserOrTestOperatorApproved {
            return Err(ContainmentRecoveryError::ExplicitApprovalRequired);
        }
        bundle.verify(trust_root)?;
        validate_health(bundle, health)?;
        validate_containment(identity.device_id(), containment, bundle.policy().phase())?;
        let evidence = containment
            .canonical_evidence_id()
            .ok_or(ContainmentRecoveryError::CanonicalEvidenceRequired)?;
        let bundle_id = bundle.bundle_id()?;
        let policy_id = bundle.policy().policy_id()?;
        let reason = format!(
            "explicit containment recovery policy {} bundle {}",
            hex_lower(&policy_id.as_bytes()),
            hex_lower(&bundle_id.as_bytes())
        );
        let candidate = SignedCanonicalSyncV2ActivationRecord::issue(
            identity,
            Some(containment),
            containment.migration_state(),
            CanonicalSyncV2ActivationDisposition::Normal,
            CanonicalSyncV2ActivationActor::ExplicitLocal,
            Some(evidence),
            reason,
        )?;
        let mut nonce = [0_u8; 32];
        getrandom::fill(&mut nonce).map_err(ContainmentRecoveryError::SecureRandom)?;
        let content = ContainmentRecoveryContent {
            version: RECOVERY_VERSION,
            trusted_policy_bundle_id: bundle_id.as_bytes(),
            health: health_binding(health),
            local_device_id: *identity.device_id().as_bytes(),
            containment_record_id: containment.record_id()?,
            containment_epoch: containment.migration_epoch(),
            canonical_evidence_id: evidence,
            candidate_activation_record: candidate.encode()?,
            nonce,
        };
        validate_content(&content)?;
        let signature = identity.sign(&recovery_signing_bytes(&content)?).to_vec();
        let approval = Self { content, signature };
        approval.verify_signature()?;
        Ok(approval)
    }

    pub fn decode_and_verify(
        encoded: &[u8],
        expected_device_id: DeviceId,
    ) -> Result<Self, ContainmentRecoveryError> {
        if encoded.is_empty() || encoded.len() > MAX_CONTAINMENT_RECOVERY_APPROVAL_BYTES {
            return Err(ContainmentRecoveryError::ApprovalLength(encoded.len()));
        }
        let wire: SignedContainmentRecoveryApprovalWire =
            postcard::from_bytes(encoded).map_err(ContainmentRecoveryError::ApprovalCodec)?;
        let approval = Self {
            content: wire.content,
            signature: wire.signature,
        };
        if approval.local_device_id() != expected_device_id {
            return Err(ContainmentRecoveryError::WrongDevice);
        }
        approval.verify_signature()?;
        if approval.encode()? != encoded {
            return Err(ContainmentRecoveryError::NonCanonicalApproval);
        }
        Ok(approval)
    }

    pub fn encode(&self) -> Result<Vec<u8>, ContainmentRecoveryError> {
        validate_content(&self.content)?;
        self.verify_signature()?;
        let encoded = postcard::to_allocvec(&SignedContainmentRecoveryApprovalWire {
            content: self.content.clone(),
            signature: self.signature.clone(),
        })
        .map_err(ContainmentRecoveryError::ApprovalCodec)?;
        if encoded.is_empty() || encoded.len() > MAX_CONTAINMENT_RECOVERY_APPROVAL_BYTES {
            return Err(ContainmentRecoveryError::ApprovalLength(encoded.len()));
        }
        Ok(encoded)
    }

    pub fn recovery_id(&self) -> Result<[u8; 32], ContainmentRecoveryError> {
        let mut hasher = Sha256::new();
        hasher.update(RECOVERY_ID_DOMAIN);
        hasher.update(self.encode()?);
        Ok(hasher.finalize().into())
    }

    pub fn apply(
        &self,
        trust_root: ReleaseTrustRoot,
        bundle: &TrustedReleasePolicyBundle,
        health: RolloutHealthSnapshot,
        current: &SignedCanonicalSyncV2ActivationRecord,
    ) -> Result<ContainmentRecoveryApplyOutcome, ContainmentRecoveryError> {
        self.verify_signature()?;
        bundle.verify(trust_root)?;
        validate_health(bundle, health)?;
        if self.content.trusted_policy_bundle_id != bundle.bundle_id()?.as_bytes()
            || self.content.health != health_binding(health)
        {
            return Err(ContainmentRecoveryError::PolicyHealthBinding);
        }
        let device_id = self.local_device_id();
        let candidate = SignedCanonicalSyncV2ActivationRecord::decode_and_verify(
            &self.content.candidate_activation_record,
            device_id,
        )?;
        if current.record_id()? == candidate.record_id()? {
            if current.encode()? != self.content.candidate_activation_record {
                return Err(ContainmentRecoveryError::CandidateBinding);
            }
            return Ok(ContainmentRecoveryApplyOutcome::AlreadyApplied(candidate));
        }

        validate_containment(device_id, current, bundle.policy().phase())?;
        if current.record_id()? != self.content.containment_record_id
            || current.migration_epoch() != self.content.containment_epoch
            || current.canonical_evidence_id() != Some(self.content.canonical_evidence_id)
        {
            return Err(ContainmentRecoveryError::StaleContainmentHead);
        }
        if candidate.migration_state() != current.migration_state()
            || candidate.disposition() != CanonicalSyncV2ActivationDisposition::Normal
            || candidate.actor() != CanonicalSyncV2ActivationActor::ExplicitLocal
            || candidate.canonical_evidence_id() != current.canonical_evidence_id()
        {
            return Err(ContainmentRecoveryError::CandidateBinding);
        }
        candidate.verify_transition(Some(current))?;
        Ok(ContainmentRecoveryApplyOutcome::Ready(candidate))
    }

    pub fn local_device_id(&self) -> DeviceId {
        DeviceId::from_bytes(self.content.local_device_id)
    }

    pub fn trusted_policy_bundle_id(&self) -> TrustedPolicyBundleId {
        TrustedPolicyBundleId::from_bytes_for_store(self.content.trusted_policy_bundle_id)
    }

    pub fn candidate_activation_record(
        &self,
    ) -> Result<SignedCanonicalSyncV2ActivationRecord, ContainmentRecoveryError> {
        SignedCanonicalSyncV2ActivationRecord::decode_and_verify(
            &self.content.candidate_activation_record,
            self.local_device_id(),
        )
        .map_err(ContainmentRecoveryError::Activation)
    }

    fn verify_signature(&self) -> Result<(), ContainmentRecoveryError> {
        validate_content(&self.content)?;
        self.local_device_id()
            .verify(&recovery_signing_bytes(&self.content)?, &self.signature)
            .map_err(ContainmentRecoveryError::Signature)
    }
}

fn validate_containment(
    expected_device_id: DeviceId,
    containment: &SignedCanonicalSyncV2ActivationRecord,
    policy_phase: RolloutPhase,
) -> Result<(), ContainmentRecoveryError> {
    containment.verify_signature()?;
    if containment.local_device_id() != expected_device_id {
        return Err(ContainmentRecoveryError::WrongDevice);
    }
    if containment.disposition() != CanonicalSyncV2ActivationDisposition::EmergencyContainment {
        return Err(ContainmentRecoveryError::ContainmentRequired);
    }
    if containment.canonical_evidence_id().is_none() {
        return Err(ContainmentRecoveryError::CanonicalEvidenceRequired);
    }
    let target = activation_target(containment.migration_state())?;
    if rollout_phase_target(policy_phase).ordinal() < target.ordinal() {
        return Err(ContainmentRecoveryError::PolicyBelowContainedState);
    }
    Ok(())
}

fn validate_health(
    bundle: &TrustedReleasePolicyBundle,
    health: RolloutHealthSnapshot,
) -> Result<(), ContainmentRecoveryError> {
    let policy = bundle.policy();
    if policy.disposition() != RolloutDisposition::Normal
        || policy.phase() == RolloutPhase::LegacyOnlyHold
    {
        return Err(ContainmentRecoveryError::NormalRecoveryPolicyRequired);
    }
    if health.policy_id != policy.policy_id()?
        || health.release_artifact_id != policy.release_artifact_id()
        || health.phase != policy.phase()
        || health.cohort_upper_bound_basis_points != policy.cohort_upper_bound_basis_points()
    {
        return Err(ContainmentRecoveryError::PolicyHealthBinding);
    }
    let counters = health.counters;
    if counters.runtime_start_succeeded == 0
        || counters.canonical_sync_completed == 0
        || counters.runtime_start_failed != 0
        || counters.legacy_sync_failed != 0
        || counters.canonical_sync_failed != 0
        || counters.canonical_sync_paused != 0
        || counters.canonical_sync_interrupted != 0
        || counters.zero_tolerance_events()? != 0
    {
        return Err(ContainmentRecoveryError::HealthyRecoveryEvidenceRequired);
    }
    Ok(())
}

fn health_binding(health: RolloutHealthSnapshot) -> RecoveryHealthBinding {
    RecoveryHealthBinding {
        policy_id: health.policy_id.as_bytes(),
        release_artifact_id: health.release_artifact_id.as_bytes(),
        phase: health.phase,
        cohort_upper_bound_basis_points: health.cohort_upper_bound_basis_points,
        counters: health.counters,
    }
}

fn validate_content(content: &ContainmentRecoveryContent) -> Result<(), ContainmentRecoveryError> {
    if content.version != RECOVERY_VERSION {
        return Err(ContainmentRecoveryError::ApprovalVersion(content.version));
    }
    if content.trusted_policy_bundle_id == [0_u8; 32]
        || content.health.policy_id == [0_u8; 32]
        || content.health.release_artifact_id == [0_u8; 32]
        || content.local_device_id == [0_u8; 32]
        || content.containment_record_id == [0_u8; 32]
        || content.containment_epoch == 0
        || content.canonical_evidence_id == [0_u8; 32]
        || content.nonce == [0_u8; 32]
        || content.candidate_activation_record.is_empty()
        || content.candidate_activation_record.len()
            > kilogram_session::canonical_sync_v2_activation::MAX_CANONICAL_SYNC_V2_ACTIVATION_RECORD_BYTES
    {
        return Err(ContainmentRecoveryError::InvalidContent);
    }
    Ok(())
}

fn recovery_signing_bytes(
    content: &ContainmentRecoveryContent,
) -> Result<Vec<u8>, ContainmentRecoveryError> {
    validate_content(content)?;
    let encoded =
        postcard::to_allocvec(content).map_err(ContainmentRecoveryError::ApprovalCodec)?;
    let mut signing = Vec::with_capacity(RECOVERY_SIGNATURE_DOMAIN.len() + encoded.len());
    signing.extend_from_slice(RECOVERY_SIGNATURE_DOMAIN);
    signing.extend_from_slice(&encoded);
    Ok(signing)
}

fn activation_target(
    state: CanonicalSyncV2MigrationState,
) -> Result<RolloutActivationTarget, ContainmentRecoveryError> {
    match state {
        CanonicalSyncV2MigrationState::LegacyOnly => Ok(RolloutActivationTarget::LegacyOnly),
        CanonicalSyncV2MigrationState::AdvertiseCanonicalV2 => {
            Ok(RolloutActivationTarget::AdvertiseCanonicalV2)
        }
        CanonicalSyncV2MigrationState::DualReaderPreferLegacy => {
            Ok(RolloutActivationTarget::DualReaderPreferLegacy)
        }
        CanonicalSyncV2MigrationState::DualReaderPreferCanonical => {
            Ok(RolloutActivationTarget::DualReaderPreferCanonical)
        }
        CanonicalSyncV2MigrationState::CanonicalRequired => {
            Err(ContainmentRecoveryError::UnmanagedCanonicalRequired)
        }
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
pub enum ContainmentRecoveryError {
    #[error("explicit local Device containment-recovery approval is required")]
    ExplicitApprovalRequired,
    #[error("current activation is not emergency containment")]
    ContainmentRequired,
    #[error("containment recovery requires preserved authenticated canonical evidence")]
    CanonicalEvidenceRequired,
    #[error("containment recovery requires a healthy normal release policy")]
    NormalRecoveryPolicyRequired,
    #[error("containment recovery requires bounded healthy local evidence")]
    HealthyRecoveryEvidenceRequired,
    #[error("recovery health does not match the exact trusted policy bundle")]
    PolicyHealthBinding,
    #[error("normal recovery policy is below the currently contained migration state")]
    PolicyBelowContainedState,
    #[error("canonical-required is outside the public rollout recovery controller")]
    UnmanagedCanonicalRequired,
    #[error("containment recovery is bound to another Device")]
    WrongDevice,
    #[error("containment recovery was prepared against a stale activation head")]
    StaleContainmentHead,
    #[error("containment recovery candidate does not preserve exact authority")]
    CandidateBinding,
    #[error("containment recovery approval has invalid length {0}")]
    ApprovalLength(usize),
    #[error("containment recovery approval uses unsupported version {0}")]
    ApprovalVersion(u8),
    #[error("containment recovery approval content is invalid")]
    InvalidContent,
    #[error("containment recovery approval encoding is invalid: {0}")]
    ApprovalCodec(postcard::Error),
    #[error("containment recovery approval is not canonically encoded")]
    NonCanonicalApproval,
    #[error("containment recovery approval signature is invalid: {0}")]
    Signature(IdentityError),
    #[error("secure random generation failed: {0}")]
    SecureRandom(getrandom::Error),
    #[error(transparent)]
    TrustedPolicy(#[from] TrustedPolicyError),
    #[error(transparent)]
    Trust(#[from] ReleaseTrustError),
    #[error(transparent)]
    Rollout(#[from] RolloutError),
    #[error(transparent)]
    Activation(#[from] CanonicalSyncV2ActivationError),
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;

    use super::*;
    use crate::{
        ReleaseArtifactId,
        trust::{ReleaseSigningKeySpec, ReleaseSigningPermissions, SignedReleaseKeyring},
    };

    fn test_identity(byte: u8) -> DeviceIdentity {
        DeviceIdentity::from_secret_bytes([byte; 32])
    }

    fn root(key: &SigningKey) -> Result<ReleaseTrustRoot, crate::trust::ReleaseTrustError> {
        ReleaseTrustRoot::from_bytes(key.verifying_key().to_bytes())
    }

    fn recovery_bundle()
    -> Result<(ReleaseTrustRoot, TrustedReleasePolicyBundle), ContainmentRecoveryError> {
        let root_key = SigningKey::from_bytes(&[20_u8; 32]);
        let policy_key = SigningKey::from_bytes(&[21_u8; 32]);
        let trust_root = root(&root_key)?;
        let keyring = SignedReleaseKeyring::issue_genesis(
            &root_key,
            &[ReleaseSigningKeySpec::from_signing_key(
                &policy_key,
                ReleaseSigningPermissions::NORMAL_AND_CONTAINMENT,
            )],
        )?;
        let genesis = TrustedReleasePolicyBundle::issue_genesis_hold(
            trust_root,
            &keyring,
            &policy_key,
            ReleaseArtifactId::from_bytes([22_u8; 32])?,
        )?;
        let advertise = TrustedReleasePolicyBundle::issue_successor(
            trust_root,
            &keyring,
            &policy_key,
            &genesis,
            ReleaseArtifactId::from_bytes([22_u8; 32])?,
            RolloutPhase::AdvertiseCanary,
            RolloutDisposition::Normal,
            50,
        )?;
        let accept = TrustedReleasePolicyBundle::issue_successor(
            trust_root,
            &keyring,
            &policy_key,
            &advertise,
            ReleaseArtifactId::from_bytes([22_u8; 32])?,
            RolloutPhase::AcceptCanary,
            RolloutDisposition::Normal,
            50,
        )?;
        Ok((trust_root, accept))
    }

    fn healthy_snapshot(
        bundle: &TrustedReleasePolicyBundle,
    ) -> Result<RolloutHealthSnapshot, ContainmentRecoveryError> {
        Ok(RolloutHealthSnapshot {
            policy_id: bundle.policy().policy_id()?,
            release_artifact_id: bundle.policy().release_artifact_id(),
            phase: bundle.policy().phase(),
            cohort_upper_bound_basis_points: bundle.policy().cohort_upper_bound_basis_points(),
            counters: RolloutHealthCounters {
                runtime_start_succeeded: 1,
                canonical_sync_completed: 2,
                containment_entered: 1,
                ..RolloutHealthCounters::default()
            },
        })
    }

    fn contained_head(
        identity: &DeviceIdentity,
    ) -> Result<SignedCanonicalSyncV2ActivationRecord, CanonicalSyncV2ActivationError> {
        let advertise = SignedCanonicalSyncV2ActivationRecord::issue(
            identity,
            None,
            CanonicalSyncV2MigrationState::AdvertiseCanonicalV2,
            CanonicalSyncV2ActivationDisposition::Normal,
            CanonicalSyncV2ActivationActor::ExplicitLocal,
            None,
            "advertise",
        )?;
        let dual = SignedCanonicalSyncV2ActivationRecord::issue(
            identity,
            Some(&advertise),
            CanonicalSyncV2MigrationState::DualReaderPreferLegacy,
            CanonicalSyncV2ActivationDisposition::Normal,
            CanonicalSyncV2ActivationActor::ExplicitLocal,
            None,
            "dual reader",
        )?;
        let evidence = SignedCanonicalSyncV2ActivationRecord::issue(
            identity,
            Some(&dual),
            CanonicalSyncV2MigrationState::DualReaderPreferLegacy,
            CanonicalSyncV2ActivationDisposition::Normal,
            CanonicalSyncV2ActivationActor::ExplicitLocal,
            Some([23_u8; 32]),
            "evidence",
        )?;
        SignedCanonicalSyncV2ActivationRecord::issue(
            identity,
            Some(&evidence),
            CanonicalSyncV2MigrationState::DualReaderPreferLegacy,
            CanonicalSyncV2ActivationDisposition::EmergencyContainment,
            CanonicalSyncV2ActivationActor::ExplicitLocal,
            Some([23_u8; 32]),
            "containment",
        )
    }

    #[test]
    fn explicit_recovery_is_device_signed_exact_and_replay_safe()
    -> Result<(), ContainmentRecoveryError> {
        let identity = test_identity(24);
        let (trust_root, bundle) = recovery_bundle()?;
        let health = healthy_snapshot(&bundle)?;
        let containment = contained_head(&identity)?;
        let approval = SignedContainmentRecoveryApproval::prepare(
            &identity,
            trust_root,
            &bundle,
            health,
            &containment,
            ExplicitContainmentRecoveryApproval::UserOrTestOperatorApproved,
        )?;
        let encoded = approval.encode()?;
        let approval =
            SignedContainmentRecoveryApproval::decode_and_verify(&encoded, identity.device_id())?;
        let recovered = if let ContainmentRecoveryApplyOutcome::Ready(recovered) =
            approval.apply(trust_root, &bundle, health, &containment)?
        {
            recovered
        } else {
            return Err(ContainmentRecoveryError::CandidateBinding);
        };
        assert_eq!(
            recovered.disposition(),
            CanonicalSyncV2ActivationDisposition::Normal
        );
        assert_eq!(recovered.migration_state(), containment.migration_state());
        assert_eq!(
            recovered.canonical_evidence_id(),
            containment.canonical_evidence_id()
        );
        assert!(matches!(
            approval.apply(trust_root, &bundle, health, &recovered)?,
            ContainmentRecoveryApplyOutcome::AlreadyApplied(_)
        ));
        Ok(())
    }

    #[test]
    fn missing_approval_and_unhealthy_evidence_fail_closed() -> Result<(), ContainmentRecoveryError>
    {
        let identity = test_identity(25);
        let (trust_root, bundle) = recovery_bundle()?;
        let health = healthy_snapshot(&bundle)?;
        let containment = contained_head(&identity)?;
        assert!(matches!(
            SignedContainmentRecoveryApproval::prepare(
                &identity,
                trust_root,
                &bundle,
                health,
                &containment,
                ExplicitContainmentRecoveryApproval::NotAuthorized,
            ),
            Err(ContainmentRecoveryError::ExplicitApprovalRequired)
        ));
        let mut unhealthy = health;
        unhealthy.counters.canonical_sync_failed = 1;
        assert!(matches!(
            SignedContainmentRecoveryApproval::prepare(
                &identity,
                trust_root,
                &bundle,
                unhealthy,
                &containment,
                ExplicitContainmentRecoveryApproval::UserOrTestOperatorApproved,
            ),
            Err(ContainmentRecoveryError::HealthyRecoveryEvidenceRequired)
        ));
        Ok(())
    }

    #[test]
    fn changed_health_device_and_head_fail_closed() -> Result<(), ContainmentRecoveryError> {
        let identity = test_identity(26);
        let other = test_identity(27);
        let (trust_root, bundle) = recovery_bundle()?;
        let health = healthy_snapshot(&bundle)?;
        let containment = contained_head(&identity)?;
        let approval = SignedContainmentRecoveryApproval::prepare(
            &identity,
            trust_root,
            &bundle,
            health,
            &containment,
            ExplicitContainmentRecoveryApproval::UserOrTestOperatorApproved,
        )?;
        assert!(matches!(
            SignedContainmentRecoveryApproval::decode_and_verify(
                &approval.encode()?,
                other.device_id()
            ),
            Err(ContainmentRecoveryError::WrongDevice)
        ));
        let mut changed = health;
        changed.counters.canonical_sync_completed = 3;
        assert!(matches!(
            approval.apply(trust_root, &bundle, changed, &containment),
            Err(ContainmentRecoveryError::PolicyHealthBinding)
        ));
        let stale = contained_head(&identity)?;
        let newer = SignedCanonicalSyncV2ActivationRecord::issue(
            &identity,
            Some(&stale),
            stale.migration_state(),
            CanonicalSyncV2ActivationDisposition::Normal,
            CanonicalSyncV2ActivationActor::ExplicitLocal,
            stale.canonical_evidence_id(),
            "another recovery",
        )?;
        assert!(matches!(
            approval.apply(trust_root, &bundle, health, &newer),
            Err(ContainmentRecoveryError::ContainmentRequired)
                | Err(ContainmentRecoveryError::StaleContainmentHead)
        ));
        Ok(())
    }

    #[test]
    fn tamper_and_canonical_required_are_rejected() -> Result<(), ContainmentRecoveryError> {
        let identity = test_identity(28);
        let (trust_root, bundle) = recovery_bundle()?;
        let health = healthy_snapshot(&bundle)?;
        let containment = contained_head(&identity)?;
        let approval = SignedContainmentRecoveryApproval::prepare(
            &identity,
            trust_root,
            &bundle,
            health,
            &containment,
            ExplicitContainmentRecoveryApproval::UserOrTestOperatorApproved,
        )?;
        let mut encoded = approval.encode()?;
        let middle = encoded.len() / 2;
        encoded[middle] ^= 1;
        assert!(
            SignedContainmentRecoveryApproval::decode_and_verify(&encoded, identity.device_id())
                .is_err()
        );
        assert!(matches!(
            activation_target(CanonicalSyncV2MigrationState::CanonicalRequired),
            Err(ContainmentRecoveryError::UnmanagedCanonicalRequired)
        ));
        Ok(())
    }
}
