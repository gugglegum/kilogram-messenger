use std::{io::Write as _, path::PathBuf, str::FromStr as _};

use anyhow::{Context as _, Result};
use clap::{Parser, Subcommand};
use kilogram_identity::AccountId;

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
    };
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&output)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}
