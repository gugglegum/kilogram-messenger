use std::{
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, ensure};
use kilogram_identity::{
    AccountAuthoritySnapshot, AccountRecoveryPolicyState, AccountRecoveryPolicyTransitionApproval,
    AccountRecoveryPolicyTransitionCertificate, AccountRecoveryPolicyTransitionRequest,
    AccountRootRecoveryApproval, AccountRootRecoveryPackage, AccountRootRecoveryWitness,
    ConversationMembershipSnapshot, DeviceCapability, DeviceCertificate, DeviceState,
    MAX_ACCOUNT_RECOVERY_POLICY_CERTIFICATE_BYTES,
    MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_APPROVAL_BYTES,
    MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_APPROVALS,
    MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_REQUEST_BYTES, MAX_ACCOUNT_ROOT_RECOVERY_PACKAGE_BYTES,
    MAX_ACCOUNT_ROOT_RECOVERY_WITNESS_BYTES, verify_account_recovery_policy_transition,
    verify_device_authorization_with_snapshot,
};
use kilogram_state::{
    DeviceIdentityStateRepository, EncryptedStateVault, StateDirectoryLock, StateMirrorRepository,
    StateRecordKind, StateTransaction, TrustStateRepository, VaultPrimaryWriteRepository,
};
use serde::Serialize;
use tempfile::NamedTempFile;

