//! Production OD2: explicit Minimal transports and real exporter-bound frames.
use crate::RoutePolicy;
use anyhow::{Context, Result, ensure};
use iroh::{
    Endpoint, EndpointAddr, EndpointId, RelayMode, RelayUrl, TransportAddr,
    endpoint::{
        Connection, LocalTransportAddr, NetReportConfig, PortmapperConfig, RecvStream, SendStream,
        presets,
    },
};
use kilogram_protocol::ordered_dialog_v2::{
    self as wire, DirectLan2, Id, ParsedFrame2, ParsedRuntimeLocator2,
};
use serde::{Deserialize, Serialize};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteConfig2 {
    pub policy: RoutePolicy,
    pub bind: Option<SocketAddr>,
    pub relay: Option<String>,
}

/// Only fresh runtime2 can opt into a LAN bind. The shared/indexed route
/// continues to deserialize and validate its original closed configuration.
#[derive(Clone, Debug)]
pub struct FreshRoute2 {
    base: RouteConfig2,
    lan: Option<DirectLan2>,
}
impl From<RouteConfig2> for FreshRoute2 {
    fn from(base: RouteConfig2) -> Self {
        Self { base, lan: None }
    }
}
impl FreshRoute2 {
    pub fn new(base: RouteConfig2, peer: Option<Ipv4Addr>) -> Result<Self> {
        let Some(peer) = peer else {
            return Ok(base.into());
        };
        let Some(SocketAddr::V4(bind)) = base.bind else {
            anyhow::bail!("LAN requires a concrete IPv4 bind");
        };
        ensure!(
            base.policy == RoutePolicy::DirectOnly && base.relay.is_none() && bind.port() == 0,
            "LAN requires direct-only, an OS-assigned port and no relay"
        );
        Ok(Self {
            base,
            lan: Some(DirectLan2::new(*bind.ip(), peer)?),
        })
    }
    pub fn base(&self) -> &RouteConfig2 {
        &self.base
    }
    pub fn lan(&self) -> Option<DirectLan2> {
        self.lan
    }
    pub fn effective(&self) -> Result<RoutePolicy> {
        if self.lan.is_some() {
            Ok(RoutePolicy::DirectOnly)
        } else {
            self.base.effective()
        }
    }
    pub fn remote(&self, locator: &ParsedRuntimeLocator2) -> Result<EndpointAddr> {
        let Some(lan) = self.lan else {
            return self.base.remote(locator);
        };
        let socket = locator.direct_lan_socket(lan.peer())?;
        Ok(
            EndpointAddr::new(EndpointId::from_bytes(&locator.fields().endpoint)?)
                .with_ip_addr(socket),
        )
    }
    pub fn connection_policy(&self, locator: &ParsedRuntimeLocator2) -> Result<RoutePolicy> {
        let Some(lan) = self.lan else {
            return self.base.connection_policy(locator);
        };
        locator.direct_lan_socket(lan.peer())?;
        Ok(RoutePolicy::DirectOnly)
    }
    pub fn advertisement(&self, endpoint: &Endpoint) -> Result<EndpointAddr> {
        let address = endpoint.addr();
        if let Some(lan) = self.lan {
            let sockets = endpoint.bound_sockets();
            let [bound] = sockets.as_slice() else {
                anyhow::bail!("LAN requires exactly one bound peer UDP socket");
            };
            ensure!(
                bound.ip() == IpAddr::V4(lan.local())
                    && bound.port() != 0
                    && address.ip_addrs().eq(std::iter::once(bound))
                    && address.relay_urls().next().is_none(),
                "LAN bound UDP socket and advertised endpoint disagree"
            );
        }
        Ok(address)
    }
    pub fn selected_lan_peer(
        &self,
        connection: &Connection,
        locator: &ParsedRuntimeLocator2,
    ) -> Result<Option<SocketAddr>> {
        let Some(lan) = self.lan else {
            return Ok(None);
        };
        let socket = locator.direct_lan_socket(lan.peer())?;
        let paths = connection.paths();
        let path = paths
            .iter()
            .find(|path| path.is_selected())
            .context("no selected LAN path")?;
        ensure!(
            path.remote_addr() == &TransportAddr::Ip(socket)
                && matches!(path.local_addr(), LocalTransportAddr::Ip(local)
                    if local.is_none_or(|ip| ip == IpAddr::V4(lan.local()))),
            "selected LAN path differs from signed socket or bound interface"
        );
        Ok(Some(socket))
    }
    pub fn check_lan_path(
        &self,
        connection: &Connection,
        own: &ParsedRuntimeLocator2,
        peer: &ParsedRuntimeLocator2,
        endpoint: Option<&Endpoint>,
    ) -> Result<Option<SocketAddr>> {
        let Some(lan) = self.lan else {
            return Ok(None);
        };
        let selected = self.selected_lan_peer(connection, peer)?;
        let actual =
            self.advertisement(endpoint.context("LAN interface not currently verified")?)?;
        let own_socket = own.direct_lan_socket(lan.local())?;
        ensure!(
            actual.id.as_bytes() == &own.fields().endpoint
                && actual.ip_addrs().eq(std::iter::once(&own_socket))
                && connection.remote_id().as_bytes() == &peer.fields().endpoint,
            "LAN endpoint or local socket no longer matches signed locators"
        );
        Ok(selected)
    }
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
    bind_alpns(config, config.effective()?, false, false).await
}
/// Existing dialogs use negotiated ALPN to select their closed wire decoder.
/// The fresh S4 entry point keeps its original single-ALPN behavior.
pub async fn bind_existing(config: &RouteConfig2) -> Result<Endpoint> {
    bind_alpns(config, config.effective()?, true, false).await
}
pub async fn bind_fresh(config: &FreshRoute2) -> Result<Endpoint> {
    let endpoint = bind_alpns(
        config.base(),
        config.effective()?,
        false,
        config.lan().is_some(),
    )
    .await?;
    if let Err(error) = config.advertisement(&endpoint) {
        endpoint.close().await;
        return Err(error);
    }
    Ok(endpoint)
}
async fn bind_alpns(
    config: &RouteConfig2,
    policy: RoutePolicy,
    existing: bool,
    lan: bool,
) -> Result<Endpoint> {
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
        .alpns(if existing {
            vec![
                wire::ALPN.to_vec(),
                kilogram_protocol::ordered_dialog_v1::ALPN.to_vec(),
            ]
        } else {
            vec![wire::ALPN.to_vec()]
        });
    if lan {
        builder = builder
            .portmapper_config(PortmapperConfig::Disabled)
            .net_report_config(NetReportConfig::minimal());
    }
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
