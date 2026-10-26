//! Explicit IPC3 bridge; no fallback to IPC2/legacy on an error.
use anyhow::{Context, Result, bail, ensure};
use clap::Subcommand;
use kilogram_cli::ordered_dialog::{admin, state};
use kilogram_protocol::ordered_dialog_v1::{self as wire, Value, a, b, opt, u};
use kilogram_runtime_ipc::{RuntimeIpcCommand, RuntimeIpcResponse};
use std::path::{Path, PathBuf};
#[derive(Debug, Subcommand)]
pub enum Command {
    Status3 {
        #[arg(long)]
        ipc_file: PathBuf,
    },
    Send3 {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        slot: u8,
        #[arg(long)]
        request_id: String,
        #[arg(long)]
        text: String,
    },
    RequestStatus3 {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        slot: u8,
        #[arg(long)]
        request_id: String,
    },
    History3 {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        slot: u8,
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long, default_value_t = 16)]
        limit: usize,
    },
    Locator3Import {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        slot: u8,
        #[arg(long)]
        locator_file: PathBuf,
    },
    Locator3Export {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        slot: u8,
        #[arg(long)]
        output_file: PathBuf,
    },
    Authority3Import {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        snapshot_file: PathBuf,
    },
    Consent1Import {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        slot: u8,
        #[arg(long)]
        expected_current: String,
        #[arg(long)]
        consent_file: PathBuf,
    },
    StorageIncreaseCapacityLive {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        config_file: PathBuf,
        #[arg(long)]
        expected_root_generation: u64,
        #[arg(long)]
        expected_config_revision: u64,
        #[arg(long)]
        expected_config_hash: String,
    },
}
fn request(text: &str) -> Result<[u8; 16]> {
    ensure!(text.len() == 32, "request ID16 hex bytes");
    let mut out = [0; 16];
    for (i, v) in out.iter_mut().enumerate() {
        *v = u8::from_str_radix(&text[2 * i..2 * i + 2], 16)?;
    }
    Ok(out)
}
async fn call(path: &Path, body: Value) -> Result<Value> {
    match kilogram_runtime_ipc::call(
        path,
        RuntimeIpcCommand::OrderedDialog {
            request: wire::encode(&body, 32 * 1024)?,
        },
    )
    .await?
    {
        RuntimeIpcResponse::OrderedDialog { response } => {
            let value = wire::decode(&response, 48 * 1024)?;
            let f = value.fields(3)?;
            ensure!(f[0].uint()? == 3, "UnsupportedVersion: expected IPC3 reply");
            Ok(f[2].clone())
        }
        RuntimeIpcResponse::Error { message } => bail!("{message}"),
        _ => bail!("unexpected IPC3 response"),
    }
}
fn json(value: &Value) -> serde_json::Value {
    match value {
        Value::U(n) => (*n).into(),
        Value::B(bytes) => state::hex(bytes).into(),
        Value::A(values) => values.iter().map(json).collect(),
    }
}
pub async fn run(command: Command) -> Result<()> {
    let result = match command {
        Command::Status3 { ipc_file } => {
            let value = call(&ipc_file, a([u(3), u(1)])).await?;
            println!("{}", std::str::from_utf8(value.bytes()?)?);
            return Ok(());
        }
        Command::Send3 {
            ipc_file,
            slot,
            request_id,
            text,
        } => {
            call(
                &ipc_file,
                a([
                    u(3),
                    u(2),
                    u(slot.into()),
                    b(request(&request_id)?),
                    b(text.as_bytes()),
                ]),
            )
            .await?
        }
        Command::RequestStatus3 {
            ipc_file,
            slot,
            request_id,
        } => {
            call(
                &ipc_file,
                a([u(3), u(3), u(slot.into()), b(request(&request_id)?)]),
            )
            .await?
        }
        Command::History3 {
            ipc_file,
            slot,
            cursor,
            limit,
        } => {
            let cursor = cursor
                .map(|s| kilogram_state::indexed_dialog::HistoryCursor::from_base64(&s))
                .transpose()?;
            let value = call(
                &ipc_file,
                a([
                    u(3),
                    u(4),
                    u(slot.into()),
                    opt(cursor.as_ref().map(|c| b(c.as_bytes()))),
                    u(limit as u64),
                ]),
            )
            .await?;
            println!("{}", std::str::from_utf8(value.bytes()?)?);
            return Ok(());
        }
        Command::Locator3Import {
            ipc_file,
            slot,
            locator_file,
        } => {
            call(
                &ipc_file,
                a([
                    u(3),
                    u(5),
                    u(slot.into()),
                    u(1),
                    u(0),
                    opt(Some(b(admin::read_public(&locator_file, 8192)?))),
                ]),
            )
            .await?
        }
        Command::Locator3Export {
            ipc_file,
            slot,
            output_file,
        } => {
            let value = call(
                &ipc_file,
                a([u(3), u(5), u(slot.into()), u(1), u(1), opt(None)]),
            )
            .await?;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output_file)?;
            use std::io::Write;
            file.write_all(value.bytes()?)?;
            file.sync_all()?;
            a([])
        }
        Command::Authority3Import {
            ipc_file,
            snapshot_file,
        } => {
            call(
                &ipc_file,
                a([
                    u(3),
                    u(6),
                    b(admin::read_public(&snapshot_file, 16 * 1024)?),
                ]),
            )
            .await?
        }
        Command::Consent1Import {
            ipc_file,
            slot,
            expected_current,
            consent_file,
        } => {
            let expected = if expected_current == "absent" {
                None
            } else {
                Some(state::parse_id(&expected_current)?)
            };
            call(
                &ipc_file,
                a([
                    u(3),
                    u(11),
                    u(slot.into()),
                    opt(expected.map(b)),
                    b(admin::read_public(&consent_file, 4096)?),
                ]),
            )
            .await?
        }
        Command::StorageIncreaseCapacityLive {
            ipc_file,
            config_file,
            expected_root_generation,
            expected_config_revision,
            expected_config_hash,
        } => {
            let raw = admin::read_public(&config_file, 16 * 1024)?;
            let _: kilogram_state::indexed_dialog::migration7::MigrationConfig =
                serde_json::from_slice(&raw).context("strict capacity config")?;
            call(
                &ipc_file,
                a([
                    u(3),
                    u(35),
                    u(expected_root_generation),
                    u(expected_config_revision),
                    b(state::parse_id(&expected_config_hash)?),
                    b(raw),
                ]),
            )
            .await?
        }
    };
    println!("{}", json(&result));
    Ok(())
}
