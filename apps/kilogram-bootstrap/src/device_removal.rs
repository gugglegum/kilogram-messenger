use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, ensure};
use kilogram_identity::{
    AccountRootRecoveryPackage, AccountRootRecoveryWitness, AccountRootState, DeviceId,
    DeviceRevocation, MAX_ACCOUNT_ROOT_RECOVERY_PACKAGE_BYTES,
    MAX_ACCOUNT_ROOT_RECOVERY_WITNESS_BYTES, verify_recovery_package_successor,
};
use serde::Serialize;
use tempfile::NamedTempFile;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceRemovalReport {
    status: &'static str,
    account_id: String,
    removed_device_id: String,
    before_authority_revision: u64,
    after_authority_revision: u64,
    before_device_count: usize,
    after_device_count: usize,
    before_package_id: String,
    after_package_id: String,
    before_package_file: PathBuf,
    before_witness_file: PathBuf,
    after_package_file: PathBuf,
    after_witness_file: PathBuf,
    device_list_file: PathBuf,
    revocation_file: PathBuf,
    removal_status: &'static str,
    policy_activation_status: &'static str,
    runtime_peer_directory_status: &'static str,
    ratchet_session_retirement_status: &'static str,
    history_availability_status: &'static str,
}

#[allow(clippy::too_many_arguments)]
pub fn remove_device(
    account_root_dir: impl AsRef<Path>,
    device_id: DeviceId,
    before_package_file: impl AsRef<Path>,
    before_witness_file: impl AsRef<Path>,
    after_package_file: impl AsRef<Path>,
    after_witness_file: impl AsRef<Path>,
    device_list_file: impl AsRef<Path>,
    revocation_file: impl AsRef<Path>,
) -> Result<DeviceRemovalReport> {
    let account_root_dir = canonical_directory(account_root_dir.as_ref(), "Account Root")?;
    let before_package_file = canonical_artifact(
        before_package_file.as_ref(),
        MAX_ACCOUNT_ROOT_RECOVERY_PACKAGE_BYTES,
        "before recovery package",
    )?;
    let before_witness_file = canonical_artifact(
        before_witness_file.as_ref(),
        MAX_ACCOUNT_ROOT_RECOVERY_WITNESS_BYTES,
        "before recovery witness",
    )?;
    let before_package = read_package(&before_package_file)?;
    let before_witness = read_witness(&before_witness_file)?;
    ensure!(
        before_package_file != before_witness_file,
        "before-removal package and witness paths must be different"
    );
    ensure!(
        !before_package_file.starts_with(&account_root_dir)
            && !before_witness_file.starts_with(&account_root_dir),
        "before-removal recovery artifacts must be stored outside the Account Root directory"
    );
    before_witness
        .verify_package(&before_package)
        .context("verify exact before-removal recovery checkpoint")?;
    ensure!(
        before_package
            .device_list()
            .certificate_for(device_id)
            .is_some(),
        "device {device_id} is not active in the exact before-removal checkpoint"
    );
    ensure!(
        before_package.device_count() > 1,
        "cannot remove the last active device from an account"
    );

    let outputs = [
        resolve_external_output(after_package_file.as_ref(), &account_root_dir)?,
        resolve_external_output(after_witness_file.as_ref(), &account_root_dir)?,
        resolve_external_output(device_list_file.as_ref(), &account_root_dir)?,
        resolve_external_output(revocation_file.as_ref(), &account_root_dir)?,
    ];
    for first in 0..outputs.len() {
        for second in first + 1..outputs.len() {
            ensure!(
                outputs[first] != outputs[second],
                "device-removal output paths must be different"
            );
        }
        ensure!(
            outputs[first] != before_package_file && outputs[first] != before_witness_file,
            "before-removal and after-removal artifact paths must be different"
        );
    }
    let after_package_file = outputs[0].clone();
    let after_witness_file = outputs[1].clone();
    let device_list_file = outputs[2].clone();
    let revocation_file = outputs[3].clone();

    let root = AccountRootState::load(&account_root_dir).context("load Account Root")?;
    ensure!(
        root.account_id() == before_package.account_id(),
        "before-removal recovery checkpoint belongs to a different account"
    );
    let current_authority = root.authority_snapshot()?;
    let already_revoked = current_authority
        .revocations()
        .iter()
        .any(|revocation| revocation.device_id() == device_id);
    if !already_revoked {
        ensure!(
            outputs.iter().all(|path| !path.exists()),
            "a device-removal output already exists before the Root operation"
        );
        let (current_package, current_witness) = root
            .export_recovery()
            .context("capture current Root state before device removal")?;
        ensure!(
            current_package == before_package && current_witness == before_witness,
            "Account Root changed after the exact before-removal checkpoint was exported"
        );
    }

    let (revocation, device_list) = root
        .revoke_and_publish_device_list(device_id)
        .context("revoke device and publish refreshed active-device list")?;
    let (after_package, after_witness) = root
        .export_recovery()
        .context("capture exact after-removal recovery checkpoint")?;
    verify_exact_removal(&before_package, &after_package, device_id, &revocation)?;
    after_witness
        .verify_package(&after_package)
        .context("verify exact after-removal recovery checkpoint")?;
    ensure!(
        after_package.device_list() == &device_list,
        "published active-device list differs from the after-removal checkpoint"
    );

    write_idempotent(&revocation_file, &revocation.encode()?)?;
    write_idempotent(&device_list_file, &device_list.encode()?)?;
    write_idempotent(&after_package_file, &after_package.encode()?)?;
    write_idempotent(&after_witness_file, &after_witness.encode()?)?;
    root.record_exported_recovery_checkpoint(&after_package, &after_witness)
        .context("record exact after-removal recovery checkpoint")?;

    Ok(DeviceRemovalReport {
        status: "device-removed",
        account_id: root.account_id().to_string(),
        removed_device_id: device_id.to_string(),
        before_authority_revision: before_package.authority_revision(),
        after_authority_revision: after_package.authority_revision(),
        before_device_count: before_package.device_count(),
        after_device_count: after_package.device_count(),
        before_package_id: encode_hex(&before_package.package_id()?),
        after_package_id: encode_hex(&after_package.package_id()?),
        before_package_file,
        before_witness_file,
        after_package_file,
        after_witness_file,
        device_list_file,
        revocation_file,
        removal_status: "complete",
        policy_activation_status: "required",
        runtime_peer_directory_status: "refresh-required",
        ratchet_session_retirement_status: "required",
        history_availability_status: "existing-copies-remain-readable",
    })
}

