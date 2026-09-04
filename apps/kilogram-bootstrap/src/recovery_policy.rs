use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use iroh::{Endpoint, EndpointAddr, RelayUrl, endpoint::Connection};
use kilogram_identity::{
    AccountAuthoritySnapshot, AccountRecoveryPolicyState, AccountRecoveryPolicyTransitionApproval,
    AccountRecoveryPolicyTransitionCertificate, AccountRecoveryPolicyTransitionRequest,
    AccountRootRecoveryApproval, AccountRootRecoveryPackage, AccountRootRecoveryWitness,
    ConversationMembershipSnapshot, DeviceCapability, DeviceCertificate, DeviceIdentity,
    DeviceState, MAX_ACCOUNT_RECOVERY_POLICY_CERTIFICATE_BYTES,
    MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_APPROVAL_BYTES,
    MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_APPROVALS,
    MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_REQUEST_BYTES, MAX_ACCOUNT_ROOT_RECOVERY_PACKAGE_BYTES,
    MAX_ACCOUNT_ROOT_RECOVERY_WITNESS_BYTES, verify_account_recovery_policy_transition,
    verify_device_authorization_with_snapshot,
};
use kilogram_state::{
    DeviceIdentityStateRepository, EncryptedStateVault, StateDirectoryLock, StateMirrorRepository,
    StateRecordKind, StateTransaction, TrustStateRepository, VaultPrimaryWriteRepository,
};
use kilogram_transport_iroh::{
    RoutePolicy, SelectedPathDiagnostics, await_route_policy, endpoint_builder_for_remote,
    endpoint_builder_with_relay,
};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use tokio::time::timeout;

