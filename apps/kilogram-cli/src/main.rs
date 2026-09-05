use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    future::Future,
    io::{self, Write},
    path::{Path, PathBuf},
    pin::Pin,
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use clap::{Parser, Subcommand, ValueEnum};
use iroh::{
    Endpoint, EndpointAddr, RelayUrl,
    endpoint::{Connection, RecvStream, SendStream},
};
use kilogram_identity::{
    AccountAuthoritySnapshot, AccountDeviceListSnapshot, AccountId, AccountRootState,
    AuthoritySnapshotStoreOutcome, AuthorizedDevice, ConversationMembershipSnapshot,
    ConversationMembershipStoreOutcome, ConversationScopeId, DeviceCapability, DeviceCertificate,
    DeviceId, DeviceIdentity, DeviceState, verify_device_authorization_with_snapshot,
};
use kilogram_protocol::{
    AuthorizedEvent, ClientRequest, ConversationId, DeviceAuthorizationAccepted,
    DeviceAuthorizationRejected, EventPayload, HistoryRewrapBundle, HistoryRewrapRejected,
    HistoryRewrapRejectionReason, HistoryRewrapSas, LocalTextProjection,
    MAX_ENDPOINT_ANNOUNCEMENT_WIRE_BYTES, MAX_HISTORY_REWRAP_ENTRIES, MAX_INVENTORY_EVENT_IDS,
    RatchetRecipient, ServerResponse, SignedDeviceSessionAuthorization, SignedEvent,
    SignedHistoryRecoveryCheckpoint, SignedHistoryRewrapRequest, SignedHistoryRewrapTransfer,
    SignedSyncInventory, SyncPause, SyncPaused, SyncSessionBinding,
};
use kilogram_ratchet::{
    AccountPrekeyDirectory, DEFAULT_PREKEY_POOL_SIZE, DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
    DecryptedMessage, MAX_PREKEY_POOL_SIZE, RatchetOperation, RatchetState, SignedPrekeyPool,
    SignedRatchetIdentity, unix_time_now,
};
use kilogram_runtime_ipc::{
    RuntimeIpcCommand, RuntimeIpcContactTicketRefresh, RuntimeIpcConversationSummary,
    RuntimeIpcDescriptor, RuntimeIpcDeviceDirectoryStatus, RuntimeIpcDeviceDirectoryUpdate,
    RuntimeIpcEndpointAnnouncementExport, RuntimeIpcEndpointAnnouncementImport,
    RuntimeIpcEndpointAnnouncementPush, RuntimeIpcEndpointCandidateState,
    RuntimeIpcEndpointCandidateStatus, RuntimeIpcEndpointTicketRefresh, RuntimeIpcHistoryCursor,
    RuntimeIpcHistoryMessage, RuntimeIpcHistoryPage, RuntimeIpcMessagePreview,
    RuntimeIpcNetworkClass, RuntimeIpcOutboxStatus, RuntimeIpcQueueItem, RuntimeIpcQueueState,
    RuntimeIpcRequestId, RuntimeIpcResponse, RuntimeIpcRoutePolicy, RuntimeIpcServer,
    RuntimeIpcTicketAutomationActionStatus, RuntimeIpcTicketAutomationStatus,
    RuntimeIpcTicketPublication, RuntimeIpcWork, RuntimeLaunchProfile, RuntimeLaunchSettings,
};
use kilogram_session::{
    MAX_SYNC_ROUNDS, ServerInventoryOutcome, SessionStore, SyncClient, SyncServer,
    authorize_device_session,
};
use kilogram_state::{
    DeviceIdentityStateRepository, EncryptedStateVault, STATE_VAULT_FILE, STATE_VAULT_KEY_FILE,
    StateDirectoryLock, StateError, StateMirrorRepository, StateRecordKind, StateTransaction,
    TrustStateRepository, TypedStateRepository, VaultIdentityShadowOutcome, VaultMigrationOutcome,
    VaultMirrorCommit, VaultMirrorOutcome, VaultPrimaryWriteRepository, VaultRecoveryWitness,
    VaultReport,
};
use kilogram_store::{
    AppendOnlyWriteReceipt, CommandEventReadOverlay, CommandLocalMessageReadOverlay,
    EventReadRepository, EventStore, ImmutableEventReadSnapshot, ImmutableLocalMessageReadSnapshot,
    LocalMessageReadRepository, LocalMessageStore, StoreError, StoreOutcome, StoredAuthorizedEvent,
};
use kilogram_transport_iroh::{
    ALPN, MAX_WIRE_MESSAGE_BYTES, RoutePolicy, SelectedPathDiagnostics, await_route_policy,
    endpoint_builder_for_remote, endpoint_builder_with_relay, read_client_request,
    read_server_response, selected_path_diagnostics, write_client_request, write_server_response,
};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use tokio::time::timeout;
use zeroize::Zeroizing;

mod recovery_discovery;
mod recovery_link;
mod recovery_plan;
mod recovery_platform;
mod recovery_qr;
mod recovery_scheduler;
mod runtime_endpoint_announcement;
mod runtime_publication;
mod runtime_queue;
mod runtime_ticket_automation;
mod runtime_ticket_checkpoint;

use recovery_discovery::{
    DEFAULT_DISCOVERY_CANDIDATES, DEFAULT_DISCOVERY_WAIT_SECONDS, MAX_DISCOVERY_CANDIDATES,
    MAX_DISCOVERY_WAIT_SECONDS, discover_recovery_links, loopback_target, multicast_target,
    publication_interval, start_recovery_discovery_publisher,
};
use recovery_link::{
    DEFAULT_HISTORY_RECOVERY_LINK_VALIDITY_SECONDS, HistoryRecoveryLinkOptions,
    MAX_HISTORY_RECOVERY_LINK_TEXT_BYTES, MAX_HISTORY_RECOVERY_LINK_VALIDITY_SECONDS,
    SignedHistoryRecoveryLink,
};
use recovery_plan::{
    DEFAULT_HISTORY_RECOVERY_PLAN_VALIDITY_HOURS, HistoryRecoveryPlanOptions,
    MAX_HISTORY_RECOVERY_PLAN_BYTES, MAX_HISTORY_RECOVERY_PLAN_VALIDITY_HOURS,
    RecoveryExecutionPolicy, RecoveryNetworkClass, RecoveryPowerSource, SignedHistoryRecoveryPlan,
};
use recovery_platform::{
    RecoveryPlatformChangeWait, print_recovery_platform_context, resolve_recovery_platform_context,
    subscribe_recovery_platform_changes, system_recovery_platform_context,
};
use recovery_qr::{
    RecoveryQrDecodeReport, RecoveryQrRenderReport, decode_recovery_link_qr_image,
    render_recovery_link_qr_png,
};
use recovery_scheduler::{
    DEFAULT_RECOVERY_RETRY_BASE_SECONDS, DEFAULT_RECOVERY_RETRY_MAX_SECONDS,
    MAX_RECOVERY_ATTEMPT_LEASE_SECONDS, MAX_RECOVERY_RETRY_BASE_SECONDS,
    MAX_RECOVERY_RETRY_MAX_SECONDS, RecoveryBackoffConfig, RecoverySchedulerLifecycle,
    RecoverySchedulerReadiness, SignedRecoverySchedulerState, load_recovery_scheduler_state,
    persist_recovery_scheduler_state,
};
use runtime_endpoint_announcement::{
    AcceptedEndpointObservationId, ContactEndpointAnnouncement,
    DEFAULT_ENDPOINT_ANNOUNCEMENT_VALIDITY_SECONDS, EncryptedEndpointAnnouncementBundle,
    EndpointAnnouncementBundleId, EndpointCandidateAnnouncement, MAX_ENDPOINT_ANNOUNCEMENT_BYTES,
    MAX_ENDPOINT_ANNOUNCEMENT_VALIDITY_SECONDS, SignedAcceptedEndpointObservation,
    SignedEndpointAnnouncementAcknowledgement, SignedEndpointAnnouncementBundle,
};
use runtime_publication::{
    DEFAULT_TICKET_PUBLICATION_TTL_SECONDS, EncryptedTicketPublication,
    MAX_TICKET_PUBLICATION_TTL_SECONDS, MIN_TICKET_PUBLICATION_TTL_SECONDS,
    SignedTicketPublication, SignedTicketPublicationObservation, TicketPublicationChannelId,
    TicketPublicationId, TicketPublicationObservationId, TicketPublicationStoreClient,
    TicketPublicationWriteCapability, TicketPublicationWriteKey,
};
use runtime_queue::{
    MAX_RUNTIME_RECORD_BYTES, RuntimeContactId, RuntimeDeviceDirectoryReceiptId,
    RuntimeEndpointCandidateId, RuntimeQueueId, SignedDeliveredMessage, SignedMaterializedMessage,
    SignedQueuedMessage, SignedRuntimeContact, SignedRuntimeDeviceDirectoryReceipt,
    SignedRuntimeEndpointCandidate, SignedRuntimeEndpointPublicationBinding,
    SignedRuntimeRetryState,
};
use runtime_ticket_automation::{
    DEFAULT_AUTOMATION_RETRY_BASE_SECONDS, DEFAULT_AUTOMATION_RETRY_MAX_SECONDS,
    DEFAULT_REFRESH_BEFORE_SECONDS, MAX_AUTOMATION_RETRY_SECONDS, MAX_REFRESH_BEFORE_SECONDS,
    MIN_REFRESH_BEFORE_SECONDS, SignedTicketAutomationAttempt, SignedTicketAutomationPolicy,
    TicketAutomationAction, TicketAutomationAttemptId, TicketAutomationPolicyId,
};
use runtime_ticket_checkpoint::{
    RuntimeTicketChainAnchor, RuntimeTicketCheckpointId, SignedRuntimeTicketCheckpoint,
};

const EVENT_STORE_DIRECTORY: &str = "events";
const LOCAL_MESSAGE_STORE_DIRECTORY: &str = "local-messages";
const HISTORY_REWRAP_STORE_DIRECTORY: &str = "history-rewraps";
const HISTORY_RECOVERY_STORE_DIRECTORY: &str = "history-recovery";
const RUNTIME_STATE_DIRECTORY: &str = "runtime";
const RUNTIME_CONTACTS_DIRECTORY: &str = "contacts";
const RUNTIME_ENDPOINT_CANDIDATES_DIRECTORY: &str = "endpoint-candidates";
const RUNTIME_ENDPOINT_PUBLICATION_BINDINGS_DIRECTORY: &str = "endpoint-publication-bindings";
const RUNTIME_OUTBOX_DIRECTORY: &str = "outbox";
const RUNTIME_DEVICE_DIRECTORY: &str = "device-directory";
const RUNTIME_TICKET_PUBLICATIONS_DIRECTORY: &str = "ticket-publications";
const RUNTIME_TICKET_OBSERVATIONS_DIRECTORY: &str = "ticket-observations";
const RUNTIME_ACCEPTED_ENDPOINT_OBSERVATIONS_DIRECTORY: &str = "accepted-endpoint-observations";
const RUNTIME_TICKET_AUTOMATION_POLICIES_DIRECTORY: &str = "ticket-automation-policies";
const RUNTIME_TICKET_AUTOMATION_ATTEMPTS_DIRECTORY: &str = "ticket-automation-attempts";
const RUNTIME_TICKET_CHECKPOINTS_DIRECTORY: &str = "ticket-checkpoints";
const MAX_RUNTIME_DEVICE_DIRECTORY_RECEIPTS: usize = 1_024;
const MAX_RUNTIME_ENDPOINT_CANDIDATES_PER_CONTACT: usize = 4;
const MAX_RUNTIME_TICKET_PUBLICATION_RECORDS: usize = 4_096;
const MAX_RUNTIME_TICKET_AUTOMATION_RECORDS: usize = 4_096;
const MAX_RUNTIME_TICKET_CHAIN_RECORDS_BEFORE_COMPACTION: usize = 8;
const RUNTIME_TICKET_AUTOMATION_CHECK_INTERVAL: Duration = Duration::from_secs(5);
const RUNTIME_DEVICE_LIST_DIGEST_DOMAIN: &[u8] = b"kilogram:runtime-device-list:v1\0";
const DIRECT_PATH_DIAGNOSTIC_WAIT: Duration = Duration::from_secs(3);
const ROUTE_POLICY_WAIT: Duration = Duration::from_secs(15);
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);
const CLIENT_RELAY_WAIT_SECONDS: u64 = 30;
const STREAM_OPEN_TIMEOUT: Duration = Duration::from_secs(15);
const HISTORY_RECOVERY_NEXT_PAGE_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_HISTORY_RECOVERY_PAGES_PER_SESSION: usize = 64;
const CLI_COORDINATOR_STACK_BYTES: usize = 8 * 1024 * 1024;
const TICKET_SIGNATURE_DOMAIN: &[u8] = b"kilogram:connection-ticket-signature:v10\0";
const TICKET_VERSION: u8 = 10;
const MAX_RECOVERY_PASSPHRASE_FILE_BYTES: u64 = 4098;
const DEFAULT_HISTORY_RECOVERY_SCHEDULER_ATTEMPTS: usize = 3;
const MAX_HISTORY_RECOVERY_SCHEDULER_ATTEMPTS: usize = 8;
const DEFAULT_HISTORY_RECOVERY_WORKER_RUNTIME_SECONDS: u64 = 60 * 60;
const MAX_HISTORY_RECOVERY_WORKER_RUNTIME_SECONDS: u64 = 24 * 60 * 60;
const DEFAULT_HISTORY_RECOVERY_WORKER_WAKEUPS: usize = 64;
const MAX_HISTORY_RECOVERY_WORKER_WAKEUPS: usize = 1024;
const DEFAULT_HISTORY_RECOVERY_WORKER_CANCEL_POLL_SECONDS: u64 = 5;
const MAX_HISTORY_RECOVERY_WORKER_CANCEL_POLL_SECONDS: u64 = 30;
const HISTORY_RECOVERY_WORKER_LOCK_RETRY: Duration = Duration::from_millis(25);
const HISTORY_RECOVERY_WORKER_LOCK_WAIT: Duration = Duration::from_secs(2);
const RUNTIME_STATE_LOCK_RETRY: Duration = Duration::from_millis(25);
const RUNTIME_STATE_LOCK_WAIT: Duration = Duration::from_secs(15);
const MAX_RUNTIME_SESSIONS: usize = 65_536;
const MAX_RUNTIME_IDLE_SECONDS: u64 = 24 * 60 * 60;
const DEFAULT_RUNTIME_POLL_MILLISECONDS: u64 = 250;
const MAX_RUNTIME_POLL_MILLISECONDS: u64 = 10_000;
const DEFAULT_RUNTIME_RETRY_BASE_SECONDS: u64 = 1;
const DEFAULT_RUNTIME_RETRY_MAX_SECONDS: u64 = 60;
const MAX_RUNTIME_RETRY_SECONDS: u64 = 3_600;
const DEFAULT_RUNTIME_AUTO_SYNC_SECONDS: u64 = 30;
const MAX_RUNTIME_AUTO_SYNC_SECONDS: u64 = 3_600;
const MAX_RUNTIME_CONTACTS: usize = 256;
const MAX_RUNTIME_QUEUE_ITEMS: usize = 4_096;
const MAX_RUNTIME_RETRY_STATES: usize = 4_096;
const MAX_RUNTIME_HISTORY_PAGE_SIZE: u16 = 100;
const MAX_RUNTIME_HISTORY_PAGE_BODY_BYTES: usize = 192 * 1024;
const MAX_RUNTIME_PREVIEW_BYTES: usize = 96;
const MAX_RUNTIME_CONVERSATION_LIST_EVENTS: usize = 16_384;
const RUNTIME_HISTORY_SNAPSHOT_DOMAIN: &[u8] = b"kilogram:runtime-history-snapshot:v1\0";

type CommandFuture = Pin<Box<dyn Future<Output = Result<()>>>>;

#[derive(Debug, Parser)]
#[command(
    name = "kilogram-cli",
    version,
    about = "Kilogram M0: exchange signed events over an authenticated Iroh connection"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Handle one delivery or synchronization connection, then exit.
    Listen {
        /// Directory containing this application's persistent development device identity.
        #[arg(long)]
        state_dir: PathBuf,

        /// Account ID allowed to authenticate a certified requester device.
        #[arg(long)]
        allow_account: AccountId,

        /// Root-signed complete device list for this listener account.
        #[arg(long)]
        device_list_file: PathBuf,

        /// Signed current prekey pool for another device in this account. Repeat for every peer device.
        #[arg(long = "peer-prekey-pool-file")]
        peer_prekey_pool_files: Vec<PathBuf>,

        /// Also write the public connection ticket to this file.
        #[arg(long)]
        ticket_file: Option<PathBuf>,

        /// How long to wait for a public relay before accepting local connections.
        #[arg(long, default_value_t = 15)]
        relay_wait_seconds: u64,

        /// Transport path required for Kilogram application frames.
        #[arg(long, value_enum, default_value = "auto")]
        route_policy: RoutePolicyArg,

        /// Restrict this listener to one explicit relay URL.
        #[arg(long)]
        relay_url: Option<RelayUrl>,

        /// Explicitly approve serving this conversation through authenticated history rewrap.
        #[arg(long)]
        history_rewrap_conversation: Option<String>,

        /// Exact same-account recipient device approved for history rewrap.
        #[arg(long)]
        history_rewrap_recipient_device: Option<DeviceId>,

        /// SAS compared out of band and explicitly approved by the source user.
        #[arg(long)]
        history_rewrap_approve_sas: Option<String>,

        /// First canonical history index approved for a network rewrap request.
        #[arg(long, default_value_t = 0)]
        history_rewrap_range_start: usize,

        /// Total consecutive text-event window approved for paginated recovery.
        #[arg(long, default_value_t = MAX_HISTORY_REWRAP_ENTRIES)]
        history_rewrap_count: usize,

        /// Write a compact signed, recipient-specific recovery link for copying or QR rendering.
        #[arg(long)]
        history_recovery_link_file: Option<PathBuf>,

        /// Render the signed recovery link directly as a no-clobber PNG QR code.
        #[arg(long)]
        history_recovery_qr_file: Option<PathBuf>,

        /// Recommended authenticated page size embedded in the recovery link.
        #[arg(long, default_value_t = 64)]
        history_recovery_link_page_size: usize,

        /// Lifetime of the signed recovery link.
        #[arg(
            long,
            default_value_t = DEFAULT_HISTORY_RECOVERY_LINK_VALIDITY_SECONDS
        )]
        history_recovery_link_valid_for_seconds: u64,

        /// Explicitly publish the signed recipient-specific recovery link on the local network.
        #[arg(long)]
        history_recovery_discovery_publish: bool,
    },

    /// Keep one stable endpoint online and serve successive messaging/sync sessions.
    Runtime {
        /// Directory containing this application's persistent device state.
        #[arg(long)]
        state_dir: PathBuf,

        /// Account ID allowed to authenticate a certified requester device.
        #[arg(long)]
        allow_account: AccountId,

        /// Root-signed complete device list for this runtime account.
        #[arg(long)]
        device_list_file: PathBuf,

        /// Signed current prekey pool for another device in this account. Repeat for every peer device.
        #[arg(long = "peer-prekey-pool-file")]
        peer_prekey_pool_files: Vec<PathBuf>,

        /// Atomically publish the current runtime connection ticket to this file.
        #[arg(long)]
        ticket_file: Option<PathBuf>,

        /// How long to wait for a public relay before accepting local connections.
        #[arg(long, default_value_t = 15)]
        relay_wait_seconds: u64,

        /// Transport path required for Kilogram application frames.
        #[arg(long, value_enum, default_value = "auto")]
        route_policy: RoutePolicyArg,

        /// Restrict this runtime to one explicit relay URL.
        #[arg(long)]
        relay_url: Option<RelayUrl>,

        /// Stop cleanly after this many accepted sessions; zero runs until Ctrl+C.
        #[arg(long, default_value_t = 0)]
        max_sessions: usize,

        /// Stop after this many idle seconds; zero disables the idle bound.
        #[arg(long, default_value_t = 0)]
        idle_seconds: u64,

        /// How often to observe newly queued local work and refreshed peer descriptors.
        #[arg(long, default_value_t = DEFAULT_RUNTIME_POLL_MILLISECONDS)]
        poll_milliseconds: u64,

        /// Initial persistent retry delay for failed queued deliveries.
        #[arg(long, default_value_t = DEFAULT_RUNTIME_RETRY_BASE_SECONDS)]
        retry_base_seconds: u64,

        /// Maximum persistent retry delay for failed queued deliveries.
        #[arg(long, default_value_t = DEFAULT_RUNTIME_RETRY_MAX_SECONDS)]
        retry_max_seconds: u64,

        /// Periodic automatic sync interval per contact; zero disables it.
        #[arg(long, default_value_t = DEFAULT_RUNTIME_AUTO_SYNC_SECONDS)]
        auto_sync_seconds: u64,

        /// Stop after this many outbound delivery/sync network actions; zero is unbounded.
        #[arg(long, default_value_t = 0)]
        max_outbound_actions: usize,

        /// Atomically publish an authenticated loopback IPC descriptor for local UI clients.
        #[arg(long)]
        ipc_file: Option<PathBuf>,
    },

    /// Write a no-clobber, secret-free profile that the desktop client can launch.
    RuntimeProfileCreate {
        /// New machine-local runtime launch profile.
        #[arg(long)]
        profile_file: PathBuf,

        /// Directory containing this application's persistent device state.
        #[arg(long)]
        state_dir: PathBuf,

        /// Account ID allowed to authenticate a certified requester device.
        #[arg(long)]
        allow_account: AccountId,

        /// Root-signed complete device list for this runtime account.
        #[arg(long)]
        device_list_file: PathBuf,

        /// Signed current prekey pool for another device in this account. Repeat for every peer device.
        #[arg(long = "peer-prekey-pool-file")]
        peer_prekey_pool_files: Vec<PathBuf>,

        /// Atomically publish the current runtime connection ticket to this file.
        #[arg(long)]
        ticket_file: Option<PathBuf>,

        /// How long to wait for a public relay before accepting local connections.
        #[arg(long, default_value_t = 15)]
        relay_wait_seconds: u64,

        /// Transport path required for Kilogram application frames.
        #[arg(long, value_enum, default_value = "auto")]
        route_policy: RoutePolicyArg,

        /// Restrict this runtime to one explicit relay URL.
        #[arg(long)]
        relay_url: Option<RelayUrl>,

        /// How often to observe newly queued local work and refreshed peer descriptors.
        #[arg(long, default_value_t = DEFAULT_RUNTIME_POLL_MILLISECONDS)]
        poll_milliseconds: u64,

        /// Initial persistent retry delay for failed queued deliveries.
        #[arg(long, default_value_t = DEFAULT_RUNTIME_RETRY_BASE_SECONDS)]
        retry_base_seconds: u64,

        /// Maximum persistent retry delay for failed queued deliveries.
        #[arg(long, default_value_t = DEFAULT_RUNTIME_RETRY_MAX_SECONDS)]
        retry_max_seconds: u64,

        /// Periodic automatic sync interval per contact; zero disables it.
        #[arg(long, default_value_t = DEFAULT_RUNTIME_AUTO_SYNC_SECONDS)]
        auto_sync_seconds: u64,

        /// Runtime-owned authenticated loopback IPC descriptor.
        #[arg(long)]
        ipc_file: PathBuf,
    },

    /// Run from a validated launch profile created for the desktop client.
    RuntimeFromProfile {
        /// Machine-local runtime launch profile.
        #[arg(long)]
        profile_file: PathBuf,
    },

    /// Enroll a signed peer endpoint; repeat for up to four devices of one contact.
    RuntimeContactAdd {
        /// Directory containing this application's persistent device state.
        #[arg(long)]
        state_dir: PathBuf,

        /// Development conversation label whose membership is already installed.
        #[arg(long)]
        conversation: String,

        /// Trusted peer Account ID expected in the current descriptor.
        #[arg(long)]
        expect_account: AccountId,

        /// Current peer runtime ticket; the same path may be atomically refreshed later.
        #[arg(long)]
        descriptor_file: PathBuf,
    },

    /// Add one locally encrypted message to the durable runtime outbox.
    RuntimeQueueMessage {
        /// Directory containing this application's persistent device state.
        #[arg(long)]
        state_dir: PathBuf,

        /// Contact conversation label.
        #[arg(long)]
        conversation: String,

        /// Peer account selecting the exact signed runtime contact.
        #[arg(long)]
        peer_account: AccountId,

        /// UTF-8 plaintext sealed immediately to this local device.
        #[arg(long)]
        message: String,
    },

    /// Verify and summarize the durable runtime contact/outbox state.
    RuntimeOutboxStatus {
        /// Directory containing this application's persistent device state.
        #[arg(long)]
        state_dir: PathBuf,
    },

    /// Authenticate to a running local runtime and print its identity.
    RuntimeIpcPing {
        /// Runtime-owned local IPC descriptor.
        #[arg(long)]
        ipc_file: PathBuf,
    },

    /// Queue one message through the running runtime actor.
    RuntimeIpcQueueMessage {
        /// Runtime-owned local IPC descriptor.
        #[arg(long)]
        ipc_file: PathBuf,

        /// Contact conversation label.
        #[arg(long)]
        conversation: String,

        /// Peer account selecting the exact signed runtime contact.
        #[arg(long)]
        peer_account: AccountId,

        /// UTF-8 plaintext transferred only over authenticated loopback IPC.
        #[arg(long)]
        message: String,

        /// Stable idempotency key to reuse after an uncertain local response.
        #[arg(long)]
        request_id: Option<RuntimeIpcRequestId>,
    },

    /// Read structured outbox status through the running runtime actor.
    RuntimeIpcOutboxStatus {
        /// Runtime-owned local IPC descriptor.
        #[arg(long)]
        ipc_file: PathBuf,
    },

    /// Apply a refreshed Root-signed own device directory to a running runtime.
    RuntimeIpcApplyDeviceDirectory {
        /// Runtime-owned local IPC descriptor.
        #[arg(long)]
        ipc_file: PathBuf,

        /// Root-signed complete local-account device list after revocation.
        #[arg(long)]
        device_list_file: PathBuf,
    },

    /// Inspect the authenticated own-device directory applied by the runtime.
    RuntimeIpcDeviceDirectoryStatus {
        /// Runtime-owned local IPC descriptor.
        #[arg(long)]
        ipc_file: PathBuf,
    },

    /// Publish this runtime's current connection ticket for one enrolled contact.
    RuntimeIpcPublishTicket {
        /// Runtime-owned local IPC descriptor.
        #[arg(long)]
        ipc_file: PathBuf,

        /// Contact conversation label.
        #[arg(long)]
        conversation: String,

        /// Peer account selecting the exact signed runtime contact.
        #[arg(long)]
        peer_account: AccountId,

        /// HTTPS base URL of the opaque ticket-publication store.
        #[arg(long)]
        service_base_url: String,

        /// Short publication lifetime; refresh before it expires.
        #[arg(long, default_value_t = DEFAULT_TICKET_PUBLICATION_TTL_SECONDS)]
        ttl_seconds: u64,
    },

    /// Fetch and install a newer signed ticket for one enrolled contact.
    RuntimeIpcRefreshContactTicket {
        /// Runtime-owned local IPC descriptor.
        #[arg(long)]
        ipc_file: PathBuf,

        /// Contact conversation label.
        #[arg(long)]
        conversation: String,

        /// Peer account selecting the exact signed runtime contact.
        #[arg(long)]
        peer_account: AccountId,

        /// HTTPS base URL of the opaque ticket-publication store.
        #[arg(long)]
        service_base_url: String,
    },

    /// Opt in, update, or disable durable automatic ticket exchange for one contact.
    RuntimeIpcConfigureTicketAutomation {
        /// Runtime-owned local IPC descriptor.
        #[arg(long)]
        ipc_file: PathBuf,

        /// Contact conversation label.
        #[arg(long)]
        conversation: String,

        /// Peer account selecting the exact signed runtime contact.
        #[arg(long)]
        peer_account: AccountId,

        /// Set false to append a disabled policy head without deleting history.
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        enabled: bool,

        /// HTTPS base URL of the opaque ticket-publication store.
        #[arg(long)]
        service_base_url: String,

        /// Lifetime of each publication.
        #[arg(long, default_value_t = DEFAULT_TICKET_PUBLICATION_TTL_SECONDS)]
        ttl_seconds: u64,

        /// Refresh this many seconds before the last successful expiry.
        #[arg(long, default_value_t = DEFAULT_REFRESH_BEFORE_SECONDS)]
        refresh_before_seconds: u64,

        /// Initial retry delay after a failed automatic action.
        #[arg(long, default_value_t = DEFAULT_AUTOMATION_RETRY_BASE_SECONDS)]
        retry_base_seconds: u64,

        /// Maximum exponential retry delay.
        #[arg(long, default_value_t = DEFAULT_AUTOMATION_RETRY_MAX_SECONDS)]
        retry_max_seconds: u64,

        /// Deny automatic exchange on wired networks.
        #[arg(long)]
        deny_ethernet: bool,

        /// Deny automatic exchange on Wi-Fi.
        #[arg(long)]
        deny_wifi: bool,

        /// Explicitly allow automatic exchange on metered/mobile networks.
        #[arg(long)]
        allow_mobile: bool,

        /// Explicitly allow exchange when the OS cannot classify the network.
        #[arg(long)]
        allow_unknown_network: bool,
    },

    /// Inspect durable ticket-automation policy and scheduler state.
    RuntimeIpcTicketAutomationStatus {
        /// Runtime-owned local IPC descriptor.
        #[arg(long)]
        ipc_file: PathBuf,
    },

    /// Export bounded endpoint candidates to another authorized device of this account.
    RuntimeIpcExportEndpointAnnouncements {
        /// Runtime-owned local IPC descriptor.
        #[arg(long)]
        ipc_file: PathBuf,

        /// Existing active account device that alone can decrypt the bundle.
        #[arg(long)]
        recipient_device: DeviceId,

        /// New output file for the encrypted, source-signed bundle.
        #[arg(long)]
        output_file: PathBuf,

        /// Short replay window for transferring the bundle.
        #[arg(long, default_value_t = DEFAULT_ENDPOINT_ANNOUNCEMENT_VALIDITY_SECONDS)]
        validity_seconds: u64,
    },

    /// Import endpoint candidates announced by another authorized device of this account.
    RuntimeIpcImportEndpointAnnouncements {
        /// Runtime-owned local IPC descriptor.
        #[arg(long)]
        ipc_file: PathBuf,

        /// Recipient-encrypted endpoint announcement bundle.
        #[arg(long)]
        bundle_file: PathBuf,

        /// Runtime-managed directory for imported public connection tickets.
        #[arg(long)]
        descriptor_directory: PathBuf,
    },

    /// Push endpoint candidates to another running device of the same account.
    RuntimeIpcPushEndpointAnnouncements {
        /// Runtime-owned local IPC descriptor.
        #[arg(long)]
        ipc_file: PathBuf,

        /// Fresh connection ticket published by the recipient runtime for this account.
        #[arg(long)]
        recipient_ticket_file: PathBuf,

        /// Short replay window for the recipient-encrypted bundle.
        #[arg(long, default_value_t = DEFAULT_ENDPOINT_ANNOUNCEMENT_VALIDITY_SECONDS)]
        validity_seconds: u64,
    },

    /// Connect to a listener, send one message, print its acknowledgement, then exit.
    Connect {
        /// Directory containing this application's persistent development device identity.
        #[arg(long)]
        state_dir: PathBuf,

        /// Connection ticket printed by the listener.
        #[arg(long, conflicts_with = "ticket_file")]
        ticket: Option<String>,

        /// Read the connection ticket from this file.
        #[arg(long, conflicts_with = "ticket")]
        ticket_file: Option<PathBuf>,

        /// UTF-8 message to send.
        #[arg(long, default_value = "hello from kilogram")]
        message: String,

        /// Development-only shared label used to derive a conversation ID.
        #[arg(long, default_value = "m0-local-smoke")]
        conversation: String,

        /// Trusted Account ID expected for the listener certificate in the ticket.
        #[arg(long)]
        expect_account: AccountId,
    },

    /// Reconcile bounded conversation event batches with a listener until converged.
    Sync {
        /// Directory containing this application's development state.
        #[arg(long)]
        state_dir: PathBuf,

        /// Connection ticket printed by the listener.
        #[arg(long, conflicts_with = "ticket_file")]
        ticket: Option<String>,

        /// Read the connection ticket from this file.
        #[arg(long, conflicts_with = "ticket")]
        ticket_file: Option<PathBuf>,

        /// Development-only shared label used to derive a conversation ID.
        #[arg(long, default_value = "m0-local-smoke")]
        conversation: String,

        /// Stop cleanly after this many completed rounds if more events remain.
        #[arg(long, default_value_t = MAX_SYNC_ROUNDS)]
        max_rounds: usize,

        /// Trusted Account ID expected for the listener certificate in the ticket.
        #[arg(long)]
        expect_account: AccountId,
    },

    /// Add signed local-only events for deterministic synchronization tests.
    SeedHistory {
        /// Directory containing this application's development state.
        #[arg(long)]
        state_dir: PathBuf,

        /// Development-only shared label used to derive a conversation ID.
        #[arg(long, default_value = "m0-local-smoke")]
        conversation: String,

        /// Number of chained local events to create.
        #[arg(long, default_value_t = 70)]
        count: usize,

        /// Prefix used in the generated test message bodies.
        #[arg(long, default_value = "seed")]
        message_prefix: String,

        /// Public certificate for the peer device that must also decrypt the fixtures.
        #[arg(long)]
        peer_certificate_file: PathBuf,

        /// Root-signed device list containing exactly this development peer.
        #[arg(long)]
        peer_device_list_file: PathBuf,

        /// Device-signed fresh one-time prekey pool for the peer recipient.
        #[arg(long)]
        peer_prekey_pool_file: PathBuf,
    },

    /// Export this device's signed asynchronous Olm prekey bundle.
    RatchetBundle {
        /// Directory containing this application's development device state.
        #[arg(long)]
        state_dir: PathBuf,

        /// Write the signed public bundle to this new file.
        #[arg(long)]
        bundle_file: PathBuf,
    },

    /// Export or explicitly rotate this device's signed Olm prekey pool.
    RatchetPrekeyPool {
        /// Directory containing this application's development device state.
        #[arg(long)]
        state_dir: PathBuf,

        /// Write the signed public pool to this new file.
        #[arg(long)]
        pool_file: PathBuf,

        /// Number of independently consumable one-time keys.
        #[arg(long, default_value_t = DEFAULT_PREKEY_POOL_SIZE)]
        count: usize,

        /// Freshness lifetime advertised by the signed pool.
        #[arg(long, default_value_t = 168)]
        valid_for_hours: u64,

        /// Rotate even when a current fresh pool with the requested size exists.
        #[arg(long)]
        refresh: bool,
    },

    /// Export an authenticated plaintext-history range to another device of this account.
    HistoryRewrapExport {
        /// Directory containing the live source device and readable local history.
        #[arg(long)]
        state_dir: PathBuf,

        /// Development-only shared label used to derive a conversation ID.
        #[arg(long, default_value = "m0-local-smoke")]
        conversation: String,

        /// Fresh root-signed list containing both source and recipient devices.
        #[arg(long)]
        device_list_file: PathBuf,

        /// Authorized device that should receive the rewrapped history.
        #[arg(long)]
        recipient_device: DeviceId,

        /// First canonical text-event inventory index to export.
        #[arg(long, default_value_t = 0)]
        range_start: usize,

        /// Maximum number of consecutive text events in this bundle.
        #[arg(long, default_value_t = MAX_HISTORY_REWRAP_ENTRIES)]
        count: usize,

        /// Write the signed encrypted rewrap bundle to this new file.
        #[arg(long)]
        bundle_file: PathBuf,
    },

    /// Import an authenticated history-rewrap bundle addressed to this device.
    HistoryRewrapImport {
        /// Directory containing the new or restored recipient device state.
        #[arg(long)]
        state_dir: PathBuf,

        /// Development-only shared label expected in the bundle.
        #[arg(long, default_value = "m0-local-smoke")]
        conversation: String,

        /// Signed encrypted rewrap bundle received from a live account device.
        #[arg(long)]
        bundle_file: PathBuf,
    },

    /// Derive the out-of-band SAS for a source/recipient pair in one signed device list.
    HistoryRewrapSas {
        /// Root-signed account device list used by both devices.
        #[arg(long)]
        device_list_file: PathBuf,

        /// Live source device that can read and rewrap the old history.
        #[arg(long)]
        source_device: DeviceId,

        /// New same-account device that will receive the history.
        #[arg(long)]
        recipient_device: DeviceId,
    },

    /// Fetch one approved history range over an authenticated device session.
    HistoryRewrapFetch {
        /// Directory containing the recipient device state.
        #[arg(long)]
        state_dir: PathBuf,

        /// Connection ticket printed by the source listener.
        #[arg(long, conflicts_with = "ticket_file")]
        ticket: Option<String>,

        /// Read the source connection ticket from this file.
        #[arg(long, conflicts_with = "ticket")]
        ticket_file: Option<PathBuf>,

        /// Development-only shared label expected in the transfer.
        #[arg(long, default_value = "m0-local-smoke")]
        conversation: String,

        /// First canonical text-event inventory index to request.
        #[arg(long, default_value_t = 0)]
        range_start: usize,

        /// Maximum number of consecutive text events to request.
        #[arg(long, default_value_t = MAX_HISTORY_REWRAP_ENTRIES)]
        count: usize,

        /// SAS independently compared by both users before the request is sent.
        #[arg(long)]
        confirm_sas: String,

        /// Trusted Account ID shared by the source and recipient devices.
        #[arg(long)]
        expect_account: AccountId,
    },

    /// Fetch the next authenticated page and atomically advance a signed local checkpoint.
    HistoryRecoveryResume {
        /// Directory containing the recipient device state and recovery checkpoints.
        #[arg(long)]
        state_dir: PathBuf,

        /// Connection ticket printed by the explicitly selected source listener.
        #[arg(long, conflicts_with = "ticket_file")]
        ticket: Option<String>,

        /// Read the selected source connection ticket from this file.
        #[arg(long, conflicts_with = "ticket")]
        ticket_file: Option<PathBuf>,

        /// Development-only shared label expected in every page.
        #[arg(long, default_value = "m0-local-smoke")]
        conversation: String,

        /// Exact live source device selected by the recipient user.
        #[arg(long)]
        source_device: DeviceId,

        /// First canonical inventory index covered by this recovery plan.
        #[arg(long, default_value_t = 0)]
        range_start: usize,

        /// Total consecutive inventory window approved for this recovery plan.
        #[arg(long)]
        count: usize,

        /// Maximum events requested in each authenticated recovery page.
        #[arg(long, default_value_t = 64)]
        page_size: usize,

        /// Maximum pages transferred over this one authenticated connection.
        #[arg(long, default_value_t = MAX_HISTORY_RECOVERY_PAGES_PER_SESSION)]
        max_pages: usize,

        /// SAS independently compared by both users before recovery starts.
        #[arg(long)]
        confirm_sas: String,

        /// Trusted account shared by source and recipient devices.
        #[arg(long)]
        expect_account: AccountId,
    },

    /// Verify and display a compact signed recovery link without connecting.
    HistoryRecoveryLinkInspect {
        /// Recovery URI copied from the source listener.
        #[arg(long, conflicts_with_all = ["link_file", "qr_file"])]
        link: Option<String>,

        /// Read the recovery URI from this file.
        #[arg(long, conflicts_with_all = ["link", "qr_file"])]
        link_file: Option<PathBuf>,

        /// Decode the recovery URI from one bounded PNG or JPEG containing exactly one QR.
        #[arg(long, conflicts_with_all = ["link", "link_file"])]
        qr_file: Option<PathBuf>,
    },

    /// Render a verified compact recovery link as a no-clobber PNG QR code.
    HistoryRecoveryLinkQrRender {
        /// Recovery URI copied from the source listener.
        #[arg(long, conflicts_with = "link_file")]
        link: Option<String>,

        /// Read the recovery URI from this file.
        #[arg(long, conflicts_with = "link")]
        link_file: Option<PathBuf>,

        /// New PNG file that will receive the QR code.
        #[arg(long)]
        qr_file: PathBuf,
    },

    /// Explicitly accept a verified recovery link and run its bounded recovery plan.
    HistoryRecoveryLinkAccept {
        /// Directory containing the exact recipient device named by the link.
        #[arg(long)]
        state_dir: PathBuf,

        /// Recovery URI copied from the source listener.
        #[arg(long, conflicts_with_all = ["link_file", "qr_file"])]
        link: Option<String>,

        /// Read the recovery URI from this file.
        #[arg(long, conflicts_with_all = ["link", "qr_file"])]
        link_file: Option<PathBuf>,

        /// Decode the recovery URI from one bounded PNG or JPEG containing exactly one QR.
        #[arg(long, conflicts_with_all = ["link", "link_file"])]
        qr_file: Option<PathBuf>,

        /// Local conversation label whose derived ID must match the signed link.
        #[arg(long)]
        conversation: String,

        /// SAS independently compared by both users before accepting the link.
        #[arg(long)]
        confirm_sas: String,

        /// Maximum pages transferred over this one authenticated connection.
        #[arg(long, default_value_t = MAX_HISTORY_RECOVERY_PAGES_PER_SESSION)]
        max_pages: usize,
    },

    /// Discover verified recipient-specific recovery links on the local network without connecting.
    HistoryRecoveryLinkDiscover {
        /// Directory containing the exact recipient device named by discovered links.
        #[arg(long)]
        state_dir: PathBuf,

        /// Local conversation label whose derived ID must match every candidate.
        #[arg(long)]
        conversation: String,

        /// Optionally restrict discovery to one exact source device.
        #[arg(long)]
        expect_source: Option<DeviceId>,

        /// Bounded time to listen for local discovery publications.
        #[arg(
            long,
            default_value_t = DEFAULT_DISCOVERY_WAIT_SECONDS,
            value_parser = clap::value_parser!(u64).range(1..=MAX_DISCOVERY_WAIT_SECONDS)
        )]
        wait_seconds: u64,

        /// Maximum distinct verified candidates to collect before stopping.
        #[arg(long, default_value_t = DEFAULT_DISCOVERY_CANDIDATES)]
        max_candidates: usize,

        /// Write the signed URI only when discovery finds exactly one candidate.
        #[arg(long)]
        output_link_file: Option<PathBuf>,
    },

    /// Explicitly approve and sign a bounded background recovery plan without connecting.
    HistoryRecoveryPlanApprove {
        /// Directory containing the exact recipient device named by the link.
        #[arg(long)]
        state_dir: PathBuf,

        /// Recovery URI copied from the source listener.
        #[arg(long, conflicts_with_all = ["link_file", "qr_file"])]
        link: Option<String>,

        /// Read the recovery URI from this file.
        #[arg(long, conflicts_with_all = ["link", "qr_file"])]
        link_file: Option<PathBuf>,

        /// Decode the recovery URI from one bounded PNG or JPEG containing exactly one QR.
        #[arg(long, conflicts_with_all = ["link", "link_file"])]
        qr_file: Option<PathBuf>,

        /// Local conversation label whose derived ID must match the signed link.
        #[arg(long)]
        conversation: String,

        /// SAS independently compared before granting durable retry consent.
        #[arg(long)]
        confirm_sas: String,

        /// New no-clobber file that will receive the recipient-signed plan.
        #[arg(long)]
        plan_file: PathBuf,

        /// Do not run this plan while the platform reports Ethernet.
        #[arg(long)]
        deny_ethernet: bool,

        /// Do not run this plan while the platform reports Wi-Fi.
        #[arg(long)]
        deny_wifi: bool,

        /// Permit this plan while the platform reports a mobile/metered network.
        #[arg(long)]
        allow_mobile: bool,

        /// Permit this plan when the platform cannot classify the network.
        #[arg(long)]
        allow_unknown_network: bool,

        /// Permit retries only while external power is reported.
        #[arg(long)]
        require_external_power: bool,

        /// Lifetime of the recipient-signed retry consent.
        #[arg(
            long,
            default_value_t = DEFAULT_HISTORY_RECOVERY_PLAN_VALIDITY_HOURS,
            value_parser = clap::value_parser!(u64).range(1..=MAX_HISTORY_RECOVERY_PLAN_VALIDITY_HOURS)
        )]
        valid_for_hours: u64,
    },

    /// Run bounded discovery/recovery retries under a previously signed local plan.
    HistoryRecoveryPlanRun {
        /// Directory containing the recipient device and recovery checkpoints.
        #[arg(long)]
        state_dir: PathBuf,

        /// Recipient-signed plan created by history-recovery-plan-approve.
        #[arg(long)]
        plan_file: PathBuf,

        /// Local conversation label whose derived ID must match the approved plan.
        #[arg(long)]
        conversation: String,

        /// Development override for the current network class; omit with power-source to use the OS probe.
        #[arg(long, value_enum)]
        network_class: Option<RecoveryNetworkClass>,

        /// Development override for the current power source; omit with network-class to use the OS probe.
        #[arg(long, value_enum)]
        power_source: Option<RecoveryPowerSource>,

        /// Maximum discovery/recovery attempts in this bounded run.
        #[arg(long, default_value_t = DEFAULT_HISTORY_RECOVERY_SCHEDULER_ATTEMPTS)]
        max_attempts: usize,

        /// LAN discovery duration within each attempt.
        #[arg(
            long,
            default_value_t = DEFAULT_DISCOVERY_WAIT_SECONDS,
            value_parser = clap::value_parser!(u64).range(1..=MAX_DISCOVERY_WAIT_SECONDS)
        )]
        discovery_wait_seconds: u64,

        /// Initial persistent retry delay before exponential backoff and jitter.
        #[arg(
            long = "retry-base-seconds",
            alias = "retry-delay-seconds",
            default_value_t = DEFAULT_RECOVERY_RETRY_BASE_SECONDS,
            value_parser = clap::value_parser!(u64).range(0..=MAX_RECOVERY_RETRY_BASE_SECONDS)
        )]
        retry_base_seconds: u64,

        /// Maximum persistent retry delay after exponential backoff and jitter.
        #[arg(
            long,
            default_value_t = DEFAULT_RECOVERY_RETRY_MAX_SECONDS,
            value_parser = clap::value_parser!(u64).range(0..=MAX_RECOVERY_RETRY_MAX_SECONDS)
        )]
        retry_max_seconds: u64,

        /// Maximum pages transferred over any one authenticated connection.
        #[arg(long, default_value_t = MAX_HISTORY_RECOVERY_PAGES_PER_SESSION)]
        max_pages: usize,
    },

    /// Watch one approved plan and wake its bounded runner on native platform changes or deadlines.
    HistoryRecoveryPlanWatch {
        /// Directory containing the recipient device, checkpoints, and scheduler state.
        #[arg(long)]
        state_dir: PathBuf,

        /// Recipient-signed plan created by history-recovery-plan-approve.
        #[arg(long)]
        plan_file: PathBuf,

        /// Local conversation label whose derived ID must match the approved plan.
        #[arg(long)]
        conversation: String,

        /// Active runtime bound; bounded signed-state cleanup may follow.
        #[arg(
            long,
            default_value_t = DEFAULT_HISTORY_RECOVERY_WORKER_RUNTIME_SECONDS,
            value_parser = clap::value_parser!(u64).range(1..=MAX_HISTORY_RECOVERY_WORKER_RUNTIME_SECONDS)
        )]
        max_runtime_seconds: u64,

        /// Maximum initial/deadline/platform/scheduler-state wakes handled by this process.
        #[arg(long, default_value_t = DEFAULT_HISTORY_RECOVERY_WORKER_WAKEUPS)]
        max_wakeups: usize,

        /// Maximum delay before observing a signed cancellation made by another process.
        #[arg(
            long,
            default_value_t = DEFAULT_HISTORY_RECOVERY_WORKER_CANCEL_POLL_SECONDS,
            value_parser = clap::value_parser!(u64).range(1..=MAX_HISTORY_RECOVERY_WORKER_CANCEL_POLL_SECONDS)
        )]
        cancel_poll_seconds: u64,

        /// LAN discovery duration within each worker attempt.
        #[arg(
            long,
            default_value_t = DEFAULT_DISCOVERY_WAIT_SECONDS,
            value_parser = clap::value_parser!(u64).range(1..=MAX_DISCOVERY_WAIT_SECONDS)
        )]
        discovery_wait_seconds: u64,

        /// Initial persistent retry delay before exponential backoff and jitter.
        #[arg(
            long = "retry-base-seconds",
            default_value_t = DEFAULT_RECOVERY_RETRY_BASE_SECONDS,
            value_parser = clap::value_parser!(u64).range(0..=MAX_RECOVERY_RETRY_BASE_SECONDS)
        )]
        retry_base_seconds: u64,

        /// Maximum persistent retry delay after exponential backoff and jitter.
        #[arg(
            long,
            default_value_t = DEFAULT_RECOVERY_RETRY_MAX_SECONDS,
            value_parser = clap::value_parser!(u64).range(0..=MAX_RECOVERY_RETRY_MAX_SECONDS)
        )]
        retry_max_seconds: u64,

        /// Maximum pages transferred over any one authenticated connection.
        #[arg(long, default_value_t = MAX_HISTORY_RECOVERY_PAGES_PER_SESSION)]
        max_pages: usize,
    },

    /// Irreversibly cancel one previously approved local recovery plan.
    HistoryRecoveryPlanCancel {
        /// Directory containing the exact recipient device and scheduler state.
        #[arg(long)]
        state_dir: PathBuf,

        /// Recipient-signed plan whose local execution consent is being cancelled.
        #[arg(long)]
        plan_file: PathBuf,

        /// Local conversation label whose derived ID must match the approved plan.
        #[arg(long)]
        conversation: String,
    },

    /// Probe the native platform network, metering, roaming, and power context.
    PlatformContext,

    /// Reconcile source-signed completeness claims from locally stored rewrap bundles.
    HistoryRewrapReconcile {
        /// Directory containing imported history-rewrap bundles.
        #[arg(long)]
        state_dir: PathBuf,

        /// Development-only shared label used to derive a conversation ID.
        #[arg(long, default_value = "m0-local-smoke")]
        conversation: String,
    },

    /// Verify and print locally stored events without connecting to a peer.
    History {
        /// Directory containing this application's development state.
        #[arg(long)]
        state_dir: PathBuf,

        /// Development-only shared label used to derive a conversation ID.
        #[arg(long, default_value = "m0-local-smoke")]
        conversation: String,
    },

    /// Create or load a development device identity and print its public ID.
    Identity {
        /// Directory containing this application's development state.
        #[arg(long)]
        state_dir: PathBuf,
    },

    /// Create a development Account Root authority in a separate directory.
    AccountCreate {
        /// Directory reserved for the offline Account Root secret and authority sequence.
        #[arg(long)]
        account_dir: PathBuf,
    },

    /// Print the public Account ID for an existing Account Root authority.
    AccountShow {
        /// Directory containing an existing development Account Root secret.
        #[arg(long)]
        account_dir: PathBuf,
    },

    /// Export the current complete root-signed authority snapshot.
    AccountSnapshot {
        /// Directory containing an existing development Account Root authority.
        #[arg(long)]
        account_dir: PathBuf,

        /// Write the signed snapshot to this new file.
        #[arg(long)]
        snapshot_file: PathBuf,
    },

    /// Publish a complete root-signed list of authorized account devices.
    AccountDeviceList {
        /// Directory containing an existing development Account Root authority.
        #[arg(long)]
        account_dir: PathBuf,

        /// Public device certificate to include. Repeat for every active device.
        #[arg(long = "device-certificate-file", required = true)]
        device_certificate_files: Vec<PathBuf>,

        /// Write the complete signed list to this new file.
        #[arg(long)]
        device_list_file: PathBuf,
    },

    /// Create an owner-signed, add-only conversation membership snapshot.
    ConversationCreate {
        /// Directory containing the owner Account Root authority.
        #[arg(long)]
        account_dir: PathBuf,

        /// Development-only shared label used to derive a conversation ID.
        #[arg(long)]
        conversation: String,

        /// Account to include. Repeat this option for every initial member.
        #[arg(long = "member-account")]
        member_accounts: Vec<AccountId>,

        /// Write the signed membership snapshot to this new file.
        #[arg(long)]
        membership_file: PathBuf,
    },

    /// Add accounts to an existing owner-signed conversation membership.
    ConversationMemberAdd {
        /// Directory containing the owner Account Root authority.
        #[arg(long)]
        account_dir: PathBuf,

        /// Development-only shared label used to derive a conversation ID.
        #[arg(long)]
        conversation: String,

        /// Account to add. Repeat this option to add multiple accounts atomically.
        #[arg(long = "member-account", required = true)]
        member_accounts: Vec<AccountId>,

        /// Write the updated signed membership snapshot to this new file.
        #[arg(long)]
        membership_file: PathBuf,
    },

    /// Install or update a trusted conversation membership on one device.
    ConversationMembershipInstall {
        /// Directory containing this application's development device state.
        #[arg(long)]
        state_dir: PathBuf,

        /// Owner-signed membership snapshot to install.
        #[arg(long)]
        membership_file: PathBuf,
    },

    /// Root-sign and install a messaging certificate for one device state.
    DeviceEnroll {
        /// Directory containing an existing development Account Root authority.
        #[arg(long)]
        account_dir: PathBuf,

        /// Directory containing the device identity to authorize.
        #[arg(long)]
        state_dir: PathBuf,

        /// Also export the public device certificate to a new file.
        #[arg(long)]
        certificate_file: Option<PathBuf>,
    },

    /// Install a newer signed authority snapshot for this device's own account.
    DeviceAuthorityUpdate {
        /// Directory containing the device identity and installed certificate.
        #[arg(long)]
        state_dir: PathBuf,

        /// Root-signed complete authority snapshot to install.
        #[arg(long)]
        snapshot_file: PathBuf,
    },

    /// Verify an installed device certificate against its pinned authority snapshot.
    DeviceAuthorize {
        /// Directory containing the device identity and installed certificate.
        #[arg(long)]
        state_dir: PathBuf,

        /// Trusted public Account ID expected to have signed the certificate.
        #[arg(long)]
        account_id: AccountId,
    },

    /// Permanently revoke one device key with the Account Root authority.
    DeviceRevoke {
        /// Directory containing an existing development Account Root authority.
        #[arg(long)]
        account_dir: PathBuf,

        /// Public device key to revoke. Re-enrollment requires a new device key.
        #[arg(long)]
        device_id: DeviceId,

        /// Write the public root-signed revocation to this new file.
        #[arg(long)]
        revocation_file: PathBuf,
    },

    /// Atomically copy the current legacy device state into an encrypted transactional vault.
    StateVaultMigrate {
        /// Directory containing the existing development device state.
        #[arg(long)]
        state_dir: PathBuf,
    },

    /// Authenticate every encrypted vault record and compare it with the retained legacy state.
    StateVaultVerify {
        /// Directory containing the migrated device state and encrypted vault.
        #[arg(long)]
        state_dir: PathBuf,
    },

    /// Compare each typed encrypted repository view with its retained legacy view.
    StateVaultShadowRead {
        /// Directory containing the migrated device state and encrypted vault.
        #[arg(long)]
        state_dir: PathBuf,
    },

    /// Recover a vault mirror only when a prior authenticated dual-write intent exists.
    StateVaultRecover {
        /// Directory containing an interrupted encrypted vault mirror.
        #[arg(long)]
        state_dir: PathBuf,
    },

    /// Export the vault master key as a portable passphrase-encrypted recovery package.
    StateVaultKeyExport {
        /// Directory containing the encrypted vault and local protected key envelope.
        #[arg(long)]
        state_dir: PathBuf,

        /// New external file that will receive the portable recovery package.
        #[arg(long)]
        output_file: PathBuf,

        /// File containing the recovery passphrase; one trailing CRLF or LF is removed.
        #[arg(long)]
        passphrase_file: PathBuf,
    },

    /// Install a local protected key envelope from a portable recovery package.
    StateVaultKeyImport {
        /// Directory containing the encrypted vault database to authenticate.
        #[arg(long)]
        state_dir: PathBuf,

        /// External portable recovery package created by state-vault-key-export.
        #[arg(long)]
        recovery_file: PathBuf,

        /// File containing the recovery passphrase; one trailing CRLF or LF is removed.
        #[arg(long)]
        passphrase_file: PathBuf,
    },

    /// Restore an authenticated vault snapshot into a new, previously absent directory.
    StateVaultRestore {
        /// Directory containing the encrypted vault and its development key file.
        #[arg(long)]
        state_dir: PathBuf,

        /// New directory that will receive the restored legacy state snapshot.
        #[arg(long)]
        output_state_dir: PathBuf,
    },
}

impl Command {
    fn state_directory(&self) -> Option<&Path> {
        match self {
            Self::Listen { state_dir, .. }
            | Self::Runtime { state_dir, .. }
            | Self::RuntimeContactAdd { state_dir, .. }
            | Self::RuntimeQueueMessage { state_dir, .. }
            | Self::RuntimeOutboxStatus { state_dir, .. }
            | Self::Connect { state_dir, .. }
            | Self::Sync { state_dir, .. }
            | Self::SeedHistory { state_dir, .. }
            | Self::RatchetBundle { state_dir, .. }
            | Self::RatchetPrekeyPool { state_dir, .. }
            | Self::HistoryRewrapExport { state_dir, .. }
            | Self::HistoryRewrapImport { state_dir, .. }
            | Self::HistoryRewrapFetch { state_dir, .. }
            | Self::HistoryRecoveryResume { state_dir, .. }
            | Self::HistoryRecoveryLinkAccept { state_dir, .. }
            | Self::HistoryRecoveryLinkDiscover { state_dir, .. }
            | Self::HistoryRecoveryPlanApprove { state_dir, .. }
            | Self::HistoryRecoveryPlanRun { state_dir, .. }
            | Self::HistoryRecoveryPlanWatch { state_dir, .. }
            | Self::HistoryRecoveryPlanCancel { state_dir, .. }
            | Self::HistoryRewrapReconcile { state_dir, .. }
            | Self::History { state_dir, .. }
            | Self::Identity { state_dir }
            | Self::ConversationMembershipInstall { state_dir, .. }
            | Self::DeviceEnroll { state_dir, .. }
            | Self::DeviceAuthorityUpdate { state_dir, .. }
            | Self::DeviceAuthorize { state_dir, .. }
            | Self::StateVaultMigrate { state_dir }
            | Self::StateVaultVerify { state_dir }
            | Self::StateVaultShadowRead { state_dir }
            | Self::StateVaultRecover { state_dir }
            | Self::StateVaultKeyExport { state_dir, .. }
            | Self::StateVaultKeyImport { state_dir, .. }
            | Self::StateVaultRestore { state_dir, .. } => Some(state_dir),
            Self::AccountCreate { .. }
            | Self::RuntimeProfileCreate { .. }
            | Self::RuntimeFromProfile { .. }
            | Self::HistoryRewrapSas { .. }
            | Self::HistoryRecoveryLinkInspect { .. }
            | Self::HistoryRecoveryLinkQrRender { .. }
            | Self::AccountShow { .. }
            | Self::AccountSnapshot { .. }
            | Self::AccountDeviceList { .. }
            | Self::ConversationCreate { .. }
            | Self::ConversationMemberAdd { .. }
            | Self::DeviceRevoke { .. }
            | Self::RuntimeIpcPing { .. }
            | Self::RuntimeIpcQueueMessage { .. }
            | Self::RuntimeIpcOutboxStatus { .. }
            | Self::RuntimeIpcApplyDeviceDirectory { .. }
            | Self::RuntimeIpcDeviceDirectoryStatus { .. }
            | Self::RuntimeIpcPublishTicket { .. }
            | Self::RuntimeIpcRefreshContactTicket { .. }
            | Self::RuntimeIpcConfigureTicketAutomation { .. }
            | Self::RuntimeIpcTicketAutomationStatus { .. }
            | Self::RuntimeIpcExportEndpointAnnouncements { .. }
            | Self::RuntimeIpcImportEndpointAnnouncements { .. }
            | Self::RuntimeIpcPushEndpointAnnouncements { .. }
            | Self::PlatformContext => None,
        }
    }

    fn uses_state_vault_dual_write(&self) -> bool {
        self.state_directory().is_some()
            && !matches!(
                self,
                Self::StateVaultMigrate { .. }
                    | Self::StateVaultVerify { .. }
                    | Self::StateVaultShadowRead { .. }
                    | Self::StateVaultRecover { .. }
                    | Self::StateVaultKeyExport { .. }
                    | Self::StateVaultKeyImport { .. }
                    | Self::StateVaultRestore { .. }
                    | Self::Runtime { .. }
                    | Self::HistoryRecoveryPlanRun { .. }
                    | Self::HistoryRecoveryPlanWatch { .. }
            )
    }

    fn uses_outer_state_lock(&self) -> bool {
        self.state_directory().is_some()
            && !matches!(
                self,
                Self::Runtime { .. }
                    | Self::HistoryRecoveryPlanRun { .. }
                    | Self::HistoryRecoveryPlanWatch { .. }
            )
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum RoutePolicyArg {
    Auto,
    DirectOnly,
    RelayOnly,
}

struct ListenOptions {
    state_dir: PathBuf,
    allowed_requester_account_id: AccountId,
    device_list_file: PathBuf,
    peer_prekey_pool_files: Vec<PathBuf>,
    ticket_file: Option<PathBuf>,
    relay_wait_seconds: u64,
    route_policy: RoutePolicy,
    relay_url: Option<RelayUrl>,
    history_rewrap_conversation: Option<String>,
    history_rewrap_recipient_device: Option<DeviceId>,
    history_rewrap_approve_sas: Option<String>,
    history_rewrap_range_start: usize,
    history_rewrap_count: usize,
    history_recovery_link_file: Option<PathBuf>,
    history_recovery_qr_file: Option<PathBuf>,
    history_recovery_link_page_size: usize,
    history_recovery_link_valid_for_seconds: u64,
    history_recovery_discovery_publish: bool,
}

struct RuntimeOptions {
    state_dir: PathBuf,
    allowed_requester_account_id: AccountId,
    device_list_file: PathBuf,
    peer_prekey_pool_files: Vec<PathBuf>,
    ticket_file: Option<PathBuf>,
    relay_wait_seconds: u64,
    route_policy: RoutePolicy,
    relay_url: Option<RelayUrl>,
    max_sessions: usize,
    idle_seconds: u64,
    poll_milliseconds: u64,
    retry_base_seconds: u64,
    retry_max_seconds: u64,
    auto_sync_seconds: u64,
    max_outbound_actions: usize,
    ipc_file: Option<PathBuf>,
}

fn create_runtime_launch_profile(
    profile_file: PathBuf,
    mut settings: RuntimeLaunchSettings,
) -> Result<()> {
    settings.state_dir = fs::canonicalize(&settings.state_dir).with_context(|| {
        format!(
            "resolve runtime profile state directory {}",
            settings.state_dir.display()
        )
    })?;
    settings.device_list_file =
        fs::canonicalize(&settings.device_list_file).with_context(|| {
            format!(
                "resolve runtime profile device list {}",
                settings.device_list_file.display()
            )
        })?;
    settings.peer_prekey_pool_files = settings
        .peer_prekey_pool_files
        .iter()
        .map(|path| {
            fs::canonicalize(path)
                .with_context(|| format!("resolve runtime profile prekey pool {}", path.display()))
        })
        .collect::<Result<_>>()?;
    let canonical_state = settings.state_dir.clone();
    settings.ticket_file = settings
        .ticket_file
        .as_deref()
        .map(|path| resolve_runtime_profile_output(path, &canonical_state))
        .transpose()?;
    settings.ipc_file = resolve_runtime_profile_output(&settings.ipc_file, &canonical_state)?;
    let profile = RuntimeLaunchProfile::new(settings)?;
    let options = runtime_options_from_launch_settings(profile.settings())?;
    validate_runtime_options(&options)?;
    profile.write_new(&profile_file)?;
    println!("runtime_profile_file={}", profile_file.display());
    println!("runtime_ipc_file={}", profile.settings().ipc_file.display());
    println!("runtime_profile_contains_secrets=false");
    println!("status=runtime-profile-ready");
    Ok(())
}

fn runtime_options_from_profile(profile_file: &Path) -> Result<RuntimeOptions> {
    let profile = RuntimeLaunchProfile::load(profile_file)?;
    let options = runtime_options_from_launch_settings(profile.settings())?;
    validate_runtime_options(&options)?;
    Ok(options)
}

fn runtime_options_from_launch_settings(
    settings: &RuntimeLaunchSettings,
) -> Result<RuntimeOptions> {
    Ok(RuntimeOptions {
        state_dir: settings.state_dir.clone(),
        allowed_requester_account_id: settings.allowed_requester_account_id,
        device_list_file: settings.device_list_file.clone(),
        peer_prekey_pool_files: settings.peer_prekey_pool_files.clone(),
        ticket_file: settings.ticket_file.clone(),
        relay_wait_seconds: settings.relay_wait_seconds,
        route_policy: match settings.route_policy {
            RuntimeIpcRoutePolicy::Auto => RoutePolicy::Auto,
            RuntimeIpcRoutePolicy::DirectOnly => RoutePolicy::DirectOnly,
            RuntimeIpcRoutePolicy::RelayOnly => RoutePolicy::RelayOnly,
        },
        relay_url: settings
            .relay_url
            .as_deref()
            .map(str::parse)
            .transpose()
            .context("parse runtime launch profile relay URL")?,
        max_sessions: 0,
        idle_seconds: 0,
        poll_milliseconds: settings.poll_milliseconds,
        retry_base_seconds: settings.retry_base_seconds,
        retry_max_seconds: settings.retry_max_seconds,
        auto_sync_seconds: settings.auto_sync_seconds,
        max_outbound_actions: 0,
        ipc_file: Some(settings.ipc_file.clone()),
    })
}

fn resolve_runtime_profile_output(path: &Path, canonical_state: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .context("read current directory for runtime profile output")?
            .join(path)
    };
    ensure!(
        !absolute.starts_with(canonical_state),
        "runtime profile output must live outside the protected state directory"
    );
    let file_name = absolute
        .file_name()
        .context("runtime profile output path has no file name")?;
    let parent = absolute
        .parent()
        .context("runtime profile output path has no parent")?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "create runtime profile output directory {}",
            parent.display()
        )
    })?;
    let resolved = fs::canonicalize(parent)
        .with_context(|| format!("resolve runtime profile output parent {}", parent.display()))?
        .join(file_name);
    ensure!(
        !resolved.starts_with(canonical_state),
        "runtime profile output must live outside the protected state directory"
    );
    Ok(resolved)
}

#[derive(Clone, Debug)]
struct HistoryRewrapApproval {
    conversation_id: ConversationId,
    recipient_device_id: DeviceId,
    sas: HistoryRewrapSas,
    range_start: usize,
    count: usize,
}

impl HistoryRewrapApproval {
    fn contains_range(&self, range_start: usize, count: usize) -> bool {
        range_start >= self.range_start
            && range_start
                .checked_add(count)
                .zip(self.range_start.checked_add(self.count))
                .is_some_and(|(requested_end, approved_end)| requested_end <= approved_end)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HistoryRewrapServeOutcome {
    Rejected,
    Transferred {
        next_range_start: u64,
        recovery_complete: bool,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct HistoryRecoverySessionProgress {
    pages_served: usize,
    expected_range_start: Option<u64>,
    complete: bool,
}

impl HistoryRecoverySessionProgress {
    fn accepts(&self, range_start: u64) -> bool {
        self.expected_range_start
            .is_none_or(|expected| expected == range_start)
    }

    fn record_page(&mut self, next_range_start: u64, complete: bool) {
        self.pages_served += 1;
        self.expected_range_start = Some(next_range_start);
        self.complete = complete;
    }

    fn can_request_next(&self) -> bool {
        !self.complete && self.pages_served < MAX_HISTORY_RECOVERY_PAGES_PER_SESSION
    }
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

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ConnectionTicketContent {
    version: u8,
    endpoint: EndpointAddr,
    listener_certificate: DeviceCertificate,
    listener_directory: AccountPrekeyDirectory,
    allowed_requester_account_id: AccountId,
    ticket_publication_write_key: TicketPublicationWriteKey,
    route_policy: RoutePolicy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ConnectionTicket {
    content: ConnectionTicketContent,
    signature: Vec<u8>,
}

impl ConnectionTicket {
    fn new(
        endpoint: EndpointAddr,
        listener_identity: &DeviceIdentity,
        listener_certificate: DeviceCertificate,
        listener_directory: AccountPrekeyDirectory,
        allowed_requester_account_id: AccountId,
        route_policy: RoutePolicy,
    ) -> Result<Self> {
        ensure!(
            listener_certificate.device_id() == listener_identity.device_id(),
            "listener certificate belongs to a different device"
        );
        listener_directory
            .verify_at(unix_time_now()?)
            .context("verify listener account prekey directory")?;
        ensure!(
            listener_directory.account_id() == listener_certificate.account_id(),
            "listener prekey directory belongs to a different account"
        );
        ensure!(
            listener_directory.certificate_for(listener_identity.device_id())
                == Some(&listener_certificate),
            "listener certificate is not present exactly in its signed device list"
        );
        ensure!(
            listener_directory
                .pool_for(listener_identity.device_id())
                .is_some(),
            "listener prekey directory has no pool for the listening device"
        );
        let content = ConnectionTicketContent {
            version: TICKET_VERSION,
            endpoint,
            listener_certificate,
            listener_directory,
            allowed_requester_account_id,
            ticket_publication_write_key: ticket_publication_write_capability(
                listener_identity,
                allowed_requester_account_id,
            )
            .write_key(),
            route_policy,
        };
        let signature = listener_identity
            .sign(&ticket_signing_bytes(&content)?)
            .to_vec();
        Ok(Self { content, signature })
    }

    fn encode(&self) -> Result<String> {
        self.verify()
            .context("verify connection ticket before encoding")?;
        let json = serde_json::to_vec(self).context("serialize connection ticket")?;
        Ok(URL_SAFE_NO_PAD.encode(json))
    }

    fn decode(encoded: &str) -> Result<Self> {
        let ticket = Self::decode_authenticated(encoded)?;
        ticket.verify()?;
        Ok(ticket)
    }

    /// Decodes and authenticates all immutable ticket fields without treating
    /// an expired prekey pool as a usable transport credential. This is only
    /// used to migrate the self-authenticating publication-channel binding.
    fn decode_authenticated(encoded: &str) -> Result<Self> {
        let bytes = URL_SAFE_NO_PAD
            .decode(encoded.trim())
            .context("decode connection ticket as base64url")?;
        let ticket: Self =
            serde_json::from_slice(&bytes).context("decode connection ticket payload")?;
        ticket.verify_authenticated()?;
        Ok(ticket)
    }

    fn endpoint(&self) -> &EndpointAddr {
        &self.content.endpoint
    }

    fn listener_account_id(&self) -> AccountId {
        self.content.listener_certificate.account_id()
    }

    fn listener_device_id(&self) -> DeviceId {
        self.content.listener_certificate.device_id()
    }

    fn listener_certificate(&self) -> &DeviceCertificate {
        &self.content.listener_certificate
    }

    fn allowed_requester_account_id(&self) -> AccountId {
        self.content.allowed_requester_account_id
    }

    fn ticket_publication_write_key(&self) -> TicketPublicationWriteKey {
        self.content.ticket_publication_write_key
    }

    fn listener_authority_snapshot(&self) -> &AccountAuthoritySnapshot {
        self.content.listener_directory.authority_snapshot()
    }

    fn listener_directory(&self) -> &AccountPrekeyDirectory {
        &self.content.listener_directory
    }

    fn route_policy(&self) -> RoutePolicy {
        self.content.route_policy
    }

    fn verify(&self) -> Result<()> {
        self.verify_authenticated()?;
        self.content
            .listener_directory
            .verify_at(unix_time_now()?)?;
        Ok(())
    }

    fn verify_authenticated(&self) -> Result<()> {
        ensure!(
            self.content.version == TICKET_VERSION,
            "unsupported connection ticket version: {}",
            self.content.version
        );
        self.content
            .ticket_publication_write_key
            .verify()
            .context("verify ticket publication write key")?;
        self.content.listener_certificate.verify()?;
        self.content.listener_directory.verify()?;
        ensure!(
            self.content.listener_directory.account_id()
                == self.content.listener_certificate.account_id(),
            "connection ticket directory does not belong to the listener account"
        );
        ensure!(
            self.content
                .listener_directory
                .certificate_for(self.content.listener_certificate.device_id())
                == Some(&self.content.listener_certificate),
            "connection ticket listener certificate is absent from its signed device list"
        );
        self.content
            .listener_certificate
            .device_id()
            .verify(&ticket_signing_bytes(&self.content)?, &self.signature)
            .context("verify listener signature on connection ticket")
    }

    fn verify_authenticated_listener_authorization(
        &self,
        expected_account: AccountId,
    ) -> Result<AuthorizedDevice> {
        self.verify_authenticated()?;
        verify_device_authorization_with_snapshot(
            expected_account,
            &self.content.listener_certificate,
            self.content.listener_directory.authority_snapshot(),
            &DeviceCapability::MESSAGING,
        )
        .context("verify listener Account Root authorization")
    }

    fn verify_authenticated_listener_account(&self, expected_account: AccountId) -> Result<()> {
        self.verify_authenticated()?;
        self.content
            .listener_directory
            .authority_snapshot()
            .verify_for_account(expected_account)
            .context("verify expected listener Account ID")
    }

    fn verify_listener_authorization(
        &self,
        expected_account: AccountId,
    ) -> Result<AuthorizedDevice> {
        self.verify()?;
        verify_device_authorization_with_snapshot(
            expected_account,
            &self.content.listener_certificate,
            self.content.listener_directory.authority_snapshot(),
            &DeviceCapability::MESSAGING,
        )
        .context("verify listener Account Root authorization")
    }

    fn verify_listener_account(&self, expected_account: AccountId) -> Result<()> {
        self.verify()?;
        self.content
            .listener_directory
            .authority_snapshot()
            .verify_for_account(expected_account)
            .context("verify expected listener Account ID")
    }
}

fn ticket_publication_write_capability(
    identity: &DeviceIdentity,
    peer_account_id: AccountId,
) -> TicketPublicationWriteCapability {
    TicketPublicationWriteCapability::derive(identity.secret_bytes(), peer_account_id.as_bytes())
}

#[derive(Clone, Debug)]
struct HistoryRecoveryBootstrap {
    endpoint: EndpointAddr,
    account_device_list: AccountDeviceListSnapshot,
    account_id: AccountId,
    source_device_id: DeviceId,
    route_policy: RoutePolicy,
}

impl HistoryRecoveryBootstrap {
    fn from_ticket(ticket: &ConnectionTicket, expected_account_id: AccountId) -> Result<Self> {
        ticket.verify_listener_account(expected_account_id)?;
        ensure!(
            ticket.allowed_requester_account_id() == expected_account_id,
            "source ticket does not authorize this same account"
        );
        let source = ticket.verify_listener_authorization(expected_account_id)?;
        Ok(Self {
            endpoint: ticket.endpoint().clone(),
            account_device_list: ticket.listener_directory().device_list().clone(),
            account_id: expected_account_id,
            source_device_id: source.device_id(),
            route_policy: ticket.route_policy(),
        })
    }

    fn from_link(link: &SignedHistoryRecoveryLink) -> Self {
        Self {
            endpoint: link.endpoint().clone(),
            account_device_list: link.account_device_list().clone(),
            account_id: link.account_id(),
            source_device_id: link.source_device_id(),
            route_policy: link.route_policy(),
        }
    }

    fn authority_snapshot(&self) -> &AccountAuthoritySnapshot {
        self.account_device_list.authority_snapshot()
    }
}

fn ticket_signing_bytes(content: &ConnectionTicketContent) -> Result<Vec<u8>> {
    let encoded = serde_json::to_vec(content).context("serialize connection ticket content")?;
    let mut bytes = Vec::with_capacity(TICKET_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(TICKET_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn main() -> Result<()> {
    match std::thread::Builder::new()
        .name("kilogram-cli-coordinator".to_owned())
        .stack_size(CLI_COORDINATOR_STACK_BYTES)
        .spawn(async_main)
        .context("start Kilogram CLI coordinator thread")?
        .join()
    {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

#[tokio::main]
async fn async_main() -> Result<()> {
    let cli = Cli::parse();
    let command = cli.command;
    let state_directory = command.state_directory().map(Path::to_path_buf);
    let uses_state_vault_dual_write = command.uses_state_vault_dual_write();
    let _state_lock = command
        .uses_outer_state_lock()
        .then(|| command.state_directory())
        .flatten()
        .map(StateDirectoryLock::acquire)
        .transpose()
        .context("lock state directory and recover interrupted local transaction")?;
    let vault_guard = if uses_state_vault_dual_write {
        state_directory
            .as_deref()
            .map(VaultDualWriteGuard::prepare)
            .transpose()?
            .flatten()
    } else {
        None
    };
    let command_result = Box::pin(run_command(command)).await;
    let mirror_result = match vault_guard {
        Some(guard) => guard.finish(),
        None => Ok(()),
    };
    combine_operation_and_mirror(command_result, mirror_result)
}

fn combine_operation_and_mirror<T>(
    operation_result: Result<T>,
    mirror_result: Result<()>,
) -> Result<T> {
    match (operation_result, mirror_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(command_error), Ok(())) => Err(command_error),
        (Ok(_), Err(mirror_error)) => Err(mirror_error),
        (Err(command_error), Err(mirror_error)) => Err(command_error.context(format!(
            "command failed and state vault dual-write completion also failed: {mirror_error:#}"
        ))),
    }
}

struct VaultDualWriteGuard {
    state_directory: PathBuf,
}

impl VaultDualWriteGuard {
    fn prepare(state_directory: &Path) -> Result<Option<Self>> {
        if !EncryptedStateVault::is_initialized(state_directory)
            .context("inspect encrypted state vault before live command")?
        {
            return Ok(None);
        }
        let vault = EncryptedStateVault::open_existing(state_directory)
            .context("open encrypted state vault before live command")?;
        print_vault_key_status(&vault);
        if let Some(report) = vault
            .recover_primary_shadow()
            .context("recover retained legacy shadow from a committed vault-primary checkpoint")?
        {
            println!("vault_primary_shadow_recovery=restored");
            print_vault_report(&report);
        }
        if let Some(commit) = vault
            .recover_pending_dual_write()
            .context("recover interrupted state vault dual-write")?
        {
            println!(
                "vault_dual_write_recovery={}",
                vault_mirror_outcome_name(commit.outcome())
            );
            print_vault_mirror_commit(&commit);
        }
        let (identity_shadow, report) = vault
            .retire_device_identity_shadow()
            .context("retire plaintext device identity compatibility shadow")?;
        println!(
            "vault_device_identity_shadow={}",
            vault_identity_shadow_outcome_name(identity_shadow)
        );
        if identity_shadow == VaultIdentityShadowOutcome::Retired {
            print_vault_report(&report);
        }
        let base = vault
            .begin_dual_write()
            .context("prepare authenticated state vault dual-write intent")?;
        println!("vault_dual_write_intent=prepared");
        println!(
            "vault_dual_write_base_generation={}",
            base.mirror_generation()
        );
        Ok(Some(Self {
            state_directory: state_directory.to_path_buf(),
        }))
    }

    fn finish(self) -> Result<()> {
        let vault = EncryptedStateVault::open_existing(&self.state_directory)
            .context("reopen encrypted state vault after live command")?;
        if let Some(report) = vault
            .recover_primary_shadow()
            .context("recover retained legacy shadow before final vault mirror")?
        {
            println!("vault_primary_shadow_recovery=restored");
            print_vault_report(&report);
        }
        let commit = vault
            .finish_dual_write()
            .context("commit live legacy state to encrypted state vault")?;
        println!(
            "vault_dual_write={}",
            vault_mirror_outcome_name(commit.outcome())
        );
        print_vault_mirror_commit(&commit);
        Ok(())
    }
}

fn with_locked_state<T>(
    state_directory: &Path,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let _state_lock = StateDirectoryLock::acquire(state_directory)
        .context("lock state directory for bounded scheduler operation")?;
    let vault_guard = VaultDualWriteGuard::prepare(state_directory)?;
    let operation_result = operation();
    let mirror_result = match vault_guard {
        Some(guard) => guard.finish(),
        None => Ok(()),
    };
    combine_operation_and_mirror(operation_result, mirror_result)
}

fn vault_identity_shadow_outcome_name(outcome: VaultIdentityShadowOutcome) -> &'static str {
    match outcome {
        VaultIdentityShadowOutcome::Retired => "retired",
        VaultIdentityShadowOutcome::AlreadyRetired => "already-retired",
    }
}

fn vault_mirror_outcome_name(outcome: VaultMirrorOutcome) -> &'static str {
    match outcome {
        VaultMirrorOutcome::Mirrored => "mirrored",
        VaultMirrorOutcome::AlreadyCurrent => "already-current",
    }
}

async fn run_command(command: Command) -> Result<()> {
    match command {
        Command::Listen {
            state_dir,
            allow_account,
            device_list_file,
            peer_prekey_pool_files,
            ticket_file,
            relay_wait_seconds,
            route_policy,
            relay_url,
            history_rewrap_conversation,
            history_rewrap_recipient_device,
            history_rewrap_approve_sas,
            history_rewrap_range_start,
            history_rewrap_count,
            history_recovery_link_file,
            history_recovery_qr_file,
            history_recovery_link_page_size,
            history_recovery_link_valid_for_seconds,
            history_recovery_discovery_publish,
        } => {
            Box::pin(listen(ListenOptions {
                state_dir,
                allowed_requester_account_id: allow_account,
                device_list_file,
                peer_prekey_pool_files,
                ticket_file,
                relay_wait_seconds,
                route_policy: route_policy.into(),
                relay_url,
                history_rewrap_conversation,
                history_rewrap_recipient_device,
                history_rewrap_approve_sas,
                history_rewrap_range_start,
                history_rewrap_count,
                history_recovery_link_file,
                history_recovery_qr_file,
                history_recovery_link_page_size,
                history_recovery_link_valid_for_seconds,
                history_recovery_discovery_publish,
            }))
            .await
        }
        Command::Runtime {
            state_dir,
            allow_account,
            device_list_file,
            peer_prekey_pool_files,
            ticket_file,
            relay_wait_seconds,
            route_policy,
            relay_url,
            max_sessions,
            idle_seconds,
            poll_milliseconds,
            retry_base_seconds,
            retry_max_seconds,
            auto_sync_seconds,
            max_outbound_actions,
            ipc_file,
        } => {
            runtime(RuntimeOptions {
                state_dir,
                allowed_requester_account_id: allow_account,
                device_list_file,
                peer_prekey_pool_files,
                ticket_file,
                relay_wait_seconds,
                route_policy: route_policy.into(),
                relay_url,
                max_sessions,
                idle_seconds,
                poll_milliseconds,
                retry_base_seconds,
                retry_max_seconds,
                auto_sync_seconds,
                max_outbound_actions,
                ipc_file,
            })
            .await
        }
        Command::RuntimeProfileCreate {
            profile_file,
            state_dir,
            allow_account,
            device_list_file,
            peer_prekey_pool_files,
            ticket_file,
            relay_wait_seconds,
            route_policy,
            relay_url,
            poll_milliseconds,
            retry_base_seconds,
            retry_max_seconds,
            auto_sync_seconds,
            ipc_file,
        } => create_runtime_launch_profile(
            profile_file,
            RuntimeLaunchSettings {
                state_dir,
                allowed_requester_account_id: allow_account,
                device_list_file,
                peer_prekey_pool_files,
                ticket_file,
                relay_wait_seconds,
                route_policy: runtime_ipc_route_policy(route_policy.into()),
                relay_url: relay_url.map(|url| url.to_string()),
                poll_milliseconds,
                retry_base_seconds,
                retry_max_seconds,
                auto_sync_seconds,
                ipc_file,
            },
        ),
        Command::RuntimeFromProfile { profile_file } => {
            runtime(runtime_options_from_profile(&profile_file)?).await
        }
        Command::RuntimeContactAdd {
            state_dir,
            conversation,
            expect_account,
            descriptor_file,
        } => add_runtime_contact(state_dir, conversation, expect_account, descriptor_file),
        Command::RuntimeQueueMessage {
            state_dir,
            conversation,
            peer_account,
            message,
        } => queue_runtime_message(state_dir, conversation, peer_account, message),
        Command::RuntimeOutboxStatus { state_dir } => runtime_outbox_status(state_dir),
        Command::RuntimeIpcPing { ipc_file } => runtime_ipc_ping(ipc_file).await,
        Command::RuntimeIpcQueueMessage {
            ipc_file,
            conversation,
            peer_account,
            message,
            request_id,
        } => {
            runtime_ipc_queue_message(ipc_file, conversation, peer_account, message, request_id)
                .await
        }
        Command::RuntimeIpcOutboxStatus { ipc_file } => runtime_ipc_outbox_status(ipc_file).await,
        Command::RuntimeIpcApplyDeviceDirectory {
            ipc_file,
            device_list_file,
        } => runtime_ipc_apply_device_directory(ipc_file, device_list_file).await,
        Command::RuntimeIpcDeviceDirectoryStatus { ipc_file } => {
            runtime_ipc_device_directory_status(ipc_file).await
        }
        Command::RuntimeIpcPublishTicket {
            ipc_file,
            conversation,
            peer_account,
            service_base_url,
            ttl_seconds,
        } => {
            runtime_ipc_publish_ticket(
                ipc_file,
                conversation,
                peer_account,
                service_base_url,
                ttl_seconds,
            )
            .await
        }
        Command::RuntimeIpcRefreshContactTicket {
            ipc_file,
            conversation,
            peer_account,
            service_base_url,
        } => {
            runtime_ipc_refresh_contact_ticket(
                ipc_file,
                conversation,
                peer_account,
                service_base_url,
            )
            .await
        }
        Command::RuntimeIpcConfigureTicketAutomation {
            ipc_file,
            conversation,
            peer_account,
            enabled,
            service_base_url,
            ttl_seconds,
            refresh_before_seconds,
            retry_base_seconds,
            retry_max_seconds,
            deny_ethernet,
            deny_wifi,
            allow_mobile,
            allow_unknown_network,
        } => {
            runtime_ipc_configure_ticket_automation(
                ipc_file,
                conversation,
                peer_account,
                enabled,
                service_base_url,
                ttl_seconds,
                refresh_before_seconds,
                retry_base_seconds,
                retry_max_seconds,
                !deny_ethernet,
                !deny_wifi,
                allow_mobile,
                allow_unknown_network,
            )
            .await
        }
        Command::RuntimeIpcTicketAutomationStatus { ipc_file } => {
            runtime_ipc_ticket_automation_status(ipc_file).await
        }
        Command::RuntimeIpcExportEndpointAnnouncements {
            ipc_file,
            recipient_device,
            output_file,
            validity_seconds,
        } => {
            runtime_ipc_export_endpoint_announcements(
                ipc_file,
                recipient_device,
                output_file,
                validity_seconds,
            )
            .await
        }
        Command::RuntimeIpcImportEndpointAnnouncements {
            ipc_file,
            bundle_file,
            descriptor_directory,
        } => {
            runtime_ipc_import_endpoint_announcements(ipc_file, bundle_file, descriptor_directory)
                .await
        }
        Command::RuntimeIpcPushEndpointAnnouncements {
            ipc_file,
            recipient_ticket_file,
            validity_seconds,
        } => {
            runtime_ipc_push_endpoint_announcements(
                ipc_file,
                recipient_ticket_file,
                validity_seconds,
            )
            .await
        }
        Command::Connect {
            state_dir,
            ticket,
            ticket_file,
            message,
            conversation,
            expect_account,
        } => {
            connect(
                state_dir,
                ticket,
                ticket_file,
                message,
                conversation,
                expect_account,
            )
            .await
        }
        Command::Sync {
            state_dir,
            ticket,
            ticket_file,
            conversation,
            max_rounds,
            expect_account,
        } => {
            sync(
                state_dir,
                ticket,
                ticket_file,
                conversation,
                max_rounds,
                expect_account,
            )
            .await
        }
        Command::SeedHistory {
            state_dir,
            conversation,
            count,
            message_prefix,
            peer_certificate_file,
            peer_device_list_file,
            peer_prekey_pool_file,
        } => seed_history(
            state_dir,
            conversation,
            count,
            message_prefix,
            peer_certificate_file,
            peer_device_list_file,
            peer_prekey_pool_file,
        ),
        Command::RatchetBundle {
            state_dir,
            bundle_file,
        } => export_ratchet_bundle(state_dir, bundle_file),
        Command::RatchetPrekeyPool {
            state_dir,
            pool_file,
            count,
            valid_for_hours,
            refresh,
        } => export_ratchet_prekey_pool(state_dir, pool_file, count, valid_for_hours, refresh),
        Command::HistoryRewrapExport {
            state_dir,
            conversation,
            device_list_file,
            recipient_device,
            range_start,
            count,
            bundle_file,
        } => export_history_rewrap(
            state_dir,
            conversation,
            device_list_file,
            recipient_device,
            range_start,
            count,
            bundle_file,
        ),
        Command::HistoryRewrapImport {
            state_dir,
            conversation,
            bundle_file,
        } => import_history_rewrap(state_dir, conversation, bundle_file),
        Command::HistoryRewrapSas {
            device_list_file,
            source_device,
            recipient_device,
        } => show_history_rewrap_sas(device_list_file, source_device, recipient_device),
        Command::HistoryRewrapFetch {
            state_dir,
            ticket,
            ticket_file,
            conversation,
            range_start,
            count,
            confirm_sas,
            expect_account,
        } => {
            fetch_history_rewrap(
                state_dir,
                ticket,
                ticket_file,
                conversation,
                range_start,
                count,
                confirm_sas,
                expect_account,
                None,
                None,
                "history-rewrap-fetched",
            )
            .await
        }
        Command::HistoryRecoveryResume {
            state_dir,
            ticket,
            ticket_file,
            conversation,
            source_device,
            range_start,
            count,
            page_size,
            max_pages,
            confirm_sas,
            expect_account,
        } => {
            Box::pin(resume_history_recovery(
                state_dir,
                ticket,
                ticket_file,
                conversation,
                source_device,
                range_start,
                count,
                page_size,
                max_pages,
                confirm_sas,
                expect_account,
            ))
            .await
        }
        Command::HistoryRecoveryLinkInspect {
            link,
            link_file,
            qr_file,
        } => inspect_history_recovery_link(link, link_file, qr_file).await,
        Command::HistoryRecoveryLinkQrRender {
            link,
            link_file,
            qr_file,
        } => render_history_recovery_link_qr(link, link_file, qr_file).await,
        Command::HistoryRecoveryLinkAccept {
            state_dir,
            link,
            link_file,
            qr_file,
            conversation,
            confirm_sas,
            max_pages,
        } => {
            Box::pin(accept_history_recovery_link(
                state_dir,
                link,
                link_file,
                qr_file,
                conversation,
                confirm_sas,
                max_pages,
            ))
            .await
        }
        Command::HistoryRecoveryLinkDiscover {
            state_dir,
            conversation,
            expect_source,
            wait_seconds,
            max_candidates,
            output_link_file,
        } => {
            discover_history_recovery_links(
                state_dir,
                conversation,
                expect_source,
                wait_seconds,
                max_candidates,
                output_link_file,
            )
            .await
        }
        Command::HistoryRecoveryPlanApprove {
            state_dir,
            link,
            link_file,
            qr_file,
            conversation,
            confirm_sas,
            plan_file,
            deny_ethernet,
            deny_wifi,
            allow_mobile,
            allow_unknown_network,
            require_external_power,
            valid_for_hours,
        } => {
            approve_history_recovery_plan(
                state_dir,
                link,
                link_file,
                qr_file,
                conversation,
                confirm_sas,
                plan_file,
                deny_ethernet,
                deny_wifi,
                allow_mobile,
                allow_unknown_network,
                require_external_power,
                valid_for_hours,
            )
            .await
        }
        Command::HistoryRecoveryPlanRun {
            state_dir,
            plan_file,
            conversation,
            network_class,
            power_source,
            max_attempts,
            discovery_wait_seconds,
            retry_base_seconds,
            retry_max_seconds,
            max_pages,
        } => {
            Box::pin(run_history_recovery_plan(
                state_dir,
                plan_file,
                conversation,
                network_class,
                power_source,
                max_attempts,
                discovery_wait_seconds,
                retry_base_seconds,
                retry_max_seconds,
                max_pages,
            ))
            .await
        }
        Command::HistoryRecoveryPlanWatch {
            state_dir,
            plan_file,
            conversation,
            max_runtime_seconds,
            max_wakeups,
            cancel_poll_seconds,
            discovery_wait_seconds,
            retry_base_seconds,
            retry_max_seconds,
            max_pages,
        } => {
            Box::pin(watch_history_recovery_plan(HistoryRecoveryWorkerOptions {
                state_dir,
                plan_file,
                conversation,
                max_runtime_seconds,
                max_wakeups,
                cancel_poll_seconds,
                discovery_wait_seconds,
                retry_base_seconds,
                retry_max_seconds,
                max_pages,
            }))
            .await
        }
        Command::HistoryRecoveryPlanCancel {
            state_dir,
            plan_file,
            conversation,
        } => cancel_history_recovery_plan(state_dir, plan_file, conversation).await,
        Command::PlatformContext => {
            let context = system_recovery_platform_context();
            print_recovery_platform_context(&context);
            println!("status=platform-context-probed");
            Ok(())
        }
        Command::HistoryRewrapReconcile {
            state_dir,
            conversation,
        } => reconcile_history_rewrap(state_dir, conversation),
        Command::History {
            state_dir,
            conversation,
        } => show_history(state_dir, conversation),
        Command::Identity { state_dir } => show_identity(state_dir),
        Command::AccountCreate { account_dir } => create_account(account_dir),
        Command::AccountShow { account_dir } => show_account(account_dir),
        Command::AccountSnapshot {
            account_dir,
            snapshot_file,
        } => export_account_snapshot(account_dir, snapshot_file),
        Command::AccountDeviceList {
            account_dir,
            device_certificate_files,
            device_list_file,
        } => publish_account_device_list(account_dir, device_certificate_files, device_list_file),
        Command::ConversationCreate {
            account_dir,
            conversation,
            member_accounts,
            membership_file,
        } => create_conversation_membership(
            account_dir,
            conversation,
            member_accounts,
            membership_file,
        ),
        Command::ConversationMemberAdd {
            account_dir,
            conversation,
            member_accounts,
            membership_file,
        } => add_conversation_members(account_dir, conversation, member_accounts, membership_file),
        Command::ConversationMembershipInstall {
            state_dir,
            membership_file,
        } => install_conversation_membership(state_dir, membership_file),
        Command::DeviceEnroll {
            account_dir,
            state_dir,
            certificate_file,
        } => enroll_device(account_dir, state_dir, certificate_file),
        Command::DeviceAuthorityUpdate {
            state_dir,
            snapshot_file,
        } => update_device_authority(state_dir, snapshot_file),
        Command::DeviceAuthorize {
            state_dir,
            account_id,
        } => authorize_device(state_dir, account_id),
        Command::DeviceRevoke {
            account_dir,
            device_id,
            revocation_file,
        } => revoke_device(account_dir, device_id, revocation_file),
        Command::StateVaultMigrate { state_dir } => migrate_state_vault(state_dir),
        Command::StateVaultVerify { state_dir } => verify_state_vault(state_dir),
        Command::StateVaultShadowRead { state_dir } => shadow_read_state_vault(state_dir),
        Command::StateVaultRecover { state_dir } => recover_state_vault(state_dir),
        Command::StateVaultKeyExport {
            state_dir,
            output_file,
            passphrase_file,
        } => export_state_vault_key(state_dir, output_file, passphrase_file),
        Command::StateVaultKeyImport {
            state_dir,
            recovery_file,
            passphrase_file,
        } => import_state_vault_key(state_dir, recovery_file, passphrase_file),
        Command::StateVaultRestore {
            state_dir,
            output_state_dir,
        } => restore_state_vault(state_dir, output_state_dir),
    }
}

fn migrate_state_vault(state_dir: PathBuf) -> Result<()> {
    let vault = EncryptedStateVault::open_or_create(&state_dir)
        .context("open encrypted transactional state vault")?;
    let (outcome, report) = vault
        .migrate_legacy_snapshot()
        .context("atomically migrate legacy device state into encrypted vault")?;
    println!("vault_file={}", state_dir.join(STATE_VAULT_FILE).display());
    println!(
        "vault_key_file={}",
        state_dir.join(STATE_VAULT_KEY_FILE).display()
    );
    print_vault_key_status(&vault);
    println!(
        "migration={}",
        match outcome {
            VaultMigrationOutcome::Migrated => "committed",
            VaultMigrationOutcome::AlreadyCurrent => "already-current",
        }
    );
    print_vault_report(&report);
    println!("legacy_non_identity_files_retained=true");
    println!("legacy_device_identity_files_retained=false");
    println!("status=state-vault-migrated");
    Ok(())
}

fn verify_state_vault(state_dir: PathBuf) -> Result<()> {
    let vault = EncryptedStateVault::open_existing(&state_dir)
        .context("open encrypted transactional state vault")?;
    print_vault_key_status(&vault);
    let report = vault
        .verify_against_legacy()
        .context("verify vault records and retained legacy state")?;
    print_vault_report(&report);
    println!("legacy_snapshot_match=true");
    println!("status=state-vault-verified");
    Ok(())
}

fn shadow_read_state_vault(state_dir: PathBuf) -> Result<()> {
    let vault = EncryptedStateVault::open_existing(&state_dir)
        .context("open encrypted transactional state vault")?;
    let reports = vault
        .verify_typed_shadow_reads()
        .context("compare typed encrypted and legacy repository reads")?;
    for report in &reports {
        println!(
            "vault_shadow_kind={} records={} plaintext_bytes={}",
            report.kind().as_str(),
            report.record_count(),
            report.plaintext_bytes()
        );
    }
    println!("shadow_kind_count={}", reports.len());
    println!("shadow_reads_equal=true");
    println!("status=state-vault-shadow-read-verified");
    Ok(())
}

fn recover_state_vault(state_dir: PathBuf) -> Result<()> {
    let vault = EncryptedStateVault::open_existing(&state_dir)
        .context("open encrypted transactional state vault")?;
    let report = match vault
        .recover_pending_dual_write()
        .context("recover authenticated pending state vault dual-write")?
    {
        Some(commit) => {
            println!(
                "vault_dual_write_recovery={}",
                vault_mirror_outcome_name(commit.outcome())
            );
            print_vault_mirror_delta(&commit);
            commit.report().clone()
        }
        None => {
            println!("vault_dual_write_recovery=not-needed");
            vault
                .verify_against_legacy()
                .context("verify vault without a pending dual-write intent")?
        }
    };
    print_vault_report(&report);
    println!("legacy_snapshot_match=true");
    println!("status=state-vault-recovered");
    Ok(())
}

fn restore_state_vault(state_dir: PathBuf, output_state_dir: PathBuf) -> Result<()> {
    let vault = EncryptedStateVault::open_existing(&state_dir)
        .context("open encrypted transactional state vault")?;
    let report = vault
        .restore_to_new_directory(&output_state_dir)
        .context("restore authenticated state vault snapshot")?;
    print_vault_report(&report);
    println!("output_state_dir={}", output_state_dir.display());
    println!("status=state-vault-restored");
    Ok(())
}

fn export_state_vault_key(
    state_dir: PathBuf,
    output_file: PathBuf,
    passphrase_file: PathBuf,
) -> Result<()> {
    let passphrase = read_recovery_passphrase(&passphrase_file)?;
    let vault = EncryptedStateVault::open_existing(&state_dir)
        .context("open encrypted transactional state vault for key export")?;
    let exported = vault
        .export_key_recovery(&output_file, passphrase.as_slice())
        .context("export portable vault key recovery package")?;
    println!("vault_key_recovery_file={}", output_file.display());
    println!("vault_key_recovery_format=portable-recovery-v1");
    println!("vault_key_recovery_kdf=argon2id-v19-m65536-t3-p1");
    print_recovery_witness(exported.witness());
    println!("status=state-vault-key-exported");
    Ok(())
}

fn import_state_vault_key(
    state_dir: PathBuf,
    recovery_file: PathBuf,
    passphrase_file: PathBuf,
) -> Result<()> {
    let passphrase = read_recovery_passphrase(&passphrase_file)?;
    let imported =
        EncryptedStateVault::import_key_recovery(&state_dir, &recovery_file, passphrase.as_slice())
            .context("authenticate vault and import portable key recovery package")?;
    println!("vault_key_recovery_file={}", recovery_file.display());
    println!("vault_key_recovery_format=portable-recovery-v1");
    print_recovery_witness(imported.witness());
    println!(
        "vault_key_recovery_database_generation={}",
        imported.current().mirror_generation()
    );
    println!(
        "vault_key_recovery_database_snapshot_id={}",
        encode_hex(imported.current().snapshot_id())
    );
    println!("vault_key_file_format=protected-envelope-v1");
    println!(
        "vault_key_protection={}",
        imported.key_protection().as_str()
    );
    println!("vault_key_recovery_rollback_check=passed");
    println!("status=state-vault-key-imported");
    Ok(())
}

fn read_recovery_passphrase(path: &Path) -> Result<Zeroizing<Vec<u8>>> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("inspect recovery passphrase file {}", path.display()))?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "recovery passphrase file must not be a symbolic link: {}",
        path.display()
    );
    ensure!(
        metadata.is_file(),
        "recovery passphrase path is not a regular file: {}",
        path.display()
    );
    ensure!(
        metadata.len() <= MAX_RECOVERY_PASSPHRASE_FILE_BYTES,
        "recovery passphrase file is too large: {} bytes",
        metadata.len()
    );
    let mut passphrase = Zeroizing::new(
        fs::read(path)
            .with_context(|| format!("read recovery passphrase file {}", path.display()))?,
    );
    if passphrase.ends_with(b"\r\n") {
        let trimmed_len = passphrase.len() - 2;
        passphrase.truncate(trimmed_len);
    } else if passphrase.ends_with(b"\n") {
        let trimmed_len = passphrase.len() - 1;
        passphrase.truncate(trimmed_len);
    }
    Ok(passphrase)
}

fn print_vault_report(report: &VaultReport) {
    println!("vault_schema_version={}", report.schema_version());
    println!("vault_mirror_generation={}", report.mirror_generation());
    println!("vault_record_count={}", report.record_count());
    println!("vault_plaintext_bytes={}", report.plaintext_bytes());
    println!("vault_snapshot_id={}", encode_hex(report.snapshot_id()));
}

fn print_recovery_witness(witness: &VaultRecoveryWitness) {
    println!(
        "vault_key_recovery_witness_schema_version={}",
        witness.schema_version()
    );
    println!(
        "vault_key_recovery_witness_generation={}",
        witness.mirror_generation()
    );
    println!(
        "vault_key_recovery_witness_snapshot_id={}",
        encode_hex(witness.snapshot_id())
    );
}

fn print_vault_key_status(vault: &EncryptedStateVault) {
    println!("vault_key_file_format=protected-envelope-v1");
    println!("vault_key_protection={}", vault.key_protection().as_str());
    println!("vault_key_load={}", vault.key_load_outcome().as_str());
}

fn print_vault_mirror_delta(commit: &VaultMirrorCommit) {
    println!(
        "vault_delta_upserted_records={}",
        commit.delta().upserted_records()
    );
    println!(
        "vault_delta_removed_records={}",
        commit.delta().removed_records()
    );
    println!(
        "vault_delta_unchanged_records={}",
        commit.delta().unchanged_records()
    );
}

fn print_vault_mirror_commit(commit: &VaultMirrorCommit) {
    print_vault_mirror_delta(commit);
    print_vault_report(commit.report());
}

struct PendingVaultPrimaryWrite {
    vault: EncryptedStateVault,
    commit: VaultMirrorCommit,
    ratchet_workspace_prepared: bool,
    trust_workspace_prepared: bool,
    append_only_write_count: usize,
}

impl PendingVaultPrimaryWrite {
    fn prepare(
        state_directory: &Path,
        transaction: &StateTransaction,
    ) -> Result<Option<Self>, kilogram_state::StateError> {
        if !EncryptedStateVault::is_initialized(state_directory)? {
            return Ok(None);
        }
        let vault = EncryptedStateVault::open_existing(state_directory)?;
        let commit = vault.commit_primary_transaction(transaction)?;
        Ok(Some(Self {
            vault,
            commit,
            ratchet_workspace_prepared: transaction.ratchet_workspace_prepared(),
            trust_workspace_prepared: transaction.trust_workspace_prepared(),
            append_only_write_count: transaction.registered_append_only_write_count(),
        }))
    }

    fn confirm(self) -> Result<(), kilogram_state::StateError> {
        let report = self.vault.confirm_primary_shadow()?;
        println!("vault_primary_write_mode=typed-registered-delta");
        println!(
            "vault_ratchet_workspace_committed={}",
            self.ratchet_workspace_prepared
        );
        println!(
            "vault_trust_workspace_committed={}",
            self.trust_workspace_prepared
        );
        println!(
            "vault_append_write_set_count={}",
            self.append_only_write_count
        );
        println!(
            "vault_repository_write_receipt_path_count={}",
            self.append_only_write_count
        );
        println!(
            "vault_manifest_index_mode={}",
            self.commit.manifest_index_mode().as_str()
        );
        println!(
            "vault_payload_records_loaded={}",
            self.commit.payload_records_loaded()
        );
        println!(
            "vault_primary_write={}",
            match self.commit.outcome() {
                VaultMirrorOutcome::Mirrored => "committed",
                VaultMirrorOutcome::AlreadyCurrent => "already-current",
            }
        );
        println!(
            "vault_primary_write_upserted_records={}",
            self.commit.delta().upserted_records()
        );
        println!(
            "vault_primary_write_removed_records={}",
            self.commit.delta().removed_records()
        );
        println!(
            "vault_primary_write_unchanged_records={}",
            self.commit.delta().unchanged_records()
        );
        println!(
            "vault_primary_write_generation={}",
            report.mirror_generation()
        );
        println!("vault_primary_shadow=confirmed");
        Ok(())
    }
}

enum TransactionSequenceSource {
    Unloaded,
    Filesystem,
    Vault { next: u64 },
}

struct CommandTransactionContext<'a> {
    state_directory: &'a Path,
    transaction: &'a mut StateTransaction,
    sequence: TransactionSequenceSource,
    ratchet_workspace_prepared: bool,
    trust_workspace_prepared: bool,
}

impl<'a> CommandTransactionContext<'a> {
    fn new(state_directory: &'a Path, transaction: &'a mut StateTransaction) -> Self {
        Self {
            state_directory,
            transaction,
            sequence: TransactionSequenceSource::Unloaded,
            ratchet_workspace_prepared: false,
            trust_workspace_prepared: false,
        }
    }

    fn load_ratchet_state(&mut self) -> Result<RatchetState> {
        if !self.ratchet_workspace_prepared {
            if EncryptedStateVault::is_initialized(self.state_directory)
                .context("inspect state vault before mutable ratchet read")?
            {
                let vault = EncryptedStateVault::open_existing(self.state_directory)
                    .context("open state vault for mutable ratchet read")?;
                let read = vault
                    .read_mutable_primary_canary(&[StateRecordKind::Ratchet])
                    .context("read mutable ratchet workspace from authenticated vault state")?;
                self.transaction
                    .prepare_ratchet_workspace(&read)
                    .context("prepare retained ratchet staging from DB-primary state")?;
                println!("vault_mutable_read_kind=ratchet");
                println!("vault_mutable_read_source=db-primary");
                println!("vault_mutable_read_generation={}", read.mirror_generation());
                println!("vault_mutable_read_record_count={}", read.records().len());
                println!("vault_ratchet_workspace=prepared");
            }
            self.ratchet_workspace_prepared = true;
        }
        RatchetState::load_or_create(self.state_directory)
            .context("load persistent ratchet state from the transaction workspace")
    }

    fn load_ratchet_state_for_store(&mut self) -> Result<RatchetState, StoreError> {
        self.load_ratchet_state().map_err(|error| {
            StoreError::from(io::Error::other(format!(
                "prepare DB-primary ratchet workspace: {error:#}"
            )))
        })
    }

    fn prepare_trust_workspace(&mut self) -> Result<()> {
        if self.trust_workspace_prepared {
            return Ok(());
        }
        if EncryptedStateVault::is_initialized(self.state_directory)
            .context("inspect state vault before mutable trust access")?
        {
            let vault = EncryptedStateVault::open_existing(self.state_directory)
                .context("open state vault for mutable trust access")?;
            let read = vault
                .read_primary_trust()
                .context("read authority/contact trust repository from authenticated vault")?;
            self.transaction
                .prepare_trust_workspace(&read)
                .context("prepare retained trust staging from DB-primary state")?;
            println!("vault_mutable_read_kind=trust");
            println!("vault_mutable_read_source=db-primary");
            println!("vault_mutable_read_generation={}", read.mirror_generation());
            println!("vault_mutable_read_record_count={}", read.records().len());
            println!("vault_trust_workspace=prepared");
        } else {
            self.transaction
                .prepare_legacy_trust_workspace()
                .context("prepare legacy trust workspace")?;
        }
        self.trust_workspace_prepared = true;
        Ok(())
    }

    #[cfg(test)]
    fn register_append_only_write(&mut self, relative_path: impl AsRef<Path>) -> Result<()> {
        self.transaction
            .register_append_only_write(relative_path)
            .context("register append-only state write")
    }

    fn register_append_only_receipt_path(&mut self, path: impl AsRef<Path>) -> Result<()> {
        self.transaction
            .register_append_only_receipt_path(path)
            .context("register append-only state receipt")
    }

    fn register_store_receipt(&mut self, receipt: &AppendOnlyWriteReceipt) -> Result<()> {
        for path in receipt.paths() {
            self.transaction
                .register_append_only_receipt_path(path)
                .context("register repository append-only write receipt")?;
        }
        Ok(())
    }

    fn register_store_receipt_for_store(
        &mut self,
        receipt: &AppendOnlyWriteReceipt,
    ) -> Result<(), StoreError> {
        for path in receipt.paths() {
            self.transaction
                .register_append_only_receipt_path(path)
                .map_err(state_transaction_store_error)?;
        }
        Ok(())
    }

    fn compact_runtime_record(&mut self, relative_path: impl AsRef<Path>) -> Result<()> {
        self.transaction
            .compact_runtime_record(relative_path)
            .context("register crash-safe runtime compaction removal")
    }

    fn allocate_sequence(&mut self, device_state: &DeviceState) -> Result<u64> {
        if matches!(self.sequence, TransactionSequenceSource::Unloaded) {
            self.sequence = self.load_sequence_source()?;
        }
        match &mut self.sequence {
            TransactionSequenceSource::Unloaded => {
                bail!("transaction sequence source remained uninitialized")
            }
            TransactionSequenceSource::Filesystem => device_state
                .allocate_sequence()
                .context("allocate sequence from retained filesystem state"),
            TransactionSequenceSource::Vault { next } => {
                let current = *next;
                device_state
                    .allocate_sequence_from(current)
                    .context("allocate sequence from authenticated vault-primary state")?;
                *next = current
                    .checked_add(1)
                    .context("device sequence is exhausted")?;
                Ok(current)
            }
        }
    }

    fn load_sequence_source(&mut self) -> Result<TransactionSequenceSource> {
        if !EncryptedStateVault::is_initialized(self.state_directory)
            .context("inspect state vault before mutable sequence read")?
        {
            return Ok(TransactionSequenceSource::Filesystem);
        }
        let vault = EncryptedStateVault::open_existing(self.state_directory)
            .context("open state vault for mutable sequence read")?;
        let read = vault
            .read_mutable_primary_canary(&[StateRecordKind::Sequence])
            .context("read mutable sequence from authenticated vault state")?;
        self.transaction
            .prepare_sequence_workspace(&read)
            .context("prepare retained sequence staging from DB-primary state")?;
        ensure!(
            read.records().len() <= 1,
            "vault sequence repository contains more than one record"
        );
        let next = match read.records().first() {
            Some(record) => {
                ensure!(
                    record.kind() == StateRecordKind::Sequence
                        && record.relative_path() == "next-sequence",
                    "vault sequence repository returned an unexpected record"
                );
                std::str::from_utf8(record.content())
                    .context("vault next-sequence is not UTF-8")?
                    .trim()
                    .parse::<u64>()
                    .context("vault next-sequence is invalid")?
            }
            None => 0,
        };
        println!("vault_mutable_read_kind=sequence");
        println!("vault_mutable_read_source=db-primary");
        println!("vault_mutable_read_generation={}", read.mirror_generation());
        println!("vault_mutable_read_record_count={}", read.records().len());
        Ok(TransactionSequenceSource::Vault { next })
    }
}

fn run_state_transaction<T>(
    state_directory: &Path,
    operation: impl FnOnce(&mut CommandTransactionContext<'_>) -> Result<T>,
) -> Result<T> {
    let mut transaction = StateTransaction::begin(state_directory)
        .context("prepare crash-consistent local state transaction")?;
    let operation_result = {
        let mut context = CommandTransactionContext::new(state_directory, &mut transaction);
        operation(&mut context)
    };
    match operation_result {
        Ok(value) => {
            let primary_write = match PendingVaultPrimaryWrite::prepare(
                state_directory,
                &transaction,
            ) {
                Ok(primary_write) => primary_write,
                Err(primary_error) => {
                    return match transaction.rollback() {
                        Ok(()) => Err(primary_error)
                            .context("commit staged local state to the encrypted vault"),
                        Err(rollback_error) => Err(anyhow::Error::new(primary_error).context(
                            format!(
                                "vault-primary commit failed and local rollback also failed: {rollback_error}"
                            ),
                        )),
                    };
                }
            };
            transaction
                .commit()
                .context("commit crash-consistent local state transaction")?;
            if let Some(primary_write) = primary_write {
                primary_write
                    .confirm()
                    .context("confirm retained legacy shadow after vault-primary commit")?;
            }
            Ok(value)
        }
        Err(operation_error) => match transaction.rollback() {
            Ok(()) => Err(operation_error),
            Err(rollback_error) => Err(operation_error.context(format!(
                "local state operation failed and rollback also failed: {rollback_error}"
            ))),
        },
    }
}

fn run_store_transaction<T>(
    state_directory: &Path,
    operation: impl FnOnce(&mut CommandTransactionContext<'_>) -> Result<T, StoreError>,
) -> Result<T, StoreError> {
    let mut transaction =
        StateTransaction::begin(state_directory).map_err(state_transaction_store_error)?;
    let operation_result = {
        let mut context = CommandTransactionContext::new(state_directory, &mut transaction);
        operation(&mut context)
    };
    match operation_result {
        Ok(value) => {
            let primary_write = match PendingVaultPrimaryWrite::prepare(
                state_directory,
                &transaction,
            ) {
                Ok(primary_write) => primary_write,
                Err(primary_error) => {
                    return match transaction.rollback() {
                        Ok(()) => Err(state_transaction_store_error(primary_error)),
                        Err(rollback_error) => Err(StoreError::from(io::Error::other(format!(
                            "vault-primary store commit failed ({primary_error}) and local rollback also failed: {rollback_error}"
                        )))),
                    };
                }
            };
            transaction
                .commit()
                .map_err(state_transaction_store_error)?;
            if let Some(primary_write) = primary_write {
                primary_write
                    .confirm()
                    .map_err(state_transaction_store_error)?;
            }
            Ok(value)
        }
        Err(operation_error) => match transaction.rollback() {
            Ok(()) => Err(operation_error),
            Err(rollback_error) => Err(StoreError::from(io::Error::other(format!(
                "local store operation failed ({operation_error}) and rollback also failed: {rollback_error}"
            )))),
        },
    }
}

fn state_transaction_store_error(error: kilogram_state::StateError) -> StoreError {
    StoreError::from(io::Error::other(error))
}

fn load_command_device_state(state_directory: &Path) -> Result<DeviceState> {
    if !EncryptedStateVault::is_initialized(state_directory)
        .context("inspect state vault before device identity read")?
    {
        let device_state = DeviceState::load_or_create(state_directory).with_context(|| {
            format!(
                "load retained filesystem device state from {}",
                state_directory.display()
            )
        })?;
        println!("device_identity_read_source=filesystem");
        return Ok(device_state);
    }

    let read = EncryptedStateVault::open_existing(state_directory)
        .context("open encrypted vault device identity repository")?
        .read_primary_device_identity()
        .context("read authenticated DB-primary device identity repository")?;
    let mirror_generation = read.mirror_generation();
    let device_state = DeviceState::from_secret_material(
        state_directory,
        *read.signing_secret(),
        *read.encryption_secret(),
    );
    println!("vault_device_identity_read_source=db-primary");
    println!("vault_device_identity_read_generation={mirror_generation}");
    println!("vault_device_identity_record_count=2");
    Ok(device_state)
}

enum TrustReadSource {
    Filesystem,
    Vault { records: BTreeMap<String, Vec<u8>> },
}

struct CommandTrustReadRepository<'a> {
    device_state: &'a DeviceState,
    source: TrustReadSource,
}

impl<'a> CommandTrustReadRepository<'a> {
    fn open(state_directory: &Path, device_state: &'a DeviceState) -> Result<Self> {
        if !EncryptedStateVault::is_initialized(state_directory)
            .context("inspect state vault before trust repository read")?
        {
            return Ok(Self {
                device_state,
                source: TrustReadSource::Filesystem,
            });
        }
        let read = EncryptedStateVault::open_existing(state_directory)
            .context("open encrypted vault trust repository")?
            .read_primary_trust()
            .context("read authenticated DB-primary trust repository")?;
        let mirror_generation = read.mirror_generation();
        let mut records = BTreeMap::new();
        for record in read.into_records() {
            ensure!(
                record.kind() == StateRecordKind::Trust,
                "trust repository returned non-trust record {}",
                record.relative_path()
            );
            let (_, relative_path, content) = record.into_parts();
            ensure!(
                records.insert(relative_path.clone(), content).is_none(),
                "trust repository returned duplicate record {relative_path}"
            );
        }
        println!("vault_trust_read_source=db-primary");
        println!("vault_trust_read_generation={mirror_generation}");
        println!("vault_trust_read_record_count={}", records.len());
        Ok(Self {
            device_state,
            source: TrustReadSource::Vault { records },
        })
    }

    fn vault_record(&self, relative_path: &str) -> Result<Option<&[u8]>> {
        match &self.source {
            TrustReadSource::Filesystem => Ok(None),
            TrustReadSource::Vault { records } => records
                .get(relative_path)
                .map(Vec::as_slice)
                .map(Some)
                .with_context(|| format!("DB-primary trust record is missing: {relative_path}")),
        }
    }

    fn load_certificate(&self) -> Result<DeviceCertificate> {
        let Some(bytes) = self.vault_record("device-certificate.cert")? else {
            return self
                .device_state
                .load_certificate()
                .context("load retained device certificate");
        };
        let certificate = DeviceCertificate::decode_and_verify(bytes)
            .context("decode DB-primary device certificate")?;
        ensure!(
            certificate.device_id() == self.device_state.identity().device_id(),
            "DB-primary certificate belongs to a different device"
        );
        ensure!(
            certificate.encryption_public_key() == self.device_state.encryption().public_key(),
            "DB-primary certificate has a different device encryption key"
        );
        Ok(certificate)
    }

    fn load_own_authority_snapshot(
        &self,
        certificate: &DeviceCertificate,
    ) -> Result<AccountAuthoritySnapshot> {
        let Some(bytes) = self.vault_record("account-authority.snapshot")? else {
            return self
                .device_state
                .load_own_authority_snapshot()
                .context("load retained own authority snapshot");
        };
        let snapshot = AccountAuthoritySnapshot::decode_and_verify(bytes)
            .context("decode DB-primary own authority snapshot")?;
        snapshot
            .verify_for_account(certificate.account_id())
            .context("verify DB-primary own authority account")?;
        ensure!(
            certificate.authority_sequence() < snapshot.revision(),
            "device certificate sequence is outside DB-primary authority snapshot"
        );
        Ok(snapshot)
    }

    fn load_peer_authority_snapshot(
        &self,
        account_id: AccountId,
    ) -> Result<AccountAuthoritySnapshot> {
        let relative_path = format!("peer-authority/{account_id}.snapshot");
        let Some(bytes) = self.vault_record(&relative_path)? else {
            return self
                .device_state
                .load_peer_authority_snapshot(account_id)
                .context("load retained peer authority snapshot");
        };
        let snapshot = AccountAuthoritySnapshot::decode_and_verify(bytes)
            .context("decode DB-primary peer authority snapshot")?;
        snapshot
            .verify_for_account(account_id)
            .context("verify DB-primary peer authority account")?;
        Ok(snapshot)
    }

    fn load_conversation_membership(
        &self,
        conversation_id: ConversationScopeId,
    ) -> Result<ConversationMembershipSnapshot> {
        let relative_path = format!("conversation-memberships/{conversation_id}.membership");
        let Some(bytes) = self.vault_record(&relative_path)? else {
            return self
                .device_state
                .load_conversation_membership(conversation_id)
                .context("load retained conversation membership");
        };
        let membership = ConversationMembershipSnapshot::decode_and_verify(bytes)
            .context("decode DB-primary conversation membership")?;
        ensure!(
            membership.conversation_id() == conversation_id,
            "DB-primary membership belongs to a different conversation"
        );
        Ok(membership)
    }
}

fn install_own_authority_primary(
    state_directory: &Path,
    device_state: &DeviceState,
    snapshot: &AccountAuthoritySnapshot,
) -> Result<AuthoritySnapshotStoreOutcome> {
    run_state_transaction(state_directory, |transaction| {
        transaction.prepare_trust_workspace()?;
        device_state
            .install_own_authority_snapshot(snapshot)
            .context("install own authority snapshot in DB-primary trust workspace")
    })
}

fn install_enrollment_primary(
    state_directory: &Path,
    device_state: &DeviceState,
    certificate: &DeviceCertificate,
    snapshot: &AccountAuthoritySnapshot,
) -> Result<AuthoritySnapshotStoreOutcome> {
    run_state_transaction(state_directory, |transaction| {
        transaction.prepare_trust_workspace()?;
        device_state
            .install_certificate(certificate)
            .context("install certificate in DB-primary trust workspace")?;
        device_state
            .install_own_authority_snapshot(snapshot)
            .context("install enrollment authority in DB-primary trust workspace")
    })
}

fn pin_peer_authority_primary(
    state_directory: &Path,
    device_state: &DeviceState,
    snapshot: &AccountAuthoritySnapshot,
) -> Result<AuthoritySnapshotStoreOutcome> {
    run_state_transaction(state_directory, |transaction| {
        transaction.prepare_trust_workspace()?;
        device_state
            .pin_peer_authority_snapshot(snapshot)
            .context("pin peer authority snapshot in DB-primary trust workspace")
    })
}

fn install_membership_primary(
    state_directory: &Path,
    device_state: &DeviceState,
    membership: &ConversationMembershipSnapshot,
) -> Result<ConversationMembershipStoreOutcome> {
    run_state_transaction(state_directory, |transaction| {
        transaction.prepare_trust_workspace()?;
        device_state
            .install_conversation_membership(membership)
            .context("install membership in DB-primary trust workspace")
    })
}

#[derive(Default)]
struct RuntimeStateSnapshot {
    contacts: BTreeMap<RuntimeContactId, SignedRuntimeContact>,
    endpoint_candidates: BTreeMap<RuntimeEndpointCandidateId, SignedRuntimeEndpointCandidate>,
    endpoint_publication_bindings:
        BTreeMap<RuntimeEndpointCandidateId, SignedRuntimeEndpointPublicationBinding>,
    queued: BTreeMap<RuntimeQueueId, SignedQueuedMessage>,
    materialized: BTreeMap<RuntimeQueueId, SignedMaterializedMessage>,
    delivered: BTreeMap<RuntimeQueueId, SignedDeliveredMessage>,
    retries: BTreeMap<RuntimeQueueId, Vec<SignedRuntimeRetryState>>,
    device_directory_receipts: Vec<SignedRuntimeDeviceDirectoryReceipt>,
    ticket_publications: BTreeMap<TicketPublicationChannelId, Vec<SignedTicketPublication>>,
    ticket_observations:
        BTreeMap<TicketPublicationChannelId, Vec<SignedTicketPublicationObservation>>,
    accepted_endpoint_observations:
        BTreeMap<TicketPublicationChannelId, Vec<SignedAcceptedEndpointObservation>>,
    ticket_automation_policies: BTreeMap<RuntimeContactId, Vec<SignedTicketAutomationPolicy>>,
    ticket_automation_attempts:
        BTreeMap<(RuntimeContactId, TicketAutomationAction), Vec<SignedTicketAutomationAttempt>>,
    ticket_checkpoint: Option<SignedRuntimeTicketCheckpoint>,
}

impl RuntimeStateSnapshot {
    fn pending_count(&self) -> usize {
        self.queued
            .keys()
            .filter(|queue_id| !self.delivered.contains_key(queue_id))
            .count()
    }

    fn latest_retry(&self, queue_id: RuntimeQueueId) -> Option<&SignedRuntimeRetryState> {
        self.retries.get(&queue_id).and_then(|states| states.last())
    }

    fn latest_device_directory_receipt(&self) -> Option<&SignedRuntimeDeviceDirectoryReceipt> {
        self.device_directory_receipts.last()
    }

    fn latest_ticket_publication(
        &self,
        channel_id: TicketPublicationChannelId,
    ) -> Option<&SignedTicketPublication> {
        self.ticket_publications
            .get(&channel_id)
            .and_then(|publications| publications.last())
    }

    fn latest_ticket_observation(
        &self,
        channel_id: TicketPublicationChannelId,
    ) -> Option<&SignedTicketPublicationObservation> {
        self.ticket_observations
            .get(&channel_id)
            .and_then(|observations| observations.last())
    }

    fn accepted_endpoint_observation_high_water(
        &self,
        channel_id: TicketPublicationChannelId,
    ) -> Result<Option<(u64, TicketPublicationId, [u8; 32])>> {
        let Some(evidence) = self.accepted_endpoint_observations.get(&channel_id) else {
            return Ok(None);
        };
        let Some(highest_generation) = evidence
            .iter()
            .map(SignedAcceptedEndpointObservation::publication_generation)
            .max()
        else {
            return Ok(None);
        };
        let mut highest = evidence
            .iter()
            .filter(|value| value.publication_generation() == highest_generation);
        let first = highest
            .next()
            .context("accepted endpoint observation high-water is empty")?;
        let value = (
            first.publication_generation(),
            first.publication_id(),
            first.ticket_digest(),
        );
        ensure!(
            highest.all(|candidate| {
                candidate.publication_id() == value.1 && candidate.ticket_digest() == value.2
            }),
            "authorized devices equivocated at the imported publication high-water"
        );
        Ok(Some(value))
    }

    fn latest_ticket_automation_policy(
        &self,
        contact_id: RuntimeContactId,
    ) -> Option<&SignedTicketAutomationPolicy> {
        self.ticket_automation_policies
            .get(&contact_id)
            .and_then(|policies| policies.last())
    }

    fn latest_ticket_automation_attempt(
        &self,
        contact_id: RuntimeContactId,
        action: TicketAutomationAction,
    ) -> Option<&SignedTicketAutomationAttempt> {
        self.ticket_automation_attempts
            .get(&(contact_id, action))
            .and_then(|attempts| attempts.last())
    }
}

fn runtime_contact_relative_path(contact_id: RuntimeContactId) -> PathBuf {
    PathBuf::from(RUNTIME_STATE_DIRECTORY)
        .join(RUNTIME_CONTACTS_DIRECTORY)
        .join(format!("{contact_id}.contact"))
}

fn runtime_endpoint_candidate_relative_path(candidate_id: RuntimeEndpointCandidateId) -> PathBuf {
    PathBuf::from(RUNTIME_STATE_DIRECTORY)
        .join(RUNTIME_ENDPOINT_CANDIDATES_DIRECTORY)
        .join(format!("{candidate_id}.endpoint-candidate"))
}

fn runtime_endpoint_publication_binding_relative_path(
    binding_id: RuntimeEndpointCandidateId,
) -> PathBuf {
    PathBuf::from(RUNTIME_STATE_DIRECTORY)
        .join(RUNTIME_ENDPOINT_PUBLICATION_BINDINGS_DIRECTORY)
        .join(format!("{binding_id}.epb"))
}

fn runtime_queued_relative_path(queue_id: RuntimeQueueId) -> PathBuf {
    PathBuf::from(RUNTIME_STATE_DIRECTORY)
        .join(RUNTIME_OUTBOX_DIRECTORY)
        .join(format!("{queue_id}.queued"))
}

fn runtime_materialized_relative_path(queue_id: RuntimeQueueId) -> PathBuf {
    PathBuf::from(RUNTIME_STATE_DIRECTORY)
        .join(RUNTIME_OUTBOX_DIRECTORY)
        .join(format!("{queue_id}.materialized"))
}

fn runtime_delivered_relative_path(queue_id: RuntimeQueueId) -> PathBuf {
    PathBuf::from(RUNTIME_STATE_DIRECTORY)
        .join(RUNTIME_OUTBOX_DIRECTORY)
        .join(format!("{queue_id}.delivered"))
}

fn runtime_retry_relative_path(queue_id: RuntimeQueueId, generation: u32) -> PathBuf {
    PathBuf::from(RUNTIME_STATE_DIRECTORY)
        .join(RUNTIME_OUTBOX_DIRECTORY)
        .join(format!("{queue_id}.retry-{generation:010}"))
}

fn runtime_device_directory_receipt_relative_path(
    generation: u64,
    receipt_id: RuntimeDeviceDirectoryReceiptId,
) -> PathBuf {
    PathBuf::from(RUNTIME_STATE_DIRECTORY)
        .join(RUNTIME_DEVICE_DIRECTORY)
        .join(format!("{generation:020}-{receipt_id}.directory-receipt"))
}

fn runtime_ticket_publication_relative_path(
    channel_id: TicketPublicationChannelId,
    generation: u64,
    publication_id: TicketPublicationId,
) -> PathBuf {
    PathBuf::from(RUNTIME_STATE_DIRECTORY)
        .join(RUNTIME_TICKET_PUBLICATIONS_DIRECTORY)
        .join(format!(
            "{channel_id}-{generation:020}-{publication_id}.ticket-publication"
        ))
}

fn runtime_ticket_observation_relative_path(
    channel_id: TicketPublicationChannelId,
    observation_generation: u64,
    observation_id: TicketPublicationObservationId,
) -> PathBuf {
    PathBuf::from(RUNTIME_STATE_DIRECTORY)
        .join(RUNTIME_TICKET_OBSERVATIONS_DIRECTORY)
        .join(format!(
            "{channel_id}-{observation_generation:020}-{observation_id}.ticket-observation"
        ))
}

fn runtime_accepted_endpoint_observation_relative_path(
    evidence_id: AcceptedEndpointObservationId,
) -> PathBuf {
    PathBuf::from(RUNTIME_STATE_DIRECTORY)
        .join(RUNTIME_ACCEPTED_ENDPOINT_OBSERVATIONS_DIRECTORY)
        .join(format!("{evidence_id}.aeo"))
}

fn runtime_ticket_automation_policy_relative_path(policy_id: TicketAutomationPolicyId) -> PathBuf {
    PathBuf::from(RUNTIME_STATE_DIRECTORY)
        .join(RUNTIME_TICKET_AUTOMATION_POLICIES_DIRECTORY)
        .join(format!("{policy_id}.tap"))
}

fn runtime_ticket_automation_attempt_relative_path(
    attempt_id: TicketAutomationAttemptId,
) -> PathBuf {
    PathBuf::from(RUNTIME_STATE_DIRECTORY)
        .join(RUNTIME_TICKET_AUTOMATION_ATTEMPTS_DIRECTORY)
        .join(format!("{attempt_id}.taa"))
}

fn runtime_ticket_checkpoint_relative_path(checkpoint_id: RuntimeTicketCheckpointId) -> PathBuf {
    PathBuf::from(RUNTIME_STATE_DIRECTORY)
        .join(RUNTIME_TICKET_CHECKPOINTS_DIRECTORY)
        .join(format!("{checkpoint_id}.rtc"))
}

fn runtime_device_list_digest(device_list: &AccountDeviceListSnapshot) -> Result<[u8; 32]> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(RUNTIME_DEVICE_LIST_DIGEST_DOMAIN);
    hasher.update(
        &device_list
            .encode()
            .context("encode runtime device list for authenticated digest")?,
    );
    Ok(*hasher.finalize().as_bytes())
}

fn persist_runtime_record(
    state_directory: &Path,
    relative_path: &Path,
    bytes: &[u8],
    transaction: &mut CommandTransactionContext<'_>,
) -> Result<StoreOutcome> {
    ensure!(
        bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
        "runtime state record is too large"
    );
    let path = state_directory.join(relative_path);
    let parent = path.parent().context("runtime record path has no parent")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("create runtime state directory {}", parent.display()))?;
    if path.exists() {
        let existing = fs::read(&path)
            .with_context(|| format!("read existing runtime record {}", path.display()))?;
        ensure!(
            existing == bytes,
            "runtime append-only record already exists with different content: {}",
            path.display()
        );
        return Ok(StoreOutcome::AlreadyPresent);
    }
    let mut temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("create temporary runtime record in {}", parent.display()))?;
    temporary
        .write_all(bytes)
        .with_context(|| format!("write temporary runtime record for {}", path.display()))?;
    temporary
        .as_file()
        .sync_all()
        .with_context(|| format!("sync temporary runtime record for {}", path.display()))?;
    match temporary.persist_noclobber(&path) {
        Ok(file) => file
            .sync_all()
            .with_context(|| format!("sync runtime record {}", path.display()))?,
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
            let existing = fs::read(&path)
                .with_context(|| format!("read raced runtime record {}", path.display()))?;
            ensure!(
                existing == bytes,
                "runtime append-only record raced with different content: {}",
                path.display()
            );
            return Ok(StoreOutcome::AlreadyPresent);
        }
        Err(error) => {
            return Err(error.error)
                .with_context(|| format!("persist runtime record {}", path.display()));
        }
    }
    transaction.register_append_only_receipt_path(&path)?;
    Ok(StoreOutcome::Inserted)
}

fn read_runtime_record_files(state_directory: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>> {
    if EncryptedStateVault::is_initialized(state_directory)? {
        let vault = EncryptedStateVault::open_existing(state_directory)
            .context("open state vault for runtime primary read")?;
        let read = vault
            .read_primary_canary(&[StateRecordKind::Runtime])
            .context("read authenticated runtime records from DB-primary state")?;
        println!("vault_runtime_read_source=db-primary");
        println!("vault_runtime_read_generation={}", read.mirror_generation());
        println!("vault_runtime_read_record_count={}", read.records().len());
        return Ok(read
            .into_records()
            .into_iter()
            .map(|record| {
                let (_, relative_path, content) = record.into_parts();
                (PathBuf::from(relative_path), content)
            })
            .collect());
    }

    let mut records = Vec::new();
    for directory in [
        RUNTIME_CONTACTS_DIRECTORY,
        RUNTIME_ENDPOINT_CANDIDATES_DIRECTORY,
        RUNTIME_ENDPOINT_PUBLICATION_BINDINGS_DIRECTORY,
        RUNTIME_OUTBOX_DIRECTORY,
        RUNTIME_DEVICE_DIRECTORY,
        RUNTIME_TICKET_PUBLICATIONS_DIRECTORY,
        RUNTIME_TICKET_OBSERVATIONS_DIRECTORY,
        RUNTIME_ACCEPTED_ENDPOINT_OBSERVATIONS_DIRECTORY,
        RUNTIME_TICKET_AUTOMATION_POLICIES_DIRECTORY,
        RUNTIME_TICKET_AUTOMATION_ATTEMPTS_DIRECTORY,
        RUNTIME_TICKET_CHECKPOINTS_DIRECTORY,
    ] {
        let root = state_directory
            .join(RUNTIME_STATE_DIRECTORY)
            .join(directory);
        let entries = match fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error).with_context(|| format!("read {}", root.display())),
        };
        for entry in entries {
            let entry = entry.with_context(|| format!("read entry in {}", root.display()))?;
            let file_type = entry
                .file_type()
                .with_context(|| format!("inspect runtime record {}", entry.path().display()))?;
            ensure!(
                file_type.is_file() && !file_type.is_symlink(),
                "runtime state contains a non-regular record: {}",
                entry.path().display()
            );
            let metadata = entry
                .metadata()
                .with_context(|| format!("inspect runtime record {}", entry.path().display()))?;
            ensure!(
                metadata.len() <= MAX_RUNTIME_RECORD_BYTES as u64,
                "runtime state record is too large: {}",
                entry.path().display()
            );
            let relative = entry
                .path()
                .strip_prefix(state_directory)
                .context("runtime record escaped its state directory")?
                .to_owned();
            records.push((
                relative,
                fs::read(entry.path()).context("read runtime state record")?,
            ));
        }
    }
    Ok(records)
}

fn load_runtime_state_snapshot(
    state_directory: &Path,
    local_account_id: AccountId,
    local_device_id: DeviceId,
) -> Result<RuntimeStateSnapshot> {
    let mut snapshot = RuntimeStateSnapshot::default();
    for (relative_path, bytes) in read_runtime_record_files(state_directory)? {
        let file_name = relative_path
            .file_name()
            .and_then(|name| name.to_str())
            .context("runtime record filename is not UTF-8")?;
        if file_name.ends_with(".directory-receipt") {
            let value = SignedRuntimeDeviceDirectoryReceipt::decode(&bytes)?;
            ensure!(
                value.account_id() == local_account_id
                    && value.local_device_id() == local_device_id,
                "runtime device-directory receipt belongs to another local identity"
            );
            ensure!(
                relative_path
                    == runtime_device_directory_receipt_relative_path(
                        value.generation(),
                        value.receipt_id()?,
                    ),
                "runtime device-directory receipt filename does not match its authenticated state"
            );
            snapshot.device_directory_receipts.push(value);
        } else if file_name.ends_with(".rtc") {
            let value = SignedRuntimeTicketCheckpoint::decode(&bytes)?;
            ensure!(
                value.local_account_id() == local_account_id
                    && value.local_device_id() == local_device_id,
                "runtime ticket checkpoint belongs to another local identity"
            );
            ensure!(
                relative_path == runtime_ticket_checkpoint_relative_path(value.checkpoint_id()?),
                "runtime ticket checkpoint filename does not match its authenticated state"
            );
            ensure!(
                snapshot.ticket_checkpoint.replace(value).is_none(),
                "runtime state contains more than one ticket checkpoint"
            );
        } else if file_name.ends_with(".ticket-publication") {
            let value = SignedTicketPublication::decode(&bytes)?;
            ensure!(
                value.publisher_account_id() == local_account_id
                    && value.publisher_device_id() == local_device_id,
                "ticket publication belongs to another local identity"
            );
            ensure!(
                relative_path
                    == runtime_ticket_publication_relative_path(
                        value.channel_id(),
                        value.generation(),
                        value.publication_id()?,
                    ),
                "ticket publication filename does not match its authenticated state"
            );
            snapshot
                .ticket_publications
                .entry(value.channel_id())
                .or_default()
                .push(value);
        } else if file_name.ends_with(".ticket-observation") {
            let value = SignedTicketPublicationObservation::decode(&bytes)?;
            ensure!(
                value.local_account_id() == local_account_id
                    && value.local_device_id() == local_device_id,
                "ticket publication observation belongs to another local identity"
            );
            ensure!(
                relative_path
                    == runtime_ticket_observation_relative_path(
                        value.channel_id(),
                        value.observation_generation(),
                        value.observation_id()?,
                    ),
                "ticket publication observation filename does not match its authenticated state"
            );
            snapshot
                .ticket_observations
                .entry(value.channel_id())
                .or_default()
                .push(value);
        } else if file_name.ends_with(".aeo") {
            let value = SignedAcceptedEndpointObservation::decode(&bytes)?;
            value.verify_local(local_account_id, local_device_id)?;
            ensure!(
                relative_path
                    == runtime_accepted_endpoint_observation_relative_path(value.evidence_id()?),
                "accepted endpoint observation filename does not match its authenticated state"
            );
            snapshot
                .accepted_endpoint_observations
                .entry(value.channel_id())
                .or_default()
                .push(value);
        } else if file_name.ends_with(".tap") {
            let value = SignedTicketAutomationPolicy::decode(&bytes)?;
            ensure!(
                value.local_account_id() == local_account_id
                    && value.local_device_id() == local_device_id,
                "ticket automation policy belongs to another local identity"
            );
            ensure!(
                relative_path == runtime_ticket_automation_policy_relative_path(value.policy_id()?),
                "ticket automation policy filename does not match its authenticated state"
            );
            snapshot
                .ticket_automation_policies
                .entry(value.contact_id())
                .or_default()
                .push(value);
        } else if file_name.ends_with(".taa") {
            let value = SignedTicketAutomationAttempt::decode(&bytes)?;
            ensure!(
                value.local_account_id() == local_account_id
                    && value.local_device_id() == local_device_id,
                "ticket automation attempt belongs to another local identity"
            );
            ensure!(
                relative_path
                    == runtime_ticket_automation_attempt_relative_path(value.attempt_id()?),
                "ticket automation attempt filename does not match its authenticated state"
            );
            snapshot
                .ticket_automation_attempts
                .entry((value.contact_id(), value.action()))
                .or_default()
                .push(value);
        } else if file_name.ends_with(".epb") {
            let value = SignedRuntimeEndpointPublicationBinding::decode(&bytes)?;
            value.verify_local(local_account_id, local_device_id)?;
            ensure!(
                relative_path
                    == runtime_endpoint_publication_binding_relative_path(value.binding_id()),
                "runtime endpoint-publication binding filename does not match its authenticated ID"
            );
            ensure!(
                snapshot
                    .endpoint_publication_bindings
                    .insert(value.binding_id(), value)
                    .is_none(),
                "duplicate runtime endpoint-publication binding ID"
            );
        } else if file_name.ends_with(".endpoint-candidate") {
            let value = SignedRuntimeEndpointCandidate::decode(&bytes)?;
            value.verify_local(local_account_id, local_device_id)?;
            ensure!(
                relative_path == runtime_endpoint_candidate_relative_path(value.candidate_id()),
                "runtime endpoint-candidate filename does not match its authenticated ID"
            );
            ensure!(
                snapshot
                    .endpoint_candidates
                    .insert(value.candidate_id(), value)
                    .is_none(),
                "duplicate runtime endpoint-candidate ID"
            );
        } else if file_name.ends_with(".contact") {
            let value = SignedRuntimeContact::decode(&bytes)?;
            value.verify_local(local_account_id, local_device_id)?;
            ensure!(
                relative_path == runtime_contact_relative_path(value.contact_id()),
                "runtime contact filename does not match its authenticated ID"
            );
            ensure!(
                snapshot
                    .contacts
                    .insert(value.contact_id(), value)
                    .is_none(),
                "duplicate runtime contact ID"
            );
        } else if file_name.ends_with(".queued") {
            let value = SignedQueuedMessage::decode(&bytes)?;
            ensure!(
                value.local_account_id() == local_account_id
                    && value.local_device_id() == local_device_id,
                "runtime queue record belongs to another local identity"
            );
            ensure!(
                relative_path == runtime_queued_relative_path(value.queue_id()),
                "runtime queue filename does not match its authenticated ID"
            );
            ensure!(
                snapshot.queued.insert(value.queue_id(), value).is_none(),
                "duplicate runtime queue ID"
            );
        } else if file_name.ends_with(".materialized") {
            let value = SignedMaterializedMessage::decode(&bytes)?;
            ensure!(
                value.local_device_id() == local_device_id,
                "runtime materialization belongs to another local device"
            );
            ensure!(
                relative_path == runtime_materialized_relative_path(value.queue_id()),
                "runtime materialization filename does not match its authenticated ID"
            );
            ensure!(
                snapshot
                    .materialized
                    .insert(value.queue_id(), value)
                    .is_none(),
                "duplicate runtime materialization"
            );
        } else if file_name.ends_with(".delivered") {
            let value = SignedDeliveredMessage::decode(&bytes)?;
            ensure!(
                value.local_device_id() == local_device_id,
                "runtime delivery marker belongs to another local device"
            );
            ensure!(
                relative_path == runtime_delivered_relative_path(value.queue_id()),
                "runtime delivery filename does not match its authenticated ID"
            );
            ensure!(
                snapshot.delivered.insert(value.queue_id(), value).is_none(),
                "duplicate runtime delivery marker"
            );
        } else if file_name.contains(".retry-") {
            let value = SignedRuntimeRetryState::decode(&bytes)?;
            ensure!(
                value.local_device_id() == local_device_id,
                "runtime retry state belongs to another local device"
            );
            ensure!(
                relative_path == runtime_retry_relative_path(value.queue_id(), value.generation()),
                "runtime retry filename does not match its authenticated state"
            );
            snapshot
                .retries
                .entry(value.queue_id())
                .or_default()
                .push(value);
        } else {
            bail!(
                "unknown authenticated runtime record: {}",
                relative_path.display()
            );
        }
    }
    for candidate in snapshot.endpoint_candidates.values() {
        let contact = snapshot
            .contacts
            .get(&candidate.contact_id())
            .context("runtime endpoint candidate names an absent contact")?;
        ensure!(
            candidate.peer_account_id() == contact.peer_account_id()
                && candidate.conversation_id() == contact.conversation_id()
                && candidate.peer_device_id() != contact.peer_device_id(),
            "runtime endpoint candidate does not match its authenticated contact"
        );
    }
    for binding in snapshot.endpoint_publication_bindings.values() {
        let contact = snapshot
            .contacts
            .get(&binding.contact_id())
            .context("runtime endpoint-publication binding names an absent contact")?;
        let enrollment =
            exact_runtime_endpoint_enrollment(&snapshot, contact, binding.peer_device_id())?;
        ensure!(
            binding.peer_account_id() == enrollment.peer_account_id
                && binding.conversation_id() == contact.conversation_id()
                && binding.route_policy() == enrollment.route_policy
                && binding.descriptor_file() == &enrollment.descriptor_file,
            "runtime endpoint-publication binding does not match its authenticated endpoint enrollment"
        );
    }
    for (channel_id, evidence) in &snapshot.accepted_endpoint_observations {
        for value in evidence {
            ensure!(
                value.channel_id() == *channel_id,
                "accepted endpoint observation map key is inconsistent"
            );
            ensure!(
                snapshot
                    .endpoint_publication_bindings
                    .values()
                    .any(|binding| {
                        binding.ticket_publication_write_key().channel_id() == *channel_id
                            && binding.peer_account_id() == value.publisher_account_id()
                            && binding.peer_device_id() == value.publisher_device_id()
                    }),
                "accepted endpoint observation has no matching enrolled endpoint"
            );
        }
        snapshot.accepted_endpoint_observation_high_water(*channel_id)?;
    }
    let mut candidate_counts = BTreeMap::<RuntimeContactId, usize>::new();
    for candidate in snapshot.endpoint_candidates.values() {
        let count = candidate_counts.entry(candidate.contact_id()).or_insert(1);
        *count = count
            .checked_add(1)
            .context("runtime endpoint-candidate count overflow")?;
        ensure!(
            *count <= MAX_RUNTIME_ENDPOINT_CANDIDATES_PER_CONTACT,
            "runtime contact exceeds the bounded endpoint-candidate limit of {MAX_RUNTIME_ENDPOINT_CANDIDATES_PER_CONTACT}"
        );
    }
    ensure!(
        snapshot.contacts.len() <= MAX_RUNTIME_CONTACTS,
        "runtime contact limit exceeded"
    );
    ensure!(
        snapshot.queued.len() <= MAX_RUNTIME_QUEUE_ITEMS,
        "runtime queue limit exceeded"
    );
    let retry_count: usize = snapshot.retries.values().map(Vec::len).sum();
    ensure!(
        retry_count <= MAX_RUNTIME_RETRY_STATES,
        "runtime retry state limit exceeded"
    );
    ensure!(
        snapshot.device_directory_receipts.len() <= MAX_RUNTIME_DEVICE_DIRECTORY_RECEIPTS,
        "runtime device-directory receipt limit exceeded"
    );
    let publication_record_count = snapshot
        .ticket_publications
        .values()
        .map(Vec::len)
        .sum::<usize>()
        .saturating_add(
            snapshot
                .ticket_observations
                .values()
                .map(Vec::len)
                .sum::<usize>(),
        )
        .saturating_add(
            snapshot
                .accepted_endpoint_observations
                .values()
                .map(Vec::len)
                .sum::<usize>(),
        );
    ensure!(
        publication_record_count <= MAX_RUNTIME_TICKET_PUBLICATION_RECORDS,
        "runtime ticket-publication record limit exceeded"
    );
    let automation_record_count = snapshot
        .ticket_automation_policies
        .values()
        .map(Vec::len)
        .sum::<usize>()
        .saturating_add(
            snapshot
                .ticket_automation_attempts
                .values()
                .map(Vec::len)
                .sum::<usize>(),
        );
    ensure!(
        automation_record_count <= MAX_RUNTIME_TICKET_AUTOMATION_RECORDS,
        "runtime ticket-automation record limit exceeded"
    );
    for states in snapshot.retries.values_mut() {
        states.sort_by_key(SignedRuntimeRetryState::generation);
        let mut previous = None;
        for state in states.iter() {
            state.verify(previous)?;
            previous = Some(state);
        }
    }
    snapshot
        .device_directory_receipts
        .sort_by_key(SignedRuntimeDeviceDirectoryReceipt::generation);
    let mut previous = None;
    for receipt in &snapshot.device_directory_receipts {
        receipt.verify(previous)?;
        previous = Some(receipt);
    }
    for (channel_id, publications) in &mut snapshot.ticket_publications {
        publications.sort_by_key(SignedTicketPublication::generation);
        let mut remaining = publications.iter();
        let mut previous = if let Some((generation, record_id)) = snapshot
            .ticket_checkpoint
            .as_ref()
            .and_then(|checkpoint| checkpoint.publication_anchor(*channel_id))
        {
            let head = remaining
                .next()
                .context("runtime ticket checkpoint publication anchor is absent")?;
            head.verify_signature()?;
            ensure!(
                head.generation() == generation && head.publication_id()? == record_id,
                "runtime ticket checkpoint publication anchor does not match retained head"
            );
            Some(head)
        } else {
            None
        };
        for publication in remaining {
            publication.verify(previous)?;
            previous = Some(publication);
        }
    }
    for (channel_id, observations) in &mut snapshot.ticket_observations {
        observations.sort_by_key(SignedTicketPublicationObservation::observation_generation);
        let mut remaining = observations.iter();
        let mut previous = if let Some((generation, publication_generation, record_id)) = snapshot
            .ticket_checkpoint
            .as_ref()
            .and_then(|checkpoint| checkpoint.observation_anchor(*channel_id))
        {
            let head = remaining
                .next()
                .context("runtime ticket checkpoint observation anchor is absent")?;
            head.verify_signature()?;
            ensure!(
                head.observation_generation() == generation
                    && head.publication_generation() == publication_generation
                    && head.observation_id()? == record_id,
                "runtime ticket checkpoint observation anchor does not match retained head"
            );
            Some(head)
        } else {
            None
        };
        for observation in remaining {
            observation.verify(previous)?;
            previous = Some(observation);
        }
    }
    for (contact_id, policies) in &mut snapshot.ticket_automation_policies {
        policies.sort_by_key(SignedTicketAutomationPolicy::generation);
        let mut remaining = policies.iter();
        let mut previous = if let Some((generation, record_id)) = snapshot
            .ticket_checkpoint
            .as_ref()
            .and_then(|checkpoint| checkpoint.policy_anchor(*contact_id))
        {
            let head = remaining
                .next()
                .context("runtime ticket checkpoint policy anchor is absent")?;
            head.verify_signature()?;
            ensure!(
                head.generation() == generation && head.policy_id()? == record_id,
                "runtime ticket checkpoint policy anchor does not match retained head"
            );
            Some(head)
        } else {
            None
        };
        for policy in remaining {
            policy.verify(previous)?;
            previous = Some(policy);
        }
    }
    for ((contact_id, action), attempts) in &mut snapshot.ticket_automation_attempts {
        attempts.sort_by_key(SignedTicketAutomationAttempt::generation);
        let mut remaining = attempts.iter();
        let mut previous = if let Some((generation, policy_generation, record_id)) = snapshot
            .ticket_checkpoint
            .as_ref()
            .and_then(|checkpoint| checkpoint.attempt_anchor(*contact_id, *action))
        {
            let head = remaining
                .next()
                .context("runtime ticket checkpoint attempt anchor is absent")?;
            head.verify_signature()?;
            ensure!(
                head.generation() == generation
                    && head.policy_generation() == policy_generation
                    && head.attempt_id()? == record_id,
                "runtime ticket checkpoint attempt anchor does not match retained head"
            );
            Some(head)
        } else {
            None
        };
        for attempt in remaining {
            attempt.verify(previous)?;
            previous = Some(attempt);
        }
    }
    if let Some(checkpoint) = &snapshot.ticket_checkpoint {
        for anchor in checkpoint.anchors() {
            let present = match anchor {
                RuntimeTicketChainAnchor::Publication { channel_id, .. } => {
                    snapshot.ticket_publications.contains_key(channel_id)
                }
                RuntimeTicketChainAnchor::Observation { channel_id, .. } => {
                    snapshot.ticket_observations.contains_key(channel_id)
                }
                RuntimeTicketChainAnchor::Policy { contact_id, .. } => {
                    snapshot.ticket_automation_policies.contains_key(contact_id)
                }
                RuntimeTicketChainAnchor::Attempt {
                    contact_id, action, ..
                } => snapshot
                    .ticket_automation_attempts
                    .contains_key(&(*contact_id, *action)),
            };
            ensure!(
                present,
                "runtime ticket checkpoint references an absent chain"
            );
        }
    }
    for (contact_id, policies) in &snapshot.ticket_automation_policies {
        let contact = snapshot
            .contacts
            .get(contact_id)
            .context("ticket automation policy references an absent contact")?;
        for policy in policies {
            ensure!(
                policy.conversation() == contact.conversation_label()
                    && policy.peer_account_id() == contact.peer_account_id(),
                "ticket automation policy differs from its signed contact"
            );
        }
    }
    for ((contact_id, _), attempts) in &snapshot.ticket_automation_attempts {
        let policies = snapshot
            .ticket_automation_policies
            .get(contact_id)
            .context("ticket automation attempt has no policy chain")?;
        for attempt in attempts {
            let exact_policy_present = policies
                .iter()
                .any(|policy| policy.generation() == attempt.policy_generation());
            let compacted_policy_present = snapshot
                .ticket_checkpoint
                .as_ref()
                .and_then(|checkpoint| checkpoint.policy_anchor(*contact_id))
                .is_some_and(|(generation, _)| generation >= attempt.policy_generation());
            ensure!(
                exact_policy_present || compacted_policy_present,
                "ticket automation attempt references an absent policy generation"
            );
        }
    }
    for queued in snapshot.queued.values() {
        let contact = snapshot
            .contacts
            .get(&queued.contact_id())
            .context("runtime queue references an absent contact")?;
        ensure!(
            queued.peer_account_id() == contact.peer_account_id()
                && queued.conversation_id() == contact.conversation_id(),
            "runtime queue metadata differs from its contact"
        );
    }
    for (queue_id, materialized) in &snapshot.materialized {
        let queued = snapshot
            .queued
            .get(queue_id)
            .context("runtime materialization references an absent queue record")?;
        ensure!(
            materialized.event().event().author_device_id() == local_device_id
                && materialized.event().event().conversation_id() == queued.conversation_id(),
            "runtime materialized event does not match its queue record"
        );
    }
    for (queue_id, delivered) in &snapshot.delivered {
        let materialized = snapshot
            .materialized
            .get(queue_id)
            .context("runtime delivery marker has no materialized event")?;
        ensure!(
            materialized.event().event().event_id()? == delivered.event_id(),
            "runtime delivery marker references a different event"
        );
    }
    Ok(snapshot)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeTicketCompactionReport {
    checkpoint_generation: u64,
    removed_records: usize,
    retained_anchors: usize,
    compacted_total_records: u64,
}

fn runtime_ticket_state_needs_compaction(snapshot: &RuntimeStateSnapshot) -> bool {
    snapshot
        .ticket_publications
        .values()
        .any(|records| records.len() > MAX_RUNTIME_TICKET_CHAIN_RECORDS_BEFORE_COMPACTION)
        || snapshot
            .ticket_observations
            .values()
            .any(|records| records.len() > MAX_RUNTIME_TICKET_CHAIN_RECORDS_BEFORE_COMPACTION)
        || snapshot
            .ticket_automation_policies
            .values()
            .any(|records| records.len() > MAX_RUNTIME_TICKET_CHAIN_RECORDS_BEFORE_COMPACTION)
        || snapshot
            .ticket_automation_attempts
            .values()
            .any(|records| records.len() > MAX_RUNTIME_TICKET_CHAIN_RECORDS_BEFORE_COMPACTION)
}

fn canonical_runtime_record_name(path: &Path) -> Result<String> {
    let components = path
        .iter()
        .map(|component| {
            component
                .to_str()
                .context("runtime compaction path is not UTF-8")
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(!components.is_empty(), "runtime compaction path is empty");
    Ok(components.join("/"))
}

fn add_runtime_ticket_compaction_record(
    state_directory: &Path,
    records: &mut Vec<(PathBuf, Vec<u8>)>,
    relative_path: PathBuf,
    expected_bytes: Vec<u8>,
) -> Result<()> {
    let actual = fs::read(state_directory.join(&relative_path)).with_context(|| {
        format!(
            "read runtime ticket record selected for compaction {}",
            relative_path.display()
        )
    })?;
    ensure!(
        actual == expected_bytes,
        "runtime ticket shadow differs from authenticated DB-primary record selected for compaction: {}",
        relative_path.display()
    );
    records.push((relative_path, actual));
    Ok(())
}

fn compact_runtime_ticket_state_if_needed(
    state_directory: &Path,
) -> Result<Option<RuntimeTicketCompactionReport>> {
    let device_state = load_command_device_state(state_directory)?;
    let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
    let certificate = trust.load_certificate()?;
    let snapshot = load_runtime_state_snapshot(
        state_directory,
        certificate.account_id(),
        device_state.identity().device_id(),
    )?;
    if !runtime_ticket_state_needs_compaction(&snapshot) {
        return Ok(None);
    }

    let mut anchors = Vec::new();
    let mut removed = Vec::<(PathBuf, Vec<u8>)>::new();
    for records in snapshot.ticket_publications.values() {
        let head = records
            .last()
            .context("runtime ticket publication chain is empty")?;
        anchors.push(RuntimeTicketChainAnchor::publication(head)?);
        for record in &records[..records.len() - 1] {
            add_runtime_ticket_compaction_record(
                state_directory,
                &mut removed,
                runtime_ticket_publication_relative_path(
                    record.channel_id(),
                    record.generation(),
                    record.publication_id()?,
                ),
                record.encode()?,
            )?;
        }
    }
    for records in snapshot.ticket_observations.values() {
        let head = records
            .last()
            .context("runtime ticket observation chain is empty")?;
        anchors.push(RuntimeTicketChainAnchor::observation(head)?);
        for record in &records[..records.len() - 1] {
            add_runtime_ticket_compaction_record(
                state_directory,
                &mut removed,
                runtime_ticket_observation_relative_path(
                    record.channel_id(),
                    record.observation_generation(),
                    record.observation_id()?,
                ),
                record.encode()?,
            )?;
        }
    }
    for records in snapshot.ticket_automation_policies.values() {
        let head = records
            .last()
            .context("runtime ticket automation policy chain is empty")?;
        anchors.push(RuntimeTicketChainAnchor::policy(head)?);
        for record in &records[..records.len() - 1] {
            add_runtime_ticket_compaction_record(
                state_directory,
                &mut removed,
                runtime_ticket_automation_policy_relative_path(record.policy_id()?),
                record.encode()?,
            )?;
        }
    }
    for records in snapshot.ticket_automation_attempts.values() {
        let head = records
            .last()
            .context("runtime ticket automation attempt chain is empty")?;
        anchors.push(RuntimeTicketChainAnchor::attempt(head)?);
        for record in &records[..records.len() - 1] {
            add_runtime_ticket_compaction_record(
                state_directory,
                &mut removed,
                runtime_ticket_automation_attempt_relative_path(record.attempt_id()?),
                record.encode()?,
            )?;
        }
    }
    if let Some(previous) = &snapshot.ticket_checkpoint {
        add_runtime_ticket_compaction_record(
            state_directory,
            &mut removed,
            runtime_ticket_checkpoint_relative_path(previous.checkpoint_id()?),
            previous.encode()?,
        )?;
    }
    ensure!(
        !removed.is_empty(),
        "runtime ticket compaction trigger selected no removable records"
    );
    let compacted_records = removed
        .iter()
        .map(|(path, bytes)| {
            Ok((
                canonical_runtime_record_name(path)?,
                *blake3::hash(bytes).as_bytes(),
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let checkpoint = SignedRuntimeTicketCheckpoint::sign(
        device_state.identity(),
        certificate.account_id(),
        unix_time_now()?,
        &compacted_records,
        anchors,
        snapshot.ticket_checkpoint.as_ref(),
    )?;
    let checkpoint_id = checkpoint.checkpoint_id()?;
    let checkpoint_path = runtime_ticket_checkpoint_relative_path(checkpoint_id);
    let checkpoint_bytes = checkpoint.encode()?;
    run_state_transaction(state_directory, |transaction| {
        let outcome = persist_runtime_record(
            state_directory,
            &checkpoint_path,
            &checkpoint_bytes,
            transaction,
        )?;
        ensure!(
            outcome == StoreOutcome::Inserted,
            "runtime ticket compaction checkpoint already exists"
        );
        for (relative_path, _) in &removed {
            transaction.compact_runtime_record(relative_path)?;
        }
        Ok(())
    })?;

    let verified = load_runtime_state_snapshot(
        state_directory,
        certificate.account_id(),
        device_state.identity().device_id(),
    )?;
    ensure!(
        verified
            .ticket_checkpoint
            .as_ref()
            .map(SignedRuntimeTicketCheckpoint::checkpoint_id)
            .transpose()?
            == Some(checkpoint_id),
        "runtime ticket checkpoint did not become the verified current head"
    );
    Ok(Some(RuntimeTicketCompactionReport {
        checkpoint_generation: checkpoint.generation(),
        removed_records: removed.len(),
        retained_anchors: checkpoint.anchors().len(),
        compacted_total_records: checkpoint.compacted_total_records(),
    }))
}

async fn attempt_runtime_ticket_compaction(
    state_directory: &Path,
) -> Result<Option<RuntimeTicketCompactionReport>> {
    let state_lock = acquire_runtime_state_lock(state_directory)
        .await?
        .context("runtime state lock remained busy while compacting ticket state")?;
    let result = compact_runtime_ticket_state_if_needed(state_directory);
    drop(state_lock);
    result
}

fn load_runtime_contact_ticket(
    contact: &SignedRuntimeContact,
    local_certificate: &DeviceCertificate,
    local_authority: &AccountAuthoritySnapshot,
) -> Result<ConnectionTicket> {
    load_runtime_endpoint_ticket(
        contact.peer_account_id(),
        contact.peer_device_id(),
        contact.route_policy(),
        contact.descriptor_file(),
        local_certificate,
        local_authority,
    )
}

#[derive(Clone, Eq, PartialEq)]
struct RuntimeEndpointEnrollment {
    peer_account_id: AccountId,
    peer_device_id: DeviceId,
    route_policy: RoutePolicy,
    descriptor_file: PathBuf,
    primary: bool,
}

fn runtime_endpoint_publication_binding_id(
    local_account_id: AccountId,
    contact_id: RuntimeContactId,
    peer_device_id: DeviceId,
) -> RuntimeEndpointCandidateId {
    RuntimeEndpointCandidateId::for_endpoint(local_account_id, contact_id, peer_device_id)
}

fn load_authenticated_runtime_endpoint_ticket_for_binding(
    enrollment: &RuntimeEndpointEnrollment,
    local_certificate: &DeviceCertificate,
    local_authority: &AccountAuthoritySnapshot,
) -> Result<ConnectionTicket> {
    let encoded = fs::read_to_string(&enrollment.descriptor_file).with_context(|| {
        format!(
            "read runtime peer descriptor {}",
            enrollment.descriptor_file.display()
        )
    })?;
    ensure!(
        encoded.len() <= MAX_RUNTIME_RECORD_BYTES,
        "runtime peer descriptor is too large"
    );
    let ticket = ConnectionTicket::decode_authenticated(&encoded)
        .context("authenticate runtime peer descriptor for publication binding")?;
    ticket.verify_authenticated_listener_account(enrollment.peer_account_id)?;
    let peer = ticket
        .verify_authenticated_listener_authorization(enrollment.peer_account_id)
        .context("authenticate publication-binding peer authorization")?;
    ensure!(
        peer.device_id() == enrollment.peer_device_id,
        "runtime descriptor names a different peer device"
    );
    ensure!(
        ticket.route_policy() == enrollment.route_policy,
        "runtime descriptor route policy changed"
    );
    ensure!(
        ticket.allowed_requester_account_id() == local_certificate.account_id(),
        "runtime descriptor does not authorize this local account"
    );
    verify_device_authorization_with_snapshot(
        ticket.allowed_requester_account_id(),
        local_certificate,
        local_authority,
        &DeviceCapability::MESSAGING,
    )
    .context("local device is not authorized by the runtime descriptor")?;
    Ok(ticket)
}

fn sign_runtime_endpoint_publication_binding(
    identity: &DeviceIdentity,
    local_account_id: AccountId,
    contact: &SignedRuntimeContact,
    enrollment: &RuntimeEndpointEnrollment,
    ticket_publication_write_key: TicketPublicationWriteKey,
) -> Result<SignedRuntimeEndpointPublicationBinding> {
    SignedRuntimeEndpointPublicationBinding::sign(
        identity,
        local_account_id,
        contact.contact_id(),
        enrollment.peer_account_id,
        enrollment.peer_device_id,
        contact.conversation_id(),
        enrollment.route_policy,
        enrollment.descriptor_file.clone(),
        ticket_publication_write_key,
    )
}

fn exact_runtime_endpoint_publication_binding<'a>(
    snapshot: &'a RuntimeStateSnapshot,
    local_account_id: AccountId,
    contact: &SignedRuntimeContact,
    enrollment: &RuntimeEndpointEnrollment,
) -> Result<&'a SignedRuntimeEndpointPublicationBinding> {
    let binding_id = runtime_endpoint_publication_binding_id(
        local_account_id,
        contact.contact_id(),
        enrollment.peer_device_id,
    );
    let binding = snapshot
        .endpoint_publication_bindings
        .get(&binding_id)
        .context("runtime endpoint has no durable publication-channel binding")?;
    ensure!(
        binding.contact_id() == contact.contact_id()
            && binding.peer_account_id() == enrollment.peer_account_id
            && binding.peer_device_id() == enrollment.peer_device_id
            && binding.conversation_id() == contact.conversation_id()
            && binding.route_policy() == enrollment.route_policy
            && binding.descriptor_file() == &enrollment.descriptor_file,
        "runtime endpoint-publication binding changed its endpoint contract"
    );
    Ok(binding)
}

fn verify_runtime_endpoint_publication_binding_if_present(
    snapshot: &RuntimeStateSnapshot,
    local_account_id: AccountId,
    contact: &SignedRuntimeContact,
    enrollment: &RuntimeEndpointEnrollment,
    ticket: &ConnectionTicket,
) -> Result<()> {
    let binding_id = runtime_endpoint_publication_binding_id(
        local_account_id,
        contact.contact_id(),
        enrollment.peer_device_id,
    );
    let Some(binding) = snapshot.endpoint_publication_bindings.get(&binding_id) else {
        return Ok(());
    };
    exact_runtime_endpoint_publication_binding(snapshot, local_account_id, contact, enrollment)?;
    ensure!(
        ticket.ticket_publication_write_key() == binding.ticket_publication_write_key(),
        "runtime descriptor changes the pinned publication-channel binding"
    );
    Ok(())
}

fn exact_runtime_endpoint_enrollment(
    snapshot: &RuntimeStateSnapshot,
    contact: &SignedRuntimeContact,
    peer_device_id: DeviceId,
) -> Result<RuntimeEndpointEnrollment> {
    runtime_endpoint_enrollments(snapshot, contact)?
        .into_iter()
        .find(|candidate| candidate.peer_device_id == peer_device_id)
        .with_context(|| {
            format!(
                "runtime endpoint candidate {peer_device_id} is no longer enrolled for contact {}",
                contact.contact_id()
            )
        })
}

fn runtime_endpoint_enrollments(
    snapshot: &RuntimeStateSnapshot,
    contact: &SignedRuntimeContact,
) -> Result<Vec<RuntimeEndpointEnrollment>> {
    let mut enrollments = vec![RuntimeEndpointEnrollment {
        peer_account_id: contact.peer_account_id(),
        peer_device_id: contact.peer_device_id(),
        route_policy: contact.route_policy(),
        descriptor_file: contact.descriptor_file().clone(),
        primary: true,
    }];
    enrollments.extend(
        snapshot
            .endpoint_candidates
            .values()
            .filter(|candidate| candidate.contact_id() == contact.contact_id())
            .map(|candidate| RuntimeEndpointEnrollment {
                peer_account_id: candidate.peer_account_id(),
                peer_device_id: candidate.peer_device_id(),
                route_policy: candidate.route_policy(),
                descriptor_file: candidate.descriptor_file().clone(),
                primary: false,
            }),
    );
    ensure!(
        enrollments.len() <= MAX_RUNTIME_ENDPOINT_CANDIDATES_PER_CONTACT,
        "runtime contact exceeds the bounded endpoint-candidate limit of {MAX_RUNTIME_ENDPOINT_CANDIDATES_PER_CONTACT}"
    );
    enrollments.sort_by(|left, right| {
        right.primary.cmp(&left.primary).then_with(|| {
            left.peer_device_id
                .as_bytes()
                .cmp(right.peer_device_id.as_bytes())
        })
    });
    Ok(enrollments)
}

fn collect_runtime_endpoint_candidate_statuses(
    snapshot: &RuntimeStateSnapshot,
    contact: &SignedRuntimeContact,
    local_certificate: &DeviceCertificate,
    local_authority: &AccountAuthoritySnapshot,
    pinned_peer_authority: &AccountAuthoritySnapshot,
) -> Result<Vec<RuntimeIpcEndpointCandidateStatus>> {
    let enrollments = runtime_endpoint_enrollments(snapshot, contact)?;
    let mut loaded = enrollments
        .into_iter()
        .map(|enrollment| {
            let ticket = load_runtime_endpoint_ticket(
                enrollment.peer_account_id,
                enrollment.peer_device_id,
                enrollment.route_policy,
                &enrollment.descriptor_file,
                local_certificate,
                local_authority,
            )
            .and_then(|ticket| {
                verify_runtime_endpoint_publication_binding_if_present(
                    snapshot,
                    local_certificate.account_id(),
                    contact,
                    &enrollment,
                    &ticket,
                )?;
                Ok(ticket)
            });
            (enrollment, ticket)
        })
        .collect::<Vec<_>>();
    let newest_revision = loaded
        .iter()
        .filter_map(|(_, ticket)| {
            ticket
                .as_ref()
                .ok()
                .map(|ticket| ticket.listener_authority_snapshot().revision())
        })
        .max();
    let newest_authorities = loaded
        .iter()
        .filter_map(|(_, ticket)| ticket.as_ref().ok())
        .filter(|ticket| Some(ticket.listener_authority_snapshot().revision()) == newest_revision)
        .filter_map(|ticket| ticket.listener_authority_snapshot().encode().ok())
        .collect::<BTreeSet<_>>();
    let pinned_authority_bytes = pinned_peer_authority.encode()?;
    let newest_conflicts_with_pinned = newest_revision == Some(pinned_peer_authority.revision())
        && newest_authorities
            .iter()
            .any(|authority| authority != &pinned_authority_bytes);
    let authority_equivocation = newest_authorities.len() > 1 || newest_conflicts_with_pinned;
    let behind_local_high_water =
        newest_revision.is_some_and(|revision| revision < pinned_peer_authority.revision());
    let active_certificates = if authority_equivocation || behind_local_high_water {
        BTreeMap::new()
    } else {
        loaded
            .iter()
            .filter_map(|(_, ticket)| ticket.as_ref().ok())
            .find(|ticket| Some(ticket.listener_authority_snapshot().revision()) == newest_revision)
            .map(|ticket| {
                ticket
                    .listener_directory()
                    .device_list()
                    .devices()
                    .iter()
                    .map(|certificate| (certificate.device_id(), certificate.clone()))
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default()
    };

    Ok(loaded
        .drain(..)
        .map(|(enrollment, ticket)| match ticket {
            Err(error) => {
                let binding_id = runtime_endpoint_publication_binding_id(
                    local_certificate.account_id(),
                    contact.contact_id(),
                    enrollment.peer_device_id,
                );
                let channel_id = snapshot
                    .endpoint_publication_bindings
                    .get(&binding_id)
                    .map(SignedRuntimeEndpointPublicationBinding::ticket_publication_write_key)
                    .map(|write_key| write_key.channel_id());
                let observation = channel_id
                    .and_then(|channel_id| snapshot.latest_ticket_observation(channel_id));
                RuntimeIpcEndpointCandidateStatus {
                    peer_device_id: enrollment.peer_device_id,
                    primary: enrollment.primary,
                    route_policy: runtime_ipc_route_policy(enrollment.route_policy),
                    descriptor_file: enrollment.descriptor_file,
                    state: RuntimeIpcEndpointCandidateState::Stale,
                    authority_revision: None,
                    publication_channel_id: channel_id.map(|channel_id| channel_id.to_string()),
                    observed_publication_generation: observation
                        .map(SignedTicketPublicationObservation::publication_generation),
                    observed_at_unix_seconds: observation
                        .map(SignedTicketPublicationObservation::observed_at_unix_seconds),
                    detail: if channel_id.is_some() {
                        format!("descriptor-unusable-refresh-channel-pinned: {error:#}")
                    } else {
                        format!("descriptor-unusable-no-refresh-binding: {error:#}")
                    },
                }
            }
            Ok(ticket) => {
                let revision = ticket.listener_authority_snapshot().revision();
                let channel_id = ticket.ticket_publication_write_key().channel_id();
                let observation = snapshot.latest_ticket_observation(channel_id);
                let (state, detail) = if behind_local_high_water {
                    (
                        RuntimeIpcEndpointCandidateState::Stale,
                        "authority-behind-local-high-water".to_owned(),
                    )
                } else if authority_equivocation && Some(revision) == newest_revision {
                    (
                        RuntimeIpcEndpointCandidateState::Stale,
                        "authority-equivocation-detected".to_owned(),
                    )
                } else if Some(revision) != newest_revision {
                    (
                        RuntimeIpcEndpointCandidateState::Stale,
                        "authority-refresh-required".to_owned(),
                    )
                } else if !active_certificates
                    .get(&enrollment.peer_device_id)
                    .is_some_and(|certificate| certificate == &ticket.content.listener_certificate)
                {
                    (
                        RuntimeIpcEndpointCandidateState::Stale,
                        "device-not-active-in-current-authority".to_owned(),
                    )
                } else {
                    (
                        RuntimeIpcEndpointCandidateState::Usable,
                        "authenticated-current-authority".to_owned(),
                    )
                };
                RuntimeIpcEndpointCandidateStatus {
                    peer_device_id: enrollment.peer_device_id,
                    primary: enrollment.primary,
                    route_policy: runtime_ipc_route_policy(enrollment.route_policy),
                    descriptor_file: enrollment.descriptor_file,
                    state,
                    authority_revision: Some(revision),
                    publication_channel_id: Some(channel_id.to_string()),
                    observed_publication_generation: observation
                        .map(SignedTicketPublicationObservation::publication_generation),
                    observed_at_unix_seconds: observation
                        .map(SignedTicketPublicationObservation::observed_at_unix_seconds),
                    detail,
                }
            }
        })
        .collect())
}

fn load_runtime_endpoint_ticket(
    peer_account_id: AccountId,
    peer_device_id: DeviceId,
    route_policy: RoutePolicy,
    descriptor_file: &Path,
    local_certificate: &DeviceCertificate,
    local_authority: &AccountAuthoritySnapshot,
) -> Result<ConnectionTicket> {
    let encoded = fs::read_to_string(descriptor_file)
        .with_context(|| format!("read runtime peer descriptor {}", descriptor_file.display()))?;
    ensure!(
        encoded.len() <= MAX_RUNTIME_RECORD_BYTES,
        "runtime peer descriptor is too large"
    );
    let ticket = ConnectionTicket::decode(&encoded).context("decode runtime peer descriptor")?;
    ticket.verify_listener_account(peer_account_id)?;
    let peer = ticket.verify_listener_authorization(peer_account_id)?;
    ensure!(
        peer.device_id() == peer_device_id,
        "runtime descriptor names a different peer device"
    );
    ensure!(
        ticket.route_policy() == route_policy,
        "runtime descriptor route policy changed"
    );
    ensure!(
        ticket.allowed_requester_account_id() == local_certificate.account_id(),
        "runtime descriptor does not authorize this local account"
    );
    verify_device_authorization_with_snapshot(
        ticket.allowed_requester_account_id(),
        local_certificate,
        local_authority,
        &DeviceCapability::MESSAGING,
    )
    .context("local device is not authorized by the runtime descriptor")?;
    Ok(ticket)
}

#[derive(Clone)]
struct ResolvedRuntimeEndpointCandidate {
    ticket: ConnectionTicket,
    peer_device_id: DeviceId,
    primary: bool,
    authority_current: bool,
}

fn load_runtime_endpoint_candidate_set(
    snapshot: &RuntimeStateSnapshot,
    contact: &SignedRuntimeContact,
    local_certificate: &DeviceCertificate,
    local_authority: &AccountAuthoritySnapshot,
) -> Result<Vec<ResolvedRuntimeEndpointCandidate>> {
    let mut candidates = Vec::new();
    let mut rejected = Vec::new();
    let primary_enrollment = RuntimeEndpointEnrollment {
        peer_account_id: contact.peer_account_id(),
        peer_device_id: contact.peer_device_id(),
        route_policy: contact.route_policy(),
        descriptor_file: contact.descriptor_file().clone(),
        primary: true,
    };
    match load_runtime_contact_ticket(contact, local_certificate, local_authority).and_then(
        |ticket| {
            verify_runtime_endpoint_publication_binding_if_present(
                snapshot,
                local_certificate.account_id(),
                contact,
                &primary_enrollment,
                &ticket,
            )?;
            Ok(ticket)
        },
    ) {
        Ok(ticket) => candidates.push(ResolvedRuntimeEndpointCandidate {
            ticket,
            peer_device_id: contact.peer_device_id(),
            primary: true,
            authority_current: false,
        }),
        Err(error) => rejected.push(format!("{}: {error:#}", contact.peer_device_id())),
    }
    for candidate in snapshot
        .endpoint_candidates
        .values()
        .filter(|candidate| candidate.contact_id() == contact.contact_id())
    {
        let enrollment = RuntimeEndpointEnrollment {
            peer_account_id: candidate.peer_account_id(),
            peer_device_id: candidate.peer_device_id(),
            route_policy: candidate.route_policy(),
            descriptor_file: candidate.descriptor_file().clone(),
            primary: false,
        };
        match load_runtime_endpoint_ticket(
            candidate.peer_account_id(),
            candidate.peer_device_id(),
            candidate.route_policy(),
            candidate.descriptor_file(),
            local_certificate,
            local_authority,
        )
        .and_then(|ticket| {
            verify_runtime_endpoint_publication_binding_if_present(
                snapshot,
                local_certificate.account_id(),
                contact,
                &enrollment,
                &ticket,
            )?;
            Ok(ticket)
        }) {
            Ok(ticket) => candidates.push(ResolvedRuntimeEndpointCandidate {
                ticket,
                peer_device_id: candidate.peer_device_id(),
                primary: false,
                authority_current: false,
            }),
            Err(error) => rejected.push(format!("{}: {error:#}", candidate.peer_device_id())),
        }
    }
    ensure!(
        candidates.len() <= MAX_RUNTIME_ENDPOINT_CANDIDATES_PER_CONTACT,
        "runtime contact exceeds the bounded endpoint-candidate limit of {MAX_RUNTIME_ENDPOINT_CANDIDATES_PER_CONTACT}"
    );
    candidates.sort_by(|left, right| {
        right
            .ticket
            .listener_authority_snapshot()
            .revision()
            .cmp(&left.ticket.listener_authority_snapshot().revision())
            .then_with(|| right.primary.cmp(&left.primary))
            .then_with(|| {
                left.peer_device_id
                    .as_bytes()
                    .cmp(right.peer_device_id.as_bytes())
            })
    });
    let authoritative = candidates
        .first()
        .context("runtime endpoint candidate set is empty")?;
    let authority_revision = authoritative
        .ticket
        .listener_authority_snapshot()
        .revision();
    let authority_bytes = authoritative
        .ticket
        .listener_authority_snapshot()
        .encode()
        .context("encode authoritative endpoint-candidate snapshot")?;
    let active_certificates = authoritative
        .ticket
        .listener_directory()
        .device_list()
        .devices()
        .iter()
        .map(|certificate| (certificate.device_id(), certificate.clone()))
        .collect::<BTreeMap<_, _>>();
    for candidate in &mut candidates {
        let candidate_revision = candidate.ticket.listener_authority_snapshot().revision();
        if candidate_revision == authority_revision {
            ensure!(
                candidate
                    .ticket
                    .listener_authority_snapshot()
                    .encode()
                    .is_ok_and(|encoded| encoded == authority_bytes),
                "endpoint candidates equivocate at the same authority revision"
            );
            candidate.authority_current = true;
        }
    }
    candidates.retain(|candidate| {
        active_certificates
            .get(&candidate.peer_device_id)
            .is_some_and(|certificate| {
                certificate == &candidate.ticket.content.listener_certificate
            })
    });
    ensure!(
        !candidates.is_empty(),
        "no authenticated endpoint candidate is usable: {}",
        rejected.join(" | ")
    );
    for failure in rejected {
        eprintln!("runtime_endpoint_candidate_status=rejected error={failure}");
    }
    Ok(candidates)
}

fn add_runtime_contact(
    state_directory: PathBuf,
    conversation: String,
    expected_peer_account_id: AccountId,
    descriptor_file: PathBuf,
) -> Result<()> {
    let receipt = add_runtime_contact_record(
        &state_directory,
        conversation,
        expected_peer_account_id,
        descriptor_file,
    )?;
    println!("runtime_contact_id={}", receipt.contact_id);
    println!("peer_account_id={}", receipt.peer_account_id);
    println!("peer_device_id={}", receipt.peer_device_id);
    println!(
        "endpoint_candidate_count={}",
        receipt.endpoint_candidate_count
    );
    println!(
        "endpoint_candidate_added={}",
        receipt.endpoint_candidate_added
    );
    println!("conversation_id={}", receipt.conversation_id);
    println!("route_policy={}", receipt.route_policy.as_str());
    println!(
        "runtime_contact_store={}",
        if receipt.inserted {
            "Inserted"
        } else {
            "AlreadyPresent"
        }
    );
    println!("status=runtime-contact-ready");
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeContactReceipt {
    contact_id: RuntimeContactId,
    peer_account_id: AccountId,
    peer_device_id: DeviceId,
    conversation_id: ConversationId,
    route_policy: RoutePolicy,
    endpoint_candidate_count: usize,
    endpoint_candidate_added: bool,
    inserted: bool,
}

fn add_runtime_contact_record(
    state_directory: &Path,
    conversation: String,
    expected_peer_account_id: AccountId,
    descriptor_file: PathBuf,
) -> Result<RuntimeContactReceipt> {
    let device_state = load_command_device_state(state_directory)?;
    let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
    let local_certificate = trust.load_certificate()?;
    let local_authority = trust.load_own_authority_snapshot(&local_certificate)?;
    let conversation_id = ConversationId::from_label(&conversation);
    let membership = trust.load_conversation_membership(conversation_id.scope_id())?;
    require_conversation_participants(
        &membership,
        local_certificate.account_id(),
        expected_peer_account_id,
    )?;
    let descriptor_file = fs::canonicalize(&descriptor_file).with_context(|| {
        format!(
            "resolve runtime peer descriptor {}",
            descriptor_file.display()
        )
    })?;
    let canonical_state = fs::canonicalize(state_directory)
        .context("resolve local state directory for runtime contact")?;
    ensure!(
        !descriptor_file.starts_with(&canonical_state),
        "runtime peer descriptor must live outside the protected state directory"
    );
    let encoded = fs::read_to_string(&descriptor_file)
        .with_context(|| format!("read runtime peer descriptor {}", descriptor_file.display()))?;
    ensure!(
        encoded.len() <= MAX_RUNTIME_RECORD_BYTES,
        "runtime peer descriptor is too large"
    );
    let ticket = ConnectionTicket::decode(&encoded)?;
    ticket.verify_listener_account(expected_peer_account_id)?;
    let authorized_peer = ticket.verify_listener_authorization(expected_peer_account_id)?;
    ensure!(
        ticket.allowed_requester_account_id() == local_certificate.account_id(),
        "runtime descriptor does not authorize this local account"
    );
    verify_device_authorization_with_snapshot(
        ticket.allowed_requester_account_id(),
        &local_certificate,
        &local_authority,
        &DeviceCapability::MESSAGING,
    )
    .context("local device is not authorized by the runtime descriptor")?;
    let snapshot = load_runtime_state_snapshot(
        state_directory,
        local_certificate.account_id(),
        device_state.identity().device_id(),
    )?;
    let mut matching_contacts = snapshot.contacts.values().filter(|contact| {
        contact.peer_account_id() == expected_peer_account_id
            && contact.conversation_id() == conversation_id
            && contact.conversation_label() == conversation
    });
    if let Some(contact) = matching_contacts.next() {
        ensure!(
            matching_contacts.next().is_none(),
            "multiple runtime contacts match the peer account and conversation"
        );
        let existing_candidates = snapshot
            .endpoint_candidates
            .values()
            .filter(|candidate| candidate.contact_id() == contact.contact_id())
            .collect::<Vec<_>>();
        if authorized_peer.device_id() == contact.peer_device_id() {
            ensure!(
                contact.descriptor_file() == &descriptor_file
                    && contact.route_policy() == ticket.route_policy(),
                "primary peer device is already enrolled with another descriptor contract"
            );
            load_runtime_contact_ticket(contact, &local_certificate, &local_authority)?;
            let enrollment = RuntimeEndpointEnrollment {
                peer_account_id: contact.peer_account_id(),
                peer_device_id: contact.peer_device_id(),
                route_policy: contact.route_policy(),
                descriptor_file: contact.descriptor_file().clone(),
                primary: true,
            };
            let binding = sign_runtime_endpoint_publication_binding(
                device_state.identity(),
                local_certificate.account_id(),
                contact,
                &enrollment,
                ticket.ticket_publication_write_key(),
            )?;
            run_state_transaction(state_directory, |transaction| {
                persist_runtime_record(
                    state_directory,
                    &runtime_endpoint_publication_binding_relative_path(binding.binding_id()),
                    &binding.encode()?,
                    transaction,
                )
            })?;
            return Ok(RuntimeContactReceipt {
                contact_id: contact.contact_id(),
                peer_account_id: contact.peer_account_id(),
                peer_device_id: contact.peer_device_id(),
                conversation_id: contact.conversation_id(),
                route_policy: contact.route_policy(),
                endpoint_candidate_count: existing_candidates.len() + 1,
                endpoint_candidate_added: false,
                inserted: false,
            });
        }
        ensure!(
            existing_candidates.len() + 1 < MAX_RUNTIME_ENDPOINT_CANDIDATES_PER_CONTACT
                || existing_candidates
                    .iter()
                    .any(|candidate| candidate.peer_device_id() == authorized_peer.device_id()),
            "runtime contact already has the maximum of {MAX_RUNTIME_ENDPOINT_CANDIDATES_PER_CONTACT} endpoint candidates"
        );
        if let Some(existing) = existing_candidates
            .iter()
            .find(|candidate| candidate.peer_device_id() == authorized_peer.device_id())
        {
            ensure!(
                existing.descriptor_file() == &descriptor_file
                    && existing.route_policy() == ticket.route_policy(),
                "peer device is already enrolled with another descriptor contract"
            );
        }
        let candidate = SignedRuntimeEndpointCandidate::sign(
            device_state.identity(),
            local_certificate.account_id(),
            contact.contact_id(),
            expected_peer_account_id,
            authorized_peer.device_id(),
            conversation_id,
            ticket.route_policy(),
            descriptor_file,
        )?;
        load_runtime_endpoint_ticket(
            candidate.peer_account_id(),
            candidate.peer_device_id(),
            candidate.route_policy(),
            candidate.descriptor_file(),
            &local_certificate,
            &local_authority,
        )?;
        let enrollment = RuntimeEndpointEnrollment {
            peer_account_id: candidate.peer_account_id(),
            peer_device_id: candidate.peer_device_id(),
            route_policy: candidate.route_policy(),
            descriptor_file: candidate.descriptor_file().clone(),
            primary: false,
        };
        let binding = sign_runtime_endpoint_publication_binding(
            device_state.identity(),
            local_certificate.account_id(),
            contact,
            &enrollment,
            ticket.ticket_publication_write_key(),
        )?;
        let outcome = run_state_transaction(state_directory, |transaction| {
            transaction.prepare_trust_workspace()?;
            device_state
                .pin_peer_authority_snapshot(ticket.listener_authority_snapshot())
                .context("pin endpoint-candidate peer authority in DB-primary trust workspace")?;
            transaction
                .load_ratchet_state()?
                .observe_prekey_directory(ticket.listener_directory(), unix_time_now()?)?;
            let outcome = persist_runtime_record(
                state_directory,
                &runtime_endpoint_candidate_relative_path(candidate.candidate_id()),
                &candidate.encode()?,
                transaction,
            )?;
            persist_runtime_record(
                state_directory,
                &runtime_endpoint_publication_binding_relative_path(binding.binding_id()),
                &binding.encode()?,
                transaction,
            )?;
            Ok(outcome)
        })?;
        return Ok(RuntimeContactReceipt {
            contact_id: contact.contact_id(),
            peer_account_id: contact.peer_account_id(),
            peer_device_id: candidate.peer_device_id(),
            conversation_id: contact.conversation_id(),
            route_policy: candidate.route_policy(),
            endpoint_candidate_count: existing_candidates.len()
                + usize::from(outcome == StoreOutcome::Inserted)
                + 1,
            endpoint_candidate_added: outcome == StoreOutcome::Inserted,
            inserted: outcome == StoreOutcome::Inserted,
        });
    }
    let contact = SignedRuntimeContact::sign(
        device_state.identity(),
        local_certificate.account_id(),
        expected_peer_account_id,
        authorized_peer.device_id(),
        conversation,
        conversation_id,
        ticket.route_policy(),
        descriptor_file,
    )?;
    load_runtime_contact_ticket(&contact, &local_certificate, &local_authority)?;
    let encoded = contact.encode()?;
    let enrollment = RuntimeEndpointEnrollment {
        peer_account_id: contact.peer_account_id(),
        peer_device_id: contact.peer_device_id(),
        route_policy: contact.route_policy(),
        descriptor_file: contact.descriptor_file().clone(),
        primary: true,
    };
    let binding = sign_runtime_endpoint_publication_binding(
        device_state.identity(),
        local_certificate.account_id(),
        &contact,
        &enrollment,
        ticket.ticket_publication_write_key(),
    )?;
    let outcome = run_state_transaction(state_directory, |transaction| {
        transaction.prepare_trust_workspace()?;
        device_state
            .pin_peer_authority_snapshot(ticket.listener_authority_snapshot())
            .context("pin contact peer authority in DB-primary trust workspace")?;
        transaction
            .load_ratchet_state()?
            .observe_prekey_directory(ticket.listener_directory(), unix_time_now()?)?;
        let outcome = persist_runtime_record(
            state_directory,
            &runtime_contact_relative_path(contact.contact_id()),
            &encoded,
            transaction,
        )?;
        persist_runtime_record(
            state_directory,
            &runtime_endpoint_publication_binding_relative_path(binding.binding_id()),
            &binding.encode()?,
            transaction,
        )?;
        Ok(outcome)
    })?;
    Ok(RuntimeContactReceipt {
        contact_id: contact.contact_id(),
        peer_account_id: contact.peer_account_id(),
        peer_device_id: contact.peer_device_id(),
        conversation_id: contact.conversation_id(),
        route_policy: contact.route_policy(),
        endpoint_candidate_count: 1,
        endpoint_candidate_added: true,
        inserted: outcome == StoreOutcome::Inserted,
    })
}

struct PreparedRuntimeEndpointAnnouncementExport {
    bundle_id: EndpointAnnouncementBundleId,
    source_device_id: DeviceId,
    recipient_device_id: DeviceId,
    authority_revision: u64,
    contact_count: usize,
    endpoint_count: usize,
    observation_count: usize,
    expires_at_unix_seconds: u64,
    envelope: EncryptedEndpointAnnouncementBundle,
}

fn build_runtime_endpoint_announcements(
    state_directory: &Path,
    current_ticket: &ConnectionTicket,
    recipient_device_id: DeviceId,
    validity_seconds: u64,
) -> Result<PreparedRuntimeEndpointAnnouncementExport> {
    ensure!(
        (1..=MAX_ENDPOINT_ANNOUNCEMENT_VALIDITY_SECONDS).contains(&validity_seconds),
        "endpoint announcement validity must be between 1 and {MAX_ENDPOINT_ANNOUNCEMENT_VALIDITY_SECONDS} seconds"
    );
    let device_state = load_command_device_state(state_directory)?;
    let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
    let local_certificate = trust
        .load_certificate()
        .context("load local certificate for endpoint announcement export")?;
    let local_authority = trust
        .load_own_authority_snapshot(&local_certificate)
        .context("load local authority for endpoint announcement export")?;
    let account_device_list = current_ticket.listener_directory().device_list();
    ensure!(
        current_ticket.listener_account_id() == local_certificate.account_id()
            && current_ticket.listener_device_id() == device_state.identity().device_id()
            && account_device_list.authority_snapshot() == &local_authority
            && account_device_list.certificate_for(local_certificate.device_id())
                == Some(&local_certificate),
        "running endpoint announcement authority does not match local authenticated state"
    );
    ensure!(
        recipient_device_id != local_certificate.device_id(),
        "endpoint announcements must target another device"
    );
    account_device_list
        .certificate_for(recipient_device_id)
        .context("endpoint announcement recipient is not an active device in this account")?;

    let snapshot = load_runtime_state_snapshot(
        state_directory,
        local_certificate.account_id(),
        device_state.identity().device_id(),
    )?;
    let mut announcements = Vec::with_capacity(snapshot.contacts.len());
    let mut endpoint_count = 0_usize;
    let mut observation_count = 0_usize;
    for contact in snapshot.contacts.values() {
        let membership = trust
            .load_conversation_membership(contact.conversation_id().scope_id())
            .context("load conversation membership for endpoint announcement export")?;
        require_conversation_participants(
            &membership,
            local_certificate.account_id(),
            contact.peer_account_id(),
        )?;
        let mut endpoints = Vec::new();
        for enrollment in runtime_endpoint_enrollments(&snapshot, contact)? {
            let binding = exact_runtime_endpoint_publication_binding(
                &snapshot,
                local_certificate.account_id(),
                contact,
                &enrollment,
            )?;
            let encoded = fs::read_to_string(&enrollment.descriptor_file).with_context(|| {
                format!(
                    "read endpoint announcement descriptor {}",
                    enrollment.descriptor_file.display()
                )
            })?;
            ensure!(
                encoded.len() <= MAX_RUNTIME_RECORD_BYTES,
                "endpoint announcement descriptor is too large"
            );
            let authenticated = ConnectionTicket::decode_authenticated(&encoded)
                .context("authenticate descriptor for endpoint announcement export")?;
            authenticated.verify_authenticated_listener_account(enrollment.peer_account_id)?;
            let peer = authenticated
                .verify_authenticated_listener_authorization(enrollment.peer_account_id)?;
            ensure!(
                peer.device_id() == enrollment.peer_device_id
                    && authenticated.allowed_requester_account_id()
                        == local_certificate.account_id()
                    && authenticated.route_policy() == enrollment.route_policy
                    && authenticated.ticket_publication_write_key()
                        == binding.ticket_publication_write_key(),
                "endpoint announcement descriptor changed its authenticated endpoint contract"
            );
            let channel_id = binding.ticket_publication_write_key().channel_id();
            let observation = snapshot.latest_ticket_observation(channel_id).cloned();
            observation_count += usize::from(observation.is_some());
            endpoint_count += 1;
            endpoints.push(EndpointCandidateAnnouncement::new(
                enrollment.peer_device_id,
                enrollment.primary,
                enrollment.route_policy,
                binding.ticket_publication_write_key(),
                encoded,
                observation,
            )?);
        }
        announcements.push(ContactEndpointAnnouncement::new(
            contact.conversation_label().to_owned(),
            contact.peer_account_id(),
            endpoints,
        )?);
    }
    let now_unix_seconds = unix_time_now()?;
    let bundle = SignedEndpointAnnouncementBundle::sign(
        device_state.identity(),
        account_device_list.clone(),
        recipient_device_id,
        now_unix_seconds,
        validity_seconds,
        announcements,
    )?;
    let bundle_id = bundle.bundle_id()?;
    let envelope = EncryptedEndpointAnnouncementBundle::seal(&bundle)?;
    Ok(PreparedRuntimeEndpointAnnouncementExport {
        bundle_id,
        source_device_id: bundle.source_device_id(),
        recipient_device_id,
        authority_revision: account_device_list.revision(),
        contact_count: bundle.contacts().len(),
        endpoint_count,
        observation_count,
        expires_at_unix_seconds: bundle.expires_at_unix_seconds(),
        envelope,
    })
}

fn export_runtime_endpoint_announcements(
    state_directory: &Path,
    current_ticket: &ConnectionTicket,
    recipient_device_id: DeviceId,
    output_file: PathBuf,
    validity_seconds: u64,
) -> Result<RuntimeIpcEndpointAnnouncementExport> {
    let prepared = build_runtime_endpoint_announcements(
        state_directory,
        current_ticket,
        recipient_device_id,
        validity_seconds,
    )?;
    let output_file = absolute_new_external_path(state_directory, &output_file)?;
    write_new_authority_file(&output_file, &prepared.envelope.encode()?).with_context(|| {
        format!(
            "write recipient-encrypted endpoint announcement to {}",
            output_file.display()
        )
    })?;
    Ok(RuntimeIpcEndpointAnnouncementExport {
        bundle_id: prepared.bundle_id.to_string(),
        source_device_id: prepared.source_device_id,
        recipient_device_id: prepared.recipient_device_id,
        authority_revision: prepared.authority_revision,
        contact_count: prepared.contact_count,
        endpoint_count: prepared.endpoint_count,
        observation_count: prepared.observation_count,
        expires_at_unix_seconds: prepared.expires_at_unix_seconds,
        output_file,
        protection: "source-device-signed-recipient-device-hpke-exact-root-roster".to_owned(),
    })
}

#[derive(Debug)]
struct RuntimeEndpointAnnouncementImportPlan {
    relative_records: Vec<(PathBuf, Vec<u8>)>,
    external_descriptors: Vec<(PathBuf, Vec<u8>)>,
    fresh_tickets: Vec<ConnectionTicket>,
    contact_count: usize,
    contact_added_count: usize,
    endpoint_count: usize,
    endpoint_added_count: usize,
    publication_binding_added_count: usize,
    observation_evidence_count: usize,
}

fn import_runtime_endpoint_announcements(
    state_directory: &Path,
    current_ticket: &ConnectionTicket,
    bundle_file: &Path,
    descriptor_directory: &Path,
) -> Result<RuntimeIpcEndpointAnnouncementImport> {
    let bundle_file = fs::canonicalize(bundle_file).with_context(|| {
        format!(
            "resolve encrypted endpoint announcement {}",
            bundle_file.display()
        )
    })?;
    let metadata = fs::symlink_metadata(&bundle_file)
        .context("inspect encrypted endpoint announcement input")?;
    ensure!(
        metadata.file_type().is_file()
            && !metadata.file_type().is_symlink()
            && metadata.len() <= MAX_ENDPOINT_ANNOUNCEMENT_BYTES as u64,
        "endpoint announcement input must be a bounded regular non-symlink file"
    );
    let envelope = EncryptedEndpointAnnouncementBundle::decode(
        &fs::read(&bundle_file).context("read encrypted endpoint announcement")?,
    )?;
    import_runtime_endpoint_announcement_envelope(
        state_directory,
        current_ticket,
        envelope,
        descriptor_directory,
        None,
    )
}

fn import_runtime_endpoint_announcement_envelope(
    state_directory: &Path,
    current_ticket: &ConnectionTicket,
    envelope: EncryptedEndpointAnnouncementBundle,
    descriptor_directory: &Path,
    expected_source_device_id: Option<DeviceId>,
) -> Result<RuntimeIpcEndpointAnnouncementImport> {
    let device_state = load_command_device_state(state_directory)?;
    let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
    let local_certificate = trust
        .load_certificate()
        .context("load local certificate for endpoint announcement import")?;
    let local_authority = trust
        .load_own_authority_snapshot(&local_certificate)
        .context("load local authority for endpoint announcement import")?;
    let current_device_list = current_ticket.listener_directory().device_list();
    ensure!(
        current_ticket.listener_account_id() == local_certificate.account_id()
            && current_ticket.listener_device_id() == device_state.identity().device_id()
            && current_device_list.authority_snapshot() == &local_authority
            && current_device_list.certificate_for(local_certificate.device_id())
                == Some(&local_certificate),
        "running endpoint announcement authority does not match local authenticated state"
    );
    let now_unix_seconds = unix_time_now()?;
    let bundle = envelope.open(
        local_certificate.device_id(),
        device_state.encryption(),
        now_unix_seconds,
    )?;
    ensure!(
        bundle.account_id() == local_certificate.account_id()
            && bundle.recipient_device_id() == local_certificate.device_id(),
        "endpoint announcement belongs to another account or recipient"
    );
    ensure!(
        bundle.source_device_id() != local_certificate.device_id(),
        "endpoint announcement source must be another device"
    );
    if let Some(expected_source_device_id) = expected_source_device_id {
        ensure!(
            bundle.source_device_id() == expected_source_device_id,
            "endpoint announcement source does not match the authenticated Device session"
        );
    }
    ensure!(
        bundle.account_device_list() == current_device_list,
        "endpoint announcement does not carry the exact current Root-signed device list"
    );
    let source_certificate = current_device_list
        .certificate_for(bundle.source_device_id())
        .context("endpoint announcement source is not an active account device")?;
    verify_device_authorization_with_snapshot(
        local_certificate.account_id(),
        source_certificate,
        &local_authority,
        &DeviceCapability::MESSAGING,
    )
    .context("endpoint announcement source is no longer authorized")?;

    fs::create_dir_all(descriptor_directory).with_context(|| {
        format!(
            "create endpoint announcement descriptor directory {}",
            descriptor_directory.display()
        )
    })?;
    let unresolved_descriptor_metadata = fs::symlink_metadata(descriptor_directory)
        .context("inspect unresolved endpoint announcement descriptor directory")?;
    ensure!(
        unresolved_descriptor_metadata.file_type().is_dir()
            && !unresolved_descriptor_metadata.file_type().is_symlink(),
        "endpoint announcement descriptor path must be a regular non-symlink directory"
    );
    let descriptor_directory = fs::canonicalize(descriptor_directory).with_context(|| {
        format!(
            "resolve endpoint announcement descriptor directory {}",
            descriptor_directory.display()
        )
    })?;
    let descriptor_metadata = fs::symlink_metadata(&descriptor_directory)
        .context("inspect endpoint announcement descriptor directory")?;
    ensure!(
        descriptor_metadata.file_type().is_dir() && !descriptor_metadata.file_type().is_symlink(),
        "endpoint announcement descriptor path must be a regular non-symlink directory"
    );
    let canonical_state = fs::canonicalize(state_directory)
        .context("resolve state directory for endpoint announcement import")?;
    ensure!(
        !descriptor_directory.starts_with(&canonical_state),
        "imported endpoint descriptors must live outside the protected state directory"
    );

    let snapshot = load_runtime_state_snapshot(
        state_directory,
        local_certificate.account_id(),
        device_state.identity().device_id(),
    )?;
    let bundle_id = bundle.bundle_id()?;
    let plan = prepare_runtime_endpoint_announcement_import(
        &snapshot,
        &trust,
        &local_certificate,
        device_state.identity(),
        &bundle,
        bundle_id,
        &descriptor_directory,
    )?;

    for (path, bytes) in &plan.external_descriptors {
        write_new_or_verify_identical(path, bytes)?;
    }
    let mut evidence_added_count = 0_usize;
    run_state_transaction(state_directory, |transaction| {
        if !plan.fresh_tickets.is_empty() {
            transaction.prepare_trust_workspace()?;
        }
        for ticket in &plan.fresh_tickets {
            device_state
                .pin_peer_authority_snapshot(ticket.listener_authority_snapshot())
                .context("pin fresh announced peer authority in DB-primary trust workspace")?;
            transaction
                .load_ratchet_state()?
                .observe_prekey_directory(ticket.listener_directory(), now_unix_seconds)
                .context("observe fresh announced peer prekey directory")?;
        }
        for (relative_path, bytes) in &plan.relative_records {
            let outcome =
                persist_runtime_record(state_directory, relative_path, bytes, transaction)?;
            if relative_path
                .extension()
                .is_some_and(|extension| extension == "aeo")
                && outcome == StoreOutcome::Inserted
            {
                evidence_added_count += 1;
            }
        }
        Ok(())
    })?;
    load_runtime_state_snapshot(
        state_directory,
        local_certificate.account_id(),
        device_state.identity().device_id(),
    )
    .context("verify runtime state after endpoint announcement import")?;

    Ok(RuntimeIpcEndpointAnnouncementImport {
        bundle_id: bundle_id.to_string(),
        source_device_id: bundle.source_device_id(),
        recipient_device_id: bundle.recipient_device_id(),
        authority_revision: bundle.account_device_list().revision(),
        contact_count: plan.contact_count,
        contact_added_count: plan.contact_added_count,
        endpoint_count: plan.endpoint_count,
        endpoint_added_count: plan.endpoint_added_count,
        publication_binding_added_count: plan.publication_binding_added_count,
        observation_evidence_count: plan.observation_evidence_count,
        observation_evidence_added_count: evidence_added_count,
        descriptor_directory,
        authority_status: "exact-current-root-signed-device-list".to_owned(),
    })
}

async fn push_runtime_endpoint_announcements(
    state_directory: &Path,
    endpoint: &Endpoint,
    current_ticket: &ConnectionTicket,
    recipient_ticket_file: &Path,
    validity_seconds: u64,
) -> Result<RuntimeIpcEndpointAnnouncementPush> {
    let (recipient_ticket, prepared, envelope_bytes) = {
        let _state_lock = StateDirectoryLock::acquire(state_directory)
            .context("lock runtime state for endpoint announcement network push")?;
        let recipient_ticket_file = fs::canonicalize(recipient_ticket_file).with_context(|| {
            format!(
                "resolve endpoint announcement recipient ticket {}",
                recipient_ticket_file.display()
            )
        })?;
        let metadata = fs::symlink_metadata(&recipient_ticket_file)
            .context("inspect endpoint announcement recipient ticket")?;
        ensure!(
            metadata.file_type().is_file()
                && !metadata.file_type().is_symlink()
                && metadata.len() <= MAX_RUNTIME_RECORD_BYTES as u64,
            "endpoint announcement recipient ticket must be a bounded regular non-symlink file"
        );
        let recipient_ticket = ConnectionTicket::decode(
            &fs::read_to_string(&recipient_ticket_file)
                .context("read endpoint announcement recipient ticket")?,
        )
        .context("verify endpoint announcement recipient ticket")?;
        let local_account_id = current_ticket.listener_account_id();
        ensure!(
            recipient_ticket.listener_account_id() == local_account_id
                && recipient_ticket.allowed_requester_account_id() == local_account_id,
            "endpoint announcement recipient ticket is not a same-account Device session"
        );
        ensure!(
            recipient_ticket.listener_device_id() != current_ticket.listener_device_id(),
            "endpoint announcement recipient must be another device"
        );
        ensure!(
            recipient_ticket.listener_directory().device_list()
                == current_ticket.listener_directory().device_list(),
            "endpoint announcement recipient ticket does not carry the exact current own-device list"
        );
        recipient_ticket
            .verify_listener_authorization(local_account_id)
            .context("verify endpoint announcement recipient authorization")?;
        let prepared = build_runtime_endpoint_announcements(
            state_directory,
            current_ticket,
            recipient_ticket.listener_device_id(),
            validity_seconds,
        )?;
        let envelope_bytes = prepared.envelope.encode()?;
        ensure!(
            envelope_bytes.len() <= MAX_ENDPOINT_ANNOUNCEMENT_WIRE_BYTES,
            "endpoint announcement encrypted bundle is too large for one bounded network transfer"
        );
        (recipient_ticket, prepared, envelope_bytes)
    };

    let route_policy = recipient_ticket.route_policy();
    let connection = timeout(
        CONNECTION_TIMEOUT,
        endpoint.connect(recipient_ticket.endpoint().clone(), ALPN),
    )
    .await
    .with_context(|| {
        timeout_message(
            "connect endpoint announcement push to recipient runtime",
            CONNECTION_TIMEOUT,
        )
    })?
    .context("connect endpoint announcement push to recipient runtime")?;
    let ready_path = await_route_policy(&connection, route_policy, ROUTE_POLICY_WAIT)
        .await
        .context("wait for an endpoint announcement path allowed by the recipient ticket")?;
    let session_binding =
        SyncSessionBinding::from_transport_label(&recipient_ticket.endpoint().id.to_string());
    let device_state = load_command_device_state(state_directory)?;
    let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
    let local_certificate = trust
        .load_certificate()
        .context("load endpoint announcement source certificate")?;
    let local_authority = trust
        .load_own_authority_snapshot(&local_certificate)
        .context("load endpoint announcement source authority")?;
    ensure!(
        current_ticket
            .listener_directory()
            .device_list()
            .authority_snapshot()
            == &local_authority,
        "endpoint announcement source authority changed during network transfer"
    );
    authorize_with_listener(
        &connection,
        device_state.identity(),
        local_certificate,
        local_authority,
        session_binding,
    )
    .await?;
    let (mut send, mut receive) =
        open_bi(&connection, "open endpoint announcement push stream").await?;
    write_client_request(
        &mut send,
        &ClientRequest::EndpointAnnouncementPush(envelope_bytes.clone()),
    )
    .await?;
    let acknowledgement_bytes = match read_server_response(&mut receive).await? {
        ServerResponse::EndpointAnnouncementAcknowledged(bytes) => bytes,
        ServerResponse::EndpointAnnouncementRejected => {
            bail!("recipient rejected the endpoint announcement bundle")
        }
        _ => bail!("endpoint announcement push received an unexpected response"),
    };
    let acknowledgement =
        SignedEndpointAnnouncementAcknowledgement::decode(&acknowledgement_bytes)?;
    acknowledgement.verify_for_session(
        session_binding,
        prepared.bundle_id,
        prepared.source_device_id,
        prepared.recipient_device_id,
    )?;
    ensure!(
        acknowledgement.authority_revision() == prepared.authority_revision
            && acknowledgement.contact_count() == prepared.contact_count
            && acknowledgement.endpoint_count() == prepared.endpoint_count
            && acknowledgement.observation_evidence_count() == prepared.observation_count,
        "endpoint announcement acknowledgement does not match the transferred inventory"
    );
    let transport_path = ready_path.kind.as_str().to_owned();
    connection.close(
        0_u32.into(),
        b"kilogram endpoint announcement push complete",
    );
    Ok(RuntimeIpcEndpointAnnouncementPush {
        bundle_id: prepared.bundle_id.to_string(),
        source_device_id: prepared.source_device_id,
        recipient_device_id: prepared.recipient_device_id,
        authority_revision: prepared.authority_revision,
        contact_count: prepared.contact_count,
        endpoint_count: prepared.endpoint_count,
        observation_count: prepared.observation_count,
        encrypted_bundle_bytes: envelope_bytes.len(),
        recipient_contact_added_count: acknowledgement.contact_added_count(),
        recipient_endpoint_added_count: acknowledgement.endpoint_added_count(),
        recipient_publication_binding_added_count: acknowledgement
            .publication_binding_added_count(),
        recipient_observation_evidence_added_count: acknowledgement
            .observation_evidence_added_count(),
        transport_path,
        acknowledgement_status: "recipient-device-signed-session-bound".to_owned(),
    })
}

#[allow(clippy::too_many_arguments)]
fn prepare_runtime_endpoint_announcement_import(
    snapshot: &RuntimeStateSnapshot,
    trust: &CommandTrustReadRepository,
    local_certificate: &DeviceCertificate,
    identity: &DeviceIdentity,
    bundle: &SignedEndpointAnnouncementBundle,
    bundle_id: runtime_endpoint_announcement::EndpointAnnouncementBundleId,
    descriptor_directory: &Path,
) -> Result<RuntimeEndpointAnnouncementImportPlan> {
    let mut relative_records = Vec::new();
    let mut external_descriptors = Vec::new();
    let mut fresh_tickets = Vec::new();
    let mut contact_added_count = 0_usize;
    let mut endpoint_count = 0_usize;
    let mut endpoint_added_count = 0_usize;
    let mut publication_binding_added_count = 0_usize;
    let mut observation_evidence_count = 0_usize;
    let mut planned_contact_ids = BTreeSet::new();
    let mut planned_endpoint_ids = BTreeSet::new();
    let mut known_observations =
        BTreeMap::<(TicketPublicationChannelId, u64), (TicketPublicationId, [u8; 32])>::new();
    for observations in snapshot.ticket_observations.values() {
        for observation in observations {
            merge_runtime_publication_observation(
                &mut known_observations,
                observation.channel_id(),
                observation.publication_generation(),
                observation.publication_id(),
                observation.ticket_digest(),
            )?;
        }
    }
    for evidence in snapshot.accepted_endpoint_observations.values() {
        for observation in evidence {
            merge_runtime_publication_observation(
                &mut known_observations,
                observation.channel_id(),
                observation.publication_generation(),
                observation.publication_id(),
                observation.ticket_digest(),
            )?;
        }
    }

    for announcement in bundle.contacts() {
        let membership = trust
            .load_conversation_membership(announcement.conversation_id().scope_id())
            .context("load conversation membership for endpoint announcement import")?;
        require_conversation_participants(
            &membership,
            local_certificate.account_id(),
            announcement.peer_account_id(),
        )?;
        let existing_contact = snapshot.contacts.values().find(|contact| {
            contact.peer_account_id() == announcement.peer_account_id()
                && contact.conversation_id() == announcement.conversation_id()
                && contact.conversation_label() == announcement.conversation_label()
        });
        let primary_announcement = announcement
            .endpoints()
            .iter()
            .find(|endpoint| endpoint.primary())
            .context("endpoint announcement contact has no primary endpoint")?;
        let primary_descriptor = imported_endpoint_descriptor_path(
            descriptor_directory,
            bundle_id,
            announcement.peer_account_id(),
            announcement.conversation_id(),
            primary_announcement.peer_device_id(),
        );
        let contact = if let Some(contact) = existing_contact {
            contact.clone()
        } else {
            let contact = SignedRuntimeContact::sign(
                identity,
                local_certificate.account_id(),
                announcement.peer_account_id(),
                primary_announcement.peer_device_id(),
                announcement.conversation_label().to_owned(),
                announcement.conversation_id(),
                primary_announcement.route_policy(),
                primary_descriptor,
            )?;
            ensure!(
                planned_contact_ids.insert(contact.contact_id()),
                "endpoint announcement creates a duplicate contact"
            );
            relative_records.push((
                runtime_contact_relative_path(contact.contact_id()),
                contact.encode()?,
            ));
            contact_added_count += 1;
            contact
        };
        let existing_enrollments = if existing_contact.is_some() {
            runtime_endpoint_enrollments(snapshot, &contact)?
        } else {
            Vec::new()
        };
        let new_endpoint_count = announcement
            .endpoints()
            .iter()
            .filter(|endpoint| {
                !existing_enrollments
                    .iter()
                    .any(|value| value.peer_device_id == endpoint.peer_device_id())
            })
            .count();
        ensure!(
            existing_enrollments.len() + new_endpoint_count
                <= MAX_RUNTIME_ENDPOINT_CANDIDATES_PER_CONTACT,
            "endpoint announcement would exceed the bounded endpoint-candidate limit"
        );

        for endpoint in announcement.endpoints() {
            endpoint_count += 1;
            let authenticated = ConnectionTicket::decode_authenticated(endpoint.ticket())
                .context("authenticate imported endpoint descriptor")?;
            authenticated.verify_authenticated_listener_account(announcement.peer_account_id())?;
            let peer = authenticated
                .verify_authenticated_listener_authorization(announcement.peer_account_id())?;
            ensure!(
                peer.device_id() == endpoint.peer_device_id()
                    && authenticated.allowed_requester_account_id()
                        == local_certificate.account_id()
                    && authenticated.route_policy() == endpoint.route_policy()
                    && authenticated.ticket_publication_write_key()
                        == endpoint.ticket_publication_write_key(),
                "imported endpoint descriptor changes its announced contract"
            );

            let existing_enrollment = existing_enrollments
                .iter()
                .find(|value| value.peer_device_id == endpoint.peer_device_id());
            let enrollment = if let Some(existing) = existing_enrollment {
                ensure!(
                    existing.peer_account_id == announcement.peer_account_id()
                        && existing.route_policy == endpoint.route_policy(),
                    "announced endpoint conflicts with an existing enrollment"
                );
                existing.clone()
            } else {
                let descriptor_file = imported_endpoint_descriptor_path(
                    descriptor_directory,
                    bundle_id,
                    announcement.peer_account_id(),
                    announcement.conversation_id(),
                    endpoint.peer_device_id(),
                );
                external_descriptors.push((
                    descriptor_file.clone(),
                    endpoint.ticket().as_bytes().to_vec(),
                ));
                let primary = existing_contact.is_none() && endpoint.primary();
                if !primary {
                    let candidate = SignedRuntimeEndpointCandidate::sign(
                        identity,
                        local_certificate.account_id(),
                        contact.contact_id(),
                        announcement.peer_account_id(),
                        endpoint.peer_device_id(),
                        announcement.conversation_id(),
                        endpoint.route_policy(),
                        descriptor_file.clone(),
                    )?;
                    ensure!(
                        planned_endpoint_ids.insert(candidate.candidate_id()),
                        "endpoint announcement creates a duplicate endpoint candidate"
                    );
                    relative_records.push((
                        runtime_endpoint_candidate_relative_path(candidate.candidate_id()),
                        candidate.encode()?,
                    ));
                }
                endpoint_added_count += 1;
                RuntimeEndpointEnrollment {
                    peer_account_id: announcement.peer_account_id(),
                    peer_device_id: endpoint.peer_device_id(),
                    route_policy: endpoint.route_policy(),
                    descriptor_file,
                    primary,
                }
            };

            let binding_id = runtime_endpoint_publication_binding_id(
                local_certificate.account_id(),
                contact.contact_id(),
                endpoint.peer_device_id(),
            );
            if let Some(existing) = snapshot.endpoint_publication_bindings.get(&binding_id) {
                ensure!(
                    existing.contact_id() == contact.contact_id()
                        && existing.peer_account_id() == announcement.peer_account_id()
                        && existing.peer_device_id() == endpoint.peer_device_id()
                        && existing.conversation_id() == announcement.conversation_id()
                        && existing.route_policy() == endpoint.route_policy()
                        && existing.descriptor_file() == &enrollment.descriptor_file
                        && existing.ticket_publication_write_key()
                            == endpoint.ticket_publication_write_key(),
                    "announced endpoint conflicts with its durable publication binding"
                );
            } else {
                let binding = sign_runtime_endpoint_publication_binding(
                    identity,
                    local_certificate.account_id(),
                    &contact,
                    &enrollment,
                    endpoint.ticket_publication_write_key(),
                )?;
                relative_records.push((
                    runtime_endpoint_publication_binding_relative_path(binding.binding_id()),
                    binding.encode()?,
                ));
                publication_binding_added_count += 1;
            }

            if existing_enrollment.is_none()
                && let Ok(fresh) = ConnectionTicket::decode(endpoint.ticket())
            {
                fresh_tickets.push(fresh);
            }
            if let Some(source_observation) = endpoint.latest_observation() {
                ensure!(
                    source_observation.local_account_id() == local_certificate.account_id()
                        && source_observation.local_device_id() == bundle.source_device_id()
                        && source_observation.publisher_account_id()
                            == announcement.peer_account_id()
                        && source_observation.publisher_device_id() == endpoint.peer_device_id()
                        && source_observation.channel_id()
                            == endpoint.ticket_publication_write_key().channel_id(),
                    "announced publication observation changes endpoint identity"
                );
                merge_runtime_publication_observation(
                    &mut known_observations,
                    source_observation.channel_id(),
                    source_observation.publication_generation(),
                    source_observation.publication_id(),
                    source_observation.ticket_digest(),
                )?;
                let evidence = SignedAcceptedEndpointObservation::sign(
                    identity,
                    local_certificate.account_id(),
                    bundle.source_device_id(),
                    bundle_id,
                    bundle.created_at_unix_seconds(),
                    source_observation.clone(),
                )?;
                relative_records.push((
                    runtime_accepted_endpoint_observation_relative_path(evidence.evidence_id()?),
                    evidence.encode()?,
                ));
                observation_evidence_count += 1;
            }
        }
    }
    ensure!(
        snapshot.contacts.len() + contact_added_count <= MAX_RUNTIME_CONTACTS,
        "endpoint announcement would exceed the bounded runtime contact limit"
    );
    let existing_publication_records = snapshot
        .ticket_publications
        .values()
        .map(Vec::len)
        .sum::<usize>()
        .saturating_add(
            snapshot
                .ticket_observations
                .values()
                .map(Vec::len)
                .sum::<usize>(),
        )
        .saturating_add(
            snapshot
                .accepted_endpoint_observations
                .values()
                .map(Vec::len)
                .sum::<usize>(),
        );
    ensure!(
        existing_publication_records.saturating_add(observation_evidence_count)
            <= MAX_RUNTIME_TICKET_PUBLICATION_RECORDS,
        "endpoint announcement would exceed the bounded ticket-observation record limit"
    );
    Ok(RuntimeEndpointAnnouncementImportPlan {
        relative_records,
        external_descriptors,
        fresh_tickets,
        contact_count: bundle.contacts().len(),
        contact_added_count,
        endpoint_count,
        endpoint_added_count,
        publication_binding_added_count,
        observation_evidence_count,
    })
}

fn merge_runtime_publication_observation(
    known: &mut BTreeMap<(TicketPublicationChannelId, u64), (TicketPublicationId, [u8; 32])>,
    channel_id: TicketPublicationChannelId,
    generation: u64,
    publication_id: TicketPublicationId,
    ticket_digest: [u8; 32],
) -> Result<()> {
    if let Some((known_id, known_digest)) = known.get(&(channel_id, generation)) {
        ensure!(
            *known_id == publication_id && *known_digest == ticket_digest,
            "authorized devices equivocated at publication generation {generation}"
        );
    } else {
        known.insert((channel_id, generation), (publication_id, ticket_digest));
    }
    Ok(())
}

fn absolute_new_external_path(state_directory: &Path, path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .context("read current directory for endpoint announcement output")?
            .join(path)
    };
    let parent = absolute
        .parent()
        .context("endpoint announcement output has no parent")?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "create endpoint announcement output directory {}",
            parent.display()
        )
    })?;
    let parent = fs::canonicalize(parent).with_context(|| {
        format!(
            "resolve endpoint announcement output directory {}",
            parent.display()
        )
    })?;
    let file_name = absolute
        .file_name()
        .context("endpoint announcement output has no file name")?;
    let resolved = parent.join(file_name);
    let canonical_state = fs::canonicalize(state_directory)
        .context("resolve protected state directory for endpoint announcement output")?;
    ensure!(
        !resolved.starts_with(canonical_state),
        "endpoint announcement output must live outside the protected state directory"
    );
    Ok(resolved)
}

fn imported_endpoint_descriptor_path(
    descriptor_directory: &Path,
    bundle_id: runtime_endpoint_announcement::EndpointAnnouncementBundleId,
    peer_account_id: AccountId,
    conversation_id: ConversationId,
    peer_device_id: DeviceId,
) -> PathBuf {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"kilogram:imported-endpoint-descriptor:v1\0");
    hasher.update(peer_account_id.as_bytes());
    hasher.update(conversation_id.as_bytes());
    hasher.update(peer_device_id.as_bytes());
    hasher.update(bundle_id.to_string().as_bytes());
    descriptor_directory.join(format!("{}.ticket", hasher.finalize().to_hex()))
}

fn write_new_or_verify_identical(path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() {
        let metadata = fs::symlink_metadata(path)
            .with_context(|| format!("inspect imported endpoint descriptor {}", path.display()))?;
        ensure!(
            metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
            "imported endpoint descriptor path is not a regular non-symlink file"
        );
        ensure!(
            fs::read(path)
                .with_context(|| format!("read imported endpoint descriptor {}", path.display()))?
                == bytes,
            "imported endpoint descriptor already exists with different content"
        );
        return Ok(());
    }
    write_new_authority_file(path, bytes)
        .with_context(|| format!("write imported endpoint descriptor {}", path.display()))
}

fn queue_runtime_message(
    state_directory: PathBuf,
    conversation: String,
    peer_account_id: AccountId,
    message: String,
) -> Result<()> {
    let receipt = queue_runtime_message_with_id(
        &state_directory,
        RuntimeQueueId::generate()?,
        conversation,
        peer_account_id,
        message,
    )?;
    print_runtime_queue_receipt(&receipt);
    Ok(())
}

#[derive(Clone, Copy)]
struct RuntimeQueueReceipt {
    queue_id: RuntimeQueueId,
    contact_id: RuntimeContactId,
    inserted: bool,
}

fn queue_runtime_message_with_id(
    state_directory: &Path,
    queue_id: RuntimeQueueId,
    conversation: String,
    peer_account_id: AccountId,
    message: String,
) -> Result<RuntimeQueueReceipt> {
    let device_state = load_command_device_state(state_directory)?;
    let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
    let local_certificate = trust.load_certificate()?;
    let conversation_id = ConversationId::from_label(&conversation);
    let snapshot = load_runtime_state_snapshot(
        state_directory,
        local_certificate.account_id(),
        device_state.identity().device_id(),
    )?;
    if let Some(existing) = snapshot.queued.get(&queue_id) {
        ensure!(
            existing.peer_account_id() == peer_account_id
                && existing.conversation_id() == conversation_id
                && existing.open(device_state.encryption())? == message,
            "runtime IPC request ID was already used for different message content"
        );
        return Ok(RuntimeQueueReceipt {
            queue_id,
            contact_id: existing.contact_id(),
            inserted: false,
        });
    }
    let contact = snapshot
        .contacts
        .values()
        .find(|contact| {
            contact.peer_account_id() == peer_account_id
                && contact.conversation_id() == conversation_id
        })
        .context("no runtime contact matches the peer account and conversation")?;
    let membership = trust.load_conversation_membership(conversation_id.scope_id())?;
    require_conversation_participants(
        &membership,
        local_certificate.account_id(),
        peer_account_id,
    )?;
    let queued = SignedQueuedMessage::seal_with_queue_id(
        device_state.identity(),
        device_state.encryption(),
        contact,
        queue_id,
        &message,
        unix_time_now()?,
    )?;
    let encoded = queued.encode()?;
    let outcome = run_state_transaction(state_directory, |transaction| {
        persist_runtime_record(
            state_directory,
            &runtime_queued_relative_path(queued.queue_id()),
            &encoded,
            transaction,
        )
    })?;
    Ok(RuntimeQueueReceipt {
        queue_id: queued.queue_id(),
        contact_id: queued.contact_id(),
        inserted: outcome == StoreOutcome::Inserted,
    })
}

fn runtime_outbox_status(state_directory: PathBuf) -> Result<()> {
    let status = collect_runtime_outbox_status(&state_directory)?;
    print_runtime_outbox_status(&status);
    println!("status=runtime-outbox-inspected");
    Ok(())
}

fn collect_runtime_outbox_status(state_directory: &Path) -> Result<RuntimeIpcOutboxStatus> {
    let device_state = load_command_device_state(state_directory)?;
    let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
    let local_certificate = trust.load_certificate()?;
    let snapshot = load_runtime_state_snapshot(
        state_directory,
        local_certificate.account_id(),
        device_state.identity().device_id(),
    )?;
    let mut items = Vec::with_capacity(snapshot.queued.len());
    for (queue_id, queued) in &snapshot.queued {
        let state = if snapshot.delivered.contains_key(queue_id) {
            RuntimeIpcQueueState::Delivered
        } else if snapshot.materialized.contains_key(queue_id) {
            RuntimeIpcQueueState::Materialized
        } else {
            RuntimeIpcQueueState::Queued
        };
        items.push(RuntimeIpcQueueItem {
            queue_id: queue_id.to_string(),
            peer_account_id: queued.peer_account_id(),
            conversation_id: queued.conversation_id(),
            state,
            acknowledgement_event_id: snapshot
                .delivered
                .get(queue_id)
                .map(SignedDeliveredMessage::acknowledgement_event_id),
        });
    }
    Ok(RuntimeIpcOutboxStatus {
        contact_count: snapshot.contacts.len(),
        queue_count: snapshot.queued.len(),
        pending_count: snapshot.pending_count(),
        materialized_count: snapshot.materialized.len(),
        delivered_count: snapshot.delivered.len(),
        retry_state_count: snapshot.retries.values().map(Vec::len).sum(),
        items,
    })
}

fn runtime_ipc_route_policy(route_policy: RoutePolicy) -> RuntimeIpcRoutePolicy {
    match route_policy {
        RoutePolicy::Auto => RuntimeIpcRoutePolicy::Auto,
        RoutePolicy::DirectOnly => RuntimeIpcRoutePolicy::DirectOnly,
        RoutePolicy::RelayOnly => RuntimeIpcRoutePolicy::RelayOnly,
    }
}

fn exact_runtime_contact<'a>(
    snapshot: &'a RuntimeStateSnapshot,
    conversation: &str,
    peer_account_id: AccountId,
) -> Result<&'a SignedRuntimeContact> {
    ensure!(
        !conversation.is_empty() && conversation.len() <= 4_096,
        "runtime contact conversation label is invalid"
    );
    let conversation_id = ConversationId::from_label(conversation);
    let mut matches = snapshot.contacts.values().filter(|contact| {
        contact.peer_account_id() == peer_account_id
            && contact.conversation_id() == conversation_id
            && contact.conversation_label() == conversation
    });
    let contact = matches
        .next()
        .context("no runtime contact matches the peer account and conversation")?;
    ensure!(
        matches.next().is_none(),
        "multiple runtime contacts match the peer account and conversation"
    );
    Ok(contact)
}

fn order_runtime_history(events: Vec<StoredAuthorizedEvent>) -> Result<Vec<StoredAuthorizedEvent>> {
    ensure!(
        events.len() <= MAX_INVENTORY_EVENT_IDS,
        "runtime history exceeds the bounded local event limit of {MAX_INVENTORY_EVENT_IDS}"
    );
    let known = events
        .iter()
        .map(|stored| stored.id)
        .collect::<BTreeSet<_>>();
    let mut remaining = events
        .into_iter()
        .map(|stored| (stored.id, stored))
        .collect::<BTreeMap<_, _>>();
    let mut dependency_counts = BTreeMap::new();
    let mut children = BTreeMap::<_, Vec<_>>::new();
    let mut ready = BTreeSet::new();

    for stored in remaining.values() {
        let dependencies = stored
            .event
            .event()
            .parents()
            .iter()
            .filter(|parent| known.contains(parent))
            .count();
        dependency_counts.insert(stored.id, dependencies);
        for parent in stored
            .event
            .event()
            .parents()
            .iter()
            .filter(|parent| known.contains(parent))
        {
            children.entry(*parent).or_default().push(stored.id);
        }
        if dependencies == 0 {
            ready.insert((
                stored.event.event().author_sequence(),
                stored.event.event().author_device_id().to_string(),
                stored.id,
            ));
        }
    }

    let event_count = remaining.len();
    let mut ordered = Vec::with_capacity(event_count);
    while let Some((_, _, event_id)) = ready.pop_first() {
        let stored = remaining
            .remove(&event_id)
            .context("runtime history ordering lost a ready event")?;
        if let Some(child_ids) = children.get(&event_id) {
            for child_id in child_ids {
                let count = dependency_counts
                    .get_mut(child_id)
                    .context("runtime history ordering lost a child dependency")?;
                *count = count
                    .checked_sub(1)
                    .context("runtime history dependency count underflow")?;
                if *count == 0 {
                    let child = remaining
                        .get(child_id)
                        .context("runtime history ordering lost a child event")?;
                    ready.insert((
                        child.event.event().author_sequence(),
                        child.event.event().author_device_id().to_string(),
                        *child_id,
                    ));
                }
            }
        }
        ordered.push(stored);
    }
    ensure!(
        ordered.len() == event_count,
        "runtime history contains an invalid causal cycle"
    );
    Ok(ordered)
}

fn truncate_runtime_preview(body: &str) -> (String, bool) {
    if body.len() <= MAX_RUNTIME_PREVIEW_BYTES {
        return (body.to_owned(), false);
    }
    let mut end = MAX_RUNTIME_PREVIEW_BYTES;
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    (body[..end].to_owned(), true)
}

fn runtime_history_page_end(
    total_messages: usize,
    snapshot_id: [u8; 32],
    cursor: Option<RuntimeIpcHistoryCursor>,
) -> Result<usize> {
    let Some(cursor) = cursor else {
        return Ok(total_messages);
    };
    ensure!(
        cursor.snapshot_id() == snapshot_id,
        "runtime history changed; restart pagination"
    );
    let before_index = usize::try_from(cursor.before_index())
        .context("runtime history cursor index conversion")?;
    ensure!(
        before_index <= total_messages,
        "runtime history cursor is outside this conversation"
    );
    Ok(before_index)
}

fn open_runtime_history_message(
    stored: &StoredAuthorizedEvent,
    read_repositories: &ImmutableReadRepositories,
    device_state: &DeviceState,
    local_account_id: AccountId,
) -> Result<RuntimeIpcHistoryMessage> {
    let event = stored.event.event();
    ensure!(
        matches!(event.payload(), EventPayload::RatchetText { .. }),
        "runtime history attempted to expose a non-text event"
    );
    let projection = read_repositories
        .local_messages
        .get(stored.id)
        .with_context(|| format!("load local projection for runtime event {}", stored.id))?;
    let body = projection
        .open_for_account(
            event,
            device_state.identity().device_id(),
            local_account_id,
            device_state.encryption(),
        )
        .with_context(|| format!("decrypt local projection for runtime event {}", stored.id))?;
    Ok(RuntimeIpcHistoryMessage {
        event_id: stored.id,
        author_account_id: stored.event.author_account_id(),
        author_device_id: event.author_device_id(),
        author_sequence: event.author_sequence(),
        body,
    })
}

fn collect_runtime_conversation_list(
    state_directory: &Path,
) -> Result<Vec<RuntimeIpcConversationSummary>> {
    let device_state = load_command_device_state(state_directory)?;
    let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
    let local_certificate = trust.load_certificate()?;
    let local_account_id = local_certificate.account_id();
    let snapshot = load_runtime_state_snapshot(
        state_directory,
        local_account_id,
        device_state.identity().device_id(),
    )?;
    let local_authority = trust.load_own_authority_snapshot(&local_certificate)?;
    let read_repositories = open_immutable_read_repositories(state_directory)?;
    let mut summaries = Vec::with_capacity(snapshot.contacts.len());
    let mut inspected_events = 0_usize;

    for contact in snapshot.contacts.values() {
        let membership = trust
            .load_conversation_membership(contact.conversation_id().scope_id())
            .with_context(|| {
                format!(
                    "load membership for runtime conversation {}",
                    contact.conversation_label()
                )
            })?;
        require_conversation_participants(
            &membership,
            local_account_id,
            contact.peer_account_id(),
        )?;
        let events = read_repositories
            .events
            .load_authorized_conversation(contact.conversation_id(), &membership)
            .with_context(|| {
                format!("load runtime conversation {}", contact.conversation_label())
            })?;
        inspected_events = inspected_events
            .checked_add(events.len())
            .context("runtime conversation-list event count overflow")?;
        ensure!(
            inspected_events <= MAX_RUNTIME_CONVERSATION_LIST_EVENTS,
            "runtime conversation list exceeds the bounded aggregate event limit of {MAX_RUNTIME_CONVERSATION_LIST_EVENTS}"
        );
        let ordered = order_runtime_history(events)?;
        let text_events = ordered
            .iter()
            .filter(|stored| {
                matches!(
                    stored.event.event().payload(),
                    EventPayload::RatchetText { .. }
                )
            })
            .collect::<Vec<_>>();
        let latest_message = text_events
            .last()
            .map(|stored| {
                let message = open_runtime_history_message(
                    stored,
                    &read_repositories,
                    &device_state,
                    local_account_id,
                )?;
                let (body, truncated) = truncate_runtime_preview(&message.body);
                Ok::<_, anyhow::Error>(RuntimeIpcMessagePreview {
                    event_id: message.event_id,
                    author_account_id: message.author_account_id,
                    body,
                    truncated,
                })
            })
            .transpose()?;
        let pinned_peer_authority = trust
            .load_peer_authority_snapshot(contact.peer_account_id())
            .context("load peer authority high-water for endpoint health")?;
        let endpoint_candidates = collect_runtime_endpoint_candidate_statuses(
            &snapshot,
            contact,
            &local_certificate,
            &local_authority,
            &pinned_peer_authority,
        )?;
        let usable_endpoint_candidate_count = u8::try_from(
            endpoint_candidates
                .iter()
                .filter(|candidate| candidate.state == RuntimeIpcEndpointCandidateState::Usable)
                .count(),
        )
        .context("usable runtime endpoint-candidate count exceeds IPC representation")?;
        let stale_endpoint_candidate_count = u8::try_from(
            endpoint_candidates
                .iter()
                .filter(|candidate| candidate.state == RuntimeIpcEndpointCandidateState::Stale)
                .count(),
        )
        .context("stale runtime endpoint-candidate count exceeds IPC representation")?;
        summaries.push(RuntimeIpcConversationSummary {
            contact_id: contact.contact_id().to_string(),
            conversation_label: contact.conversation_label().to_owned(),
            conversation_id: contact.conversation_id(),
            peer_account_id: contact.peer_account_id(),
            peer_device_id: contact.peer_device_id(),
            endpoint_candidate_count: u8::try_from(endpoint_candidates.len())
                .context("runtime endpoint-candidate count exceeds IPC representation")?,
            usable_endpoint_candidate_count,
            stale_endpoint_candidate_count,
            endpoint_candidates,
            route_policy: runtime_ipc_route_policy(contact.route_policy()),
            message_count: u32::try_from(text_events.len())
                .context("runtime conversation message count overflow")?,
            latest_message,
        });
    }
    summaries.sort_by(|left, right| {
        left.conversation_label
            .cmp(&right.conversation_label)
            .then_with(|| {
                left.peer_account_id
                    .as_bytes()
                    .cmp(right.peer_account_id.as_bytes())
            })
    });
    Ok(summaries)
}

fn collect_runtime_history_page(
    state_directory: &Path,
    conversation: &str,
    cursor: Option<RuntimeIpcHistoryCursor>,
    limit: u16,
) -> Result<RuntimeIpcHistoryPage> {
    ensure!(
        !conversation.is_empty() && conversation.len() <= 4_096,
        "runtime history conversation label is invalid"
    );
    ensure!(
        (1..=MAX_RUNTIME_HISTORY_PAGE_SIZE).contains(&limit),
        "runtime history page limit must be between 1 and {MAX_RUNTIME_HISTORY_PAGE_SIZE}"
    );
    let device_state = load_command_device_state(state_directory)?;
    let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
    let local_certificate = trust.load_certificate()?;
    let local_account_id = local_certificate.account_id();
    let conversation_id = ConversationId::from_label(conversation);
    let runtime_snapshot = load_runtime_state_snapshot(
        state_directory,
        local_account_id,
        device_state.identity().device_id(),
    )?;
    ensure!(
        runtime_snapshot.contacts.values().any(|contact| {
            contact.conversation_id() == conversation_id
                && contact.conversation_label() == conversation
        }),
        "runtime history is not exposed without a matching signed contact"
    );
    let membership = trust
        .load_conversation_membership(conversation_id.scope_id())
        .context("load membership for runtime history page")?;
    membership
        .require_member(local_account_id)
        .context("runtime account is not a conversation member")?;
    let read_repositories = open_immutable_read_repositories(state_directory)?;
    let ordered = order_runtime_history(
        read_repositories
            .events
            .load_authorized_conversation(conversation_id, &membership)
            .context("load authorized runtime history page")?,
    )?;
    let text_events = ordered
        .iter()
        .filter(|stored| {
            matches!(
                stored.event.event().payload(),
                EventPayload::RatchetText { .. }
            )
        })
        .collect::<Vec<_>>();
    let total_messages =
        u32::try_from(text_events.len()).context("runtime history message count overflow")?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(RUNTIME_HISTORY_SNAPSHOT_DOMAIN);
    hasher.update(conversation_id.as_bytes());
    for stored in &text_events {
        hasher.update(stored.id.as_bytes());
    }
    let snapshot_id = *hasher.finalize().as_bytes();
    let end = runtime_history_page_end(text_events.len(), snapshot_id, cursor)?;

    let mut messages = Vec::new();
    let mut page_body_bytes = 0_usize;
    let mut start = end;
    while start > 0 && messages.len() < usize::from(limit) {
        let candidate = open_runtime_history_message(
            text_events[start - 1],
            &read_repositories,
            &device_state,
            local_account_id,
        )?;
        if !messages.is_empty()
            && page_body_bytes.saturating_add(candidate.body.len())
                > MAX_RUNTIME_HISTORY_PAGE_BODY_BYTES
        {
            break;
        }
        page_body_bytes = page_body_bytes.saturating_add(candidate.body.len());
        messages.push(candidate);
        start -= 1;
    }
    messages.reverse();
    let next_cursor = if start == 0 {
        None
    } else {
        Some(RuntimeIpcHistoryCursor::new(
            snapshot_id,
            u32::try_from(start).context("runtime history cursor index overflow")?,
        ))
    };
    Ok(RuntimeIpcHistoryPage {
        conversation_id,
        total_messages,
        messages,
        next_cursor,
    })
}

fn print_runtime_queue_receipt(receipt: &RuntimeQueueReceipt) {
    println!("runtime_queue_id={}", receipt.queue_id);
    println!("runtime_contact_id={}", receipt.contact_id);
    println!("runtime_queue_body=encrypted-at-rest");
    println!(
        "runtime_queue_store={}",
        if receipt.inserted {
            "Inserted"
        } else {
            "AlreadyPresent"
        }
    );
    println!("status=runtime-message-queued");
}

fn print_runtime_outbox_status(status: &RuntimeIpcOutboxStatus) {
    println!("runtime_contact_count={}", status.contact_count);
    println!("runtime_queue_count={}", status.queue_count);
    println!("runtime_pending_count={}", status.pending_count);
    println!("runtime_materialized_count={}", status.materialized_count);
    println!("runtime_delivered_count={}", status.delivered_count);
    println!("runtime_retry_state_count={}", status.retry_state_count);
    for item in &status.items {
        println!(
            "runtime_queue_id={} runtime_queue_state={} peer_account_id={} conversation_id={}",
            item.queue_id,
            item.state.as_str(),
            item.peer_account_id,
            item.conversation_id
        );
        if let Some(acknowledgement_event_id) = item.acknowledgement_event_id {
            println!(
                "runtime_queue_id={} acknowledgement_event_id={acknowledgement_event_id}",
                item.queue_id
            );
        }
    }
}

async fn runtime_ipc_ping(ipc_file: PathBuf) -> Result<()> {
    let descriptor = RuntimeIpcDescriptor::load(&ipc_file)?;
    match kilogram_runtime_ipc::call(&ipc_file, RuntimeIpcCommand::Ping).await? {
        RuntimeIpcResponse::Pong {
            account_id,
            device_id,
        } => {
            ensure!(
                account_id == descriptor.account_id() && device_id == descriptor.device_id(),
                "runtime IPC ping identity does not match its signed descriptor"
            );
            println!("runtime_ipc_account_id={account_id}");
            println!("runtime_ipc_device_id={device_id}");
            println!("status=runtime-ipc-ready");
            Ok(())
        }
        RuntimeIpcResponse::Error { message } => bail!("runtime IPC rejected ping: {message}"),
        _ => bail!("runtime IPC returned an unexpected ping response"),
    }
}

async fn runtime_ipc_queue_message(
    ipc_file: PathBuf,
    conversation: String,
    peer_account_id: AccountId,
    message: String,
    request_id: Option<RuntimeIpcRequestId>,
) -> Result<()> {
    let request_id = match request_id {
        Some(request_id) => request_id,
        None => RuntimeIpcRequestId::generate()?,
    };
    println!("runtime_ipc_request_id={request_id}");
    let response = kilogram_runtime_ipc::call(
        &ipc_file,
        RuntimeIpcCommand::QueueMessage {
            request_id,
            conversation,
            peer_account_id,
            message,
        },
    )
    .await?;
    match response {
        RuntimeIpcResponse::MessageQueued {
            queue_id,
            contact_id,
            inserted,
        } => {
            ensure!(
                queue_id == request_id.to_string(),
                "runtime IPC returned a different queue ID"
            );
            println!("runtime_queue_id={queue_id}");
            println!("runtime_contact_id={contact_id}");
            println!("runtime_queue_body=encrypted-at-rest");
            println!(
                "runtime_queue_store={}",
                if inserted {
                    "Inserted"
                } else {
                    "AlreadyPresent"
                }
            );
            println!("status=runtime-message-queued");
            Ok(())
        }
        RuntimeIpcResponse::Error { message } => {
            bail!("runtime IPC rejected queued message: {message}")
        }
        _ => bail!("runtime IPC returned an unexpected queue response"),
    }
}

async fn runtime_ipc_outbox_status(ipc_file: PathBuf) -> Result<()> {
    match kilogram_runtime_ipc::call(&ipc_file, RuntimeIpcCommand::OutboxStatus).await? {
        RuntimeIpcResponse::OutboxStatus(status) => {
            print_runtime_outbox_status(&status);
            println!("status=runtime-ipc-outbox-inspected");
            Ok(())
        }
        RuntimeIpcResponse::Error { message } => {
            bail!("runtime IPC rejected outbox status: {message}")
        }
        _ => bail!("runtime IPC returned an unexpected outbox response"),
    }
}

async fn runtime_ipc_apply_device_directory(
    ipc_file: PathBuf,
    device_list_file: PathBuf,
) -> Result<()> {
    let device_list_file = fs::canonicalize(&device_list_file).with_context(|| {
        format!(
            "resolve refreshed device list {}",
            device_list_file.display()
        )
    })?;
    match kilogram_runtime_ipc::call(
        &ipc_file,
        RuntimeIpcCommand::ApplyOwnDeviceDirectory { device_list_file },
    )
    .await?
    {
        RuntimeIpcResponse::OwnDeviceDirectoryApplied(update) => {
            print_runtime_device_directory_update(&update);
            println!("status=runtime-own-device-directory-applied");
            Ok(())
        }
        RuntimeIpcResponse::Error { message } => {
            bail!("runtime IPC rejected device-directory update: {message}")
        }
        _ => bail!("runtime IPC returned an unexpected device-directory response"),
    }
}

async fn runtime_ipc_device_directory_status(ipc_file: PathBuf) -> Result<()> {
    match kilogram_runtime_ipc::call(&ipc_file, RuntimeIpcCommand::OwnDeviceDirectoryStatus).await?
    {
        RuntimeIpcResponse::OwnDeviceDirectoryStatus(status) => {
            print_runtime_device_directory_status(&status);
            println!("status=runtime-own-device-directory-inspected");
            Ok(())
        }
        RuntimeIpcResponse::Error { message } => {
            bail!("runtime IPC rejected device-directory status: {message}")
        }
        _ => bail!("runtime IPC returned an unexpected device-directory status response"),
    }
}

async fn runtime_ipc_publish_ticket(
    ipc_file: PathBuf,
    conversation: String,
    peer_account_id: AccountId,
    service_base_url: String,
    ttl_seconds: u64,
) -> Result<()> {
    match kilogram_runtime_ipc::call(
        &ipc_file,
        RuntimeIpcCommand::PublishOwnTicket {
            conversation,
            peer_account_id,
            service_base_url,
            ttl_seconds,
        },
    )
    .await?
    {
        RuntimeIpcResponse::OwnTicketPublished(publication) => {
            print_runtime_ticket_publication(&publication);
            println!("status=runtime-own-ticket-published");
            Ok(())
        }
        RuntimeIpcResponse::Error { message } => {
            bail!("runtime IPC rejected ticket publication: {message}")
        }
        _ => bail!("runtime IPC returned an unexpected ticket-publication response"),
    }
}

async fn runtime_ipc_refresh_contact_ticket(
    ipc_file: PathBuf,
    conversation: String,
    peer_account_id: AccountId,
    service_base_url: String,
) -> Result<()> {
    match kilogram_runtime_ipc::call(
        &ipc_file,
        RuntimeIpcCommand::RefreshContactTicket {
            conversation,
            peer_account_id,
            service_base_url,
        },
    )
    .await?
    {
        RuntimeIpcResponse::ContactTicketRefreshed(refresh) => {
            print_runtime_contact_ticket_refresh(&refresh);
            println!(
                "status={}",
                if refresh.complete {
                    "runtime-contact-ticket-refreshed"
                } else {
                    "runtime-contact-ticket-refresh-partial"
                }
            );
            Ok(())
        }
        RuntimeIpcResponse::Error { message } => {
            bail!("runtime IPC rejected contact-ticket refresh: {message}")
        }
        _ => bail!("runtime IPC returned an unexpected contact-ticket response"),
    }
}

#[allow(clippy::too_many_arguments)]
async fn runtime_ipc_configure_ticket_automation(
    ipc_file: PathBuf,
    conversation: String,
    peer_account_id: AccountId,
    enabled: bool,
    service_base_url: String,
    ttl_seconds: u64,
    refresh_before_seconds: u64,
    retry_base_seconds: u64,
    retry_max_seconds: u64,
    allow_ethernet: bool,
    allow_wifi: bool,
    allow_mobile: bool,
    allow_unknown_network: bool,
) -> Result<()> {
    match kilogram_runtime_ipc::call(
        &ipc_file,
        RuntimeIpcCommand::ConfigureTicketAutomation {
            conversation,
            peer_account_id,
            enabled,
            service_base_url,
            ttl_seconds,
            refresh_before_seconds,
            retry_base_seconds,
            retry_max_seconds,
            allow_ethernet,
            allow_wifi,
            allow_mobile,
            allow_unknown_network,
        },
    )
    .await?
    {
        RuntimeIpcResponse::TicketAutomationConfigured(status) => {
            print_runtime_ticket_automation_status(&status);
            println!("status=runtime-ticket-automation-configured");
            Ok(())
        }
        RuntimeIpcResponse::Error { message } => {
            bail!("runtime IPC rejected ticket automation policy: {message}")
        }
        _ => bail!("runtime IPC returned an unexpected ticket-automation response"),
    }
}

async fn runtime_ipc_ticket_automation_status(ipc_file: PathBuf) -> Result<()> {
    match kilogram_runtime_ipc::call(&ipc_file, RuntimeIpcCommand::TicketAutomationStatus).await? {
        RuntimeIpcResponse::TicketAutomationStatus(statuses) => {
            println!("ticket_automation_policy_count={}", statuses.len());
            for status in &statuses {
                print_runtime_ticket_automation_status(status);
            }
            println!("status=runtime-ticket-automation-status");
            Ok(())
        }
        RuntimeIpcResponse::Error { message } => {
            bail!("runtime IPC rejected ticket automation status: {message}")
        }
        _ => bail!("runtime IPC returned an unexpected ticket-automation status response"),
    }
}

async fn runtime_ipc_export_endpoint_announcements(
    ipc_file: PathBuf,
    recipient_device_id: DeviceId,
    output_file: PathBuf,
    validity_seconds: u64,
) -> Result<()> {
    match kilogram_runtime_ipc::call(
        &ipc_file,
        RuntimeIpcCommand::ExportEndpointAnnouncements {
            recipient_device_id,
            output_file,
            validity_seconds,
        },
    )
    .await?
    {
        RuntimeIpcResponse::EndpointAnnouncementsExported(report) => {
            print_runtime_endpoint_announcement_export(&report);
            Ok(())
        }
        RuntimeIpcResponse::Error { message } => {
            bail!("runtime IPC rejected endpoint announcement export: {message}")
        }
        response => bail!(
            "runtime IPC returned an unexpected endpoint announcement export response: {response:?}"
        ),
    }
}

async fn runtime_ipc_import_endpoint_announcements(
    ipc_file: PathBuf,
    bundle_file: PathBuf,
    descriptor_directory: PathBuf,
) -> Result<()> {
    match kilogram_runtime_ipc::call(
        &ipc_file,
        RuntimeIpcCommand::ImportEndpointAnnouncements {
            bundle_file,
            descriptor_directory,
        },
    )
    .await?
    {
        RuntimeIpcResponse::EndpointAnnouncementsImported(report) => {
            print_runtime_endpoint_announcement_import(&report);
            Ok(())
        }
        RuntimeIpcResponse::Error { message } => {
            bail!("runtime IPC rejected endpoint announcement import: {message}")
        }
        response => bail!(
            "runtime IPC returned an unexpected endpoint announcement import response: {response:?}"
        ),
    }
}

async fn runtime_ipc_push_endpoint_announcements(
    ipc_file: PathBuf,
    recipient_ticket_file: PathBuf,
    validity_seconds: u64,
) -> Result<()> {
    match kilogram_runtime_ipc::call(
        &ipc_file,
        RuntimeIpcCommand::PushEndpointAnnouncements {
            recipient_ticket_file,
            validity_seconds,
        },
    )
    .await?
    {
        RuntimeIpcResponse::EndpointAnnouncementsPushed(report) => {
            print_runtime_endpoint_announcement_push(&report);
            Ok(())
        }
        RuntimeIpcResponse::Error { message } => {
            bail!("runtime IPC rejected endpoint announcement push: {message}")
        }
        response => bail!(
            "runtime IPC returned an unexpected endpoint announcement push response: {response:?}"
        ),
    }
}

fn print_runtime_endpoint_announcement_export(report: &RuntimeIpcEndpointAnnouncementExport) {
    println!("endpoint_announcement_bundle_id={}", report.bundle_id);
    println!("source_device_id={}", report.source_device_id);
    println!("recipient_device_id={}", report.recipient_device_id);
    println!("authority_revision={}", report.authority_revision);
    println!("contact_count={}", report.contact_count);
    println!("endpoint_count={}", report.endpoint_count);
    println!("observation_count={}", report.observation_count);
    println!("expires_at_unix_seconds={}", report.expires_at_unix_seconds);
    println!("output_file={}", report.output_file.display());
    println!("protection={}", report.protection);
    println!("status=endpoint-announcements-exported");
}

fn print_runtime_endpoint_announcement_import(report: &RuntimeIpcEndpointAnnouncementImport) {
    println!("endpoint_announcement_bundle_id={}", report.bundle_id);
    println!("source_device_id={}", report.source_device_id);
    println!("recipient_device_id={}", report.recipient_device_id);
    println!("authority_revision={}", report.authority_revision);
    println!("contact_count={}", report.contact_count);
    println!("contact_added_count={}", report.contact_added_count);
    println!("endpoint_count={}", report.endpoint_count);
    println!("endpoint_added_count={}", report.endpoint_added_count);
    println!(
        "publication_binding_added_count={}",
        report.publication_binding_added_count
    );
    println!(
        "observation_evidence_count={}",
        report.observation_evidence_count
    );
    println!(
        "observation_evidence_added_count={}",
        report.observation_evidence_added_count
    );
    println!(
        "descriptor_directory={}",
        report.descriptor_directory.display()
    );
    println!("authority_status={}", report.authority_status);
    println!("status=endpoint-announcements-imported");
}

fn print_runtime_endpoint_announcement_push(report: &RuntimeIpcEndpointAnnouncementPush) {
    println!("endpoint_announcement_bundle_id={}", report.bundle_id);
    println!("source_device_id={}", report.source_device_id);
    println!("recipient_device_id={}", report.recipient_device_id);
    println!("authority_revision={}", report.authority_revision);
    println!("contact_count={}", report.contact_count);
    println!("endpoint_count={}", report.endpoint_count);
    println!("observation_count={}", report.observation_count);
    println!("encrypted_bundle_bytes={}", report.encrypted_bundle_bytes);
    println!(
        "recipient_contact_added_count={}",
        report.recipient_contact_added_count
    );
    println!(
        "recipient_endpoint_added_count={}",
        report.recipient_endpoint_added_count
    );
    println!(
        "recipient_publication_binding_added_count={}",
        report.recipient_publication_binding_added_count
    );
    println!(
        "recipient_observation_evidence_added_count={}",
        report.recipient_observation_evidence_added_count
    );
    println!("transport_path={}", report.transport_path);
    println!("acknowledgement_status={}", report.acknowledgement_status);
    println!("status=endpoint-announcements-pushed");
}

fn print_runtime_ticket_automation_status(status: &RuntimeIpcTicketAutomationStatus) {
    println!("ticket_automation_contact_id={}", status.contact_id);
    println!("ticket_automation_conversation={}", status.conversation);
    println!(
        "ticket_automation_peer_account_id={}",
        status.peer_account_id
    );
    println!("ticket_automation_enabled={}", status.enabled);
    println!(
        "ticket_automation_policy_generation={}",
        status.policy_generation
    );
    println!(
        "ticket_automation_service_base_url={}",
        status.service_base_url
    );
    println!(
        "ticket_automation_current_network={}",
        status.current_network.as_str()
    );
    println!(
        "ticket_automation_network_allowed={}",
        status.network_allowed
    );
    println!(
        "ticket_automation_execution_scope={}",
        status.execution_scope
    );
    println!(
        "ticket_automation_os_background_service_enabled={}",
        status.os_background_service_enabled
    );
    for action in [&status.publish, &status.refresh] {
        println!("ticket_automation_{}_state={}", action.action, action.state);
        println!(
            "ticket_automation_{}_next_attempt_unix_seconds={}",
            action.action,
            action
                .next_attempt_unix_seconds
                .map_or_else(|| "none".to_owned(), |value| value.to_string())
        );
        println!(
            "ticket_automation_{}_consecutive_failures={}",
            action.action, action.consecutive_failures
        );
        println!(
            "ticket_automation_{}_last_result={}",
            action.action, action.last_result
        );
    }
}

fn print_runtime_ticket_publication(publication: &RuntimeIpcTicketPublication) {
    println!("runtime_contact_id={}", publication.contact_id);
    println!("ticket_publication_channel_id={}", publication.channel_id);
    println!("ticket_publication_id={}", publication.publication_id);
    println!(
        "ticket_publication_generation={}",
        publication.publication_generation
    );
    println!(
        "ticket_publication_expires_at_unix_seconds={}",
        publication.expires_at_unix_seconds
    );
    println!(
        "ticket_publication_recipient_device_count={}",
        publication.recipient_device_count
    );
    println!(
        "ticket_publication_encrypted_record_bytes={}",
        publication.encrypted_record_bytes
    );
    println!(
        "ticket_publication_service_base_url={}",
        publication.service_base_url
    );
    println!(
        "ticket_publication_local_store={}",
        publication.local_store_status
    );
    println!("ticket_publication_upload={}", publication.upload_status);
    println!(
        "ticket_publication_lookup_privacy={}",
        publication.lookup_privacy_status
    );
    println!(
        "ticket_publication_first_contact_freshness={}",
        publication.first_contact_freshness
    );
}

fn print_runtime_contact_ticket_refresh(refresh: &RuntimeIpcContactTicketRefresh) {
    println!("runtime_contact_id={}", refresh.contact_id);
    println!(
        "endpoint_candidate_count={}",
        refresh.endpoint_candidate_count
    );
    println!(
        "refreshed_endpoint_candidate_count={}",
        refresh.refreshed_endpoint_candidate_count
    );
    println!("ticket_refresh_complete={}", refresh.complete);
    for result in &refresh.results {
        println!("endpoint_peer_device_id={}", result.peer_device_id);
        println!("endpoint_primary={}", result.primary);
        println!("endpoint_route_policy={}", result.route_policy.as_str());
        println!(
            "endpoint_descriptor_file={}",
            result.descriptor_file.display()
        );
        println!("endpoint_state={}", result.state.as_str());
        println!(
            "endpoint_publication_channel_id={}",
            result.channel_id.as_deref().unwrap_or("none")
        );
        println!(
            "endpoint_publication_generation={}",
            result
                .publication_generation
                .map_or_else(|| "none".to_owned(), |value| value.to_string())
        );
        println!(
            "endpoint_publication_expires_at_unix_seconds={}",
            result
                .expires_at_unix_seconds
                .map_or_else(|| "none".to_owned(), |value| value.to_string())
        );
        println!("endpoint_detail={}", result.detail);
    }
}

fn print_runtime_device_directory_status(status: &RuntimeIpcDeviceDirectoryStatus) {
    println!("account_id={}", status.account_id);
    println!("device_id={}", status.local_device_id);
    println!("state_dir={}", status.state_dir.display());
    println!("authority_revision={}", status.authority_revision);
    println!("active_device_count={}", status.active_device_count);
    if let Some(receipt_id) = &status.receipt_id {
        println!("directory_receipt_id={receipt_id}");
    }
    if let Some(generation) = status.receipt_generation {
        println!("directory_receipt_generation={generation}");
    }
    println!("device_list_digest={}", status.device_list_digest);
    println!(
        "applied_device_list_file={}",
        status.applied_device_list_file.display()
    );
    println!(
        "launch_device_list_file={}",
        status.launch_device_list_file.display()
    );
    println!(
        "profile_convergence_status={}",
        status.profile_convergence_status
    );
    println!("restart_recovery_status={}", status.restart_recovery_status);
}

fn print_runtime_device_directory_update(update: &RuntimeIpcDeviceDirectoryUpdate) {
    println!("account_id={}", update.account_id);
    println!("device_id={}", update.local_device_id);
    println!(
        "previous_authority_revision={}",
        update.previous_authority_revision
    );
    println!("authority_revision={}", update.authority_revision);
    println!("active_device_count={}", update.active_device_count);
    for device_id in &update.removed_device_ids {
        println!("removed_device_id={device_id}");
    }
    println!(
        "ratchet_session_records_retired={}",
        update.ratchet_session_records_retired
    );
    println!(
        "prekey_observations_retired={}",
        update.prekey_observations_retired
    );
    println!(
        "pending_unmaterialized_messages={}",
        update.pending_unmaterialized_messages
    );
    println!(
        "pending_materialized_messages={}",
        update.pending_materialized_messages
    );
    println!(
        "future_recipient_slot_status={}",
        update.future_recipient_slot_status
    );
    println!(
        "preexisting_recipient_slot_status={}",
        update.preexisting_recipient_slot_status
    );
    println!("ticket_published={}", update.ticket_published);
    println!(
        "launch_profile_update_required={}",
        update.launch_profile_update_required
    );
    println!(
        "history_availability_status={}",
        update.history_availability_status
    );
    print_runtime_device_directory_status(&update.directory_status);
}

struct RuntimeIpcDispatchOutcome {
    shutdown_requested: bool,
    state_changed: bool,
}

async fn handle_runtime_ipc_work(
    state_directory: &Path,
    endpoint: &Endpoint,
    ticket: &mut ConnectionTicket,
    directory_state: &mut RuntimeDeviceDirectoryState,
    ticket_file: Option<&Path>,
    work: RuntimeIpcWork,
) -> RuntimeIpcDispatchOutcome {
    let (command, response_sender) = work.into_parts();
    let shutdown_requested = matches!(command, RuntimeIpcCommand::Shutdown);
    let mut state_changed = false;
    let account_id = ticket.listener_account_id();
    let device_id = ticket.listener_device_id();
    let response = match command {
        RuntimeIpcCommand::Ping => RuntimeIpcResponse::Pong {
            account_id,
            device_id,
        },
        RuntimeIpcCommand::AddContact {
            conversation,
            expected_peer_account_id,
            descriptor_file,
        } => match with_locked_state(state_directory, || {
            add_runtime_contact_record(
                state_directory,
                conversation,
                expected_peer_account_id,
                descriptor_file,
            )
        }) {
            Ok(receipt) => {
                state_changed = receipt.inserted;
                RuntimeIpcResponse::ContactAdded {
                    contact_id: receipt.contact_id.to_string(),
                    peer_account_id: receipt.peer_account_id,
                    peer_device_id: receipt.peer_device_id,
                    endpoint_candidate_count: u8::try_from(receipt.endpoint_candidate_count)
                        .unwrap_or(u8::MAX),
                    endpoint_candidate_added: receipt.endpoint_candidate_added,
                    inserted: receipt.inserted,
                }
            }
            Err(error) => RuntimeIpcResponse::Error {
                message: format!("{error:#}"),
            },
        },
        RuntimeIpcCommand::ConfigureTicketAutomation {
            conversation,
            peer_account_id,
            enabled,
            service_base_url,
            ttl_seconds,
            refresh_before_seconds,
            retry_base_seconds,
            retry_max_seconds,
            allow_ethernet,
            allow_wifi,
            allow_mobile,
            allow_unknown_network,
        } => match with_locked_state(state_directory, || {
            configure_runtime_ticket_automation(
                state_directory,
                conversation,
                peer_account_id,
                enabled,
                service_base_url,
                ttl_seconds,
                refresh_before_seconds,
                retry_base_seconds,
                retry_max_seconds,
                allow_ethernet,
                allow_wifi,
                allow_mobile,
                allow_unknown_network,
                current_runtime_network_class(),
            )
        }) {
            Ok(status) => {
                state_changed = true;
                RuntimeIpcResponse::TicketAutomationConfigured(Box::new(status))
            }
            Err(error) => RuntimeIpcResponse::Error {
                message: format!("{error:#}"),
            },
        },
        RuntimeIpcCommand::TicketAutomationStatus => {
            let status = StateDirectoryLock::acquire(state_directory)
                .context("lock runtime state for ticket automation status")
                .and_then(|_lock| {
                    collect_runtime_ticket_automation_status(
                        state_directory,
                        current_runtime_network_class(),
                    )
                });
            match status {
                Ok(status) => RuntimeIpcResponse::TicketAutomationStatus(status),
                Err(error) => RuntimeIpcResponse::Error {
                    message: format!("{error:#}"),
                },
            }
        }
        RuntimeIpcCommand::QueueMessage {
            request_id,
            conversation,
            peer_account_id,
            message,
        } => match with_locked_state(state_directory, || {
            queue_runtime_message_with_id(
                state_directory,
                RuntimeQueueId::from_bytes(*request_id.as_bytes()),
                conversation,
                peer_account_id,
                message,
            )
        }) {
            Ok(receipt) => {
                state_changed = receipt.inserted;
                RuntimeIpcResponse::MessageQueued {
                    queue_id: receipt.queue_id.to_string(),
                    contact_id: receipt.contact_id.to_string(),
                    inserted: receipt.inserted,
                }
            }
            Err(error) => RuntimeIpcResponse::Error {
                message: format!("{error:#}"),
            },
        },
        RuntimeIpcCommand::OutboxStatus => {
            let status = StateDirectoryLock::acquire(state_directory)
                .context("lock runtime state for IPC outbox snapshot")
                .and_then(|_lock| collect_runtime_outbox_status(state_directory));
            match status {
                Ok(status) => RuntimeIpcResponse::OutboxStatus(status),
                Err(error) => RuntimeIpcResponse::Error {
                    message: format!("{error:#}"),
                },
            }
        }
        RuntimeIpcCommand::ApplyOwnDeviceDirectory { device_list_file } => {
            match with_locked_state(state_directory, || {
                apply_runtime_own_device_directory(
                    state_directory,
                    endpoint,
                    ticket,
                    directory_state,
                    ticket_file,
                    &device_list_file,
                )
            }) {
                Ok((replacement, replacement_directory_state, update)) => {
                    *ticket = replacement;
                    *directory_state = replacement_directory_state;
                    state_changed = true;
                    RuntimeIpcResponse::OwnDeviceDirectoryApplied(Box::new(update))
                }
                Err(error) => RuntimeIpcResponse::Error {
                    message: format!("{error:#}"),
                },
            }
        }
        RuntimeIpcCommand::OwnDeviceDirectoryStatus => match directory_state.status(ticket) {
            Ok(status) => RuntimeIpcResponse::OwnDeviceDirectoryStatus(status),
            Err(error) => RuntimeIpcResponse::Error {
                message: format!("{error:#}"),
            },
        },
        RuntimeIpcCommand::PublishOwnTicket {
            conversation,
            peer_account_id,
            service_base_url,
            ttl_seconds,
        } => match publish_runtime_own_ticket(
            state_directory,
            ticket,
            &conversation,
            peer_account_id,
            &service_base_url,
            ttl_seconds,
            MIN_TICKET_PUBLICATION_TTL_SECONDS,
        )
        .await
        {
            Ok(publication) => {
                state_changed = true;
                RuntimeIpcResponse::OwnTicketPublished(Box::new(publication))
            }
            Err(error) => RuntimeIpcResponse::Error {
                message: format!("{error:#}"),
            },
        },
        RuntimeIpcCommand::RefreshContactTicket {
            conversation,
            peer_account_id,
            service_base_url,
        } => match refresh_runtime_contact_ticket(
            state_directory,
            &conversation,
            peer_account_id,
            &service_base_url,
        )
        .await
        {
            Ok(refresh) => {
                state_changed = refresh.refreshed_endpoint_candidate_count != 0;
                RuntimeIpcResponse::ContactTicketRefreshed(Box::new(refresh))
            }
            Err(error) => RuntimeIpcResponse::Error {
                message: format!("{error:#}"),
            },
        },
        RuntimeIpcCommand::ExportEndpointAnnouncements {
            recipient_device_id,
            output_file,
            validity_seconds,
        } => {
            let export = StateDirectoryLock::acquire(state_directory)
                .context("lock runtime state for endpoint announcement export")
                .and_then(|_lock| {
                    export_runtime_endpoint_announcements(
                        state_directory,
                        ticket,
                        recipient_device_id,
                        output_file,
                        validity_seconds,
                    )
                });
            match export {
                Ok(report) => RuntimeIpcResponse::EndpointAnnouncementsExported(Box::new(report)),
                Err(error) => RuntimeIpcResponse::Error {
                    message: format!("{error:#}"),
                },
            }
        }
        RuntimeIpcCommand::ImportEndpointAnnouncements {
            bundle_file,
            descriptor_directory,
        } => match with_locked_state(state_directory, || {
            import_runtime_endpoint_announcements(
                state_directory,
                ticket,
                &bundle_file,
                &descriptor_directory,
            )
        }) {
            Ok(report) => {
                state_changed = report.contact_added_count != 0
                    || report.endpoint_added_count != 0
                    || report.publication_binding_added_count != 0
                    || report.observation_evidence_added_count != 0;
                RuntimeIpcResponse::EndpointAnnouncementsImported(Box::new(report))
            }
            Err(error) => RuntimeIpcResponse::Error {
                message: format!("{error:#}"),
            },
        },
        RuntimeIpcCommand::PushEndpointAnnouncements {
            recipient_ticket_file,
            validity_seconds,
        } => match push_runtime_endpoint_announcements(
            state_directory,
            endpoint,
            ticket,
            &recipient_ticket_file,
            validity_seconds,
        )
        .await
        {
            Ok(report) => RuntimeIpcResponse::EndpointAnnouncementsPushed(Box::new(report)),
            Err(error) => RuntimeIpcResponse::Error {
                message: format!("{error:#}"),
            },
        },
        RuntimeIpcCommand::ConversationList => {
            let conversations = StateDirectoryLock::acquire(state_directory)
                .context("lock runtime state for IPC conversation snapshot")
                .and_then(|_lock| collect_runtime_conversation_list(state_directory));
            match conversations {
                Ok(conversations) => RuntimeIpcResponse::ConversationList(conversations),
                Err(error) => RuntimeIpcResponse::Error {
                    message: format!("{error:#}"),
                },
            }
        }
        RuntimeIpcCommand::HistoryPage {
            conversation,
            cursor,
            limit,
        } => {
            let history = StateDirectoryLock::acquire(state_directory)
                .context("lock runtime state for IPC history snapshot")
                .and_then(|_lock| {
                    collect_runtime_history_page(state_directory, &conversation, cursor, limit)
                });
            match history {
                Ok(page) => RuntimeIpcResponse::HistoryPage(page),
                Err(error) => RuntimeIpcResponse::Error {
                    message: format!("{error:#}"),
                },
            }
        }
        RuntimeIpcCommand::WaitForChange { .. } => RuntimeIpcResponse::Error {
            message: "change waits are handled by the IPC acceptor".to_owned(),
        },
        RuntimeIpcCommand::Shutdown => RuntimeIpcResponse::ShutdownAccepted,
    };
    let _ = response_sender.send(response);
    RuntimeIpcDispatchOutcome {
        shutdown_requested,
        state_changed,
    }
}

fn resolve_runtime_ipc_descriptor_path(
    state_directory: &Path,
    descriptor_path: &Path,
) -> Result<PathBuf> {
    let absolute = if descriptor_path.is_absolute() {
        descriptor_path.to_path_buf()
    } else {
        std::env::current_dir()
            .context("read current directory for runtime IPC descriptor")?
            .join(descriptor_path)
    };
    let file_name = absolute
        .file_name()
        .context("runtime IPC descriptor path has no file name")?;
    let parent = absolute
        .parent()
        .context("runtime IPC descriptor path has no parent")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("create runtime IPC descriptor parent {}", parent.display()))?;
    let resolved = fs::canonicalize(parent)
        .with_context(|| format!("resolve runtime IPC descriptor parent {}", parent.display()))?
        .join(file_name);
    let canonical_state = fs::canonicalize(state_directory)
        .context("resolve local state directory for runtime IPC")?;
    ensure!(
        !resolved.starts_with(&canonical_state),
        "runtime IPC descriptor must live outside the protected state directory"
    );
    Ok(resolved)
}

fn resolve_runtime_received_endpoint_descriptor_directory(
    state_directory: &Path,
    ipc_file: Option<&Path>,
    ticket_file: Option<&Path>,
) -> Result<Option<PathBuf>> {
    let Some(anchor) = ipc_file.or(ticket_file) else {
        return Ok(None);
    };
    let absolute = if anchor.is_absolute() {
        anchor.to_path_buf()
    } else {
        std::env::current_dir()
            .context("read current directory for received endpoint descriptors")?
            .join(anchor)
    };
    let parent = absolute
        .parent()
        .context("runtime endpoint-announcement anchor has no parent")?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "create received endpoint descriptor parent {}",
            parent.display()
        )
    })?;
    let parent = fs::canonicalize(parent).with_context(|| {
        format!(
            "resolve received endpoint descriptor parent {}",
            parent.display()
        )
    })?;
    let directory = parent.join("kilogram-received-endpoints");
    let canonical_state = fs::canonicalize(state_directory)
        .context("resolve protected state for received endpoint descriptors")?;
    ensure!(
        !directory.starts_with(canonical_state),
        "received endpoint descriptor directory must live outside protected runtime state"
    );
    Ok(Some(directory))
}

fn listen(options: ListenOptions) -> CommandFuture {
    Box::pin(listen_inner(options))
}

struct PreparedRuntimeListener {
    device_state: DeviceState,
    listener_certificate: DeviceCertificate,
    listener_directory: AccountPrekeyDirectory,
    listener_prekey_pool: SignedPrekeyPool,
    authority_snapshot_store: AuthoritySnapshotStoreOutcome,
    device_directory_state: RuntimeDeviceDirectoryState,
}

#[derive(Clone)]
struct RuntimeDeviceDirectoryState {
    state_dir: PathBuf,
    launch_device_list_file: PathBuf,
    applied_device_list_file: PathBuf,
    device_list_digest: [u8; 32],
    latest_receipt: Option<SignedRuntimeDeviceDirectoryReceipt>,
    profile_convergence_status: &'static str,
    restart_recovery_status: &'static str,
}

impl RuntimeDeviceDirectoryState {
    fn status(&self, ticket: &ConnectionTicket) -> Result<RuntimeIpcDeviceDirectoryStatus> {
        let receipt_id = self
            .latest_receipt
            .as_ref()
            .map(SignedRuntimeDeviceDirectoryReceipt::receipt_id)
            .transpose()?
            .map(|receipt_id| receipt_id.to_string());
        Ok(RuntimeIpcDeviceDirectoryStatus {
            account_id: ticket.listener_account_id(),
            local_device_id: ticket.listener_device_id(),
            state_dir: self.state_dir.clone(),
            authority_revision: ticket.listener_authority_snapshot().revision(),
            active_device_count: ticket.listener_directory().device_list().devices().len(),
            receipt_id,
            receipt_generation: self
                .latest_receipt
                .as_ref()
                .map(SignedRuntimeDeviceDirectoryReceipt::generation),
            device_list_digest: encode_hex(&self.device_list_digest),
            applied_device_list_file: self.applied_device_list_file.clone(),
            launch_device_list_file: self.launch_device_list_file.clone(),
            profile_convergence_status: self.profile_convergence_status.to_owned(),
            restart_recovery_status: self.restart_recovery_status.to_owned(),
        })
    }
}

fn apply_runtime_own_device_directory(
    state_directory: &Path,
    endpoint: &Endpoint,
    current_ticket: &ConnectionTicket,
    current_directory_state: &RuntimeDeviceDirectoryState,
    ticket_file: Option<&Path>,
    device_list_file: &Path,
) -> Result<(
    ConnectionTicket,
    RuntimeDeviceDirectoryState,
    RuntimeIpcDeviceDirectoryUpdate,
)> {
    ensure!(
        device_list_file.is_absolute(),
        "refreshed runtime device-list path must be absolute"
    );
    let metadata = fs::symlink_metadata(device_list_file).with_context(|| {
        format!(
            "inspect refreshed runtime device list {}",
            device_list_file.display()
        )
    })?;
    ensure!(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "refreshed runtime device list must be a regular non-symlink file"
    );
    let device_list_file = fs::canonicalize(device_list_file).with_context(|| {
        format!(
            "resolve refreshed runtime device list {}",
            device_list_file.display()
        )
    })?;
    let new_device_list = AccountDeviceListSnapshot::decode_and_verify(
        &fs::read(&device_list_file).with_context(|| {
            format!(
                "read refreshed runtime device list {}",
                device_list_file.display()
            )
        })?,
    )
    .context("decode and verify refreshed runtime device list")?;
    let device_list_digest = runtime_device_list_digest(&new_device_list)?;
    let old_device_list = current_ticket.listener_directory().device_list();
    new_device_list
        .verify_for_account(current_ticket.listener_account_id())
        .context("verify refreshed runtime device-list account")?;
    ensure!(
        new_device_list.revision() >= old_device_list.revision(),
        "refreshed runtime device list rolls authority revision back from {} to {}",
        old_device_list.revision(),
        new_device_list.revision()
    );
    if new_device_list.revision() == old_device_list.revision() {
        ensure!(
            &new_device_list == old_device_list,
            "refreshed runtime device list equivocates at authority revision {}",
            new_device_list.revision()
        );
    }

    let device_state = load_command_device_state(state_directory)?;
    let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
    let local_certificate = trust
        .load_certificate()
        .context("load runtime certificate for directory refresh")?;
    ensure!(
        new_device_list.certificate_for(local_certificate.device_id()) == Some(&local_certificate),
        "the running device is absent or changed in the refreshed device list"
    );

    let new_device_ids = new_device_list
        .devices()
        .iter()
        .map(DeviceCertificate::device_id)
        .collect::<BTreeSet<_>>();
    for certificate in new_device_list.devices() {
        ensure!(
            old_device_list.certificate_for(certificate.device_id()) == Some(certificate),
            "live device-directory refresh cannot add or replace device {}",
            certificate.device_id()
        );
    }
    let removed_device_ids = old_device_list
        .devices()
        .iter()
        .map(DeviceCertificate::device_id)
        .filter(|device_id| !new_device_ids.contains(device_id))
        .collect::<Vec<_>>();
    let revoked_device_ids = new_device_list
        .authority_snapshot()
        .revocations()
        .iter()
        .map(|revocation| revocation.device_id())
        .collect::<BTreeSet<_>>();
    for device_id in &removed_device_ids {
        ensure!(
            revoked_device_ids.contains(device_id),
            "device {device_id} disappeared without a permanent Root revocation"
        );
    }

    let retained_pools = current_ticket
        .listener_directory()
        .pools()
        .iter()
        .filter(|pool| new_device_ids.contains(&pool.device_id()))
        .cloned()
        .collect::<Vec<_>>();
    let new_directory = AccountPrekeyDirectory::new(new_device_list.clone(), retained_pools)
        .context("assemble refreshed complete runtime prekey directory")?;
    new_directory
        .verify_at(unix_time_now()?)
        .context("verify refreshed runtime prekey directory freshness")?;

    let snapshot = load_runtime_state_snapshot(
        state_directory,
        current_ticket.listener_account_id(),
        current_ticket.listener_device_id(),
    )?;
    let previous_receipt = snapshot.latest_device_directory_receipt();
    let directory_receipt = match previous_receipt {
        Some(receipt)
            if receipt.authority_revision() == new_device_list.revision()
                && receipt.device_list_digest() == device_list_digest
                && receipt.active_device_count() == new_device_list.devices().len()
                && receipt.device_list_file() == device_list_file =>
        {
            receipt.clone()
        }
        previous => SignedRuntimeDeviceDirectoryReceipt::sign(
            device_state.identity(),
            current_ticket.listener_account_id(),
            new_device_list.revision(),
            device_list_digest,
            new_device_list.devices().len(),
            device_list_file.clone(),
            previous,
        )?,
    };
    let directory_receipt_id = directory_receipt.receipt_id()?;
    let directory_receipt_path = runtime_device_directory_receipt_relative_path(
        directory_receipt.generation(),
        directory_receipt_id,
    );
    let directory_receipt_bytes = directory_receipt.encode()?;
    let mut pending_unmaterialized_messages = 0_usize;
    let mut pending_materialized_messages = 0_usize;
    for (queue_id, _) in snapshot
        .queued
        .iter()
        .filter(|(queue_id, _)| !snapshot.delivered.contains_key(queue_id))
    {
        if !snapshot.materialized.contains_key(queue_id) {
            pending_unmaterialized_messages += 1;
            continue;
        }
        pending_materialized_messages += 1;
    }

    let (
        authority_store,
        ratchet_session_records_retired,
        prekey_observations_retired,
        directory_receipt_store,
    ) = run_state_transaction(state_directory, |transaction| {
        transaction.prepare_trust_workspace()?;
        let authority_store = device_state
            .install_own_authority_snapshot(new_device_list.authority_snapshot())
            .context("install refreshed own authority in DB-primary trust workspace")?;
        let ratchet_state = transaction.load_ratchet_state()?;
        let mut sessions = 0_usize;
        let mut observations = 0_usize;
        for device_id in &revoked_device_ids {
            let retirement = ratchet_state
                .retire_peer_device(*device_id)
                .with_context(|| format!("retire ratchet state for device {device_id}"))?;
            sessions += usize::from(retirement.session_removed);
            observations += usize::from(retirement.prekey_observation_removed);
        }
        let receipt_store = persist_runtime_record(
            state_directory,
            &directory_receipt_path,
            &directory_receipt_bytes,
            transaction,
        )?;
        Ok((authority_store, sessions, observations, receipt_store))
    })?;

    let replacement = ConnectionTicket::new(
        endpoint.addr(),
        device_state.identity(),
        local_certificate,
        new_directory,
        current_ticket.allowed_requester_account_id(),
        current_ticket.route_policy(),
    )?;
    let encoded = replacement.encode()?;
    let ticket_published = if let Some(path) = ticket_file {
        publish_runtime_ticket(path, encoded.as_bytes())?;
        true
    } else {
        false
    };
    println!("runtime_directory_authority_store={authority_store:?}");
    println!("runtime_directory_receipt_id={directory_receipt_id}");
    println!(
        "runtime_directory_receipt_generation={}",
        directory_receipt.generation()
    );
    println!("runtime_directory_receipt_store={directory_receipt_store:?}");
    println!("runtime_directory_ticket={encoded}");
    if let Some(path) = ticket_file {
        println!("runtime_directory_ticket_file={}", path.display());
        println!("runtime_directory_ticket_publish=atomic-replace");
    }

    let directory_state = RuntimeDeviceDirectoryState {
        state_dir: current_directory_state.state_dir.clone(),
        launch_device_list_file: current_directory_state.launch_device_list_file.clone(),
        applied_device_list_file: device_list_file.clone(),
        device_list_digest,
        latest_receipt: Some(directory_receipt),
        profile_convergence_status: if device_list_file
            == current_directory_state.launch_device_list_file
        {
            "current"
        } else {
            "convergence-required"
        },
        restart_recovery_status: "authenticated-receipt-current",
    };
    let directory_status = directory_state.status(&replacement)?;
    Ok((
        replacement,
        directory_state,
        RuntimeIpcDeviceDirectoryUpdate {
            account_id: current_ticket.listener_account_id(),
            local_device_id: current_ticket.listener_device_id(),
            previous_authority_revision: old_device_list.revision(),
            authority_revision: new_device_list.revision(),
            active_device_count: new_device_list.devices().len(),
            removed_device_ids: revoked_device_ids.into_iter().collect(),
            ratchet_session_records_retired,
            prekey_observations_retired,
            pending_unmaterialized_messages,
            pending_materialized_messages,
            ticket_published,
            launch_profile_update_required: device_list_file
                != current_directory_state.launch_device_list_file,
            future_recipient_slot_status: "removed-devices-excluded-by-refreshed-ticket".to_owned(),
            preexisting_recipient_slot_status: "immutable-cannot-be-remotely-rewritten-or-erased"
                .to_owned(),
            history_availability_status: "existing-copies-remain-readable".to_owned(),
            directory_status,
        },
    ))
}

async fn publish_runtime_own_ticket(
    state_directory: &Path,
    current_ticket: &ConnectionTicket,
    conversation: &str,
    peer_account_id: AccountId,
    service_base_url: &str,
    ttl_seconds: u64,
    renew_before_seconds: u64,
) -> Result<RuntimeIpcTicketPublication> {
    ensure!(
        (MIN_TICKET_PUBLICATION_TTL_SECONDS..=MAX_TICKET_PUBLICATION_TTL_SECONDS)
            .contains(&ttl_seconds),
        "ticket publication TTL must be between {MIN_TICKET_PUBLICATION_TTL_SECONDS} and \
         {MAX_TICKET_PUBLICATION_TTL_SECONDS} seconds"
    );
    ensure!(
        renew_before_seconds <= ttl_seconds,
        "ticket publication renewal lead exceeds its TTL"
    );
    let store_client = TicketPublicationStoreClient::new(service_base_url)?;
    let (
        contact_id,
        channel_id,
        publication,
        publication_id,
        publication_store,
        encrypted,
        write_capability,
    ) = with_locked_state(state_directory, || {
        let device_state = load_command_device_state(state_directory)?;
        let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
        let local_certificate = trust
            .load_certificate()
            .context("load local certificate for ticket publication")?;
        let local_authority = trust
            .load_own_authority_snapshot(&local_certificate)
            .context("load local authority for ticket publication")?;
        ensure!(
            current_ticket.listener_account_id() == local_certificate.account_id()
                && current_ticket.listener_device_id() == device_state.identity().device_id()
                && current_ticket.allowed_requester_account_id() == peer_account_id,
            "running ticket does not target the selected contact account"
        );
        let snapshot = load_runtime_state_snapshot(
            state_directory,
            local_certificate.account_id(),
            device_state.identity().device_id(),
        )?;
        let contact = exact_runtime_contact(&snapshot, conversation, peer_account_id)?;
        let membership = trust
            .load_conversation_membership(contact.conversation_id().scope_id())
            .context("load conversation membership for ticket publication")?;
        require_conversation_participants(
            &membership,
            local_certificate.account_id(),
            peer_account_id,
        )?;
        let recipient_candidates = load_runtime_endpoint_candidate_set(
            &snapshot,
            contact,
            &local_certificate,
            &local_authority,
        )
        .context("load recipient endpoint set for ticket publication")?;
        let recipient_ticket = &recipient_candidates
            .first()
            .context("recipient endpoint set is empty")?
            .ticket;
        let write_capability =
            ticket_publication_write_capability(device_state.identity(), peer_account_id);
        ensure!(
            current_ticket.ticket_publication_write_key() == write_capability.write_key(),
            "running ticket write capability does not match its selected contact account"
        );
        let channel_id = write_capability.write_key().channel_id();
        let encoded_ticket = current_ticket.encode()?;
        let now_unix_seconds = unix_time_now().context("read time for ticket publication")?;
        let previous = snapshot.latest_ticket_publication(channel_id);
        let publication = match previous {
            Some(previous)
                if previous.ticket() == encoded_ticket
                    && previous.expires_at_unix_seconds()
                        > now_unix_seconds.saturating_add(renew_before_seconds) =>
            {
                previous.clone()
            }
            previous => SignedTicketPublication::sign(
                device_state.identity(),
                channel_id,
                local_certificate.account_id(),
                peer_account_id,
                encoded_ticket,
                now_unix_seconds,
                ttl_seconds,
                previous,
            )?,
        };
        let publication_id = publication.publication_id()?;
        let publication_path = runtime_ticket_publication_relative_path(
            channel_id,
            publication.generation(),
            publication_id,
        );
        let publication_bytes = publication.encode()?;
        let publication_store = run_state_transaction(state_directory, |transaction| {
            persist_runtime_record(
                state_directory,
                &publication_path,
                &publication_bytes,
                transaction,
            )
        })?;
        let recipients = recipient_ticket
            .listener_directory()
            .device_list()
            .devices()
            .iter()
            .map(|certificate| (certificate.device_id(), certificate.encryption_public_key()))
            .collect::<Vec<_>>();
        let encrypted = EncryptedTicketPublication::seal(&publication, &recipients)?;
        Ok((
            contact.contact_id().to_string(),
            channel_id,
            publication,
            publication_id,
            publication_store,
            encrypted,
            write_capability,
        ))
    })?;
    let encrypted_record_bytes = store_client.put(&encrypted, &write_capability).await?;
    Ok(RuntimeIpcTicketPublication {
        contact_id,
        channel_id: channel_id.to_string(),
        publication_id: publication_id.to_string(),
        publication_generation: publication.generation(),
        expires_at_unix_seconds: publication.expires_at_unix_seconds(),
        recipient_device_count: encrypted.recipient_count(),
        encrypted_record_bytes,
        service_base_url: store_client.base_url().to_owned(),
        local_store_status: store_outcome_name(publication_store).to_owned(),
        upload_status: "confirmed-http-success".to_owned(),
        lookup_privacy_status:
            "opaque-hpke-recipient-slots-self-authenticating-capability-channel-traffic-analysis-visible"
                .to_owned(),
        first_contact_freshness: "bootstrap-contact-required".to_owned(),
    })
}

fn runtime_endpoint_ticket_refresh_channel(
    state_directory: &Path,
    conversation: &str,
    peer_account_id: AccountId,
    enrollment: &RuntimeEndpointEnrollment,
) -> Result<TicketPublicationChannelId> {
    with_locked_state(state_directory, || {
        let device_state = load_command_device_state(state_directory)?;
        let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
        let local_certificate = trust
            .load_certificate()
            .context("load local certificate for ticket publication lookup")?;
        let snapshot = load_runtime_state_snapshot(
            state_directory,
            local_certificate.account_id(),
            device_state.identity().device_id(),
        )?;
        let contact = exact_runtime_contact(&snapshot, conversation, peer_account_id)?;
        let current_enrollment =
            exact_runtime_endpoint_enrollment(&snapshot, contact, enrollment.peer_device_id)?;
        ensure!(
            &current_enrollment == enrollment,
            "runtime endpoint candidate changed before ticket publication lookup"
        );
        let membership = trust
            .load_conversation_membership(contact.conversation_id().scope_id())
            .context("load conversation membership for ticket publication lookup")?;
        require_conversation_participants(
            &membership,
            local_certificate.account_id(),
            peer_account_id,
        )?;
        let local_authority = trust
            .load_own_authority_snapshot(&local_certificate)
            .context("load local authority for ticket publication lookup")?;
        if let Ok(binding) = exact_runtime_endpoint_publication_binding(
            &snapshot,
            local_certificate.account_id(),
            contact,
            &current_enrollment,
        ) {
            return Ok(binding.ticket_publication_write_key().channel_id());
        }

        // Compatibility migration for contacts enrolled before the durable
        // binding existed. The exact old ticket must still authenticate every
        // immutable identity and contract field, but its prekeys may be
        // expired because it is never returned as a connection candidate.
        let authenticated_ticket = load_authenticated_runtime_endpoint_ticket_for_binding(
            &current_enrollment,
            &local_certificate,
            &local_authority,
        )
        .context("recover publication-channel binding from legacy endpoint descriptor")?;
        let binding = sign_runtime_endpoint_publication_binding(
            device_state.identity(),
            local_certificate.account_id(),
            contact,
            &current_enrollment,
            authenticated_ticket.ticket_publication_write_key(),
        )?;
        run_state_transaction(state_directory, |transaction| {
            persist_runtime_record(
                state_directory,
                &runtime_endpoint_publication_binding_relative_path(binding.binding_id()),
                &binding.encode()?,
                transaction,
            )
        })?;
        Ok(binding.ticket_publication_write_key().channel_id())
    })
}

fn install_runtime_endpoint_ticket_refresh(
    state_directory: &Path,
    conversation: &str,
    peer_account_id: AccountId,
    enrollment: &RuntimeEndpointEnrollment,
    channel_id: TicketPublicationChannelId,
    encrypted: EncryptedTicketPublication,
) -> Result<RuntimeIpcEndpointTicketRefresh> {
    ensure!(
        encrypted.channel_id() == channel_id,
        "ticket publication service returned another lookup channel"
    );
    with_locked_state(state_directory, || {
        let device_state = load_command_device_state(state_directory)?;
        let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
        let local_certificate = trust
            .load_certificate()
            .context("load local certificate for ticket publication fetch")?;
        let local_authority = trust
            .load_own_authority_snapshot(&local_certificate)
            .context("load local authority for ticket publication fetch")?;
        let snapshot = load_runtime_state_snapshot(
            state_directory,
            local_certificate.account_id(),
            device_state.identity().device_id(),
        )?;
        let contact = exact_runtime_contact(&snapshot, conversation, peer_account_id)?;
        let current_enrollment =
            exact_runtime_endpoint_enrollment(&snapshot, contact, enrollment.peer_device_id)?;
        ensure!(
            &current_enrollment == enrollment,
            "runtime endpoint candidate changed during ticket publication fetch"
        );
        let membership = trust
            .load_conversation_membership(contact.conversation_id().scope_id())
            .context("reload conversation membership after ticket publication fetch")?;
        require_conversation_participants(
            &membership,
            local_certificate.account_id(),
            peer_account_id,
        )?;
        let binding = exact_runtime_endpoint_publication_binding(
            &snapshot,
            local_certificate.account_id(),
            contact,
            &current_enrollment,
        )
        .context("reload durable endpoint publication-channel binding")?;
        let expected_channel_id = binding.ticket_publication_write_key().channel_id();
        ensure!(
            expected_channel_id == channel_id,
            "ticket publication contact changed during fetch"
        );
        let now_unix_seconds = unix_time_now().context("read time for ticket publication fetch")?;
        let publication = encrypted.open(
            device_state.identity().device_id(),
            device_state.encryption(),
            now_unix_seconds,
        )?;
        ensure!(
            publication.channel_id() == channel_id
                && publication.publisher_account_id() == peer_account_id
                && publication.publisher_device_id() == current_enrollment.peer_device_id
                && publication.recipient_account_id() == local_certificate.account_id(),
            "ticket publication identity does not match the selected endpoint candidate"
        );
        let ticket = ConnectionTicket::decode(publication.ticket())
            .context("decode ticket from signed publication")?;
        ticket.verify_listener_account(peer_account_id)?;
        let authorized_peer = ticket.verify_listener_authorization(peer_account_id)?;
        ensure!(
            authorized_peer.device_id() == current_enrollment.peer_device_id
                && ticket.route_policy() == current_enrollment.route_policy
                && ticket.allowed_requester_account_id() == local_certificate.account_id()
                && ticket.ticket_publication_write_key() == binding.ticket_publication_write_key()
                && ticket.ticket_publication_write_key().channel_id() == channel_id,
            "published ticket does not match the selected endpoint-candidate contract"
        );
        verify_device_authorization_with_snapshot(
            ticket.allowed_requester_account_id(),
            &local_certificate,
            &local_authority,
            &DeviceCapability::MESSAGING,
        )
        .context("local device is not authorized by the published ticket")?;

        let imported_high_water = snapshot.accepted_endpoint_observation_high_water(channel_id)?;
        if let Some((generation, publication_id, ticket_digest)) = imported_high_water {
            ensure!(
                publication.generation() > generation
                    || (publication.generation() == generation
                        && publication.publication_id()? == publication_id
                        && publication.ticket_digest() == ticket_digest),
                "ticket publication rolls back or equivocates against an authorized sibling-device high-water"
            );
        }
        let previous_observation = snapshot.latest_ticket_observation(channel_id);
        let first_contact_freshness = if previous_observation.is_some() {
            "local-monotonic-high-water"
        } else if imported_high_water.is_some() {
            "authorized-sibling-high-water"
        } else {
            "non-expired-signed-first-observation-no-global-freshness"
        };
        let observation = match previous_observation {
            Some(previous)
                if publication.generation() == previous.publication_generation()
                    && publication.publication_id()? == previous.publication_id()
                    && publication.ticket_digest() == previous.ticket_digest() =>
            {
                previous.clone()
            }
            Some(previous) => {
                ensure!(
                    publication.generation() > previous.publication_generation(),
                    "ticket publication rolls the locally observed generation back or equivocates"
                );
                SignedTicketPublicationObservation::sign(
                    device_state.identity(),
                    local_certificate.account_id(),
                    &publication,
                    now_unix_seconds,
                    Some(previous),
                )?
            }
            None => SignedTicketPublicationObservation::sign(
                device_state.identity(),
                local_certificate.account_id(),
                &publication,
                now_unix_seconds,
                None,
            )?,
        };
        let observation_id = observation.observation_id()?;
        let observation_path = runtime_ticket_observation_relative_path(
            channel_id,
            observation.observation_generation(),
            observation_id,
        );
        let observation_bytes = observation.encode()?;
        let (observation_store, retired_sessions, retired_observations) =
            run_state_transaction(state_directory, |transaction| {
                transaction.prepare_trust_workspace()?;
                device_state
                    .pin_peer_authority_snapshot(ticket.listener_authority_snapshot())
                    .context("pin published peer authority in DB-primary trust workspace")?;
                let ratchet_state = transaction.load_ratchet_state()?;
                let mut retired_sessions = 0_usize;
                let mut retired_observations = 0_usize;
                for revocation in ticket.listener_authority_snapshot().revocations() {
                    let retirement = ratchet_state
                        .retire_peer_device(revocation.device_id())
                        .with_context(|| {
                            format!(
                                "retire published peer ratchet state for revoked device {}",
                                revocation.device_id()
                            )
                        })?;
                    retired_sessions += usize::from(retirement.session_removed);
                    retired_observations += usize::from(retirement.prekey_observation_removed);
                }
                ratchet_state
                    .observe_prekey_directory(ticket.listener_directory(), now_unix_seconds)
                    .context("observe published peer prekey directory")?;
                let store = persist_runtime_record(
                    state_directory,
                    &observation_path,
                    &observation_bytes,
                    transaction,
                )?;
                Ok((store, retired_sessions, retired_observations))
            })?;
        if retired_sessions != 0 || retired_observations != 0 {
            println!("runtime_peer_ratchet_sessions_retired={retired_sessions}");
            println!("runtime_peer_prekey_observations_retired={retired_observations}");
        }
        let descriptor_file = &current_enrollment.descriptor_file;
        let metadata = fs::symlink_metadata(descriptor_file).with_context(|| {
            format!(
                "inspect runtime peer descriptor {}",
                descriptor_file.display()
            )
        })?;
        ensure!(
            metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
            "runtime peer descriptor must remain a regular non-symlink file"
        );
        ensure!(
            fs::canonicalize(descriptor_file)?.as_path() == descriptor_file.as_path(),
            "runtime peer descriptor path changed since contact enrollment"
        );
        publish_runtime_ticket(descriptor_file, publication.ticket().as_bytes())?;
        Ok(RuntimeIpcEndpointTicketRefresh {
            peer_device_id: current_enrollment.peer_device_id,
            primary: current_enrollment.primary,
            route_policy: runtime_ipc_route_policy(current_enrollment.route_policy),
            descriptor_file: descriptor_file.clone(),
            state: RuntimeIpcEndpointCandidateState::Usable,
            channel_id: Some(channel_id.to_string()),
            publication_id: Some(publication.publication_id()?.to_string()),
            publication_generation: Some(publication.generation()),
            expires_at_unix_seconds: Some(publication.expires_at_unix_seconds()),
            publisher_account_id: Some(publication.publisher_account_id()),
            authority_revision: Some(ticket.listener_authority_snapshot().revision()),
            active_device_count: Some(ticket.listener_directory().device_list().devices().len()),
            local_observation_status: Some(store_outcome_name(observation_store).to_owned()),
            descriptor_publish_status: Some("atomic-replace".to_owned()),
            freshness_status: Some("signed-non-expired-local-high-water".to_owned()),
            first_contact_freshness: Some(first_contact_freshness.to_owned()),
            detail: "refreshed-and-installed".to_owned(),
        })
    })
}

async fn refresh_runtime_contact_ticket(
    state_directory: &Path,
    conversation: &str,
    peer_account_id: AccountId,
    service_base_url: &str,
) -> Result<RuntimeIpcContactTicketRefresh> {
    let store_client = TicketPublicationStoreClient::new(service_base_url)?;
    let (contact_id, enrollments) = with_locked_state(state_directory, || {
        let device_state = load_command_device_state(state_directory)?;
        let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
        let local_certificate = trust
            .load_certificate()
            .context("load local certificate for multi-endpoint ticket refresh")?;
        let snapshot = load_runtime_state_snapshot(
            state_directory,
            local_certificate.account_id(),
            device_state.identity().device_id(),
        )?;
        let contact = exact_runtime_contact(&snapshot, conversation, peer_account_id)?;
        let membership = trust
            .load_conversation_membership(contact.conversation_id().scope_id())
            .context("load conversation membership for multi-endpoint ticket refresh")?;
        require_conversation_participants(
            &membership,
            local_certificate.account_id(),
            peer_account_id,
        )?;
        Ok((
            contact.contact_id().to_string(),
            runtime_endpoint_enrollments(&snapshot, contact)?,
        ))
    })?;

    let mut results = Vec::new();
    let mut refresh_tasks = Vec::with_capacity(enrollments.len());
    for enrollment in enrollments {
        match runtime_endpoint_ticket_refresh_channel(
            state_directory,
            conversation,
            peer_account_id,
            &enrollment,
        ) {
            Ok(channel_id) => {
                let store_client = store_client.clone();
                refresh_tasks.push((
                    enrollment,
                    channel_id,
                    tokio::spawn(async move { store_client.get(channel_id).await }),
                ));
            }
            Err(error) => {
                results.push(failed_runtime_endpoint_ticket_refresh(
                    enrollment, None, error,
                ));
            }
        }
    }
    for (enrollment, channel_id, refresh_task) in refresh_tasks {
        let fetched = refresh_task
            .await
            .context("join bounded endpoint ticket-refresh task")?;
        let result = match fetched {
            Ok(encrypted) => install_runtime_endpoint_ticket_refresh(
                state_directory,
                conversation,
                peer_account_id,
                &enrollment,
                channel_id,
                encrypted,
            ),
            Err(error) => Err(error),
        };
        results.push(match result {
            Ok(refresh) => refresh,
            Err(error) => {
                failed_runtime_endpoint_ticket_refresh(enrollment, Some(channel_id), error)
            }
        });
    }
    results.sort_by(|left, right| {
        right.primary.cmp(&left.primary).then_with(|| {
            left.peer_device_id
                .as_bytes()
                .cmp(right.peer_device_id.as_bytes())
        })
    });
    let refreshed_endpoint_candidate_count = u8::try_from(
        results
            .iter()
            .filter(|result| result.state == RuntimeIpcEndpointCandidateState::Usable)
            .count(),
    )
    .context("refreshed endpoint-candidate count exceeds IPC representation")?;
    let endpoint_candidate_count = u8::try_from(results.len())
        .context("endpoint-candidate count exceeds IPC representation")?;
    Ok(RuntimeIpcContactTicketRefresh {
        contact_id,
        endpoint_candidate_count,
        refreshed_endpoint_candidate_count,
        complete: refreshed_endpoint_candidate_count == endpoint_candidate_count,
        results,
    })
}

fn failed_runtime_endpoint_ticket_refresh(
    enrollment: RuntimeEndpointEnrollment,
    channel_id: Option<TicketPublicationChannelId>,
    error: anyhow::Error,
) -> RuntimeIpcEndpointTicketRefresh {
    RuntimeIpcEndpointTicketRefresh {
        peer_device_id: enrollment.peer_device_id,
        primary: enrollment.primary,
        route_policy: runtime_ipc_route_policy(enrollment.route_policy),
        descriptor_file: enrollment.descriptor_file,
        state: RuntimeIpcEndpointCandidateState::Stale,
        channel_id: channel_id.map(|channel_id| channel_id.to_string()),
        publication_id: None,
        publication_generation: None,
        expires_at_unix_seconds: None,
        publisher_account_id: None,
        authority_revision: None,
        active_device_count: None,
        local_observation_status: None,
        descriptor_publish_status: None,
        freshness_status: None,
        first_contact_freshness: None,
        detail: format!("refresh-failed: {error:#}"),
    }
}

#[allow(clippy::too_many_arguments)]
fn configure_runtime_ticket_automation(
    state_directory: &Path,
    conversation: String,
    peer_account_id: AccountId,
    enabled: bool,
    service_base_url: String,
    ttl_seconds: u64,
    refresh_before_seconds: u64,
    retry_base_seconds: u64,
    retry_max_seconds: u64,
    allow_ethernet: bool,
    allow_wifi: bool,
    allow_mobile: bool,
    allow_unknown_network: bool,
    current_network: RuntimeIpcNetworkClass,
) -> Result<RuntimeIpcTicketAutomationStatus> {
    ensure!(
        (MIN_TICKET_PUBLICATION_TTL_SECONDS..=MAX_TICKET_PUBLICATION_TTL_SECONDS)
            .contains(&ttl_seconds),
        "ticket automation TTL is outside publication bounds"
    );
    ensure!(
        (MIN_REFRESH_BEFORE_SECONDS..=MAX_REFRESH_BEFORE_SECONDS).contains(&refresh_before_seconds)
            && refresh_before_seconds < ttl_seconds,
        "ticket automation refresh lead must be within bounds and shorter than TTL"
    );
    ensure!(
        (1..=MAX_AUTOMATION_RETRY_SECONDS).contains(&retry_base_seconds)
            && retry_max_seconds >= retry_base_seconds
            && retry_max_seconds <= MAX_AUTOMATION_RETRY_SECONDS,
        "ticket automation retry bounds are invalid"
    );
    let store_client = TicketPublicationStoreClient::new(&service_base_url)?;
    let service_base_url = store_client.base_url().to_owned();
    let device_state = load_command_device_state(state_directory)?;
    let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
    let certificate = trust.load_certificate()?;
    let snapshot = load_runtime_state_snapshot(
        state_directory,
        certificate.account_id(),
        device_state.identity().device_id(),
    )?;
    let contact = exact_runtime_contact(&snapshot, &conversation, peer_account_id)?;
    let previous = snapshot.latest_ticket_automation_policy(contact.contact_id());
    let candidate = SignedTicketAutomationPolicy::sign(
        device_state.identity(),
        certificate.account_id(),
        contact.contact_id(),
        conversation,
        peer_account_id,
        unix_time_now()?,
        enabled,
        service_base_url,
        ttl_seconds,
        refresh_before_seconds,
        retry_base_seconds,
        retry_max_seconds,
        allow_ethernet,
        allow_wifi,
        allow_mobile,
        allow_unknown_network,
        previous,
    )?;
    let policy = if previous.is_some_and(|previous| previous.same_configuration(&candidate)) {
        previous
            .context("ticket automation previous policy disappeared")?
            .clone()
    } else {
        let policy_id = candidate.policy_id()?;
        let path = runtime_ticket_automation_policy_relative_path(policy_id);
        let bytes = candidate.encode()?;
        run_state_transaction(state_directory, |transaction| {
            persist_runtime_record(state_directory, &path, &bytes, transaction)
        })?;
        candidate
    };
    let snapshot = load_runtime_state_snapshot(
        state_directory,
        certificate.account_id(),
        device_state.identity().device_id(),
    )?;
    ticket_automation_status_for_policy(&snapshot, &policy, current_network, unix_time_now()?)
}

fn current_runtime_network_class() -> RuntimeIpcNetworkClass {
    match system_recovery_platform_context().network_class() {
        RecoveryNetworkClass::Ethernet => RuntimeIpcNetworkClass::Ethernet,
        RecoveryNetworkClass::Wifi => RuntimeIpcNetworkClass::Wifi,
        RecoveryNetworkClass::Mobile => RuntimeIpcNetworkClass::Mobile,
        RecoveryNetworkClass::Unknown => RuntimeIpcNetworkClass::Unknown,
    }
}

fn collect_runtime_ticket_automation_status(
    state_directory: &Path,
    current_network: RuntimeIpcNetworkClass,
) -> Result<Vec<RuntimeIpcTicketAutomationStatus>> {
    let device_state = load_command_device_state(state_directory)?;
    let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
    let certificate = trust.load_certificate()?;
    let snapshot = load_runtime_state_snapshot(
        state_directory,
        certificate.account_id(),
        device_state.identity().device_id(),
    )?;
    let now = unix_time_now()?;
    snapshot
        .ticket_automation_policies
        .values()
        .filter_map(|policies| policies.last())
        .map(|policy| ticket_automation_status_for_policy(&snapshot, policy, current_network, now))
        .collect()
}

fn ticket_automation_status_for_policy(
    snapshot: &RuntimeStateSnapshot,
    policy: &SignedTicketAutomationPolicy,
    current_network: RuntimeIpcNetworkClass,
    now_unix_seconds: u64,
) -> Result<RuntimeIpcTicketAutomationStatus> {
    let network_allowed = policy.allows_network(current_network);
    Ok(RuntimeIpcTicketAutomationStatus {
        contact_id: policy.contact_id().to_string(),
        conversation: policy.conversation().to_owned(),
        peer_account_id: policy.peer_account_id(),
        enabled: policy.enabled(),
        policy_generation: policy.generation(),
        service_base_url: policy.service_base_url().to_owned(),
        ttl_seconds: policy.ttl_seconds(),
        refresh_before_seconds: policy.refresh_before_seconds(),
        retry_base_seconds: policy.retry_base_seconds(),
        retry_max_seconds: policy.retry_max_seconds(),
        allow_ethernet: policy.allow_ethernet(),
        allow_wifi: policy.allow_wifi(),
        allow_mobile: policy.allow_mobile(),
        allow_unknown_network: policy.allow_unknown_network(),
        current_network,
        network_allowed,
        execution_scope: "only-while-runtime-process-is-running".to_owned(),
        os_background_service_enabled: false,
        publish: ticket_automation_action_status(
            snapshot,
            policy,
            TicketAutomationAction::Publish,
            network_allowed,
            now_unix_seconds,
        ),
        refresh: ticket_automation_action_status(
            snapshot,
            policy,
            TicketAutomationAction::Refresh,
            network_allowed,
            now_unix_seconds,
        ),
    })
}

fn ticket_automation_action_status(
    snapshot: &RuntimeStateSnapshot,
    policy: &SignedTicketAutomationPolicy,
    action: TicketAutomationAction,
    network_allowed: bool,
    now_unix_seconds: u64,
) -> RuntimeIpcTicketAutomationActionStatus {
    let attempts = snapshot
        .ticket_automation_attempts
        .get(&(policy.contact_id(), action));
    let latest = attempts
        .and_then(|attempts| attempts.last())
        .filter(|attempt| attempt.policy_generation() == policy.generation());
    let latest_success = attempts.and_then(|attempts| {
        attempts.iter().rev().find(|attempt| {
            attempt.policy_generation() == policy.generation() && attempt.succeeded()
        })
    });
    let state = if !policy.enabled() {
        "disabled"
    } else if !network_allowed {
        "network-blocked"
    } else if latest.is_some_and(|attempt| attempt.not_before_unix_seconds() > now_unix_seconds) {
        if latest.is_some_and(SignedTicketAutomationAttempt::succeeded) {
            "fresh"
        } else {
            "backoff"
        }
    } else {
        "due"
    };
    RuntimeIpcTicketAutomationActionStatus {
        action: action.as_str().to_owned(),
        state: state.to_owned(),
        last_attempt_unix_seconds: latest
            .map(SignedTicketAutomationAttempt::attempted_at_unix_seconds),
        last_success_unix_seconds: latest_success
            .map(SignedTicketAutomationAttempt::attempted_at_unix_seconds),
        next_attempt_unix_seconds: latest
            .map(SignedTicketAutomationAttempt::not_before_unix_seconds),
        consecutive_failures: latest.map_or(0, SignedTicketAutomationAttempt::consecutive_failures),
        publication_generation: latest_success
            .and_then(SignedTicketAutomationAttempt::publication_generation),
        expires_at_unix_seconds: latest_success
            .and_then(SignedTicketAutomationAttempt::expires_at_unix_seconds),
        last_result: latest
            .map_or("never", |attempt| {
                if attempt.succeeded() {
                    "success"
                } else {
                    "failure"
                }
            })
            .to_owned(),
    }
}

#[derive(Clone, Copy)]
enum RuntimeTicketAutomationAttempt {
    NoWork,
    Completed,
}

async fn attempt_next_runtime_ticket_automation(
    state_directory: &Path,
    current_ticket: &ConnectionTicket,
) -> Result<RuntimeTicketAutomationAttempt> {
    let current_network = current_runtime_network_class();
    let selection_lock = acquire_runtime_state_lock(state_directory)
        .await?
        .context("runtime state lock remained busy while selecting ticket automation")?;
    let device_state = load_command_device_state(state_directory)?;
    let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
    let certificate = trust.load_certificate()?;
    let snapshot = load_runtime_state_snapshot(
        state_directory,
        certificate.account_id(),
        device_state.identity().device_id(),
    )?;
    let now = unix_time_now()?;
    let selected = snapshot
        .ticket_automation_policies
        .values()
        .filter_map(|policies| policies.last())
        .filter(|policy| policy.enabled() && policy.allows_network(current_network))
        .find_map(|policy| {
            [
                TicketAutomationAction::Publish,
                TicketAutomationAction::Refresh,
            ]
            .into_iter()
            .find(|action| {
                snapshot
                    .latest_ticket_automation_attempt(policy.contact_id(), *action)
                    .filter(|attempt| attempt.policy_generation() == policy.generation())
                    .is_none_or(|attempt| attempt.not_before_unix_seconds() <= now)
            })
            .map(|action| (policy.clone(), action))
        });
    drop(selection_lock);
    let Some((policy, action)) = selected else {
        return Ok(RuntimeTicketAutomationAttempt::NoWork);
    };

    let operation = match action {
        TicketAutomationAction::Publish => publish_runtime_own_ticket(
            state_directory,
            current_ticket,
            policy.conversation(),
            policy.peer_account_id(),
            policy.service_base_url(),
            policy.ttl_seconds(),
            policy.ttl_seconds(),
        )
        .await
        .map(|publication| {
            (
                publication.publication_generation,
                publication.expires_at_unix_seconds,
            )
        }),
        TicketAutomationAction::Refresh => refresh_runtime_contact_ticket(
            state_directory,
            policy.conversation(),
            policy.peer_account_id(),
            policy.service_base_url(),
        )
        .await
        .and_then(|refresh| {
            ensure!(
                refresh.complete,
                "multi-endpoint ticket refresh completed only {}/{} candidates",
                refresh.refreshed_endpoint_candidate_count,
                refresh.endpoint_candidate_count
            );
            let publication_generation = refresh
                .results
                .iter()
                .filter_map(|result| result.publication_generation)
                .min()
                .context("complete multi-endpoint refresh has no publication generation")?;
            let expires_at_unix_seconds = refresh
                .results
                .iter()
                .filter_map(|result| result.expires_at_unix_seconds)
                .min()
                .context("complete multi-endpoint refresh has no expiry")?;
            Ok((publication_generation, expires_at_unix_seconds))
        }),
    };
    if let Err(error) = &operation {
        eprintln!(
            "runtime_ticket_automation_status=failed contact_id={} action={} error={error:#}",
            policy.contact_id(),
            action.as_str()
        );
    }
    let attempted_at = unix_time_now()?;
    let persist_lock = acquire_runtime_state_lock(state_directory)
        .await?
        .context("runtime state lock remained busy while persisting ticket automation")?;
    let device_state = load_command_device_state(state_directory)?;
    let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
    let certificate = trust.load_certificate()?;
    let snapshot = load_runtime_state_snapshot(
        state_directory,
        certificate.account_id(),
        device_state.identity().device_id(),
    )?;
    let current_policy = snapshot
        .latest_ticket_automation_policy(policy.contact_id())
        .context("ticket automation policy disappeared during network operation")?;
    if current_policy.policy_id()? != policy.policy_id()? {
        drop(persist_lock);
        return Ok(RuntimeTicketAutomationAttempt::Completed);
    }
    let previous = snapshot.latest_ticket_automation_attempt(policy.contact_id(), action);
    let attempt = SignedTicketAutomationAttempt::sign(
        device_state.identity(),
        certificate.account_id(),
        &policy,
        action,
        attempted_at,
        operation.ok(),
        previous,
    )?;
    let attempt_id = attempt.attempt_id()?;
    let path = runtime_ticket_automation_attempt_relative_path(attempt_id);
    let bytes = attempt.encode()?;
    run_state_transaction(state_directory, |transaction| {
        persist_runtime_record(state_directory, &path, &bytes, transaction)
    })?;
    drop(persist_lock);
    println!(
        "runtime_ticket_automation_status={} contact_id={} action={} next_attempt_unix_seconds={}",
        if attempt.succeeded() {
            "succeeded"
        } else {
            "backoff"
        },
        attempt.contact_id(),
        action.as_str(),
        attempt.not_before_unix_seconds()
    );
    Ok(RuntimeTicketAutomationAttempt::Completed)
}

fn store_outcome_name(outcome: StoreOutcome) -> &'static str {
    match outcome {
        StoreOutcome::Inserted => "Inserted",
        StoreOutcome::AlreadyPresent => "AlreadyPresent",
    }
}

enum RuntimeEvent {
    Connection(Connection),
    Ipc(RuntimeIpcWork),
    IpcClosed,
    Tick,
    IdleTimeout,
    Shutdown,
}

async fn runtime(options: RuntimeOptions) -> Result<()> {
    validate_runtime_options(&options)?;
    let RuntimeOptions {
        state_dir,
        allowed_requester_account_id,
        device_list_file,
        peer_prekey_pool_files,
        ticket_file,
        relay_wait_seconds,
        route_policy,
        relay_url,
        max_sessions,
        idle_seconds,
        poll_milliseconds,
        retry_base_seconds,
        retry_max_seconds,
        auto_sync_seconds,
        max_outbound_actions,
        ipc_file,
    } = options;
    let endpoint_announcement_descriptor_directory =
        resolve_runtime_received_endpoint_descriptor_directory(
            &state_dir,
            ipc_file.as_deref(),
            ticket_file.as_deref(),
        )?;
    let prepared = with_locked_state(&state_dir, || {
        prepare_runtime_listener(&state_dir, &device_list_file, &peer_prekey_pool_files)
    })?;
    let mut device_directory_state = prepared.device_directory_state;
    let endpoint_builder = endpoint_builder_with_relay(route_policy, relay_url);
    #[cfg(test)]
    let endpoint_builder = endpoint_builder
        .relay_mode(iroh::RelayMode::Disabled)
        .clear_ip_transports()
        .clear_address_lookup()
        .portmapper_config(iroh::endpoint::PortmapperConfig::Disabled)
        .net_report_config(iroh::endpoint::NetReportConfig::minimal())
        .bind_addr((std::net::Ipv4Addr::LOCALHOST, 0))
        .context("bind test runtime endpoint exclusively to IPv4 loopback")?;
    let endpoint = endpoint_builder
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .context("bind long-lived Iroh runtime endpoint")?;
    wait_for_relay(&endpoint, route_policy, relay_wait_seconds).await?;

    let mut ticket = ConnectionTicket::new(
        endpoint.addr(),
        prepared.device_state.identity(),
        prepared.listener_certificate.clone(),
        prepared.listener_directory,
        allowed_requester_account_id,
        route_policy,
    )?;
    let encoded_ticket = ticket.encode()?;
    let (mut ipc_server, mut ipc_receiver, _ipc_keepalive) = if let Some(ipc_file) = ipc_file {
        let ipc_file = resolve_runtime_ipc_descriptor_path(&state_dir, &ipc_file)?;
        let (server, receiver) = RuntimeIpcServer::start(
            ipc_file.clone(),
            ticket.listener_account_id(),
            prepared.device_state.identity(),
        )
        .await?;
        println!("runtime_ipc_file={}", ipc_file.display());
        println!("runtime_ipc_address={}", server.address());
        println!("runtime_ipc_auth=bearer-token");
        (Some(server), receiver, None)
    } else {
        let (keepalive, receiver) = tokio::sync::mpsc::channel(1);
        (None, receiver, Some(keepalive))
    };
    println!("runtime_mode=multi-session-v1");
    println!("transport_endpoint_id={}", endpoint.id());
    println!("account_id={}", ticket.listener_account_id());
    println!("device_id={}", prepared.device_state.identity().device_id());
    println!(
        "ratchet_prekey_pool_generation={}",
        prepared.listener_prekey_pool.generation()
    );
    println!(
        "fanout_device_count={}",
        ticket.listener_directory().pools().len()
    );
    println!("route_policy={}", route_policy.as_str());
    println!("allowed_requester_account_id={allowed_requester_account_id}");
    println!(
        "authority_revision={}",
        ticket.listener_authority_snapshot().revision()
    );
    println!("authority_store={:?}", prepared.authority_snapshot_store);
    print_runtime_device_directory_status(&device_directory_state.status(&ticket)?);
    println!("runtime_max_sessions={max_sessions}");
    println!("runtime_idle_seconds={idle_seconds}");
    println!("runtime_poll_milliseconds={poll_milliseconds}");
    println!("runtime_retry_base_seconds={retry_base_seconds}");
    println!("runtime_retry_max_seconds={retry_max_seconds}");
    println!("runtime_auto_sync_seconds={auto_sync_seconds}");
    println!("runtime_ticket_automation=opt-in");
    println!("runtime_ticket_automation_scope=only-while-runtime-process-is-running");
    println!("runtime_ticket_automation_os_background_service=false");
    println!("runtime_max_outbound_actions={max_outbound_actions}");
    if let Some(directory) = &endpoint_announcement_descriptor_directory {
        println!(
            "runtime_received_endpoint_descriptor_directory={}",
            directory.display()
        );
    }
    println!("ticket={encoded_ticket}");
    if let Some(path) = &ticket_file {
        publish_runtime_ticket(path, encoded_ticket.as_bytes())?;
        println!("ticket_file={}", path.display());
        println!("ticket_publish=atomic-replace");
    }
    println!("status=runtime-listening");

    let session_binding = SyncSessionBinding::from_transport_label(&endpoint.id().to_string());
    let mut accepted_sessions = 0_usize;
    let mut outbound_actions = 0_usize;
    let mut last_activity = tokio::time::Instant::now();
    let automatic_sync_started_at = tokio::time::Instant::now();
    let mut last_sync_attempts = BTreeMap::new();
    let mut last_ticket_automation_check =
        tokio::time::Instant::now() - RUNTIME_TICKET_AUTOMATION_CHECK_INTERVAL;
    // Keep the accept future alive across polling ticks. Dropping an Iroh
    // Incoming while a handshake is in progress actively rejects that peer.
    let mut accept: Pin<Box<dyn Future<Output = Result<Connection>> + Send + '_>> =
        Box::pin(accept_authenticated_connection(&endpoint));
    let mut shutdown: Pin<Box<dyn Future<Output = io::Result<()>> + Send + '_>> =
        Box::pin(tokio::signal::ctrl_c());
    let poll_interval = Duration::from_millis(poll_milliseconds);
    let mut poll_tick =
        tokio::time::interval_at(tokio::time::Instant::now() + poll_interval, poll_interval);
    poll_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let stop_reason = loop {
        let idle_deadline =
            (idle_seconds != 0).then(|| last_activity + Duration::from_secs(idle_seconds));
        let runtime_event = wait_for_runtime_event(
            &mut accept,
            &mut shutdown,
            &mut ipc_receiver,
            idle_deadline,
            &mut poll_tick,
        )
        .await?;
        if matches!(runtime_event, RuntimeEvent::Connection(_)) {
            accept = Box::pin(accept_authenticated_connection(&endpoint));
        }
        match runtime_event {
            RuntimeEvent::IdleTimeout => break "idle-timeout",
            RuntimeEvent::Shutdown => break "ctrl-c",
            RuntimeEvent::IpcClosed => bail!("runtime IPC acceptor stopped unexpectedly"),
            RuntimeEvent::Ipc(work) => {
                last_activity = tokio::time::Instant::now();
                let outcome = handle_runtime_ipc_work(
                    &state_dir,
                    &endpoint,
                    &mut ticket,
                    &mut device_directory_state,
                    ticket_file.as_deref(),
                    work,
                )
                .await;
                if outcome.state_changed
                    && let Some(server) = ipc_server.as_ref()
                {
                    server.publish_change();
                }
                if outcome.shutdown_requested {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    break "ipc-shutdown";
                }
            }
            RuntimeEvent::Tick => {
                let delivery_attempt = attempt_next_runtime_delivery(
                    &endpoint,
                    &state_dir,
                    retry_base_seconds,
                    retry_max_seconds,
                )
                .await?;
                match delivery_attempt {
                    RuntimeDeliveryAttempt::NoWork => {}
                    RuntimeDeliveryAttempt::Delivered | RuntimeDeliveryAttempt::RetryScheduled => {
                        if let Some(server) = ipc_server.as_ref() {
                            server.publish_change();
                        }
                        outbound_actions += 1;
                        last_activity = tokio::time::Instant::now();
                        if max_outbound_actions != 0 && outbound_actions >= max_outbound_actions {
                            break "outbound-action-limit";
                        }
                    }
                }
                let sync_attempted = matches!(delivery_attempt, RuntimeDeliveryAttempt::NoWork)
                    && auto_sync_seconds != 0
                    && automatic_sync_started_at.elapsed()
                        >= Duration::from_secs(auto_sync_seconds)
                    && attempt_runtime_contact_sync(
                        &state_dir,
                        Duration::from_secs(auto_sync_seconds),
                        &mut last_sync_attempts,
                    )
                    .await?;
                if sync_attempted {
                    if let Some(server) = ipc_server.as_ref() {
                        server.publish_change();
                    }
                    outbound_actions += 1;
                    last_activity = tokio::time::Instant::now();
                    if max_outbound_actions != 0 && outbound_actions >= max_outbound_actions {
                        break "outbound-action-limit";
                    }
                }
                if matches!(delivery_attempt, RuntimeDeliveryAttempt::NoWork)
                    && !sync_attempted
                    && last_ticket_automation_check.elapsed()
                        >= RUNTIME_TICKET_AUTOMATION_CHECK_INTERVAL
                {
                    last_ticket_automation_check = tokio::time::Instant::now();
                    if matches!(
                        attempt_next_runtime_ticket_automation(&state_dir, &ticket).await?,
                        RuntimeTicketAutomationAttempt::Completed
                    ) {
                        if let Some(server) = ipc_server.as_ref() {
                            server.publish_change();
                        }
                        outbound_actions += 1;
                        last_activity = tokio::time::Instant::now();
                        if max_outbound_actions != 0 && outbound_actions >= max_outbound_actions {
                            break "outbound-action-limit";
                        }
                    }
                    if let Some(report) = attempt_runtime_ticket_compaction(&state_dir).await? {
                        println!(
                            "runtime_ticket_compaction_status=committed checkpoint_generation={} removed_records={} retained_anchors={} compacted_total_records={}",
                            report.checkpoint_generation,
                            report.removed_records,
                            report.retained_anchors,
                            report.compacted_total_records
                        );
                        if let Some(server) = ipc_server.as_ref() {
                            server.publish_change();
                        }
                        last_activity = tokio::time::Instant::now();
                    }
                }
            }
            RuntimeEvent::Connection(connection) => {
                accepted_sessions += 1;
                last_activity = tokio::time::Instant::now();
                println!("runtime_session={accepted_sessions}");
                println!("peer_id={}", connection.remote_id());
                let route_result = await_route_policy(&connection, route_policy, ROUTE_POLICY_WAIT)
                    .await
                    .context("wait for an incoming path allowed by the runtime ticket");
                let session_result = match route_result {
                    Ok(ready_path) => {
                        print_ready_path(&ready_path);
                        match handle_runtime_application_connection(
                            &connection,
                            &state_dir,
                            endpoint_announcement_descriptor_directory.as_deref(),
                            &ticket,
                            session_binding,
                            allowed_requester_account_id,
                            route_policy,
                        )
                        .await
                        {
                            Ok(result) => result,
                            Err(error) => {
                                connection
                                    .close(1_u32.into(), b"kilogram runtime local state failure");
                                endpoint.close().await;
                                return Err(error).context(
                                    "stop runtime after a local state/vault session failure",
                                );
                            }
                        }
                    }
                    Err(error) => Err(error),
                };
                if session_result.is_ok()
                    && let Some(server) = ipc_server.as_ref()
                {
                    server.publish_change();
                }
                match session_result {
                    Ok(()) => println!("runtime_session_status=completed"),
                    Err(error) => {
                        eprintln!(
                            "runtime_session_status=failed runtime_session={accepted_sessions} error={error:#}"
                        );
                    }
                }
                let _ = timeout(Duration::from_secs(2), connection.closed()).await;
                connection.close(0_u32.into(), b"kilogram runtime session complete");
                if max_sessions != 0 && accepted_sessions >= max_sessions {
                    break "session-limit";
                }
                println!("status=runtime-listening");
            }
        }
    };

    drop(accept);
    drop(shutdown);
    if let Some(server) = ipc_server.take() {
        server.shutdown().await?;
    }
    endpoint.close().await;
    println!("runtime_sessions_accepted={accepted_sessions}");
    println!("runtime_outbound_actions={outbound_actions}");
    println!("runtime_stop_reason={stop_reason}");
    println!("status=runtime-stopped");
    Ok(())
}

fn validate_runtime_options(options: &RuntimeOptions) -> Result<()> {
    ensure!(
        options.max_sessions <= MAX_RUNTIME_SESSIONS,
        "--max-sessions must be zero or at most {MAX_RUNTIME_SESSIONS}"
    );
    ensure!(
        options.idle_seconds <= MAX_RUNTIME_IDLE_SECONDS,
        "--idle-seconds must be zero or at most {MAX_RUNTIME_IDLE_SECONDS}"
    );
    ensure!(
        (10..=MAX_RUNTIME_POLL_MILLISECONDS).contains(&options.poll_milliseconds),
        "--poll-milliseconds must be between 10 and {MAX_RUNTIME_POLL_MILLISECONDS}"
    );
    ensure!(
        (1..=MAX_RUNTIME_RETRY_SECONDS).contains(&options.retry_base_seconds),
        "--retry-base-seconds must be between 1 and {MAX_RUNTIME_RETRY_SECONDS}"
    );
    ensure!(
        options.retry_max_seconds >= options.retry_base_seconds
            && options.retry_max_seconds <= MAX_RUNTIME_RETRY_SECONDS,
        "--retry-max-seconds must be at least the base and at most {MAX_RUNTIME_RETRY_SECONDS}"
    );
    ensure!(
        options.auto_sync_seconds <= MAX_RUNTIME_AUTO_SYNC_SECONDS,
        "--auto-sync-seconds must be zero or at most {MAX_RUNTIME_AUTO_SYNC_SECONDS}"
    );
    Ok(())
}

fn prepare_runtime_listener(
    state_directory: &Path,
    device_list_file: &Path,
    peer_prekey_pool_files: &[PathBuf],
) -> Result<PreparedRuntimeListener> {
    let device_state = load_command_device_state(state_directory)?;
    let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
    let listener_certificate = trust
        .load_certificate()
        .context("load runtime Account Root certificate")?;
    let canonical_state_dir = fs::canonicalize(state_directory)
        .context("resolve state directory for runtime device-directory status")?;
    let snapshot = load_runtime_state_snapshot(
        state_directory,
        listener_certificate.account_id(),
        device_state.identity().device_id(),
    )?;
    let latest_receipt = snapshot.latest_device_directory_receipt().cloned();
    let launch_device_list_file = match fs::canonicalize(device_list_file) {
        Ok(path) => path,
        Err(error) if latest_receipt.is_some() && error.kind() == io::ErrorKind::NotFound => {
            device_list_file.to_path_buf()
        }
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "resolve runtime launch-profile device list {}",
                    device_list_file.display()
                )
            });
        }
    };
    let applied_device_list_file = latest_receipt
        .as_ref()
        .map(|receipt| receipt.device_list_file().to_path_buf())
        .unwrap_or_else(|| launch_device_list_file.clone());
    let metadata = fs::symlink_metadata(&applied_device_list_file).with_context(|| {
        format!(
            "inspect authenticated runtime device list {}",
            applied_device_list_file.display()
        )
    })?;
    ensure!(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "authenticated runtime device list must be a regular non-symlink file"
    );
    let applied_device_list_file =
        fs::canonicalize(&applied_device_list_file).with_context(|| {
            format!(
                "resolve authenticated runtime device list {}",
                applied_device_list_file.display()
            )
        })?;
    let listener_device_list = AccountDeviceListSnapshot::decode_and_verify(
        &fs::read(&applied_device_list_file).with_context(|| {
            format!(
                "read runtime device list from {}",
                applied_device_list_file.display()
            )
        })?,
    )
    .context("decode and verify runtime account device list")?;
    let device_list_digest = runtime_device_list_digest(&listener_device_list)?;
    if let Some(receipt) = latest_receipt.as_ref() {
        ensure!(
            receipt.device_list_file() == applied_device_list_file,
            "authenticated runtime device-directory receipt path changed on disk"
        );
        ensure!(
            receipt.account_id() == listener_certificate.account_id()
                && receipt.local_device_id() == device_state.identity().device_id(),
            "authenticated runtime device-directory receipt belongs to another identity"
        );
        ensure!(
            receipt.authority_revision() == listener_device_list.revision()
                && receipt.active_device_count() == listener_device_list.devices().len()
                && receipt.device_list_digest() == device_list_digest,
            "authenticated runtime device-directory receipt does not match its device-list file"
        );
    }
    ensure!(
        listener_device_list.certificate_for(listener_certificate.device_id())
            == Some(&listener_certificate),
        "runtime certificate is not present exactly in the supplied device list"
    );
    let authority_snapshot_store = install_own_authority_primary(
        state_directory,
        &device_state,
        listener_device_list.authority_snapshot(),
    )
    .context("install authority snapshot embedded in runtime device list")?;
    let now_unix_seconds = unix_time_now().context("read time for runtime prekey freshness")?;
    let listener_prekey_pool = run_state_transaction(state_directory, |transaction| {
        let mut ratchet_state = transaction.load_ratchet_state()?;
        ratchet_state
            .prekey_pool(
                device_state.identity(),
                DEFAULT_PREKEY_POOL_SIZE,
                now_unix_seconds,
                DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
            )
            .context("publish runtime one-time prekey pool")
    })?;
    let mut prekey_pools = vec![listener_prekey_pool.clone()];
    for path in peer_prekey_pool_files {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if latest_receipt.is_some() && error.kind() == io::ErrorKind::NotFound => {
                continue;
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("read runtime peer prekey pool from {}", path.display())
                });
            }
        };
        let pool = SignedPrekeyPool::decode(&bytes)
            .with_context(|| format!("verify runtime peer prekey pool from {}", path.display()))?;
        if listener_device_list
            .certificate_for(pool.device_id())
            .is_some()
        {
            prekey_pools.push(pool);
        } else {
            ensure!(
                latest_receipt.is_some()
                    && listener_device_list
                        .authority_snapshot()
                        .revocations()
                        .iter()
                        .any(|revocation| revocation.device_id() == pool.device_id()),
                "runtime peer prekey pool belongs to a device absent without a persisted revocation"
            );
        }
    }
    let listener_directory = AccountPrekeyDirectory::new(listener_device_list, prekey_pools)
        .context("assemble complete runtime account prekey directory")?;
    listener_directory
        .verify_at(now_unix_seconds)
        .context("verify runtime account prekey directory freshness")?;
    Ok(PreparedRuntimeListener {
        device_state,
        listener_certificate,
        listener_directory,
        listener_prekey_pool,
        authority_snapshot_store,
        device_directory_state: RuntimeDeviceDirectoryState {
            state_dir: canonical_state_dir,
            profile_convergence_status: if applied_device_list_file == launch_device_list_file {
                "current"
            } else {
                "convergence-required"
            },
            restart_recovery_status: if latest_receipt.is_some() {
                "authenticated-receipt-recovered"
            } else {
                "launch-profile-directory"
            },
            launch_device_list_file,
            applied_device_list_file,
            device_list_digest,
            latest_receipt,
        },
    })
}

fn publish_runtime_ticket(path: &Path, encoded_ticket: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        fs::create_dir_all(parent)
            .with_context(|| format!("create runtime ticket directory {}", parent.display()))?;
    }
    let temporary_directory = parent.unwrap_or_else(|| Path::new("."));
    let mut temporary =
        NamedTempFile::new_in(temporary_directory).context("create temporary runtime ticket")?;
    temporary
        .write_all(encoded_ticket)
        .context("write temporary runtime ticket")?;
    temporary
        .as_file()
        .sync_all()
        .context("sync temporary runtime ticket")?;
    let persisted = temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("atomically publish runtime ticket to {}", path.display()))?;
    persisted
        .sync_all()
        .with_context(|| format!("sync published runtime ticket at {}", path.display()))?;
    Ok(())
}

#[derive(Clone, Copy)]
enum RuntimeDeliveryAttempt {
    NoWork,
    Delivered,
    RetryScheduled,
}

struct PreparedRuntimeDelivery {
    queue_id: RuntimeQueueId,
    endpoint_candidates: Vec<ResolvedRuntimeEndpointCandidate>,
    event: AuthorizedEvent,
    membership: ConversationMembershipSnapshot,
    local_certificate: DeviceCertificate,
    local_authority: AccountAuthoritySnapshot,
    peer_account_id: AccountId,
}

fn select_due_runtime_queue(state_directory: &Path) -> Result<Option<RuntimeQueueId>> {
    let device_state = load_command_device_state(state_directory)?;
    let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
    let certificate = trust.load_certificate()?;
    let snapshot = load_runtime_state_snapshot(
        state_directory,
        certificate.account_id(),
        device_state.identity().device_id(),
    )?;
    let now = unix_time_now()?;
    Ok(snapshot.queued.keys().copied().find(|queue_id| {
        !snapshot.delivered.contains_key(queue_id)
            && snapshot
                .latest_retry(*queue_id)
                .is_none_or(|retry| retry.not_before_unix_seconds() <= now)
    }))
}

async fn attempt_next_runtime_delivery(
    endpoint: &Endpoint,
    state_directory: &Path,
    retry_base_seconds: u64,
    retry_max_seconds: u64,
) -> Result<RuntimeDeliveryAttempt> {
    let selection_lock = acquire_runtime_state_lock(state_directory)
        .await?
        .context("runtime state lock remained busy while selecting outbound work")?;
    let selected = select_due_runtime_queue(state_directory);
    drop(selection_lock);
    let Some(queue_id) = selected? else {
        return Ok(RuntimeDeliveryAttempt::NoWork);
    };
    let prepared = match prepare_runtime_delivery(state_directory, queue_id).await {
        Ok(prepared) => prepared,
        Err(error) => {
            eprintln!(
                "runtime_outbound_status=failed runtime_queue_id={queue_id} stage=prepare error={error:#}"
            );
            persist_runtime_retry(
                state_directory,
                queue_id,
                retry_base_seconds,
                retry_max_seconds,
            )
            .await?;
            return Ok(RuntimeDeliveryAttempt::RetryScheduled);
        }
    };
    match send_runtime_delivery_with_failover(endpoint, state_directory, &prepared).await {
        Ok(acknowledgement) => {
            persist_runtime_delivery(state_directory, &prepared, &acknowledgement).await?;
            println!("runtime_queue_id={queue_id}");
            println!(
                "runtime_sent_event_id={}",
                prepared.event.event().event_id()?
            );
            println!(
                "runtime_acknowledgement_event_id={}",
                acknowledgement.event().event_id()?
            );
            println!("runtime_outbound_status=delivered");
            Ok(RuntimeDeliveryAttempt::Delivered)
        }
        Err(error) => {
            eprintln!(
                "runtime_outbound_status=failed runtime_queue_id={queue_id} stage=network error={error:#}"
            );
            persist_runtime_retry(
                state_directory,
                queue_id,
                retry_base_seconds,
                retry_max_seconds,
            )
            .await?;
            Ok(RuntimeDeliveryAttempt::RetryScheduled)
        }
    }
}

async fn prepare_runtime_delivery(
    state_directory: &Path,
    queue_id: RuntimeQueueId,
) -> Result<PreparedRuntimeDelivery> {
    let state_lock = acquire_runtime_state_lock(state_directory)
        .await?
        .context("runtime state lock remained busy while preparing outbound delivery")?;
    let vault_guard = VaultDualWriteGuard::prepare(state_directory)?;
    let operation_result = (|| {
        let device_state = load_command_device_state(state_directory)?;
        let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
        let local_certificate = trust.load_certificate()?;
        let local_authority = trust.load_own_authority_snapshot(&local_certificate)?;
        let snapshot = load_runtime_state_snapshot(
            state_directory,
            local_certificate.account_id(),
            device_state.identity().device_id(),
        )?;
        let queued = snapshot
            .queued
            .get(&queue_id)
            .context("selected runtime queue record disappeared")?;
        ensure!(
            !snapshot.delivered.contains_key(&queue_id),
            "selected runtime queue record is already delivered"
        );
        let contact = snapshot
            .contacts
            .get(&queued.contact_id())
            .context("selected runtime queue contact disappeared")?;
        let endpoint_candidates = load_runtime_endpoint_candidate_set(
            &snapshot,
            contact,
            &local_certificate,
            &local_authority,
        )?;
        let ticket = endpoint_candidates
            .first()
            .context("runtime endpoint candidate set is empty")?
            .ticket
            .clone();
        pin_peer_authority_primary(
            state_directory,
            &device_state,
            ticket.listener_authority_snapshot(),
        )?;
        run_state_transaction(state_directory, |transaction| {
            let ratchet_state = transaction.load_ratchet_state()?;
            let mut retired_sessions = 0_usize;
            let mut retired_observations = 0_usize;
            for revocation in ticket.listener_authority_snapshot().revocations() {
                let retirement = ratchet_state
                    .retire_peer_device(revocation.device_id())
                    .with_context(|| {
                        format!(
                            "retire peer ratchet state for revoked device {}",
                            revocation.device_id()
                        )
                    })?;
                retired_sessions += usize::from(retirement.session_removed);
                retired_observations += usize::from(retirement.prekey_observation_removed);
            }
            ratchet_state
                .observe_prekey_directory(ticket.listener_directory(), unix_time_now()?)?;
            if retired_sessions != 0 || retired_observations != 0 {
                println!("runtime_peer_ratchet_sessions_retired={retired_sessions}");
                println!("runtime_peer_prekey_observations_retired={retired_observations}");
            }
            Ok(())
        })?;
        let membership = trust
            .load_conversation_membership(queued.conversation_id().scope_id())
            .context("load runtime queued conversation membership")?;
        require_conversation_participants(
            &membership,
            local_certificate.account_id(),
            contact.peer_account_id(),
        )?;
        let event = match snapshot.materialized.get(&queue_id) {
            Some(materialized) => materialized.event().clone(),
            None => {
                let body = queued.open(device_state.encryption())?;
                let event_store = open_event_store(state_directory)?;
                let local_message_store = open_local_message_store(state_directory)?;
                let event = run_state_transaction(state_directory, |transaction| {
                    let author_sequence = transaction.allocate_sequence(&device_state)?;
                    let parents = event_store.frontier(queued.conversation_id())?;
                    let mut ratchet_state = transaction.load_ratchet_state()?;
                    let fanout = encrypt_ratchet_fanout(
                        &mut ratchet_state,
                        device_state.identity(),
                        ticket.listener_directory(),
                        &body,
                    )?;
                    let signed = SignedEvent::sign_ratchet_text(
                        device_state.identity(),
                        queued.conversation_id(),
                        author_sequence,
                        parents,
                        ticket.listener_directory().device_list().clone(),
                        fanout.sender_identity,
                        fanout.recipients,
                    )?;
                    let event = AuthorizedEvent::new(
                        signed,
                        local_certificate.clone(),
                        local_authority.clone(),
                    )?;
                    let (_, projection_receipt) = ensure_authored_local_text_projection(
                        &local_message_store,
                        &device_state,
                        local_certificate.account_id(),
                        event.event(),
                        &body,
                    )?;
                    transaction.register_store_receipt(&projection_receipt)?;
                    let (_, event_receipt) =
                        event_store.put_authorized_with_receipt(&event, &membership)?;
                    transaction.register_store_receipt(&event_receipt)?;
                    let marker = SignedMaterializedMessage::sign(
                        device_state.identity(),
                        queue_id,
                        event.clone(),
                    )?;
                    persist_runtime_record(
                        state_directory,
                        &runtime_materialized_relative_path(queue_id),
                        &marker.encode()?,
                        transaction,
                    )?;
                    Ok(event)
                })?;
                println!("runtime_queue_id={queue_id}");
                println!(
                    "runtime_materialized_event_id={}",
                    event.event().event_id()?
                );
                println!("runtime_materialization_store=Inserted");
                event
            }
        };
        event.verify_for_membership(&membership)?;
        let immutable_revoked_slots = event
            .event()
            .ratchet_recipients()?
            .iter()
            .filter(|recipient| {
                ticket
                    .listener_authority_snapshot()
                    .revocations()
                    .iter()
                    .any(|revocation| revocation.device_id() == recipient.device_id())
            })
            .count();
        if immutable_revoked_slots != 0 {
            println!("runtime_queue_id={queue_id}");
            println!(
                "runtime_preexisting_revoked_recipient_slots_immutable={immutable_revoked_slots}"
            );
            println!("runtime_preexisting_recipient_slot_action=not-rewritten");
        }
        Ok(PreparedRuntimeDelivery {
            queue_id,
            event,
            membership,
            local_certificate,
            local_authority,
            peer_account_id: contact.peer_account_id(),
            endpoint_candidates,
        })
    })();
    let mirror_result = match vault_guard {
        Some(guard) => guard.finish(),
        None => Ok(()),
    };
    drop(state_lock);
    combine_operation_and_mirror(operation_result, mirror_result)
}

async fn send_runtime_delivery_with_failover(
    endpoint: &Endpoint,
    state_directory: &Path,
    prepared: &PreparedRuntimeDelivery,
) -> Result<AuthorizedEvent> {
    let mut failures = Vec::new();
    for (index, candidate) in prepared.endpoint_candidates.iter().enumerate() {
        println!("runtime_endpoint_attempt={}", index + 1);
        println!(
            "runtime_endpoint_candidate_device_id={}",
            candidate.peer_device_id
        );
        match send_runtime_delivery_to_candidate(endpoint, state_directory, prepared, candidate)
            .await
        {
            Ok(acknowledgement) => {
                println!("runtime_endpoint_failover_count={index}");
                return Ok(acknowledgement);
            }
            Err(error) => {
                eprintln!(
                    "runtime_endpoint_candidate_status=failed peer_device_id={} error={error:#}",
                    candidate.peer_device_id
                );
                failures.push(format!("{}: {error:#}", candidate.peer_device_id));
            }
        }
    }
    bail!(
        "all {} authenticated endpoint candidates failed: {}",
        prepared.endpoint_candidates.len(),
        failures.join(" | ")
    )
}

async fn send_runtime_delivery_to_candidate(
    endpoint: &Endpoint,
    state_directory: &Path,
    prepared: &PreparedRuntimeDelivery,
    candidate: &ResolvedRuntimeEndpointCandidate,
) -> Result<AuthorizedEvent> {
    let ticket = &candidate.ticket;
    let route_policy = ticket.route_policy();
    let connection = timeout(
        CONNECTION_TIMEOUT,
        endpoint.connect(ticket.endpoint().clone(), ALPN),
    )
    .await
    .with_context(|| timeout_message("connect runtime outbox to peer", CONNECTION_TIMEOUT))?
    .context("connect runtime outbox to peer")?;
    let ready_path = await_route_policy(&connection, route_policy, ROUTE_POLICY_WAIT)
        .await
        .context("wait for a path allowed by the runtime contact")?;
    print_ready_path(&ready_path);
    let session_binding =
        SyncSessionBinding::from_transport_label(&ticket.endpoint().id.to_string());
    let device_state = load_command_device_state(state_directory)?;
    authorize_with_listener(
        &connection,
        device_state.identity(),
        prepared.local_certificate.clone(),
        prepared.local_authority.clone(),
        session_binding,
    )
    .await?;
    let (mut send, mut receive) = open_bi(&connection, "open runtime delivery stream").await?;
    write_client_request(
        &mut send,
        &ClientRequest::DeliverEvent(Box::new(prepared.event.clone())),
    )
    .await?;
    let acknowledgement = match read_server_response(&mut receive).await? {
        ServerResponse::EventAcknowledgement(event) => *event,
        _ => bail!("runtime outbox expected an event acknowledgement"),
    };
    acknowledgement.verify_for_membership(&prepared.membership)?;
    ensure!(
        acknowledgement.author_account_id() == prepared.peer_account_id,
        "runtime acknowledgement came from another account"
    );
    let acknowledgement_event = acknowledgement.event();
    ensure!(
        acknowledgement_event.author_device_id() == candidate.peer_device_id,
        "runtime acknowledgement came from another peer device"
    );
    let event_id = prepared.event.event().event_id()?;
    ensure!(
        acknowledgement_event.conversation_id() == prepared.event.event().conversation_id(),
        "runtime acknowledgement belongs to another conversation"
    );
    ensure!(
        matches!(
            acknowledgement_event.payload(),
            EventPayload::Acknowledgement { acknowledged_event_id }
                if *acknowledged_event_id == event_id
        ) && acknowledgement_event.parents() == [event_id],
        "runtime acknowledgement does not causally acknowledge the queued event"
    );
    print_transport_diagnostics(&connection, route_policy).await?;
    connection.close(0_u32.into(), b"kilogram runtime delivery complete");
    Ok(acknowledgement)
}

async fn persist_runtime_delivery(
    state_directory: &Path,
    prepared: &PreparedRuntimeDelivery,
    acknowledgement: &AuthorizedEvent,
) -> Result<()> {
    let state_lock = acquire_runtime_state_lock(state_directory)
        .await?
        .context("runtime state lock remained busy while storing delivery")?;
    let vault_guard = VaultDualWriteGuard::prepare(state_directory)?;
    let operation_result = (|| {
        let device_state = load_command_device_state(state_directory)?;
        let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
        let certificate = trust.load_certificate()?;
        let snapshot = load_runtime_state_snapshot(
            state_directory,
            certificate.account_id(),
            device_state.identity().device_id(),
        )?;
        let materialized = snapshot
            .materialized
            .get(&prepared.queue_id)
            .context("runtime materialization disappeared before delivery commit")?;
        ensure!(
            materialized.event() == &prepared.event,
            "runtime materialization changed before delivery commit"
        );
        let membership = trust
            .load_conversation_membership(prepared.event.event().conversation_id().scope_id())?;
        acknowledgement.verify_for_membership(&membership)?;
        let event_id = prepared.event.event().event_id()?;
        let acknowledgement_id = acknowledgement.event().event_id()?;
        let marker = SignedDeliveredMessage::sign(
            device_state.identity(),
            prepared.queue_id,
            event_id,
            acknowledgement_id,
        )?;
        let event_store = open_event_store(state_directory)?;
        run_state_transaction(state_directory, |transaction| {
            let (_, receipt) =
                event_store.put_authorized_with_receipt(acknowledgement, &membership)?;
            transaction.register_store_receipt(&receipt)?;
            persist_runtime_record(
                state_directory,
                &runtime_delivered_relative_path(prepared.queue_id),
                &marker.encode()?,
                transaction,
            )?;
            Ok(())
        })
    })();
    let mirror_result = match vault_guard {
        Some(guard) => guard.finish(),
        None => Ok(()),
    };
    drop(state_lock);
    combine_operation_and_mirror(operation_result, mirror_result)
}

async fn persist_runtime_retry(
    state_directory: &Path,
    queue_id: RuntimeQueueId,
    retry_base_seconds: u64,
    retry_max_seconds: u64,
) -> Result<()> {
    let state_lock = acquire_runtime_state_lock(state_directory)
        .await?
        .context("runtime state lock remained busy while storing retry")?;
    let vault_guard = VaultDualWriteGuard::prepare(state_directory)?;
    let operation_result = (|| {
        let device_state = load_command_device_state(state_directory)?;
        let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
        let certificate = trust.load_certificate()?;
        let snapshot = load_runtime_state_snapshot(
            state_directory,
            certificate.account_id(),
            device_state.identity().device_id(),
        )?;
        ensure!(
            snapshot.queued.contains_key(&queue_id),
            "cannot retry an absent runtime queue record"
        );
        if snapshot.delivered.contains_key(&queue_id) {
            return Ok(());
        }
        let previous = snapshot.latest_retry(queue_id);
        let generation = previous.map_or(1, |value| value.generation().saturating_add(1));
        let shift = generation.saturating_sub(1).min(20);
        let cap = retry_base_seconds
            .saturating_mul(1_u64 << shift)
            .min(retry_max_seconds);
        let floor = cap / 2;
        let mut entropy = blake3::Hasher::new();
        entropy.update(b"kilogram:runtime-retry-jitter:v1\0");
        entropy.update(queue_id.as_bytes());
        entropy.update(&generation.to_le_bytes());
        let entropy = entropy.finalize();
        let mut sample_bytes = [0_u8; 8];
        sample_bytes.copy_from_slice(&entropy.as_bytes()[..8]);
        let sample = u64::from_le_bytes(sample_bytes);
        let delay = floor + sample % (cap.saturating_sub(floor).saturating_add(1));
        let not_before = unix_time_now()?.saturating_add(delay.max(1));
        let retry =
            SignedRuntimeRetryState::sign(device_state.identity(), queue_id, previous, not_before)?;
        run_state_transaction(state_directory, |transaction| {
            persist_runtime_record(
                state_directory,
                &runtime_retry_relative_path(queue_id, retry.generation()),
                &retry.encode()?,
                transaction,
            )?;
            Ok(())
        })?;
        println!("runtime_queue_id={queue_id}");
        println!("runtime_retry_generation={}", retry.generation());
        println!("runtime_retry_delay_seconds={}", delay.max(1));
        println!("runtime_retry_not_before_unix_seconds={not_before}");
        println!("runtime_outbound_status=retry-scheduled");
        Ok(())
    })();
    let mirror_result = match vault_guard {
        Some(guard) => guard.finish(),
        None => Ok(()),
    };
    drop(state_lock);
    combine_operation_and_mirror(operation_result, mirror_result)
}

struct PreparedRuntimeSync {
    contact: SignedRuntimeContact,
    candidates: Vec<(DeviceId, String)>,
}

async fn attempt_runtime_contact_sync(
    state_directory: &Path,
    interval: Duration,
    last_attempts: &mut BTreeMap<RuntimeContactId, tokio::time::Instant>,
) -> Result<bool> {
    let state_lock = acquire_runtime_state_lock(state_directory)
        .await?
        .context("runtime state lock remained busy while preparing automatic sync")?;
    let vault_guard = VaultDualWriteGuard::prepare(state_directory)?;
    let preparation: Result<Option<PreparedRuntimeSync>> = (|| {
        let device_state = load_command_device_state(state_directory)?;
        let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
        let certificate = trust.load_certificate()?;
        let authority = trust.load_own_authority_snapshot(&certificate)?;
        let snapshot = load_runtime_state_snapshot(
            state_directory,
            certificate.account_id(),
            device_state.identity().device_id(),
        )?;
        let now = tokio::time::Instant::now();
        let contact = snapshot
            .contacts
            .values()
            .find(|contact| {
                last_attempts
                    .get(&contact.contact_id())
                    .is_none_or(|last| now.duration_since(*last) >= interval)
            })
            .cloned();
        let Some(contact) = contact else {
            return Ok(None);
        };
        last_attempts.insert(contact.contact_id(), now);
        let candidates =
            load_runtime_endpoint_candidate_set(&snapshot, &contact, &certificate, &authority)?
                .into_iter()
                .filter(|candidate| candidate.authority_current)
                .map(|candidate| Ok((candidate.peer_device_id, candidate.ticket.encode()?)))
                .collect::<Result<Vec<_>>>()?;
        ensure!(
            !candidates.is_empty(),
            "automatic sync has no endpoint candidate at the authority high-water revision"
        );
        Ok(Some(PreparedRuntimeSync {
            contact,
            candidates,
        }))
    })();

    let operation_result = match preparation {
        Ok(Some(PreparedRuntimeSync {
            contact,
            candidates,
        })) => {
            println!("runtime_sync_contact_id={}", contact.contact_id());
            let mut failures = Vec::new();
            let mut synchronized = false;
            for (index, (peer_device_id, encoded_ticket)) in candidates.iter().enumerate() {
                println!("runtime_sync_endpoint_attempt={}", index + 1);
                println!("runtime_sync_endpoint_device_id={peer_device_id}");
                match sync(
                    state_directory.to_path_buf(),
                    Some(encoded_ticket.clone()),
                    None,
                    contact.conversation_label().to_owned(),
                    MAX_SYNC_ROUNDS,
                    contact.peer_account_id(),
                )
                .await
                {
                    Ok(()) => {
                        println!("runtime_sync_endpoint_failover_count={index}");
                        synchronized = true;
                        break;
                    }
                    Err(error) => {
                        eprintln!(
                            "runtime_sync_endpoint_status=failed peer_device_id={peer_device_id} error={error:#}"
                        );
                        failures.push(format!("{peer_device_id}: {error:#}"));
                    }
                }
            }
            ensure!(
                synchronized,
                "all {} authenticated sync endpoint candidates failed: {}",
                candidates.len(),
                failures.join(" | ")
            );
            Ok(())
        }
        Ok(None) => {
            let mirror_result = match vault_guard {
                Some(guard) => guard.finish(),
                None => Ok(()),
            };
            drop(state_lock);
            mirror_result?;
            return Ok(false);
        }
        Err(error) => Err(error),
    };
    let mirror_result = match vault_guard {
        Some(guard) => guard.finish(),
        None => Ok(()),
    };
    drop(state_lock);
    mirror_result.context("finish automatic sync state-vault mirror")?;
    match operation_result {
        Ok(()) => println!("runtime_sync_status=synchronized"),
        Err(error) => eprintln!("runtime_sync_status=failed error={error:#}"),
    }
    Ok(true)
}

async fn wait_for_runtime_event(
    accept: &mut Pin<Box<dyn Future<Output = Result<Connection>> + Send + '_>>,
    shutdown: &mut Pin<Box<dyn Future<Output = io::Result<()>> + Send + '_>>,
    ipc_receiver: &mut tokio::sync::mpsc::Receiver<RuntimeIpcWork>,
    idle_deadline: Option<tokio::time::Instant>,
    poll_tick: &mut tokio::time::Interval,
) -> Result<RuntimeEvent> {
    if let Some(idle_deadline) = idle_deadline {
        tokio::select! {
            connection = accept.as_mut() => connection.map(RuntimeEvent::Connection),
            signal = shutdown.as_mut() => {
                signal.context("install or receive Ctrl+C runtime signal")?;
                Ok(RuntimeEvent::Shutdown)
            }
            work = ipc_receiver.recv() => Ok(work.map_or(RuntimeEvent::IpcClosed, RuntimeEvent::Ipc)),
            _ = poll_tick.tick() => Ok(RuntimeEvent::Tick),
            () = tokio::time::sleep_until(idle_deadline) => Ok(RuntimeEvent::IdleTimeout),
        }
    } else {
        tokio::select! {
            connection = accept.as_mut() => connection.map(RuntimeEvent::Connection),
            signal = shutdown.as_mut() => {
                signal.context("install or receive Ctrl+C runtime signal")?;
                Ok(RuntimeEvent::Shutdown)
            }
            work = ipc_receiver.recv() => Ok(work.map_or(RuntimeEvent::IpcClosed, RuntimeEvent::Ipc)),
            _ = poll_tick.tick() => Ok(RuntimeEvent::Tick),
        }
    }
}

async fn acquire_runtime_state_lock(state_directory: &Path) -> Result<Option<StateDirectoryLock>> {
    let started = tokio::time::Instant::now();
    loop {
        match StateDirectoryLock::acquire(state_directory) {
            Ok(state_lock) => return Ok(Some(state_lock)),
            Err(StateError::AlreadyLocked { .. })
                if started.elapsed() < RUNTIME_STATE_LOCK_WAIT =>
            {
                tokio::time::sleep(RUNTIME_STATE_LOCK_RETRY).await;
            }
            Err(StateError::AlreadyLocked { .. }) => return Ok(None),
            Err(error) => {
                return Err(error)
                    .context("lock state directory for an accepted runtime application session");
            }
        }
    }
}

async fn handle_runtime_application_connection(
    connection: &Connection,
    state_directory: &Path,
    endpoint_announcement_descriptor_directory: Option<&Path>,
    ticket: &ConnectionTicket,
    session_binding: SyncSessionBinding,
    allowed_requester_account_id: AccountId,
    route_policy: RoutePolicy,
) -> Result<Result<()>> {
    let Some(state_lock) = acquire_runtime_state_lock(state_directory).await? else {
        return Ok(Err(anyhow::Error::msg(format!(
            "runtime state lock remained busy for {:.1}s",
            RUNTIME_STATE_LOCK_WAIT.as_secs_f64()
        ))));
    };
    let vault_guard = VaultDualWriteGuard::prepare(state_directory)?;
    let preparation_result = (|| {
        let device_state = load_command_device_state(state_directory)?;
        ensure!(
            device_state.identity().device_id() == ticket.listener_device_id(),
            "runtime device identity changed after the connection ticket was published"
        );
        let trust = CommandTrustReadRepository::open(state_directory, &device_state)?;
        let listener_certificate = trust
            .load_certificate()
            .context("load runtime listener certificate for accepted session")?;
        ensure!(
            listener_certificate == *ticket.listener_certificate(),
            "runtime listener certificate changed after the connection ticket was published"
        );
        let immutable_reads = open_immutable_read_repositories(state_directory)
            .context("capture immutable runtime session state before local changes")?;
        let event_store = open_event_store(state_directory)?;
        let local_message_store = open_local_message_store(state_directory)?;
        Ok::<_, anyhow::Error>((
            device_state,
            listener_certificate,
            immutable_reads,
            event_store,
            local_message_store,
        ))
    })();
    let operation_result = match preparation_result {
        Ok((
            device_state,
            listener_certificate,
            immutable_reads,
            event_store,
            local_message_store,
        )) => {
            handle_authorized_application_connection(
                connection,
                state_directory,
                &device_state,
                &event_store,
                &local_message_store,
                immutable_reads,
                &listener_certificate,
                session_binding,
                allowed_requester_account_id,
                route_policy,
                ticket,
                ticket.listener_directory().device_list(),
                None,
                endpoint_announcement_descriptor_directory,
            )
            .await
        }
        Err(error) => {
            let mirror_result = match vault_guard {
                Some(guard) => guard.finish(),
                None => Ok(()),
            };
            drop(state_lock);
            return match mirror_result {
                Ok(()) => Err(error.context("prepare accepted runtime session state")),
                Err(mirror_error) => Err(error.context(format!(
                    "prepare accepted runtime session state and finish failed vault mirror: {mirror_error:#}"
                ))),
            };
        }
    };
    let mirror_result = match vault_guard {
        Some(guard) => guard.finish(),
        None => Ok(()),
    };
    let result = match mirror_result {
        Ok(()) => Ok(operation_result),
        Err(mirror_error) => Err(mirror_error
            .context("finish runtime session state-vault mirror after application operation")),
    };
    drop(state_lock);
    result
}

async fn listen_inner(options: ListenOptions) -> Result<()> {
    let ListenOptions {
        state_dir,
        allowed_requester_account_id,
        device_list_file,
        peer_prekey_pool_files,
        ticket_file,
        relay_wait_seconds,
        route_policy,
        relay_url,
        history_rewrap_conversation,
        history_rewrap_recipient_device,
        history_rewrap_approve_sas,
        history_rewrap_range_start,
        history_rewrap_count,
        history_recovery_link_file,
        history_recovery_qr_file,
        history_recovery_link_page_size,
        history_recovery_link_valid_for_seconds,
        history_recovery_discovery_publish,
    } = options;
    let device_state = load_command_device_state(&state_dir)?;
    let trust = CommandTrustReadRepository::open(&state_dir, &device_state)?;
    let listener_certificate = trust
        .load_certificate()
        .context("load listener Account Root certificate")?;
    let listener_device_list = AccountDeviceListSnapshot::decode_and_verify(
        &fs::read(&device_list_file)
            .with_context(|| format!("read device list from {}", device_list_file.display()))?,
    )
    .context("decode and verify listener account device list")?;
    ensure!(
        listener_device_list.certificate_for(listener_certificate.device_id())
            == Some(&listener_certificate),
        "listener certificate is not present exactly in the supplied device list"
    );
    let history_rewrap_approval = prepare_history_rewrap_approval(
        history_rewrap_conversation,
        history_rewrap_recipient_device,
        history_rewrap_approve_sas,
        history_rewrap_range_start,
        history_rewrap_count,
        &listener_device_list,
        &listener_certificate,
        allowed_requester_account_id,
    )?;
    let publishes_history_recovery_link = history_recovery_link_file.is_some()
        || history_recovery_qr_file.is_some()
        || history_recovery_discovery_publish;
    if publishes_history_recovery_link {
        ensure!(
            (1..=MAX_HISTORY_REWRAP_ENTRIES).contains(&history_recovery_link_page_size),
            "--history-recovery-link-page-size must be between 1 and {MAX_HISTORY_REWRAP_ENTRIES}"
        );
        ensure!(
            (1..=MAX_HISTORY_RECOVERY_LINK_VALIDITY_SECONDS)
                .contains(&history_recovery_link_valid_for_seconds),
            "--history-recovery-link-valid-for-seconds must be between 1 and {MAX_HISTORY_RECOVERY_LINK_VALIDITY_SECONDS}"
        );
    }
    let immutable_reads = open_immutable_read_repositories(&state_dir)
        .context("capture immutable vault-primary listener state before local changes")?;
    let authority_snapshot_store = install_own_authority_primary(
        &state_dir,
        &device_state,
        listener_device_list.authority_snapshot(),
    )
    .context("install authority snapshot embedded in listener device list")?;
    let event_store = open_event_store(&state_dir)?;
    let local_message_store = open_local_message_store(&state_dir)?;
    let now_unix_seconds = unix_time_now().context("read time for listener prekey freshness")?;
    let listener_prekey_pool = run_state_transaction(&state_dir, |transaction| {
        let mut ratchet_state = transaction.load_ratchet_state()?;
        let listener_prekey_pool = ratchet_state
            .prekey_pool(
                device_state.identity(),
                DEFAULT_PREKEY_POOL_SIZE,
                now_unix_seconds,
                DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
            )
            .context("publish listener one-time prekey pool")?;
        Ok(listener_prekey_pool)
    })?;
    let mut prekey_pools = vec![listener_prekey_pool.clone()];
    for path in peer_prekey_pool_files {
        prekey_pools.push(
            SignedPrekeyPool::decode(
                &fs::read(&path)
                    .with_context(|| format!("read peer prekey pool from {}", path.display()))?,
            )
            .with_context(|| format!("verify peer prekey pool from {}", path.display()))?,
        );
    }
    let listener_directory = AccountPrekeyDirectory::new(listener_device_list, prekey_pools)
        .context("assemble complete listener account prekey directory")?;
    listener_directory
        .verify_at(now_unix_seconds)
        .context("verify listener account prekey directory freshness")?;
    let endpoint = endpoint_builder_with_relay(route_policy, relay_url)
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .context("bind listening Iroh endpoint")?;

    wait_for_relay(&endpoint, route_policy, relay_wait_seconds).await?;

    let ticket = ConnectionTicket::new(
        endpoint.addr(),
        device_state.identity(),
        listener_certificate.clone(),
        listener_directory,
        allowed_requester_account_id,
        route_policy,
    )?;
    let encoded_ticket = ticket.encode()?;
    let recovery_link = publishes_history_recovery_link
        .then(|| {
            let approval = history_rewrap_approval.as_ref().context(
                "history recovery link/QR output requires complete history-rewrap approval flags",
            )?;
            let issued_at_unix_seconds =
                unix_time_now().context("read time for history recovery link")?;
            let link = SignedHistoryRecoveryLink::sign(
                device_state.identity(),
                HistoryRecoveryLinkOptions {
                    endpoint: ticket.endpoint().clone(),
                    source_certificate: listener_certificate.clone(),
                    account_device_list: ticket.listener_directory().device_list().clone(),
                    recipient_device_id: approval.recipient_device_id,
                    conversation_id: approval.conversation_id,
                    approved_range_start: approval.range_start,
                    approved_event_count: approval.count,
                    page_size: history_recovery_link_page_size,
                    route_policy,
                    issued_at_unix_seconds,
                    valid_for_seconds: history_recovery_link_valid_for_seconds,
                },
            )?;
            let encoded = link.encode_text()?;
            Ok::<_, anyhow::Error>((link, encoded))
        })
        .transpose()?;
    if let (Some(ticket_path), Some(link_path)) =
        (ticket_file.as_ref(), history_recovery_link_file.as_ref())
    {
        ensure!(
            ticket_path != link_path,
            "--ticket-file and --history-recovery-link-file must be different paths"
        );
    }
    if let (Some(ticket_path), Some(qr_path)) =
        (ticket_file.as_ref(), history_recovery_qr_file.as_ref())
    {
        ensure!(
            ticket_path != qr_path,
            "--ticket-file and --history-recovery-qr-file must be different paths"
        );
    }
    if let (Some(link_path), Some(qr_path)) = (
        history_recovery_link_file.as_ref(),
        history_recovery_qr_file.as_ref(),
    ) {
        ensure!(
            link_path != qr_path,
            "--history-recovery-link-file and --history-recovery-qr-file must be different paths"
        );
    }
    println!("transport_endpoint_id={}", endpoint.id());
    println!("account_id={}", ticket.listener_account_id());
    println!("device_id={}", device_state.identity().device_id());
    println!(
        "ratchet_prekey_pool_generation={}",
        listener_prekey_pool.generation()
    );
    println!(
        "ratchet_prekey_sequence_range={}..={}",
        listener_prekey_pool.first_sequence(),
        listener_prekey_pool.last_sequence()
    );
    println!(
        "ratchet_prekey_pool_expires_at={}",
        listener_prekey_pool.expires_at_unix_seconds()
    );
    println!(
        "fanout_device_count={}",
        ticket.listener_directory().pools().len()
    );
    println!("route_policy={}", route_policy.as_str());
    println!("allowed_requester_account_id={allowed_requester_account_id}");
    println!(
        "authority_revision={}",
        ticket.listener_authority_snapshot().revision()
    );
    println!("authority_store={authority_snapshot_store:?}");
    if let Some(approval) = &history_rewrap_approval {
        print_immutable_read_diagnostics("history_rewrap", &immutable_reads);
        println!(
            "history_rewrap_source_device_id={}",
            listener_certificate.device_id()
        );
        println!(
            "history_rewrap_recipient_device_id={}",
            approval.recipient_device_id
        );
        println!("history_rewrap_sas={}", approval.sas);
        println!("history_rewrap_range_start={}", approval.range_start);
        println!("history_rewrap_count={}", approval.count);
        println!("history_rewrap_user_consent=approved");
    }
    println!("ticket={encoded_ticket}");

    if let Some(path) = ticket_file {
        tokio::fs::write(&path, &encoded_ticket)
            .await
            .with_context(|| format!("write ticket to {}", path.display()))?;
        println!("ticket_file={}", path.display());
    }
    let mut recovery_discovery_publisher = None;
    if let Some((link, encoded)) = recovery_link {
        println!("history_recovery_link_id={}", encode_hex(&link.link_id()?));
        println!(
            "history_recovery_link_expires_at={}",
            link.expires_at_unix_seconds()
        );
        println!("history_recovery_link_text_bytes={}", encoded.len());
        println!("history_recovery_link_qr_ready=true");
        println!("history_recovery_link={encoded}");
        if let Some(path) = history_recovery_link_file {
            tokio::fs::write(&path, &encoded)
                .await
                .with_context(|| format!("write history recovery link to {}", path.display()))?;
            println!("history_recovery_link_file={}", path.display());
        }
        if let Some(path) = history_recovery_qr_file {
            let report = render_recovery_link_qr_png(&encoded, &path)?;
            print_recovery_qr_render_report(&report, &path);
        }
        if history_recovery_discovery_publish {
            let (publisher, report) = start_recovery_discovery_publisher(encoded).await?;
            println!("history_recovery_discovery_scope=local-network");
            println!("history_recovery_discovery_opt_in=true");
            println!(
                "history_recovery_discovery_multicast_target={}",
                multicast_target()
            );
            println!(
                "history_recovery_discovery_loopback_target={}",
                loopback_target()
            );
            println!(
                "history_recovery_discovery_interval_ms={}",
                publication_interval().as_millis()
            );
            println!(
                "history_recovery_discovery_multicast_initial_sent={}",
                report.multicast_initial_sent
            );
            println!(
                "history_recovery_discovery_loopback_initial_sent={}",
                report.loopback_initial_sent
            );
            println!("history_recovery_discovery_metadata_visible_to_lan=true");
            println!("history_recovery_discovery_status=publishing");
            recovery_discovery_publisher = Some(publisher);
        }
    }

    println!("status=listening");
    let connection = accept_authenticated_connection(&endpoint).await?;
    println!("peer_id={}", connection.remote_id());

    let ready_path = await_route_policy(&connection, route_policy, ROUTE_POLICY_WAIT)
        .await
        .context("wait for an incoming path allowed by the connection ticket")?;
    print_ready_path(&ready_path);

    let session_binding = SyncSessionBinding::from_transport_label(&endpoint.id().to_string());
    handle_authorized_application_connection(
        &connection,
        &state_dir,
        &device_state,
        &event_store,
        &local_message_store,
        immutable_reads,
        &listener_certificate,
        session_binding,
        allowed_requester_account_id,
        route_policy,
        &ticket,
        ticket.listener_directory().device_list(),
        history_rewrap_approval.as_ref(),
        None,
    )
    .await?;
    let _ = timeout(Duration::from_secs(2), connection.closed()).await;
    drop(recovery_discovery_publisher);
    endpoint.close().await;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_authorized_application_connection(
    connection: &Connection,
    state_directory: &Path,
    device_state: &DeviceState,
    event_store: &EventStore,
    local_message_store: &LocalMessageStore,
    immutable_reads: ImmutableReadRepositories,
    listener_certificate: &DeviceCertificate,
    session_binding: SyncSessionBinding,
    allowed_requester_account_id: AccountId,
    route_policy: RoutePolicy,
    current_ticket: &ConnectionTicket,
    listener_device_list: &AccountDeviceListSnapshot,
    history_rewrap_approval: Option<&HistoryRewrapApproval>,
    endpoint_announcement_descriptor_directory: Option<&Path>,
) -> Result<()> {
    let authorized_requester = accept_device_authorization(
        connection,
        state_directory,
        device_state,
        session_binding,
        allowed_requester_account_id,
        listener_certificate.account_id(),
    )
    .await?;

    let (mut send, mut receive) =
        accept_bi(connection, "accept authorized application stream").await?;
    let request = read_client_request(&mut receive).await?;
    let print_transport_after_request = match request {
        ClientRequest::DeliverEvent(event) => {
            handle_delivery_request(
                DeliveryState {
                    state_directory,
                    device_state,
                    event_store,
                    local_message_store,
                },
                &mut send,
                *event,
                &authorized_requester,
            )
            .await?;
            true
        }
        ClientRequest::SyncInventory(inventory) => {
            print_immutable_read_diagnostics("sync", &immutable_reads);
            let decrypting_store = DecryptingSessionStore::new(
                state_directory,
                event_store,
                local_message_store,
                device_state,
                listener_certificate.account_id(),
                immutable_reads,
            );
            handle_sync_request(
                SyncHandlerState {
                    state_directory,
                    device_state,
                    decrypting_store: &decrypting_store,
                },
                connection,
                send,
                inventory,
                session_binding,
                &authorized_requester,
            )
            .await?;
            true
        }
        ClientRequest::SyncEvents(_) => bail!("sync event batch cannot be the first request"),
        ClientRequest::SyncPause(_) => bail!("sync pause cannot be the first request"),
        ClientRequest::HistoryRewrap(request) => {
            drop(receive);
            serve_history_rewrap_session(
                state_directory,
                device_state,
                Some(&immutable_reads),
                connection,
                route_policy,
                send,
                request,
                session_binding,
                &authorized_requester,
                listener_device_list,
                history_rewrap_approval,
            )
            .await?;
            false
        }
        ClientRequest::EndpointAnnouncementPush(envelope_bytes) => {
            let descriptor_directory = endpoint_announcement_descriptor_directory.context(
                "endpoint announcement network push requires a long-lived runtime descriptor directory",
            )?;
            ensure!(
                authorized_requester.account_id() == listener_certificate.account_id(),
                "endpoint announcement network push requires a same-account Device session"
            );
            let import = (|| {
                let envelope = EncryptedEndpointAnnouncementBundle::decode(&envelope_bytes)?;
                let bundle_id = envelope.bundle_id();
                let report = import_runtime_endpoint_announcement_envelope(
                    state_directory,
                    current_ticket,
                    envelope,
                    descriptor_directory,
                    Some(authorized_requester.device_id()),
                )?;
                Ok::<_, anyhow::Error>((bundle_id, report))
            })();
            let (bundle_id, report) = match import {
                Ok(imported) => imported,
                Err(error) => {
                    write_server_response(&mut send, &ServerResponse::EndpointAnnouncementRejected)
                        .await?;
                    return Err(error).context("reject endpoint announcement network push");
                }
            };
            let acknowledgement = SignedEndpointAnnouncementAcknowledgement::sign(
                device_state.identity(),
                session_binding,
                bundle_id,
                authorized_requester.device_id(),
                report.authority_revision,
                report.contact_count,
                report.contact_added_count,
                report.endpoint_count,
                report.endpoint_added_count,
                report.publication_binding_added_count,
                report.observation_evidence_count,
                report.observation_evidence_added_count,
            )?;
            write_server_response(
                &mut send,
                &ServerResponse::EndpointAnnouncementAcknowledged(acknowledgement.encode()?),
            )
            .await?;
            print_runtime_endpoint_announcement_import(&report);
            println!("endpoint_announcement_transport=authenticated-same-account-device-session");
            println!("endpoint_announcement_acknowledgement=session-bound-recipient-signed");
            true
        }
        ClientRequest::AuthorizeDevice(_) => {
            bail!("device authorization cannot be repeated on an authorized connection")
        }
    };

    if print_transport_after_request {
        print_transport_diagnostics(connection, route_policy).await?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn serve_history_rewrap_session(
    state_directory: &Path,
    device_state: &DeviceState,
    read_repositories: Option<&ImmutableReadRepositories>,
    connection: &Connection,
    route_policy: RoutePolicy,
    mut send: SendStream,
    mut request: SignedHistoryRewrapRequest,
    expected_session: SyncSessionBinding,
    authorized_requester: &AuthorizedDevice,
    device_list: &AccountDeviceListSnapshot,
    approval: Option<&HistoryRewrapApproval>,
) -> Result<()> {
    let mut progress = HistoryRecoverySessionProgress::default();

    loop {
        if !progress.accepts(request.range_start()) {
            write_server_response(
                &mut send,
                &ServerResponse::HistoryRewrapRejected(HistoryRewrapRejected::new(
                    request.conversation_id(),
                    HistoryRewrapRejectionReason::ApprovalMismatch,
                )),
            )
            .await?;
            println!("history_rewrap_rejected=NonContiguousRange");
            println!("status=rejected");
            break;
        }

        let outcome = handle_history_rewrap_request(
            state_directory,
            device_state,
            read_repositories,
            &mut send,
            request,
            expected_session,
            authorized_requester,
            device_list,
            approval,
        )
        .await?;
        if progress.pages_served == 0 {
            print_transport_diagnostics(connection, route_policy).await?;
        }
        let HistoryRewrapServeOutcome::Transferred {
            next_range_start,
            recovery_complete,
        } = outcome
        else {
            break;
        };
        progress.record_page(next_range_start, recovery_complete);
        println!(
            "history_recovery_session_pages_served={}",
            progress.pages_served
        );
        if progress.complete {
            break;
        }
        if !progress.can_request_next() {
            println!("history_recovery_session_limit_reached=true");
            break;
        }
        drop(send);
        let Some((next_send, mut receive)) =
            accept_optional_history_rewrap_stream(connection).await?
        else {
            break;
        };
        send = next_send;
        request = match read_client_request(&mut receive).await? {
            ClientRequest::HistoryRewrap(request) => request,
            _ => bail!("history recovery session only accepts history-rewrap page requests"),
        };
    }

    println!("history_recovery_session_complete={}", progress.complete);
    println!(
        "status={}",
        if progress.complete {
            "history-recovery-session-served"
        } else {
            "history-recovery-session-paused"
        }
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn prepare_history_rewrap_approval(
    conversation: Option<String>,
    recipient_device_id: Option<DeviceId>,
    approved_sas: Option<String>,
    range_start: usize,
    count: usize,
    device_list: &AccountDeviceListSnapshot,
    source_certificate: &DeviceCertificate,
    allowed_requester_account_id: AccountId,
) -> Result<Option<HistoryRewrapApproval>> {
    let any_option =
        conversation.is_some() || recipient_device_id.is_some() || approved_sas.is_some();
    if !any_option {
        return Ok(None);
    }
    let conversation = conversation.context(
        "--history-rewrap-conversation is required when history rewrap approval is configured",
    )?;
    let recipient_device_id = recipient_device_id.context(
        "--history-rewrap-recipient-device is required when history rewrap approval is configured",
    )?;
    let approved_sas = approved_sas.context(
        "--history-rewrap-approve-sas is required when history rewrap approval is configured",
    )?;
    ensure!(
        count > 0,
        "--history-rewrap-count must be greater than zero"
    );
    let approved_range_end = range_start
        .checked_add(count)
        .context("--history-rewrap-range-start plus --history-rewrap-count overflows")?;
    ensure!(
        approved_range_end <= MAX_INVENTORY_EVENT_IDS,
        "approved history-rewrap range end must not exceed the bounded inventory limit of {MAX_INVENTORY_EVENT_IDS}"
    );
    ensure!(
        source_certificate.account_id() == allowed_requester_account_id,
        "network history rewrap is restricted to devices of the listener account"
    );
    ensure!(
        device_list.certificate_for(recipient_device_id).is_some(),
        "history-rewrap recipient is absent from the supplied root-signed device list"
    );
    let sas = HistoryRewrapSas::derive(
        device_list,
        source_certificate.device_id(),
        recipient_device_id,
    )?;
    ensure!(
        approved_sas.trim() == sas.to_string(),
        "source user approved SAS {}, but the signed device list derives {sas}",
        approved_sas.trim()
    );
    Ok(Some(HistoryRewrapApproval {
        conversation_id: ConversationId::from_label(&conversation),
        recipient_device_id,
        sas,
        range_start,
        count,
    }))
}

#[allow(clippy::too_many_arguments)]
async fn handle_history_rewrap_request(
    state_directory: &Path,
    device_state: &DeviceState,
    read_repositories: Option<&ImmutableReadRepositories>,
    send: &mut SendStream,
    request: SignedHistoryRewrapRequest,
    expected_session: SyncSessionBinding,
    authorized_requester: &AuthorizedDevice,
    device_list: &AccountDeviceListSnapshot,
    approval: Option<&HistoryRewrapApproval>,
) -> Result<HistoryRewrapServeOutcome> {
    let Some(approval) = approval else {
        write_server_response(
            send,
            &ServerResponse::HistoryRewrapRejected(HistoryRewrapRejected::new(
                request.conversation_id(),
                HistoryRewrapRejectionReason::NotApproved,
            )),
        )
        .await?;
        println!("history_rewrap_rejected=NotApproved");
        println!("status=rejected");
        return Ok(HistoryRewrapServeOutcome::Rejected);
    };
    let trust = CommandTrustReadRepository::open(state_directory, device_state)?;
    let source_certificate = trust
        .load_certificate()
        .context("load source certificate for network history rewrap")?;
    let membership = trust
        .load_conversation_membership(approval.conversation_id.scope_id())
        .context("load DB-primary membership for network history rewrap")?;
    let requested_start = usize::try_from(request.range_start()).ok();
    let requested_count = usize::try_from(request.max_event_count()).ok();
    let request_matches_approval = request
        .verify_for_session(
            expected_session,
            source_certificate.device_id(),
            authorized_requester.device_id(),
            approval.sas,
        )
        .is_ok()
        && authorized_requester.account_id() == source_certificate.account_id()
        && request.conversation_id() == approval.conversation_id
        && request.recipient_device_id() == approval.recipient_device_id
        && requested_start
            .zip(requested_count)
            .is_some_and(|(start, count)| approval.contains_range(start, count))
        && device_list
            .certificate_for(authorized_requester.device_id())
            .is_some();
    if !request_matches_approval {
        write_server_response(
            send,
            &ServerResponse::HistoryRewrapRejected(HistoryRewrapRejected::new(
                request.conversation_id(),
                HistoryRewrapRejectionReason::ApprovalMismatch,
            )),
        )
        .await?;
        println!("history_rewrap_rejected=ApprovalMismatch");
        println!("status=rejected");
        return Ok(HistoryRewrapServeOutcome::Rejected);
    }
    let read_repositories = read_repositories
        .context("approved network history rewrap is missing its immutable read snapshot")?;

    let bundle = build_history_rewrap_bundle(
        device_state,
        &source_certificate,
        device_list.clone(),
        approval.recipient_device_id,
        approval.conversation_id,
        &membership,
        read_repositories.events.as_ref(),
        read_repositories.local_messages.as_ref(),
        requested_start.context("history-rewrap request range cannot be represented")?,
        requested_count.context("history-rewrap request count cannot be represented")?,
    )?;
    let transfer = SignedHistoryRewrapTransfer::sign(device_state.identity(), request, bundle)?;
    let transfer_size = transfer.encode()?.len();
    if transfer_size > MAX_WIRE_MESSAGE_BYTES {
        write_server_response(
            send,
            &ServerResponse::HistoryRewrapRejected(HistoryRewrapRejected::new(
                approval.conversation_id,
                HistoryRewrapRejectionReason::TransferTooLarge,
            )),
        )
        .await?;
        println!("history_rewrap_rejected=TransferTooLarge");
        println!("history_rewrap_transfer_bytes={transfer_size}");
        println!("status=rejected");
        return Ok(HistoryRewrapServeOutcome::Rejected);
    }
    write_server_response(
        send,
        &ServerResponse::HistoryRewrapTransfer(Box::new(transfer.clone())),
    )
    .await?;
    println!("history_rewrap_id={}", transfer.bundle().bundle_id()?);
    println!("history_rewrap_transfer_bytes={transfer_size}");
    println!("history_rewrap_user_consent=approved");
    println!("status=history-rewrap-transferred");
    let approved_range_end = u64::try_from(
        approval
            .range_start
            .checked_add(approval.count)
            .context("approved history-rewrap range overflows")?,
    )
    .context("approved history-rewrap range cannot be represented")?;
    let recovery_complete = transfer.bundle().manifest().range_end()
        >= approved_range_end.min(transfer.bundle().manifest().inventory_event_count());
    Ok(HistoryRewrapServeOutcome::Transferred {
        next_range_start: transfer.bundle().manifest().range_end(),
        recovery_complete,
    })
}

async fn accept_device_authorization(
    connection: &Connection,
    state_directory: &Path,
    device_state: &DeviceState,
    expected_session: SyncSessionBinding,
    allowed_account: AccountId,
    listener_account: AccountId,
) -> Result<AuthorizedDevice> {
    let (mut send, mut receive) =
        accept_bi(connection, "accept device authorization stream").await?;
    let authorization = match read_client_request(&mut receive).await? {
        ClientRequest::AuthorizeDevice(authorization) => authorization,
        _ => bail!("device authorization must be the first application request"),
    };
    let authorization_result = (|| {
        authorization.verify_for_session(expected_session)?;
        authorization
            .authority_snapshot()
            .verify_for_account(allowed_account)?;
        let snapshot_store = if allowed_account == listener_account {
            let trust = CommandTrustReadRepository::open(state_directory, device_state)?;
            let listener_certificate = trust
                .load_certificate()
                .context("load listener certificate for same-account authorization")?;
            let own_authority = trust
                .load_own_authority_snapshot(&listener_certificate)
                .context("load current own authority for same-account authorization")?;
            ensure!(
                authorization.authority_snapshot() == &own_authority,
                "same-account Device session does not carry the exact current Root authority"
            );
            AuthoritySnapshotStoreOutcome::Unchanged
        } else {
            pin_peer_authority_primary(
                state_directory,
                device_state,
                authorization.authority_snapshot(),
            )
            .context("pin requester authority snapshot and reject rollback")?
        };
        let authorized = authorize_device_session(
            allowed_account,
            &authorization,
            &DeviceCapability::MESSAGING,
            expected_session,
        )?;
        Ok::<_, anyhow::Error>((authorized, snapshot_store))
    })();
    match authorization_result {
        Ok((authorized, snapshot_store)) => {
            write_server_response(
                &mut send,
                &ServerResponse::DeviceAuthorized(DeviceAuthorizationAccepted::new(
                    expected_session,
                    authorized.account_id(),
                    authorized.device_id(),
                )),
            )
            .await?;
            println!(
                "authorized_requester_account_id={}",
                authorized.account_id()
            );
            println!("authorized_requester_device_id={}", authorized.device_id());
            println!(
                "requester_authority_revision={}",
                authorization.authority_snapshot().revision()
            );
            println!("requester_authority_store={snapshot_store:?}");
            println!("authorization=valid");
            Ok(authorized)
        }
        Err(error) => {
            write_server_response(
                &mut send,
                &ServerResponse::DeviceAuthorizationRejected(DeviceAuthorizationRejected::new()),
            )
            .await?;
            println!("authorization=rejected");
            Err(error).context("reject requester Account Root authorization")
        }
    }
}

async fn authorize_with_listener(
    connection: &Connection,
    identity: &DeviceIdentity,
    certificate: DeviceCertificate,
    authority_snapshot: AccountAuthoritySnapshot,
    session_binding: SyncSessionBinding,
) -> Result<()> {
    let expected_account = certificate.account_id();
    let expected_device = certificate.device_id();
    let authorization = SignedDeviceSessionAuthorization::sign(
        identity,
        certificate,
        authority_snapshot,
        session_binding,
    )
    .context("sign device authorization for transport session")?;
    let (mut send, mut receive) = open_bi(connection, "open device authorization stream").await?;
    write_client_request(&mut send, &ClientRequest::AuthorizeDevice(authorization)).await?;
    match read_server_response(&mut receive).await? {
        ServerResponse::DeviceAuthorized(accepted) => {
            accepted.verify(session_binding, expected_account, expected_device)?;
            println!("authorization=valid");
            Ok(())
        }
        ServerResponse::DeviceAuthorizationRejected(_) => {
            bail!("device authorization was rejected by listener")
        }
        _ => bail!("client expected a device authorization response"),
    }
}

struct DeliveryState<'a> {
    state_directory: &'a Path,
    device_state: &'a DeviceState,
    event_store: &'a EventStore,
    local_message_store: &'a LocalMessageStore,
}

async fn handle_delivery_request(
    state: DeliveryState<'_>,
    send: &mut SendStream,
    event: AuthorizedEvent,
    authorized_requester: &AuthorizedDevice,
) -> Result<()> {
    let DeliveryState {
        state_directory,
        device_state,
        event_store,
        local_message_store,
    } = state;
    let signed_event = event.event();
    let trust = CommandTrustReadRepository::open(state_directory, device_state)?;
    let membership = trust
        .load_conversation_membership(signed_event.conversation_id().scope_id())
        .context("load trusted conversation membership for received event")?;
    let listener_certificate = trust
        .load_certificate()
        .context("load listener certificate for acknowledgement")?;
    let listener_authority_snapshot = trust
        .load_own_authority_snapshot(&listener_certificate)
        .context("load listener authority snapshot for acknowledgement")?;
    require_conversation_participants(
        &membership,
        listener_certificate.account_id(),
        authorized_requester.account_id(),
    )?;
    event_store
        .authorized_inventory(signed_event.conversation_id(), &membership)
        .context("validate existing authorized history before delivery")?;
    event
        .verify_for_membership(&membership)
        .context("verify received event author and conversation membership")?;
    ensure!(
        event.author_account_id() == authorized_requester.account_id(),
        "event author account is not the requester account allowed by this listener"
    );
    ensure!(
        signed_event.author_device_id() == authorized_requester.device_id(),
        "event author is not the requester device allowed by this listener"
    );
    let event_id = signed_event
        .event_id()
        .context("calculate received event ID")?;
    let EventPayload::RatchetText { .. } = signed_event.payload() else {
        bail!("listener expected a text event");
    };
    ensure!(
        signed_event.recipient_device_list()?.account_id() == listener_certificate.account_id(),
        "received ratchet text recipient device list belongs to a different account"
    );
    let local_device_id = device_state.identity().device_id();
    ensure!(
        signed_event.ratchet_message_for(local_device_id).is_ok(),
        "received ratchet text has no ciphertext for this device"
    );
    let existing_acknowledgement = event_store
        .load_authorized_conversation(signed_event.conversation_id(), &membership)?
        .into_iter()
        .find(|candidate| {
            candidate.event.event().author_device_id() == local_device_id
                && matches!(
                    candidate.event.event().payload(),
                    EventPayload::Acknowledgement {
                        acknowledged_event_id
                    } if acknowledged_event_id == &event_id
                )
                && candidate.event.event().parents() == [event_id]
        })
        .map(|stored| stored.event);
    if let Some(acknowledgement) = existing_acknowledgement {
        let body = open_local_text_projection_if_present(
            local_message_store,
            device_state,
            listener_certificate.account_id(),
            signed_event,
        )?
        .context("a replayed acknowledged event has no retained local projection")?;
        let acknowledgement_id = acknowledgement.event().event_id()?;
        write_server_response(
            send,
            &ServerResponse::EventAcknowledgement(Box::new(acknowledgement)),
        )
        .await?;
        println!("received_event_id={event_id}");
        println!("received={body}");
        println!("received_store={:?}", StoreOutcome::AlreadyPresent);
        println!("delivery_replay=true");
        println!("acknowledgement_event_id={acknowledgement_id}");
        println!("acknowledgement_store={:?}", StoreOutcome::AlreadyPresent);
        println!("status=acknowledged");
        return Ok(());
    }
    let (
        body,
        local_projection_store_outcome,
        ratchet_operation,
        received_store_outcome,
        acknowledgement,
        acknowledgement_id,
        acknowledgement_store_outcome,
    ) = run_state_transaction(state_directory, |transaction| {
        let (body, local_projection_store_outcome, ratchet_operation) =
            match open_local_text_projection_if_present(
                local_message_store,
                device_state,
                listener_certificate.account_id(),
                signed_event,
            )? {
                Some(body) => (body, kilogram_store::StoreOutcome::AlreadyPresent, None),
                None => {
                    let mut ratchet_state = transaction.load_ratchet_state()?;
                    let (sender_ratchet_identity, ciphertext) =
                        signed_event.ratchet_message_for(local_device_id)?;
                    let (decrypted, operation) = ratchet_state
                        .decrypt(device_state.identity(), sender_ratchet_identity, ciphertext)
                        .context("decrypt received text through the persistent ratchet")?;
                    let body = decrypted.as_str().to_owned();
                    let (outcome, receipt) = ensure_received_local_text_projection(
                        local_message_store,
                        device_state,
                        listener_certificate.account_id(),
                        signed_event,
                        &decrypted,
                    )
                    .context("persist local history projection before the received event")?;
                    transaction.register_store_receipt(&receipt)?;
                    (body, outcome, Some(operation))
                }
            };
        let (received_store_outcome, received_receipt) = event_store
            .put_authorized_with_receipt(&event, &membership)
            .context("persist received event before acknowledging it")?;
        transaction.register_store_receipt(&received_receipt)?;

        let acknowledgement_sequence = transaction
            .allocate_sequence(device_state)
            .context("allocate acknowledgement sequence")?;
        let acknowledgement = SignedEvent::sign_acknowledgement(
            device_state.identity(),
            signed_event.conversation_id(),
            acknowledgement_sequence,
            vec![event_id],
            event_id,
        )
        .context("sign acknowledgement event")?;
        let acknowledgement = AuthorizedEvent::new(
            acknowledgement,
            listener_certificate.clone(),
            listener_authority_snapshot.clone(),
        )
        .context("attach listener Account Root authorization to acknowledgement")?;
        let acknowledgement_id = acknowledgement
            .event()
            .event_id()
            .context("calculate acknowledgement event ID")?;
        let (acknowledgement_store_outcome, acknowledgement_receipt) = event_store
            .put_authorized_with_receipt(&acknowledgement, &membership)
            .context("persist acknowledgement before sending it")?;
        transaction.register_store_receipt(&acknowledgement_receipt)?;
        Ok((
            body,
            local_projection_store_outcome,
            ratchet_operation,
            received_store_outcome,
            acknowledgement,
            acknowledgement_id,
            acknowledgement_store_outcome,
        ))
    })?;
    println!("received_event_id={event_id}");
    println!("received_author_account_id={}", event.author_account_id());
    println!(
        "received_author_device_id={}",
        signed_event.author_device_id()
    );
    println!(
        "received_author_sequence={}",
        signed_event.author_sequence()
    );
    println!("received={body}");
    if let Some(operation) = ratchet_operation {
        println!("ratchet_session_id={}", operation.session_id);
        println!("ratchet_session_created={}", operation.session_created);
        println!("ratchet_message_type={}", operation.message_kind);
        println!(
            "ratchet_concurrent_session_resolved={}",
            operation.concurrent_session_resolved
        );
        println!(
            "ratchet_retained_session_count={}",
            operation.retained_session_count
        );
    } else {
        println!("ratchet_replay=local-projection");
    }
    println!("received_local_projection={local_projection_store_outcome:?}");
    println!("received_store={received_store_outcome:?}");

    write_server_response(
        send,
        &ServerResponse::EventAcknowledgement(Box::new(acknowledgement)),
    )
    .await?;
    println!("acknowledgement_event_id={acknowledgement_id}");
    println!("acknowledgement_store={acknowledgement_store_outcome:?}");
    println!("status=acknowledged");
    Ok(())
}

struct SyncHandlerState<'a> {
    state_directory: &'a Path,
    device_state: &'a DeviceState,
    decrypting_store: &'a DecryptingSessionStore<'a>,
}

async fn handle_sync_request(
    state: SyncHandlerState<'_>,
    connection: &iroh::endpoint::Connection,
    first_send: SendStream,
    first_inventory: SignedSyncInventory,
    expected_session: SyncSessionBinding,
    authorized_requester: &AuthorizedDevice,
) -> Result<()> {
    let SyncHandlerState {
        state_directory,
        device_state,
        decrypting_store,
    } = state;
    let trust = CommandTrustReadRepository::open(state_directory, device_state)?;
    let membership = trust
        .load_conversation_membership(first_inventory.conversation_id().scope_id())
        .context("load trusted conversation membership for synchronization")?;
    let listener_account_id = trust
        .load_certificate()
        .context("load listener certificate for synchronization")?
        .account_id();
    require_conversation_participants(
        &membership,
        listener_account_id,
        authorized_requester.account_id(),
    )?;
    let server = SyncServer::new(
        device_state.identity(),
        decrypting_store,
        expected_session,
        authorized_requester.device_id(),
        &membership,
    );
    let mut inventory = first_inventory;
    let mut inventory_send = first_send;
    let mut total_sent_events = 0;
    let mut total_received_events = 0;

    for round_number in 1..=MAX_SYNC_ROUNDS {
        let server_round = match server.accept_inventory(&inventory)? {
            ServerInventoryOutcome::Accepted(round) => round,
            ServerInventoryOutcome::Rejected(rejected) => {
                write_server_response(
                    &mut inventory_send,
                    &ServerResponse::SyncRejected(rejected.clone()),
                )
                .await?;
                println!("sync_rejected={:?}", rejected.reason());
                println!("sync_rounds_completed={}", round_number - 1);
                println!("status=rejected");
                return Ok(());
            }
        };
        write_server_response(
            &mut inventory_send,
            &ServerResponse::SyncDiff(server_round.diff().clone()),
        )
        .await?;

        let (mut batch_send, mut batch_receive) =
            accept_bi(connection, "accept sync event batch stream").await?;
        let batch = match read_client_request(&mut batch_receive).await? {
            ClientRequest::SyncEvents(batch) => batch,
            _ => bail!("listener expected a sync event batch after a sync diff"),
        };
        let completion = server.complete_round(*server_round, batch)?;
        let stats = completion.stats();
        write_server_response(
            &mut batch_send,
            &ServerResponse::SyncComplete(completion.response().clone()),
        )
        .await?;
        total_sent_events += stats.sent_events;
        total_received_events += stats.received_events;
        println!(
            "sync_round_{round_number}_sent_events={}",
            stats.sent_events
        );
        println!(
            "sync_round_{round_number}_received_events={}",
            stats.received_events
        );

        if !stats.more_available {
            println!("sync_rounds_completed={round_number}");
            println!("sync_sent_events={total_sent_events}");
            println!("sync_received_events={total_received_events}");
            println!("sync_more_available=false");
            print_sync_overlay_diagnostics(decrypting_store)?;
            println!("status=synchronized");
            return Ok(());
        }
        let (mut next_send, mut next_receive) =
            accept_bi(connection, "accept next sync inventory stream").await?;
        inventory = match read_client_request(&mut next_receive).await? {
            ClientRequest::SyncInventory(inventory) => inventory,
            ClientRequest::SyncPause(pause) => {
                ensure!(
                    pause.conversation_id() == inventory.conversation_id(),
                    "sync pause belongs to a different conversation"
                );
                write_server_response(
                    &mut next_send,
                    &ServerResponse::SyncPaused(SyncPaused::new(pause.conversation_id())),
                )
                .await?;
                println!("sync_rounds_completed={round_number}");
                println!("sync_sent_events={total_sent_events}");
                println!("sync_received_events={total_received_events}");
                println!("sync_more_available=true");
                println!("sync_resume_checkpoint=event-store");
                print_sync_overlay_diagnostics(decrypting_store)?;
                println!("status=paused");
                return Ok(());
            }
            _ => bail!("listener expected another sync inventory for continuation"),
        };
        if round_number == MAX_SYNC_ROUNDS {
            bail!("sync exceeded the limit of {MAX_SYNC_ROUNDS} rounds");
        }
        inventory_send = next_send;
    }
    bail!("sync exceeded the limit of {MAX_SYNC_ROUNDS} rounds")
}

struct RatchetFanout {
    sender_identity: SignedRatchetIdentity,
    recipients: Vec<RatchetRecipient>,
    operations: Vec<(DeviceId, RatchetOperation)>,
}

fn encrypt_ratchet_fanout(
    ratchet_state: &mut RatchetState,
    sender: &DeviceIdentity,
    directory: &AccountPrekeyDirectory,
    plaintext: &str,
) -> Result<RatchetFanout> {
    let now_unix_seconds = unix_time_now().context("read time for recipient prekey freshness")?;
    directory
        .verify_at(now_unix_seconds)
        .context("verify recipient account prekey directory before fan-out")?;
    let mut sender_identity = None;
    let mut recipients = Vec::with_capacity(directory.pools().len());
    let mut operations = Vec::with_capacity(directory.pools().len());
    for pool in directory.pools() {
        let (current_sender_identity, ciphertext, operation) = ratchet_state
            .encrypt_with_pool(sender, pool, plaintext, now_unix_seconds)
            .with_context(|| {
                format!(
                    "advance and persist sender ratchet session for device {}",
                    pool.device_id()
                )
            })?;
        if let Some(expected) = &sender_identity {
            ensure!(
                expected == &current_sender_identity,
                "ratchet account returned inconsistent sender identities during fan-out"
            );
        } else {
            sender_identity = Some(current_sender_identity.clone());
        }
        recipients.push(RatchetRecipient::new(pool.device_id(), ciphertext)?);
        operations.push((pool.device_id(), operation));
    }
    Ok(RatchetFanout {
        sender_identity: sender_identity
            .context("recipient account prekey directory contains no devices")?,
        recipients,
        operations,
    })
}

async fn connect(
    state_dir: PathBuf,
    ticket: Option<String>,
    ticket_file: Option<PathBuf>,
    message: String,
    conversation: String,
    expected_listener_account_id: AccountId,
) -> Result<()> {
    let device_state = load_command_device_state(&state_dir)?;
    let trust = CommandTrustReadRepository::open(&state_dir, &device_state)?;
    let requester_certificate = trust
        .load_certificate()
        .context("load requester Account Root certificate")?;
    let requester_authority_snapshot = trust
        .load_own_authority_snapshot(&requester_certificate)
        .context("load requester Account Root authority snapshot")?;
    let requester_account_id = requester_certificate.account_id();
    let event_store = open_event_store(&state_dir)?;
    let local_message_store = open_local_message_store(&state_dir)?;
    let ticket = load_connection_ticket(ticket, ticket_file).await?;
    ticket.verify_listener_account(expected_listener_account_id)?;
    run_state_transaction(&state_dir, |transaction| {
        let ratchet_state = transaction.load_ratchet_state()?;
        ratchet_state
            .observe_prekey_directory(ticket.listener_directory(), unix_time_now()?)
            .context("observe listener prekey directory and reject rollback")
    })?;
    let listener_snapshot_store = pin_peer_authority_primary(
        &state_dir,
        &device_state,
        ticket.listener_authority_snapshot(),
    )
    .context("pin listener authority snapshot and reject rollback")?;
    let authorized_listener = ticket.verify_listener_authorization(expected_listener_account_id)?;
    let expected_listener_device_id = authorized_listener.device_id();
    let route_policy = ticket.route_policy();
    verify_device_authorization_with_snapshot(
        ticket.allowed_requester_account_id(),
        &requester_certificate,
        &requester_authority_snapshot,
        &DeviceCapability::MESSAGING,
    )
    .context("this device account is not authorized by the connection ticket")?;
    let conversation_id = ConversationId::from_label(&conversation);
    let membership = trust
        .load_conversation_membership(conversation_id.scope_id())
        .context("load trusted conversation membership before delivery")?;
    require_conversation_participants(
        &membership,
        requester_certificate.account_id(),
        authorized_listener.account_id(),
    )?;
    event_store
        .authorized_inventory(conversation_id, &membership)
        .context("validate existing authorized history before delivery")?;
    let session_binding =
        SyncSessionBinding::from_transport_label(&ticket.endpoint().id.to_string());

    let endpoint = endpoint_builder_for_remote(route_policy, ticket.endpoint())?
        .bind()
        .await
        .context("bind connecting Iroh endpoint")?;
    println!("transport_endpoint_id={}", endpoint.id());
    println!("account_id={}", requester_certificate.account_id());
    println!("device_id={}", device_state.identity().device_id());
    println!("target_account_id={}", authorized_listener.account_id());
    println!(
        "target_authority_revision={}",
        ticket.listener_authority_snapshot().revision()
    );
    println!("target_authority_store={listener_snapshot_store:?}");
    println!("route_policy={}", route_policy.as_str());
    print_connection_target(&ticket);

    if route_policy == RoutePolicy::RelayOnly {
        wait_for_relay(&endpoint, route_policy, CLIENT_RELAY_WAIT_SECONDS).await?;
    }

    let connection = timeout(
        CONNECTION_TIMEOUT,
        endpoint.connect(ticket.endpoint().clone(), ALPN),
    )
    .await
    .with_context(|| timeout_message("connect to listening endpoint", CONNECTION_TIMEOUT))?
    .context("connect to listening endpoint")?;
    println!("peer_id={}", connection.remote_id());

    let ready_path = await_route_policy(&connection, route_policy, ROUTE_POLICY_WAIT)
        .await
        .context("wait for a path allowed by the connection ticket")?;
    print_ready_path(&ready_path);

    authorize_with_listener(
        &connection,
        device_state.identity(),
        requester_certificate.clone(),
        requester_authority_snapshot.clone(),
        session_binding,
    )
    .await?;

    let (mut send, mut receive) =
        open_bi(&connection, "open delivery bidirectional stream").await?;
    let (
        author_sequence,
        ratchet_fanout,
        event,
        event_id,
        local_projection_store_outcome,
        sent_store_outcome,
    ) = run_state_transaction(&state_dir, |transaction| {
        let author_sequence = transaction
            .allocate_sequence(&device_state)
            .context("allocate message sequence")?;
        let parents = event_store
            .frontier(conversation_id)
            .context("calculate local conversation frontier")?;
        let mut ratchet_state = transaction.load_ratchet_state()?;
        let ratchet_fanout = encrypt_ratchet_fanout(
            &mut ratchet_state,
            device_state.identity(),
            ticket.listener_directory(),
            &message,
        )?;
        let signed_event = SignedEvent::sign_ratchet_text(
            device_state.identity(),
            conversation_id,
            author_sequence,
            parents,
            ticket.listener_directory().device_list().clone(),
            ratchet_fanout.sender_identity.clone(),
            ratchet_fanout.recipients.clone(),
        )
        .context("sign ratchet text event")?;
        let event = AuthorizedEvent::new(
            signed_event,
            requester_certificate,
            requester_authority_snapshot,
        )
        .context("attach requester Account Root authorization to sent event")?;
        let event_id = event
            .event()
            .event_id()
            .context("calculate sent event ID")?;
        let (local_projection_store_outcome, local_projection_receipt) =
            ensure_authored_local_text_projection(
                &local_message_store,
                &device_state,
                requester_account_id,
                event.event(),
                &message,
            )
            .context("persist local history projection before the sent event")?;
        transaction.register_store_receipt(&local_projection_receipt)?;
        let (sent_store_outcome, sent_receipt) = event_store
            .put_authorized_with_receipt(&event, &membership)
            .context("persist authorized event before sending it")?;
        transaction.register_store_receipt(&sent_receipt)?;
        Ok((
            author_sequence,
            ratchet_fanout,
            event,
            event_id,
            local_projection_store_outcome,
            sent_store_outcome,
        ))
    })?;
    write_client_request(
        &mut send,
        &ClientRequest::DeliverEvent(Box::new(event.clone())),
    )
    .await?;
    println!("sent_event_id={event_id}");
    println!("sent_author_sequence={author_sequence}");
    println!("fanout_recipient_count={}", ratchet_fanout.operations.len());
    for (device_id, operation) in &ratchet_fanout.operations {
        println!(
            "fanout_recipient_device_id={device_id} ratchet_session_id={} ratchet_session_created={} ratchet_message_type={} ratchet_retained_session_count={}",
            operation.session_id,
            operation.session_created,
            operation.message_kind,
            operation.retained_session_count
        );
    }
    let listener_operation = ratchet_fanout
        .operations
        .iter()
        .find(|(device_id, _)| *device_id == expected_listener_device_id)
        .map(|(_, operation)| operation)
        .context("fan-out diagnostics are missing the connected listener device")?;
    println!("ratchet_session_id={}", listener_operation.session_id);
    println!(
        "ratchet_session_created={}",
        listener_operation.session_created
    );
    println!("ratchet_message_type={}", listener_operation.message_kind);
    println!(
        "ratchet_retained_session_count={}",
        listener_operation.retained_session_count
    );
    println!("sent_local_projection={local_projection_store_outcome:?}");
    println!("sent_store={sent_store_outcome:?}");

    let acknowledgement = match read_server_response(&mut receive).await? {
        ServerResponse::EventAcknowledgement(event) => *event,
        _ => bail!("connector expected an event acknowledgement response"),
    };
    acknowledgement
        .verify_for_membership(&membership)
        .context("verify acknowledgement author and conversation membership")?;
    ensure!(
        acknowledgement.author_account_id() == authorized_listener.account_id(),
        "acknowledgement was authorized by an account not named in the connection ticket"
    );
    let acknowledgement_event = acknowledgement.event();
    ensure!(
        acknowledgement_event.conversation_id() == conversation_id,
        "acknowledgement belongs to a different conversation"
    );
    ensure!(
        acknowledgement_event.author_device_id() == expected_listener_device_id,
        "acknowledgement was signed by a device not named in the connection ticket"
    );
    let EventPayload::Acknowledgement {
        acknowledged_event_id,
    } = acknowledgement_event.payload()
    else {
        bail!("connector expected an acknowledgement event");
    };
    ensure!(
        *acknowledged_event_id == event_id,
        "acknowledgement references a different event"
    );
    ensure!(
        acknowledgement_event.parents() == [event_id],
        "acknowledgement does not causally reference the sent event"
    );
    let acknowledgement_store_outcome = run_state_transaction(&state_dir, |transaction| {
        let (outcome, receipt) = event_store
            .put_authorized_with_receipt(&acknowledgement, &membership)
            .context("persist verified acknowledgement")?;
        transaction.register_store_receipt(&receipt)?;
        Ok(outcome)
    })?;
    println!(
        "acknowledgement_event_id={}",
        acknowledgement_event.event_id()?
    );
    println!(
        "acknowledgement_author_account_id={}",
        acknowledgement.author_account_id()
    );
    println!(
        "acknowledgement_author_device_id={}",
        acknowledgement_event.author_device_id()
    );
    println!("acknowledgement_store={acknowledgement_store_outcome:?}");
    println!("status=acknowledged");

    print_transport_diagnostics(&connection, route_policy).await?;
    connection.close(0_u32.into(), b"kilogram m0 complete");
    endpoint.close().await;
    Ok(())
}

async fn sync(
    state_dir: PathBuf,
    ticket: Option<String>,
    ticket_file: Option<PathBuf>,
    conversation: String,
    max_rounds: usize,
    expected_listener_account_id: AccountId,
) -> Result<()> {
    ensure!(
        (1..=MAX_SYNC_ROUNDS).contains(&max_rounds),
        "--max-rounds must be between 1 and {MAX_SYNC_ROUNDS}"
    );
    let device_state = load_command_device_state(&state_dir)?;
    let trust = CommandTrustReadRepository::open(&state_dir, &device_state)?;
    let requester_certificate = trust
        .load_certificate()
        .context("load requester Account Root certificate")?;
    let requester_authority_snapshot = trust
        .load_own_authority_snapshot(&requester_certificate)
        .context("load requester Account Root authority snapshot")?;
    let immutable_reads = open_immutable_read_repositories(&state_dir)
        .context("capture immutable vault-primary sync state before local changes")?;
    let event_store = open_event_store(&state_dir)?;
    let local_message_store = open_local_message_store(&state_dir)?;
    let ticket = load_connection_ticket(ticket, ticket_file).await?;
    ticket.verify_listener_account(expected_listener_account_id)?;
    run_state_transaction(&state_dir, |transaction| {
        let ratchet_state = transaction.load_ratchet_state()?;
        ratchet_state
            .observe_prekey_directory(ticket.listener_directory(), unix_time_now()?)
            .context("observe listener prekey directory and reject rollback")
    })?;
    let listener_snapshot_store = pin_peer_authority_primary(
        &state_dir,
        &device_state,
        ticket.listener_authority_snapshot(),
    )
    .context("pin listener authority snapshot and reject rollback")?;
    let authorized_listener = ticket.verify_listener_authorization(expected_listener_account_id)?;
    let expected_listener_device_id = authorized_listener.device_id();
    let route_policy = ticket.route_policy();
    verify_device_authorization_with_snapshot(
        ticket.allowed_requester_account_id(),
        &requester_certificate,
        &requester_authority_snapshot,
        &DeviceCapability::MESSAGING,
    )
    .context("this device account is not authorized by the connection ticket")?;
    let session_binding =
        SyncSessionBinding::from_transport_label(&ticket.endpoint().id.to_string());
    let conversation_id = ConversationId::from_label(&conversation);
    let membership = trust
        .load_conversation_membership(conversation_id.scope_id())
        .context("load trusted conversation membership before synchronization")?;
    require_conversation_participants(
        &membership,
        requester_certificate.account_id(),
        authorized_listener.account_id(),
    )?;
    print_immutable_read_diagnostics("sync", &immutable_reads);
    let decrypting_store = DecryptingSessionStore::new(
        &state_dir,
        &event_store,
        &local_message_store,
        &device_state,
        requester_certificate.account_id(),
        immutable_reads,
    );
    let client = SyncClient::new(
        device_state.identity(),
        &decrypting_store,
        conversation_id,
        &membership,
        session_binding,
        expected_listener_device_id,
    );

    let endpoint = endpoint_builder_for_remote(route_policy, ticket.endpoint())?
        .bind()
        .await
        .context("bind syncing Iroh endpoint")?;
    println!("transport_endpoint_id={}", endpoint.id());
    println!("account_id={}", requester_certificate.account_id());
    println!("device_id={}", device_state.identity().device_id());
    println!("target_account_id={}", authorized_listener.account_id());
    println!(
        "target_authority_revision={}",
        ticket.listener_authority_snapshot().revision()
    );
    println!("target_authority_store={listener_snapshot_store:?}");
    println!("route_policy={}", route_policy.as_str());
    print_connection_target(&ticket);

    if route_policy == RoutePolicy::RelayOnly {
        wait_for_relay(&endpoint, route_policy, CLIENT_RELAY_WAIT_SECONDS).await?;
    }

    let connection = timeout(
        CONNECTION_TIMEOUT,
        endpoint.connect(ticket.endpoint().clone(), ALPN),
    )
    .await
    .with_context(|| timeout_message("connect to listening endpoint for sync", CONNECTION_TIMEOUT))?
    .context("connect to listening endpoint for sync")?;
    println!("peer_id={}", connection.remote_id());

    let ready_path = await_route_policy(&connection, route_policy, ROUTE_POLICY_WAIT)
        .await
        .context("wait for a sync path allowed by the connection ticket")?;
    print_ready_path(&ready_path);

    authorize_with_listener(
        &connection,
        device_state.identity(),
        requester_certificate,
        requester_authority_snapshot,
        session_binding,
    )
    .await?;

    let mut total_sent_events = 0;
    let mut total_received_events = 0;
    for round_number in 1..=MAX_SYNC_ROUNDS {
        let inventory_round = client.begin_round()?;
        let inventory_event_count = inventory_round.inventory_event_count();
        let (mut inventory_send, mut inventory_receive) =
            open_bi(&connection, "open sync inventory stream").await?;
        write_client_request(
            &mut inventory_send,
            &ClientRequest::SyncInventory(inventory_round.inventory().clone()),
        )
        .await?;
        let diff = match read_server_response(&mut inventory_receive).await? {
            ServerResponse::SyncDiff(diff) => diff,
            ServerResponse::SyncRejected(rejected) => {
                ensure!(
                    rejected.conversation_id() == conversation_id,
                    "sync rejection belongs to a different conversation"
                );
                bail!("sync rejected by listener: {:?}", rejected.reason());
            }
            _ => bail!("sync client expected a sync diff response"),
        };
        let batch_round = client.accept_diff(inventory_round, diff)?;

        let (mut batch_send, mut batch_receive) =
            open_bi(&connection, "open sync event batch stream").await?;
        write_client_request(
            &mut batch_send,
            &ClientRequest::SyncEvents(batch_round.batch().clone()),
        )
        .await?;
        let complete = match read_server_response(&mut batch_receive).await? {
            ServerResponse::SyncComplete(complete) => complete,
            _ => bail!("sync client expected a sync completion response"),
        };
        let stats = client.accept_complete(batch_round, complete)?;
        total_sent_events += stats.sent_events;
        total_received_events += stats.received_events;
        println!("sync_round_{round_number}_inventory_events={inventory_event_count}");
        println!(
            "sync_round_{round_number}_received_events={}",
            stats.received_events
        );
        println!(
            "sync_round_{round_number}_sent_events={}",
            stats.sent_events
        );

        if !stats.more_available {
            println!("sync_rounds_completed={round_number}");
            println!("sync_received_events={total_received_events}");
            println!("sync_sent_events={total_sent_events}");
            println!("sync_more_available=false");
            print_sync_overlay_diagnostics(&decrypting_store)?;
            println!("status=synchronized");

            print_transport_diagnostics(&connection, route_policy).await?;
            connection.close(0_u32.into(), b"kilogram m0 sync complete");
            endpoint.close().await;
            return Ok(());
        }

        if round_number == max_rounds {
            let (mut pause_send, mut pause_receive) =
                open_bi(&connection, "open sync pause stream").await?;
            write_client_request(
                &mut pause_send,
                &ClientRequest::SyncPause(SyncPause::new(conversation_id)),
            )
            .await?;
            let paused = match read_server_response(&mut pause_receive).await? {
                ServerResponse::SyncPaused(paused) => paused,
                _ => bail!("sync client expected a sync paused response"),
            };
            ensure!(
                paused.conversation_id() == conversation_id,
                "sync paused response belongs to a different conversation"
            );
            println!("sync_rounds_completed={round_number}");
            println!("sync_received_events={total_received_events}");
            println!("sync_sent_events={total_sent_events}");
            println!("sync_more_available=true");
            println!("sync_resume_checkpoint=event-store");
            print_sync_overlay_diagnostics(&decrypting_store)?;
            println!("status=paused");

            print_transport_diagnostics(&connection, route_policy).await?;
            connection.close(0_u32.into(), b"kilogram m0 sync paused");
            endpoint.close().await;
            return Ok(());
        }
    }
    bail!("sync exceeded the limit of {MAX_SYNC_ROUNDS} rounds")
}

async fn wait_for_relay(
    endpoint: &Endpoint,
    route_policy: RoutePolicy,
    relay_wait_seconds: u64,
) -> Result<()> {
    if relay_wait_seconds == 0 {
        ensure!(
            route_policy != RoutePolicy::RelayOnly,
            "relay-only requires --relay-wait-seconds greater than zero"
        );
        println!("relay_status=skipped");
        return Ok(());
    }

    match timeout(Duration::from_secs(relay_wait_seconds), endpoint.online()).await {
        Ok(()) => {
            println!("relay_status=online");
            let endpoint_addr = endpoint.addr();
            for relay_url in endpoint_addr.relay_urls() {
                println!("relay_home_url={relay_url}");
            }
        }
        Err(_) if route_policy == RoutePolicy::RelayOnly => bail!(
            "required relay did not become online within {relay_wait_seconds}s; route_policy=relay-only"
        ),
        Err(_) => eprintln!(
            "relay_status=timeout after {relay_wait_seconds}s (direct connections may still work)"
        ),
    }
    Ok(())
}

fn print_connection_target(ticket: &ConnectionTicket) {
    print_endpoint_target(ticket.endpoint());
}

fn print_endpoint_target(endpoint: &EndpointAddr) {
    println!("target_endpoint_id={}", endpoint.id);
    for relay_url in endpoint.relay_urls() {
        println!("target_relay_url={relay_url}");
    }
}

/// Waits for one valid QUIC connection while treating malformed or retransmitted
/// Initial datagrams as recoverable network input. Iroh explicitly documents that
/// `Incoming::accept` can fail for ordinary UDP traffic and retransmissions.
async fn accept_authenticated_connection(endpoint: &Endpoint) -> Result<Connection> {
    let mut ignored_attempts = 0_u64;
    loop {
        let incoming = endpoint
            .accept()
            .await
            .context("listener endpoint closed before receiving a connection")?;
        let accepting = match incoming.accept() {
            Ok(accepting) => accepting,
            Err(error) => {
                ignored_attempts = ignored_attempts.saturating_add(1);
                eprintln!(
                    "incoming_connection_ignored={ignored_attempts} stage=initial error={error:#}"
                );
                continue;
            }
        };

        match timeout(CONNECTION_TIMEOUT, accepting).await {
            Ok(Ok(connection)) => return Ok(connection),
            Ok(Err(error)) => {
                ignored_attempts = ignored_attempts.saturating_add(1);
                eprintln!(
                    "incoming_connection_ignored={ignored_attempts} stage=handshake error={error:#}"
                );
            }
            Err(_) => {
                ignored_attempts = ignored_attempts.saturating_add(1);
                eprintln!(
                    "incoming_connection_ignored={ignored_attempts} stage=handshake error={}",
                    timeout_message("incoming Iroh handshake", CONNECTION_TIMEOUT)
                );
            }
        }
    }
}

async fn open_bi(connection: &Connection, operation: &str) -> Result<(SendStream, RecvStream)> {
    timeout(STREAM_OPEN_TIMEOUT, connection.open_bi())
        .await
        .with_context(|| timeout_message(operation, STREAM_OPEN_TIMEOUT))?
        .with_context(|| operation.to_owned())
}

async fn accept_bi(connection: &Connection, operation: &str) -> Result<(SendStream, RecvStream)> {
    timeout(STREAM_OPEN_TIMEOUT, connection.accept_bi())
        .await
        .with_context(|| timeout_message(operation, STREAM_OPEN_TIMEOUT))?
        .with_context(|| operation.to_owned())
}

async fn accept_optional_history_rewrap_stream(
    connection: &Connection,
) -> Result<Option<(SendStream, RecvStream)>> {
    tokio::select! {
        _ = connection.closed() => Ok(None),
        result = timeout(HISTORY_RECOVERY_NEXT_PAGE_TIMEOUT, connection.accept_bi()) => {
            result
                .context("waiting for the next history-rewrap page request timed out")?
                .context("accept next history-rewrap page request")
                .map(Some)
        }
    }
}

fn print_ready_path(path: &SelectedPathDiagnostics) {
    println!("transport_ready_path={}", path.kind.as_str());
}

async fn print_transport_diagnostics(
    connection: &Connection,
    route_policy: RoutePolicy,
) -> Result<()> {
    let path = selected_path_diagnostics(connection, DIRECT_PATH_DIAGNOSTIC_WAIT)
        .await
        .context("selected transport path is unavailable")?;
    ensure!(
        route_policy.accepts(path.kind),
        "selected transport path {} violates route policy {}",
        path.kind.as_str(),
        route_policy.as_str()
    );
    println!("transport_path={}", path.kind.as_str());
    println!("transport_remote_address={}", path.remote_address);
    println!(
        "transport_rtt_ms={:.1}",
        path.round_trip_time.as_secs_f64() * 1_000.0
    );
    println!("transport_open_paths={}", path.open_paths);
    Ok(())
}

fn timeout_message(operation: &str, duration: Duration) -> String {
    format!("{operation} timed out after {:.1}s", duration.as_secs_f64())
}

async fn load_connection_ticket(
    ticket: Option<String>,
    ticket_file: Option<PathBuf>,
) -> Result<ConnectionTicket> {
    let encoded_ticket = match (ticket, ticket_file) {
        (Some(ticket), None) => ticket,
        (None, Some(path)) => tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("read ticket from {}", path.display()))?,
        (None, None) => bail!("provide either --ticket or --ticket-file"),
        (Some(_), Some(_)) => bail!("--ticket and --ticket-file are mutually exclusive"),
    };
    ConnectionTicket::decode(&encoded_ticket)
}

async fn load_history_recovery_link(
    link: Option<String>,
    link_file: Option<PathBuf>,
    qr_file: Option<PathBuf>,
) -> Result<LoadedHistoryRecoveryLink> {
    let (encoded, input) = match (link, link_file, qr_file) {
        (Some(link), None, None) => (link, HistoryRecoveryLinkInput::Uri),
        (None, Some(path), None) => {
            let bytes = tokio::fs::read(&path)
                .await
                .with_context(|| format!("read history recovery link from {}", path.display()))?;
            ensure!(
                bytes.len() <= MAX_HISTORY_RECOVERY_LINK_TEXT_BYTES,
                "history recovery link file exceeds the QR-ready size limit"
            );
            (
                String::from_utf8(bytes).context("history recovery link file is not UTF-8")?,
                HistoryRecoveryLinkInput::TextFile,
            )
        }
        (None, None, Some(path)) => {
            let report = decode_recovery_link_qr_image(&path)?;
            (
                report.payload.clone(),
                HistoryRecoveryLinkInput::QrImage(report),
            )
        }
        (None, None, None) => bail!("provide exactly one of --link, --link-file, or --qr-file"),
        _ => bail!("--link, --link-file, and --qr-file are mutually exclusive"),
    };
    let link = SignedHistoryRecoveryLink::decode_text(&encoded)?;
    Ok(LoadedHistoryRecoveryLink {
        link,
        encoded: encoded.trim().to_owned(),
        input,
    })
}

enum HistoryRecoveryLinkInput {
    Uri,
    TextFile,
    QrImage(RecoveryQrDecodeReport),
}

struct LoadedHistoryRecoveryLink {
    link: SignedHistoryRecoveryLink,
    encoded: String,
    input: HistoryRecoveryLinkInput,
}

async fn inspect_history_recovery_link(
    link: Option<String>,
    link_file: Option<PathBuf>,
    qr_file: Option<PathBuf>,
) -> Result<()> {
    let loaded = load_history_recovery_link(link, link_file, qr_file).await?;
    loaded.link.verify_at(unix_time_now()?)?;
    print_history_recovery_link_input(&loaded.input);
    print_history_recovery_link(&loaded.link, loaded.encoded.len())?;
    println!("connection_attempted=false");
    println!("status=history-recovery-link-verified");
    Ok(())
}

async fn render_history_recovery_link_qr(
    link: Option<String>,
    link_file: Option<PathBuf>,
    qr_file: PathBuf,
) -> Result<()> {
    let loaded = load_history_recovery_link(link, link_file, None).await?;
    loaded.link.verify_at(unix_time_now()?)?;
    let report = render_recovery_link_qr_png(&loaded.encoded, &qr_file)?;
    print_history_recovery_link_input(&loaded.input);
    print_history_recovery_link(&loaded.link, loaded.encoded.len())?;
    print_recovery_qr_render_report(&report, &qr_file);
    println!("connection_attempted=false");
    println!("status=history-recovery-qr-rendered");
    Ok(())
}

fn print_history_recovery_link_input(input: &HistoryRecoveryLinkInput) {
    match input {
        HistoryRecoveryLinkInput::Uri => println!("history_recovery_link_input=uri"),
        HistoryRecoveryLinkInput::TextFile => println!("history_recovery_link_input=text-file"),
        HistoryRecoveryLinkInput::QrImage(report) => {
            println!("history_recovery_link_input=qr-image");
            println!("history_recovery_qr_image_format={}", report.image_format);
            println!("history_recovery_qr_image_bytes={}", report.image_bytes);
            println!(
                "history_recovery_qr_image_dimensions={}x{}",
                report.image_width, report.image_height
            );
        }
    }
}

fn print_recovery_qr_render_report(report: &RecoveryQrRenderReport, path: &Path) {
    println!("history_recovery_qr_error_correction=L");
    println!("history_recovery_qr_version={}", report.qr_version);
    println!("history_recovery_qr_module_count={}", report.module_count);
    println!(
        "history_recovery_qr_image_dimensions={}x{}",
        report.pixel_width, report.pixel_height
    );
    println!("history_recovery_qr_png_bytes={}", report.png_bytes);
    println!("history_recovery_qr_file={}", path.display());
}

fn print_history_recovery_link(
    link: &SignedHistoryRecoveryLink,
    encoded_length: usize,
) -> Result<()> {
    println!("history_recovery_link_id={}", encode_hex(&link.link_id()?));
    println!("account_id={}", link.account_id());
    println!("source_device_id={}", link.source_device_id());
    println!("recipient_device_id={}", link.recipient_device_id());
    println!("conversation_id={}", link.conversation_id());
    println!(
        "authority_revision={}",
        link.account_device_list().revision()
    );
    println!("history_rewrap_sas={}", link.sas()?);
    println!(
        "history_recovery_approved_range={}-{}",
        link.approved_range_start(),
        link.approved_range_end()
    );
    println!("history_recovery_page_size={}", link.page_size());
    println!("route_policy={}", link.route_policy().as_str());
    println!(
        "history_recovery_link_issued_at={}",
        link.issued_at_unix_seconds()
    );
    println!(
        "history_recovery_link_expires_at={}",
        link.expires_at_unix_seconds()
    );
    println!("history_recovery_link_text_bytes={encoded_length}");
    println!("history_recovery_link_qr_ready=true");
    println!("history_recovery_link_requires_device_authentication=true");
    println!("history_recovery_link_requires_sas_confirmation=true");
    print_endpoint_target(link.endpoint());
    Ok(())
}

async fn discover_history_recovery_links(
    state_dir: PathBuf,
    conversation: String,
    expected_source: Option<DeviceId>,
    wait_seconds: u64,
    max_candidates: usize,
    output_link_file: Option<PathBuf>,
) -> Result<()> {
    ensure!(
        (1..=MAX_DISCOVERY_WAIT_SECONDS).contains(&wait_seconds),
        "--wait-seconds must be between 1 and {MAX_DISCOVERY_WAIT_SECONDS}"
    );
    ensure!(
        (1..=MAX_DISCOVERY_CANDIDATES).contains(&max_candidates),
        "--max-candidates must be between 1 and {MAX_DISCOVERY_CANDIDATES}"
    );

    let device_state = load_command_device_state(&state_dir)?;
    let trust = CommandTrustReadRepository::open(&state_dir, &device_state)?;
    let recipient_certificate = trust
        .load_certificate()
        .context("load discovery recipient Account Root certificate")?;
    let local_authority = trust
        .load_own_authority_snapshot(&recipient_certificate)
        .context("load discovery recipient authority snapshot")?;
    let conversation_id = ConversationId::from_label(&conversation);
    let membership = trust
        .load_conversation_membership(conversation_id.scope_id())
        .context("load discovery conversation membership")?;
    membership
        .require_member(recipient_certificate.account_id())
        .context("verify discovery recipient conversation membership")?;
    let now_unix_seconds = unix_time_now().context("read time for history recovery discovery")?;

    println!("history_recovery_discovery_scope=local-network");
    println!("history_recovery_discovery_opt_in=true");
    println!(
        "history_recovery_discovery_multicast_target={}",
        multicast_target()
    );
    println!(
        "history_recovery_discovery_loopback_target={}",
        loopback_target()
    );
    println!("history_recovery_discovery_wait_seconds={wait_seconds}");
    println!("history_recovery_discovery_max_candidates={max_candidates}");
    println!("recipient_device_id={}", recipient_certificate.device_id());
    println!("conversation_id={conversation_id}");
    if let Some(source) = expected_source {
        println!("expected_source_device_id={source}");
    }

    let scan = discover_recovery_links(
        Duration::from_secs(wait_seconds),
        max_candidates,
        |encoded| {
            let Ok(link) = SignedHistoryRecoveryLink::decode_text(encoded) else {
                return false;
            };
            if link
                .verify_for_recipient(now_unix_seconds, &recipient_certificate)
                .is_err()
                || link.conversation_id() != conversation_id
                || expected_source.is_some_and(|source| source != link.source_device_id())
            {
                return false;
            }
            let remote_authority = link.account_device_list().authority_snapshot();
            remote_authority.revision() > local_authority.revision()
                || (remote_authority.revision() == local_authority.revision()
                    && remote_authority == &local_authority)
        },
    )
    .await?;

    println!(
        "history_recovery_discovery_multicast_joined={}",
        scan.multicast_joined
    );
    println!(
        "history_recovery_discovery_datagrams_received={}",
        scan.datagrams_received
    );
    println!(
        "history_recovery_discovery_datagrams_rejected={}",
        scan.datagrams_rejected
    );
    println!(
        "history_recovery_discovery_duplicate_candidates={}",
        scan.duplicate_candidates
    );
    println!(
        "history_recovery_discovery_datagram_limit_reached={}",
        scan.datagram_limit_reached
    );
    println!(
        "history_recovery_discovery_candidate_limit_reached={}",
        scan.candidate_limit_reached
    );
    println!(
        "history_recovery_discovery_candidate_count={}",
        scan.candidates.len()
    );

    for (offset, encoded) in scan.candidates.iter().enumerate() {
        let candidate = SignedHistoryRecoveryLink::decode_text(encoded)
            .context("decode already verified discovery candidate")?;
        let index = offset + 1;
        println!(
            "history_recovery_candidate_{index}_link_id={}",
            encode_hex(&candidate.link_id()?)
        );
        println!(
            "history_recovery_candidate_{index}_account_id={}",
            candidate.account_id()
        );
        println!(
            "history_recovery_candidate_{index}_source_device_id={}",
            candidate.source_device_id()
        );
        println!(
            "history_recovery_candidate_{index}_recipient_device_id={}",
            candidate.recipient_device_id()
        );
        println!(
            "history_recovery_candidate_{index}_conversation_id={}",
            candidate.conversation_id()
        );
        println!(
            "history_recovery_candidate_{index}_authority_revision={}",
            candidate.account_device_list().revision()
        );
        println!(
            "history_recovery_candidate_{index}_sas={}",
            candidate.sas()?
        );
        println!(
            "history_recovery_candidate_{index}_endpoint_id={}",
            candidate.endpoint().id
        );
        println!(
            "history_recovery_candidate_{index}_route_policy={}",
            candidate.route_policy().as_str()
        );
        println!(
            "history_recovery_candidate_{index}_expires_at={}",
            candidate.expires_at_unix_seconds()
        );
        println!("history_recovery_candidate_{index}_link={encoded}");
    }

    println!("history_recovery_discovery_metadata_visible_to_lan=true");
    println!("history_recovery_discovery_user_consent=not-granted");
    println!("connection_attempted=false");

    ensure!(
        !scan.candidates.is_empty(),
        "no valid history recovery link was discovered for this device and conversation"
    );
    ensure!(
        scan.candidates.len() == 1 && !scan.candidate_limit_reached,
        "history recovery discovery is ambiguous; inspect and select one exact signed link"
    );
    if let Some(path) = output_link_file {
        write_new_authority_file(&path, scan.candidates[0].as_bytes())
            .with_context(|| format!("write discovered recovery link to {}", path.display()))?;
        println!("history_recovery_discovery_link_file={}", path.display());
    }
    println!("status=history-recovery-link-discovered");
    Ok(())
}

async fn load_history_recovery_plan(path: &Path) -> Result<SignedHistoryRecoveryPlan> {
    let metadata = tokio::fs::metadata(path)
        .await
        .with_context(|| format!("inspect history recovery plan {}", path.display()))?;
    ensure!(
        metadata.is_file(),
        "history recovery plan input is not a regular file"
    );
    ensure!(
        (1..=MAX_HISTORY_RECOVERY_PLAN_BYTES as u64).contains(&metadata.len()),
        "history recovery plan file must contain 1..={MAX_HISTORY_RECOVERY_PLAN_BYTES} bytes"
    );
    let bytes = tokio::fs::read(path)
        .await
        .with_context(|| format!("read history recovery plan {}", path.display()))?;
    SignedHistoryRecoveryPlan::decode(&bytes)
}

fn print_history_recovery_plan(plan: &SignedHistoryRecoveryPlan) -> Result<()> {
    let policy = plan.execution_policy();
    println!("history_recovery_plan_id={}", encode_hex(&plan.plan_id()?));
    println!("account_id={}", plan.account_id());
    println!("source_device_id={}", plan.source_device_id());
    println!("recipient_device_id={}", plan.recipient_device_id());
    println!("conversation_id={}", plan.conversation_id());
    println!(
        "authority_revision={}",
        plan.account_device_list().revision()
    );
    println!("history_rewrap_sas={}", plan.sas());
    println!(
        "history_recovery_approved_range={}-{}",
        plan.approved_range_start(),
        plan.approved_range_end()
    );
    println!("history_recovery_page_size={}", plan.page_size());
    println!("route_policy={}", plan.route_policy().as_str());
    println!(
        "history_recovery_plan_allow_ethernet={}",
        policy.allow_ethernet()
    );
    println!("history_recovery_plan_allow_wifi={}", policy.allow_wifi());
    println!(
        "history_recovery_plan_allow_mobile={}",
        policy.allow_mobile()
    );
    println!(
        "history_recovery_plan_allow_unknown_network={}",
        policy.allow_unknown_network()
    );
    println!(
        "history_recovery_plan_require_external_power={}",
        policy.require_external_power()
    );
    println!(
        "history_recovery_plan_approved_at={}",
        plan.approved_at_unix_seconds()
    );
    println!(
        "history_recovery_plan_expires_at={}",
        plan.expires_at_unix_seconds()
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn approve_history_recovery_plan(
    state_dir: PathBuf,
    link: Option<String>,
    link_file: Option<PathBuf>,
    qr_file: Option<PathBuf>,
    conversation: String,
    confirmed_sas: String,
    plan_file: PathBuf,
    deny_ethernet: bool,
    deny_wifi: bool,
    allow_mobile: bool,
    allow_unknown_network: bool,
    require_external_power: bool,
    valid_for_hours: u64,
) -> Result<()> {
    let loaded = load_history_recovery_link(link, link_file, qr_file).await?;
    let now_unix_seconds = unix_time_now().context("read time for recovery plan approval")?;
    let device_state = load_command_device_state(&state_dir)?;
    let trust = CommandTrustReadRepository::open(&state_dir, &device_state)?;
    let recipient_certificate = trust
        .load_certificate()
        .context("load recovery plan recipient certificate")?;
    let local_authority = trust
        .load_own_authority_snapshot(&recipient_certificate)
        .context("load recovery plan recipient authority snapshot")?;
    loaded
        .link
        .verify_for_recipient(now_unix_seconds, &recipient_certificate)?;
    let conversation_id = ConversationId::from_label(&conversation);
    ensure!(
        loaded.link.conversation_id() == conversation_id,
        "history recovery link is bound to a different conversation"
    );
    let membership = trust
        .load_conversation_membership(conversation_id.scope_id())
        .context("load recovery plan conversation membership")?;
    membership
        .require_member(recipient_certificate.account_id())
        .context("verify recovery plan recipient conversation membership")?;
    let link_authority = loaded.link.account_device_list().authority_snapshot();
    ensure!(
        link_authority.revision() > local_authority.revision()
            || (link_authority.revision() == local_authority.revision()
                && link_authority == &local_authority),
        "history recovery link authority is older than or conflicts with local authority"
    );
    let sas = loaded.link.sas()?;
    ensure!(
        confirmed_sas.trim() == sas.to_string(),
        "recipient confirmed SAS {}, but the signed history recovery link derives {sas}",
        confirmed_sas.trim()
    );
    let execution_policy = RecoveryExecutionPolicy::new(
        !deny_ethernet,
        !deny_wifi,
        allow_mobile,
        allow_unknown_network,
        require_external_power,
    )?;
    let plan = SignedHistoryRecoveryPlan::approve(
        device_state.identity(),
        HistoryRecoveryPlanOptions {
            link: loaded.link,
            execution_policy,
            approved_at_unix_seconds: now_unix_seconds,
            valid_for_hours,
        },
    )?;
    write_new_authority_file(&plan_file, &plan.encode()?)
        .with_context(|| format!("write history recovery plan to {}", plan_file.display()))?;

    print_history_recovery_link_input(&loaded.input);
    print_history_recovery_plan(&plan)?;
    println!("history_recovery_plan_file={}", plan_file.display());
    println!("history_recovery_plan_user_consent=approved");
    println!("connection_attempted=false");
    println!("status=history-recovery-plan-approved");
    Ok(())
}

fn verify_history_recovery_plan_locally(
    state_dir: &Path,
    conversation: &str,
    plan: &SignedHistoryRecoveryPlan,
    now_unix_seconds: u64,
) -> Result<(DeviceState, DeviceCertificate)> {
    plan.verify_at(now_unix_seconds)?;
    let device_state = load_command_device_state(state_dir)?;
    let trust = CommandTrustReadRepository::open(state_dir, &device_state)?;
    let recipient_certificate = trust
        .load_certificate()
        .context("load approved recovery plan recipient certificate")?;
    ensure!(
        recipient_certificate.device_id() == plan.recipient_device_id(),
        "history recovery plan was signed by a different recipient device"
    );
    ensure!(
        recipient_certificate.account_id() == plan.account_id(),
        "history recovery plan belongs to a different account"
    );
    ensure!(
        plan.account_device_list()
            .certificate_for(recipient_certificate.device_id())
            == Some(&recipient_certificate),
        "local recipient certificate is not present exactly in the recovery plan device list"
    );
    ensure!(
        ConversationId::from_label(conversation) == plan.conversation_id(),
        "history recovery plan is bound to a different conversation"
    );
    let local_authority = trust
        .load_own_authority_snapshot(&recipient_certificate)
        .context("load local authority for approved recovery plan")?;
    let plan_authority = plan.account_device_list().authority_snapshot();
    ensure!(
        plan_authority.revision() > local_authority.revision()
            || (plan_authority.revision() == local_authority.revision()
                && plan_authority == &local_authority),
        "history recovery plan authority is older than or conflicts with local authority"
    );
    let membership = trust
        .load_conversation_membership(plan.conversation_id().scope_id())
        .context("load approved recovery plan conversation membership")?;
    membership
        .require_member(plan.account_id())
        .context("verify approved recovery plan conversation membership")?;
    Ok((device_state, recipient_certificate))
}

fn latest_checkpoint_for_plan(
    state_dir: &Path,
    device_state: &DeviceState,
    plan: &SignedHistoryRecoveryPlan,
) -> Result<SignedHistoryRecoveryCheckpoint> {
    let approved_range_start = usize::try_from(plan.approved_range_start())
        .context("approved recovery plan range start cannot be represented")?;
    let approved_event_count = usize::try_from(plan.approved_event_count())
        .context("approved recovery plan event count cannot be represented")?;
    let page_size = usize::try_from(plan.page_size())
        .context("approved recovery plan page size cannot be represented")?;
    let initial = SignedHistoryRecoveryCheckpoint::start(
        device_state.identity(),
        plan.account_id(),
        plan.conversation_id(),
        plan.source_device_id(),
        plan.sas(),
        approved_range_start,
        approved_event_count,
        page_size,
    )?;
    load_latest_history_recovery_checkpoint(state_dir, initial)
}

#[allow(clippy::too_many_arguments)]
async fn run_approved_recovery_attempt(
    state_dir: &Path,
    conversation: &str,
    plan: &SignedHistoryRecoveryPlan,
    link: &SignedHistoryRecoveryLink,
    approved_range_start: usize,
    approved_event_count: usize,
    page_size: usize,
    max_pages: usize,
) -> Result<()> {
    let _state_lock = StateDirectoryLock::acquire(state_dir)
        .context("lock state directory for active recovery attempt")?;
    let vault_guard = VaultDualWriteGuard::prepare(state_dir)?;
    let preflight =
        verify_history_recovery_plan_locally(state_dir, conversation, plan, unix_time_now()?);
    let operation_result = match preflight {
        Ok(_) => {
            Box::pin(resume_history_recovery_inner(
                state_dir.to_path_buf(),
                None,
                None,
                Some(HistoryRecoveryBootstrap::from_link(link)),
                conversation.to_owned(),
                plan.source_device_id(),
                approved_range_start,
                approved_event_count,
                page_size,
                max_pages,
                plan.sas().to_string(),
                plan.account_id(),
            ))
            .await
        }
        Err(error) => Err(error),
    };
    let mirror_result = match vault_guard {
        Some(guard) => guard.finish(),
        None => Ok(()),
    };
    combine_operation_and_mirror(operation_result, mirror_result)
}

fn prepare_history_recovery_scheduler_state(
    state_dir: &Path,
    conversation: &str,
    plan: &SignedHistoryRecoveryPlan,
    now_unix_seconds: u64,
) -> Result<(SignedRecoverySchedulerState, bool)> {
    with_locked_state(state_dir, || {
        let (device_state, _) =
            verify_history_recovery_plan_locally(state_dir, conversation, plan, now_unix_seconds)?;
        let complete = latest_checkpoint_for_plan(state_dir, &device_state, plan)?.is_complete();
        let mut scheduler_state = load_or_initialize_recovery_scheduler_state(
            state_dir,
            device_state.identity(),
            plan,
            now_unix_seconds,
        )?;
        if complete && !scheduler_state.lifecycle().is_terminal() {
            scheduler_state =
                scheduler_state.complete(device_state.identity(), now_unix_seconds)?;
            persist_recovery_scheduler_state(state_dir, &scheduler_state)?;
        }
        ensure!(
            scheduler_state.lifecycle() != RecoverySchedulerLifecycle::Completed || complete,
            "scheduler claims completion but the signed recovery checkpoint is incomplete"
        );
        Ok((scheduler_state, complete))
    })
}

#[derive(Debug)]
struct RecoveryPolicyBlocked {
    stage: &'static str,
}

impl std::fmt::Display for RecoveryPolicyBlocked {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "current network/power context is blocked by the recipient-signed recovery plan at {}",
            self.stage
        )
    }
}

impl std::error::Error for RecoveryPolicyBlocked {}

fn recheck_recovery_execution_policy(
    plan: &SignedHistoryRecoveryPlan,
    platform_context: recovery_platform::RecoveryPlatformContext,
    stage: &'static str,
) -> Result<()> {
    println!("history_recovery_policy_recheck_stage={stage}");
    print_recovery_platform_context(&platform_context);
    let allowed = plan.execution_policy().allows(
        platform_context.network_class(),
        platform_context.power_source(),
    );
    println!("history_recovery_policy_recheck_allowed={allowed}");
    if allowed {
        Ok(())
    } else {
        Err(RecoveryPolicyBlocked { stage }.into())
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_history_recovery_plan(
    state_dir: PathBuf,
    plan_file: PathBuf,
    conversation: String,
    network_class: Option<RecoveryNetworkClass>,
    power_source: Option<RecoveryPowerSource>,
    max_attempts: usize,
    discovery_wait_seconds: u64,
    retry_base_seconds: u64,
    retry_max_seconds: u64,
    max_pages: usize,
) -> Result<()> {
    ensure!(
        (1..=MAX_HISTORY_RECOVERY_SCHEDULER_ATTEMPTS).contains(&max_attempts),
        "--max-attempts must be between 1 and {MAX_HISTORY_RECOVERY_SCHEDULER_ATTEMPTS}"
    );
    ensure!(
        (1..=MAX_DISCOVERY_WAIT_SECONDS).contains(&discovery_wait_seconds),
        "--discovery-wait-seconds must be between 1 and {MAX_DISCOVERY_WAIT_SECONDS}"
    );
    ensure!(
        (1..=MAX_HISTORY_RECOVERY_PAGES_PER_SESSION).contains(&max_pages),
        "--max-pages must be between 1 and {MAX_HISTORY_RECOVERY_PAGES_PER_SESSION}"
    );
    let backoff = RecoveryBackoffConfig::new(retry_base_seconds, retry_max_seconds)?;
    let platform_context = resolve_recovery_platform_context(network_class, power_source)?;
    let network_class = platform_context.network_class();
    let power_source = platform_context.power_source();
    let plan = load_history_recovery_plan(&plan_file).await?;
    let now_unix_seconds = unix_time_now().context("read time for recovery scheduler")?;
    let (mut scheduler_state, initially_complete) = prepare_history_recovery_scheduler_state(
        &state_dir,
        &conversation,
        &plan,
        now_unix_seconds,
    )?;
    print_history_recovery_plan(&plan)?;
    println!("history_recovery_plan_file={}", plan_file.display());
    println!("history_recovery_plan_user_consent=previously-approved");
    println!("history_recovery_scheduler_state_mode=signed-append-only-v1");
    println!("history_recovery_scheduler_restart_resume=true");
    println!("history_recovery_scheduler_backoff=exponential-equal-jitter");
    println!("history_recovery_scheduler_retry_base_seconds={retry_base_seconds}");
    println!("history_recovery_scheduler_retry_max_seconds={retry_max_seconds}");
    print_recovery_scheduler_state(&scheduler_state)?;
    print_recovery_platform_context(&platform_context);
    let policy_allowed = plan.execution_policy().allows(network_class, power_source);
    println!("history_recovery_execution_policy_allowed={policy_allowed}");

    if initially_complete || scheduler_state.lifecycle() == RecoverySchedulerLifecycle::Completed {
        println!("history_recovery_scheduler_discovery_attempted=false");
        println!("connection_attempted=false");
        println!("history_recovery_complete=true");
        println!("status=history-recovery-scheduler-complete");
        return Ok(());
    }
    if scheduler_state.lifecycle() == RecoverySchedulerLifecycle::Cancelled {
        println!("history_recovery_scheduler_discovery_attempted=false");
        println!("connection_attempted=false");
        println!("status=history-recovery-scheduler-cancelled");
        return Ok(());
    }
    if !policy_allowed {
        println!("history_recovery_scheduler_discovery_attempted=false");
        println!("connection_attempted=false");
        println!("status=history-recovery-scheduler-policy-blocked");
        return Err(RecoveryPolicyBlocked { stage: "initial" }.into());
    }

    match scheduler_state.readiness(now_unix_seconds) {
        RecoverySchedulerReadiness::Cancelled => {
            println!("history_recovery_scheduler_discovery_attempted=false");
            println!("connection_attempted=false");
            println!("status=history-recovery-scheduler-cancelled");
            return Ok(());
        }
        RecoverySchedulerReadiness::Completed => {
            bail!("scheduler is complete but the signed recovery checkpoint is incomplete")
        }
        RecoverySchedulerReadiness::ClockRollback {
            last_observed_unix_seconds,
        } => {
            println!("history_recovery_scheduler_discovery_attempted=false");
            println!("connection_attempted=false");
            println!("history_recovery_scheduler_clock_rollback_detected=true");
            println!(
                "history_recovery_scheduler_last_observed_unix_seconds={last_observed_unix_seconds}"
            );
            println!("status=history-recovery-scheduler-clock-blocked");
            bail!("wall clock is older than the signed scheduler high-water mark")
        }
        RecoverySchedulerReadiness::AttemptLeaseExpired => {
            scheduler_state =
                finalize_expired_recovery_attempt(&state_dir, &plan, now_unix_seconds, backoff)?;
            println!("history_recovery_scheduler_stale_attempt_recovered=true");
            print_recovery_scheduler_state(&scheduler_state)?;
            println!("history_recovery_scheduler_discovery_attempted=false");
            println!("connection_attempted=false");
            println!("status=history-recovery-scheduler-deferred");
            return Ok(());
        }
        RecoverySchedulerReadiness::Deferred {
            not_before_unix_seconds,
        } => {
            println!(
                "history_recovery_scheduler_not_before_unix_seconds={not_before_unix_seconds}"
            );
            println!("history_recovery_scheduler_discovery_attempted=false");
            println!("connection_attempted=false");
            println!("status=history-recovery-scheduler-deferred");
            return Ok(());
        }
        RecoverySchedulerReadiness::Ready => {}
    }

    let approved_range_start = usize::try_from(plan.approved_range_start())
        .context("approved recovery plan range start cannot be represented")?;
    let approved_event_count = usize::try_from(plan.approved_event_count())
        .context("approved recovery plan event count cannot be represented")?;
    let page_size = usize::try_from(plan.page_size())
        .context("approved recovery plan page size cannot be represented")?;
    let attempt_lease_seconds =
        history_recovery_attempt_lease_seconds(discovery_wait_seconds, max_pages)?;

    for local_attempt in 1..=max_attempts {
        let attempt_now = unix_time_now()?;
        plan.verify_at(attempt_now)?;
        recheck_recovery_execution_policy(&plan, platform_context.refreshed(), "before-discovery")?;
        let attempt_gate = with_locked_state(&state_dir, || {
            let (device_state, recipient_certificate) = verify_history_recovery_plan_locally(
                &state_dir,
                &conversation,
                &plan,
                attempt_now,
            )?;
            let current = load_required_recovery_scheduler_state(&state_dir, &plan)?;
            match current.readiness(attempt_now) {
                RecoverySchedulerReadiness::Ready => {
                    let started = current.start_attempt(
                        device_state.identity(),
                        attempt_now,
                        attempt_lease_seconds,
                    )?;
                    persist_recovery_scheduler_state(&state_dir, &started)?;
                    Ok(RecoveryAttemptGate::Started {
                        state: Box::new(started),
                        recipient_certificate,
                    })
                }
                RecoverySchedulerReadiness::Cancelled => Ok(RecoveryAttemptGate::Cancelled),
                RecoverySchedulerReadiness::Completed => Ok(RecoveryAttemptGate::Completed),
                RecoverySchedulerReadiness::Deferred {
                    not_before_unix_seconds,
                } => Ok(RecoveryAttemptGate::Deferred {
                    not_before_unix_seconds,
                }),
                RecoverySchedulerReadiness::ClockRollback {
                    last_observed_unix_seconds,
                } => Ok(RecoveryAttemptGate::ClockRollback {
                    last_observed_unix_seconds,
                }),
                RecoverySchedulerReadiness::AttemptLeaseExpired => {
                    bail!("a previous recovery attempt lease expired without reconciliation")
                }
            }
        })?;
        let (started_state, recipient_certificate) = match attempt_gate {
            RecoveryAttemptGate::Started {
                state,
                recipient_certificate,
            } => (*state, recipient_certificate),
            RecoveryAttemptGate::Cancelled => {
                println!("history_recovery_scheduler_discovery_attempted=false");
                println!("connection_attempted=false");
                println!("status=history-recovery-scheduler-cancelled");
                return Ok(());
            }
            RecoveryAttemptGate::Completed => {
                println!("history_recovery_scheduler_discovery_attempted=false");
                println!("connection_attempted=false");
                println!("status=history-recovery-scheduler-complete");
                return Ok(());
            }
            RecoveryAttemptGate::Deferred {
                not_before_unix_seconds,
            } => {
                println!(
                    "history_recovery_scheduler_not_before_unix_seconds={not_before_unix_seconds}"
                );
                println!("history_recovery_scheduler_discovery_attempted=false");
                println!("connection_attempted=false");
                println!("status=history-recovery-scheduler-deferred");
                return Ok(());
            }
            RecoveryAttemptGate::ClockRollback {
                last_observed_unix_seconds,
            } => {
                println!("history_recovery_scheduler_clock_rollback_detected=true");
                println!(
                    "history_recovery_scheduler_last_observed_unix_seconds={last_observed_unix_seconds}"
                );
                bail!("wall clock is older than the signed scheduler high-water mark")
            }
        };
        let persistent_attempt = started_state.total_attempts();
        println!("history_recovery_scheduler_local_attempt={local_attempt}");
        println!("history_recovery_scheduler_attempt={persistent_attempt}");
        println!("history_recovery_scheduler_attempt_lease_seconds={attempt_lease_seconds}");
        println!("history_recovery_scheduler_discovery_attempted=true");
        let scan_time = unix_time_now()?;
        let scan_result = discover_recovery_links(
            Duration::from_secs(discovery_wait_seconds),
            DEFAULT_DISCOVERY_CANDIDATES,
            |encoded| {
                let Ok(link) = SignedHistoryRecoveryLink::decode_text(encoded) else {
                    return false;
                };
                link.verify_for_recipient(scan_time, &recipient_certificate)
                    .is_ok()
                    && plan.matches_link(&link).unwrap_or(false)
            },
        )
        .await;
        let scan = match scan_result {
            Ok(scan) => scan,
            Err(error) => {
                let message = error.to_string().replace(['\r', '\n'], " ");
                println!("history_recovery_scheduler_attempt_{persistent_attempt}_result=failed");
                println!("history_recovery_scheduler_attempt_{persistent_attempt}_error={message}");
                scheduler_state = finalize_recovery_scheduler_attempt(
                    &state_dir,
                    &plan,
                    &started_state,
                    SchedulerAttemptResult::Failed,
                    unix_time_now()?,
                    backoff,
                )?;
                if scheduler_state.lifecycle() == RecoverySchedulerLifecycle::Cancelled {
                    println!("status=history-recovery-scheduler-cancelled");
                    return Ok(());
                }
                print_recovery_scheduler_state(&scheduler_state)?;
                if local_attempt < max_attempts {
                    wait_for_recovery_scheduler_deadline(&state_dir, &plan).await?;
                    continue;
                }
                println!("status=history-recovery-scheduler-scheduled");
                return Ok(());
            }
        };
        println!(
            "history_recovery_scheduler_attempt_{persistent_attempt}_datagrams_received={}",
            scan.datagrams_received
        );
        println!(
            "history_recovery_scheduler_attempt_{persistent_attempt}_candidates={}",
            scan.candidates.len()
        );
        println!(
            "history_recovery_scheduler_attempt_{persistent_attempt}_candidate_limit_reached={}",
            scan.candidate_limit_reached
        );

        let attempt_result = if scan.candidates.len() == 1 && !scan.candidate_limit_reached {
            let link = SignedHistoryRecoveryLink::decode_text(&scan.candidates[0])
                .context("decode scheduler discovery candidate")?;
            println!(
                "history_recovery_scheduler_attempt_{persistent_attempt}_target_endpoint_id={}",
                link.endpoint().id
            );
            if !recovery_scheduler_attempt_is_current(&state_dir, &plan, &started_state)? {
                println!("connection_attempted=false");
                println!("status=history-recovery-scheduler-cancelled");
                return Ok(());
            }
            let connect_policy = recheck_recovery_execution_policy(
                &plan,
                platform_context.refreshed(),
                "before-connect",
            );
            match connect_policy {
                Err(error) => {
                    println!("connection_attempted=false");
                    println!(
                        "history_recovery_scheduler_attempt_{persistent_attempt}_result=policy-blocked"
                    );
                    println!(
                        "history_recovery_scheduler_attempt_{persistent_attempt}_error={}",
                        error.to_string().replace(['\r', '\n'], " ")
                    );
                    SchedulerAttemptResult::Failed
                }
                Ok(()) => {
                    println!("connection_attempted=true");
                    let transfer_result = run_approved_recovery_attempt(
                        &state_dir,
                        &conversation,
                        &plan,
                        &link,
                        approved_range_start,
                        approved_event_count,
                        page_size,
                        max_pages,
                    )
                    .await;
                    match transfer_result {
                        Ok(()) => {
                            let complete = with_locked_state(&state_dir, || {
                                let device_state =
                                    load_scheduler_signing_device(&state_dir, &plan)?;
                                Ok(
                                    latest_checkpoint_for_plan(&state_dir, &device_state, &plan)?
                                        .is_complete(),
                                )
                            })?;
                            if complete {
                                println!(
                                    "history_recovery_scheduler_attempt_{persistent_attempt}_result=completed"
                                );
                                SchedulerAttemptResult::Completed
                            } else {
                                println!(
                                    "history_recovery_scheduler_attempt_{persistent_attempt}_result=paused"
                                );
                                SchedulerAttemptResult::Progressed
                            }
                        }
                        Err(error) => {
                            let message = error.to_string().replace(['\r', '\n'], " ");
                            println!(
                                "history_recovery_scheduler_attempt_{persistent_attempt}_result=failed"
                            );
                            println!(
                                "history_recovery_scheduler_attempt_{persistent_attempt}_error={message}"
                            );
                            SchedulerAttemptResult::Failed
                        }
                    }
                }
            }
        } else if scan.candidates.is_empty() {
            println!("history_recovery_scheduler_attempt_{persistent_attempt}_result=no-candidate");
            println!("connection_attempted=false");
            SchedulerAttemptResult::Failed
        } else {
            println!("history_recovery_scheduler_attempt_{persistent_attempt}_result=ambiguous");
            println!("connection_attempted=false");
            SchedulerAttemptResult::Failed
        };

        scheduler_state = finalize_recovery_scheduler_attempt(
            &state_dir,
            &plan,
            &started_state,
            attempt_result,
            unix_time_now()?,
            backoff,
        )?;
        print_recovery_scheduler_state(&scheduler_state)?;
        match scheduler_state.lifecycle() {
            RecoverySchedulerLifecycle::Completed => {
                println!("status=history-recovery-scheduler-complete");
                return Ok(());
            }
            RecoverySchedulerLifecycle::Cancelled => {
                println!("status=history-recovery-scheduler-cancelled");
                return Ok(());
            }
            RecoverySchedulerLifecycle::Active if local_attempt < max_attempts => {
                wait_for_recovery_scheduler_deadline(&state_dir, &plan).await?;
            }
            RecoverySchedulerLifecycle::Active => {
                println!("status=history-recovery-scheduler-scheduled");
                return Ok(());
            }
            RecoverySchedulerLifecycle::Attempting => {
                bail!("history recovery attempt remained in progress after finalization")
            }
        }
    }

    bail!("history recovery scheduler reached an unreachable local attempt state")
}

#[derive(Debug)]
struct HistoryRecoveryWorkerOptions {
    state_dir: PathBuf,
    plan_file: PathBuf,
    conversation: String,
    max_runtime_seconds: u64,
    max_wakeups: usize,
    cancel_poll_seconds: u64,
    discovery_wait_seconds: u64,
    retry_base_seconds: u64,
    retry_max_seconds: u64,
    max_pages: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HistoryRecoveryWorkerWakeReason {
    Initial,
    SchedulerDeadline,
    PlatformChange,
    SchedulerStateChange,
}

impl HistoryRecoveryWorkerWakeReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Initial => "initial",
            Self::SchedulerDeadline => "scheduler-deadline",
            Self::PlatformChange => "platform-change",
            Self::SchedulerStateChange => "scheduler-state-change",
        }
    }
}

fn recovery_worker_state_lock_is_busy(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<StateError>(),
            Some(StateError::AlreadyLocked { .. })
        )
    })
}

async fn retry_recovery_worker_state_operation<T>(
    mut operation: impl FnMut() -> Result<T>,
) -> Result<T> {
    let started = std::time::Instant::now();
    loop {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) if recovery_worker_state_lock_is_busy(&error) => {
                if started.elapsed() >= HISTORY_RECOVERY_WORKER_LOCK_WAIT {
                    return Err(error).context("wait for a concurrent bounded state operation");
                }
                tokio::time::sleep(HISTORY_RECOVERY_WORKER_LOCK_RETRY).await;
            }
            Err(error) => return Err(error),
        }
    }
}

fn observe_terminal_recovery_scheduler_state(
    state_dir: &Path,
    conversation: &str,
    plan: &SignedHistoryRecoveryPlan,
) -> Result<Option<SignedRecoverySchedulerState>> {
    plan.verify()?;
    ensure!(
        ConversationId::from_label(conversation) == plan.conversation_id(),
        "history recovery plan is bound to a different conversation"
    );
    with_state_lock_only(state_dir, || {
        let Some(state) =
            load_recovery_scheduler_state(state_dir, plan.plan_id()?, plan.recipient_device_id())?
        else {
            return Ok(None);
        };
        if !state.lifecycle().is_terminal() {
            return Ok(None);
        }
        load_scheduler_signing_device(state_dir, plan)?;
        Ok(Some(state))
    })
}

fn print_history_recovery_worker_terminal(state: &SignedRecoverySchedulerState) -> Result<()> {
    print_recovery_scheduler_state(state)?;
    println!("history_recovery_scheduler_discovery_attempted=false");
    println!("connection_attempted=false");
    match state.lifecycle() {
        RecoverySchedulerLifecycle::Cancelled => {
            println!("status=history-recovery-worker-cancelled")
        }
        RecoverySchedulerLifecycle::Completed => {
            println!("history_recovery_complete=true");
            println!("status=history-recovery-worker-complete");
        }
        RecoverySchedulerLifecycle::Active | RecoverySchedulerLifecycle::Attempting => {
            bail!("non-terminal scheduler state passed to worker terminal printer")
        }
    }
    Ok(())
}

fn finalize_interrupted_recovery_worker_attempt(
    state_dir: &Path,
    plan: &SignedHistoryRecoveryPlan,
    now_unix_seconds: u64,
    backoff: RecoveryBackoffConfig,
) -> Result<SignedRecoverySchedulerState> {
    with_locked_state(state_dir, || {
        let device_state = load_scheduler_signing_device(state_dir, plan)?;
        let current = load_required_recovery_scheduler_state(state_dir, plan)?;
        if current.lifecycle() != RecoverySchedulerLifecycle::Attempting {
            return Ok(current);
        }
        let next = current.record_failure(device_state.identity(), now_unix_seconds, backoff)?;
        persist_recovery_scheduler_state(state_dir, &next)?;
        Ok(next)
    })
}

fn recovery_worker_wait_duration(
    cancel_poll: Duration,
    runtime_remaining: Duration,
    scheduler_deadline: Option<(u64, u64)>,
) -> Duration {
    let mut duration = cancel_poll.min(runtime_remaining);
    if let Some((now_unix_seconds, deadline_unix_seconds)) = scheduler_deadline {
        let until_deadline = Duration::from_secs(
            deadline_unix_seconds
                .saturating_sub(now_unix_seconds)
                .max(1),
        );
        duration = duration.min(until_deadline);
    }
    duration
}

async fn watch_history_recovery_plan(options: HistoryRecoveryWorkerOptions) -> Result<()> {
    ensure!(
        (1..=MAX_HISTORY_RECOVERY_WORKER_WAKEUPS).contains(&options.max_wakeups),
        "--max-wakeups must be between 1 and {MAX_HISTORY_RECOVERY_WORKER_WAKEUPS}"
    );
    ensure!(
        (1..=MAX_HISTORY_RECOVERY_WORKER_RUNTIME_SECONDS).contains(&options.max_runtime_seconds),
        "--max-runtime-seconds must be between 1 and {MAX_HISTORY_RECOVERY_WORKER_RUNTIME_SECONDS}"
    );
    ensure!(
        (1..=MAX_HISTORY_RECOVERY_WORKER_CANCEL_POLL_SECONDS)
            .contains(&options.cancel_poll_seconds),
        "--cancel-poll-seconds must be between 1 and {MAX_HISTORY_RECOVERY_WORKER_CANCEL_POLL_SECONDS}"
    );
    ensure!(
        (1..=MAX_DISCOVERY_WAIT_SECONDS).contains(&options.discovery_wait_seconds),
        "--discovery-wait-seconds must be between 1 and {MAX_DISCOVERY_WAIT_SECONDS}"
    );
    ensure!(
        (1..=MAX_HISTORY_RECOVERY_PAGES_PER_SESSION).contains(&options.max_pages),
        "--max-pages must be between 1 and {MAX_HISTORY_RECOVERY_PAGES_PER_SESSION}"
    );
    let backoff =
        RecoveryBackoffConfig::new(options.retry_base_seconds, options.retry_max_seconds)?;
    let plan = load_history_recovery_plan(&options.plan_file).await?;
    plan.verify()?;
    ensure!(
        ConversationId::from_label(&options.conversation) == plan.conversation_id(),
        "history recovery plan is bound to a different conversation"
    );
    let platform_changes = subscribe_recovery_platform_changes()
        .context("subscribe to native recovery platform changes")?;
    let started_at = std::time::Instant::now();
    let runtime = Duration::from_secs(options.max_runtime_seconds);
    let cancel_poll = Duration::from_secs(options.cancel_poll_seconds);
    let mut wake_count = 0_usize;
    let mut wake_reason = HistoryRecoveryWorkerWakeReason::Initial;

    println!("history_recovery_plan_file={}", options.plan_file.display());
    println!("history_recovery_worker_mode=bounded-process-v1");
    println!(
        "history_recovery_worker_native_events={}",
        platform_changes.is_native()
    );
    println!(
        "history_recovery_worker_max_runtime_seconds={}",
        options.max_runtime_seconds
    );
    println!(
        "history_recovery_worker_max_wakeups={}",
        options.max_wakeups
    );
    println!(
        "history_recovery_worker_cancel_poll_seconds={}",
        options.cancel_poll_seconds
    );

    loop {
        if let Some(terminal) = retry_recovery_worker_state_operation(|| {
            observe_terminal_recovery_scheduler_state(
                &options.state_dir,
                &options.conversation,
                &plan,
            )
        })
        .await?
        {
            return print_history_recovery_worker_terminal(&terminal);
        }
        if started_at.elapsed() >= runtime {
            println!("status=history-recovery-worker-runtime-expired");
            return Ok(());
        }
        if wake_count >= options.max_wakeups {
            println!("status=history-recovery-worker-wakeup-limit");
            return Ok(());
        }
        wake_count += 1;
        println!("history_recovery_worker_wake={wake_count}");
        println!(
            "history_recovery_worker_wake_reason={}",
            wake_reason.as_str()
        );

        let now = unix_time_now().context("read time for recovery worker")?;
        let (mut scheduler_state, complete) = retry_recovery_worker_state_operation(|| {
            prepare_history_recovery_scheduler_state(
                &options.state_dir,
                &options.conversation,
                &plan,
                now,
            )
        })
        .await?;
        if complete || scheduler_state.lifecycle().is_terminal() {
            return print_history_recovery_worker_terminal(&scheduler_state);
        }
        if scheduler_state.readiness(now) == RecoverySchedulerReadiness::AttemptLeaseExpired {
            scheduler_state = retry_recovery_worker_state_operation(|| {
                finalize_expired_recovery_attempt(&options.state_dir, &plan, now, backoff)
            })
            .await?;
            println!("history_recovery_scheduler_stale_attempt_recovered=true");
            print_recovery_scheduler_state(&scheduler_state)?;
        }

        let platform_context = system_recovery_platform_context();
        print_recovery_platform_context(&platform_context);
        let policy_allowed = plan.execution_policy().allows(
            platform_context.network_class(),
            platform_context.power_source(),
        );
        println!("history_recovery_execution_policy_allowed={policy_allowed}");

        match scheduler_state.readiness(now) {
            RecoverySchedulerReadiness::Ready if policy_allowed => {
                let attempt = Box::pin(run_history_recovery_plan(
                    options.state_dir.clone(),
                    options.plan_file.clone(),
                    options.conversation.clone(),
                    None,
                    None,
                    1,
                    options.discovery_wait_seconds,
                    options.retry_base_seconds,
                    options.retry_max_seconds,
                    options.max_pages,
                ));
                let runtime_remaining = runtime.saturating_sub(started_at.elapsed());
                match timeout(runtime_remaining, attempt).await {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        if error.downcast_ref::<RecoveryPolicyBlocked>().is_some() {
                            println!("history_recovery_worker_policy_race_blocked=true");
                        } else if recovery_worker_state_lock_is_busy(&error) {
                            println!("history_recovery_worker_state_lock_race=true");
                        } else {
                            return Err(error);
                        }
                    }
                    Err(_) => {
                        println!("history_recovery_worker_attempt_interrupted_by_runtime=true");
                        let interrupted_at = unix_time_now()
                            .context("read time while stopping bounded recovery attempt")?;
                        let state = retry_recovery_worker_state_operation(|| {
                            finalize_interrupted_recovery_worker_attempt(
                                &options.state_dir,
                                &plan,
                                interrupted_at,
                                backoff,
                            )
                        })
                        .await?;
                        if state.lifecycle().is_terminal() {
                            return print_history_recovery_worker_terminal(&state);
                        }
                        print_recovery_scheduler_state(&state)?;
                        println!("status=history-recovery-worker-runtime-expired");
                        return Ok(());
                    }
                }
            }
            RecoverySchedulerReadiness::ClockRollback {
                last_observed_unix_seconds,
            } => {
                bail!(
                    "wall clock is older than signed scheduler high-water mark {last_observed_unix_seconds}"
                )
            }
            RecoverySchedulerReadiness::AttemptLeaseExpired => {
                bail!("expired recovery attempt lease remained after worker reconciliation")
            }
            RecoverySchedulerReadiness::Cancelled | RecoverySchedulerReadiness::Completed => {
                return print_history_recovery_worker_terminal(&scheduler_state);
            }
            RecoverySchedulerReadiness::Deferred { .. } | RecoverySchedulerReadiness::Ready => {}
        }

        if let Some(terminal) = retry_recovery_worker_state_operation(|| {
            observe_terminal_recovery_scheduler_state(
                &options.state_dir,
                &options.conversation,
                &plan,
            )
        })
        .await?
        {
            return print_history_recovery_worker_terminal(&terminal);
        }

        let observed_state = retry_recovery_worker_state_operation(|| {
            with_state_lock_only(&options.state_dir, || {
                load_required_recovery_scheduler_state(&options.state_dir, &plan)
            })
        })
        .await?;
        let mut observed_state_id = observed_state.state_id()?;
        let observed_platform_sequence = platform_changes.sequence();
        println!("history_recovery_worker_waiting=true");

        wake_reason = loop {
            if let Some(terminal) = retry_recovery_worker_state_operation(|| {
                observe_terminal_recovery_scheduler_state(
                    &options.state_dir,
                    &options.conversation,
                    &plan,
                )
            })
            .await?
            {
                return print_history_recovery_worker_terminal(&terminal);
            }
            if started_at.elapsed() >= runtime {
                println!("status=history-recovery-worker-runtime-expired");
                return Ok(());
            }

            let wait_now = unix_time_now().context("read time while recovery worker waits")?;
            plan.verify_at(wait_now)?;
            let current = retry_recovery_worker_state_operation(|| {
                with_state_lock_only(&options.state_dir, || {
                    load_required_recovery_scheduler_state(&options.state_dir, &plan)
                })
            })
            .await?;
            let current_state_id = current.state_id()?;
            if current_state_id != observed_state_id {
                break HistoryRecoveryWorkerWakeReason::SchedulerStateChange;
            }

            let readiness = current.readiness(wait_now);
            match readiness {
                RecoverySchedulerReadiness::ClockRollback {
                    last_observed_unix_seconds,
                } => {
                    bail!(
                        "wall clock is older than signed scheduler high-water mark {last_observed_unix_seconds}"
                    )
                }
                RecoverySchedulerReadiness::AttemptLeaseExpired => {
                    break HistoryRecoveryWorkerWakeReason::SchedulerDeadline;
                }
                RecoverySchedulerReadiness::Ready => {
                    let context = system_recovery_platform_context();
                    if plan
                        .execution_policy()
                        .allows(context.network_class(), context.power_source())
                    {
                        break HistoryRecoveryWorkerWakeReason::SchedulerDeadline;
                    }
                }
                RecoverySchedulerReadiness::Cancelled | RecoverySchedulerReadiness::Completed => {
                    return print_history_recovery_worker_terminal(&current);
                }
                RecoverySchedulerReadiness::Deferred { .. } => {}
            }

            let runtime_remaining = runtime.saturating_sub(started_at.elapsed());
            let scheduler_deadline = match readiness {
                RecoverySchedulerReadiness::Deferred {
                    not_before_unix_seconds,
                } => Some((wait_now, not_before_unix_seconds)),
                _ => None,
            };
            let wait_duration =
                recovery_worker_wait_duration(cancel_poll, runtime_remaining, scheduler_deadline);
            match platform_changes
                .wait(observed_platform_sequence, wait_duration)
                .await
            {
                RecoveryPlatformChangeWait::Changed => {
                    break HistoryRecoveryWorkerWakeReason::PlatformChange;
                }
                RecoveryPlatformChangeWait::TimedOut => {
                    observed_state_id = current_state_id;
                }
            }
        };
    }
}

#[derive(Debug)]
enum RecoveryAttemptGate {
    Started {
        state: Box<SignedRecoverySchedulerState>,
        recipient_certificate: DeviceCertificate,
    },
    Deferred {
        not_before_unix_seconds: u64,
    },
    ClockRollback {
        last_observed_unix_seconds: u64,
    },
    Cancelled,
    Completed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SchedulerAttemptResult {
    Failed,
    Progressed,
    Completed,
}

fn load_or_initialize_recovery_scheduler_state(
    state_dir: &Path,
    identity: &DeviceIdentity,
    plan: &SignedHistoryRecoveryPlan,
    now_unix_seconds: u64,
) -> Result<SignedRecoverySchedulerState> {
    let plan_id = plan.plan_id()?;
    if let Some(state) =
        load_recovery_scheduler_state(state_dir, plan_id, plan.recipient_device_id())?
    {
        return Ok(state);
    }
    ensure!(
        identity.device_id() == plan.recipient_device_id(),
        "history recovery scheduler state can only be initialized by the plan recipient"
    );
    let state = SignedRecoverySchedulerState::initialize(identity, plan_id, now_unix_seconds)?;
    persist_recovery_scheduler_state(state_dir, &state)?;
    Ok(state)
}

fn load_required_recovery_scheduler_state(
    state_dir: &Path,
    plan: &SignedHistoryRecoveryPlan,
) -> Result<SignedRecoverySchedulerState> {
    load_recovery_scheduler_state(state_dir, plan.plan_id()?, plan.recipient_device_id())?
        .context("history recovery scheduler state is not initialized")
}

fn recovery_scheduler_attempt_is_current(
    state_dir: &Path,
    plan: &SignedHistoryRecoveryPlan,
    started: &SignedRecoverySchedulerState,
) -> Result<bool> {
    let current = with_state_lock_only(state_dir, || {
        load_required_recovery_scheduler_state(state_dir, plan)
    })?;
    if current.lifecycle() == RecoverySchedulerLifecycle::Cancelled {
        return Ok(false);
    }
    ensure!(
        current.state_id()? == started.state_id()?
            && current.lifecycle() == RecoverySchedulerLifecycle::Attempting,
        "history recovery scheduler attempt lease was replaced unexpectedly"
    );
    Ok(true)
}

fn finalize_expired_recovery_attempt(
    state_dir: &Path,
    plan: &SignedHistoryRecoveryPlan,
    now_unix_seconds: u64,
    backoff: RecoveryBackoffConfig,
) -> Result<SignedRecoverySchedulerState> {
    with_locked_state(state_dir, || {
        let device_state = load_scheduler_signing_device(state_dir, plan)?;
        let current = load_required_recovery_scheduler_state(state_dir, plan)?;
        ensure!(
            current.readiness(now_unix_seconds) == RecoverySchedulerReadiness::AttemptLeaseExpired,
            "history recovery attempt lease is no longer expired"
        );
        let next = current.record_failure(device_state.identity(), now_unix_seconds, backoff)?;
        persist_recovery_scheduler_state(state_dir, &next)?;
        Ok(next)
    })
}

fn finalize_recovery_scheduler_attempt(
    state_dir: &Path,
    plan: &SignedHistoryRecoveryPlan,
    started: &SignedRecoverySchedulerState,
    result: SchedulerAttemptResult,
    now_unix_seconds: u64,
    backoff: RecoveryBackoffConfig,
) -> Result<SignedRecoverySchedulerState> {
    with_locked_state(state_dir, || {
        let device_state = load_scheduler_signing_device(state_dir, plan)?;
        let current = load_required_recovery_scheduler_state(state_dir, plan)?;
        if current.lifecycle() == RecoverySchedulerLifecycle::Cancelled {
            return Ok(current);
        }
        ensure!(
            current.state_id()? == started.state_id()?
                && current.lifecycle() == RecoverySchedulerLifecycle::Attempting,
            "history recovery scheduler attempt cannot finalize a replaced lease"
        );
        let next = match result {
            SchedulerAttemptResult::Failed => {
                current.record_failure(device_state.identity(), now_unix_seconds, backoff)?
            }
            SchedulerAttemptResult::Progressed => {
                current.record_progress(device_state.identity(), now_unix_seconds, backoff)?
            }
            SchedulerAttemptResult::Completed => {
                current.complete(device_state.identity(), now_unix_seconds)?
            }
        };
        persist_recovery_scheduler_state(state_dir, &next)?;
        Ok(next)
    })
}

fn load_scheduler_signing_device(
    state_dir: &Path,
    plan: &SignedHistoryRecoveryPlan,
) -> Result<DeviceState> {
    plan.verify()?;
    let device_state = load_command_device_state(state_dir)?;
    ensure!(
        device_state.identity().device_id() == plan.recipient_device_id(),
        "local device cannot sign scheduler state for a different plan recipient"
    );
    Ok(device_state)
}

fn print_recovery_scheduler_state(state: &SignedRecoverySchedulerState) -> Result<()> {
    println!(
        "history_recovery_scheduler_state_id={}",
        encode_hex(&state.state_id()?)
    );
    println!(
        "history_recovery_scheduler_generation={}",
        state.generation()
    );
    println!(
        "history_recovery_scheduler_transition={}",
        state.transition().as_str()
    );
    println!(
        "history_recovery_scheduler_lifecycle={}",
        state.lifecycle().as_str()
    );
    println!(
        "history_recovery_scheduler_total_attempts={}",
        state.total_attempts()
    );
    println!(
        "history_recovery_scheduler_consecutive_failures={}",
        state.consecutive_failures()
    );
    println!(
        "history_recovery_scheduler_last_observed_unix_seconds={}",
        state.last_observed_unix_seconds()
    );
    println!(
        "history_recovery_scheduler_next_attempt_at_unix_seconds={}",
        state.next_attempt_at_unix_seconds()
    );
    println!(
        "history_recovery_scheduler_scheduled_delay_seconds={}",
        state.scheduled_delay_seconds()
    );
    Ok(())
}

fn history_recovery_attempt_lease_seconds(
    discovery_wait_seconds: u64,
    max_pages: usize,
) -> Result<u64> {
    let page_seconds = u64::try_from(max_pages)?
        .checked_mul(HISTORY_RECOVERY_NEXT_PAGE_TIMEOUT.as_secs())
        .context("history recovery attempt page timeout bound overflows")?;
    let seconds = discovery_wait_seconds
        .checked_add(CONNECTION_TIMEOUT.as_secs())
        .and_then(|value| value.checked_add(ROUTE_POLICY_WAIT.as_secs()))
        .and_then(|value| value.checked_add(CLIENT_RELAY_WAIT_SECONDS))
        .and_then(|value| value.checked_add(page_seconds))
        .and_then(|value| value.checked_add(60))
        .context("history recovery attempt lease bound overflows")?;
    ensure!(
        seconds <= MAX_RECOVERY_ATTEMPT_LEASE_SECONDS,
        "history recovery attempt lease exceeds {MAX_RECOVERY_ATTEMPT_LEASE_SECONDS} seconds"
    );
    Ok(seconds.max(1))
}

async fn wait_for_recovery_scheduler_deadline(
    state_dir: &Path,
    plan: &SignedHistoryRecoveryPlan,
) -> Result<()> {
    loop {
        let now = unix_time_now()?;
        plan.verify_at(now)?;
        let current = with_state_lock_only(state_dir, || {
            load_required_recovery_scheduler_state(state_dir, plan)
        })?;
        match current.readiness(now) {
            RecoverySchedulerReadiness::Ready => return Ok(()),
            RecoverySchedulerReadiness::Deferred {
                not_before_unix_seconds,
            } => {
                let remaining = not_before_unix_seconds.saturating_sub(now);
                println!("history_recovery_scheduler_retry_wait_remaining_seconds={remaining}");
                tokio::time::sleep(Duration::from_secs(remaining.clamp(1, 5))).await;
            }
            RecoverySchedulerReadiness::Cancelled => return Ok(()),
            RecoverySchedulerReadiness::Completed => return Ok(()),
            RecoverySchedulerReadiness::ClockRollback {
                last_observed_unix_seconds,
            } => {
                bail!(
                    "wall clock is older than signed scheduler high-water mark {last_observed_unix_seconds}"
                )
            }
            RecoverySchedulerReadiness::AttemptLeaseExpired => {
                bail!("history recovery scheduler attempt lease expired while waiting")
            }
        }
    }
}

fn with_state_lock_only<T>(
    state_directory: &Path,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let _state_lock = StateDirectoryLock::acquire(state_directory)
        .context("lock state directory for scheduler state observation")?;
    operation()
}

async fn cancel_history_recovery_plan(
    state_dir: PathBuf,
    plan_file: PathBuf,
    conversation: String,
) -> Result<()> {
    let plan = load_history_recovery_plan(&plan_file).await?;
    plan.verify()?;
    ensure!(
        ConversationId::from_label(&conversation) == plan.conversation_id(),
        "history recovery plan is bound to a different conversation"
    );
    let device_state = load_scheduler_signing_device(&state_dir, &plan)?;
    let now = unix_time_now()?;
    let current = load_or_initialize_recovery_scheduler_state(
        &state_dir,
        device_state.identity(),
        &plan,
        now,
    )?;
    let state = match current.lifecycle() {
        RecoverySchedulerLifecycle::Cancelled => current,
        RecoverySchedulerLifecycle::Completed => {
            bail!("completed history recovery plan cannot be cancelled")
        }
        RecoverySchedulerLifecycle::Active | RecoverySchedulerLifecycle::Attempting => {
            let cancelled = current.cancel(device_state.identity(), now)?;
            persist_recovery_scheduler_state(&state_dir, &cancelled)?;
            cancelled
        }
    };
    println!("history_recovery_plan_id={}", encode_hex(&plan.plan_id()?));
    println!("history_recovery_plan_file={}", plan_file.display());
    print_recovery_scheduler_state(&state)?;
    println!("history_recovery_scheduler_discovery_attempted=false");
    println!("connection_attempted=false");
    println!("status=history-recovery-scheduler-cancelled");
    Ok(())
}

fn accept_history_recovery_link(
    state_dir: PathBuf,
    link: Option<String>,
    link_file: Option<PathBuf>,
    qr_file: Option<PathBuf>,
    conversation: String,
    confirmed_sas: String,
    max_pages: usize,
) -> CommandFuture {
    Box::pin(accept_history_recovery_link_inner(
        state_dir,
        link,
        link_file,
        qr_file,
        conversation,
        confirmed_sas,
        max_pages,
    ))
}

async fn accept_history_recovery_link_inner(
    state_dir: PathBuf,
    link: Option<String>,
    link_file: Option<PathBuf>,
    qr_file: Option<PathBuf>,
    conversation: String,
    confirmed_sas: String,
    max_pages: usize,
) -> Result<()> {
    let loaded = load_history_recovery_link(link, link_file, qr_file).await?;
    let link = loaded.link;
    let recipient_certificate = {
        let device_state = load_command_device_state(&state_dir)?;
        let trust = CommandTrustReadRepository::open(&state_dir, &device_state)?;
        trust
            .load_certificate()
            .context("load recipient Account Root certificate")?
    };
    link.verify_for_recipient(unix_time_now()?, &recipient_certificate)?;

    let conversation_id = ConversationId::from_label(&conversation);
    ensure!(
        conversation_id == link.conversation_id(),
        "history recovery link is bound to a different conversation"
    );
    let sas = link.sas()?;
    ensure!(
        confirmed_sas.trim() == sas.to_string(),
        "recipient confirmed SAS {}, but the signed history recovery link derives {sas}",
        confirmed_sas.trim()
    );

    print_history_recovery_link_input(&loaded.input);
    print_history_recovery_link(&link, loaded.encoded.len())?;
    println!("history_recovery_link_user_consent=confirmed");

    let expected_account_id = link.account_id();
    let expected_source_device_id = link.source_device_id();
    let approved_range_start = usize::try_from(link.approved_range_start())
        .context("history recovery link range start cannot be represented on this platform")?;
    let approved_event_count = usize::try_from(link.approved_event_count())
        .context("history recovery link event count cannot be represented on this platform")?;
    let page_size = usize::try_from(link.page_size())
        .context("history recovery link page size cannot be represented on this platform")?;
    let bootstrap = HistoryRecoveryBootstrap::from_link(&link);

    resume_history_recovery_inner(
        state_dir,
        None,
        None,
        Some(bootstrap),
        conversation,
        expected_source_device_id,
        approved_range_start,
        approved_event_count,
        page_size,
        max_pages,
        confirmed_sas,
        expected_account_id,
    )
    .await
}

fn show_identity(state_dir: PathBuf) -> Result<()> {
    let device_state = load_command_device_state(&state_dir)?;
    println!("device_id={}", device_state.identity().device_id());
    println!(
        "device_encryption_public_key={}",
        device_state.encryption().public_key()
    );
    Ok(())
}

fn create_account(account_dir: PathBuf) -> Result<()> {
    let account = AccountRootState::create(&account_dir)
        .with_context(|| format!("create Account Root state in {}", account_dir.display()))?;
    println!("account_id={}", account.account_id());
    println!("account_root_dir={}", account_dir.display());
    println!("root_key_protection={}", account.key_protection().as_str());
    println!("root_key_load={}", account.key_load_outcome().as_str());
    println!("status=account-created");
    Ok(())
}

fn show_account(account_dir: PathBuf) -> Result<()> {
    let account = AccountRootState::load(&account_dir)
        .with_context(|| format!("load Account Root state from {}", account_dir.display()))?;
    println!("account_id={}", account.account_id());
    println!("root_key_protection={}", account.key_protection().as_str());
    println!("root_key_load={}", account.key_load_outcome().as_str());
    println!("status=account-loaded");
    Ok(())
}

fn export_account_snapshot(account_dir: PathBuf, snapshot_file: PathBuf) -> Result<()> {
    let account = AccountRootState::load(&account_dir)
        .with_context(|| format!("load Account Root state from {}", account_dir.display()))?;
    let snapshot = account
        .authority_snapshot()
        .context("create complete root-signed authority snapshot")?;
    let encoded = snapshot.encode()?;
    write_new_authority_file(&snapshot_file, &encoded)
        .with_context(|| format!("export authority snapshot to {}", snapshot_file.display()))?;
    println!("account_id={}", snapshot.account_id());
    println!("authority_revision={}", snapshot.revision());
    println!("revocation_count={}", snapshot.revocations().len());
    println!("snapshot_file={}", snapshot_file.display());
    println!("snapshot={}", URL_SAFE_NO_PAD.encode(encoded));
    println!("status=account-snapshot-exported");
    Ok(())
}

fn publish_account_device_list(
    account_dir: PathBuf,
    device_certificate_files: Vec<PathBuf>,
    device_list_file: PathBuf,
) -> Result<()> {
    let account = AccountRootState::load(&account_dir)
        .with_context(|| format!("load Account Root state from {}", account_dir.display()))?;
    let mut certificates = Vec::with_capacity(device_certificate_files.len());
    for path in device_certificate_files {
        certificates.push(
            DeviceCertificate::decode_and_verify(
                &fs::read(&path)
                    .with_context(|| format!("read device certificate from {}", path.display()))?,
            )
            .with_context(|| format!("verify device certificate from {}", path.display()))?,
        );
    }
    let device_list = account
        .publish_device_list(&certificates)
        .context("publish complete root-signed account device list")?;
    write_new_authority_file(&device_list_file, &device_list.encode()?)
        .with_context(|| format!("export device list to {}", device_list_file.display()))?;
    println!("account_id={}", device_list.account_id());
    println!("authority_revision={}", device_list.revision());
    println!("device_count={}", device_list.devices().len());
    for certificate in device_list.devices() {
        println!("device_id={}", certificate.device_id());
    }
    println!("device_list_file={}", device_list_file.display());
    println!("status=account-device-list-published");
    Ok(())
}

fn create_conversation_membership(
    account_dir: PathBuf,
    conversation: String,
    member_accounts: Vec<AccountId>,
    membership_file: PathBuf,
) -> Result<()> {
    let account = AccountRootState::load(&account_dir)
        .with_context(|| format!("load Account Root state from {}", account_dir.display()))?;
    let conversation_id = ConversationId::from_label(&conversation);
    let membership = account
        .create_conversation_membership(conversation_id.scope_id(), &member_accounts)
        .context("create owner-signed conversation membership")?;
    export_conversation_membership(&membership_file, &membership)?;
    println!("conversation={conversation}");
    print_conversation_membership(&membership, &membership_file);
    println!("status=conversation-created");
    Ok(())
}

fn add_conversation_members(
    account_dir: PathBuf,
    conversation: String,
    member_accounts: Vec<AccountId>,
    membership_file: PathBuf,
) -> Result<()> {
    ensure!(
        !member_accounts.is_empty(),
        "provide at least one --member-account"
    );
    let account = AccountRootState::load(&account_dir)
        .with_context(|| format!("load Account Root state from {}", account_dir.display()))?;
    let conversation_id = ConversationId::from_label(&conversation);
    let membership = account
        .add_conversation_members(conversation_id.scope_id(), &member_accounts)
        .context("add accounts to owner-signed conversation membership")?;
    export_conversation_membership(&membership_file, &membership)?;
    println!("conversation={conversation}");
    print_conversation_membership(&membership, &membership_file);
    println!("status=conversation-members-added");
    Ok(())
}

fn install_conversation_membership(state_dir: PathBuf, membership_file: PathBuf) -> Result<()> {
    let device = load_command_device_state(&state_dir)?;
    let trust = CommandTrustReadRepository::open(&state_dir, &device)?;
    let certificate = trust
        .load_certificate()
        .context("load device certificate before installing conversation membership")?;
    let bytes = fs::read(&membership_file).with_context(|| {
        format!(
            "read conversation membership from {}",
            membership_file.display()
        )
    })?;
    let membership =
        ConversationMembershipSnapshot::decode_and_verify(&bytes).with_context(|| {
            format!(
                "verify conversation membership from {}",
                membership_file.display()
            )
        })?;
    membership
        .require_member(certificate.account_id())
        .context("this device account is not a member of the conversation")?;
    let store = install_membership_primary(&state_dir, &device, &membership)
        .context("install conversation membership and reject rollback or equivocation")?;
    println!("account_id={}", certificate.account_id());
    println!("conversation_id={}", membership.conversation_id());
    println!(
        "conversation_owner_account_id={}",
        membership.owner_account_id()
    );
    println!("membership_revision={}", membership.revision());
    println!("membership_store={store:?}");
    println!("status=conversation-membership-installed");
    Ok(())
}

fn export_conversation_membership(
    path: &Path,
    membership: &ConversationMembershipSnapshot,
) -> Result<()> {
    let encoded = membership.encode()?;
    write_new_authority_file(path, &encoded)
        .with_context(|| format!("export conversation membership to {}", path.display()))
}

fn print_conversation_membership(
    membership: &ConversationMembershipSnapshot,
    membership_file: &Path,
) {
    println!("conversation_id={}", membership.conversation_id());
    println!(
        "conversation_owner_account_id={}",
        membership.owner_account_id()
    );
    println!("membership_revision={}", membership.revision());
    println!("member_count={}", membership.members().len());
    println!(
        "member_accounts={}",
        membership
            .members()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    );
    println!("membership_file={}", membership_file.display());
}

fn enroll_device(
    account_dir: PathBuf,
    state_dir: PathBuf,
    certificate_file: Option<PathBuf>,
) -> Result<()> {
    let account = AccountRootState::load(&account_dir)
        .with_context(|| format!("load Account Root state from {}", account_dir.display()))?;
    let device = load_command_device_state(&state_dir)?;
    let certificate = account
        .issue_device_certificate(
            device.identity().device_id(),
            device.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )
        .context("issue root-signed device certificate")?;
    let authority_snapshot = account
        .authority_snapshot()
        .context("create authority snapshot after device enrollment")?;
    let snapshot_store =
        install_enrollment_primary(&state_dir, &device, &certificate, &authority_snapshot)
            .context("install current authority snapshot into device state")?;
    let encoded = certificate.encode()?;
    if let Some(path) = certificate_file {
        write_new_authority_file(&path, &encoded)
            .with_context(|| format!("export device certificate to {}", path.display()))?;
        println!("certificate_file={}", path.display());
    }

    println!("account_id={}", certificate.account_id());
    println!("device_id={}", certificate.device_id());
    println!(
        "device_encryption_public_key={}",
        certificate.encryption_public_key()
    );
    println!(
        "certificate_authority_sequence={}",
        certificate.authority_sequence()
    );
    println!(
        "device_capabilities={}",
        format_capabilities(certificate.capabilities())
    );
    println!("authority_revision={}", authority_snapshot.revision());
    println!("authority_snapshot_store={snapshot_store:?}");
    println!("certificate={}", URL_SAFE_NO_PAD.encode(encoded));
    println!("status=device-enrolled");
    Ok(())
}

fn authorize_device(state_dir: PathBuf, account_id: AccountId) -> Result<()> {
    let device = load_command_device_state(&state_dir)?;
    let trust = CommandTrustReadRepository::open(&state_dir, &device)?;
    let certificate = trust
        .load_certificate()
        .context("load installed root-signed device certificate")?;
    let snapshot = trust
        .load_own_authority_snapshot(&certificate)
        .context("load installed authority snapshot")?;
    let authorization = verify_device_authorization_with_snapshot(
        account_id,
        &certificate,
        &snapshot,
        &DeviceCapability::MESSAGING,
    )
    .context("verify Account Root to device authorization")?;

    println!("account_id={}", authorization.account_id());
    println!("device_id={}", authorization.device_id());
    println!(
        "certificate_authority_sequence={}",
        authorization.certificate_authority_sequence()
    );
    println!(
        "device_capabilities={}",
        format_capabilities(authorization.capabilities())
    );
    println!("authority_revision={}", snapshot.revision());
    println!("revocation_count={}", snapshot.revocations().len());
    println!("authorization=valid");
    println!("status=device-authorized");
    Ok(())
}

fn update_device_authority(state_dir: PathBuf, snapshot_file: PathBuf) -> Result<()> {
    let device = load_command_device_state(&state_dir)?;
    let bytes = fs::read(&snapshot_file)
        .with_context(|| format!("read authority snapshot from {}", snapshot_file.display()))?;
    let snapshot = AccountAuthoritySnapshot::decode_and_verify(&bytes)
        .with_context(|| format!("verify authority snapshot from {}", snapshot_file.display()))?;
    let store = install_own_authority_primary(&state_dir, &device, &snapshot)
        .context("install own authority snapshot and reject rollback")?;
    println!("account_id={}", snapshot.account_id());
    println!("authority_revision={}", snapshot.revision());
    println!("revocation_count={}", snapshot.revocations().len());
    println!("authority_snapshot_store={store:?}");
    println!("status=device-authority-updated");
    Ok(())
}

fn revoke_device(
    account_dir: PathBuf,
    device_id: DeviceId,
    revocation_file: PathBuf,
) -> Result<()> {
    let account = AccountRootState::load(&account_dir)
        .with_context(|| format!("load Account Root state from {}", account_dir.display()))?;
    let revocation = account
        .revoke_device(device_id)
        .context("issue root-signed permanent device revocation")?;
    let encoded = revocation.encode()?;
    write_new_authority_file(&revocation_file, &encoded)
        .with_context(|| format!("write device revocation to {}", revocation_file.display()))?;

    println!("account_id={}", revocation.account_id());
    println!("revoked_device_id={}", revocation.device_id());
    println!(
        "revocation_authority_sequence={}",
        revocation.authority_sequence()
    );
    println!("revocation_file={}", revocation_file.display());
    println!("revocation={}", URL_SAFE_NO_PAD.encode(encoded));
    println!("status=device-revoked");
    Ok(())
}

fn format_capabilities(capabilities: &[DeviceCapability]) -> String {
    capabilities
        .iter()
        .map(|capability| capability.as_str())
        .collect::<Vec<_>>()
        .join(",")
}

fn write_new_authority_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("create authority output directory {}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .context("create authority output without overwriting an existing file")?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

struct ImmutableReadRepositories {
    events: Box<dyn EventReadRepository>,
    local_messages: Box<dyn LocalMessageReadRepository>,
    primary: &'static str,
    shadow: &'static str,
    mirror_generation: Option<u64>,
    event_record_count: u64,
    local_projection_record_count: u64,
}

fn open_immutable_read_repositories(state_dir: &Path) -> Result<ImmutableReadRepositories> {
    if !EncryptedStateVault::is_initialized(state_dir)
        .context("inspect encrypted state vault before history read")?
    {
        return open_legacy_read_repositories(
            &state_dir.join(EVENT_STORE_DIRECTORY),
            &state_dir.join(LOCAL_MESSAGE_STORE_DIRECTORY),
        );
    }

    let vault = EncryptedStateVault::open_existing(state_dir)
        .context("open encrypted vault for immutable primary-read canary")?;
    let primary = vault
        .read_primary_canary(&[StateRecordKind::Event, StateRecordKind::LocalProjection])
        .context("read immutable history from vault and compare legacy shadow")?;
    let mirror_generation = primary.mirror_generation();
    let event_record_count = primary
        .shadow_reports()
        .iter()
        .find(|report| report.kind() == StateRecordKind::Event)
        .map_or(0, |report| report.record_count());
    let local_projection_record_count = primary
        .shadow_reports()
        .iter()
        .find(|report| report.kind() == StateRecordKind::LocalProjection)
        .map_or(0, |report| report.record_count());
    let mut event_records = Vec::new();
    let mut local_projection_records = Vec::new();
    for record in primary.into_records() {
        let (kind, relative_path, content) = record.into_parts();
        match kind {
            StateRecordKind::Event => event_records.push((
                strip_vault_record_prefix(&relative_path, "events/")?,
                content,
            )),
            StateRecordKind::LocalProjection => local_projection_records.push((
                strip_vault_record_prefix(&relative_path, "local-messages/")?,
                content,
            )),
            _ => bail!(
                "vault immutable history selection returned unexpected {} record",
                kind.as_str()
            ),
        }
    }
    let events = ImmutableEventReadSnapshot::from_records(event_records)
        .context("construct authenticated event read snapshot from vault")?;
    let local_messages = ImmutableLocalMessageReadSnapshot::from_records(local_projection_records)
        .context("construct authenticated local-projection read snapshot from vault")?;
    Ok(ImmutableReadRepositories {
        events: Box::new(events),
        local_messages: Box::new(local_messages),
        primary: "encrypted-vault",
        shadow: "legacy-verified",
        mirror_generation: Some(mirror_generation),
        event_record_count,
        local_projection_record_count,
    })
}

fn open_legacy_read_repositories(
    event_root: &Path,
    local_projection_root: &Path,
) -> Result<ImmutableReadRepositories> {
    Ok(ImmutableReadRepositories {
        events: Box::new(EventStore::open(event_root)?),
        local_messages: Box::new(LocalMessageStore::open(local_projection_root)?),
        primary: "legacy-filesystem",
        shadow: "not-enabled",
        mirror_generation: None,
        event_record_count: 0,
        local_projection_record_count: 0,
    })
}

fn print_immutable_read_diagnostics(prefix: &str, repositories: &ImmutableReadRepositories) {
    println!("{prefix}_primary_read={}", repositories.primary);
    println!("{prefix}_shadow_read={}", repositories.shadow);
    if let Some(generation) = repositories.mirror_generation {
        println!("{prefix}_vault_generation={generation}");
        println!(
            "{prefix}_vault_event_records={}",
            repositories.event_record_count
        );
        println!(
            "{prefix}_vault_local_projection_records={}",
            repositories.local_projection_record_count
        );
    }
}

fn strip_vault_record_prefix(relative_path: &str, prefix: &str) -> Result<String> {
    let stripped = relative_path
        .strip_prefix(prefix)
        .filter(|path| !path.is_empty())
        .with_context(|| {
            format!("vault record {relative_path} does not have required prefix {prefix}")
        })?;
    Ok(stripped.to_owned())
}

fn show_history(state_dir: PathBuf, conversation: String) -> Result<()> {
    let device_state = load_command_device_state(&state_dir)?;
    let trust = CommandTrustReadRepository::open(&state_dir, &device_state)?;
    let certificate = trust
        .load_certificate()
        .context("load device certificate before reading history")?;
    let read_repositories = open_immutable_read_repositories(&state_dir)?;
    let conversation_id = ConversationId::from_label(&conversation);
    let membership = trust
        .load_conversation_membership(conversation_id.scope_id())
        .context("load trusted conversation membership before reading history")?;
    membership
        .require_member(certificate.account_id())
        .context("this device account is not a member of the conversation")?;
    let events = read_repositories
        .events
        .load_authorized_conversation(conversation_id, &membership)
        .context("load and verify authorized local conversation history")?;
    let frontier = read_repositories
        .events
        .frontier(conversation_id)
        .context("calculate local conversation frontier")?;

    print_immutable_read_diagnostics("history", &read_repositories);
    println!("conversation_id={conversation_id}");
    println!("membership_revision={}", membership.revision());
    println!("event_count={}", events.len());
    println!("frontier_count={}", frontier.len());
    println!(
        "frontier={}",
        frontier
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    );
    for stored in events {
        let author_account_id = stored.event.author_account_id();
        let event = stored.event.into_event();
        let parents = event
            .parents()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        match event.payload() {
            EventPayload::RatchetText { .. } => {
                let projection = read_repositories
                    .local_messages
                    .get(stored.id)
                    .with_context(|| format!("load local projection for event {}", stored.id))?;
                let body = projection
                    .open_for_account(
                        &event,
                        device_state.identity().device_id(),
                        certificate.account_id(),
                        device_state.encryption(),
                    )
                    .with_context(|| format!("decrypt local projection for event {}", stored.id))?;
                println!(
                    "event_id={} author_account_id={author_account_id} author_device_id={} author_sequence={} parents=[{parents}] payload=ratchet-text body={body:?}",
                    stored.id,
                    event.author_device_id(),
                    event.author_sequence()
                );
            }
            EventPayload::Acknowledgement {
                acknowledged_event_id,
            } => println!(
                "event_id={} author_account_id={author_account_id} author_device_id={} author_sequence={} parents=[{parents}] payload=acknowledgement acknowledged_event_id={acknowledged_event_id}",
                stored.id,
                event.author_device_id(),
                event.author_sequence()
            ),
        }
    }
    Ok(())
}

fn export_ratchet_bundle(state_dir: PathBuf, bundle_file: PathBuf) -> Result<()> {
    let device_state = load_command_device_state(&state_dir)?;
    let trust = CommandTrustReadRepository::open(&state_dir, &device_state)?;
    let certificate = trust
        .load_certificate()
        .context("load device certificate before exporting a prekey bundle")?;
    ensure!(
        certificate.device_id() == device_state.identity().device_id(),
        "installed certificate belongs to a different device"
    );
    let bundle = run_state_transaction(&state_dir, |transaction| {
        let mut ratchet_state = transaction.load_ratchet_state()?;
        ratchet_state
            .prekey_bundle(device_state.identity())
            .context("create or reuse the current one-time prekey bundle")
    })?;
    write_new_authority_file(&bundle_file, &bundle.encode()?)
        .with_context(|| format!("export prekey bundle to {}", bundle_file.display()))?;
    println!("device_id={}", bundle.device_id());
    println!("ratchet_prekey_sequence={}", bundle.sequence());
    println!("prekey_bundle_file={}", bundle_file.display());
    println!("status=ratchet-bundle-exported");
    Ok(())
}

fn export_ratchet_prekey_pool(
    state_dir: PathBuf,
    pool_file: PathBuf,
    count: usize,
    valid_for_hours: u64,
    refresh: bool,
) -> Result<()> {
    ensure!(
        (1..=MAX_PREKEY_POOL_SIZE).contains(&count),
        "--count must be within 1..={MAX_PREKEY_POOL_SIZE}"
    );
    let validity_seconds = valid_for_hours
        .checked_mul(60 * 60)
        .context("--valid-for-hours overflows seconds")?;
    let device_state = load_command_device_state(&state_dir)?;
    let trust = CommandTrustReadRepository::open(&state_dir, &device_state)?;
    let certificate = trust
        .load_certificate()
        .context("load device certificate before exporting a prekey pool")?;
    ensure!(
        certificate.device_id() == device_state.identity().device_id(),
        "installed certificate belongs to a different device"
    );
    let now_unix_seconds = unix_time_now().context("read time for prekey pool publication")?;
    let pool = run_state_transaction(&state_dir, |transaction| {
        let mut ratchet_state = transaction.load_ratchet_state()?;
        if refresh {
            ratchet_state
                .refresh_prekey_pool(
                    device_state.identity(),
                    count,
                    now_unix_seconds,
                    validity_seconds,
                )
                .context("rotate signed one-time prekey pool")
        } else {
            ratchet_state
                .prekey_pool(
                    device_state.identity(),
                    count,
                    now_unix_seconds,
                    validity_seconds,
                )
                .context("create or reuse signed one-time prekey pool")
        }
    })?;
    write_new_authority_file(&pool_file, &pool.encode()?)
        .with_context(|| format!("export prekey pool to {}", pool_file.display()))?;
    println!("device_id={}", pool.device_id());
    println!("prekey_pool_generation={}", pool.generation());
    println!("prekey_count={}", pool.bundles().len());
    println!("prekey_sequence_first={}", pool.first_sequence());
    println!("prekey_sequence_last={}", pool.last_sequence());
    println!(
        "prekey_pool_published_at={}",
        pool.published_at_unix_seconds()
    );
    println!("prekey_pool_expires_at={}", pool.expires_at_unix_seconds());
    println!("prekey_pool_file={}", pool_file.display());
    println!("status=ratchet-prekey-pool-exported");
    Ok(())
}

fn show_history_rewrap_sas(
    device_list_file: PathBuf,
    source_device_id: DeviceId,
    recipient_device_id: DeviceId,
) -> Result<()> {
    let device_list = AccountDeviceListSnapshot::decode_and_verify(
        &fs::read(&device_list_file)
            .with_context(|| format!("read device list from {}", device_list_file.display()))?,
    )
    .context("decode and verify history-rewrap account device list")?;
    let sas = HistoryRewrapSas::derive(&device_list, source_device_id, recipient_device_id)?;
    println!("account_id={}", device_list.account_id());
    println!("authority_revision={}", device_list.revision());
    println!("source_device_id={source_device_id}");
    println!("recipient_device_id={recipient_device_id}");
    println!("history_rewrap_sas={sas}");
    println!("status=history-rewrap-sas-derived");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn fetch_history_rewrap(
    state_dir: PathBuf,
    ticket: Option<String>,
    ticket_file: Option<PathBuf>,
    conversation: String,
    range_start: usize,
    count: usize,
    confirmed_sas: String,
    expected_account_id: AccountId,
    expected_source_device_id: Option<DeviceId>,
    recovery_checkpoint: Option<SignedHistoryRecoveryCheckpoint>,
    final_status: &'static str,
) -> Result<()> {
    ensure!(count > 0, "--count must be greater than zero");
    ensure!(
        count <= MAX_HISTORY_REWRAP_ENTRIES,
        "--count must not exceed {MAX_HISTORY_REWRAP_ENTRIES}"
    );
    let device_state = load_command_device_state(&state_dir)?;
    let trust = CommandTrustReadRepository::open(&state_dir, &device_state)?;
    let recipient_certificate = trust
        .load_certificate()
        .context("load recipient Account Root certificate")?;
    let recipient_authority_snapshot = trust
        .load_own_authority_snapshot(&recipient_certificate)
        .context("load recipient Account Root authority snapshot")?;
    ensure!(
        recipient_certificate.account_id() == expected_account_id,
        "network history rewrap requires the recipient to belong to --expect-account"
    );
    let ticket = load_connection_ticket(ticket, ticket_file).await?;
    ticket.verify_listener_account(expected_account_id)?;
    ensure!(
        ticket.allowed_requester_account_id() == expected_account_id,
        "source ticket does not authorize this same account"
    );
    let source = ticket.verify_listener_authorization(expected_account_id)?;
    let source_device_id = source.device_id();
    if let Some(expected_source_device_id) = expected_source_device_id {
        ensure!(
            source_device_id == expected_source_device_id,
            "source ticket belongs to device {source_device_id}; explicitly selected source is {expected_source_device_id}"
        );
    }
    let device_list = ticket.listener_directory().device_list();
    ensure!(
        device_list.certificate_for(recipient_certificate.device_id())
            == Some(&recipient_certificate),
        "recipient certificate is not present exactly in the source signed device list"
    );
    let sas = HistoryRewrapSas::derive(
        device_list,
        source_device_id,
        recipient_certificate.device_id(),
    )?;
    println!("history_rewrap_sas={sas}");
    ensure!(
        confirmed_sas.trim() == sas.to_string(),
        "recipient confirmed SAS {}, but the signed source ticket derives {sas}",
        confirmed_sas.trim()
    );
    println!("history_rewrap_user_consent=confirmed");
    let conversation_id = ConversationId::from_label(&conversation);
    let membership = trust
        .load_conversation_membership(conversation_id.scope_id())
        .context("load trusted conversation membership before network history rewrap")?;
    membership
        .require_member(expected_account_id)
        .context("account is not a member of the requested conversation")?;
    let source_snapshot_store = install_own_authority_primary(
        &state_dir,
        &device_state,
        ticket.listener_authority_snapshot(),
    )
    .context("install same-account authority snapshot from source ticket")?;
    let session_binding =
        SyncSessionBinding::from_transport_label(&ticket.endpoint().id.to_string());
    let route_policy = ticket.route_policy();
    let endpoint = endpoint_builder_for_remote(route_policy, ticket.endpoint())?
        .bind()
        .await
        .context("bind history-rewrap recipient endpoint")?;
    println!("transport_endpoint_id={}", endpoint.id());
    println!("account_id={expected_account_id}");
    println!("device_id={}", recipient_certificate.device_id());
    println!("source_device_id={source_device_id}");
    println!("source_authority_store={source_snapshot_store:?}");
    println!("route_policy={}", route_policy.as_str());
    print_connection_target(&ticket);
    if route_policy == RoutePolicy::RelayOnly {
        wait_for_relay(&endpoint, route_policy, CLIENT_RELAY_WAIT_SECONDS).await?;
    }
    let connection = timeout(
        CONNECTION_TIMEOUT,
        endpoint.connect(ticket.endpoint().clone(), ALPN),
    )
    .await
    .with_context(|| {
        timeout_message(
            "connect to listening endpoint for history rewrap",
            CONNECTION_TIMEOUT,
        )
    })?
    .context("connect to listening endpoint for history rewrap")?;
    println!("peer_id={}", connection.remote_id());
    let ready_path = await_route_policy(&connection, route_policy, ROUTE_POLICY_WAIT)
        .await
        .context("wait for a history-rewrap path allowed by the connection ticket")?;
    print_ready_path(&ready_path);
    authorize_with_listener(
        &connection,
        device_state.identity(),
        recipient_certificate,
        recipient_authority_snapshot,
        session_binding,
    )
    .await?;
    let (transfer, transfer_encoded) = request_history_rewrap_transfer(
        &connection,
        device_state.identity(),
        conversation_id,
        source_device_id,
        session_binding,
        range_start,
        count,
        sas,
    )
    .await?;
    let bundle = transfer.bundle().clone();
    let bundle_encoded = bundle.encode()?;
    let recovery_checkpoint = recovery_checkpoint
        .map(|checkpoint| {
            let requested_range_start = u64::try_from(range_start)
                .context("history-rewrap request range cannot be represented")?;
            ensure!(
                checkpoint.next_range_start() == requested_range_start,
                "history recovery checkpoint expects range {}, but request starts at {}",
                checkpoint.next_range_start(),
                requested_range_start
            );
            let advanced = checkpoint
                .advance(device_state.identity(), bundle.manifest())
                .context("advance signed history recovery checkpoint")?;
            let encoded = advanced.encode()?;
            Ok::<_, anyhow::Error>((advanced, encoded))
        })
        .transpose()?;
    println!("history_rewrap_transfer_bytes={}", transfer_encoded.len());
    print_transport_diagnostics(&connection, route_policy).await?;
    connection.close(0_u32.into(), b"kilogram history rewrap complete");
    endpoint.close().await;
    import_history_rewrap_material(
        state_dir,
        conversation,
        bundle,
        bundle_encoded,
        Some((transfer, transfer_encoded)),
        recovery_checkpoint,
        final_status,
    )
}

#[allow(clippy::too_many_arguments)]
async fn request_history_rewrap_transfer(
    connection: &Connection,
    recipient_identity: &DeviceIdentity,
    conversation_id: ConversationId,
    source_device_id: DeviceId,
    session_binding: SyncSessionBinding,
    range_start: usize,
    count: usize,
    sas: HistoryRewrapSas,
) -> Result<(SignedHistoryRewrapTransfer, Vec<u8>)> {
    let request = SignedHistoryRewrapRequest::sign(
        recipient_identity,
        conversation_id,
        source_device_id,
        session_binding,
        range_start,
        count,
        sas,
    )?;
    let (mut send, mut receive) = open_bi(connection, "open history-rewrap request stream").await?;
    write_client_request(&mut send, &ClientRequest::HistoryRewrap(request.clone())).await?;
    let transfer = match read_server_response(&mut receive).await? {
        ServerResponse::HistoryRewrapTransfer(transfer) => *transfer,
        ServerResponse::HistoryRewrapRejected(rejected) => {
            ensure!(
                rejected.conversation_id() == conversation_id,
                "history-rewrap rejection belongs to a different conversation"
            );
            bail!("history rewrap rejected by source: {:?}", rejected.reason());
        }
        _ => bail!("recipient expected a history-rewrap transfer response"),
    };
    transfer
        .verify_for_request(&request)
        .context("verify source-signed transfer against exact recipient request")?;
    let transfer_encoded = transfer.encode()?;
    ensure!(
        transfer_encoded.len() <= MAX_WIRE_MESSAGE_BYTES,
        "history-rewrap transfer exceeds the bounded wire frame"
    );
    Ok((transfer, transfer_encoded))
}

#[allow(clippy::too_many_arguments)]
fn resume_history_recovery(
    state_dir: PathBuf,
    ticket: Option<String>,
    ticket_file: Option<PathBuf>,
    conversation: String,
    expected_source_device_id: DeviceId,
    approved_range_start: usize,
    approved_event_count: usize,
    page_size: usize,
    max_pages: usize,
    confirmed_sas: String,
    expected_account_id: AccountId,
) -> CommandFuture {
    Box::pin(resume_history_recovery_inner(
        state_dir,
        ticket,
        ticket_file,
        None,
        conversation,
        expected_source_device_id,
        approved_range_start,
        approved_event_count,
        page_size,
        max_pages,
        confirmed_sas,
        expected_account_id,
    ))
}

#[allow(clippy::too_many_arguments)]
async fn resume_history_recovery_inner(
    state_dir: PathBuf,
    ticket: Option<String>,
    ticket_file: Option<PathBuf>,
    bootstrap_override: Option<HistoryRecoveryBootstrap>,
    conversation: String,
    expected_source_device_id: DeviceId,
    approved_range_start: usize,
    approved_event_count: usize,
    page_size: usize,
    max_pages: usize,
    confirmed_sas: String,
    expected_account_id: AccountId,
) -> Result<()> {
    ensure!(
        approved_event_count > 0,
        "--count must be greater than zero"
    );
    approved_range_start
        .checked_add(approved_event_count)
        .context("--range-start plus --count overflows")?;
    ensure!(page_size > 0, "--page-size must be greater than zero");
    ensure!(
        page_size <= MAX_HISTORY_REWRAP_ENTRIES,
        "--page-size must not exceed {MAX_HISTORY_REWRAP_ENTRIES}"
    );
    ensure!(max_pages > 0, "--max-pages must be greater than zero");
    ensure!(
        max_pages <= MAX_HISTORY_RECOVERY_PAGES_PER_SESSION,
        "--max-pages must not exceed {MAX_HISTORY_RECOVERY_PAGES_PER_SESSION}"
    );

    let device_state = load_command_device_state(&state_dir)?;
    let trust = CommandTrustReadRepository::open(&state_dir, &device_state)?;
    let recipient_certificate = trust
        .load_certificate()
        .context("load recipient Account Root certificate")?;
    let recipient_authority_snapshot = trust
        .load_own_authority_snapshot(&recipient_certificate)
        .context("load recipient Account Root authority snapshot")?;
    ensure!(
        recipient_certificate.account_id() == expected_account_id,
        "history recovery requires the recipient to belong to --expect-account"
    );
    let bootstrap = match bootstrap_override {
        Some(bootstrap) => bootstrap,
        None => {
            let inspected_ticket = load_connection_ticket(ticket, ticket_file).await?;
            HistoryRecoveryBootstrap::from_ticket(&inspected_ticket, expected_account_id)?
        }
    };
    ensure!(
        bootstrap.account_id == expected_account_id,
        "history recovery source belongs to a different account"
    );
    ensure!(
        bootstrap.source_device_id == expected_source_device_id,
        "history recovery source belongs to device {}; explicitly selected source is {expected_source_device_id}",
        bootstrap.source_device_id
    );
    let device_list = &bootstrap.account_device_list;
    ensure!(
        device_list.certificate_for(recipient_certificate.device_id())
            == Some(&recipient_certificate),
        "recipient certificate is not present exactly in the source signed device list"
    );
    let sas = HistoryRewrapSas::derive(
        device_list,
        expected_source_device_id,
        recipient_certificate.device_id(),
    )?;
    ensure!(
        confirmed_sas.trim() == sas.to_string(),
        "recipient confirmed SAS {}, but the signed source descriptor derives {sas}",
        confirmed_sas.trim()
    );
    println!("history_rewrap_sas={sas}");
    println!("history_rewrap_user_consent=confirmed");

    let conversation_id = ConversationId::from_label(&conversation);
    let initial_checkpoint = SignedHistoryRecoveryCheckpoint::start(
        device_state.identity(),
        expected_account_id,
        conversation_id,
        expected_source_device_id,
        sas,
        approved_range_start,
        approved_event_count,
        page_size,
    )?;
    let mut checkpoint = load_latest_history_recovery_checkpoint(&state_dir, initial_checkpoint)?;
    println!("history_recovery_id={}", checkpoint.recovery_id()?);
    println!("source_device_id={expected_source_device_id}");
    println!(
        "history_recovery_approved_range={}-{}",
        checkpoint.approved_range_start(),
        checkpoint.approved_range_end()
    );
    println!(
        "history_recovery_next_range_start={}",
        checkpoint.next_range_start()
    );
    if let (Some(count), Some(digest)) = (
        checkpoint.inventory_event_count(),
        checkpoint.inventory_digest(),
    ) {
        println!("source_inventory_event_count={count}");
        println!("source_inventory_digest={}", encode_hex(digest));
    }
    if checkpoint.is_complete() {
        println!("history_recovery_complete=true");
        println!("status=history-recovery-complete");
        return Ok(());
    }

    let membership = trust
        .load_conversation_membership(conversation_id.scope_id())
        .context("load trusted conversation membership before history recovery")?;
    membership
        .require_member(expected_account_id)
        .context("account is not a member of the requested conversation")?;
    let source_snapshot_store =
        install_own_authority_primary(&state_dir, &device_state, bootstrap.authority_snapshot())
            .context("install same-account authority snapshot from recovery source ticket")?;
    let session_binding =
        SyncSessionBinding::from_transport_label(&bootstrap.endpoint.id.to_string());
    let route_policy = bootstrap.route_policy;
    let endpoint = endpoint_builder_for_remote(route_policy, &bootstrap.endpoint)?
        .bind()
        .await
        .context("bind history recovery recipient endpoint")?;
    println!("transport_endpoint_id={}", endpoint.id());
    println!("account_id={expected_account_id}");
    println!("device_id={}", recipient_certificate.device_id());
    println!("source_authority_store={source_snapshot_store:?}");
    println!("route_policy={}", route_policy.as_str());
    print_endpoint_target(&bootstrap.endpoint);
    if route_policy == RoutePolicy::RelayOnly {
        wait_for_relay(&endpoint, route_policy, CLIENT_RELAY_WAIT_SECONDS).await?;
    }
    let connection = timeout(
        CONNECTION_TIMEOUT,
        endpoint.connect(bootstrap.endpoint.clone(), ALPN),
    )
    .await
    .with_context(|| {
        timeout_message(
            "connect to listening endpoint for history recovery",
            CONNECTION_TIMEOUT,
        )
    })?
    .context("connect to listening endpoint for history recovery")?;
    println!("peer_id={}", connection.remote_id());
    let ready_path = await_route_policy(&connection, route_policy, ROUTE_POLICY_WAIT)
        .await
        .context("wait for a history recovery path allowed by the connection ticket")?;
    print_ready_path(&ready_path);
    authorize_with_listener(
        &connection,
        device_state.identity(),
        recipient_certificate,
        recipient_authority_snapshot,
        session_binding,
    )
    .await?;

    let mut pages_completed = 0_usize;
    while pages_completed < max_pages && !checkpoint.is_complete() {
        let next_range_start = usize::try_from(checkpoint.next_range_start())
            .context("history recovery next range cannot be represented on this platform")?;
        let approved_range_end = usize::try_from(checkpoint.approved_range_end())
            .context("history recovery approved range cannot be represented on this platform")?;
        let request_count = page_size.min(approved_range_end - next_range_start);
        let (transfer, transfer_encoded) = request_history_rewrap_transfer(
            &connection,
            device_state.identity(),
            conversation_id,
            expected_source_device_id,
            session_binding,
            next_range_start,
            request_count,
            sas,
        )
        .await?;
        let bundle = transfer.bundle().clone();
        let bundle_encoded = bundle.encode()?;
        let advanced_checkpoint = checkpoint
            .advance(device_state.identity(), bundle.manifest())
            .context("advance signed history recovery checkpoint")?;
        let checkpoint_encoded = advanced_checkpoint.encode()?;
        println!("history_rewrap_transfer_bytes={}", transfer_encoded.len());
        import_history_rewrap_material(
            state_dir.clone(),
            conversation.clone(),
            bundle,
            bundle_encoded,
            Some((transfer, transfer_encoded)),
            Some((advanced_checkpoint.clone(), checkpoint_encoded)),
            "history-recovery-page-imported",
        )?;
        checkpoint = advanced_checkpoint;
        pages_completed += 1;
        println!("history_recovery_session_page_completed={pages_completed}");
    }

    print_transport_diagnostics(&connection, route_policy).await?;
    connection.close(0_u32.into(), b"kilogram history recovery session complete");
    endpoint.close().await;
    println!("history_recovery_session_pages_completed={pages_completed}");
    println!(
        "history_recovery_session_complete={}",
        checkpoint.is_complete()
    );
    println!(
        "status={}",
        if checkpoint.is_complete() {
            "history-recovery-complete"
        } else {
            "history-recovery-session-paused"
        }
    );
    Ok(())
}

fn load_latest_history_recovery_checkpoint(
    state_dir: &Path,
    initial: SignedHistoryRecoveryCheckpoint,
) -> Result<SignedHistoryRecoveryCheckpoint> {
    let recovery_id = initial.recovery_id()?;
    let directory = state_dir.join(HISTORY_RECOVERY_STORE_DIRECTORY);
    let mut checkpoints = match fs::read_dir(&directory) {
        Ok(entries) => entries
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::result::Result<Vec<_>, _>>()?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    checkpoints
        .retain(|path| path.extension().and_then(|value| value.to_str()) == Some("checkpoint"));
    checkpoints.sort();

    let mut matching = Vec::new();
    for path in checkpoints {
        let checkpoint = SignedHistoryRecoveryCheckpoint::decode_and_verify(
            &fs::read(&path).with_context(|| format!("read {}", path.display()))?,
        )
        .with_context(|| format!("verify history recovery checkpoint {}", path.display()))?;
        if checkpoint.recovery_id()? == recovery_id {
            matching.push(checkpoint);
        }
    }
    matching.sort_by_key(SignedHistoryRecoveryCheckpoint::next_range_start);

    let mut current = initial;
    for checkpoint in matching {
        ensure!(
            checkpoint.next_range_start() > current.next_range_start(),
            "history recovery checkpoint chain contains duplicate or regressing progress"
        );
        ensure!(
            checkpoint.previous_checkpoint_id() == Some(&current.checkpoint_id()?),
            "history recovery checkpoint chain is forked or missing an intermediate page"
        );
        current = checkpoint;
    }
    Ok(current)
}

fn export_history_rewrap(
    state_dir: PathBuf,
    conversation: String,
    device_list_file: PathBuf,
    recipient_device_id: DeviceId,
    range_start: usize,
    count: usize,
    bundle_file: PathBuf,
) -> Result<()> {
    ensure!(count > 0, "--count must be greater than zero");
    ensure!(
        count <= MAX_HISTORY_REWRAP_ENTRIES,
        "--count must not exceed {MAX_HISTORY_REWRAP_ENTRIES}"
    );
    let device_state = load_command_device_state(&state_dir)?;
    let trust = CommandTrustReadRepository::open(&state_dir, &device_state)?;
    let read_repositories = open_immutable_read_repositories(&state_dir)
        .context("capture immutable vault-primary source history before export state changes")?;
    let source_certificate = trust
        .load_certificate()
        .context("load source device certificate before history rewrap")?;
    let device_list = AccountDeviceListSnapshot::decode_and_verify(
        &fs::read(&device_list_file)
            .with_context(|| format!("read device list from {}", device_list_file.display()))?,
    )
    .context("decode and verify history-rewrap account device list")?;
    device_list
        .verify_for_account(source_certificate.account_id())
        .context("history-rewrap device list belongs to a different account")?;
    ensure!(
        device_list.certificate_for(source_certificate.device_id()) == Some(&source_certificate),
        "source certificate is not present exactly in the supplied device list"
    );
    ensure!(
        device_list.certificate_for(recipient_device_id).is_some(),
        "recipient device is absent from the supplied root-signed device list"
    );
    let conversation_id = ConversationId::from_label(&conversation);
    let membership = trust
        .load_conversation_membership(conversation_id.scope_id())
        .context("load DB-primary membership before history rewrap")?;
    let snapshot_store =
        install_own_authority_primary(&state_dir, &device_state, device_list.authority_snapshot())
            .context("install authority snapshot from history-rewrap device list")?;
    let bundle = build_history_rewrap_bundle(
        &device_state,
        &source_certificate,
        device_list,
        recipient_device_id,
        conversation_id,
        &membership,
        read_repositories.events.as_ref(),
        read_repositories.local_messages.as_ref(),
        range_start,
        count,
    )
    .context("build authenticated history-rewrap bundle from immutable source snapshot")?;
    let encoded = bundle.encode()?;
    write_new_authority_file(&bundle_file, &encoded)
        .with_context(|| format!("write history rewrap to {}", bundle_file.display()))?;
    println!("history_rewrap_id={}", bundle.bundle_id()?);
    println!("account_id={}", bundle.manifest().account_id());
    println!("source_device_id={}", bundle.manifest().source_device_id());
    println!(
        "recipient_device_id={}",
        bundle.manifest().recipient_device_id()
    );
    println!(
        "source_inventory_event_count={}",
        bundle.manifest().inventory_event_count()
    );
    println!(
        "source_inventory_digest={}",
        encode_hex(bundle.manifest().inventory_digest())
    );
    println!("range_start={}", bundle.manifest().range_start());
    println!("range_end={}", bundle.manifest().range_end());
    println!("rewrapped_event_count={}", bundle.entries().len());
    println!(
        "source_inventory_complete={}",
        bundle.is_complete_source_inventory()
    );
    println!("authority_snapshot_store={snapshot_store:?}");
    print_immutable_read_diagnostics("history_rewrap", &read_repositories);
    println!("bundle_file={}", bundle_file.display());
    println!("status=history-rewrap-exported");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_history_rewrap_bundle(
    device_state: &DeviceState,
    source_certificate: &DeviceCertificate,
    device_list: AccountDeviceListSnapshot,
    recipient_device_id: DeviceId,
    conversation_id: ConversationId,
    membership: &ConversationMembershipSnapshot,
    event_reads: &dyn EventReadRepository,
    local_message_reads: &dyn LocalMessageReadRepository,
    range_start: usize,
    count: usize,
) -> Result<HistoryRewrapBundle> {
    ensure!(count > 0, "history-rewrap count must be greater than zero");
    ensure!(
        count <= MAX_HISTORY_REWRAP_ENTRIES,
        "history-rewrap count must not exceed {MAX_HISTORY_REWRAP_ENTRIES}"
    );
    device_list
        .verify_for_account(source_certificate.account_id())
        .context("history-rewrap device list belongs to a different account")?;
    ensure!(
        device_list.certificate_for(source_certificate.device_id()) == Some(source_certificate),
        "source certificate is not present exactly in the signed device list"
    );
    ensure!(
        device_list.certificate_for(recipient_device_id).is_some(),
        "recipient device is absent from the supplied root-signed device list"
    );
    membership
        .require_member(source_certificate.account_id())
        .context("source account is not a member of this conversation")?;
    let stored_events = event_reads
        .load_authorized_conversation(conversation_id, membership)
        .context("load and verify source history before rewrap")?;
    let mut inventory = Vec::new();
    for stored in stored_events {
        if !matches!(
            stored.event.event().payload(),
            EventPayload::RatchetText { .. }
        ) {
            continue;
        }
        let projection = local_message_reads
            .get(stored.id)
            .with_context(|| format!("load readable source projection for event {}", stored.id))?;
        let body = projection
            .open_for_account(
                stored.event.event(),
                source_certificate.device_id(),
                source_certificate.account_id(),
                device_state.encryption(),
            )
            .with_context(|| format!("open source projection for event {}", stored.id))?;
        inventory.push((stored.event, body));
    }
    ensure!(
        range_start < inventory.len(),
        "history-rewrap range start {range_start} is outside text inventory with {} events",
        inventory.len()
    );
    let requested_end = range_start
        .checked_add(count)
        .context("history-rewrap range overflow")?;
    let range_end = requested_end.min(inventory.len());
    HistoryRewrapBundle::seal(
        device_state.identity(),
        device_list,
        recipient_device_id,
        conversation_id,
        &inventory,
        range_start,
        range_end,
    )
    .context("seal authenticated history-rewrap bundle")
}

fn import_history_rewrap(
    state_dir: PathBuf,
    conversation: String,
    bundle_file: PathBuf,
) -> Result<()> {
    let encoded = fs::read(&bundle_file)
        .with_context(|| format!("read history rewrap from {}", bundle_file.display()))?;
    let bundle = HistoryRewrapBundle::decode_and_verify(&encoded)
        .context("decode and verify authenticated history-rewrap bundle")?;
    import_history_rewrap_material(
        state_dir,
        conversation,
        bundle,
        encoded,
        None,
        None,
        "history-rewrap-imported",
    )
}

fn import_history_rewrap_material(
    state_dir: PathBuf,
    conversation: String,
    bundle: HistoryRewrapBundle,
    encoded: Vec<u8>,
    transfer: Option<(SignedHistoryRewrapTransfer, Vec<u8>)>,
    recovery_checkpoint: Option<(SignedHistoryRecoveryCheckpoint, Vec<u8>)>,
    final_status: &str,
) -> Result<()> {
    let device_state = load_command_device_state(&state_dir)?;
    let trust = CommandTrustReadRepository::open(&state_dir, &device_state)?;
    let recipient_certificate = trust
        .load_certificate()
        .context("load recipient device certificate before history rewrap")?;
    let conversation_id = ConversationId::from_label(&conversation);
    ensure!(
        bundle.manifest().conversation_id() == conversation_id,
        "history-rewrap bundle belongs to a different conversation"
    );
    ensure!(
        bundle.manifest().recipient_device_id() == recipient_certificate.device_id(),
        "history-rewrap bundle is addressed to a different device"
    );
    ensure!(
        bundle.manifest().account_id() == recipient_certificate.account_id(),
        "history-rewrap bundle belongs to a different account"
    );
    ensure!(
        bundle
            .manifest()
            .account_device_list()
            .certificate_for(recipient_certificate.device_id())
            == Some(&recipient_certificate),
        "recipient certificate is not present exactly in the signed device list"
    );
    let snapshot_store = install_own_authority_primary(
        &state_dir,
        &device_state,
        bundle.manifest().account_device_list().authority_snapshot(),
    )
    .context("install authority snapshot from history-rewrap bundle")?;
    let membership = trust
        .load_conversation_membership(conversation_id.scope_id())
        .context("load trusted conversation membership before history rewrap import")?;
    membership
        .require_member(recipient_certificate.account_id())
        .context("recipient account is not a member of this conversation")?;
    let event_store = open_event_store(&state_dir)?;
    let local_message_store = open_local_message_store(&state_dir)?;
    let mut prepared = Vec::with_capacity(bundle.entries().len());
    for entry_offset in 0..bundle.entries().len() {
        let (authorized_event, body) = bundle
            .open_entry(
                entry_offset,
                recipient_certificate.device_id(),
                device_state.encryption(),
            )
            .with_context(|| format!("open history-rewrap entry {entry_offset}"))?;
        authorized_event
            .verify_for_membership(&membership)
            .with_context(|| format!("authorize history-rewrap entry {entry_offset}"))?;
        let projection = LocalTextProjection::from_history_rewrap(
            authorized_event.event(),
            &bundle,
            entry_offset,
            recipient_certificate.device_id(),
            recipient_certificate.account_id(),
            device_state.encryption(),
        )?;
        let event_id = authorized_event.event().event_id()?;
        let projection_exists = match local_message_store.get(event_id) {
            Ok(existing) => {
                let existing_body = existing.open_for_account(
                    authorized_event.event(),
                    recipient_certificate.device_id(),
                    recipient_certificate.account_id(),
                    device_state.encryption(),
                )?;
                ensure!(
                    existing_body == body,
                    "existing local projection conflicts with history-rewrap event {event_id}"
                );
                true
            }
            Err(StoreError::LocalTextProjectionMissing { .. }) => false,
            Err(error) => return Err(error.into()),
        };
        prepared.push((authorized_event, projection, projection_exists));
    }
    let (bundle_store, transfer_store, checkpoint_store, inserted_projections, inserted_events) =
        run_state_transaction(&state_dir, |transaction| {
            let (bundle_store, bundle_receipt) =
                persist_history_rewrap_bundle(&state_dir, &bundle, &encoded)?;
            transaction.register_store_receipt(&bundle_receipt)?;
            let transfer_store = if let Some((transfer, encoded)) = transfer.as_ref() {
                let (outcome, receipt) =
                    persist_history_rewrap_transfer(&state_dir, transfer, encoded)?;
                transaction.register_store_receipt(&receipt)?;
                Some(outcome)
            } else {
                None
            };
            let checkpoint_store = if let Some((checkpoint, encoded)) = recovery_checkpoint.as_ref()
            {
                let (outcome, receipt) =
                    persist_history_recovery_checkpoint(&state_dir, checkpoint, encoded)?;
                transaction.register_store_receipt(&receipt)?;
                Some(outcome)
            } else {
                None
            };
            let mut inserted_projections = 0_usize;
            let mut inserted_events = 0_usize;
            for (authorized_event, projection, projection_exists) in prepared {
                if !projection_exists && {
                    let (outcome, receipt) = local_message_store.put_with_receipt(&projection)?;
                    transaction.register_store_receipt(&receipt)?;
                    outcome == StoreOutcome::Inserted
                } {
                    inserted_projections += 1;
                }
                let (outcome, receipt) =
                    event_store.put_authorized_with_receipt(&authorized_event, &membership)?;
                transaction.register_store_receipt(&receipt)?;
                if outcome == StoreOutcome::Inserted {
                    inserted_events += 1;
                }
            }
            Ok((
                bundle_store,
                transfer_store,
                checkpoint_store,
                inserted_projections,
                inserted_events,
            ))
        })?;
    println!("history_rewrap_id={}", bundle.bundle_id()?);
    println!("source_device_id={}", bundle.manifest().source_device_id());
    println!(
        "recipient_device_id={}",
        bundle.manifest().recipient_device_id()
    );
    println!(
        "source_inventory_event_count={}",
        bundle.manifest().inventory_event_count()
    );
    println!(
        "source_inventory_digest={}",
        encode_hex(bundle.manifest().inventory_digest())
    );
    println!("range_start={}", bundle.manifest().range_start());
    println!("range_end={}", bundle.manifest().range_end());
    println!("rewrapped_event_count={}", bundle.entries().len());
    println!("inserted_event_count={inserted_events}");
    println!("inserted_projection_count={inserted_projections}");
    println!(
        "source_inventory_complete={}",
        bundle.is_complete_source_inventory()
    );
    println!("authority_snapshot_store={snapshot_store:?}");
    println!("bundle_store={bundle_store:?}");
    if let Some(transfer_store) = transfer_store {
        println!("transfer_store={transfer_store:?}");
    }
    if let Some((checkpoint, _)) = recovery_checkpoint {
        println!("history_recovery_id={}", checkpoint.recovery_id()?);
        println!(
            "history_recovery_next_range_start={}",
            checkpoint.next_range_start()
        );
        println!("history_recovery_complete={}", checkpoint.is_complete());
    }
    if let Some(checkpoint_store) = checkpoint_store {
        println!("recovery_checkpoint_store={checkpoint_store:?}");
    }
    println!("status={final_status}");
    Ok(())
}

fn persist_history_rewrap_bundle(
    state_dir: &Path,
    bundle: &HistoryRewrapBundle,
    encoded: &[u8],
) -> Result<(StoreOutcome, AppendOnlyWriteReceipt)> {
    let directory = state_dir.join(HISTORY_REWRAP_STORE_DIRECTORY);
    fs::create_dir_all(&directory)?;
    let directory = fs::canonicalize(directory)?;
    let path = directory.join(format!("{}.rewrap", bundle.bundle_id()?));
    let outcome = if path.try_exists()? {
        validate_existing_history_rewrap(&path, encoded)?
    } else {
        let mut temporary = NamedTempFile::new_in(&directory)?;
        temporary.write_all(encoded)?;
        temporary.as_file().sync_all()?;
        match temporary.persist_noclobber(&path) {
            Ok(file) => {
                file.sync_all()?;
                StoreOutcome::Inserted
            }
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                validate_existing_history_rewrap(&path, encoded)?
            }
            Err(error) => return Err(error.error.into()),
        }
    };
    Ok((outcome, AppendOnlyWriteReceipt::single(path)))
}

fn validate_existing_history_rewrap(path: &Path, expected: &[u8]) -> Result<StoreOutcome> {
    let existing = fs::read(path)?;
    ensure!(
        existing == expected,
        "stored history-rewrap bundle conflicts with immutable file {}",
        path.display()
    );
    Ok(StoreOutcome::AlreadyPresent)
}

fn persist_history_rewrap_transfer(
    state_dir: &Path,
    transfer: &SignedHistoryRewrapTransfer,
    encoded: &[u8],
) -> Result<(StoreOutcome, AppendOnlyWriteReceipt)> {
    transfer.verify_signature()?;
    let directory = state_dir.join(HISTORY_REWRAP_STORE_DIRECTORY);
    fs::create_dir_all(&directory)?;
    let directory = fs::canonicalize(directory)?;
    let path = directory.join(format!("{}.transfer", transfer.bundle().bundle_id()?));
    let outcome = if path.try_exists()? {
        validate_existing_history_rewrap(&path, encoded)?
    } else {
        let mut temporary = NamedTempFile::new_in(&directory)?;
        temporary.write_all(encoded)?;
        temporary.as_file().sync_all()?;
        match temporary.persist_noclobber(&path) {
            Ok(file) => {
                file.sync_all()?;
                StoreOutcome::Inserted
            }
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                validate_existing_history_rewrap(&path, encoded)?
            }
            Err(error) => return Err(error.error.into()),
        }
    };
    Ok((outcome, AppendOnlyWriteReceipt::single(path)))
}

fn persist_history_recovery_checkpoint(
    state_dir: &Path,
    checkpoint: &SignedHistoryRecoveryCheckpoint,
    encoded: &[u8],
) -> Result<(StoreOutcome, AppendOnlyWriteReceipt)> {
    checkpoint.verify_signature()?;
    let directory = state_dir.join(HISTORY_RECOVERY_STORE_DIRECTORY);
    fs::create_dir_all(&directory)?;
    let directory = fs::canonicalize(directory)?;
    let checkpoint_id = checkpoint.checkpoint_id()?;
    let path = directory.join(format!(
        "{}-{:020}-{}.checkpoint",
        checkpoint.recovery_id()?,
        checkpoint.next_range_start(),
        encode_hex(&checkpoint_id)
    ));
    let outcome = if path.try_exists()? {
        validate_existing_history_rewrap(&path, encoded)?
    } else {
        let mut temporary = NamedTempFile::new_in(&directory)?;
        temporary.write_all(encoded)?;
        temporary.as_file().sync_all()?;
        match temporary.persist_noclobber(&path) {
            Ok(file) => {
                file.sync_all()?;
                StoreOutcome::Inserted
            }
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                validate_existing_history_rewrap(&path, encoded)?
            }
            Err(error) => return Err(error.error.into()),
        }
    };
    Ok((outcome, AppendOnlyWriteReceipt::single(path)))
}

#[derive(Debug, Default)]
struct HistoryRewrapClaimCoverage {
    ranges: Vec<(u64, u64)>,
    bundle_count: usize,
}

fn reconcile_history_rewrap(state_dir: PathBuf, conversation: String) -> Result<()> {
    let device_state = load_command_device_state(&state_dir)?;
    let trust = CommandTrustReadRepository::open(&state_dir, &device_state)?;
    let recipient_certificate = trust
        .load_certificate()
        .context("load recipient certificate for history-rewrap reconciliation")?;
    let conversation_id = ConversationId::from_label(&conversation);
    let directory = state_dir.join(HISTORY_REWRAP_STORE_DIRECTORY);
    let mut paths = match fs::read_dir(&directory) {
        Ok(entries) => entries
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::result::Result<Vec<_>, _>>()?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    paths.retain(|path| path.extension().and_then(|value| value.to_str()) == Some("rewrap"));
    paths.sort();

    type ClaimKey = (DeviceId, u64, [u8; 32]);
    let mut claims = BTreeMap::<ClaimKey, HistoryRewrapClaimCoverage>::new();
    let mut source_claims = BTreeMap::<DeviceId, BTreeSet<(u64, [u8; 32])>>::new();
    let mut covered_event_ids = BTreeSet::new();
    let mut matched_bundle_count = 0_usize;
    for path in &paths {
        let bundle = HistoryRewrapBundle::decode_and_verify(
            &fs::read(path).with_context(|| format!("read {}", path.display()))?,
        )
        .with_context(|| format!("verify stored history rewrap {}", path.display()))?;
        let manifest = bundle.manifest();
        if manifest.conversation_id() != conversation_id {
            continue;
        }
        matched_bundle_count += 1;
        ensure!(
            manifest.account_id() == recipient_certificate.account_id(),
            "stored history rewrap {} belongs to a different account",
            path.display()
        );
        ensure!(
            manifest.recipient_device_id() == recipient_certificate.device_id(),
            "stored history rewrap {} is addressed to a different device",
            path.display()
        );
        ensure!(
            manifest
                .account_device_list()
                .certificate_for(recipient_certificate.device_id())
                == Some(&recipient_certificate),
            "stored history rewrap {} contains a different recipient certificate",
            path.display()
        );
        let digest = *manifest.inventory_digest();
        let key = (
            manifest.source_device_id(),
            manifest.inventory_event_count(),
            digest,
        );
        let coverage = claims.entry(key).or_default();
        coverage
            .ranges
            .push((manifest.range_start(), manifest.range_end()));
        coverage.bundle_count += 1;
        source_claims
            .entry(manifest.source_device_id())
            .or_default()
            .insert((manifest.inventory_event_count(), digest));
        for entry in bundle.entries() {
            covered_event_ids.insert(entry.event().event().event_id()?);
        }
    }

    let mut complete_claims = Vec::new();
    let mut complete_sources = BTreeSet::new();
    for ((source, count, digest), coverage) in &mut claims {
        coverage.ranges.sort_unstable();
        let complete = ranges_cover_inventory(&coverage.ranges, *count);
        if complete {
            complete_claims.push((*source, *count, *digest));
            complete_sources.insert(*source);
        }
        println!(
            "claim_source_device_id={source} inventory_event_count={count} inventory_digest={} bundle_count={} range_count={} complete={complete}",
            encode_hex(digest),
            coverage.bundle_count,
            coverage.ranges.len()
        );
    }
    let equivocation_count = source_claims
        .values()
        .filter(|claims| claims.len() > 1)
        .count();
    let agreement = classify_history_rewrap_agreement(&complete_claims, equivocation_count);
    let selected_inventory = (agreement == "agreed")
        .then(|| {
            complete_claims
                .first()
                .map(|(_, count, digest)| (*count, *digest))
        })
        .flatten();
    println!("conversation_id={conversation_id}");
    println!("recipient_device_id={}", recipient_certificate.device_id());
    println!("rewrap_bundle_count={matched_bundle_count}");
    println!("source_device_count={}", source_claims.len());
    println!("source_claim_count={}", claims.len());
    println!("complete_source_count={}", complete_sources.len());
    println!("source_equivocation_count={equivocation_count}");
    println!("covered_event_count={}", covered_event_ids.len());
    println!("source_claim_agreement={agreement}");
    if let Some((count, digest)) = selected_inventory {
        println!("inventory_selection=agreed");
        println!("selected_inventory_event_count={count}");
        println!("selected_inventory_digest={}", encode_hex(&digest));
        println!("selected_inventory_source_count={}", complete_sources.len());
    } else {
        println!("inventory_selection=none");
    }
    println!("global_completeness_proven=false");
    println!("status=history-rewrap-reconciled");
    Ok(())
}

fn ranges_cover_inventory(ranges: &[(u64, u64)], inventory_event_count: u64) -> bool {
    if inventory_event_count == 0 {
        return false;
    }
    let mut covered_until = 0_u64;
    for &(start, end) in ranges {
        if start > covered_until {
            return false;
        }
        covered_until = covered_until.max(end);
        if covered_until >= inventory_event_count {
            return true;
        }
    }
    false
}

fn classify_history_rewrap_agreement(
    complete_claims: &[(DeviceId, u64, [u8; 32])],
    equivocation_count: usize,
) -> &'static str {
    if complete_claims.is_empty() {
        return "incomplete";
    }
    let sources = complete_claims
        .iter()
        .map(|(source, _, _)| *source)
        .collect::<BTreeSet<_>>();
    let inventories = complete_claims
        .iter()
        .map(|(_, count, digest)| (*count, *digest))
        .collect::<BTreeSet<_>>();
    if inventories.len() > 1 || equivocation_count > 0 {
        "divergent"
    } else if sources.len() == 1 {
        "single-source"
    } else {
        "agreed"
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn seed_history(
    state_dir: PathBuf,
    conversation: String,
    count: usize,
    message_prefix: String,
    peer_certificate_file: PathBuf,
    peer_device_list_file: PathBuf,
    peer_prekey_pool_file: PathBuf,
) -> Result<()> {
    ensure!(count > 0, "--count must be greater than zero");
    let device_state = load_command_device_state(&state_dir)?;
    let trust = CommandTrustReadRepository::open(&state_dir, &device_state)?;
    let certificate = trust
        .load_certificate()
        .context("load device certificate before seeding history")?;
    let authority_snapshot = trust
        .load_own_authority_snapshot(&certificate)
        .context("load device authority snapshot before seeding history")?;
    let peer_certificate = DeviceCertificate::decode_and_verify(
        &fs::read(&peer_certificate_file).with_context(|| {
            format!(
                "read peer certificate from {}",
                peer_certificate_file.display()
            )
        })?,
    )
    .context("decode and verify peer device certificate")?;
    ensure!(
        peer_certificate.device_id() != certificate.device_id(),
        "peer certificate belongs to this same device"
    );
    let peer_device_list = AccountDeviceListSnapshot::decode_and_verify(
        &fs::read(&peer_device_list_file).with_context(|| {
            format!(
                "read peer device list from {}",
                peer_device_list_file.display()
            )
        })?,
    )
    .context("decode and verify peer account device list")?;
    ensure!(
        peer_device_list.devices() == std::slice::from_ref(&peer_certificate),
        "development seed-history requires a single-device list matching the peer certificate"
    );
    let peer_prekey_pool =
        SignedPrekeyPool::decode(&fs::read(&peer_prekey_pool_file).with_context(|| {
            format!(
                "read peer prekey pool from {}",
                peer_prekey_pool_file.display()
            )
        })?)
        .context("decode and verify peer prekey pool")?;
    let now_unix_seconds = unix_time_now().context("read time for peer prekey freshness")?;
    peer_prekey_pool
        .verify_at(now_unix_seconds)
        .context("verify peer prekey pool freshness")?;
    ensure!(
        peer_prekey_pool.device_id() == peer_certificate.device_id(),
        "peer prekey pool belongs to a different device than the peer certificate"
    );
    let conversation_id = ConversationId::from_label(&conversation);
    let membership = trust
        .load_conversation_membership(conversation_id.scope_id())
        .context("load DB-primary membership before seeding history")?;
    let (conversation_id, existing_count, first_event_id, last_event_id) = run_state_transaction(
        &state_dir,
        |transaction| {
            let mut ratchet_state = transaction.load_ratchet_state()?;
            ratchet_state
                .observe_prekey_pool(&peer_prekey_pool, now_unix_seconds)
                .context("observe peer prekey pool before allocating seeded events")?;
            let event_store = open_event_store(&state_dir)?;
            let local_message_store = open_local_message_store(&state_dir)?;
            membership
                .require_member(certificate.account_id())
                .context("this device account is not a member of the conversation")?;
            membership
                .require_member(peer_certificate.account_id())
                .context("peer device account is not a member of the conversation")?;
            let existing_count = event_store
                .authorized_inventory(conversation_id, &membership)
                .context("load current authorized inventory before seeding history")?
                .len();
            ensure!(
                existing_count.saturating_add(count) <= MAX_INVENTORY_EVENT_IDS,
                "seeded history would exceed the M0 inventory limit of {MAX_INVENTORY_EVENT_IDS} events"
            );

            let mut parents = event_store
                .frontier(conversation_id)
                .context("calculate initial frontier for seeded history")?;
            let mut events = Vec::with_capacity(count);
            let mut local_projections = Vec::with_capacity(count);
            let mut first_event_id = None;
            let mut last_event_id = None;
            for index in 1..=count {
                let author_sequence = transaction
                    .allocate_sequence(&device_state)
                    .context("allocate seeded event sequence")?;
                let body = format!("{message_prefix}-{index}");
                let (sender_ratchet_identity, ciphertext, _) = ratchet_state
                    .encrypt_with_pool(
                        device_state.identity(),
                        &peer_prekey_pool,
                        &body,
                        now_unix_seconds,
                    )
                    .context("advance and persist seeded ratchet message")?;
                let event = SignedEvent::sign_ratchet_text(
                    device_state.identity(),
                    conversation_id,
                    author_sequence,
                    parents,
                    peer_device_list.clone(),
                    sender_ratchet_identity,
                    vec![RatchetRecipient::new(
                        peer_certificate.device_id(),
                        ciphertext,
                    )?],
                )
                .context("sign seeded ratchet history event")?;
                let local_projection = LocalTextProjection::seal_authored(
                    &event,
                    device_state.identity().device_id(),
                    certificate.encryption_public_key(),
                    &body,
                )
                .context("encrypt seeded text into the local history projection")?;
                let event_id = event.event_id().context("calculate seeded event ID")?;
                first_event_id.get_or_insert(event_id);
                last_event_id = Some(event_id);
                parents = vec![event_id];
                events.push(
                    AuthorizedEvent::new(event, certificate.clone(), authority_snapshot.clone())
                        .context("attach Account Root authorization to seeded event")?,
                );
                local_projections.push(local_projection);
            }
            for projection in &local_projections {
                let (_, receipt) = local_message_store
                    .put_with_receipt(projection)
                    .context("persist seeded local history projection")?;
                transaction.register_store_receipt(&receipt)?;
            }
            let receipt = event_store
                .put_authorized_batch_with_receipt(&events, &membership)
                .context("persist authorized seeded history events")?;
            transaction.register_store_receipt(&receipt)?;
            Ok((
                conversation_id,
                existing_count,
                first_event_id,
                last_event_id,
            ))
        },
    )?;

    println!("conversation_id={conversation_id}");
    println!("device_id={}", device_state.identity().device_id());
    println!("seeded_event_count={count}");
    if let Some(event_id) = first_event_id {
        println!("seeded_first_event_id={event_id}");
    }
    if let Some(event_id) = last_event_id {
        println!("seeded_last_event_id={event_id}");
    }
    println!("event_count={}", existing_count + count);
    println!("status=seeded");
    Ok(())
}

fn open_event_store(state_dir: &Path) -> Result<EventStore> {
    let path = state_dir.join(EVENT_STORE_DIRECTORY);
    EventStore::open(&path).with_context(|| format!("open event store at {}", path.display()))
}

fn open_local_message_store(state_dir: &Path) -> Result<LocalMessageStore> {
    let path = state_dir.join(LOCAL_MESSAGE_STORE_DIRECTORY);
    LocalMessageStore::open(&path)
        .with_context(|| format!("open local message store at {}", path.display()))
}

fn ensure_authored_local_text_projection(
    store: &LocalMessageStore,
    device_state: &DeviceState,
    local_account_id: AccountId,
    event: &SignedEvent,
    body: &str,
) -> Result<(StoreOutcome, AppendOnlyWriteReceipt), StoreError> {
    let event_id = event.event_id()?;
    let local_device_id = device_state.identity().device_id();
    match store.get(event_id) {
        Ok(projection) => {
            let stored_body = projection.open_for_account(
                event,
                local_device_id,
                local_account_id,
                device_state.encryption(),
            )?;
            if stored_body != body {
                return Err(StoreError::LocalTextProjectionPlaintextConflict { event_id });
            }
            store.put_with_receipt(&projection)
        }
        Err(StoreError::LocalTextProjectionMissing { .. }) => {
            let projection = LocalTextProjection::seal_authored(
                event,
                local_device_id,
                device_state.encryption().public_key(),
                body,
            )?;
            store.put_with_receipt(&projection)
        }
        Err(error) => Err(error),
    }
}

fn open_local_text_projection_if_present(
    store: &LocalMessageStore,
    device_state: &DeviceState,
    local_account_id: AccountId,
    event: &SignedEvent,
) -> Result<Option<String>, StoreError> {
    let event_id = event.event_id()?;
    match store.get(event_id) {
        Ok(projection) => Ok(Some(projection.open_for_account(
            event,
            device_state.identity().device_id(),
            local_account_id,
            device_state.encryption(),
        )?)),
        Err(StoreError::LocalTextProjectionMissing { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

fn ensure_received_local_text_projection(
    store: &LocalMessageStore,
    device_state: &DeviceState,
    local_account_id: AccountId,
    event: &SignedEvent,
    decrypted: &DecryptedMessage,
) -> Result<(StoreOutcome, AppendOnlyWriteReceipt), StoreError> {
    let event_id = event.event_id()?;
    if let Some(body) =
        open_local_text_projection_if_present(store, device_state, local_account_id, event)?
    {
        if body != decrypted.as_str() {
            return Err(StoreError::LocalTextProjectionPlaintextConflict { event_id });
        }
        let projection = store.get(event_id)?;
        return store.put_with_receipt(&projection);
    }
    let projection = LocalTextProjection::seal_received(
        event,
        device_state.identity().device_id(),
        device_state.encryption().public_key(),
        decrypted,
    )?;
    store.put_with_receipt(&projection)
}

struct DecryptingSessionStore<'a> {
    state_directory: PathBuf,
    event_writes: &'a EventStore,
    local_message_writes: &'a LocalMessageStore,
    event_reads: CommandEventReadOverlay,
    local_message_reads: CommandLocalMessageReadOverlay,
    device_state: &'a DeviceState,
    local_account_id: AccountId,
}

impl<'a> DecryptingSessionStore<'a> {
    fn new(
        state_directory: impl AsRef<Path>,
        store: &'a EventStore,
        local_messages: &'a LocalMessageStore,
        device_state: &'a DeviceState,
        local_account_id: AccountId,
        read_repositories: ImmutableReadRepositories,
    ) -> Self {
        let ImmutableReadRepositories {
            events,
            local_messages: local_message_reads,
            ..
        } = read_repositories;
        Self {
            state_directory: state_directory.as_ref().to_path_buf(),
            event_writes: store,
            local_message_writes: local_messages,
            event_reads: CommandEventReadOverlay::new(events),
            local_message_reads: CommandLocalMessageReadOverlay::new(local_message_reads),
            device_state,
            local_account_id,
        }
    }

    fn ensure_local_projections(
        &self,
        transaction: &mut CommandTransactionContext<'_>,
        ratchet_state: &mut RatchetState,
        events: &[AuthorizedEvent],
    ) -> Result<Vec<LocalTextProjection>, StoreError> {
        let mut text_events = events
            .iter()
            .filter(|event| matches!(event.event().payload(), EventPayload::RatchetText { .. }))
            .collect::<Vec<_>>();
        text_events.sort_by_key(|event| {
            (
                event.event().author_device_id(),
                event.event().author_sequence(),
            )
        });
        let mut created = Vec::new();
        for event in text_events {
            if let Some(projection) =
                self.ensure_local_projection(transaction, ratchet_state, event.event())?
            {
                created.push(projection);
            }
        }
        Ok(created)
    }

    fn ensure_local_projection(
        &self,
        transaction: &mut CommandTransactionContext<'_>,
        ratchet_state: &mut RatchetState,
        event: &SignedEvent,
    ) -> Result<Option<LocalTextProjection>, StoreError> {
        let event_id = event.event_id()?;
        let local_device_id = self.device_state.identity().device_id();
        match self.local_message_reads.get(event_id) {
            Ok(projection) => {
                projection.open_for_account(
                    event,
                    local_device_id,
                    self.local_account_id,
                    self.device_state.encryption(),
                )?;
                Ok(None)
            }
            Err(error @ StoreError::LocalTextProjectionMissing { .. }) => {
                if event.author_device_id() == local_device_id {
                    return Err(error);
                }
                let recipient_account_id = event.recipient_device_list()?.account_id();
                if recipient_account_id != self.local_account_id {
                    return Err(
                        kilogram_protocol::ProtocolError::RatchetRecipientAccountMismatch {
                            expected: self.local_account_id,
                            actual: recipient_account_id,
                        }
                        .into(),
                    );
                }
                if event.ratchet_message_for(local_device_id).is_err() {
                    return Err(
                        kilogram_protocol::ProtocolError::LocalProjectionDeviceNotParticipant(
                            local_device_id,
                        )
                        .into(),
                    );
                }
                let (sender_identity, ciphertext) = event.ratchet_message_for(local_device_id)?;
                let (decrypted, _) = ratchet_state
                    .decrypt(self.device_state.identity(), sender_identity, ciphertext)
                    .map_err(kilogram_protocol::ProtocolError::from)?;
                let projection = LocalTextProjection::seal_received(
                    event,
                    local_device_id,
                    self.device_state.encryption().public_key(),
                    &decrypted,
                )?;
                let (_, receipt) = self.local_message_writes.put_with_receipt(&projection)?;
                transaction.register_store_receipt_for_store(&receipt)?;
                Ok(Some(projection))
            }
            Err(error) => Err(error),
        }
    }

    fn overlay_event_count(&self) -> Result<usize, StoreError> {
        self.event_reads.committed_count()
    }

    fn overlay_local_projection_count(&self) -> Result<usize, StoreError> {
        self.local_message_reads.committed_count()
    }
}

impl SessionStore for DecryptingSessionStore<'_> {
    fn inventory(
        &self,
        conversation_id: ConversationId,
        membership: &ConversationMembershipSnapshot,
    ) -> Result<Vec<kilogram_protocol::EventId>, StoreError> {
        self.event_reads
            .authorized_inventory(conversation_id, membership)
    }

    fn events_by_id(
        &self,
        conversation_id: ConversationId,
        event_ids: &[kilogram_protocol::EventId],
        membership: &ConversationMembershipSnapshot,
    ) -> Result<Vec<AuthorizedEvent>, StoreError> {
        let events =
            self.event_reads
                .authorized_events_by_id(conversation_id, event_ids, membership)?;
        let staged_projections = run_store_transaction(&self.state_directory, |transaction| {
            let mut ratchet_state = transaction.load_ratchet_state_for_store()?;
            let created =
                self.ensure_local_projections(transaction, &mut ratchet_state, &events)?;
            self.local_message_reads.stage_committed(&created)
        })?;
        self.local_message_reads.commit_staged(staged_projections)?;
        Ok(events)
    }

    fn put_events(
        &self,
        events: &[AuthorizedEvent],
        membership: &ConversationMembershipSnapshot,
    ) -> Result<(), StoreError> {
        let staged_events = self.event_reads.stage_committed(events, membership)?;
        let staged_projections = run_store_transaction(&self.state_directory, |transaction| {
            for event in events {
                event.verify_for_membership(membership)?;
            }
            let mut ratchet_state = transaction.load_ratchet_state_for_store()?;
            let created = self.ensure_local_projections(transaction, &mut ratchet_state, events)?;
            let receipt = self
                .event_writes
                .put_authorized_batch_with_receipt(events, membership)?;
            transaction.register_store_receipt_for_store(&receipt)?;
            self.local_message_reads.stage_committed(&created)
        })?;
        self.local_message_reads.commit_staged(staged_projections)?;
        self.event_reads.commit_staged(staged_events)?;
        Ok(())
    }
}

fn print_sync_overlay_diagnostics(store: &DecryptingSessionStore<'_>) -> Result<()> {
    println!(
        "sync_overlay_committed_events={}",
        store
            .overlay_event_count()
            .context("read command-local sync event overlay size")?
    );
    println!(
        "sync_overlay_committed_local_projections={}",
        store
            .overlay_local_projection_count()
            .context("read command-local sync projection overlay size")?
    );
    Ok(())
}

fn require_conversation_participants(
    membership: &ConversationMembershipSnapshot,
    local_account_id: AccountId,
    peer_account_id: AccountId,
) -> Result<()> {
    membership
        .require_member(local_account_id)
        .context("local account is not a member of the conversation")?;
    membership
        .require_member(peer_account_id)
        .context("peer account is not a member of the conversation")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroh::{RelayMode, SecretKey, endpoint::Builder};
    use kilogram_identity::{DeviceEncryptionIdentity, DeviceIdentity};
    use kilogram_ticket_store::{StoreConfig, TicketStoreServer};
    use kilogram_transport_iroh::endpoint_builder;
    use tokio::sync::oneshot;

    const UNSUPPORTED_TEST_ALPN: &[u8] = b"kilogram/test/unsupported/1";

    fn local_test_endpoint_builder() -> Result<Builder> {
        Ok(endpoint_builder(RoutePolicy::Auto)
            .relay_mode(RelayMode::Disabled)
            .clear_ip_transports()
            .clear_address_lookup()
            .portmapper_config(iroh::endpoint::PortmapperConfig::Disabled)
            .net_report_config(iroh::endpoint::NetReportConfig::minimal())
            .bind_addr((std::net::Ipv4Addr::LOCALHOST, 0))?)
    }

    #[test]
    fn recovery_worker_wait_is_bounded_by_poll_runtime_and_signed_deadline() {
        assert_eq!(
            recovery_worker_wait_duration(
                Duration::from_secs(5),
                Duration::from_secs(20),
                Some((100, 102)),
            ),
            Duration::from_secs(2)
        );
        assert_eq!(
            recovery_worker_wait_duration(
                Duration::from_secs(5),
                Duration::from_secs(3),
                Some((100, 120)),
            ),
            Duration::from_secs(3)
        );
        assert_eq!(
            recovery_worker_wait_duration(Duration::from_secs(5), Duration::from_secs(20), None,),
            Duration::from_secs(5)
        );
    }

    #[test]
    fn recovery_worker_recognizes_only_typed_state_lock_contention() {
        let busy: anyhow::Error = StateError::AlreadyLocked {
            path: PathBuf::from("state"),
        }
        .into();
        assert!(recovery_worker_state_lock_is_busy(&busy));
        assert!(!recovery_worker_state_lock_is_busy(&anyhow::anyhow!(
            "state directory text mentions a lock"
        )));
    }

    #[test]
    fn runtime_history_order_is_causal_with_a_deterministic_ready_tie_break() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let root = AccountRootState::create(directory.path().join("root"))?;
        let identity = DeviceIdentity::generate()?;
        let certificate = root.issue_device_certificate(
            identity.device_id(),
            DeviceEncryptionIdentity::generate()?.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let authority = root.authority_snapshot()?;
        let conversation_id = ConversationId::from_label("runtime-read-order");
        let acknowledged_id: kilogram_protocol::EventId = postcard::from_bytes(&[1_u8; 32])?;
        let first = SignedEvent::sign_acknowledgement(
            &identity,
            conversation_id,
            10,
            Vec::new(),
            acknowledged_id,
        )?;
        let first_id = first.event_id()?;
        let child = SignedEvent::sign_acknowledgement(
            &identity,
            conversation_id,
            0,
            vec![first_id],
            first_id,
        )?;
        let child_id = child.event_id()?;
        let independent =
            SignedEvent::sign_acknowledgement(&identity, conversation_id, 5, Vec::new(), first_id)?;
        let independent_id = independent.event_id()?;
        let authorize = |id, event| -> Result<StoredAuthorizedEvent> {
            Ok(StoredAuthorizedEvent {
                id,
                event: AuthorizedEvent::new(event, certificate.clone(), authority.clone())?,
            })
        };
        let ordered = order_runtime_history(vec![
            authorize(child_id, child)?,
            authorize(first_id, first)?,
            authorize(independent_id, independent)?,
        ])?;
        assert_eq!(
            ordered.iter().map(|stored| stored.id).collect::<Vec<_>>(),
            vec![independent_id, first_id, child_id]
        );
        Ok(())
    }

    #[test]
    fn runtime_history_cursor_is_bound_to_one_immutable_text_snapshot() -> Result<()> {
        let snapshot = [7_u8; 32];
        assert_eq!(runtime_history_page_end(80, snapshot, None)?, 80);
        assert_eq!(
            runtime_history_page_end(
                80,
                snapshot,
                Some(RuntimeIpcHistoryCursor::new(snapshot, 30)),
            )?,
            30
        );
        assert!(
            runtime_history_page_end(
                80,
                snapshot,
                Some(RuntimeIpcHistoryCursor::new([8_u8; 32], 30)),
            )
            .is_err()
        );
        assert!(
            runtime_history_page_end(
                80,
                snapshot,
                Some(RuntimeIpcHistoryCursor::new(snapshot, 81)),
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn history_rewrap_source_consent_requires_exact_sas_and_same_account() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let root = AccountRootState::create(directory.path().join("root"))?;
        let source = DeviceIdentity::generate()?;
        let recipient = DeviceIdentity::generate()?;
        let source_certificate = root.issue_device_certificate(
            source.device_id(),
            DeviceEncryptionIdentity::generate()?.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let recipient_certificate = root.issue_device_certificate(
            recipient.device_id(),
            DeviceEncryptionIdentity::generate()?.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let device_list =
            root.publish_device_list(&[source_certificate.clone(), recipient_certificate])?;
        let sas =
            HistoryRewrapSas::derive(&device_list, source.device_id(), recipient.device_id())?;
        let approval = prepare_history_rewrap_approval(
            Some("consent-test".to_owned()),
            Some(recipient.device_id()),
            Some(sas.to_string()),
            4,
            8,
            &device_list,
            &source_certificate,
            root.account_id(),
        )?
        .context("complete history-rewrap approval was ignored")?;
        assert_eq!(approval.sas, sas);
        assert_eq!(approval.range_start, 4);
        assert_eq!(approval.count, 8);
        let paginated_approval = prepare_history_rewrap_approval(
            Some("consent-test".to_owned()),
            Some(recipient.device_id()),
            Some(sas.to_string()),
            4,
            MAX_HISTORY_REWRAP_ENTRIES * 4,
            &device_list,
            &source_certificate,
            root.account_id(),
        )?
        .context("paginated history-rewrap approval was ignored")?;
        assert_eq!(paginated_approval.count, MAX_HISTORY_REWRAP_ENTRIES * 4);
        assert!(paginated_approval.contains_range(4, MAX_HISTORY_REWRAP_ENTRIES));
        assert!(paginated_approval.contains_range(260, MAX_HISTORY_REWRAP_ENTRIES));
        assert!(!paginated_approval.contains_range(3, 1));
        assert!(!paginated_approval.contains_range(1027, 2));
        assert!(
            prepare_history_rewrap_approval(
                Some("consent-test".to_owned()),
                Some(recipient.device_id()),
                Some("000-000-000-000".to_owned()),
                4,
                8,
                &device_list,
                &source_certificate,
                root.account_id(),
            )
            .is_err()
        );
        let other_root = AccountRootState::create(directory.path().join("other-root"))?;
        assert!(
            prepare_history_rewrap_approval(
                Some("consent-test".to_owned()),
                Some(recipient.device_id()),
                Some(sas.to_string()),
                4,
                8,
                &device_list,
                &source_certificate,
                other_root.account_id(),
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn history_recovery_session_requires_contiguous_pages_and_stops_at_bound() {
        let mut progress = HistoryRecoverySessionProgress::default();
        assert!(progress.accepts(17));
        progress.record_page(19, false);
        assert_eq!(progress.pages_served, 1);
        assert!(progress.accepts(19));
        assert!(!progress.accepts(18));
        assert!(progress.can_request_next());

        for next in 20..=(MAX_HISTORY_RECOVERY_PAGES_PER_SESSION as u64 + 18) {
            progress.record_page(next, false);
        }
        assert_eq!(
            progress.pages_served,
            MAX_HISTORY_RECOVERY_PAGES_PER_SESSION
        );
        assert!(!progress.can_request_next());

        let mut completed = HistoryRecoverySessionProgress::default();
        completed.record_page(1, true);
        assert!(!completed.can_request_next());
    }

    #[test]
    fn history_rewrap_reconciliation_distinguishes_coverage_and_source_claims() -> Result<()> {
        assert!(ranges_cover_inventory(&[(0, 2), (2, 5)], 5));
        assert!(ranges_cover_inventory(&[(0, 3), (2, 5)], 5));
        assert!(!ranges_cover_inventory(&[(0, 2), (3, 5)], 5));
        assert!(!ranges_cover_inventory(&[], 5));

        let first = DeviceIdentity::generate()?.device_id();
        let second = DeviceIdentity::generate()?.device_id();
        let digest = [7_u8; 32];
        assert_eq!(
            classify_history_rewrap_agreement(&[(first, 5, digest)], 0),
            "single-source"
        );
        assert_eq!(
            classify_history_rewrap_agreement(&[(first, 5, digest), (second, 5, digest)], 0,),
            "agreed"
        );
        assert_eq!(
            classify_history_rewrap_agreement(&[(first, 5, digest), (second, 6, [8_u8; 32])], 0,),
            "divergent"
        );
        assert_eq!(
            classify_history_rewrap_agreement(&[(first, 5, digest)], 1),
            "divergent"
        );
        assert_eq!(classify_history_rewrap_agreement(&[], 0), "incomplete");
        Ok(())
    }

    #[test]
    fn cli_state_transaction_rolls_back_all_managed_roots_on_operation_error() -> Result<()> {
        let directory = tempfile::tempdir()?;
        fs::create_dir_all(directory.path().join("ratchet"))?;
        fs::write(directory.path().join("ratchet/session.bin"), b"before")?;
        fs::write(directory.path().join("next-sequence"), b"4")?;

        let result: Result<()> = run_state_transaction(directory.path(), |_| {
            fs::write(directory.path().join("ratchet/session.bin"), b"after")?;
            fs::write(directory.path().join("next-sequence"), b"5")?;
            fs::create_dir_all(directory.path().join("events/conversation"))?;
            fs::write(
                directory
                    .path()
                    .join("events/conversation/interrupted.event"),
                b"partial",
            )?;
            fs::create_dir_all(directory.path().join("local-messages"))?;
            fs::write(
                directory
                    .path()
                    .join("local-messages/interrupted.local-text"),
                b"partial",
            )?;
            bail!("injected local state failure")
        });

        assert!(result.is_err());
        assert_eq!(
            fs::read(directory.path().join("ratchet/session.bin"))?,
            b"before"
        );
        assert_eq!(fs::read(directory.path().join("next-sequence"))?, b"4");
        assert!(
            !directory
                .path()
                .join("events/conversation/interrupted.event")
                .exists()
        );
        assert!(
            !directory
                .path()
                .join("local-messages/interrupted.local-text")
                .exists()
        );
        assert!(
            !directory
                .path()
                .join(".kilogram-transactions/active")
                .exists()
        );
        Ok(())
    }

    #[test]
    fn cli_sequence_uses_db_primary_value_and_commits_a_typed_delta() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let device_state = DeviceState::load_or_create(directory.path())?;
        assert_eq!(device_state.allocate_sequence()?, 0);
        assert_eq!(device_state.allocate_sequence()?, 1);
        fs::create_dir_all(directory.path().join("events/conversation"))?;
        fs::write(
            directory.path().join("events/conversation/existing.event"),
            b"existing",
        )?;
        let vault = EncryptedStateVault::open_or_create(directory.path())?;
        assert_eq!(vault.migrate_legacy_snapshot()?.1.mirror_generation(), 1);
        drop(vault);

        let guard = VaultDualWriteGuard::prepare(directory.path())?
            .context("expected an initialized vault guard")?;
        fs::write(directory.path().join("next-sequence"), b"99\n")?;
        let allocated = run_state_transaction(directory.path(), |transaction| {
            transaction.allocate_sequence(&device_state)
        })?;
        assert_eq!(allocated, 2);
        assert_eq!(fs::read(directory.path().join("next-sequence"))?, b"3\n");
        guard.finish()?;

        let vault = EncryptedStateVault::open_existing(directory.path())?;
        let report = vault.verify_against_legacy()?;
        assert_eq!(report.mirror_generation(), 2);
        assert_eq!(report.record_count(), 4);
        Ok(())
    }

    #[test]
    fn cli_ratchet_workspace_uses_db_primary_and_registered_append_writes() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let device_state = DeviceState::load_or_create(directory.path())?;
        let mut ratchet_state = RatchetState::load_or_create(directory.path())?;
        ratchet_state.prekey_bundle(device_state.identity())?;
        drop(ratchet_state);
        let secret_path = directory.path().join("ratchet/pickle-secret.key");
        let committed_secret = fs::read(&secret_path)?;
        let vault = EncryptedStateVault::open_or_create(directory.path())?;
        vault.migrate_legacy_snapshot()?;
        drop(vault);

        let guard = VaultDualWriteGuard::prepare(directory.path())?
            .context("expected an initialized vault guard")?;
        fs::write(&secret_path, b"tampered-retained-shadow")?;
        run_state_transaction(directory.path(), |transaction| {
            let mut ratchet_state = transaction.load_ratchet_state()?;
            assert_eq!(fs::read(&secret_path)?, committed_secret);
            ratchet_state.prekey_bundle(device_state.identity())?;
            let event_path = directory.path().join("events/canary.event");
            fs::create_dir_all(event_path.parent().context("event parent")?)?;
            fs::write(&event_path, b"registered-canary")?;
            transaction.register_append_only_write("events/canary.event")?;
            Ok(())
        })?;
        guard.finish()?;

        assert_eq!(fs::read(&secret_path)?, committed_secret);
        let vault = EncryptedStateVault::open_existing(directory.path())?;
        let read = vault.read_primary_canary(&[StateRecordKind::Event])?;
        assert_eq!(read.records().len(), 1);
        assert_eq!(read.records()[0].relative_path(), "events/canary.event");
        assert_eq!(read.records()[0].content(), b"registered-canary");
        assert_eq!(vault.verify_against_legacy()?.mirror_generation(), 2);
        Ok(())
    }

    #[test]
    fn ratchet_retirement_rolls_back_on_failure_and_commits_as_vault_delta() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let peer_directory = tempfile::tempdir()?;
        let local = DeviceIdentity::generate()?;
        let peer = DeviceIdentity::generate()?;
        let now = unix_time_now()?;
        let peer_pool = RatchetState::load_or_create(peer_directory.path())?.prekey_pool(
            &peer,
            4,
            now,
            DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
        )?;
        let mut local_ratchet = RatchetState::load_or_create(directory.path())?;
        local_ratchet.observe_prekey_pool(&peer_pool, now)?;
        local_ratchet.encrypt_with_pool(&local, &peer_pool, "transactional retirement", now)?;
        drop(local_ratchet);
        EncryptedStateVault::open_or_create(directory.path())?.migrate_legacy_snapshot()?;

        let failed_guard = VaultDualWriteGuard::prepare(directory.path())?
            .context("prepare failed retirement dual-write")?;
        let failed: Result<()> = run_state_transaction(directory.path(), |transaction| {
            let ratchet = transaction.load_ratchet_state()?;
            let retirement = ratchet.retire_peer_device(peer.device_id())?;
            ensure!(retirement.session_removed && retirement.prekey_observation_removed);
            bail!("injected failure after ratchet retirement")
        });
        assert!(failed.is_err());
        failed_guard.finish()?;
        assert!(
            RatchetState::load_or_create(directory.path())?.has_session(peer.device_id()),
            "rollback must restore the retired session"
        );

        let committed_guard = VaultDualWriteGuard::prepare(directory.path())?
            .context("prepare committed retirement dual-write")?;
        let retirement = run_state_transaction(directory.path(), |transaction| {
            transaction
                .load_ratchet_state()?
                .retire_peer_device(peer.device_id())
                .context("commit peer retirement")
        })?;
        committed_guard.finish()?;
        assert!(retirement.session_removed && retirement.prekey_observation_removed);
        assert!(!RatchetState::load_or_create(directory.path())?.has_session(peer.device_id()));
        let read_guard = VaultDualWriteGuard::prepare(directory.path())?
            .context("prepare retirement verification read")?;
        let read = EncryptedStateVault::open_existing(directory.path())?
            .read_mutable_primary_canary(&[StateRecordKind::Ratchet])?;
        let peer_id = peer.device_id().to_string();
        assert!(
            read.records()
                .iter()
                .all(|record| !record.relative_path().contains(&peer_id))
        );
        read_guard.finish()?;
        Ok(())
    }

    #[test]
    fn cli_vault_guard_mirrors_live_state_and_recovers_a_crashed_command() -> Result<()> {
        let directory = tempfile::tempdir()?;
        fs::write(directory.path().join("state"), b"initial")?;
        let vault = EncryptedStateVault::open_or_create(directory.path())?;
        assert_eq!(vault.migrate_legacy_snapshot()?.1.mirror_generation(), 1);
        drop(vault);

        let guard = VaultDualWriteGuard::prepare(directory.path())?
            .context("expected an initialized vault guard")?;
        fs::write(directory.path().join("state"), b"committed")?;
        guard.finish()?;
        let vault = EncryptedStateVault::open_existing(directory.path())?;
        assert_eq!(vault.verify_against_legacy()?.mirror_generation(), 2);
        drop(vault);

        let interrupted = VaultDualWriteGuard::prepare(directory.path())?
            .context("expected a second vault guard")?;
        fs::write(directory.path().join("state"), b"after-crash")?;
        drop(interrupted);

        let recovered = VaultDualWriteGuard::prepare(directory.path())?
            .context("expected recovery to prepare the next intent")?;
        recovered.finish()?;
        let vault = EncryptedStateVault::open_existing(directory.path())?;
        assert_eq!(vault.verify_against_legacy()?.mirror_generation(), 3);
        drop(vault);
        shadow_read_state_vault(directory.path().to_path_buf())?;

        fs::write(directory.path().join("state"), b"external-tamper")?;
        assert!(VaultDualWriteGuard::prepare(directory.path()).is_err());
        Ok(())
    }

    #[test]
    fn cli_trust_repository_reads_db_and_repairs_tampered_shadow_transactionally() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let account_directory = directory.path().join("account");
        let state_directory = directory.path().join("device");
        create_account(account_directory.clone())?;
        enroll_device(account_directory.clone(), state_directory.clone(), None)?;
        let account = AccountRootState::load(&account_directory)?;
        let membership = account.create_conversation_membership(
            ConversationId::from_label("db-primary-trust").scope_id(),
            &[],
        )?;
        install_conversation_membership(state_directory.clone(), {
            let path = directory.path().join("membership.snapshot");
            write_new_authority_file(&path, &membership.encode()?)?;
            path
        })?;
        let vault = EncryptedStateVault::open_or_create(&state_directory)?;
        vault.migrate_legacy_snapshot()?;
        drop(vault);

        let guard = VaultDualWriteGuard::prepare(&state_directory)?
            .context("expected initialized trust vault guard")?;
        fs::write(
            state_directory.join("account-authority.snapshot"),
            b"tampered-shadow",
        )?;
        let device = load_command_device_state(&state_directory)?;
        let trust = CommandTrustReadRepository::open(&state_directory, &device)?;
        let certificate = trust.load_certificate()?;
        let authority = trust.load_own_authority_snapshot(&certificate)?;
        assert_eq!(
            trust.load_conversation_membership(membership.conversation_id())?,
            membership
        );
        assert_eq!(
            install_own_authority_primary(&state_directory, &device, &authority)?,
            AuthoritySnapshotStoreOutcome::Unchanged
        );
        guard.finish()?;

        assert_eq!(
            load_command_device_state(&state_directory)?.load_own_authority_snapshot()?,
            authority
        );
        assert_eq!(
            EncryptedStateVault::open_existing(&state_directory)?
                .verify_against_legacy()?
                .record_count(),
            5
        );
        Ok(())
    }

    #[test]
    fn cli_device_identity_reads_db_primary_without_filesystem_fallback() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let original = DeviceState::load_or_create(directory.path())?;
        let expected_device_id = original.identity().device_id();
        let expected_encryption_key = original.encryption().public_key();
        drop(original);
        let signing_path = directory.path().join("device-secret.key");
        let encryption_path = directory.path().join("device-encryption-secret.key");
        let vault = EncryptedStateVault::open_or_create(directory.path())?;
        vault.migrate_legacy_snapshot()?;
        drop(vault);
        assert!(!signing_path.exists());
        assert!(!encryption_path.exists());

        let guard = VaultDualWriteGuard::prepare(directory.path())?
            .context("expected initialized identity vault guard")?;
        fs::write(&signing_path, [41_u8; 32])?;
        fs::write(&encryption_path, [43_u8; 32])?;
        let loaded = load_command_device_state(directory.path())?;
        assert_eq!(loaded.identity().device_id(), expected_device_id);
        assert_eq!(loaded.encryption().public_key(), expected_encryption_key);

        fs::remove_file(&signing_path)?;
        fs::remove_file(&encryption_path)?;
        guard.finish()?;
        assert!(!signing_path.exists());
        assert!(!encryption_path.exists());
        assert_eq!(
            EncryptedStateVault::open_existing(directory.path())?
                .verify_against_legacy()?
                .mirror_generation(),
            1
        );
        Ok(())
    }

    fn authority_for(
        identity: &DeviceIdentity,
    ) -> Result<(
        AccountId,
        DeviceCertificate,
        AccountAuthoritySnapshot,
        AccountPrekeyDirectory,
    )> {
        let directory = tempfile::tempdir()?;
        let root = AccountRootState::create(directory.path())?;
        let encryption = DeviceEncryptionIdentity::generate()?;
        let certificate = root.issue_device_certificate(
            identity.device_id(),
            encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let snapshot = root.authority_snapshot()?;
        let device_list = root.publish_device_list(std::slice::from_ref(&certificate))?;
        let prekey_directory =
            AccountPrekeyDirectory::new(device_list, vec![prekey_pool_for(identity)?])?;
        Ok((root.account_id(), certificate, snapshot, prekey_directory))
    }

    fn prekey_pool_for(identity: &DeviceIdentity) -> Result<SignedPrekeyPool> {
        let directory = tempfile::tempdir()?;
        Ok(RatchetState::load_or_create(directory.path())?.prekey_pool(
            identity,
            4,
            unix_time_now()?,
            DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
        )?)
    }

    fn empty_immutable_read_repositories() -> Result<ImmutableReadRepositories> {
        Ok(ImmutableReadRepositories {
            events: Box::new(ImmutableEventReadSnapshot::from_records(Vec::<(
                String,
                Vec<u8>,
            )>::new(
            ))?),
            local_messages: Box::new(ImmutableLocalMessageReadSnapshot::from_records(Vec::<(
                String,
                Vec<u8>,
            )>::new(
            ))?),
            primary: "test-immutable-snapshot",
            shadow: "not-enabled",
            mirror_generation: None,
            event_record_count: 0,
            local_projection_record_count: 0,
        })
    }

    #[test]
    fn runtime_ticket_publication_creates_parent_and_replaces_existing_value() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let ticket = directory.path().join("published/runtime.ticket");

        publish_runtime_ticket(&ticket, b"first-ticket")?;
        assert_eq!(fs::read(&ticket)?, b"first-ticket");
        publish_runtime_ticket(&ticket, b"second-ticket")?;
        assert_eq!(fs::read(&ticket)?, b"second-ticket");

        Ok(())
    }

    #[test]
    fn sibling_publication_high_water_rejects_same_generation_equivocation() -> Result<()> {
        let publisher = DeviceIdentity::generate()?;
        let recipient = DeviceIdentity::generate()?;
        let (publisher_account, _, _, _) = authority_for(&publisher)?;
        let (recipient_account, _, _, _) = authority_for(&recipient)?;
        let channel = ticket_publication_write_capability(&publisher, recipient_account)
            .write_key()
            .channel_id();
        let now = unix_time_now()?;
        let first = SignedTicketPublication::sign(
            &publisher,
            channel,
            publisher_account,
            recipient_account,
            "first-ticket".to_owned(),
            now,
            300,
            None,
        )?;
        let conflicting = SignedTicketPublication::sign(
            &publisher,
            channel,
            publisher_account,
            recipient_account,
            "conflicting-ticket".to_owned(),
            now,
            300,
            None,
        )?;
        let mut known = BTreeMap::new();
        merge_runtime_publication_observation(
            &mut known,
            channel,
            first.generation(),
            first.publication_id()?,
            first.ticket_digest(),
        )?;
        let error = merge_runtime_publication_observation(
            &mut known,
            channel,
            conflicting.generation(),
            conflicting.publication_id()?,
            conflicting.ticket_digest(),
        )
        .err()
        .context("same-generation publication equivocation was accepted")?;
        assert!(format!("{error:#}").contains("equivocated"));
        Ok(())
    }

    #[test]
    fn runtime_ticket_compaction_preserves_signed_head_and_bounded_restart_state() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let root_dir = directory.path().join("root");
        let state_dir = directory.path().join("state");
        let peer_root_dir = directory.path().join("peer-root");
        create_account(root_dir.clone())?;
        create_account(peer_root_dir.clone())?;
        enroll_device(root_dir.clone(), state_dir.clone(), None)?;
        let root = AccountRootState::load(&root_dir)?;
        let peer = AccountRootState::load(&peer_root_dir)?;
        let device = DeviceState::load_or_create(&state_dir)?;
        let channel_id = ticket_publication_write_capability(device.identity(), peer.account_id())
            .write_key()
            .channel_id();
        let started_at = unix_time_now()?;
        let mut previous = None;
        for index in 0..=MAX_RUNTIME_TICKET_CHAIN_RECORDS_BEFORE_COMPACTION {
            let publication = SignedTicketPublication::sign(
                device.identity(),
                channel_id,
                root.account_id(),
                peer.account_id(),
                format!("ticket-{index}"),
                started_at + index as u64,
                300,
                previous.as_ref(),
            )?;
            let path = runtime_ticket_publication_relative_path(
                channel_id,
                publication.generation(),
                publication.publication_id()?,
            );
            let bytes = publication.encode()?;
            run_state_transaction(&state_dir, |transaction| {
                persist_runtime_record(&state_dir, &path, &bytes, transaction)
            })?;
            previous = Some(publication);
        }

        let first = compact_runtime_ticket_state_if_needed(&state_dir)?
            .context("first runtime ticket compaction was not triggered")?;
        assert_eq!(first.checkpoint_generation, 1);
        assert_eq!(first.removed_records, 8);
        assert_eq!(first.retained_anchors, 1);
        let snapshot = load_runtime_state_snapshot(
            &state_dir,
            root.account_id(),
            device.identity().device_id(),
        )?;
        assert_eq!(snapshot.ticket_publications[&channel_id].len(), 1);
        assert_eq!(snapshot.ticket_publications[&channel_id][0].generation(), 9);
        assert_eq!(
            snapshot
                .ticket_checkpoint
                .as_ref()
                .context("first runtime ticket checkpoint is absent")?
                .compacted_total_records(),
            8
        );
        assert!(compact_runtime_ticket_state_if_needed(&state_dir)?.is_none());

        previous = snapshot.latest_ticket_publication(channel_id).cloned();
        for index in 9..17 {
            let publication = SignedTicketPublication::sign(
                device.identity(),
                channel_id,
                root.account_id(),
                peer.account_id(),
                format!("ticket-{index}"),
                started_at + index as u64,
                300,
                previous.as_ref(),
            )?;
            let path = runtime_ticket_publication_relative_path(
                channel_id,
                publication.generation(),
                publication.publication_id()?,
            );
            let bytes = publication.encode()?;
            run_state_transaction(&state_dir, |transaction| {
                persist_runtime_record(&state_dir, &path, &bytes, transaction)
            })?;
            previous = Some(publication);
        }
        let second = compact_runtime_ticket_state_if_needed(&state_dir)?
            .context("second runtime ticket compaction was not triggered")?;
        assert_eq!(second.checkpoint_generation, 2);
        assert_eq!(second.removed_records, 9);
        assert_eq!(second.compacted_total_records, 17);
        let restarted = load_runtime_state_snapshot(
            &state_dir,
            root.account_id(),
            device.identity().device_id(),
        )?;
        assert_eq!(restarted.ticket_publications[&channel_id].len(), 1);
        assert_eq!(
            restarted.ticket_publications[&channel_id][0].generation(),
            17
        );
        let checkpoint = restarted
            .ticket_checkpoint
            .context("second runtime ticket checkpoint is absent")?;
        assert_eq!(checkpoint.generation(), 2);
        assert_eq!(checkpoint.compacted_total_records(), 17);
        let mut tampered = checkpoint.encode()?;
        let last = tampered
            .last_mut()
            .context("encoded runtime ticket checkpoint is empty")?;
        *last ^= 1;
        assert!(SignedRuntimeTicketCheckpoint::decode(&tampered).is_err());
        assert_eq!(
            fs::read_dir(
                state_dir
                    .join(RUNTIME_STATE_DIRECTORY)
                    .join(RUNTIME_TICKET_CHECKPOINTS_DIRECTORY)
            )?
            .count(),
            1
        );
        Ok(())
    }

    #[test]
    fn runtime_contact_enrolls_bounded_authenticated_endpoint_candidate_set() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let local_root_dir = directory.path().join("local-root");
        let local_state_dir = directory.path().join("local-state");
        create_account(local_root_dir.clone())?;
        enroll_device(local_root_dir.clone(), local_state_dir.clone(), None)?;
        let local_root = AccountRootState::load(&local_root_dir)?;
        let local_state = DeviceState::load_or_create(&local_state_dir)?;

        let peer_root = AccountRootState::create(directory.path().join("peer-root"))?;
        let mut peer_identities = Vec::new();
        let mut peer_certificates = Vec::new();
        let mut peer_pools = Vec::new();
        for index in 0..=MAX_RUNTIME_ENDPOINT_CANDIDATES_PER_CONTACT {
            let identity = DeviceIdentity::generate()?;
            let encryption = DeviceEncryptionIdentity::generate()?;
            let certificate = peer_root.issue_device_certificate(
                identity.device_id(),
                encryption.public_key(),
                &DeviceCapability::MESSAGING,
            )?;
            let ratchet_directory = directory.path().join(format!("peer-ratchet-{index}"));
            let pool = RatchetState::load_or_create(ratchet_directory)?.prekey_pool(
                &identity,
                4,
                unix_time_now()?,
                DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
            )?;
            peer_identities.push(identity);
            peer_certificates.push(certificate);
            peer_pools.push(pool);
        }
        let peer_device_list = peer_root.publish_device_list(&peer_certificates)?;
        let conversation = "endpoint-candidate-set";
        let conversation_id = ConversationId::from_label(conversation);
        let membership = local_root.create_conversation_membership(
            conversation_id.scope_id(),
            &[peer_root.account_id()],
        )?;
        local_state.install_conversation_membership(&membership)?;

        let mut ticket_files = Vec::new();
        for (index, (identity, certificate)) in peer_identities
            .iter()
            .zip(peer_certificates.iter())
            .enumerate()
        {
            let ticket = ConnectionTicket::new(
                EndpointAddr::new(SecretKey::generate().public()),
                identity,
                certificate.clone(),
                AccountPrekeyDirectory::new(peer_device_list.clone(), peer_pools.clone())?,
                local_root.account_id(),
                RoutePolicy::Auto,
            )?;
            let path = directory.path().join(format!("peer-{index}.ticket"));
            fs::write(&path, ticket.encode()?)?;
            ticket_files.push(path);
        }

        let first = add_runtime_contact_record(
            &local_state_dir,
            conversation.to_owned(),
            peer_root.account_id(),
            ticket_files[0].clone(),
        )?;
        assert!(first.inserted);
        assert!(first.endpoint_candidate_added);
        assert_eq!(first.endpoint_candidate_count, 1);
        for (index, ticket_file) in ticket_files
            .iter()
            .enumerate()
            .take(MAX_RUNTIME_ENDPOINT_CANDIDATES_PER_CONTACT)
            .skip(1)
        {
            let receipt = add_runtime_contact_record(
                &local_state_dir,
                conversation.to_owned(),
                peer_root.account_id(),
                ticket_file.clone(),
            )?;
            assert!(receipt.inserted);
            assert!(receipt.endpoint_candidate_added);
            assert_eq!(receipt.endpoint_candidate_count, index + 1);
            assert_eq!(receipt.contact_id, first.contact_id);
        }
        let repeated = add_runtime_contact_record(
            &local_state_dir,
            conversation.to_owned(),
            peer_root.account_id(),
            ticket_files[1].clone(),
        )?;
        assert!(!repeated.inserted);
        assert!(!repeated.endpoint_candidate_added);
        assert_eq!(
            repeated.endpoint_candidate_count,
            MAX_RUNTIME_ENDPOINT_CANDIDATES_PER_CONTACT
        );
        let overflow = add_runtime_contact_record(
            &local_state_dir,
            conversation.to_owned(),
            peer_root.account_id(),
            ticket_files[MAX_RUNTIME_ENDPOINT_CANDIDATES_PER_CONTACT].clone(),
        )
        .err()
        .context("fifth endpoint candidate unexpectedly enrolled")?;
        assert!(format!("{overflow:#}").contains("maximum"));

        let local_certificate = local_state.load_certificate()?;
        let local_authority = local_state.load_own_authority_snapshot()?;
        let snapshot = load_runtime_state_snapshot(
            &local_state_dir,
            local_root.account_id(),
            local_state.identity().device_id(),
        )?;
        assert_eq!(snapshot.contacts.len(), 1);
        assert_eq!(snapshot.endpoint_candidates.len(), 3);
        let contact = snapshot
            .contacts
            .get(&first.contact_id)
            .context("runtime contact disappeared")?;
        let candidates = load_runtime_endpoint_candidate_set(
            &snapshot,
            contact,
            &local_certificate,
            &local_authority,
        )?;
        assert_eq!(
            candidates.len(),
            MAX_RUNTIME_ENDPOINT_CANDIDATES_PER_CONTACT
        );
        assert!(candidates[0].primary);
        assert_eq!(candidates[0].peer_device_id, peer_identities[0].device_id());
        let pinned_peer_authority =
            local_state.load_peer_authority_snapshot(peer_root.account_id())?;
        let statuses = collect_runtime_endpoint_candidate_statuses(
            &snapshot,
            contact,
            &local_certificate,
            &local_authority,
            &pinned_peer_authority,
        )?;
        assert_eq!(statuses.len(), MAX_RUNTIME_ENDPOINT_CANDIDATES_PER_CONTACT);
        assert!(
            statuses
                .iter()
                .all(|status| status.state == RuntimeIpcEndpointCandidateState::Usable)
        );
        fs::remove_file(&ticket_files[0])?;
        let surviving = load_runtime_endpoint_candidate_set(
            &snapshot,
            contact,
            &local_certificate,
            &local_authority,
        )?;
        assert_eq!(
            surviving.len(),
            MAX_RUNTIME_ENDPOINT_CANDIDATES_PER_CONTACT - 1
        );
        assert!(surviving.iter().all(|candidate| !candidate.primary));
        let statuses = collect_runtime_endpoint_candidate_statuses(
            &snapshot,
            contact,
            &local_certificate,
            &local_authority,
            &pinned_peer_authority,
        )?;
        assert_eq!(
            statuses
                .iter()
                .filter(|status| status.state == RuntimeIpcEndpointCandidateState::Usable)
                .count(),
            MAX_RUNTIME_ENDPOINT_CANDIDATES_PER_CONTACT - 1
        );
        assert!(statuses.iter().any(|status| {
            status.primary
                && status.state == RuntimeIpcEndpointCandidateState::Stale
                && status.detail.contains("descriptor-unusable")
        }));
        peer_root.revoke_device(
            peer_identities[MAX_RUNTIME_ENDPOINT_CANDIDATES_PER_CONTACT].device_id(),
        )?;
        local_state.pin_peer_authority_snapshot(&peer_root.authority_snapshot()?)?;
        let pinned_peer_authority =
            local_state.load_peer_authority_snapshot(peer_root.account_id())?;
        let statuses = collect_runtime_endpoint_candidate_statuses(
            &snapshot,
            contact,
            &local_certificate,
            &local_authority,
            &pinned_peer_authority,
        )?;
        assert!(statuses.iter().all(|status| {
            status.state == RuntimeIpcEndpointCandidateState::Stale
                && (status.detail.contains("descriptor-unusable")
                    || status.detail == "authority-behind-local-high-water")
        }));
        Ok(())
    }

    #[test]
    fn endpoint_announcements_transfer_enrollments_and_high_water_between_own_devices() -> Result<()>
    {
        let directory = tempfile::tempdir()?;
        let local_root = AccountRootState::create(directory.path().join("local-root"))?;
        let source_state_dir = directory.path().join("source-state");
        let recipient_state_dir = directory.path().join("recipient-state");
        let source = DeviceState::load_or_create(&source_state_dir)?;
        let recipient = DeviceState::load_or_create(&recipient_state_dir)?;
        let source_certificate = local_root.issue_device_certificate(
            source.identity().device_id(),
            source.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let recipient_certificate = local_root.issue_device_certificate(
            recipient.identity().device_id(),
            recipient.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let local_authority = local_root.authority_snapshot()?;
        let local_device_list = local_root
            .publish_device_list(&[source_certificate.clone(), recipient_certificate.clone()])?;
        for (state, certificate) in [
            (&source, &source_certificate),
            (&recipient, &recipient_certificate),
        ] {
            state.install_certificate(certificate)?;
            state.install_own_authority_snapshot(&local_authority)?;
        }

        let peer_root = AccountRootState::create(directory.path().join("peer-root"))?;
        let peer = DeviceIdentity::generate()?;
        let peer_encryption = DeviceEncryptionIdentity::generate()?;
        let peer_certificate = peer_root.issue_device_certificate(
            peer.device_id(),
            peer_encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let peer_device_list =
            peer_root.publish_device_list(std::slice::from_ref(&peer_certificate))?;
        let now = unix_time_now()?;
        let peer_pool = RatchetState::load_or_create(directory.path().join("peer-ratchet"))?
            .prekey_pool(&peer, 4, now, DEFAULT_PREKEY_POOL_VALIDITY_SECONDS)?;
        let peer_directory =
            AccountPrekeyDirectory::new(peer_device_list, vec![peer_pool.clone()])?;
        let peer_ticket = ConnectionTicket::new(
            EndpointAddr::new(SecretKey::generate().public()),
            &peer,
            peer_certificate,
            peer_directory.clone(),
            local_root.account_id(),
            RoutePolicy::Auto,
        )?;
        let peer_ticket_file = directory.path().join("peer.ticket");
        fs::write(&peer_ticket_file, peer_ticket.encode()?)?;

        let conversation = "own-device-endpoint-announcements";
        let conversation_id = ConversationId::from_label(conversation);
        let membership = local_root.create_conversation_membership(
            conversation_id.scope_id(),
            &[peer_root.account_id()],
        )?;
        source.install_conversation_membership(&membership)?;
        recipient.install_conversation_membership(&membership)?;
        let receipt = add_runtime_contact_record(
            &source_state_dir,
            conversation.to_owned(),
            peer_root.account_id(),
            peer_ticket_file,
        )?;

        let publication = SignedTicketPublication::sign(
            &peer,
            peer_ticket.ticket_publication_write_key().channel_id(),
            peer_root.account_id(),
            local_root.account_id(),
            peer_ticket.encode()?,
            now,
            300,
            None,
        )?;
        let observation = SignedTicketPublicationObservation::sign(
            source.identity(),
            local_root.account_id(),
            &publication,
            now,
            None,
        )?;
        run_state_transaction(&source_state_dir, |transaction| {
            persist_runtime_record(
                &source_state_dir,
                &runtime_ticket_observation_relative_path(
                    observation.channel_id(),
                    observation.observation_generation(),
                    observation.observation_id()?,
                ),
                &observation.encode()?,
                transaction,
            )
        })?;

        let source_pool = RatchetState::load_or_create(&source_state_dir)?.prekey_pool(
            source.identity(),
            4,
            now,
            DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
        )?;
        let recipient_pool = RatchetState::load_or_create(&recipient_state_dir)?.prekey_pool(
            recipient.identity(),
            4,
            now,
            DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
        )?;
        let local_directory = AccountPrekeyDirectory::new(
            local_device_list.clone(),
            vec![source_pool, recipient_pool],
        )?;
        let source_ticket = ConnectionTicket::new(
            EndpointAddr::new(SecretKey::generate().public()),
            source.identity(),
            source_certificate,
            local_directory.clone(),
            peer_root.account_id(),
            RoutePolicy::Auto,
        )?;
        let recipient_ticket = ConnectionTicket::new(
            EndpointAddr::new(SecretKey::generate().public()),
            recipient.identity(),
            recipient_certificate,
            local_directory,
            peer_root.account_id(),
            RoutePolicy::Auto,
        )?;

        let bundle_file = directory.path().join("source-to-recipient.eab");
        let export = export_runtime_endpoint_announcements(
            &source_state_dir,
            &source_ticket,
            recipient.identity().device_id(),
            bundle_file.clone(),
            300,
        )?;
        assert_eq!(export.contact_count, 1);
        assert_eq!(export.endpoint_count, 1);
        assert_eq!(export.observation_count, 1);

        let descriptors = directory.path().join("recipient-descriptors");
        let imported = import_runtime_endpoint_announcements(
            &recipient_state_dir,
            &recipient_ticket,
            &bundle_file,
            &descriptors,
        )?;
        assert_eq!(imported.contact_added_count, 1);
        assert_eq!(imported.endpoint_added_count, 1);
        assert_eq!(imported.publication_binding_added_count, 1);
        assert_eq!(imported.observation_evidence_added_count, 1);
        let recipient_snapshot = load_runtime_state_snapshot(
            &recipient_state_dir,
            local_root.account_id(),
            recipient.identity().device_id(),
        )?;
        assert_eq!(recipient_snapshot.contacts.len(), 1);
        assert_eq!(recipient_snapshot.endpoint_candidates.len(), 0);
        assert_eq!(recipient_snapshot.endpoint_publication_bindings.len(), 1);
        assert_eq!(recipient_snapshot.accepted_endpoint_observations.len(), 1);
        assert_eq!(
            recipient_snapshot
                .accepted_endpoint_observation_high_water(observation.channel_id())?,
            Some((
                publication.generation(),
                publication.publication_id()?,
                publication.ticket_digest(),
            ))
        );
        assert_eq!(
            recipient_snapshot
                .contacts
                .get(&receipt.contact_id)
                .context("imported runtime contact is absent")?
                .peer_device_id(),
            peer.device_id()
        );

        let repeated = import_runtime_endpoint_announcements(
            &recipient_state_dir,
            &recipient_ticket,
            &bundle_file,
            &descriptors,
        )?;
        assert_eq!(repeated.contact_added_count, 0);
        assert_eq!(repeated.endpoint_added_count, 0);
        assert_eq!(repeated.publication_binding_added_count, 0);
        assert_eq!(repeated.observation_evidence_added_count, 0);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn runtime_pushes_endpoint_announcements_over_authenticated_own_device_session()
    -> Result<()> {
        let directory = tempfile::tempdir()?;
        let root = AccountRootState::create(directory.path().join("root"))?;
        let source_state_dir = directory.path().join("source-state");
        let recipient_state_dir = directory.path().join("recipient-state");
        let source = DeviceState::load_or_create(&source_state_dir)?;
        let recipient = DeviceState::load_or_create(&recipient_state_dir)?;
        let source_certificate = root.issue_device_certificate(
            source.identity().device_id(),
            source.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let recipient_certificate = root.issue_device_certificate(
            recipient.identity().device_id(),
            recipient.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let device_list =
            root.publish_device_list(&[source_certificate.clone(), recipient_certificate.clone()])?;
        let authority = device_list.authority_snapshot().clone();
        for (state, certificate) in [
            (&source, &source_certificate),
            (&recipient, &recipient_certificate),
        ] {
            state.install_certificate(certificate)?;
            state.install_own_authority_snapshot(&authority)?;
        }

        let peer_root = AccountRootState::create(directory.path().join("peer-root"))?;
        let peer = DeviceIdentity::generate()?;
        let peer_encryption = DeviceEncryptionIdentity::generate()?;
        let peer_certificate = peer_root.issue_device_certificate(
            peer.device_id(),
            peer_encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let peer_devices =
            peer_root.publish_device_list(std::slice::from_ref(&peer_certificate))?;
        let now = unix_time_now()?;
        let peer_pool = RatchetState::load_or_create(directory.path().join("peer-ratchet"))?
            .prekey_pool(&peer, 4, now, DEFAULT_PREKEY_POOL_VALIDITY_SECONDS)?;
        let peer_ticket = ConnectionTicket::new(
            EndpointAddr::new(SecretKey::generate().public()),
            &peer,
            peer_certificate,
            AccountPrekeyDirectory::new(peer_devices, vec![peer_pool])?,
            root.account_id(),
            RoutePolicy::DirectOnly,
        )?;
        let peer_ticket_file = directory.path().join("peer.ticket");
        fs::write(&peer_ticket_file, peer_ticket.encode()?)?;
        let conversation = "network-own-device-endpoint-announcements";
        let membership = root.create_conversation_membership(
            ConversationId::from_label(conversation).scope_id(),
            &[peer_root.account_id()],
        )?;
        source.install_conversation_membership(&membership)?;
        recipient.install_conversation_membership(&membership)?;
        add_runtime_contact_record(
            &source_state_dir,
            conversation.to_owned(),
            peer_root.account_id(),
            peer_ticket_file,
        )?;

        let source_pool_file = directory.path().join("source.prekeys");
        let recipient_pool_file = directory.path().join("recipient.prekeys");
        let source_pool = RatchetState::load_or_create(&source_state_dir)?.prekey_pool(
            source.identity(),
            4,
            now,
            DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
        )?;
        let recipient_pool = RatchetState::load_or_create(&recipient_state_dir)?.prekey_pool(
            recipient.identity(),
            4,
            now,
            DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
        )?;
        fs::write(&source_pool_file, source_pool.encode()?)?;
        fs::write(&recipient_pool_file, recipient_pool.encode()?)?;
        let device_list_file = directory.path().join("own-devices.snapshot");
        fs::write(&device_list_file, device_list.encode()?)?;
        let source_ticket_file = directory.path().join("source-runtime.ticket");
        let recipient_ticket_file = directory.path().join("recipient-runtime.ticket");
        let source_ipc = directory.path().join("source-runtime.ipc.json");
        let recipient_ipc = directory.path().join("recipient-runtime.ipc.json");
        let source_task = tokio::spawn(runtime(RuntimeOptions {
            state_dir: source_state_dir.clone(),
            allowed_requester_account_id: root.account_id(),
            device_list_file: device_list_file.clone(),
            peer_prekey_pool_files: vec![recipient_pool_file],
            ticket_file: Some(source_ticket_file.clone()),
            relay_wait_seconds: 0,
            route_policy: RoutePolicy::DirectOnly,
            relay_url: None,
            max_sessions: 0,
            idle_seconds: 0,
            poll_milliseconds: 500,
            retry_base_seconds: 1,
            retry_max_seconds: 1,
            auto_sync_seconds: 0,
            max_outbound_actions: 0,
            ipc_file: Some(source_ipc.clone()),
        }));
        let recipient_task = tokio::spawn(runtime(RuntimeOptions {
            state_dir: recipient_state_dir.clone(),
            allowed_requester_account_id: root.account_id(),
            device_list_file,
            peer_prekey_pool_files: vec![source_pool_file],
            ticket_file: Some(recipient_ticket_file.clone()),
            relay_wait_seconds: 0,
            route_policy: RoutePolicy::DirectOnly,
            relay_url: None,
            max_sessions: 0,
            idle_seconds: 0,
            poll_milliseconds: 500,
            retry_base_seconds: 1,
            retry_max_seconds: 1,
            auto_sync_seconds: 0,
            max_outbound_actions: 0,
            ipc_file: Some(recipient_ipc.clone()),
        }));
        timeout(Duration::from_secs(10), async {
            while !source_ipc.is_file()
                || !recipient_ipc.is_file()
                || !source_ticket_file.is_file()
                || !recipient_ticket_file.is_file()
            {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .context("own-device endpoint announcement runtimes did not become ready")?;

        let response = kilogram_runtime_ipc::call(
            &source_ipc,
            RuntimeIpcCommand::PushEndpointAnnouncements {
                recipient_ticket_file: recipient_ticket_file.clone(),
                validity_seconds: 300,
            },
        )
        .await?;
        let RuntimeIpcResponse::EndpointAnnouncementsPushed(push) = response else {
            bail!("source runtime returned an unexpected endpoint announcement response")
        };
        assert_eq!(push.source_device_id, source.identity().device_id());
        assert_eq!(push.recipient_device_id, recipient.identity().device_id());
        assert_eq!(push.contact_count, 1);
        assert_eq!(push.endpoint_count, 1);
        assert_eq!(push.recipient_contact_added_count, 1);
        assert_eq!(push.recipient_endpoint_added_count, 1);
        assert_eq!(push.recipient_publication_binding_added_count, 1);
        assert_eq!(push.transport_path, "direct");
        assert_eq!(
            push.acknowledgement_status,
            "recipient-device-signed-session-bound"
        );

        for ipc in [&source_ipc, &recipient_ipc] {
            assert!(matches!(
                kilogram_runtime_ipc::call(ipc, RuntimeIpcCommand::Shutdown).await?,
                RuntimeIpcResponse::ShutdownAccepted
            ));
        }
        for task in [source_task, recipient_task] {
            timeout(Duration::from_secs(10), task)
                .await
                .context("own-device endpoint announcement runtime did not stop")?
                .context("join own-device endpoint announcement runtime")??;
        }
        let recipient_snapshot = load_runtime_state_snapshot(
            &recipient_state_dir,
            root.account_id(),
            recipient.identity().device_id(),
        )?;
        assert_eq!(recipient_snapshot.contacts.len(), 1);
        assert_eq!(recipient_snapshot.endpoint_publication_bindings.len(), 1);
        let expected_descriptor_directory =
            fs::canonicalize(directory.path().join("kilogram-received-endpoints"))?;
        assert!(recipient_snapshot.contacts.values().all(|contact| {
            contact
                .descriptor_file()
                .starts_with(&expected_descriptor_directory)
        }));
        Ok(())
    }

    #[test]
    fn runtime_ticket_compaction_covers_observation_policy_and_attempt_heads() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let root_dir = directory.path().join("root");
        let state_dir = directory.path().join("state");
        let peer_root_dir = directory.path().join("peer-root");
        create_account(root_dir.clone())?;
        create_account(peer_root_dir.clone())?;
        enroll_device(root_dir.clone(), state_dir.clone(), None)?;
        let root = AccountRootState::load(&root_dir)?;
        let peer_root = AccountRootState::load(&peer_root_dir)?;
        let device = DeviceState::load_or_create(&state_dir)?;
        let peer_device = DeviceIdentity::generate()?;
        let conversation = "runtime-ticket-all-chain-compaction";
        let conversation_id = ConversationId::from_label(conversation);
        let contact = SignedRuntimeContact::sign(
            device.identity(),
            root.account_id(),
            peer_root.account_id(),
            peer_device.device_id(),
            conversation.to_owned(),
            conversation_id,
            RoutePolicy::Auto,
            directory.path().join("peer.ticket"),
        )?;
        let contact_path = runtime_contact_relative_path(contact.contact_id());
        let contact_bytes = contact.encode()?;
        run_state_transaction(&state_dir, |transaction| {
            persist_runtime_record(&state_dir, &contact_path, &contact_bytes, transaction)
        })?;

        let peer_channel = ticket_publication_write_capability(&peer_device, root.account_id())
            .write_key()
            .channel_id();
        let started_at = unix_time_now()?;
        let mut previous_publication = None;
        let mut previous_observation = None;
        let mut previous_policy = None;
        let mut previous_publish_attempt = None;
        let mut previous_refresh_attempt = None;
        for index in 0..=MAX_RUNTIME_TICKET_CHAIN_RECORDS_BEFORE_COMPACTION {
            let publication = SignedTicketPublication::sign(
                &peer_device,
                peer_channel,
                peer_root.account_id(),
                root.account_id(),
                format!("peer-ticket-{index}"),
                started_at + index as u64,
                300,
                previous_publication.as_ref(),
            )?;
            let observation = SignedTicketPublicationObservation::sign(
                device.identity(),
                root.account_id(),
                &publication,
                started_at + index as u64,
                previous_observation.as_ref(),
            )?;
            let policy = SignedTicketAutomationPolicy::sign(
                device.identity(),
                root.account_id(),
                contact.contact_id(),
                conversation.to_owned(),
                peer_root.account_id(),
                started_at + index as u64,
                true,
                "https://ticket-store.invalid/".to_owned(),
                300,
                60,
                1,
                8,
                true,
                true,
                false,
                false,
                previous_policy.as_ref(),
            )?;
            let publish_attempt = SignedTicketAutomationAttempt::sign(
                device.identity(),
                root.account_id(),
                &policy,
                TicketAutomationAction::Publish,
                started_at + index as u64,
                None,
                previous_publish_attempt.as_ref(),
            )?;
            let refresh_attempt = SignedTicketAutomationAttempt::sign(
                device.identity(),
                root.account_id(),
                &policy,
                TicketAutomationAction::Refresh,
                started_at + index as u64,
                None,
                previous_refresh_attempt.as_ref(),
            )?;
            let records = [
                (
                    runtime_ticket_observation_relative_path(
                        observation.channel_id(),
                        observation.observation_generation(),
                        observation.observation_id()?,
                    ),
                    observation.encode()?,
                ),
                (
                    runtime_ticket_automation_policy_relative_path(policy.policy_id()?),
                    policy.encode()?,
                ),
                (
                    runtime_ticket_automation_attempt_relative_path(publish_attempt.attempt_id()?),
                    publish_attempt.encode()?,
                ),
                (
                    runtime_ticket_automation_attempt_relative_path(refresh_attempt.attempt_id()?),
                    refresh_attempt.encode()?,
                ),
            ];
            run_state_transaction(&state_dir, |transaction| {
                for (path, bytes) in &records {
                    persist_runtime_record(&state_dir, path, bytes, transaction)?;
                }
                Ok(())
            })?;
            previous_publication = Some(publication);
            previous_observation = Some(observation);
            previous_policy = Some(policy);
            previous_publish_attempt = Some(publish_attempt);
            previous_refresh_attempt = Some(refresh_attempt);
        }

        let report = compact_runtime_ticket_state_if_needed(&state_dir)?
            .context("multi-chain runtime ticket compaction was not triggered")?;
        assert_eq!(report.removed_records, 32);
        assert_eq!(report.retained_anchors, 4);
        let restarted = load_runtime_state_snapshot(
            &state_dir,
            root.account_id(),
            device.identity().device_id(),
        )?;
        assert_eq!(restarted.ticket_observations[&peer_channel].len(), 1);
        assert_eq!(
            restarted.ticket_automation_policies[&contact.contact_id()].len(),
            1
        );
        for action in [
            TicketAutomationAction::Publish,
            TicketAutomationAction::Refresh,
        ] {
            assert_eq!(
                restarted.ticket_automation_attempts[&(contact.contact_id(), action)].len(),
                1
            );
            assert_eq!(
                restarted.ticket_automation_attempts[&(contact.contact_id(), action)][0]
                    .generation(),
                9
            );
        }
        assert_eq!(
            restarted
                .ticket_checkpoint
                .context("multi-chain runtime ticket checkpoint is absent")?
                .anchors()
                .len(),
            4
        );
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn runtime_launch_profile_starts_and_stops_through_authenticated_ipc() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let root_dir = directory.path().join("root");
        let state_dir = directory.path().join("state");
        create_account(root_dir.clone())?;
        enroll_device(root_dir.clone(), state_dir.clone(), None)?;
        let root = AccountRootState::load(&root_dir)?;
        let device = DeviceState::load_or_create(&state_dir)?;
        let certificate = device.load_certificate()?;
        let device_list = root.publish_device_list(std::slice::from_ref(&certificate))?;
        let device_list_file = directory.path().join("device-list.snapshot");
        write_new_authority_file(&device_list_file, &device_list.encode()?)?;
        let profile_file = directory.path().join("runtime.launch.json");
        let ticket_file = directory.path().join("runtime.ticket");
        let ipc_file = directory.path().join("runtime.ipc.json");
        create_runtime_launch_profile(
            profile_file.clone(),
            RuntimeLaunchSettings {
                state_dir,
                allowed_requester_account_id: root.account_id(),
                device_list_file,
                peer_prekey_pool_files: Vec::new(),
                ticket_file: Some(ticket_file.clone()),
                relay_wait_seconds: 0,
                route_policy: RuntimeIpcRoutePolicy::DirectOnly,
                relay_url: None,
                poll_milliseconds: 20,
                retry_base_seconds: 1,
                retry_max_seconds: 1,
                auto_sync_seconds: 0,
                ipc_file: ipc_file.clone(),
            },
        )?;
        let task = tokio::spawn(runtime(runtime_options_from_profile(&profile_file)?));
        timeout(Duration::from_secs(10), async {
            while !ipc_file.is_file() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .context("profile runtime did not publish its IPC descriptor")?;
        assert!(matches!(
            kilogram_runtime_ipc::call(&ipc_file, RuntimeIpcCommand::Shutdown).await?,
            RuntimeIpcResponse::ShutdownAccepted
        ));
        timeout(Duration::from_secs(10), task)
            .await
            .context("profile runtime did not stop after IPC shutdown")?
            .context("join profile runtime")??;
        assert!(ticket_file.is_file());
        assert!(!ipc_file.exists());
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn runtime_publishes_and_refreshes_an_opaque_contact_ticket_idempotently() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let alice_root_dir = directory.path().join("alice-root");
        let bob_root_dir = directory.path().join("bob-root");
        let alice_state = directory.path().join("alice-state");
        let bob_state = directory.path().join("bob-state");
        create_account(alice_root_dir.clone())?;
        create_account(bob_root_dir.clone())?;
        enroll_device(alice_root_dir.clone(), alice_state.clone(), None)?;
        enroll_device(bob_root_dir.clone(), bob_state.clone(), None)?;

        let alice_root = AccountRootState::load(&alice_root_dir)?;
        let bob_root = AccountRootState::load(&bob_root_dir)?;
        let alice_device = DeviceState::load_or_create(&alice_state)?;
        let bob_device = DeviceState::load_or_create(&bob_state)?;
        let alice_certificate = alice_device.load_certificate()?;
        let bob_certificate = bob_device.load_certificate()?;
        let alice_devices =
            alice_root.publish_device_list(std::slice::from_ref(&alice_certificate))?;
        let bob_devices = bob_root.publish_device_list(std::slice::from_ref(&bob_certificate))?;
        let alice_devices_file = directory.path().join("alice.devices");
        let bob_devices_file = directory.path().join("bob.devices");
        write_new_authority_file(&alice_devices_file, &alice_devices.encode()?)?;
        write_new_authority_file(&bob_devices_file, &bob_devices.encode()?)?;

        let conversation = "ticket-publication-runtime-test";
        let conversation_id = ConversationId::from_label(conversation);
        let membership = alice_root
            .create_conversation_membership(conversation_id.scope_id(), &[bob_root.account_id()])?;
        alice_device.install_conversation_membership(&membership)?;
        bob_device.install_conversation_membership(&membership)?;

        let alice_ticket = directory.path().join("alice-runtime.ticket");
        let bob_ticket = directory.path().join("bob-runtime.ticket");
        let alice_ipc = directory.path().join("alice-runtime.ipc.json");
        let bob_ipc = directory.path().join("bob-runtime.ipc.json");
        let alice_task = tokio::spawn(runtime(RuntimeOptions {
            state_dir: alice_state.clone(),
            allowed_requester_account_id: bob_root.account_id(),
            device_list_file: alice_devices_file,
            peer_prekey_pool_files: Vec::new(),
            ticket_file: Some(alice_ticket.clone()),
            relay_wait_seconds: 0,
            route_policy: RoutePolicy::DirectOnly,
            relay_url: None,
            max_sessions: 0,
            idle_seconds: 0,
            poll_milliseconds: 500,
            retry_base_seconds: 1,
            retry_max_seconds: 1,
            auto_sync_seconds: 0,
            max_outbound_actions: 0,
            ipc_file: Some(alice_ipc.clone()),
        }));
        let bob_task = tokio::spawn(runtime(RuntimeOptions {
            state_dir: bob_state.clone(),
            allowed_requester_account_id: alice_root.account_id(),
            device_list_file: bob_devices_file,
            peer_prekey_pool_files: Vec::new(),
            ticket_file: Some(bob_ticket.clone()),
            relay_wait_seconds: 0,
            route_policy: RoutePolicy::DirectOnly,
            relay_url: None,
            max_sessions: 0,
            idle_seconds: 0,
            poll_milliseconds: 500,
            retry_base_seconds: 1,
            retry_max_seconds: 1,
            auto_sync_seconds: 0,
            max_outbound_actions: 0,
            ipc_file: Some(bob_ipc.clone()),
        }));
        timeout(Duration::from_secs(10), async {
            while !alice_ticket.is_file()
                || !bob_ticket.is_file()
                || !alice_ipc.is_file()
                || !bob_ipc.is_file()
            {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .context("ticket publication runtimes did not become ready")?;

        for (ipc, peer_account_id, descriptor_file) in [
            (&alice_ipc, bob_root.account_id(), &bob_ticket),
            (&bob_ipc, alice_root.account_id(), &alice_ticket),
        ] {
            assert!(matches!(
                kilogram_runtime_ipc::call(
                    ipc,
                    RuntimeIpcCommand::AddContact {
                        conversation: conversation.to_owned(),
                        expected_peer_account_id: peer_account_id,
                        descriptor_file: descriptor_file.clone(),
                    },
                )
                .await?,
                RuntimeIpcResponse::ContactAdded { inserted: true, .. }
            ));
        }

        let store_server = TicketStoreServer::bind(StoreConfig::local_test(
            directory.path().join("opaque-publication-store"),
        ))
        .await?;
        let service_base_url = format!("http://{}", store_server.local_addr());
        let (store_shutdown_sender, store_shutdown_receiver) = oneshot::channel();
        let store_task = tokio::spawn(store_server.run_until(async {
            store_shutdown_receiver
                .await
                .map_err(|_| anyhow::anyhow!("publication-store shutdown sender dropped"))
        }));
        let publication = kilogram_runtime_ipc::call(
            &bob_ipc,
            RuntimeIpcCommand::PublishOwnTicket {
                conversation: conversation.to_owned(),
                peer_account_id: alice_root.account_id(),
                service_base_url: service_base_url.clone(),
                ttl_seconds: 300,
            },
        )
        .await?;
        let RuntimeIpcResponse::OwnTicketPublished(publication) = publication else {
            bail!("Bob runtime returned an unexpected ticket-publication response")
        };
        assert_eq!(publication.publication_generation, 1);
        assert_eq!(publication.recipient_device_count, 1);
        assert_eq!(publication.local_store_status, "Inserted");

        for expected_store in ["Inserted", "AlreadyPresent"] {
            let refresh = kilogram_runtime_ipc::call(
                &alice_ipc,
                RuntimeIpcCommand::RefreshContactTicket {
                    conversation: conversation.to_owned(),
                    peer_account_id: bob_root.account_id(),
                    service_base_url: service_base_url.clone(),
                },
            )
            .await?;
            let RuntimeIpcResponse::ContactTicketRefreshed(refresh) = refresh else {
                bail!("Alice runtime returned an unexpected ticket-refresh response")
            };
            assert!(refresh.complete);
            assert_eq!(refresh.endpoint_candidate_count, 1);
            assert_eq!(refresh.refreshed_endpoint_candidate_count, 1);
            assert_eq!(refresh.results[0].publication_generation, Some(1));
            assert_eq!(
                refresh.results[0].local_observation_status.as_deref(),
                Some(expected_store)
            );
            assert_eq!(
                refresh.results[0].descriptor_publish_status.as_deref(),
                Some("atomic-replace")
            );
        }

        for (ipc, peer_account_id) in [
            (&alice_ipc, bob_root.account_id()),
            (&bob_ipc, alice_root.account_id()),
        ] {
            let configured = kilogram_runtime_ipc::call(
                ipc,
                RuntimeIpcCommand::ConfigureTicketAutomation {
                    conversation: conversation.to_owned(),
                    peer_account_id,
                    enabled: true,
                    service_base_url: service_base_url.clone(),
                    ttl_seconds: 300,
                    refresh_before_seconds: 60,
                    retry_base_seconds: 1,
                    retry_max_seconds: 4,
                    allow_ethernet: true,
                    allow_wifi: true,
                    allow_mobile: true,
                    allow_unknown_network: true,
                },
            )
            .await?;
            let status = match configured {
                RuntimeIpcResponse::TicketAutomationConfigured(status) => status,
                RuntimeIpcResponse::Error { message } => {
                    bail!("runtime rejected ticket automation: {message}")
                }
                _ => bail!("runtime returned an unexpected ticket-automation response"),
            };
            assert!(status.enabled && status.network_allowed);
            assert_eq!(
                status.execution_scope,
                "only-while-runtime-process-is-running"
            );
            assert!(!status.os_background_service_enabled);
        }
        let automation_convergence = timeout(Duration::from_secs(35), async {
            loop {
                let mut converged = true;
                for ipc in [&alice_ipc, &bob_ipc] {
                    let response =
                        kilogram_runtime_ipc::call(ipc, RuntimeIpcCommand::TicketAutomationStatus)
                            .await?;
                    let RuntimeIpcResponse::TicketAutomationStatus(statuses) = response else {
                        bail!("runtime returned unexpected ticket-automation status")
                    };
                    converged &= statuses.len() == 1
                        && statuses[0].publish.last_result == "success"
                        && statuses[0].refresh.last_result == "success";
                }
                if converged {
                    return Ok::<_, anyhow::Error>(());
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        })
        .await;
        if automation_convergence.is_err() {
            let alice_status =
                kilogram_runtime_ipc::call(&alice_ipc, RuntimeIpcCommand::TicketAutomationStatus)
                    .await?;
            let bob_status =
                kilogram_runtime_ipc::call(&bob_ipc, RuntimeIpcCommand::TicketAutomationStatus)
                    .await?;
            bail!(
                "automatic ticket publish/refresh did not converge: alice={alice_status:?}; bob={bob_status:?}"
            );
        }
        automation_convergence.context("automatic ticket publish/refresh did not converge")??;
        let disabled = kilogram_runtime_ipc::call(
            &alice_ipc,
            RuntimeIpcCommand::ConfigureTicketAutomation {
                conversation: conversation.to_owned(),
                peer_account_id: bob_root.account_id(),
                enabled: false,
                service_base_url: service_base_url.clone(),
                ttl_seconds: 300,
                refresh_before_seconds: 60,
                retry_base_seconds: 1,
                retry_max_seconds: 4,
                allow_ethernet: true,
                allow_wifi: true,
                allow_mobile: true,
                allow_unknown_network: true,
            },
        )
        .await?;
        let RuntimeIpcResponse::TicketAutomationConfigured(disabled) = disabled else {
            bail!("runtime returned unexpected disabled ticket-automation status")
        };
        assert!(!disabled.enabled);
        assert_eq!(disabled.policy_generation, 2);
        assert_eq!(disabled.publish.state, "disabled");
        assert_eq!(disabled.refresh.state, "disabled");
        let _ = store_shutdown_sender.send(());
        timeout(Duration::from_secs(5), store_task)
            .await
            .context("publication store did not finish")?
            .context("join publication store")??;
        for ipc in [&alice_ipc, &bob_ipc] {
            assert!(matches!(
                kilogram_runtime_ipc::call(ipc, RuntimeIpcCommand::Shutdown).await?,
                RuntimeIpcResponse::ShutdownAccepted
            ));
        }
        for (name, task) in [("Alice", alice_task), ("Bob", bob_task)] {
            timeout(Duration::from_secs(10), task)
                .await
                .with_context(|| format!("{name} publication runtime did not stop"))?
                .with_context(|| format!("join {name} publication runtime"))??;
        }

        let bob_snapshot = load_runtime_state_snapshot(
            &bob_state,
            bob_root.account_id(),
            bob_device.identity().device_id(),
        )?;
        assert!(
            bob_snapshot
                .ticket_publications
                .values()
                .filter_map(|records| records.last())
                .any(|publication| publication.generation() >= 2)
        );
        let alice_snapshot = load_runtime_state_snapshot(
            &alice_state,
            alice_root.account_id(),
            alice_device.identity().device_id(),
        )?;
        assert!(
            alice_snapshot
                .ticket_observations
                .values()
                .filter_map(|records| records.last())
                .any(|observation| observation.publication_generation() >= 2)
        );
        assert_eq!(
            alice_snapshot
                .ticket_automation_policies
                .values()
                .filter_map(|records| records.last())
                .map(SignedTicketAutomationPolicy::generation)
                .max(),
            Some(2)
        );
        for action in [
            TicketAutomationAction::Publish,
            TicketAutomationAction::Refresh,
        ] {
            assert!(
                alice_snapshot
                    .ticket_automation_attempts
                    .values()
                    .filter_map(|records| records.last())
                    .any(|attempt| attempt.action() == action)
            );
        }
        assert_eq!(
            ConnectionTicket::decode(&fs::read_to_string(&bob_ticket)?)?.listener_device_id(),
            bob_device.identity().device_id()
        );
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn runtime_refreshes_each_enrolled_endpoint_channel_independently() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let alice_root_dir = directory.path().join("alice-root");
        let alice_state = directory.path().join("alice-state");
        create_account(alice_root_dir.clone())?;
        enroll_device(alice_root_dir.clone(), alice_state.clone(), None)?;
        let alice_root = AccountRootState::load(&alice_root_dir)?;
        let alice_device = DeviceState::load_or_create(&alice_state)?;
        let alice_certificate = alice_device.load_certificate()?;
        let alice_devices =
            alice_root.publish_device_list(std::slice::from_ref(&alice_certificate))?;
        let alice_devices_file = directory.path().join("alice.devices");
        write_new_authority_file(&alice_devices_file, &alice_devices.encode()?)?;

        let bob_root = AccountRootState::create(directory.path().join("bob-root"))?;
        let mut bob_identities = Vec::new();
        let mut bob_certificates = Vec::new();
        let mut bob_pools = Vec::new();
        for index in 0..2 {
            let identity = DeviceIdentity::generate()?;
            let encryption = DeviceEncryptionIdentity::generate()?;
            let certificate = bob_root.issue_device_certificate(
                identity.device_id(),
                encryption.public_key(),
                &DeviceCapability::MESSAGING,
            )?;
            let pool = RatchetState::load_or_create(
                directory.path().join(format!("bob-ratchet-{index}")),
            )?
            .prekey_pool(
                &identity,
                4,
                unix_time_now()?,
                DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
            )?;
            bob_identities.push(identity);
            bob_certificates.push(certificate);
            bob_pools.push(pool);
        }
        let bob_devices = bob_root.publish_device_list(&bob_certificates)?;
        let bob_directory = AccountPrekeyDirectory::new(bob_devices, bob_pools)?;
        let conversation = "multi-endpoint-publication-refresh";
        let conversation_id = ConversationId::from_label(conversation);
        let membership = alice_root
            .create_conversation_membership(conversation_id.scope_id(), &[bob_root.account_id()])?;
        alice_device.install_conversation_membership(&membership)?;

        let mut bob_tickets = Vec::new();
        let mut bob_ticket_files = Vec::new();
        for (index, (identity, certificate)) in bob_identities
            .iter()
            .zip(bob_certificates.iter())
            .enumerate()
        {
            let ticket = ConnectionTicket::new(
                EndpointAddr::new(SecretKey::generate().public()),
                identity,
                certificate.clone(),
                bob_directory.clone(),
                alice_root.account_id(),
                RoutePolicy::Auto,
            )?;
            let path = directory.path().join(format!("bob-{index}.ticket"));
            fs::write(&path, ticket.encode()?)?;
            bob_tickets.push(ticket);
            bob_ticket_files.push(path);
        }

        let alice_ticket = directory.path().join("alice-runtime.ticket");
        let alice_ipc = directory.path().join("alice-runtime.ipc.json");
        let alice_task = tokio::spawn(runtime(RuntimeOptions {
            state_dir: alice_state.clone(),
            allowed_requester_account_id: bob_root.account_id(),
            device_list_file: alice_devices_file,
            peer_prekey_pool_files: Vec::new(),
            ticket_file: Some(alice_ticket.clone()),
            relay_wait_seconds: 0,
            route_policy: RoutePolicy::DirectOnly,
            relay_url: None,
            max_sessions: 0,
            idle_seconds: 0,
            poll_milliseconds: 500,
            retry_base_seconds: 1,
            retry_max_seconds: 1,
            auto_sync_seconds: 0,
            max_outbound_actions: 0,
            ipc_file: Some(alice_ipc.clone()),
        }));
        timeout(Duration::from_secs(10), async {
            while !alice_ticket.is_file() || !alice_ipc.is_file() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .context("multi-endpoint refresh runtime did not become ready")?;
        for ticket_file in &bob_ticket_files {
            assert!(matches!(
                kilogram_runtime_ipc::call(
                    &alice_ipc,
                    RuntimeIpcCommand::AddContact {
                        conversation: conversation.to_owned(),
                        expected_peer_account_id: bob_root.account_id(),
                        descriptor_file: ticket_file.clone(),
                    },
                )
                .await?,
                RuntimeIpcResponse::ContactAdded { inserted: true, .. }
            ));
        }
        let enrolled_snapshot = load_runtime_state_snapshot(
            &alice_state,
            alice_root.account_id(),
            alice_device.identity().device_id(),
        )?;
        assert_eq!(enrolled_snapshot.endpoint_publication_bindings.len(), 2);
        let contact = enrolled_snapshot
            .contacts
            .values()
            .next()
            .context("multi-endpoint test contact is absent")?;
        let legacy_binding_id = runtime_endpoint_publication_binding_id(
            alice_root.account_id(),
            contact.contact_id(),
            bob_identities[0].device_id(),
        );
        run_state_transaction(&alice_state, |transaction| {
            transaction.compact_runtime_record(runtime_endpoint_publication_binding_relative_path(
                legacy_binding_id,
            ))
        })?;
        let legacy_snapshot = load_runtime_state_snapshot(
            &alice_state,
            alice_root.account_id(),
            alice_device.identity().device_id(),
        )?;
        assert_eq!(legacy_snapshot.endpoint_publication_bindings.len(), 1);

        let expired_published_at = unix_time_now()?
            .saturating_sub(DEFAULT_PREKEY_POOL_VALIDITY_SECONDS.saturating_add(3_600));
        let mut expired_pools = Vec::new();
        for (index, identity) in bob_identities.iter().enumerate() {
            expired_pools.push(
                RatchetState::load_or_create(
                    directory
                        .path()
                        .join(format!("bob-expired-ratchet-{index}")),
                )?
                .prekey_pool(
                    identity,
                    4,
                    expired_published_at,
                    DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
                )?,
            );
        }
        let expired_directory =
            AccountPrekeyDirectory::new(bob_directory.device_list().clone(), expired_pools)?;
        for index in 0..2 {
            let expired_content = ConnectionTicketContent {
                version: TICKET_VERSION,
                endpoint: bob_tickets[index].endpoint().clone(),
                listener_certificate: bob_certificates[index].clone(),
                listener_directory: expired_directory.clone(),
                allowed_requester_account_id: alice_root.account_id(),
                ticket_publication_write_key: bob_tickets[index].ticket_publication_write_key(),
                route_policy: RoutePolicy::Auto,
            };
            let expired_ticket = ConnectionTicket {
                signature: bob_identities[index]
                    .sign(&ticket_signing_bytes(&expired_content)?)
                    .to_vec(),
                content: expired_content,
            };
            expired_ticket.verify_authenticated()?;
            assert!(expired_ticket.verify().is_err());
            let expired_ticket_text = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&expired_ticket)?);
            fs::write(&bob_ticket_files[index], expired_ticket_text)?;
            assert!(
                ConnectionTicket::decode(&fs::read_to_string(&bob_ticket_files[index])?).is_err()
            );
        }

        let store_server = TicketStoreServer::bind(StoreConfig::local_test(
            directory.path().join("multi-endpoint-publication-store"),
        ))
        .await?;
        let service_base_url = format!("http://{}", store_server.local_addr());
        let (store_shutdown_sender, store_shutdown_receiver) = oneshot::channel();
        let store_task = tokio::spawn(store_server.run_until(async {
            store_shutdown_receiver
                .await
                .map_err(|_| anyhow::anyhow!("multi-endpoint store shutdown sender dropped"))
        }));
        let store_client = TicketPublicationStoreClient::new(&service_base_url)?;
        let recipients = vec![(
            alice_certificate.device_id(),
            alice_certificate.encryption_public_key(),
        )];
        let mut encrypted_publications = Vec::new();
        for (identity, ticket) in bob_identities.iter().zip(&bob_tickets) {
            let capability = ticket_publication_write_capability(identity, alice_root.account_id());
            let channel_id = capability.write_key().channel_id();
            assert_eq!(
                channel_id,
                ticket.ticket_publication_write_key().channel_id()
            );
            let publication = SignedTicketPublication::sign(
                identity,
                channel_id,
                bob_root.account_id(),
                alice_root.account_id(),
                ticket.encode()?,
                unix_time_now()?,
                300,
                None,
            )?;
            encrypted_publications.push((
                EncryptedTicketPublication::seal(&publication, &recipients)?,
                capability,
            ));
        }

        store_client
            .put(&encrypted_publications[0].0, &encrypted_publications[0].1)
            .await?;
        let partial = kilogram_runtime_ipc::call(
            &alice_ipc,
            RuntimeIpcCommand::RefreshContactTicket {
                conversation: conversation.to_owned(),
                peer_account_id: bob_root.account_id(),
                service_base_url: service_base_url.clone(),
            },
        )
        .await?;
        let RuntimeIpcResponse::ContactTicketRefreshed(partial) = partial else {
            bail!("runtime returned an unexpected partial multi-endpoint refresh response")
        };
        assert!(!partial.complete);
        assert_eq!(partial.endpoint_candidate_count, 2);
        assert_eq!(
            partial.refreshed_endpoint_candidate_count, 1,
            "partial refresh details: {partial:#?}"
        );
        assert_eq!(
            partial
                .results
                .iter()
                .filter(|result| result.state == RuntimeIpcEndpointCandidateState::Stale)
                .count(),
            1
        );
        assert_eq!(
            ConnectionTicket::decode(&fs::read_to_string(&bob_ticket_files[0])?)?
                .listener_device_id(),
            bob_identities[0].device_id(),
            "refresh must replace an unusable expired descriptor through its durable channel binding"
        );
        let migrated_snapshot = load_runtime_state_snapshot(
            &alice_state,
            alice_root.account_id(),
            alice_device.identity().device_id(),
        )?;
        assert_eq!(
            migrated_snapshot.endpoint_publication_bindings.len(),
            2,
            "legacy enrollment must receive a durable binding before the remote fetch"
        );
        assert!(
            ConnectionTicket::decode(&fs::read_to_string(&bob_ticket_files[1])?).is_err(),
            "the still-unpublished second endpoint must remain unusable while expired"
        );

        store_client
            .put(&encrypted_publications[1].0, &encrypted_publications[1].1)
            .await?;
        let complete = kilogram_runtime_ipc::call(
            &alice_ipc,
            RuntimeIpcCommand::RefreshContactTicket {
                conversation: conversation.to_owned(),
                peer_account_id: bob_root.account_id(),
                service_base_url: service_base_url.clone(),
            },
        )
        .await?;
        let RuntimeIpcResponse::ContactTicketRefreshed(complete) = complete else {
            bail!("runtime returned an unexpected complete multi-endpoint refresh response")
        };
        assert!(complete.complete);
        assert_eq!(complete.refreshed_endpoint_candidate_count, 2);
        assert_eq!(
            complete
                .results
                .iter()
                .filter_map(|result| result.channel_id.as_deref())
                .collect::<BTreeSet<_>>()
                .len(),
            2
        );
        let snapshot = load_runtime_state_snapshot(
            &alice_state,
            alice_root.account_id(),
            alice_device.identity().device_id(),
        )?;
        assert_eq!(snapshot.ticket_observations.len(), 2);
        assert_eq!(snapshot.endpoint_publication_bindings.len(), 2);
        for (ticket_file, identity) in bob_ticket_files.iter().zip(&bob_identities) {
            assert_eq!(
                ConnectionTicket::decode(&fs::read_to_string(ticket_file)?)?.listener_device_id(),
                identity.device_id()
            );
        }

        let _ = store_shutdown_sender.send(());
        timeout(Duration::from_secs(5), store_task)
            .await
            .context("multi-endpoint publication store did not finish")?
            .context("join multi-endpoint publication store")??;
        assert!(matches!(
            kilogram_runtime_ipc::call(&alice_ipc, RuntimeIpcCommand::Shutdown).await?,
            RuntimeIpcResponse::ShutdownAccepted
        ));
        timeout(Duration::from_secs(10), alice_task)
            .await
            .context("multi-endpoint refresh runtime did not stop")?
            .context("join multi-endpoint refresh runtime")??;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn live_runtime_applies_revocation_republishes_ticket_and_retires_ratchet() -> Result<()>
    {
        let directory = tempfile::tempdir()?;
        let root_dir = directory.path().join("root");
        let retained_state = directory.path().join("retained-state");
        let removed_state = directory.path().join("removed-state");
        create_account(root_dir.clone())?;
        enroll_device(root_dir.clone(), retained_state.clone(), None)?;
        enroll_device(root_dir.clone(), removed_state.clone(), None)?;
        let root = AccountRootState::load(&root_dir)?;
        let retained = DeviceState::load_or_create(&retained_state)?;
        let removed = DeviceState::load_or_create(&removed_state)?;
        let retained_certificate = retained.load_certificate()?;
        let removed_certificate = removed.load_certificate()?;
        let initial_list =
            root.publish_device_list(&[retained_certificate.clone(), removed_certificate.clone()])?;
        let initial_list_file = directory.path().join("devices-before.snapshot");
        write_new_authority_file(&initial_list_file, &initial_list.encode()?)?;

        let now = unix_time_now()?;
        let removed_pool = RatchetState::load_or_create(&removed_state)?.prekey_pool(
            removed.identity(),
            4,
            now,
            DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
        )?;
        let removed_pool_file = directory.path().join("removed.prekeys");
        write_new_authority_file(&removed_pool_file, &removed_pool.encode()?)?;
        let mut retained_ratchet = RatchetState::load_or_create(&retained_state)?;
        retained_ratchet.observe_prekey_pool(&removed_pool, now)?;
        retained_ratchet.encrypt_with_pool(
            retained.identity(),
            &removed_pool,
            "retire this local session",
            now,
        )?;
        drop(retained_ratchet);
        EncryptedStateVault::open_or_create(&retained_state)?.migrate_legacy_snapshot()?;

        let profile_file = directory.path().join("runtime.launch.json");
        let ticket_file = directory.path().join("runtime.ticket");
        let ipc_file = directory.path().join("runtime.ipc.json");
        create_runtime_launch_profile(
            profile_file.clone(),
            RuntimeLaunchSettings {
                state_dir: retained_state.clone(),
                allowed_requester_account_id: root.account_id(),
                device_list_file: initial_list_file.clone(),
                peer_prekey_pool_files: vec![removed_pool_file.clone()],
                ticket_file: Some(ticket_file.clone()),
                relay_wait_seconds: 0,
                route_policy: RuntimeIpcRoutePolicy::DirectOnly,
                relay_url: None,
                poll_milliseconds: 20,
                retry_base_seconds: 1,
                retry_max_seconds: 1,
                auto_sync_seconds: 0,
                ipc_file: ipc_file.clone(),
            },
        )?;
        let task = tokio::spawn(runtime(runtime_options_from_profile(&profile_file)?));
        timeout(Duration::from_secs(10), async {
            while !ipc_file.is_file() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .context("runtime did not publish its IPC descriptor")?;

        let (_, refreshed_list) =
            root.revoke_and_publish_device_list(removed.identity().device_id())?;
        let refreshed_list_file = directory.path().join("devices-after.snapshot");
        write_new_authority_file(&refreshed_list_file, &refreshed_list.encode()?)?;
        let response = kilogram_runtime_ipc::call(
            &ipc_file,
            RuntimeIpcCommand::ApplyOwnDeviceDirectory {
                device_list_file: fs::canonicalize(&refreshed_list_file)?,
            },
        )
        .await?;
        let RuntimeIpcResponse::OwnDeviceDirectoryApplied(update) = response else {
            bail!("runtime returned an unexpected directory-update response")
        };
        assert_eq!(update.previous_authority_revision, initial_list.revision());
        assert_eq!(update.authority_revision, refreshed_list.revision());
        assert_eq!(update.active_device_count, 1);
        assert_eq!(
            update.removed_device_ids,
            vec![removed.identity().device_id()]
        );
        assert_eq!(update.ratchet_session_records_retired, 1);
        assert_eq!(update.prekey_observations_retired, 1);
        assert!(update.ticket_published);
        assert!(update.launch_profile_update_required);
        assert_eq!(update.directory_status.receipt_generation, Some(1));
        assert_eq!(
            update.directory_status.profile_convergence_status,
            "convergence-required"
        );
        assert_eq!(
            update.history_availability_status,
            "existing-copies-remain-readable"
        );
        let published = ConnectionTicket::decode(&fs::read_to_string(&ticket_file)?)?;
        assert_eq!(
            published.listener_authority_snapshot().revision(),
            refreshed_list.revision()
        );
        assert!(
            published
                .listener_directory()
                .certificate_for(removed.identity().device_id())
                .is_none()
        );
        let retry = kilogram_runtime_ipc::call(
            &ipc_file,
            RuntimeIpcCommand::ApplyOwnDeviceDirectory {
                device_list_file: fs::canonicalize(&refreshed_list_file)?,
            },
        )
        .await?;
        let RuntimeIpcResponse::OwnDeviceDirectoryApplied(retry) = retry else {
            bail!("runtime returned an unexpected retry response")
        };
        assert_eq!(retry.previous_authority_revision, refreshed_list.revision());
        assert_eq!(retry.authority_revision, refreshed_list.revision());
        assert_eq!(retry.ratchet_session_records_retired, 0);
        assert_eq!(retry.prekey_observations_retired, 0);
        assert_eq!(retry.directory_status.receipt_generation, Some(1));
        let retained_ratchet = RatchetState::load_or_create(&retained_state)?;
        assert!(!retained_ratchet.has_session(removed.identity().device_id()));
        assert_eq!(
            kilogram_runtime_ipc::call(&ipc_file, RuntimeIpcCommand::Shutdown).await?,
            RuntimeIpcResponse::ShutdownAccepted
        );
        timeout(Duration::from_secs(10), task)
            .await
            .context("runtime did not stop after directory-update test")?
            .context("join directory-update runtime")??;
        fs::remove_file(&removed_pool_file)?;

        let restarted = tokio::spawn(runtime(runtime_options_from_profile(&profile_file)?));
        timeout(Duration::from_secs(10), async {
            while !ipc_file.is_file() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .context("runtime did not restart from its authenticated directory receipt")?;
        let status =
            kilogram_runtime_ipc::call(&ipc_file, RuntimeIpcCommand::OwnDeviceDirectoryStatus)
                .await?;
        let RuntimeIpcResponse::OwnDeviceDirectoryStatus(status) = status else {
            bail!("runtime returned an unexpected directory-status response")
        };
        assert_eq!(status.authority_revision, refreshed_list.revision());
        assert_eq!(status.active_device_count, 1);
        assert_eq!(status.receipt_generation, Some(1));
        assert_eq!(status.profile_convergence_status, "convergence-required");
        assert_eq!(
            status.restart_recovery_status,
            "authenticated-receipt-recovered"
        );
        let republished = ConnectionTicket::decode(&fs::read_to_string(&ticket_file)?)?;
        assert_eq!(
            republished.listener_authority_snapshot().revision(),
            refreshed_list.revision()
        );
        assert!(
            republished
                .listener_directory()
                .certificate_for(removed.identity().device_id())
                .is_none()
        );
        assert_eq!(
            kilogram_runtime_ipc::call(&ipc_file, RuntimeIpcCommand::Shutdown).await?,
            RuntimeIpcResponse::ShutdownAccepted
        );
        timeout(Duration::from_secs(10), restarted)
            .await
            .context("receipt-recovered runtime did not stop")?
            .context("join receipt-recovered runtime")??;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn runtime_outbox_delivers_and_automatic_sync_converges() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let alice_root_dir = directory.path().join("alice-root");
        let bob_root_dir = directory.path().join("bob-root");
        let alice_state = directory.path().join("alice-state");
        let bob_state = directory.path().join("bob-state");
        create_account(alice_root_dir.clone())?;
        create_account(bob_root_dir.clone())?;
        enroll_device(alice_root_dir.clone(), alice_state.clone(), None)?;
        enroll_device(bob_root_dir.clone(), bob_state.clone(), None)?;

        let alice_root = AccountRootState::load(&alice_root_dir)?;
        let bob_root = AccountRootState::load(&bob_root_dir)?;
        let alice_device = DeviceState::load_or_create(&alice_state)?;
        let bob_device = DeviceState::load_or_create(&bob_state)?;
        let alice_certificate = alice_device.load_certificate()?;
        let bob_certificate = bob_device.load_certificate()?;
        let alice_devices =
            alice_root.publish_device_list(std::slice::from_ref(&alice_certificate))?;
        let bob_devices = bob_root.publish_device_list(std::slice::from_ref(&bob_certificate))?;
        let alice_devices_file = directory.path().join("alice.devices");
        let bob_devices_file = directory.path().join("bob.devices");
        write_new_authority_file(&alice_devices_file, &alice_devices.encode()?)?;
        write_new_authority_file(&bob_devices_file, &bob_devices.encode()?)?;

        let conversation_label = "runtime-outbox-process-test";
        let conversation_id = ConversationId::from_label(conversation_label);
        let membership = alice_root
            .create_conversation_membership(conversation_id.scope_id(), &[bob_root.account_id()])?;
        alice_device.install_conversation_membership(&membership)?;
        bob_device.install_conversation_membership(&membership)?;

        let bob_ticket = directory.path().join("bob-runtime.ticket");
        let bob_ipc = directory.path().join("bob-runtime.ipc.json");
        let bob_task = tokio::spawn(runtime(RuntimeOptions {
            state_dir: bob_state.clone(),
            allowed_requester_account_id: alice_root.account_id(),
            device_list_file: bob_devices_file,
            peer_prekey_pool_files: Vec::new(),
            ticket_file: Some(bob_ticket.clone()),
            relay_wait_seconds: 0,
            route_policy: RoutePolicy::DirectOnly,
            relay_url: None,
            max_sessions: 0,
            idle_seconds: 0,
            poll_milliseconds: 20,
            retry_base_seconds: 1,
            retry_max_seconds: 1,
            auto_sync_seconds: 0,
            max_outbound_actions: 0,
            ipc_file: Some(bob_ipc.clone()),
        }));
        timeout(Duration::from_secs(10), async {
            while !bob_ticket.is_file() || !bob_ipc.is_file() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .context("Bob runtime did not publish its descriptor")?;

        let alice_ipc = directory.path().join("alice-runtime.ipc.json");
        let alice_task = tokio::spawn(runtime(RuntimeOptions {
            state_dir: alice_state.clone(),
            allowed_requester_account_id: bob_root.account_id(),
            device_list_file: alice_devices_file,
            peer_prekey_pool_files: Vec::new(),
            ticket_file: Some(directory.path().join("alice-runtime.ticket")),
            relay_wait_seconds: 0,
            route_policy: RoutePolicy::DirectOnly,
            relay_url: None,
            max_sessions: 0,
            idle_seconds: 20,
            poll_milliseconds: 20,
            retry_base_seconds: 1,
            retry_max_seconds: 1,
            auto_sync_seconds: 1,
            max_outbound_actions: 0,
            ipc_file: Some(alice_ipc.clone()),
        }));

        timeout(Duration::from_secs(10), async {
            while !alice_ipc.is_file() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .context("Alice runtime did not publish its IPC descriptor")?;
        assert!(matches!(
            kilogram_runtime_ipc::call(&alice_ipc, RuntimeIpcCommand::Ping).await?,
            RuntimeIpcResponse::Pong {
                account_id,
                device_id,
            } if account_id == alice_root.account_id()
                && device_id == alice_device.identity().device_id()
        ));
        assert!(matches!(
            kilogram_runtime_ipc::call(
                &alice_ipc,
                RuntimeIpcCommand::AddContact {
                    conversation: conversation_label.to_owned(),
                    expected_peer_account_id: bob_root.account_id(),
                    descriptor_file: bob_ticket,
                },
            )
            .await?,
            RuntimeIpcResponse::ContactAdded {
                peer_account_id,
                peer_device_id,
                inserted: true,
                ..
            } if peer_account_id == bob_root.account_id()
                && peer_device_id == bob_device.identity().device_id()
        ));
        assert!(matches!(
            kilogram_runtime_ipc::call(&alice_ipc, RuntimeIpcCommand::ConversationList).await?,
            RuntimeIpcResponse::ConversationList(conversations)
                if conversations.len() == 1
                    && conversations[0].conversation_label == conversation_label
                    && conversations[0].message_count == 0
                    && conversations[0].latest_message.is_none()
        ));
        assert!(matches!(
            kilogram_runtime_ipc::call(
                &alice_ipc,
                RuntimeIpcCommand::HistoryPage {
                    conversation: conversation_label.to_owned(),
                    cursor: None,
                    limit: 50,
                },
            )
            .await?,
            RuntimeIpcResponse::HistoryPage(page)
                if page.total_messages == 0
                    && page.messages.is_empty()
                    && page.next_cursor.is_none()
        ));
        let request_id = RuntimeIpcRequestId::generate()?;
        let queue_command = RuntimeIpcCommand::QueueMessage {
            request_id,
            conversation: conversation_label.to_owned(),
            peer_account_id: bob_root.account_id(),
            message: "durable runtime outbox message".to_owned(),
        };
        assert!(matches!(
            kilogram_runtime_ipc::call(&alice_ipc, queue_command.clone()).await?,
            RuntimeIpcResponse::MessageQueued {
                queue_id: returned,
                inserted: true,
                ..
            } if returned == request_id.to_string()
        ));
        assert!(matches!(
            kilogram_runtime_ipc::call(&alice_ipc, queue_command).await?,
            RuntimeIpcResponse::MessageQueued {
                queue_id: returned,
                inserted: false,
                ..
            } if returned == request_id.to_string()
        ));
        assert!(matches!(
            kilogram_runtime_ipc::call(&alice_ipc, RuntimeIpcCommand::OutboxStatus).await?,
            RuntimeIpcResponse::OutboxStatus(status)
                if status.queue_count == 1 && status.items.len() == 1
        ));

        timeout(Duration::from_secs(30), async {
            loop {
                let delivered = matches!(
                    kilogram_runtime_ipc::call(&alice_ipc, RuntimeIpcCommand::OutboxStatus)
                        .await?,
                    RuntimeIpcResponse::OutboxStatus(status)
                        if status.pending_count == 0 && status.delivered_count == 1
                );
                if delivered {
                    return Ok::<_, anyhow::Error>(());
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .context("runtime outbox delivery/sync did not converge")??;
        tokio::time::sleep(Duration::from_millis(1_200)).await;
        for ipc in [&alice_ipc, &bob_ipc] {
            assert_eq!(
                kilogram_runtime_ipc::call(ipc, RuntimeIpcCommand::Shutdown).await?,
                RuntimeIpcResponse::ShutdownAccepted
            );
        }
        let (alice_result, bob_result) = timeout(Duration::from_secs(10), async {
            tokio::join!(alice_task, bob_task)
        })
        .await
        .context("runtime outbox processes did not stop")?;
        alice_result.context("join Alice runtime")??;
        bob_result.context("join Bob runtime")??;

        let alice_events = open_event_store(&alice_state)?
            .load_authorized_conversation(conversation_id, &membership)?;
        let bob_events = open_event_store(&bob_state)?
            .load_authorized_conversation(conversation_id, &membership)?;
        assert_eq!(alice_events, bob_events);
        assert_eq!(alice_events.len(), 2);
        assert_eq!(
            alice_events
                .iter()
                .filter(|stored| matches!(
                    stored.event.event().payload(),
                    EventPayload::RatchetText { .. }
                ))
                .count(),
            1
        );
        let snapshot = load_runtime_state_snapshot(
            &alice_state,
            alice_root.account_id(),
            alice_device.identity().device_id(),
        )?;
        assert_eq!(snapshot.queued.len(), 1);
        assert_eq!(snapshot.materialized.len(), 1);
        assert_eq!(snapshot.delivered.len(), 1);
        assert_eq!(snapshot.pending_count(), 0);
        let conversations = collect_runtime_conversation_list(&alice_state)?;
        assert_eq!(conversations.len(), 1);
        assert_eq!(conversations[0].message_count, 1);
        assert_eq!(
            conversations[0]
                .latest_message
                .as_ref()
                .map(|message| message.body.as_str()),
            Some("durable runtime outbox message")
        );
        let history = collect_runtime_history_page(
            &alice_state,
            conversation_label,
            None,
            MAX_RUNTIME_HISTORY_PAGE_SIZE,
        )?;
        assert_eq!(history.total_messages, 1);
        assert_eq!(history.messages.len(), 1);
        assert_eq!(history.messages[0].body, "durable runtime outbox message");
        assert!(history.next_cursor.is_none());
        Ok(())
    }

    #[test]
    fn seeded_history_is_signed_chained_and_persistent() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let account_dir = directory.path().join("account");
        let peer_account_dir = directory.path().join("peer-account");
        let state_dir = directory.path().join("seeded-device");
        let peer_state_dir = directory.path().join("peer-device");
        let peer_certificate_file = directory.path().join("peer-device.cert");
        let peer_device_list_file = directory.path().join("peer-devices.snapshot");
        let peer_prekey_pool_file = directory.path().join("peer-device.prekey-pool");
        let conversation = "seed-history-test";
        create_account(account_dir.clone())?;
        enroll_device(account_dir.clone(), state_dir.clone(), None)?;
        let account = AccountRootState::load(&account_dir)?;
        let peer_account = AccountRootState::create(&peer_account_dir)?;
        let peer = DeviceState::load_or_create(&peer_state_dir)?;
        let peer_certificate = peer_account.issue_device_certificate(
            peer.identity().device_id(),
            peer.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        write_new_authority_file(&peer_certificate_file, &peer_certificate.encode()?)?;
        let peer_device_list =
            peer_account.publish_device_list(std::slice::from_ref(&peer_certificate))?;
        peer.install_certificate(&peer_certificate)?;
        peer.install_own_authority_snapshot(peer_device_list.authority_snapshot())?;
        write_new_authority_file(&peer_device_list_file, &peer_device_list.encode()?)?;
        let peer_prekey_pool = RatchetState::load_or_create(&peer_state_dir)?.prekey_pool(
            peer.identity(),
            4,
            unix_time_now()?,
            DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
        )?;
        write_new_authority_file(&peer_prekey_pool_file, &peer_prekey_pool.encode()?)?;
        let membership = account.create_conversation_membership(
            ConversationId::from_label(conversation).scope_id(),
            &[peer_account.account_id()],
        )?;
        DeviceState::load_or_create(&state_dir)?.install_conversation_membership(&membership)?;
        seed_history(
            state_dir.clone(),
            conversation.to_owned(),
            3,
            "fixture".to_owned(),
            peer_certificate_file,
            peer_device_list_file,
            peer_prekey_pool_file,
        )?;

        let store = open_event_store(&state_dir)?;
        let conversation_id = ConversationId::from_label(conversation);
        let events = store.load_authorized_conversation(conversation_id, &membership)?;
        assert_eq!(events.len(), 3);
        assert_eq!(store.frontier(conversation_id)?.len(), 1);
        let local_messages = open_local_message_store(&state_dir)?;
        let seeded_device = DeviceState::load_or_create(&state_dir)?;
        let mut bodies = Vec::with_capacity(events.len());
        for stored in &events {
            let projection = local_messages.get(stored.id)?;
            bodies.push(projection.open(
                stored.event.event(),
                seeded_device.identity().device_id(),
                seeded_device.encryption(),
            )?);
        }
        bodies.sort();
        assert_eq!(bodies, ["fixture-1", "fixture-2", "fixture-3"]);
        assert!(
            events
                .iter()
                .all(|stored| stored.event.verify_for_membership(&membership).is_ok())
        );

        let vault = EncryptedStateVault::open_or_create(&state_dir)?;
        vault.migrate_legacy_snapshot()?;
        drop(vault);
        let guard = VaultDualWriteGuard::prepare(&state_dir)?
            .context("expected vault guard for immutable history canary")?;
        let primary = open_immutable_read_repositories(&state_dir)?;
        assert_eq!(primary.primary, "encrypted-vault");
        assert_eq!(primary.shadow, "legacy-verified");
        assert_eq!(primary.event_record_count, 6);
        assert_eq!(primary.local_projection_record_count, 3);
        assert_eq!(
            primary
                .events
                .load_authorized_conversation(conversation_id, &membership)?,
            events
        );
        for stored in &events {
            assert_eq!(
                primary.local_messages.get(stored.id)?,
                local_messages.get(stored.id)?
            );
        }
        let immutable_inventory = primary
            .events
            .authorized_inventory(conversation_id, &membership)?;
        assert_eq!(immutable_inventory.len(), 3);
        assert_eq!(
            primary.events.authorized_events_by_id(
                conversation_id,
                &immutable_inventory,
                &membership,
            )?,
            events
                .iter()
                .map(|stored| stored.event.clone())
                .collect::<Vec<_>>()
        );

        let source_certificate = seeded_device.load_certificate()?;
        let recovery_identity = DeviceIdentity::generate()?;
        let recovery_encryption = DeviceEncryptionIdentity::generate()?;
        let recovery_certificate = account.issue_device_certificate(
            recovery_identity.device_id(),
            recovery_encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let rewrap_device_list = account
            .publish_device_list(&[source_certificate.clone(), recovery_certificate.clone()])?;
        let events_backup = directory.path().join("events-vault-primary-test");
        let projections_backup = directory.path().join("projections-vault-primary-test");
        fs::rename(state_dir.join("events"), &events_backup)?;
        fs::rename(state_dir.join("local-messages"), &projections_backup)?;
        let rewrap_bundle = build_history_rewrap_bundle(
            &seeded_device,
            &source_certificate,
            rewrap_device_list,
            recovery_certificate.device_id(),
            conversation_id,
            &membership,
            primary.events.as_ref(),
            primary.local_messages.as_ref(),
            0,
            3,
        )?;
        assert_eq!(rewrap_bundle.entries().len(), 3);
        assert!(rewrap_bundle.is_complete_source_inventory());
        fs::rename(&events_backup, state_dir.join("events"))?;
        fs::rename(&projections_backup, state_dir.join("local-messages"))?;
        guard.finish()?;

        let peer_events = open_event_store(&peer_state_dir)?;
        let peer_messages = open_local_message_store(&peer_state_dir)?;
        RatchetState::load_or_create(&peer_state_dir)?;
        let peer_vault = EncryptedStateVault::open_or_create(&peer_state_dir)?;
        peer_vault.migrate_legacy_snapshot()?;
        drop(peer_vault);
        let peer_guard = VaultDualWriteGuard::prepare(&peer_state_dir)?
            .context("expected vault guard for command-local sync overlay canary")?;
        let peer_reads = open_immutable_read_repositories(&peer_state_dir)?;
        assert_eq!(peer_reads.primary, "encrypted-vault");
        assert_eq!(peer_reads.event_record_count, 0);
        assert_eq!(peer_reads.local_projection_record_count, 0);
        let peer_sync = DecryptingSessionStore::new(
            &peer_state_dir,
            &peer_events,
            &peer_messages,
            &peer,
            peer_account.account_id(),
            peer_reads,
        );
        let authorized_events = events
            .iter()
            .map(|stored| stored.event.clone())
            .collect::<Vec<_>>();
        peer_sync.put_events(&authorized_events[..2], &membership)?;
        assert_eq!(peer_sync.inventory(conversation_id, &membership)?.len(), 2);
        assert_eq!(peer_sync.overlay_event_count()?, 2);
        assert_eq!(peer_sync.overlay_local_projection_count()?, 2);
        peer_sync.put_events(&authorized_events[2..], &membership)?;
        assert_eq!(peer_sync.inventory(conversation_id, &membership)?.len(), 3);
        assert_eq!(peer_sync.overlay_event_count()?, 3);
        assert_eq!(peer_sync.overlay_local_projection_count()?, 3);
        let mut peer_bodies = Vec::with_capacity(events.len());
        for stored in &events {
            peer_bodies.push(peer_messages.get(stored.id)?.open(
                stored.event.event(),
                peer.identity().device_id(),
                peer.encryption(),
            )?);
        }
        peer_bodies.sort();
        assert_eq!(peer_bodies, ["fixture-1", "fixture-2", "fixture-3"]);
        drop(peer_sync);
        peer_guard.finish()?;
        assert_eq!(
            EncryptedStateVault::open_existing(&peer_state_dir)?
                .verify_against_legacy()?
                .mirror_generation(),
            3
        );
        Ok(())
    }

    #[test]
    fn sync_store_rejects_event_without_a_local_recipient_box() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let local_state = DeviceState::load_or_create(directory.path().join("local-state"))?;
        let local_root = AccountRootState::create(directory.path().join("local-root"))?;
        let peer_root = AccountRootState::create(directory.path().join("peer-root"))?;
        let local_certificate = local_root.issue_device_certificate(
            local_state.identity().device_id(),
            local_state.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let peer_identity = DeviceIdentity::generate()?;
        let peer_encryption = DeviceEncryptionIdentity::generate()?;
        let peer_certificate = peer_root.issue_device_certificate(
            peer_identity.device_id(),
            peer_encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let outsider_identity = DeviceIdentity::generate()?;
        let outsider_certificate = local_root.issue_device_certificate(
            outsider_identity.device_id(),
            DeviceEncryptionIdentity::generate()?.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let outsider_device_list =
            local_root.publish_device_list(std::slice::from_ref(&outsider_certificate))?;
        local_state.install_certificate(&local_certificate)?;
        local_state.install_own_authority_snapshot(outsider_device_list.authority_snapshot())?;
        let mut outsider_ratchet =
            RatchetState::load_or_create(directory.path().join("outsider-ratchet"))?;
        let outsider_bundle = outsider_ratchet.prekey_bundle(&outsider_identity)?;
        let mut peer_ratchet = RatchetState::load_or_create(directory.path().join("peer-ratchet"))?;
        let (sender_ratchet_identity, ciphertext, _) =
            peer_ratchet.encrypt(&peer_identity, &outsider_bundle, "must not reach disk")?;
        let conversation_id = ConversationId::from_label("missing-local-recipient");
        let membership = local_root.create_conversation_membership(
            conversation_id.scope_id(),
            &[peer_root.account_id()],
        )?;
        let event = AuthorizedEvent::new(
            SignedEvent::sign_ratchet_text(
                &peer_identity,
                conversation_id,
                0,
                Vec::new(),
                outsider_device_list,
                sender_ratchet_identity,
                vec![RatchetRecipient::new(
                    outsider_identity.device_id(),
                    ciphertext,
                )?],
            )?,
            peer_certificate,
            peer_root.authority_snapshot()?,
        )?;
        let store = EventStore::open(directory.path().join("events"))?;
        let local_messages = LocalMessageStore::open(directory.path().join("local-messages"))?;
        let guarded = DecryptingSessionStore::new(
            directory.path(),
            &store,
            &local_messages,
            &local_state,
            local_root.account_id(),
            empty_immutable_read_repositories()?,
        );

        assert!(matches!(
            guarded.put_events(&[event], &membership),
            Err(StoreError::Protocol(
                kilogram_protocol::ProtocolError::LocalProjectionDeviceNotParticipant(device_id)
            )) if device_id == local_state.identity().device_id()
        ));
        assert!(
            store
                .authorized_inventory(conversation_id, &membership)?
                .is_empty()
        );
        assert!(guarded.inventory(conversation_id, &membership)?.is_empty());
        assert_eq!(guarded.overlay_event_count()?, 0);
        assert_eq!(guarded.overlay_local_projection_count()?, 0);
        Ok(())
    }

    #[test]
    fn sync_store_creates_a_local_projection_for_a_received_event() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let local_state = DeviceState::load_or_create(directory.path().join("local-state"))?;
        let local_root = AccountRootState::create(directory.path().join("local-root"))?;
        let peer_root = AccountRootState::create(directory.path().join("peer-root"))?;
        let peer_identity = DeviceIdentity::generate()?;
        let peer_encryption = DeviceEncryptionIdentity::generate()?;
        let peer_certificate = peer_root.issue_device_certificate(
            peer_identity.device_id(),
            peer_encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let local_certificate = local_root.issue_device_certificate(
            local_state.identity().device_id(),
            local_state.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let local_device_list =
            local_root.publish_device_list(std::slice::from_ref(&local_certificate))?;
        local_state.install_certificate(&local_certificate)?;
        local_state.install_own_authority_snapshot(local_device_list.authority_snapshot())?;
        let conversation_id = ConversationId::from_label("received-local-projection");
        let membership = local_root.create_conversation_membership(
            conversation_id.scope_id(),
            &[peer_root.account_id()],
        )?;
        let mut local_ratchet = RatchetState::load_or_create(directory.path())?;
        let local_bundle = local_ratchet.prekey_bundle(local_state.identity())?;
        let mut peer_ratchet = RatchetState::load_or_create(directory.path().join("peer-ratchet"))?;
        let (sender_ratchet_identity, ciphertext, _) =
            peer_ratchet.encrypt(&peer_identity, &local_bundle, "received through sync")?;
        let event = AuthorizedEvent::new(
            SignedEvent::sign_ratchet_text(
                &peer_identity,
                conversation_id,
                0,
                Vec::new(),
                local_device_list,
                sender_ratchet_identity,
                vec![RatchetRecipient::new(
                    local_state.identity().device_id(),
                    ciphertext,
                )?],
            )?,
            peer_certificate,
            peer_root.authority_snapshot()?,
        )?;
        let event_id = event.event().event_id()?;
        let store = EventStore::open(directory.path().join("events"))?;
        let local_messages = LocalMessageStore::open(directory.path().join("local-messages"))?;
        let guarded = DecryptingSessionStore::new(
            directory.path(),
            &store,
            &local_messages,
            &local_state,
            local_root.account_id(),
            empty_immutable_read_repositories()?,
        );

        guarded.put_events(std::slice::from_ref(&event), &membership)?;
        guarded.put_events(std::slice::from_ref(&event), &membership)?;
        assert_eq!(
            local_messages.get(event_id)?.open(
                event.event(),
                local_state.identity().device_id(),
                local_state.encryption(),
            )?,
            "received through sync"
        );
        assert_eq!(
            store.authorized_inventory(conversation_id, &membership)?,
            vec![event_id]
        );
        assert_eq!(
            guarded.inventory(conversation_id, &membership)?,
            vec![event_id]
        );
        assert_eq!(guarded.overlay_event_count()?, 1);
        assert_eq!(guarded.overlay_local_projection_count()?, 1);
        Ok(())
    }

    #[test]
    fn account_device_cli_lifecycle_persists_and_revokes() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let account_dir = directory.path().join("account");
        let state_dir = directory.path().join("device");
        let certificate_file = directory.path().join("public-device.cert");
        let revocation_file = directory.path().join("device.revocation");
        let snapshot_file = directory.path().join("account.snapshot");

        create_account(account_dir.clone())?;
        let account_id = AccountRootState::load(&account_dir)?.account_id();
        enroll_device(
            account_dir.clone(),
            state_dir.clone(),
            Some(certificate_file.clone()),
        )?;
        authorize_device(state_dir.clone(), account_id)?;
        assert!(certificate_file.is_file());

        let device_id = DeviceState::load_or_create(&state_dir)?
            .identity()
            .device_id();
        revoke_device(account_dir.clone(), device_id, revocation_file.clone())?;
        assert!(revocation_file.is_file());
        export_account_snapshot(account_dir, snapshot_file.clone())?;
        update_device_authority(state_dir.clone(), snapshot_file)?;
        assert!(authorize_device(state_dir, account_id).is_err());
        Ok(())
    }

    #[test]
    fn connection_ticket_round_trips() -> Result<()> {
        let endpoint = EndpointAddr::new(SecretKey::generate().public());
        let listener_identity = DeviceIdentity::generate()?;
        let listener_device_id = listener_identity.device_id();
        let (listener_account_id, listener_certificate, _, listener_directory) =
            authority_for(&listener_identity)?;
        let requester_identity = DeviceIdentity::generate()?;
        let (allowed_requester_account_id, _, _, _) = authority_for(&requester_identity)?;
        let encoded = ConnectionTicket::new(
            endpoint.clone(),
            &listener_identity,
            listener_certificate,
            listener_directory,
            allowed_requester_account_id,
            RoutePolicy::DirectOnly,
        )?
        .encode()?;
        let decoded = ConnectionTicket::decode(&encoded)?;

        assert_eq!(decoded.content.version, TICKET_VERSION);
        assert_eq!(decoded.endpoint(), &endpoint);
        assert_eq!(
            decoded.content.listener_certificate.device_id(),
            listener_device_id
        );
        assert_eq!(decoded.listener_account_id(), listener_account_id);
        assert_eq!(decoded.route_policy(), RoutePolicy::DirectOnly);
        assert_eq!(
            decoded.allowed_requester_account_id(),
            allowed_requester_account_id
        );
        assert_eq!(
            decoded.ticket_publication_write_key(),
            ticket_publication_write_capability(&listener_identity, allowed_requester_account_id,)
                .write_key()
        );
        decoded.verify_listener_authorization(listener_account_id)?;
        assert!(
            decoded
                .verify_listener_authorization(allowed_requester_account_id)
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn connection_ticket_rejects_an_expired_prekey_directory() -> Result<()> {
        let root_directory = tempfile::tempdir()?;
        let ratchet_directory = tempfile::tempdir()?;
        let root = AccountRootState::create(root_directory.path())?;
        let identity = DeviceIdentity::generate()?;
        let certificate = root.issue_device_certificate(
            identity.device_id(),
            DeviceEncryptionIdentity::generate()?.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let device_list = root.publish_device_list(std::slice::from_ref(&certificate))?;
        let now = unix_time_now()?;
        let published_at = now
            .checked_sub(kilogram_ratchet::PREKEY_CLOCK_SKEW_SECONDS + 2)
            .context("test clock is unexpectedly close to Unix epoch")?;
        let expired_pool = RatchetState::load_or_create(ratchet_directory.path())?.prekey_pool(
            &identity,
            4,
            published_at,
            1,
        )?;
        let directory = AccountPrekeyDirectory::new(device_list, vec![expired_pool])?;
        let requester = DeviceIdentity::generate()?;
        let (requester_account_id, _, _, _) = authority_for(&requester)?;

        let error = ConnectionTicket::new(
            EndpointAddr::new(SecretKey::generate().public()),
            &identity,
            certificate,
            directory,
            requester_account_id,
            RoutePolicy::Auto,
        )
        .err()
        .context("expired prekey directory unexpectedly produced a ticket")?;
        assert!(format!("{error:#}").contains("expired"));
        Ok(())
    }

    #[test]
    fn ratchet_fanout_encrypts_one_event_for_every_signed_account_device() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let account = AccountRootState::create(directory.path().join("recipient-account"))?;
        let sender = DeviceIdentity::generate()?;
        let first = DeviceIdentity::generate()?;
        let second = DeviceIdentity::generate()?;
        let first_certificate = account.issue_device_certificate(
            first.device_id(),
            DeviceEncryptionIdentity::generate()?.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let second_certificate = account.issue_device_certificate(
            second.device_id(),
            DeviceEncryptionIdentity::generate()?.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let device_list = account
            .publish_device_list(&[second_certificate.clone(), first_certificate.clone()])?;
        let mut first_ratchet =
            RatchetState::load_or_create(directory.path().join("first-ratchet"))?;
        let mut second_ratchet =
            RatchetState::load_or_create(directory.path().join("second-ratchet"))?;
        let prekey_directory = AccountPrekeyDirectory::new(
            device_list,
            vec![
                second_ratchet.prekey_pool(
                    &second,
                    4,
                    unix_time_now()?,
                    DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
                )?,
                first_ratchet.prekey_pool(
                    &first,
                    4,
                    unix_time_now()?,
                    DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
                )?,
            ],
        )?;
        let mut sender_ratchet =
            RatchetState::load_or_create(directory.path().join("sender-ratchet"))?;
        let fanout = encrypt_ratchet_fanout(
            &mut sender_ratchet,
            &sender,
            &prekey_directory,
            "one event for both devices",
        )?;
        assert_eq!(fanout.recipients.len(), 2);
        assert_eq!(fanout.operations.len(), 2);
        assert!(fanout.operations.iter().all(|(_, operation)| {
            operation.message_kind == kilogram_ratchet::RatchetMessageKind::PreKey
        }));
        let event = SignedEvent::sign_ratchet_text(
            &sender,
            ConversationId::from_label("fanout-unit"),
            0,
            Vec::new(),
            prekey_directory.device_list().clone(),
            fanout.sender_identity,
            fanout.recipients,
        )?;

        let (sender_identity, first_ciphertext) = event.ratchet_message_for(first.device_id())?;
        let (first_body, _) = first_ratchet.decrypt(&first, sender_identity, first_ciphertext)?;
        let (sender_identity, second_ciphertext) = event.ratchet_message_for(second.device_id())?;
        let (second_body, _) =
            second_ratchet.decrypt(&second, sender_identity, second_ciphertext)?;
        assert_eq!(first_body.as_str(), "one event for both devices");
        assert_eq!(second_body.as_str(), first_body.as_str());
        assert!(
            event
                .ratchet_message_for(DeviceIdentity::generate()?.device_id())
                .is_err()
        );
        assert!(
            !event
                .encode()?
                .windows(b"one event for both devices".len())
                .any(|window| window == b"one event for both devices")
        );
        Ok(())
    }

    #[test]
    fn pinned_peer_state_rejects_an_older_ticket_snapshot() -> Result<()> {
        let root_directory = tempfile::tempdir()?;
        let peer_directory = tempfile::tempdir()?;
        let listener_root = AccountRootState::create(root_directory.path())?;
        let listener_identity = DeviceIdentity::generate()?;
        let listener_encryption = DeviceEncryptionIdentity::generate()?;
        let listener_certificate = listener_root.issue_device_certificate(
            listener_identity.device_id(),
            listener_encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let requester = DeviceIdentity::generate()?;
        let (requester_account_id, _, _, _) = authority_for(&requester)?;
        let old_device_list =
            listener_root.publish_device_list(std::slice::from_ref(&listener_certificate))?;
        let old_directory = AccountPrekeyDirectory::new(
            old_device_list,
            vec![prekey_pool_for(&listener_identity)?],
        )?;
        let encoded = ConnectionTicket::new(
            EndpointAddr::new(SecretKey::generate().public()),
            &listener_identity,
            listener_certificate,
            old_directory,
            requester_account_id,
            RoutePolicy::Auto,
        )?
        .encode()?;
        let ticket = ConnectionTicket::decode(&encoded)?;
        listener_root.revoke_device(listener_identity.device_id())?;
        let newer_snapshot = listener_root.authority_snapshot()?;
        let peer_state = DeviceState::load_or_create(peer_directory.path())?;
        peer_state.pin_peer_authority_snapshot(&newer_snapshot)?;

        assert!(
            peer_state
                .pin_peer_authority_snapshot(ticket.listener_authority_snapshot())
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn connection_ticket_rejects_unknown_version() -> Result<()> {
        let identity = DeviceIdentity::generate()?;
        let (_, certificate, _, directory) = authority_for(&identity)?;
        let requester = DeviceIdentity::generate()?;
        let (requester_account_id, _, _, _) = authority_for(&requester)?;
        let mut ticket = ConnectionTicket::new(
            EndpointAddr::new(SecretKey::generate().public()),
            &identity,
            certificate,
            directory,
            requester_account_id,
            RoutePolicy::Auto,
        )?;
        ticket.content.version = TICKET_VERSION + 1;
        let encoded = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&ticket)?);

        let error = ConnectionTicket::decode(&encoded)
            .err()
            .context("unknown ticket version unexpectedly succeeded")?;

        assert!(
            error
                .to_string()
                .contains("unsupported connection ticket version")
        );
        Ok(())
    }

    #[test]
    fn connection_ticket_rejects_tampering() -> Result<()> {
        let identity = DeviceIdentity::generate()?;
        let (_, certificate, _, directory) = authority_for(&identity)?;
        let requester = DeviceIdentity::generate()?;
        let (requester_account_id, _, _, _) = authority_for(&requester)?;
        let mut ticket = ConnectionTicket::new(
            EndpointAddr::new(SecretKey::generate().public()),
            &identity,
            certificate,
            directory,
            requester_account_id,
            RoutePolicy::Auto,
        )?;
        ticket.content.endpoint = EndpointAddr::new(SecretKey::generate().public());
        let encoded = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&ticket)?);

        let error = ConnectionTicket::decode(&encoded)
            .err()
            .context("tampered connection ticket unexpectedly succeeded")?;
        assert!(
            error
                .to_string()
                .contains("verify listener signature on connection ticket")
        );
        Ok(())
    }

    #[test]
    fn connection_ticket_rejects_route_policy_tampering() -> Result<()> {
        let identity = DeviceIdentity::generate()?;
        let (_, certificate, _, directory) = authority_for(&identity)?;
        let requester = DeviceIdentity::generate()?;
        let (requester_account_id, _, _, _) = authority_for(&requester)?;
        let mut ticket = ConnectionTicket::new(
            EndpointAddr::new(SecretKey::generate().public()),
            &identity,
            certificate,
            directory,
            requester_account_id,
            RoutePolicy::Auto,
        )?;
        ticket.content.route_policy = RoutePolicy::RelayOnly;
        let encoded = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&ticket)?);

        let error = ConnectionTicket::decode(&encoded)
            .err()
            .context("tampered route policy unexpectedly succeeded")?;
        assert!(
            error
                .to_string()
                .contains("verify listener signature on connection ticket")
        );
        Ok(())
    }

    #[tokio::test]
    async fn certified_device_authorizes_before_application_exchange() -> Result<()> {
        let requester_identity = DeviceIdentity::generate()?;
        let (requester_account_id, requester_certificate, requester_snapshot, _) =
            authority_for(&requester_identity)?;
        let listener_device_directory = tempfile::tempdir()?;
        let listener_state_path = listener_device_directory.path().to_path_buf();
        let listener_device_state = DeviceState::load_or_create(&listener_state_path)?;
        let listener_root_directory = tempfile::tempdir()?;
        let listener_account_id =
            AccountRootState::create(listener_root_directory.path())?.account_id();
        let listener = local_test_endpoint_builder()?
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await?;
        let listener_address = listener.addr();
        let session_binding = SyncSessionBinding::from_transport_label(&listener.id().to_string());
        let accept_task = tokio::spawn({
            let listener = listener.clone();
            async move {
                let connection = accept_authenticated_connection(&listener).await?;
                let authorized = accept_device_authorization(
                    &connection,
                    &listener_state_path,
                    &listener_device_state,
                    session_binding,
                    requester_account_id,
                    listener_account_id,
                )
                .await?;
                connection.closed().await;
                Ok::<_, anyhow::Error>(authorized)
            }
        });

        let client = local_test_endpoint_builder()?.bind().await?;
        let connection = client.connect(listener_address, ALPN).await?;
        authorize_with_listener(
            &connection,
            &requester_identity,
            requester_certificate,
            requester_snapshot,
            session_binding,
        )
        .await?;
        connection.close(0_u32.into(), b"authorization test complete");
        let authorized = accept_task.await??;

        assert_eq!(authorized.account_id(), requester_account_id);
        assert_eq!(authorized.device_id(), requester_identity.device_id());
        client.close().await;
        listener.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn listener_pins_new_snapshot_before_rejecting_revoked_device() -> Result<()> {
        let requester_root_directory = tempfile::tempdir()?;
        let requester_root = AccountRootState::create(requester_root_directory.path())?;
        let requester_identity = DeviceIdentity::generate()?;
        let requester_encryption = DeviceEncryptionIdentity::generate()?;
        let requester_certificate = requester_root.issue_device_certificate(
            requester_identity.device_id(),
            requester_encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        requester_root.revoke_device(requester_identity.device_id())?;
        let requester_snapshot = requester_root.authority_snapshot()?;
        let requester_account_id = requester_root.account_id();

        let listener_device_directory = tempfile::tempdir()?;
        let listener_state_path = listener_device_directory.path().to_path_buf();
        let listener_root_directory = tempfile::tempdir()?;
        let listener_account_id =
            AccountRootState::create(listener_root_directory.path())?.account_id();
        let listener = local_test_endpoint_builder()?
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await?;
        let listener_address = listener.addr();
        let session_binding = SyncSessionBinding::from_transport_label(&listener.id().to_string());
        let accept_task = tokio::spawn({
            let listener = listener.clone();
            let listener_state_path = listener_state_path.clone();
            async move {
                let listener_state = DeviceState::load_or_create(&listener_state_path)?;
                let connection = accept_authenticated_connection(&listener).await?;
                accept_device_authorization(
                    &connection,
                    &listener_state_path,
                    &listener_state,
                    session_binding,
                    requester_account_id,
                    listener_account_id,
                )
                .await
            }
        });

        let client = local_test_endpoint_builder()?.bind().await?;
        let connection = client.connect(listener_address, ALPN).await?;
        assert!(
            authorize_with_listener(
                &connection,
                &requester_identity,
                requester_certificate,
                requester_snapshot.clone(),
                session_binding,
            )
            .await
            .is_err()
        );
        connection.close(0_u32.into(), b"revoked authorization test complete");
        assert!(accept_task.await?.is_err());

        let reloaded_listener_state = DeviceState::load_or_create(listener_state_path)?;
        let pinned = reloaded_listener_state.load_peer_authority_snapshot(requester_account_id)?;
        assert_eq!(pinned, requester_snapshot);
        assert_eq!(pinned.revision(), 2);
        client.close().await;
        listener.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn listener_ignores_failed_handshake_and_accepts_the_next_connection() -> Result<()> {
        let listener = local_test_endpoint_builder()?
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await?;
        let listener_address = listener.addr();
        let listener_id = listener.id();
        let accept_task = tokio::spawn({
            let listener = listener.clone();
            async move { accept_authenticated_connection(&listener).await }
        });

        let incompatible_client = local_test_endpoint_builder()?.bind().await?;
        let incompatible_result = timeout(
            CONNECTION_TIMEOUT,
            incompatible_client.connect(listener_address.clone(), UNSUPPORTED_TEST_ALPN),
        )
        .await
        .context("incompatible test handshake timed out")?;
        assert!(incompatible_result.is_err());
        incompatible_client.close().await;

        let valid_client = local_test_endpoint_builder()?.bind().await?;
        let valid_connection = timeout(
            CONNECTION_TIMEOUT,
            valid_client.connect(listener_address, ALPN),
        )
        .await
        .context("valid test handshake timed out")??;
        let accepted_connection = timeout(CONNECTION_TIMEOUT, accept_task)
            .await
            .context("listener did not accept the valid test connection")?
            .context("join listener accept task")??;

        assert_eq!(valid_connection.remote_id(), listener_id);
        assert_eq!(accepted_connection.remote_id(), valid_client.id());

        valid_connection.close(0_u32.into(), b"test complete");
        accepted_connection.close(0_u32.into(), b"test complete");
        valid_client.close().await;
        listener.close().await;
        Ok(())
    }
}
