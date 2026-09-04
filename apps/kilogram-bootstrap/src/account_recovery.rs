use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use kilogram_identity::{
    AccountRecoveryPhrase, AccountRootRecoveryCheckpointState, AccountRootRecoveryPackage,
    AccountRootRecoveryWitness, AccountRootState, MAX_ACCOUNT_ROOT_RECOVERY_PACKAGE_BYTES,
    MAX_ACCOUNT_ROOT_RECOVERY_WITNESS_BYTES,
};
use serde::Serialize;
use tempfile::NamedTempFile;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AccountRecoveryReport {
    status: &'static str,
    account_id: String,
    authority_revision: u64,
    device_count: usize,
    conversation_membership_count: usize,
    package_id: String,
    package_file: PathBuf,
    witness_file: PathBuf,
    account_root_dir: Option<PathBuf>,
    root_key_protection: Option<String>,
    freshness_scope: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AccountRecoveryStatusReport {
    status: &'static str,
    checkpoint_state: &'static str,
    account_id: String,
    authority_revision: u64,
    device_count: usize,
    conversation_membership_count: usize,
    current_package_id: String,
    recorded_package_id: Option<String>,
    recorded_authority_revision: Option<u64>,
    account_root_dir: PathBuf,
    lifecycle_scope: &'static str,
}

pub fn account_root_status(
    account_root_dir: impl AsRef<Path>,
) -> Result<AccountRecoveryStatusReport> {
    let account_root_dir = canonical_account_root(account_root_dir.as_ref())?;
    let root = AccountRootState::load(&account_root_dir).context("load Account Root")?;
    let checkpoint = root
        .recovery_checkpoint_status()
        .context("calculate Account Root recovery checkpoint status")?;
    let status = match checkpoint.state() {
        AccountRootRecoveryCheckpointState::Current => "account-root-recovery-current",
        AccountRootRecoveryCheckpointState::UpdateRequired => {
            "account-root-recovery-update-required"
        }
    };
    Ok(AccountRecoveryStatusReport {
        status,
        checkpoint_state: checkpoint.state().as_str(),
        account_id: checkpoint.account_id().to_string(),
        authority_revision: checkpoint.authority_revision(),
        device_count: checkpoint.device_count(),
        conversation_membership_count: checkpoint.conversation_membership_count(),
        current_package_id: encode_hex(checkpoint.current_package_id()),
        recorded_package_id: checkpoint
            .recorded_package_id()
            .map(|package_id| encode_hex(package_id)),
        recorded_authority_revision: checkpoint.recorded_authority_revision(),
        account_root_dir,
        lifecycle_scope: "local-exact-export-receipt-not-global-freshness-proof",
    })
}

pub fn export_account_root(
    account_root_dir: impl AsRef<Path>,
    package_file: impl AsRef<Path>,
    witness_file: impl AsRef<Path>,
) -> Result<AccountRecoveryReport> {
    let account_root_dir = canonical_account_root(account_root_dir.as_ref())?;
    let package_file = resolve_new_external_file(package_file.as_ref(), &account_root_dir)?;
    let witness_file = resolve_new_external_file(witness_file.as_ref(), &account_root_dir)?;
    ensure!(
        package_file != witness_file,
        "recovery package and witness paths must be different"
    );

    let root = AccountRootState::load(&account_root_dir).context("load Account Root")?;
    let (package, witness) = root
        .export_recovery()
        .context("capture Account Root recovery checkpoint")?;
    let package_bytes = package.encode().context("encode recovery package")?;
    let witness_bytes = witness.encode().context("encode recovery witness")?;
    write_new_pair(&package_file, &package_bytes, &witness_file, &witness_bytes)?;
    if let Err(error) = root.record_exported_recovery_checkpoint(&package, &witness) {
        let mut cleanup_failures = Vec::new();
        for path in [&package_file, &witness_file] {
            if let Err(cleanup) = fs::remove_file(path) {
                cleanup_failures.push(format!("{}: {cleanup}", path.display()));
            }
        }
        let detail = if cleanup_failures.is_empty() {
            "external package and witness were removed".to_owned()
        } else {
            format!(
                "external cleanup also failed for {}",
                cleanup_failures.join(", ")
            )
        };
        return Err(error).context(format!(
            "record exported Account Root recovery checkpoint; {detail}"
        ));
    }
    report(
        "account-root-recovery-exported",
        &package,
        &witness,
        package_file,
        witness_file,
        None,
        None,
    )
}

fn canonical_account_root(path: &Path) -> Result<PathBuf> {
    let resolved = fs::canonicalize(path)
        .with_context(|| format!("resolve Account Root directory {}", path.display()))?;
    ensure!(
        resolved.is_dir(),
        "Account Root path is not a directory: {}",
        resolved.display()
    );
    Ok(resolved)
}

pub fn inspect_account_root(
    package_file: impl AsRef<Path>,
    witness_file: impl AsRef<Path>,
) -> Result<AccountRecoveryReport> {
    let package_file = canonical_regular_artifact(
        package_file.as_ref(),
        MAX_ACCOUNT_ROOT_RECOVERY_PACKAGE_BYTES,
        "Account Root recovery package",
    )?;
    let witness_file = canonical_regular_artifact(
        witness_file.as_ref(),
        MAX_ACCOUNT_ROOT_RECOVERY_WITNESS_BYTES,
        "Account Root recovery witness",
    )?;
    let package = read_package(&package_file)?;
    let witness = read_witness(&witness_file)?;
    witness
        .verify_package(&package)
        .context("verify recovery witness against exact package")?;
    report(
        "account-root-recovery-verified",
        &package,
        &witness,
        package_file,
        witness_file,
        None,
        None,
    )
}

pub fn restore_account_root(
    account_root_dir: impl AsRef<Path>,
    package_file: impl AsRef<Path>,
    witness_file: impl AsRef<Path>,
    phrase: &AccountRecoveryPhrase,
    expected_package_id: &str,
    expected_authority_revision: u64,
) -> Result<AccountRecoveryReport> {
    let package_file = canonical_regular_artifact(
        package_file.as_ref(),
        MAX_ACCOUNT_ROOT_RECOVERY_PACKAGE_BYTES,
        "Account Root recovery package",
    )?;
    let witness_file = canonical_regular_artifact(
        witness_file.as_ref(),
        MAX_ACCOUNT_ROOT_RECOVERY_WITNESS_BYTES,
        "Account Root recovery witness",
    )?;
    let package = read_package(&package_file)?;
    let witness = read_witness(&witness_file)?;
    ensure!(
        encode_hex(
            &package
                .package_id()
                .context("calculate recovery package ID")?
        ) == expected_package_id,
        "recovery package changed after inspection"
    );
    ensure!(
        package.authority_revision() == expected_authority_revision,
        "recovery authority revision changed after inspection"
    );
    let root = AccountRootState::recover(account_root_dir.as_ref(), phrase, &package, &witness)
        .context("restore Account Root from authenticated authority checkpoint")?;
    let account_root_dir = fs::canonicalize(account_root_dir.as_ref()).with_context(|| {
        format!(
            "resolve restored Account Root directory {}",
            account_root_dir.as_ref().display()
        )
    })?;
    report(
        "account-root-recovery-restored",
        &package,
        &witness,
        package_file,
        witness_file,
        Some(account_root_dir),
        Some(root.key_protection().as_str().to_owned()),
    )
}

fn read_package(path: &Path) -> Result<AccountRootRecoveryPackage> {
    let bytes = read_bounded_regular_file(
        path,
        MAX_ACCOUNT_ROOT_RECOVERY_PACKAGE_BYTES,
        "Account Root recovery package",
    )?;
    AccountRootRecoveryPackage::decode_and_verify(&bytes)
        .context("decode and authenticate Account Root recovery package")
}

fn read_witness(path: &Path) -> Result<AccountRootRecoveryWitness> {
    let bytes = read_bounded_regular_file(
        path,
        MAX_ACCOUNT_ROOT_RECOVERY_WITNESS_BYTES,
        "Account Root recovery witness",
    )?;
    AccountRootRecoveryWitness::decode_and_verify(&bytes)
        .context("decode and authenticate Account Root recovery witness")
}

fn read_bounded_regular_file(path: &Path, maximum: usize, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("read {label} metadata at {}", path.display()))?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "{label} must not be a symlink"
    );
    ensure!(metadata.is_file(), "{label} is not a regular file");
    ensure!(
        metadata.len() <= maximum as u64,
        "{label} is too large: {} bytes; maximum is {maximum}",
        metadata.len()
    );
    fs::read(path).with_context(|| format!("read {label} at {}", path.display()))
}

