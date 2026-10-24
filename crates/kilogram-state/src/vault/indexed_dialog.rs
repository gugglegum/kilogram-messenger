//! A3-S1 library boundary. No wire, enrollment, native engine or live migration entrypoint.
#[path = "indexed_dialog/backend.rs"]
mod backend;
#[path = "indexed_dialog/credits.rs"]
mod credits;
#[path = "authenticated_index.rs"]
mod tree;
#[path = "indexed_dialog/types.rs"]
mod types;
use super::{
    EncryptedStateVault, MANIFEST_KEY, META_TABLE, ORDERED_ACCOUNT_FENCE, STATE_VAULT_FILE,
    STATE_VAULT_KEY_FILE, VaultManifest,
};
use crate::{
    StateDirectoryLock,
    key_provider::{load_master_key, load_or_create_master_key},
};
pub use backend::BackendStats;
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, Payload},
};
pub use credits::{CreditAction, CreditReport, PendingView};
use redb::{Database, Durability, ReadableDatabase, ReadableTable, TableDefinition};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};
pub use types::*;
use types::{NODE_CAP, ROOTS, Reader, Summary, db};
use zeroize::Zeroizing;

const NODES: TableDefinition<&[u8], &[u8]> = TableDefinition::new("od2-nodes-v5");
const VALUES: TableDefinition<&[u8], &[u8]> = TableDefinition::new("od2-values-v5");
pub(super) const ANCHOR: &str = "od2-root-v5";

// Conservative recovery fence: never run an old filesystem journal over schema5.
pub(crate) fn has_indexed_anchor(
    root: &std::path::Path,
) -> std::result::Result<bool, crate::StateError> {
    if !super::ordered::has_ordered_fence(root)? {
        return Ok(false);
    }
    let path = root.join(STATE_VAULT_FILE);
    if !crate::path_exists(&path)? {
        return Ok(false);
    }
    crate::reject_symlink(&path)?;
    let mut builder = Database::builder();
    builder.set_cache_size(64 * 1024 * 1024);
    let database = builder.open(&path).map_err(super::vault_database_error)?;
    let tx = database.begin_read().map_err(super::vault_database_error)?;
    let table = match tx.open_table(META_TABLE) {
        Ok(t) => t,
        Err(redb::TableError::TableDoesNotExist(_)) => return Ok(false),
        Err(e) => return Err(super::vault_database_error(e)),
    };
    if table
        .get(ANCHOR)
        .map_err(super::vault_database_error)?
        .is_some()
    {
        return Ok(true);
    }
    if let Some(raw) = table
        .get(MANIFEST_KEY)
        .map_err(super::vault_database_error)?
    {
        let manifest: VaultManifest = postcard::from_bytes(raw.value())
            .map_err(|_| crate::StateError::VaultManifestMismatch)?;
        return Ok(manifest.schema_version == 5);
    }
    Ok(false)
}

