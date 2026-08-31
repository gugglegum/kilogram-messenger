use std::{
    fs::{self, OpenOptions},
    io::Write,
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
    AccountAuthoritySnapshot, AccountId, AccountRootState, AuthorizedDevice,
    ConversationMembershipSnapshot, DeviceCapability, DeviceCertificate, DeviceId, DeviceIdentity,
    DeviceState, verify_device_authorization_with_snapshot,
};
use kilogram_protocol::{
    AuthorizedEvent, ClientRequest, ConversationId, DeviceAuthorizationAccepted,
    DeviceAuthorizationRejected, EventPayload, MAX_INVENTORY_EVENT_IDS, ServerResponse,
    SignedDeviceSessionAuthorization, SignedEvent, SignedSyncInventory, SyncPause, SyncPaused,
    SyncSessionBinding,
};
use kilogram_session::{
    MAX_SYNC_ROUNDS, ServerInventoryOutcome, SessionStore, SyncClient, SyncServer,
    authorize_device_session,
};
use kilogram_store::{EventStore, StoreError};
use kilogram_transport_iroh::{
    ALPN, RoutePolicy, SelectedPathDiagnostics, await_route_policy, endpoint_builder_for_remote,
    endpoint_builder_with_relay, read_client_request, read_server_response,
    selected_path_diagnostics, write_client_request, write_server_response,
};
use serde::{Deserialize, Serialize};
use tokio::time::timeout;

const EVENT_STORE_DIRECTORY: &str = "events";
const DIRECT_PATH_DIAGNOSTIC_WAIT: Duration = Duration::from_secs(3);
const ROUTE_POLICY_WAIT: Duration = Duration::from_secs(15);
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);
const CLIENT_RELAY_WAIT_SECONDS: u64 = 30;
const STREAM_OPEN_TIMEOUT: Duration = Duration::from_secs(15);
const TICKET_SIGNATURE_DOMAIN: &[u8] = b"kilogram:connection-ticket-signature:v5\0";
const TICKET_VERSION: u8 = 5;

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
}

#[derive(Clone, Copy, Debug, ValueEnum)]
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

