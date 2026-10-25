//! Explicit mode2 CLI; production codecs/owner are shared with network and process tests.
use anyhow::{Context, Result, bail, ensure};
use clap::{Args, Subcommand};
use kilogram_cli::ordered_dialog::{
    admin as shared,
    runtime2::{self, config::Profile2},
    state::{hex, parse_id},
};
use kilogram_identity::{AccountId, DeviceId};
use kilogram_protocol::ordered_dialog_v2::{self as wire, Value, a, b, opt, u};
use kilogram_runtime_ipc::{RuntimeIpcCommand, RuntimeIpcResponse};
use kilogram_state::{StateDirectoryLock, indexed_dialog as db};
use std::path::{Path, PathBuf};

#[derive(Debug, Subcommand)]
pub enum Command {
    Consent2Create(Ceremony2),
    Context2Create(Ceremony2),
    Context2Import {
        #[arg(long)]
        state_dir: PathBuf,
        #[arg(long)]
        expect_account: AccountId,
        #[arg(long)]
        expect_claim: String,
        #[arg(long)]
        invitation_file: PathBuf,
    },
    Context2ImportLive {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        invitation_file: PathBuf,
    },
    Enrollment2Resume {
        #[arg(long)]
        state_dir: PathBuf,
        #[arg(long)]
        expect_account: AccountId,
        #[arg(long)]
        expect_claim: String,
        #[arg(long)]
        expect_device: DeviceId,
        #[arg(long)]
        certificate_file: PathBuf,
    },
    Send {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        slot: u8,
        #[arg(long)]
        request_id: String,
        #[arg(long)]
        text: String,
    },
    RequestStatus {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        slot: u8,
        #[arg(long)]
        request_id: String,
    },
    Status2 {
        #[arg(long)]
        ipc_file: PathBuf,
    },
    History2 {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        slot: u8,
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long, default_value_t = 16)]
        limit: usize,
    },
    Locator2Import {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        slot: u8,
        #[arg(long)]
        locator_file: PathBuf,
    },
    Locator2Export {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        slot: u8,
        #[arg(long)]
        output_file: PathBuf,
    },
    Authority2Import {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        snapshot_file: PathBuf,
    },
    Consent2Import {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        slot: u8,
        #[arg(long)]
        consent_file: PathBuf,
        #[arg(long)]
        expected_current: String,
        #[arg(long)]
        historical: bool,
    },
    Sync2 {
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        slot: u8,
    },
    StorageUpgrade {
        #[command(flatten)]
        access: StorageAccess,
        #[arg(long)]
        config_file: PathBuf,
        #[arg(long, conflicts_with = "resume", required_unless_present = "resume")]
        check: bool,
        #[arg(long, conflicts_with = "check", required_unless_present = "check")]
        resume: bool,
        #[arg(long)]
        expected_profile_hash: Option<String>,
    },
    StorageUpgradeStatus(StorageAccess),
    StorageUpgradePause(StorageAccess),
    StorageHistory {
        #[command(flatten)]
        access: StorageAccess,
        #[arg(long)]
        context: String,
        #[arg(long, default_value_t = 0)]
        offset: usize,
        #[arg(long, default_value_t = 8)]
        limit: usize,
    },
    StorageIncreaseCapacity {
        #[command(flatten)]
        access: StorageAccess,
        #[arg(long)]
        config_file: PathBuf,
        #[arg(long)]
        expected_root_generation: u64,
        #[arg(long)]
        expected_config_revision: u64,
        #[arg(long)]
        expected_config_hash: String,
    },
    Upgrade {
        #[arg(long)]
        state_dir: PathBuf,
    },
}
#[derive(Debug, Args)]
pub struct StorageAccess {
    #[arg(long)]
    state_dir: PathBuf,
    #[arg(long)]
    expect_account: AccountId,
    #[arg(long)]
    expect_device: DeviceId,
}
#[derive(Debug, Args)]
pub struct Ceremony2 {
    #[arg(long)]
    context: String,
    #[arg(long = "credentials-file", num_args = 2)]
    credentials: Vec<PathBuf>,
    #[arg(long = "account-dir", num_args = 2)]
    accounts: Vec<PathBuf>,
    #[arg(long)]
    not_before: Option<u64>,
    #[arg(long,default_value_t=30*86400)]
    valid_for_seconds: u64,
    #[arg(long)]
    output_file: PathBuf,
}
impl Command {
    pub fn state_directory(&self) -> Option<&Path> {
        match self {
            Self::Context2Import { state_dir, .. } | Self::Enrollment2Resume { state_dir, .. } => {
                Some(state_dir)
            }
            Self::StorageUpgrade { access, .. }
            | Self::StorageUpgradeStatus(access)
            | Self::StorageUpgradePause(access)
            | Self::StorageHistory { access, .. }
            | Self::StorageIncreaseCapacity { access, .. } => Some(&access.state_dir),
            _ => None,
        }
    }
}
pub fn is_launch(path: &Path) -> Result<bool> {
    let raw = shared::read_public(path, 64 * 1024)?;
    let value: serde_json::Value = serde_json::from_slice(&raw)?;
    Ok(value.get("profile_mode").and_then(|v| v.as_str()) == Some("ordered-dialog-v2"))
}
pub struct CreateProfile2 {
    pub state_dir: PathBuf,
    pub profile_file: PathBuf,
    pub ipc_file: PathBuf,
    pub locator_directory: PathBuf,
    pub enabled: Vec<String>,
    pub account: AccountId,
    pub claim: String,
    pub device: DeviceId,
    pub route: kilogram_transport_iroh::RoutePolicy,
    pub relay: Option<String>,
    pub bind: String,
}
pub fn create_profile(input: CreateProfile2) -> Result<()> {
    let CreateProfile2 {
        state_dir,
        profile_file,
        ipc_file,
        locator_directory,
        enabled,
        account,
        claim,
        device,
        route,
        relay,
        bind,
    } = input;
    let owner = db::ExpectedOwner {
        account: *account.as_bytes(),
        claim: parse_id(&claim)?,
    };
    let mut repository = db::RuntimeRepository::open(
        StateDirectoryLock::acquire(&state_dir)?,
        owner,
        db::Config::default(),
    )?;
    let p = repository.profile()?;
    let contexts = p.contexts.map(|c| hex(&c));
    let enabled = enabled
        .iter()
        .map(|id| {
            p.contexts
                .iter()
                .position(|c| hex(c) == *id)
                .map(|n| n as u8)
                .context("enabled C outside committed slots")
        })
        .collect::<Result<Vec<_>>>()?;
    drop(repository);
    runtime2::admin::create_profile(
        Profile2 {
            version: 2,
            profile_mode: "ordered-dialog-v2".into(),
            state_dir,
            account: account.to_string(),
            claim,
            device: device.to_string(),
            contexts,
            enabled,
            ipc_file,
            locator_directory,
            route: kilogram_transport_iroh::ordered2::RouteConfig2 {
                policy: route,
                bind: if route == kilogram_transport_iroh::RoutePolicy::RelayOnly {
                    None
                } else {
                    Some(bind.parse()?)
                },
                relay,
            },
            logical_quota: db::Config::default().logical_quota,
            physical_quota: db::Config::default().physical_quota,
        },
        &profile_file,
    )
}
fn request_id(raw: &str) -> Result<[u8; 16]> {
    ensure!(
        raw.len() == 32 && raw.is_ascii(),
        "request ID must be 32 hexadecimal chars"
    );
    let mut id = [0; 16];
    for (i, b) in id.iter_mut().enumerate() {
        *b = u8::from_str_radix(&raw[2 * i..2 * i + 2], 16)?;
    }
    Ok(id)
}
pub async fn call(path: &Path, body: Value) -> Result<Value> {
    let request = wire::encode(&body, 32 * 1024)?;
    match kilogram_runtime_ipc::call(path, RuntimeIpcCommand::OrderedDialog { request }).await? {
        RuntimeIpcResponse::OrderedDialog { response } => {
            let value = wire::decode(&response, 48 * 1024)?;
            ensure!(value.fields(3)?[0].uint()? == 2, "IPC reply version");
            Ok(value.fields(3)?[2].clone())
        }
        RuntimeIpcResponse::Error { message } => bail!("{message}"),
        _ => bail!("unexpected IPC response"),
    }
}
fn json(value: &Value) -> serde_json::Value {
    match value {
        Value::U(n) => (*n).into(),
        Value::B(raw) => hex(raw).into(),
        Value::A(v) => v.iter().map(json).collect(),
    }
}
pub async fn run(command: Command, lease: Option<StateDirectoryLock>) -> Result<()> {
    use kilogram_cli::ordered_dialog::storage_migration as migration;
    let command = match command {
        Command::StorageUpgrade {
            access,
            config_file,
            check,
            expected_profile_hash,
            ..
        } => {
            let config = migration::load_config(&config_file)?;
            let output = migration::upgrade(
                lease.context("migration lifetime lease")?,
                access.expect_account,
                access.expect_device,
                &config,
                check,
                expected_profile_hash.as_deref().map(parse_id).transpose()?,
            )?;
            println!("{output}");
            return Ok(());
        }
        Command::StorageUpgradeStatus(access) => {
            println!(
                "{}",
                migration::status(
                    lease.context("migration lease")?,
                    access.expect_account,
                    access.expect_device
                )?
            );
            return Ok(());
        }
        Command::StorageUpgradePause(access) => {
            println!(
                "{}",
                migration::pause(
                    lease.context("migration lease")?,
                    access.expect_account,
                    access.expect_device
                )?
            );
            return Ok(());
        }
        Command::StorageHistory {
            access,
            context,
            offset,
            limit,
        } => {
            println!(
                "{}",
                migration::history(
                    lease.context("migration lease")?,
                    access.expect_account,
                    access.expect_device,
                    parse_id(&context)?,
                    offset,
                    limit
                )?
            );
            return Ok(());
        }
        Command::StorageIncreaseCapacity {
            access,
            config_file,
            expected_root_generation,
            expected_config_revision,
            expected_config_hash,
        } => {
            let config = migration::load_config(&config_file)?;
            println!(
                "{}",
                migration::increase_capacity(
                    lease.context("migration lease")?,
                    access.expect_account,
                    access.expect_device,
                    &config,
                    expected_root_generation,
                    expected_config_revision,
                    parse_id(&expected_config_hash)?
                )?
            );
            return Ok(());
        }
        other => other,
    };
    if let Command::Consent2Create(args) | Command::Context2Create(args) = &command {
        ensure!(
            args.credentials.len() == 2 && args.accounts.len() == 2,
            "two credential files and Root directories required"
        );
        let nb = args.not_before.unwrap_or(runtime2::now()?);
        let expires = nb
            .checked_add(args.valid_for_seconds)
            .context("consent interval")?;
        let artifact = runtime2::admin::context_artifact(
            parse_id(&args.context)?,
            [&args.credentials[0], &args.credentials[1]],
            [&args.accounts[0], &args.accounts[1]],
            nb,
            expires,
        )?;
        let output = if matches!(&command, Command::Consent2Create(_)) {
            wire::decode(&artifact, 96 * 1024)?.fields(6)?[5]
                .bytes()?
                .to_vec()
        } else {
            artifact
        };
        std::fs::create_dir_all(args.output_file.parent().context("output parent")?)?;
        shared::write_public(
            &args.output_file,
            &output,
            &args.accounts[0].canonicalize()?,
        )?;
        return Ok(());
    }
    let result = match command {
        Command::Consent2Create(_) | Command::Context2Create(_) => {
            unreachable!("ceremony dispatched first")
        }
        Command::Context2Import {
            expect_account,
            expect_claim,
            invitation_file,
            ..
        } => {
            let mut repository = db::RuntimeRepository::open(
                lease.context("root lease")?,
                db::ExpectedOwner {
                    account: *expect_account.as_bytes(),
                    claim: parse_id(&expect_claim)?,
                },
                db::Config::default(),
            )?;
            runtime2::admin::install(
                &mut repository,
                &shared::read_public(&invitation_file, 96 * 1024)?,
            )?;
            return Ok(());
        }
        Command::Enrollment2Resume {
            expect_account,
            expect_claim,
            expect_device,
            certificate_file,
            ..
        } => {
            let mut repository = db::RuntimeRepository::open(
                lease.context("root lease")?,
                db::ExpectedOwner {
                    account: *expect_account.as_bytes(),
                    claim: parse_id(&expect_claim)?,
                },
                db::Config::default(),
            )?;
            runtime2::admin::resume_enrollment(
                &mut repository,
                *expect_device.as_bytes(),
                &certificate_file,
            )?;
            return Ok(());
        }
        Command::Context2ImportLive {
            ipc_file,
            invitation_file,
        } => {
            let raw = shared::read_public(&invitation_file, 96 * 1024)?;
            let id = call(
                &ipc_file,
                a([u(2), u(12), u(raw.len() as u64), b(wire::hash(b"", &raw))]),
            )
            .await?
            .fixed::<32>()?;
            for (index, part) in raw.chunks(16 * 1024).enumerate() {
                call(
                    &ipc_file,
                    a([u(2), u(13), b(id), u((index * 16 * 1024) as u64), b(part)]),
                )
                .await?;
            }
            call(&ipc_file, a([u(2), u(14), b(id)])).await?
        }
        Command::Send {
            ipc_file,
            slot,
            request_id: request,
            text,
        } => {
            call(
                &ipc_file,
                a([
                    u(2),
                    u(2),
                    u(u64::from(slot)),
                    b(request_id(&request)?),
                    b(text.as_bytes()),
                ]),
            )
            .await?
        }
        Command::RequestStatus {
            ipc_file,
            slot,
            request_id: request,
        } => {
            call(
                &ipc_file,
                a([u(2), u(3), u(u64::from(slot)), b(request_id(&request)?)]),
            )
            .await?
        }
        Command::Status2 { ipc_file } => {
            let result = call(&ipc_file, a([u(2), u(1)])).await?;
            println!("{}", std::str::from_utf8(result.bytes()?)?);
            return Ok(());
        }
        Command::History2 {
            ipc_file,
            slot,
            cursor,
            limit,
        } => {
            let cursor = cursor
                .map(|c| db::HistoryCursor::from_base64(&c))
                .transpose()?;
            let result = call(
                &ipc_file,
                a([
                    u(2),
                    u(4),
                    u(u64::from(slot)),
                    opt(cursor.as_ref().map(|c| b(c.as_bytes()))),
                    u(limit as u64),
                ]),
            )
            .await?;
            println!("{}", std::str::from_utf8(result.bytes()?)?);
            return Ok(());
        }
        Command::Locator2Import {
            ipc_file,
            slot,
            locator_file,
        } => {
            call(
                &ipc_file,
                a([
                    u(2),
                    u(5),
                    u(u64::from(slot)),
                    b(shared::read_public(&locator_file, 8192)?),
                ]),
            )
            .await?
        }
        Command::Locator2Export {
            ipc_file,
            slot,
            output_file,
        } => {
            let result = call(&ipc_file, a([u(2), u(6), u(u64::from(slot))])).await?;
            call(
                &ipc_file,
                a([
                    u(2),
                    u(15),
                    u(u64::from(slot)),
                    b(output_file.to_str().context("UTF-8 locator output path")?),
                    b(wire::hash(b"", result.bytes()?)),
                ]),
            )
            .await?;
            return Ok(());
        }
        Command::Authority2Import {
            ipc_file,
            snapshot_file,
        } => {
            call(
                &ipc_file,
                a([
                    u(2),
                    u(8),
                    b(shared::read_public(&snapshot_file, 16 * 1024)?),
                ]),
            )
            .await?
        }
        Command::Consent2Import {
            ipc_file,
            slot,
            consent_file,
            expected_current,
            historical,
        } => {
            call(
                &ipc_file,
                a([
                    u(2),
                    u(9),
                    u(u64::from(slot)),
                    b(parse_id(&expected_current)?),
                    b(shared::read_public(&consent_file, 4096)?),
                    u(u64::from(historical)),
                ]),
            )
            .await?
        }
        Command::Sync2 { ipc_file, slot } => {
            call(&ipc_file, a([u(2), u(7), u(u64::from(slot))])).await?
        }
        Command::StorageUpgrade { .. }
        | Command::StorageUpgradeStatus(_)
        | Command::StorageUpgradePause(_)
        | Command::StorageHistory { .. }
        | Command::StorageIncreaseCapacity { .. } => {
            unreachable!("offline migration dispatched under original lease")
        }
        Command::Upgrade { .. } => bail!("UnsupportedHandoff: successor S5"),
    };
    println!("{}", json(&result));
    Ok(())
}
