use std::{path::PathBuf, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use clap::{Parser, Subcommand};
use iroh::{Endpoint, EndpointAddr, endpoint::presets};
use kilogram_identity::{DeviceId, DeviceState};
use kilogram_protocol::{ConversationId, EventPayload, SignedEvent};
use kilogram_store::EventStore;
use serde::{Deserialize, Serialize};
use tokio::time::timeout;

const ALPN: &[u8] = b"kilogram/m0/signed-event/1";
const EVENT_STORE_DIRECTORY: &str = "events";
const MAX_WIRE_EVENT_BYTES: usize = 128 * 1024;
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

    /// Verify and print locally stored events without connecting to a peer.
    History {
        /// Directory containing this application's development state.
        #[arg(long)]
        state_dir: PathBuf,

        /// Development-only shared label used to derive a conversation ID.
        #[arg(long, default_value = "m0-local-smoke")]
        conversation: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct ConnectionTicket {
    version: u8,
    endpoint: EndpointAddr,
    listener_device_id: DeviceId,
}

impl ConnectionTicket {
    fn new(endpoint: EndpointAddr, listener_device_id: DeviceId) -> Self {
        Self {
            version: TICKET_VERSION,
            endpoint,
            listener_device_id,
        }
    }

    fn encode(&self) -> Result<String> {
        let json = serde_json::to_vec(self).context("serialize connection ticket")?;
        Ok(URL_SAFE_NO_PAD.encode(json))
    }

    fn decode(encoded: &str) -> Result<Self> {
        let bytes = URL_SAFE_NO_PAD
            .decode(encoded.trim())
            .context("decode connection ticket as base64url")?;
        let ticket: Self =
            serde_json::from_slice(&bytes).context("decode connection ticket payload")?;
        ensure!(
            ticket.version == TICKET_VERSION,
            "unsupported connection ticket version: {}",
            ticket.version
        );
        Ok(ticket)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Listen {
            state_dir,
            ticket_file,
            relay_wait_seconds,
        } => listen(state_dir, ticket_file, relay_wait_seconds).await,
        Command::Connect {
            state_dir,
            ticket,
            ticket_file,
            message,
            conversation,
        } => connect(state_dir, ticket, ticket_file, message, conversation).await,
        Command::History {
            state_dir,
            conversation,
        } => show_history(state_dir, conversation),
    }
}

async fn listen(
    state_dir: PathBuf,
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

    let ticket =
        ConnectionTicket::new(endpoint.addr(), device_state.identity().device_id()).encode()?;
    println!("transport_endpoint_id={}", endpoint.id());
    println!("device_id={}", device_state.identity().device_id());
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
    let request = receive
        .read_to_end(MAX_WIRE_EVENT_BYTES)
        .await
        .context("read signed event")?;
    let event = SignedEvent::decode_and_verify(&request).context("verify received event")?;
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
    send.write_all(&acknowledgement.encode()?)
        .await
        .context("send signed acknowledgement")?;
    send.finish().context("finish acknowledgement stream")?;
    println!("acknowledgement_event_id={acknowledgement_id}");
    println!("acknowledgement_store={acknowledgement_store_outcome:?}");
    println!("status=acknowledged");

    let _ = timeout(Duration::from_secs(2), connection.closed()).await;
    endpoint.close().await;
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

    let encoded_ticket = match (ticket, ticket_file) {
        (Some(ticket), None) => ticket,
        (None, Some(path)) => tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("read ticket from {}", path.display()))?,
        (None, None) => bail!("provide either --ticket or --ticket-file"),
        (Some(_), Some(_)) => bail!("--ticket and --ticket-file are mutually exclusive"),
    };
    let ticket = ConnectionTicket::decode(&encoded_ticket)?;
    let expected_listener_device_id = ticket.listener_device_id;

    let endpoint = Endpoint::bind(presets::N0)
        .await
        .context("bind connecting Iroh endpoint")?;
    println!("transport_endpoint_id={}", endpoint.id());
    println!("device_id={}", device_state.identity().device_id());

    let connection = endpoint
        .connect(ticket.endpoint, ALPN)
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
    send.write_all(&event.encode()?)
        .await
        .context("send signed event")?;
    send.finish().context("finish signed event stream")?;
    println!("sent_event_id={event_id}");
    println!("sent_author_sequence={author_sequence}");
    println!("sent_store={sent_store_outcome:?}");

    let response = receive
        .read_to_end(MAX_WIRE_EVENT_BYTES)
        .await
        .context("read signed acknowledgement")?;
    let acknowledgement =
        SignedEvent::decode_and_verify(&response).context("verify signed acknowledgement")?;
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

fn open_event_store(state_dir: &std::path::Path) -> Result<EventStore> {
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
        let listener_device_id = DeviceIdentity::generate()?.device_id();
        let encoded = ConnectionTicket::new(endpoint.clone(), listener_device_id).encode()?;
        let decoded = ConnectionTicket::decode(&encoded)?;

        assert_eq!(decoded.version, TICKET_VERSION);
        assert_eq!(decoded.endpoint, endpoint);
        assert_eq!(decoded.listener_device_id, listener_device_id);
        Ok(())
    }

    #[test]
    fn connection_ticket_rejects_unknown_version() -> Result<()> {
        let ticket = ConnectionTicket {
            version: TICKET_VERSION + 1,
            endpoint: EndpointAddr::new(SecretKey::generate().public()),
            listener_device_id: DeviceIdentity::generate()?.device_id(),
        };

        let error = ConnectionTicket::decode(&ticket.encode()?)
            .err()
            .context("unknown ticket version unexpectedly succeeded")?;

        assert!(
            error
                .to_string()
                .contains("unsupported connection ticket version")
        );
        Ok(())
    }
}