fn canonical_regular_artifact(path: &Path, maximum: usize, label: &str) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("read {label} metadata at {}", path.display()))?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "{label} must not be a symlink"
    );
    ensure!(metadata.is_file(), "{label} is not a regular file");
    ensure!(
        metadata.len() <= maximum as u64,
        "{label} is too large: {} bytes; maximum is {maximum}",
        metadata.len()
    );
    fs::canonicalize(path).with_context(|| format!("resolve {label} {}", path.display()))
}

fn resolve_new_external_file(requested: &Path, account_root_dir: &Path) -> Result<PathBuf> {
    ensure!(
        !requested.as_os_str().is_empty(),
        "recovery output path is empty"
    );
    let lexical = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        std::env::current_dir()
            .context("read current directory for recovery export")?
            .join(requested)
    };
    ensure!(
        !lexical.exists(),
        "recovery output already exists: {}",
        lexical.display()
    );
    let name = lexical
        .file_name()
        .context("recovery output path has no final component")?;
    let parent = lexical
        .parent()
        .context("recovery output path has no parent")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("create recovery output parent {}", parent.display()))?;
    let parent = fs::canonicalize(parent)
        .with_context(|| format!("resolve recovery output parent {}", parent.display()))?;
    let resolved = parent.join(name);
    ensure!(
        !resolved.starts_with(account_root_dir),
        "recovery artifacts must be stored outside the Account Root directory"
    );
    ensure!(
        !resolved.exists(),
        "recovery output already exists: {}",
        resolved.display()
    );
    Ok(resolved)
}

