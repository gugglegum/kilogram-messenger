//! OD1 deterministic CBOR. Explicit ALPN only; legacy encoders remain unchanged.
use kilogram_identity::{DeviceId, DeviceIdentity};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::Zeroize;

use crate::canonical_v1::{
    CanonicalWireError, push_array_header, push_bytes, push_unsigned, validate_canonical_profile_v1,
};

pub const ALPN: &[u8] = b"kilogram/m1/ordered-dialog/cbor/1";
pub const MAX_WIRE: usize = 1024 * 1024;
pub const MAX_EVENT: usize = 16 * 1024;
pub const MAX_INNER: usize = 8 * 1024;
pub const MAX_AUTH: usize = 16 * 1024;
pub const LIFETIME: u64 = 512;
pub type Id = [u8; 32];

#[derive(Debug, Error)]
pub enum Error {
    #[error("OD1 {0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Canonical(#[from] CanonicalWireError),
    #[error(transparent)]
    Identity(#[from] kilogram_identity::IdentityError),
}
pub type Result<T> = std::result::Result<T, Error>;

/// Restricted value tree; canonical validation happens before any tree allocation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Value {
    U(u64),
    B(Vec<u8>),
    A(Vec<Value>),
}
impl Zeroize for Value {
    fn zeroize(&mut self) {
        match self {
            Self::U(n) => n.zeroize(),
            Self::B(raw) => raw.zeroize(),
            Self::A(values) => values.iter_mut().for_each(Zeroize::zeroize),
        }
    }
}
pub fn u(n: u64) -> Value {
    Value::U(n)
}
pub fn b(bytes: impl AsRef<[u8]>) -> Value {
    Value::B(bytes.as_ref().to_vec())
}
pub fn a(values: impl IntoIterator<Item = Value>) -> Value {
    Value::A(values.into_iter().collect())
}
pub fn opt(value: Option<Value>) -> Value {
    a(value)
}

impl Value {
    pub fn uint(&self) -> Result<u64> {
        match self {
            Self::U(n) => Ok(*n),
            _ => Err(Error::Invalid("uint")),
        }
    }
    pub fn bytes(&self) -> Result<&[u8]> {
        match self {
            Self::B(v) => Ok(v),
            _ => Err(Error::Invalid("bstr")),
        }
    }
    pub fn fixed<const N: usize>(&self) -> Result<[u8; N]> {
        self.bytes()?
            .try_into()
            .map_err(|_| Error::Invalid("fixed length"))
    }
    pub fn list(&self) -> Result<&[Value]> {
        match self {
            Self::A(v) => Ok(v),
            _ => Err(Error::Invalid("array")),
        }
    }
    pub fn fields(&self, len: usize) -> Result<&[Value]> {
        let fields = self.list()?;
        if fields.len() != len {
            return Err(Error::Invalid("array arity"));
        }
        Ok(fields)
    }
    pub fn option(&self) -> Result<Option<&Value>> {
        match self.list()? {
            [] => Ok(None),
            [value] => Ok(Some(value)),
            _ => Err(Error::Invalid("option arity")),
        }
    }
    pub fn text(&self) -> Result<&str> {
        std::str::from_utf8(self.bytes()?).map_err(|_| Error::Invalid("UTF8"))
    }
}

pub fn encode(value: &Value, limit: usize) -> Result<Vec<u8>> {
    fn emit(
        value: &Value,
        out: &mut Vec<u8>,
        depth: usize,
        items: &mut usize,
        limit: usize,
    ) -> Result<()> {
        *items += 1;
        if depth > 8 || *items > 8192 {
            return Err(Error::Invalid("depth/items"));
        }
        match value {
            Value::U(n) => push_unsigned(out, *n),
            Value::B(v) => {
                if v.len() > limit.saturating_sub(out.len()) {
                    return Err(Error::Invalid("encoded byte cap"));
                }
                push_bytes(out, v);
            }
            Value::A(v) => {
                if v.len() > 8192 {
                    return Err(Error::Invalid("array count"));
                }
                push_array_header(out, v.len() as u64);
                for value in v {
                    emit(value, out, depth + 1, items, limit)?;
                }
            }
        }
        if out.len() > limit {
            return Err(Error::Invalid("encoded byte cap"));
        }
        Ok(())
    }
    let mut out = Vec::new();
    emit(value, &mut out, 0, &mut 0, limit)?;
    Ok(out)
}

