//! Explicit ten-field production locator. The S3 nine-field descriptor is separate.
use super::*;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

/// The two IPv4 addresses explicitly chosen for a fresh direct LAN profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DirectLan2 {
    local: Ipv4Addr,
    peer: Ipv4Addr,
}

impl DirectLan2 {
    pub fn new(local: Ipv4Addr, peer: Ipv4Addr) -> Result<Self> {
        require(
            local.is_private() && peer.is_private() && local != peer,
            "LAN requires two distinct private IPv4 addresses",
        )?;
        Ok(Self { local, peer })
    }
    pub fn local(self) -> Ipv4Addr {
        self.local
    }
    pub fn peer(self) -> Ipv4Addr {
        self.peer
    }
    /// Uses the prefix of the interface that actually owns the local address.
    pub fn check_on_link(self, prefix: u8) -> Result<()> {
        require(
            (1..=30).contains(&prefix),
            "unsupported LAN interface prefix",
        )?;
        let mask = u32::MAX << (32 - prefix);
        let local = u32::from(self.local);
        let peer = u32::from(self.peer);
        let network = local & mask;
        let broadcast = network | !mask;
        require(
            peer & mask == network
                && [local, peer]
                    .iter()
                    .all(|ip| *ip != network && *ip != broadcast),
            "LAN requires on-link unicast addresses",
        )
    }
}

#[derive(Clone)]
pub struct RuntimeLocatorFields2 {
    pub c: Id,
    pub cert: Id,
    pub peer_cert: Id,
    pub endpoint: Id,
    pub ips: Vec<SocketAddr>,
    pub relay: Option<String>,
    pub revision: u64,
    pub not_before: u64,
    pub expires: u64,
}
pub fn canonical_runtime_relay(raw: &str) -> Result<()> {
    require(raw.len() <= 2048, "relay URL length")?;
    let url = url::Url::parse(raw).map_err(|_| Error::Invalid("relay URL"))?;
    let loopback = url
        .host_str()
        .and_then(|h| h.trim_matches(['[', ']']).parse::<IpAddr>().ok())
        .is_some_and(|ip| ip.is_loopback());
    require(
        url.as_str() == raw
            && url.host().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
            && (url.scheme() == "https" || (url.scheme() == "http" && loopback)),
        "canonical relay URL",
    )
}
fn address(addr: SocketAddr) -> Value {
    let (family, raw) = match addr.ip() {
        IpAddr::V4(ip) => (4, ip.octets().to_vec()),
        IpAddr::V6(ip) => (6, ip.octets().to_vec()),
    };
    a([u(family), b(raw), u(u64::from(addr.port()))])
}
impl RuntimeLocatorFields2 {
    fn body(&self) -> Result<Value> {
        let mut ips = self
            .ips
            .iter()
            .copied()
            .map(address)
            .map(|v| Ok((encode(&v, 64)?, v)))
            .collect::<Result<Vec<_>>>()?;
        ips.sort_by(|a, b| a.0.cmp(&b.0));
        require(
            ips.windows(2).all(|w| w[0].0 != w[1].0),
            "duplicate locator address",
        )?;
        Ok(a([
            u(2),
            b(self.c),
            b(self.cert),
            b(self.peer_cert),
            b(self.endpoint),
            a(ips.into_iter().map(|(_, v)| v)),
            opt(self.relay.as_ref().map(|s| b(s.as_bytes()))),
            u(self.revision),
            u(self.not_before),
            u(self.expires),
        ]))
    }
}
#[derive(Clone)]
pub struct ParsedRuntimeLocator2 {
    raw: Vec<u8>,
    body: Value,
    fields: RuntimeLocatorFields2,
    signature: [u8; 64],
}
impl ParsedRuntimeLocator2 {
    pub fn sign(fields: RuntimeLocatorFields2, signer: &DeviceIdentity) -> Result<Self> {
        let body = fields.body()?;
        let sig = signer.sign(&signing_bytes(DESCRIPTOR, &body, 8192)?);
        Self::parse(&encode(&a([body, b(sig)]), 8192)?)
    }
    pub fn parse(raw: &[u8]) -> Result<Self> {
        let v = decode2(raw, WireLimit2::Descriptor)?;
        let f = v.fields(2)?;
        let t = f[0].fields(10)?;
        require(
            t[0].uint()? == 2 && t[7].uint()? > 0 && t[8].uint()? < t[9].uint()?,
            "runtime locator version/revision/interval",
        )?;
        let mut ips = vec![];
        let mut previous = None;
        require(t[5].list()?.len() <= 16, "locator IP count")?;
        for value in t[5].list()? {
            let x = value.fields(3)?;
            let ip = match x[0].uint()? {
                4 => IpAddr::V4(Ipv4Addr::from(x[1].fixed::<4>()?)),
                6 => IpAddr::V6(Ipv6Addr::from(x[1].fixed::<16>()?)),
                _ => return Err(Error::Invalid("locator IP family")),
            };
            require((1..=65535).contains(&x[2].uint()?), "locator port")?;
            let encoded = encode(value, 64)?;
            require(
                previous.as_ref().is_none_or(|p| p < &encoded),
                "locator sorted addresses",
            )?;
            previous = Some(encoded);
            ips.push(SocketAddr::new(ip, x[2].uint()? as u16));
        }
        let relay = t[6]
            .option()?
            .map(|v| -> Result<String> {
                let text =
                    std::str::from_utf8(v.bytes()?).map_err(|_| Error::Invalid("relay UTF8"))?;
                canonical_runtime_relay(text)?;
                Ok(text.to_owned())
            })
            .transpose()?;
        require(!ips.is_empty() || relay.is_some(), "empty runtime locator")?;
        Ok(Self {
            raw: raw.to_vec(),
            body: f[0].clone(),
            signature: f[1].fixed()?,
            fields: RuntimeLocatorFields2 {
                c: t[1].fixed()?,
                cert: t[2].fixed()?,
                peer_cert: t[3].fixed()?,
                endpoint: t[4].fixed()?,
                ips,
                relay,
                revision: t[7].uint()?,
                not_before: t[8].uint()?,
                expires: t[9].uint()?,
            },
        })
    }
    pub fn verify(
        &self,
        c: Id,
        own: &RootVerifiedCredential,
        peer: &RootVerifiedCredential,
    ) -> Result<()> {
        require(
            (self.fields.c, self.fields.cert, self.fields.peer_cert)
                == (c, own.cert_ref(), peer.cert_ref()),
            "runtime locator scope",
        )?;
        own.certificate().device_id().verify(
            &signing_bytes(DESCRIPTOR, &self.body, 8192)?,
            &self.signature,
        )?;
        Ok(())
    }
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
    pub fn id(&self) -> Id {
        hash(DESCRIPTOR, &self.raw)
    }
    pub fn fields(&self) -> &RuntimeLocatorFields2 {
        &self.fields
    }
    pub fn direct_lan_socket(&self, expected_ip: Ipv4Addr) -> Result<SocketAddr> {
        let [socket] = self.fields.ips.as_slice() else {
            return Err(Error::Invalid("LAN locator requires exactly one socket"));
        };
        require(
            self.fields.relay.is_none() && socket.ip() == IpAddr::V4(expected_ip),
            "LAN locator outside selected IPv4 route",
        )?;
        Ok(*socket)
    }
}

