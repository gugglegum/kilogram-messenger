#![forbid(unsafe_code)]

use std::{
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail, ensure};
use clap::{Parser, Subcommand, ValueEnum};
use ed25519_dalek::{Signer, SigningKey};
use iroh::{Endpoint, EndpointAddr, RelayUrl, SecretKey, endpoint::Connection};
use kilogram_cli::canonical_sync_v2_runtime::{
    CanonicalSyncV2ClientOutcome, CanonicalSyncV2ServeOutcome, drive_canonical_sync_v2_client,
    serve_canonical_sync_v2_connection,
};
use kilogram_identity::{
    AccountId, AccountRootState, ConversationMembershipSnapshot, DeviceCapability,
    DeviceEncryptionIdentity, DeviceId, DeviceIdentity,
};
use kilogram_protocol::{
    AuthorizedEvent, ConversationId, SignedEvent, SyncSessionBinding,
    canonical_authority_v1::{
        CanonicalAccountAuthoritySnapshotContentV1, CanonicalDeviceCapabilityV1,
        CanonicalDeviceCertificateContentV1, CanonicalDeviceSessionAuthorizationContentV1,
        CanonicalSignedAccountAuthoritySnapshotV1, CanonicalSignedDeviceCertificateV1,
        CanonicalSignedDeviceSessionAuthorizationV1, account_authority_snapshot_signing_bytes_v1,
        decode_and_verify_signed_device_session_authorization_v1,
        device_certificate_signing_bytes_v1, device_session_authorization_signing_bytes_v1,
        encode_signed_device_session_authorization_v1,
    },
    canonical_sync_history_v2::{CanonicalSyncEventCarrierV2, CanonicalSyncEventKeyV2},
    canonical_sync_v2::{
        MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH, decode_and_verify_sync_client_frame_v2,
    },
};
use kilogram_session::canonical_sync_v2_adapter::{
    CanonicalSyncV2Client, CanonicalSyncV2DormantClientDispatch,
    CanonicalSyncV2DormantDualReaderCoordinator, CanonicalSyncV2DormantResponderDispatch,
    CanonicalSyncV2DurableResponderAction, CanonicalSyncV2MembershipPolicy,
    CanonicalSyncV2MigrationState, CanonicalSyncV2RollbackTrigger, CanonicalSyncV2Server,
    capture_dormant_canonical_sync_v2_rollback_snapshot,
    evaluate_dormant_canonical_sync_v2_transport_policy, plan_dormant_canonical_sync_v2_rollback,
    verify_durable_mutual_canonical_candidate_selection,
};
use kilogram_store::{CanonicalSyncV2CarrierStore, EventStore};
use kilogram_sync_capability::{
    SignedSyncTransportCandidateAcceptance, SignedSyncTransportCandidateOffer,
    SignedSyncTransportCapability, SyncTransportCandidateOfferBinding,
    SyncTransportCapabilityBinding, SyncTransportProtocol,
};
use kilogram_transport_iroh::{
    ALPN, CANONICAL_SYNC_ALPN_V2, RoutePolicy, await_route_policy, endpoint_builder_for_remote,
    endpoint_builder_with_relay,
};
use serde::{Deserialize, Serialize};
use tokio::time::{sleep, timeout};

const ROLE_CONFIG_FILE: &str = "evidence-role.json";
const STORE_DIRECTORY: &str = "canonical-store";
const SOURCE_DIRECTORY: &str = "legacy-source";
const EVIDENCE_SCHEMA_VERSION: u32 = 1;
const TICKET_SCHEMA_VERSION: u32 = 1;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);
const ACCEPT_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const STREAM_TIMEOUT: Duration = Duration::from_secs(20);
const ROUTE_WAIT: Duration = Duration::from_secs(30);
const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
const CAPABILITY_VALIDITY_SECONDS: u64 = 24 * 60 * 60;
const LEGACY_PROBE_DOMAIN: &[u8] = b"kilogram:sync-v2-evidence-legacy-probe:v1\0";

