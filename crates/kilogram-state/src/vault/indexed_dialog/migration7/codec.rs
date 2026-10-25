//! Closed local canonical uint/bstr/array codec. No protocol dependency.
use super::super::{Error, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    U(u64),
    B(Vec<u8>),
    A(Vec<Self>),
}
pub fn u(n: u64) -> Value {
    Value::U(n)
}
pub fn b(raw: impl AsRef<[u8]>) -> Value {
    Value::B(raw.as_ref().to_vec())
}
pub fn a(values: impl IntoIterator<Item = Value>) -> Value {
    Value::A(values.into_iter().collect())
}
pub fn opt(value: Option<Value>) -> Value {
    a(value)
}
impl Value {
    pub fn fields(&self, n: usize) -> Result<&[Self]> {
        let values = self.list()?;
        if values.len() != n {
            return Err(Error::Integrity("migration arity"));
        }
        Ok(values)
    }
    pub fn list(&self) -> Result<&[Self]> {
        match self {
            Self::A(xs) => Ok(xs),
            _ => Err(Error::Integrity("migration array")),
        }
    }
    pub fn uint(&self) -> Result<u64> {
        match self {
            Self::U(n) => Ok(*n),
            _ => Err(Error::Integrity("migration uint")),
        }
    }
    pub fn bytes(&self) -> Result<&[u8]> {
        match self {
            Self::B(raw) => Ok(raw),
            _ => Err(Error::Integrity("migration bstr")),
        }
    }
    pub fn fixed<const N: usize>(&self) -> Result<[u8; N]> {
        self.bytes()?
            .try_into()
            .map_err(|_| Error::Integrity("migration width"))
    }
    pub fn text(&self) -> Result<&str> {
        std::str::from_utf8(self.bytes()?).map_err(|_| Error::Integrity("migration UTF8"))
    }
    pub fn optional(&self) -> Result<Option<&Self>> {
        match self.list()? {
            [] => Ok(None),
            [x] => Ok(Some(x)),
            _ => Err(Error::Integrity("migration optional")),
        }
    }
}
fn head(out: &mut Vec<u8>, major: u8, n: u64) {
    match n {
        0..=23 => out.push(major << 5 | n as u8),
        24..=255 => out.extend([major << 5 | 24, n as u8]),
        256..=65535 => {
            out.push(major << 5 | 25);
            out.extend((n as u16).to_be_bytes());
        }
        65536..=0xffff_ffff => {
            out.push(major << 5 | 26);
            out.extend((n as u32).to_be_bytes());
        }
        _ => {
            out.push(major << 5 | 27);
            out.extend(n.to_be_bytes());
        }
    }
}
pub fn encode(value: &Value, cap: usize) -> Result<Vec<u8>> {
    fn emit(v: &Value, out: &mut Vec<u8>, cap: usize, depth: usize) -> Result<()> {
        if depth > 24 {
            return Err(Error::Bound("migration depth"));
        }
        match v {
            Value::U(n) => head(out, 0, *n),
            Value::B(raw) => {
                if raw.len() > cap.saturating_sub(out.len()) {
                    return Err(Error::Bound("migration bytes"));
                }
                head(out, 2, raw.len() as u64);
                out.extend(raw);
            }
            Value::A(xs) => {
                if xs.len() > 8192 {
                    return Err(Error::Bound("migration array count"));
                }
                head(out, 4, xs.len() as u64);
                for x in xs {
                    emit(x, out, cap, depth + 1)?;
                }
            }
        }
        if out.len() > cap {
            return Err(Error::Bound("migration encoding cap"));
        }
        Ok(())
    }
    let mut out = Vec::new();
    emit(value, &mut out, cap, 0)?;
    Ok(out)
}
pub fn decode(raw: &[u8], cap: usize) -> Result<Value> {
    struct Input<'a> {
        raw: &'a [u8],
        pos: usize,
        nodes: usize,
    }
    impl Input<'_> {
        fn take(&mut self, n: usize) -> Result<&[u8]> {
            let end = self
                .pos
                .checked_add(n)
                .ok_or(Error::Bound("migration length"))?;
            let value = self
                .raw
                .get(self.pos..end)
                .ok_or(Error::Integrity("migration truncation"))?;
            self.pos = end;
            Ok(value)
        }
        fn value(&mut self, depth: usize) -> Result<Value> {
            self.nodes += 1;
            if depth > 24 || self.nodes > 32768 {
                return Err(Error::Bound("migration decode budget"));
            }
            let first = self.take(1)?[0];
            let info = first & 31;
            let n = match info {
                0..=23 => u64::from(info),
                24 => u64::from(self.take(1)?[0]),
                25 => u64::from(u16::from_be_bytes(
                    self.take(2)?
                        .try_into()
                        .map_err(|_| Error::Integrity("width"))?,
                )),
                26 => u64::from(u32::from_be_bytes(
                    self.take(4)?
                        .try_into()
                        .map_err(|_| Error::Integrity("width"))?,
                )),
                27 => u64::from_be_bytes(
                    self.take(8)?
                        .try_into()
                        .map_err(|_| Error::Integrity("width"))?,
                ),
                _ => return Err(Error::Integrity("migration definite CBOR")),
            };
            if matches!(info, 24) && n < 24
                || info == 25 && n <= 255
                || info == 26 && n <= 65535
                || info == 27 && n <= 0xffff_ffff
            {
                return Err(Error::Integrity("migration shortest CBOR"));
            }
            match first >> 5 {
                0 => Ok(Value::U(n)),
                2 => {
                    let length =
                        usize::try_from(n).map_err(|_| Error::Bound("migration length"))?;
                    Ok(Value::B(self.take(length)?.to_vec()))
                }
                4 => {
                    let length = usize::try_from(n).map_err(|_| Error::Bound("migration count"))?;
                    if length > 8192 || length > self.raw.len().saturating_sub(self.pos) {
                        return Err(Error::Bound("migration array count"));
                    }
                    let mut xs = Vec::with_capacity(length);
                    for _ in 0..length {
                        xs.push(self.value(depth + 1)?);
                    }
                    Ok(Value::A(xs))
                }
                _ => Err(Error::Integrity("migration CBOR subset")),
            }
        }
    }
    if raw.len() > cap {
        return Err(Error::Bound("migration input cap"));
    }
    let mut input = Input {
        raw,
        pos: 0,
        nodes: 0,
    };
    let value = input.value(0)?;
    if input.pos != raw.len() {
        return Err(Error::Integrity("migration trailing bytes"));
    }
    Ok(value)
}
pub fn record(kind: u8, value: &Value, cap: usize) -> Result<Vec<u8>> {
    let body = encode(value, cap.saturating_sub(11))?;
    let mut raw = b"L7R\0\x00\x01".to_vec();
    raw.push(kind);
    raw.extend((body.len() as u32).to_be_bytes());
    raw.extend(body);
    Ok(raw)
}
pub fn read_record(raw: &[u8], kind: u8, cap: usize) -> Result<Value> {
    if raw.len() > cap || raw.len() < 11 || &raw[..6] != b"L7R\0\x00\x01" || raw[6] != kind {
        return Err(Error::Integrity("migration local record"));
    }
    let length = u32::from_be_bytes(
        raw[7..11]
            .try_into()
            .map_err(|_| Error::Integrity("record length"))?,
    ) as usize;
    if length != raw.len() - 11 {
        return Err(Error::Integrity("record length"));
    }
    decode(&raw[11..], cap - 11)
}
pub fn digest(domain: &[u8], value: &Value) -> Result<[u8; 32]> {
    let mut h = blake3::Hasher::new();
    h.update(domain);
    h.update(&encode(value, 512 * 1024)?);
    Ok(*h.finalize().as_bytes())
}
