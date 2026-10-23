use std::{net::Ipv4Addr, time::Duration};

use anyhow::{Context, Result, ensure};
use iroh::{Endpoint, RelayMode, endpoint::presets};
use iroh_relay::server::{RelayConfig, Server, ServerConfig};
use kilogram_protocol::{ClientRequest, ServerResponse};
use kilogram_transport_iroh::{
    ALPN, RoutePolicy, SelectedPathKind, await_route_policy, endpoint_builder_for_remote,
    read_client_request, read_server_response, write_client_request, write_server_response,
};

#[tokio::test]
async fn real_local_relay_preserves_adapter_frames_route_policies_and_reconnect() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(15), async {
        let mut config = ServerConfig::default();
        config.relay = Some(RelayConfig::new((Ipv4Addr::LOCALHOST, 0)));
        let relay = Server::spawn(config).await?;
        let relay_addr = relay.http_addr().context("loopback relay listener")?;
        ensure!(relay_addr.ip().is_loopback());
        let url: iroh::RelayUrl = format!("http://{relay_addr}/").parse()?;
        let receiver = Endpoint::builder(presets::Minimal)
            .clear_ip_transports()
            .relay_mode(RelayMode::custom([url]))
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await?;
        receiver.online().await;
        // Exercise the actual production RelayOnly builder, removing external
        // address lookup services only in this local test fixture.
        let sender = endpoint_builder_for_remote(RoutePolicy::RelayOnly, &receiver.addr())?
            .clear_address_lookup()
            .bind()
            .await?;
        sender.online().await;
        let mut previous_exporter = None;
        for round in 0..3_u8 {
        let (left, right) = tokio::try_join!(
            async { Ok::<_, anyhow::Error>(sender.connect(receiver.addr(), ALPN).await?) },
            async {
                Ok::<_, anyhow::Error>(receiver.accept().await.context("incoming relay")?.await?)
            },
        )?;
        ensure!(left.remote_id() == receiver.id() && right.remote_id() == sender.id());
        let mut local_exporter = [0; 32];
        let mut remote_exporter = [0; 32];
        left.export_keying_material(&mut local_exporter, b"kilogram local relay", b"functional")
            .map_err(|_| anyhow::anyhow!("local relay exporter"))?;
        right.export_keying_material(&mut remote_exporter, b"kilogram local relay", b"functional")
            .map_err(|_| anyhow::anyhow!("remote relay exporter"))?;
        ensure!(local_exporter == remote_exporter);
        ensure!(previous_exporter != Some(local_exporter), "reconnect reused a TLS exporter");
        previous_exporter = Some(local_exporter);
        for connection in [&left, &right] {
            for policy in [RoutePolicy::Auto, RoutePolicy::RelayOnly] {
                ensure!(
                    await_route_policy(connection, policy, Duration::from_secs(2))
                        .await?
                        .kind
                        == SelectedPathKind::Relay
                );
            }
            ensure!(
                await_route_policy(
                    connection,
                    RoutePolicy::DirectOnly,
                    Duration::from_millis(100)
                )
                .await
                .is_err()
            );
        }
        let frame = ClientRequest::EndpointAnnouncementPush(vec![round; 8_123]);
        let response = ServerResponse::EndpointAnnouncementRejected;
        tokio::try_join!(
            async {
                let (mut send, mut receive) = left.open_bi().await?;
                write_client_request(&mut send, &frame).await?;
                ensure!(read_server_response(&mut receive).await? == response);
                Ok::<_, anyhow::Error>(())
            },
            async {
                let (mut send, mut receive) = right.accept_bi().await?;
                ensure!(read_client_request(&mut receive).await? == frame);
                write_server_response(&mut send, &response).await?;
                Ok::<_, anyhow::Error>(())
            },
        )?;
        println!("relay_round={round} exact_adapter_payload=8123 selected=relay peer_identity=verified exporter=symmetric,fresh");
        left.close(0_u32.into(), b"functional reconnect");
        right.close(0_u32.into(), b"functional reconnect");
        }
        sender.close().await;
        receiver.close().await;
        relay.shutdown().await?;
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("local adapter relay timeout")??;
    Ok(())
}
