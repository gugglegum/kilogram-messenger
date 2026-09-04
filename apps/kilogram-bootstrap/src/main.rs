use std::{
    io::{Read as _, Write as _},
    path::PathBuf,
    str::FromStr as _,
};

use anyhow::{Context as _, Result};
use clap::{Parser, Subcommand, ValueEnum};
use kilogram_identity::{
    AccountId, AccountRecoveryPhrase, DEFAULT_ACCOUNT_ROOT_RECOVERY_APPROVAL_VALIDITY_SECONDS,
    MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_VALIDITY_SECONDS,
};
use kilogram_transport_iroh::RoutePolicy;
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
    /// Report whether the current Root state has an exact successful export.
    AccountRecoveryStatus {
        #[arg(long)]
        account_root_dir: PathBuf,
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
        #[arg(long)]
        expected_package_id: String,
        #[arg(long)]
        expected_authority_revision: u64,
        #[arg(long, action = clap::ArgAction::SetTrue)]
        recovery_phrase_stdin: bool,
    },
    /// Create a fresh bounded approval request for an exact recovery checkpoint.
    AccountRecoveryQuorumRequest {
        #[arg(long)]
        package_file: PathBuf,
        #[arg(long)]
        witness_file: PathBuf,
        #[arg(long)]
        request_file: PathBuf,
        #[arg(
            long,
            default_value_t = DEFAULT_ACCOUNT_ROOT_RECOVERY_APPROVAL_VALIDITY_SECONDS,
            value_parser = clap::value_parser!(u64).range(1..=MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_VALIDITY_SECONDS)
        )]
        valid_for_seconds: u64,
    },
    /// Approve an exact recovery request after committing the local DB-primary head.
    AccountRecoveryQuorumApprove {
        #[arg(long)]
        state_dir: PathBuf,
        #[arg(long)]
        request_file: PathBuf,
        #[arg(long)]
        approval_file: PathBuf,
    },
    /// Verify distinct current-device approvals and report the exact freshness claim.
    AccountRecoveryQuorumVerify {
        #[arg(long)]
        request_file: PathBuf,
        #[arg(long = "approval-file")]
        approval_files: Vec<PathBuf>,
        #[arg(long, action = clap::ArgAction::SetTrue)]
        require_majority: bool,
        /// Joint old/new-majority certificate which bridges an earlier roster.
        #[arg(long)]
        policy_certificate_file: Option<PathBuf>,
    },
    /// Commit an approval and expose it once over a signed LAN-or-relay ticket.
    AccountRecoveryQuorumListen {
        #[arg(long)]
        state_dir: PathBuf,
        #[arg(long)]
        request_file: PathBuf,
        #[arg(long)]
        ticket_file: PathBuf,
        #[arg(long, value_enum, default_value_t = RoutePolicyArg::Auto)]
        route_policy: RoutePolicyArg,
        #[arg(long)]
        relay_url: Option<String>,
        #[arg(long, default_value_t = 30)]
        relay_wait_seconds: u64,
    },
    /// Collect distinct signed approvals from one-shot LAN-or-relay tickets.
    AccountRecoveryQuorumCollect {
        #[arg(long)]
        request_file: PathBuf,
        #[arg(long = "ticket-file", required = true)]
        ticket_files: Vec<PathBuf>,
        #[arg(long)]
        approval_dir: PathBuf,
        #[arg(long, action = clap::ArgAction::SetTrue)]
        require_majority: bool,
        #[arg(long, default_value_t = 30)]
        relay_wait_seconds: u64,
    },
    /// Propose an exact recovery voter-set change from epoch N to N+1.
    AccountRecoveryPolicyTransitionRequest {
        #[arg(long)]
        old_package_file: PathBuf,
        #[arg(long)]
        old_witness_file: PathBuf,
        #[arg(long)]
        new_package_file: PathBuf,
        #[arg(long)]
        new_witness_file: PathBuf,
        #[arg(long)]
        old_epoch: u64,
        #[arg(
            long,
            default_value = "0000000000000000000000000000000000000000000000000000000000000000"
        )]
        previous_transition_id: String,
        #[arg(long)]
        request_file: PathBuf,
        #[arg(
            long,
            default_value_t = DEFAULT_ACCOUNT_ROOT_RECOVERY_APPROVAL_VALIDITY_SECONDS,
            value_parser = clap::value_parser!(u64).range(1..=MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_VALIDITY_SECONDS)
        )]
        valid_for_seconds: u64,
    },
    /// Approve one exact transition after committing a DB-primary anti-equivocation head.
    AccountRecoveryPolicyTransitionApprove {
        #[arg(long)]
        state_dir: PathBuf,
        #[arg(long)]
        request_file: PathBuf,
        #[arg(long)]
        approval_file: PathBuf,
    },
    /// Commit a transition approval and expose it once over a signed LAN-or-relay ticket.
    AccountRecoveryPolicyTransitionListen {
        #[arg(long)]
        state_dir: PathBuf,
        #[arg(long)]
        request_file: PathBuf,
        #[arg(long)]
        ticket_file: PathBuf,
        #[arg(long, value_enum, default_value_t = RoutePolicyArg::Auto)]
        route_policy: RoutePolicyArg,
        #[arg(long)]
        relay_url: Option<String>,
        #[arg(long, default_value_t = 30)]
        relay_wait_seconds: u64,
    },
    /// Collect distinct transition approvals from one-shot LAN-or-relay tickets.
    AccountRecoveryPolicyTransitionCollect {
        #[arg(long)]
        request_file: PathBuf,
        #[arg(long = "ticket-file", required = true)]
        ticket_files: Vec<PathBuf>,
        #[arg(long)]
        approval_dir: PathBuf,
        #[arg(long, action = clap::ArgAction::SetTrue)]
        require_joint_majority: bool,
        #[arg(long, default_value_t = 30)]
        relay_wait_seconds: u64,
    },
    /// Build a permanent certificate only when both old and new strict majorities sign.
    AccountRecoveryPolicyTransitionCertify {
        #[arg(long)]
        request_file: PathBuf,
        #[arg(long = "approval-file", required = true)]
        approval_files: Vec<PathBuf>,
        #[arg(long)]
        certificate_file: PathBuf,
    },
    /// Verify a permanent joint-majority recovery-policy transition certificate.
    AccountRecoveryPolicyTransitionVerify {
        #[arg(long)]
        certificate_file: PathBuf,
    },
    /// Install a certified transition into one active new-roster device vault.
    AccountRecoveryPolicyTransitionInstall {
        #[arg(long)]
        state_dir: PathBuf,
        #[arg(long)]
        certificate_file: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum RoutePolicyArg {
    Auto,
    DirectOnly,
    RelayOnly,
}

impl From<RoutePolicyArg> for RoutePolicy {
    fn from(value: RoutePolicyArg) -> Self {
        match value {
            RoutePolicyArg::Auto => Self::Auto,
            RoutePolicyArg::DirectOnly => Self::DirectOnly,
            RoutePolicyArg::RelayOnly => Self::RelayOnly,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
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
        Command::AccountRecoveryStatus { account_root_dir } => serde_json::to_vec(
            &kilogram_bootstrap::account_recovery::account_root_status(account_root_dir)?,
        )?,
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
            expected_package_id,
            expected_authority_revision,
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
                &expected_package_id,
                expected_authority_revision,
            )?)?
        }
        Command::AccountRecoveryQuorumRequest {
            package_file,
            witness_file,
            request_file,
            valid_for_seconds,
        } => serde_json::to_vec(&kilogram_bootstrap::recovery_quorum::create_request(
            package_file,
            witness_file,
            request_file,
            valid_for_seconds,
        )?)?,
        Command::AccountRecoveryQuorumApprove {
            state_dir,
            request_file,
            approval_file,
        } => serde_json::to_vec(&kilogram_bootstrap::recovery_quorum::approve_request(
            state_dir,
            request_file,
            approval_file,
        )?)?,
        Command::AccountRecoveryQuorumVerify {
            request_file,
            approval_files,
            require_majority,
            policy_certificate_file,
        } => serde_json::to_vec(&match policy_certificate_file {
            Some(policy_certificate_file) => {
                kilogram_bootstrap::recovery_quorum::verify_request_with_policy_certificate(
                    request_file,
                    &approval_files,
                    policy_certificate_file,
                    require_majority,
                )?
            }
            None => kilogram_bootstrap::recovery_quorum::verify_request(
                request_file,
                &approval_files,
                require_majority,
            )?,
        })?,
        Command::AccountRecoveryQuorumListen {
            state_dir,
            request_file,
            ticket_file,
            route_policy,
            relay_url,
            relay_wait_seconds,
        } => {
            let relay_url = relay_url
                .map(|url| url.parse().context("parse recovery approval relay URL"))
                .transpose()?;
            serde_json::to_vec(
                &kilogram_bootstrap::recovery_quorum::listen_for_approval_collection(
                    state_dir,
                    request_file,
                    ticket_file,
                    route_policy.into(),
                    relay_url,
                    relay_wait_seconds,
                )
                .await?,
            )?
        }
        Command::AccountRecoveryQuorumCollect {
            request_file,
            ticket_files,
            approval_dir,
            require_majority,
            relay_wait_seconds,
        } => serde_json::to_vec(
            &kilogram_bootstrap::recovery_quorum::collect_approvals(
                request_file,
                &ticket_files,
                approval_dir,
                require_majority,
                relay_wait_seconds,
            )
            .await?,
        )?,
        Command::AccountRecoveryPolicyTransitionRequest {
            old_package_file,
            old_witness_file,
            new_package_file,
            new_witness_file,
            old_epoch,
            previous_transition_id,
            request_file,
            valid_for_seconds,
        } => serde_json::to_vec(
            &kilogram_bootstrap::recovery_policy::create_transition_request(
                old_package_file,
                old_witness_file,
                new_package_file,
                new_witness_file,
                old_epoch,
                &previous_transition_id,
                request_file,
                valid_for_seconds,
            )?,
        )?,
        Command::AccountRecoveryPolicyTransitionApprove {
            state_dir,
            request_file,
            approval_file,
        } => serde_json::to_vec(&kilogram_bootstrap::recovery_policy::approve_transition(
            state_dir,
            request_file,
            approval_file,
        )?)?,
        Command::AccountRecoveryPolicyTransitionListen {
            state_dir,
            request_file,
            ticket_file,
            route_policy,
            relay_url,
            relay_wait_seconds,
        } => {
            let relay_url = relay_url
                .map(|url| {
                    url.parse()
                        .context("parse recovery-policy approval relay URL")
                })
                .transpose()?;
            serde_json::to_vec(
                &kilogram_bootstrap::recovery_policy::listen_for_transition_approval(
                    state_dir,
                    request_file,
                    ticket_file,
                    route_policy.into(),
                    relay_url,
                    relay_wait_seconds,
                )
                .await?,
            )?
        }
        Command::AccountRecoveryPolicyTransitionCollect {
            request_file,
            ticket_files,
            approval_dir,
            require_joint_majority,
            relay_wait_seconds,
        } => serde_json::to_vec(
            &kilogram_bootstrap::recovery_policy::collect_transition_approvals(
                request_file,
                &ticket_files,
                approval_dir,
                require_joint_majority,
                relay_wait_seconds,
            )
            .await?,
        )?,
        Command::AccountRecoveryPolicyTransitionCertify {
            request_file,
            approval_files,
            certificate_file,
        } => serde_json::to_vec(&kilogram_bootstrap::recovery_policy::certify_transition(
            request_file,
            &approval_files,
            certificate_file,
        )?)?,
        Command::AccountRecoveryPolicyTransitionVerify { certificate_file } => serde_json::to_vec(
            &kilogram_bootstrap::recovery_policy::verify_transition_certificate(certificate_file)?,
        )?,
        Command::AccountRecoveryPolicyTransitionInstall {
            state_dir,
            certificate_file,
        } => serde_json::to_vec(&kilogram_bootstrap::recovery_policy::install_transition(
            state_dir,
            certificate_file,
        )?)?,
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