fn verify_exact_removal(
    before: &AccountRootRecoveryPackage,
    after: &AccountRootRecoveryPackage,
    device_id: DeviceId,
    revocation: &DeviceRevocation,
) -> Result<()> {
    verify_recovery_package_successor(before, after, true)
        .context("verify monotonic recovery-package successor")?;
    ensure!(
        after.authority_revision()
            == before
                .authority_revision()
                .checked_add(1)
                .context("before-removal authority revision is exhausted")?,
        "after-removal authority revision is not the exact next revision"
    );
    ensure!(
        after.device_count().saturating_add(1) == before.device_count(),
        "after-removal active-device count is not exactly one smaller"
    );
    ensure!(
        after.device_list().certificate_for(device_id).is_none(),
        "removed device remains in the after-removal active-device list"
    );
    for certificate in before.device_list().devices() {
        if certificate.device_id() != device_id {
            ensure!(
                after.device_list().certificate_for(certificate.device_id()) == Some(certificate),
                "an unrelated active-device certificate changed during removal"
            );
        }
    }
    ensure!(
        before.conversation_memberships() == after.conversation_memberships(),
        "conversation authority changed during device removal"
    );
    ensure!(
        revocation.account_id() == before.account_id()
            && revocation.device_id() == device_id
            && revocation.authority_sequence() == before.authority_revision(),
        "device revocation is not the exact next authority operation"
    );
    let old_revocations = before.authority_snapshot().revocations();
    let new_revocations = after.authority_snapshot().revocations();
    ensure!(
        new_revocations.len() == old_revocations.len().saturating_add(1)
            && new_revocations.iter().any(|item| item == revocation)
            && old_revocations
                .iter()
                .all(|item| new_revocations.contains(item)),
        "after-removal authority contains changes other than the exact device revocation"
    );
    Ok(())
}

fn read_package(path: &Path) -> Result<AccountRootRecoveryPackage> {
    AccountRootRecoveryPackage::decode_and_verify(&fs::read(path)?)
        .context("decode before-removal recovery package")
}