struct Crypto {
    node: Zeroizing<Id>,
    value: Zeroizing<Id>,
    integrity: Zeroizing<Id>,
    lookup: Zeroizing<Id>,
    incarnation: Id,
    contexts: [Id; 2],
    account: Id,
}
impl Crypto {
    fn new(master: &Id, incarnation: Id, contexts: [Id; 2], account: Id) -> Self {
        Self {
            node: Zeroizing::new(blake3::derive_key("kilogram a3 v5 node encryption", master)),
            value: Zeroizing::new(blake3::derive_key(
                "kilogram a3 v5 value encryption",
                master,
            )),
            integrity: Zeroizing::new(blake3::derive_key("kilogram a3 v5 integrity", master)),
            lookup: Zeroizing::new(blake3::derive_key("kilogram a3 v5 lookup", master)),
            incarnation,
            contexts,
            account,
        }
    }
    fn aad(&self, scope: u8, kind: &[u8], address: Id) -> Vec<u8> {
        let mut aad = b"kilogram:a3:v5\0".to_vec();
        aad.extend(self.incarnation);
        aad.push(scope);
        aad.extend(if scope < 48 {
            self.contexts[usize::from(scope) / 24]
        } else {
            self.account
        });
        aad.extend((kind.len() as u16).to_be_bytes());
        aad.extend(kind);
        aad.extend(address);
        aad
    }
    fn address(&self, scope: u8, kind: &[u8], key: &[u8]) -> Id {
        let mut h = blake3::Hasher::new_keyed(&self.lookup);
        h.update(&self.aad(scope, kind, [0; 32]));
        h.update(key);
        *h.finalize().as_bytes()
    }
    fn hash(&self, scope: u8, kind: &[u8], bytes: &[u8]) -> Id {
        let mut h = blake3::Hasher::new_keyed(&self.integrity);
        h.update(&self.aad(scope, kind, [0; 32]));
        h.update(bytes);
        *h.finalize().as_bytes()
    }
    fn seal_node(&self, scope: u8, address: Id, plain: &[u8]) -> Result<Vec<u8>> {
        seal(&self.node, &self.aad(scope, b"node", address), plain)
    }
    fn open_node(&self, scope: u8, address: Id, raw: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        open(
            &self.node,
            &self.aad(scope, b"node", address),
            raw,
            NODE_CAP,
        )
    }
}
fn seal(key: &Id, aad: &[u8], plain: &[u8]) -> Result<Vec<u8>> {
    let mut nonce = [0; 24];
    getrandom::fill(&mut nonce).map_err(|_| Error::Bound("secure random"))?;
    let cipher = XChaCha20Poly1305::new(key.into());
    let encrypted = cipher
        .encrypt(XNonce::from_slice(&nonce), Payload { msg: plain, aad })
        .map_err(|_| Error::Integrity("encrypt"))?;
    let mut out = Vec::with_capacity(25 + encrypted.len());
    out.push(5);
    out.extend(nonce);
    out.extend(encrypted);
    Ok(out)
}
fn open(key: &Id, aad: &[u8], raw: &[u8], cap: usize) -> Result<Zeroizing<Vec<u8>>> {
    if raw.len() < 41 || raw.len() > cap || raw[0] != 5 {
        return Err(Error::Integrity("envelope bound/version"));
    }
    let cipher = XChaCha20Poly1305::new(key.into());
    cipher
        .decrypt(
            XNonce::from_slice(&raw[1..25]),
            Payload {
                msg: &raw[25..],
                aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| Error::Integrity("AEAD"))
}

#[derive(Clone)]
struct Root {
    incarnation: Id,
    account: Id,
    claim: Id,
    signing: Zeroizing<Id>,
    encryption: Zeroizing<Id>,
    contexts: [Id; 2],
    generation: u64,
    commit_id: Id,
    request_digest: Id,
    roots: [Summary; ROOTS],
    ledger: [credits::Ledger; 2],
}
impl Root {
    fn encode(&self) -> Zeroizing<Vec<u8>> {
        let mut out = Zeroizing::new(Vec::with_capacity(8192));
        out.push(5);
        for id in [
            self.incarnation,
            self.account,
            self.claim,
            *self.signing,
            *self.encryption,
            self.contexts[0],
            self.contexts[1],
        ] {
            out.extend(id);
        }
        out.extend(self.generation.to_be_bytes());
        out.extend(self.commit_id);
        out.extend(self.request_digest);
        for summary in self.roots {
            summary.encode(&mut out);
        }
        for ledger in &self.ledger {
            ledger.encode(&mut out);
        }
        out
    }
    fn decode(raw: &[u8]) -> Result<Self> {
        if raw.len() > 32 * 1024 {
            return Err(Error::Integrity("anchor cap"));
        }
        let mut r = Reader::new(raw);
        if r.byte()? != 5 {
            return Err(Error::Integrity("anchor version"));
        }
        let incarnation = r.id()?;
        let account = r.id()?;
        let claim = r.id()?;
        let signing = Zeroizing::new(r.id()?);
        let encryption = Zeroizing::new(r.id()?);
        let contexts = [r.id()?, r.id()?];
        let generation = r.u64()?;
        let commit_id = r.id()?;
        let request_digest = r.id()?;
        let mut roots = [Summary::default(); ROOTS];
        for s in &mut roots {
            *s = Summary::decode(&mut r)?;
            if s.count == 0 && *s != Summary::default() {
                return Err(Error::Integrity("empty root"));
            }
        }
        let ledger = [
            credits::Ledger::decode(&mut r)?,
            credits::Ledger::decode(&mut r)?,
        ];
        r.end()?;
        let result = Self {
            incarnation,
            account,
            claim,
            signing,
            encryption,
            contexts,
            generation,
            commit_id,
            request_digest,
            roots,
            ledger,
        };
        result.check()?;
        Ok(result)
    }
    fn totals(&self) -> Result<([u64; 2], [u64; 2])> {
        let mut used = [0u64; 2];
        let mut reserved = [0u64; 2];
        for summary in self.roots {
            for c in 0..2 {
                used[c] = used[c]
                    .checked_add(summary.used[c])
                    .ok_or(Error::Bound("used total"))?;
                reserved[c] = reserved[c]
                    .checked_add(summary.reserved[c])
                    .ok_or(Error::Bound("reserved total"))?;
            }
        }
        Ok((used, reserved))
    }
    fn check(&self) -> Result<()> {
        if self.contexts[0] == self.contexts[1]
            || self.account == [0; 32]
            || self.claim == [0; 32]
            || self.generation == 0
        {
            return Err(Error::Integrity("anchor identity/generation"));
        }
        let (used, reserved) = self.totals()?;
        for c in 0..2 {
            self.ledger[c].check(used[c], reserved[c])?;
            let control = self.roots[c * 24 + Namespace::Control as usize].used[c]
                .checked_add(self.roots[c * 24 + Namespace::Current as usize].used[c])
                .and_then(|v| {
                    v.checked_add(self.roots[c * 24 + Namespace::Native as usize].used[c])
                })
                .ok_or(Error::Bound("control budget"))?;
            if control > 4 * 1024 * 1024 {
                return Err(Error::Quota);
            }
        }
        Ok(())
    }
}

pub struct IndexedDialogVault {
    vault: EncryptedStateVault,
    _lease: StateDirectoryLock,
    crypto: Crypto,
    root: Root,
    backend: backend::BackendControl,
    poisoned: bool,
}
impl IndexedDialogVault {
    pub fn create(lease: StateDirectoryLock, claim: CreationClaim, config: Config) -> Result<Self> {
        for entry in fs::read_dir(lease.root())? {
            if entry?.file_name() != crate::LOCK_FILE {
                return Err(Error::CreationConflict);
            }
        }
        if claim.account == [0; 32]
            || claim.claim == [0; 32]
            || claim.contexts[0] == claim.contexts[1]
        {
            return Err(Error::Bound("explicit owner/context claim"));
        }
        fs::create_dir_all(lease.root().join("ratchet/sessions"))?;
        crate::write_staged_file(
            &lease.root().join("ratchet/account.pickle"),
            ORDERED_ACCOUNT_FENCE,
        )?;
        let key = load_or_create_master_key(&lease.root().join(STATE_VAULT_KEY_FILE))?;
        let config = account_external(lease.root(), config)?;
        let (backend, control) =
            backend::Backend::open(&lease.root().join(STATE_VAULT_FILE), true, config)?;
        let mut builder = Database::builder();
        builder.set_cache_size(64 * 1024 * 1024);
        let database = builder.create_with_backend(backend).map_err(db)?;
        let mut incarnation = [0; 32];
        getrandom::fill(&mut incarnation).map_err(|_| Error::Bound("random"))?;
        let root = Root {
            incarnation,
            account: claim.account,
            claim: claim.claim,
            signing: claim.signing_secret,
            encryption: claim.encryption_secret,
            contexts: claim.contexts,
            generation: 1,
            commit_id: [0; 32],
            request_digest: [0; 32],
            roots: [Summary::default(); ROOTS],
            ledger: [
                credits::Ledger::new(config.logical_quota[0])?,
                credits::Ledger::new(config.logical_quota[1])?,
            ],
        };
        root.check()?;
        let crypto = Crypto::new(&key.master_key.0, incarnation, root.contexts, root.account);
        let vault = EncryptedStateVault {
            root: lease.root().to_path_buf(),
            database,
            master_key: key.master_key,
            key_protection: key.protection,
            key_load_outcome: key.load_outcome,
        };
        let result = Self {
            vault,
            _lease: lease,
            crypto,
            root,
            backend: control,
            poisoned: false,
        };
        let mut tx = result.vault.database.begin_write().map_err(db)?;
        tx.set_durability(Durability::Immediate).map_err(db)?;
        {
            tx.open_table(NODES).map_err(db)?;
            tx.open_table(VALUES).map_err(db)?;
        }
        result.write_anchor(&tx, &result.root)?;
        tx.commit().map_err(db)?;
        Ok(result)
    }
    pub fn open(lease: StateDirectoryLock, owner: ExpectedOwner, config: Config) -> Result<Self> {
        if !super::ordered::has_ordered_fence(lease.root())? {
            return Err(Error::OwnerMismatch);
        }
        crate::reject_symlink(&lease.root().join(STATE_VAULT_FILE))?;
        let key = load_master_key(&lease.root().join(STATE_VAULT_KEY_FILE))?;
        let config = account_external(lease.root(), config)?;
        let (backend, control) =
            backend::Backend::open(&lease.root().join(STATE_VAULT_FILE), false, config)?;
        let mut builder = Database::builder();
        builder.set_cache_size(64 * 1024 * 1024);
        let database = builder.create_with_backend(backend).map_err(db)?;
        let root = {
            let tx = database.begin_read().map_err(db)?;
            let meta = tx.open_table(META_TABLE).map_err(db)?;
            let manifest = meta
                .get(MANIFEST_KEY)
                .map_err(db)?
                .ok_or(Error::Integrity("manifest missing"))?;
            let manifest: VaultManifest = postcard::from_bytes(manifest.value())
                .map_err(|_| Error::Integrity("schema sentinel"))?;
            if manifest.schema_version != 5
                || manifest.record_count != 0
                || manifest.plaintext_bytes != 0
            {
                return Err(Error::OwnerMismatch);
            }
            let anchor = meta
                .get(ANCHOR)
                .map_err(db)?
                .ok_or(Error::Integrity("anchor missing"))?;
            let raw = anchor.value();
            if raw.len() < 32 + 41 || raw.len() > 32 * 1024 {
                return Err(Error::Integrity("anchor envelope"));
            }
            if *blake3::hash(raw).as_bytes() != manifest.snapshot_id {
                return Err(Error::Integrity("sentinel digest"));
            }
            let incarnation: Id = raw[..32]
                .try_into()
                .map_err(|_| Error::Integrity("incarnation"))?;
            let derived = Zeroizing::new(blake3::derive_key(
                "kilogram a3 v5 value encryption",
                &key.master_key.0,
            ));
            let plain = open(&derived, &root_aad(incarnation), &raw[32..], 32 * 1024)?;
            let root = Root::decode(&plain)?;
            if root.incarnation != incarnation {
                return Err(Error::Integrity("incarnation binding"));
            }
            root
        };
        if root.account != owner.account || root.claim != owner.claim {
            return Err(Error::OwnerMismatch);
        }
        // Configuration cannot confiscate previously committed obligations.
        let mut checked = root.clone();
        for c in 0..2 {
            checked.ledger[c].quota = config.logical_quota[c];
        }
        checked.check()?;
        let root = checked;
        let crypto = Crypto::new(
            &key.master_key.0,
            root.incarnation,
            root.contexts,
            root.account,
        );
        let vault = EncryptedStateVault {
            root: lease.root().to_path_buf(),
            database,
            master_key: key.master_key,
            key_protection: key.protection,
            key_load_outcome: key.load_outcome,
        };
        Ok(Self {
            vault,
            _lease: lease,
            crypto,
            root,
            backend: control,
            poisoned: false,
        })
    }
    fn write_anchor(&self, tx: &redb::WriteTransaction, root: &Root) -> Result<usize> {
        let (raw, sentinel) = self.anchor_bytes(root)?;
        let length = raw.len() + sentinel.len();
        let mut table = tx.open_table(META_TABLE).map_err(db)?;
        table.insert(ANCHOR, raw.as_slice()).map_err(db)?;
        table
            .insert(MANIFEST_KEY, sentinel.as_slice())
            .map_err(db)?;
        Ok(length)
    }
    fn anchor_bytes(&self, root: &Root) -> Result<(Vec<u8>, Vec<u8>)> {
        let mut raw = root.incarnation.to_vec();
        raw.extend(seal(
            &self.crypto.value,
            &root_aad(root.incarnation),
            &root.encode(),
        )?);
        if raw.len() > 32 * 1024 {
            return Err(Error::Bound("root envelope"));
        }
        let manifest = VaultManifest {
            schema_version: 5,
            record_count: 0,
            plaintext_bytes: 0,
            snapshot_id: *blake3::hash(&raw).as_bytes(),
        };
        let sentinel = postcard::to_allocvec(&manifest).map_err(|_| Error::Bound("sentinel"))?;
        Ok((raw, sentinel))
    }
    pub fn generation(&self) -> u64 {
        self.root.generation
    }
    pub fn last_commit(&self) -> Result<Id> {
        if self.poisoned {
            return Err(Error::ReopenRequired);
        }
        Ok(self.root.commit_id)
    }
    pub fn backend_stats(&self) -> Result<BackendStats> {
        self.backend.stats()
    }
    /// Typed records and physical rows are deliberately separate counters.
    pub fn record_counts(&self) -> Result<(u64, u64, u64)> {
        use redb::ReadableTableMetadata;
        if self.poisoned {
            return Err(Error::ReopenRequired);
        }
        let count = self.root.roots.iter().try_fold(0u64, |a, s| {
            a.checked_add(s.count).ok_or(Error::Bound("record count"))
        })?;
        let tx = self.vault.database.begin_read().map_err(db)?;
        Ok((
            count,
            tx.open_table(NODES).map_err(db)?.len().map_err(db)?,
            tx.open_table(VALUES).map_err(db)?.len().map_err(db)?,
        ))
    }
    pub fn credits(&self) -> Result<CreditReport> {
        if self.poisoned {
            return Err(Error::ReopenRequired);
        }
        credits::report(&self.root)
    }
    pub fn get(&mut self, key: Key) -> Result<Option<Vec<u8>>> {
        if self.poisoned {
            return Err(Error::ReopenRequired);
        }
        let tx = self.vault.database.begin_read().map_err(db)?;
        let nodes = tx.open_table(NODES).map_err(db)?;
        let values = tx.open_table(VALUES).map_err(db)?;
        let mut w = Work::new(&nodes, &values, &self.crypto, self.root.clone());
        w.get(key)
    }
    pub fn pending(&mut self, context: u8, ordinal: u64) -> Result<Option<PendingView>> {
        self.get(Key::ordinal(context, Namespace::Pending, ordinal)?)?
            .map(|raw| credits::pending_view(&raw))
            .transpose()
    }
    pub fn range(&mut self, first: Key, limit: usize) -> Result<Vec<(Key, Vec<u8>)>> {
        if self.poisoned {
            return Err(Error::ReopenRequired);
        }
        if !(1..=128).contains(&limit) {
            return Err(Error::Bound("range count"));
        }
        let slot = first.slot()?;
        let tx = self.vault.database.begin_read().map_err(db)?;
        let nodes = tx.open_table(NODES).map_err(db)?;
        let values = tx.open_table(VALUES).map_err(db)?;
        let mut w = Work::new(&nodes, &values, &self.crypto, self.root.clone());
        let leaves = w
            .tree
            .range(slot as u8, self.root.roots[slot], first.bytes, limit)?;
        let mut result = Vec::with_capacity(leaves.len());
        let mut bytes = 0usize;
        for leaf in leaves {
            let tree::Body::Leaf { length, .. } = leaf.body else {
                return Err(Error::Integrity("range body"));
            };
            if bytes + length as usize > 1024 * 1024 {
                break;
            }
            let key = Key {
                bytes: leaf.prefix,
                ..first
            };
            let value = w.get(key)?.ok_or(Error::Integrity("range leaf missing"))?;
            bytes += value.len();
            if bytes > 1024 * 1024 {
                return Err(Error::Bound("range bytes"));
            }
            result.push((key, value));
        }
        Ok(result)
    }
    pub fn commit(
        &mut self,
        expected: u64,
        id: Id,
        mutations: Vec<Mutation>,
        credits: Vec<CreditAction>,
    ) -> Result<CommitReport> {
        if self.poisoned {
            return Err(Error::ReopenRequired);
        }
        if id == [0; 32] || mutations.len() > 4096 || credits.len() > 256 {
            return Err(Error::Bound("commit arguments"));
        }
        let mut input_bytes = 0usize;
        for m in &mutations {
            let n = match m {
                Mutation::Put { value, .. } => value.len(),
                Mutation::Retire { .. } => 0,
            };
            if n > MAX_VALUE {
                return Err(Error::Bound("input value"));
            }
            input_bytes = input_bytes
                .checked_add(n + 128)
                .ok_or(Error::Bound("input"))?;
        }
        for c in &credits {
            input_bytes = input_bytes
                .checked_add(c.input_bound()?)
                .ok_or(Error::Bound("input"))?;
        }
        if input_bytes > MAX_PLAN {
            return Err(Error::Bound("input plan"));
        }
        let request = Zeroizing::new(
            postcard::to_allocvec(&(expected, &mutations, &credits))
                .map_err(|_| Error::Bound("request codec"))?,
        );
        let digest = self.crypto.hash(0, b"commit-request", &request);
        if id == self.root.commit_id {
            if digest != self.root.request_digest {
                return Err(Error::Conflict);
            }
            return Ok(CommitReport {
                generation: self.root.generation,
                commit_id: id,
                encoded_plan_bytes: 0,
                nodes_read: 0,
                values_read: 0,
                changed_nodes: 0,
                duplicate: true,
            });
        }
        if expected != self.root.generation {
            return Err(Error::StaleGeneration);
        }
        let tx = self.vault.database.begin_read().map_err(db)?;
        let nodes = tx.open_table(NODES).map_err(db)?;
        let values = tx.open_table(VALUES).map_err(db)?;
        let mut w = Work::new(&nodes, &values, &self.crypto, self.root.clone());
        for mutation in mutations {
            match mutation {
                Mutation::Put { key, value } => {
                    if matches!(
                        key.namespace,
                        Namespace::Credit
                            | Namespace::Queue
                            | Namespace::Pending
                            | Namespace::Undelivered
                    ) {
                        return Err(Error::Bound("credit-managed namespace"));
                    }
                    w.put(key, &value, 0, false)?;
                }
                Mutation::Retire { .. } => {
                    return Err(Error::Bound("retirement requires typed credit transition"));
                }
            }
        }
        for credit in credits {
            credits::apply(&mut w, credit)?;
        }
        for (key, charge) in &w.new_charge {
            if *charge > 0
                && w.settled[usize::from(key.context)]
                && !w.spent.contains(key)
                && !matches!(
                    key.namespace,
                    Namespace::Queue
                        | Namespace::Pending
                        | Namespace::Undelivered
                        | Namespace::Credit
                )
            {
                return Err(Error::Bound("unfunded retention mutation"));
            }
        }
        w.root.check()?;
        let duplicate =
            w.tree.changed.is_empty() && w.changed.is_empty() && w.root.ledger == self.root.ledger;
        let mut report = CommitReport {
            generation: self.root.generation,
            commit_id: self.root.commit_id,
            encoded_plan_bytes: 0,
            nodes_read: w.tree.reads,
            values_read: w.values_read,
            changed_nodes: w.tree.changed.len() as u64,
            duplicate,
        };
        if duplicate {
            return Ok(report);
        }
        w.root.generation = w
            .root
            .generation
            .checked_add(1)
            .ok_or(Error::Bound("generation"))?;
        w.root.commit_id = id;
        w.root.request_digest = digest;
        let (new_anchor, new_sentinel) = self.anchor_bytes(&w.root)?;
        let mut plan = Vec::with_capacity((w.tree.plan_bytes + w.plan_bytes + 32768).min(MAX_PLAN));
        plan.push(5);
        plan.extend(expected.to_be_bytes());
        plan.extend(w.root.generation.to_be_bytes());
        plan.extend(id);
        let meta = tx.open_table(META_TABLE).map_err(db)?;
        let old_anchor = {
            let guard = meta
                .get(ANCHOR)
                .map_err(db)?
                .ok_or(Error::Integrity("anchor missing before commit"))?;
            if guard.value().len() > 32 * 1024 {
                return Err(Error::Integrity("anchor bound"));
            }
            guard.value().to_vec()
        };
        let plain = open(
            &self.crypto.value,
            &root_aad(self.root.incarnation),
            old_anchor
                .get(32..)
                .ok_or(Error::Integrity("anchor length"))?,
            32 * 1024,
        )?;
        if Root::decode(&plain)?.encode() != self.root.encode() {
            // A configured quota change is applied below, not a second writer's state.
            let mut old = Root::decode(&plain)?;
            for c in 0..2 {
                old.ledger[c].quota = self.root.ledger[c].quota;
            }
            if old.encode() != self.root.encode() {
                return Err(Error::StaleGeneration);
            }
        }
        let old_sentinel = meta
            .get(MANIFEST_KEY)
            .map_err(db)?
            .ok_or(Error::Integrity("sentinel missing"))?;
        let sentinel: VaultManifest = postcard::from_bytes(old_sentinel.value())
            .map_err(|_| Error::Integrity("sentinel codec"))?;
        if sentinel.schema_version != 5
            || sentinel.record_count != 0
            || sentinel.plaintext_bytes != 0
            || sentinel.snapshot_id != *blake3::hash(&old_anchor).as_bytes()
            || old_anchor[..32] != self.root.incarnation
        {
            return Err(Error::Integrity("sentinel/root binding before commit"));
        }
        plan_entry(
            &mut plan,
            2,
            ANCHOR.as_bytes(),
            Some(&old_anchor),
            Some(&new_anchor),
        )?;
        plan_entry(
            &mut plan,
            2,
            MANIFEST_KEY.as_bytes(),
            Some(old_sentinel.value()),
            Some(&new_sentinel),
        )?;
        for (key, value) in &w.tree.changed {
            let old = nodes.get(key.as_slice()).map_err(db)?;
            plan_entry(
                &mut plan,
                0,
                key,
                old.as_ref().map(|v| v.value()),
                value.as_deref(),
            )?;
        }
        for (key, value) in &w.changed {
            let old = values.get(key.as_slice()).map_err(db)?;
            plan_entry(
                &mut plan,
                1,
                key,
                old.as_ref().map(|v| v.value()),
                value.as_deref(),
            )?;
        }
        let plan = plan.len();
        let new_root = w.root;
        let changed = w.changed;
        let changed_nodes = w.tree.changed;
        drop(nodes);
        drop(values);
        drop(old_sentinel);
        drop(meta);
        drop(tx);
        self.backend.begin()?;
        let write_result = (|| -> Result<()> {
            let mut tx = self.vault.database.begin_write().map_err(db)?;
            tx.set_durability(Durability::Immediate).map_err(db)?;
            {
                let meta = tx.open_table(META_TABLE).map_err(db)?;
                if meta
                    .get(ANCHOR)
                    .map_err(db)?
                    .is_none_or(|v| v.value() != old_anchor)
                {
                    return Err(Error::StaleGeneration);
                }
            }
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
            {
                let mut meta = tx.open_table(META_TABLE).map_err(db)?;
                meta.insert(ANCHOR, new_anchor.as_slice()).map_err(db)?;
                meta.insert(MANIFEST_KEY, new_sentinel.as_slice())
                    .map_err(db)?;
            }
            checkpoint("before_commit");
            tx.commit().map_err(db)?;
            checkpoint("after_commit");
            Ok(())
        })();
        if let Err(e) = write_result {
            self.poisoned = true;
            let stats = self.backend.stats()?;
            if stats.last_failure.as_deref() == Some("ConfiguredCapacity") {
                return Err(Error::ConfiguredCapacity {
                    requested: stats.last_requested_length,
                    quota: stats.physical_quota,
                });
            }
            return Err(e);
        }
        self.root = new_root;
        report.generation = self.root.generation;
        report.commit_id = id;
        report.encoded_plan_bytes = plan as u64;
        Ok(report)
    }
}
fn root_aad(incarnation: Id) -> Vec<u8> {
    let mut aad = b"kilogram:a3:v5:anchor\0".to_vec();
    aad.extend(incarnation);
    aad
}
fn account_external(root: &std::path::Path, mut config: Config) -> Result<Config> {
    for file in [
        STATE_VAULT_KEY_FILE,
        "ratchet/account.pickle",
        crate::LOCK_FILE,
    ] {
        config.retained_external_bytes = config
            .retained_external_bytes
            .checked_add(fs::metadata(root.join(file))?.len())
            .ok_or(Error::Bound("external bytes"))?;
    }
    Ok(config)
}
fn plan_entry(
    out: &mut Vec<u8>,
    table: u8,
    key: &[u8],
    old: Option<&[u8]>,
    new: Option<&[u8]>,
) -> Result<()> {
    let length = 3usize
        .checked_add(key.len())
        .and_then(|v| v.checked_add(2))
        .and_then(|v| v.checked_add(old.map_or(0, |b| 8 + b.len())))
        .and_then(|v| v.checked_add(new.map_or(0, |b| 8 + b.len())))
        .ok_or(Error::Bound("plan length"))?;
    if out.len().checked_add(length).is_none_or(|v| v > MAX_PLAN) {
        return Err(Error::Bound("encoded old/new plan"));
    }
    out.push(table);
    out.extend((key.len() as u16).to_be_bytes());
    out.extend(key);
    for bytes in [old, new] {
        out.push(u8::from(bytes.is_some()));
        if let Some(bytes) = bytes {
            out.extend((bytes.len() as u64).to_be_bytes());
            out.extend(bytes);
        }
    }
    Ok(())
}
fn checkpoint(_point: &str) {
    #[cfg(test)]
    tests::checkpoint(_point);
}

struct Work<
    'a,
    N: ReadableTable<&'static [u8], &'static [u8]>,
    V: ReadableTable<&'static [u8], &'static [u8]>,
> {
    tree: tree::Tree<'a, N>,
    values: &'a V,
    crypto: &'a Crypto,
    root: Root,
    changed: BTreeMap<Id, Option<Vec<u8>>>,
    // First mutation captures the committed charge, never an intermediate overlay.
    baseline_charge: BTreeMap<Key, u64>,
    new_charge: BTreeMap<Key, u64>,
    spent: BTreeSet<Key>,
    plan_bytes: usize,
    values_read: u64,
    settled: [bool; 2],
}
impl<
    'a,
    N: ReadableTable<&'static [u8], &'static [u8]>,
    V: ReadableTable<&'static [u8], &'static [u8]>,
> Work<'a, N, V>
{
    fn new(nodes: &'a N, values: &'a V, crypto: &'a Crypto, root: Root) -> Self {
        Self {
            tree: tree::Tree::new(nodes, crypto),
            values,
            crypto,
            root,
            changed: BTreeMap::new(),
            baseline_charge: BTreeMap::new(),
            new_charge: BTreeMap::new(),
            spent: BTreeSet::new(),
            plan_bytes: 0,
            values_read: 0,
            settled: [false; 2],
        }
    }
    fn get(&mut self, key: Key) -> Result<Option<Vec<u8>>> {
        let slot = key.slot()?;
        let Some(leaf) = self
            .tree
            .find(slot as u8, self.root.roots[slot], key.bytes)?
        else {
            return Ok(None);
        };
        let tree::Body::Leaf {
            hash,
            length,
            owner,
            ..
        } = leaf.body
        else {
            return Err(Error::Integrity("value leaf"));
        };
        let address = self.crypto.address(slot as u8, b"value", &key.bytes);
        let raw = if let Some(value) = self.changed.get(&address) {
            value.clone().ok_or(Error::Integrity("removed value"))?
        } else {
            self.values_read += 1;
            let g = self
                .values
                .get(address.as_slice())
                .map_err(db)?
                .ok_or(Error::Integrity("missing expected value"))?;
            if g.value().len() > MAX_VALUE + 41 {
                return Err(Error::Integrity("value cap"));
            }
            g.value().to_vec()
        };
        let plain = open(
            &self.crypto.value,
            &self.crypto.aad(slot as u8, b"value", address),
            &raw,
            MAX_VALUE + 41,
        )?;
        if plain.len() as u64 != length || self.crypto.hash(slot as u8, b"value", &plain) != hash {
            return Err(Error::Integrity("value digest/length"));
        }
        if key.namespace.global()
            && (plain.len() != 64 || plain[..32] != self.root.contexts[usize::from(owner)])
        {
            return Err(Error::Integrity("author scope"));
        }
        Ok(Some(plain.to_vec()))
    }
    fn change_value(&mut self, address: Id, value: Option<Vec<u8>>) -> Result<()> {
        if let Some(old) = self.changed.get(&address) {
            self.plan_bytes = self
                .plan_bytes
                .saturating_sub(old.as_ref().map_or(0, Vec::len));
        } else {
            let old = self.values.get(address.as_slice()).map_err(db)?;
            self.plan_bytes = self
                .plan_bytes
                .checked_add(old.as_ref().map_or(0, |v| v.value().len()) + 32)
                .ok_or(Error::Bound("value plan"))?;
        }
        self.plan_bytes = self
            .plan_bytes
            .checked_add(value.as_ref().map_or(0, Vec::len))
            .ok_or(Error::Bound("value plan"))?;
        if self.plan_bytes + self.tree.plan_bytes > MAX_PLAN {
            return Err(Error::Bound("affected plan"));
        }
        self.changed.insert(address, value);
        Ok(())
    }
    fn put(&mut self, key: Key, value: &[u8], reserved: u64, internal: bool) -> Result<()> {
        let cap = match key.namespace {
            Namespace::Event | Namespace::Auth => 16 * 1024,
            Namespace::Projection | Namespace::Queue => 8 * 1024,
            Namespace::Native => 128 * 1024,
            Namespace::Request => 128,
            Namespace::ReceiptA | Namespace::ReceiptB => 1024,
            _ => MAX_VALUE,
        };
        if value.len() > cap {
            return Err(Error::Bound("typed value cap"));
        }
        if key.namespace == Namespace::Admission && (value.len() != 65 || value[0] != 5) {
            return Err(Error::Bound("admission value"));
        }
        if key.namespace == Namespace::Projection {
            ProjectionValue::decode(value)?;
        }
        if matches!(key.namespace, Namespace::LaneA | Namespace::LaneB) && value.len() != 32
            || matches!(key.namespace, Namespace::SeenA | Namespace::SeenB) && value.len() != 40
        {
            return Err(Error::Bound("typed fixed value"));
        }
        let slot = key.slot()?;
        if let Some(old) = self.get(key)? {
            let leaf = self
                .tree
                .find(slot as u8, self.root.roots[slot], key.bytes)?
                .ok_or(Error::Integrity("existing leaf"))?;
            let tree::Body::Leaf {
                reserved: old_reserved,
                owner,
                ..
            } = leaf.body
            else {
                return Err(Error::Integrity("leaf"));
            };
            if key.namespace.global() && owner != key.context {
                return Err(Error::Conflict);
            }
            if old == value && old_reserved == reserved {
                return Ok(());
            }
            if !internal && !key.namespace.mutable() {
                return Err(Error::Conflict);
            }
        }
        if key.namespace.global()
            && (value.len() != 64 || value[..32] != self.root.contexts[usize::from(key.context)])
        {
            return Err(Error::Bound("author context/EventId value"));
        }
        let old = self
            .tree
            .find(slot as u8, self.root.roots[slot], key.bytes)?;
        let old_charge = old.as_ref().map_or(0, |n| match n.body {
            tree::Body::Leaf { charge, .. } => charge,
            _ => 0,
        });
        let leaf = tree::leaf(
            key.bytes,
            self.crypto.hash(slot as u8, b"value", value),
            value.len() as u64,
            key.context,
            reserved,
        );
        let charge = value.len() as u64 + 41 + 32 + 2 * NODE_CAP as u64;
        let baseline = *self.baseline_charge.entry(key).or_insert(old_charge);
        self.new_charge.insert(key, charge.saturating_sub(baseline));
        let address = self.crypto.address(slot as u8, b"value", &key.bytes);
        let encrypted = seal(
            &self.crypto.value,
            &self.crypto.aad(slot as u8, b"value", address),
            value,
        )?;
        self.change_value(address, Some(encrypted))?;
        self.root.roots[slot] = self.tree.put(slot as u8, self.root.roots[slot], leaf)?;
        Ok(())
    }
    fn remove(&mut self, key: Key) -> Result<()> {
        if self.get(key)?.is_none() {
            return Err(Error::Conflict);
        }
        let slot = key.slot()?;
        let leaf = self
            .tree
            .find(slot as u8, self.root.roots[slot], key.bytes)?
            .ok_or(Error::Integrity("removed leaf"))?;
        let tree::Body::Leaf { charge, .. } = leaf.body else {
            return Err(Error::Integrity("removed leaf kind"));
        };
        self.baseline_charge.entry(key).or_insert(charge);
        self.new_charge.insert(key, 0);
        self.root.roots[slot] = self
            .tree
            .remove(slot as u8, self.root.roots[slot], key.bytes)?;
        self.change_value(self.crypto.address(slot as u8, b"value", &key.bytes), None)
    }
}

#[cfg(test)]
#[path = "indexed_dialog/tests.rs"]
mod tests;
