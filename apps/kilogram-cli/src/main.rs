use std::{path::PathBuf, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use clap::{Parser, Subcommand};
use iroh::{Endpoint, EndpointAddr, endpoint::presets};
use serde::{Deserialize, Serialize};
use tokio::time::timeout;

const ALPN: &[u8] = b"kilogram/m0/hello/1";
const MAX_MESSAGE_BYTES: usize = 64 * 1024;
const TICKET_VERSION: u8 = 1;

#[derive(Debug, Parser)]
#[command(
    name = "kilogram-cli",
    version,
    about = "Kilogram M0: exchange one message over an authenticated Iroh connection"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Listen for one connection, acknowledge one message, then exit.
    Listen {
        /// Also write the public connection ticket to this file.
        #[arg(long)]
        ticket_file: Option<PathBuf>,

        /// How long to wait for a public relay before accepting local connections.
        #[arg(long, default_value_t = 15)]
        relay_wait_seconds: u64,
    },

    /// Connect to a listener, send one message, print its acknowledgement, then exit.
    Connect {
        /// Connection ticket printed by the listener.
        #[arg(long, conflicts_with = "ticket_file")]
        ticket: Option<String>,

        /// Read the connection ticket from this file.
        #[arg(long, conflicts_with = "ticket")]
        ticket_file: Option<PathBuf>,

        /// UTF-8 message to send.
        #[arg(long, default_value = "hello from kilogram")]
        message: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct ConnectionTicket {
    version: u8,
    endpoint: EndpointAddr,
}

impl ConnectionTicket {
    fn new(endpoint: EndpointAddr) -> Self {
        Self {
            version: TICKET_VERSION,
            endpoint,
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
            ticket_file,
            relay_wait_seconds,
        } => listen(ticket_file, relay_wait_seconds).await,
        Command::Connect {
            ticket,
            ticket_file,
            message,
        } => connect(ticket, ticket_file, message).await,
    }
}

async fn listen(ticket_file: Option<PathBuf>, relay_wait_seconds: u64) -> Result<()> {
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

    let ticket = ConnectionTicket::new(endpoint.addr()).encode()?;
    println!("endpoint_id={}", endpoint.id());
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
        .read_to_end(MAX_MESSAGE_BYTES)
        .await
        .context("read message")?;
    let message = std::str::from_utf8(&request).context("message is not valid UTF-8")?;
    println!("received={message}");

    let acknowledgement = format!("ack:{message}");
    send.write_all(acknowledgement.as_bytes())
        .await
        .context("send acknowledgement")?;
    send.finish().context("finish acknowledgement stream")?;
    println!("status=acknowledged");

    let _ = timeout(Duration::from_secs(2), connection.closed()).await;
    endpoint.close().await;
    Ok(())
}

async fn connect(
    ticket: Option<String>,
    ticket_file: Option<PathBuf>,
    message: String,
) -> Result<()> {
    ensure!(
        message.len() <= MAX_MESSAGE_BYTES,
        "message is too large: {} bytes (maximum {MAX_MESSAGE_BYTES})",
        message.len()
    );

    let encoded_ticket = match (ticket, ticket_file) {
        (Some(ticket), None) => ticket,
        (None, Some(path)) => tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("read ticket from {}", path.display()))?,
        (None, None) => bail!("provide either --ticket or --ticket-file"),
        (Some(_), Some(_)) => bail!("--ticket and --ticket-file are mutually exclusive"),
    };
    let ticket = ConnectionTicket::decode(&encoded_ticket)?;

    let endpoint = Endpoint::bind(presets::N0)
        .await
        .context("bind connecting Iroh endpoint")?;
    println!("endpoint_id={}", endpoint.id());

    let connection = endpoint
        .connect(ticket.endpoint, ALPN)
        .await
        .context("connect to listening endpoint")?;
    println!("peer_id={}", connection.remote_id());

    let (mut send, mut receive) = connection
        .open_bi()
        .await
        .context("open bidirectional stream")?;
    send.write_all(message.as_bytes())
        .await
        .context("send message")?;
    send.finish().context("finish message stream")?;

    let response = receive
        .read_to_end(MAX_MESSAGE_BYTES)
        .await
        .context("read acknowledgement")?;
    let response = std::str::from_utf8(&response).context("response is not valid UTF-8")?;
    println!("response={response}");

    connection.close(0_u32.into(), b"kilogram m0 complete");
    endpoint.close().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroh::SecretKey;

    #[test]
    fn connection_ticket_round_trips() -> Result<()> {
        let endpoint = EndpointAddr::new(SecretKey::generate().public());
        let encoded = ConnectionTicket::new(endpoint.clone()).encode()?;
        let decoded = ConnectionTicket::decode(&encoded)?;

        assert_eq!(decoded.version, TICKET_VERSION);
        assert_eq!(decoded.endpoint, endpoint);
        Ok(())
    }

    #[test]
    fn connection_ticket_rejects_unknown_version() -> Result<()> {
        let ticket = ConnectionTicket {
            version: TICKET_VERSION + 1,
            endpoint: EndpointAddr::new(SecretKey::generate().public()),
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
