//! Production runtime ownership. Component and runtime root codecs never downgrade.
//! Wire signatures, native MAC and current-time truth belong to the single owner.
use super::*;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
#[path = "runtime2_ops.rs"]
mod operations;
pub use operations::*;
#[path = "runtime2_read.rs"]
mod read;
pub use read::*;

const PROFILE_BYTES: usize = 64 * 1024;
const SPEC_BYTES: usize = 128 * 1024;
const HEAD_BYTES: usize = 8 * 1024;
const SELECTION_BYTES: usize = 256 * 1024;
const PRE_NATIVE_BYTES: usize = 256 * 1024;

/// The complete metadata capacity is prepaid through authenticated leaf reservations.
/// Encrypt only real bytes; unused capacity is funding, never unauthenticated padding.
fn packed<T: Serialize>(value: &T, size: usize) -> Result<Vec<u8>> {
    let raw = postcard::to_allocvec(value).map_err(|_| Error::Bound("runtime metadata codec"))?;
    if raw.len() > size.saturating_sub(8) || size > MAX_VALUE {
        return Err(Error::Bound("runtime metadata"));
    }
    let mut out = vec![0; raw.len() + 8];
    out[..4].copy_from_slice(b"RTM2");
    out[4..8].copy_from_slice(&(raw.len() as u32).to_be_bytes());
    out[8..8 + raw.len()].copy_from_slice(&raw);
    Ok(out)
}
fn unpack<T: DeserializeOwned>(raw: &[u8], size: usize) -> Result<T> {
    if raw.len() < 8 || raw.len() > size || raw.get(..4) != Some(b"RTM2") {
        return Err(Error::Integrity("runtime metadata envelope"));
    }
    let len = u32::from_be_bytes(
        raw[4..8]
            .try_into()
            .map_err(|_| Error::Integrity("runtime length"))?,
    ) as usize;
    let end = len
        .checked_add(8)
        .ok_or(Error::Integrity("runtime length"))?;
    if end != raw.len() {
        return Err(Error::Integrity("runtime metadata length"));
    }
    let (value, tail) = postcard::take_from_bytes(&raw[8..end])
        .map_err(|_| Error::Integrity("runtime metadata codec"))?;
    if !tail.is_empty() {
        return Err(Error::Integrity("runtime metadata trailing bytes"));
    }
    Ok(value)
}
fn profile_key() -> Result<Key> {
    Key::hash(
        0,
        Namespace::RuntimeProfile,
        *blake3::hash(b"kg:runtime2:profile/v1\0").as_bytes(),
    )
}
fn key(c: u8, name: &[u8]) -> Result<Key> {
    let mut h = blake3::Hasher::new();
    h.update(b"kg:runtime2:record/v1\0");
    h.update(name);
    Key::hash(c, Namespace::Runtime, *h.finalize().as_bytes())
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RuntimeEnrollment {
    pub device: Id,
    pub certificate: Vec<u8>,
    pub authority: Vec<u8>,
    pub authority_revision: u64,
}
impl RuntimeEnrollment {
    fn check(&self) -> Result<()> {
        if self.device == [0; 32]
            || self.certificate.is_empty()
            || self.certificate.len() > 16 * 1024
            || self.authority.is_empty()
            || self.authority.len() > 16 * 1024
        {
            return Err(Error::Bound("runtime enrollment"));
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct RuntimeProfile {
    version: u8,
    pub incarnation: Id,
    pub account: Id,
    pub claim: Id,
    pub contexts: [Id; 2],
    pub enrollment: RuntimeEnrollment,
    pub peer: Option<RuntimePeerPin>,
    pub policy_generation: u64,
    pub next_author_sequence: u64,
    pub wall_high_water: u64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct RuntimeContextSpec {
    pub role: u8,
    pub accounts: [Id; 2],
    pub certs: [Id; 2],
    pub credentials: [Vec<u8>; 2],
    pub membership: Vec<u8>,
    pub consent: Vec<u8>,
    pub own_locator: Option<Vec<u8>>,
    pub peer_locator: Option<Vec<u8>>,
    pub own_locator_revision: u64,
    pub peer_locator_revision: u64,
}
impl RuntimeContextSpec {
    fn check(&self, profile: &RuntimeProfile) -> Result<()> {
        if self.role > 1
            || self.accounts[usize::from(self.role)] != profile.account
            || self.certs[usize::from(self.role)]
                != *blake3::hash(&profile.enrollment.certificate).as_bytes()
            || self
                .credentials
                .iter()
                .any(|v| v.is_empty() || v.len() > 16 * 1024)
            || self.membership.is_empty()
            || self.membership.len() > 32 * 1024
            || self.consent.is_empty()
            || self.consent.len() > 4096
            || self.own_locator.as_ref().is_some_and(|v| v.len() > 8192)
            || self.peer_locator.as_ref().is_some_and(|v| v.len() > 8192)
        {
            return Err(Error::Bound("runtime context spec"));
        }
        Ok(())
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct RuntimeSelection {
    pub initial_pair: Vec<u8>,
    pub selected_consent: Vec<u8>,
    pub offer: Vec<u8>,
    pub grant: Vec<u8>,
    pub proof: Vec<u8>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct RuntimeHead {
    pub context: Id,
    pub role: u8,
    pub generation: u64,
    pub native_generation: u64,
    pub phase: u8,
    pub active: bool,
    pub offer: Option<Id>,
    pub grant: Option<Id>,
    pub dialog: Option<Id>,
    pub epoch: Option<Id>,
    pub sid: Option<String>,
    pub mode_ref: Option<Id>,
    pub native_hash: Option<Id>,
    pub ready: [Option<Vec<u8>>; 2],
    pub ack_next: u64,
    pub peer_source_next: u64,
    pub peer_source_last: Option<Id>,
    pub blocked: Option<String>,
}
impl RuntimeHead {
    fn new(context: Id, role: u8) -> Self {
        Self {
            context,
            role,
            generation: 0,
            native_generation: 0,
            phase: 0,
            active: false,
            offer: None,
            grant: None,
            dialog: None,
            epoch: None,
            sid: None,
            mode_ref: None,
            native_hash: None,
            ready: [None, None],
            ack_next: 1,
            peer_source_next: 0,
            peer_source_last: None,
            blocked: None,
        }
    }
    fn check(&self) -> Result<()> {
        let epoch = self.offer.is_some()
            && self.grant.is_some()
            && self.dialog.is_some()
            && self.epoch.is_some()
            && self.mode_ref.is_some();
        let no_epoch = self.grant.is_none()
            && self.dialog.is_none()
            && self.epoch.is_none()
            && self.sid.is_none()
            && self.mode_ref.is_none();
        let valid = match self.phase {
            0 => {
                self.offer.is_none()
                    && no_epoch
                    && self.native_hash.is_none()
                    && self.native_generation == 0
            }
            1 => {
                self.role == 0
                    && self.offer.is_some()
                    && no_epoch
                    && self.native_hash.is_some()
                    && self.native_generation == 1
            }
            2 => {
                self.role == 1
                    && epoch
                    && self.sid.is_none()
                    && self.native_hash.is_some()
                    && self.native_generation == 1
            }
            3 => {
                epoch
                    && self
                        .sid
                        .as_ref()
                        .is_some_and(|s| s.len() == 43 && s.is_ascii())
                    && self.native_hash.is_some()
                    && self.native_generation >= 2
            }
            _ => false,
        };
        if !valid
            || self.role > 1
            || self.ack_next == 0
            || self.active != self.ready.iter().all(Option::is_some)
            || (self.phase != 3 && self.ready.iter().any(Option::is_some))
            || ((self.peer_source_next == 0) != self.peer_source_last.is_none())
            || self.blocked.as_ref().is_some_and(|s| s.len() > 1024)
        {
            return Err(Error::Integrity("runtime phase/activation shape"));
        }
        Ok(())
    }
}
pub struct RuntimeOwnerMaterial {
    pub profile: RuntimeProfile,
    pub signing: Zeroizing<Id>,
    pub encryption: Zeroizing<Id>,
}
pub struct RuntimeSnapshot {
    pub root_generation: u64,
    pub profile: RuntimeProfile,
    pub spec: RuntimeContextSpec,
    pub head: RuntimeHead,
    pub selection: RuntimeSelection,
    pub pre_native: Option<Vec<u8>>,
}

/// Closed inputs; no caller-supplied key list, retention refs or commit digest.
#[derive(Clone, Serialize)]
pub enum RuntimeOperation {
    InstallContext {
        context: u8,
        spec: RuntimeContextSpec,
        peer_revision: u64,
    },
    PinAuthority {
        account: Id,
        revision: u64,
        raw: Vec<u8>,
        expected_policy: u64,
    },
    RenewConsent {
        context: u8,
        fence: RuntimeFence,
        expected_ref: Id,
        raw: Vec<u8>,
    },
    Locator {
        context: u8,
        fence: RuntimeFence,
        own: bool,
        raw: Vec<u8>,
    },
    PrepareOffer {
        context: u8,
        fence: RuntimeFence,
        selection: RuntimeSelection,
        private: Vec<u8>,
    },
    ReserveGrant {
        context: u8,
        fence: RuntimeFence,
        selection: RuntimeSelection,
        private: Vec<u8>,
    },
    HistoricalConsent {
        context: u8,
        fence: RuntimeFence,
        raw: Vec<u8>,
    },
    Genesis {
        context: u8,
        fence: RuntimeFence,
        selection: RuntimeSelection,
        steps: Vec<RuntimeGenesisStep>,
    },
    Ready {
        context: u8,
        fence: RuntimeFence,
        raw: Vec<u8>,
    },
    Queue {
        context: u8,
        fence: RuntimeFence,
        request: [u8; 16],
        text: String,
    },
    Materialize {
        context: u8,
        fence: RuntimeFence,
        rows: Vec<HistoryReadable2>,
        native: HistoryNativeTransition,
    },
    AcceptPending {
        context: u8,
        fence: RuntimeFence,
        source: Vec<u8>,
        head: bool,
        grant: Option<Id>,
        admission_author_ref: Id,
        auth: Vec<HistoryAuth>,
    },
    Drain {
        context: u8,
        fence: RuntimeFence,
        rows: Vec<HistoryReadable2>,
        native: HistoryNativeTransition,
    },
    Acknowledge {
        context: u8,
        fence: RuntimeFence,
        raw: Vec<u8>,
        auth: Vec<HistoryAuth>,
    },
    Repair {
        context: u8,
        fence: RuntimeFence,
        repair: HistoryRepair,
    },
    AckProgress {
        context: u8,
        fence: RuntimeFence,
        first: u64,
        ids: Vec<Id>,
    },
    PeerHead {
        context: u8,
        fence: RuntimeFence,
        next: u64,
        last: Option<Id>,
    },
    Block {
        context: u8,
        fence: RuntimeFence,
        reason: String,
        evidence: Vec<Vec<u8>>,
    },
}

/// Owns the exclusive profile lock; never exposes a writable component vault.
pub struct RuntimeRepository {
    pub(super) store: IndexedDialogVault,
    config: Config,
}
#[derive(Clone, Copy)]
pub enum RuntimeBootstrapPoint {
    Fenced,
    Staged,
}
pub struct RuntimeReopenFailure {
    pub error: Error,
    pub lease: StateDirectoryLock,
}
impl RuntimeRepository {
    pub fn create(
        lock: StateDirectoryLock,
        claim: CreationClaim,
        enrollment: RuntimeEnrollment,
        config: Config,
    ) -> Result<Self> {
        enrollment.check()?;
        let store = IndexedDialogVault::create_inner(lock, claim, config, Some(enrollment), None)?;
        let mut this = Self { store, config };
        this.profile()?;
        Ok(this)
    }
    pub fn create_with_hook(
        lock: StateDirectoryLock,
        claim: CreationClaim,
        enrollment: RuntimeEnrollment,
        config: Config,
        hook: &mut dyn FnMut(RuntimeBootstrapPoint) -> Result<()>,
    ) -> Result<Self> {
        enrollment.check()?;
        let store =
            IndexedDialogVault::create_inner(lock, claim, config, Some(enrollment), Some(hook))?;
        let mut this = Self { store, config };
        this.profile()?;
        Ok(this)
    }
    pub fn open(lock: StateDirectoryLock, owner: ExpectedOwner, config: Config) -> Result<Self> {
        let store = IndexedDialogVault::open_inner(lock, owner, config, RootLayout::Runtime6)?;
        let mut this = Self { store, config };
        this.profile()?;
        Ok(this)
    }
    /// Drop the poisoned backend and recover redb while retaining the same lifetime directory lock.
    pub fn reopen(self) -> std::result::Result<Self, RuntimeReopenFailure> {
        let config = self.config;
        let (lease, owner) = self.store.into_owner_lease();
        let loaded =
            match IndexedDialogVault::load_borrowed(&lease, owner, config, RootLayout::Runtime6) {
                Ok(loaded) => loaded,
                Err(error) => return Err(RuntimeReopenFailure { error, lease }),
            };
        let mut this = Self {
            store: IndexedDialogVault::from_loaded(lease, loaded),
            config,
        };
        if let Err(error) = this.profile() {
            let (lease, _) = this.store.into_owner_lease();
            return Err(RuntimeReopenFailure { error, lease });
        }
        Ok(this)
    }
    pub fn state_directory(&self) -> &std::path::Path {
        self.store._lease.root()
    }
    pub fn requires_reopen(&self) -> bool {
        self.store.poisoned
    }
    #[cfg(debug_assertions)]
    pub fn verification_io_fault(&self, point: &str, after: u64) -> Result<()> {
        let point = match point {
            "before_write" => "before_write",
            "after_write" => "after_write",
            "before_sync" => "before_sync",
            "after_sync" => "after_sync",
            "before_resize" => "before_resize",
            "after_resize" => "after_resize",
            "storage_full" => "storage_full",
            _ => return Err(Error::Bound("verification backend fault")),
        };
        self.store.backend.fail(point, after)
    }
    pub fn generation(&self) -> u64 {
        self.store.generation()
    }
    pub fn last_commit(&self) -> Result<Id> {
        self.store.last_commit()
    }
    pub fn credits(&self) -> Result<CreditReport> {
        self.store.credits()
    }
    pub fn backend_stats(&self) -> Result<BackendStats> {
        self.store.backend_stats()
    }
    pub fn profile(&mut self) -> Result<RuntimeProfile> {
        let raw = self
            .store
            .get(profile_key()?)?
            .ok_or(Error::Integrity("runtime marker missing"))?;
        let profile: RuntimeProfile = unpack(&raw, PROFILE_BYTES)?;
        profile.enrollment.check()?;
        if profile.version != 1
            || profile.incarnation != self.store.root.incarnation
            || profile.account != self.store.root.account
            || profile.claim != self.store.root.claim
            || profile.contexts != self.store.root.contexts
            || profile.policy_generation == 0
        {
            return Err(Error::Integrity("runtime marker binding"));
        }
        Ok(profile)
    }
    pub fn owner_material(&mut self) -> Result<RuntimeOwnerMaterial> {
        let profile = self.profile()?;
        Ok(RuntimeOwnerMaterial {
            profile,
            signing: Zeroizing::new(*self.store.root.signing),
            encryption: Zeroizing::new(*self.store.root.encryption),
        })
    }
    pub fn snapshot(&mut self, c: u8) -> Result<RuntimeSnapshot> {
        if c > 1 {
            return Err(Error::Bound("runtime context"));
        }
        if self.store.poisoned {
            return Err(Error::ReopenRequired);
        }
        // One immutable read transaction and its bounded verified-node reuse.
        // No metadata/cache entry crosses a transaction or a root generation.
        let tx = self.store.vault.database.begin_read().map_err(db)?;
        let nodes = tx.open_table(NODES).map_err(db)?;
        let values = tx.open_table(VALUES).map_err(db)?;
        let mut w = Work::new(&nodes, &values, &self.store.crypto, self.store.root.clone());
        w.tree.enable_read_reuse();
        let profile = w.runtime_profile()?;
        profile.enrollment.check()?;
        if profile.policy_generation == 0 {
            return Err(Error::Integrity("runtime marker binding"));
        }
        let mut read = |name: &[u8]| -> Result<Vec<u8>> {
            w.get(key(c, name)?)?
                .ok_or(Error::Integrity("runtime record missing"))
        };
        let spec: RuntimeContextSpec = unpack(&read(b"spec")?, SPEC_BYTES)?;
        spec.check(&profile)?;
        let head: RuntimeHead = unpack(&read(b"head")?, HEAD_BYTES)?;
        head.check()?;
        if c > 1 || head.context != profile.contexts[usize::from(c)] || head.role != spec.role {
            return Err(Error::Integrity("runtime head binding"));
        }
        let selection = unpack(&read(b"selection")?, SELECTION_BYTES)?;
        let pre_native = unpack(&read(b"pre-native")?, PRE_NATIVE_BYTES)?;
        Ok(RuntimeSnapshot {
            root_generation: self.generation(),
            profile,
            spec,
            head,
            selection,
            pre_native,
        })
    }
    pub fn commit(
        &mut self,
        expected: u64,
        commit_id: Id,
        operation: RuntimeOperation,
        gate: &mut dyn FnMut() -> Result<()>,
    ) -> Result<CommitReport> {
        operation.bound()?;
        let raw = Zeroizing::new(
            postcard::to_allocvec(&(expected, &operation))
                .map_err(|_| Error::Bound("runtime operation"))?,
        );
        if raw.len() > MAX_PLAN {
            return Err(Error::Bound("runtime input"));
        }
        let digest = self.store.crypto.hash(0, b"runtime2-operation/v1", &raw);
        self.store.commit_with_gate(
            expected,
            commit_id,
            digest,
            CommitInput::Runtime2(Box::new(operation)),
            raw.len() * 3 + 16 * 1024,
            Some(gate),
        )
    }
}
impl IndexedDialogVault {
    pub(super) fn bootstrap_runtime(
        &self,
        tx: &redb::WriteTransaction,
        enrollment: RuntimeEnrollment,
    ) -> Result<Root> {
        let nodes = tx.open_table(NODES).map_err(db)?;
        let values = tx.open_table(VALUES).map_err(db)?;
        let mut w = Work::new(&nodes, &values, &self.crypto, self.root.clone());
        let profile = RuntimeProfile {
            version: 1,
            incarnation: self.root.incarnation,
            account: self.root.account,
            claim: self.root.claim,
            contexts: self.root.contexts,
            enrollment,
            peer: None,
            policy_generation: 1,
            next_author_sequence: 0,
            wall_high_water: 0,
        };
        w.runtime_put_key(profile_key()?, &profile, PROFILE_BYTES)?;
        w.root.check()?;
        w.tree.seal_changes()?;
        let root = w.root;
        let changed_nodes = w.tree.changed;
        let changed = w.changed;
        drop(nodes);
        drop(values);
        {
            let mut table = tx.open_table(NODES).map_err(db)?;
            for (key, value) in changed_nodes {
                match value {
                    Some(value) => {
                        table.insert(key.as_slice(), value.as_slice()).map_err(db)?;
                    }
                    None => {
                        table.remove(key.as_slice()).map_err(db)?;
                    }
                }
            }
        }
        {
            let mut table = tx.open_table(VALUES).map_err(db)?;
            for (key, value) in changed {
                match value {
                    Some(value) => {
                        table.insert(key.as_slice(), value.as_slice()).map_err(db)?;
                    }
                    None => {
                        table.remove(key.as_slice()).map_err(db)?;
                    }
                }
            }
        }
        Ok(root)
    }
}
impl<
    'a,
    N: ReadableTable<&'static [u8], &'static [u8]>,
    V: ReadableTable<&'static [u8], &'static [u8]>,
> Work<'a, N, V>
{
    fn runtime_put_key<T: Serialize>(
        &mut self,
        key: Key,
        value: &T,
        capacity: usize,
    ) -> Result<()> {
        if !matches!(
            key.namespace,
            Namespace::Runtime | Namespace::RuntimeProfile
        ) || self.root.layout != RootLayout::Runtime6
        {
            return Err(Error::Bound("runtime metadata purpose"));
        }
        let slot = key.slot()?;
        let old = self
            .tree
            .find(slot as u8, self.root.roots[slot], key.bytes)?;
        if let Some(old) = &old {
            let tree::Body::Leaf {
                charge,
                reserved,
                length,
                ..
            } = old.body
            else {
                return Err(Error::Integrity("runtime metadata leaf"));
            };
            if charge.checked_add(reserved) != Some(capacity as u64 + 41 + 32 + 2 * NODE_CAP as u64)
                || length > capacity as u64
            {
                return Err(Error::Integrity("runtime metadata prepaid capacity"));
            }
        }
        let raw = packed(value, capacity)?;
        self.put(key, &raw, (capacity - raw.len()) as u64, false)?;
        if old.is_some() {
            // Growth is covered by this exact leaf's old reserved bytes. The
            // immutable total charge+reservation was checked above and remains fixed.
            self.new_charge.insert(key, 0);
        }
        Ok(())
    }
    fn runtime_put<T: Serialize>(
        &mut self,
        c: u8,
        name: &[u8],
        value: &T,
        capacity: usize,
    ) -> Result<()> {
        self.runtime_put_key(key(c, name)?, value, capacity)
    }
    fn runtime_profile(&mut self) -> Result<RuntimeProfile> {
        if self.root.layout != RootLayout::Runtime6 {
            return Err(Error::UnsupportedRuntimeProfile);
        }
        let raw = self
            .get(profile_key()?)?
            .ok_or(Error::Integrity("runtime marker missing"))?;
        let p: RuntimeProfile = unpack(&raw, PROFILE_BYTES)?;
        if p.version != 1
            || p.account != self.root.account
            || p.claim != self.root.claim
            || p.incarnation != self.root.incarnation
            || p.contexts != self.root.contexts
        {
            return Err(Error::Integrity("runtime marker binding"));
        }
        Ok(p)
    }
    pub(super) fn runtime_check(
        &mut self,
        operation: &RuntimeOperation,
        retry: bool,
    ) -> Result<()> {
        let profile = self.runtime_profile()?;
        match operation {
            RuntimeOperation::InstallContext { context, spec, .. } => {
                if *context > 1 {
                    return Err(Error::Bound("runtime context"));
                }
                spec.check(&profile)?;
                if retry {
                    let raw = self
                        .get(key(*context, b"spec")?)?
                        .ok_or(Error::Integrity("runtime retry spec"))?;
                    if raw != packed(spec, SPEC_BYTES)? {
                        return Err(Error::Conflict);
                    }
                    let expected = [
                        (
                            b"head".as_slice(),
                            packed(
                                &RuntimeHead::new(
                                    profile.contexts[usize::from(*context)],
                                    spec.role,
                                ),
                                HEAD_BYTES,
                            )?,
                        ),
                        (
                            b"selection",
                            packed(&RuntimeSelection::default(), SELECTION_BYTES)?,
                        ),
                        (
                            b"pre-native",
                            packed(&Option::<Vec<u8>>::None, PRE_NATIVE_BYTES)?,
                        ),
                        (b"quarantine", packed(&Vec::<Vec<u8>>::new(), 128 * 1024)?),
                        (
                            b"repair-evidence",
                            packed(&Option::<(Vec<u8>, Vec<u8>)>::None, 4096)?,
                        ),
                    ];
                    for (name, raw) in expected {
                        if self.get(key(*context, name)?)?.as_ref() != Some(&raw) {
                            return Err(Error::Integrity("runtime retry evidence"));
                        }
                    }
                }
            }
            other => self.runtime_check_operation(other, retry)?,
        }
        Ok(())
    }
    pub(super) fn runtime_apply(&mut self, operation: RuntimeOperation) -> Result<()> {
        let mut profile = self.runtime_profile()?;
        match operation {
            RuntimeOperation::InstallContext {
                context: c,
                spec,
                peer_revision,
            } => {
                if self.get(key(c, b"spec")?)?.is_some() {
                    return Err(Error::Conflict);
                }
                self.runtime_put(c, b"spec", &spec, SPEC_BYTES)?;
                self.runtime_put(
                    c,
                    b"head",
                    &RuntimeHead::new(self.root.contexts[usize::from(c)], spec.role),
                    HEAD_BYTES,
                )?;
                self.runtime_put(
                    c,
                    b"selection",
                    &RuntimeSelection::default(),
                    SELECTION_BYTES,
                )?;
                self.runtime_put(c, b"pre-native", &Option::<Vec<u8>>::None, PRE_NATIVE_BYTES)?;
                self.runtime_put(c, b"quarantine", &Vec::<Vec<u8>>::new(), 128 * 1024)?;
                self.runtime_put(
                    c,
                    b"repair-evidence",
                    &Option::<(Vec<u8>, Vec<u8>)>::None,
                    4096,
                )?;
                self.runtime_install_peer(&mut profile, &spec, peer_revision)?;
                profile.policy_generation = profile
                    .policy_generation
                    .checked_add(1)
                    .ok_or(Error::Bound("policy generation"))?;
                self.runtime_put_key(profile_key()?, &profile, PROFILE_BYTES)
            }
            other => self.runtime_apply_operation(other),
        }
    }
}

#[cfg(test)]
#[path = "runtime2_tests.rs"]
mod tests;