const DEVICE_CERTIFICATE_PATH: &str = "device-certificate.cert";
const AUTHORITY_SNAPSHOT_PATH: &str = "account-authority.snapshot";
const MEMBERSHIP_PREFIX: &str = "conversation-memberships/";
pub(crate) const RECOVERY_APPROVAL_HEAD_PATH: &str = "recovery-approval/latest.approval";
pub(crate) const RECOVERY_POLICY_STATE_PATH: &str = "recovery-policy/current.policy";
pub(crate) const RECOVERY_POLICY_CERTIFICATE_PATH: &str = "recovery-policy/latest-transition.karpc";
const RECOVERY_POLICY_APPROVAL_HEAD_PATH: &str = "recovery-policy/latest-transition-approval.karpa";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryPolicyTransitionRequestOutput {
    status: &'static str,
    account_id: String,
    request_id: String,
    old_epoch: u64,
    new_epoch: u64,
    previous_transition_id: String,
    old_recovery_roster_digest: String,
    new_recovery_roster_digest: String,
    old_roster_count: usize,
    new_roster_count: usize,
    old_required_approvals: usize,
    new_required_approvals: usize,
    expires_at_unix_seconds: u64,
    request_file: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryPolicyTransitionApprovalOutput {
    status: &'static str,
    account_id: String,
    request_id: String,
    old_epoch: u64,
    new_epoch: u64,
    approver_device_id: String,
    approval_id: String,
    approval_file: PathBuf,
    approval_head_committed_before_publish: bool,
    reused_committed_approval: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryPolicyTransitionCertificateOutput {
    status: &'static str,
    account_id: String,
    certificate_id: String,
    old_epoch: u64,
    new_epoch: u64,
    previous_transition_id: String,
    old_recovery_roster_digest: String,
    new_recovery_roster_digest: String,
    old_observed_approvals: usize,
    old_required_approvals: usize,
    new_observed_approvals: usize,
    new_required_approvals: usize,
    joint_majority_satisfied: bool,
    cross_roster_fork_safety: bool,
    certificate_file: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryPolicyInstallOutput {
    status: &'static str,
    account_id: String,
    policy_epoch: u64,
    recovery_roster_digest: String,
    latest_transition_id: String,
    policy_state_source: &'static str,
    installed_certificate_file: PathBuf,
    cross_roster_fork_safety: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn create_transition_request(
    old_package_file: impl AsRef<Path>,
    old_witness_file: impl AsRef<Path>,
    new_package_file: impl AsRef<Path>,
    new_witness_file: impl AsRef<Path>,
    old_epoch: u64,
    previous_transition_id_hex: &str,
    request_file: impl AsRef<Path>,
    validity_seconds: u64,
) -> Result<RecoveryPolicyTransitionRequestOutput> {
    let old_package = read_package(old_package_file.as_ref())?;
    let old_witness = read_witness(old_witness_file.as_ref())?;
    let new_package = read_package(new_package_file.as_ref())?;
    let new_witness = read_witness(new_witness_file.as_ref())?;
    let previous_transition_id = decode_hex_32(previous_transition_id_hex)
        .context("parse previous recovery-policy transition ID")?;
    let now = unix_time_now()?;
    let request = AccountRecoveryPolicyTransitionRequest::issue(
        old_epoch,
        previous_transition_id,
        old_package,
        old_witness,
        new_package,
        new_witness,
        now,
        validity_seconds,
    )
    .context("issue recovery-policy transition request")?;
    write_new(request_file.as_ref(), &request.encode()?)?;
    transition_request_output(&request, absolute_existing_path(request_file.as_ref())?)
}

pub fn approve_transition(
    state_dir: impl AsRef<Path>,
    request_file: impl AsRef<Path>,
    approval_file: impl AsRef<Path>,
) -> Result<RecoveryPolicyTransitionApprovalOutput> {
    let now = unix_time_now()?;
    let request = read_transition_request(request_file.as_ref(), now)?;
    let state_dir = fs::canonicalize(state_dir.as_ref()).with_context(|| {
        format!(
            "resolve recovery-policy approving device state {}",
            state_dir.as_ref().display()
        )
    })?;
    ensure!(
        state_dir.is_dir(),
        "approving device state is not a directory"
    );
    let _lock = StateDirectoryLock::acquire(&state_dir)
        .context("lock recovery-policy approving device state")?;
    ensure!(
        EncryptedStateVault::is_initialized(&state_dir)?,
        "recovery-policy approval requires an initialized DB-primary vault"
    );
    let vault = EncryptedStateVault::open_existing(&state_dir)
        .context("open recovery-policy approving device DB-primary vault")?;
    vault.recover_primary_shadow()?;
    vault.recover_pending_dual_write()?;
    let identity = vault.read_primary_device_identity()?;
    let device = DeviceState::from_secret_material(
        &state_dir,
        *identity.signing_secret(),
        *identity.encryption_secret(),
    );
    let trust = vault.read_primary_trust()?;
    let certificate = decode_required_trust_record::<DeviceCertificate>(
        &trust,
        DEVICE_CERTIFICATE_PATH,
        DeviceCertificate::decode_and_verify,
    )?;
    ensure!(
        certificate.device_id() == device.identity().device_id(),
        "DB-primary certificate belongs to a different device"
    );
    let authority = decode_required_trust_record::<AccountAuthoritySnapshot>(
        &trust,
        AUTHORITY_SNAPSHOT_PATH,
        AccountAuthoritySnapshot::decode_and_verify,
    )?;
    let memberships = owned_memberships(&trust, request.account_id())?;
    let in_old = package_authorizes_certificate(request.old_package(), &certificate);
    let in_new = package_authorizes_certificate(request.new_package(), &certificate);
    ensure!(
        in_old || in_new,
        "local device is not active in either transition roster"
    );
    if in_new {
        request
            .new_package()
            .verify_dominates_device_state(&certificate, &authority, &memberships)
            .context("new recovery-policy package does not dominate local device state")?;
    } else {
        request
            .old_package()
            .verify_dominates_device_state(&certificate, &authority, &memberships)
            .context("old recovery-policy package does not match local old-only voter state")?;
    }

    let policy_state = optional_policy_state(&trust)?;
    if in_old {
        match policy_state.as_ref() {
            Some(policy) => policy
                .verify_transition_anchor(&request)
                .context("transition does not extend the DB-primary recovery-policy anchor")?,
            None => verify_legacy_genesis_anchor(&trust, &request)?,
        }
    }
    let existing = optional_transition_approval(&trust)?;
    if let Some(existing) = existing.as_ref()
        && existing.request_id() == &request.request_id()?
    {
        let encoded = existing.encode()?;
        write_idempotent(approval_file.as_ref(), &encoded)?;
        return transition_approval_output(
            existing,
            absolute_existing_path(approval_file.as_ref())?,
            true,
        );
    }
    if let Some(existing) = existing.as_ref() {
        ensure!(
            existing.account_id() == request.account_id()
                && existing.old_epoch() < request.old_epoch(),
            "device already committed a different recovery-policy transition for this or a later epoch"
        );
    }
    let previous_approval_id = existing
        .as_ref()
        .map(AccountRecoveryPolicyTransitionApproval::approval_id)
        .transpose()?
        .unwrap_or([0_u8; 32]);
    let approval = AccountRecoveryPolicyTransitionApproval::issue(
        device.identity(),
        &request,
        previous_approval_id,
        now,
    )?;
    let approval_bytes = approval.encode()?;

    vault.begin_dual_write()?;
    let commit_result = commit_transition_approval_primary(
        &state_dir,
        &vault,
        &device,
        if in_new {
            request.new_package()
        } else {
            request.old_package()
        },
        (in_old && policy_state.is_none())
            .then(|| AccountRecoveryPolicyState::genesis(request.old_package()))
            .transpose()?,
        &approval_bytes,
    );
    let mirror_result = vault
        .finish_dual_write()
        .context("complete policy approval vault mirror");
    combine_dual_write(commit_result, mirror_result.map(|_| ()))?;
    write_idempotent(approval_file.as_ref(), &approval_bytes)?;
    transition_approval_output(
        &approval,
        absolute_existing_path(approval_file.as_ref())?,
        false,
    )
}

pub fn certify_transition(
    request_file: impl AsRef<Path>,
    approval_files: &[PathBuf],
    certificate_file: impl AsRef<Path>,
) -> Result<RecoveryPolicyTransitionCertificateOutput> {
    ensure!(
        approval_files.len() <= MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_APPROVALS,
        "too many transition approval files"
    );
    let now = unix_time_now()?;
    let request = read_transition_request(request_file.as_ref(), now)?;
    let mut approvals = Vec::with_capacity(approval_files.len());
    for path in approval_files {
        approvals.push(AccountRecoveryPolicyTransitionApproval::decode_and_verify(
            &read_bounded_regular_file(
                path,
                MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_APPROVAL_BYTES,
                "recovery-policy transition approval",
            )?,
        )?);
    }
    let certificate = AccountRecoveryPolicyTransitionCertificate::issue(request, approvals, now)
        .context("require old and new strict-majority transition quorums")?;
    write_new(certificate_file.as_ref(), &certificate.encode()?)?;
    transition_certificate_output(
        &certificate,
        absolute_existing_path(certificate_file.as_ref())?,
    )
}

pub fn verify_transition_certificate(
    certificate_file: impl AsRef<Path>,
) -> Result<RecoveryPolicyTransitionCertificateOutput> {
    let certificate = read_transition_certificate(certificate_file.as_ref())?;
    transition_certificate_output(
        &certificate,
        absolute_existing_path(certificate_file.as_ref())?,
    )
}

pub fn install_transition(
    state_dir: impl AsRef<Path>,
    certificate_file: impl AsRef<Path>,
) -> Result<RecoveryPolicyInstallOutput> {
    let certificate = read_transition_certificate(certificate_file.as_ref())?;
    let state_dir = fs::canonicalize(state_dir.as_ref())?;
    let _lock = StateDirectoryLock::acquire(&state_dir)?;
    ensure!(
        EncryptedStateVault::is_initialized(&state_dir)?,
        "recovery-policy install requires an initialized DB-primary vault"
    );
    let vault = EncryptedStateVault::open_existing(&state_dir)?;
    vault.recover_primary_shadow()?;
    vault.recover_pending_dual_write()?;
    let identity = vault.read_primary_device_identity()?;
    let device = DeviceState::from_secret_material(
        &state_dir,
        *identity.signing_secret(),
        *identity.encryption_secret(),
    );
    let trust = vault.read_primary_trust()?;
    let local_certificate = decode_required_trust_record::<DeviceCertificate>(
        &trust,
        DEVICE_CERTIFICATE_PATH,
        DeviceCertificate::decode_and_verify,
    )?;
    let target = AccountRecoveryPolicyState::transitioned(&certificate)?;
    let existing = optional_policy_state(&trust)?;
    let source = if existing.as_ref() == Some(&target) {
        "db-primary-idempotent"
    } else {
        ensure!(
            package_authorizes_certificate(certificate.new_package(), &local_certificate),
            "only an active voter in the new recovery roster may install this transition"
        );
        let authority = decode_required_trust_record::<AccountAuthoritySnapshot>(
            &trust,
            AUTHORITY_SNAPSHOT_PATH,
            AccountAuthoritySnapshot::decode_and_verify,
        )?;
        let memberships = owned_memberships(&trust, certificate.account_id())?;
        certificate.new_package().verify_dominates_device_state(
            &local_certificate,
            &authority,
            &memberships,
        )?;
        match existing.as_ref() {
            Some(policy) => policy.verify_transition_anchor(certificate.request())?,
            None if package_authorizes_certificate(
                certificate.old_package(),
                &local_certificate,
            ) =>
            {
                verify_legacy_genesis_anchor(&trust, certificate.request())?;
            }
            None => {}
        }
        vault.begin_dual_write()?;
        let commit_result = commit_policy_install_primary(
            &state_dir,
            &vault,
            &device,
            certificate.new_package(),
            &target.encode()?,
            &certificate.encode()?,
        );
        let mirror_result = vault
            .finish_dual_write()
            .context("complete policy install vault mirror");
        combine_dual_write(commit_result, mirror_result.map(|_| ()))?;
        "db-primary-installed"
    };
    Ok(RecoveryPolicyInstallOutput {
        status: "account-recovery-policy-transition-installed",
        account_id: target.account_id().to_string(),
        policy_epoch: target.epoch(),
        recovery_roster_digest: encode_hex(target.recovery_roster_digest()),
        latest_transition_id: encode_hex(target.latest_transition_id()),
        policy_state_source: source,
        installed_certificate_file: state_dir.join(RECOVERY_POLICY_CERTIFICATE_PATH),
        cross_roster_fork_safety: true,
    })
}

pub(crate) fn verify_candidate_against_certificate(
    candidate: &AccountRootRecoveryPackage,
    certificate: &AccountRecoveryPolicyTransitionCertificate,
) -> Result<()> {
    certificate.verify()?;
    ensure!(
        candidate.recovery_roster_digest()? == certificate.new_recovery_roster_digest()?,
        "recovery candidate roster does not match certified new policy roster"
    );
    kilogram_identity::verify_recovery_package_successor(
        certificate.new_package(),
        candidate,
        false,
    )
    .context("recovery candidate does not extend the certified new policy state")
}

fn commit_transition_approval_primary(
    state_dir: &Path,
    vault: &EncryptedStateVault,
    device: &DeviceState,
    package: &AccountRootRecoveryPackage,
    genesis: Option<AccountRecoveryPolicyState>,
    approval_bytes: &[u8],
) -> Result<()> {
    commit_policy_trust_primary(state_dir, vault, device, package, |state_dir| {
        if let Some(genesis) = genesis {
            write_replace(
                &state_dir.join(RECOVERY_POLICY_STATE_PATH),
                &genesis.encode()?,
            )?;
        }
        write_replace(
            &state_dir.join(RECOVERY_POLICY_APPROVAL_HEAD_PATH),
            approval_bytes,
        )
    })
}

fn commit_policy_install_primary(
    state_dir: &Path,
    vault: &EncryptedStateVault,
    device: &DeviceState,
    package: &AccountRootRecoveryPackage,
    policy_bytes: &[u8],
    certificate_bytes: &[u8],
) -> Result<()> {
    commit_policy_trust_primary(state_dir, vault, device, package, |state_dir| {
        write_replace(&state_dir.join(RECOVERY_POLICY_STATE_PATH), policy_bytes)?;
        write_replace(
            &state_dir.join(RECOVERY_POLICY_CERTIFICATE_PATH),
            certificate_bytes,
        )
    })
}

fn commit_policy_trust_primary(
    state_dir: &Path,
    vault: &EncryptedStateVault,
    device: &DeviceState,
    package: &AccountRootRecoveryPackage,
    write_policy: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    let trust = vault.read_primary_trust()?;
    let mut transaction = StateTransaction::begin(state_dir)?;
    if let Err(error) = transaction.prepare_trust_workspace(&trust) {
        let _ = transaction.rollback();
        return Err(error).context("prepare DB-primary recovery-policy trust workspace");
    }
    let operation = device
        .install_own_authority_snapshot(package.authority_snapshot())
        .context("advance recovery-policy authority high-water")
        .and_then(|_| {
            for membership in package.conversation_memberships() {
                device.install_conversation_membership(membership)?;
            }
            write_policy(state_dir)
        });
    if let Err(error) = operation {
        return match transaction.rollback() {
            Ok(()) => Err(error),
            Err(rollback) => Err(error.context(format!(
                "policy staging failed and trust rollback also failed: {rollback}"
            ))),
        };
    }
    if let Err(error) = vault.commit_primary_transaction(&transaction) {
        return match transaction.rollback() {
            Ok(()) => Err(error).context("commit recovery-policy state to DB-primary vault"),
            Err(rollback) => Err(anyhow::Error::new(error).context(format!(
                "policy vault commit failed and trust rollback also failed: {rollback}"
            ))),
        };
    }
    transaction.commit()?;
    vault.confirm_primary_shadow()?;
    Ok(())
}

fn verify_legacy_genesis_anchor(
    trust: &kilogram_state::VaultMutableRead,
    request: &AccountRecoveryPolicyTransitionRequest,
) -> Result<()> {
    ensure!(
        request.old_epoch() == 0 && request.previous_transition_id() == &[0_u8; 32],
        "a non-genesis policy transition requires an installed policy anchor"
    );
    let head = trust
        .records()
        .iter()
        .find(|record| record.relative_path() == RECOVERY_APPROVAL_HEAD_PATH)
        .context("legacy policy genesis requires an existing recovery approval head")?;
    let approval = AccountRootRecoveryApproval::decode_and_verify(head.content())?;
    ensure!(
        approval.account_id() == request.account_id()
            && approval.recovery_roster_digest() == &request.old_recovery_roster_digest()?,
        "legacy recovery approval head does not anchor the old transition roster"
    );
    Ok(())
}

fn optional_policy_state(
    trust: &kilogram_state::VaultMutableRead,
) -> Result<Option<AccountRecoveryPolicyState>> {
    trust
        .records()
        .iter()
        .find(|record| record.relative_path() == RECOVERY_POLICY_STATE_PATH)
        .map(|record| AccountRecoveryPolicyState::decode_and_verify(record.content()))
        .transpose()
        .context("decode DB-primary recovery-policy state")
}

fn optional_transition_approval(
    trust: &kilogram_state::VaultMutableRead,
) -> Result<Option<AccountRecoveryPolicyTransitionApproval>> {
    trust
        .records()
        .iter()
        .find(|record| record.relative_path() == RECOVERY_POLICY_APPROVAL_HEAD_PATH)
        .map(|record| AccountRecoveryPolicyTransitionApproval::decode_and_verify(record.content()))
        .transpose()
        .context("decode DB-primary recovery-policy transition approval head")
}

fn owned_memberships(
    trust: &kilogram_state::VaultMutableRead,
    account_id: kilogram_identity::AccountId,
) -> Result<Vec<ConversationMembershipSnapshot>> {
    let mut memberships = Vec::new();
    for record in trust.records() {
        if record.kind() == StateRecordKind::Trust
            && record.relative_path().starts_with(MEMBERSHIP_PREFIX)
        {
            let membership = ConversationMembershipSnapshot::decode_and_verify(record.content())?;
            if membership.owner_account_id() == account_id {
                memberships.push(membership);
            }
        }
    }
    Ok(memberships)
}

fn package_authorizes_certificate(
    package: &AccountRootRecoveryPackage,
    local: &DeviceCertificate,
) -> bool {
    package
        .device_list()
        .certificate_for(local.device_id())
        .is_some_and(|candidate| {
            candidate == local
                && verify_device_authorization_with_snapshot(
                    package.account_id(),
                    candidate,
                    package.authority_snapshot(),
                    &DeviceCapability::MESSAGING,
                )
                .is_ok()
        })
}

fn transition_request_output(
    request: &AccountRecoveryPolicyTransitionRequest,
    request_file: PathBuf,
) -> Result<RecoveryPolicyTransitionRequestOutput> {
    Ok(RecoveryPolicyTransitionRequestOutput {
        status: "account-recovery-policy-transition-request-created",
        account_id: request.account_id().to_string(),
        request_id: encode_hex(&request.request_id()?),
        old_epoch: request.old_epoch(),
        new_epoch: request.new_epoch(),
        previous_transition_id: encode_hex(request.previous_transition_id()),
        old_recovery_roster_digest: encode_hex(&request.old_recovery_roster_digest()?),
        new_recovery_roster_digest: encode_hex(&request.new_recovery_roster_digest()?),
        old_roster_count: request.old_roster_count(),
        new_roster_count: request.new_roster_count(),
        old_required_approvals: request.old_required_approvals(),
        new_required_approvals: request.new_required_approvals(),
        expires_at_unix_seconds: request.expires_at_unix_seconds(),
        request_file,
    })
}

fn transition_approval_output(
    approval: &AccountRecoveryPolicyTransitionApproval,
    approval_file: PathBuf,
    reused: bool,
) -> Result<RecoveryPolicyTransitionApprovalOutput> {
    Ok(RecoveryPolicyTransitionApprovalOutput {
        status: "account-recovery-policy-transition-approved",
        account_id: approval.account_id().to_string(),
        request_id: encode_hex(approval.request_id()),
        old_epoch: approval.old_epoch(),
        new_epoch: approval.new_epoch(),
        approver_device_id: approval.approver_device_id().to_string(),
        approval_id: encode_hex(&approval.approval_id()?),
        approval_file,
        approval_head_committed_before_publish: true,
        reused_committed_approval: reused,
    })
}

fn transition_certificate_output(
    certificate: &AccountRecoveryPolicyTransitionCertificate,
    certificate_file: PathBuf,
) -> Result<RecoveryPolicyTransitionCertificateOutput> {
    let report =
        verify_account_recovery_policy_transition(certificate.request(), certificate.approvals())?;
    report.require_joint_majority()?;
    Ok(RecoveryPolicyTransitionCertificateOutput {
        status: "account-recovery-policy-transition-certified",
        account_id: certificate.account_id().to_string(),
        certificate_id: encode_hex(&certificate.certificate_id()?),
        old_epoch: certificate.old_epoch(),
        new_epoch: certificate.new_epoch(),
        previous_transition_id: encode_hex(certificate.previous_transition_id()),
        old_recovery_roster_digest: encode_hex(&certificate.old_recovery_roster_digest()?),
        new_recovery_roster_digest: encode_hex(&certificate.new_recovery_roster_digest()?),
        old_observed_approvals: report.old_observed(),
        old_required_approvals: report.old_required(),
        new_observed_approvals: report.new_observed(),
        new_required_approvals: report.new_required(),
        joint_majority_satisfied: true,
        cross_roster_fork_safety: true,
        certificate_file,
    })
}

fn read_transition_request(
    path: &Path,
    now: u64,
) -> Result<AccountRecoveryPolicyTransitionRequest> {
    AccountRecoveryPolicyTransitionRequest::decode_and_verify(
        &read_bounded_regular_file(
            path,
            MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_REQUEST_BYTES,
            "recovery-policy transition request",
        )?,
        now,
    )
    .context("decode and verify current recovery-policy transition request")
}

fn read_transition_certificate(path: &Path) -> Result<AccountRecoveryPolicyTransitionCertificate> {
    AccountRecoveryPolicyTransitionCertificate::decode_and_verify(&read_bounded_regular_file(
        path,
        MAX_ACCOUNT_RECOVERY_POLICY_CERTIFICATE_BYTES,
        "recovery-policy transition certificate",
    )?)
    .context("decode and verify recovery-policy transition certificate")
}

fn read_package(path: &Path) -> Result<AccountRootRecoveryPackage> {
    AccountRootRecoveryPackage::decode_and_verify(&read_bounded_regular_file(
        path,
        MAX_ACCOUNT_ROOT_RECOVERY_PACKAGE_BYTES,
        "Account Root recovery package",
    )?)
    .context("decode Account Root recovery package")
}

fn read_witness(path: &Path) -> Result<AccountRootRecoveryWitness> {
    AccountRootRecoveryWitness::decode_and_verify(&read_bounded_regular_file(
        path,
        MAX_ACCOUNT_ROOT_RECOVERY_WITNESS_BYTES,
        "Account Root recovery witness",
    )?)
    .context("decode Account Root recovery witness")
}

fn decode_required_trust_record<T>(
    trust: &kilogram_state::VaultMutableRead,
    path: &str,
    decode: impl FnOnce(&[u8]) -> Result<T, kilogram_identity::IdentityError>,
) -> Result<T> {
    let record = trust
        .records()
        .iter()
        .find(|record| record.relative_path() == path)
        .with_context(|| format!("DB-primary trust record is missing: {path}"))?;
    ensure!(
        record.kind() == StateRecordKind::Trust,
        "unexpected record kind"
    );
    decode(record.content()).with_context(|| format!("decode DB-primary trust record {path}"))
}

fn read_bounded_regular_file(path: &Path, maximum: usize, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("inspect {label} {}", path.display()))?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "{label} must be a regular file"
    );
    let length = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
    ensure!(length <= maximum, "{label} exceeds maximum size");
    fs::read(path).with_context(|| format!("read {label} {}", path.display()))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("artifact has no parent")?;
    fs::create_dir_all(parent)?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(path)
        .map_err(|error| error.error)
        .with_context(|| format!("publish no-clobber artifact {}", path.display()))?;
    Ok(())
}

fn write_idempotent(path: &Path, bytes: &[u8]) -> Result<()> {
    match fs::read(path) {
        Ok(existing) => {
            ensure!(existing == bytes, "existing artifact has different content");
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => write_new(path, bytes),
        Err(error) => Err(error.into()),
    }
}

fn write_replace(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("policy path has no parent")?;
    fs::create_dir_all(parent)?;
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn combine_dual_write(primary: Result<()>, mirror: Result<()>) -> Result<()> {
    match (primary, mirror) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(mirror)) => Err(error.context(format!(
            "DB-primary operation failed and vault mirror also failed: {mirror:#}"
        ))),
    }
}

fn absolute_existing_path(path: &Path) -> Result<PathBuf> {
    fs::canonicalize(path).with_context(|| format!("resolve artifact {}", path.display()))
}

fn unix_time_now() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before Unix epoch")?
        .as_secs())
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

fn decode_hex_32(value: &str) -> Result<[u8; 32]> {
    ensure!(value.len() == 64, "expected 64 hexadecimal characters");
    let mut result = [0_u8; 32];
    for (index, byte) in result.iter_mut().enumerate() {
        let offset = index * 2;
        *byte = u8::from_str_radix(&value[offset..offset + 2], 16)
            .with_context(|| format!("invalid hexadecimal at character {offset}"))?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use kilogram_identity::AccountRootState;

    use super::*;

    #[test]
    fn one_to_two_transition_requires_both_and_unfreezes_new_roster() -> Result<(), Box<dyn Error>>
    {
        let parent = tempfile::tempdir()?;
        let owner = parent.path().join("owner");
        let first = crate::create_account(&owner)?;
        let old_package = parent.path().join("old.karp");
        let old_witness = parent.path().join("old.karw");
        crate::account_recovery::export_account_root(
            first.account_root_dir(),
            &old_package,
            &old_witness,
        )?;
        let old_request = parent.path().join("old.karq");
        crate::recovery_quorum::create_request(&old_package, &old_witness, &old_request, 600)?;
        crate::recovery_quorum::approve_request(
            first.state_dir(),
            &old_request,
            parent.path().join("old.kara"),
        )?;

        let joining = parent.path().join("joining");
        let link = crate::device_link::create_request(&joining, first.account_id())?;
        let response = parent.path().join("link.kdl");
        let device_list = owner.join("public").join("account-device-list.snapshot");
        crate::device_link::authorize_request(
            first.account_root_dir(),
            link.request_file(),
            link.sas(),
            &response,
            &device_list,
        )?;
        let joined = crate::device_link::accept_response(&joining, &response)?;
        let new_package = parent.path().join("new.karp");
        let new_witness = parent.path().join("new.karw");
        crate::account_recovery::export_account_root(
            first.account_root_dir(),
            &new_package,
            &new_witness,
        )?;
        let transition_request = parent.path().join("transition.karpt");
        create_transition_request(
            &old_package,
            &old_witness,
            &new_package,
            &new_witness,
            0,
            &"0".repeat(64),
            &transition_request,
            600,
        )?;
        let first_approval = parent.path().join("first.karpa");
        approve_transition(first.state_dir(), &transition_request, &first_approval)?;
        let conflicting_request = parent.path().join("conflicting.karpt");
        create_transition_request(
            &old_package,
            &old_witness,
            &new_package,
            &new_witness,
            0,
            &"0".repeat(64),
            &conflicting_request,
            600,
        )?;
        assert!(
            approve_transition(
                first.state_dir(),
                &conflicting_request,
                parent.path().join("conflicting.karpa"),
            )
            .is_err()
        );
        assert!(
            certify_transition(
                &transition_request,
                std::slice::from_ref(&first_approval),
                parent.path().join("partial.karpc"),
            )
            .is_err()
        );
        let second_approval = parent.path().join("second.karpa");
        approve_transition(joined.state_dir(), &transition_request, &second_approval)?;
        let policy_certificate = parent.path().join("transition.karpc");
        let certified = certify_transition(
            &transition_request,
            &[first_approval, second_approval],
            &policy_certificate,
        )?;
        assert!(certified.cross_roster_fork_safety);
        install_transition(first.state_dir(), &policy_certificate)?;
        install_transition(joining.join("device"), &policy_certificate)?;

        let new_request = parent.path().join("new.karq");
        crate::recovery_quorum::create_request(&new_package, &new_witness, &new_request, 600)?;
        let first_new_approval = parent.path().join("new-first.kara");
        crate::recovery_quorum::approve_request(
            first.state_dir(),
            &new_request,
            &first_new_approval,
        )?;
        let second_new_approval = parent.path().join("new-second.kara");
        crate::recovery_quorum::approve_request(
            joined.state_dir(),
            &new_request,
            &second_new_approval,
        )?;
        let plain = crate::recovery_quorum::verify_request(
            &new_request,
            &[first_new_approval.clone(), second_new_approval.clone()],
            true,
        )?;
        assert!(!plain.cross_roster_fork_safety());
        let policy_bound = crate::recovery_quorum::verify_request_with_policy_certificate(
            &new_request,
            &[first_new_approval, second_new_approval],
            &policy_certificate,
            true,
        )?;
        assert!(policy_bound.cross_roster_fork_safety());
        let vault = EncryptedStateVault::open_existing(first.state_dir())?;
        let trust = vault.read_primary_trust()?;
        let state = optional_policy_state(&trust)?.context("installed policy state is missing")?;
        assert_eq!(state.epoch(), 1);
        assert_eq!(
            AccountRootState::load(first.account_root_dir())?.account_id(),
            state.account_id()
        );
        Ok(())
    }
}
