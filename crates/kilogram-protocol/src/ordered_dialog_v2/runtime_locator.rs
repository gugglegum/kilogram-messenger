//! Explicit ten-field production locator. The S3 nine-field descriptor is separate.
use super::*;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

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
}
