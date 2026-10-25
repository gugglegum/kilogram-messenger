//! Production OD2: explicit Minimal transports and real exporter-bound frames.
use crate::RoutePolicy;
use anyhow::{Context, Result, ensure};
use iroh::{
    Endpoint, EndpointAddr, EndpointId, RelayMode, RelayUrl,
    endpoint::{Connection, RecvStream, SendStream, presets},
};
use kilogram_protocol::ordered_dialog_v2::{self as wire, Id, ParsedFrame2, ParsedRuntimeLocator2};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, time::Duration};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteConfig2 {
    pub policy: RoutePolicy,
    pub bind: Option<SocketAddr>,
    pub relay: Option<String>,
}
impl RouteConfig2 {
    pub fn effective(&self) -> Result<RoutePolicy> {
        if let Some(url) = &self.relay {
            wire::canonical_runtime_relay(url)?;
        }
        if self.policy != RoutePolicy::RelayOnly {
            let bind = self.bind.context("explicit IP bind required")?;
            ensure!(
                bind.ip().is_loopback(),
                "S4 IP transports require an explicit loopback bind"
            );
            if let SocketAddr::V6(addr) = bind {
                ensure!(
                    addr.scope_id() == 0 && addr.flowinfo() == 0,
                    "unsupported scoped locator address"
                );
            }
        }
        match (self.policy, self.relay.is_some()) {
            (RoutePolicy::DirectOnly, true) => {
                anyhow::bail!("DirectOnly forbids relay configuration")
            }
            (RoutePolicy::RelayOnly, false) => {
                anyhow::bail!("RelayOnly requires an explicit relay")
            }
            (RoutePolicy::RelayOnly, true) => {
                ensure!(self.bind.is_none(), "RelayOnly forbids IP bind");
                Ok(RoutePolicy::RelayOnly)
            }
            (RoutePolicy::Auto, false) => Ok(RoutePolicy::DirectOnly),
            (policy, _) => Ok(policy),
        }
    }
    pub fn remote(&self, locator: &ParsedRuntimeLocator2) -> Result<EndpointAddr> {
        let policy = self.connection_policy(locator)?;
        let f = locator.fields();
        ensure!(
            policy != RoutePolicy::RelayOnly || f.relay.is_some(),
            "relay locator required"
        );
        ensure!(
            policy != RoutePolicy::DirectOnly || !f.ips.is_empty(),
            "direct locator required"
        );
        let mut addr = EndpointAddr::new(EndpointId::from_bytes(&f.endpoint)?);
        if policy != RoutePolicy::RelayOnly {
            for ip in &f.ips {
                addr = addr.with_ip_addr(*ip);
            }
        }
        if policy != RoutePolicy::DirectOnly
            && let Some(relay) = &f.relay
        {
            addr = addr.with_relay_url(relay.parse::<RelayUrl>()?);
        }
        Ok(addr)
    }
    pub fn connection_policy(&self, locator: &ParsedRuntimeLocator2) -> Result<RoutePolicy> {
        let policy = self.effective()?;
        let f = locator.fields();
        if policy == RoutePolicy::DirectOnly {
            ensure!(!f.ips.is_empty(), "direct locator required");
            return Ok(policy);
        }
        match &f.relay {
            Some(url) => {
                ensure!(Some(url) == self.relay.as_ref(), "RelayNotPermitted");
                Ok(policy)
            }
            None => {
                ensure!(
                    policy != RoutePolicy::RelayOnly && !f.ips.is_empty(),
                    "required peer relay missing"
                );
                Ok(RoutePolicy::DirectOnly)
            }
        }
    }
}
pub async fn bind(config: &RouteConfig2) -> Result<Endpoint> {
    let policy = config.effective()?;
    let mut builder = Endpoint::builder(presets::Minimal)
        .clear_address_lookup()
        .clear_ip_transports()
        .clear_relay_transports()
        .transport_config(
            iroh::endpoint::QuicTransportConfig::builder()
                .max_concurrent_bidi_streams(4_u32.into())
                .max_concurrent_uni_streams(0_u32.into())
                .receive_window((4_u32 * 1024 * 1024).into())
                .build(),
        )
        .alpns(vec![wire::ALPN.to_vec()]);
    if policy != RoutePolicy::RelayOnly {
        builder = builder.bind_addr(config.bind.context("explicit IP bind")?)?;
    }
    if let Some(url) = &config.relay {
        builder = builder.relay_mode(RelayMode::custom([url.parse::<RelayUrl>()?]));
    }
    let endpoint = builder.bind().await?;
    if config.relay.is_some() {
        if let Err(error) = tokio::time::timeout(Duration::from_secs(15), endpoint.online()).await {
            endpoint.close().await;
            return Err(error).context("explicit relay registration timeout");
        }
        let addr = endpoint.addr();
        let urls = addr.relay_urls().collect::<Vec<_>>();
        ensure!(
            urls.len() == 1 && Some(urls[0].as_str()) == config.relay.as_deref(),
            "unexpected relay registration"
        );
    }
    Ok(endpoint)
}