#[derive(Parser)]
#[command(name = "kilogram-sync-v2-evidence")]
#[command(about = "Feature-gated evidence harness for dormant canonical sync v2")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Prepare {
        #[arg(long)]
        run_dir: PathBuf,
        #[arg(long)]
        source_revision: String,
        #[arg(long)]
        run_id: Option<String>,
    },
    Listen {
        #[arg(long)]
        role_dir: PathBuf,
        #[arg(long)]
        ticket_file: PathBuf,
        #[arg(long, value_enum, default_value_t = EvidenceRoute::Auto)]
        route_policy: EvidenceRoute,
        #[arg(long)]
        stop_after_diff: bool,
        #[arg(long)]
        hosted_loopback: bool,
        #[arg(long)]
        advertise_ip: Option<IpAddr>,
        #[arg(long)]
        relay_url: Option<RelayUrl>,
        #[arg(long)]
        report_file: Option<PathBuf>,
    },
    Dial {
        #[arg(long)]
        role_dir: PathBuf,
        #[arg(long)]
        ticket_file: PathBuf,
        #[arg(long, value_enum, default_value_t = EvidenceRoute::Auto)]
        route_policy: EvidenceRoute,
        #[arg(long)]
        stop_after_diff: bool,
        #[arg(long)]
        hosted_loopback: bool,
        #[arg(long)]
        report_file: Option<PathBuf>,
    },
    CorruptListen {
        #[arg(long)]
        role_dir: PathBuf,
        #[arg(long)]
        ticket_file: PathBuf,
        #[arg(long, value_enum, default_value_t = EvidenceRoute::Auto)]
        route_policy: EvidenceRoute,
        #[arg(long)]
        hosted_loopback: bool,
        #[arg(long)]
        advertise_ip: Option<IpAddr>,
        #[arg(long)]
        relay_url: Option<RelayUrl>,
        #[arg(long)]
        report_file: Option<PathBuf>,
    },
    CorruptDial {
        #[arg(long)]
        role_dir: PathBuf,
        #[arg(long)]
        ticket_file: PathBuf,
        #[arg(long, value_enum, default_value_t = EvidenceRoute::Auto)]
        route_policy: EvidenceRoute,
        #[arg(long)]
        hosted_loopback: bool,
        #[arg(long)]
        report_file: Option<PathBuf>,
    },
    LegacyListen {
        #[arg(long)]
        role_dir: PathBuf,
        #[arg(long)]
        ticket_file: PathBuf,
        #[arg(long, value_enum, default_value_t = EvidenceRoute::Auto)]
        route_policy: EvidenceRoute,
        #[arg(long)]
        hosted_loopback: bool,
        #[arg(long)]
        advertise_ip: Option<IpAddr>,
        #[arg(long)]
        relay_url: Option<RelayUrl>,
        #[arg(long)]
        report_file: Option<PathBuf>,
    },
    LegacyDial {
        #[arg(long)]
        role_dir: PathBuf,
        #[arg(long)]
        ticket_file: PathBuf,
        #[arg(long, value_enum, default_value_t = EvidenceRoute::Auto)]
        route_policy: EvidenceRoute,
        #[arg(long)]
        hosted_loopback: bool,
        #[arg(long)]
        report_file: Option<PathBuf>,
    },
    Inspect {
        #[arg(long)]
        role_dir: PathBuf,
        #[arg(long)]
        report_file: Option<PathBuf>,
    },
    RollbackDrill {
        #[arg(long)]
        role_dir: PathBuf,
        #[arg(long)]
        report_file: Option<PathBuf>,
    },
    HostedSelfTest {
        #[arg(long)]
        run_dir: PathBuf,
        #[arg(long)]
        source_revision: String,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum EvidenceRoute {
    Auto,
    DirectOnly,
    RelayOnly,
}

impl From<EvidenceRoute> for RoutePolicy {
    fn from(value: EvidenceRoute) -> Self {
        match value {
            EvidenceRoute::Auto => Self::Auto,
            EvidenceRoute::DirectOnly => Self::DirectOnly,
            EvidenceRoute::RelayOnly => Self::RelayOnly,
        }
    }
}

struct ListenerEndpointOptions {
    hosted_loopback: bool,
    advertise_ip: Option<IpAddr>,
    relay_url: Option<RelayUrl>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum EvidenceRole {
    Alice,
    Bob,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct EvidenceRoleConfig {
    schema_version: u32,
    source_revision: String,
    run_id: String,
    role: EvidenceRole,
    conversation_id: [u8; 32],
    session_binding: SyncSessionBinding,
    local_device_secret: [u8; 32],
    requester_device_id: DeviceId,
    listener_device_id: DeviceId,
    requester_account_id: AccountId,
    listener_account_id: AccountId,
    requester_authority_revision: u64,
    listener_authority_revision: u64,
    membership: Vec<u8>,
    listener_capability: SignedSyncTransportCapability,
    candidate_offer: SignedSyncTransportCandidateOffer,
    candidate_acceptance: SignedSyncTransportCandidateAcceptance,
    requester_authorization: Vec<u8>,
    listener_authorization: Vec<u8>,
    endpoint_secret: Option<[u8; 32]>,
    endpoint_id: String,
    ticket_publication_channel_epoch: u64,
    capability_issued_at_unix_seconds: u64,
    capability_expires_at_unix_seconds: u64,
    migration_epoch: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct EvidenceTicket {
    schema_version: u32,
    source_revision: String,
    run_id: String,
    protocol: String,
    endpoint: EndpointAddr,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct EvidenceReport {
    schema_version: u32,
    source_revision: String,
    run_id: String,
    role: EvidenceRole,
    scenario: String,
    status: String,
    protocol: String,
    route_policy: Option<String>,
    selected_path: Option<String>,
    remote_address: Option<String>,
    inventory_count: usize,
    inventory_digest: String,
    client_action: String,
    responder_action: String,
    canonical_selected: bool,
    legacy_dial_attempts: u64,
    fallback_attempts: u64,
    detail: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct LegacyProbe {
    run_id: String,
    device_id: DeviceId,
    signature: Vec<u8>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Prepare {
            run_dir,
            source_revision,
            run_id,
        } => prepare(&run_dir, &source_revision, run_id.as_deref()),
        Command::Listen {
            role_dir,
            ticket_file,
            route_policy,
            stop_after_diff,
            hosted_loopback,
            advertise_ip,
            relay_url,
            report_file,
        } => {
            run_listener(
                &role_dir,
                &ticket_file,
                route_policy.into(),
                stop_after_diff,
                ListenerEndpointOptions {
                    hosted_loopback,
                    advertise_ip,
                    relay_url,
                },
                report_file.as_deref(),
            )
            .await
        }
        Command::Dial {
            role_dir,
            ticket_file,
            route_policy,
            stop_after_diff,
            hosted_loopback,
            report_file,
        } => {
            run_dialer(
                &role_dir,
                &ticket_file,
                route_policy.into(),
                stop_after_diff,
                hosted_loopback,
                report_file.as_deref(),
            )
            .await
        }
        Command::CorruptListen {
            role_dir,
            ticket_file,
            route_policy,
            hosted_loopback,
            advertise_ip,
            relay_url,
            report_file,
        } => {
            run_corrupt_listener(
                &role_dir,
                &ticket_file,
                route_policy.into(),
                ListenerEndpointOptions {
                    hosted_loopback,
                    advertise_ip,
                    relay_url,
                },
                report_file.as_deref(),
            )
            .await
        }
        Command::CorruptDial {
            role_dir,
            ticket_file,
            route_policy,
            hosted_loopback,
            report_file,
        } => {
            run_corrupt_dialer(
                &role_dir,
                &ticket_file,
                route_policy.into(),
                hosted_loopback,
                report_file.as_deref(),
            )
            .await
        }
        Command::LegacyListen {
            role_dir,
            ticket_file,
            route_policy,
            hosted_loopback,
            advertise_ip,
            relay_url,
            report_file,
        } => {
            run_legacy_listener(
                &role_dir,
                &ticket_file,
                route_policy.into(),
                ListenerEndpointOptions {
                    hosted_loopback,
                    advertise_ip,
                    relay_url,
                },
                report_file.as_deref(),
            )
            .await
        }
        Command::LegacyDial {
            role_dir,
            ticket_file,
            route_policy,
            hosted_loopback,
            report_file,
        } => {
            run_legacy_dialer(
                &role_dir,
                &ticket_file,
                route_policy.into(),
                hosted_loopback,
                report_file.as_deref(),
            )
            .await
        }
        Command::Inspect {
            role_dir,
            report_file,
        } => inspect(&role_dir, report_file.as_deref()),
        Command::RollbackDrill {
            role_dir,
            report_file,
        } => rollback_drill(&role_dir, report_file.as_deref()),
        Command::HostedSelfTest {
            run_dir,
            source_revision,
        } => hosted_self_test(&run_dir, &source_revision).await,
    }
}

async fn hosted_self_test(run_dir: &Path, source_revision: &str) -> Result<()> {
    prepare(run_dir, source_revision, Some("m1180-hosted-self-test"))?;
    let ticket = run_dir.join("hosted-self-test.ticket.json");
    let alice_report = run_dir.join("hosted-self-test-alice.json");
    let bob_report = run_dir.join("hosted-self-test-bob.json");
    let bob_dir = run_dir.join("bob");
    let listener_ticket = ticket.clone();
    let listener_report = bob_report.clone();
    let listener = tokio::spawn(async move {
        run_listener(
            &bob_dir,
            &listener_ticket,
            RoutePolicy::DirectOnly,
            false,
            ListenerEndpointOptions {
                hosted_loopback: true,
                advertise_ip: None,
                relay_url: None,
            },
            Some(&listener_report),
        )
        .await
    });
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while !ticket.is_file() {
        ensure!(
            !listener.is_finished(),
            "hosted self-test listener exited before publishing its ticket"
        );
        ensure!(
            tokio::time::Instant::now() < deadline,
            "hosted self-test listener ticket deadline elapsed"
        );
        sleep(Duration::from_millis(50)).await;
    }
    let dial_result = run_dialer(
        &run_dir.join("alice"),
        &ticket,
        RoutePolicy::DirectOnly,
        false,
        true,
        Some(&alice_report),
    )
    .await;
    if dial_result.is_err() {
        listener.abort();
    }
    dial_result?;
    listener.await.context("join hosted self-test listener")??;
    let alice: EvidenceReport = serde_json::from_slice(&fs::read(&alice_report)?)?;
    let bob: EvidenceReport = serde_json::from_slice(&fs::read(&bob_report)?)?;
    ensure!(
        alice.inventory_count == 2
            && bob.inventory_count == 2
            && alice.inventory_digest == bob.inventory_digest,
        "hosted self-test histories did not converge"
    );
    println!("hosted_self_test_path=direct");
    println!("hosted_self_test_inventory_count=2");
    println!("status=hosted-self-test-verified");
    Ok(())
}

fn prepare(run_dir: &Path, source_revision: &str, requested_run_id: Option<&str>) -> Result<()> {
    validate_source_revision(source_revision)?;
    ensure!(!run_dir.exists(), "evidence run directory already exists");
    let run_id = requested_run_id
        .map(str::to_owned)
        .unwrap_or_else(default_run_id);
    validate_run_id(&run_id)?;
    fs::create_dir_all(run_dir).context("create evidence run directory")?;

    let setup = run_dir.join("setup-private");
    let alice_root = AccountRootState::create(setup.join("alice-root"))?;
    let bob_root = AccountRootState::create(setup.join("bob-root"))?;
    let owner_root = AccountRootState::create(setup.join("owner-root"))?;
    let alice = DeviceIdentity::generate()?;
    let bob = DeviceIdentity::generate()?;
    let alice_encryption = DeviceEncryptionIdentity::generate()?;
    let bob_encryption = DeviceEncryptionIdentity::generate()?;
    let alice_certificate = alice_root.issue_device_certificate(
        alice.device_id(),
        alice_encryption.public_key(),
        &DeviceCapability::MESSAGING,
    )?;
    let bob_certificate = bob_root.issue_device_certificate(
        bob.device_id(),
        bob_encryption.public_key(),
        &DeviceCapability::MESSAGING,
    )?;
    let alice_authority = alice_root.authority_snapshot()?;
    let bob_authority = bob_root.authority_snapshot()?;

    let conversation = ConversationId::from_label(&format!("sync-v2-evidence-{run_id}"));
    let membership = owner_root.create_conversation_membership(
        conversation.scope_id(),
        &[alice_root.account_id(), bob_root.account_id()],
    )?;
    let reference_event_id = fixture_reference_event_id()?;
    let alice_event = AuthorizedEvent::new(
        SignedEvent::sign_acknowledgement(&alice, conversation, 0, Vec::new(), reference_event_id)?,
        alice_certificate,
        alice_authority,
    )?;
    let bob_event = AuthorizedEvent::new(
        SignedEvent::sign_acknowledgement(&bob, conversation, 0, Vec::new(), reference_event_id)?,
        bob_certificate,
        bob_authority,
    )?;

    let session_binding = SyncSessionBinding::from_transport_label(&format!(
        "kilogram-sync-v2-field-evidence:{run_id}"
    ));
    let mut endpoint_secret_bytes = [0_u8; 32];
    getrandom::fill(&mut endpoint_secret_bytes).context("generate Iroh endpoint secret")?;
    let endpoint_secret = SecretKey::from_bytes(&endpoint_secret_bytes);
    let endpoint_id = endpoint_secret.public().to_string();

    let mut requester_root_secret = [0_u8; 32];
    let mut listener_root_secret = [0_u8; 32];
    getrandom::fill(&mut requester_root_secret).context("generate requester account secret")?;
    getrandom::fill(&mut listener_root_secret).context("generate listener account secret")?;
    let requester_authority_revision = 7;
    let listener_authority_revision = 9;
    let (requester_account_id, requester_authorization) = canonical_session_authorization(
        requester_root_secret,
        &alice,
        *session_binding.as_bytes(),
        requester_authority_revision,
    )?;
    let (listener_account_id, listener_authorization) = canonical_session_authorization(
        listener_root_secret,
        &bob,
        *session_binding.as_bytes(),
        listener_authority_revision,
    )?;
    let issued_at = unix_time_now()?;
    let expires_at = issued_at
        .checked_add(CAPABILITY_VALIDITY_SECONDS)
        .context("capability expiry overflow")?;
    let publication_epoch = 1;
    let capability_binding = SyncTransportCapabilityBinding {
        listener_account_id,
        listener_device_id: bob.device_id(),
        authority_revision: listener_authority_revision,
        allowed_requester_account_id: requester_account_id,
        endpoint_id: &endpoint_id,
        ticket_publication_channel_epoch: publication_epoch,
        issued_at_unix_seconds: issued_at,
        expires_at_unix_seconds: expires_at,
    };
    let capability = SignedSyncTransportCapability::issue_legacy_with_canonical_candidate(
        &bob,
        &capability_binding,
    )?;
    let migration_epoch = 1;
    let offer = SignedSyncTransportCandidateOffer::issue(
        &alice,
        SyncTransportCandidateOfferBinding {
            listener_capability_id: capability.artifact_id()?,
            conversation_id: *conversation.as_bytes(),
            listener_account_id,
            listener_device_id: bob.device_id(),
            requester_account_id,
            requester_device_id: alice.device_id(),
            session_binding: *session_binding.as_bytes(),
            listener_authority_revision,
            requester_authority_revision,
            migration_epoch,
        },
    )?;
    let acceptance = SignedSyncTransportCandidateAcceptance::issue(&bob, &offer)?;
    let requester_authorization =
        encode_signed_device_session_authorization_v1(&requester_authorization)?;
    let listener_authorization =
        encode_signed_device_session_authorization_v1(&listener_authorization)?;

    prepare_role_store(
        &run_dir.join("alice"),
        &alice_event,
        &membership,
        &alice,
        &bob,
        session_binding,
    )?;
    prepare_role_store(
        &run_dir.join("bob"),
        &bob_event,
        &membership,
        &alice,
        &bob,
        session_binding,
    )?;

    let membership_bytes = membership.encode()?;
    let common = |role, local_device_secret, endpoint_secret| EvidenceRoleConfig {
        schema_version: EVIDENCE_SCHEMA_VERSION,
        source_revision: source_revision.to_owned(),
        run_id: run_id.clone(),
        role,
        conversation_id: *conversation.as_bytes(),
        session_binding,
        local_device_secret,
        requester_device_id: alice.device_id(),
        listener_device_id: bob.device_id(),
        requester_account_id,
        listener_account_id,
        requester_authority_revision,
        listener_authority_revision,
        membership: membership_bytes.clone(),
        listener_capability: capability.clone(),
        candidate_offer: offer.clone(),
        candidate_acceptance: acceptance.clone(),
        requester_authorization: requester_authorization.clone(),
        listener_authorization: listener_authorization.clone(),
        endpoint_secret,
        endpoint_id: endpoint_id.clone(),
        ticket_publication_channel_epoch: publication_epoch,
        capability_issued_at_unix_seconds: issued_at,
        capability_expires_at_unix_seconds: expires_at,
        migration_epoch,
    };
    let alice_config = common(EvidenceRole::Alice, alice.secret_bytes(), None);
    let bob_config = common(
        EvidenceRole::Bob,
        bob.secret_bytes(),
        Some(endpoint_secret_bytes),
    );
    write_json(&run_dir.join("alice").join(ROLE_CONFIG_FILE), &alice_config)?;
    write_json(&run_dir.join("bob").join(ROLE_CONFIG_FILE), &bob_config)?;

    verify_prepared_role(&run_dir.join("alice"))?;
    verify_prepared_role(&run_dir.join("bob"))?;
    println!("evidence_run_id={run_id}");
    println!("source_revision={source_revision}");
    println!(
        "candidate_alpn={}",
        String::from_utf8_lossy(CANONICAL_SYNC_ALPN_V2)
    );
    println!("production_default_changed=false");
    println!("status=prepared");
    Ok(())
}

fn prepare_role_store(
    role_dir: &Path,
    local_event: &AuthorizedEvent,
    membership: &ConversationMembershipSnapshot,
    requester: &DeviceIdentity,
    listener: &DeviceIdentity,
    session_binding: SyncSessionBinding,
) -> Result<()> {
    fs::create_dir_all(role_dir).context("create evidence role directory")?;
    let source = EventStore::open(role_dir.join(SOURCE_DIRECTORY))?;
    source.put_authorized(local_event, membership)?;
    let plan = source
        .canonical_sync_v2_legacy_import_plan(local_event.event().conversation_id(), membership)?;
    let store = CanonicalSyncV2CarrierStore::open(role_dir.join(STORE_DIRECTORY))?;
    let progress = store.import_legacy_page(
        &source,
        membership,
        &plan,
        plan.start_cursor(),
        MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH,
    )?;
    ensure!(progress.is_complete(), "legacy import did not complete");
    let policy = CanonicalSyncV2MembershipPolicy::from_legacy(membership.clone())?;
    CanonicalSyncV2Client::bootstrap_from_completed_legacy_import(
        requester,
        &store,
        &policy,
        &plan,
        session_binding,
        listener.device_id(),
    )?;
    CanonicalSyncV2Server::bootstrap_responder_from_completed_legacy_import(
        listener,
        &store,
        &policy,
        &plan,
        session_binding,
        requester.device_id(),
    )?;
    Ok(())
}

fn verify_prepared_role(role_dir: &Path) -> Result<()> {
    let config = load_config(role_dir)?;
    let store = CanonicalSyncV2CarrierStore::open(role_dir.join(STORE_DIRECTORY))?;
    let selected = verify_selection(&config, &store)?;
    let plan = evaluate_dormant_canonical_sync_v2_transport_policy(
        CanonicalSyncV2MigrationState::DualReaderPreferCanonical,
        Some(&selected),
        SyncTransportProtocol::CanonicalCborV2,
        config.migration_epoch,
        false,
    )?;
    ensure!(
        !plan.active_transport_authorized(),
        "dormant plan activated production transport"
    );
    let coordinator = CanonicalSyncV2DormantDualReaderCoordinator::new(plan);
    let client_runtime = load_client_runtime(&config, &store)?;
    let responder_runtime = load_responder_runtime(&config, &store)?;
    ensure!(
        matches!(
            coordinator.plan_client_dispatch(Some(&client_runtime))?,
            CanonicalSyncV2DormantClientDispatch::CanonicalCborV2(_)
        ),
        "client evidence dispatch is not canonical v2"
    );
    ensure!(
        matches!(
            coordinator.plan_responder_dispatch(Some(&responder_runtime))?,
            CanonicalSyncV2DormantResponderDispatch::CanonicalCborV2(_)
        ),
        "responder evidence dispatch is not canonical v2"
    );
    Ok(())
}

async fn run_listener(
    role_dir: &Path,
    ticket_file: &Path,
    route_policy: RoutePolicy,
    stop_after_diff: bool,
    endpoint_options: ListenerEndpointOptions,
    report_file: Option<&Path>,
) -> Result<()> {
    let config = require_role(role_dir, EvidenceRole::Bob)?;
    verify_prepared_role(role_dir)?;
    let store = CanonicalSyncV2CarrierStore::open(role_dir.join(STORE_DIRECTORY))?;
    let identity = DeviceIdentity::from_secret_bytes(config.local_device_secret);
    ensure!(
        identity.device_id() == config.listener_device_id,
        "listener identity mismatch"
    );
    let membership = membership_policy(&config)?;
    let server = CanonicalSyncV2Server::new(
        &identity,
        &store,
        &membership,
        config.conversation_id,
        config.session_binding,
        config.requester_device_id,
    );
    let endpoint = bind_listener_endpoint(
        &config,
        route_policy,
        CANONICAL_SYNC_ALPN_V2,
        &endpoint_options,
    )
    .await?;
    write_ticket(
        ticket_file,
        &config,
        &endpoint,
        CANONICAL_SYNC_ALPN_V2,
        endpoint_options.advertise_ip,
    )?;
    println!("status=listening");
    println!("transport_endpoint_id={}", endpoint.id());
    let mut selected_path = None;
    loop {
        if matches!(
            server.durable_responder_action(&server.restore_durable_responder_runtime()?)?,
            CanonicalSyncV2DurableResponderAction::Complete
        ) {
            break;
        }
        let connection = accept_connection(&endpoint).await?;
        ensure!(
            connection.alpn() == CANONICAL_SYNC_ALPN_V2,
            "listener accepted unexpected ALPN"
        );
        let path = await_route_policy(&connection, route_policy, ROUTE_WAIT).await?;
        selected_path = Some((path.kind.as_str().to_owned(), path.remote_address.clone()));
        let outcome =
            serve_canonical_sync_v2_connection(&connection, &server, stop_after_diff).await;
        connection.close(0_u32.into(), b"kilogram sync v2 evidence session complete");
        match outcome? {
            CanonicalSyncV2ServeOutcome::Complete => break,
            CanonicalSyncV2ServeOutcome::StoppedAfterDiff => {
                let report = build_report(
                    &config,
                    &store,
                    "restart-checkpoint-listener",
                    "checkpoint-stop",
                    CANONICAL_SYNC_ALPN_V2,
                    Some(route_policy),
                    selected_path.as_ref(),
                    true,
                    "durable diff persisted and sent; process stopped before event batch",
                )?;
                write_optional_report(report_file, &report)?;
                endpoint.close().await;
                println!("status=stopped-after-diff");
                return Ok(());
            }
            CanonicalSyncV2ServeOutcome::ConnectionInterrupted => continue,
        }
    }
    let report = build_report(
        &config,
        &store,
        "canonical-v2-listener",
        "synchronized",
        CANONICAL_SYNC_ALPN_V2,
        Some(route_policy),
        selected_path.as_ref(),
        true,
        "durable responder reached Complete without legacy fallback",
    )?;
    write_optional_report(report_file, &report)?;
    endpoint.close().await;
    println!("status=synchronized");
    Ok(())
}

async fn run_dialer(
    role_dir: &Path,
    ticket_file: &Path,
    route_policy: RoutePolicy,
    stop_after_diff: bool,
    hosted_loopback: bool,
    report_file: Option<&Path>,
) -> Result<()> {
    let config = require_role(role_dir, EvidenceRole::Alice)?;
    verify_prepared_role(role_dir)?;
    let store = CanonicalSyncV2CarrierStore::open(role_dir.join(STORE_DIRECTORY))?;
    let identity = DeviceIdentity::from_secret_bytes(config.local_device_secret);
    ensure!(
        identity.device_id() == config.requester_device_id,
        "requester identity mismatch"
    );
    let membership = membership_policy(&config)?;
    let client = CanonicalSyncV2Client::new(
        &identity,
        &store,
        &membership,
        config.conversation_id,
        config.session_binding,
        config.listener_device_id,
    );
    let ticket = read_ticket(ticket_file, &config, CANONICAL_SYNC_ALPN_V2)?;
    let endpoint = evidence_dialer_builder(route_policy, &ticket.endpoint, hosted_loopback)?
        .bind()
        .await
        .context("bind canonical evidence dialer endpoint")?;
    wait_for_endpoint_online(&endpoint, route_policy).await?;
    let connection = timeout(
        CONNECTION_TIMEOUT,
        endpoint.connect(ticket.endpoint.clone(), CANONICAL_SYNC_ALPN_V2),
    )
    .await
    .context("connect canonical evidence deadline")?
    .context("connect canonical evidence endpoint")?;
    let path = await_route_policy(&connection, route_policy, ROUTE_WAIT).await?;
    println!("transport_ready_path={}", path.kind.as_str());
    let outcome = drive_canonical_sync_v2_client(&connection, &client, stop_after_diff).await?;
    let (scenario, status, detail) = match outcome {
        CanonicalSyncV2ClientOutcome::Complete => (
            "canonical-v2-dialer",
            "synchronized",
            "durable client reached Complete without legacy fallback",
        ),
        CanonicalSyncV2ClientOutcome::StoppedAfterDiff => (
            "restart-checkpoint-dialer",
            "checkpoint-stop",
            "durable event batch persisted; process stopped before batch send",
        ),
    };
    let selected = (path.kind.as_str().to_owned(), path.remote_address.clone());
    let report = build_report(
        &config,
        &store,
        scenario,
        status,
        CANONICAL_SYNC_ALPN_V2,
        Some(route_policy),
        Some(&selected),
        true,
        detail,
    )?;
    write_optional_report(report_file, &report)?;
    connection.close(0_u32.into(), b"kilogram sync v2 evidence client complete");
    endpoint.close().await;
    println!("status={status}");
    Ok(())
}

async fn run_corrupt_listener(
    role_dir: &Path,
    ticket_file: &Path,
    route_policy: RoutePolicy,
    endpoint_options: ListenerEndpointOptions,
    report_file: Option<&Path>,
) -> Result<()> {
    let config = require_role(role_dir, EvidenceRole::Bob)?;
    verify_prepared_role(role_dir)?;
    let store = CanonicalSyncV2CarrierStore::open(role_dir.join(STORE_DIRECTORY))?;
    let before = inventory_digest(&store, config.conversation_id)?;
    let endpoint = bind_listener_endpoint(
        &config,
        route_policy,
        CANONICAL_SYNC_ALPN_V2,
        &endpoint_options,
    )
    .await?;
    write_ticket(
        ticket_file,
        &config,
        &endpoint,
        CANONICAL_SYNC_ALPN_V2,
        endpoint_options.advertise_ip,
    )?;
    println!("status=listening-for-corrupt-selected-frame");
    let connection = accept_connection(&endpoint).await?;
    ensure!(
        connection.alpn() == CANONICAL_SYNC_ALPN_V2,
        "corrupt listener ALPN mismatch"
    );
    let path = await_route_policy(&connection, route_policy, ROUTE_WAIT).await?;
    let (_, mut receive) = timeout(STREAM_TIMEOUT, connection.accept_bi())
        .await
        .context("accept corrupt evidence stream deadline")?
        .context("accept corrupt evidence stream")?;
    let bytes = timeout(STREAM_TIMEOUT, receive.read_to_end(MAX_FRAME_BYTES))
        .await
        .context("read corrupt evidence frame deadline")?
        .context("read corrupt evidence frame")?;
    ensure!(
        decode_and_verify_sync_client_frame_v2(&bytes).is_err(),
        "corrupt selected frame unexpectedly decoded"
    );
    ensure!(
        inventory_digest(&store, config.conversation_id)? == before,
        "corrupt selected frame changed durable history"
    );
    let selected = (path.kind.as_str().to_owned(), path.remote_address.clone());
    let report = build_report(
        &config,
        &store,
        "post-selection-corrupt-listener",
        "failed-closed",
        CANONICAL_SYNC_ALPN_V2,
        Some(route_policy),
        Some(&selected),
        true,
        "selected canonical decode failed; endpoint registered no legacy ALPN",
    )?;
    write_optional_report(report_file, &report)?;
    connection.close(7_u32.into(), b"canonical evidence decode failed closed");
    endpoint.close().await;
    println!("fallback_attempts=0");
    println!("status=failed-closed-as-expected");
    Ok(())
}

async fn run_corrupt_dialer(
    role_dir: &Path,
    ticket_file: &Path,
    route_policy: RoutePolicy,
    hosted_loopback: bool,
    report_file: Option<&Path>,
) -> Result<()> {
    let config = require_role(role_dir, EvidenceRole::Alice)?;
    verify_prepared_role(role_dir)?;
    let store = CanonicalSyncV2CarrierStore::open(role_dir.join(STORE_DIRECTORY))?;
    let ticket = read_ticket(ticket_file, &config, CANONICAL_SYNC_ALPN_V2)?;
    let endpoint = evidence_dialer_builder(route_policy, &ticket.endpoint, hosted_loopback)?
        .bind()
        .await?;
    wait_for_endpoint_online(&endpoint, route_policy).await?;
    let connection = timeout(
        CONNECTION_TIMEOUT,
        endpoint.connect(ticket.endpoint.clone(), CANONICAL_SYNC_ALPN_V2),
    )
    .await
    .context("connect corrupt evidence deadline")??;
    let path = await_route_policy(&connection, route_policy, ROUTE_WAIT).await?;
    let (mut send, _) = open_bi(&connection).await?;
    timeout(STREAM_TIMEOUT, send.write_all(&[0xff, 0xff, 0x00]))
        .await
        .context("send corrupt evidence deadline")??;
    send.finish().context("finish corrupt evidence frame")?;
    let selected = (path.kind.as_str().to_owned(), path.remote_address.clone());
    let report = build_report(
        &config,
        &store,
        "post-selection-corrupt-dialer",
        "corrupt-frame-sent",
        CANONICAL_SYNC_ALPN_V2,
        Some(route_policy),
        Some(&selected),
        true,
        "one canonical dial; zero legacy fallback attempts",
    )?;
    write_optional_report(report_file, &report)?;
    connection.close(7_u32.into(), b"canonical evidence corrupt probe complete");
    endpoint.close().await;
    println!("legacy_dial_attempts=0");
    println!("status=corrupt-frame-sent");
    Ok(())
}

async fn run_legacy_listener(
    role_dir: &Path,
    ticket_file: &Path,
    route_policy: RoutePolicy,
    endpoint_options: ListenerEndpointOptions,
    report_file: Option<&Path>,
) -> Result<()> {
    let config = require_role(role_dir, EvidenceRole::Bob)?;
    let store = CanonicalSyncV2CarrierStore::open(role_dir.join(STORE_DIRECTORY))?;
    verify_legacy_plan()?;
    let endpoint = bind_listener_endpoint(&config, route_policy, ALPN, &endpoint_options).await?;
    write_ticket(
        ticket_file,
        &config,
        &endpoint,
        ALPN,
        endpoint_options.advertise_ip,
    )?;
    println!("status=listening-for-legacy-control");
    let connection = accept_connection(&endpoint).await?;
    ensure!(connection.alpn() == ALPN, "legacy control ALPN mismatch");
    let path = await_route_policy(&connection, route_policy, ROUTE_WAIT).await?;
    let (mut send, mut receive) = timeout(STREAM_TIMEOUT, connection.accept_bi())
        .await
        .context("accept legacy probe deadline")??;
    let bytes = timeout(STREAM_TIMEOUT, receive.read_to_end(16 * 1024))
        .await
        .context("read legacy probe deadline")??;
    let request: LegacyProbe = serde_json::from_slice(&bytes)?;
    verify_legacy_probe(&request, &config.run_id, config.requester_device_id)?;
    let identity = DeviceIdentity::from_secret_bytes(config.local_device_secret);
    let response = sign_legacy_probe(&identity, &config.run_id)?;
    timeout(
        STREAM_TIMEOUT,
        send.write_all(&serde_json::to_vec(&response)?),
    )
    .await
    .context("send legacy probe response deadline")??;
    send.finish()?;
    let selected = (path.kind.as_str().to_owned(), path.remote_address.clone());
    let report = build_report(
        &config,
        &store,
        "mixed-version-legacy-listener",
        "legacy-agreed",
        ALPN,
        Some(route_policy),
        Some(&selected),
        false,
        "legacy selected before canonical mutual selection",
    )?;
    write_optional_report(report_file, &report)?;
    connection.close(0_u32.into(), b"legacy evidence control complete");
    endpoint.close().await;
    println!("canonical_selected=false");
    println!("status=legacy-agreed");
    Ok(())
}

async fn run_legacy_dialer(
    role_dir: &Path,
    ticket_file: &Path,
    route_policy: RoutePolicy,
    hosted_loopback: bool,
    report_file: Option<&Path>,
) -> Result<()> {
    let config = require_role(role_dir, EvidenceRole::Alice)?;
    let store = CanonicalSyncV2CarrierStore::open(role_dir.join(STORE_DIRECTORY))?;
    verify_legacy_plan()?;
    let ticket = read_ticket(ticket_file, &config, ALPN)?;
    let endpoint = evidence_dialer_builder(route_policy, &ticket.endpoint, hosted_loopback)?
        .bind()
        .await?;
    wait_for_endpoint_online(&endpoint, route_policy).await?;
    let connection = timeout(
        CONNECTION_TIMEOUT,
        endpoint.connect(ticket.endpoint.clone(), ALPN),
    )
    .await
    .context("connect legacy control deadline")??;
    let path = await_route_policy(&connection, route_policy, ROUTE_WAIT).await?;
    let identity = DeviceIdentity::from_secret_bytes(config.local_device_secret);
    let request = sign_legacy_probe(&identity, &config.run_id)?;
    let (mut send, mut receive) = open_bi(&connection).await?;
    timeout(
        STREAM_TIMEOUT,
        send.write_all(&serde_json::to_vec(&request)?),
    )
    .await
    .context("send legacy control request deadline")??;
    send.finish()?;
    let bytes = timeout(STREAM_TIMEOUT, receive.read_to_end(16 * 1024))
        .await
        .context("read legacy control response deadline")??;
    let response: LegacyProbe = serde_json::from_slice(&bytes)?;
    verify_legacy_probe(&response, &config.run_id, config.listener_device_id)?;
    let selected = (path.kind.as_str().to_owned(), path.remote_address.clone());
    let report = build_report(
        &config,
        &store,
        "mixed-version-legacy-dialer",
        "legacy-agreed",
        ALPN,
        Some(route_policy),
        Some(&selected),
        false,
        "legacy selected before canonical mutual selection",
    )?;
    write_optional_report(report_file, &report)?;
    connection.close(0_u32.into(), b"legacy evidence control complete");
    endpoint.close().await;
    println!("canonical_selected=false");
    println!("status=legacy-agreed");
    Ok(())
}

fn inspect(role_dir: &Path, report_file: Option<&Path>) -> Result<()> {
    let config = load_config(role_dir)?;
    let store = CanonicalSyncV2CarrierStore::open(role_dir.join(STORE_DIRECTORY))?;
    let report = build_report(
        &config,
        &store,
        "inspect",
        "inspected",
        CANONICAL_SYNC_ALPN_V2,
        None,
        None,
        false,
        "local durable state inspection",
    )?;
    write_optional_report(report_file, &report)?;
    println!("inventory_count={}", report.inventory_count);
    println!("inventory_digest={}", report.inventory_digest);
    println!("client_action={}", report.client_action);
    println!("responder_action={}", report.responder_action);
    println!("status=inspected");
    Ok(())
}

fn rollback_drill(role_dir: &Path, report_file: Option<&Path>) -> Result<()> {
    let config = load_config(role_dir)?;
    let store = CanonicalSyncV2CarrierStore::open(role_dir.join(STORE_DIRECTORY))?;
    let before = inventory_digest(&store, config.conversation_id)?;
    let snapshot = capture_dormant_canonical_sync_v2_rollback_snapshot(
        CanonicalSyncV2MigrationState::DualReaderPreferCanonical,
        config.migration_epoch,
        None,
        None,
        None,
    )?;
    let record = plan_dormant_canonical_sync_v2_rollback(
        snapshot,
        CanonicalSyncV2MigrationState::DualReaderPreferLegacy,
        config.migration_epoch + 1,
        CanonicalSyncV2RollbackTrigger::ExplicitLocal,
        "M1.1.80 evidence-only explicit rollback drill",
    )?;
    let encoded = record.encode()?;
    let restored =
        kilogram_session::canonical_sync_v2_adapter::CanonicalSyncV2RollbackDrillRecord::decode(
            &encoded,
        )?;
    let replacement = restored.apply(snapshot)?;
    ensure!(
        replacement.migration_state() == CanonicalSyncV2MigrationState::DualReaderPreferLegacy,
        "rollback did not move exactly one state"
    );
    ensure!(
        !restored
            .dormant_legacy_transport_plan()?
            .active_transport_authorized(),
        "rollback unexpectedly activated production transport"
    );
    let selected = verify_selection(&config, &store)?;
    let blocked_snapshot = capture_dormant_canonical_sync_v2_rollback_snapshot(
        CanonicalSyncV2MigrationState::DualReaderPreferCanonical,
        config.migration_epoch,
        Some(&selected),
        None,
        None,
    )?;
    ensure!(
        plan_dormant_canonical_sync_v2_rollback(
            blocked_snapshot,
            CanonicalSyncV2MigrationState::DualReaderPreferLegacy,
            config.migration_epoch + 1,
            CanonicalSyncV2RollbackTrigger::ExplicitLocal,
            "M1.1.80 selected-session rollback must be blocked",
        )
        .is_err(),
        "rollback with selected canonical evidence did not fail closed"
    );
    ensure!(
        inventory_digest(&store, config.conversation_id)? == before,
        "rollback drill changed user history"
    );
    let report = build_report(
        &config,
        &store,
        "explicit-one-step-rollback",
        "rollback-verified",
        ALPN,
        None,
        None,
        false,
        "one-step explicit rollback preserved history; selected-evidence rollback was blocked",
    )?;
    write_optional_report(report_file, &report)?;
    println!("rollback_record_bytes={}", encoded.len());
    println!("history_unchanged=true");
    println!("selected_evidence_rollback_blocked=true");
    println!("status=rollback-verified");
    Ok(())
}

fn verify_selection(
    config: &EvidenceRoleConfig,
    store: &CanonicalSyncV2CarrierStore,
) -> Result<kilogram_session::canonical_sync_v2_adapter::CanonicalSyncV2MutualCandidateSelection> {
    let listener_authorization =
        decode_and_verify_signed_device_session_authorization_v1(&config.listener_authorization)?;
    let requester_authorization =
        decode_and_verify_signed_device_session_authorization_v1(&config.requester_authorization)?;
    let binding = capability_binding(config);
    let client_runtime = load_client_runtime(config, store)?;
    let responder_runtime = load_responder_runtime(config, store)?;
    verify_durable_mutual_canonical_candidate_selection(
        &config.listener_capability,
        &binding,
        &config.candidate_offer,
        &config.candidate_acceptance,
        &listener_authorization,
        &requester_authorization,
        &client_runtime,
        &responder_runtime,
        unix_time_now()?,
    )
    .map_err(Into::into)
}

fn load_client_runtime(
    config: &EvidenceRoleConfig,
    store: &CanonicalSyncV2CarrierStore,
) -> Result<kilogram_store::CanonicalSyncV2StoredSessionRuntime> {
    store
        .session_runtime_state(
            config.conversation_id,
            *config.requester_device_id.as_bytes(),
            *config.listener_device_id.as_bytes(),
            *config.session_binding.as_bytes(),
        )?
        .context("canonical evidence client runtime is missing")
}

fn load_responder_runtime(
    config: &EvidenceRoleConfig,
    store: &CanonicalSyncV2CarrierStore,
) -> Result<kilogram_store::CanonicalSyncV2StoredResponderRuntime> {
    store
        .responder_runtime_state(
            config.conversation_id,
            *config.listener_device_id.as_bytes(),
            *config.requester_device_id.as_bytes(),
            *config.session_binding.as_bytes(),
        )?
        .context("canonical evidence responder runtime is missing")
}

fn capability_binding(config: &EvidenceRoleConfig) -> SyncTransportCapabilityBinding<'_> {
    SyncTransportCapabilityBinding {
        listener_account_id: config.listener_account_id,
        listener_device_id: config.listener_device_id,
        authority_revision: config.listener_authority_revision,
        allowed_requester_account_id: config.requester_account_id,
        endpoint_id: &config.endpoint_id,
        ticket_publication_channel_epoch: config.ticket_publication_channel_epoch,
        issued_at_unix_seconds: config.capability_issued_at_unix_seconds,
        expires_at_unix_seconds: config.capability_expires_at_unix_seconds,
    }
}

fn membership_policy(config: &EvidenceRoleConfig) -> Result<CanonicalSyncV2MembershipPolicy> {
    CanonicalSyncV2MembershipPolicy::from_legacy(ConversationMembershipSnapshot::decode_and_verify(
        &config.membership,
    )?)
    .map_err(Into::into)
}

fn verify_legacy_plan() -> Result<()> {
    let plan = evaluate_dormant_canonical_sync_v2_transport_policy(
        CanonicalSyncV2MigrationState::DualReaderPreferLegacy,
        None,
        SyncTransportProtocol::LegacyPostcardV6,
        1,
        false,
    )?;
    ensure!(
        !plan.mutual_candidate_selected(),
        "legacy control unexpectedly selected candidate"
    );
    ensure!(
        !plan.active_transport_authorized(),
        "legacy control activated production transport"
    );
    Ok(())
}

async fn bind_listener_endpoint(
    config: &EvidenceRoleConfig,
    route_policy: RoutePolicy,
    alpn: &[u8],
    endpoint_options: &ListenerEndpointOptions,
) -> Result<Endpoint> {
    ensure!(
        !endpoint_options.hosted_loopback || route_policy == RoutePolicy::DirectOnly,
        "hosted loopback evidence requires direct-only routing"
    );
    let secret = config
        .endpoint_secret
        .context("listener endpoint secret is missing")?;
    ensure!(
        endpoint_options.relay_url.is_none() || !endpoint_options.hosted_loopback,
        "hosted loopback evidence cannot configure a relay"
    );
    let builder = endpoint_builder_with_relay(route_policy, endpoint_options.relay_url.clone());
    let builder = if endpoint_options.hosted_loopback {
        builder
            .relay_mode(iroh::RelayMode::Disabled)
            .clear_ip_transports()
            .clear_address_lookup()
            .portmapper_config(iroh::endpoint::PortmapperConfig::Disabled)
            .net_report_config(iroh::endpoint::NetReportConfig::minimal())
            .bind_addr((Ipv4Addr::LOCALHOST, 0))
            .context("bind hosted evidence listener exclusively to IPv4 loopback")?
    } else {
        builder
    };
    let endpoint = builder
        .secret_key(SecretKey::from_bytes(&secret))
        .alpns(vec![alpn.to_vec()])
        .bind()
        .await
        .context("bind evidence listener endpoint")?;
    ensure!(
        endpoint.id().to_string() == config.endpoint_id,
        "endpoint identity mismatch"
    );
    wait_for_endpoint_online(&endpoint, route_policy).await?;
    Ok(endpoint)
}

fn evidence_dialer_builder(
    route_policy: RoutePolicy,
    remote: &EndpointAddr,
    hosted_loopback: bool,
) -> Result<iroh::endpoint::Builder> {
    ensure!(
        !hosted_loopback || route_policy == RoutePolicy::DirectOnly,
        "hosted loopback evidence requires direct-only routing"
    );
    let builder = endpoint_builder_for_remote(route_policy, remote)?;
    if hosted_loopback {
        Ok(builder
            .relay_mode(iroh::RelayMode::Disabled)
            .clear_ip_transports()
            .clear_address_lookup()
            .portmapper_config(iroh::endpoint::PortmapperConfig::Disabled)
            .net_report_config(iroh::endpoint::NetReportConfig::minimal())
            .bind_addr((Ipv4Addr::LOCALHOST, 0))
            .context("bind hosted evidence dialer exclusively to IPv4 loopback")?)
    } else {
        Ok(builder)
    }
}

async fn wait_for_endpoint_online(endpoint: &Endpoint, route_policy: RoutePolicy) -> Result<()> {
    if route_policy == RoutePolicy::DirectOnly {
        println!("relay_status=not-required");
        return Ok(());
    }
    match timeout(Duration::from_secs(30), endpoint.online()).await {
        Ok(()) => {
            println!("relay_status=online");
            Ok(())
        }
        Err(_) if route_policy == RoutePolicy::RelayOnly => {
            bail!("relay-only evidence endpoint did not become online")
        }
        Err(_) => {
            println!("relay_status=timeout-direct-still-allowed");
            Ok(())
        }
    }
}

async fn accept_connection(endpoint: &Endpoint) -> Result<Connection> {
    loop {
        let incoming = timeout(ACCEPT_TIMEOUT, endpoint.accept())
            .await
            .context("accept evidence connection deadline")?
            .context("evidence endpoint closed")?;
        let accepting = match incoming.accept() {
            Ok(accepting) => accepting,
            Err(error) => {
                eprintln!("incoming_connection_ignored stage=initial error={error:#}");
                continue;
            }
        };
        match timeout(CONNECTION_TIMEOUT, accepting).await {
            Ok(Ok(connection)) => return Ok(connection),
            Ok(Err(error)) => {
                eprintln!("incoming_connection_ignored stage=handshake error={error:#}")
            }
            Err(_) => eprintln!("incoming_connection_ignored stage=handshake error=timeout"),
        }
    }
}

async fn open_bi(
    connection: &Connection,
) -> Result<(iroh::endpoint::SendStream, iroh::endpoint::RecvStream)> {
    timeout(STREAM_TIMEOUT, connection.open_bi())
        .await
        .context("open evidence stream deadline")?
        .context("open evidence stream")
}

fn write_ticket(
    path: &Path,
    config: &EvidenceRoleConfig,
    endpoint: &Endpoint,
    alpn: &[u8],
    advertise_ip: Option<IpAddr>,
) -> Result<()> {
    let endpoint_addr = if let Some(ip) = advertise_ip {
        let discovered_addr = endpoint.addr();
        let mut advertised = discovered_addr
            .ip_addrs()
            .filter(|address| address.ip() == ip)
            .collect::<Vec<_>>();
        advertised.sort_unstable();
        advertised.dedup();
        ensure!(
            advertised.len() == 1,
            "requested evidence advertise IP is not one exact local endpoint address"
        );
        EndpointAddr::new(endpoint.id()).with_ip_addr(SocketAddr::new(ip, advertised[0].port()))
    } else {
        endpoint.addr()
    };
    let ticket = EvidenceTicket {
        schema_version: TICKET_SCHEMA_VERSION,
        source_revision: config.source_revision.clone(),
        run_id: config.run_id.clone(),
        protocol: String::from_utf8(alpn.to_vec()).context("evidence ALPN is not UTF-8")?,
        endpoint: endpoint_addr,
    };
    write_json(path, &ticket)
}

fn read_ticket(path: &Path, config: &EvidenceRoleConfig, alpn: &[u8]) -> Result<EvidenceTicket> {
    let ticket: EvidenceTicket = serde_json::from_slice(
        &fs::read(path).with_context(|| format!("read evidence ticket {}", path.display()))?,
    )?;
    ensure!(
        ticket.schema_version == TICKET_SCHEMA_VERSION,
        "unsupported ticket schema"
    );
    ensure!(
        ticket.source_revision == config.source_revision,
        "ticket source revision mismatch"
    );
    ensure!(ticket.run_id == config.run_id, "ticket run id mismatch");
    ensure!(
        ticket.endpoint.id.to_string() == config.endpoint_id,
        "ticket endpoint id mismatch"
    );
    ensure!(
        ticket.protocol.as_bytes() == alpn,
        "ticket protocol mismatch"
    );
    Ok(ticket)
}

fn load_config(role_dir: &Path) -> Result<EvidenceRoleConfig> {
    let path = role_dir.join(ROLE_CONFIG_FILE);
    let config: EvidenceRoleConfig = serde_json::from_slice(
        &fs::read(&path)
            .with_context(|| format!("read evidence role config {}", path.display()))?,
    )?;
    ensure!(
        config.schema_version == EVIDENCE_SCHEMA_VERSION,
        "unsupported role schema"
    );
    validate_source_revision(&config.source_revision)?;
    validate_run_id(&config.run_id)?;
    Ok(config)
}

fn require_role(role_dir: &Path, expected: EvidenceRole) -> Result<EvidenceRoleConfig> {
    let config = load_config(role_dir)?;
    ensure!(config.role == expected, "evidence role mismatch");
    Ok(config)
}

#[allow(
    clippy::too_many_arguments,
    reason = "the bounded evidence record keeps every transport decision explicit at each call site"
)]
fn build_report(
    config: &EvidenceRoleConfig,
    store: &CanonicalSyncV2CarrierStore,
    scenario: &str,
    status: &str,
    protocol: &[u8],
    route_policy: Option<RoutePolicy>,
    selected_path: Option<&(String, String)>,
    canonical_selected: bool,
    detail: &str,
) -> Result<EvidenceReport> {
    let inventory = store.inventory(config.conversation_id)?;
    let client_runtime = load_client_runtime(config, store)?;
    let responder_runtime = load_responder_runtime(config, store)?;
    Ok(EvidenceReport {
        schema_version: EVIDENCE_SCHEMA_VERSION,
        source_revision: config.source_revision.clone(),
        run_id: config.run_id.clone(),
        role: config.role,
        scenario: scenario.to_owned(),
        status: status.to_owned(),
        protocol: String::from_utf8(protocol.to_vec())?,
        route_policy: route_policy.map(|policy| policy.as_str().to_owned()),
        selected_path: selected_path.map(|value| value.0.clone()),
        remote_address: selected_path.map(|value| value.1.clone()),
        inventory_count: inventory.len(),
        inventory_digest: digest_inventory(&inventory),
        client_action: client_action_label(&client_runtime),
        responder_action: responder_action_label(&responder_runtime),
        canonical_selected,
        legacy_dial_attempts: 0,
        fallback_attempts: 0,
        detail: detail.to_owned(),
    })
}

fn client_action_label(runtime: &kilogram_store::CanonicalSyncV2StoredSessionRuntime) -> String {
    let content = &runtime.checkpoint().checkpoint().content;
    if content.paused {
        return "paused".to_owned();
    }
    format!("{:?}", content.next_action)
}

fn responder_action_label(
    runtime: &kilogram_store::CanonicalSyncV2StoredResponderRuntime,
) -> String {
    let content = &runtime.checkpoint().checkpoint().content;
    if content.paused {
        return "paused".to_owned();
    }
    format!("{:?}", content.next_action)
}

fn inventory_digest(
    store: &CanonicalSyncV2CarrierStore,
    conversation_id: [u8; 32],
) -> Result<(usize, String)> {
    let inventory = store.inventory(conversation_id)?;
    Ok((inventory.len(), digest_inventory(&inventory)))
}

fn digest_inventory(inventory: &[CanonicalSyncEventKeyV2]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"kilogram:sync-v2-evidence-inventory:v1\0");
    for key in inventory {
        hasher.update(&key.namespace.code().to_be_bytes());
        hasher.update(&key.event_id);
    }
    hasher.finalize().to_hex().to_string()
}

fn write_optional_report(path: Option<&Path>, report: &EvidenceReport) -> Result<()> {
    if let Some(path) = path {
        write_json(path, report)?;
    }
    Ok(())
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes).with_context(|| format!("write {}", path.display()))
}

