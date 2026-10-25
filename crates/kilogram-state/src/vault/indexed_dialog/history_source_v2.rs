//! Restricted borrowed structural reader. No identity signatures, Olm parsing or policy.
use super::super::{Error, HistoryScope, Id, Result, history_types::hash_parts};

#[derive(Debug)]
enum Value<'a> {
    U(u64),
    B(&'a [u8]),
    A(Vec<Node<'a>>),
}
#[derive(Debug)]
struct Node<'a> {
    v: Value<'a>,
    start: usize,
    end: usize,
}
impl<'a> Node<'a> {
    fn fields(&self, n: usize) -> Result<&[Self]> {
        let v = self.list()?;
        if v.len() != n {
            return Err(Error::Integrity("wire2 arity"));
        }
        Ok(v)
    }
    fn list(&self) -> Result<&[Self]> {
        if let Value::A(v) = &self.v {
            Ok(v)
        } else {
            Err(Error::Integrity("wire2 array"))
        }
    }
    fn uint(&self) -> Result<u64> {
        if let Value::U(n) = self.v {
            Ok(n)
        } else {
            Err(Error::Integrity("wire2 uint"))
        }
    }
    fn bytes(&self) -> Result<&'a [u8]> {
        if let Value::B(b) = self.v {
            Ok(b)
        } else {
            Err(Error::Integrity("wire2 bstr"))
        }
    }
    fn fixed<const N: usize>(&self) -> Result<[u8; N]> {
        self.bytes()?
            .try_into()
            .map_err(|_| Error::Integrity("wire2 width"))
    }
    fn optional(&self) -> Result<Option<&Self>> {
        match self.list()? {
            [] => Ok(None),
            [x] => Ok(Some(x)),
            _ => Err(Error::Integrity("wire2 option")),
        }
    }
    fn id_opt(&self) -> Result<Option<Id>> {
        self.optional()?.map(Self::fixed).transpose()
    }
    fn sid(&self) -> Result<String> {
        let b = self.bytes()?;
        if b.len() != 43 || !b.is_ascii() {
            return Err(Error::Integrity("wire2 SID"));
        }
        Ok(std::str::from_utf8(b)
            .map_err(|_| Error::Integrity("SID"))?
            .into())
    }
}
struct Cursor<'a> {
    raw: &'a [u8],
    pos: usize,
    items: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or(Error::Bound("wire2 length"))?;
        let b = self
            .raw
            .get(self.pos..end)
            .ok_or(Error::Integrity("wire2 truncated"))?;
        self.pos = end;
        Ok(b)
    }
    fn head(&mut self) -> Result<(u8, u64)> {
        let first = self.take(1)?[0];
        let extra = first & 31;
        let n = match extra {
            0..=23 => u64::from(extra),
            24 => u64::from(self.take(1)?[0]),
            25 => u64::from(u16::from_be_bytes(
                self.take(2)?
                    .try_into()
                    .map_err(|_| Error::Integrity("u16"))?,
            )),
            26 => u64::from(u32::from_be_bytes(
                self.take(4)?
                    .try_into()
                    .map_err(|_| Error::Integrity("u32"))?,
            )),
            27 => u64::from_be_bytes(
                self.take(8)?
                    .try_into()
                    .map_err(|_| Error::Integrity("u64"))?,
            ),
            _ => return Err(Error::Integrity("wire2 indefinite/head")),
        };
        if (extra == 24 && n < 24)
            || (extra == 25 && n <= 255)
            || (extra == 26 && n <= 65535)
            || (extra == 27 && n <= u64::from(u32::MAX))
        {
            return Err(Error::Integrity("wire2 noncanonical"));
        }
        Ok((first >> 5, n))
    }
    fn scan(&mut self, depth: usize) -> Result<()> {
        self.items += 1;
        if depth > 8 || self.items > 8192 {
            return Err(Error::Bound("wire2 depth/items"));
        }
        let (major, n) = self.head()?;
        match major {
            0 => {}
            2 => {
                self.take(usize::try_from(n).map_err(|_| Error::Bound("bstr length"))?)?;
            }
            4 => {
                if n > 8192 {
                    return Err(Error::Bound("wire2 count"));
                }
                for _ in 0..n {
                    self.scan(depth + 1)?;
                }
            }
            _ => return Err(Error::Integrity("wire2 profile")),
        }
        Ok(())
    }
    fn node(&mut self) -> Result<Node<'a>> {
        let start = self.pos;
        let (major, n) = self.head()?;
        let v = match major {
            0 => Value::U(n),
            2 => Value::B(self.take(usize::try_from(n).map_err(|_| Error::Bound("bstr length"))?)?),
            4 => {
                let mut v = Vec::new();
                for _ in 0..n {
                    v.push(self.node()?);
                }
                Value::A(v)
            }
            _ => return Err(Error::Integrity("wire2 profile")),
        };
        Ok(Node {
            v,
            start,
            end: self.pos,
        })
    }
}
fn read(raw: &[u8], cap: usize) -> Result<Node<'_>> {
    if raw.is_empty() || raw.len() > cap {
        return Err(Error::Bound("wire2 cap"));
    }
    let mut c = Cursor {
        raw,
        pos: 0,
        items: 0,
    };
    c.scan(0)?;
    if c.pos != raw.len() {
        return Err(Error::Integrity("wire2 trailing"));
    }
    c.pos = 0;
    c.node()
}
fn yes(ok: bool, why: &'static str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::Integrity(why))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Tail {
    pub g: u64,
    pub key: Id,
    pub index: u64,
    pub last: Id,
}
#[derive(Clone, Debug)]
pub(super) struct EventFields {
    #[cfg(test)]
    pub native_kind: u8,
    pub scope: HistoryScope,
    pub q: Id,
    pub role: u8,
    pub n: u64,
    pub previous: Option<Id>,
    pub sequence: u64,
    pub kind: u8,
    pub request: [u8; 16],
    pub parents: Vec<Id>,
    pub g: u64,
    pub index: u64,
    pub key: Id,
    pub parent: Option<Id>,
    pub tail: Option<Tail>,
    pub author: Id,
    pub id: Id,
    pub core_offset: u16,
    pub core_len: u16,
}
pub(super) fn read_event2(raw: &[u8]) -> Result<EventFields> {
    let v = read(raw, 16 * 1024)?;
    let f = v.fields(5)?;
    yes(f[0].uint()? == 2, "Event2 version")?;
    let c = f[1].fields(15)?;
    yes(f[1].end - f[1].start <= 2048, "Core2 cap")?;
    let h = f[2].fields(3)?;
    f[4].fixed::<64>()?;
    yes(
        c[0].uint()? == 2
            && c[5].uint()? <= 1
            && (1..=3).contains(&c[9].uint()?)
            && h[0].uint()? <= 1,
        "Core/header fields",
    )?;
    yes(
        !f[3].bytes()?.is_empty() && f[3].bytes()?.len() <= 12 * 1024,
        "Olm cap",
    )?;
    let role = c[5].uint()? as u8;
    let n = c[6].uint()?;
    let kind = c[9].uint()? as u8;
    let previous = c[7].id_opt()?;
    let parents = c[11]
        .list()?
        .iter()
        .map(Node::fixed)
        .collect::<Result<Vec<Id>>>()?;
    yes(
        parents.len() <= 2 && parents.windows(2).all(|p| p[0] < p[1]),
        "parents",
    )?;
    yes(
        if n == 0 {
            kind == role + 1 && previous.is_none() && parents.is_empty()
        } else {
            kind == 3 && previous.is_some()
        },
        "genesis shape",
    )?;
    yes(
        n <= 1 || previous.is_some_and(|id| parents.contains(&id)),
        "previous user parent",
    )?;
    let ex = c[12].fields(4)?;
    let index = h[2].uint()?;
    yes(
        ex[1].uint()? == index && (kind == 1) == (h[0].uint()? == 0),
        "outer header",
    )?;
    let tail = ex[3]
        .optional()?
        .map(|t| -> Result<Tail> {
            let t = t.fields(4)?;
            Ok(Tail {
                g: t[0].uint()?,
                key: t[1].fixed()?,
                index: t[2].uint()?,
                last: t[3].fixed()?,
            })
        })
        .transpose()?;
    Ok(EventFields {
        #[cfg(test)]
        native_kind: h[0].uint()? as u8,
        scope: HistoryScope {
            context: c[1].fixed()?,
            dialog: c[2].fixed()?,
            epoch: c[3].fixed()?,
            session_id: c[4].sid()?,
        },
        q: c[14].fixed()?,
        role,
        n,
        previous,
        sequence: c[8].uint()?,
        kind,
        request: c[10].fixed()?,
        parents,
        g: ex[0].uint()?,
        index,
        key: h[1].fixed()?,
        parent: ex[2].id_opt()?,
        tail,
        author: c[13].fixed()?,
        id: hash_parts(b"kg:od2:event-id\0", &[raw]),
        core_offset: u16::try_from(f[1].start).map_err(|_| Error::Bound("Core offset"))?,
        core_len: u16::try_from(f[1].end - f[1].start).map_err(|_| Error::Bound("Core length"))?,
    })
}
#[derive(Clone, Debug)]
pub(super) struct ReceiptFields {
    pub scope: HistoryScope,
    pub q: Id,
    pub format: u8,
    pub role: u8,
    pub n: u64,
    pub event: Id,
    pub cert: Id,
    pub auth: Id,
}
pub(super) fn read_receipt2(raw: &[u8]) -> Result<ReceiptFields> {
    let v = read(raw, 1024)?;
    let f = v.fields(2)?;
    let t = f[0].fields(12)?;
    f[1].fixed::<64>()?;
    yes(
        t[0].uint()? == 2
            && (1..=2).contains(&t[6].uint()?)
            && t[7].uint()? <= 1
            && t[8].uint()? > 0,
        "receipt fields",
    )?;
    Ok(ReceiptFields {
        scope: HistoryScope {
            context: t[1].fixed()?,
            dialog: t[2].fixed()?,
            epoch: t[3].fixed()?,
            session_id: t[4].sid()?,
        },
        q: t[5].fixed()?,
        format: t[6].uint()? as u8,
        role: t[7].uint()? as u8,
        n: t[8].uint()?,
        event: t[9].fixed()?,
        cert: t[10].fixed()?,
        auth: t[11].fixed()?,
    })
}
#[derive(Clone, Debug)]
pub(super) struct AuthFields {
    pub id: Id,
    pub cert: Id,
}
pub(super) fn read_auth_bundle(raw: &[u8]) -> Result<AuthFields> {
    let v = read(raw, 16 * 1024)?;
    let f = v.fields(3)?;
    yes(f[0].uint()? == 1, "bundle version")?;
    f[2].bytes()?;
    Ok(AuthFields {
        id: hash_parts(b"", &[raw]),
        cert: hash_parts(b"", &[f[1].bytes()?]),
    })
}

