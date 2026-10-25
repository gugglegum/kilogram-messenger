use serde::Serialize;

pub type Id = [u8; 32];
/// Readable storage text and adapter-owned Core/parent binding, not a replacement
/// for the engine's MAC, inner and signature validation.
pub struct ProjectionValue {
    pub text: String,
    pub binding: Vec<u8>,
}
impl ProjectionValue {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.text.len() > 4096 || self.binding.len() > 2304 {
            return Err(Error::Bound("projection fields"));
        }
        let mut out = vec![5];
        out.extend((self.text.len() as u16).to_be_bytes());
        out.extend(self.text.as_bytes());
        out.extend((self.binding.len() as u16).to_be_bytes());
        out.extend(&self.binding);
        Ok(out)
    }
    pub fn decode(raw: &[u8]) -> Result<Self> {
        profiled!(ProjectionDecode, Self::decode_fields(raw))
    }
    fn decode_fields(raw: &[u8]) -> Result<Self> {
        let mut r = Reader::new(raw);
        if r.byte()? != 5 {
            return Err(Error::Integrity("projection version"));
        }
        let len = usize::from(r.u16()?);
        if len > 4096 {
            return Err(Error::Integrity("projection text bound"));
        }
        let text = std::str::from_utf8(r.take(len)?)
            .map_err(|_| Error::Integrity("projection UTF8"))?
            .to_owned();
        let len = usize::from(r.u16()?);
        if len > 2304 {
            return Err(Error::Integrity("projection binding bound"));
        }
        let binding = r.take(len)?.to_vec();
        r.end()?;
        Ok(Self { text, binding })
    }
}
pub const MAX_VALUE: usize = 512 * 1024;
pub const MAX_PLAN: usize = 16 * 1024 * 1024;
pub const FRAME_CREDIT: u64 = 128 * 1024;
pub const TEMP_CREDIT: u64 = 64 * 1024;
pub const REPAIR_POOL: u64 = 64 * 1024 * 1024;
pub const HEADROOM: u64 = 256 * 1024 * 1024;
pub(crate) const ROOTS: usize = 64;
pub(crate) const NODE_CAP: usize = 256;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    State(#[from] crate::StateError),
    #[error("indexed database: {0}")]
    Database(String),
    #[error("indexed I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("integrity blocked: {0}")]
    Integrity(&'static str),
    #[error("indexed bound: {0}")]
    Bound(&'static str),
    #[error("CreationConflict")]
    CreationConflict,
    #[error("owner claim mismatch")]
    OwnerMismatch,
    #[error("stale generation")]
    StaleGeneration,
    #[error("immutable/key conflict")]
    Conflict,
    #[error("quota or funded credit exhausted")]
    Quota,
    #[error("ConfiguredCapacity requested={requested} quota={quota}")]
    ConfiguredCapacity { requested: u64, quota: u64 },
    #[error("reopen required before resolving commit outcome")]
    ReopenRequired,
    #[error("HistoryUnavailable")]
    HistoryUnavailable,
    #[error("ExistingStateRequiresMigration")]
    ExistingStateRequiresMigration,
    #[error("InvalidCursor")]
    InvalidCursor,
    #[error("SnapshotUnavailable")]
    SnapshotUnavailable,
    #[error("RuntimeWriterRequired")]
    RuntimeWriterRequired,
    #[error("UnsupportedRuntimeProfile")]
    UnsupportedRuntimeProfile,
    #[error("policy rejected before commit: {0}")]
    PolicyRejected(&'static str),
}
pub type Result<T> = std::result::Result<T, Error>;
pub(crate) fn db(error: impl std::fmt::Display) -> Error {
    Error::Database(error.to_string())
}

/// Fixed namespace discriminants are part of storage format5; ordinals are BE.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[repr(u8)]
pub enum Namespace {
    Event = 0,
    LaneA = 1,
    LaneB = 2,
    Projection = 3,
    Request = 4,
    Queue = 5,
    Undelivered = 6,
    ReceiptA = 7,
    ReceiptB = 8,
    Pending = 9,
    SeenA = 10,
    SeenB = 11,
    Auth = 12,
    Control = 13,
    Current = 14,
    Native = 15,
    Credit = 16,
    Author0 = 17,
    Author1 = 18,
    Author2 = 19,
    Admission = 20,
    Runtime = 21,
    RuntimeProfile = 22,
    Migration = 23,
    LegacyBudget = 24,
}
impl Namespace {
    pub(crate) fn mutable(self) -> bool {
        matches!(
            self,
            Self::Request
                | Self::Current
                | Self::Native
                | Self::Runtime
                | Self::RuntimeProfile
                | Self::Migration
                | Self::LegacyBudget
        )
    }
    pub(crate) fn numeric(self) -> bool {
        matches!(
            self,
            Self::LaneA
                | Self::LaneB
                | Self::Queue
                | Self::Undelivered
                | Self::ReceiptA
                | Self::ReceiptB
                | Self::Pending
                | Self::Author0
                | Self::Author1
                | Self::Author2
        )
    }
    pub(crate) fn global(self) -> bool {
        self.author() || self == Self::Migration
    }
    pub(crate) fn author(self) -> bool {
        matches!(self, Self::Author0 | Self::Author1 | Self::Author2)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
pub struct Key {
    pub context: u8,
    pub namespace: Namespace,
    pub bytes: Id,
}
impl Key {
    pub fn request(context: u8, id: [u8; 16]) -> Result<Self> {
        let mut bytes = [0; 32];
        bytes[..16].copy_from_slice(&id);
        Self::hash(context, Namespace::Request, bytes)
    }
    pub fn hash(context: u8, namespace: Namespace, bytes: Id) -> Result<Self> {
        if context > 1 || namespace.numeric() {
            return Err(Error::Bound("hash key"));
        }
        Ok(Self {
            context,
            namespace,
            bytes,
        })
    }
    pub fn ordinal(context: u8, namespace: Namespace, ordinal: u64) -> Result<Self> {
        if context > 1 || !namespace.numeric() {
            return Err(Error::Bound("ordinal key"));
        }
        let mut bytes = [0; 32];
        bytes[..8].copy_from_slice(&ordinal.to_be_bytes());
        Ok(Self {
            context,
            namespace,
            bytes,
        })
    }
    pub fn ordinal_value(self) -> Result<u64> {
        if !self.namespace.numeric() || self.bytes[8..] != [0; 24] {
            return Err(Error::Bound("noncanonical ordinal"));
        }
        Ok(u64::from_be_bytes(
            self.bytes[..8]
                .try_into()
                .map_err(|_| Error::Bound("ordinal"))?,
        ))
    }
    pub(crate) fn slot(self) -> Result<usize> {
        if self.context > 1 {
            return Err(Error::Bound("context"));
        }
        if self.namespace == Namespace::Migration {
            if self.context != 0 {
                return Err(Error::Bound("global migration owner"));
            }
            return Ok(51);
        }
        if self.namespace == Namespace::LegacyBudget {
            if self.bytes != [0; 32] {
                return Err(Error::Bound("legacy budget singleton"));
            }
            return Ok(usize::from(self.context) * 24 + 23);
        }
        if self.namespace == Namespace::RuntimeProfile && self.context != 0 {
            return Err(Error::Bound("runtime profile slot"));
        }
        if self.namespace.numeric() {
            self.ordinal_value()?;
        }
        if self.namespace == Namespace::Native && self.bytes != [0; 32] {
            return Err(Error::Bound("single native slot"));
        }
        if self.namespace == Namespace::Request && self.bytes[16..] != [0; 16] {
            return Err(Error::Bound("request128"));
        }
        Ok(if self.namespace.global() {
            48 + self.namespace as usize - 17
        } else {
            usize::from(self.context) * 24 + self.namespace as usize
        })
    }
}

/// No implicit identity generation or adoption: caller pins both account and claim.
pub struct CreationClaim {
    pub account: Id,
    pub claim: Id,
    pub signing_secret: zeroize::Zeroizing<Id>,
    pub encryption_secret: zeroize::Zeroizing<Id>,
    pub contexts: [Id; 2],
}
#[derive(Clone, Copy, Debug)]
pub struct ExpectedOwner {
    pub account: Id,
    pub claim: Id,
}

#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub logical_quota: [u64; 2],
    pub physical_quota: u64,
    /// Additional managed files outside DB; library key/fence/lock are counted once on open.
    pub retained_external_bytes: u64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            logical_quota: [16 * 1024 * 1024 * 1024; 2],
            physical_quota: 48 * 1024 * 1024 * 1024,
            retained_external_bytes: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Summary {
    pub digest: Id,
    pub count: u64,
    pub used: [u64; 2],
    pub reserved: [u64; 2],
}
impl Summary {
    pub fn encode(self, out: &mut Vec<u8>) {
        out.extend(self.digest);
        out.extend(self.count.to_be_bytes());
        for v in self.used.into_iter().chain(self.reserved) {
            out.extend(v.to_be_bytes());
        }
    }
    pub fn decode(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            digest: r.id()?,
            count: r.u64()?,
            used: [r.u64()?, r.u64()?],
            reserved: [r.u64()?, r.u64()?],
        })
    }
}

pub(crate) struct Reader<'a> {
    raw: &'a [u8],
    pos: usize,
}
impl<'a> Reader<'a> {
    pub fn new(raw: &'a [u8]) -> Self {
        Self { raw, pos: 0 }
    }
    pub fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(count)
            .ok_or(Error::Integrity("length"))?;
        let result = self
            .raw
            .get(self.pos..end)
            .ok_or(Error::Integrity("truncated"))?;
        self.pos = end;
        Ok(result)
    }
    pub fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn id(&mut self) -> Result<Id> {
        self.take(32)?
            .try_into()
            .map_err(|_| Error::Integrity("id"))
    }
    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_be_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| Error::Integrity("u64"))?,
        ))
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| Error::Integrity("u16"))?,
        ))
    }
    pub fn end(&self) -> Result<()> {
        if self.pos != self.raw.len() {
            return Err(Error::Integrity("trailing bytes"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
pub enum Mutation {
    Put {
        key: Key,
        value: Vec<u8>,
    },
    /// Raw retirement is rejected; use the typed CreditAction transitions.
    Retire {
        key: Key,
        replacement: Key,
    },
}
impl Mutation {
    pub fn put(key: Key, value: Vec<u8>) -> Self {
        Self::Put { key, value }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct CommitReport {
    pub generation: u64,
    pub commit_id: Id,
    pub encoded_plan_bytes: u64,
    pub nodes_read: u64,
    pub values_read: u64,
    pub changed_nodes: u64,
    /// Conservative app-owned live-buffer bound, from live overlay sizes plus input/plan budgets.
    pub app_buffer_bound_bytes: u64,
    pub duplicate: bool,
}
