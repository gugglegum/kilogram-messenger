use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
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
    AuthorizedDevice, ConversationMembershipSnapshot, DeviceCapability, DeviceCertificate,
    DeviceId, DeviceIdentity, DeviceState, verify_device_authorization_with_snapshot,
};
use kilogram_protocol::{
    AuthorizedEvent, ClientRequest, ConversationId, DeviceAuthorizationAccepted,
    DeviceAuthorizationRejected, EventPayload, HistoryRewrapBundle, HistoryRewrapRejected,
    HistoryRewrapRejectionReason, HistoryRewrapSas, LocalTextProjection,
    MAX_HISTORY_REWRAP_ENTRIES, MAX_INVENTORY_EVENT_IDS, RatchetRecipient, ServerResponse,
    SignedDeviceSessionAuthorization, SignedEvent, SignedHistoryRecoveryCheckpoint,
    SignedHistoryRewrapRequest, SignedHistoryRewrapTransfer, SignedSyncInventory, SyncPause,
    SyncPaused, SyncSessionBinding,
};
use kilogram_ratchet::{
    AccountPrekeyDirectory, DEFAULT_PREKEY_POOL_SIZE, DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
    DecryptedMessage, MAX_PREKEY_POOL_SIZE, RatchetOperation, RatchetState, SignedPrekeyPool,
    SignedRatchetIdentity, unix_time_now,
};
use kilogram_session::{
    MAX_SYNC_ROUNDS, ServerInventoryOutcome, SessionStore, SyncClient, SyncServer,
    authorize_device_session,
};
use kilogram_state::{
    EncryptedStateVault, STATE_VAULT_FILE, STATE_VAULT_KEY_FILE, StateDirectoryLock,
    StateMirrorRepository, StateRecordKind, StateTransaction, TypedStateRepository,
    VaultMigrationOutcome, VaultMirrorCommit, VaultMirrorOutcome, VaultPrimaryWriteRepository,
    VaultReport,
};
use kilogram_store::{
    CommandEventReadOverlay, CommandLocalMessageReadOverlay, EventReadRepository, EventStore,
    ImmutableEventReadSnapshot, ImmutableLocalMessageReadSnapshot, LocalMessageReadRepository,
    LocalMessageStore, StoreError, StoreOutcome,
};
use kilogram_transport_iroh::{
    ALPN, MAX_WIRE_MESSAGE_BYTES, RoutePolicy, SelectedPathDiagnostics, await_route_policy,
    endpoint_builder_for_remote, endpoint_builder_with_relay, read_client_request,
    read_server_response, selected_path_diagnostics, write_client_request, write_server_response,
};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use tokio::time::timeout;

const EVENT_STORE_DIRECTORY: &str = "events";
const LOCAL_MESSAGE_STORE_DIRECTORY: &str = "local-messages";
const HISTORY_REWRAP_STORE_DIRECTORY: &str = "history-rewraps";
const HISTORY_RECOVERY_STORE_DIRECTORY: &str = "history-recovery";
const DIRECT_PATH_DIAGNOSTIC_WAIT: Duration = Duration::from_secs(3);
const ROUTE_POLICY_WAIT: Duration = Duration::from_secs(15);
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);
const CLIENT_RELAY_WAIT_SECONDS: u64 = 30;
const STREAM_OPEN_TIMEOUT: Duration = Duration::from_secs(15);
const TICKET_SIGNATURE_DOMAIN: &[u8] = b"kilogram:connection-ticket-signature:v9\0";
const TICKET_VERSION: u8 = 9;

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

        /// Maximum events requested in this invocation; repeat with a fresh ticket to resume.
        #[arg(long, default_value_t = 64)]
        page_size: usize,

        /// SAS independently compared by both users before recovery starts.
        #[arg(long)]
        confirm_sas: String,

        /// Trusted account shared by source and recipient devices.
        #[arg(long)]
        expect_account: AccountId,
    },

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
            | Self::Connect { state_dir, .. }
            | Self::Sync { state_dir, .. }
            | Self::SeedHistory { state_dir, .. }
            | Self::RatchetBundle { state_dir, .. }
            | Self::RatchetPrekeyPool { state_dir, .. }
            | Self::HistoryRewrapExport { state_dir, .. }
            | Self::HistoryRewrapImport { state_dir, .. }
            | Self::HistoryRewrapFetch { state_dir, .. }
            | Self::HistoryRecoveryResume { state_dir, .. }
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
            | Self::StateVaultRestore { state_dir, .. } => Some(state_dir),
            Self::AccountCreate { .. }
            | Self::HistoryRewrapSas { .. }
            | Self::AccountShow { .. }
            | Self::AccountSnapshot { .. }
            | Self::AccountDeviceList { .. }
            | Self::ConversationCreate { .. }
            | Self::ConversationMemberAdd { .. }
            | Self::DeviceRevoke { .. } => None,
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
                    | Self::StateVaultRestore { .. }
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