pub fn decode(raw: &[u8], limit: usize) -> Result<Value> {
    if raw.len() > limit {
        return Err(Error::Invalid("encoded byte cap"));
    }
    validate_canonical_profile_v1(raw)?;
    fn take<'a>(raw: &'a [u8], pos: &mut usize, n: usize) -> Result<&'a [u8]> {
        let end = pos
            .checked_add(n)
            .ok_or(Error::Invalid("length overflow"))?;
        let value = raw.get(*pos..end).ok_or(Error::Invalid("truncated"))?;
        *pos = end;
        Ok(value)
    }
    fn item(raw: &[u8], pos: &mut usize) -> Result<Value> {
        let head = take(raw, pos, 1)?[0];
        let n = match head & 31 {
            n @ 0..=23 => u64::from(n),
            24 => u64::from(take(raw, pos, 1)?[0]),
            25 => u64::from(u16::from_be_bytes(
                take(raw, pos, 2)?
                    .try_into()
                    .map_err(|_| Error::Invalid("u16"))?,
            )),
            26 => u64::from(u32::from_be_bytes(
                take(raw, pos, 4)?
                    .try_into()
                    .map_err(|_| Error::Invalid("u32"))?,
            )),
            27 => u64::from_be_bytes(
                take(raw, pos, 8)?
                    .try_into()
                    .map_err(|_| Error::Invalid("u64"))?,
            ),
            _ => return Err(Error::Invalid("head")),
        };
        match head >> 5 {
            0 => Ok(u(n)),
            2 => Ok(b(take(
                raw,
                pos,
                usize::try_from(n).map_err(|_| Error::Invalid("length"))?,
            )?)),
            4 => (0..n)
                .map(|_| item(raw, pos))
                .collect::<Result<Vec<_>>>()
                .map(Value::A),
            _ => Err(Error::Invalid("major type")),
        }
    }
    item(raw, &mut 0)
}

pub fn hash(tag: &'static [u8], bytes: &[u8]) -> Id {
    let mut h = blake3::Hasher::new();
    h.update(tag);
    h.update(bytes);
    *h.finalize().as_bytes()
}
pub fn signing_bytes(tag: &[u8], fields: &Value, limit: usize) -> Result<Vec<u8>> {
    let raw = encode(fields, limit)?;
    let mut bytes = Vec::with_capacity(tag.len() + raw.len());
    bytes.extend_from_slice(tag);
    bytes.extend_from_slice(&raw);
    Ok(bytes)
}