const DEVICE_CERTIFICATE_PATH: &str = "device-certificate.cert";
const AUTHORITY_SNAPSHOT_PATH: &str = "account-authority.snapshot";
const MEMBERSHIP_PREFIX: &str = "conversation-memberships/";
pub(crate) const RECOVERY_APPROVAL_HEAD_PATH: &str = "recovery-approval/latest.approval";
pub(crate) const RECOVERY_POLICY_STATE_PATH: &str = "recovery-policy/current.policy";
pub(crate) const RECOVERY_POLICY_CERTIFICATE_PATH: &str = "recovery-policy/latest-transition.karpc";
const RECOVERY_POLICY_APPROVAL_HEAD_PATH: &str = "recovery-policy/latest-transition-approval.karpa";
const POLICY_APPROVAL_ALPN: &[u8] = b"kilogram/m0/recovery-policy-approval/1";
const POLICY_APPROVAL_TICKET_VERSION: u8 = 1;
const POLICY_APPROVAL_FETCH_VERSION: u8 = 1;
const POLICY_APPROVAL_TICKET_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:recovery-policy-approval-ticket:v1\0";
const MAX_POLICY_APPROVAL_TICKET_BYTES: usize = 256 * 1024;
const MAX_POLICY_APPROVAL_FETCH_BYTES: usize = 16 * 1024;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);
const ROUTE_POLICY_WAIT: Duration = Duration::from_secs(15);
const WIRE_IO_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryPolicyTransitionRequestOutput {
    status: &'static str,
    account_id: String,
    request_id: String,
    old_epoch: u64,
    new_epoch: u64,
    previous_transition_id: String,
    old_recovery_roster_digest: String,
    new_recovery_roster_digest: String,
    old_roster_count: usize,
    new_roster_count: usize,
    old_required_approvals: usize,
    new_required_approvals: usize,
    expires_at_unix_seconds: u64,
    request_file: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryPolicyTransitionApprovalOutput {
    status: &'static str,
    account_id: String,
    request_id: String,
    old_epoch: u64,
    new_epoch: u64,
    approver_device_id: String,
    approval_id: String,
    approval_file: PathBuf,
    approval_head_committed_before_publish: bool,
    reused_committed_approval: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryPolicyTransitionCertificateOutput {
    status: &'static str,
    account_id: String,
    certificate_id: String,
    old_epoch: u64,
    new_epoch: u64,
    previous_transition_id: String,
    old_recovery_roster_digest: String,
    new_recovery_roster_digest: String,
    old_observed_approvals: usize,
    old_required_approvals: usize,
    new_observed_approvals: usize,
    new_required_approvals: usize,
    joint_majority_satisfied: bool,
    cross_roster_fork_safety: bool,
    certificate_file: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryPolicyInstallOutput {
    status: &'static str,
    account_id: String,
    policy_epoch: u64,
    recovery_roster_digest: String,
    latest_transition_id: String,
    policy_state_source: &'static str,
    installed_certificate_file: PathBuf,
    cross_roster_fork_safety: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryPolicyTransitionListenOutput {
    status: &'static str,
    account_id: String,
    request_id: String,
    old_epoch: u64,
    new_epoch: u64,
    approver_device_id: String,
    approval_id: String,
    ticket_file: PathBuf,
    route_policy: &'static str,
    transport_path: &'static str,
    transport_remote_address: String,
    transport_rtt_milliseconds: u64,
    transport_open_paths: usize,
    approval_head_source: &'static str,
    approval_head_committed_before_ticket_publish: bool,
    reused_committed_approval: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryPolicyTransitionTransportObservation {
    approver_device_id: String,
    ticket_file: PathBuf,
    approval_file: PathBuf,
    route_policy: &'static str,
    transport_path: &'static str,
    transport_remote_address: String,
    transport_rtt_milliseconds: u64,
    transport_open_paths: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryPolicyTransitionCollectOutput {
    status: &'static str,
    account_id: String,
    request_id: String,
    old_epoch: u64,
    new_epoch: u64,
    old_observed_approvals: usize,
    old_required_approvals: usize,
    new_observed_approvals: usize,
    new_required_approvals: usize,
    joint_majority_satisfied: bool,
    cross_roster_fork_safety: bool,
    approval_directory: PathBuf,
    transports: Vec<RecoveryPolicyTransitionTransportObservation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct PolicyApprovalTicketContent {
    version: u8,
    endpoint: EndpointAddr,
    account_id: kilogram_identity::AccountId,
    request_id: [u8; 32],
    request_expires_at_unix_seconds: u64,
    approver_certificate: DeviceCertificate,
    bearer_token: [u8; 32],
    route_policy: RoutePolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct PolicyApprovalTicket {
    content: PolicyApprovalTicketContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct PolicyApprovalFetch {
    version: u8,
    request_id: [u8; 32],
    bearer_token: [u8; 32],
}

struct CommittedTransitionApproval {
    approval: AccountRecoveryPolicyTransitionApproval,
    reused: bool,
    ticket: Option<PolicyApprovalTicket>,
}

impl PolicyApprovalTicket {
    fn issue(
        request: &AccountRecoveryPolicyTransitionRequest,
        endpoint: EndpointAddr,
        approver_identity: &DeviceIdentity,
        approver_certificate: DeviceCertificate,
        bearer_token: [u8; 32],
        route_policy: RoutePolicy,
    ) -> Result<Self> {
        ensure!(
            approver_certificate.device_id() == approver_identity.device_id(),
            "recovery-policy ticket certificate belongs to a different device"
        );
        let content = PolicyApprovalTicketContent {
            version: POLICY_APPROVAL_TICKET_VERSION,
            endpoint,
            account_id: request.account_id(),
            request_id: request.request_id()?,
            request_expires_at_unix_seconds: request.expires_at_unix_seconds(),
            approver_certificate,
            bearer_token,
            route_policy,
        };
        let signature = approver_identity
            .sign(&policy_ticket_signing_bytes(&content)?)
            .to_vec();
        let ticket = Self { content, signature };
        ticket.verify_for_request(request, unix_time_now()?)?;
        Ok(ticket)
    }

    fn encode(&self) -> Result<String> {
        let encoded = serde_json::to_vec(self).context("serialize recovery-policy ticket")?;
        ensure!(
            encoded.len() <= MAX_POLICY_APPROVAL_TICKET_BYTES,
            "recovery-policy ticket is too large"
        );
        Ok(URL_SAFE_NO_PAD.encode(encoded))
    }

    fn decode(encoded: &str) -> Result<Self> {
        ensure!(
            encoded.len() <= MAX_POLICY_APPROVAL_TICKET_BYTES.saturating_mul(2),
            "encoded recovery-policy ticket is too large"
        );
        let bytes = URL_SAFE_NO_PAD
            .decode(encoded.trim())
            .context("decode recovery-policy ticket as base64url")?;
        ensure!(
            bytes.len() <= MAX_POLICY_APPROVAL_TICKET_BYTES,
            "recovery-policy ticket is too large"
        );
        serde_json::from_slice(&bytes).context("decode recovery-policy ticket")
    }

    fn verify_for_request(
        &self,
        request: &AccountRecoveryPolicyTransitionRequest,
        now: u64,
    ) -> Result<()> {
        request.verify_at(now)?;
        ensure!(
            self.content.version == POLICY_APPROVAL_TICKET_VERSION,
            "unsupported recovery-policy ticket version"
        );
        ensure!(
            self.content.account_id == request.account_id()
                && self.content.request_id == request.request_id()?
                && self.content.request_expires_at_unix_seconds
                    == request.expires_at_unix_seconds(),
            "recovery-policy ticket is bound to a different transition request"
        );
        ensure!(
            now <= self.content.request_expires_at_unix_seconds,
            "recovery-policy ticket has expired"
        );
        self.content.approver_certificate.verify()?;
        ensure!(
            package_authorizes_certificate(
                request.old_package(),
                &self.content.approver_certificate,
            ) || package_authorizes_certificate(
                request.new_package(),
                &self.content.approver_certificate,
            ),
            "recovery-policy ticket signer is not exact and active in either roster"
        );
        self.content
            .approver_certificate
            .device_id()
            .verify(
                &policy_ticket_signing_bytes(&self.content)?,
                &self.signature,
            )
            .context("verify device signature on recovery-policy ticket")?;
        if self.content.route_policy == RoutePolicy::RelayOnly {
            ensure!(
                self.content.endpoint.relay_urls().next().is_some(),
                "relay-only recovery-policy ticket has no relay address"
            );
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
pub fn create_transition_request(
    old_package_file: impl AsRef<Path>,
    old_witness_file: impl AsRef<Path>,
    new_package_file: impl AsRef<Path>,
    new_witness_file: impl AsRef<Path>,
    old_epoch: u64,
    previous_transition_id_hex: &str,
    request_file: impl AsRef<Path>,
    validity_seconds: u64,
) -> Result<RecoveryPolicyTransitionRequestOutput> {
    let old_package = read_package(old_package_file.as_ref())?;
    let old_witness = read_witness(old_witness_file.as_ref())?;
    let new_package = read_package(new_package_file.as_ref())?;
    let new_witness = read_witness(new_witness_file.as_ref())?;
    let previous_transition_id = decode_hex_32(previous_transition_id_hex)
        .context("parse previous recovery-policy transition ID")?;
    let now = unix_time_now()?;
    let request = AccountRecoveryPolicyTransitionRequest::issue(
        old_epoch,
        previous_transition_id,
        old_package,
        old_witness,
        new_package,
        new_witness,
        now,
        validity_seconds,
    )
    .context("issue recovery-policy transition request")?;
    write_new(request_file.as_ref(), &request.encode()?)?;
    transition_request_output(&request, absolute_existing_path(request_file.as_ref())?)
}

pub fn approve_transition(
    state_dir: impl AsRef<Path>,
    request_file: impl AsRef<Path>,
    approval_file: impl AsRef<Path>,
) -> Result<RecoveryPolicyTransitionApprovalOutput> {
    let now = unix_time_now()?;
    let request = read_transition_request(request_file.as_ref(), now)?;
    let committed = approve_decoded_transition(state_dir.as_ref(), &request, None, now)?;
    let encoded = committed.approval.encode()?;
    write_idempotent(approval_file.as_ref(), &encoded)?;
    transition_approval_output(
        &committed.approval,
        absolute_existing_path(approval_file.as_ref())?,
        committed.reused,
    )
}

fn approve_decoded_transition(
    state_dir: &Path,
    request: &AccountRecoveryPolicyTransitionRequest,
    ticket_draft: Option<(EndpointAddr, [u8; 32], RoutePolicy)>,
    now: u64,
) -> Result<CommittedTransitionApproval> {
    let state_dir = fs::canonicalize(state_dir).with_context(|| {
        format!(
            "resolve recovery-policy approving device state {}",
            state_dir.display()
        )
    })?;
    ensure!(
        state_dir.is_dir(),
        "approving device state is not a directory"
    );
    let _lock = StateDirectoryLock::acquire(&state_dir)
        .context("lock recovery-policy approving device state")?;
    ensure!(
        EncryptedStateVault::is_initialized(&state_dir)?,
        "recovery-policy approval requires an initialized DB-primary vault"
    );
    let vault = EncryptedStateVault::open_existing(&state_dir)
        .context("open recovery-policy approving device DB-primary vault")?;
    vault.recover_primary_shadow()?;
    vault.recover_pending_dual_write()?;
    let identity = vault.read_primary_device_identity()?;
    let device = DeviceState::from_secret_material(
        &state_dir,
        *identity.signing_secret(),
        *identity.encryption_secret(),
    );
    let trust = vault.read_primary_trust()?;
    let certificate = decode_required_trust_record::<DeviceCertificate>(
        &trust,
        DEVICE_CERTIFICATE_PATH,
        DeviceCertificate::decode_and_verify,
    )?;
    ensure!(
        certificate.device_id() == device.identity().device_id(),
        "DB-primary certificate belongs to a different device"
    );
    let authority = decode_required_trust_record::<AccountAuthoritySnapshot>(
        &trust,
        AUTHORITY_SNAPSHOT_PATH,
        AccountAuthoritySnapshot::decode_and_verify,
    )?;
    let memberships = owned_memberships(&trust, request.account_id())?;
    let in_old = package_authorizes_certificate(request.old_package(), &certificate);
    let in_new = package_authorizes_certificate(request.new_package(), &certificate);
    ensure!(
        in_old || in_new,
        "local device is not active in either transition roster"
    );
    if in_new {
        request
            .new_package()
            .verify_dominates_device_state(&certificate, &authority, &memberships)
            .context("new recovery-policy package does not dominate local device state")?;
    } else {
        request
            .old_package()
            .verify_dominates_device_state(&certificate, &authority, &memberships)
            .context("old recovery-policy package does not match local old-only voter state")?;
    }

    let policy_state = optional_policy_state(&trust)?;
    if in_old {
        match policy_state.as_ref() {
            Some(policy) => policy
                .verify_transition_anchor(request)
                .context("transition does not extend the DB-primary recovery-policy anchor")?,
            None => verify_legacy_genesis_anchor(&trust, request)?,
        }
    }
    let existing = optional_transition_approval(&trust)?;
    if let Some(existing) = existing.as_ref()
        && existing.request_id() == &request.request_id()?
    {
        existing.verify_for_request(request)?;
        ensure!(
            existing.approver_device_id() == device.identity().device_id(),
            "committed recovery-policy approval belongs to a different device"
        );
        let ticket = ticket_draft
            .map(|(endpoint, bearer_token, route_policy)| {
                PolicyApprovalTicket::issue(
                    request,
                    endpoint,
                    device.identity(),
                    certificate.clone(),
                    bearer_token,
                    route_policy,
                )
            })
            .transpose()?;
        return Ok(CommittedTransitionApproval {
            approval: existing.clone(),
            reused: true,
            ticket,
        });
    }
    if let Some(existing) = existing.as_ref() {
        ensure!(
            existing.account_id() == request.account_id()
                && existing.old_epoch() < request.old_epoch(),
            "device already committed a different recovery-policy transition for this or a later epoch"
        );
    }
    let previous_approval_id = existing
        .as_ref()
        .map(AccountRecoveryPolicyTransitionApproval::approval_id)
        .transpose()?
        .unwrap_or([0_u8; 32]);
    let approval = AccountRecoveryPolicyTransitionApproval::issue(
        device.identity(),
        request,
        previous_approval_id,
        now,
    )?;
    let approval_bytes = approval.encode()?;

    vault.begin_dual_write()?;
    let commit_result = commit_transition_approval_primary(
        &state_dir,
        &vault,
        &device,
        if in_new {
            request.new_package()
        } else {
            request.old_package()
        },
        (in_old && policy_state.is_none())
            .then(|| AccountRecoveryPolicyState::genesis(request.old_package()))
            .transpose()?,
        &approval_bytes,
    );
    let mirror_result = vault
        .finish_dual_write()
        .context("complete policy approval vault mirror");
    combine_dual_write(commit_result, mirror_result.map(|_| ()))?;
    let ticket = ticket_draft
        .map(|(endpoint, bearer_token, route_policy)| {
            PolicyApprovalTicket::issue(
                request,
                endpoint,
                device.identity(),
                certificate,
                bearer_token,
                route_policy,
            )
        })
        .transpose()?;
    Ok(CommittedTransitionApproval {
        approval,
        reused: false,
        ticket,
    })
}

pub async fn listen_for_transition_approval(
    state_dir: impl AsRef<Path>,
    request_file: impl AsRef<Path>,
    ticket_file: impl AsRef<Path>,
    route_policy: RoutePolicy,
    relay_url: Option<RelayUrl>,
    relay_wait_seconds: u64,
) -> Result<RecoveryPolicyTransitionListenOutput> {
    let endpoint = endpoint_builder_with_relay(route_policy, relay_url)
        .alpns(vec![POLICY_APPROVAL_ALPN.to_vec()])
        .bind()
        .await
        .context("bind recovery-policy approval listener")?;
    wait_for_relay(&endpoint, route_policy, relay_wait_seconds).await?;
    listen_for_transition_approval_on_endpoint(
        state_dir.as_ref(),
        request_file.as_ref(),
        ticket_file.as_ref(),
        route_policy,
        endpoint,
    )
    .await
}

async fn listen_for_transition_approval_on_endpoint(
    state_dir: &Path,
    request_file: &Path,
    ticket_file: &Path,
    route_policy: RoutePolicy,
    endpoint: Endpoint,
) -> Result<RecoveryPolicyTransitionListenOutput> {
    let now = unix_time_now()?;
    let request = read_transition_request(request_file, now)?;
    let mut bearer_token = [0_u8; 32];
    getrandom::fill(&mut bearer_token).context("generate recovery-policy approval bearer token")?;
    let committed = approve_decoded_transition(
        state_dir,
        &request,
        Some((endpoint.addr(), bearer_token, route_policy)),
        now,
    )?;
    let ticket = committed
        .ticket
        .as_ref()
        .context("network transition approval did not produce a ticket")?;
    write_new(ticket_file, ticket.encode()?.as_bytes())?;
    let ticket_file = absolute_existing_path(ticket_file)?;

    let remaining = request
        .expires_at_unix_seconds()
        .checked_sub(unix_time_now()?)
        .context("recovery-policy request expired before listener publication")?;
    let connection =
        accept_authenticated_connection(&endpoint, Duration::from_secs(remaining)).await?;
    let diagnostics = await_route_policy(&connection, route_policy, ROUTE_POLICY_WAIT)
        .await
        .context("wait for a path allowed by the recovery-policy ticket")?;
    let (mut send, mut receive) = timeout(WIRE_IO_TIMEOUT, connection.accept_bi())
        .await
        .context("accept recovery-policy approval stream timed out")?
        .context("accept recovery-policy approval stream")?;
    let fetch_bytes = timeout(
        WIRE_IO_TIMEOUT,
        receive.read_to_end(MAX_POLICY_APPROVAL_FETCH_BYTES),
    )
    .await
    .context("read recovery-policy approval fetch timed out")?
    .context("read recovery-policy approval fetch")?;
    let fetch: PolicyApprovalFetch =
        postcard::from_bytes(&fetch_bytes).context("decode recovery-policy approval fetch")?;
    ensure!(
        fetch.version == POLICY_APPROVAL_FETCH_VERSION
            && fetch.request_id == request.request_id()?
            && fetch.bearer_token == bearer_token,
        "recovery-policy approval fetch is not authorized by this one-shot ticket"
    );
    let approval_bytes = committed.approval.encode()?;
    timeout(WIRE_IO_TIMEOUT, send.write_all(&approval_bytes))
        .await
        .context("send recovery-policy approval timed out")?
        .context("send recovery-policy approval")?;
    send.finish()
        .context("finish recovery-policy approval response")?;
    let _ = timeout(WIRE_IO_TIMEOUT, connection.closed()).await;
    endpoint.close().await;

    Ok(RecoveryPolicyTransitionListenOutput {
        status: "account-recovery-policy-transition-approved-over-transport",
        account_id: committed.approval.account_id().to_string(),
        request_id: encode_hex(committed.approval.request_id()),
        old_epoch: committed.approval.old_epoch(),
        new_epoch: committed.approval.new_epoch(),
        approver_device_id: committed.approval.approver_device_id().to_string(),
        approval_id: encode_hex(&committed.approval.approval_id()?),
        ticket_file,
        route_policy: route_policy.as_str(),
        transport_path: diagnostics.kind.as_str(),
        transport_remote_address: diagnostics.remote_address,
        transport_rtt_milliseconds: duration_milliseconds(diagnostics.round_trip_time),
        transport_open_paths: diagnostics.open_paths,
        approval_head_source: "db-primary",
        approval_head_committed_before_ticket_publish: true,
        reused_committed_approval: committed.reused,
    })
}

pub async fn collect_transition_approvals(
    request_file: impl AsRef<Path>,
    ticket_files: &[PathBuf],
    approval_directory: impl AsRef<Path>,
    require_joint_majority: bool,
    relay_wait_seconds: u64,
) -> Result<RecoveryPolicyTransitionCollectOutput> {
    ensure!(
        !ticket_files.is_empty(),
        "at least one recovery-policy approval ticket is required"
    );
    ensure!(
        ticket_files.len() <= MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_APPROVALS,
        "too many recovery-policy approval tickets"
    );
    let request = read_transition_request(request_file.as_ref(), unix_time_now()?)?;
    let approval_directory = resolve_approval_directory(approval_directory.as_ref())?;
    let mut ticket_approvers = BTreeSet::new();
    let mut approvals = Vec::with_capacity(ticket_files.len());
    let mut transports = Vec::with_capacity(ticket_files.len());

    for ticket_file in ticket_files {
        let ticket_text = String::from_utf8(read_bounded_regular_file(
            ticket_file,
            MAX_POLICY_APPROVAL_TICKET_BYTES.saturating_mul(2),
            "recovery-policy approval ticket",
        )?)
        .context("recovery-policy approval ticket is not UTF-8")?;
        let ticket = PolicyApprovalTicket::decode(&ticket_text)?;
        ticket.verify_for_request(&request, unix_time_now()?)?;
        let approver = ticket.content.approver_certificate.device_id();
        ensure!(
            ticket_approvers.insert(approver.to_string()),
            "duplicate recovery-policy approval ticket for device {approver}"
        );
        let (approval, diagnostics) =
            collect_one_transition_approval(&request, &ticket, relay_wait_seconds).await?;
        ensure!(
            approval.approver_device_id() == approver,
            "recovery-policy approval response came from a different device"
        );
        let approval_file = approval_directory.join(format!("{approver}.karpa"));
        write_idempotent(&approval_file, &approval.encode()?)?;
        let approval_file = absolute_existing_path(&approval_file)?;
        transports.push(RecoveryPolicyTransitionTransportObservation {
            approver_device_id: approver.to_string(),
            ticket_file: absolute_existing_path(ticket_file)?,
            approval_file,
            route_policy: ticket.content.route_policy.as_str(),
            transport_path: diagnostics.kind.as_str(),
            transport_remote_address: diagnostics.remote_address,
            transport_rtt_milliseconds: duration_milliseconds(diagnostics.round_trip_time),
            transport_open_paths: diagnostics.open_paths,
        });
        approvals.push(approval);
    }

    let report = verify_account_recovery_policy_transition(&request, &approvals)
        .context("verify collected recovery-policy transition approvals")?;
    if require_joint_majority {
        report
            .require_joint_majority()
            .context("require old and new strict-majority transition quorums")?;
    }
    let joint_majority_satisfied = report.joint_majority_satisfied();
    Ok(RecoveryPolicyTransitionCollectOutput {
        status: if joint_majority_satisfied {
            "account-recovery-policy-transition-joint-majority-collected"
        } else {
            "account-recovery-policy-transition-partial-collected"
        },
        account_id: request.account_id().to_string(),
        request_id: encode_hex(&request.request_id()?),
        old_epoch: request.old_epoch(),
        new_epoch: request.new_epoch(),
        old_observed_approvals: report.old_observed(),
        old_required_approvals: report.old_required(),
        new_observed_approvals: report.new_observed(),
        new_required_approvals: report.new_required(),
        joint_majority_satisfied,
        cross_roster_fork_safety: false,
        approval_directory,
        transports,
    })
}

async fn collect_one_transition_approval(
    request: &AccountRecoveryPolicyTransitionRequest,
    ticket: &PolicyApprovalTicket,
    relay_wait_seconds: u64,
) -> Result<(
    AccountRecoveryPolicyTransitionApproval,
    SelectedPathDiagnostics,
)> {
    let route_policy = ticket.content.route_policy;
    let endpoint = endpoint_builder_for_remote(route_policy, &ticket.content.endpoint)?
        .alpns(vec![POLICY_APPROVAL_ALPN.to_vec()])
        .bind()
        .await
        .context("bind recovery-policy approval collector")?;
    wait_for_relay(&endpoint, route_policy, relay_wait_seconds).await?;
    let connection = timeout(
        CONNECTION_TIMEOUT,
        endpoint.connect(ticket.content.endpoint.clone(), POLICY_APPROVAL_ALPN),
    )
    .await
    .context("connect recovery-policy approval endpoint timed out")?
    .context("connect recovery-policy approval endpoint")?;
    let diagnostics = await_route_policy(&connection, route_policy, ROUTE_POLICY_WAIT)
        .await
        .context("wait for a path allowed by recovery-policy ticket")?;
    let (mut send, mut receive) = timeout(WIRE_IO_TIMEOUT, connection.open_bi())
        .await
        .context("open recovery-policy approval stream timed out")?
        .context("open recovery-policy approval stream")?;
    let fetch = PolicyApprovalFetch {
        version: POLICY_APPROVAL_FETCH_VERSION,
        request_id: request.request_id()?,
        bearer_token: ticket.content.bearer_token,
    };
    let fetch_bytes =
        postcard::to_allocvec(&fetch).context("encode recovery-policy approval fetch")?;
    ensure!(
        fetch_bytes.len() <= MAX_POLICY_APPROVAL_FETCH_BYTES,
        "recovery-policy approval fetch is too large"
    );
    timeout(WIRE_IO_TIMEOUT, send.write_all(&fetch_bytes))
        .await
        .context("send recovery-policy approval fetch timed out")?
        .context("send recovery-policy approval fetch")?;
    send.finish()
        .context("finish recovery-policy approval fetch")?;
    let approval_bytes = timeout(
        WIRE_IO_TIMEOUT,
        receive.read_to_end(MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_APPROVAL_BYTES),
    )
    .await
    .context("read recovery-policy approval timed out")?
    .context("read recovery-policy approval")?;
    let approval = AccountRecoveryPolicyTransitionApproval::decode_and_verify(&approval_bytes)?;
    approval
        .verify_for_request(request)
        .context("verify transported recovery-policy approval")?;
    connection.close(0_u32.into(), b"recovery-policy approval collected");
    endpoint.close().await;
    Ok((approval, diagnostics))
}

async fn wait_for_relay(
    endpoint: &Endpoint,
    route_policy: RoutePolicy,
    relay_wait_seconds: u64,
) -> Result<()> {
    if relay_wait_seconds == 0 {
        ensure!(
            route_policy != RoutePolicy::RelayOnly,
            "relay-only requires a positive relay wait"
        );
        return Ok(());
    }
    match timeout(Duration::from_secs(relay_wait_seconds), endpoint.online()).await {
        Ok(()) => Ok(()),
        Err(_) if route_policy == RoutePolicy::RelayOnly => {
            bail!("required relay did not become online within {relay_wait_seconds}s")
        }
        Err(_) => Ok(()),
    }
}

async fn accept_authenticated_connection(
    endpoint: &Endpoint,
    remaining_request_lifetime: Duration,
) -> Result<Connection> {
    timeout(remaining_request_lifetime, async {
        loop {
            let incoming = endpoint
                .accept()
                .await
                .context("recovery-policy endpoint closed before a connection arrived")?;
            let accepting = match incoming.accept() {
                Ok(accepting) => accepting,
                Err(_) => continue,
            };
            match timeout(CONNECTION_TIMEOUT, accepting).await {
                Ok(Ok(connection)) => return Ok(connection),
                Ok(Err(_)) | Err(_) => continue,
            }
        }
    })
    .await
    .context("recovery-policy listener reached the request expiry")?
}

fn policy_ticket_signing_bytes(content: &PolicyApprovalTicketContent) -> Result<Vec<u8>> {
    let encoded =
        serde_json::to_vec(content).context("serialize recovery-policy ticket content")?;
    let mut bytes =
        Vec::with_capacity(POLICY_APPROVAL_TICKET_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(POLICY_APPROVAL_TICKET_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn resolve_approval_directory(path: &Path) -> Result<PathBuf> {
    ensure!(
        !path.as_os_str().is_empty(),
        "approval directory path is empty"
    );
    fs::create_dir_all(path)
        .with_context(|| format!("create approval directory {}", path.display()))?;
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("inspect approval directory {}", path.display()))?;
    ensure!(
        !metadata.file_type().is_symlink() && metadata.is_dir(),
        "approval output must be a regular directory"
    );
    fs::canonicalize(path).context("resolve approval directory")
}

fn duration_milliseconds(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

pub fn certify_transition(
    request_file: impl AsRef<Path>,
    approval_files: &[PathBuf],
    certificate_file: impl AsRef<Path>,
) -> Result<RecoveryPolicyTransitionCertificateOutput> {
    ensure!(
        approval_files.len() <= MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_APPROVALS,
        "too many transition approval files"
    );
    let now = unix_time_now()?;
    let request = read_transition_request(request_file.as_ref(), now)?;
    let mut approvals = Vec::with_capacity(approval_files.len());
    for path in approval_files {
        approvals.push(AccountRecoveryPolicyTransitionApproval::decode_and_verify(
            &read_bounded_regular_file(
                path,
                MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_APPROVAL_BYTES,
                "recovery-policy transition approval",
            )?,
        )?);
    }
    let certificate = AccountRecoveryPolicyTransitionCertificate::issue(request, approvals, now)
        .context("require old and new strict-majority transition quorums")?;
    write_new(certificate_file.as_ref(), &certificate.encode()?)?;
    transition_certificate_output(
        &certificate,
        absolute_existing_path(certificate_file.as_ref())?,
    )
}

pub fn verify_transition_certificate(
    certificate_file: impl AsRef<Path>,
) -> Result<RecoveryPolicyTransitionCertificateOutput> {
    let certificate = read_transition_certificate(certificate_file.as_ref())?;
    transition_certificate_output(
        &certificate,
        absolute_existing_path(certificate_file.as_ref())?,
    )
}

pub fn install_transition(
    state_dir: impl AsRef<Path>,
    certificate_file: impl AsRef<Path>,
) -> Result<RecoveryPolicyInstallOutput> {
    let certificate = read_transition_certificate(certificate_file.as_ref())?;
    let state_dir = fs::canonicalize(state_dir.as_ref())?;
    let _lock = StateDirectoryLock::acquire(&state_dir)?;
    ensure!(
        EncryptedStateVault::is_initialized(&state_dir)?,
        "recovery-policy install requires an initialized DB-primary vault"
    );
    let vault = EncryptedStateVault::open_existing(&state_dir)?;
    vault.recover_primary_shadow()?;
    vault.recover_pending_dual_write()?;
    let identity = vault.read_primary_device_identity()?;
    let device = DeviceState::from_secret_material(
        &state_dir,
        *identity.signing_secret(),
        *identity.encryption_secret(),
    );
    let trust = vault.read_primary_trust()?;
    let local_certificate = decode_required_trust_record::<DeviceCertificate>(
        &trust,
        DEVICE_CERTIFICATE_PATH,
        DeviceCertificate::decode_and_verify,
    )?;
    let target = AccountRecoveryPolicyState::transitioned(&certificate)?;
    let existing = optional_policy_state(&trust)?;
    let source = if existing.as_ref() == Some(&target) {
        "db-primary-idempotent"
    } else {
        ensure!(
            package_authorizes_certificate(certificate.new_package(), &local_certificate),
            "only an active voter in the new recovery roster may install this transition"
        );
        let authority = decode_required_trust_record::<AccountAuthoritySnapshot>(
            &trust,
            AUTHORITY_SNAPSHOT_PATH,
            AccountAuthoritySnapshot::decode_and_verify,
        )?;
        let memberships = owned_memberships(&trust, certificate.account_id())?;
        certificate.new_package().verify_dominates_device_state(
            &local_certificate,
            &authority,
            &memberships,
        )?;
        match existing.as_ref() {
            Some(policy) => policy.verify_transition_anchor(certificate.request())?,
            None if package_authorizes_certificate(
                certificate.old_package(),
                &local_certificate,
            ) =>
            {
                verify_legacy_genesis_anchor(&trust, certificate.request())?;
            }
            None => {}
        }
        vault.begin_dual_write()?;
        let commit_result = commit_policy_install_primary(
            &state_dir,
            &vault,
            &device,
            certificate.new_package(),
            &target.encode()?,
            &certificate.encode()?,
        );
        let mirror_result = vault
            .finish_dual_write()
            .context("complete policy install vault mirror");
        combine_dual_write(commit_result, mirror_result.map(|_| ()))?;
        "db-primary-installed"
    };
    Ok(RecoveryPolicyInstallOutput {
        status: "account-recovery-policy-transition-installed",
        account_id: target.account_id().to_string(),
        policy_epoch: target.epoch(),
        recovery_roster_digest: encode_hex(target.recovery_roster_digest()),
        latest_transition_id: encode_hex(target.latest_transition_id()),
        policy_state_source: source,
        installed_certificate_file: state_dir.join(RECOVERY_POLICY_CERTIFICATE_PATH),
        cross_roster_fork_safety: true,
    })
}

pub(crate) fn verify_candidate_against_certificate(
    candidate: &AccountRootRecoveryPackage,
    certificate: &AccountRecoveryPolicyTransitionCertificate,
) -> Result<()> {
    certificate.verify()?;
    ensure!(
        candidate.recovery_roster_digest()? == certificate.new_recovery_roster_digest()?,
        "recovery candidate roster does not match certified new policy roster"
    );
    kilogram_identity::verify_recovery_package_successor(
        certificate.new_package(),
        candidate,
        false,
    )
    .context("recovery candidate does not extend the certified new policy state")
}

fn commit_transition_approval_primary(
    state_dir: &Path,
    vault: &EncryptedStateVault,
    device: &DeviceState,
    package: &AccountRootRecoveryPackage,
    genesis: Option<AccountRecoveryPolicyState>,
    approval_bytes: &[u8],
) -> Result<()> {
    commit_policy_trust_primary(state_dir, vault, device, package, |state_dir| {
        if let Some(genesis) = genesis {
            write_replace(
                &state_dir.join(RECOVERY_POLICY_STATE_PATH),
                &genesis.encode()?,
            )?;
        }
        write_replace(
            &state_dir.join(RECOVERY_POLICY_APPROVAL_HEAD_PATH),
            approval_bytes,
        )
    })
}

fn commit_policy_install_primary(
    state_dir: &Path,
    vault: &EncryptedStateVault,
    device: &DeviceState,
    package: &AccountRootRecoveryPackage,
    policy_bytes: &[u8],
    certificate_bytes: &[u8],
) -> Result<()> {
    commit_policy_trust_primary(state_dir, vault, device, package, |state_dir| {
        write_replace(&state_dir.join(RECOVERY_POLICY_STATE_PATH), policy_bytes)?;
        write_replace(
            &state_dir.join(RECOVERY_POLICY_CERTIFICATE_PATH),
            certificate_bytes,
        )
    })
}

fn commit_policy_trust_primary(
    state_dir: &Path,
    vault: &EncryptedStateVault,
    device: &DeviceState,
    package: &AccountRootRecoveryPackage,
    write_policy: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    let trust = vault.read_primary_trust()?;
    let mut transaction = StateTransaction::begin(state_dir)?;
    if let Err(error) = transaction.prepare_trust_workspace(&trust) {
        let _ = transaction.rollback();
        return Err(error).context("prepare DB-primary recovery-policy trust workspace");
    }
    let operation = device
        .install_own_authority_snapshot(package.authority_snapshot())
        .context("advance recovery-policy authority high-water")
        .and_then(|_| {
            for membership in package.conversation_memberships() {
                device.install_conversation_membership(membership)?;
            }
            write_policy(state_dir)
        });
    if let Err(error) = operation {
        return match transaction.rollback() {
            Ok(()) => Err(error),
            Err(rollback) => Err(error.context(format!(
                "policy staging failed and trust rollback also failed: {rollback}"
            ))),
        };
    }
    if let Err(error) = vault.commit_primary_transaction(&transaction) {
        return match transaction.rollback() {
            Ok(()) => Err(error).context("commit recovery-policy state to DB-primary vault"),
            Err(rollback) => Err(anyhow::Error::new(error).context(format!(
                "policy vault commit failed and trust rollback also failed: {rollback}"
            ))),
        };
    }
    transaction.commit()?;
    vault.confirm_primary_shadow()?;
    Ok(())
}

fn verify_legacy_genesis_anchor(
    trust: &kilogram_state::VaultMutableRead,
    request: &AccountRecoveryPolicyTransitionRequest,
) -> Result<()> {
    ensure!(
        request.old_epoch() == 0 && request.previous_transition_id() == &[0_u8; 32],
        "a non-genesis policy transition requires an installed policy anchor"
    );
    let head = trust
        .records()
        .iter()
        .find(|record| record.relative_path() == RECOVERY_APPROVAL_HEAD_PATH)
        .context("legacy policy genesis requires an existing recovery approval head")?;
    let approval = AccountRootRecoveryApproval::decode_and_verify(head.content())?;
    ensure!(
        approval.account_id() == request.account_id()
            && approval.recovery_roster_digest() == &request.old_recovery_roster_digest()?,
        "legacy recovery approval head does not anchor the old transition roster"
    );
    Ok(())
}

fn optional_policy_state(
    trust: &kilogram_state::VaultMutableRead,
) -> Result<Option<AccountRecoveryPolicyState>> {
    trust
        .records()
        .iter()
        .find(|record| record.relative_path() == RECOVERY_POLICY_STATE_PATH)
        .map(|record| AccountRecoveryPolicyState::decode_and_verify(record.content()))
        .transpose()
        .context("decode DB-primary recovery-policy state")
}

fn optional_transition_approval(
    trust: &kilogram_state::VaultMutableRead,
) -> Result<Option<AccountRecoveryPolicyTransitionApproval>> {
    trust
        .records()
        .iter()
        .find(|record| record.relative_path() == RECOVERY_POLICY_APPROVAL_HEAD_PATH)
        .map(|record| AccountRecoveryPolicyTransitionApproval::decode_and_verify(record.content()))
        .transpose()
        .context("decode DB-primary recovery-policy transition approval head")
}

fn owned_memberships(
    trust: &kilogram_state::VaultMutableRead,
    account_id: kilogram_identity::AccountId,
) -> Result<Vec<ConversationMembershipSnapshot>> {
    let mut memberships = Vec::new();
    for record in trust.records() {
        if record.kind() == StateRecordKind::Trust
            && record.relative_path().starts_with(MEMBERSHIP_PREFIX)
        {
            let membership = ConversationMembershipSnapshot::decode_and_verify(record.content())?;
            if membership.owner_account_id() == account_id {
                memberships.push(membership);
            }
        }
    }
    Ok(memberships)
}

fn package_authorizes_certificate(
    package: &AccountRootRecoveryPackage,
    local: &DeviceCertificate,
) -> bool {
    package
        .device_list()
        .certificate_for(local.device_id())
        .is_some_and(|candidate| {
            candidate == local
                && verify_device_authorization_with_snapshot(
                    package.account_id(),
                    candidate,
                    package.authority_snapshot(),
                    &DeviceCapability::MESSAGING,
                )
                .is_ok()
        })
}

fn transition_request_output(
    request: &AccountRecoveryPolicyTransitionRequest,
    request_file: PathBuf,
) -> Result<RecoveryPolicyTransitionRequestOutput> {
    Ok(RecoveryPolicyTransitionRequestOutput {
        status: "account-recovery-policy-transition-request-created",
        account_id: request.account_id().to_string(),
        request_id: encode_hex(&request.request_id()?),
        old_epoch: request.old_epoch(),
        new_epoch: request.new_epoch(),
        previous_transition_id: encode_hex(request.previous_transition_id()),
        old_recovery_roster_digest: encode_hex(&request.old_recovery_roster_digest()?),
        new_recovery_roster_digest: encode_hex(&request.new_recovery_roster_digest()?),
        old_roster_count: request.old_roster_count(),
        new_roster_count: request.new_roster_count(),
        old_required_approvals: request.old_required_approvals(),
        new_required_approvals: request.new_required_approvals(),
        expires_at_unix_seconds: request.expires_at_unix_seconds(),
        request_file,
    })
}

fn transition_approval_output(
    approval: &AccountRecoveryPolicyTransitionApproval,
    approval_file: PathBuf,
    reused: bool,
) -> Result<RecoveryPolicyTransitionApprovalOutput> {
    Ok(RecoveryPolicyTransitionApprovalOutput {
        status: "account-recovery-policy-transition-approved",
        account_id: approval.account_id().to_string(),
        request_id: encode_hex(approval.request_id()),
        old_epoch: approval.old_epoch(),
        new_epoch: approval.new_epoch(),
        approver_device_id: approval.approver_device_id().to_string(),
        approval_id: encode_hex(&approval.approval_id()?),
        approval_file,
        approval_head_committed_before_publish: true,
        reused_committed_approval: reused,
    })
}

fn transition_certificate_output(
    certificate: &AccountRecoveryPolicyTransitionCertificate,
    certificate_file: PathBuf,
) -> Result<RecoveryPolicyTransitionCertificateOutput> {
    let report =
        verify_account_recovery_policy_transition(certificate.request(), certificate.approvals())?;
    report.require_joint_majority()?;
    Ok(RecoveryPolicyTransitionCertificateOutput {
        status: "account-recovery-policy-transition-certified",
        account_id: certificate.account_id().to_string(),
        certificate_id: encode_hex(&certificate.certificate_id()?),
        old_epoch: certificate.old_epoch(),
        new_epoch: certificate.new_epoch(),
        previous_transition_id: encode_hex(certificate.previous_transition_id()),
        old_recovery_roster_digest: encode_hex(&certificate.old_recovery_roster_digest()?),
        new_recovery_roster_digest: encode_hex(&certificate.new_recovery_roster_digest()?),
        old_observed_approvals: report.old_observed(),
        old_required_approvals: report.old_required(),
        new_observed_approvals: report.new_observed(),
        new_required_approvals: report.new_required(),
        joint_majority_satisfied: true,
        cross_roster_fork_safety: true,
        certificate_file,
    })
}

fn read_transition_request(
    path: &Path,
    now: u64,
) -> Result<AccountRecoveryPolicyTransitionRequest> {
    AccountRecoveryPolicyTransitionRequest::decode_and_verify(
        &read_bounded_regular_file(
            path,
            MAX_ACCOUNT_RECOVERY_POLICY_TRANSITION_REQUEST_BYTES,
            "recovery-policy transition request",
        )?,
        now,
    )
    .context("decode and verify current recovery-policy transition request")
}

fn read_transition_certificate(path: &Path) -> Result<AccountRecoveryPolicyTransitionCertificate> {
    AccountRecoveryPolicyTransitionCertificate::decode_and_verify(&read_bounded_regular_file(
        path,
        MAX_ACCOUNT_RECOVERY_POLICY_CERTIFICATE_BYTES,
        "recovery-policy transition certificate",
    )?)
    .context("decode and verify recovery-policy transition certificate")
}

fn read_package(path: &Path) -> Result<AccountRootRecoveryPackage> {
    AccountRootRecoveryPackage::decode_and_verify(&read_bounded_regular_file(
        path,
        MAX_ACCOUNT_ROOT_RECOVERY_PACKAGE_BYTES,
        "Account Root recovery package",
    )?)
    .context("decode Account Root recovery package")
}

fn read_witness(path: &Path) -> Result<AccountRootRecoveryWitness> {
    AccountRootRecoveryWitness::decode_and_verify(&read_bounded_regular_file(
        path,
        MAX_ACCOUNT_ROOT_RECOVERY_WITNESS_BYTES,
        "Account Root recovery witness",
    )?)
    .context("decode Account Root recovery witness")
}

fn decode_required_trust_record<T>(
    trust: &kilogram_state::VaultMutableRead,
    path: &str,
    decode: impl FnOnce(&[u8]) -> Result<T, kilogram_identity::IdentityError>,
) -> Result<T> {
    let record = trust
        .records()
        .iter()
        .find(|record| record.relative_path() == path)
        .with_context(|| format!("DB-primary trust record is missing: {path}"))?;
    ensure!(
        record.kind() == StateRecordKind::Trust,
        "unexpected record kind"
    );
    decode(record.content()).with_context(|| format!("decode DB-primary trust record {path}"))
}

fn read_bounded_regular_file(path: &Path, maximum: usize, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("inspect {label} {}", path.display()))?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "{label} must be a regular file"
    );
    let length = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
    ensure!(length <= maximum, "{label} exceeds maximum size");
    fs::read(path).with_context(|| format!("read {label} {}", path.display()))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("artifact has no parent")?;
    fs::create_dir_all(parent)?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(path)
        .map_err(|error| error.error)
        .with_context(|| format!("publish no-clobber artifact {}", path.display()))?;
    Ok(())
}

fn write_idempotent(path: &Path, bytes: &[u8]) -> Result<()> {
    match fs::read(path) {
        Ok(existing) => {
            ensure!(existing == bytes, "existing artifact has different content");
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => write_new(path, bytes),
        Err(error) => Err(error.into()),
    }
}

fn write_replace(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("policy path has no parent")?;
    fs::create_dir_all(parent)?;
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn combine_dual_write(primary: Result<()>, mirror: Result<()>) -> Result<()> {
    match (primary, mirror) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(mirror)) => Err(error.context(format!(
            "DB-primary operation failed and vault mirror also failed: {mirror:#}"
        ))),
    }
}

fn absolute_existing_path(path: &Path) -> Result<PathBuf> {
    fs::canonicalize(path).with_context(|| format!("resolve artifact {}", path.display()))
}

fn unix_time_now() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before Unix epoch")?
        .as_secs())
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

fn decode_hex_32(value: &str) -> Result<[u8; 32]> {
    ensure!(value.len() == 64, "expected 64 hexadecimal characters");
    let mut result = [0_u8; 32];
    for (index, byte) in result.iter_mut().enumerate() {
        let offset = index * 2;
        *byte = u8::from_str_radix(&value[offset..offset + 2], 16)
            .with_context(|| format!("invalid hexadecimal at character {offset}"))?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use iroh::{RelayMode, endpoint::Builder};
    use kilogram_identity::AccountRootState;

    use super::*;

    fn local_test_endpoint_builder() -> Result<Builder> {
        Ok(endpoint_builder_with_relay(RoutePolicy::Auto, None)
            .relay_mode(RelayMode::Disabled)
            .clear_ip_transports()
            .bind_addr((std::net::Ipv4Addr::LOCALHOST, 0))?)
    }

    #[test]
    fn one_to_two_transition_requires_both_and_unfreezes_new_roster() -> Result<(), Box<dyn Error>>
    {
        let parent = tempfile::tempdir()?;
        let owner = parent.path().join("owner");
        let first = crate::create_account(&owner)?;
        let old_package = parent.path().join("old.karp");
        let old_witness = parent.path().join("old.karw");
        crate::account_recovery::export_account_root(
            first.account_root_dir(),
            &old_package,
            &old_witness,
        )?;
        let old_request = parent.path().join("old.karq");
        crate::recovery_quorum::create_request(&old_package, &old_witness, &old_request, 600)?;
        crate::recovery_quorum::approve_request(
            first.state_dir(),
            &old_request,
            parent.path().join("old.kara"),
        )?;

        let joining = parent.path().join("joining");
        let link = crate::device_link::create_request(&joining, first.account_id())?;
        let response = parent.path().join("link.kdl");
        let device_list = owner.join("public").join("account-device-list.snapshot");
        crate::device_link::authorize_request(
            first.account_root_dir(),
            link.request_file(),
            link.sas(),
            &response,
            &device_list,
        )?;
        let joined = crate::device_link::accept_response(&joining, &response)?;
        let new_package = parent.path().join("new.karp");
        let new_witness = parent.path().join("new.karw");
        crate::account_recovery::export_account_root(
            first.account_root_dir(),
            &new_package,
            &new_witness,
        )?;
        let transition_request = parent.path().join("transition.karpt");
        create_transition_request(
            &old_package,
            &old_witness,
            &new_package,
            &new_witness,
            0,
            &"0".repeat(64),
            &transition_request,
            600,
        )?;
        let first_approval = parent.path().join("first.karpa");
        approve_transition(first.state_dir(), &transition_request, &first_approval)?;
        let conflicting_request = parent.path().join("conflicting.karpt");
        create_transition_request(
            &old_package,
            &old_witness,
            &new_package,
            &new_witness,
            0,
            &"0".repeat(64),
            &conflicting_request,
            600,
        )?;
        assert!(
            approve_transition(
                first.state_dir(),
                &conflicting_request,
                parent.path().join("conflicting.karpa"),
            )
            .is_err()
        );
        assert!(
            certify_transition(
                &transition_request,
                std::slice::from_ref(&first_approval),
                parent.path().join("partial.karpc"),
            )
            .is_err()
        );
        let second_approval = parent.path().join("second.karpa");
        approve_transition(joined.state_dir(), &transition_request, &second_approval)?;
        let policy_certificate = parent.path().join("transition.karpc");
        let certified = certify_transition(
            &transition_request,
            &[first_approval, second_approval],
            &policy_certificate,
        )?;
        assert!(certified.cross_roster_fork_safety);
        install_transition(first.state_dir(), &policy_certificate)?;
        install_transition(joining.join("device"), &policy_certificate)?;

        let new_request = parent.path().join("new.karq");
        crate::recovery_quorum::create_request(&new_package, &new_witness, &new_request, 600)?;
        let first_new_approval = parent.path().join("new-first.kara");
        crate::recovery_quorum::approve_request(
            first.state_dir(),
            &new_request,
            &first_new_approval,
        )?;
        let second_new_approval = parent.path().join("new-second.kara");
        crate::recovery_quorum::approve_request(
            joined.state_dir(),
            &new_request,
            &second_new_approval,
        )?;
        let plain = crate::recovery_quorum::verify_request(
            &new_request,
            &[first_new_approval.clone(), second_new_approval.clone()],
            true,
        )?;
        assert!(!plain.cross_roster_fork_safety());
        let policy_bound = crate::recovery_quorum::verify_request_with_policy_certificate(
            &new_request,
            &[first_new_approval, second_new_approval],
            &policy_certificate,
            true,
        )?;
        assert!(policy_bound.cross_roster_fork_safety());
        let vault = EncryptedStateVault::open_existing(first.state_dir())?;
        let trust = vault.read_primary_trust()?;
        let state = optional_policy_state(&trust)?.context("installed policy state is missing")?;
        assert_eq!(state.epoch(), 1);
        assert_eq!(
            AccountRootState::load(first.account_root_dir())?.account_id(),
            state.account_id()
        );
        Ok(())
    }

    #[tokio::test]
    async fn one_shot_transport_collects_joint_transition_majority() -> Result<(), Box<dyn Error>> {
        let parent = tempfile::tempdir()?;
        let owner = parent.path().join("owner");
        let first = crate::create_account(&owner)?;
        let old_package = parent.path().join("old.karp");
        let old_witness = parent.path().join("old.karw");
        crate::account_recovery::export_account_root(
            first.account_root_dir(),
            &old_package,
            &old_witness,
        )?;
        let old_request = parent.path().join("old.karq");
        crate::recovery_quorum::create_request(&old_package, &old_witness, &old_request, 600)?;
        crate::recovery_quorum::approve_request(
            first.state_dir(),
            &old_request,
            parent.path().join("old.kara"),
        )?;

        let joining = parent.path().join("joining");
        let link = crate::device_link::create_request(&joining, first.account_id())?;
        let response = parent.path().join("link.kdl");
        crate::device_link::authorize_request(
            first.account_root_dir(),
            link.request_file(),
            link.sas(),
            &response,
            owner.join("public").join("account-device-list.snapshot"),
        )?;
        let joined = crate::device_link::accept_response(&joining, &response)?;
        let new_package = parent.path().join("new.karp");
        let new_witness = parent.path().join("new.karw");
        crate::account_recovery::export_account_root(
            first.account_root_dir(),
            &new_package,
            &new_witness,
        )?;
        let request_file = parent.path().join("transition.karpt");
        create_transition_request(
            &old_package,
            &old_witness,
            &new_package,
            &new_witness,
            0,
            &"0".repeat(64),
            &request_file,
            600,
        )?;

        let first_ticket = parent.path().join("first.karpticket");
        let second_ticket = parent.path().join("second.karpticket");
        let first_endpoint = local_test_endpoint_builder()?
            .alpns(vec![POLICY_APPROVAL_ALPN.to_vec()])
            .bind()
            .await?;
        let second_endpoint = local_test_endpoint_builder()?
            .alpns(vec![POLICY_APPROVAL_ALPN.to_vec()])
            .bind()
            .await?;
        let first_listener = tokio::spawn({
            let state_dir = first.state_dir().to_path_buf();
            let request_file = request_file.clone();
            let ticket_file = first_ticket.clone();
            async move {
                listen_for_transition_approval_on_endpoint(
                    &state_dir,
                    &request_file,
                    &ticket_file,
                    RoutePolicy::Auto,
                    first_endpoint,
                )
                .await
            }
        });
        let second_listener = tokio::spawn({
            let state_dir = joined.state_dir().to_path_buf();
            let request_file = request_file.clone();
            let ticket_file = second_ticket.clone();
            async move {
                listen_for_transition_approval_on_endpoint(
                    &state_dir,
                    &request_file,
                    &ticket_file,
                    RoutePolicy::Auto,
                    second_endpoint,
                )
                .await
            }
        });
        timeout(Duration::from_secs(5), async {
            while !first_ticket.is_file() || !second_ticket.is_file() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await?;

        let request = read_transition_request(&request_file, unix_time_now()?)?;
        let ticket_text = fs::read_to_string(&first_ticket)?;
        let mut tampered_ticket = PolicyApprovalTicket::decode(&ticket_text)?;
        tampered_ticket.content.route_policy = RoutePolicy::DirectOnly;
        assert!(
            tampered_ticket
                .verify_for_request(&request, unix_time_now()?)
                .is_err()
        );

        let approval_dir = parent.path().join("transition-approvals");
        let collected = collect_transition_approvals(
            &request_file,
            &[first_ticket, second_ticket],
            &approval_dir,
            true,
            0,
        )
        .await?;
        let first_listened = first_listener.await??;
        let second_listened = second_listener.await??;
        assert_eq!(
            (
                collected.old_observed_approvals,
                collected.old_required_approvals,
                collected.new_observed_approvals,
                collected.new_required_approvals,
            ),
            (1, 1, 2, 2)
        );
        assert!(collected.joint_majority_satisfied);
        assert!(!collected.cross_roster_fork_safety);
        assert!(
            collected
                .transports
                .iter()
                .all(|transport| transport.transport_path == "direct")
        );
        assert!(first_listened.approval_head_committed_before_ticket_publish);
        assert!(second_listened.approval_head_committed_before_ticket_publish);

        let approval_files = collected
            .transports
            .iter()
            .map(|transport| transport.approval_file.clone())
            .collect::<Vec<_>>();
        let certificate = certify_transition(
            &request_file,
            &approval_files,
            parent.path().join("transition.karpc"),
        )?;
        assert!(certificate.joint_majority_satisfied);
        assert!(certificate.cross_roster_fork_safety);
        Ok(())
    }
}
