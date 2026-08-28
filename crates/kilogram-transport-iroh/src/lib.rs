use std::time::Duration;

use anyhow::{Context, Result, bail};
use iroh::{
    Endpoint,
    endpoint::{Builder, Connection, RecvStream, SendStream, presets},
};
use kilogram_protocol::{ClientRequest, ServerResponse};
use serde::{Deserialize, Serialize};
use tokio::time::{Instant, sleep, timeout};

pub const ALPN: &[u8] = b"kilogram/m0/sync/1";
pub const MAX_WIRE_MESSAGE_BYTES: usize = 8 * 1024 * 1024;
pub const WIRE_IO_TIMEOUT: Duration = Duration::from_secs(15);

/// Controls which transport may carry Kilogram application protocol frames.
///
/// `DirectOnly` deliberately keeps Iroh's relay transport available for connection
/// establishment and NAT traversal. The application must call [`await_route_policy`]
/// before opening or accepting its first protocol stream, so no Kilogram frame is sent
/// until a direct IP path is selected.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoutePolicy {
    Auto,
    DirectOnly,
    RelayOnly,
}

impl RoutePolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::DirectOnly => "direct-only",
            Self::RelayOnly => "relay-only",
        }
    }

    pub fn accepts(self, path: SelectedPathKind) -> bool {
        match self {
            Self::Auto => true,
            Self::DirectOnly => path == SelectedPathKind::Direct,
            Self::RelayOnly => path == SelectedPathKind::Relay,
        }
    }
}

/// Builds an Iroh endpoint with the transports required by `policy`.
pub fn endpoint_builder(policy: RoutePolicy) -> Builder {
    let builder = Endpoint::builder(presets::N0);
    match policy {
        // Relay remains available to coordinate hole punching. Application traffic is
        // gated by `await_route_policy` until a direct path exists.
        RoutePolicy::Auto | RoutePolicy::DirectOnly => builder,
        // Removing all IP transports makes relay-only strict at the Iroh layer.
        RoutePolicy::RelayOnly => builder.clear_ip_transports(),
    }
}

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

/// Waits until the selected path satisfies the policy or returns a bounded diagnostic error.
pub async fn await_route_policy(
    connection: &Connection,
    policy: RoutePolicy,
    wait: Duration,
) -> Result<SelectedPathDiagnostics> {
    let deadline = Instant::now() + wait;
    let mut last_path = None;
    loop {
        if let Some(path) = selected_path_snapshot(connection) {
            if policy.accepts(path.kind) {
                return Ok(path);
            }
            last_path = Some(path.kind);
        }
        if Instant::now() >= deadline {
            let selected = last_path.map_or("unknown", SelectedPathKind::as_str);
            bail!(
                "route policy {} was not satisfied within {:.1}s; selected path={selected}",
                policy.as_str(),
                wait.as_secs_f64()
            );
        }
        sleep(Duration::from_millis(100)).await;
    }
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
    let bytes = timeout(WIRE_IO_TIMEOUT, receive.read_to_end(MAX_WIRE_MESSAGE_BYTES))
        .await
        .with_context(|| wire_timeout_message("read client protocol request"))?
        .context("read client protocol request")?;
    ClientRequest::decode(&bytes).context("decode and verify client protocol request")
}

pub async fn write_client_request(send: &mut SendStream, request: &ClientRequest) -> Result<()> {
    timeout(WIRE_IO_TIMEOUT, send.write_all(&request.encode()?))
        .await
        .with_context(|| wire_timeout_message("send client protocol request"))?
        .context("send client protocol request")?;
    send.finish().context("finish client protocol request")?;
    Ok(())
}

pub async fn read_server_response(receive: &mut RecvStream) -> Result<ServerResponse> {
    let bytes = timeout(WIRE_IO_TIMEOUT, receive.read_to_end(MAX_WIRE_MESSAGE_BYTES))
        .await
        .with_context(|| wire_timeout_message("read server protocol response"))?
        .context("read server protocol response")?;
    ServerResponse::decode(&bytes).context("decode and verify server protocol response")
}

pub async fn write_server_response(send: &mut SendStream, response: &ServerResponse) -> Result<()> {
    timeout(WIRE_IO_TIMEOUT, send.write_all(&response.encode()?))
        .await
        .with_context(|| wire_timeout_message("send server protocol response"))?
        .context("send server protocol response")?;
    send.finish().context("finish server protocol response")?;
    Ok(())
}

fn wire_timeout_message(operation: &str) -> String {
    format!(
        "{operation} timed out after {:.1}s",
        WIRE_IO_TIMEOUT.as_secs_f64()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_route_policies_accept_only_the_requested_path() {
        assert!(RoutePolicy::Auto.accepts(SelectedPathKind::Direct));
        assert!(RoutePolicy::Auto.accepts(SelectedPathKind::Relay));
        assert!(RoutePolicy::DirectOnly.accepts(SelectedPathKind::Direct));
        assert!(!RoutePolicy::DirectOnly.accepts(SelectedPathKind::Relay));
        assert!(RoutePolicy::RelayOnly.accepts(SelectedPathKind::Relay));
        assert!(!RoutePolicy::RelayOnly.accepts(SelectedPathKind::Direct));
    }

    #[test]
    fn route_policy_uses_stable_ticket_names() -> Result<()> {
        assert_eq!(serde_json::to_string(&RoutePolicy::Auto)?, "\"auto\"");
        assert_eq!(
            serde_json::to_string(&RoutePolicy::DirectOnly)?,
            "\"direct-only\""
        );
        assert_eq!(
            serde_json::to_string(&RoutePolicy::RelayOnly)?,
            "\"relay-only\""
        );
        Ok(())
    }
}
