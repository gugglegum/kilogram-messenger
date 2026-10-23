//! Public credential, consent, locator and transcript checks. No filesystem IO.
use anyhow::{Context, Result, ensure};
use iroh::{EndpointAddr, EndpointId};
use kilogram_identity::{
    AccountAuthoritySnapshot, ConversationMembershipSnapshot, DeviceCapability, DeviceCertificate,
    DeviceId, DeviceIdentity, verify_device_authorization_with_snapshot,
};
use kilogram_protocol::ordered_dialog_v1::{self as wire, Frame, Id, Value, a, b, u};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

pub const CONSENT: &[u8] = b"kg:od1:consent\0";
pub const DESCRIPTOR: &[u8] = b"kg:od1:descriptor\0";
pub const AUTH_PAIR: &[u8] = b"kg:od1:auth-pair\0";

#[derive(Clone)]
pub struct Credentials {
    pub cert: DeviceCertificate,
    pub authority: AccountAuthoritySnapshot,
    pub raw: Vec<u8>,
}
impl Credentials {
    pub fn new(cert: DeviceCertificate, authority: AccountAuthoritySnapshot) -> Result<Self> {
        let raw = wire::encode(
            &a([u(1), b(cert.encode()?), b(authority.encode()?)]),
            wire::MAX_AUTH,
        )?;
        Self::parse(&raw)
    }
    pub fn parse(raw: &[u8]) -> Result<Self> {
        let this = Self::decode(raw)?;
        verify_device_authorization_with_snapshot(
            this.cert.account_id(),
            &this.cert,
            &this.authority,
            &DeviceCapability::MESSAGING,
        )?;
        Ok(this)
    }
    /// Verifies Root signatures/encoding before policy; callers can pin a newer
    /// signed revocation even when it denies this Device's current authorization.
    pub fn decode(raw: &[u8]) -> Result<Self> {
        let v = wire::decode(raw, wire::MAX_AUTH)?;
        let f = v.fields(3)?;
        ensure!(f[0].uint()? == 1, "OD1 credentials version");
        let cert = DeviceCertificate::decode_and_verify(f[1].bytes()?)?;
        let authority = AccountAuthoritySnapshot::decode_and_verify(f[2].bytes()?)?;
        ensure!(
            cert.encode()? == f[1].bytes()? && authority.encode()? == f[2].bytes()?,
            "OD1 exact credential encoding"
        );
        ensure!(
            authority.account_id() == cert.account_id(),
            "credential authority account mismatch"
        );
        Ok(Self {
            cert,
            authority,
            raw: raw.to_vec(),
        })
    }
    pub fn reference(&self) -> Id {
        wire::hash(b"", &self.raw)
    }
    pub fn cert_ref(&self) -> Result<Id> {
        Ok(wire::hash(b"", &self.cert.encode()?))
    }
}

