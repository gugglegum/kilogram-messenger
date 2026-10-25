//! Explicit protocol2 component. Parsing, signatures, native MAC and live policy are distinct.
pub use crate::ordered_dialog_v1::{
    Id, Value, a, b, decode, encode, hash, opt, optional_id, signing_bytes, u,
};
use kilogram_identity::{DeviceId, DeviceIdentity};
pub use kilogram_ratchet::ordered_v2::{ClosedTail, Expected};
use thiserror::Error;

mod authority;
mod frame;
mod mode;
mod runtime_locator;
pub use authority::*;
pub use frame::*;
pub use mode::*;
pub use runtime_locator::*;

pub const ALPN: &[u8] = b"kilogram/m1/ordered-dialog/cbor/2";
pub const MAX_WIRE: usize = 1024 * 1024;
pub const MAX_CORE: usize = 2048;
pub const MAX_EVENT: usize = 16 * 1024;
pub const MAX_INNER: usize = 8 * 1024;
pub const MAX_OLM: usize = 12 * 1024;
pub const MAX_TEXT: usize = 4096;
pub const MAX_AUTH: usize = 16 * 1024;
pub const MAX_PROOF: usize = 16 * 1024;
pub const MAX_RECEIPT: usize = 1024;
pub const EVENT_SIGN: &[u8] = b"kg:od2:event-sign\0";
pub const EVENT_ID: &[u8] = b"kg:od2:event-id\0";
pub const CONTROL_SIGN: &[u8] = b"kg:od2:control-sign\0";
pub const CONTROL_ID: &[u8] = b"kg:od2:control-id\0";
pub const RECEIPT_SIGN: &[u8] = b"kg:od2:receipt-sign\0";
pub const RECEIPT_ID: &[u8] = b"kg:od2:receipt-id\0";
pub const CONSENT: &[u8] = b"kg:od2:consent\0";
pub const MODE: &[u8] = b"kg:od2:mode-certificate\0";
pub const AUTH_PAIR: &[u8] = b"kg:od2:auth-pair\0";
pub const DESCRIPTOR: &[u8] = b"kg:od2:descriptor\0";

