use std::{
    io::{Read as _, Write as _},
    path::PathBuf,
    str::FromStr as _,
};

use anyhow::{Context as _, Result};
use clap::{Parser, Subcommand};
use kilogram_identity::{AccountId, AccountRecoveryPhrase};
use zeroize::Zeroizing;

#[derive(Debug, Parser)]
#[command(name = "kilogram-bootstrap")]
#[command(about = "One-shot Kilogram account/device bootstrap helper")]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a new recoverable account and its first enrolled device.
    Create {
        #[arg(long)]
        workspace_dir: PathBuf,
    },
    /// Create a short-lived request from a new device for an existing account.
    DeviceLinkRequest {
        #[arg(long)]
        workspace_dir: PathBuf,
        #[arg(long)]
        account_id: String,
    },
    /// Verify a device-link request offline and display its comparison code.
    DeviceLinkInspect {
        #[arg(long)]
        request_file: PathBuf,
    },
    /// Authorize an exact request with the existing Account Root.
    DeviceLinkAuthorize {
        #[arg(long)]
        account_root_dir: PathBuf,
        #[arg(long)]
        request_file: PathBuf,
        #[arg(long)]
        confirm_sas: String,
        #[arg(long)]
        response_file: PathBuf,
        #[arg(long)]
        device_list_file: PathBuf,
    },
    /// Accept the encrypted authorization on the exact requesting device.
    DeviceLinkAccept {
        #[arg(long)]
        workspace_dir: PathBuf,
        #[arg(long)]
        response_file: PathBuf,
    },
    /// Export current Root authority history and a separately retained witness.
    AccountRecoveryExport {
        #[arg(long)]
        account_root_dir: PathBuf,
        #[arg(long)]
        package_file: PathBuf,
        #[arg(long)]
        witness_file: PathBuf,
    },
    /// Authenticate and inspect a recovery package without reading the phrase.
    AccountRecoveryInspect {
        #[arg(long)]
        package_file: PathBuf,
        #[arg(long)]
        witness_file: PathBuf,
    },
    /// Restore a Root into a new directory; reads the 24 words from standard input.
    AccountRecoveryRestore {
        #[arg(long)]
        account_root_dir: PathBuf,
        #[arg(long)]
        package_file: PathBuf,
        #[arg(long)]
        witness_file: PathBuf,
        #[arg(long, action = clap::ArgAction::SetTrue)]
        recovery_phrase_stdin: bool,
    },
}

fn main() -> Result<()> {
    let output = match Arguments::parse().command {
        Command::Create { workspace_dir } => kilogram_bootstrap::create_account(workspace_dir)?
            .encode()?
            .to_vec(),
        Command::DeviceLinkRequest {
            workspace_dir,
            account_id,
        } => serde_json::to_vec(&kilogram_bootstrap::device_link::create_request(
            workspace_dir,
            AccountId::from_str(&account_id).context("parse account ID")?,
        )?)?,
        Command::DeviceLinkInspect { request_file } => serde_json::to_vec(
            &kilogram_bootstrap::device_link::inspect_request(request_file)?,
        )?,
        Command::DeviceLinkAuthorize {
            account_root_dir,
            request_file,
            confirm_sas,
            response_file,
            device_list_file,
        } => serde_json::to_vec(&kilogram_bootstrap::device_link::authorize_request(
            account_root_dir,
            request_file,
            &confirm_sas,
            response_file,
            device_list_file,
        )?)?,
        Command::DeviceLinkAccept {
            workspace_dir,
            response_file,
        } => serde_json::to_vec(&kilogram_bootstrap::device_link::accept_response(
            workspace_dir,
            response_file,
        )?)?,
        Command::AccountRecoveryExport {
            account_root_dir,
            package_file,
            witness_file,
        } => serde_json::to_vec(&kilogram_bootstrap::account_recovery::export_account_root(
            account_root_dir,
            package_file,
            witness_file,
        )?)?,
        Command::AccountRecoveryInspect {
            package_file,
            witness_file,
        } => serde_json::to_vec(&kilogram_bootstrap::account_recovery::inspect_account_root(
            package_file,
            witness_file,
        )?)?,
        Command::AccountRecoveryRestore {
            account_root_dir,
            package_file,
            witness_file,
            recovery_phrase_stdin,
        } => {
            anyhow::ensure!(
                recovery_phrase_stdin,
                "Account Root recovery requires --recovery-phrase-stdin"
            );
            let phrase = read_recovery_phrase()?;
            serde_json::to_vec(&kilogram_bootstrap::account_recovery::restore_account_root(
                account_root_dir,
                package_file,
                witness_file,
                &phrase,
            )?)?
        }
    };
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&output)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

fn read_recovery_phrase() -> Result<AccountRecoveryPhrase> {
    const MAX_PHRASE_BYTES: usize = 4096;
    let mut bytes = Zeroizing::new(Vec::new());
    std::io::stdin()
        .lock()
        .take((MAX_PHRASE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .context("read recovery phrase from standard input")?;
    anyhow::ensure!(
        bytes.len() <= MAX_PHRASE_BYTES,
        "recovery phrase input is too large"
    );
    let phrase = Zeroizing::new(
        String::from_utf8(bytes.to_vec()).context("recovery phrase input is not UTF-8")?,
    );
    AccountRecoveryPhrase::parse(&phrase).context("parse Account Root recovery phrase")
}
