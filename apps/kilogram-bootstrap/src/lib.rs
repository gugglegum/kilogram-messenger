use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use kilogram_bootstrap_contract::{DesktopBootstrapOutput, new_desktop_bootstrap_output};
use kilogram_identity::{AccountRootState, DeviceCapability, DeviceState};
use kilogram_ratchet::{
    DEFAULT_PREKEY_POOL_SIZE, DEFAULT_PREKEY_POOL_VALIDITY_SECONDS, RatchetState, unix_time_now,
};
use kilogram_state::EncryptedStateVault;
use serde::Serialize;
use tempfile::NamedTempFile;
use zeroize::Zeroize;

const RECEIPT_VERSION: u8 = 1;
const ACCOUNT_ROOT_DIRECTORY: &str = "account-root";
const DEVICE_STATE_DIRECTORY: &str = "device";
const PUBLIC_DIRECTORY: &str = "public";
const DEVICE_CERTIFICATE_FILE: &str = "device-certificate.cert";
const ACCOUNT_DEVICE_LIST_FILE: &str = "account-device-list.snapshot";
const PREKEY_POOL_FILE: &str = "prekey-pool.bin";
const BOOTSTRAP_RECEIPT_FILE: &str = "bootstrap-receipt.json";

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct BootstrapReceipt {
    version: u8,
    account_id: String,
    device_id: String,
    account_root_dir: PathBuf,
    state_dir: PathBuf,
    device_list_file: PathBuf,
    certificate_file: PathBuf,
    prekey_pool_file: PathBuf,
    root_key_protection: String,
    vault_key_protection: String,
    recovery_scope: &'static str,
}

pub fn create_account(workspace_dir: impl AsRef<Path>) -> Result<DesktopBootstrapOutput> {
    let workspace_dir = resolve_new_workspace(workspace_dir.as_ref())?;
    let parent = workspace_dir
        .parent()
        .context("bootstrap workspace has no parent")?;
    let staging = tempfile::Builder::new()
        .prefix(".kilogram-bootstrap-")
        .tempdir_in(parent)
        .context("create bootstrap staging directory")?;

    let staging_root = staging.path().join(ACCOUNT_ROOT_DIRECTORY);
    let staging_state = staging.path().join(DEVICE_STATE_DIRECTORY);
    let staging_public = staging.path().join(PUBLIC_DIRECTORY);
    fs::create_dir(&staging_public).context("create bootstrap public directory")?;

    let (root, phrase) = AccountRootState::create_recoverable(&staging_root)
        .context("create recoverable Account Root")?;
    let device = DeviceState::load_or_create(&staging_state).context("create first device")?;
    let certificate = root
        .issue_device_certificate(
            device.identity().device_id(),
            device.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )
        .context("issue first device certificate")?;
    device
        .install_certificate(&certificate)
        .context("install first device certificate")?;
    let authority = root
        .authority_snapshot()
        .context("build authority snapshot")?;
    device
        .install_own_authority_snapshot(&authority)
        .context("install first authority snapshot")?;
    let device_list = root
        .publish_device_list(std::slice::from_ref(&certificate))
        .context("publish first account device list")?;

    let mut ratchet = RatchetState::load_or_create(&staging_state)
        .context("create first device ratchet state")?;
    let prekey_pool = ratchet
        .prekey_pool(
            device.identity(),
            DEFAULT_PREKEY_POOL_SIZE,
            unix_time_now().context("read prekey publication time")?,
            DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
        )
        .context("create first device prekey pool")?;

    write_new_file(
        &staging_public.join(DEVICE_CERTIFICATE_FILE),
        &certificate.encode().context("encode device certificate")?,
    )?;
    write_new_file(
        &staging_public.join(ACCOUNT_DEVICE_LIST_FILE),
        &device_list.encode().context("encode account device list")?,
    )?;
    write_new_file(
        &staging_public.join(PREKEY_POOL_FILE),
        &prekey_pool.encode().context("encode prekey pool")?,
    )?;

    drop(ratchet);
    let vault = EncryptedStateVault::open_or_create(&staging_state)
        .context("create encrypted device state vault")?;
    let root_key_protection = root.key_protection().as_str().to_owned();
    let vault_key_protection = vault.key_protection().as_str().to_owned();
    vault
        .migrate_legacy_snapshot()
        .context("commit initial device state to encrypted vault")?;
    vault
        .verify()
        .context("verify initial encrypted device state")?;
    drop(vault);

    let final_root = workspace_dir.join(ACCOUNT_ROOT_DIRECTORY);
    let final_state = workspace_dir.join(DEVICE_STATE_DIRECTORY);
    let final_public = workspace_dir.join(PUBLIC_DIRECTORY);
    let final_certificate = final_public.join(DEVICE_CERTIFICATE_FILE);
    let final_device_list = final_public.join(ACCOUNT_DEVICE_LIST_FILE);
    let final_prekey_pool = final_public.join(PREKEY_POOL_FILE);
    let final_receipt = workspace_dir.join(BOOTSTRAP_RECEIPT_FILE);
    let receipt = BootstrapReceipt {
        version: RECEIPT_VERSION,
        account_id: root.account_id().to_string(),
        device_id: device.identity().device_id().to_string(),
        account_root_dir: final_root.clone(),
        state_dir: final_state.clone(),
        device_list_file: final_device_list.clone(),
        certificate_file: final_certificate.clone(),
        prekey_pool_file: final_prekey_pool.clone(),
        root_key_protection: root_key_protection.clone(),
        vault_key_protection: vault_key_protection.clone(),
        recovery_scope: "root-key-only-authority-history-required",
    };
    let mut receipt_bytes = serde_json::to_vec_pretty(&receipt).context("encode receipt")?;
    receipt_bytes.push(b'\n');
    write_new_file(&staging.path().join(BOOTSTRAP_RECEIPT_FILE), &receipt_bytes)?;
    receipt_bytes.zeroize();

    fs::rename(staging.path(), &workspace_dir).with_context(|| {
        format!(
            "publish completed bootstrap workspace at {}",
            workspace_dir.display()
        )
    })?;

    let output = new_desktop_bootstrap_output(
        root.account_id(),
        device.identity().device_id(),
        phrase.expose_secret().to_owned(),
        workspace_dir,
        final_root,
        final_state,
        final_device_list,
        final_certificate,
        final_prekey_pool,
        final_receipt,
        root_key_protection,
        vault_key_protection,
    );
    output.encode().context("validate bootstrap result")?;
    Ok(output)
}

