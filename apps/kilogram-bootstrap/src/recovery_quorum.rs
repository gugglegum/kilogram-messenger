use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use iroh::{Endpoint, EndpointAddr, RelayUrl, endpoint::Connection};
use kilogram_identity::{
    AccountAuthoritySnapshot, AccountRecoveryPolicyState,
    AccountRecoveryPolicyTransitionCertificate, AccountRootRecoveryApproval,
    AccountRootRecoveryApprovalRequest, AccountRootRecoveryPackage, AccountRootRecoveryWitness,
    ConversationMembershipSnapshot, DeviceCertificate, DeviceIdentity, DeviceState,
    MAX_ACCOUNT_DEVICES, MAX_ACCOUNT_RECOVERY_POLICY_CERTIFICATE_BYTES,
    MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_BYTES, MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_REQUEST_BYTES,
    MAX_ACCOUNT_ROOT_RECOVERY_PACKAGE_BYTES, MAX_ACCOUNT_ROOT_RECOVERY_WITNESS_BYTES,
    verify_account_root_recovery_quorum, verify_device_authorization_with_snapshot,
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
const APPROVAL_HEAD_PATH: &str = "recovery-approval/latest.approval";
const RECOVERY_APPROVAL_ALPN: &[u8] = b"kilogram/m0/account-recovery-quorum/1";
const RECOVERY_APPROVAL_TICKET_VERSION: u8 = 1;
const RECOVERY_APPROVAL_FETCH_VERSION: u8 = 1;
const RECOVERY_APPROVAL_TICKET_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:account-root-recovery-approval-ticket:v1\0";
const MAX_RECOVERY_APPROVAL_TICKET_BYTES: usize = 256 * 1024;
const MAX_RECOVERY_APPROVAL_FETCH_BYTES: usize = 16 * 1024;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);
const ROUTE_POLICY_WAIT: Duration = Duration::from_secs(15);
const WIRE_IO_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryQuorumRequestOutput {
    status: &'static str,
    account_id: String,
    request_id: String,
    package_id: String,
    authority_revision: u64,
    recovery_roster_digest: String,
    roster_count: usize,
    required_approvals: usize,
    issued_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
    request_file: PathBuf,
    freshness_scope: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryQuorumApprovalOutput {
    status: &'static str,
    account_id: String,
    request_id: String,
    package_id: String,
    authority_revision: u64,
    recovery_roster_digest: String,
    approver_device_id: String,
    previous_approval_head_digest: String,
    approval_id: String,
    approval_file: PathBuf,
    approval_head_source: &'static str,
    approval_head_committed_before_publish: bool,
    reused_committed_approval: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryQuorumVerifyOutput {
    status: &'static str,
    account_id: String,
    request_id: String,
    package_id: String,
    authority_revision: u64,
    recovery_roster_digest: String,
    roster_count: usize,
    observed_approvals: usize,
    required_approvals: usize,
    majority_satisfied: bool,
    freshness_claim: &'static str,
    cross_roster_fork_safety: bool,
}

impl RecoveryQuorumVerifyOutput {
    pub fn cross_roster_fork_safety(&self) -> bool {
        self.cross_roster_fork_safety
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryQuorumListenOutput {
    status: &'static str,
    account_id: String,
    request_id: String,
    package_id: String,
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
pub struct RecoveryQuorumTransportObservation {
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
pub struct RecoveryQuorumCollectOutput {
    status: &'static str,
    account_id: String,
    request_id: String,
    package_id: String,
    authority_revision: u64,
    recovery_roster_digest: String,
    roster_count: usize,
    observed_approvals: usize,
    required_approvals: usize,
    majority_satisfied: bool,
    freshness_claim: &'static str,
    cross_roster_fork_safety: bool,
    approval_directory: PathBuf,
    transports: Vec<RecoveryQuorumTransportObservation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct RecoveryApprovalTicketContent {
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
struct RecoveryApprovalTicket {
    content: RecoveryApprovalTicketContent,
    signature: Vec<u8>,
}

impl RecoveryApprovalTicket {
    fn issue(
        request: &AccountRootRecoveryApprovalRequest,
        endpoint: EndpointAddr,
        approver_identity: &DeviceIdentity,
        approver_certificate: DeviceCertificate,
        bearer_token: [u8; 32],
        route_policy: RoutePolicy,
    ) -> Result<Self> {
        ensure!(
            approver_certificate.device_id() == approver_identity.device_id(),
            "recovery approval ticket certificate belongs to a different device"
        );
        let content = RecoveryApprovalTicketContent {
            version: RECOVERY_APPROVAL_TICKET_VERSION,
            endpoint,
            account_id: request.account_id(),
            request_id: request.request_id()?,
            request_expires_at_unix_seconds: request.expires_at_unix_seconds(),
            approver_certificate,
            bearer_token,
            route_policy,
        };
        let signature = approver_identity
            .sign(&recovery_ticket_signing_bytes(&content)?)
            .to_vec();
        let ticket = Self { content, signature };
        ticket.verify_for_request(request, unix_time_now()?)?;
        Ok(ticket)
    }

    fn encode(&self) -> Result<String> {
        let encoded = serde_json::to_vec(self).context("serialize recovery approval ticket")?;
        ensure!(
            encoded.len() <= MAX_RECOVERY_APPROVAL_TICKET_BYTES,
            "recovery approval ticket is too large"
        );
        Ok(URL_SAFE_NO_PAD.encode(encoded))
    }

    fn decode(encoded: &str) -> Result<Self> {
        ensure!(
            encoded.len() <= MAX_RECOVERY_APPROVAL_TICKET_BYTES.saturating_mul(2),
            "encoded recovery approval ticket is too large"
        );
        let bytes = URL_SAFE_NO_PAD
            .decode(encoded.trim())
            .context("decode recovery approval ticket as base64url")?;
        ensure!(
            bytes.len() <= MAX_RECOVERY_APPROVAL_TICKET_BYTES,
            "recovery approval ticket is too large"
        );
        serde_json::from_slice(&bytes).context("decode recovery approval ticket")
    }

    fn verify_for_request(
        &self,
        request: &AccountRootRecoveryApprovalRequest,
        now: u64,
    ) -> Result<()> {
        request.verify_at(now)?;
        ensure!(
            self.content.version == RECOVERY_APPROVAL_TICKET_VERSION,
            "unsupported recovery approval ticket version"
        );
        ensure!(
            self.content.account_id == request.account_id()
                && self.content.request_id == request.request_id()?
                && self.content.request_expires_at_unix_seconds
                    == request.expires_at_unix_seconds(),
            "recovery approval ticket is bound to a different request"
        );
        ensure!(
            now <= self.content.request_expires_at_unix_seconds,
            "recovery approval ticket has expired"
        );
        self.content.approver_certificate.verify()?;
        let approver = self.content.approver_certificate.device_id();
        ensure!(
            request.package().device_list().certificate_for(approver)
                == Some(&self.content.approver_certificate),
            "recovery approval ticket certificate is not exact in the candidate roster"
        );
        verify_device_authorization_with_snapshot(
            request.account_id(),
            &self.content.approver_certificate,
            request.package().authority_snapshot(),
            &kilogram_identity::DeviceCapability::MESSAGING,
        )
        .context("authorize recovery approval ticket signer")?;
        approver
            .verify(
                &recovery_ticket_signing_bytes(&self.content)?,
                &self.signature,
            )
            .context("verify device signature on recovery approval ticket")?;
        if self.content.route_policy == RoutePolicy::RelayOnly {
            ensure!(
                self.content.endpoint.relay_urls().next().is_some(),
                "relay-only recovery approval ticket has no relay address"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct RecoveryApprovalFetch {
    version: u8,
    request_id: [u8; 32],
    bearer_token: [u8; 32],
}

struct CommittedApproval {
    approval: AccountRootRecoveryApproval,
    reused: bool,
    ticket: Option<RecoveryApprovalTicket>,
}

pub fn create_request(
    package_file: impl AsRef<Path>,
    witness_file: impl AsRef<Path>,
    request_file: impl AsRef<Path>,
    validity_seconds: u64,
) -> Result<RecoveryQuorumRequestOutput> {
    let package = AccountRootRecoveryPackage::decode_and_verify(&read_bounded_regular_file(
        package_file.as_ref(),
        MAX_ACCOUNT_ROOT_RECOVERY_PACKAGE_BYTES,
        "Account Root recovery package",
    )?)
    .context("decode and authenticate Account Root recovery package")?;
    let witness = AccountRootRecoveryWitness::decode_and_verify(&read_bounded_regular_file(
        witness_file.as_ref(),
        MAX_ACCOUNT_ROOT_RECOVERY_WITNESS_BYTES,
        "Account Root recovery witness",
    )?)
    .context("decode and authenticate Account Root recovery witness")?;
    witness
        .verify_package(&package)
        .context("verify witness against exact recovery package")?;
    let now = unix_time_now()?;
    let request =
        AccountRootRecoveryApprovalRequest::issue(package, witness, now, validity_seconds)
            .context("issue recovery approval request")?;
    write_new(request_file.as_ref(), &request.encode()?)?;
    request_output(&request, absolute_existing_path(request_file.as_ref())?)
}

pub fn approve_request(
    state_dir: impl AsRef<Path>,
    request_file: impl AsRef<Path>,
    approval_file: impl AsRef<Path>,
) -> Result<RecoveryQuorumApprovalOutput> {
    let now = unix_time_now()?;
    let request = AccountRootRecoveryApprovalRequest::decode_and_verify(
        &read_bounded_regular_file(
            request_file.as_ref(),
            MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_REQUEST_BYTES,
            "recovery approval request",
        )?,
        now,
    )
    .context("decode and verify current recovery approval request")?;
    let committed = approve_decoded_request(state_dir.as_ref(), &request, None, now)?;
    let encoded = committed.approval.encode()?;
    write_idempotent(approval_file.as_ref(), &encoded)?;
    approval_output(
        &committed.approval,
        absolute_existing_path(approval_file.as_ref())?,
        committed.reused,
    )
}

fn approve_decoded_request(
    state_dir: &Path,
    request: &AccountRootRecoveryApprovalRequest,
    ticket_draft: Option<(EndpointAddr, [u8; 32], RoutePolicy)>,
    now: u64,
) -> Result<CommittedApproval> {
    let state_dir = fs::canonicalize(state_dir)
        .with_context(|| format!("resolve approving device state {}", state_dir.display()))?;
    ensure!(
        state_dir.is_dir(),
        "approving device state is not a directory"
    );
    let _lock = StateDirectoryLock::acquire(&state_dir).context("lock approving device state")?;
    ensure!(
        EncryptedStateVault::is_initialized(&state_dir)?,
        "current-device recovery approval requires an initialized DB-primary vault"
    );
    let vault = EncryptedStateVault::open_existing(&state_dir)
        .context("open approving device DB-primary vault")?;
    vault
        .recover_primary_shadow()
        .context("recover interrupted approval trust shadow")?;
    vault
        .recover_pending_dual_write()
        .context("recover interrupted approval vault mirror")?;

    request
        .verify_at(now)
        .context("verify current recovery approval request")?;
    let identity = vault
        .read_primary_device_identity()
        .context("read authenticated approving device identity")?;
    let device = DeviceState::from_secret_material(
        &state_dir,
        *identity.signing_secret(),
        *identity.encryption_secret(),
    );
    let trust = vault
        .read_primary_trust()
        .context("read authenticated approving-device trust state")?;
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
    let mut memberships = Vec::new();
    for record in trust.records() {
        if record.kind() == StateRecordKind::Trust
            && record.relative_path().starts_with(MEMBERSHIP_PREFIX)
        {
            let membership = ConversationMembershipSnapshot::decode_and_verify(record.content())
                .with_context(|| {
                    format!("decode DB-primary membership {}", record.relative_path())
                })?;
            if membership.owner_account_id() == request.account_id() {
                memberships.push(membership);
            }
        }
    }
    request
        .package()
        .verify_dominates_device_state(&certificate, &authority, &memberships)
        .context("reject rollback, equivocation, roster mismatch, or revoked approver")?;

    let existing_head = trust
        .records()
        .iter()
        .find(|record| record.relative_path() == APPROVAL_HEAD_PATH)
        .map(|record| AccountRootRecoveryApproval::decode_and_verify(record.content()))
        .transpose()
        .context("decode DB-primary recovery approval head")?;
    let policy_state = trust
        .records()
        .iter()
        .find(|record| record.relative_path() == crate::recovery_policy::RECOVERY_POLICY_STATE_PATH)
        .map(|record| AccountRecoveryPolicyState::decode_and_verify(record.content()))
        .transpose()
        .context("decode DB-primary recovery-policy state")?;
    if let Some(existing) = existing_head.as_ref()
        && existing.request_id() == &request.request_id()?
    {
        existing
            .verify_for_request(request, now)
            .context("verify idempotent committed recovery approval")?;
        ensure!(
            existing.approver_device_id() == device.identity().device_id(),
            "committed recovery approval head belongs to a different device"
        );
        let ticket = ticket_draft
            .map(|(endpoint, bearer_token, route_policy)| {
                RecoveryApprovalTicket::issue(
                    request,
                    endpoint,
                    device.identity(),
                    certificate.clone(),
                    bearer_token,
                    route_policy,
                )
            })
            .transpose()?;
        return Ok(CommittedApproval {
            approval: existing.clone(),
            reused: true,
            ticket,
        });
    }

    let previous_approval_head_digest = match existing_head.as_ref() {
        Some(existing) => {
            ensure!(
                existing.account_id() == request.account_id(),
                "recovery approval head belongs to a different account"
            );
            if existing.recovery_roster_digest() != &request.package().recovery_roster_digest()? {
                let policy = policy_state.as_ref().ok_or_else(|| {
                    anyhow!(
                        kilogram_identity::IdentityError::AccountRootRecoveryApprovalRosterChanged
                    )
                })?;
                ensure!(
                    policy.account_id() == request.account_id()
                        && policy.recovery_roster_digest()
                            == &request.package().recovery_roster_digest()?,
                    "recovery candidate does not match the installed recovery-policy roster"
                );
                let certificate_record = trust
                    .records()
                    .iter()
                    .find(|record| {
                        record.relative_path()
                            == crate::recovery_policy::RECOVERY_POLICY_CERTIFICATE_PATH
                    })
                    .context(
                        "changed recovery roster requires an installed transition certificate",
                    )?;
                let transition = AccountRecoveryPolicyTransitionCertificate::decode_and_verify(
                    certificate_record.content(),
                )
                .context("decode installed recovery-policy transition certificate")?;
                ensure!(
                    transition.certificate_id()? == *policy.latest_transition_id()
                        && transition.old_recovery_roster_digest()?
                            == *existing.recovery_roster_digest(),
                    "installed policy transition does not bridge the committed approval roster"
                );
                crate::recovery_policy::verify_candidate_against_certificate(
                    request.package(),
                    &transition,
                )?;
            }
            ensure!(
                request.package().authority_revision() >= existing.authority_revision(),
                "recovery candidate authority revision is behind the committed approval head"
            );
            existing.approval_id()?
        }
        None => [0_u8; 32],
    };
    if let Some(policy) = policy_state.as_ref() {
        ensure!(
            policy.account_id() == request.account_id()
                && policy.recovery_roster_digest()
                    == &request.package().recovery_roster_digest()?,
            "recovery candidate does not match the installed recovery-policy roster"
        );
    }
    let policy = policy_state.unwrap_or(AccountRecoveryPolicyState::genesis(request.package())?);
    let approval = AccountRootRecoveryApproval::issue(
        device.identity(),
        request,
        previous_approval_head_digest,
        now,
    )
    .context("sign recovery approval")?;
    let encoded = approval.encode()?;

    vault
        .begin_dual_write()
        .context("prepare approval vault mirror intent")?;
    let commit_result = commit_approval_head_primary(
        &state_dir,
        &vault,
        &device,
        request.package(),
        &policy.encode()?,
        &encoded,
    );
    let mirror_result = vault
        .finish_dual_write()
        .context("complete approval vault mirror");
    match (commit_result, mirror_result) {
        (Ok(()), Ok(_)) => {}
        (Err(error), Ok(_)) => return Err(error),
        (Ok(()), Err(error)) => return Err(error),
        (Err(error), Err(mirror)) => {
            return Err(error.context(format!(
                "approval commit failed and vault mirror also failed: {mirror:#}"
            )));
        }
    }

    let ticket = ticket_draft
        .map(|(endpoint, bearer_token, route_policy)| {
            RecoveryApprovalTicket::issue(
                request,
                endpoint,
                device.identity(),
                certificate,
                bearer_token,
                route_policy,
            )
        })
        .transpose()?;
    Ok(CommittedApproval {
        approval,
        reused: false,
        ticket,
    })
}

pub async fn listen_for_approval_collection(
    state_dir: impl AsRef<Path>,
    request_file: impl AsRef<Path>,
    ticket_file: impl AsRef<Path>,
    route_policy: RoutePolicy,
    relay_url: Option<RelayUrl>,
    relay_wait_seconds: u64,
) -> Result<RecoveryQuorumListenOutput> {
    let endpoint = endpoint_builder_with_relay(route_policy, relay_url)
        .alpns(vec![RECOVERY_APPROVAL_ALPN.to_vec()])
        .bind()
        .await
        .context("bind recovery approval listener")?;
    wait_for_relay(&endpoint, route_policy, relay_wait_seconds).await?;
    listen_on_endpoint(
        state_dir.as_ref(),
        request_file.as_ref(),
        ticket_file.as_ref(),
        route_policy,
        endpoint,
    )
    .await
}

async fn listen_on_endpoint(
    state_dir: &Path,
    request_file: &Path,
    ticket_file: &Path,
    route_policy: RoutePolicy,
    endpoint: Endpoint,
) -> Result<RecoveryQuorumListenOutput> {
    let now = unix_time_now()?;
    let request = AccountRootRecoveryApprovalRequest::decode_and_verify(
        &read_bounded_regular_file(
            request_file,
            MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_REQUEST_BYTES,
            "recovery approval request",
        )?,
        now,
    )
    .context("decode and verify network recovery approval request")?;
    let mut bearer_token = [0_u8; 32];
    getrandom::fill(&mut bearer_token).context("generate recovery approval bearer token")?;
    let committed = approve_decoded_request(
        state_dir,
        &request,
        Some((endpoint.addr(), bearer_token, route_policy)),
        now,
    )?;
    let ticket = committed
        .ticket
        .as_ref()
        .context("network approval did not produce a ticket")?;
    write_new(ticket_file, ticket.encode()?.as_bytes())?;
    let ticket_file = absolute_existing_path(ticket_file)?;

    let remaining = request
        .expires_at_unix_seconds()
        .checked_sub(unix_time_now()?)
        .context("recovery approval request expired before listener publication")?;
    let connection =
        accept_authenticated_connection(&endpoint, Duration::from_secs(remaining)).await?;
    let diagnostics = await_route_policy(&connection, route_policy, ROUTE_POLICY_WAIT)
        .await
        .context("wait for a path allowed by the recovery approval ticket")?;
    let (mut send, mut receive) = timeout(WIRE_IO_TIMEOUT, connection.accept_bi())
        .await
        .context("accept recovery approval stream timed out")?
        .context("accept recovery approval stream")?;
    let fetch_bytes = timeout(
        WIRE_IO_TIMEOUT,
        receive.read_to_end(MAX_RECOVERY_APPROVAL_FETCH_BYTES),
    )
    .await
    .context("read recovery approval fetch timed out")?
    .context("read recovery approval fetch")?;
    let fetch: RecoveryApprovalFetch =
        postcard::from_bytes(&fetch_bytes).context("decode recovery approval fetch")?;
    ensure!(
        fetch.version == RECOVERY_APPROVAL_FETCH_VERSION
            && fetch.request_id == request.request_id()?
            && fetch.bearer_token == bearer_token,
        "recovery approval fetch is not authorized by this one-shot ticket"
    );
    let approval_bytes = committed.approval.encode()?;
    timeout(WIRE_IO_TIMEOUT, send.write_all(&approval_bytes))
        .await
        .context("send recovery approval timed out")?
        .context("send recovery approval")?;
    send.finish().context("finish recovery approval response")?;
    let _ = timeout(WIRE_IO_TIMEOUT, connection.closed()).await;
    endpoint.close().await;

    Ok(RecoveryQuorumListenOutput {
        status: "account-recovery-quorum-approved-over-transport",
        account_id: committed.approval.account_id().to_string(),
        request_id: encode_hex(committed.approval.request_id()),
        package_id: encode_hex(committed.approval.package_id()),
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

pub async fn collect_approvals(
    request_file: impl AsRef<Path>,
    ticket_files: &[PathBuf],
    approval_directory: impl AsRef<Path>,
    require_majority: bool,
    relay_wait_seconds: u64,
) -> Result<RecoveryQuorumCollectOutput> {
    ensure!(
        !ticket_files.is_empty(),
        "at least one approval ticket is required"
    );
    ensure!(
        ticket_files.len() <= MAX_ACCOUNT_DEVICES,
        "too many recovery approval tickets"
    );
    let now = unix_time_now()?;
    let request = AccountRootRecoveryApprovalRequest::decode_and_verify(
        &read_bounded_regular_file(
            request_file.as_ref(),
            MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_REQUEST_BYTES,
            "recovery approval request",
        )?,
        now,
    )?;
    let approval_directory = resolve_approval_directory(approval_directory.as_ref())?;
    let mut ticket_approvers = BTreeSet::new();
    let mut approval_files = Vec::with_capacity(ticket_files.len());
    let mut transports = Vec::with_capacity(ticket_files.len());

    for ticket_file in ticket_files {
        let ticket_text = String::from_utf8(read_bounded_regular_file(
            ticket_file,
            MAX_RECOVERY_APPROVAL_TICKET_BYTES.saturating_mul(2),
            "recovery approval ticket",
        )?)
        .context("recovery approval ticket is not UTF-8")?;
        let ticket = RecoveryApprovalTicket::decode(&ticket_text)?;
        ticket.verify_for_request(&request, unix_time_now()?)?;
        let approver = ticket.content.approver_certificate.device_id();
        ensure!(
            ticket_approvers.insert(approver.to_string()),
            "duplicate recovery approval ticket for device {approver}"
        );
        let (approval, diagnostics) =
            collect_one_approval(&request, &ticket, relay_wait_seconds).await?;
        ensure!(
            approval.approver_device_id() == approver,
            "recovery approval response came from a different device"
        );
        let approval_file = approval_directory.join(format!("{approver}.kara"));
        write_idempotent(&approval_file, &approval.encode()?)?;
        let approval_file = absolute_existing_path(&approval_file)?;
        approval_files.push(approval_file.clone());
        transports.push(RecoveryQuorumTransportObservation {
            approver_device_id: approver.to_string(),
            ticket_file: absolute_existing_path(ticket_file)?,
            approval_file,
            route_policy: ticket.content.route_policy.as_str(),
            transport_path: diagnostics.kind.as_str(),
            transport_remote_address: diagnostics.remote_address,
            transport_rtt_milliseconds: duration_milliseconds(diagnostics.round_trip_time),
            transport_open_paths: diagnostics.open_paths,
        });
    }

    let verified = verify_request(request_file, &approval_files, false)?;
    if require_majority {
        ensure!(
            verified.majority_satisfied,
            "strict current-device majority was not collected"
        );
    }
    Ok(RecoveryQuorumCollectOutput {
        status: if verified.majority_satisfied {
            "account-recovery-quorum-majority-collected"
        } else {
            "account-recovery-quorum-partial-collected"
        },
        account_id: verified.account_id,
        request_id: verified.request_id,
        package_id: verified.package_id,
        authority_revision: verified.authority_revision,
        recovery_roster_digest: verified.recovery_roster_digest,
        roster_count: verified.roster_count,
        observed_approvals: verified.observed_approvals,
        required_approvals: verified.required_approvals,
        majority_satisfied: verified.majority_satisfied,
        freshness_claim: verified.freshness_claim,
        cross_roster_fork_safety: verified.cross_roster_fork_safety,
        approval_directory,
        transports,
    })
}

async fn collect_one_approval(
    request: &AccountRootRecoveryApprovalRequest,
    ticket: &RecoveryApprovalTicket,
    relay_wait_seconds: u64,
) -> Result<(AccountRootRecoveryApproval, SelectedPathDiagnostics)> {
    let route_policy = ticket.content.route_policy;
    let endpoint = endpoint_builder_for_remote(route_policy, &ticket.content.endpoint)?
        .alpns(vec![RECOVERY_APPROVAL_ALPN.to_vec()])
        .bind()
        .await
        .context("bind recovery approval collector")?;
    wait_for_relay(&endpoint, route_policy, relay_wait_seconds).await?;
    let connection = timeout(
        CONNECTION_TIMEOUT,
        endpoint.connect(ticket.content.endpoint.clone(), RECOVERY_APPROVAL_ALPN),
    )
    .await
    .context("connect recovery approval endpoint timed out")?
    .context("connect recovery approval endpoint")?;
    let diagnostics = await_route_policy(&connection, route_policy, ROUTE_POLICY_WAIT)
        .await
        .context("wait for a path allowed by recovery approval ticket")?;
    let (mut send, mut receive) = timeout(WIRE_IO_TIMEOUT, connection.open_bi())
        .await
        .context("open recovery approval stream timed out")?
        .context("open recovery approval stream")?;
    let fetch = RecoveryApprovalFetch {
        version: RECOVERY_APPROVAL_FETCH_VERSION,
        request_id: request.request_id()?,
        bearer_token: ticket.content.bearer_token,
    };
    let fetch_bytes = postcard::to_allocvec(&fetch).context("encode recovery approval fetch")?;
    ensure!(
        fetch_bytes.len() <= MAX_RECOVERY_APPROVAL_FETCH_BYTES,
        "recovery approval fetch is too large"
    );
    timeout(WIRE_IO_TIMEOUT, send.write_all(&fetch_bytes))
        .await
        .context("send recovery approval fetch timed out")?
        .context("send recovery approval fetch")?;
    send.finish().context("finish recovery approval fetch")?;
    let approval_bytes = timeout(
        WIRE_IO_TIMEOUT,
        receive.read_to_end(MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_BYTES),
    )
    .await
    .context("read recovery approval timed out")?
    .context("read recovery approval")?;
    let approval = AccountRootRecoveryApproval::decode_and_verify(&approval_bytes)?;
    approval
        .verify_for_request(request, unix_time_now()?)
        .context("verify transported recovery approval")?;
    connection.close(0_u32.into(), b"recovery approval collected");
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
                .context("recovery approval endpoint closed before a connection arrived")?;
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
    .context("recovery approval listener reached the request expiry")?
}

fn recovery_ticket_signing_bytes(content: &RecoveryApprovalTicketContent) -> Result<Vec<u8>> {
    let encoded = serde_json::to_vec(content).context("serialize recovery ticket content")?;
    let mut bytes =
        Vec::with_capacity(RECOVERY_APPROVAL_TICKET_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(RECOVERY_APPROVAL_TICKET_SIGNATURE_DOMAIN);
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

pub fn verify_request(
    request_file: impl AsRef<Path>,
    approval_files: &[PathBuf],
    require_majority: bool,
) -> Result<RecoveryQuorumVerifyOutput> {
    let now = unix_time_now()?;
    let request = AccountRootRecoveryApprovalRequest::decode_and_verify(
        &read_bounded_regular_file(
            request_file.as_ref(),
            MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_REQUEST_BYTES,
            "recovery approval request",
        )?,
        now,
    )?;
    let mut approvals = Vec::with_capacity(approval_files.len());
    for path in approval_files {
        approvals.push(AccountRootRecoveryApproval::decode_and_verify(
            &read_bounded_regular_file(
                path,
                MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_BYTES,
                "recovery device approval",
            )?,
        )?);
    }
    let report = verify_account_root_recovery_quorum(&request, &approvals, now)
        .context("verify exact recovery device quorum")?;
    if require_majority {
        report
            .require_majority()
            .context("require strict current-device majority")?;
    }
    Ok(RecoveryQuorumVerifyOutput {
        status: "account-recovery-quorum-verified",
        account_id: report.account_id().to_string(),
        request_id: encode_hex(report.request_id()),
        package_id: encode_hex(report.package_id()),
        authority_revision: request.package().authority_revision(),
        recovery_roster_digest: encode_hex(report.recovery_roster_digest()),
        roster_count: report.roster_count(),
        observed_approvals: report.observed_approvals(),
        required_approvals: report.required_approvals(),
        majority_satisfied: report.require_majority().is_ok(),
        freshness_claim: report.claim().as_str(),
        cross_roster_fork_safety: false,
    })
}

pub fn verify_request_with_policy_certificate(
    request_file: impl AsRef<Path>,
    approval_files: &[PathBuf],
    policy_certificate_file: impl AsRef<Path>,
    require_majority: bool,
) -> Result<RecoveryQuorumVerifyOutput> {
    let mut output = verify_request(&request_file, approval_files, require_majority)?;
    let now = unix_time_now()?;
    let request = AccountRootRecoveryApprovalRequest::decode_and_verify(
        &read_bounded_regular_file(
            request_file.as_ref(),
            MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_REQUEST_BYTES,
            "recovery approval request",
        )?,
        now,
    )?;
    let certificate =
        AccountRecoveryPolicyTransitionCertificate::decode_and_verify(&read_bounded_regular_file(
            policy_certificate_file.as_ref(),
            MAX_ACCOUNT_RECOVERY_POLICY_CERTIFICATE_BYTES,
            "recovery-policy transition certificate",
        )?)?;
    crate::recovery_policy::verify_candidate_against_certificate(request.package(), &certificate)?;
    output.cross_roster_fork_safety = output.majority_satisfied;
    Ok(output)
}

fn commit_approval_head_primary(
    state_dir: &Path,
    vault: &EncryptedStateVault,
    device: &DeviceState,
    package: &AccountRootRecoveryPackage,
    policy_bytes: &[u8],
    approval_bytes: &[u8],
) -> Result<()> {
    let trust = vault
        .read_primary_trust()
        .context("re-read approval trust baseline")?;
    let mut transaction = StateTransaction::begin(state_dir)
        .context("prepare crash-consistent approval transaction")?;
    if let Err(error) = transaction.prepare_trust_workspace(&trust) {
        let _ = transaction.rollback();
        return Err(error).context("prepare DB-primary approval trust workspace");
    }
    let operation = device
        .install_own_authority_snapshot(package.authority_snapshot())
        .context("advance approving device authority high-water")
        .and_then(|_| {
            for membership in package.conversation_memberships() {
                device
                    .install_conversation_membership(membership)
                    .context("advance approving device membership high-water")?;
            }
            write_approval_head(
                &state_dir.join(crate::recovery_policy::RECOVERY_POLICY_STATE_PATH),
                policy_bytes,
            )?;
            write_approval_head(&state_dir.join(APPROVAL_HEAD_PATH), approval_bytes)
        });
    if let Err(error) = operation {
        return match transaction.rollback() {
            Ok(()) => Err(error),
            Err(rollback) => Err(error.context(format!(
                "approval staging failed and trust rollback also failed: {rollback}"
            ))),
        };
    }
    if let Err(error) = vault.commit_primary_transaction(&transaction) {
        return match transaction.rollback() {
            Ok(()) => Err(error).context("commit recovery approval head to DB-primary vault"),
            Err(rollback) => Err(anyhow::Error::new(error).context(format!(
                "vault approval commit failed and trust rollback also failed: {rollback}"
            ))),
        };
    }
    transaction
        .commit()
        .context("commit retained recovery approval shadow")?;
    vault
        .confirm_primary_shadow()
        .context("confirm retained recovery approval shadow")?;
    Ok(())
}

fn write_approval_head(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("approval head has no parent")?;
    fs::create_dir_all(parent).context("create recovery approval trust directory")?;
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .with_context(|| format!("stage recovery approval head {}", path.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
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

fn request_output(
    request: &AccountRootRecoveryApprovalRequest,
    request_file: PathBuf,
) -> Result<RecoveryQuorumRequestOutput> {
    Ok(RecoveryQuorumRequestOutput {
        status: "account-recovery-quorum-request-created",
        account_id: request.account_id().to_string(),
        request_id: encode_hex(&request.request_id()?),
        package_id: encode_hex(&request.package().package_id()?),
        authority_revision: request.package().authority_revision(),
        recovery_roster_digest: encode_hex(&request.package().recovery_roster_digest()?),
        roster_count: request.roster_count(),
        required_approvals: request.required_approvals(),
        issued_at_unix_seconds: request.issued_at_unix_seconds(),
        expires_at_unix_seconds: request.expires_at_unix_seconds(),
        request_file,
        freshness_scope: "exact-roster-current-device-observation",
    })
}

fn approval_output(
    approval: &AccountRootRecoveryApproval,
    approval_file: PathBuf,
    reused_committed_approval: bool,
) -> Result<RecoveryQuorumApprovalOutput> {
    Ok(RecoveryQuorumApprovalOutput {
        status: "account-recovery-quorum-approved",
        account_id: approval.account_id().to_string(),
        request_id: encode_hex(approval.request_id()),
        package_id: encode_hex(approval.package_id()),
        authority_revision: approval.authority_revision(),
        recovery_roster_digest: encode_hex(approval.recovery_roster_digest()),
        approver_device_id: approval.approver_device_id().to_string(),
        previous_approval_head_digest: encode_hex(approval.previous_approval_head_digest()),
        approval_id: encode_hex(&approval.approval_id()?),
        approval_file,
        approval_head_source: "db-primary",
        approval_head_committed_before_publish: true,
        reused_committed_approval,
    })
}

fn read_bounded_regular_file(path: &Path, maximum: usize, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("inspect {label} {}", path.display()))?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "{label} must not be a symbolic link"
    );
    ensure!(metadata.is_file(), "{label} must be a regular file");
    ensure!(
        metadata.len() <= maximum as u64,
        "{label} is too large: {} bytes; maximum is {maximum}",
        metadata.len()
    );
    fs::read(path).with_context(|| format!("read {label} {}", path.display()))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let absolute = resolve_output_path(path)?;
    let parent = absolute.parent().context("artifact output has no parent")?;
    let mut temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("create temporary artifact in {}", parent.display()))?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(&absolute)
        .map_err(|error| error.error)
        .with_context(|| format!("publish new artifact {}", absolute.display()))?;
    Ok(())
}

fn write_idempotent(path: &Path, bytes: &[u8]) -> Result<()> {
    let absolute = resolve_output_path(path)?;
    match fs::symlink_metadata(&absolute) {
        Ok(metadata) => {
            ensure!(
                !metadata.file_type().is_symlink() && metadata.is_file(),
                "recovery approval output exists but is not a regular file: {}",
                absolute.display()
            );
            ensure!(
                fs::read(&absolute)? == bytes,
                "recovery approval output exists with different content: {}",
                absolute.display()
            );
            return Ok(());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).with_context(|| format!("read {}", absolute.display())),
    }
    let parent = absolute.parent().context("approval output has no parent")?;
    let mut temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("create temporary approval in {}", parent.display()))?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    match temporary.persist_noclobber(&absolute) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            ensure!(
                read_bounded_regular_file(
                    &absolute,
                    MAX_ACCOUNT_ROOT_RECOVERY_APPROVAL_BYTES,
                    "raced recovery approval"
                )? == bytes,
                "another process published a different recovery approval at {}",
                absolute.display()
            );
            Ok(())
        }
        Err(error) => {
            Err(error.error).with_context(|| format!("publish approval {}", absolute.display()))
        }
    }
}

fn resolve_output_path(path: &Path) -> Result<PathBuf> {
    ensure!(
        !path.as_os_str().is_empty(),
        "artifact output path is empty"
    );
    let lexical = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let parent = lexical.parent().context("artifact output has no parent")?;
    ensure!(parent.is_dir(), "artifact output parent does not exist");
    let file_name = lexical
        .file_name()
        .context("artifact output has no file name")?;
    Ok(fs::canonicalize(parent)?.join(file_name))
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

#[cfg(test)]
mod tests {
    use std::error::Error;

    use iroh::{RelayMode, endpoint::Builder};
    use kilogram_identity::{
        AccountId, AccountRecoveryPhrase, AccountRootState, ConversationScopeId, DeviceCapability,
        DeviceState,
    };

    use super::*;

    fn local_test_endpoint_builder() -> Result<Builder> {
        Ok(endpoint_builder_with_relay(RoutePolicy::Auto, None)
            .relay_mode(RelayMode::Disabled)
            .clear_ip_transports()
            .bind_addr((std::net::Ipv4Addr::LOCALHOST, 0))?)
    }

    #[tokio::test]
    async fn signed_ticket_collects_exact_approval_over_direct_transport()
    -> Result<(), Box<dyn Error>> {
        let parent = tempfile::tempdir()?;
        let workspace = parent.path().join("account");
        let created = crate::create_account(&workspace)?;
        let package_file = parent.path().join("account.karp");
        let witness_file = parent.path().join("account.karw");
        crate::account_recovery::export_account_root(
            created.account_root_dir(),
            &package_file,
            &witness_file,
        )?;
        let request_file = parent.path().join("request.karq");
        create_request(&package_file, &witness_file, &request_file, 600)?;
        let ticket_file = parent.path().join("device.kart");
        let listener = local_test_endpoint_builder()?
            .alpns(vec![RECOVERY_APPROVAL_ALPN.to_vec()])
            .bind()
            .await?;
        let listener_task = tokio::spawn({
            let state_dir = created.state_dir().to_path_buf();
            let request_file = request_file.clone();
            let ticket_file = ticket_file.clone();
            async move {
                listen_on_endpoint(
                    &state_dir,
                    &request_file,
                    &ticket_file,
                    RoutePolicy::Auto,
                    listener,
                )
                .await
            }
        });
        timeout(Duration::from_secs(5), async {
            while !ticket_file.is_file() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await?;

        let approval_dir = parent.path().join("approvals");
        let collected = collect_approvals(
            &request_file,
            std::slice::from_ref(&ticket_file),
            &approval_dir,
            true,
            0,
        )
        .await?;
        let listened = listener_task.await??;
        assert_eq!(collected.observed_approvals, 1);
        assert!(collected.majority_satisfied);
        assert_eq!(
            collected.freshness_claim,
            "current-device-majority-observed"
        );
        assert_eq!(collected.transports[0].transport_path, "direct");
        assert_eq!(listened.transport_path, "direct");
        assert!(listened.approval_head_committed_before_ticket_publish);
        assert!(
            approval_dir
                .join(format!("{}.kara", created.device_id()))
                .is_file()
        );
        Ok(())
    }

    #[test]
    fn ticket_signature_binds_endpoint_request_and_route_policy() -> Result<(), Box<dyn Error>> {
        let parent = tempfile::tempdir()?;
        let workspace = parent.path().join("account");
        let created = crate::create_account(&workspace)?;
        let package_file = parent.path().join("account.karp");
        let witness_file = parent.path().join("account.karw");
        crate::account_recovery::export_account_root(
            created.account_root_dir(),
            &package_file,
            &witness_file,
        )?;
        let request_file = parent.path().join("request.karq");
        create_request(&package_file, &witness_file, &request_file, 600)?;
        let request = AccountRootRecoveryApprovalRequest::decode_and_verify(
            &fs::read(&request_file)?,
            unix_time_now()?,
        )?;
        let vault = EncryptedStateVault::open_existing(created.state_dir())?;
        let identity = vault.read_primary_device_identity()?;
        let device = DeviceIdentity::from_secret_bytes(*identity.signing_secret());
        let trust = vault.read_primary_trust()?;
        let certificate = decode_required_trust_record::<DeviceCertificate>(
            &trust,
            DEVICE_CERTIFICATE_PATH,
            DeviceCertificate::decode_and_verify,
        )?;
        let endpoint = EndpointAddr::new(iroh::SecretKey::generate().public());
        let ticket = RecoveryApprovalTicket::issue(
            &request,
            endpoint,
            &device,
            certificate,
            [7_u8; 32],
            RoutePolicy::Auto,
        )?;
        let mut tampered = ticket.clone();
        tampered.content.route_policy = RoutePolicy::RelayOnly;
        assert!(
            tampered
                .verify_for_request(&request, unix_time_now()?)
                .is_err()
        );
        let mut tampered = ticket;
        tampered.content.request_id = [9_u8; 32];
        assert!(
            tampered
                .verify_for_request(&request, unix_time_now()?)
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn db_primary_head_precedes_public_approval_and_retries_idempotently()
    -> Result<(), Box<dyn Error>> {
        let parent = tempfile::tempdir()?;
        let workspace = parent.path().join("account");
        let created = crate::create_account(&workspace)?;
        let package_file = parent.path().join("account.karp");
        let witness_file = parent.path().join("account.karw");
        crate::account_recovery::export_account_root(
            created.account_root_dir(),
            &package_file,
            &witness_file,
        )?;
        let request_file = parent.path().join("request.karq");
        let request = create_request(&package_file, &witness_file, &request_file, 600)?;
        assert_eq!(request.roster_count, 1);
        assert_eq!(request.required_approvals, 1);

        let blocked_output = parent.path().join("blocked-output");
        fs::create_dir(&blocked_output)?;
        assert!(approve_request(created.state_dir(), &request_file, &blocked_output).is_err());
        let vault = EncryptedStateVault::open_existing(created.state_dir())?;
        assert!(
            vault
                .read_primary_trust()?
                .records()
                .iter()
                .any(|record| record.relative_path() == APPROVAL_HEAD_PATH)
        );
        drop(vault);

        let approval_file = parent.path().join("device.kara");
        let approval = approve_request(created.state_dir(), &request_file, &approval_file)?;
        assert!(approval.reused_committed_approval);
        assert!(approval.approval_head_committed_before_publish);
        let verified = verify_request(&request_file, &[approval_file], true)?;
        assert_eq!(verified.freshness_claim, "current-device-majority-observed");
        assert!(verified.majority_satisfied);
        assert!(!verified.cross_roster_fork_safety);
        let vault = EncryptedStateVault::open_existing(created.state_dir())?;
        vault.verify()?;
        Ok(())
    }

    #[test]
    fn verifier_labels_offline_fallback_without_inventing_freshness() -> Result<(), Box<dyn Error>>
    {
        let parent = tempfile::tempdir()?;
        let workspace = parent.path().join("account");
        let created = crate::create_account(&workspace)?;
        let package_file = parent.path().join("account.karp");
        let witness_file = parent.path().join("account.karw");
        crate::account_recovery::export_account_root(
            created.account_root_dir(),
            &package_file,
            &witness_file,
        )?;
        let request_file = parent.path().join("request.karq");
        create_request(&package_file, &witness_file, &request_file, 600)?;
        let report = verify_request(&request_file, &[], false)?;
        assert_eq!(report.freshness_claim, "artifact-integrity-only");
        assert!(!report.majority_satisfied);
        assert!(verify_request(&request_file, &[], true).is_err());
        Ok(())
    }

    #[test]
    fn approval_refuses_stale_membership_and_same_revision_fork() -> Result<(), Box<dyn Error>> {
        let parent = tempfile::tempdir()?;
        let workspace = parent.path().join("account");
        let created = crate::create_account(&workspace)?;
        let phrase = AccountRecoveryPhrase::parse(created.recovery_phrase())?;

        let base_package = parent.path().join("base.karp");
        let base_witness = parent.path().join("base.karw");
        crate::account_recovery::export_account_root(
            created.account_root_dir(),
            &base_package,
            &base_witness,
        )?;
        let fork_root_path = parent.path().join("fork-root");
        let decoded_package =
            AccountRootRecoveryPackage::decode_and_verify(&fs::read(&base_package)?)?;
        let decoded_witness =
            AccountRootRecoveryWitness::decode_and_verify(&fs::read(&base_witness)?)?;
        AccountRootState::recover(&fork_root_path, &phrase, &decoded_package, &decoded_witness)?;

        let conversation = ConversationScopeId::from_bytes([41_u8; 32]);
        let root = AccountRootState::load(created.account_root_dir())?;
        root.create_conversation_membership(conversation, &[AccountId::from_bytes([51_u8; 32])])?;
        let current_package = parent.path().join("current.karp");
        let current_witness = parent.path().join("current.karw");
        crate::account_recovery::export_account_root(
            created.account_root_dir(),
            &current_package,
            &current_witness,
        )?;
        let current_request = parent.path().join("current.karq");
        create_request(&current_package, &current_witness, &current_request, 600)?;
        approve_request(
            created.state_dir(),
            &current_request,
            parent.path().join("current.kara"),
        )?;

        let stale_request = parent.path().join("stale.karq");
        create_request(&base_package, &base_witness, &stale_request, 600)?;
        assert!(
            approve_request(
                created.state_dir(),
                &stale_request,
                parent.path().join("stale.kara")
            )
            .is_err()
        );

        let fork_root = AccountRootState::load(&fork_root_path)?;
        fork_root
            .create_conversation_membership(conversation, &[AccountId::from_bytes([52_u8; 32])])?;
        let fork_package = parent.path().join("fork.karp");
        let fork_witness = parent.path().join("fork.karw");
        crate::account_recovery::export_account_root(
            &fork_root_path,
            &fork_package,
            &fork_witness,
        )?;
        let fork_request = parent.path().join("fork.karq");
        create_request(&fork_package, &fork_witness, &fork_request, 600)?;
        assert!(
            approve_request(
                created.state_dir(),
                &fork_request,
                parent.path().join("fork.kara")
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn approval_head_freezes_roster_until_joint_transition_exists() -> Result<(), Box<dyn Error>> {
        let parent = tempfile::tempdir()?;
        let workspace = parent.path().join("account");
        let created = crate::create_account(&workspace)?;
        let package_file = parent.path().join("one.karp");
        let witness_file = parent.path().join("one.karw");
        crate::account_recovery::export_account_root(
            created.account_root_dir(),
            &package_file,
            &witness_file,
        )?;
        let request_file = parent.path().join("one.karq");
        create_request(&package_file, &witness_file, &request_file, 600)?;
        approve_request(
            created.state_dir(),
            &request_file,
            parent.path().join("one.kara"),
        )?;

        let root = AccountRootState::load(created.account_root_dir())?;
        let second = DeviceState::load_or_create(parent.path().join("second-device"))?;
        root.enroll_device(
            second.identity().device_id(),
            second.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let expanded_package = parent.path().join("two.karp");
        let expanded_witness = parent.path().join("two.karw");
        crate::account_recovery::export_account_root(
            created.account_root_dir(),
            &expanded_package,
            &expanded_witness,
        )?;
        let expanded_request = parent.path().join("two.karq");
        create_request(&expanded_package, &expanded_witness, &expanded_request, 600)?;
        assert!(
            approve_request(
                created.state_dir(),
                &expanded_request,
                parent.path().join("two.kara")
            )
            .is_err()
        );
        Ok(())
    }
}