#[derive(Clone)]
pub struct Spec {
    pub c: Id,
    pub peers: [Credentials; 2],
    pub membership: Vec<u8>,
    pub consent: Vec<u8>,
}
impl Spec {
    pub fn parse(raw: &[u8]) -> Result<Self> {
        let v = wire::decode(raw, 96 * 1024)?;
        let f = v.fields(6)?;
        ensure!(f[0].uint()? == 1, "OD1 context version");
        let this = Self {
            c: f[1].fixed()?,
            peers: [
                Credentials::parse(f[2].bytes()?)?,
                Credentials::parse(f[3].bytes()?)?,
            ],
            membership: f[4].bytes()?.to_vec(),
            consent: f[5].bytes()?.to_vec(),
        };
        this.validate()?;
        Ok(this)
    }
    pub fn raw(&self) -> Result<Vec<u8>> {
        Ok(wire::encode(
            &a([
                u(1),
                b(self.c),
                b(&self.peers[0].raw),
                b(&self.peers[1].raw),
                b(&self.membership),
                b(&self.consent),
            ]),
            96 * 1024,
        )?)
    }
    pub fn role(&self, device: DeviceId) -> Result<u64> {
        if self.peers[0].cert.device_id() == device {
            Ok(0)
        } else if self.peers[1].cert.device_id() == device {
            Ok(1)
        } else {
            anyhow::bail!("device is outside OD1 context")
        }
    }
    pub fn validate(&self) -> Result<()> {
        let ca = &self.peers[0].cert;
        let cb = &self.peers[1].cert;
        ensure!(
            ca.device_id().as_bytes() < cb.device_id().as_bytes()
                && ca.account_id() != cb.account_id(),
            "OD1 two-account role election"
        );
        ensure!(
            self.membership.len() <= 32 * 1024,
            "OD1 membership byte cap"
        );
        let member = ConversationMembershipSnapshot::decode_and_verify(&self.membership)?;
        ensure!(
            member.encode()? == self.membership
                && member.conversation_id().as_bytes() == &self.c
                && member.owner_account_id() == ca.account_id()
                && member.members().len() == 2,
            "OD1 membership scope"
        );
        member.require_member(ca.account_id())?;
        member.require_member(cb.account_id())?;
        self.verify_consent(&self.consent)?;
        Ok(())
    }
    pub fn verify_consent(&self, raw: &[u8]) -> Result<(u64, u64)> {
        let v = wire::decode(raw, 4096)?;
        let f = v.fields(3)?;
        let body = f[0].fields(8)?;
        ensure!(
            body[0].uint()? == 1
                && body[1].fixed::<32>()? == self.c
                && body[2].fixed::<32>()? == self.peers[0].cert_ref()?
                && body[3].fixed::<32>()? == self.peers[1].cert_ref()?
                && body[4].uint()? == 1
                && body[5].uint()? == 1,
            "OD1 consent scope/suite/L1"
        );
        let nb = body[6].uint()?;
        let exp = body[7].uint()?;
        ensure!(exp > nb, "OD1 consent interval");
        let bytes = wire::encode(&f[0], 4096)?;
        self.peers[0]
            .cert
            .account_id()
            .verify_ordered_context_consent(&bytes, f[1].bytes()?)?;
        self.peers[1]
            .cert
            .account_id()
            .verify_ordered_context_consent(&bytes, f[2].bytes()?)?;
        Ok((nb, exp))
    }
    pub fn valid_now(&self, consent: &[u8], now: u64) -> Result<u64> {
        let (nb, exp) = self.verify_consent(consent)?;
        ensure!(
            now.saturating_add(120) >= nb && now <= exp.saturating_add(120),
            "AuthBlocked: consent interval"
        );
        Ok(exp)
    }
    pub fn cert_refs(&self) -> Result<[Id; 2]> {
        Ok([self.peers[0].cert_ref()?, self.peers[1].cert_ref()?])
    }
}