pub fn binding(connection: &Connection, local: Id, dialer: bool) -> Result<Id> {
    ensure!(connection.alpn() == wire::ALPN, "unsupported OD2 ALPN");
    let remote = *connection.remote_id().as_bytes();
    let (a, b) = if dialer {
        (local, remote)
    } else {
        (remote, local)
    };
    let mut material = [0; 32];
    connection
        .export_keying_material(
            &mut material,
            wire::EXPORTER_LABEL,
            &wire::exporter_context(a, b)?,
        )
        .map_err(|_| anyhow::anyhow!("OD2 TLS exporter"))?;
    let result = wire::exporter_hash(&material);
    material.fill(0);
    Ok(result)
}
pub async fn write(send: &mut SendStream, frame: &ParsedFrame2) -> Result<()> {
    ensure!(frame.raw().len() <= wire::MAX_WIRE - 4, "OD2 frame cap");
    send.write_all(&(frame.raw().len() as u32).to_be_bytes())
        .await?;
    send.write_all(frame.raw()).await?;
    send.finish()?;
    Ok(())
}
pub async fn read(receive: &mut RecvStream) -> Result<ParsedFrame2> {
    let mut length = [0; 4];
    receive.read_exact(&mut length).await?;
    let n = u32::from_be_bytes(length) as usize;
    ensure!((1..=wire::MAX_WIRE - 4).contains(&n), "OD2 frame length");
    let mut raw = vec![0; n];
    receive.read_exact(&mut raw).await?;
    let mut trailing = [0; 1];
    ensure!(
        receive.read(&mut trailing).await?.is_none(),
        "OD2 trailing bytes"
    );
    Ok(ParsedFrame2::parse(&raw)?)
}

#[cfg(test)]
mod route_tests {
    use super::*;

    #[test]
    fn a3_s4_f12_auto_explicit_relay_loopback_bind_table() -> Result<()> {
        let relay = Some("http://127.0.0.1:21002/".to_owned());
        for bind in ["127.0.0.1:0", "[::1]:0"] {
            let auto = RouteConfig2 {
                policy: RoutePolicy::Auto,
                bind: Some(bind.parse()?),
                relay: relay.clone(),
            };
            ensure!(
                auto.effective()? == RoutePolicy::Auto,
                "Auto relay loopback"
            );
            let direct = RouteConfig2 {
                policy: RoutePolicy::DirectOnly,
                relay: None,
                ..auto.clone()
            };
            ensure!(
                direct.effective()? == RoutePolicy::DirectOnly,
                "Direct loopback"
            );
            ensure!(
                RouteConfig2 {
                    relay: None,
                    ..auto.clone()
                }
                .effective()?
                    == RoutePolicy::DirectOnly,
                "Auto None direct"
            );
            ensure!(
                RouteConfig2 {
                    relay: relay.clone(),
                    ..direct
                }
                .effective()
                .is_err(),
                "DirectOnly relay accepted"
            );
            ensure!(
                RouteConfig2 {
                    policy: RoutePolicy::RelayOnly,
                    ..auto
                }
                .effective()
                .is_err(),
                "RelayOnly IP accepted"
            );
        }
        for bind in [
            "0.0.0.0:0",
            "[::]:0",
            "192.168.1.9:0",
            "[2001:db8::1]:0",
            "[fe80::1]:0",
        ] {
            for policy in [RoutePolicy::DirectOnly, RoutePolicy::Auto] {
                for url in [None, relay.clone()] {
                    ensure!(
                        RouteConfig2 {
                            policy,
                            bind: Some(bind.parse()?),
                            relay: url
                        }
                        .effective()
                        .is_err(),
                        "non-loopback accepted before bind: {policy:?}/{bind}"
                    );
                }
            }
        }
        ensure!(
            RouteConfig2 {
                policy: RoutePolicy::RelayOnly,
                bind: None,
                relay: relay.clone()
            }
            .effective()?
                == RoutePolicy::RelayOnly,
            "URL-only RelayOnly"
        );
        for policy in [
            RoutePolicy::DirectOnly,
            RoutePolicy::Auto,
            RoutePolicy::RelayOnly,
        ] {
            ensure!(
                RouteConfig2 {
                    policy,
                    bind: None,
                    relay: None
                }
                .effective()
                .is_err(),
                "empty/empty accepted"
            );
        }
        println!(
            "S4_F12_R1_PASS Auto_explicit_relay_loopback_IPv4_IPv6=true wildcard_LAN_global_linklocal_rejected_before_bind=true prior_route_table_preserved=true"
        );
        Ok(())
    }
}