#[derive(Clone, Copy, Debug)]
pub enum WireLimit2 {
    Core,
    Event,
    Inner,
    Olm,
    Text,
    Auth,
    Membership,
    Consent,
    ModeProof,
    Frame,
    Descriptor,
    Receipt,
    InitialPair,
}
impl WireLimit2 {
    pub fn maximum(self) -> usize {
        match self {
            Self::Core => MAX_CORE,
            Self::Event => MAX_EVENT,
            Self::Inner => MAX_INNER,
            Self::Olm => MAX_OLM,
            Self::Text => MAX_TEXT,
            Self::Auth => MAX_AUTH,
            Self::Membership => 32 * 1024,
            Self::Consent => 4096,
            Self::ModeProof => MAX_PROOF,
            Self::Frame => MAX_WIRE - 4,
            Self::Descriptor => 8192,
            Self::Receipt => MAX_RECEIPT,
            Self::InitialPair => 192 * 1024,
        }
    }
    pub fn check_len(self, len: usize) -> Result<()> {
        require(len <= self.maximum(), "wire length cap")
    }
}
fn decode2(raw: &[u8], limit: WireLimit2) -> Result<Value> {
    limit.check_len(raw.len())?;
    Ok(decode(raw, limit.maximum())?)
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("OD2 {0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Profile(#[from] crate::ordered_dialog_v1::Error),
    #[error(transparent)]
    Identity(#[from] kilogram_identity::IdentityError),
}
pub type Result<T> = std::result::Result<T, Error>;
fn require(ok: bool, reason: &'static str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::Invalid(reason))
    }
}
pub fn expected_value(e: &Expected) -> Value {
    a([
        u(e.g),
        u(e.index),
        opt(e.parent.map(b)),
        opt(e
            .tail
            .as_ref()
            .map(|t| a([u(t.g), b(t.key), u(t.index), b(t.last)]))),
    ])
}
pub fn parse_expected(value: &Value) -> Result<Expected> {
    let old = crate::ordered_dialog_v1::Expected::parse(value)?;
    Ok(Expected {
        g: old.g,
        index: old.index,
        parent: old.parent,
        tail: old.tail.map(|t| ClosedTail {
            g: t.g,
            key: t.key,
            index: t.index,
            last: t.last,
        }),
    })
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Core2 {
    pub c: Id,
    pub d: Id,
    pub e: Id,
    pub sid: String,
    pub role: u8,
    pub n: u64,
    pub previous: Option<Id>,
    pub sequence: u64,
    pub kind: u8,
    pub request: [u8; 16],
    pub parents: Vec<Id>,
    pub expected: Expected,
    pub author_auth: Id,
    pub q: Id,
}
impl Core2 {
    pub fn value(&self) -> Value {
        a([
            u(2),
            b(self.c),
            b(self.d),
            b(self.e),
            b(self.sid.as_bytes()),
            u(u64::from(self.role)),
            u(self.n),
            opt(self.previous.map(b)),
            u(self.sequence),
            u(u64::from(self.kind)),
            b(self.request),
            a(self.parents.iter().map(b)),
            expected_value(&self.expected),
            b(self.author_auth),
            b(self.q),
        ])
    }
    pub fn parse(value: &Value) -> Result<Self> {
        let f = value.fields(15)?;
        require(
            f[0].uint()? == 2 && f[5].uint()? <= 1 && (1..=3).contains(&f[9].uint()?),
            "Core version/role/kind",
        )?;
        let sid = f[4].bytes()?;
        require(sid.len() == 43 && sid.is_ascii(), "SID")?;
        let parents = f[11]
            .list()?
            .iter()
            .map(Value::fixed)
            .collect::<std::result::Result<Vec<Id>, _>>()?;
        require(
            parents.len() <= 2 && parents.windows(2).all(|p| p[0] < p[1]),
            "parents",
        )?;
        let core = Self {
            c: f[1].fixed()?,
            d: f[2].fixed()?,
            e: f[3].fixed()?,
            sid: f[4].text()?.to_owned(),
            role: f[5].uint()? as u8,
            n: f[6].uint()?,
            previous: optional_id(&f[7])?,
            sequence: f[8].uint()?,
            kind: f[9].uint()? as u8,
            request: f[10].fixed()?,
            parents,
            expected: parse_expected(&f[12])?,
            author_auth: f[13].fixed()?,
            q: f[14].fixed()?,
        };
        require(
            if core.n == 0 {
                core.kind == core.role + 1 && core.previous.is_none() && core.parents.is_empty()
            } else {
                core.kind == 3 && core.previous.is_some()
            },
            "genesis/predecessor",
        )?;
        require(
            core.n <= 1 || core.previous.is_some_and(|id| core.parents.contains(&id)),
            "previous user parent",
        )?;
        encode(value, MAX_CORE)?;
        Ok(core)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedEvent2 {
    raw: Vec<u8>,
    core: Core2,
    kind: u8,
    key: Id,
    index: u64,
    olm: Vec<u8>,
    signature: [u8; 64],
    core_offset: usize,
    core_len: usize,
}
impl ParsedEvent2 {
    pub fn parse(raw: &[u8]) -> Result<Self> {
        let v = decode2(raw, WireLimit2::Event)?;
        let f = v.fields(5)?;
        let h = f[2].fields(3)?;
        require(
            f[0].uint()? == 2 && h[0].uint()? <= 1,
            "Event version/header",
        )?;
        require(
            !f[3].bytes()?.is_empty() && f[3].bytes()?.len() <= MAX_OLM,
            "Olm cap",
        )?;
        let core = Core2::parse(&f[1])?;
        let kind = h[0].uint()? as u8;
        let index = h[2].uint()?;
        require(
            core.expected.index == index && (core.kind == 1) == (kind == 0),
            "outer header kind/index",
        )?;
        let core_raw = encode(&f[1], MAX_CORE)?;
        // Event root array5/version2 are each one canonical byte.
        require(
            raw.get(2..2 + core_raw.len()) == Some(core_raw.as_slice()),
            "Core span",
        )?;
        Ok(Self {
            raw: raw.to_vec(),
            core,
            kind,
            key: h[1].fixed()?,
            index,
            olm: f[3].bytes()?.to_vec(),
            signature: f[4].fixed()?,
            core_offset: 2,
            core_len: core_raw.len(),
        })
    }
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
    pub fn id(&self) -> Id {
        hash(EVENT_ID, &self.raw)
    }
    pub fn core(&self) -> &Core2 {
        &self.core
    }
    pub fn core_span(&self) -> (usize, usize) {
        (self.core_offset, self.core_len)
    }
    pub fn packet(&self) -> kilogram_ratchet::ordered::Packet {
        kilogram_ratchet::ordered::Packet {
            kind: self.kind,
            bytes: self.olm.clone(),
        }
    }
    pub fn meta(&self) -> kilogram_ratchet::ordered_v2::EventMeta {
        kilogram_ratchet::ordered_v2::EventMeta {
            role: self.core.role,
            n: self.core.n,
            previous: self.core.previous,
            sequence: self.core.sequence,
            kind: self.core.kind,
            native_kind: self.kind,
            key: self.key,
            index: self.index,
            expected: self.core.expected.clone(),
        }
    }
    fn unsigned(&self) -> Value {
        a([
            u(2),
            self.core.value(),
            a([u(u64::from(self.kind)), b(self.key), u(self.index)]),
            b(&self.olm),
        ])
    }
    pub fn verify(
        self,
        selection: &StaticSelection2,
        author: &RootVerifiedCredential,
    ) -> Result<VerifiedEvent2> {
        let c = &self.core;
        require(
            (c.c, c.d, c.e, c.q) == (selection.c, selection.d, selection.e, selection.proof.q()),
            "event scope/Q",
        )?;
        require(
            selection.sid.as_ref().is_none_or(|s| s == &c.sid),
            "selected SID",
        )?;
        require(
            selection.proof.format_for(c.role, c.n)? == 2,
            "format2 cut/origin",
        )?;
        require(
            author.cert_ref() == selection.proof.certs()[usize::from(c.role)]
                && author.reference() == c.author_auth,
            "event author",
        )?;
        author.cert.device_id().verify(
            &signing_bytes(EVENT_SIGN, &self.unsigned(), MAX_EVENT)?,
            &self.signature,
        )?;
        if c.n == 0 {
            require(
                selection
                    .proof
                    .1
                    .is_some_and(|refs| refs[usize::from(c.role)] == c.author_auth),
                "genesis selected original authority",
            )?;
            let request = if c.role == 0 {
                selection.proof.0.offer_id()
            } else {
                selection.proof.0.grant_id()
            };
            require(
                request.is_some_and(|id| c.request.as_slice() == &id[..16]),
                "genesis request/R4",
            )?;
        }
        let actual = self
            .packet()
            .header()
            .map_err(|_| Error::Invalid("actual native header"))?;
        require(
            actual.key == self.key
                && actual.index == self.index
                && actual.prekey_sid.as_ref().is_none_or(|s| s == &c.sid),
            "actual native/outer",
        )?;
        if c.kind == 1 {
            require(
                actual
                    .prekey_keys
                    .is_some_and(|k| selection.proof.0.prekey_selection() == Some((k[0], k[2]))),
                "prekey selection",
            )?;
        }
        Ok(VerifiedEvent2(self))
    }
}
pub fn sign_event(
    core: Core2,
    packet: &kilogram_ratchet::ordered::Packet,
    signer: &DeviceIdentity,
) -> Result<ParsedEvent2> {
    Core2::parse(&core.value())?;
    let h = packet
        .header()
        .map_err(|_| Error::Invalid("native output header"))?;
    let fields = a([
        u(2),
        core.value(),
        a([u(u64::from(packet.kind)), b(h.key), u(h.index)]),
        b(&packet.bytes),
    ]);
    let sig = signer.sign(&signing_bytes(EVENT_SIGN, &fields, MAX_EVENT)?);
    let mut f = fields.list()?.to_vec();
    f.push(b(sig));
    ParsedEvent2::parse(&encode(&a(f), MAX_EVENT)?)
}
pub struct VerifiedEvent2(ParsedEvent2);
impl VerifiedEvent2 {
    pub fn event(&self) -> &ParsedEvent2 {
        &self.0
    }
    /// Call only with the successful native trial plaintext. It does not itself perform a MAC.
    pub fn verify_inner(&self, raw: &[u8]) -> Result<VerifiedPayload2> {
        let v = decode2(raw, WireLimit2::Inner)?;
        let f = v.fields(3)?;
        require(
            f[0].uint()? == 2 && f[1] == self.0.core.value(),
            "inner Core/version",
        )?;
        validate_payload(self.0.core.kind, &f[2])?;
        Ok(VerifiedPayload2(f[2].clone()))
    }
}
pub struct VerifiedPayload2(Value);
impl VerifiedPayload2 {
    pub fn value(&self) -> &Value {
        &self.0
    }
}
pub fn inner2(core: &Core2, payload: Value) -> Result<Vec<u8>> {
    Core2::parse(&core.value())?;
    validate_payload(core.kind, &payload)?;
    Ok(encode(&a([u(2), core.value(), payload]), MAX_INNER)?)
}
fn validate_payload(kind: u8, payload: &Value) -> Result<()> {
    match kind {
        1 => {
            let f = payload.fields(4)?;
            for x in &f[..3] {
                x.fixed::<32>()?;
            }
            for x in f[3].fields(3)? {
                x.fixed::<32>()?;
            }
        }
        2 => {
            let f = payload.fields(4)?;
            f[0].fixed::<32>()?;
            f[1].fixed::<32>()?;
            require(
                f[2].bytes()?.len() == 43 && f[2].bytes()?.is_ascii(),
                "Confirm SID",
            )?;
            f[3].fixed::<32>()?;
        }
        3 => {
            let f = payload.fields(1)?;
            WireLimit2::Text.check_len(f[0].bytes()?.len())?;
            f[0].text()?;
        }
        _ => return Err(Error::Invalid("payload kind")),
    }
    Ok(())
}

#[cfg(test)]
mod tests;
