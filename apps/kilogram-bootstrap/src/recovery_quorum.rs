use std::{
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, ensure};
use kilogram_identity::{
    AccountAuthoritySnapshot, AccountRootRecoveryApproval, AccountRootRecoveryApprovalRequest,
    AccountRootRecoveryPackage, AccountRootRecoveryWitness, ConversationMembershipSnapshot,
    DeviceCertificate, DeviceState, MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_BYTES,
    MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_REQUEST_BYTES, MAX_ACCOUNT_ROOT_RECOVERY_PACKAGE_BYTES,
    MAX_ACCOUNT_ROOT_RECOVERY_WITNESS_BYTES, verify_account_root_recovery_quorum,
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
const APPROVAL_HEAD_PATH: &str = "recovery-approval/latest.approval";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryQuorumRequestOutput {
    status: &'static str,
    account_id: String,
    request_id: String,
    package_id: String,
    authority_revision: u64,
    recovery_roster_digest: String,
    roster_count: usize,
    required_approvals: usize,
    issued_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
    request_file: PathBuf,
    freshness_scope: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryQuorumApprovalOutput {
    status: &'static str,
    account_id: String,
    request_id: String,
    package_id: String,
    authority_revision: u64,
    recovery_roster_digest: String,
    approver_device_id: String,
    previous_approval_head_digest: String,
    approval_id: String,
    approval_file: PathBuf,
    approval_head_source: &'static str,
    approval_head_committed_before_publish: bool,
    reused_committed_approval: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryQuorumVerifyOutput {
    status: &'static str,
    account_id: String,
    request_id: String,
    package_id: String,
    authority_revision: u64,
    recovery_roster_digest: String,
    roster_count: usize,
    observed_approvals: usize,
    required_approvals: usize,
    majority_satisfied: bool,
    freshness_claim: &'static str,
    cross_roster_fork_safety: bool,
}

pub fn create_request(
    package_file: impl AsRef<Path>,
    witness_file: impl AsRef<Path>,
    request_file: impl AsRef<Path>,
    validity_seconds: u64,
) -> Result<RecoveryQuorumRequestOutput> {
    let package = AccountRootRecoveryPackage::decode_and_verify(&read_bounded_regular_file(
        package_file.as_ref(),
        MAX_ACCOUNT_ROOT_RECOVERY_PACKAGE_BYTES,
        "Account Root recovery package",
    )?)
    .context("decode and authenticate Account Root recovery package")?;
    let witness = AccountRootRecoveryWitness::decode_and_verify(&read_bounded_regular_file(
        witness_file.as_ref(),
        MAX_ACCOUNT_ROOT_RECOVERY_WITNESS_BYTES,
        "Account Root recovery witness",
    )?)
    .context("decode and authenticate Account Root recovery witness")?;
    witness
        .verify_package(&package)
        .context("verify witness against exact recovery package")?;
    let now = unix_time_now()?;
    let request =
        AccountRootRecoveryApprovalRequest::issue(package, witness, now, validity_seconds)
            .context("issue recovery approval request")?;
    write_new(request_file.as_ref(), &request.encode()?)?;
    request_output(&request, absolute_existing_path(request_file.as_ref())?)
}

pub fn approve_request(
    state_dir: impl AsRef<Path>,
    request_file: impl AsRef<Path>,
    approval_file: impl AsRef<Path>,
) -> Result<RecoveryQuorumApprovalOutput> {
    let state_dir = fs::canonicalize(state_dir.as_ref()).with_context(|| {
        format!(
            "resolve approving device state {}",
            state_dir.as_ref().display()
        )
    })?;
    ensure!(
        state_dir.is_dir(),
        "approving device state is not a directory"
    );
    let _lock = StateDirectoryLock::acquire(&state_dir).context("lock approving device state")?;
    ensure!(
        EncryptedStateVault::is_initialized(&state_dir)?,
        "current-device recovery approval requires an initialized DB-primary vault"
    );
    let vault = EncryptedStateVault::open_existing(&state_dir)
        .context("open approving device DB-primary vault")?;
    vault
        .recover_primary_shadow()
        .context("recover interrupted approval trust shadow")?;
    vault
        .recover_pending_dual_write()
        .context("recover interrupted approval vault mirror")?;

    let now = unix_time_now()?;
    let request = AccountRootRecoveryApprovalRequest::decode_and_verify(
        &read_bounded_regular_file(
            request_file.as_ref(),
            MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_REQUEST_BYTES,
            "recovery approval request",
        )?,
        now,
    )
    .context("decode and verify current recovery approval request")?;
    let identity = vault
        .read_primary_device_identity()
        .context("read authenticated approving device identity")?;
    let device = DeviceState::from_secret_material(
        &state_dir,
        *identity.signing_secret(),
        *identity.encryption_secret(),
    );
    let trust = vault
        .read_primary_trust()
        .context("read authenticated approving-device trust state")?;
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
    let mut memberships = Vec::new();
    for record in trust.records() {
        if record.kind() == StateRecordKind::Trust
            && record.relative_path().starts_with(MEMBERSHIP_PREFIX)
        {
            let membership = ConversationMembershipSnapshot::decode_and_verify(record.content())
                .with_context(|| {
                    format!("decode DB-primary membership {}", record.relative_path())
                })?;
            if membership.owner_account_id() == request.account_id() {
                memberships.push(membership);
            }
        }
    }
    request
        .package()
        .verify_dominates_device_state(&certificate, &authority, &memberships)
        .context("reject rollback, equivocation, roster mismatch, or revoked approver")?;

    let existing_head = trust
        .records()
        .iter()
        .find(|record| record.relative_path() == APPROVAL_HEAD_PATH)
        .map(|record| AccountRootRecoveryApproval::decode_and_verify(record.content()))
        .transpose()
        .context("decode DB-primary recovery approval head")?;
    if let Some(existing) = existing_head.as_ref()
        && existing.request_id() == &request.request_id()?
    {
        existing
            .verify_for_request(&request, now)
            .context("verify idempotent committed recovery approval")?;
        ensure!(
            existing.approver_device_id() == device.identity().device_id(),
            "committed recovery approval head belongs to a different device"
        );
        let encoded = existing.encode()?;
        write_idempotent(approval_file.as_ref(), &encoded)?;
        return approval_output(
            existing,
            absolute_existing_path(approval_file.as_ref())?,
            true,
        );
    }

    let previous_approval_head_digest = match existing_head.as_ref() {
        Some(existing) => {
            ensure!(
                existing.account_id() == request.account_id(),
                "recovery approval head belongs to a different account"
            );
            if existing.recovery_roster_digest() != &request.package().recovery_roster_digest()? {
                return Err(anyhow!(
                    kilogram_identity::IdentityError::AccountRootRecoveryApprovalRosterChanged
                ));
            }
            ensure!(
                request.package().authority_revision() >= existing.authority_revision(),
                "recovery candidate authority revision is behind the committed approval head"
            );
            existing.approval_id()?
        }
        None => [0_u8; 32],
    };
    let approval = AccountRootRecoveryApproval::issue(
        device.identity(),
        &request,
        previous_approval_head_digest,
        now,
    )
    .context("sign recovery approval")?;
    let encoded = approval.encode()?;

    vault
        .begin_dual_write()
        .context("prepare approval vault mirror intent")?;
    let commit_result =
        commit_approval_head_primary(&state_dir, &vault, &device, request.package(), &encoded);
    let mirror_result = vault
        .finish_dual_write()
        .context("complete approval vault mirror");
    match (commit_result, mirror_result) {
        (Ok(()), Ok(_)) => {}
        (Err(error), Ok(_)) => return Err(error),
        (Ok(()), Err(error)) => return Err(error),
        (Err(error), Err(mirror)) => {
            return Err(error.context(format!(
                "approval commit failed and vault mirror also failed: {mirror:#}"
            )));
        }
    }

    // The public signature is deliberately written only after the DB-primary
    // approval head and retained shadow have both committed.
    write_idempotent(approval_file.as_ref(), &encoded)?;
    approval_output(
        &approval,
        absolute_existing_path(approval_file.as_ref())?,
        false,
    )
}

pub fn verify_request(
    request_file: impl AsRef<Path>,
    approval_files: &[PathBuf],
    require_majority: bool,
) -> Result<RecoveryQuorumVerifyOutput> {
    let now = unix_time_now()?;
    let request = AccountRootRecoveryApprovalRequest::decode_and_verify(
        &read_bounded_regular_file(
            request_file.as_ref(),
            MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_REQUEST_BYTES,
            "recovery approval request",
        )?,
        now,
    )?;
    let mut approvals = Vec::with_capacity(approval_files.len());
    for path in approval_files {
        approvals.push(AccountRootRecoveryApproval::decode_and_verify(
            &read_bounded_regular_file(
                path,
                MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_BYTES,
                "recovery device approval",
            )?,
        )?);
    }
    let report = verify_account_root_recovery_quorum(&request, &approvals, now)
        .context("verify exact recovery device quorum")?;
    if require_majority {
        report
            .require_majority()
            .context("require strict current-device majority")?;
    }
    Ok(RecoveryQuorumVerifyOutput {
        status: "account-recovery-quorum-verified",
        account_id: report.account_id().to_string(),
        request_id: encode_hex(report.request_id()),
        package_id: encode_hex(report.package_id()),
        authority_revision: request.package().authority_revision(),
        recovery_roster_digest: encode_hex(report.recovery_roster_digest()),
        roster_count: report.roster_count(),
        observed_approvals: report.observed_approvals(),
        required_approvals: report.required_approvals(),
        majority_satisfied: report.require_majority().is_ok(),
        freshness_claim: report.claim().as_str(),
        cross_roster_fork_safety: false,
    })
}

fn commit_approval_head_primary(
    state_dir: &Path,
    vault: &EncryptedStateVault,
    device: &DeviceState,
    package: &AccountRootRecoveryPackage,
    approval_bytes: &[u8],
) -> Result<()> {
    let trust = vault
        .read_primary_trust()
        .context("re-read approval trust baseline")?;
    let mut transaction = StateTransaction::begin(state_dir)
        .context("prepare crash-consistent approval transaction")?;
    if let Err(error) = transaction.prepare_trust_workspace(&trust) {
        let _ = transaction.rollback();
        return Err(error).context("prepare DB-primary approval trust workspace");
    }
    let operation = device
        .install_own_authority_snapshot(package.authority_snapshot())
        .context("advance approving device authority high-water")
        .and_then(|_| {
            for membership in package.conversation_memberships() {
                device
                    .install_conversation_membership(membership)
                    .context("advance approving device membership high-water")?;
            }
            write_approval_head(&state_dir.join(APPROVAL_HEAD_PATH), approval_bytes)
        });
    if let Err(error) = operation {
        return match transaction.rollback() {
            Ok(()) => Err(error),
            Err(rollback) => Err(error.context(format!(
                "approval staging failed and trust rollback also failed: {rollback}"
            ))),
        };
    }
    if let Err(error) = vault.commit_primary_transaction(&transaction) {
        return match transaction.rollback() {
            Ok(()) => Err(error).context("commit recovery approval head to DB-primary vault"),
            Err(rollback) => Err(anyhow::Error::new(error).context(format!(
                "vault approval commit failed and trust rollback also failed: {rollback}"
            ))),
        };
    }
    transaction
        .commit()
        .context("commit retained recovery approval shadow")?;
    vault
        .confirm_primary_shadow()
        .context("confirm retained recovery approval shadow")?;
    Ok(())
}

fn write_approval_head(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("approval head has no parent")?;
    fs::create_dir_all(parent).context("create recovery approval trust directory")?;
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .with_context(|| format!("stage recovery approval head {}", path.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
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

fn request_output(
    request: &AccountRootRecoveryApprovalRequest,
    request_file: PathBuf,
) -> Result<RecoveryQuorumRequestOutput> {
    Ok(RecoveryQuorumRequestOutput {
        status: "account-recovery-quorum-request-created",
        account_id: request.account_id().to_string(),
        request_id: encode_hex(&request.request_id()?),
        package_id: encode_hex(&request.package().package_id()?),
        authority_revision: request.package().authority_revision(),
        recovery_roster_digest: encode_hex(&request.package().recovery_roster_digest()?),
        roster_count: request.roster_count(),
        required_approvals: request.required_approvals(),
        issued_at_unix_seconds: request.issued_at_unix_seconds(),
        expires_at_unix_seconds: request.expires_at_unix_seconds(),
        request_file,
        freshness_scope: "exact-roster-current-device-observation",
    })
}

fn approval_output(
    approval: &AccountRootRecoveryApproval,
    approval_file: PathBuf,
    reused_committed_approval: bool,
) -> Result<RecoveryQuorumApprovalOutput> {
    Ok(RecoveryQuorumApprovalOutput {
        status: "account-recovery-quorum-approved",
        account_id: approval.account_id().to_string(),
        request_id: encode_hex(approval.request_id()),
        package_id: encode_hex(approval.package_id()),
        authority_revision: approval.authority_revision(),
        recovery_roster_digest: encode_hex(approval.recovery_roster_digest()),
        approver_device_id: approval.approver_device_id().to_string(),
        previous_approval_head_digest: encode_hex(approval.previous_approval_head_digest()),
        approval_id: encode_hex(&approval.approval_id()?),
        approval_file,
        approval_head_source: "db-primary",
        approval_head_committed_before_publish: true,
        reused_committed_approval,
    })
}

fn read_bounded_regular_file(path: &Path, maximum: usize, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("inspect {label} {}", path.display()))?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "{label} must not be a symbolic link"
    );
    ensure!(metadata.is_file(), "{label} must be a regular file");
    ensure!(
        metadata.len() <= maximum as u64,
        "{label} is too large: {} bytes; maximum is {maximum}",
        metadata.len()
    );
    fs::read(path).with_context(|| format!("read {label} {}", path.display()))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let absolute = resolve_output_path(path)?;
    let parent = absolute.parent().context("artifact output has no parent")?;
    let mut temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("create temporary artifact in {}", parent.display()))?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(&absolute)
        .map_err(|error| error.error)
        .with_context(|| format!("publish new artifact {}", absolute.display()))?;
    Ok(())
}

fn write_idempotent(path: &Path, bytes: &[u8]) -> Result<()> {
    let absolute = resolve_output_path(path)?;
    match fs::symlink_metadata(&absolute) {
        Ok(metadata) => {
            ensure!(
                !metadata.file_type().is_symlink() && metadata.is_file(),
                "recovery approval output exists but is not a regular file: {}",
                absolute.display()
            );
            ensure!(
                fs::read(&absolute)? == bytes,
                "recovery approval output exists with different content: {}",
                absolute.display()
            );
            return Ok(());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).with_context(|| format!("read {}", absolute.display())),
    }
    let parent = absolute.parent().context("approval output has no parent")?;
    let mut temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("create temporary approval in {}", parent.display()))?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    match temporary.persist_noclobber(&absolute) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            ensure!(
                read_bounded_regular_file(
                    &absolute,
                    MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_BYTES,
                    "raced recovery approval"
                )? == bytes,
                "another process published a different recovery approval at {}",
                absolute.display()
            );
            Ok(())
        }
        Err(error) => {
            Err(error.error).with_context(|| format!("publish approval {}", absolute.display()))
        }
    }
}