#[derive(Clone)]
pub struct Descriptor {
    pub c: Id,
    pub cert: Id,
    pub peer_cert: Id,
    pub endpoint: Id,
    pub addresses: Vec<SocketAddr>,
    pub revision: u64,
    pub not_before: u64,
    pub expires: u64,
    pub raw: Vec<u8>,
}
impl Descriptor {
    pub fn issue(
        spec: &Spec,
        role: u64,
        endpoint: Id,
        mut addresses: Vec<SocketAddr>,
        revision: u64,
        now: u64,
        identity: &DeviceIdentity,
    ) -> Result<Self> {
        ensure!(
            role <= 1
                && revision > 0
                && identity.device_id() == spec.peers[role as usize].cert.device_id(),
            "OD1 locator issuer"
        );
        addresses.sort();
        addresses.dedup();
        ensure!(
            !addresses.is_empty() && addresses.len() <= 16,
            "OD1 locator address cap"
        );
        let refs = spec.cert_refs()?;
        let body = a([
            u(1),
            b(spec.c),
            b(refs[role as usize]),
            b(refs[1 - role as usize]),
            b(endpoint),
            a(addresses.iter().map(address_value)),
            u(revision),
            u(now),
            u(now.checked_add(3600).context("locator time overflow")?),
        ]);
        let signature = identity.sign(&wire::signing_bytes(DESCRIPTOR, &body, 8192)?);
        let raw = wire::encode(&a([body, b(signature)]), 8192)?;
        Self::parse(spec, role, &raw)
    }
    pub fn parse(spec: &Spec, role: u64, raw: &[u8]) -> Result<Self> {
        ensure!(role <= 1, "OD1 descriptor role");
        let v = wire::decode(raw, 8192)?;
        let f = v.fields(2)?;
        let body = f[0].fields(9)?;
        let refs = spec.cert_refs()?;
        ensure!(
            body[0].uint()? == 1
                && body[1].fixed::<32>()? == spec.c
                && body[2].fixed::<32>()? == refs[role as usize]
                && body[3].fixed::<32>()? == refs[1 - role as usize],
            "OD1 descriptor pins"
        );
        spec.peers[role as usize].cert.device_id().verify(
            &wire::signing_bytes(DESCRIPTOR, &f[0], 8192)?,
            f[1].bytes()?,
        )?;
        let addresses = body[5]
            .list()?
            .iter()
            .map(parse_address)
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            !addresses.is_empty()
                && addresses.len() <= 16
                && addresses.windows(2).all(|w| w[0] < w[1]),
            "OD1 descriptor canonical addresses"
        );
        let nb = body[7].uint()?;
        let exp = body[8].uint()?;
        ensure!(
            body[6].uint()? > 0 && exp > nb && exp <= nb.saturating_add(3600),
            "OD1 descriptor revision/lifetime"
        );
        Ok(Self {
            c: spec.c,
            cert: refs[role as usize],
            peer_cert: refs[1 - role as usize],
            endpoint: body[4].fixed()?,
            addresses,
            revision: body[6].uint()?,
            not_before: nb,
            expires: exp,
            raw: raw.to_vec(),
        })
    }
    pub fn reference(&self) -> Id {
        wire::hash(DESCRIPTOR, &self.raw)
    }
    pub fn valid_now(&self, now: u64) -> Result<()> {
        ensure!(
            now.saturating_add(120) >= self.not_before && now < self.expires,
            "LocatorRefreshRequired"
        );
        Ok(())
    }
    pub fn addr(&self) -> Result<EndpointAddr> {
        let id = EndpointId::from_bytes(&self.endpoint)?;
        let mut addr = EndpointAddr::new(id);
        for ip in &self.addresses {
            addr = addr.with_ip_addr(*ip);
        }
        Ok(addr)
    }
}
fn address_value(addr: &SocketAddr) -> Value {
    match addr.ip() {
        IpAddr::V4(ip) => a([u(4), b(ip.octets()), u(u64::from(addr.port()))]),
        IpAddr::V6(ip) => a([u(6), b(ip.octets()), u(u64::from(addr.port()))]),
    }
}
fn parse_address(v: &Value) -> Result<SocketAddr> {
    let f = v.fields(3)?;
    let ip = match f[0].uint()? {
        4 => IpAddr::V4(Ipv4Addr::from(f[1].fixed::<4>()?)),
        6 => IpAddr::V6(Ipv6Addr::from(f[1].fixed::<16>()?)),
        _ => anyhow::bail!("IP family"),
    };
    let port = u16::try_from(f[2].uint()?)?;
    ensure!(port > 0, "zero locator port");
    Ok(SocketAddr::new(ip, port))
}