fn fixture_reference_event_id() -> Result<kilogram_protocol::EventId> {
    let value: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-SYNC-HISTORY-V2-VECTORS.json"
    )))?;
    let hex = value
        .pointer("/expected/legacy_carrier_cbor_hex")
        .and_then(serde_json::Value::as_str)
        .context("legacy carrier fixture is missing")?;
    let bytes = decode_hex(hex)?;
    let carrier = CanonicalSyncEventCarrierV2::decode_and_verify(&bytes)?;
    let CanonicalSyncEventCarrierV2::LegacyPostcardV6(bridge) = carrier else {
        bail!("legacy carrier fixture has wrong namespace")
    };
    bridge
        .into_active_authorized_event()
        .event()
        .event_id()
        .map_err(Into::into)
}

fn decode_hex(value: &str) -> Result<Vec<u8>> {
    ensure!(value.len().is_multiple_of(2), "hex string has odd length");
    (0..value.len())
        .step_by(2)
        .map(|offset| u8::from_str_radix(&value[offset..offset + 2], 16).map_err(Into::into))
        .collect()
}

fn canonical_session_authorization(
    root_secret: [u8; 32],
    device: &DeviceIdentity,
    session_binding: [u8; 32],
    authority_revision: u64,
) -> Result<(AccountId, CanonicalSignedDeviceSessionAuthorizationV1)> {
    let root = SigningKey::from_bytes(&root_secret);
    let account_id = AccountId::from_bytes(root.verifying_key().to_bytes());
    let certificate_content = CanonicalDeviceCertificateContentV1 {
        account_id: *account_id.as_bytes(),
        device_id: *device.device_id().as_bytes(),
        encryption_public_key: [root_secret[0]; 32],
        authority_sequence: 1,
        capabilities: vec![CanonicalDeviceCapabilityV1::SyncHistory],
    };
    let certificate = CanonicalSignedDeviceCertificateV1 {
        signature: root
            .sign(&device_certificate_signing_bytes_v1(&certificate_content)?)
            .to_bytes(),
        content: certificate_content,
    };
    let authority_content = CanonicalAccountAuthoritySnapshotContentV1 {
        account_id: *account_id.as_bytes(),
        revision: authority_revision,
        revocations: Vec::new(),
    };
    let authority_snapshot = CanonicalSignedAccountAuthoritySnapshotV1 {
        signature: root
            .sign(&account_authority_snapshot_signing_bytes_v1(
                &authority_content,
            )?)
            .to_bytes(),
        content: authority_content,
    };
    let content = CanonicalDeviceSessionAuthorizationContentV1 {
        session_binding,
        certificate,
        authority_snapshot,
    };
    let authorization = CanonicalSignedDeviceSessionAuthorizationV1 {
        signature: device.sign(&device_session_authorization_signing_bytes_v1(&content)?),
        content,
    };
    Ok((account_id, authorization))
}