#[cfg(test)]
mod lan_tests {
    use super::*;

    #[test]
    fn cs41_on_link_unicast_uses_the_actual_prefix() -> Result<()> {
        let local = Ipv4Addr::new(192, 168, 4, 9);
        let with = |ip| DirectLan2::new(local, ip);
        assert!(
            with(Ipv4Addr::new(192, 168, 4, 255))?
                .check_on_link(23)
                .is_ok()
        );
        assert!(
            with(Ipv4Addr::new(192, 168, 4, 255))?
                .check_on_link(24)
                .is_err()
        );
        assert!(
            with(Ipv4Addr::new(192, 168, 5, 255))?
                .check_on_link(23)
                .is_err()
        );
        assert!(
            with(Ipv4Addr::new(192, 168, 4, 127))?
                .check_on_link(25)
                .is_err()
        );
        assert!(
            with(Ipv4Addr::new(192, 168, 4, 129))?
                .check_on_link(25)
                .is_err()
        );
        assert!(
            with(Ipv4Addr::new(192, 168, 4, 0))?
                .check_on_link(23)
                .is_err()
        );
        assert!(
            with(Ipv4Addr::new(192, 168, 4, 10))?
                .check_on_link(31)
                .is_err()
        );
        assert!(with(Ipv4Addr::LOCALHOST).is_err() && with(local).is_err());
        assert!(
            DirectLan2::new(Ipv4Addr::new(10, 0, 0, 0), Ipv4Addr::new(10, 0, 0, 1))?
                .check_on_link(24)
                .is_err()
        );
        Ok(())
    }
}
