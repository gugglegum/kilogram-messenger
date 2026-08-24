use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use clap::{Parser, Subcommand};
use iroh::{
    Endpoint, EndpointAddr,
    endpoint::{RecvStream, SendStream, presets},
};
use kilogram_identity::{DeviceId, DeviceIdentity, DeviceState};
use kilogram_protocol::{
    ClientRequest, ConversationId, EventId, EventPayload, MAX_SYNC_EVENTS_PER_BATCH,
    ServerResponse, SignedEvent, SignedSyncInventory, SyncComplete, SyncDiff, SyncEventBatch,
    SyncRejected, SyncRejectionReason, SyncSessionBinding,
};
use kilogram_store::EventStore;
use serde::{Deserialize, Serialize};
use tokio::time::timeout;

const ALPN: &[u8] = b"kilogram/m0/sync/1";
const EVENT_STORE_DIRECTORY: &str = "events";
const MAX_WIRE_MESSAGE_BYTES: usize = 8 * 1024 * 1024;
const TICKET_SIGNATURE_DOMAIN: &[u8] = b"kilogram:connection-ticket-signature:v1\0";
const TICKET_VERSION: u8 = 1;

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
    /// Listen for one connection, acknowledge one message, then exit.
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

    /// Reconcile one bounded batch of conversation events with a listener.
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

