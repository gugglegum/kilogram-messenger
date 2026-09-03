use std::{io::Write as _, path::PathBuf};

use anyhow::Result;
use clap::{Parser, Subcommand};

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
}

fn main() -> Result<()> {
    let output = match Arguments::parse().command {
        Command::Create { workspace_dir } => kilogram_bootstrap::create_account(workspace_dir)?,
    };
    let encoded = output.encode()?;
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&encoded)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}
