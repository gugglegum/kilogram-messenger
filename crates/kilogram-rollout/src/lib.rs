//! Dormant, installation-local authority for a privacy-preserving rollout.
//!
//! This crate deliberately has no network or runtime dependency. Merely adding
//! it to a build cannot enroll an installation or change Kilogram's public
//! `legacy-only` default. A caller must verify and durably install a release
//! policy, explicitly evaluate the installation-local cohort, and separately
//! translate that result into the existing Device-bound activation ceremony.
//! That last integration does not exist in this milestone.

use std::{
    fmt,
    path::{Path, PathBuf},
};

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use redb::{
    Database, Durability, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const DATABASE_FILE: &str = "rollout-local-v1.redb";
const META_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("rollout-meta-v1");
const HEALTH_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("rollout-health-v1");

const DATABASE_SCHEMA_KEY: &str = "schema-version";
const RELEASE_AUTHORITY_KEY: &str = "release-authority";
const INSTALLATION_SEED_KEY: &str = "installation-cohort-seed";
const POLICY_HEAD_KEY: &str = "signed-release-policy-head";
const DATABASE_SCHEMA_VERSION: u8 = 1;

const RELEASE_POLICY_VERSION: u8 = 1;
const RELEASE_POLICY_SIGNATURE_DOMAIN: &[u8] = b"kilogram:rollout:release-policy:v1\0";
const RELEASE_POLICY_ID_DOMAIN: &[u8] = b"kilogram:rollout:release-policy-id:v1\0";
const HEALTH_RECORD_VERSION: u8 = 1;
const HEALTH_RECORD_MAC_DOMAIN: &[u8] = b"kilogram:rollout:health-record:v1\0";

pub const COHORT_BUCKETS: u16 = 10_000;
pub const MAX_DURABLE_HEALTH_AGGREGATES: u64 = 16;
pub const MAX_SIGNED_RELEASE_POLICY_BYTES: usize = 512;
pub const MAX_DURABLE_HEALTH_RECORD_BYTES: usize = 2_048;

/// This stage cannot activate canonical sync v2 by construction.
pub const RUNTIME_ROLLOUT_INTEGRATION_ENABLED: bool = false;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleasePolicyAuthority([u8; 32]);

impl ReleasePolicyAuthority {
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, RolloutError> {
        VerifyingKey::from_bytes(&bytes).map_err(RolloutError::InvalidReleaseAuthority)?;
        Ok(Self(bytes))
    }

    pub fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    fn verifying_key(self) -> Result<VerifyingKey, RolloutError> {
        VerifyingKey::from_bytes(&self.0).map_err(RolloutError::InvalidReleaseAuthority)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReleaseArtifactId([u8; 32]);

impl ReleaseArtifactId {
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, RolloutError> {
        if bytes == [0_u8; 32] {
            return Err(RolloutError::ZeroReleaseArtifactId);
        }
        Ok(Self(bytes))
    }

    pub fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReleasePolicyId([u8; 32]);

impl ReleasePolicyId {
    pub fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for ReleasePolicyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ReleasePolicyId")
            .field(&hex_lower(&self.0))
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RolloutPhase {
    LegacyOnlyHold,
    AdvertiseCanary,
    AcceptCanary,
    InitiateCanary,
    ExpandedDualReader,
}

impl RolloutPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LegacyOnlyHold => "legacy-only-hold",
            Self::AdvertiseCanary => "advertise-canary",
            Self::AcceptCanary => "accept-canary",
            Self::InitiateCanary => "initiate-canary",
            Self::ExpandedDualReader => "expanded-dual-reader",
        }
    }

    pub const fn maximum_cohort_basis_points(self) -> u16 {
        match self {
            Self::LegacyOnlyHold => 0,
            Self::AdvertiseCanary | Self::AcceptCanary => 100,
            Self::InitiateCanary => 500,
            Self::ExpandedDualReader => 2_500,
        }
    }

    const fn ordinal(self) -> u8 {
        match self {
            Self::LegacyOnlyHold => 0,
            Self::AdvertiseCanary => 1,
            Self::AcceptCanary => 2,
            Self::InitiateCanary => 3,
            Self::ExpandedDualReader => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RolloutDisposition {
    Normal,
    Containment,
}

impl RolloutDisposition {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Containment => "containment",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ReleasePolicyContent {
    version: u8,
    policy_epoch: u64,
    previous_policy_id: Option<[u8; 32]>,
    release_artifact_id: ReleaseArtifactId,
    phase: RolloutPhase,
    disposition: RolloutDisposition,
    cohort_upper_bound_basis_points: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedReleasePolicy {
    content: ReleasePolicyContent,
    signature: Vec<u8>,
}

#[derive(Deserialize, Serialize)]
struct SignedReleasePolicyWire {
    content: ReleasePolicyContent,
    signature: Vec<u8>,
}

impl SignedReleasePolicy {
    pub fn issue_genesis_hold(
        signing_key: &SigningKey,
        release_artifact_id: ReleaseArtifactId,
    ) -> Result<Self, RolloutError> {
        Self::issue(
            signing_key,
            None,
            release_artifact_id,
            RolloutPhase::LegacyOnlyHold,
            RolloutDisposition::Normal,
            0,
        )
    }

    pub fn issue_successor(
        signing_key: &SigningKey,
        previous: &Self,
        release_artifact_id: ReleaseArtifactId,
        phase: RolloutPhase,
        disposition: RolloutDisposition,
        cohort_upper_bound_basis_points: u16,
    ) -> Result<Self, RolloutError> {
        Self::issue(
            signing_key,
            Some(previous),
            release_artifact_id,
            phase,
            disposition,
            cohort_upper_bound_basis_points,
        )
    }

    fn issue(
        signing_key: &SigningKey,
        previous: Option<&Self>,
        release_artifact_id: ReleaseArtifactId,
        phase: RolloutPhase,
        disposition: RolloutDisposition,
        cohort_upper_bound_basis_points: u16,
    ) -> Result<Self, RolloutError> {
        let authority = ReleasePolicyAuthority::from_bytes(signing_key.verifying_key().to_bytes())?;
        let (policy_epoch, previous_policy_id) = match previous {
            Some(previous) => {
                previous.verify(authority)?;
                (
                    previous
                        .policy_epoch()
                        .checked_add(1)
                        .ok_or(RolloutError::PolicyEpochOverflow)?,
                    Some(previous.policy_id()?.as_bytes()),
                )
            }
            None => (1, None),
        };
        let content = ReleasePolicyContent {
            version: RELEASE_POLICY_VERSION,
            policy_epoch,
            previous_policy_id,
            release_artifact_id,
            phase,
            disposition,
            cohort_upper_bound_basis_points,
        };
        validate_policy_content(&content)?;
        validate_policy_transition(&content, previous)?;
        let signature = signing_key
            .sign(&policy_signing_bytes(&content)?)
            .to_bytes()
            .to_vec();
        let policy = Self { content, signature };
        policy.verify(authority)?;
        Ok(policy)
    }

    pub fn decode_and_verify(
        encoded: &[u8],
        authority: ReleasePolicyAuthority,
    ) -> Result<Self, RolloutError> {
        if encoded.is_empty() || encoded.len() > MAX_SIGNED_RELEASE_POLICY_BYTES {
            return Err(RolloutError::PolicyLength(encoded.len()));
        }
        let wire: SignedReleasePolicyWire =
            postcard::from_bytes(encoded).map_err(RolloutError::PolicyCodec)?;
        let policy = Self {
            content: wire.content,
            signature: wire.signature,
        };
        policy.verify(authority)?;
        if policy.encode()? != encoded {
            return Err(RolloutError::NonCanonicalPolicy);
        }
        Ok(policy)
    }

    pub fn encode(&self) -> Result<Vec<u8>, RolloutError> {
        validate_policy_content(&self.content)?;
        let wire = SignedReleasePolicyWire {
            content: self.content.clone(),
            signature: self.signature.clone(),
        };
        let encoded = postcard::to_allocvec(&wire).map_err(RolloutError::PolicyCodec)?;
        if encoded.is_empty() || encoded.len() > MAX_SIGNED_RELEASE_POLICY_BYTES {
            return Err(RolloutError::PolicyLength(encoded.len()));
        }
        Ok(encoded)
    }

    pub fn verify(&self, authority: ReleasePolicyAuthority) -> Result<(), RolloutError> {
        validate_policy_content(&self.content)?;
        let signature = Signature::try_from(self.signature.as_slice())
            .map_err(RolloutError::InvalidPolicySignature)?;
        authority
            .verifying_key()?
            .verify_strict(&policy_signing_bytes(&self.content)?, &signature)
            .map_err(RolloutError::PolicyAuthorization)
    }

    pub fn policy_id(&self) -> Result<ReleasePolicyId, RolloutError> {
        let encoded = self.encode()?;
        let mut hasher = Sha256::new();
        hasher.update(RELEASE_POLICY_ID_DOMAIN);
        hasher.update(encoded);
        Ok(ReleasePolicyId(hasher.finalize().into()))
    }

    pub fn policy_epoch(&self) -> u64 {
        self.content.policy_epoch
    }

    pub fn previous_policy_id(&self) -> Option<ReleasePolicyId> {
        self.content.previous_policy_id.map(ReleasePolicyId)
    }

    pub fn release_artifact_id(&self) -> ReleaseArtifactId {
        self.content.release_artifact_id
    }

    pub fn phase(&self) -> RolloutPhase {
        self.content.phase
    }

    pub fn disposition(&self) -> RolloutDisposition {
        self.content.disposition
    }

    pub fn cohort_upper_bound_basis_points(&self) -> u16 {
        self.content.cohort_upper_bound_basis_points
    }

    pub fn verify_successor_of(&self, previous: &Self) -> Result<(), RolloutError> {
        validate_policy_transition(&self.content, Some(previous))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CohortEvaluation {
    pub policy_id: ReleasePolicyId,
    pub policy_epoch: u64,
    pub phase: RolloutPhase,
    pub disposition: RolloutDisposition,
    pub cohort_bucket: u16,
    pub cohort_upper_bound_basis_points: u16,
    pub selected: bool,
    /// Always false until a later milestone explicitly joins this authority to
    /// the Device-bound runtime activation ceremony.
    pub runtime_activation_authorized: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RolloutHealthEvent {
    RuntimeStartSucceeded,
    RuntimeStartFailed,
    LegacySyncCompleted,
    LegacySyncFailed,
    CanonicalSyncCompleted,
    CanonicalSyncFailed,
    CanonicalSyncPaused,
    CanonicalSyncInterrupted,
    ContainmentEntered,
    HistoryLossOrDuplication,
    SelectedCanonicalLegacyFallback,
    AuthorityOrDeviceBindingMismatch,
    AuthenticatedDowngradeAcceptance,
    UnrecoverableStateCorruption,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RolloutHealthCounters {
    pub runtime_start_succeeded: u64,
    pub runtime_start_failed: u64,
    pub legacy_sync_completed: u64,
    pub legacy_sync_failed: u64,
    pub canonical_sync_completed: u64,
    pub canonical_sync_failed: u64,
    pub canonical_sync_paused: u64,
    pub canonical_sync_interrupted: u64,
    pub containment_entered: u64,
    pub history_loss_or_duplication: u64,
    pub selected_canonical_legacy_fallback: u64,
    pub authority_or_device_binding_mismatch: u64,
    pub authenticated_downgrade_acceptance: u64,
    pub unrecoverable_state_corruption: u64,
}

impl RolloutHealthCounters {
    pub fn zero_tolerance_events(self) -> Result<u64, RolloutError> {
        [
            self.history_loss_or_duplication,
            self.selected_canonical_legacy_fallback,
            self.authority_or_device_binding_mismatch,
            self.authenticated_downgrade_acceptance,
            self.unrecoverable_state_corruption,
        ]
        .into_iter()
        .try_fold(0_u64, |total, value| {
            total
                .checked_add(value)
                .ok_or(RolloutError::HealthCounterOverflow)
        })
    }

    fn increment(&mut self, event: RolloutHealthEvent) -> Result<(), RolloutError> {
        let counter = match event {
            RolloutHealthEvent::RuntimeStartSucceeded => &mut self.runtime_start_succeeded,
            RolloutHealthEvent::RuntimeStartFailed => &mut self.runtime_start_failed,
            RolloutHealthEvent::LegacySyncCompleted => &mut self.legacy_sync_completed,
            RolloutHealthEvent::LegacySyncFailed => &mut self.legacy_sync_failed,
            RolloutHealthEvent::CanonicalSyncCompleted => &mut self.canonical_sync_completed,
            RolloutHealthEvent::CanonicalSyncFailed => &mut self.canonical_sync_failed,
            RolloutHealthEvent::CanonicalSyncPaused => &mut self.canonical_sync_paused,
            RolloutHealthEvent::CanonicalSyncInterrupted => &mut self.canonical_sync_interrupted,
            RolloutHealthEvent::ContainmentEntered => &mut self.containment_entered,
            RolloutHealthEvent::HistoryLossOrDuplication => &mut self.history_loss_or_duplication,
            RolloutHealthEvent::SelectedCanonicalLegacyFallback => {
                &mut self.selected_canonical_legacy_fallback
            }
            RolloutHealthEvent::AuthorityOrDeviceBindingMismatch => {
                &mut self.authority_or_device_binding_mismatch
            }
            RolloutHealthEvent::AuthenticatedDowngradeAcceptance => {
                &mut self.authenticated_downgrade_acceptance
            }
            RolloutHealthEvent::UnrecoverableStateCorruption => {
                &mut self.unrecoverable_state_corruption
            }
        };
        *counter = counter
            .checked_add(1)
            .ok_or(RolloutError::HealthCounterOverflow)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct HealthRecordContent {
    version: u8,
    policy_id: [u8; 32],
    release_artifact_id: ReleaseArtifactId,
    phase: RolloutPhase,
    cohort_upper_bound_basis_points: u16,
    counters: RolloutHealthCounters,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct StoredHealthRecord {
    content: HealthRecordContent,
    mac: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RolloutHealthSnapshot {
    pub policy_id: ReleasePolicyId,
    pub release_artifact_id: ReleaseArtifactId,
    pub phase: RolloutPhase,
    pub cohort_upper_bound_basis_points: u16,
    pub counters: RolloutHealthCounters,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RolloutActivationTarget {
    LegacyOnly,
    AdvertiseCanonicalV2,
    DualReaderPreferLegacy,
    DualReaderPreferCanonical,
}

impl RolloutActivationTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LegacyOnly => "legacy-only",
            Self::AdvertiseCanonicalV2 => "advertise-cbor-v2",
            Self::DualReaderPreferLegacy => "dual-reader-prefer-legacy",
            Self::DualReaderPreferCanonical => "dual-reader-prefer-canonical",
        }
    }

    const fn ordinal(self) -> u8 {
        match self {
            Self::LegacyOnly => 0,
            Self::AdvertiseCanonicalV2 => 1,
            Self::DualReaderPreferLegacy => 2,
            Self::DualReaderPreferCanonical => 3,
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::LegacyOnly => Self::AdvertiseCanonicalV2,
            Self::AdvertiseCanonicalV2 => Self::DualReaderPreferLegacy,
            Self::DualReaderPreferLegacy => Self::DualReaderPreferCanonical,
            Self::DualReaderPreferCanonical => Self::DualReaderPreferCanonical,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RolloutActivationDisposition {
    Normal,
    EmergencyContainment,
}

impl RolloutActivationDisposition {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::EmergencyContainment => "emergency-containment",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RolloutControllerCurrentState {
    pub target: RolloutActivationTarget,
    pub disposition: RolloutActivationDisposition,
    pub has_authenticated_canonical_evidence: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RolloutControllerHoldReason {
    MissingSignedPolicy,
    MissingHealthSnapshot,
    HealthPolicyMismatch,
    LegacyOnlyPolicy,
    InstallationOutsideCohort,
    AlreadyAtOrBeyondPolicyTarget,
    ContainmentWithoutCanonicalEvidence,
}

impl RolloutControllerHoldReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingSignedPolicy => "missing-signed-policy",
            Self::MissingHealthSnapshot => "missing-health-snapshot",
            Self::HealthPolicyMismatch => "health-policy-mismatch",
            Self::LegacyOnlyPolicy => "legacy-only-policy",
            Self::InstallationOutsideCohort => "installation-outside-cohort",
            Self::AlreadyAtOrBeyondPolicyTarget => "already-at-or-beyond-policy-target",
            Self::ContainmentWithoutCanonicalEvidence => "containment-without-canonical-evidence",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RolloutControllerDecision {
    Hold {
        reason: RolloutControllerHoldReason,
        current: RolloutControllerCurrentState,
    },
    ProposeDeviceActivation {
        policy_id: ReleasePolicyId,
        policy_epoch: u64,
        target: RolloutActivationTarget,
        disposition: RolloutActivationDisposition,
        requires_explicit_device_approval: bool,
        release_policy_can_sign_activation: bool,
    },
}

impl RolloutControllerDecision {
    pub const fn runtime_activation_authorized(self) -> bool {
        false
    }
}

/// Evaluate a dormant rollout proposal without writing activation authority.
///
/// The output is advisory. Even a selected healthy cohort can advance by only
/// one activation state and must pass through the separate Device-signed
/// plan/apply ceremony. Health failure can request containment only for an
/// already activated state with authenticated canonical evidence.
pub fn evaluate_rollout_controller(
    evaluation: Option<CohortEvaluation>,
    health: Option<RolloutHealthSnapshot>,
    current: RolloutControllerCurrentState,
) -> Result<RolloutControllerDecision, RolloutError> {
    let Some(evaluation) = evaluation else {
        return Ok(controller_hold(
            RolloutControllerHoldReason::MissingSignedPolicy,
            current,
        ));
    };
    let Some(health) = health else {
        return Ok(controller_hold(
            RolloutControllerHoldReason::MissingHealthSnapshot,
            current,
        ));
    };
    if health.policy_id != evaluation.policy_id
        || health.phase != evaluation.phase
        || health.cohort_upper_bound_basis_points != evaluation.cohort_upper_bound_basis_points
    {
        return Ok(controller_hold(
            RolloutControllerHoldReason::HealthPolicyMismatch,
            current,
        ));
    }

    let containment_required = evaluation.disposition == RolloutDisposition::Containment
        || health.counters.zero_tolerance_events()? > 0;
    if containment_required {
        if current.target == RolloutActivationTarget::LegacyOnly
            || !current.has_authenticated_canonical_evidence
        {
            return Ok(controller_hold(
                RolloutControllerHoldReason::ContainmentWithoutCanonicalEvidence,
                current,
            ));
        }
        if current.disposition == RolloutActivationDisposition::EmergencyContainment {
            return Ok(controller_hold(
                RolloutControllerHoldReason::AlreadyAtOrBeyondPolicyTarget,
                current,
            ));
        }
        return Ok(controller_proposal(
            evaluation,
            current.target,
            RolloutActivationDisposition::EmergencyContainment,
        ));
    }

    if evaluation.phase == RolloutPhase::LegacyOnlyHold {
        return Ok(controller_hold(
            RolloutControllerHoldReason::LegacyOnlyPolicy,
            current,
        ));
    }
    if !evaluation.selected {
        return Ok(controller_hold(
            RolloutControllerHoldReason::InstallationOutsideCohort,
            current,
        ));
    }
    let policy_target = rollout_phase_target(evaluation.phase);
    if current.target.ordinal() >= policy_target.ordinal() {
        return Ok(controller_hold(
            RolloutControllerHoldReason::AlreadyAtOrBeyondPolicyTarget,
            current,
        ));
    }
    Ok(controller_proposal(
        evaluation,
        current.target.next(),
        RolloutActivationDisposition::Normal,
    ))
}

fn rollout_phase_target(phase: RolloutPhase) -> RolloutActivationTarget {
    match phase {
        RolloutPhase::LegacyOnlyHold => RolloutActivationTarget::LegacyOnly,
        RolloutPhase::AdvertiseCanary => RolloutActivationTarget::AdvertiseCanonicalV2,
        RolloutPhase::AcceptCanary => RolloutActivationTarget::DualReaderPreferLegacy,
        RolloutPhase::InitiateCanary | RolloutPhase::ExpandedDualReader => {
            RolloutActivationTarget::DualReaderPreferCanonical
        }
    }
}

fn controller_hold(
    reason: RolloutControllerHoldReason,
    current: RolloutControllerCurrentState,
) -> RolloutControllerDecision {
    RolloutControllerDecision::Hold { reason, current }
}

fn controller_proposal(
    evaluation: CohortEvaluation,
    target: RolloutActivationTarget,
    disposition: RolloutActivationDisposition,
) -> RolloutControllerDecision {
    RolloutControllerDecision::ProposeDeviceActivation {
        policy_id: evaluation.policy_id,
        policy_epoch: evaluation.policy_epoch,
        target,
        disposition,
        requires_explicit_device_approval: true,
        release_policy_can_sign_activation: false,
    }
}

/// A deliberate call-site marker. Constructing it does not grant network
/// upload permission; it only prevents an ordinary snapshot read from
/// accidentally turning into an export operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplicitHealthExport {
    UserOrTestOperatorRequested,
}

#[derive(Serialize)]
struct HealthExportDocument<'a> {
    schema_version: u8,
    policy_id: &'a str,
    release_artifact_id: &'a str,
    rollout_phase: &'static str,
    cohort_upper_bound_basis_points: u16,
    counters: RolloutHealthCounters,
}

pub struct RolloutLocalStore {
    database: Database,
    authority: ReleasePolicyAuthority,
    database_path: PathBuf,
}

impl RolloutLocalStore {
    pub fn open(
        installation_local_directory: impl AsRef<Path>,
        authority: ReleasePolicyAuthority,
    ) -> Result<Self, RolloutError> {
        let requested = installation_local_directory.as_ref();
        std::fs::create_dir_all(requested).map_err(|source| RolloutError::Io {
            action: "create rollout local directory",
            source,
        })?;
        let metadata = std::fs::symlink_metadata(requested).map_err(|source| RolloutError::Io {
            action: "inspect rollout local directory",
            source,
        })?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(RolloutError::UnsafeLocalDirectory);
        }
        let canonical = std::fs::canonicalize(requested).map_err(|source| RolloutError::Io {
            action: "canonicalize rollout local directory",
            source,
        })?;
        let database_path = canonical.join(DATABASE_FILE);
        if let Ok(database_metadata) = std::fs::symlink_metadata(&database_path)
            && database_metadata.file_type().is_symlink()
        {
            return Err(RolloutError::UnsafeDatabaseFile);
        }
        let database = Database::create(&database_path).map_err(database_error)?;
        let store = Self {
            database,
            authority,
            database_path,
        };
        store.initialize()?;
        Ok(store)
    }

    pub fn database_file_name(&self) -> &'static str {
        DATABASE_FILE
    }

    pub fn has_installed_policy(&self) -> Result<bool, RolloutError> {
        let read = self.database.begin_read().map_err(database_error)?;
        let table = read.open_table(META_TABLE).map_err(database_error)?;
        Ok(table
            .get(POLICY_HEAD_KEY)
            .map_err(database_error)?
            .is_some())
    }

    pub fn install_policy(&self, encoded_policy: &[u8]) -> Result<CohortEvaluation, RolloutError> {
        let policy = SignedReleasePolicy::decode_and_verify(encoded_policy, self.authority)?;
        let policy_id = policy.policy_id()?;

        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        let seed = {
            let mut meta = write.open_table(META_TABLE).map_err(database_error)?;
            let seed = read_seed_from_table(&meta)?;
            let previous_bytes = meta
                .get(POLICY_HEAD_KEY)
                .map_err(database_error)?
                .map(|value| value.value().to_vec());
            match previous_bytes {
                Some(previous_bytes) => {
                    let previous =
                        SignedReleasePolicy::decode_and_verify(&previous_bytes, self.authority)?;
                    if previous.policy_id()? != policy_id {
                        policy.verify_successor_of(&previous)?;
                        meta.insert(POLICY_HEAD_KEY, encoded_policy)
                            .map_err(database_error)?;
                    } else if previous_bytes != encoded_policy {
                        return Err(RolloutError::PolicyIdCollision);
                    }
                }
                None => {
                    validate_policy_transition(&policy.content, None)?;
                    meta.insert(POLICY_HEAD_KEY, encoded_policy)
                        .map_err(database_error)?;
                }
            }
            seed
        };
        self.ensure_health_record_in_write(&write, &policy, policy_id, &seed)?;
        write.commit().map_err(database_error)?;
        evaluate_cohort(seed, &policy, policy_id)
    }

    pub fn current_evaluation(&self) -> Result<Option<CohortEvaluation>, RolloutError> {
        let read = self.database.begin_read().map_err(database_error)?;
        let meta = read.open_table(META_TABLE).map_err(database_error)?;
        let seed = read_seed_from_table(&meta)?;
        let Some(encoded) = meta
            .get(POLICY_HEAD_KEY)
            .map_err(database_error)?
            .map(|value| value.value().to_vec())
        else {
            return Ok(None);
        };
        let policy = SignedReleasePolicy::decode_and_verify(&encoded, self.authority)?;
        let policy_id = policy.policy_id()?;
        evaluate_cohort(seed, &policy, policy_id).map(Some)
    }

    pub fn record_health(
        &self,
        policy_id: ReleasePolicyId,
        event: RolloutHealthEvent,
    ) -> Result<RolloutHealthSnapshot, RolloutError> {
        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        let seed = {
            let meta = write.open_table(META_TABLE).map_err(database_error)?;
            let seed = read_seed_from_table(&meta)?;
            let current = read_current_policy_from_table(&meta, self.authority)?
                .ok_or(RolloutError::NoInstalledPolicy)?;
            if current.policy_id()? != policy_id {
                return Err(RolloutError::StaleHealthPolicy);
            }
            seed
        };
        let snapshot = {
            let mut health = write.open_table(HEALTH_TABLE).map_err(database_error)?;
            let bytes = health
                .get(policy_id.0.as_slice())
                .map_err(database_error)?
                .ok_or(RolloutError::MissingHealthRecord)?
                .value()
                .to_vec();
            let mut record = decode_health_record(&bytes, &seed)?;
            record.content.counters.increment(event)?;
            let encoded = encode_health_record(record.content.clone(), &seed)?;
            health
                .insert(policy_id.0.as_slice(), encoded.as_slice())
                .map_err(database_error)?;
            snapshot_from_content(&record.content)?
        };
        write.commit().map_err(database_error)?;
        Ok(snapshot)
    }

    pub fn current_health_snapshot(&self) -> Result<Option<RolloutHealthSnapshot>, RolloutError> {
        let read = self.database.begin_read().map_err(database_error)?;
        let meta = read.open_table(META_TABLE).map_err(database_error)?;
        let seed = read_seed_from_table(&meta)?;
        let Some(policy) = read_current_policy_from_table(&meta, self.authority)? else {
            return Ok(None);
        };
        let policy_id = policy.policy_id()?;
        let health = read.open_table(HEALTH_TABLE).map_err(database_error)?;
        let bytes = health
            .get(policy_id.0.as_slice())
            .map_err(database_error)?
            .ok_or(RolloutError::MissingHealthRecord)?
            .value()
            .to_vec();
        let record = decode_health_record(&bytes, &seed)?;
        Ok(Some(snapshot_from_content(&record.content)?))
    }

    pub fn export_current_health_explicitly(
        &self,
        authorization: ExplicitHealthExport,
    ) -> Result<Vec<u8>, RolloutError> {
        if authorization != ExplicitHealthExport::UserOrTestOperatorRequested {
            return Err(RolloutError::ExplicitExportRequired);
        }
        let snapshot = self
            .current_health_snapshot()?
            .ok_or(RolloutError::NoInstalledPolicy)?;
        let policy_id = hex_lower(&snapshot.policy_id.0);
        let release_artifact_id = hex_lower(&snapshot.release_artifact_id.0);
        let document = HealthExportDocument {
            schema_version: 1,
            policy_id: &policy_id,
            release_artifact_id: &release_artifact_id,
            rollout_phase: snapshot.phase.as_str(),
            cohort_upper_bound_basis_points: snapshot.cohort_upper_bound_basis_points,
            counters: snapshot.counters,
        };
        serde_json::to_vec_pretty(&document).map_err(RolloutError::HealthExportJson)
    }

    fn initialize(&self) -> Result<(), RolloutError> {
        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut meta = write.open_table(META_TABLE).map_err(database_error)?;
            let current_schema = meta
                .get(DATABASE_SCHEMA_KEY)
                .map_err(database_error)?
                .map(|value| value.value().to_vec());
            match current_schema {
                Some(version) if version.as_slice() == [DATABASE_SCHEMA_VERSION] => {}
                Some(_) => return Err(RolloutError::DatabaseSchema),
                None => {
                    meta.insert(DATABASE_SCHEMA_KEY, [DATABASE_SCHEMA_VERSION].as_slice())
                        .map_err(database_error)?;
                }
            }
            let current_authority = meta
                .get(RELEASE_AUTHORITY_KEY)
                .map_err(database_error)?
                .map(|value| value.value().to_vec());
            match current_authority {
                Some(bytes) if bytes.as_slice() == self.authority.0 => {}
                Some(_) => return Err(RolloutError::ReleaseAuthorityMismatch),
                None => {
                    meta.insert(RELEASE_AUTHORITY_KEY, self.authority.0.as_slice())
                        .map_err(database_error)?;
                }
            }
            let current_seed = meta
                .get(INSTALLATION_SEED_KEY)
                .map_err(database_error)?
                .map(|value| value.value().to_vec());
            match current_seed {
                Some(seed) if seed.len() == 32 => {}
                Some(_) => return Err(RolloutError::InstallationSeedLength),
                None => {
                    let mut seed = [0_u8; 32];
                    getrandom::fill(&mut seed).map_err(RolloutError::SecureRandom)?;
                    meta.insert(INSTALLATION_SEED_KEY, seed.as_slice())
                        .map_err(database_error)?;
                }
            }
        }
        write.open_table(HEALTH_TABLE).map_err(database_error)?;
        write.commit().map_err(database_error)
    }

    fn ensure_health_record_in_write(
        &self,
        write: &redb::WriteTransaction,
        policy: &SignedReleasePolicy,
        policy_id: ReleasePolicyId,
        seed: &[u8; 32],
    ) -> Result<(), RolloutError> {
        let mut health = write.open_table(HEALTH_TABLE).map_err(database_error)?;
        if let Some(existing) = health.get(policy_id.0.as_slice()).map_err(database_error)? {
            let record = decode_health_record(existing.value(), seed)?;
            validate_health_policy_binding(&record.content, policy, policy_id)?;
            return Ok(());
        }
        if health.len().map_err(database_error)? >= MAX_DURABLE_HEALTH_AGGREGATES {
            return Err(RolloutError::HealthAggregateCapacity);
        }
        let content = HealthRecordContent {
            version: HEALTH_RECORD_VERSION,
            policy_id: policy_id.0,
            release_artifact_id: policy.release_artifact_id(),
            phase: policy.phase(),
            cohort_upper_bound_basis_points: policy.cohort_upper_bound_basis_points(),
            counters: RolloutHealthCounters::default(),
        };
        let encoded = encode_health_record(content, seed)?;
        health
            .insert(policy_id.0.as_slice(), encoded.as_slice())
            .map_err(database_error)?;
        Ok(())
    }

    #[cfg(test)]
    fn installation_seed_for_test(&self) -> Result<[u8; 32], RolloutError> {
        let read = self.database.begin_read().map_err(database_error)?;
        let meta = read.open_table(META_TABLE).map_err(database_error)?;
        read_seed_from_table(&meta)
    }

    #[cfg(test)]
    fn replace_current_health_for_test(&self, bytes: &[u8]) -> Result<(), RolloutError> {
        let evaluation = self
            .current_evaluation()?
            .ok_or(RolloutError::NoInstalledPolicy)?;
        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut health = write.open_table(HEALTH_TABLE).map_err(database_error)?;
            health
                .insert(evaluation.policy_id.0.as_slice(), bytes)
                .map_err(database_error)?;
        }
        write.commit().map_err(database_error)
    }

    #[cfg(test)]
    fn current_health_bytes_for_test(&self) -> Result<Vec<u8>, RolloutError> {
        let evaluation = self
            .current_evaluation()?
            .ok_or(RolloutError::NoInstalledPolicy)?;
        let read = self.database.begin_read().map_err(database_error)?;
        let health = read.open_table(HEALTH_TABLE).map_err(database_error)?;
        health
            .get(evaluation.policy_id.0.as_slice())
            .map_err(database_error)?
            .ok_or(RolloutError::MissingHealthRecord)
            .map(|value| value.value().to_vec())
    }
}

impl fmt::Debug for RolloutLocalStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RolloutLocalStore")
            .field("database_file", &self.database_path.file_name())
            .field("release_authority", &self.authority)
            .finish_non_exhaustive()
    }
}

fn validate_policy_content(content: &ReleasePolicyContent) -> Result<(), RolloutError> {
    if content.version != RELEASE_POLICY_VERSION {
        return Err(RolloutError::PolicyVersion(content.version));
    }
    if content.release_artifact_id.0 == [0_u8; 32] {
        return Err(RolloutError::ZeroReleaseArtifactId);
    }
    match (content.phase, content.disposition) {
        (RolloutPhase::LegacyOnlyHold, RolloutDisposition::Normal)
            if content.cohort_upper_bound_basis_points == 0 => {}
        (RolloutPhase::LegacyOnlyHold, _) => return Err(RolloutError::InvalidPolicyCombination),
        (_, RolloutDisposition::Containment) if content.cohort_upper_bound_basis_points == 0 => {}
        (_, RolloutDisposition::Normal)
            if (1..=content.phase.maximum_cohort_basis_points())
                .contains(&content.cohort_upper_bound_basis_points) => {}
        _ => return Err(RolloutError::InvalidPolicyCombination),
    }
    Ok(())
}

fn validate_policy_transition(
    content: &ReleasePolicyContent,
    previous: Option<&SignedReleasePolicy>,
) -> Result<(), RolloutError> {
    validate_policy_content(content)?;
    match previous {
        None => {
            if content.policy_epoch != 1
                || content.previous_policy_id.is_some()
                || content.phase != RolloutPhase::LegacyOnlyHold
                || content.disposition != RolloutDisposition::Normal
                || content.cohort_upper_bound_basis_points != 0
            {
                return Err(RolloutError::InvalidGenesisPolicy);
            }
        }
        Some(previous) => {
            let expected_epoch = previous
                .policy_epoch()
                .checked_add(1)
                .ok_or(RolloutError::PolicyEpochOverflow)?;
            if content.policy_epoch != expected_epoch {
                return Err(RolloutError::PolicyEpochGap {
                    expected: expected_epoch,
                    found: content.policy_epoch,
                });
            }
            if content.previous_policy_id != Some(previous.policy_id()?.0) {
                return Err(RolloutError::PreviousPolicyMismatch);
            }
            let old = previous.phase().ordinal();
            let new = content.phase.ordinal();
            if new < old || new > old.saturating_add(1) {
                return Err(RolloutError::PolicyPhaseSkipOrRewind);
            }
            if previous.phase() == RolloutPhase::LegacyOnlyHold
                && content.phase == RolloutPhase::LegacyOnlyHold
                && content.disposition == RolloutDisposition::Containment
            {
                return Err(RolloutError::InvalidPolicyCombination);
            }
        }
    }
    Ok(())
}

fn policy_signing_bytes(content: &ReleasePolicyContent) -> Result<Vec<u8>, RolloutError> {
    let encoded = postcard::to_allocvec(content).map_err(RolloutError::PolicyCodec)?;
    let mut bytes = Vec::with_capacity(RELEASE_POLICY_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(RELEASE_POLICY_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn evaluate_cohort(
    seed: [u8; 32],
    policy: &SignedReleasePolicy,
    policy_id: ReleasePolicyId,
) -> Result<CohortEvaluation, RolloutError> {
    let cohort_bucket = cohort_bucket(&seed, &policy_id.0);
    let selected = policy.disposition() == RolloutDisposition::Normal
        && policy.phase() != RolloutPhase::LegacyOnlyHold
        && cohort_bucket < policy.cohort_upper_bound_basis_points();
    Ok(CohortEvaluation {
        policy_id,
        policy_epoch: policy.policy_epoch(),
        phase: policy.phase(),
        disposition: policy.disposition(),
        cohort_bucket,
        cohort_upper_bound_basis_points: policy.cohort_upper_bound_basis_points(),
        selected,
        runtime_activation_authorized: false,
    })
}

fn cohort_bucket(seed: &[u8; 32], policy_id: &[u8; 32]) -> u16 {
    let mut hasher = Sha256::new();
    hasher.update(seed);
    hasher.update(policy_id);
    let digest: [u8; 32] = hasher.finalize().into();
    let mut prefix_bytes = [0_u8; 8];
    prefix_bytes.copy_from_slice(&digest[..8]);
    let prefix = u64::from_be_bytes(prefix_bytes);
    (prefix % u64::from(COHORT_BUCKETS)) as u16
}

fn read_seed_from_table(
    table: &impl ReadableTable<&'static str, &'static [u8]>,
) -> Result<[u8; 32], RolloutError> {
    let bytes = table
        .get(INSTALLATION_SEED_KEY)
        .map_err(database_error)?
        .ok_or(RolloutError::MissingInstallationSeed)?
        .value()
        .to_vec();
    bytes
        .try_into()
        .map_err(|_| RolloutError::InstallationSeedLength)
}

fn read_current_policy_from_table(
    table: &impl ReadableTable<&'static str, &'static [u8]>,
    authority: ReleasePolicyAuthority,
) -> Result<Option<SignedReleasePolicy>, RolloutError> {
    let Some(bytes) = table
        .get(POLICY_HEAD_KEY)
        .map_err(database_error)?
        .map(|value| value.value().to_vec())
    else {
        return Ok(None);
    };
    SignedReleasePolicy::decode_and_verify(&bytes, authority).map(Some)
}

fn encode_health_record(
    content: HealthRecordContent,
    seed: &[u8; 32],
) -> Result<Vec<u8>, RolloutError> {
    validate_health_content(&content)?;
    let mac = health_record_mac(seed, &content)?;
    let record = StoredHealthRecord { content, mac };
    let encoded = postcard::to_allocvec(&record).map_err(RolloutError::HealthCodec)?;
    if encoded.is_empty() || encoded.len() > MAX_DURABLE_HEALTH_RECORD_BYTES {
        return Err(RolloutError::HealthRecordLength(encoded.len()));
    }
    Ok(encoded)
}

fn decode_health_record(
    encoded: &[u8],
    seed: &[u8; 32],
) -> Result<StoredHealthRecord, RolloutError> {
    if encoded.is_empty() || encoded.len() > MAX_DURABLE_HEALTH_RECORD_BYTES {
        return Err(RolloutError::HealthRecordLength(encoded.len()));
    }
    let record: StoredHealthRecord =
        postcard::from_bytes(encoded).map_err(RolloutError::HealthCodec)?;
    validate_health_content(&record.content)?;
    if record.mac != health_record_mac(seed, &record.content)? {
        return Err(RolloutError::HealthRecordAuthentication);
    }
    if postcard::to_allocvec(&record).map_err(RolloutError::HealthCodec)? != encoded {
        return Err(RolloutError::NonCanonicalHealthRecord);
    }
    Ok(record)
}

fn health_record_mac(
    seed: &[u8; 32],
    content: &HealthRecordContent,
) -> Result<[u8; 32], RolloutError> {
    let encoded = postcard::to_allocvec(content).map_err(RolloutError::HealthCodec)?;
    let mut bytes = Vec::with_capacity(HEALTH_RECORD_MAC_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(HEALTH_RECORD_MAC_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(*blake3::keyed_hash(seed, &bytes).as_bytes())
}

fn validate_health_content(content: &HealthRecordContent) -> Result<(), RolloutError> {
    if content.version != HEALTH_RECORD_VERSION
        || content.policy_id == [0_u8; 32]
        || content.release_artifact_id.0 == [0_u8; 32]
        || content.cohort_upper_bound_basis_points > content.phase.maximum_cohort_basis_points()
    {
        return Err(RolloutError::InvalidHealthRecord);
    }
    content.counters.zero_tolerance_events()?;
    Ok(())
}

fn validate_health_policy_binding(
    content: &HealthRecordContent,
    policy: &SignedReleasePolicy,
    policy_id: ReleasePolicyId,
) -> Result<(), RolloutError> {
    if content.policy_id != policy_id.0
        || content.release_artifact_id != policy.release_artifact_id()
        || content.phase != policy.phase()
        || content.cohort_upper_bound_basis_points != policy.cohort_upper_bound_basis_points()
    {
        return Err(RolloutError::HealthPolicyBinding);
    }
    Ok(())
}

fn snapshot_from_content(
    content: &HealthRecordContent,
) -> Result<RolloutHealthSnapshot, RolloutError> {
    validate_health_content(content)?;
    Ok(RolloutHealthSnapshot {
        policy_id: ReleasePolicyId(content.policy_id),
        release_artifact_id: content.release_artifact_id,
        phase: content.phase,
        cohort_upper_bound_basis_points: content.cohort_upper_bound_basis_points,
        counters: content.counters,
    })
}

fn database_error(error: impl fmt::Display) -> RolloutError {
    RolloutError::Database(error.to_string())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(char::from(HEX[usize::from(byte >> 4)]));
        result.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    result
}

#[derive(Debug, Error)]
pub enum RolloutError {
    #[error("rollout local I/O failed while trying to {action}: {source}")]
    Io {
        action: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("rollout database operation failed: {0}")]
    Database(String),
    #[error("rollout local directory must be a real directory, not a symlink")]
    UnsafeLocalDirectory,
    #[error("rollout database file must not be a symlink")]
    UnsafeDatabaseFile,
    #[error("rollout database schema is unsupported")]
    DatabaseSchema,
    #[error("release authority does not match the pinned local authority")]
    ReleaseAuthorityMismatch,
    #[error("invalid release-policy authority key: {0}")]
    InvalidReleaseAuthority(ed25519_dalek::SignatureError),
    #[error("secure random generation failed: {0}")]
    SecureRandom(getrandom::Error),
    #[error("installation cohort seed is missing")]
    MissingInstallationSeed,
    #[error("installation cohort seed has an invalid length")]
    InstallationSeedLength,
    #[error("release artifact identifier must not be zero")]
    ZeroReleaseArtifactId,
    #[error("signed release policy has invalid length {0}")]
    PolicyLength(usize),
    #[error("signed release policy uses unsupported version {0}")]
    PolicyVersion(u8),
    #[error("signed release policy encoding is invalid: {0}")]
    PolicyCodec(postcard::Error),
    #[error("signed release policy is not canonically encoded")]
    NonCanonicalPolicy,
    #[error("signed release policy signature is malformed: {0}")]
    InvalidPolicySignature(ed25519_dalek::SignatureError),
    #[error("signed release policy is not authorized: {0}")]
    PolicyAuthorization(ed25519_dalek::SignatureError),
    #[error("release policy epoch overflowed")]
    PolicyEpochOverflow,
    #[error("release policy epoch must be {expected}, found {found}")]
    PolicyEpochGap { expected: u64, found: u64 },
    #[error("release policy predecessor does not match the durable head")]
    PreviousPolicyMismatch,
    #[error("release policy phase cannot skip or rewind")]
    PolicyPhaseSkipOrRewind,
    #[error("release policy field combination is not allowed")]
    InvalidPolicyCombination,
    #[error("first release policy must be an epoch-one legacy-only hold")]
    InvalidGenesisPolicy,
    #[error("distinct release policies produced the same policy identifier")]
    PolicyIdCollision,
    #[error("no signed release policy is installed")]
    NoInstalledPolicy,
    #[error("health event targets a policy other than the durable head")]
    StaleHealthPolicy,
    #[error("durable health aggregate is missing for the current policy")]
    MissingHealthRecord,
    #[error("durable health aggregate capacity is exhausted")]
    HealthAggregateCapacity,
    #[error("durable health record has invalid length {0}")]
    HealthRecordLength(usize),
    #[error("durable health record encoding is invalid: {0}")]
    HealthCodec(postcard::Error),
    #[error("durable health record is not canonically encoded")]
    NonCanonicalHealthRecord,
    #[error("durable health record authentication failed")]
    HealthRecordAuthentication,
    #[error("durable health record fields are invalid")]
    InvalidHealthRecord,
    #[error("durable health record is not bound to its exact release policy")]
    HealthPolicyBinding,
    #[error("durable health counter overflowed")]
    HealthCounterOverflow,
    #[error("health export requires an explicit user or test-operator request")]
    ExplicitExportRequired,
    #[error("health export JSON encoding failed: {0}")]
    HealthExportJson(serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signing_key(byte: u8) -> SigningKey {
        SigningKey::from_bytes(&[byte; 32])
    }

    fn authority(key: &SigningKey) -> Result<ReleasePolicyAuthority, RolloutError> {
        ReleasePolicyAuthority::from_bytes(key.verifying_key().to_bytes())
    }

    fn artifact(byte: u8) -> Result<ReleaseArtifactId, RolloutError> {
        ReleaseArtifactId::from_bytes([byte; 32])
    }

    fn policy_chain(
        key: &SigningKey,
    ) -> Result<(SignedReleasePolicy, SignedReleasePolicy), RolloutError> {
        let hold = SignedReleasePolicy::issue_genesis_hold(key, artifact(1)?)?;
        let advertise = SignedReleasePolicy::issue_successor(
            key,
            &hold,
            artifact(2)?,
            RolloutPhase::AdvertiseCanary,
            RolloutDisposition::Normal,
            100,
        )?;
        Ok((hold, advertise))
    }

    #[test]
    fn cohort_assignment_has_stable_exact_sha256_vector() {
        let mut seed = [0_u8; 32];
        let mut policy_id = [0_u8; 32];
        for (index, byte) in seed.iter_mut().enumerate() {
            *byte = index as u8;
        }
        for (index, byte) in policy_id.iter_mut().enumerate() {
            *byte = (index + 32) as u8;
        }
        assert_eq!(cohort_bucket(&seed, &policy_id), 4_706);
    }

    #[test]
    fn signed_policy_is_canonical_and_authority_bound() -> Result<(), RolloutError> {
        let key = signing_key(7);
        let foreign = signing_key(8);
        let hold = SignedReleasePolicy::issue_genesis_hold(&key, artifact(1)?)?;
        let encoded = hold.encode()?;
        assert_eq!(
            SignedReleasePolicy::decode_and_verify(&encoded, authority(&key)?)?,
            hold
        );
        assert!(matches!(
            SignedReleasePolicy::decode_and_verify(&encoded, authority(&foreign)?),
            Err(RolloutError::PolicyAuthorization(_))
        ));
        let mut tampered = encoded;
        let last = tampered.len().saturating_sub(1);
        tampered[last] ^= 1;
        assert!(SignedReleasePolicy::decode_and_verify(&tampered, authority(&key)?).is_err());
        Ok(())
    }

    #[test]
    fn policy_chain_rejects_skip_rewind_and_oversized_cohort() -> Result<(), RolloutError> {
        let key = signing_key(9);
        let (hold, advertise) = policy_chain(&key)?;
        assert!(matches!(
            SignedReleasePolicy::issue_successor(
                &key,
                &hold,
                artifact(3)?,
                RolloutPhase::AcceptCanary,
                RolloutDisposition::Normal,
                100,
            ),
            Err(RolloutError::PolicyPhaseSkipOrRewind)
        ));
        assert!(matches!(
            SignedReleasePolicy::issue_successor(
                &key,
                &advertise,
                artifact(4)?,
                RolloutPhase::LegacyOnlyHold,
                RolloutDisposition::Normal,
                0,
            ),
            Err(RolloutError::PolicyPhaseSkipOrRewind)
        ));
        assert!(matches!(
            SignedReleasePolicy::issue_successor(
                &key,
                &advertise,
                artifact(5)?,
                RolloutPhase::AdvertiseCanary,
                RolloutDisposition::Normal,
                101,
            ),
            Err(RolloutError::InvalidPolicyCombination)
        ));
        Ok(())
    }

    #[test]
    fn containment_stops_selection_without_rewinding_phase() -> Result<(), RolloutError> {
        let key = signing_key(10);
        let (_, advertise) = policy_chain(&key)?;
        let containment = SignedReleasePolicy::issue_successor(
            &key,
            &advertise,
            artifact(3)?,
            RolloutPhase::AdvertiseCanary,
            RolloutDisposition::Containment,
            0,
        )?;
        assert_eq!(containment.phase(), RolloutPhase::AdvertiseCanary);
        assert_eq!(containment.disposition(), RolloutDisposition::Containment);
        Ok(())
    }

    #[test]
    fn store_defaults_to_no_policy_and_preserves_private_seed() -> Result<(), RolloutError> {
        let directory = tempfile::tempdir().map_err(|source| RolloutError::Io {
            action: "create test directory",
            source,
        })?;
        let key = signing_key(11);
        let store = RolloutLocalStore::open(directory.path(), authority(&key)?)?;
        assert!(!store.has_installed_policy()?);
        assert_eq!(store.current_evaluation()?, None);
        let seed = store.installation_seed_for_test()?;
        drop(store);
        let reopened = RolloutLocalStore::open(directory.path(), authority(&key)?)?;
        assert_eq!(reopened.installation_seed_for_test()?, seed);
        Ok(())
    }

    #[test]
    fn policy_install_is_monotonic_idempotent_and_restart_stable() -> Result<(), RolloutError> {
        let directory = tempfile::tempdir().map_err(|source| RolloutError::Io {
            action: "create test directory",
            source,
        })?;
        let key = signing_key(12);
        let (hold, advertise) = policy_chain(&key)?;
        let store = RolloutLocalStore::open(directory.path(), authority(&key)?)?;
        let hold_evaluation = store.install_policy(&hold.encode()?)?;
        assert_eq!(hold_evaluation.phase, RolloutPhase::LegacyOnlyHold);
        assert!(!hold_evaluation.selected);
        assert!(!hold_evaluation.runtime_activation_authorized);
        let evaluation = store.install_policy(&advertise.encode()?)?;
        assert_eq!(evaluation, store.install_policy(&advertise.encode()?)?);
        assert_eq!(evaluation.selected, evaluation.cohort_bucket < 100);
        assert!(matches!(
            store.install_policy(&hold.encode()?),
            Err(RolloutError::PolicyEpochGap { .. })
                | Err(RolloutError::PreviousPolicyMismatch)
                | Err(RolloutError::PolicyPhaseSkipOrRewind)
        ));
        drop(store);
        let reopened = RolloutLocalStore::open(directory.path(), authority(&key)?)?;
        assert_eq!(reopened.current_evaluation()?, Some(evaluation));
        Ok(())
    }

    #[test]
    fn store_refuses_a_different_pinned_release_authority() -> Result<(), RolloutError> {
        let directory = tempfile::tempdir().map_err(|source| RolloutError::Io {
            action: "create test directory",
            source,
        })?;
        let key = signing_key(13);
        let foreign = signing_key(14);
        drop(RolloutLocalStore::open(directory.path(), authority(&key)?)?);
        assert!(matches!(
            RolloutLocalStore::open(directory.path(), authority(&foreign)?),
            Err(RolloutError::ReleaseAuthorityMismatch)
        ));
        Ok(())
    }

    #[test]
    fn installations_receive_independent_unsynchronized_seeds() -> Result<(), RolloutError> {
        let first = tempfile::tempdir().map_err(|source| RolloutError::Io {
            action: "create first test directory",
            source,
        })?;
        let second = tempfile::tempdir().map_err(|source| RolloutError::Io {
            action: "create second test directory",
            source,
        })?;
        let key = signing_key(14);
        let first = RolloutLocalStore::open(first.path(), authority(&key)?)?;
        let second = RolloutLocalStore::open(second.path(), authority(&key)?)?;
        assert_ne!(
            first.installation_seed_for_test()?,
            second.installation_seed_for_test()?
        );
        Ok(())
    }

    #[test]
    fn health_is_durable_bounded_authenticated_and_explicitly_exported() -> Result<(), RolloutError>
    {
        let directory = tempfile::tempdir().map_err(|source| RolloutError::Io {
            action: "create test directory",
            source,
        })?;
        let key = signing_key(15);
        let (hold, advertise) = policy_chain(&key)?;
        let store = RolloutLocalStore::open(directory.path(), authority(&key)?)?;
        store.install_policy(&hold.encode()?)?;
        let evaluation = store.install_policy(&advertise.encode()?)?;
        store.record_health(
            evaluation.policy_id,
            RolloutHealthEvent::RuntimeStartSucceeded,
        )?;
        store.record_health(
            evaluation.policy_id,
            RolloutHealthEvent::CanonicalSyncCompleted,
        )?;
        store.record_health(
            evaluation.policy_id,
            RolloutHealthEvent::HistoryLossOrDuplication,
        )?;
        let snapshot = store
            .current_health_snapshot()?
            .ok_or(RolloutError::MissingHealthRecord)?;
        assert_eq!(snapshot.counters.runtime_start_succeeded, 1);
        assert_eq!(snapshot.counters.canonical_sync_completed, 1);
        assert_eq!(snapshot.counters.zero_tolerance_events()?, 1);

        let seed_hex = hex_lower(&store.installation_seed_for_test()?);
        let exported = store
            .export_current_health_explicitly(ExplicitHealthExport::UserOrTestOperatorRequested)?;
        let exported =
            String::from_utf8(exported).map_err(|_| RolloutError::InvalidHealthRecord)?;
        assert!(!exported.contains(&seed_hex));
        for forbidden in [
            "cohort_bucket",
            "account_id",
            "device_id",
            "peer_id",
            "conversation_id",
            "ip_address",
            "relay_url",
            "filesystem_path",
            "message",
        ] {
            assert!(
                !exported.contains(forbidden),
                "forbidden export field: {forbidden}"
            );
        }

        drop(store);
        let reopened = RolloutLocalStore::open(directory.path(), authority(&key)?)?;
        assert_eq!(reopened.current_health_snapshot()?, Some(snapshot));
        Ok(())
    }

    #[test]
    fn stale_policy_cannot_receive_health_events() -> Result<(), RolloutError> {
        let directory = tempfile::tempdir().map_err(|source| RolloutError::Io {
            action: "create test directory",
            source,
        })?;
        let key = signing_key(16);
        let (hold, advertise) = policy_chain(&key)?;
        let store = RolloutLocalStore::open(directory.path(), authority(&key)?)?;
        let hold_id = store.install_policy(&hold.encode()?)?.policy_id;
        store.install_policy(&advertise.encode()?)?;
        assert!(matches!(
            store.record_health(hold_id, RolloutHealthEvent::LegacySyncCompleted),
            Err(RolloutError::StaleHealthPolicy)
        ));
        Ok(())
    }

    #[test]
    fn tampered_health_record_fails_closed() -> Result<(), RolloutError> {
        let directory = tempfile::tempdir().map_err(|source| RolloutError::Io {
            action: "create test directory",
            source,
        })?;
        let key = signing_key(17);
        let hold = SignedReleasePolicy::issue_genesis_hold(&key, artifact(1)?)?;
        let store = RolloutLocalStore::open(directory.path(), authority(&key)?)?;
        store.install_policy(&hold.encode()?)?;
        let mut tampered = store.current_health_bytes_for_test()?;
        let last = tampered.len().saturating_sub(1);
        tampered[last] ^= 1;
        store.replace_current_health_for_test(&tampered)?;
        assert!(matches!(
            store.current_health_snapshot(),
            Err(RolloutError::HealthRecordAuthentication)
        ));
        Ok(())
    }

    #[test]
    fn health_history_has_a_hard_policy_count_bound() -> Result<(), RolloutError> {
        let directory = tempfile::tempdir().map_err(|source| RolloutError::Io {
            action: "create test directory",
            source,
        })?;
        let key = signing_key(18);
        let store = RolloutLocalStore::open(directory.path(), authority(&key)?)?;
        let mut policy = SignedReleasePolicy::issue_genesis_hold(&key, artifact(1)?)?;
        store.install_policy(&policy.encode()?)?;
        for index in 2..=MAX_DURABLE_HEALTH_AGGREGATES as u8 {
            policy = SignedReleasePolicy::issue_successor(
                &key,
                &policy,
                artifact(index)?,
                RolloutPhase::LegacyOnlyHold,
                RolloutDisposition::Normal,
                0,
            )?;
            store.install_policy(&policy.encode()?)?;
        }
        let overflow = SignedReleasePolicy::issue_successor(
            &key,
            &policy,
            artifact((MAX_DURABLE_HEALTH_AGGREGATES + 1) as u8)?,
            RolloutPhase::LegacyOnlyHold,
            RolloutDisposition::Normal,
            0,
        )?;
        assert!(matches!(
            store.install_policy(&overflow.encode()?),
            Err(RolloutError::HealthAggregateCapacity)
        ));
        Ok(())
    }

    fn controller_evaluation(
        phase: RolloutPhase,
        disposition: RolloutDisposition,
        selected: bool,
    ) -> CohortEvaluation {
        CohortEvaluation {
            policy_id: ReleasePolicyId([31; 32]),
            policy_epoch: 7,
            phase,
            disposition,
            cohort_bucket: 1,
            cohort_upper_bound_basis_points: if phase == RolloutPhase::LegacyOnlyHold {
                0
            } else {
                phase.maximum_cohort_basis_points()
            },
            selected,
            runtime_activation_authorized: false,
        }
    }

    fn controller_health(
        evaluation: CohortEvaluation,
        counters: RolloutHealthCounters,
    ) -> RolloutHealthSnapshot {
        RolloutHealthSnapshot {
            policy_id: evaluation.policy_id,
            release_artifact_id: ReleaseArtifactId([41; 32]),
            phase: evaluation.phase,
            cohort_upper_bound_basis_points: evaluation.cohort_upper_bound_basis_points,
            counters,
        }
    }

    fn legacy_controller_state() -> RolloutControllerCurrentState {
        RolloutControllerCurrentState {
            target: RolloutActivationTarget::LegacyOnly,
            disposition: RolloutActivationDisposition::Normal,
            has_authenticated_canonical_evidence: false,
        }
    }

    #[test]
    fn controller_fails_closed_when_policy_or_health_is_missing() -> Result<(), RolloutError> {
        let current = legacy_controller_state();
        assert_eq!(
            evaluate_rollout_controller(None, None, current)?,
            RolloutControllerDecision::Hold {
                reason: RolloutControllerHoldReason::MissingSignedPolicy,
                current,
            }
        );
        let evaluation = controller_evaluation(
            RolloutPhase::AdvertiseCanary,
            RolloutDisposition::Normal,
            true,
        );
        assert_eq!(
            evaluate_rollout_controller(Some(evaluation), None, current)?,
            RolloutControllerDecision::Hold {
                reason: RolloutControllerHoldReason::MissingHealthSnapshot,
                current,
            }
        );
        Ok(())
    }

    #[test]
    fn selected_healthy_controller_advances_exactly_one_state() -> Result<(), RolloutError> {
        let evaluation = controller_evaluation(
            RolloutPhase::InitiateCanary,
            RolloutDisposition::Normal,
            true,
        );
        let health = controller_health(evaluation, RolloutHealthCounters::default());
        let decision =
            evaluate_rollout_controller(Some(evaluation), Some(health), legacy_controller_state())?;
        assert_eq!(
            decision,
            RolloutControllerDecision::ProposeDeviceActivation {
                policy_id: evaluation.policy_id,
                policy_epoch: 7,
                target: RolloutActivationTarget::AdvertiseCanonicalV2,
                disposition: RolloutActivationDisposition::Normal,
                requires_explicit_device_approval: true,
                release_policy_can_sign_activation: false,
            }
        );
        assert!(!decision.runtime_activation_authorized());
        Ok(())
    }

    #[test]
    fn installation_outside_cohort_never_regresses_or_enrolls() -> Result<(), RolloutError> {
        let evaluation = controller_evaluation(
            RolloutPhase::ExpandedDualReader,
            RolloutDisposition::Normal,
            false,
        );
        let health = controller_health(evaluation, RolloutHealthCounters::default());
        let current = RolloutControllerCurrentState {
            target: RolloutActivationTarget::DualReaderPreferLegacy,
            disposition: RolloutActivationDisposition::Normal,
            has_authenticated_canonical_evidence: true,
        };
        assert_eq!(
            evaluate_rollout_controller(Some(evaluation), Some(health), current)?,
            RolloutControllerDecision::Hold {
                reason: RolloutControllerHoldReason::InstallationOutsideCohort,
                current,
            }
        );
        Ok(())
    }

    #[test]
    fn zero_tolerance_health_requires_evidence_before_containment() -> Result<(), RolloutError> {
        let evaluation = controller_evaluation(
            RolloutPhase::InitiateCanary,
            RolloutDisposition::Normal,
            true,
        );
        let counters = RolloutHealthCounters {
            authenticated_downgrade_acceptance: 1,
            ..RolloutHealthCounters::default()
        };
        let health = controller_health(evaluation, counters);
        let no_evidence = RolloutControllerCurrentState {
            target: RolloutActivationTarget::DualReaderPreferLegacy,
            disposition: RolloutActivationDisposition::Normal,
            has_authenticated_canonical_evidence: false,
        };
        assert_eq!(
            evaluate_rollout_controller(Some(evaluation), Some(health), no_evidence)?,
            RolloutControllerDecision::Hold {
                reason: RolloutControllerHoldReason::ContainmentWithoutCanonicalEvidence,
                current: no_evidence,
            }
        );

        let with_evidence = RolloutControllerCurrentState {
            has_authenticated_canonical_evidence: true,
            ..no_evidence
        };
        assert_eq!(
            evaluate_rollout_controller(Some(evaluation), Some(health), with_evidence)?,
            RolloutControllerDecision::ProposeDeviceActivation {
                policy_id: evaluation.policy_id,
                policy_epoch: 7,
                target: RolloutActivationTarget::DualReaderPreferLegacy,
                disposition: RolloutActivationDisposition::EmergencyContainment,
                requires_explicit_device_approval: true,
                release_policy_can_sign_activation: false,
            }
        );
        Ok(())
    }

    #[test]
    fn mismatched_health_policy_is_not_treated_as_healthy() -> Result<(), RolloutError> {
        let evaluation = controller_evaluation(
            RolloutPhase::AdvertiseCanary,
            RolloutDisposition::Normal,
            true,
        );
        let mut health = controller_health(evaluation, RolloutHealthCounters::default());
        health.policy_id = ReleasePolicyId([99; 32]);
        let current = legacy_controller_state();
        assert_eq!(
            evaluate_rollout_controller(Some(evaluation), Some(health), current)?,
            RolloutControllerDecision::Hold {
                reason: RolloutControllerHoldReason::HealthPolicyMismatch,
                current,
            }
        );
        Ok(())
    }
}