#[derive(Debug, Serialize, Deserialize)]
struct ConnectionTicketContent {
    version: u8,
    endpoint: EndpointAddr,
    listener_device_id: DeviceId,
    allowed_requester_device_id: DeviceId,
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
    ) -> Result<Self> {
        let content = ConnectionTicketContent {
            version: TICKET_VERSION,
            endpoint,
            listener_device_id: listener_identity.device_id(),
            allowed_requester_device_id,
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
        } => listen(state_dir, allow_device, ticket_file, relay_wait_seconds).await,
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
) -> Result<()> {
    let device_state = DeviceState::load_or_create(&state_dir)
        .with_context(|| format!("load device state from {}", state_dir.display()))?;
    let event_store = open_event_store(&state_dir)?;
    let endpoint = Endpoint::builder(presets::N0)
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .context("bind listening Iroh endpoint")?;

    if relay_wait_seconds > 0 {
        match timeout(Duration::from_secs(relay_wait_seconds), endpoint.online()).await {
            Ok(()) => println!("relay_status=online"),
            Err(_) => eprintln!("relay_status=timeout (local/direct connections can still work)"),
        }
    }

    let ticket = ConnectionTicket::new(
        endpoint.addr(),
        device_state.identity(),
        allowed_requester_device_id,
    )?
    .encode()?;
    println!("transport_endpoint_id={}", endpoint.id());
    println!("device_id={}", device_state.identity().device_id());
    println!("allowed_requester_device_id={allowed_requester_device_id}");
    println!("ticket={ticket}");

    if let Some(path) = ticket_file {
        tokio::fs::write(&path, &ticket)
            .await
            .with_context(|| format!("write ticket to {}", path.display()))?;
        println!("ticket_file={}", path.display());
    }

    println!("status=listening");
    let incoming = endpoint
        .accept()
        .await
        .context("listener endpoint closed before receiving a connection")?;
    let connection = incoming.await.context("accept Iroh connection")?;
    println!("peer_id={}", connection.remote_id());

    let (mut send, mut receive) = connection
        .accept_bi()
        .await
        .context("accept bidirectional stream")?;
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
                &mut send,
                inventory,
                session_binding,
                allowed_requester_device_id,
            )
            .await?;
        }
        ClientRequest::SyncEvents(_) => bail!("sync event batch cannot be the first request"),
    }

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
    first_send: &mut SendStream,
    inventory: SignedSyncInventory,
    expected_session: SyncSessionBinding,
    allowed_requester_device_id: DeviceId,
) -> Result<()> {
    inventory
        .verify_for_session(expected_session)
        .context("verify sync inventory session binding")?;
    let conversation_id = inventory.conversation_id();
    if inventory.requester_device_id() != allowed_requester_device_id {
        let rejected = SyncRejected::new(conversation_id, SyncRejectionReason::RequesterNotAllowed);
        write_server_response(first_send, &ServerResponse::SyncRejected(rejected)).await?;
        println!("sync_rejected=RequesterNotAllowed");
        println!("status=rejected");
        return Ok(());
    }
    let requester_is_known = event_store
        .contains_author(conversation_id, inventory.requester_device_id())
        .context("check sync requester against local conversation authors")?;
    if !requester_is_known {
        let rejected = SyncRejected::new(conversation_id, SyncRejectionReason::RequesterNotKnown);
        write_server_response(first_send, &ServerResponse::SyncRejected(rejected)).await?;
        println!("sync_rejected=RequesterNotKnown");
        println!("status=rejected");
        return Ok(());
    }

    let plan = event_store
        .plan_sync(
            conversation_id,
            inventory.event_ids(),
            MAX_SYNC_EVENTS_PER_BATCH,
        )
        .context("calculate bounded sync diff")?;
    let requested_event_ids = plan.requested_from_remote;
    let events_for_remote_count = plan.events_for_remote.len();
    let more_available = plan.more_available;
    let diff = SyncDiff::sign(
        device_state.identity(),
        conversation_id,
        expected_session,
        requested_event_ids.clone(),
        plan.events_for_remote,
        more_available,
    )?;
    write_server_response(first_send, &ServerResponse::SyncDiff(diff)).await?;

    let (mut second_send, mut second_receive) = connection
        .accept_bi()
        .await
        .context("accept sync event batch stream")?;
    let batch = match read_client_request(&mut second_receive).await? {
        ClientRequest::SyncEvents(batch) => batch,
        _ => bail!("listener expected a sync event batch as the second request"),
    };
    ensure!(
        batch.conversation_id() == conversation_id,
        "sync event batch belongs to a different conversation"
    );
    let supplied_event_ids = event_ids(batch.events())?;
    ensure!(
        same_event_ids(&supplied_event_ids, &requested_event_ids),
        "sync event batch does not exactly satisfy the requested event IDs"
    );
    for event in batch.into_events() {
        event_store
            .put(&event)
            .context("persist event received during sync")?;
    }

    let complete = SyncComplete::new(conversation_id, supplied_event_ids.clone(), more_available)?;
    write_server_response(&mut second_send, &ServerResponse::SyncComplete(complete)).await?;
    println!("sync_sent_events={events_for_remote_count}");
    println!("sync_received_events={}", supplied_event_ids.len());
    println!("sync_more_available={more_available}");
    println!("status=synchronized");
    Ok(())
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
    ensure!(
        device_state.identity().device_id() == ticket.allowed_requester_device_id(),
        "this device is not the requester authorized by the connection ticket"
    );

    let endpoint = Endpoint::bind(presets::N0)
        .await
        .context("bind connecting Iroh endpoint")?;
    println!("transport_endpoint_id={}", endpoint.id());
    println!("device_id={}", device_state.identity().device_id());

    let connection = endpoint
        .connect(ticket.endpoint().clone(), ALPN)
        .await
        .context("connect to listening endpoint")?;
    println!("peer_id={}", connection.remote_id());

    let (mut send, mut receive) = connection
        .open_bi()
        .await
        .context("open bidirectional stream")?;
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
    ensure!(
        device_state.identity().device_id() == ticket.allowed_requester_device_id(),
        "this device is not the requester authorized by the connection ticket"
    );
    let session_binding =
        SyncSessionBinding::from_transport_label(&ticket.endpoint().id.to_string());
    let conversation_id = ConversationId::from_label(&conversation);
    let inventory_ids = event_store
        .inventory(conversation_id)
        .context("build local sync inventory")?;
    let inventory = SignedSyncInventory::sign(
        device_state.identity(),
        conversation_id,
        session_binding,
        inventory_ids.clone(),
    )
    .context("sign sync inventory")?;

    let endpoint = Endpoint::bind(presets::N0)
        .await
        .context("bind syncing Iroh endpoint")?;
    println!("transport_endpoint_id={}", endpoint.id());
    println!("device_id={}", device_state.identity().device_id());
    let connection = endpoint
        .connect(ticket.endpoint().clone(), ALPN)
        .await
        .context("connect to listening endpoint for sync")?;
    println!("peer_id={}", connection.remote_id());

    let (mut first_send, mut first_receive) = connection
        .open_bi()
        .await
        .context("open sync inventory stream")?;
    write_client_request(&mut first_send, &ClientRequest::SyncInventory(inventory)).await?;
    let diff = match read_server_response(&mut first_receive).await? {
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
    diff.verify_for_session(session_binding, expected_listener_device_id)
        .context("verify sync responder identity and session binding")?;
    ensure!(
        diff.conversation_id() == conversation_id,
        "sync diff belongs to a different conversation"
    );
    let advertised_ids: HashSet<_> = inventory_ids.iter().copied().collect();
    ensure!(
        diff.requested_event_ids()
            .iter()
            .all(|event_id| advertised_ids.contains(event_id)),
        "listener requested an event that was not in the signed inventory"
    );

    let received_event_ids = event_ids(diff.events())?;
    for event in diff.events() {
        event_store
            .put(event)
            .context("persist event received from sync diff")?;
    }
    let requested_event_ids = diff.requested_event_ids().to_vec();
    let events_for_listener = event_store
        .events_by_id(conversation_id, &requested_event_ids)
        .context("load events requested by listener")?;
    let sent_event_ids = event_ids(&events_for_listener)?;
    let more_available = diff.more_available();

    let (mut second_send, mut second_receive) = connection
        .open_bi()
        .await
        .context("open sync event batch stream")?;
    let batch = SyncEventBatch::new(conversation_id, events_for_listener)?;
    write_client_request(&mut second_send, &ClientRequest::SyncEvents(batch)).await?;
    let complete = match read_server_response(&mut second_receive).await? {
        ServerResponse::SyncComplete(complete) => complete,
        _ => bail!("sync client expected a sync completion response"),
    };
    ensure!(
        complete.conversation_id() == conversation_id,
        "sync completion belongs to a different conversation"
    );
    ensure!(
        same_event_ids(complete.stored_event_ids(), &sent_event_ids),
        "listener did not confirm the exact event batch sent by the client"
    );
    ensure!(
        complete.more_available() == more_available,
        "sync completion disagrees about continuation state"
    );

    println!("sync_inventory_events={}", inventory_ids.len());
    println!("sync_received_events={}", received_event_ids.len());
    println!("sync_sent_events={}", sent_event_ids.len());
    println!("sync_more_available={more_available}");
    println!("status=synchronized");

    connection.close(0_u32.into(), b"kilogram m0 sync complete");
    endpoint.close().await;
    Ok(())
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

async fn read_client_request(receive: &mut RecvStream) -> Result<ClientRequest> {
    let bytes = receive
        .read_to_end(MAX_WIRE_MESSAGE_BYTES)
        .await
        .context("read client protocol request")?;
    ClientRequest::decode(&bytes).context("decode and verify client protocol request")
}

async fn write_client_request(send: &mut SendStream, request: &ClientRequest) -> Result<()> {
    send.write_all(&request.encode()?)
        .await
        .context("send client protocol request")?;
    send.finish().context("finish client protocol request")?;
    Ok(())
}

async fn read_server_response(receive: &mut RecvStream) -> Result<ServerResponse> {
    let bytes = receive
        .read_to_end(MAX_WIRE_MESSAGE_BYTES)
        .await
        .context("read server protocol response")?;
    ServerResponse::decode(&bytes).context("decode and verify server protocol response")
}

async fn write_server_response(send: &mut SendStream, response: &ServerResponse) -> Result<()> {
    send.write_all(&response.encode()?)
        .await
        .context("send server protocol response")?;
    send.finish().context("finish server protocol response")?;
    Ok(())
}

fn event_ids(events: &[SignedEvent]) -> Result<Vec<EventId>> {
    events
        .iter()
        .map(|event| event.event_id().map_err(Into::into))
        .collect()
}

fn same_event_ids(left: &[EventId], right: &[EventId]) -> bool {
    left.len() == right.len()
        && left.iter().copied().collect::<HashSet<_>>()
            == right.iter().copied().collect::<HashSet<_>>()
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
        )?
        .encode()?;
        let decoded = ConnectionTicket::decode(&encoded)?;

        assert_eq!(decoded.content.version, TICKET_VERSION);
        assert_eq!(decoded.endpoint(), &endpoint);
        assert_eq!(decoded.listener_device_id(), listener_device_id);
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
}
