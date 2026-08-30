use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use clap::{Parser, Subcommand, ValueEnum};
use iroh::{
    Endpoint, EndpointAddr,
    endpoint::{Connection, RecvStream, SendStream},
};
use kilogram_identity::{DeviceId, DeviceIdentity, DeviceState};
use kilogram_protocol::{
    ClientRequest, ConversationId, EventPayload, ServerResponse, SignedEvent, SignedSyncInventory,
    SyncSessionBinding,
};
use kilogram_session::{MAX_SYNC_ROUNDS, ServerInventoryOutcome, SyncClient, SyncServer};
use kilogram_store::EventStore;
use kilogram_transport_iroh::{
    ALPN, RoutePolicy, SelectedPathDiagnostics, await_route_policy, endpoint_builder,
    endpoint_builder_for_remote, read_client_request, read_server_response,
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
const TICKET_SIGNATURE_DOMAIN: &[u8] = b"kilogram:connection-ticket-signature:v2\0";
const TICKET_VERSION: u8 = 2;

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

        /// Application device ID allowed to deliver or synchronize events.
        #[arg(long)]
        allow_device: DeviceId,

        /// Also write the public connection ticket to this file.
        #[arg(long)]
        ticket_file: Option<PathBuf>,

        /// How long to wait for a public relay before accepting local connections.
        #[arg(long, default_value_t = 15)]
        relay_wait_seconds: u64,

        /// Transport path required for Kilogram application frames.
        #[arg(long, value_enum, default_value = "auto")]
        route_policy: RoutePolicyArg,
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
    listener_device_id: DeviceId,
    allowed_requester_device_id: DeviceId,
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
        allowed_requester_device_id: DeviceId,
        route_policy: RoutePolicy,
    ) -> Result<Self> {
        let content = ConnectionTicketContent {
            version: TICKET_VERSION,
            endpoint,
            listener_device_id: listener_identity.device_id(),
            allowed_requester_device_id,
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

    fn listener_device_id(&self) -> DeviceId {
        self.content.listener_device_id
    }

    fn allowed_requester_device_id(&self) -> DeviceId {
        self.content.allowed_requester_device_id
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
        self.content
            .listener_device_id
            .verify(&ticket_signing_bytes(&self.content)?, &self.signature)
            .context("verify listener signature on connection ticket")
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
            allow_device,
            ticket_file,
            relay_wait_seconds,
            route_policy,
        } => {
            listen(
                state_dir,
                allow_device,
                ticket_file,
                relay_wait_seconds,
                route_policy.into(),
            )
            .await
        }
        Command::Connect {
            state_dir,
            ticket,
            ticket_file,
            message,
            conversation,
        } => connect(state_dir, ticket, ticket_file, message, conversation).await,
        Command::Sync {
            state_dir,
            ticket,
            ticket_file,
            conversation,
        } => sync(state_dir, ticket, ticket_file, conversation).await,
        Command::History {
            state_dir,
            conversation,
        } => show_history(state_dir, conversation),
        Command::Identity { state_dir } => show_identity(state_dir),
    }
}