fn sign_legacy_probe(identity: &DeviceIdentity, run_id: &str) -> Result<LegacyProbe> {
    let bytes = legacy_probe_signing_bytes(run_id, identity.device_id());
    Ok(LegacyProbe {
        run_id: run_id.to_owned(),
        device_id: identity.device_id(),
        signature: identity.sign(&bytes).to_vec(),
    })
}

fn verify_legacy_probe(probe: &LegacyProbe, run_id: &str, device_id: DeviceId) -> Result<()> {
    ensure!(probe.run_id == run_id, "legacy probe run mismatch");
    ensure!(probe.device_id == device_id, "legacy probe device mismatch");
    device_id.verify(
        &legacy_probe_signing_bytes(run_id, device_id),
        &probe.signature,
    )?;
    Ok(())
}

fn legacy_probe_signing_bytes(run_id: &str, device_id: DeviceId) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(LEGACY_PROBE_DOMAIN.len() + run_id.len() + 32);
    bytes.extend_from_slice(LEGACY_PROBE_DOMAIN);
    bytes.extend_from_slice(run_id.as_bytes());
    bytes.extend_from_slice(device_id.as_bytes());
    bytes
}

fn unix_time_now() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time precedes Unix epoch")?
        .as_secs())
}

fn default_run_id() -> String {
    format!("m1180-{}", unix_time_now().unwrap_or_default())
}

fn validate_source_revision(value: &str) -> Result<()> {
    ensure!(
        value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "source revision must be an exact 40-character commit id"
    );
    Ok(())
}

fn validate_run_id(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 96
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')),
        "run id contains unsupported characters"
    );
    Ok(())
}
