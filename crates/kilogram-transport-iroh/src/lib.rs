use std::time::Duration;

use anyhow::{Context, Result};
use iroh::endpoint::{Connection, RecvStream, SendStream};
use kilogram_protocol::{ClientRequest, ServerResponse};
use tokio::time::{Instant, sleep};

pub const ALPN: &[u8] = b"kilogram/m0/sync/1";
pub const MAX_WIRE_MESSAGE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectedPathKind {
    Direct,
    Relay,
    Custom,
}

impl SelectedPathKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Relay => "relay",
            Self::Custom => "custom",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedPathDiagnostics {
    pub kind: SelectedPathKind,
    pub remote_address: String,
    pub round_trip_time: Duration,
    pub open_paths: usize,
}

/// Waits briefly for relay-to-direct path migration and returns the final snapshot.
pub async fn selected_path_diagnostics(
    connection: &Connection,
    direct_wait: Duration,
) -> Option<SelectedPathDiagnostics> {
    let deadline = Instant::now() + direct_wait;
    loop {
        let diagnostics = selected_path_snapshot(connection);
        if diagnostics
            .as_ref()
            .is_some_and(|path| path.kind == SelectedPathKind::Direct)
            || Instant::now() >= deadline
        {
            return diagnostics;
        }
        sleep(Duration::from_millis(100)).await;
    }
}

fn selected_path_snapshot(connection: &Connection) -> Option<SelectedPathDiagnostics> {
    let paths = connection.paths();
    let open_paths = paths.len();
    let selected = paths.iter().find(|path| path.is_selected())?;
    let kind = if selected.is_ip() {
        SelectedPathKind::Direct
    } else if selected.is_relay() {
        SelectedPathKind::Relay
    } else {
        SelectedPathKind::Custom
    };
    Some(SelectedPathDiagnostics {
        kind,
        remote_address: selected.remote_addr().to_string(),
        round_trip_time: selected.rtt(),
        open_paths,
    })
}

pub async fn read_client_request(receive: &mut RecvStream) -> Result<ClientRequest> {
    let bytes = receive
        .read_to_end(MAX_WIRE_MESSAGE_BYTES)
        .await
        .context("read client protocol request")?;
    ClientRequest::decode(&bytes).context("decode and verify client protocol request")
}

pub async fn write_client_request(send: &mut SendStream, request: &ClientRequest) -> Result<()> {
    send.write_all(&request.encode()?)
        .await
        .context("send client protocol request")?;
    send.finish().context("finish client protocol request")?;
    Ok(())
}

pub async fn read_server_response(receive: &mut RecvStream) -> Result<ServerResponse> {
    let bytes = receive
        .read_to_end(MAX_WIRE_MESSAGE_BYTES)
        .await
        .context("read server protocol response")?;
    ServerResponse::decode(&bytes).context("decode and verify server protocol response")
}

pub async fn write_server_response(send: &mut SendStream, response: &ServerResponse) -> Result<()> {
    send.write_all(&response.encode()?)
        .await
        .context("send server protocol response")?;
    send.finish().context("finish server protocol response")?;
    Ok(())
}