fn resolve_output_path(path: &Path) -> Result<PathBuf> {
    ensure!(
        !path.as_os_str().is_empty(),
        "artifact output path is empty"
    );
    let lexical = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let parent = lexical.parent().context("artifact output has no parent")?;
    ensure!(parent.is_dir(), "artifact output parent does not exist");
    let file_name = lexical
        .file_name()
        .context("artifact output has no file name")?;
    Ok(fs::canonicalize(parent)?.join(file_name))
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

#[cfg(test)]
mod tests {
    use std::error::Error;

    use kilogram_identity::{
        AccountId, AccountRecoveryPhrase, AccountRootState, ConversationScopeId, DeviceCapability,
        DeviceState,
    };

    use super::*;

    #[test]
    fn db_primary_head_precedes_public_approval_and_retries_idempotently()
    -> Result<(), Box<dyn Error>> {
        let parent = tempfile::tempdir()?;
        let workspace = parent.path().join("account");
        let created = crate::create_account(&workspace)?;
        let package_file = parent.path().join("account.karp");
        let witness_file = parent.path().join("account.karw");
        crate::account_recovery::export_account_root(
            created.account_root_dir(),
            &package_file,
            &witness_file,
        )?;
        let request_file = parent.path().join("request.karq");
        let request = create_request(&package_file, &witness_file, &request_file, 600)?;
        assert_eq!(request.roster_count, 1);
        assert_eq!(request.required_approvals, 1);

        let blocked_output = parent.path().join("blocked-output");
        fs::create_dir(&blocked_output)?;
        assert!(approve_request(created.state_dir(), &request_file, &blocked_output).is_err());
        let vault = EncryptedStateVault::open_existing(created.state_dir())?;
        assert!(
            vault
                .read_primary_trust()?
                .records()
                .iter()
                .any(|record| record.relative_path() == APPROVAL_HEAD_PATH)
        );
        drop(vault);

        let approval_file = parent.path().join("device.kara");
        let approval = approve_request(created.state_dir(), &request_file, &approval_file)?;
        assert!(approval.reused_committed_approval);
        assert!(approval.approval_head_committed_before_publish);
        let verified = verify_request(&request_file, &[approval_file], true)?;
        assert_eq!(verified.freshness_claim, "current-device-majority-observed");
        assert!(verified.majority_satisfied);
        assert!(!verified.cross_roster_fork_safety);
        let vault = EncryptedStateVault::open_existing(created.state_dir())?;
        vault.verify()?;
        Ok(())
    }

    #[test]
    fn verifier_labels_offline_fallback_without_inventing_freshness() -> Result<(), Box<dyn Error>>
    {
        let parent = tempfile::tempdir()?;
        let workspace = parent.path().join("account");
        let created = crate::create_account(&workspace)?;
        let package_file = parent.path().join("account.karp");
        let witness_file = parent.path().join("account.karw");
        crate::account_recovery::export_account_root(
            created.account_root_dir(),
            &package_file,
            &witness_file,
        )?;
        let request_file = parent.path().join("request.karq");
        create_request(&package_file, &witness_file, &request_file, 600)?;
        let report = verify_request(&request_file, &[], false)?;
        assert_eq!(report.freshness_claim, "artifact-integrity-only");
        assert!(!report.majority_satisfied);
        assert!(verify_request(&request_file, &[], true).is_err());
        Ok(())
    }

    #[test]
    fn approval_refuses_stale_membership_and_same_revision_fork() -> Result<(), Box<dyn Error>> {
        let parent = tempfile::tempdir()?;
        let workspace = parent.path().join("account");
        let created = crate::create_account(&workspace)?;
        let phrase = AccountRecoveryPhrase::parse(created.recovery_phrase())?;

        let base_package = parent.path().join("base.karp");
        let base_witness = parent.path().join("base.karw");
        crate::account_recovery::export_account_root(
            created.account_root_dir(),
            &base_package,
            &base_witness,
        )?;
        let fork_root_path = parent.path().join("fork-root");
        let decoded_package =
            AccountRootRecoveryPackage::decode_and_verify(&fs::read(&base_package)?)?;
        let decoded_witness =
            AccountRootRecoveryWitness::decode_and_verify(&fs::read(&base_witness)?)?;
        AccountRootState::recover(&fork_root_path, &phrase, &decoded_package, &decoded_witness)?;

        let conversation = ConversationScopeId::from_bytes([41_u8; 32]);
        let root = AccountRootState::load(created.account_root_dir())?;
        root.create_conversation_membership(conversation, &[AccountId::from_bytes([51_u8; 32])])?;
        let current_package = parent.path().join("current.karp");
        let current_witness = parent.path().join("current.karw");
        crate::account_recovery::export_account_root(
            created.account_root_dir(),
            &current_package,
            &current_witness,
        )?;
        let current_request = parent.path().join("current.karq");
        create_request(&current_package, &current_witness, &current_request, 600)?;
        approve_request(
            created.state_dir(),
            &current_request,
            parent.path().join("current.kara"),
        )?;

        let stale_request = parent.path().join("stale.karq");
        create_request(&base_package, &base_witness, &stale_request, 600)?;
        assert!(
            approve_request(
                created.state_dir(),
                &stale_request,
                parent.path().join("stale.kara")
            )
            .is_err()
        );

        let fork_root = AccountRootState::load(&fork_root_path)?;
        fork_root
            .create_conversation_membership(conversation, &[AccountId::from_bytes([52_u8; 32])])?;
        let fork_package = parent.path().join("fork.karp");
        let fork_witness = parent.path().join("fork.karw");
        crate::account_recovery::export_account_root(
            &fork_root_path,
            &fork_package,
            &fork_witness,
        )?;
        let fork_request = parent.path().join("fork.karq");
        create_request(&fork_package, &fork_witness, &fork_request, 600)?;
        assert!(
            approve_request(
                created.state_dir(),
                &fork_request,
                parent.path().join("fork.kara")
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn approval_head_freezes_roster_until_joint_transition_exists() -> Result<(), Box<dyn Error>> {
        let parent = tempfile::tempdir()?;
        let workspace = parent.path().join("account");
        let created = crate::create_account(&workspace)?;
        let package_file = parent.path().join("one.karp");
        let witness_file = parent.path().join("one.karw");
        crate::account_recovery::export_account_root(
            created.account_root_dir(),
            &package_file,
            &witness_file,
        )?;
        let request_file = parent.path().join("one.karq");
        create_request(&package_file, &witness_file, &request_file, 600)?;
        approve_request(
            created.state_dir(),
            &request_file,
            parent.path().join("one.kara"),
        )?;

        let root = AccountRootState::load(created.account_root_dir())?;
        let second = DeviceState::load_or_create(parent.path().join("second-device"))?;
        root.enroll_device(
            second.identity().device_id(),
            second.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let expanded_package = parent.path().join("two.karp");
        let expanded_witness = parent.path().join("two.karw");
        crate::account_recovery::export_account_root(
            created.account_root_dir(),
            &expanded_package,
            &expanded_witness,
        )?;
        let expanded_request = parent.path().join("two.karq");
        create_request(&expanded_package, &expanded_witness, &expanded_request, 600)?;
        assert!(
            approve_request(
                created.state_dir(),
                &expanded_request,
                parent.path().join("two.kara")
            )
            .is_err()
        );
        Ok(())
    }
}