#[derive(Clone, Debug)]
pub(super) struct FreshFields {
    pub c: Id,
    pub certs: [Id; 2],
    pub q: Id,
    pub d: Id,
    pub e: Id,
    pub offer: Id,
    pub grant: Id,
    #[cfg(test)]
    pub transcript: Id,
}
pub(super) fn read_fresh_binding2(raw: &[u8]) -> Result<FreshFields> {
    let v = read(raw, 16 * 1024)?;
    let f = v.fields(6)?;
    yes(f[0].uint()? == 2 && f[1].uint()? == 0, "fresh proof tag")?;
    let c = f[2].fixed::<32>()?;
    let consent = read(f[3].bytes()?, 4096)?;
    let cf = consent.fields(3)?;
    cf[1].fixed::<64>()?;
    cf[2].fixed::<64>()?;
    let cb = cf[0].fields(10)?;
    yes(
        cb[0].uint()? == 2
            && cb[1].fixed::<32>()? == c
            && cb[4].uint()? == 1
            && cb[5].uint()? == 2
            && cb[6].fields(1)?[0].uint()? == 0
            && cb[7].uint()? == 1
            && cb[8].uint()? < cb[9].uint()?,
        "fresh consent fields",
    )?;
    let certs = [cb[2].fixed::<32>()?, cb[3].fixed::<32>()?];
    let offer_raw = f[4].bytes()?;
    let grant_raw = f[5].bytes()?;
    let offer = read(offer_raw, 32 * 1024)?;
    let grant = read(grant_raw, 32 * 1024)?;
    let of = offer.fields(4)?;
    let gf = grant.fields(4)?;
    of[3].fixed::<64>()?;
    gf[3].fixed::<64>()?;
    yes(
        of[0].uint()? == 2 && of[1].uint()? == 1 && gf[0].uint()? == 2 && gf[1].uint()? == 2,
        "selected Control kinds",
    )?;
    let oa = of[2].fields(8)?;
    let gb = gf[2].fields(8)?;
    for x in oa.iter().take(6).chain(gb.iter().take(6)) {
        x.fixed::<32>()?;
    }
    let oid = hash_parts(b"kg:od2:control-id\0", &[offer_raw]);
    let gid = hash_parts(b"kg:od2:control-id\0", &[grant_raw]);
    yes(
        oa[0].fixed::<32>()? == c
            && oa[1].fixed::<32>()? == certs[0]
            && oa[2].fixed::<32>()? == certs[1]
            && gb[0].fixed::<32>()? == c
            && gb[1].fixed::<32>()? == oid
            && oa[3].fixed::<32>()? == gb[2].fixed::<32>()?
            && oa[6].uint()? == 1
            && oa[7].uint()? == 2
            && gb[6].uint()? == 1
            && gb[7].uint()? == 2,
        "selected binding",
    )?;
    let mut epoch = vec![0x86, 2];
    for bytes in [&c[..], &certs[0], &certs[1], offer_raw, grant_raw] {
        push_bytes(&mut epoch, bytes);
    }
    let e = hash_parts(b"kg:od2:epoch\0", &[&epoch]);
    let mut domain = vec![0x87, 2, 1, 2];
    for bytes in [&c, &certs[0], &certs[1], &e] {
        push_bytes(&mut domain, bytes);
    }
    let d = hash_parts(b"kg:od2:domain\0", &[&domain]);
    Ok(FreshFields {
        c,
        certs,
        q: hash_parts(b"kg:od2:mode-certificate\0", &[raw]),
        d,
        e,
        offer: oid,
        grant: gid,
        #[cfg(test)]
        transcript: oa[3].fixed()?,
    })
}
fn push_bytes(out: &mut Vec<u8>, raw: &[u8]) {
    let n = raw.len();
    match n {
        0..=23 => out.push(0x40 | n as u8),
        24..=255 => out.extend([0x58, n as u8]),
        _ => {
            out.push(0x59);
            out.extend((n as u16).to_be_bytes());
        }
    }
    out.extend(raw);
}