async fn listen(
    state_dir: PathBuf,
    allowed_requester_device_id: DeviceId,
    ticket_file: Option<PathBuf>,
    relay_wait_seconds: u64,
    route_policy: RoutePolicy,
) -> Result<()> {
    let device_state = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load device state from {}", state_dir.display()))?;
    let event_store = open_event_store(&state_dir)?;
    let endpoint = endpoint_builder(route_policy)
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .context("bind listening Iroh endpoint")?;

    wait_for_relay(&endpoint, route_policy, relay_wait_seconds).await?;

    let ticket = ConnectionTicket::new(
        endpoint.addr(),
        device_state.identity(),
        allowed_requester_device_id,
        route_policy,
    )?
    .encode()?;
    println!("transport_endpoint_id={}", endpoint.id());
    println!("device_id={}", device_state.identity().device_id());
    println!("route_policy={}", route_policy.as_str());
    println!("allowed_requester_device_id={allowed_requester_device_id}");
    println!("ticket={ticket}");

    if let Some(path) = ticket_file {
        tokio::fs::write(&path, &ticket)
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

    let (mut send, mut receive) =
        accept_bi(&connection, "accept initial bidirectional stream").await?;
    let request = read_client_request(&mut receive).await?;
    match request {
        ClientRequest::DeliverEvent(event) => {
            handle_delivery_request(
                &device_state,
                &event_store,
                &mut send,
                event,
                allowed_requester_device_id,
            )
            .await?;
        }
        ClientRequest::SyncInventory(inventory) => {
            let session_binding =
                SyncSessionBinding::from_transport_label(&endpoint.id().to_string());
            handle_sync_request(
                &device_state,
                &event_store,
                &connection,
                send,
                inventory,
                session_binding,
                allowed_requester_device_id,
            )
            .await?;
        }
        ClientRequest::SyncEvents(_) => bail!("sync event batch cannot be the first request"),
    }

    print_transport_diagnostics(&connection, route_policy).await?;
    let _ = timeout(Duration::from_secs(2), connection.closed()).await;
    endpoint.close().await;
    Ok(())
}

async fn handle_delivery_request(
    device_state: &DeviceState,
    event_store: &EventStore,
    send: &mut SendStream,
    event: SignedEvent,
    allowed_requester_device_id: DeviceId,
) -> Result<()> {
    ensure!(
        event.author_device_id() == allowed_requester_device_id,
        "event author is not the requester device allowed by this listener"
    );
    let event_id = event.event_id().context("calculate received event ID")?;
    let EventPayload::Text { body } = event.payload() else {
        bail!("listener expected a text event");
    };
    let received_store_outcome = event_store
        .put(&event)
        .context("persist received event before acknowledging it")?;
    println!("received_event_id={event_id}");
    println!("received_author_device_id={}", event.author_device_id());
    println!("received_author_sequence={}", event.author_sequence());
    println!("received={body}");
    println!("received_store={received_store_outcome:?}");

    let acknowledgement_sequence = device_state
        .allocate_sequence()
        .context("allocate acknowledgement sequence")?;
    let acknowledgement = SignedEvent::sign_acknowledgement(
        device_state.identity(),
        event.conversation_id(),
        acknowledgement_sequence,
        vec![event_id],
        event_id,
    )
    .context("sign acknowledgement event")?;
    let acknowledgement_id = acknowledgement
        .event_id()
        .context("calculate acknowledgement event ID")?;
    let acknowledgement_store_outcome = event_store
        .put(&acknowledgement)
        .context("persist acknowledgement before sending it")?;
    write_server_response(send, &ServerResponse::EventAcknowledgement(acknowledgement)).await?;
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
    allowed_requester_device_id: DeviceId,
) -> Result<()> {
    let server = SyncServer::new(
        device_state.identity(),
        event_store,
        expected_session,
        allowed_requester_device_id,
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
        if round_number == MAX_SYNC_ROUNDS {
            bail!("sync exceeded the limit of {MAX_SYNC_ROUNDS} rounds");
        }

        let (next_send, mut next_receive) =
            accept_bi(connection, "accept next sync inventory stream").await?;
        inventory = match read_client_request(&mut next_receive).await? {
            ClientRequest::SyncInventory(inventory) => inventory,
            _ => bail!("listener expected another sync inventory for continuation"),
        };
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
) -> Result<()> {
    let device_state = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load device state from {}", state_dir.display()))?;
    let event_store = open_event_store(&state_dir)?;

    let ticket = load_connection_ticket(ticket, ticket_file).await?;
    let expected_listener_device_id = ticket.listener_device_id();
    let route_policy = ticket.route_policy();
    ensure!(
        device_state.identity().device_id() == ticket.allowed_requester_device_id(),
        "this device is not the requester authorized by the connection ticket"
    );

    let endpoint = endpoint_builder_for_remote(route_policy, ticket.endpoint())?
        .bind()
        .await
        .context("bind connecting Iroh endpoint")?;
    println!("transport_endpoint_id={}", endpoint.id());
    println!("device_id={}", device_state.identity().device_id());
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

    let (mut send, mut receive) =
        open_bi(&connection, "open delivery bidirectional stream").await?;
    let conversation_id = ConversationId::from_label(&conversation);
    let author_sequence = device_state
        .allocate_sequence()
        .context("allocate message sequence")?;
    let parents = event_store
        .frontier(conversation_id)
        .context("calculate local conversation frontier")?;
    let event = SignedEvent::sign_text(
        device_state.identity(),
        conversation_id,
        author_sequence,
        parents,
        message,
    )
    .context("sign text event")?;
    let event_id = event.event_id().context("calculate sent event ID")?;
    let sent_store_outcome = event_store
        .put(&event)
        .context("persist signed event before sending it")?;
    write_client_request(&mut send, &ClientRequest::DeliverEvent(event.clone())).await?;
    println!("sent_event_id={event_id}");
    println!("sent_author_sequence={author_sequence}");
    println!("sent_store={sent_store_outcome:?}");

    let acknowledgement = match read_server_response(&mut receive).await? {
        ServerResponse::EventAcknowledgement(event) => event,
        _ => bail!("connector expected an event acknowledgement response"),
    };
    ensure!(
        acknowledgement.conversation_id() == conversation_id,
        "acknowledgement belongs to a different conversation"
    );
    ensure!(
        acknowledgement.author_device_id() == expected_listener_device_id,
        "acknowledgement was signed by a device not named in the connection ticket"
    );
    let EventPayload::Acknowledgement {
        acknowledged_event_id,
    } = acknowledgement.payload()
    else {
        bail!("connector expected an acknowledgement event");
    };
    ensure!(
        *acknowledged_event_id == event_id,
        "acknowledgement references a different event"
    );
    ensure!(
        acknowledgement.parents() == [event_id],
        "acknowledgement does not causally reference the sent event"
    );
    let acknowledgement_store_outcome = event_store
        .put(&acknowledgement)
        .context("persist verified acknowledgement")?;
    println!("acknowledgement_event_id={}", acknowledgement.event_id()?);
    println!(
        "acknowledgement_author_device_id={}",
        acknowledgement.author_device_id()
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
) -> Result<()> {
    let device_state = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load device state from {}", state_dir.display()))?;
    let event_store = open_event_store(&state_dir)?;
    let ticket = load_connection_ticket(ticket, ticket_file).await?;
    let expected_listener_device_id = ticket.listener_device_id();
    let route_policy = ticket.route_policy();
    ensure!(
        device_state.identity().device_id() == ticket.allowed_requester_device_id(),
        "this device is not the requester authorized by the connection ticket"
    );
    let session_binding =
        SyncSessionBinding::from_transport_label(&ticket.endpoint().id.to_string());
    let conversation_id = ConversationId::from_label(&conversation);
    let client = SyncClient::new(
        device_state.identity(),
        &event_store,
        conversation_id,
        session_binding,
        expected_listener_device_id,
    );

    let endpoint = endpoint_builder_for_remote(route_policy, ticket.endpoint())?
        .bind()
        .await
        .context("bind syncing Iroh endpoint")?;
    println!("transport_endpoint_id={}", endpoint.id());
    println!("device_id={}", device_state.identity().device_id());
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
    Ok(())
}

fn show_history(state_dir: PathBuf, conversation: String) -> Result<()> {
    let event_store = open_event_store(&state_dir)?;
    let conversation_id = ConversationId::from_label(&conversation);
    let events = event_store
        .load_conversation(conversation_id)
        .context("load and verify local conversation history")?;
    let frontier = event_store
        .frontier(conversation_id)
        .context("calculate local conversation frontier")?;

    println!("conversation_id={conversation_id}");
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
        let event = stored.event;
        let parents = event
            .parents()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        match event.payload() {
            EventPayload::Text { body } => println!(
                "event_id={} author_device_id={} author_sequence={} parents=[{parents}] payload=text body={body:?}",
                stored.id,
                event.author_device_id(),
                event.author_sequence()
            ),
            EventPayload::Acknowledgement {
                acknowledged_event_id,
            } => println!(
                "event_id={} author_device_id={} author_sequence={} parents=[{parents}] payload=acknowledgement acknowledged_event_id={acknowledged_event_id}",
                stored.id,
                event.author_device_id(),
                event.author_sequence()
            ),
        }
    }
    Ok(())
}

fn open_event_store(state_dir: &Path) -> Result<EventStore> {
    let path = state_dir.join(EVENT_STORE_DIRECTORY);
    EventStore::open(&path).with_context(|| format!("open event store at {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroh::SecretKey;
    use kilogram_identity::DeviceIdentity;

    const UNSUPPORTED_TEST_ALPN: &[u8] = b"kilogram/test/unsupported/1";

    #[test]
    fn connection_ticket_round_trips() -> Result<()> {
        let endpoint = EndpointAddr::new(SecretKey::generate().public());
        let listener_identity = DeviceIdentity::generate()?;
        let listener_device_id = listener_identity.device_id();
        let allowed_requester_device_id = DeviceIdentity::generate()?.device_id();
        let encoded = ConnectionTicket::new(
            endpoint.clone(),
            &listener_identity,
            allowed_requester_device_id,
            RoutePolicy::DirectOnly,
        )?
        .encode()?;
        let decoded = ConnectionTicket::decode(&encoded)?;

        assert_eq!(decoded.content.version, TICKET_VERSION);
        assert_eq!(decoded.endpoint(), &endpoint);
        assert_eq!(decoded.listener_device_id(), listener_device_id);
        assert_eq!(decoded.route_policy(), RoutePolicy::DirectOnly);
        assert_eq!(
            decoded.allowed_requester_device_id(),
            allowed_requester_device_id
        );
        Ok(())
    }

    #[test]
    fn connection_ticket_rejects_unknown_version() -> Result<()> {
        let identity = DeviceIdentity::generate()?;
        let mut ticket = ConnectionTicket::new(
            EndpointAddr::new(SecretKey::generate().public()),
            &identity,
            DeviceIdentity::generate()?.device_id(),
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
        let mut ticket = ConnectionTicket::new(
            EndpointAddr::new(SecretKey::generate().public()),
            &identity,
            DeviceIdentity::generate()?.device_id(),
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
        let mut ticket = ConnectionTicket::new(
            EndpointAddr::new(SecretKey::generate().public()),
            &identity,
            DeviceIdentity::generate()?.device_id(),
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