fn write_new_pair(
    first_path: &Path,
    first_bytes: &[u8],
    second_path: &Path,
    second_bytes: &[u8],
) -> Result<()> {
    let first = prepare_temporary(first_path, first_bytes)?;
    let second = prepare_temporary(second_path, second_bytes)?;
    ensure!(
        !first_path.exists() && !second_path.exists(),
        "recovery output appeared while preparing export"
    );
    first
        .persist_noclobber(first_path)
        .map_err(|error| error.error)
        .with_context(|| format!("publish recovery package {}", first_path.display()))?;
    if let Err(error) = second.persist_noclobber(second_path) {
        let publish_error = error.error;
        fs::remove_file(first_path).with_context(|| {
            format!(
                "remove incomplete recovery package {} after witness publication failed",
                first_path.display()
            )
        })?;
        return Err(publish_error)
            .with_context(|| format!("publish recovery witness {}", second_path.display()));
    }
    Ok(())
}

fn prepare_temporary(path: &Path, bytes: &[u8]) -> Result<NamedTempFile> {
    let parent = path.parent().context("recovery output has no parent")?;
    let mut temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("create temporary recovery file in {}", parent.display()))?;
    temporary
        .write_all(bytes)
        .with_context(|| format!("write temporary recovery file for {}", path.display()))?;
    temporary
        .as_file()
        .sync_all()
        .with_context(|| format!("sync temporary recovery file for {}", path.display()))?;
    Ok(temporary)
}

