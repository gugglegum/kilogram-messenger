use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    str::FromStr as _,
};

use anyhow::{Context as _, Result, bail, ensure};
use kilogram_identity::{AccountId, DeviceId};
use serde::Deserialize;

pub(crate) const MAX_WIZARD_OUTPUT_BYTES: usize = 256 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeviceLinkRequestOutput {
    pub(crate) status: String,
    pub(crate) account_id: String,
    pub(crate) device_id: String,
    pub(crate) request_id: String,
    pub(crate) sas: String,
    pub(crate) expires_at_unix_seconds: u64,
    pub(crate) workspace_dir: PathBuf,
    pub(crate) state_dir: PathBuf,
    pub(crate) request_file: PathBuf,
    pub(crate) prekey_pool_file: PathBuf,
    pub(crate) vault_key_protection: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeviceLinkInspectOutput {
    pub(crate) status: String,
    pub(crate) account_id: String,
    pub(crate) device_id: String,
    pub(crate) request_id: String,
    pub(crate) sas: String,
    pub(crate) issued_at_unix_seconds: u64,
    pub(crate) expires_at_unix_seconds: u64,
    pub(crate) request_fresh: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeviceLinkAuthorizeOutput {
    pub(crate) status: String,
    pub(crate) account_id: String,
    pub(crate) device_id: String,
    pub(crate) request_id: String,
    pub(crate) authority_revision: u64,
    pub(crate) response_file: PathBuf,
    pub(crate) device_list_file: PathBuf,
    pub(crate) response_encrypted_for_device: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeviceLinkAcceptOutput {
    pub(crate) status: String,
    pub(crate) account_id: String,
    pub(crate) device_id: String,
    pub(crate) request_id: String,
    pub(crate) authority_revision: u64,
    pub(crate) workspace_dir: PathBuf,
    pub(crate) state_dir: PathBuf,
    pub(crate) certificate_file: PathBuf,
    pub(crate) device_list_file: PathBuf,
    pub(crate) prekey_pool_file: PathBuf,
    pub(crate) vault_key_protection: String,
    pub(crate) history_recovery: String,
}

pub(crate) trait WizardJsonOutput: Sized + for<'de> Deserialize<'de> {
    fn validate(&self) -> Result<()>;
}

impl WizardJsonOutput for DeviceLinkRequestOutput {
    fn validate(&self) -> Result<()> {
        validate_status(&self.status, "device-link-request-created")?;
        validate_identity_fields(&self.account_id, &self.device_id, &self.request_id)?;
        validate_sas(&self.sas)?;
        validate_workspace_paths(
            &self.workspace_dir,
            [&self.state_dir, &self.request_file, &self.prekey_pool_file],
        )?;
        ensure!(
            self.expires_at_unix_seconds > 0,
            "device-link request expiry is missing"
        );
        ensure!(
            !self.vault_key_protection.is_empty(),
            "device-link vault protection is missing"
        );
        Ok(())
    }
}

impl WizardJsonOutput for DeviceLinkInspectOutput {
    fn validate(&self) -> Result<()> {
        validate_status(&self.status, "device-link-request-verified")?;
        validate_identity_fields(&self.account_id, &self.device_id, &self.request_id)?;
        validate_sas(&self.sas)?;
        ensure!(
            self.issued_at_unix_seconds <= self.expires_at_unix_seconds,
            "device-link request timestamps are invalid"
        );
        Ok(())
    }
}

impl WizardJsonOutput for DeviceLinkAuthorizeOutput {
    fn validate(&self) -> Result<()> {
        validate_status(&self.status, "device-link-authorized")?;
        validate_identity_fields(&self.account_id, &self.device_id, &self.request_id)?;
        ensure!(
            self.authority_revision > 0,
            "device-link authority revision is missing"
        );
        ensure!(
            self.response_encrypted_for_device,
            "device-link response is not recipient encrypted"
        );
        validate_absolute_file_path(&self.response_file, "response_file")?;
        validate_absolute_file_path(&self.device_list_file, "device_list_file")?;
        Ok(())
    }
}

impl WizardJsonOutput for DeviceLinkAcceptOutput {
    fn validate(&self) -> Result<()> {
        validate_status(&self.status, "device-link-accepted")?;
        validate_identity_fields(&self.account_id, &self.device_id, &self.request_id)?;
        ensure!(
            self.authority_revision > 0,
            "device-link authority revision is missing"
        );
        validate_workspace_paths(
            &self.workspace_dir,
            [
                &self.state_dir,
                &self.certificate_file,
                &self.device_list_file,
                &self.prekey_pool_file,
            ],
        )?;
        ensure!(
            !self.vault_key_protection.is_empty(),
            "device-link vault protection is missing"
        );
        ensure!(
            self.history_recovery == "ready-for-recipient-bound-multi-source-plans",
            "device-link recovery readiness is invalid"
        );
        Ok(())
    }
}

fn validate_status(actual: &str, expected: &str) -> Result<()> {
    ensure!(
        actual == expected,
        "wizard helper returned unexpected status"
    );
    Ok(())
}

fn validate_identity_fields(account: &str, device: &str, request: &str) -> Result<()> {
    AccountId::from_str(account).context("wizard Account ID is invalid")?;
    DeviceId::from_str(device).context("wizard Device ID is invalid")?;
    validate_hex_id(request, "request ID")?;
    Ok(())
}

fn validate_sas(sas: &str) -> Result<()> {
    ensure!(
        sas.len() == 12 && sas.bytes().all(|byte| byte.is_ascii_digit()),
        "wizard SAS must contain exactly 12 digits"
    );
    Ok(())
}

fn validate_hex_id(value: &str, label: &str) -> Result<()> {
    ensure!(
        value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "wizard {label} is invalid"
    );
    Ok(())
}

fn validate_workspace_paths<'a>(
    workspace: &Path,
    paths: impl IntoIterator<Item = &'a PathBuf>,
) -> Result<()> {
    ensure!(workspace.is_absolute(), "wizard workspace must be absolute");
    for path in paths {
        ensure!(
            path.is_absolute() && path.starts_with(workspace),
            "wizard output path escaped its workspace"
        );
    }
    Ok(())
}

fn validate_absolute_file_path(path: &Path, label: &str) -> Result<()> {
    ensure!(path.is_absolute(), "wizard {label} must be absolute");
    ensure!(
        path.file_name().is_some(),
        "wizard {label} has no file name"
    );
    Ok(())
}

pub(crate) fn run_json<T>(
    executable: &Path,
    subcommand: &str,
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<T>
where
    T: WizardJsonOutput,
{
    let output = run_process(executable, subcommand, arguments)?;
    ensure_process_success(subcommand, &output)?;
    let value: T = serde_json::from_slice(&output.stdout).context("decode wizard helper JSON")?;
    value.validate()?;
    Ok(value)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RecoveryCommandOutput {
    pub(crate) fields: BTreeMap<String, String>,
}

impl RecoveryCommandOutput {
    pub(crate) fn status(&self) -> &str {
        self.fields
            .get("status")
            .map(String::as_str)
            .unwrap_or("unknown")
    }

    pub(crate) fn field(&self, name: &str) -> Option<&str> {
        self.fields.get(name).map(String::as_str)
    }
}

pub(crate) fn run_recovery_command(
    executable: &Path,
    subcommand: &str,
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<RecoveryCommandOutput> {
    let output = run_process(executable, subcommand, arguments)?;
    let mut parsed = match parse_key_value_output(&output.stdout) {
        Ok(parsed) => parsed,
        Err(error) if !output.success => {
            return Err(error.context(process_failure_message(subcommand, &output)));
        }
        Err(error) => return Err(error),
    };
    parsed.fields.insert(
        "process_exit_success".to_owned(),
        output.success.to_string(),
    );
    if !output.success {
        parsed.fields.insert(
            "process_error".to_owned(),
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        );
    }
    Ok(parsed)
}

struct ProcessOutput {
    success: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    status: String,
}

fn run_process(
    executable: &Path,
    subcommand: &str,
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<ProcessOutput> {
    let mut command = Command::new(executable);
    command
        .arg(subcommand)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command
        .output()
        .with_context(|| format!("start {} {subcommand}", executable.display()))?;
    ensure!(
        output.stdout.len() <= MAX_WIZARD_OUTPUT_BYTES,
        "wizard command output is too large"
    );
    ensure!(
        output.stderr.len() <= MAX_WIZARD_OUTPUT_BYTES,
        "wizard command error output is too large"
    );
    Ok(ProcessOutput {
        success: output.status.success(),
        stdout: output.stdout,
        stderr: output.stderr,
        status: output.status.to_string(),
    })
}

fn ensure_process_success(subcommand: &str, output: &ProcessOutput) -> Result<()> {
    if output.success {
        Ok(())
    } else {
        bail!(process_failure_message(subcommand, output))
    }
}

fn process_failure_message(subcommand: &str, output: &ProcessOutput) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    format!(
        "{subcommand} exited with {}: {}{}{}",
        output.status,
        stderr.trim(),
        if stderr.is_empty() || stdout.is_empty() {
            ""
        } else {
            "; output: "
        },
        stdout.trim()
    )
}

pub(crate) fn parse_key_value_output(bytes: &[u8]) -> Result<RecoveryCommandOutput> {
    ensure!(
        bytes.len() <= MAX_WIZARD_OUTPUT_BYTES,
        "recovery command output is too large"
    );
    let text = std::str::from_utf8(bytes).context("recovery command output is not UTF-8")?;
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        ensure!(
            line.len() <= 16 * 1024,
            "recovery command output line is too long"
        );
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            fields.insert(key.to_owned(), value.to_owned());
        }
    }
    ensure!(
        fields.contains_key("status"),
        "recovery command omitted terminal status"
    );
    Ok(RecoveryCommandOutput { fields })
}

pub(crate) fn command_arguments(
    pairs: impl IntoIterator<Item = (impl AsRef<OsStr>, impl AsRef<OsStr>)>,
) -> Vec<OsString> {
    let mut arguments = Vec::new();
    for (name, value) in pairs {
        arguments.push(name.as_ref().to_os_string());
        arguments.push(value.as_ref().to_os_string());
    }
    arguments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_output_keeps_last_structured_value_and_requires_status() -> Result<()> {
        let output = parse_key_value_output(
            b"history_recovery_scheduler_lifecycle=pending\nstatus=scheduled\nstatus=complete\n",
        )?;
        assert_eq!(output.status(), "complete");
        assert_eq!(
            output.field("history_recovery_scheduler_lifecycle"),
            Some("pending")
        );
        assert!(parse_key_value_output(b"not structured\n").is_err());
        Ok(())
    }

    #[test]
    fn device_link_json_rejects_relative_or_unexpected_outputs() {
        let json = br#"{
            "status":"device-link-request-created",
            "account_id":"0101010101010101010101010101010101010101010101010101010101010101",
            "device_id":"0202020202020202020202020202020202020202020202020202020202020202",
            "request_id":"0303030303030303030303030303030303030303030303030303030303030303",
            "sas":"123456789012",
            "expires_at_unix_seconds":1,
            "workspace_dir":"relative",
            "state_dir":"relative/device",
            "request_file":"relative/request",
            "prekey_pool_file":"relative/prekeys",
            "vault_key_protection":"test"
        }"#;
        let output: DeviceLinkRequestOutput = serde_json::from_slice(json)
            .unwrap_or_else(|error| unreachable!("fixture must decode: {error}"));
        assert!(output.validate().is_err());
    }

    #[test]
    fn device_link_json_accepts_bounded_absolute_workspace_and_denies_extensions() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let workspace = temporary.path().join("joining");
        let mut value = serde_json::json!({
            "status": "device-link-request-created",
            "account_id": "0101010101010101010101010101010101010101010101010101010101010101",
            "device_id": "0202020202020202020202020202020202020202020202020202020202020202",
            "request_id": "0303030303030303030303030303030303030303030303030303030303030303",
            "sas": "123456789012",
            "expires_at_unix_seconds": 1,
            "workspace_dir": workspace,
            "state_dir": workspace.join("device"),
            "request_file": workspace.join("device-link").join("request.bin"),
            "prekey_pool_file": workspace.join("public").join("prekeys.bin"),
            "vault_key_protection": "test"
        });
        let output: DeviceLinkRequestOutput = serde_json::from_value(value.clone())?;
        output.validate()?;

        value["unexpected"] = serde_json::Value::Bool(true);
        assert!(serde_json::from_value::<DeviceLinkRequestOutput>(value).is_err());
        Ok(())
    }
}