#[derive(Debug, Serialize, Deserialize)]
struct ConnectionTicketContent {
    version: u8,
    endpoint: EndpointAddr,
    listener_certificate: DeviceCertificate,
    listener_authority_snapshot: AccountAuthoritySnapshot,
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
        listener_authority_snapshot: AccountAuthoritySnapshot,
        allowed_requester_account_id: AccountId,
        route_policy: RoutePolicy,
    ) -> Result<Self> {
        ensure!(
            listener_certificate.device_id() == listener_identity.device_id(),
            "listener certificate belongs to a different device"
        );
        verify_device_authorization_with_snapshot(
            listener_certificate.account_id(),
            &listener_certificate,
            &listener_authority_snapshot,
            &DeviceCapability::MESSAGING,
        )
        .context("verify listener certificate for connection ticket")?;
        let content = ConnectionTicketContent {
            version: TICKET_VERSION,
            endpoint,
            listener_certificate,
            listener_authority_snapshot,
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
        &self.content.listener_authority_snapshot
    }

    fn listener_certificate(&self) -> &DeviceCertificate {
        &self.content.listener_certificate
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
            .listener_authority_snapshot
            .verify_for_account(self.content.listener_certificate.account_id())?;
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
            &self.content.listener_authority_snapshot,
            &DeviceCapability::MESSAGING,
        )
        .context("verify listener Account Root authorization")
    }

    fn verify_listener_account(&self, expected_account: AccountId) -> Result<()> {
        self.verify()?;
        self.content
            .listener_authority_snapshot
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
    match Cli::parse().command {
        Command::Listen {
            state_dir,
            allow_account,
            ticket_file,
            relay_wait_seconds,
            route_policy,
            relay_url,
        } => {
            listen(
                state_dir,
                allow_account,
                ticket_file,
                relay_wait_seconds,
                route_policy.into(),
                relay_url,
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
        } => seed_history(
            state_dir,
            conversation,
            count,
            message_prefix,
            peer_certificate_file,
        ),
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
    }
}

async fn listen(
    state_dir: PathBuf,
    allowed_requester_account_id: AccountId,
    ticket_file: Option<PathBuf>,
    relay_wait_seconds: u64,
    route_policy: RoutePolicy,
    relay_url: Option<RelayUrl>,
) -> Result<()> {
    let device_state = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load device state from {}", state_dir.display()))?;
    let listener_certificate = device_state
        .load_certificate()
        .context("load listener Account Root certificate")?;
    let listener_authority_snapshot = device_state
        .load_own_authority_snapshot()
        .context("load listener Account Root authority snapshot")?;
    let event_store = open_event_store(&state_dir)?;
    let endpoint = endpoint_builder_with_relay(route_policy, relay_url)
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .context("bind listening Iroh endpoint")?;

    wait_for_relay(&endpoint, route_policy, relay_wait_seconds).await?;

    let ticket = ConnectionTicket::new(
        endpoint.addr(),
        device_state.identity(),
        listener_certificate,
        listener_authority_snapshot,
        allowed_requester_account_id,
        route_policy,
    )?;
    let encoded_ticket = ticket.encode()?;
    println!("transport_endpoint_id={}", endpoint.id());
    println!("account_id={}", ticket.listener_account_id());
    println!("device_id={}", device_state.identity().device_id());
    println!("route_policy={}", route_policy.as_str());
    println!("allowed_requester_account_id={allowed_requester_account_id}");
    println!(
        "authority_revision={}",
        ticket.listener_authority_snapshot().revision()
    );
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
                &device_state,
                &event_store,
                &mut send,
                event,
                authorized_requester.account_id(),
                authorized_requester.device_id(),
            )
            .await?;
        }
        ClientRequest::SyncInventory(inventory) => {
            handle_sync_request(
                &device_state,
                &event_store,
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
        ClientRequest::AuthorizeDevice(_) => {
            bail!("device authorization cannot be repeated on an authorized connection")
        }
    }

    print_transport_diagnostics(&connection, route_policy).await?;
    let _ = timeout(Duration::from_secs(2), connection.closed()).await;
    endpoint.close().await;
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

async fn handle_delivery_request(
    device_state: &DeviceState,
    event_store: &EventStore,
    send: &mut SendStream,
    event: AuthorizedEvent,
    allowed_requester_account_id: AccountId,
    allowed_requester_device_id: DeviceId,
) -> Result<()> {
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
        allowed_requester_account_id,
    )?;
    event_store
        .authorized_inventory(signed_event.conversation_id(), &membership)
        .context("validate existing authorized history before delivery")?;
    event
        .verify_for_membership(&membership)
        .context("verify received event author and conversation membership")?;
    ensure!(
        event.author_account_id() == allowed_requester_account_id,
        "event author account is not the requester account allowed by this listener"
    );
    ensure!(
        signed_event.author_device_id() == allowed_requester_device_id,
        "event author is not the requester device allowed by this listener"
    );
    let event_id = signed_event
        .event_id()
        .context("calculate received event ID")?;
    let EventPayload::EncryptedText { .. } = signed_event.payload() else {
        bail!("listener expected a text event");
    };
    let body = signed_event
        .decrypt_text(
            device_state.identity().device_id(),
            device_state.encryption(),
        )
        .context("decrypt received text for this listener device")?;
    let received_store_outcome = event_store
        .put_authorized(&event, &membership)
        .context("persist received event before acknowledging it")?;
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
    println!("received_store={received_store_outcome:?}");

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
        listener_certificate,
        listener_authority_snapshot,
    )
    .context("attach listener Account Root authorization to acknowledgement")?;
    let acknowledgement_id = acknowledgement
        .event()
        .event_id()
        .context("calculate acknowledgement event ID")?;
    let acknowledgement_store_outcome = event_store
        .put_authorized(&acknowledgement, &membership)
        .context("persist acknowledgement before sending it")?;
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
    event_store: &EventStore,
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
    let decrypting_store = DecryptingSessionStore::new(event_store, device_state);
    let server = SyncServer::new(
        device_state.identity(),
        &decrypting_store,
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

    let ticket = load_connection_ticket(ticket, ticket_file).await?;
    ticket.verify_listener_account(expected_listener_account_id)?;
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
    let author_sequence = device_state
        .allocate_sequence()
        .context("allocate message sequence")?;
    let parents = event_store
        .frontier(conversation_id)
        .context("calculate local conversation frontier")?;
    let signed_event = SignedEvent::sign_encrypted_text(
        device_state.identity(),
        conversation_id,
        author_sequence,
        parents,
        message,
        [
            (
                requester_certificate.device_id(),
                requester_certificate.encryption_public_key(),
            ),
            (
                ticket.listener_certificate().device_id(),
                ticket.listener_certificate().encryption_public_key(),
            ),
        ],
    )
    .context("encrypt and sign text event")?;
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
    let sent_store_outcome = event_store
        .put_authorized(&event, &membership)
        .context("persist authorized event before sending it")?;
    write_client_request(&mut send, &ClientRequest::DeliverEvent(event.clone())).await?;
    println!("sent_event_id={event_id}");
    println!("sent_author_sequence={author_sequence}");
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
    let acknowledgement_store_outcome = event_store
        .put_authorized(&acknowledgement, &membership)
        .context("persist verified acknowledgement")?;
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
    let event_store = open_event_store(&state_dir)?;
    let ticket = load_connection_ticket(ticket, ticket_file).await?;
    ticket.verify_listener_account(expected_listener_account_id)?;
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
    let decrypting_store = DecryptingSessionStore::new(&event_store, &device_state);
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

fn show_history(state_dir: PathBuf, conversation: String) -> Result<()> {
    let device_state = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load device state from {}", state_dir.display()))?;
    let certificate = device_state
        .load_certificate()
        .context("load device certificate before reading history")?;
    let event_store = open_event_store(&state_dir)?;
    let conversation_id = ConversationId::from_label(&conversation);
    let membership = device_state
        .load_conversation_membership(conversation_id.scope_id())
        .context("load trusted conversation membership before reading history")?;
    membership
        .require_member(certificate.account_id())
        .context("this device account is not a member of the conversation")?;
    let events = event_store
        .load_authorized_conversation(conversation_id, &membership)
        .context("load and verify authorized local conversation history")?;
    let frontier = event_store
        .frontier(conversation_id)
        .context("calculate local conversation frontier")?;

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
            EventPayload::EncryptedText { .. } => {
                let body = event
                    .decrypt_text(
                        device_state.identity().device_id(),
                        device_state.encryption(),
                    )
                    .with_context(|| format!("decrypt stored event {}", stored.id))?;
                println!(
                    "event_id={} author_account_id={author_account_id} author_device_id={} author_sequence={} parents=[{parents}] payload=encrypted-text body={body:?}",
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

fn seed_history(
    state_dir: PathBuf,
    conversation: String,
    count: usize,
    message_prefix: String,
    peer_certificate_file: PathBuf,
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
    let event_store = open_event_store(&state_dir)?;
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
    let mut first_event_id = None;
    let mut last_event_id = None;
    for index in 1..=count {
        let author_sequence = device_state
            .allocate_sequence()
            .context("allocate seeded event sequence")?;
        let event = SignedEvent::sign_encrypted_text(
            device_state.identity(),
            conversation_id,
            author_sequence,
            parents,
            format!("{message_prefix}-{index}"),
            [
                (certificate.device_id(), certificate.encryption_public_key()),
                (
                    peer_certificate.device_id(),
                    peer_certificate.encryption_public_key(),
                ),
            ],
        )
        .context("encrypt and sign seeded history event")?;
        let event_id = event.event_id().context("calculate seeded event ID")?;
        first_event_id.get_or_insert(event_id);
        last_event_id = Some(event_id);
        parents = vec![event_id];
        events.push(
            AuthorizedEvent::new(event, certificate.clone(), authority_snapshot.clone())
                .context("attach Account Root authorization to seeded event")?,
        );
    }
    event_store
        .put_authorized_batch(&events, &membership)
        .context("persist authorized seeded history events")?;

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

struct DecryptingSessionStore<'a> {
    store: &'a EventStore,
    device_state: &'a DeviceState,
}

impl<'a> DecryptingSessionStore<'a> {
    fn new(store: &'a EventStore, device_state: &'a DeviceState) -> Self {
        Self {
            store,
            device_state,
        }
    }

    fn require_decryptable(&self, events: &[AuthorizedEvent]) -> Result<(), StoreError> {
        for event in events {
            if matches!(event.event().payload(), EventPayload::EncryptedText { .. }) {
                event.event().decrypt_text(
                    self.device_state.identity().device_id(),
                    self.device_state.encryption(),
                )?;
            }
        }
        Ok(())
    }
}

impl SessionStore for DecryptingSessionStore<'_> {
    fn inventory(
        &self,
        conversation_id: ConversationId,
        membership: &ConversationMembershipSnapshot,
    ) -> Result<Vec<kilogram_protocol::EventId>, StoreError> {
        self.store.authorized_inventory(conversation_id, membership)
    }

    fn events_by_id(
        &self,
        conversation_id: ConversationId,
        event_ids: &[kilogram_protocol::EventId],
        membership: &ConversationMembershipSnapshot,
    ) -> Result<Vec<AuthorizedEvent>, StoreError> {
        let events = self
            .store
            .authorized_events_by_id(conversation_id, event_ids, membership)?;
        self.require_decryptable(&events)?;
        Ok(events)
    }

    fn put_events(
        &self,
        events: &[AuthorizedEvent],
        membership: &ConversationMembershipSnapshot,
    ) -> Result<(), StoreError> {
        self.require_decryptable(events)?;
        self.store.put_authorized_batch(events, membership)?;
        Ok(())
    }
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

    fn authority_for(
        identity: &DeviceIdentity,
    ) -> Result<(AccountId, DeviceCertificate, AccountAuthoritySnapshot)> {
        let directory = tempfile::tempdir()?;
        let root = AccountRootState::create(directory.path())?;
        let encryption = DeviceEncryptionIdentity::generate()?;
        let certificate = root.issue_device_certificate(
            identity.device_id(),
            encryption.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let snapshot = root.authority_snapshot()?;
        Ok((root.account_id(), certificate, snapshot))
    }

    #[test]
    fn seeded_history_is_signed_chained_and_persistent() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let account_dir = directory.path().join("account");
        let state_dir = directory.path().join("seeded-device");
        let peer_state_dir = directory.path().join("peer-device");
        let peer_certificate_file = directory.path().join("peer-device.cert");
        let conversation = "seed-history-test";
        create_account(account_dir.clone())?;
        enroll_device(account_dir.clone(), state_dir.clone(), None)?;
        let account = AccountRootState::load(&account_dir)?;
        let peer = DeviceState::load_or_create(&peer_state_dir)?;
        let peer_certificate = account.issue_device_certificate(
            peer.identity().device_id(),
            peer.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        write_new_authority_file(&peer_certificate_file, &peer_certificate.encode()?)?;
        let membership = account.create_conversation_membership(
            ConversationId::from_label(conversation).scope_id(),
            &[],
        )?;
        DeviceState::load_or_create(&state_dir)?.install_conversation_membership(&membership)?;
        seed_history(
            state_dir.clone(),
            conversation.to_owned(),
            3,
            "fixture".to_owned(),
            peer_certificate_file,
        )?;

        let store = open_event_store(&state_dir)?;
        let conversation_id = ConversationId::from_label(conversation);
        let events = store.load_authorized_conversation(conversation_id, &membership)?;
        assert_eq!(events.len(), 3);
        assert_eq!(store.frontier(conversation_id)?.len(), 1);
        assert!(
            events
                .iter()
                .all(|stored| stored.event.verify_for_membership(&membership).is_ok())
        );
        Ok(())
    }

    #[test]
    fn sync_store_rejects_event_without_a_local_recipient_box() -> Result<()> {
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
        let outsider_identity = DeviceIdentity::generate()?;
        let outsider_encryption = DeviceEncryptionIdentity::generate()?;
        let conversation_id = ConversationId::from_label("missing-local-recipient");
        let membership = local_root.create_conversation_membership(
            conversation_id.scope_id(),
            &[peer_root.account_id()],
        )?;
        let event = AuthorizedEvent::new(
            SignedEvent::sign_encrypted_text(
                &peer_identity,
                conversation_id,
                0,
                Vec::new(),
                "must not reach disk".to_owned(),
                [
                    (peer_identity.device_id(), peer_encryption.public_key()),
                    (
                        outsider_identity.device_id(),
                        outsider_encryption.public_key(),
                    ),
                ],
            )?,
            peer_certificate,
            peer_root.authority_snapshot()?,
        )?;
        let store = EventStore::open(directory.path().join("events"))?;
        let guarded = DecryptingSessionStore::new(&store, &local_state);

        assert!(matches!(
            guarded.put_events(&[event], &membership),
            Err(StoreError::Protocol(
                kilogram_protocol::ProtocolError::MissingEncryptedRecipient(device_id)
            )) if device_id == local_state.identity().device_id()
        ));
        assert!(
            store
                .authorized_inventory(conversation_id, &membership)?
                .is_empty()
        );
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
        let (listener_account_id, listener_certificate, listener_snapshot) =
            authority_for(&listener_identity)?;
        let requester_identity = DeviceIdentity::generate()?;
        let (allowed_requester_account_id, _, _) = authority_for(&requester_identity)?;
        let encoded = ConnectionTicket::new(
            endpoint.clone(),
            &listener_identity,
            listener_certificate,
            listener_snapshot,
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
        let old_snapshot = listener_root.authority_snapshot()?;
        let requester = DeviceIdentity::generate()?;
        let (requester_account_id, _, _) = authority_for(&requester)?;
        let encoded = ConnectionTicket::new(
            EndpointAddr::new(SecretKey::generate().public()),
            &listener_identity,
            listener_certificate,
            old_snapshot,
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
        let (_, certificate, snapshot) = authority_for(&identity)?;
        let requester = DeviceIdentity::generate()?;
        let (requester_account_id, _, _) = authority_for(&requester)?;
        let mut ticket = ConnectionTicket::new(
            EndpointAddr::new(SecretKey::generate().public()),
            &identity,
            certificate,
            snapshot,
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
        let (_, certificate, snapshot) = authority_for(&identity)?;
        let requester = DeviceIdentity::generate()?;
        let (requester_account_id, _, _) = authority_for(&requester)?;
        let mut ticket = ConnectionTicket::new(
            EndpointAddr::new(SecretKey::generate().public()),
            &identity,
            certificate,
            snapshot,
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
        let (_, certificate, snapshot) = authority_for(&identity)?;
        let requester = DeviceIdentity::generate()?;
        let (requester_account_id, _, _) = authority_for(&requester)?;
        let mut ticket = ConnectionTicket::new(
            EndpointAddr::new(SecretKey::generate().public()),
            &identity,
            certificate,
            snapshot,
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
        let (requester_account_id, requester_certificate, requester_snapshot) =
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
