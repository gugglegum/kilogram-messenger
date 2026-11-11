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
    #[command(flatten)]
    Runtime3(crate::ordered3_cli::Command),
    Consent1RenewPropose {
        #[command(flatten)]
        renewal: Box<RenewalArgs>,
        #[arg(long)]
        not_before: Option<u64>,
        #[arg(long, default_value_t = 30 * 86400)]
        valid_for_seconds: u64,
        #[arg(long)]
        output_file: PathBuf,
    },
    Consent1RenewCosign {
        #[command(flatten)]
        renewal: Box<RenewalArgs>,
        #[arg(long)]
        proposal_file: PathBuf,
        #[arg(long)]
        output_invitation_file: PathBuf,
        #[arg(long)]
        output_consent_file: PathBuf,
    },
    Consent2Create(Box<Ceremony2>),
    Context2Create(Box<Ceremony2>),
    /// Propose Fresh Consent2 using only canonical role0's own offline Root.
    Context2Propose {
        #[command(flatten)]
        fresh: Box<FreshConsentArgs>,
        #[arg(long)]
        not_before: Option<u64>,
        #[arg(long, default_value_t = 30 * 86400)]
        valid_for_seconds: u64,
    },
    /// Verify a Fresh proposal and cosign using only role1's own offline Root.
    Context2Cosign {
        #[command(flatten)]
        fresh: Box<FreshConsentArgs>,
        #[arg(long)]
        proposal_file: PathBuf,
    },
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
        existing: bool,
        #[arg(long)]
        ipc_file: PathBuf,
        #[arg(long)]
        slot: u8,
        #[arg(long)]
        locator_file: PathBuf,
    },
    Locator2Export {
        #[arg(long)]
        existing: bool,
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
        existing: bool,
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
}
#[derive(Debug, Args)]
pub struct RenewalArgs {
    #[arg(long)]
    existing_invitation_file: PathBuf,
    #[arg(long)]
    expect_context: String,
    #[arg(long)]
    expect_account: AccountId,
    #[arg(long)]
    expect_peer_account: AccountId,
    #[arg(long)]
    authority_a_file: PathBuf,
    #[arg(long)]
    authority_b_file: PathBuf,
    #[arg(long)]
    minimum_authority_a_file: PathBuf,
    #[arg(long)]
    minimum_authority_b_file: PathBuf,
    #[arg(long)]
    account_dir: PathBuf,
}
#[derive(Debug, Args)]
pub struct FreshConsentArgs {
    #[arg(long)]
    account_dir: PathBuf,
    #[arg(long)]
    expect_account: AccountId,
    #[arg(long)]
    expect_peer_account: AccountId,
    #[arg(long)]
    expect_device: DeviceId,
    #[arg(long)]
    expect_peer_device: DeviceId,
    #[arg(long)]
    context: String,
    #[arg(long = "credentials-file", num_args = 2, required = true)]
    credentials: Vec<PathBuf>,
    #[arg(long = "minimum-authority-file", num_args = 2, required = true)]
    minimum: Vec<PathBuf>,
    #[arg(long)]
    output_file: PathBuf,
}
fn fresh_consent(
    args: &FreshConsentArgs,
    proposal: Option<&Path>,
    not_before: Option<u64>,
    valid_for: u64,
) -> Result<()> {
    use runtime2::admin::fresh_consent as fresh;
    use std::io::Write;
    ensure!(
        args.credentials.len() == 2 && args.minimum.len() == 2,
        "two public credentials and authority floors required"
    );
    let credentials = [
        shared::read_public(&args.credentials[0], wire::MAX_AUTH)?,
        shared::read_public(&args.credentials[1], wire::MAX_AUTH)?,
    ];
    let minimum = [
        shared::read_public(&args.minimum[0], wire::MAX_AUTH)?,
        shared::read_public(&args.minimum[1], wire::MAX_AUTH)?,
    ];
    let input = fresh::Inputs {
        credentials: credentials.each_ref().map(|r| r.as_slice()),
        minimum_authorities: minimum.each_ref().map(|r| r.as_slice()),
        context: parse_id(&args.context)?,
        account: args.expect_account,
        peer: args.expect_peer_account,
        device: args.expect_device,
        peer_device: args.expect_peer_device,
    };
    let proposal = proposal
        .map(|p| shared::read_public(p, fresh::MAX_PROPOSAL))
        .transpose()?;
    let root = args.account_dir.canonicalize()?;
    let output = if args.output_file.is_absolute() {
        args.output_file.clone()
    } else {
        std::env::current_dir()?.join(&args.output_file)
    };
    let parent = output
        .parent()
        .context("Fresh public output parent")?
        .canonicalize()?;
    ensure!(!parent.starts_with(&root), "Fresh output inside own Root");
    // Reserve before signing/membership mutation. An error leaves this empty
    // public evidence file; retry with a fresh output name, never overwrite it.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(output.file_name().context("Fresh output name")?))?;
    let now = runtime2::now()?;
    let raw = match proposal {
        Some(raw) => fresh::cosign(input, &raw, &args.account_dir, now)?,
        None => fresh::propose(
            input,
            &args.account_dir,
            not_before.unwrap_or(now),
            valid_for,
            now,
        )?,
    };
    file.write_all(&raw)?;
    file.sync_all()?;
    Ok(())
}
struct RenewalPublic {
    original: Vec<u8>,
    authorities: [Vec<u8>; 2],
    minimum: [Vec<u8>; 2],
    context: db::Id,
}
impl RenewalPublic {
    fn load(args: &RenewalArgs) -> Result<Self> {
        Ok(Self {
            original: shared::read_public(&args.existing_invitation_file, 96 * 1024)?,
            authorities: [
                shared::read_public(&args.authority_a_file, 16 * 1024)?,
                shared::read_public(&args.authority_b_file, 16 * 1024)?,
            ],
            minimum: [
                shared::read_public(&args.minimum_authority_a_file, 16 * 1024)?,
                shared::read_public(&args.minimum_authority_b_file, 16 * 1024)?,
            ],
            context: parse_id(&args.expect_context)?,
        })
    }
    fn inputs<'a>(
        &'a self,
        args: &RenewalArgs,
    ) -> kilogram_cli::ordered_dialog::consent1_admin::RenewalInputs<'a> {
        kilogram_cli::ordered_dialog::consent1_admin::RenewalInputs {
            original: &self.original,
            authorities: [&self.authorities[0], &self.authorities[1]],
            minimum_authorities: [&self.minimum[0], &self.minimum[1]],
            context: self.context,
            account: args.expect_account,
            peer: args.expect_peer_account,
        }
    }
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
    #[arg(long, default_value = "fresh", value_parser = ["fresh", "existing"])]
    origin: String,
    #[arg(long)]
    existing_origin_file: Option<PathBuf>,
    #[arg(long, value_parser = ["propose", "cosign"])]
    step: Option<String>,
    #[arg(long)]
    proposal_file: Option<PathBuf>,
    #[arg(long)]
    expect_account: Option<AccountId>,
    #[arg(long)]
    expect_peer_account: Option<AccountId>,
    #[arg(long)]
    authority_a_file: Option<PathBuf>,
    #[arg(long)]
    authority_b_file: Option<PathBuf>,
    #[arg(long)]
    minimum_authority_a_file: Option<PathBuf>,
    #[arg(long)]
    minimum_authority_b_file: Option<PathBuf>,
    #[arg(long = "credentials-file", num_args = 2)]
    credentials: Vec<PathBuf>,
    #[arg(long = "account-dir", num_args = 1..=2)]
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
    let opcode = body.list()?.get(1).context("IPC opcode")?.uint()?;
    decode_cli_reply(
        opcode,
        kilogram_runtime_ipc::call(path, RuntimeIpcCommand::OrderedDialog { request }).await?,
    )
}
fn decode_cli_reply(opcode: u64, response: RuntimeIpcResponse) -> Result<Value> {
    match response {
        RuntimeIpcResponse::Error { message } => bail!("{message}"),
        other => kilogram_runtime_ipc::ordered2::decode_reply(opcode, other),
    }
}