fn resolve_new_workspace(requested: &Path) -> Result<PathBuf> {
    ensure!(
        !requested.as_os_str().is_empty(),
        "bootstrap workspace path is empty"
    );
    let lexical = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        std::env::current_dir()
            .context("read current directory for bootstrap")?
            .join(requested)
    };
    if lexical.exists() {
        bail!("bootstrap workspace already exists: {}", lexical.display());
    }
    let name = lexical
        .file_name()
        .context("bootstrap workspace path has no final component")?;
    let parent = lexical
        .parent()
        .context("bootstrap workspace path has no parent")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("create bootstrap parent {}", parent.display()))?;
    let parent = fs::canonicalize(parent)
        .with_context(|| format!("resolve bootstrap parent {}", parent.display()))?;
    let resolved = parent.join(name);
    ensure!(
        !resolved.exists(),
        "bootstrap workspace already exists: {}",
        resolved.display()
    );
    Ok(resolved)
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("bootstrap file has no parent")?;
    let mut temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("create temporary bootstrap file in {}", parent.display()))?;
    temporary
        .write_all(bytes)
        .with_context(|| format!("write temporary bootstrap file for {}", path.display()))?;
    temporary
        .as_file()
        .sync_all()
        .with_context(|| format!("sync temporary bootstrap file for {}", path.display()))?;
    temporary
        .persist_noclobber(path)
        .map_err(|error| error.error)
        .with_context(|| format!("publish bootstrap file {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use kilogram_identity::{AccountRecoveryPhrase, AccountRootState};
    use kilogram_state::{EncryptedStateVault, STATE_VAULT_KEY_FILE};

    use super::*;

    #[test]
    fn creates_atomic_first_device_workspace_without_persisting_phrase()
    -> Result<(), Box<dyn Error>> {
        let parent = tempfile::tempdir()?;
        let workspace = parent.path().join("new-account");
        let output = create_account(&workspace)?;
        assert_eq!(output.recovery_phrase().split_whitespace().count(), 24);
        assert_eq!(
            AccountRecoveryPhrase::parse(output.recovery_phrase())?.account_id()?,
            output.account_id()
        );
        assert_eq!(
            AccountRootState::load(output.account_root_dir())?.account_id(),
            output.account_id()
        );
        assert!(output.device_list_file().is_file());
        assert!(output.certificate_file().is_file());
        assert!(output.prekey_pool_file().is_file());
        assert!(output.receipt_file().is_file());
        let receipt = fs::read_to_string(output.receipt_file())?;
        assert!(!receipt.contains(output.recovery_phrase()));
        assert!(
            EncryptedStateVault::open_existing(output.state_dir())?
                .verify()
                .is_ok()
        );
        let key_envelope = fs::read(output.state_dir().join(STATE_VAULT_KEY_FILE))?;
        assert_ne!(key_envelope.len(), 32);
        assert!(create_account(&workspace).is_err());
        Ok(())
    }
}
