use std::{fmt, path::PathBuf};

use anyhow::{Result, bail, ensure};
use kilogram_identity::{AccountId, AccountRecoveryPhrase, DeviceId};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

pub const DESKTOP_BOOTSTRAP_VERSION: u8 = 1;
pub const MAX_DESKTOP_BOOTSTRAP_OUTPUT_BYTES: usize = 64 * 1024;

/// One-shot result returned by `kilogram-bootstrap` to the first-run desktop UI.
///
/// The recovery phrase is deliberately absent from `Debug` output and is
/// zeroized when this value is dropped. It must never be written to the receipt
/// or normal runtime profile.
#[derive(Deserialize, Serialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct DesktopBootstrapOutput {
    #[zeroize(skip)]
    version: u8,
    #[zeroize(skip)]
    account_id: AccountId,
    #[zeroize(skip)]
    device_id: DeviceId,
    recovery_phrase: String,
    #[zeroize(skip)]
    workspace_dir: PathBuf,
    #[zeroize(skip)]
    account_root_dir: PathBuf,
    #[zeroize(skip)]
    state_dir: PathBuf,
    #[zeroize(skip)]
    device_list_file: PathBuf,
    #[zeroize(skip)]
    certificate_file: PathBuf,
    #[zeroize(skip)]
    prekey_pool_file: PathBuf,
    #[zeroize(skip)]
    receipt_file: PathBuf,
    root_key_protection: String,
    vault_key_protection: String,
}

impl fmt::Debug for DesktopBootstrapOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DesktopBootstrapOutput")
            .field("version", &self.version)
            .field("account_id", &self.account_id)
            .field("device_id", &self.device_id)
            .field("recovery_phrase", &"[REDACTED]")
            .field("workspace_dir", &self.workspace_dir)
            .field("account_root_dir", &self.account_root_dir)
            .field("state_dir", &self.state_dir)
            .field("device_list_file", &self.device_list_file)
            .field("certificate_file", &self.certificate_file)
            .field("prekey_pool_file", &self.prekey_pool_file)
            .field("receipt_file", &self.receipt_file)
            .field("root_key_protection", &self.root_key_protection)
            .field("vault_key_protection", &self.vault_key_protection)
            .finish()
    }
}

#[allow(clippy::too_many_arguments)]
pub fn new_desktop_bootstrap_output(
    account_id: AccountId,
    device_id: DeviceId,
    recovery_phrase: String,
    workspace_dir: PathBuf,
    account_root_dir: PathBuf,
    state_dir: PathBuf,
    device_list_file: PathBuf,
    certificate_file: PathBuf,
    prekey_pool_file: PathBuf,
    receipt_file: PathBuf,
    root_key_protection: String,
    vault_key_protection: String,
) -> DesktopBootstrapOutput {
    DesktopBootstrapOutput {
        version: DESKTOP_BOOTSTRAP_VERSION,
        account_id,
        device_id,
        recovery_phrase,
        workspace_dir,
        account_root_dir,
        state_dir,
        device_list_file,
        certificate_file,
        prekey_pool_file,
        receipt_file,
        root_key_protection,
        vault_key_protection,
    }
}

impl DesktopBootstrapOutput {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_DESKTOP_BOOTSTRAP_OUTPUT_BYTES,
            "desktop bootstrap output is too large"
        );
        let output: Self = serde_json::from_slice(bytes)?;
        output.validate()?;
        Ok(output)
    }

    pub fn encode(&self) -> Result<Zeroizing<Vec<u8>>> {
        self.validate()?;
        let bytes = Zeroizing::new(serde_json::to_vec(self)?);
        ensure!(
            bytes.len() <= MAX_DESKTOP_BOOTSTRAP_OUTPUT_BYTES,
            "desktop bootstrap output is too large"
        );
        Ok(bytes)
    }

    pub fn account_id(&self) -> AccountId {
        self.account_id
    }

    pub fn device_id(&self) -> DeviceId {
        self.device_id
    }

    pub fn recovery_phrase(&self) -> &str {
        &self.recovery_phrase
    }

    pub fn take_recovery_phrase(&mut self) -> String {
        std::mem::take(&mut self.recovery_phrase)
    }

    pub fn workspace_dir(&self) -> &PathBuf {
        &self.workspace_dir
    }

    pub fn account_root_dir(&self) -> &PathBuf {
        &self.account_root_dir
    }

    pub fn state_dir(&self) -> &PathBuf {
        &self.state_dir
    }

    pub fn device_list_file(&self) -> &PathBuf {
        &self.device_list_file
    }

    pub fn certificate_file(&self) -> &PathBuf {
        &self.certificate_file
    }

    pub fn prekey_pool_file(&self) -> &PathBuf {
        &self.prekey_pool_file
    }

    pub fn receipt_file(&self) -> &PathBuf {
        &self.receipt_file
    }

    pub fn root_key_protection(&self) -> &str {
        &self.root_key_protection
    }

    pub fn vault_key_protection(&self) -> &str {
        &self.vault_key_protection
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.version == DESKTOP_BOOTSTRAP_VERSION,
            "unsupported desktop bootstrap output version"
        );
        let phrase = AccountRecoveryPhrase::parse(&self.recovery_phrase).map_err(|error| {
            anyhow::anyhow!("invalid desktop bootstrap recovery phrase: {error}")
        })?;
        ensure!(
            phrase.account_id()? == self.account_id,
            "desktop bootstrap recovery phrase does not match Account ID"
        );
        for (name, path) in [
            ("workspace_dir", &self.workspace_dir),
            ("account_root_dir", &self.account_root_dir),
            ("state_dir", &self.state_dir),
            ("device_list_file", &self.device_list_file),
            ("certificate_file", &self.certificate_file),
            ("prekey_pool_file", &self.prekey_pool_file),
            ("receipt_file", &self.receipt_file),
        ] {
            if !path.is_absolute() {
                bail!("desktop bootstrap {name} must be absolute");
            }
            ensure!(
                path == &self.workspace_dir || path.starts_with(&self.workspace_dir),
                "desktop bootstrap {name} must remain inside its workspace"
            );
        }
        ensure!(
            !self.root_key_protection.is_empty() && !self.vault_key_protection.is_empty(),
            "desktop bootstrap key protection metadata is missing"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;

    #[test]
    fn output_round_trip_redacts_phrase_and_rejects_relative_paths() -> Result<(), Box<dyn Error>> {
        let temporary = tempfile::tempdir()?;
        let root = temporary.path().to_path_buf();
        let phrase = ["abandon"; 23]
            .into_iter()
            .chain(["art"])
            .collect::<Vec<_>>()
            .join(" ");
        let account_id = AccountRecoveryPhrase::parse(&phrase)?.account_id()?;
        let output = new_desktop_bootstrap_output(
            account_id,
            DeviceId::from_bytes([2; 32]),
            phrase.clone(),
            root.clone(),
            root.join("account-root"),
            root.join("device"),
            root.join("device-list"),
            root.join("certificate"),
            root.join("prekeys"),
            root.join("receipt"),
            "test-root".to_owned(),
            "test-vault".to_owned(),
        );
        assert!(!format!("{output:?}").contains(&phrase));
        let encoded = output.encode()?;
        assert_eq!(
            DesktopBootstrapOutput::decode(&encoded)?.account_id(),
            account_id
        );

        let mut json: serde_json::Value = serde_json::from_slice(&encoded)?;
        json["state_dir"] = serde_json::Value::String("relative".to_owned());
        assert!(DesktopBootstrapOutput::decode(&serde_json::to_vec(&json)?).is_err());
        Ok(())
    }
}