#[test]
fn cs32b_cli_runtime_refusal_preserves_message_and_strict_reply() -> Result<()> {
    let message = "request conflicts with an existing record";
    assert_eq!(
        decode_cli_reply(
            8,
            RuntimeIpcResponse::Error {
                message: message.into()
            }
        )
        .err()
        .context("CLI accepted a runtime refusal")?
        .to_string(),
        message
    );
    assert!(decode_cli_reply(8, RuntimeIpcResponse::ShutdownAccepted).is_err());
    let reply = |fields| -> Result<_> {
        Ok(RuntimeIpcResponse::OrderedDialog {
            response: wire::encode(&a(fields), 1024)?,
        })
    };
    for fields in [
        vec![u(1), u(8), u(42)],
        vec![u(2), u(9), u(42)],
        vec![u(2), u(8)],
    ] {
        assert!(decode_cli_reply(8, reply(fields)?).is_err());
    }
    assert_eq!(
        decode_cli_reply(8, reply(vec![u(2), u(8), u(42)])?)?.uint()?,
        42
    );
    Ok(())
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
        Command::Context2Propose {
            fresh,
            not_before,
            valid_for_seconds,
        } => {
            ensure!(
                lease.is_none(),
                "Fresh ceremony must not open a runtime store"
            );
            return fresh_consent(&fresh, None, not_before, valid_for_seconds);
        }
        Command::Context2Cosign {
            fresh,
            proposal_file,
        } => {
            ensure!(
                lease.is_none(),
                "Fresh ceremony must not open a runtime store"
            );
            return fresh_consent(&fresh, Some(&proposal_file), None, 0);
        }
        Command::Runtime3(command) => {
            ensure!(lease.is_none(), "IPC3 must use the held live owner");
            return crate::ordered3_cli::run(command).await;
        }
        Command::Consent1RenewPropose {
            renewal,
            not_before,
            valid_for_seconds,
            output_file,
        } => {
            ensure!(
                lease.is_none(),
                "offline renewal must not open a messenger vault"
            );
            let public = RenewalPublic::load(&renewal)?;
            let now = kilogram_cli::ordered_dialog::engine::now()?;
            let raw = kilogram_cli::ordered_dialog::consent1_admin::propose(
                public.inputs(&renewal),
                &renewal.account_dir,
                not_before.unwrap_or(now),
                valid_for_seconds,
                now,
            )?;
            shared::write_public(
                &output_file,
                &raw,
                &std::fs::canonicalize(&renewal.account_dir)?,
            )?;
            println!("status=consent1-renewal-proposed");
            return Ok(());
        }
        Command::Consent1RenewCosign {
            renewal,
            proposal_file,
            output_invitation_file,
            output_consent_file,
        } => {
            ensure!(
                lease.is_none(),
                "offline renewal must not open a messenger vault"
            );
            ensure!(
                output_invitation_file != output_consent_file,
                "distinct renewal outputs"
            );
            let public = RenewalPublic::load(&renewal)?;
            let completed = kilogram_cli::ordered_dialog::consent1_admin::cosign(
                public.inputs(&renewal),
                &shared::read_public(&proposal_file, 96 * 1024)?,
                &renewal.account_dir,
                kilogram_cli::ordered_dialog::engine::now()?,
            )?;
            let root = std::fs::canonicalize(&renewal.account_dir)?;
            shared::write_public(&output_invitation_file, &completed.invitation, &root)?;
            shared::write_public(&output_consent_file, &completed.consent, &root)?;
            println!("status=consent1-renewal-completed");
            return Ok(());
        }
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
        if args.origin == "existing" {
            ensure!(
                matches!(&command, Command::Consent2Create(_)),
                "existing requires consent2-create"
            );
            ensure!(
                lease.is_none(),
                "offline consent must not open a messenger vault"
            );
            ensure!(
                args.accounts.len() == 1 && args.credentials.is_empty(),
                "existing ceremony needs only own account-dir"
            );
            use kilogram_cli::ordered_dialog::runtime7::existing_admin::{
                self, ExistingConsentInputs,
            };
            let origin = shared::read_public(
                args.existing_origin_file
                    .as_ref()
                    .context("existing-origin-file required")?,
                wire::MAX_EXISTING_ORIGIN,
            )?;
            let authority_a = shared::read_public(
                args.authority_a_file
                    .as_ref()
                    .context("authority-a-file required")?,
                wire::MAX_AUTH,
            )?;
            let authority_b = shared::read_public(
                args.authority_b_file
                    .as_ref()
                    .context("authority-b-file required")?,
                wire::MAX_AUTH,
            )?;
            let minimum_a = shared::read_public(
                args.minimum_authority_a_file
                    .as_ref()
                    .context("minimum-authority-a-file required")?,
                wire::MAX_AUTH,
            )?;
            let minimum_b = shared::read_public(
                args.minimum_authority_b_file
                    .as_ref()
                    .context("minimum-authority-b-file required")?,
                wire::MAX_AUTH,
            )?;
            let inputs = ExistingConsentInputs {
                origin: &origin,
                authorities: [&authority_a, &authority_b],
                minimum_authorities: [&minimum_a, &minimum_b],
                context: parse_id(&args.context)?,
                account: args.expect_account.context("expect-account required")?,
                peer: args
                    .expect_peer_account
                    .context("expect-peer-account required")?,
            };
            let now = runtime2::now()?;
            let output = match args.step.as_deref().context("step required")? {
                "propose" => {
                    ensure!(
                        args.proposal_file.is_none(),
                        "proposal-file is only for cosign"
                    );
                    existing_admin::propose(
                        inputs,
                        &args.accounts[0],
                        args.not_before.unwrap_or(now),
                        args.valid_for_seconds,
                        now,
                    )?
                }
                "cosign" => {
                    ensure!(
                        args.not_before.is_none(),
                        "cosign interval comes from proposal"
                    );
                    let proposal = shared::read_public(
                        args.proposal_file
                            .as_ref()
                            .context("proposal-file required")?,
                        4096,
                    )?;
                    existing_admin::cosign(inputs, &proposal, &args.accounts[0], now)?
                }
                _ => bail!("unknown ceremony step"),
            };
            shared::write_public(
                &args.output_file,
                &output,
                &args.accounts[0].canonicalize()?,
            )?;
            return Ok(());
        }
        ensure!(
            args.existing_origin_file.is_none()
                && args.step.is_none()
                && args.proposal_file.is_none()
                && args.expect_account.is_none()
                && args.expect_peer_account.is_none()
                && args.authority_a_file.is_none()
                && args.authority_b_file.is_none()
                && args.minimum_authority_a_file.is_none()
                && args.minimum_authority_b_file.is_none(),
            "fresh ceremony rejects existing-only arguments"
        );
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
            existing,
            ipc_file,
            slot,
            locator_file,
        } => {
            if existing {
                let result = crate::ordered3_cli::call(
                    &ipc_file,
                    a([
                        u(3),
                        u(5),
                        u(slot.into()),
                        u(2),
                        u(0),
                        opt(Some(b(shared::read_public(&locator_file, 8192)?))),
                    ]),
                )
                .await?;
                println!("{}", json(&result));
                return Ok(());
            }
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
            existing,
            ipc_file,
            slot,
            output_file,
        } => {
            if existing {
                let result = crate::ordered3_cli::call(
                    &ipc_file,
                    a([u(3), u(5), u(slot.into()), u(2), u(1), opt(None)]),
                )
                .await?;
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(output_file)?;
                use std::io::Write;
                file.write_all(result.bytes()?)?;
                file.sync_all()?;
                return Ok(());
            }
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
            existing,
            ipc_file,
            slot,
            consent_file,
            expected_current,
            historical,
        } => {
            if existing {
                let expected = if expected_current == "absent" {
                    None
                } else {
                    Some(parse_id(&expected_current)?)
                };
                let result = crate::ordered3_cli::call(
                    &ipc_file,
                    a([
                        u(3),
                        u(7),
                        u(slot.into()),
                        opt(expected.map(b)),
                        b(shared::read_public(&consent_file, 4096)?),
                        u(u64::from(historical)),
                    ]),
                )
                .await?;
                println!("{}", json(&result));
                return Ok(());
            }
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
        Command::Runtime3(_)
        | Command::Context2Propose { .. }
        | Command::Context2Cosign { .. }
        | Command::Consent1RenewPropose { .. }
        | Command::Consent1RenewCosign { .. }
        | Command::StorageUpgrade { .. }
        | Command::StorageUpgradeStatus(_)
        | Command::StorageUpgradePause(_)
        | Command::StorageHistory { .. }
        | Command::StorageIncreaseCapacity { .. } => {
            unreachable!("offline migration dispatched under original lease")
        }
    };
    println!("{}", json(&result));
    Ok(())
}
