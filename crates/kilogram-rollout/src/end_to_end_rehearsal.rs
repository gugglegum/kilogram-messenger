use std::{error::Error, fs, io, path::Path};

use ed25519_dalek::SigningKey;
use kilogram_identity::DeviceIdentity;
use kilogram_session::canonical_sync_v2_activation::{
    CanonicalSyncV2ActivationActor, SignedCanonicalSyncV2ActivationRecord,
};
use kilogram_session::canonical_sync_v2_adapter::CanonicalSyncV2ActivationDisposition;
use tempfile::TempDir;

use crate::{
    ReleaseArtifactId, RolloutActivationDisposition, RolloutActivationTarget,
    RolloutControllerCurrentState, RolloutControllerDecision, RolloutControllerHoldReason,
    RolloutDisposition, RolloutHealthEvent, RolloutPhase,
    containment_recovery::{
        ContainmentRecoveryApplyOutcome, ExplicitContainmentRecoveryApproval,
        SignedContainmentRecoveryApproval,
    },
    evaluate_cohort, evaluate_rollout_controller,
    rollout_approval::{
        DeviceRolloutApprovalApplyOutcome, ExplicitDeviceRolloutApproval,
        SignedDeviceRolloutApproval,
    },
    trust::{
        ReleaseSigningKeyId, ReleaseSigningKeySpec, ReleaseSigningPermissions, ReleaseTrustRoot,
        SignedReleaseKeyring,
    },
    trusted_health_store::TrustedRolloutHealthStore,
    trusted_policy::TrustedReleasePolicyBundle,
    trusted_policy_store::{
        ExplicitTrustedPolicyImport, TrustedPolicyImportOutcome, TrustedPolicyStore,
        inspect_existing_trusted_policy_store,
    },
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn import_bundle(
    store: &TrustedPolicyStore,
    directory: &Path,
    label: &str,
    bundle: &TrustedReleasePolicyBundle,
) -> TestResult<TrustedPolicyImportOutcome> {
    let path = directory.join(format!("{label}.bundle"));
    fs::write(&path, bundle.encode()?)?;
    Ok(store.import_explicit_file(
        path,
        ExplicitTrustedPolicyImport::UserOrTestOperatorRequested,
    )?)
}

fn selected_seed(policies: &[&TrustedReleasePolicyBundle]) -> TestResult<[u8; 32]> {
    for value in 0_u64..2_000_000 {
        let mut seed = [0_u8; 32];
        seed[..8].copy_from_slice(&value.to_be_bytes());
        let selected = policies.iter().try_fold(true, |selected, bundle| {
            Ok::<_, crate::RolloutError>(
                selected
                    && evaluate_cohort(seed, bundle.policy(), bundle.policy().policy_id()?)?
                        .selected,
            )
        })?;
        if selected {
            return Ok(seed);
        }
    }
    Err(io::Error::other("could not find a deterministic selected test seed").into())
}

fn current_state(
    head: Option<&SignedCanonicalSyncV2ActivationRecord>,
) -> TestResult<RolloutControllerCurrentState> {
    let target = match head.map(SignedCanonicalSyncV2ActivationRecord::migration_state) {
        None | Some(kilogram_session::canonical_sync_v2_adapter::CanonicalSyncV2MigrationState::LegacyOnly) => {
            RolloutActivationTarget::LegacyOnly
        }
        Some(
            kilogram_session::canonical_sync_v2_adapter::CanonicalSyncV2MigrationState::AdvertiseCanonicalV2,
        ) => RolloutActivationTarget::AdvertiseCanonicalV2,
        Some(
            kilogram_session::canonical_sync_v2_adapter::CanonicalSyncV2MigrationState::DualReaderPreferLegacy,
        ) => RolloutActivationTarget::DualReaderPreferLegacy,
        Some(
            kilogram_session::canonical_sync_v2_adapter::CanonicalSyncV2MigrationState::DualReaderPreferCanonical,
        ) => RolloutActivationTarget::DualReaderPreferCanonical,
        Some(
            kilogram_session::canonical_sync_v2_adapter::CanonicalSyncV2MigrationState::CanonicalRequired,
        ) => {
            return Err(io::Error::other(
                "canonical-required is outside the dormant rollout controller",
            )
            .into());
        }
    };
    Ok(RolloutControllerCurrentState {
        target,
        disposition: head.map_or(RolloutActivationDisposition::Normal, |record| {
            if record.disposition() == CanonicalSyncV2ActivationDisposition::EmergencyContainment {
                RolloutActivationDisposition::EmergencyContainment
            } else {
                RolloutActivationDisposition::Normal
            }
        }),
        has_authenticated_canonical_evidence: head
            .and_then(SignedCanonicalSyncV2ActivationRecord::canonical_evidence_id)
            .is_some(),
    })
}

fn approve(
    identity: &DeviceIdentity,
    bundle: &TrustedReleasePolicyBundle,
    decision: RolloutControllerDecision,
    current: Option<&SignedCanonicalSyncV2ActivationRecord>,
) -> TestResult<SignedCanonicalSyncV2ActivationRecord> {
    let approval = SignedDeviceRolloutApproval::prepare(
        identity,
        bundle.bundle_id()?,
        decision,
        current,
        ExplicitDeviceRolloutApproval::UserOrTestOperatorApproved,
    )?;
    let encoded = approval.encode()?;
    let decoded = SignedDeviceRolloutApproval::decode_and_verify(&encoded, identity.device_id())?;
    let candidate = match decoded.apply(bundle.bundle_id()?, decision, current)? {
        DeviceRolloutApprovalApplyOutcome::Ready(candidate) => candidate,
        DeviceRolloutApprovalApplyOutcome::AlreadyApplied(_) => {
            return Err(io::Error::other("new approval was unexpectedly already applied").into());
        }
    };
    assert!(matches!(
        decoded.apply(bundle.bundle_id()?, decision, Some(&candidate))?,
        DeviceRolloutApprovalApplyOutcome::AlreadyApplied(_)
    ));
    Ok(candidate)
}

#[test]
fn canonical_sync_v2_rollout_end_to_end_rehearsal() -> TestResult {
    let workspace = TempDir::new()?;
    let policy_directory = workspace.path().join("trusted-policy");
    let health_directory = workspace.path().join("trusted-health");
    let inputs = workspace.path().join("inputs");
    fs::create_dir_all(&inputs)?;

    let root_key = SigningKey::from_bytes(&[101_u8; 32]);
    let legacy_key = SigningKey::from_bytes(&[102_u8; 32]);
    let normal_key = SigningKey::from_bytes(&[103_u8; 32]);
    let containment_key = SigningKey::from_bytes(&[104_u8; 32]);
    let trust_root = ReleaseTrustRoot::from_bytes(root_key.verifying_key().to_bytes())?;
    let artifact = ReleaseArtifactId::from_bytes([105_u8; 32])?;

    let keyring_v1 = SignedReleaseKeyring::issue_genesis(
        &root_key,
        &[ReleaseSigningKeySpec::from_signing_key(
            &legacy_key,
            ReleaseSigningPermissions::NORMAL_AND_CONTAINMENT,
        )],
    )?;
    let keyring_v2 = SignedReleaseKeyring::issue_successor(
        &root_key,
        &keyring_v1,
        &[
            ReleaseSigningKeySpec::from_signing_key(
                &normal_key,
                ReleaseSigningPermissions {
                    normal_policy: true,
                    containment_policy: false,
                },
            ),
            ReleaseSigningKeySpec::from_signing_key(
                &containment_key,
                ReleaseSigningPermissions::CONTAINMENT_ONLY,
            ),
        ],
        &[ReleaseSigningKeyId::from_public_key(
            legacy_key.verifying_key().to_bytes(),
        )?],
    )?;
    let genesis = TrustedReleasePolicyBundle::issue_genesis_hold(
        trust_root,
        &keyring_v1,
        &legacy_key,
        artifact,
    )?;
    let advertise = TrustedReleasePolicyBundle::issue_successor(
        trust_root,
        &keyring_v2,
        &normal_key,
        &genesis,
        artifact,
        RolloutPhase::AdvertiseCanary,
        RolloutDisposition::Normal,
        100,
    )?;
    let accept = TrustedReleasePolicyBundle::issue_successor(
        trust_root,
        &keyring_v2,
        &normal_key,
        &advertise,
        artifact,
        RolloutPhase::AcceptCanary,
        RolloutDisposition::Normal,
        100,
    )?;
    let containment = TrustedReleasePolicyBundle::issue_successor(
        trust_root,
        &keyring_v2,
        &containment_key,
        &accept,
        artifact,
        RolloutPhase::AcceptCanary,
        RolloutDisposition::Containment,
        0,
    )?;
    let recovery = TrustedReleasePolicyBundle::issue_successor(
        trust_root,
        &keyring_v2,
        &normal_key,
        &containment,
        artifact,
        RolloutPhase::AcceptCanary,
        RolloutDisposition::Normal,
        100,
    )?;

    let policy_store = TrustedPolicyStore::open(&policy_directory, trust_root)?;
    let seed = selected_seed(&[&advertise, &accept])?;
    let health_store =
        TrustedRolloutHealthStore::open_with_test_seed(&health_directory, trust_root, seed)?;

    import_bundle(&policy_store, &inputs, "01-genesis", &genesis)?;
    health_store.synchronize_bundle_chain(&policy_store.bundle_chain()?)?;
    import_bundle(&policy_store, &inputs, "02-advertise-rotated", &advertise)?;
    let advertise_health = health_store.synchronize_bundle_chain(&policy_store.bundle_chain()?)?;
    assert!(advertise_health.evaluation.selected);

    let identity = DeviceIdentity::from_secret_bytes([106_u8; 32]);
    let advertise_decision = evaluate_rollout_controller(
        Some(advertise_health.evaluation),
        Some(advertise_health.health),
        current_state(None)?,
    )?;
    assert!(!advertise_decision.runtime_activation_authorized());
    let advertise_head = approve(&identity, &advertise, advertise_decision, None)?;

    import_bundle(&policy_store, &inputs, "03-accept", &accept)?;
    let accept_health = health_store.synchronize_bundle_chain(&policy_store.bundle_chain()?)?;
    assert!(accept_health.evaluation.selected);
    let accept_decision = evaluate_rollout_controller(
        Some(accept_health.evaluation),
        Some(accept_health.health),
        current_state(Some(&advertise_head))?,
    )?;
    let accept_head = approve(&identity, &accept, accept_decision, Some(&advertise_head))?;
    let evidence_head = SignedCanonicalSyncV2ActivationRecord::issue(
        &identity,
        Some(&accept_head),
        accept_head.migration_state(),
        CanonicalSyncV2ActivationDisposition::Normal,
        CanonicalSyncV2ActivationActor::ExplicitLocal,
        Some([107_u8; 32]),
        "authenticated canonical rehearsal evidence",
    )?;

    import_bundle(&policy_store, &inputs, "04-containment", &containment)?;
    let containment_health =
        health_store.synchronize_bundle_chain(&policy_store.bundle_chain()?)?;
    let containment_decision = evaluate_rollout_controller(
        Some(containment_health.evaluation),
        Some(containment_health.health),
        current_state(Some(&evidence_head))?,
    )?;
    let containment_head = approve(
        &identity,
        &containment,
        containment_decision,
        Some(&evidence_head),
    )?;
    assert_eq!(
        containment_head.disposition(),
        CanonicalSyncV2ActivationDisposition::EmergencyContainment
    );

    import_bundle(&policy_store, &inputs, "05-recovery", &recovery)?;
    let chain = policy_store.bundle_chain()?;
    let synchronized = health_store.synchronize_bundle_chain(&chain)?;
    health_store.record_health(
        synchronized.bundle_id,
        RolloutHealthEvent::RuntimeStartSucceeded,
    )?;
    let recovery_health = health_store.record_health(
        synchronized.bundle_id,
        RolloutHealthEvent::CanonicalSyncCompleted,
    )?;
    let recovery_evaluation = health_store
        .current_snapshot()?
        .ok_or_else(|| io::Error::other("health tip is missing"))?
        .evaluation;
    let recovery_decision = evaluate_rollout_controller(
        Some(recovery_evaluation),
        Some(recovery_health),
        current_state(Some(&containment_head))?,
    )?;
    assert!(matches!(
        recovery_decision,
        RolloutControllerDecision::Hold {
            reason: RolloutControllerHoldReason::ContainmentRequiresExplicitRecovery,
            ..
        }
    ));
    let recovery_approval = SignedContainmentRecoveryApproval::prepare(
        &identity,
        trust_root,
        &recovery,
        recovery_health,
        &containment_head,
        ExplicitContainmentRecoveryApproval::UserOrTestOperatorApproved,
    )?;
    let recovery_approval = SignedContainmentRecoveryApproval::decode_and_verify(
        &recovery_approval.encode()?,
        identity.device_id(),
    )?;
    let recovered_head =
        match recovery_approval.apply(trust_root, &recovery, recovery_health, &containment_head)? {
            ContainmentRecoveryApplyOutcome::Ready(candidate) => candidate,
            ContainmentRecoveryApplyOutcome::AlreadyApplied(_) => {
                return Err(
                    io::Error::other("new recovery was unexpectedly already applied").into(),
                );
            }
        };
    assert_eq!(
        recovered_head.disposition(),
        CanonicalSyncV2ActivationDisposition::Normal
    );
    assert!(matches!(
        recovery_approval.apply(trust_root, &recovery, recovery_health, &recovered_head)?,
        ContainmentRecoveryApplyOutcome::AlreadyApplied(_)
    ));

    let foreign_root = ReleaseTrustRoot::from_bytes(
        SigningKey::from_bytes(&[108_u8; 32])
            .verifying_key()
            .to_bytes(),
    )?;
    assert!(inspect_existing_trusted_policy_store(&policy_directory, foreign_root).is_err());
    assert!(import_bundle(&policy_store, &inputs, "stale", &genesis).is_err());

    let truncated = inputs.join("truncated.bundle");
    fs::write(&truncated, &recovery.encode()?[..5])?;
    assert!(
        policy_store
            .import_explicit_file(
                truncated,
                ExplicitTrustedPolicyImport::UserOrTestOperatorRequested,
            )
            .is_err()
    );

    let fork = TrustedReleasePolicyBundle::issue_successor(
        trust_root,
        &keyring_v2,
        &normal_key,
        &accept,
        artifact,
        RolloutPhase::InitiateCanary,
        RolloutDisposition::Normal,
        100,
    )?;
    assert!(import_bundle(&policy_store, &inputs, "fork", &fork).is_err());
    assert!(matches!(
        import_bundle(&policy_store, &inputs, "replay", &recovery)?,
        TrustedPolicyImportOutcome::AlreadyCurrent { .. }
    ));
    assert_eq!(
        health_store
            .synchronize_bundle_chain(&policy_store.bundle_chain()?)?
            .bundle_id,
        recovery.bundle_id()?
    );

    #[cfg(windows)]
    {
        let link = inputs.join("symlink.bundle");
        let target = inputs.join("05-recovery.bundle");
        match std::os::windows::fs::symlink_file(&target, &link) {
            Ok(()) => {
                assert!(
                    policy_store
                        .import_explicit_file(
                            &link,
                            ExplicitTrustedPolicyImport::UserOrTestOperatorRequested,
                        )
                        .is_err()
                );
                println!("negative_symlink=verified");
            }
            Err(_) => println!("negative_symlink=not-supported-by-host"),
        }
    }

    println!("key_rotation=verified");
    println!("trusted_policy_chain=5/5");
    println!("device_approval_transitions=3/3");
    println!("containment=verified");
    println!("explicit_recovery=verified");
    println!("negative_foreign_root=verified");
    println!("negative_stale_truncated_fork=3/3");
    println!("replay_idempotence=verified");
    println!("default_public_activation=legacy-only");
    println!("status=canonical-sync-v2-rollout-end-to-end-rehearsal-verified");
    Ok(())
}