pub const EVENT_SIGN: &[u8] = b"kg:od1:event-sign\0";
pub const EVENT_ID: &[u8] = b"kg:od1:event-id\0";
pub const CONTROL_SIGN: &[u8] = b"kg:od1:control-sign\0";
pub const CONTROL_ID: &[u8] = b"kg:od1:control-id\0";
pub const RECEIPT_SIGN: &[u8] = b"kg:od1:receipt-sign\0";
pub const RECEIPT_ID: &[u8] = b"kg:od1:receipt-id\0";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ClosedTail {
    pub g: u64,
    pub key: Id,
    pub index: u64,
    pub last: Id,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Expected {
    pub g: u64,
    pub index: u64,
    pub parent: Option<Id>,
    pub tail: Option<ClosedTail>,
}
impl Expected {
    pub fn value(&self) -> Value {
        a([
            u(self.g),
            u(self.index),
            opt(self.parent.map(b)),
            opt(self
                .tail
                .as_ref()
                .map(|t| a([u(t.g), b(t.key), u(t.index), b(t.last)]))),
        ])
    }
    pub fn parse(v: &Value) -> Result<Self> {
        let f = v.fields(4)?;
        let tail = f[3]
            .option()?
            .map(|v| -> Result<ClosedTail> {
                let t = v.fields(4)?;
                Ok(ClosedTail {
                    g: t[0].uint()?,
                    key: t[1].fixed()?,
                    index: t[2].uint()?,
                    last: t[3].fixed()?,
                })
            })
            .transpose()?;
        Ok(Self {
            g: f[0].uint()?,
            index: f[1].uint()?,
            parent: optional_id(&f[2])?,
            tail,
        })
    }
}
pub fn optional_id(v: &Value) -> Result<Option<Id>> {
    v.option()?.map(Value::fixed).transpose()
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Core {
    pub c: Id,
    pub d: Id,
    pub e: Id,
    pub sid: String,
    pub role: u64,
    pub n: u64,
    pub previous: Option<Id>,
    pub sequence: u64,
    pub kind: u64,
    pub request: [u8; 16],
    pub parents: Vec<Id>,
    pub expected: Expected,
    pub author_auth: Id,
}
impl Core {
    pub fn value(&self) -> Value {
        a([
            u(1),
            b(self.c),
            b(self.d),
            b(self.e),
            b(self.sid.as_bytes()),
            u(self.role),
            u(self.n),
            opt(self.previous.map(b)),
            u(self.sequence),
            u(self.kind),
            b(self.request),
            a(self.parents.iter().map(b)),
            self.expected.value(),
            b(self.author_auth),
        ])
    }
    pub fn parse(v: &Value) -> Result<Self> {
        let f = v.fields(14)?;
        if f[0].uint()? != 1
            || f[5].uint()? > 1
            || f[6].uint()? >= LIFETIME
            || !(1..=3).contains(&f[9].uint()?)
            || f[4].bytes()?.len() != 43
            || !f[4].bytes()?.is_ascii()
        {
            return Err(Error::Invalid("core version/limits"));
        }
        let parents = f[11]
            .list()?
            .iter()
            .map(Value::fixed)
            .collect::<Result<Vec<Id>>>()?;
        if parents.len() > 2 || parents.windows(2).any(|p| p[0] >= p[1]) {
            return Err(Error::Invalid("parents"));
        }
        let core = Self {
            c: f[1].fixed()?,
            d: f[2].fixed()?,
            e: f[3].fixed()?,
            sid: f[4].text()?.to_owned(),
            role: f[5].uint()?,
            n: f[6].uint()?,
            previous: optional_id(&f[7])?,
            sequence: f[8].uint()?,
            kind: f[9].uint()?,
            request: f[10].fixed()?,
            parents,
            expected: Expected::parse(&f[12])?,
            author_auth: f[13].fixed()?,
        };
        if (core.n == 0) != (core.kind != 3) || (core.n == 0) != core.previous.is_none() {
            return Err(Error::Invalid("genesis/predecessor"));
        }
        if encode(v, 2048)?.len() > 2048 {
            return Err(Error::Invalid("core cap"));
        }
        Ok(core)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub core: Core,
    pub native_kind: u64,
    pub key: Id,
    pub index: u64,
    pub ciphertext: Vec<u8>,
    pub signature: Vec<u8>,
}
impl Event {
    fn unsigned(&self) -> Value {
        a([
            u(1),
            self.core.value(),
            a([u(self.native_kind), b(self.key), u(self.index)]),
            b(&self.ciphertext),
        ])
    }
    pub fn raw(&self) -> Result<Vec<u8>> {
        let Value::A(mut f) = self.unsigned() else {
            unreachable!()
        };
        f.push(b(&self.signature));
        encode(&a(f), MAX_EVENT)
    }
    pub fn id(&self) -> Result<Id> {
        Ok(hash(EVENT_ID, &self.raw()?))
    }
    pub fn sign(&mut self, identity: &DeviceIdentity) -> Result<()> {
        self.signature = identity
            .sign(&signing_bytes(EVENT_SIGN, &self.unsigned(), MAX_EVENT)?)
            .to_vec();
        Ok(())
    }
    pub fn parse(raw: &[u8], author: DeviceId) -> Result<Self> {
        let value = decode(raw, MAX_EVENT)?;
        let f = value.fields(5)?;
        let h = f[2].fields(3)?;
        if f[0].uint()? != 1 || h[0].uint()? > 1 || f[3].bytes()?.len() > 12 * 1024 {
            return Err(Error::Invalid("event header"));
        }
        let event = Self {
            core: Core::parse(&f[1])?,
            native_kind: h[0].uint()?,
            key: h[1].fixed()?,
            index: h[2].uint()?,
            ciphertext: f[3].bytes()?.to_vec(),
            signature: f[4].fixed::<64>()?.to_vec(),
        };
        author.verify(
            &signing_bytes(EVENT_SIGN, &event.unsigned(), MAX_EVENT)?,
            &event.signature,
        )?;
        let packet = kilogram_ratchet::ordered::Packet {
            kind: u8::try_from(event.native_kind).map_err(|_| Error::Invalid("kind"))?,
            bytes: event.ciphertext.clone(),
        };
        let actual = packet
            .header()
            .map_err(|_| Error::Invalid("native header encoding"))?;
        if actual.key != event.key
            || actual.index != event.index
            || (event.core.kind == 1) != (event.native_kind == 0)
            || actual
                .prekey_sid
                .as_ref()
                .is_some_and(|s| s != &event.core.sid)
        {
            return Err(Error::Invalid("native outer mismatch"));
        }
        Ok(event)
    }
    pub fn inner(core: &Core, payload: Value) -> Result<Vec<u8>> {
        encode(&a([u(1), core.value(), payload]), MAX_INNER)
    }
    pub fn verify_inner(&self, raw: &[u8]) -> Result<Value> {
        let v = decode(raw, MAX_INNER)?;
        let f = v.fields(3)?;
        if f[0].uint()? != 1 || f[1] != self.core.value() {
            return Err(Error::Invalid("inner/outer binding"));
        }
        Ok(f[2].clone())
    }
}

pub fn signed_control(kind: u64, body: Value, identity: &DeviceIdentity) -> Result<Vec<u8>> {
    if !(1..=3).contains(&kind) {
        return Err(Error::Invalid("control kind"));
    }
    body.fields(8)?;
    let unsigned = a([u(1), u(kind), body.clone()]);
    let sig = identity.sign(&signing_bytes(CONTROL_SIGN, &unsigned, 32 * 1024)?);
    encode(&a([u(1), u(kind), body, b(sig)]), 32 * 1024)
}
pub fn control(raw: &[u8], kind: u64, signer: DeviceId) -> Result<Value> {
    let v = decode(raw, 32 * 1024)?;
    let f = v.fields(4)?;
    if f[0].uint()? != 1 || f[1].uint()? != kind || !(1..=3).contains(&kind) {
        return Err(Error::Invalid("control kind/version"));
    }
    f[2].fields(8)?;
    signer.verify(
        &signing_bytes(CONTROL_SIGN, &a(f[..3].iter().cloned()), 32 * 1024)?,
        f[3].bytes()?,
    )?;
    Ok(f[2].clone())
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub kind: u64,
    pub binding: Id,
    pub request: [u8; 16],
    pub body: Value,
    pub signature: Vec<u8>,
}
impl Frame {
    fn tag(kind: u64) -> &'static [u8] {
        match kind {
            1 => b"kg:od1:auth-hello\0",
            2 => b"kg:od1:auth-proof\0",
            _ => CONTROL_SIGN,
        }
    }
    fn unsigned(&self) -> Value {
        a([
            u(1),
            u(self.kind),
            b(self.binding),
            b(self.request),
            self.body.clone(),
        ])
    }
    pub fn signed(
        kind: u64,
        binding: Id,
        request: [u8; 16],
        body: Value,
        identity: &DeviceIdentity,
    ) -> Result<Self> {
        let mut f = Self {
            kind,
            binding,
            request,
            body,
            signature: Vec::new(),
        };
        f.signature = identity
            .sign(&signing_bytes(
                Self::tag(kind),
                &f.unsigned(),
                MAX_WIRE - 4,
            )?)
            .to_vec();
        f.raw()?;
        Ok(f)
    }
    pub fn raw(&self) -> Result<Vec<u8>> {
        let Value::A(mut f) = self.unsigned() else {
            unreachable!()
        };
        f.push(b(&self.signature));
        encode(&a(f), MAX_WIRE - 4)
    }
    pub fn parse(raw: &[u8]) -> Result<Self> {
        let v = decode(raw, MAX_WIRE - 4)?;
        let f = v.fields(6)?;
        let kind = f[1].uint()?;
        if f[0].uint()? != 1 || !(1..=17).contains(&kind) {
            return Err(Error::Invalid("wire version/kind"));
        }
        let arity = [0, 11, 6, 4, 6, 3, 3, 4, 4, 9, 6, 12, 10, 5, 8, 7, 5, 7][kind as usize];
        f[4].fields(arity)?;
        Ok(Self {
            kind,
            binding: f[2].fixed()?,
            request: f[3].fixed()?,
            body: f[4].clone(),
            signature: f[5].fixed::<64>()?.to_vec(),
        })
    }
    pub fn verify(&self, binding: Id, signer: DeviceId) -> Result<()> {
        if self.binding != binding {
            return Err(Error::Invalid("TLS channel binding"));
        }
        signer.verify(
            &signing_bytes(Self::tag(self.kind), &self.unsigned(), MAX_WIRE - 4)?,
            &self.signature,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_noncanonical_and_allocation_bombs_and_domain_confusion() -> Result<()> {
        for raw in [
            vec![0x18, 0],
            vec![0x9f, 0xff],
            vec![0xa0],
            vec![0x60],
            vec![0, 0],
            vec![0x9a, 0xff, 0xff, 0xff, 0xff],
        ] {
            assert!(decode(&raw, 1024).is_err());
        }
        assert!(encode(&b([0; 32]), 16).is_err());
        let identity = DeviceIdentity::from_secret_bytes([7; 32]);
        let body = a([
            b([1; 32]),
            b([2; 32]),
            b([b'A'; 43]),
            b([3; 32]),
            b([4; 32]),
            u(0),
            u(1),
            u(1),
        ]);
        let ready = signed_control(3, body.clone(), &identity)?;
        assert_eq!(control(&ready, 3, identity.device_id())?, body);
        assert!(control(&ready, 1, identity.device_id()).is_err());
        assert_ne!(hash(EVENT_ID, &ready), hash(CONTROL_ID, &ready));
        Ok(())
    }
    #[test]
    fn frame_is_bound_to_exporter_and_schema() -> Result<()> {
        let identity = DeviceIdentity::from_secret_bytes([8; 32]);
        let frame = Frame::signed(
            3,
            [1; 32],
            [2; 16],
            a([b([3; 32]), b([4; 32]), u(1), u(1)]),
            &identity,
        )?;
        let parsed = Frame::parse(&frame.raw()?)?;
        parsed.verify([1; 32], identity.device_id())?;
        assert!(parsed.verify([5; 32], identity.device_id()).is_err());
        let bad = Frame::signed(3, [1; 32], [2; 16], a([u(1)]), &identity)?;
        assert!(Frame::parse(&bad.raw()?).is_err());
        Ok(())
    }
}