fn read_witness(path: &Path) -> Result<AccountRootRecoveryWitness> {
    AccountRootRecoveryWitness::decode_and_verify(&fs::read(path)?)
        .context("decode before-removal recovery witness")
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let path = fs::canonicalize(path).with_context(|| format!("resolve {label} directory"))?;
    ensure!(path.is_dir(), "{label} path is not a directory");
    Ok(path)
}

fn canonical_artifact(path: &Path, maximum: usize, label: &str) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("read {label} metadata at {}", path.display()))?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "{label} must not be a symlink"
    );
    ensure!(metadata.is_file(), "{label} is not a regular file");
    ensure!(metadata.len() <= maximum as u64, "{label} is too large");
    fs::canonicalize(path).with_context(|| format!("resolve {label}"))
}

fn resolve_external_output(requested: &Path, account_root_dir: &Path) -> Result<PathBuf> {
    ensure!(
        !requested.as_os_str().is_empty(),
        "device-removal output path is empty"
    );
    let lexical = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        std::env::current_dir()?.join(requested)
    };
    let name = lexical
        .file_name()
        .context("device-removal output has no file name")?;
    let parent = lexical
        .parent()
        .context("device-removal output has no parent")?;
    fs::create_dir_all(parent)?;
    let resolved = fs::canonicalize(parent)?.join(name);
    ensure!(
        !resolved.starts_with(account_root_dir),
        "device-removal artifacts must be stored outside the Account Root directory"
    );
    if resolved.exists() {
        let metadata = fs::symlink_metadata(&resolved)?;
        ensure!(
            !metadata.file_type().is_symlink() && metadata.is_file(),
            "existing device-removal output must be a regular non-symlink file"
        );
    }
    Ok(resolved)
}

fn write_idempotent(path: &Path, bytes: &[u8]) -> Result<()> {
    match fs::read(path) {
        Ok(existing) => {
            ensure!(
                existing == bytes,
                "existing output differs: {}",
                path.display()
            );
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut temporary =
                NamedTempFile::new_in(path.parent().context("output has no parent")?)?;
            temporary.write_all(bytes)?;
            temporary.as_file().sync_all()?;
            temporary
                .persist_noclobber(path)
                .map_err(|error| error.error)?;
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use kilogram_identity::{DeviceCapability, DeviceState};

    use super::*;

    #[test]
    fn removes_exactly_one_device_and_is_idempotently_recoverable() -> Result<(), Box<dyn Error>> {
        let parent = tempfile::tempdir()?;
        let workspace = parent.path().join("account");
        let created = crate::create_account(&workspace)?;
        let root = AccountRootState::load(created.account_root_dir())?;
        let second = DeviceState::load_or_create(parent.path().join("second"))?;
        root.enroll_device(
            second.identity().device_id(),
            second.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let before_package = parent.path().join("before.karp");
        let before_witness = parent.path().join("before.karw");
        crate::account_recovery::export_account_root(
            created.account_root_dir(),
            &before_package,
            &before_witness,
        )?;
        let after_package = parent.path().join("after.karp");
        let after_witness = parent.path().join("after.karw");
        let device_list = parent.path().join("active.snapshot");
        let revocation = parent.path().join("removed.revocation");
        let report = remove_device(
            created.account_root_dir(),
            second.identity().device_id(),
            &before_package,
            &before_witness,
            &after_package,
            &after_witness,
            &device_list,
            &revocation,
        )?;
        assert_eq!(report.removal_status, "complete");
        assert_eq!(report.policy_activation_status, "required");
        assert_eq!(report.before_device_count, 2);
        assert_eq!(report.after_device_count, 1);
        assert_eq!(
            report.after_authority_revision,
            report.before_authority_revision + 1
        );
        assert_eq!(
            root.recovery_checkpoint_status()?.state().as_str(),
            "current"
        );
        assert!(
            root.published_device_list()?
                .certificate_for(second.identity().device_id())
                .is_none()
        );
        let retry = remove_device(
            created.account_root_dir(),
            second.identity().device_id(),
            &before_package,
            &before_witness,
            &after_package,
            &after_witness,
            &device_list,
            &revocation,
        )?;
        assert_eq!(retry, report);
        assert!(
            remove_device(
                created.account_root_dir(),
                created.device_id(),
                &after_package,
                &after_witness,
                parent.path().join("invalid-after.karp"),
                parent.path().join("invalid-after.karw"),
                parent.path().join("invalid-list"),
                parent.path().join("invalid-revocation"),
            )
            .is_err()
        );
        Ok(())
    }
}
