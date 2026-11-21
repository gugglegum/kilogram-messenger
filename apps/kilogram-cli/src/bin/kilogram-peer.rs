//! Small product frontend, built separately from the protocol administration CLI.
#![forbid(unsafe_code)]
use clap::{Parser, Subcommand};
use kilogram_cli::terminal_chat;
use std::path::PathBuf;
#[path = "../peer_chat.rs"]
mod peer_chat;

#[derive(Parser)]
#[command(about = "Kilogram: одно приглашение, автоматическое подключение и сохранённая переписка")]
struct Options {
    #[command(subcommand)]
    internal: Option<Internal>,
    #[arg(long, default_value = "kilogram-profile")]
    profile: PathBuf,
    #[arg(long)]
    join: Option<String>,
    #[arg(long)]
    relay: Option<String>,
    #[arg(long)]
    jsonl: bool,
}
#[derive(Subcommand)]
enum Internal {
    #[command(hide = true)]
    Chat {
        #[arg(long)]
        setup: PathBuf,
        #[arg(long)]
        jsonl: bool,
    },
    #[command(hide = true)]
    RuntimeFromProfile {
        #[arg(long)]
        profile_file: PathBuf,
    },
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Reuse the existing, closed account-signing/import helper implementation.
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let helper = args.first().is_some_and(|s| s == "runtime-profile-create")
        || (args.first().is_some_and(|s| s == "ordered")
            && args.get(1).is_some_and(|s| {
                matches!(
                    s.to_str(),
                    Some("context2-propose" | "context2-cosign" | "context2-import")
                )
            }));
    if helper {
        let exit = tokio::process::Command::new(
            std::env::current_exe()?.with_file_name("kilogram-cli.exe"),
        )
        .args(args)
        .status()
        .await?;
        std::process::exit(exit.code().unwrap_or(1));
    }
    let options = Options::parse();
    match options.internal {
        Some(Internal::Chat { setup, jsonl }) => {
            let exit = tokio::task::spawn_blocking(move || {
                kilogram_cli::terminal_chat::run_product_owner(setup, jsonl)
            })
            .await?;
            if exit != 0 {
                std::process::exit(exit);
            }
            return Ok(());
        }
        Some(Internal::RuntimeFromProfile { profile_file }) => {
            return kilogram_cli::ordered_dialog::runtime2::runtime::run(
                kilogram_cli::ordered_dialog::runtime2::config::Profile2::load(&profile_file)?,
            )
            .await;
        }
        None => {}
    }
    peer_chat::run(options.profile, options.join, options.relay, options.jsonl).await
}
