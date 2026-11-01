//! Explicit IPC3 bridge; no fallback to IPC2/legacy on an error.
use anyhow::{Context, Result, bail, ensure};
use clap::Subcommand;
use kilogram_cli::ordered_dialog::{admin, state};
use kilogram_protocol::ordered_dialog_v1::{self as wire, Value, a, b, opt, u};
use kilogram_runtime_ipc::{RuntimeIpcCommand, RuntimeIpcResponse};
use std::path::{Path, PathBuf};
#[derive(Debug, Subcommand)]
pub enum Command {
    ExistingOriginExport {
        #[arg(long)]
        profile: PathBuf,
        #[arg(long)]
        context: String,
        #[arg(long)]
        output_file: PathBuf,
    },
    Upgrade {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        slot: u8,
        #[arg(long)]
        request_id: String,
    },
    UpgradeStatus {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        slot: u8,
    },
    CancelIntent {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        slot: u8,
        #[arg(long)]
        request_id: String,
    },
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
    ensure!(
        text.len() == 32 && text.is_ascii(),
        "request ID16 hex bytes"
    );
    let mut out = [0; 16];
    for (i, v) in out.iter_mut().enumerate() {
        *v = u8::from_str_radix(&text[2 * i..2 * i + 2], 16)?;
    }
    Ok(out)
}
pub(crate) async fn call(path: &Path, body: Value) -> Result<Value> {
    let expected = body.list()?.get(1).context("IPC3 request tag")?.uint()?;
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
            ensure!(
                f[0].uint()? == 3 && f[1].uint()? == expected,
                "UnsupportedVersion/tag: expected matching IPC3 reply"
            );
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
        Command::ExistingOriginExport {
            profile,
            context,
            output_file,
        } => {
            use kilogram_cli::ordered_dialog::runtime7::Profile3;
            use kilogram_protocol::ordered_dialog_v2::VerifiedExistingScope1;
            // Deserialize the public configuration only. In particular, do not
            // call Profile3::open or acquire a live messenger's directory lock.
            let profile: Profile3 =
                serde_json::from_slice(&admin::read_public(&profile, 64 * 1024)?)?;
            ensure!(
                profile.version == 3 && profile.profile_mode == "ordered-dialog-indexed",
                "UnsupportedProfile3"
            );
            let c = state::parse_id(&context)?;
            let root = profile.state_dir.canonicalize()?;
            let directory = profile.locator_directory.canonicalize()?;
            ensure!(
                !directory.starts_with(&root),
                "public origin directory inside state"
            );
            let path = directory.join(state::hex(&c)).join("existing-origin.cbor");
            ensure!(
                !path.canonicalize()?.starts_with(&root),
                "public origin artifact inside state"
            );
            let raw = admin::read_public(
                &path,
                kilogram_protocol::ordered_dialog_v2::MAX_EXISTING_ORIGIN,
            )?;
            let scope = VerifiedExistingScope1::verify_export(&raw)?;
            ensure!(scope.c() == c, "existing-origin context mismatch");
            let device = state::parse_id(&profile.device)?;
            let account = state::parse_id(&profile.account)?;
            ensure!(
                scope
                    .peers()
                    .iter()
                    .any(|peer| peer.certificate().device_id().as_bytes() == &device
                        && peer.certificate().account_id().as_bytes() == &account),
                "existing-origin original profile owner pin"
            );
            admin::write_public(&output_file, &raw, &root)?;
            println!("status=existing-origin-exported");
            return Ok(());
        }
        Command::Upgrade {
            ipc_file,
            slot,
            request_id,
        } => {
            call(
                &ipc_file,
                a([u(3), u(8), u(slot.into()), b(request(&request_id)?)]),
            )
            .await?
        }
        Command::UpgradeStatus { ipc_file, slot } => {
            let result = call(&ipc_file, a([u(3), u(9), u(slot.into())])).await?;
            println!("{}", std::str::from_utf8(result.bytes()?)?);
            return Ok(());
        }
        Command::CancelIntent {
            ipc_file,
            slot,
            request_id,
        } => {
            call(
                &ipc_file,
                a([u(3), u(10), u(slot.into()), b(request(&request_id)?)]),
            )
            .await?
        }
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
