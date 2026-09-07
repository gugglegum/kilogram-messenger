use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use iroh::{
    Endpoint, EndpointAddr, RelayMode, RelayUrl,
    endpoint::{Builder, Connection, RecvStream, SendStream, presets},
};
use kilogram_mailbox::{
    MAX_MAILBOX_PEER_REQUEST_BYTES, MAX_MAILBOX_PEER_RESPONSE_BYTES, MailboxPeerRequest,
    MailboxPeerResponse, SignedMailboxStorageOffer,
};
use kilogram_protocol::{ClientRequest, ServerResponse};
use serde::{Deserialize, Serialize};
use tokio::time::{Instant, sleep, timeout};

pub const ALPN: &[u8] = b"kilogram/m0/sync/8";
pub const MAILBOX_ALPN: &[u8] = b"kilogram/m0/blind-mailbox/1";
pub const MAX_WIRE_MESSAGE_BYTES: usize = 8 * 1024 * 1024;
pub const WIRE_IO_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_MAILBOX_ENDPOINT_DESCRIPTOR_BYTES: usize = 16 * 1024;

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

/// Builds an endpoint and optionally restricts relay selection to one explicit
/// relay URL.
pub fn endpoint_builder_with_relay(policy: RoutePolicy, relay_url: Option<RelayUrl>) -> Builder {
    let builder = endpoint_builder(policy);
    match relay_url {
        Some(relay_url) => builder.relay_mode(RelayMode::custom([relay_url])),
        None => builder,
    }
}

/// Builds a dialing endpoint whose strict relay route is pinned to the remote
/// endpoint's advertised home relay.
///
/// Iroh normally manages relay selection dynamically. Pinning the strict
/// relay-only dialer to the signed ticket avoids selecting a different home
/// relay from the listener while no IP transports are available.
pub fn endpoint_builder_for_remote(policy: RoutePolicy, remote: &EndpointAddr) -> Result<Builder> {
    let builder = endpoint_builder(policy);
    if policy != RoutePolicy::RelayOnly {
        return Ok(builder);
    }

    let relay_urls = remote.relay_urls().cloned().collect::<Vec<_>>();
    ensure!(
        !relay_urls.is_empty(),
        "relay-only remote endpoint has no relay address"
    );
    Ok(builder.relay_mode(RelayMode::custom(relay_urls)))
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

pub fn encode_mailbox_provider_endpoint(endpoint: &EndpointAddr) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(endpoint).context("encode mailbox provider endpoint")?;
    ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_MAILBOX_ENDPOINT_DESCRIPTOR_BYTES,
        "mailbox provider endpoint descriptor size is invalid"
    );
    Ok(bytes)
}

pub fn mailbox_provider_endpoint_from_offer(
    offer: &SignedMailboxStorageOffer,
) -> Result<EndpointAddr> {
    let bytes = offer.provider_endpoint();
    ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_MAILBOX_ENDPOINT_DESCRIPTOR_BYTES,
        "mailbox provider endpoint descriptor size is invalid"
    );
    serde_json::from_slice(bytes).context("decode mailbox provider endpoint")
}

pub async fn read_mailbox_peer_request(receive: &mut RecvStream) -> Result<MailboxPeerRequest> {
    let bytes = timeout(
        WIRE_IO_TIMEOUT,
        receive.read_to_end(MAX_MAILBOX_PEER_REQUEST_BYTES),
    )
    .await
    .with_context(|| wire_timeout_message("read blind mailbox peer request"))?
    .context("read blind mailbox peer request")?;
    MailboxPeerRequest::decode_and_verify(&bytes)
        .context("decode and authenticate blind mailbox peer request")
}

pub async fn write_mailbox_peer_request(
    send: &mut SendStream,
    request: &MailboxPeerRequest,
) -> Result<()> {
    timeout(WIRE_IO_TIMEOUT, send.write_all(&request.encode()?))
        .await
        .with_context(|| wire_timeout_message("send blind mailbox peer request"))?
        .context("send blind mailbox peer request")?;
    send.finish().context("finish blind mailbox peer request")?;
    Ok(())
}

pub async fn read_mailbox_peer_response(
    receive: &mut RecvStream,
    request: &MailboxPeerRequest,
) -> Result<MailboxPeerResponse> {
    let bytes = timeout(
        WIRE_IO_TIMEOUT,
        receive.read_to_end(MAX_MAILBOX_PEER_RESPONSE_BYTES),
    )
    .await
    .with_context(|| wire_timeout_message("read blind mailbox peer response"))?
    .context("read blind mailbox peer response")?;
    MailboxPeerResponse::decode_and_verify(&bytes, request)
        .context("decode and bind blind mailbox peer response")
}

pub async fn write_mailbox_peer_response(
    send: &mut SendStream,
    request: &MailboxPeerRequest,
    response: &MailboxPeerResponse,
) -> Result<()> {
    timeout(WIRE_IO_TIMEOUT, send.write_all(&response.encode(request)?))
        .await
        .with_context(|| wire_timeout_message("send blind mailbox peer response"))?
        .context("send blind mailbox peer response")?;
    send.finish()
        .context("finish blind mailbox peer response")?;
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
    use iroh::SecretKey;
    use kilogram_mailbox::MailboxStoreIdentity;

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

    #[test]
    fn relay_only_dialer_requires_a_remote_relay() -> Result<()> {
        let endpoint_id = SecretKey::generate().public();
        let remote_without_relay = EndpointAddr::new(endpoint_id);
        let remote_with_relay = EndpointAddr::new(endpoint_id)
            .with_relay_url("https://euc1-1.relay.n0.iroh.link./".parse()?);

        assert!(endpoint_builder_for_remote(RoutePolicy::Auto, &remote_without_relay).is_ok());
        assert!(
            endpoint_builder_for_remote(RoutePolicy::RelayOnly, &remote_without_relay).is_err()
        );
        assert!(endpoint_builder_for_remote(RoutePolicy::RelayOnly, &remote_with_relay).is_ok());
        Ok(())
    }

    #[test]
    fn signed_mailbox_offer_carries_a_bounded_iroh_endpoint() -> Result<()> {
        let endpoint_id = SecretKey::generate().public();
        let endpoint = EndpointAddr::new(endpoint_id)
            .with_relay_url("https://euc1-1.relay.n0.iroh.link./".parse()?);
        let identity = MailboxStoreIdentity::from_secret_bytes([9_u8; 32]);
        let offer = identity.storage_offer(
            encode_mailbox_provider_endpoint(&endpoint)?,
            200 * 1024 * 1024,
            1024 * 1024,
            1_000,
            300,
        )?;
        offer.verify_at(1_001)?;
        assert_eq!(mailbox_provider_endpoint_from_offer(&offer)?, endpoint);
        Ok(())
    }
}
