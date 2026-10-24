use super::{Crypto, Error, Id, Reader, Result, history_types::Identity};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

const HEADER: &[u8; 8] = b"KGH2\x01\x02\0\0";
pub(super) const CURSOR_BYTES: usize = 275;

/// Untrusted transport bytes until authenticated by history_page. No signing API.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryCursor([u8; CURSOR_BYTES]);
impl HistoryCursor {
    pub fn from_bytes(raw: &[u8]) -> Result<Self> {
        if raw.len() != CURSOR_BYTES || &raw[..8] != HEADER {
            return Err(Error::InvalidCursor);
        }
        Ok(Self(raw.try_into().map_err(|_| Error::InvalidCursor)?))
    }
    pub fn from_base64(text: &str) -> Result<Self> {
        if text.len() != 367 || !text.is_ascii() {
            return Err(Error::InvalidCursor);
        }
        let mut raw = [0; CURSOR_BYTES];
        if URL_SAFE_NO_PAD
            .decode_slice(text, &mut raw)
            .map_err(|_| Error::InvalidCursor)?
            != CURSOR_BYTES
        {
            return Err(Error::InvalidCursor);
        }
        let cursor = Self::from_bytes(&raw)?;
        if cursor.to_base64() != text {
            return Err(Error::InvalidCursor);
        }
        Ok(cursor)
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
    pub fn to_base64(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0)
    }
}

#[derive(Clone, Debug)]
pub(super) struct Position {
    pub frontiers: [u64; 2],
    pub next: [u64; 2],
}
fn tag(crypto: &Crypto, body: &[u8]) -> blake3::Hash {
    let mut h = blake3::Hasher::new_keyed(&crypto.cursor);
    h.update(b"kg:history2:cursor\0");
    h.update(body);
    h.finalize()
}
pub(super) fn seal(
    crypto: &Crypto,
    identity: &Identity,
    position: &Position,
) -> Result<HistoryCursor> {
    let mut body = Vec::with_capacity(CURSOR_BYTES);
    body.extend(HEADER);
    body.extend(identity.incarnation);
    body.extend(identity.view_id()?);
    let s = &identity.init.scope;
    for id in [s.context, s.dialog, s.epoch] {
        body.extend(id);
    }
    body.extend(s.session_id.as_bytes());
    for n in position.frontiers.into_iter().chain(position.next) {
        body.extend(n.to_be_bytes());
    }
    let mac = tag(crypto, &body);
    body.extend(mac.as_bytes());
    HistoryCursor::from_bytes(&body)
}
pub(super) fn authenticate(
    crypto: &Crypto,
    identity: &Identity,
    cursor: &HistoryCursor,
) -> Result<Position> {
    let raw = cursor.as_bytes();
    let supplied: Id = raw[243..].try_into().map_err(|_| Error::InvalidCursor)?;
    // blake3::Hash equality compares the full tag in constant time.
    if tag(crypto, &raw[..243]) != blake3::Hash::from_bytes(supplied) {
        return Err(Error::InvalidCursor);
    }
    let parsed = (|| {
        let mut r = Reader::new(&raw[..243]);
        let s = &identity.init.scope;
        if r.take(8)? != HEADER
            || r.id()? != identity.incarnation
            || r.id()? != identity.view_id()?
            || r.id()? != s.context
            || r.id()? != s.dialog
            || r.id()? != s.epoch
            || r.take(43)? != s.session_id.as_bytes()
        {
            return Err(Error::InvalidCursor);
        }
        let result = Position {
            frontiers: [r.u64()?, r.u64()?],
            next: [r.u64()?, r.u64()?],
        };
        r.end()?;
        Ok(result)
    })();
    parsed.map_err(|_| Error::InvalidCursor)
}