pub fn transcript(binding: Id, hello_a: &[u8], hello_b: &[u8]) -> Result<Id> {
    Ok(wire::hash(
        AUTH_PAIR,
        &wire::encode(&a([b(binding), b(hello_a), b(hello_b)]), 192 * 1024)?,
    ))
}
pub fn initial_pair(
    t: Id,
    hello_a: Vec<u8>,
    hello_b: Vec<u8>,
    proof_a: Vec<u8>,
    proof_b: Vec<u8>,
) -> Result<Vec<u8>> {
    Ok(wire::encode(
        &a([u(1), b(t), b(hello_a), b(hello_b), b(proof_a), b(proof_b)]),
        192 * 1024,
    )?)
}
/// A's first selection must name the exact currently authorized consent.
/// Historical replay also requires both matching Hello refs and BOTH Device
/// proofs, including our own proof of the admission that checked that consent.
/// A peer signature alone is insufficient. Every caller additionally requires
/// fresh current-channel admission; an old pair never supplies a current lease.
pub fn verify_initial_pair(spec: &Spec, raw: &[u8], selection_consent: Option<Id>) -> Result<Id> {
    let v = wire::decode(raw, 192 * 1024)?;
    let f = v.fields(6)?;
    ensure!(f[0].uint()? == 1, "initial pair version");
    let t = f[1].fixed()?;
    let ha = Frame::parse(f[2].bytes()?)?;
    let hb = Frame::parse(f[3].bytes()?)?;
    ensure!(
        ha.kind == 1 && hb.kind == 1 && ha.binding == hb.binding,
        "initial Hello pair"
    );
    let binding = ha.binding;
    let consent = ha.body.fields(11)?[9].fixed::<32>()?;
    ensure!(
        hb.body.fields(11)?[9].fixed::<32>()? == consent
            && selection_consent.is_none_or(|current| current == consent),
        "initial Hello consent agreement/selection"
    );
    for (frame, role) in [(&ha, 0), (&hb, 1)] {
        frame.verify(binding, spec.peers[role].cert.device_id())?;
        let h = frame.body.fields(11)?;
        ensure!(
            h[0].fixed::<32>()? == spec.c && h[1].uint()? == role as u64,
            "initial Hello scope/role"
        );
        ensure!(
            Credentials::parse(h[7].bytes()?)?.cert_ref()? == spec.peers[role].cert_ref()?
                && h[8].bytes()? == spec.membership,
            "initial credential/membership"
        );
        h[4].fixed::<32>()?;
        ensure!(
            h[5] == a([u(1)]) && h[6] == a([u(1)]),
            "initial negotiation"
        );
    }
    ensure!(
        ha.body.list()?[2] == hb.body.list()?[3] && ha.body.list()?[3] == hb.body.list()?[2],
        "initial endpoint roles"
    );
    ensure!(
        transcript(binding, f[2].bytes()?, f[3].bytes()?)? == t,
        "initial transcript hash"
    );
    let refs = spec.cert_refs()?;
    for (raw, role) in [(f[4].bytes()?, 0), (f[5].bytes()?, 1)] {
        let proof = Frame::parse(raw)?;
        ensure!(proof.kind == 2, "initial Proof kind");
        proof.verify(binding, spec.peers[role].cert.device_id())?;
        ensure!(
            proof.body == a([b(t), b(spec.c), u(1), u(1), b(refs[0]), b(refs[1])]),
            "initial Proof scope"
        );
    }
    Ok(t)
}
pub fn epoch(spec: &Spec, offer: &[u8], grant: &[u8]) -> Result<(Id, Id)> {
    let ca = wire::control(offer, 1, spec.peers[0].cert.device_id())?;
    let cb = wire::control(grant, 2, spec.peers[1].cert.device_id())?;
    let oa = ca.fields(8)?;
    let gb = cb.fields(8)?;
    let refs = spec.cert_refs()?;
    ensure!(
        oa[0].fixed::<32>()? == spec.c
            && oa[1].fixed::<32>()? == refs[0]
            && oa[2].fixed::<32>()? == refs[1]
            && gb[0].fixed::<32>()? == spec.c
            && gb[1].fixed::<32>()? == wire::hash(wire::CONTROL_ID, offer)
            && oa[3] == gb[2]
            && oa[6].uint()? == 1
            && oa[7].uint()? == 1
            && gb[6].uint()? == 1
            && gb[7].uint()? == 1,
        "OD1 Offer/Grant bindings"
    );
    for field in [&oa[3], &oa[4], &oa[5], &gb[3], &gb[4], &gb[5]] {
        field.fixed::<32>()?;
    }
    let e = wire::hash(
        b"kg:od1:epoch\0",
        &wire::encode(
            &a([u(1), b(spec.c), b(refs[0]), b(refs[1]), b(offer), b(grant)]),
            4096,
        )?,
    );
    let d = wire::hash(
        b"kg:od1:domain\0",
        &wire::encode(
            &a([u(1), u(1), b(spec.c), b(refs[0]), b(refs[1]), b(e)]),
            256,
        )?,
    );
    Ok((d, e))
}