impl From<RoutePolicyArg> for RoutePolicy {
    fn from(value: RoutePolicyArg) -> Self {
        match value {
            RoutePolicyArg::Auto => Self::Auto,
            RoutePolicyArg::DirectOnly => Self::DirectOnly,
            RoutePolicyArg::RelayOnly => Self::RelayOnly,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ConnectionTicketContent {
    version: u8,
    endpoint: EndpointAddr,
    listener_certificate: DeviceCertificate,
    listener_directory: AccountPrekeyDirectory,
    allowed_requester_account_id: AccountId,
    route_policy: RoutePolicy,
}

#[derive(Debug, Serialize, Deserialize)]
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
        let bytes = URL_SAFE_NO_PAD
            .decode(encoded.trim())
            .context("decode connection ticket as base64url")?;
        let ticket: Self =
            serde_json::from_slice(&bytes).context("decode connection ticket payload")?;
        ticket.verify()?;
        Ok(ticket)
    }

    fn endpoint(&self) -> &EndpointAddr {
        &self.content.endpoint
    }

    fn listener_account_id(&self) -> AccountId {
        self.content.listener_certificate.account_id()
    }

    fn allowed_requester_account_id(&self) -> AccountId {
        self.content.allowed_requester_account_id
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
        ensure!(
            self.content.version == TICKET_VERSION,
            "unsupported connection ticket version: {}",
            self.content.version
        );
        self.content.listener_certificate.verify()?;
        self.content
            .listener_directory
            .verify_at(unix_time_now()?)?;
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

fn ticket_signing_bytes(content: &ConnectionTicketContent) -> Result<Vec<u8>> {
    let encoded = serde_json::to_vec(content).context("serialize connection ticket content")?;
    let mut bytes = Vec::with_capacity(TICKET_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(TICKET_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let command = cli.command;
    let state_directory = command.state_directory().map(Path::to_path_buf);
    let uses_state_vault_dual_write = command.uses_state_vault_dual_write();
    let _state_lock = command
        .state_directory()
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
    match (command_result, mirror_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(command_error), Ok(())) => Err(command_error),
        (Ok(()), Err(mirror_error)) => Err(mirror_error),
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
        } => {
            listen(ListenOptions {
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
            })
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
            confirm_sas,
            expect_account,
        } => {
            resume_history_recovery(
                state_dir,
                ticket,
                ticket_file,
                conversation,
                source_device,
                range_start,
                count,
                page_size,
                confirm_sas,
                expect_account,
            )
            .await
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
    println!("vault_key_protection=development-file");
    println!(
        "migration={}",
        match outcome {
            VaultMigrationOutcome::Migrated => "committed",
            VaultMigrationOutcome::AlreadyCurrent => "already-current",
        }
    );
    print_vault_report(&report);
    println!("legacy_files_retained=true");
    println!("status=state-vault-migrated");
    Ok(())
}

fn verify_state_vault(state_dir: PathBuf) -> Result<()> {
    let vault = EncryptedStateVault::open_existing(&state_dir)
        .context("open encrypted transactional state vault")?;
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

fn print_vault_report(report: &VaultReport) {
    println!("vault_schema_version={}", report.schema_version());
    println!("vault_mirror_generation={}", report.mirror_generation());
    println!("vault_record_count={}", report.record_count());
    println!("vault_plaintext_bytes={}", report.plaintext_bytes());
    println!("vault_snapshot_id={}", encode_hex(report.snapshot_id()));
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
}

impl PendingVaultPrimaryWrite {
    fn prepare(state_directory: &Path) -> Result<Option<Self>, kilogram_state::StateError> {
        if !EncryptedStateVault::is_initialized(state_directory)? {
            return Ok(None);
        }
        let vault = EncryptedStateVault::open_existing(state_directory)?;
        let commit = vault.commit_primary_checkpoint()?;
        Ok(Some(Self { vault, commit }))
    }

    fn confirm(self) -> Result<(), kilogram_state::StateError> {
        let report = self.vault.confirm_primary_shadow()?;
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

fn run_state_transaction<T>(
    state_directory: &Path,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let transaction = StateTransaction::begin(state_directory)
        .context("prepare crash-consistent local state transaction")?;
    match operation() {
        Ok(value) => {
            let primary_write = match PendingVaultPrimaryWrite::prepare(state_directory) {
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
    operation: impl FnOnce() -> Result<T, StoreError>,
) -> Result<T, StoreError> {
    let transaction =
        StateTransaction::begin(state_directory).map_err(state_transaction_store_error)?;
    match operation() {
        Ok(value) => {
            let primary_write = match PendingVaultPrimaryWrite::prepare(state_directory) {
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

async fn listen(options: ListenOptions) -> Result<()> {
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
    } = options;
    let device_state = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load device state from {}", state_dir.display()))?;
    let listener_certificate = device_state
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
    let immutable_reads = open_immutable_read_repositories(&state_dir)
        .context("capture immutable vault-primary listener state before local changes")?;
    let authority_snapshot_store = device_state
        .install_own_authority_snapshot(listener_device_list.authority_snapshot())
        .context("install authority snapshot embedded in listener device list")?;
    let event_store = open_event_store(&state_dir)?;
    let local_message_store = open_local_message_store(&state_dir)?;
    let now_unix_seconds = unix_time_now().context("read time for listener prekey freshness")?;
    let (mut ratchet_state, listener_prekey_pool) = run_state_transaction(&state_dir, || {
        let mut ratchet_state = RatchetState::load_or_create(&state_dir)
            .context("load persistent listener ratchet state")?;
        let listener_prekey_pool = ratchet_state
            .prekey_pool(
                device_state.identity(),
                DEFAULT_PREKEY_POOL_SIZE,
                now_unix_seconds,
                DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
            )
            .context("publish listener one-time prekey pool")?;
        Ok((ratchet_state, listener_prekey_pool))
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

    println!("status=listening");
    let connection = accept_authenticated_connection(&endpoint).await?;
    println!("peer_id={}", connection.remote_id());

    let ready_path = await_route_policy(&connection, route_policy, ROUTE_POLICY_WAIT)
        .await
        .context("wait for an incoming path allowed by the connection ticket")?;
    print_ready_path(&ready_path);

    let session_binding = SyncSessionBinding::from_transport_label(&endpoint.id().to_string());
    let authorized_requester = accept_device_authorization(
        &connection,
        &device_state,
        session_binding,
        allowed_requester_account_id,
    )
    .await?;

    let (mut send, mut receive) =
        accept_bi(&connection, "accept authorized application stream").await?;
    let request = read_client_request(&mut receive).await?;
    match request {
        ClientRequest::DeliverEvent(event) => {
            handle_delivery_request(
                DeliveryState {
                    state_directory: &state_dir,
                    device_state: &device_state,
                    event_store: &event_store,
                    local_message_store: &local_message_store,
                    ratchet_state: &mut ratchet_state,
                },
                &mut send,
                *event,
                &authorized_requester,
            )
            .await?;
        }
        ClientRequest::SyncInventory(inventory) => {
            print_immutable_read_diagnostics("sync", &immutable_reads);
            let decrypting_store = DecryptingSessionStore::new(
                &state_dir,
                &event_store,
                &local_message_store,
                &device_state,
                ratchet_state,
                immutable_reads,
            );
            handle_sync_request(
                &device_state,
                &decrypting_store,
                &connection,
                send,
                inventory,
                session_binding,
                &authorized_requester,
            )
            .await?;
        }
        ClientRequest::SyncEvents(_) => bail!("sync event batch cannot be the first request"),
        ClientRequest::SyncPause(_) => bail!("sync pause cannot be the first request"),
        ClientRequest::HistoryRewrap(request) => {
            handle_history_rewrap_request(
                &device_state,
                Some(&immutable_reads),
                &mut send,
                request,
                session_binding,
                &authorized_requester,
                ticket.listener_directory().device_list(),
                history_rewrap_approval.as_ref(),
            )
            .await?;
        }
        ClientRequest::AuthorizeDevice(_) => {
            bail!("device authorization cannot be repeated on an authorized connection")
        }
    }

    print_transport_diagnostics(&connection, route_policy).await?;
    let _ = timeout(Duration::from_secs(2), connection.closed()).await;
    endpoint.close().await;
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
    range_start
        .checked_add(count)
        .context("--history-rewrap-range-start plus --history-rewrap-count overflows")?;
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
    device_state: &DeviceState,
    read_repositories: Option<&ImmutableReadRepositories>,
    send: &mut SendStream,
    request: SignedHistoryRewrapRequest,
    expected_session: SyncSessionBinding,
    authorized_requester: &AuthorizedDevice,
    device_list: &AccountDeviceListSnapshot,
    approval: Option<&HistoryRewrapApproval>,
) -> Result<()> {
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
        return Ok(());
    };
    let source_certificate = device_state
        .load_certificate()
        .context("load source certificate for network history rewrap")?;
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
        return Ok(());
    }
    let read_repositories = read_repositories
        .context("approved network history rewrap is missing its immutable read snapshot")?;

    let bundle = build_history_rewrap_bundle(
        device_state,
        &source_certificate,
        device_list.clone(),
        approval.recipient_device_id,
        approval.conversation_id,
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
        return Ok(());
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
    Ok(())
}

async fn accept_device_authorization(
    connection: &Connection,
    device_state: &DeviceState,
    expected_session: SyncSessionBinding,
    allowed_account: AccountId,
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
        let snapshot_store = device_state
            .pin_peer_authority_snapshot(authorization.authority_snapshot())
            .context("pin requester authority snapshot and reject rollback")?;
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
    ratchet_state: &'a mut RatchetState,
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
        ratchet_state,
    } = state;
    let signed_event = event.event();
    let membership = device_state
        .load_conversation_membership(signed_event.conversation_id().scope_id())
        .context("load trusted conversation membership for received event")?;
    let listener_certificate = device_state
        .load_certificate()
        .context("load listener certificate for acknowledgement")?;
    let listener_authority_snapshot = device_state
        .load_own_authority_snapshot()
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
    let (
        body,
        local_projection_store_outcome,
        ratchet_operation,
        received_store_outcome,
        acknowledgement,
        acknowledgement_id,
        acknowledgement_store_outcome,
    ) = run_state_transaction(state_directory, || {
        let (body, local_projection_store_outcome, ratchet_operation) =
            match open_local_text_projection_if_present(
                local_message_store,
                device_state,
                signed_event,
            )? {
                Some(body) => (body, kilogram_store::StoreOutcome::AlreadyPresent, None),
                None => {
                    let (sender_ratchet_identity, ciphertext) =
                        signed_event.ratchet_message_for(local_device_id)?;
                    let (decrypted, operation) = ratchet_state
                        .decrypt(device_state.identity(), sender_ratchet_identity, ciphertext)
                        .context("decrypt received text through the persistent ratchet")?;
                    let body = decrypted.as_str().to_owned();
                    let outcome = ensure_received_local_text_projection(
                        local_message_store,
                        device_state,
                        signed_event,
                        &decrypted,
                    )
                    .context("persist local history projection before the received event")?;
                    (body, outcome, Some(operation))
                }
            };
        let received_store_outcome = event_store
            .put_authorized(&event, &membership)
            .context("persist received event before acknowledging it")?;

        let acknowledgement_sequence = device_state
            .allocate_sequence()
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
        let acknowledgement_store_outcome = event_store
            .put_authorized(&acknowledgement, &membership)
            .context("persist acknowledgement before sending it")?;
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

async fn handle_sync_request(
    device_state: &DeviceState,
    decrypting_store: &DecryptingSessionStore<'_>,
    connection: &iroh::endpoint::Connection,
    first_send: SendStream,
    first_inventory: SignedSyncInventory,
    expected_session: SyncSessionBinding,
    authorized_requester: &AuthorizedDevice,
) -> Result<()> {
    let membership = device_state
        .load_conversation_membership(first_inventory.conversation_id().scope_id())
        .context("load trusted conversation membership for synchronization")?;
    let listener_account_id = device_state
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
    let device_state = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load device state from {}", state_dir.display()))?;
    let requester_certificate = device_state
        .load_certificate()
        .context("load requester Account Root certificate")?;
    let requester_authority_snapshot = device_state
        .load_own_authority_snapshot()
        .context("load requester Account Root authority snapshot")?;
    let event_store = open_event_store(&state_dir)?;
    let local_message_store = open_local_message_store(&state_dir)?;
    let mut ratchet_state = RatchetState::load_or_create(&state_dir)
        .context("load persistent requester ratchet state")?;

    let ticket = load_connection_ticket(ticket, ticket_file).await?;
    ticket.verify_listener_account(expected_listener_account_id)?;
    run_state_transaction(&state_dir, || {
        ratchet_state
            .observe_prekey_directory(ticket.listener_directory(), unix_time_now()?)
            .context("observe listener prekey directory and reject rollback")
    })?;
    let listener_snapshot_store = device_state
        .pin_peer_authority_snapshot(ticket.listener_authority_snapshot())
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
    let membership = device_state
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
    ) = run_state_transaction(&state_dir, || {
        let author_sequence = device_state
            .allocate_sequence()
            .context("allocate message sequence")?;
        let parents = event_store
            .frontier(conversation_id)
            .context("calculate local conversation frontier")?;
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
        let local_projection_store_outcome = ensure_authored_local_text_projection(
            &local_message_store,
            &device_state,
            event.event(),
            &message,
        )
        .context("persist local history projection before the sent event")?;
        let sent_store_outcome = event_store
            .put_authorized(&event, &membership)
            .context("persist authorized event before sending it")?;
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
    let acknowledgement_store_outcome = run_state_transaction(&state_dir, || {
        event_store
            .put_authorized(&acknowledgement, &membership)
            .context("persist verified acknowledgement")
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
    let device_state = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load device state from {}", state_dir.display()))?;
    let requester_certificate = device_state
        .load_certificate()
        .context("load requester Account Root certificate")?;
    let requester_authority_snapshot = device_state
        .load_own_authority_snapshot()
        .context("load requester Account Root authority snapshot")?;
    let immutable_reads = open_immutable_read_repositories(&state_dir)
        .context("capture immutable vault-primary sync state before local changes")?;
    let event_store = open_event_store(&state_dir)?;
    let local_message_store = open_local_message_store(&state_dir)?;
    let ratchet_state =
        RatchetState::load_or_create(&state_dir).context("load persistent sync ratchet state")?;
    let ticket = load_connection_ticket(ticket, ticket_file).await?;
    ticket.verify_listener_account(expected_listener_account_id)?;
    run_state_transaction(&state_dir, || {
        ratchet_state
            .observe_prekey_directory(ticket.listener_directory(), unix_time_now()?)
            .context("observe listener prekey directory and reject rollback")
    })?;
    let listener_snapshot_store = device_state
        .pin_peer_authority_snapshot(ticket.listener_authority_snapshot())
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
    let membership = device_state
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
        ratchet_state,
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
    println!("target_endpoint_id={}", ticket.endpoint().id);
    for relay_url in ticket.endpoint().relay_urls() {
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

fn show_identity(state_dir: PathBuf) -> Result<()> {
    let device_state = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load device state from {}", state_dir.display()))?;
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
    println!("root_secret_storage=development-plaintext");
    println!("status=account-created");
    Ok(())
}

fn show_account(account_dir: PathBuf) -> Result<()> {
    let account = AccountRootState::load(&account_dir)
        .with_context(|| format!("load Account Root state from {}", account_dir.display()))?;
    println!("account_id={}", account.account_id());
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
    let device = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load device state from {}", state_dir.display()))?;
    let certificate = device
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
    let store = device
        .install_conversation_membership(&membership)
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
    let device = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load device state from {}", state_dir.display()))?;
    let certificate = account
        .issue_device_certificate(
            device.identity().device_id(),
            device.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )
        .context("issue root-signed device certificate")?;
    device
        .install_certificate(&certificate)
        .context("install root-signed certificate into device state")?;
    let authority_snapshot = account
        .authority_snapshot()
        .context("create authority snapshot after device enrollment")?;
    let snapshot_store = device
        .install_own_authority_snapshot(&authority_snapshot)
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
    let device = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load device state from {}", state_dir.display()))?;
    let certificate = device
        .load_certificate()
        .context("load installed root-signed device certificate")?;
    let snapshot = device
        .load_own_authority_snapshot()
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
    let device = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load device state from {}", state_dir.display()))?;
    let bytes = fs::read(&snapshot_file)
        .with_context(|| format!("read authority snapshot from {}", snapshot_file.display()))?;
    let snapshot = AccountAuthoritySnapshot::decode_and_verify(&bytes)
        .with_context(|| format!("verify authority snapshot from {}", snapshot_file.display()))?;
    let store = device
        .install_own_authority_snapshot(&snapshot)
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
    let device_state = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load device state from {}", state_dir.display()))?;
    let certificate = device_state
        .load_certificate()
        .context("load device certificate before reading history")?;
    let read_repositories = open_immutable_read_repositories(&state_dir)?;
    let conversation_id = ConversationId::from_label(&conversation);
    let membership = device_state
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
    let device_state = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load device state from {}", state_dir.display()))?;
    let certificate = device_state
        .load_certificate()
        .context("load device certificate before exporting a prekey bundle")?;
    ensure!(
        certificate.device_id() == device_state.identity().device_id(),
        "installed certificate belongs to a different device"
    );
    let bundle = run_state_transaction(&state_dir, || {
        let mut ratchet_state =
            RatchetState::load_or_create(&state_dir).context("load persistent ratchet state")?;
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
    let device_state = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load device state from {}", state_dir.display()))?;
    let certificate = device_state
        .load_certificate()
        .context("load device certificate before exporting a prekey pool")?;
    ensure!(
        certificate.device_id() == device_state.identity().device_id(),
        "installed certificate belongs to a different device"
    );
    let now_unix_seconds = unix_time_now().context("read time for prekey pool publication")?;
    let pool = run_state_transaction(&state_dir, || {
        let mut ratchet_state =
            RatchetState::load_or_create(&state_dir).context("load persistent ratchet state")?;
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
    let device_state = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load recipient device state from {}", state_dir.display()))?;
    let recipient_certificate = device_state
        .load_certificate()
        .context("load recipient Account Root certificate")?;
    let recipient_authority_snapshot = device_state
        .load_own_authority_snapshot()
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
    let membership = device_state
        .load_conversation_membership(conversation_id.scope_id())
        .context("load trusted conversation membership before network history rewrap")?;
    membership
        .require_member(expected_account_id)
        .context("account is not a member of the requested conversation")?;
    let source_snapshot_store = device_state
        .install_own_authority_snapshot(ticket.listener_authority_snapshot())
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
    let request = SignedHistoryRewrapRequest::sign(
        device_state.identity(),
        conversation_id,
        source_device_id,
        session_binding,
        range_start,
        count,
        sas,
    )?;
    let (mut send, mut receive) =
        open_bi(&connection, "open history-rewrap request stream").await?;
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
    let bundle = transfer.bundle().clone();
    let bundle_encoded = bundle.encode()?;
    let recovery_checkpoint = recovery_checkpoint
        .map(|checkpoint| {
            ensure!(
                checkpoint.next_range_start() == request.range_start(),
                "history recovery checkpoint expects range {}, but request starts at {}",
                checkpoint.next_range_start(),
                request.range_start()
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
async fn resume_history_recovery(
    state_dir: PathBuf,
    ticket: Option<String>,
    ticket_file: Option<PathBuf>,
    conversation: String,
    expected_source_device_id: DeviceId,
    approved_range_start: usize,
    approved_event_count: usize,
    page_size: usize,
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

    let device_state = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load recipient device state from {}", state_dir.display()))?;
    let recipient_certificate = device_state
        .load_certificate()
        .context("load recipient Account Root certificate")?;
    ensure!(
        recipient_certificate.account_id() == expected_account_id,
        "history recovery requires the recipient to belong to --expect-account"
    );
    let inspected_ticket = load_connection_ticket(ticket.clone(), ticket_file.clone()).await?;
    inspected_ticket.verify_listener_account(expected_account_id)?;
    ensure!(
        inspected_ticket.allowed_requester_account_id() == expected_account_id,
        "source ticket does not authorize this same account"
    );
    let source = inspected_ticket.verify_listener_authorization(expected_account_id)?;
    ensure!(
        source.device_id() == expected_source_device_id,
        "source ticket belongs to device {}; explicitly selected source is {expected_source_device_id}",
        source.device_id()
    );
    let device_list = inspected_ticket.listener_directory().device_list();
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
        "recipient confirmed SAS {}, but the signed source ticket derives {sas}",
        confirmed_sas.trim()
    );

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
    let checkpoint = load_latest_history_recovery_checkpoint(&state_dir, initial_checkpoint)?;
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

    let next_range_start = usize::try_from(checkpoint.next_range_start())
        .context("history recovery next range cannot be represented on this platform")?;
    let approved_range_end = usize::try_from(checkpoint.approved_range_end())
        .context("history recovery approved range cannot be represented on this platform")?;
    let request_count = page_size.min(approved_range_end - next_range_start);
    fetch_history_rewrap(
        state_dir,
        ticket,
        ticket_file,
        conversation,
        next_range_start,
        request_count,
        confirmed_sas,
        expected_account_id,
        Some(expected_source_device_id),
        Some(checkpoint),
        "history-recovery-page-imported",
    )
    .await
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
    let device_state = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load source device state from {}", state_dir.display()))?;
    let read_repositories = open_immutable_read_repositories(&state_dir)
        .context("capture immutable vault-primary source history before export state changes")?;
    let source_certificate = device_state
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
    let snapshot_store = device_state
        .install_own_authority_snapshot(device_list.authority_snapshot())
        .context("install authority snapshot from history-rewrap device list")?;
    let bundle = build_history_rewrap_bundle(
        &device_state,
        &source_certificate,
        device_list,
        recipient_device_id,
        ConversationId::from_label(&conversation),
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
    let membership = device_state
        .load_conversation_membership(conversation_id.scope_id())
        .context("load trusted conversation membership before history rewrap")?;
    membership
        .require_member(source_certificate.account_id())
        .context("source account is not a member of this conversation")?;
    let stored_events = event_reads
        .load_authorized_conversation(conversation_id, &membership)
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
    let device_state = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load recipient device state from {}", state_dir.display()))?;
    let recipient_certificate = device_state
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
    let snapshot_store = device_state
        .install_own_authority_snapshot(
            bundle.manifest().account_device_list().authority_snapshot(),
        )
        .context("install authority snapshot from history-rewrap bundle")?;
    let membership = device_state
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
        run_state_transaction(&state_dir, || {
            let bundle_store = persist_history_rewrap_bundle(&state_dir, &bundle, &encoded)?;
            let transfer_store = transfer
                .as_ref()
                .map(|(transfer, encoded)| {
                    persist_history_rewrap_transfer(&state_dir, transfer, encoded)
                })
                .transpose()?;
            let checkpoint_store = recovery_checkpoint
                .as_ref()
                .map(|(checkpoint, encoded)| {
                    persist_history_recovery_checkpoint(&state_dir, checkpoint, encoded)
                })
                .transpose()?;
            let mut inserted_projections = 0_usize;
            let mut inserted_events = 0_usize;
            for (authorized_event, projection, projection_exists) in prepared {
                if !projection_exists
                    && local_message_store.put(&projection)? == StoreOutcome::Inserted
                {
                    inserted_projections += 1;
                }
                if event_store.put_authorized(&authorized_event, &membership)?
                    == StoreOutcome::Inserted
                {
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
) -> Result<StoreOutcome> {
    let directory = state_dir.join(HISTORY_REWRAP_STORE_DIRECTORY);
    fs::create_dir_all(&directory)?;
    let path = directory.join(format!("{}.rewrap", bundle.bundle_id()?));
    if path.try_exists()? {
        return validate_existing_history_rewrap(&path, encoded);
    }
    let mut temporary = NamedTempFile::new_in(&directory)?;
    temporary.write_all(encoded)?;
    temporary.as_file().sync_all()?;
    match temporary.persist_noclobber(&path) {
        Ok(file) => {
            file.sync_all()?;
            Ok(StoreOutcome::Inserted)
        }
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            validate_existing_history_rewrap(&path, encoded)
        }
        Err(error) => Err(error.error.into()),
    }
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
) -> Result<StoreOutcome> {
    transfer.verify_signature()?;
    let directory = state_dir.join(HISTORY_REWRAP_STORE_DIRECTORY);
    fs::create_dir_all(&directory)?;
    let path = directory.join(format!("{}.transfer", transfer.bundle().bundle_id()?));
    if path.try_exists()? {
        return validate_existing_history_rewrap(&path, encoded);
    }
    let mut temporary = NamedTempFile::new_in(&directory)?;
    temporary.write_all(encoded)?;
    temporary.as_file().sync_all()?;
    match temporary.persist_noclobber(&path) {
        Ok(file) => {
            file.sync_all()?;
            Ok(StoreOutcome::Inserted)
        }
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            validate_existing_history_rewrap(&path, encoded)
        }
        Err(error) => Err(error.error.into()),
    }
}

fn persist_history_recovery_checkpoint(
    state_dir: &Path,
    checkpoint: &SignedHistoryRecoveryCheckpoint,
    encoded: &[u8],
) -> Result<StoreOutcome> {
    checkpoint.verify_signature()?;
    let directory = state_dir.join(HISTORY_RECOVERY_STORE_DIRECTORY);
    fs::create_dir_all(&directory)?;
    let checkpoint_id = checkpoint.checkpoint_id()?;
    let path = directory.join(format!(
        "{}-{:020}-{}.checkpoint",
        checkpoint.recovery_id()?,
        checkpoint.next_range_start(),
        encode_hex(&checkpoint_id)
    ));
    if path.try_exists()? {
        return validate_existing_history_rewrap(&path, encoded);
    }
    let mut temporary = NamedTempFile::new_in(&directory)?;
    temporary.write_all(encoded)?;
    temporary.as_file().sync_all()?;
    match temporary.persist_noclobber(&path) {
        Ok(file) => {
            file.sync_all()?;
            Ok(StoreOutcome::Inserted)
        }
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            validate_existing_history_rewrap(&path, encoded)
        }
        Err(error) => Err(error.error.into()),
    }
}

#[derive(Debug, Default)]
struct HistoryRewrapClaimCoverage {
    ranges: Vec<(u64, u64)>,
    bundle_count: usize,
}

fn reconcile_history_rewrap(state_dir: PathBuf, conversation: String) -> Result<()> {
    let device_state = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load recipient device state from {}", state_dir.display()))?;
    let recipient_certificate = device_state
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
    let device_state = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load device state from {}", state_dir.display()))?;
    let certificate = device_state
        .load_certificate()
        .context("load device certificate before seeding history")?;
    let authority_snapshot = device_state
        .load_own_authority_snapshot()
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
    let (conversation_id, existing_count, first_event_id, last_event_id) = run_state_transaction(
        &state_dir,
        || {
            let mut ratchet_state = RatchetState::load_or_create(&state_dir)
                .context("load persistent ratchet state before seeding history")?;
            ratchet_state
                .observe_prekey_pool(&peer_prekey_pool, now_unix_seconds)
                .context("observe peer prekey pool before allocating seeded events")?;
            let event_store = open_event_store(&state_dir)?;
            let local_message_store = open_local_message_store(&state_dir)?;
            let conversation_id = ConversationId::from_label(&conversation);
            let membership = device_state
                .load_conversation_membership(conversation_id.scope_id())
                .context("load trusted conversation membership before seeding history")?;
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
                let author_sequence = device_state
                    .allocate_sequence()
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
                local_message_store
                    .put(projection)
                    .context("persist seeded local history projection")?;
            }
            event_store
                .put_authorized_batch(&events, &membership)
                .context("persist authorized seeded history events")?;
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
    event: &SignedEvent,
    body: &str,
) -> Result<kilogram_store::StoreOutcome, StoreError> {
    let event_id = event.event_id()?;
    let local_device_id = device_state.identity().device_id();
    match store.get(event_id) {
        Ok(projection) => {
            let local_account_id = device_state
                .load_certificate()
                .map_err(kilogram_protocol::ProtocolError::from)?
                .account_id();
            let stored_body = projection.open_for_account(
                event,
                local_device_id,
                local_account_id,
                device_state.encryption(),
            )?;
            if stored_body != body {
                return Err(StoreError::LocalTextProjectionPlaintextConflict { event_id });
            }
            Ok(kilogram_store::StoreOutcome::AlreadyPresent)
        }
        Err(StoreError::LocalTextProjectionMissing { .. }) => {
            let projection = LocalTextProjection::seal_authored(
                event,
                local_device_id,
                device_state.encryption().public_key(),
                body,
            )?;
            store.put(&projection)
        }
        Err(error) => Err(error),
    }
}

fn open_local_text_projection_if_present(
    store: &LocalMessageStore,
    device_state: &DeviceState,
    event: &SignedEvent,
) -> Result<Option<String>, StoreError> {
    let event_id = event.event_id()?;
    match store.get(event_id) {
        Ok(projection) => {
            let local_account_id = device_state
                .load_certificate()
                .map_err(kilogram_protocol::ProtocolError::from)?
                .account_id();
            Ok(Some(projection.open_for_account(
                event,
                device_state.identity().device_id(),
                local_account_id,
                device_state.encryption(),
            )?))
        }
        Err(StoreError::LocalTextProjectionMissing { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

fn ensure_received_local_text_projection(
    store: &LocalMessageStore,
    device_state: &DeviceState,
    event: &SignedEvent,
    decrypted: &DecryptedMessage,
) -> Result<kilogram_store::StoreOutcome, StoreError> {
    let event_id = event.event_id()?;
    if let Some(body) = open_local_text_projection_if_present(store, device_state, event)? {
        if body != decrypted.as_str() {
            return Err(StoreError::LocalTextProjectionPlaintextConflict { event_id });
        }
        return Ok(kilogram_store::StoreOutcome::AlreadyPresent);
    }
    let projection = LocalTextProjection::seal_received(
        event,
        device_state.identity().device_id(),
        device_state.encryption().public_key(),
        decrypted,
    )?;
    store.put(&projection)
}

struct DecryptingSessionStore<'a> {
    state_directory: PathBuf,
    event_writes: &'a EventStore,
    local_message_writes: &'a LocalMessageStore,
    event_reads: CommandEventReadOverlay,
    local_message_reads: CommandLocalMessageReadOverlay,
    device_state: &'a DeviceState,
    ratchet_state: RefCell<RatchetState>,
}

impl<'a> DecryptingSessionStore<'a> {
    fn new(
        state_directory: impl AsRef<Path>,
        store: &'a EventStore,
        local_messages: &'a LocalMessageStore,
        device_state: &'a DeviceState,
        ratchet_state: RatchetState,
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
            ratchet_state: RefCell::new(ratchet_state),
        }
    }

    fn ensure_local_projections(
        &self,
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
            if let Some(projection) = self.ensure_local_projection(event.event())? {
                created.push(projection);
            }
        }
        Ok(created)
    }

    fn ensure_local_projection(
        &self,
        event: &SignedEvent,
    ) -> Result<Option<LocalTextProjection>, StoreError> {
        let event_id = event.event_id()?;
        let local_device_id = self.device_state.identity().device_id();
        match self.local_message_reads.get(event_id) {
            Ok(projection) => {
                let local_account_id = self
                    .device_state
                    .load_certificate()
                    .map_err(kilogram_protocol::ProtocolError::from)?
                    .account_id();
                projection.open_for_account(
                    event,
                    local_device_id,
                    local_account_id,
                    self.device_state.encryption(),
                )?;
                Ok(None)
            }
            Err(error @ StoreError::LocalTextProjectionMissing { .. }) => {
                if event.author_device_id() == local_device_id {
                    return Err(error);
                }
                let local_certificate = self
                    .device_state
                    .load_certificate()
                    .map_err(kilogram_protocol::ProtocolError::from)?;
                let recipient_account_id = event.recipient_device_list()?.account_id();
                if recipient_account_id != local_certificate.account_id() {
                    return Err(
                        kilogram_protocol::ProtocolError::RatchetRecipientAccountMismatch {
                            expected: local_certificate.account_id(),
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
                let (decrypted, _) = self
                    .ratchet_state
                    .borrow_mut()
                    .decrypt(self.device_state.identity(), sender_identity, ciphertext)
                    .map_err(kilogram_protocol::ProtocolError::from)?;
                let projection = LocalTextProjection::seal_received(
                    event,
                    local_device_id,
                    self.device_state.encryption().public_key(),
                    &decrypted,
                )?;
                self.local_message_writes.put(&projection)?;
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
        let staged_projections = run_store_transaction(&self.state_directory, || {
            let created = self.ensure_local_projections(&events)?;
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
        let staged_projections = run_store_transaction(&self.state_directory, || {
            for event in events {
                event.verify_for_membership(membership)?;
            }
            let created = self.ensure_local_projections(events)?;
            self.event_writes.put_authorized_batch(events, membership)?;
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
    use iroh::SecretKey;
    use kilogram_identity::{DeviceEncryptionIdentity, DeviceIdentity};
    use kilogram_transport_iroh::endpoint_builder;

    const UNSUPPORTED_TEST_ALPN: &[u8] = b"kilogram/test/unsupported/1";

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

        let result: Result<()> = run_state_transaction(directory.path(), || {
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
        let peer_ratchet = RatchetState::load_or_create(&peer_state_dir)?;
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
            peer_ratchet,
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
            RatchetState::load_or_create(directory.path().join("local-ratchet"))?,
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
        let mut local_ratchet = RatchetState::load_or_create(directory.path().join("local-state"))?;
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
            local_ratchet,
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
        let listener_device_state = DeviceState::load_or_create(listener_device_directory.path())?;
        let listener = endpoint_builder(RoutePolicy::Auto)
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
                    &listener_device_state,
                    session_binding,
                    requester_account_id,
                )
                .await?;
                connection.closed().await;
                Ok::<_, anyhow::Error>(authorized)
            }
        });

        let client = endpoint_builder(RoutePolicy::Auto).bind().await?;
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
        let listener = endpoint_builder(RoutePolicy::Auto)
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await?;
        let listener_address = listener.addr();
        let session_binding = SyncSessionBinding::from_transport_label(&listener.id().to_string());
        let accept_task = tokio::spawn({
            let listener = listener.clone();
            let listener_state_path = listener_state_path.clone();
            async move {
                let listener_state = DeviceState::load_or_create(listener_state_path)?;
                let connection = accept_authenticated_connection(&listener).await?;
                accept_device_authorization(
                    &connection,
                    &listener_state,
                    session_binding,
                    requester_account_id,
                )
                .await
            }
        });

        let client = endpoint_builder(RoutePolicy::Auto).bind().await?;
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
        let listener = endpoint_builder(RoutePolicy::Auto)
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await?;
        let listener_address = listener.addr();
        let listener_id = listener.id();
        let accept_task = tokio::spawn({
            let listener = listener.clone();
            async move { accept_authenticated_connection(&listener).await }
        });

        let incompatible_client = endpoint_builder(RoutePolicy::Auto).bind().await?;
        let incompatible_result = timeout(
            CONNECTION_TIMEOUT,
            incompatible_client.connect(listener_address.clone(), UNSUPPORTED_TEST_ALPN),
        )
        .await
        .context("incompatible test handshake timed out")?;
        assert!(incompatible_result.is_err());
        incompatible_client.close().await;

        let valid_client = endpoint_builder(RoutePolicy::Auto).bind().await?;
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
