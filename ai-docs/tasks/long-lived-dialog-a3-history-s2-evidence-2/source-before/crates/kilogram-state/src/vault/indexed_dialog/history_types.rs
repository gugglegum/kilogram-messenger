//! Trusted local adapter claims. These types do not verify wire/native cryptography.
use super::{Error, Id, Reader, Result};
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HistoryScope {
    pub context: Id,
    pub dialog: Id,
    pub epoch: Id,
    pub session_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HistoryInit {
    pub scope: HistoryScope,
    pub own_role: u8,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HistoryParent {
    pub event: Id,
    pub role: u8,
    pub ordinal: u64,
}

#[derive(Clone, Serialize)]
pub struct HistoryAuth {
    pub id: Id,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Serialize)]
pub struct HistoryCapture {
    pub original: HistoryAuth,
    pub admission: HistoryAuth,
}

#[derive(Clone, Serialize)]
pub struct HistorySeen {
    pub key: Id,
    pub generation: u64,
}

#[derive(Clone, Default, Serialize)]
pub struct HistoryExtras {
    pub auth: Vec<HistoryAuth>,
    pub seen: Option<HistorySeen>,
    /// Shared signer slot 0..2. The facade derives sequence, owner C and EventId.
    pub author_slot: Option<u8>,
}

/// Already verified owner/adapter input, never a substitute for wire verification.
#[derive(Clone, Serialize)]
pub struct PreparedReadable {
    pub scope: HistoryScope,
    pub format: u8,
    pub source: Vec<u8>,
    pub core_offset: u16,
    pub core_len: u16,
    pub kind: u8,
    pub role: u8,
    pub ordinal: u64,
    pub sequence: u64,
    pub request: [u8; 16],
    pub previous: Option<Id>,
    pub parents: Vec<HistoryParent>,
    pub text: String,
    pub extras: HistoryExtras,
}

impl PreparedReadable {
    pub fn event_id(&self) -> Result<Id> {
        source_id(self.format, &self.source)
    }
    pub(super) fn check_bound(&self) -> Result<()> {
        if self.source.len() > 16 * 1024 || self.text.len() > 4096 || self.parents.len() > 2 {
            return Err(Error::Bound("history row input"));
        }
        validate_scope(&self.scope)?;
        self.event_id()?;
        core_span(&self.source, self.core_offset, self.core_len)?;
        self.extras.check_bound()
    }
}

impl HistoryExtras {
    pub(super) fn check_bound(&self) -> Result<()> {
        if self.auth.len() > 2 || self.author_slot.is_some_and(|s| s > 2) {
            return Err(Error::Bound("history extras"));
        }
        for auth in &self.auth {
            auth.check_bound()?;
        }
        Ok(())
    }
}
impl HistoryAuth {
    pub(super) fn check_bound(&self) -> Result<()> {
        if self.bytes.len() > 16 * 1024 {
            return Err(Error::Bound("history auth"));
        }
        Ok(())
    }
}
impl HistoryCapture {
    pub(super) fn check_bound(&self) -> Result<()> {
        self.original.check_bound()?;
        self.admission.check_bound()
    }
}

/// The opaque blob is not decoded by storage. The adapter proves its native truth.
#[derive(Clone, Serialize)]
pub struct HistoryNativeTransition {
    pub before_hash: Option<Id>,
    pub after_blob: Vec<u8>,
    pub frame_ids: Vec<Id>,
}
impl HistoryNativeTransition {
    pub(super) fn check_bound(&self) -> Result<()> {
        if self.after_blob.is_empty()
            || self.after_blob.len() > 128 * 1024
            || self.frame_ids.len() > 4
        {
            return Err(Error::Bound("history native transition"));
        }
        Ok(())
    }
}

#[derive(Clone, Serialize)]
pub struct HistoryPending {
    pub scope: HistoryScope,
    pub format: u8,
    pub ordinal: u64,
    pub source: Vec<u8>,
    pub head: bool,
    pub grant: Option<Id>,
    pub capture: Option<HistoryCapture>,
}

#[derive(Clone, Serialize)]
pub enum HistoryRepair {
    Grant { id: Id, start: u64, end: u64 },
    CancelGrant { id: Id },
    Refill { bytes: u64 },
}

/// Closed purposes: no arbitrary keys, effects, credits, retention refs or digests.
#[derive(Clone, Serialize)]
pub enum HistoryOperation {
    Init(HistoryInit),
    Genesis {
        row: PreparedReadable,
        native: HistoryNativeTransition,
    },
    TextBatch {
        rows: Vec<PreparedReadable>,
        native: HistoryNativeTransition,
    },
    QueueAccept {
        request: [u8; 16],
        text: String,
    },
    AcceptPending(HistoryPending),
    CapturePending {
        ordinal: u64,
        event: Id,
        capture: HistoryCapture,
    },
    RepairCredit(HistoryRepair),
    Acknowledge {
        ordinal: u64,
        event: Id,
        auth: Vec<HistoryAuth>,
    },
}
impl HistoryOperation {
    pub(super) fn check_bound(&self) -> Result<()> {
        match self {
            Self::Init(init) => {
                validate_scope(&init.scope)?;
                if init.own_role > 1 {
                    return Err(Error::Bound("history own role"));
                }
            }
            Self::Genesis { row, native } => {
                row.check_bound()?;
                native.check_bound()?;
            }
            Self::TextBatch { rows, native } => {
                if rows.is_empty() || rows.len() > 4 {
                    return Err(Error::Bound("history batch"));
                }
                for row in rows {
                    row.check_bound()?;
                }
                native.check_bound()?;
            }
            Self::QueueAccept { text, .. } => {
                if text.len() > 4096 {
                    return Err(Error::Bound("history queue text"));
                }
            }
            Self::AcceptPending(p) => {
                validate_scope(&p.scope)?;
                if p.source.len() > 16 * 1024 || p.ordinal == 0 {
                    return Err(Error::Bound("history pending"));
                }
                source_id(p.format, &p.source)?;
                if let Some(c) = &p.capture {
                    c.check_bound()?;
                }
            }
            Self::CapturePending { capture, .. } => capture.check_bound()?,
            Self::Acknowledge { auth, .. } => {
                if auth.len() > 2 {
                    return Err(Error::Bound("history ack auth"));
                }
                for a in auth {
                    a.check_bound()?;
                }
            }
            Self::RepairCredit(_) => (),
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryRow {
    pub id: Id,
    pub role: u8,
    pub ordinal: u64,
    pub sequence: u64,
    pub request: [u8; 16],
    pub text: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HistoryReadStats {
    pub point_gets: u64,
    pub candidate_loads: u64,
    pub max_candidates: usize,
    pub nodes_read: u64,
    pub values_read: u64,
    pub returned_payload_bytes: usize,
    /// Conservative app-owned buffer bound, not allocator attribution/PrivateBytes.
    pub app_buffer_bound_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryRequestOutcome {
    pub queue_sequence: u64,
    pub event: Option<Id>,
}

pub(super) fn validate_scope(s: &HistoryScope) -> Result<()> {
    if s.session_id.len() != 43 || !s.session_id.is_ascii() {
        return Err(Error::Bound("history SID"));
    }
    Ok(())
}
pub(super) fn source_id(format: u8, raw: &[u8]) -> Result<Id> {
    let domain: &[u8] = match format {
        1 => b"kg:od1:event-id\0",
        2 => b"kg:od2:event-id\0",
        _ => return Err(Error::Integrity("history source format")),
    };
    if raw.is_empty() || raw.len() > 16 * 1024 {
        return Err(Error::Bound("history source"));
    }
    Ok(hash_parts(domain, &[raw]))
}
pub(super) fn hash_parts(domain: &[u8], parts: &[&[u8]]) -> Id {
    let mut h = blake3::Hasher::new();
    h.update(domain);
    for part in parts {
        h.update(part);
    }
    *h.finalize().as_bytes()
}
pub(super) fn core_span(raw: &[u8], offset: u16, len: u16) -> Result<&[u8]> {
    if !(1..=2048).contains(&len) {
        return Err(Error::Integrity("history Core length"));
    }
    raw.get(usize::from(offset)..usize::from(offset) + usize::from(len))
        .ok_or(Error::Integrity("history Core span"))
}
pub(super) fn put_optional(out: &mut Vec<u8>, id: Option<Id>) {
    out.push(u8::from(id.is_some()));
    out.extend(id.unwrap_or([0; 32]));
}
pub(super) fn read_optional(r: &mut Reader<'_>) -> Result<Option<Id>> {
    let flag = r.byte()?;
    let id = r.id()?;
    match flag {
        0 if id == [0; 32] => Ok(None),
        1 => Ok(Some(id)),
        _ => Err(Error::Integrity("history optional ID")),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Identity {
    pub incarnation: Id,
    pub init: HistoryInit,
}
impl Identity {
    pub fn encode(&self) -> Result<Vec<u8>> {
        validate_scope(&self.init.scope)?;
        if self.init.own_role > 1 {
            return Err(Error::Integrity("history role"));
        }
        let s = &self.init.scope;
        let mut out = b"HVI1".to_vec();
        for id in [self.incarnation, s.context, s.dialog, s.epoch] {
            out.extend(id);
        }
        out.extend(s.session_id.as_bytes());
        out.extend([self.init.own_role, 2]);
        Ok(out)
    }
    pub fn decode(raw: &[u8]) -> Result<Self> {
        let mut r = Reader::new(raw);
        if r.take(4)? != b"HVI1" {
            return Err(Error::Integrity("history identity codec"));
        }
        let incarnation = r.id()?;
        let scope = HistoryScope {
            context: r.id()?,
            dialog: r.id()?,
            epoch: r.id()?,
            session_id: std::str::from_utf8(r.take(43)?)
                .map_err(|_| Error::Integrity("history SID UTF8"))?
                .to_owned(),
        };
        let own_role = r.byte()?;
        if r.byte()? != 2 {
            return Err(Error::Integrity("history view semantics"));
        }
        r.end()?;
        let result = Self {
            incarnation,
            init: HistoryInit { scope, own_role },
        };
        if result
            .encode()
            .map_err(|_| Error::Integrity("history identity fields"))?
            != raw
        {
            return Err(Error::Integrity("history identity canonical"));
        }
        Ok(result)
    }
    pub fn view_id(&self) -> Result<Id> {
        Ok(hash_parts(b"kg:history2:view\0", &[&self.encode()?]))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Heads {
    pub view: Id,
    pub frontiers: [u64; 2],
    pub tails: [Option<Id>; 2],
}
impl Heads {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = b"HHD1".to_vec();
        out.extend(self.view);
        for n in self.frontiers {
            out.extend(n.to_be_bytes());
        }
        for id in self.tails {
            put_optional(&mut out, id);
        }
        out
    }
    pub fn decode(raw: &[u8]) -> Result<Self> {
        let mut r = Reader::new(raw);
        if r.take(4)? != b"HHD1" {
            return Err(Error::Integrity("history heads codec"));
        }
        let result = Self {
            view: r.id()?,
            frontiers: [r.u64()?, r.u64()?],
            tails: [read_optional(&mut r)?, read_optional(&mut r)?],
        };
        r.end()?;
        if result.frontiers.contains(&0) {
            return Err(Error::Integrity("history frontier"));
        }
        Ok(result)
    }
}

#[derive(Clone, Debug)]
pub(super) struct Binding {
    pub format: u8,
    pub view: Id,
    pub kind: u8,
    pub role: u8,
    pub ordinal: u64,
    pub sequence: u64,
    pub request: [u8; 16],
    pub previous: Option<Id>,
    pub core_offset: u16,
    pub core_len: u16,
    pub core_hash: Id,
    pub source_hash: Id,
    pub parents: Vec<HistoryParent>,
}
impl Binding {
    pub fn check(&self) -> Result<()> {
        if !(1..=2).contains(&self.format)
            || self.role > 1
            || self.parents.len() > 2
            || !(1..=2048).contains(&self.core_len)
        {
            return Err(Error::Integrity("history binding fields"));
        }
        if self.kind == 3 {
            if self.ordinal == 0 || self.previous.is_none() {
                return Err(Error::Integrity("history Text"));
            }
        } else if !((self.kind == 1 && self.role == 0) || (self.kind == 2 && self.role == 1))
            || self.ordinal != 0
            || self.request != [0; 16]
            || self.previous.is_some()
            || !self.parents.is_empty()
        {
            return Err(Error::Integrity("history genesis"));
        }
        let mut previous = None;
        for parent in &self.parents {
            if parent.role > 1
                || parent.ordinal == 0
                || previous.is_some_and(|id| id >= parent.event)
            {
                return Err(Error::Integrity("history parents"));
            }
            previous = Some(parent.event);
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.check()?;
        let mut out = b"HPB1".to_vec();
        out.push(self.format);
        out.extend(self.view);
        out.extend([self.kind, self.role]);
        out.extend(self.ordinal.to_be_bytes());
        out.extend(self.sequence.to_be_bytes());
        out.extend(self.request);
        put_optional(&mut out, self.previous);
        out.extend(self.core_offset.to_be_bytes());
        out.extend(self.core_len.to_be_bytes());
        out.extend(self.core_hash);
        out.extend(self.source_hash);
        out.push(self.parents.len() as u8);
        for p in &self.parents {
            out.extend(p.event);
            out.push(p.role);
            out.extend(p.ordinal.to_be_bytes());
        }
        Ok(out)
    }
    pub fn decode(raw: &[u8]) -> Result<Self> {
        let mut r = Reader::new(raw);
        if r.take(4)? != b"HPB1" {
            return Err(Error::Integrity("history binding version"));
        }
        let format = r.byte()?;
        let view = r.id()?;
        let kind = r.byte()?;
        let role = r.byte()?;
        let ordinal = r.u64()?;
        let sequence = r.u64()?;
        let request = r
            .take(16)?
            .try_into()
            .map_err(|_| Error::Integrity("history request"))?;
        let previous = read_optional(&mut r)?;
        let core_offset = r.u16()?;
        let core_len = r.u16()?;
        let core_hash = r.id()?;
        let source_hash = r.id()?;
        let count = r.byte()?;
        if count > 2 {
            return Err(Error::Integrity("history parent count"));
        }
        let mut parents = Vec::with_capacity(usize::from(count));
        for _ in 0..count {
            parents.push(HistoryParent {
                event: r.id()?,
                role: r.byte()?,
                ordinal: r.u64()?,
            });
        }
        r.end()?;
        let result = Self {
            format,
            view,
            kind,
            role,
            ordinal,
            sequence,
            request,
            previous,
            core_offset,
            core_len,
            core_hash,
            source_hash,
            parents,
        };
        result.check()?;
        Ok(result)
    }
    pub fn verify_source(&self, id: Id, source: &[u8]) -> Result<()> {
        if source_id(self.format, source)? != id
            || hash_parts(b"kg:history2:source\0", &[&[self.format], source]) != self.source_hash
            || hash_parts(
                b"kg:history2:core\0",
                &[
                    &[self.format],
                    core_span(source, self.core_offset, self.core_len)?,
                ],
            ) != self.core_hash
        {
            return Err(Error::Integrity("history source/Core binding"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(super) struct RequestRecord {
    pub queue: u64,
    pub request: [u8; 16],
    pub body_hash: Id,
    pub event: Option<Id>,
}
impl RequestRecord {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = b"HRQ1".to_vec();
        out.push(u8::from(self.event.is_some()));
        out.extend(self.queue.to_be_bytes());
        out.extend(self.request);
        out.extend(self.body_hash);
        put_optional(&mut out, self.event);
        out
    }
    pub fn decode(raw: &[u8]) -> Result<Self> {
        let mut r = Reader::new(raw);
        if r.take(4)? != b"HRQ1" {
            return Err(Error::Integrity("history Request codec"));
        }
        let state = r.byte()?;
        let queue = r.u64()?;
        let request = r
            .take(16)?
            .try_into()
            .map_err(|_| Error::Integrity("history Request ID"))?;
        let body_hash = r.id()?;
        let event = read_optional(&mut r)?;
        r.end()?;
        if state != u8::from(event.is_some()) {
            return Err(Error::Integrity("history Request state"));
        }
        Ok(Self {
            queue,
            request,
            body_hash,
            event,
        })
    }
}