#[allow(clippy::too_many_arguments)]
fn report(
    status: &'static str,
    package: &AccountRootRecoveryPackage,
    witness: &AccountRootRecoveryWitness,
    package_file: PathBuf,
    witness_file: PathBuf,
    account_root_dir: Option<PathBuf>,
    root_key_protection: Option<String>,
) -> Result<AccountRecoveryReport> {
    witness
        .verify_package(package)
        .context("verify recovery report checkpoint")?;
    Ok(AccountRecoveryReport {
        status,
        account_id: package.account_id().to_string(),
        authority_revision: package.authority_revision(),
        device_count: package.device_count(),
        conversation_membership_count: package.conversation_membership_count(),
        package_id: encode_hex(witness.package_id()),
        package_file,
        witness_file,
        account_root_dir,
        root_key_protection,
        freshness_scope: "exact-independent-witness-not-global-monotonic-service",
    })
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
        AccountId, ConversationScopeId, DeviceCapability, DeviceState, EncryptionPublicKey,
    };

    use super::*;

    #[test]
    fn round_trip_preserves_authority_and_rejects_stale_or_wrong_inputs()
    -> Result<(), Box<dyn Error>> {
        let parent = tempfile::tempdir()?;
        let workspace = parent.path().join("account");
        let created = crate::create_account(&workspace)?;
        let phrase = AccountRecoveryPhrase::parse(created.recovery_phrase())?;
        let root = AccountRootState::load(created.account_root_dir())?;
        assert_eq!(
            account_root_status(created.account_root_dir())?.checkpoint_state,
            "update-required"
        );

        let second = DeviceState::load_or_create(parent.path().join("second-device"))?;
        root.enroll_device(
            second.identity().device_id(),
            second.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let conversation_id = ConversationScopeId::from_bytes([7_u8; 32]);
        let peer = AccountId::from_bytes([9_u8; 32]);
        root.create_conversation_membership(conversation_id, &[peer])?;
        root.add_conversation_members(conversation_id, &[AccountId::from_bytes([10_u8; 32])])?;

        let old_package = parent.path().join("old.karp");
        let old_witness = parent.path().join("old.karw");
        let exported = export_account_root(created.account_root_dir(), &old_package, &old_witness)?;
        let current_status = account_root_status(created.account_root_dir())?;
        assert_eq!(current_status.checkpoint_state, "current");
        assert_eq!(current_status.current_package_id, exported.package_id);
        assert_eq!(exported.device_count, 2);
        assert_eq!(exported.conversation_membership_count, 1);
        assert!(
            !fs::read(&old_package)?
                .windows(created.recovery_phrase().len())
                .any(|window| window == created.recovery_phrase().as_bytes())
        );

        let restored_path = parent.path().join("restored-root");
        let restored_report = restore_account_root(
            &restored_path,
            &old_package,
            &old_witness,
            &phrase,
            &exported.package_id,
            exported.authority_revision,
        )?;
        assert_eq!(restored_report.status, "account-root-recovery-restored");
        let restored = AccountRootState::load(&restored_path)?;
        assert_eq!(
            account_root_status(&restored_path)?.checkpoint_state,
            "current"
        );
        assert_eq!(restored.account_id(), root.account_id());
        assert_eq!(restored.authority_snapshot()?, root.authority_snapshot()?);
        assert_eq!(
            restored.published_device_list()?,
            root.published_device_list()?
        );
        assert_eq!(
            restored.load_root_conversation_membership(conversation_id)?,
            root.load_root_conversation_membership(conversation_id)?
        );

        root.add_conversation_members(conversation_id, &[AccountId::from_bytes([11_u8; 32])])?;
        let membership_due = account_root_status(created.account_root_dir())?;
        assert_eq!(membership_due.checkpoint_state, "update-required");
        assert_eq!(
            membership_due.authority_revision,
            exported.authority_revision
        );

        let third_key = EncryptionPublicKey::from_bytes([13_u8; 32])?;
        let (_, advanced_list) = restored.enroll_device(
            kilogram_identity::DeviceId::from_bytes([12_u8; 32]),
            third_key,
            &DeviceCapability::MESSAGING,
        )?;
        assert!(advanced_list.revision() > exported.authority_revision);
        assert_eq!(
            account_root_status(&restored_path)?.checkpoint_state,
            "update-required"
        );

        let wrong_workspace = parent.path().join("wrong-account");
        let wrong = crate::create_account(&wrong_workspace)?;
        let wrong_phrase = AccountRecoveryPhrase::parse(wrong.recovery_phrase())?;
        let wrong_target = parent.path().join("wrong-target");
        assert!(
            restore_account_root(
                &wrong_target,
                &old_package,
                &old_witness,
                &wrong_phrase,
                &exported.package_id,
                exported.authority_revision,
            )
            .is_err()
        );
        assert!(!wrong_target.exists());

        let fourth = DeviceState::load_or_create(parent.path().join("fourth-device"))?;
        root.enroll_device(
            fourth.identity().device_id(),
            fourth.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        assert_eq!(
            account_root_status(created.account_root_dir())?.checkpoint_state,
            "update-required"
        );
        let new_package = parent.path().join("new.karp");
        let new_witness = parent.path().join("new.karw");
        let new_exported =
            export_account_root(created.account_root_dir(), &new_package, &new_witness)?;
        assert_eq!(
            account_root_status(created.account_root_dir())?.checkpoint_state,
            "current"
        );
        let stale_target = parent.path().join("stale-target");
        assert!(
            restore_account_root(
                &stale_target,
                &old_package,
                &new_witness,
                &phrase,
                &new_exported.package_id,
                new_exported.authority_revision,
            )
            .is_err()
        );
        assert!(!stale_target.exists());

        assert!(
            restore_account_root(
                &restored_path,
                &new_package,
                &new_witness,
                &phrase,
                &new_exported.package_id,
                new_exported.authority_revision,
            )
            .is_err()
        );
        fs::copy(&new_package, &old_package)?;
        fs::copy(&new_witness, &old_witness)?;
        let changed_target = parent.path().join("changed-target");
        let changed_result = restore_account_root(
            &changed_target,
            &old_package,
            &old_witness,
            &phrase,
            &exported.package_id,
            exported.authority_revision,
        );
        assert!(changed_result.is_err());
        let changed_error = changed_result
            .err()
            .context("same-path package replacement must be rejected")?;
        assert!(format!("{changed_error:#}").contains("recovery package changed after inspection"));
        assert!(!changed_target.exists());

        let revision_target = parent.path().join("revision-target");
        let revision_result = restore_account_root(
            &revision_target,
            &old_package,
            &old_witness,
            &phrase,
            &new_exported.package_id,
            exported.authority_revision,
        );
        assert!(revision_result.is_err());
        let revision_error = revision_result
            .err()
            .context("authority revision mismatch must be rejected")?;
        assert!(
            format!("{revision_error:#}")
                .contains("recovery authority revision changed after inspection")
        );
        assert!(!revision_target.exists());
        Ok(())
    }

    #[test]
    fn rejects_tampering_and_outputs_inside_root() -> Result<(), Box<dyn Error>> {
        let parent = tempfile::tempdir()?;
        let workspace = parent.path().join("account");
        let created = crate::create_account(&workspace)?;
        assert!(
            export_account_root(
                created.account_root_dir(),
                created.account_root_dir().join("backup.karp"),
                parent.path().join("backup.karw")
            )
            .is_err()
        );

        let package = parent.path().join("backup.karp");
        let witness = parent.path().join("backup.karw");
        export_account_root(created.account_root_dir(), &package, &witness)?;
        let mut tampered = fs::read(&package)?;
        let index = tampered.len() / 2;
        tampered[index] ^= 1;
        fs::write(&package, tampered)?;
        assert!(inspect_account_root(&package, &witness).is_err());
        Ok(())
    }
}
