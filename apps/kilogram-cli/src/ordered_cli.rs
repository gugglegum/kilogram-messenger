//! Thin CLI bridge; OD1 never enters legacy repository preparation.
use anyhow::{Context, Result, bail, ensure};
use clap::Subcommand;
use kilogram_cli::ordered_dialog::{admin, engine, state};
use kilogram_identity::{AccountAuthoritySnapshot, AccountId};
use kilogram_protocol::ordered_dialog_v1::{self as wire, Value, a, b, u};
use kilogram_runtime_ipc::{RuntimeIpcCommand, RuntimeIpcResponse};
use kilogram_state::{EncryptedStateVault, OrderedMutation, OrderedVault, StateDirectoryLock};
use std::path::{Path, PathBuf};

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(flatten)]
    Runtime2(crate::ordered2_cli::Command),
    Shutdown {
        #[arg(long)]
        ipc_file: PathBuf,
    },
    Inspect {
        #[arg(long)]
        state_dir: PathBuf,
    },
    CredentialsExport {
        #[arg(long)]
        state_dir: PathBuf,
        #[arg(long)]
        output_file: PathBuf,
    },
    ContextCreate {
        #[arg(long)]
        state_dir: PathBuf,
        #[arg(long)]
        account_dir: PathBuf,
        #[arg(long)]
        peer_credentials_file: PathBuf,
        #[arg(long)]
        expect_account: AccountId,
        #[arg(long)]
        output_file: PathBuf,
    },
    ContextImport {
        #[arg(long)]
        state_dir: PathBuf,
        #[arg(long)]
        account_dir: Option<PathBuf>,
        #[arg(long)]
        invitation_file: PathBuf,
        #[arg(long)]
        expect_account: AccountId,
        #[arg(long)]
        output_file: Option<PathBuf>,
    },
    AuthorityUpdate {
        #[arg(long)]
        state_dir: PathBuf,
        #[arg(long)]
        snapshot_file: PathBuf,
    },
    AuthorityImport {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        snapshot_file: PathBuf,
    },
    LocatorImport {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        context: String,
        #[arg(long)]
        locator_file: PathBuf,
    },
    LocatorExport {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        context: String,
        #[arg(long)]
        output_file: PathBuf,
        #[arg(long)]
        refresh: bool,
    },
    Queue {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        context: String,
        /// Stable 16-byte hexadecimal logical request ID. Reuse it for retry.
        #[arg(long)]
        request_id: String,
        #[arg(long)]
        text: String,
    },
    Status {
        #[arg(long)]
        ipc_file: PathBuf,
    },
    History {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        context: String,
        #[arg(long, default_value_t = 0)]
        offset: usize,
        #[arg(long, default_value_t = 8)]
        limit: usize,
    },
    Open {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        context: String,
    },
    Sync {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        context: String,
    },
}
impl Command {
    pub fn state_directory(&self) -> Option<&Path> {
        match self {
            Self::Runtime2(command) => command.state_directory(),
            Self::CredentialsExport { state_dir, .. }
            | Self::Inspect { state_dir }
            | Self::ContextCreate { state_dir, .. }
            | Self::ContextImport { state_dir, .. }
            | Self::AuthorityUpdate { state_dir, .. } => Some(state_dir),
            _ => None,
        }
    }
}
pub fn reject_legacy(root: &Path) -> Result<()> {
    let fence = root.join("ratchet/account.pickle");
    if fence.exists() {
        ensure!(
            std::fs::read(fence)? != kilogram_state::ORDERED_ACCOUNT_FENCE,
            "UnsupportedProfile: ordered-dialog-v1 requires an explicit ordered writer"
        );
    }
    if EncryptedStateVault::is_initialized(root)? {
        ensure!(
            !EncryptedStateVault::open_existing(root)?.is_ordered_profile()?,
            "UnsupportedProfile: schema4 forbids legacy writers"
        );
    }
    Ok(())
}
pub fn is_ordered_launch(path: &Path) -> Result<bool> {
    let raw = admin::read_public(path, 64 * 1024)?;
    let json: serde_json::Value = serde_json::from_slice(&raw)?;
    match json.get("profile_mode") {
        None => Ok(false),
        Some(mode) if mode.as_str() == Some("ordered-dialog-v1") => Ok(true),
        Some(_) => bail!("UnsupportedProfile: unknown explicit profile mode"),
    }
}
fn json(value: &Value) -> serde_json::Value {
    match value {
        Value::U(n) => (*n).into(),
        Value::B(raw) => state::hex(raw).into(),
        Value::A(items) => items.iter().map(json).collect(),
    }
}
async fn ipc(path: &Path, value: Value) -> Result<Value> {
    let request = wire::encode(&value, 32 * 1024)?;
    match kilogram_runtime_ipc::call(path, RuntimeIpcCommand::OrderedDialog { request }).await? {
        RuntimeIpcResponse::OrderedDialog { response } => Ok(wire::decode(&response, 48 * 1024)?),
        RuntimeIpcResponse::Error { message } => bail!("{message}"),
        _ => bail!("unexpected IPC response"),
    }
}
pub async fn run(command: Command, lease: Option<StateDirectoryLock>) -> Result<()> {
    if let Command::Runtime2(command) = command {
        return crate::ordered2_cli::run(command, lease).await;
    }
    let mut store = lease.map(OrderedVault::open).transpose()?;
    match command {
        Command::Runtime2(_) => unreachable!("runtime2 dispatch before V4 open"),
        Command::Shutdown { ipc_file } => {
            ensure!(
                matches!(
                    kilogram_runtime_ipc::call(&ipc_file, RuntimeIpcCommand::Shutdown).await?,
                    RuntimeIpcResponse::ShutdownAccepted
                ),
                "unexpected shutdown response"
            );
            println!("status=shutdown-accepted");
        }
        Command::Inspect { .. } => {
            let engine = kilogram_cli::ordered_dialog::engine::Engine::new(
                store.take().context("root lease")?,
            )?;
            let rows=engine.contexts.keys().map(|c|->Result<serde_json::Value>{
                let head=engine.head(*c)?;
                Ok(serde_json::json!({"context":state::hex(c),"phase":head.phase,"native_generation":head.native_generation,
                    "d":head.d.map(|id|state::hex(&id)),"e":head.e.map(|id|state::hex(&id)),"sid":head.sid,
                    "offer":head.offer.map(|id|state::hex(&id)),"grant":head.grant.map(|id|state::hex(&id))}))
            }).collect::<Result<Vec<_>>>()?;
            println!(
                "{}",
                serde_json::json!({"profile_mode":"ordered-dialog-v1","contexts":rows})
            );
        }
        Command::CredentialsExport { output_file, .. } => {
            let store = store.as_ref().context("root lease")?;
            admin::write_public(&output_file, &admin::credentials(store)?.raw, store.root())?;
            println!("status=credentials-exported");
        }
        Command::ContextCreate {
            account_dir,
            peer_credentials_file,
            expect_account,
            output_file,
            ..
        } => {
            let store = store.as_ref().context("root lease")?;
            let (c, raw) = admin::create_context(
                store,
                &account_dir,
                &admin::read_public(&peer_credentials_file, wire::MAX_AUTH)?,
                expect_account,
                engine::now()?,
            )?;
            admin::write_public(&output_file, &raw, store.root())?;
            println!("context={}", state::hex(&c));
        }
        Command::ContextImport {
            account_dir,
            invitation_file,
            expect_account,
            output_file,
            ..
        } => {
            let store = store.as_mut().context("root lease")?;
            let (c, raw) = admin::import_context(
                store,
                account_dir.as_deref(),
                &admin::read_public(&invitation_file, 96 * 1024)?,
                expect_account,
                engine::now()?,
            )?;
            if let Some(path) = output_file {
                admin::write_public(&path, &raw, store.root())?;
            }
            println!("context={}", state::hex(&c));
            println!("status=context-imported");
        }
        Command::AuthorityUpdate { snapshot_file, .. } => {
            let store = store.as_mut().context("root lease")?;
            let snapshot = AccountAuthoritySnapshot::decode_and_verify(&admin::read_public(
                &snapshot_file,
                wire::MAX_AUTH,
            )?)?;
            let own = admin::claim(store)?.1;
            let path = if snapshot.account_id() == own {
                "account-authority.snapshot".to_owned()
            } else {
                ensure!(
                    admin::specs(store)?.iter().any(|s| s
                        .peers
                        .iter()
                        .any(|p| p.cert.account_id() == snapshot.account_id())),
                    "unpinned authority account"
                );
                format!("peer-authority/{}.snapshot", snapshot.account_id())
            };
            let prior =
                AccountAuthoritySnapshot::decode_and_verify(&admin::required(store, &path)?)?;
            ensure!(
                snapshot.revision() >= prior.revision(),
                "authority rollback"
            );
            ensure!(
                snapshot.revision() != prior.revision() || snapshot.encode()? == prior.encode()?,
                "authority revision conflict"
            );
            admin::commit(store, vec![OrderedMutation::put(path, snapshot.encode()?)])?;
            println!("status=authority-updated");
        }
        Command::LocatorImport {
            ipc_file,
            context,
            locator_file,
        } => {
            let c = state::parse_id(&context)?;
            let absolute = std::fs::canonicalize(locator_file)?;
            let result = ipc(
                &ipc_file,
                a([
                    u(4),
                    b(c),
                    b(absolute.to_str().context("locator UTF8 path")?.as_bytes()),
                ]),
            )
            .await?;
            println!("{}", json(&result));
        }
        Command::AuthorityImport {
            ipc_file,
            snapshot_file,
        } => {
            let absolute = std::fs::canonicalize(snapshot_file)?;
            let result = ipc(
                &ipc_file,
                a([
                    u(8),
                    b(absolute.to_str().context("snapshot UTF8 path")?.as_bytes()),
                ]),
            )
            .await?;
            println!("{}", json(&result));
        }
        Command::LocatorExport {
            ipc_file,
            context,
            output_file,
            refresh,
        } => {
            let absolute = if output_file.is_absolute() {
                output_file
            } else {
                std::env::current_dir()?.join(output_file)
            };
            let _result = ipc(
                &ipc_file,
                a([
                    u(if refresh { 7 } else { 5 }),
                    b(state::parse_id(&context)?),
                    b(absolute.to_str().context("export UTF8 path")?.as_bytes()),
                ]),
            )
            .await?;
            println!("status=locator-exported");
        }
        Command::Queue {
            ipc_file,
            context,
            request_id,
            text,
        } => {
            ensure!(
                request_id.len() == 32 && request_id.is_ascii(),
                "request ID must be 32 hexadecimal characters"
            );
            let mut id = [0; 16];
            for (i, x) in id.iter_mut().enumerate() {
                *x = u8::from_str_radix(&request_id[2 * i..2 * i + 2], 16)?;
            }
            let result = ipc(
                &ipc_file,
                a([
                    u(2),
                    b(state::parse_id(&context)?),
                    b(id),
                    b(text.as_bytes()),
                ]),
            )
            .await?;
            println!("{}", json(&result));
        }
        Command::Status { ipc_file } => {
            let result = ipc(&ipc_file, a([u(1)])).await?;
            let f = result.fields(3)?;
            let rows=f[2].list()?.iter().map(|r|->Result<serde_json::Value>{
                let r=r.fields(9)?;let counts=r[4].fields(4)?;let credit=r[8].fields(4)?;
                Ok(serde_json::json!({"context":state::hex(r[0].bytes()?),"phase":r[1].uint()?,"native_generation":r[2].uint()?,"session_id":r[3].option()?.map(|v|v.text()).transpose()?,"send_next":counts[0].uint()?,"receive_next":counts[1].uint()?,"own_generation":counts[2].uint()?,"peer_generation":counts[3].uint()?,"pending":r[5].uint()?,"unacked":r[6].uint()?,"status":r[7].text()?,"ordinary_pending":credit[0].uint()?,"ordinary_pending_bytes":credit[1].uint()?,"head_used":credit[2].uint()?,"head_credit_bytes":credit[3].uint()?}))
            }).collect::<Result<Vec<_>>>()?;
            println!(
                "{}",
                serde_json::json!({"profile_mode":"ordered-dialog-v1","commits":f[1].uint()?,"contexts":rows})
            );
        }
        Command::History {
            ipc_file,
            context,
            offset,
            limit,
        } => {
            let result = ipc(
                &ipc_file,
                a([
                    u(3),
                    b(state::parse_id(&context)?),
                    u(offset as u64),
                    u(limit as u64),
                ]),
            )
            .await?;
            let f = result.fields(4)?;
            let rows=f[3].list()?.iter().map(|r|->Result<serde_json::Value>{let r=r.fields(6)?;Ok(serde_json::json!({"event_id":state::hex(r[0].bytes()?),"role":r[1].uint()?,"ordinal":r[2].uint()?,"author_sequence":r[3].uint()?,"request_id":state::hex(r[4].bytes()?),"text":r[5].text()?}))}).collect::<Result<Vec<_>>>()?;
            println!(
                "{}",
                serde_json::json!({"profile_mode":"ordered-dialog-v1","total":f[1].uint()?,"offset":f[2].uint()?,"items":rows})
            );
        }
        Command::Open { ipc_file, context } | Command::Sync { ipc_file, context } => {
            let result = ipc(&ipc_file, a([u(6), b(state::parse_id(&context)?)])).await?;
            println!("{}", json(&result));
        }
    }
    Ok(())
}
