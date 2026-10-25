//! Schema4/OD1: an exclusive owner, incremental primary writes and a v2 journal.
//! No mutable ratchet workspace is exposed to a legacy filesystem transaction.
use super::*;
use crate::{
    StateDirectoryLock, TransactionManifest, active_directory, write_manifest, write_marker,
};

pub const ORDERED_ACCOUNT_FENCE: &[u8] = b"kilogram:ordered-dialog:account:v1\n";
const MODE_PATH: &str = "runtime/od1/profile-mode";
const MODE: &[u8] = b"kilogram:ordered-dialog:v1\0";
const CLAIM: &str = "runtime/od1/profile-claim";
const LOGICAL_CAP: u64 = 384 * 1024 * 1024;
const PHYSICAL_CAP: u64 = 2 * 1024 * 1024 * 1024;
const INDEX_CAP: usize = 8 * 1024 * 1024;
const DELTA_CAP: u64 = 32 * 1024 * 1024;

#[derive(Clone)]
pub struct OrderedMutation {
    path: String,
    content: Option<Vec<u8>>,
}
impl OrderedMutation {
    pub fn put(path: impl Into<String>, content: Vec<u8>) -> Self {
        Self {
            path: path.into(),
            content: Some(content),
        }
    }
    pub fn remove(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content: None,
        }
    }
}

/// A primary commit is durable even if a later derived-shadow write needs recovery.
/// Callers must reload/invalidate caches and withhold dispatch until visible=true.
pub struct OrderedCommit {
    pub generation: u64,
    pub visible: bool,
    pub encoded_delta_bytes: u64,
}

pub struct OrderedVault {
    vault: EncryptedStateVault,
    _lease: StateDirectoryLock,
    index: BTreeMap<String, VaultManifestIndexEntry>,
    generation: u64,
    recovery_needed: bool,
    pub commits: u64,
}

enum OrderedProfile {
    Missing,
    Other,
    Verified(VaultManifestIndex),
}

impl EncryptedStateVault {
    pub fn is_ordered_profile(&self) -> Result<bool, StateError> {
        Ok(matches!(
            self.bounded_ordered_profile()?,
            OrderedProfile::Verified(_)
        ))
    }

    // Recovery and profile dispatch share one bounded, authenticated metadata
    // read. Inspect borrowed ciphertext before either copying or decrypting it.
    fn bounded_ordered_profile(&self) -> Result<OrderedProfile, StateError> {
        let tx = self.database.begin_read().map_err(vault_database_error)?;
        let meta = tx.open_table(META_TABLE).map_err(vault_database_error)?;
        for (name, cap) in [(MANIFEST_KEY, 256), (GENERATION_KEY, 1024)] {
            if meta
                .get(name)
                .map_err(vault_database_error)?
                .is_some_and(|raw| raw.value().len() > cap)
            {
                return Err(StateError::OrderedState("recovery metadata cap"));
            }
        }
        let Some(raw) = meta.get(MANIFEST_KEY).map_err(vault_database_error)? else {
            return Ok(OrderedProfile::Missing);
        };
        let manifest = postcard::from_bytes(raw.value())?;
        validate_manifest(&manifest)?;
        if manifest.schema_version != ORDERED_VAULT_SCHEMA_VERSION {
            return Ok(OrderedProfile::Other);
        }
        let raw = meta
            .get(MANIFEST_INDEX_KEY)
            .map_err(vault_database_error)?
            .ok_or(StateError::VaultManifestIndexMissing)?;
        if raw.value().len() > INDEX_CAP + 41 {
            return Err(StateError::OrderedState("index recovery cap"));
        }
        let index = self.decrypt_manifest_index(raw.value())?;
        if self.manifest_for_index_schema(&index, ORDERED_VAULT_SCHEMA_VERSION)? != manifest {
            return Err(StateError::VaultManifestMismatch);
        }
        if let Some(raw) = meta.get(GENERATION_KEY).map_err(vault_database_error)? {
            self.decode_generation(&manifest, raw.value())?;
        }
        Ok(OrderedProfile::Verified(index))
    }

    pub(super) fn reject_ordered_writer(&self) -> Result<(), StateError> {
        if self.is_ordered_profile()? {
            return Err(StateError::OrderedWriterRequired);
        }
        Ok(())
    }
}

pub(crate) fn has_ordered_fence(root: &Path) -> Result<bool, StateError> {
    let path = root.join("ratchet/account.pickle");
    match fs::metadata(&path) {
        Ok(meta) if meta.len() != ORDERED_ACCOUNT_FENCE.len() as u64 => return Ok(false),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(source) => return Err(StateError::Io { path, source }),
    }
    match fs::read(&path) {
        Ok(raw) => Ok(raw == ORDERED_ACCOUNT_FENCE),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(StateError::Io { path, source }),
    }
}

impl OrderedVault {
    pub fn create(
        lease: StateDirectoryLock,
        mut initial: Vec<OrderedMutation>,
    ) -> Result<Self, StateError> {
        let root = lease.root();
        // A fence is compatibility protection, not an authenticated creation
        // claim. Only open() with a committed manifest can resume saved keys.
        if has_ordered_fence(root)? {
            return Err(StateError::OrderedState(
                "CreationConflict: incomplete initialization has no saved claim",
            ));
        }
        for entry in io_at(root, fs::read_dir(root))? {
            let entry = io_at(root, entry)?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == crate::LOCK_FILE {
                continue;
            }
            return Err(StateError::OrderedState(
                "CreationConflict: existing profile",
            ));
        }
        for dir in ["ratchet/sessions", "ratchet/peer-prekey-pools"] {
            io_at(root, fs::create_dir_all(root.join(dir)))?;
        }
        crate::write_staged_file(&root.join("ratchet/account.pickle"), ORDERED_ACCOUNT_FENCE)?;
        let secret = root.join("ratchet/pickle-secret.key");
        if !path_exists(&secret)? {
            crate::write_staged_file(&secret, &[0; 32])?;
        }
        let vault = EncryptedStateVault::open_or_create(root)?;
        if vault.load_manifest()?.is_some() {
            return Err(StateError::OrderedState(
                "CreationConflict: initialized vault",
            ));
        }
        initial.push(OrderedMutation::put(MODE_PATH, MODE.to_vec()));
        initial.push(OrderedMutation::put(
            "ratchet/account.pickle",
            ORDERED_ACCOUNT_FENCE.to_vec(),
        ));
        initial.push(OrderedMutation::put(
            "ratchet/pickle-secret.key",
            vec![0; 32],
        ));
        let mut records = BTreeMap::new();
        for mutation in initial {
            validate_path(&mutation.path, true)?;
            let content = mutation
                .content
                .ok_or(StateError::OrderedState("initial deletion"))?;
            let record = VaultRecord {
                version: VAULT_RECORD_VERSION,
                relative_path: mutation.path.clone(),
                content,
            };
            validate_record(&record)?;
            if records.insert(mutation.path, record).is_some() {
                return Err(StateError::OrderedState("duplicate initial record"));
            }
        }
        if !records.contains_key(CLAIM) {
            return Err(StateError::OrderedState("initial claim missing"));
        }
        let records = records.into_values().collect::<Vec<_>>();
        let index = vault.index_for_records(&records)?;
        let manifest = vault.manifest_for_index_schema(&index, ORDERED_VAULT_SCHEMA_VERSION)?;
        prepare_journal(root)?;
        vault.commit_snapshot(&records, &manifest, &index, 1, true, None)?;
        restore_shadow(&vault)?;
        finish_journal(root)?;
        Self::from_parts(vault, lease, index, 1)
    }

    pub fn open(lease: StateDirectoryLock) -> Result<Self, StateError> {
        let vault = EncryptedStateVault::open_existing(lease.root())?;
        if !vault.is_ordered_profile()? {
            return Err(StateError::OrderedState("OD1 schema required"));
        }
        let report = vault.verify()?;
        restore_shadow(&vault)?;
        let index = vault
            .load_manifest_index()?
            .ok_or(StateError::VaultManifestIndexMissing)?;
        Self::from_parts(vault, lease, index, report.mirror_generation)
    }

    fn from_parts(
        vault: EncryptedStateVault,
        lease: StateDirectoryLock,
        index: VaultManifestIndex,
        generation: u64,
    ) -> Result<Self, StateError> {
        let this = Self {
            vault,
            _lease: lease,
            index: index
                .entries
                .into_iter()
                .map(|v| (v.relative_path.clone(), v))
                .collect(),
            generation,
            recovery_needed: false,
            commits: 0,
        };
        if this.get(MODE_PATH)?.as_deref() != Some(MODE) || this.get(CLAIM)?.is_none() {
            return Err(StateError::OrderedState(
                "required OD1 feature/claim absent",
            ));
        }
        Ok(this)
    }
    pub fn root(&self) -> &Path {
        &self.vault.root
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn paths(&self, prefix: &str) -> Vec<String> {
        self.index
            .keys()
            .filter(|p| p.starts_with(prefix))
            .cloned()
            .collect()
    }
    pub fn logical_bytes(&self) -> u64 {
        self.index.values().map(|v| v.content_len).sum()
    }

    pub fn get(&self, path: &str) -> Result<Option<Vec<u8>>, StateError> {
        if self.recovery_needed {
            return Err(StateError::OrderedState("recovery required"));
        }
        let Some(expected) = self.index.get(path) else {
            return Ok(None);
        };
        let key = self.vault.record_key(path);
        let read = self
            .vault
            .database
            .begin_read()
            .map_err(vault_database_error)?;
        let table = read
            .open_table(RECORD_TABLE)
            .map_err(vault_database_error)?;
        let value = table
            .get(key.as_slice())
            .map_err(vault_database_error)?
            .ok_or(StateError::VaultManifestMismatch)?;
        let record = self.vault.decrypt_record(value.value(), &key)?;
        if manifest_index_entry_for_record(&record)? != *expected {
            return Err(StateError::VaultManifestMismatch);
        }
        Ok(Some(record.content))
    }

    /// Validates the full index, old/new records and physical staging before any DB write.
    /// `expected_generation` is the owner actor's stale-workspace fence.
    pub fn commit(
        &mut self,
        expected_generation: u64,
        mutations: Vec<OrderedMutation>,
    ) -> Result<OrderedCommit, StateError> {
        self.commit_internal(expected_generation, mutations, None)
    }

    fn commit_internal(
        &mut self,
        expected_generation: u64,
        mutations: Vec<OrderedMutation>,
        fail_after: Option<usize>,
    ) -> Result<OrderedCommit, StateError> {
        if self.recovery_needed || expected_generation != self.generation {
            return Err(StateError::OrderedState(
                "stale generation/recovery required",
            ));
        }
        let mut staged = self.index.clone();
        let mut upserts = Vec::new();
        let mut removals = Vec::new();
        let mut seen = BTreeSet::new();
        let mut touched_bytes = 0_u64;
        for mutation in mutations {
            validate_path(&mutation.path, false)?;
            if !seen.insert(mutation.path.clone()) {
                return Err(StateError::OrderedState("duplicate mutation path"));
            }
            if let Some(existing) = self.index.get(&mutation.path) {
                touched_bytes = touched_bytes.saturating_add(existing.content_len);
            }
            match mutation.content {
                Some(content) => {
                    let cap = record_cap(&mutation.path)
                        .ok_or(StateError::OrderedState("unsupported scoped record path"))?;
                    if content.len() as u64 > cap {
                        return Err(StateError::OrderedState(
                            "UpgradeRequired: encoded record cap",
                        ));
                    }
                    let record = VaultRecord {
                        version: VAULT_RECORD_VERSION,
                        relative_path: mutation.path.clone(),
                        content,
                    };
                    validate_record(&record)?;
                    let entry = manifest_index_entry_for_record(&record)?;
                    if self.index.get(&mutation.path) == Some(&entry) {
                        continue;
                    }
                    if self.index.contains_key(&mutation.path) && !mutable_path(&mutation.path) {
                        return Err(StateError::AppendOnlyRecordModified(mutation.path));
                    }
                    touched_bytes = touched_bytes.saturating_add(entry.content_len);
                    staged.insert(mutation.path, entry);
                    upserts.push(record);
                }
                None => {
                    if !removable_path(&mutation.path) {
                        return Err(StateError::OrderedState("immutable deletion"));
                    }
                    if staged.remove(&mutation.path).is_some() {
                        removals.push(mutation.path);
                    }
                }
            }
        }
        let index = VaultManifestIndex {
            version: VAULT_MANIFEST_INDEX_VERSION,
            entries: staged.values().cloned().collect(),
        };
        validate_reservations(&index)?;
        let old_index = VaultManifestIndex {
            version: VAULT_MANIFEST_INDEX_VERSION,
            entries: self.index.values().cloned().collect(),
        };
        let new_index_len = postcard::to_allocvec(&index)?.len();
        let old_index_len = postcard::to_allocvec(&old_index)?.len();
        let full_delta = touched_bytes
            .saturating_add(new_index_len as u64)
            .saturating_add(old_index_len as u64)
            .saturating_add((seen.len() as u64).saturating_mul(512))
            .saturating_add(4096);
        let manifest = self
            .vault
            .manifest_for_index_schema(&index, ORDERED_VAULT_SCHEMA_VERSION)?;
        if new_index_len > INDEX_CAP
            || old_index_len > INDEX_CAP
            || full_delta > DELTA_CAP
            || manifest.plaintext_bytes > LOGICAL_CAP
        {
            return Err(StateError::OrderedState(
                "StorageBlocked: encoded commit budget",
            ));
        }
        let database_path = self.root().join(STATE_VAULT_FILE);
        let database_bytes = io_at(&database_path, fs::metadata(&database_path))?.len();
        // A pessimistic next redb allocation plus retained shadow and old/new staging.
        let staging = database_bytes
            .saturating_mul(2)
            .saturating_add(self.logical_bytes())
            .saturating_add(full_delta)
            .saturating_add(1024 * 1024);
        if staging > PHYSICAL_CAP {
            return Err(StateError::OrderedState("StorageBlocked: physical staging"));
        }
        if upserts.is_empty() && removals.is_empty() {
            return Ok(OrderedCommit {
                generation: self.generation,
                visible: true,
                encoded_delta_bytes: 0,
            });
        }
        let next = self
            .generation
            .checked_add(1)
            .ok_or(StateError::VaultGenerationExhausted)?;
        let delta = PendingVaultDelta {
            unchanged_records: 0,
            upserts,
            removals,
        };
        prepare_journal(self.root())?;
        // A redb commit error can be ambiguous. Never publish/re-encrypt from an
        // old cache; reopen/recovery resolves the authenticated primary first.
        self.recovery_needed = true;
        self.vault
            .commit_delta(&delta, &manifest, &index, next, fail_after)?;
        primary_test_checkpoint(self.root(), &delta)?;
        self.index = staged;
        self.generation = next;
        self.commits += 1;
        let visible = (|| {
            for record in &delta.upserts {
                if classify_record_kind(&record.relative_path) != StateRecordKind::DeviceIdentity {
                    self.vault.restore_legacy_record(record)?;
                }
            }
            for path in &delta.removals {
                let target = self.root().join(path_from_vault(path)?);
                if path_exists(&target)? {
                    reject_symlink(&target)?;
                    io_at(&target, fs::remove_file(&target))?;
                }
            }
            finish_journal(self.root())
        })()
        .is_ok();
        self.recovery_needed = !visible;
        Ok(OrderedCommit {
            generation: next,
            visible,
            encoded_delta_bytes: full_delta,
        })
    }

    pub fn recover(&mut self) -> Result<(), StateError> {
        let report = self.vault.verify()?;
        restore_shadow(&self.vault)?;
        finish_journal(self.root())?;
        self.index = self
            .vault
            .load_manifest_index()?
            .ok_or(StateError::VaultManifestIndexMissing)?
            .entries
            .into_iter()
            .map(|v| (v.relative_path.clone(), v))
            .collect();
        self.generation = report.mirror_generation;
        self.recovery_needed = false;
        Ok(())
    }
}

fn primary_test_checkpoint(root: &Path, delta: &PendingVaultDelta) -> Result<(), StateError> {
    let Some(path) = std::env::var_os("KILOGRAM_TEST_OD1_PRIMARY_PAUSE_FILE") else {
        return Ok(());
    };
    if !cfg!(debug_assertions) {
        return Err(StateError::OrderedState("OD1 primary pause is debug only"));
    }
    if !delta
        .upserts
        .iter()
        .any(|r| r.relative_path.contains("/ack-intent/"))
    {
        return Ok(());
    }
    let path = PathBuf::from(path);
    let absolute = io_at(&path, fs::canonicalize(&path))?;
    if absolute.starts_with(root) {
        return Err(StateError::OrderedState(
            "primary pause must be outside root",
        ));
    }
    if io_at(&path, fs::read(&path))? != b"readable-primary" {
        return Ok(());
    }
    let hit = path.with_extension("primary-hit");
    let mut marker = io_at(
        &hit,
        NamedTempFile::new_in(
            hit.parent()
                .ok_or(StateError::OrderedState("primary marker parent"))?,
        ),
    )?;
    io_at(&hit, marker.write_all(b"readable-primary"))?;
    io_at(&hit, marker.as_file().sync_all())?;
    io_at(&hit, marker.persist(&hit).map_err(|error| error.error))?;
    sync_directory(
        hit.parent()
            .ok_or(StateError::OrderedState("marker parent"))?,
    )?;
    while io_at(&path, fs::read(&path))? == b"readable-primary" {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    Ok(())
}

fn validate_path(path: &str, initial: bool) -> Result<(), StateError> {
    path_from_vault(path)?;
    if path.len() > 256 || path.split('/').any(|part| part.len() > 80) {
        return Err(StateError::OrderedState("scoped record path length"));
    }
    let permitted = record_cap(path).is_some()
        && !matches!(
            path,
            "device-secret.key"
                | "device-encryption-secret.key"
                | "ratchet/account.pickle"
                | "ratchet/pickle-secret.key"
        );
    if permitted
        || (initial
            && matches!(
                path,
                "device-secret.key"
                    | "device-encryption-secret.key"
                    | "ratchet/account.pickle"
                    | "ratchet/pickle-secret.key"
            ))
    {
        Ok(())
    } else {
        Err(StateError::OrderedState("unsupported scoped record path"))
    }
}
fn hex_component(value: &str, n: usize) -> bool {
    value.len() == n
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn ordinal(value: &str) -> bool {
    value.len() == 20 && value.bytes().all(|b| b.is_ascii_digit())
}
pub(super) fn record_cap(path: &str) -> Option<u64> {
    const K: u64 = 1024;
    match path {
        MODE_PATH => return Some(64),
        CLAIM => return Some(K),
        "device-secret.key" | "device-encryption-secret.key" | "ratchet/pickle-secret.key" => {
            return Some(32);
        }
        "ratchet/account.pickle" => return Some(64),
        "next-sequence" => return Some(32),
        "account-authority.snapshot" => return Some(16 * K),
        "device-certificate.cert" => return Some(8 * K),
        _ => {}
    }
    let p = path.split('/').collect::<Vec<_>>();
    match p.as_slice() {
        ["peer-authority", name]
            if name
                .strip_suffix(".snapshot")
                .is_some_and(|s| hex_component(s, 64)) =>
        {
            Some(16 * K)
        }
        ["ratchet", "od1-contexts", c, "native"] if hex_component(c, 64) => Some(128 * K),
        ["events", "od1", c, id] if hex_component(c, 64) && hex_component(id, 64) => Some(16 * K),
        ["local-messages", "od1", c, id] if hex_component(c, 64) && hex_component(id, 64) => {
            Some(8 * K)
        }
        ["runtime", "od1", "auth", id] if hex_component(id, 64) => Some(16 * K),
        ["runtime", "od1", "authored", device, n] if hex_component(device, 64) && ordinal(n) => {
            Some(32)
        }
        ["runtime", "od1", "contexts", c, tail @ ..] if hex_component(c, 64) => match tail {
            ["head"] => Some(4 * K),
            ["spec"] => Some(96 * K),
            ["consent-current"] => Some(4 * K),
            ["current-auth" | "initial-pair"] => Some(192 * K),
            ["peer-locator" | "local-locator"] => Some(8 * K),
            ["blocked-fork"] => Some(68 * K),
            ["controls", id] if hex_component(id, 64) => Some(32 * K),
            ["ready", "0" | "1"] => Some(4 * K),
            ["lane", "0" | "1", n] if ordinal(n) => Some(32),
            ["receipts" | "ack-intent", "0" | "1", n] if ordinal(n) => Some(K),
            ["pending", n] if ordinal(n) => Some(16 * K),
            ["pending-state", n] if ordinal(n) => Some(2 * K),
            ["outbox", n] if ordinal(n) => Some(K),
            ["requests", id] if hex_component(id, 32) => Some(32),
            ["queue", id]
                if id
                    .split_once('-')
                    .is_some_and(|(g, r)| ordinal(g) && hex_component(r, 32)) =>
            {
                Some(8 * K)
            }
            _ => None,
        },
        _ => None,
    }
}
/// Reserve both complete lifetime lanes when a context is installed. Actual
/// permanent records are charged within that reservation, never borrowed from
/// another context's prepaid capacity or from pending/head/commit reserves.
pub(super) fn validate_reservations(index: &VaultManifestIndex) -> Result<(), StateError> {
    let mut contexts = BTreeMap::<String, (u64, u64, u64, u64, u64, u64)>::new();
    let mut base = 0_u64;
    let mut auth = 0_u64;
    let mut authored = 0_u64;
    let mut pending_frames = 0_u64;
    for e in &index.entries {
        let p = e.relative_path.split('/').collect::<Vec<_>>();
        match p.as_slice() {
            ["runtime", "od1", "auth", _] => auth += e.content_len,
            ["runtime", "od1", "authored", _, _] => authored += 1,
            ["runtime", "od1", "contexts", c, tail @ ..] => {
                let v = contexts.entry((*c).to_owned()).or_default();
                match tail {
                    ["queue", _] => {
                        v.1 += 1;
                        v.2 += e.content_len;
                    }
                    ["pending", _] => {
                        v.3 += e.content_len;
                        pending_frames += 1;
                    }
                    ["pending-state", _] => v.3 += e.content_len,
                    ["lane", _, _]
                    | ["receipts", _, _]
                    | ["ack-intent", _, _]
                    | ["requests", _]
                    | ["outbox", _] => v.4 += e.content_len,
                    _ => v.0 += e.content_len,
                }
            }
            ["ratchet", "od1-contexts", c, "native"] => {
                contexts.entry((*c).to_owned()).or_default().0 += e.content_len
            }
            ["events", "od1", c, _] => {
                let v = contexts.entry((*c).to_owned()).or_default();
                v.4 += e.content_len;
                v.5 += 1;
            }
            ["local-messages", "od1", c, _] => {
                contexts.entry((*c).to_owned()).or_default().4 += e.content_len
            }
            _ => base += e.content_len,
        }
    }
    if contexts.len() > 2
        || base > 16 * 1024 * 1024
        || authored > 2048
        || auth
            > (4 * contexts.values().map(|v| v.5).sum::<u64>() + 2 * pending_frames + 1) * 16 * 1024
    {
        return Err(StateError::OrderedState(
            "StorageBlocked: profile reservation",
        ));
    }
    for (controls, queue_count, queue_bytes, pending, permanent, events) in contexts.values() {
        if *controls > 4 * 1024 * 1024
            || *queue_count > 128
            || *queue_bytes > 1024 * 1024
            || *pending > 4160 * 1024
            || *events > 1024
            || *permanent > 1024 * 64 * 1024
        {
            return Err(StateError::OrderedState(
                "StorageBlocked: context reservation",
            ));
        }
    }
    // Per frame 64 KiB record/index/projection + four 16 KiB authorities
    // conservatively fits the prepaid 128 KiB, even without deduplication.
    let planned = 16 * 1024 * 1024
        + contexts.len() as u64
            * (128 * 1024 * 1024 + 4 * 1024 * 1024 + 1024 * 1024 + 2 * 4160 * 1024)
        + 4 * 1024 * 1024
        + 32 * 1024 * 1024;
    if planned > LOGICAL_CAP {
        return Err(StateError::OrderedState(
            "StorageBlocked: prepaid logical capacity",
        ));
    }
    Ok(())
}
fn mutable_path(path: &str) -> bool {
    path == "next-sequence"
        || path == "account-authority.snapshot"
        || path.starts_with("peer-authority/")
        || (path.starts_with("ratchet/od1-contexts/") && path.ends_with("/native"))
        || (path.starts_with("runtime/od1/contexts/")
            && (path.ends_with("/head")
                || path.contains("/pending-state/")
                || path.contains("/outbox/")
                || path.contains("/ack-intent/")
                || path.ends_with("/current-auth")
                || path.ends_with("/consent-current")
                || path.ends_with("/peer-locator")
                || path.ends_with("/local-locator")))
}
fn removable_path(path: &str) -> bool {
    path.starts_with("runtime/od1/contexts/")
        && (path.contains("/pending/")
            || path.contains("/pending-state/")
            || path.contains("/queue/"))
}

fn prepare_journal(root: &Path) -> Result<(), StateError> {
    let active = active_directory(root);
    if path_exists(&active)? {
        return Err(StateError::TransactionAlreadyActive { path: active });
    }
    io_at(&active, fs::create_dir_all(&active))?;
    let manifest = TransactionManifest {
        version: 2,
        ratchet_existed: false,
        next_sequence_existed: false,
        append_only_files: Vec::new(),
        compacted_append_only_files: Vec::new(),
        ratchet_primary_existed: None,
        next_sequence_primary_existed: None,
        trust_primary_prepared: false,
    };
    write_manifest(&active, &manifest)?;
    write_marker(&active.join(crate::PREPARED_MARKER))
}
fn finish_journal(root: &Path) -> Result<(), StateError> {
    let active = active_directory(root);
    if path_exists(&active)? {
        write_marker(&active.join(crate::COMMITTED_MARKER))?;
        crate::remove_tree_if_present(&active)?;
    }
    Ok(())
}

pub(crate) fn recover_ordered_journal(root: &Path) -> Result<(), StateError> {
    // Pre-initialization may have stopped before a vault existed. No OD1 data
    // or credentials have been published in this state.
    if !path_exists(&root.join(STATE_VAULT_FILE))? {
        return Ok(());
    }
    let vault = EncryptedStateVault::open_existing_cache(root, Some(64 * 1024 * 1024))?;
    match vault.bounded_ordered_profile()? {
        OrderedProfile::Missing => Ok(()),
        OrderedProfile::Other => Err(StateError::OrderedState("v2 journal over a legacy profile")),
        OrderedProfile::Verified(index) => restore_verified_shadow(&vault, &index),
    }
}
fn restore_shadow(vault: &EncryptedStateVault) -> Result<(), StateError> {
    let OrderedProfile::Verified(index) = vault.bounded_ordered_profile()? else {
        return Err(StateError::OrderedState("OD1 recovery schema"));
    };
    restore_verified_shadow(vault, &index)
}
fn restore_verified_shadow(
    vault: &EncryptedStateVault,
    index: &VaultManifestIndex,
) -> Result<(), StateError> {
    // Validate the authenticated bounded index and each primary row before changing
    // any derived shadow. Neither primary nor filesystem bodies accumulate in RAM.
    validate_reservations(index)?;
    {
        let tx = vault.database.begin_read().map_err(vault_database_error)?;
        let rows = tx.open_table(RECORD_TABLE).map_err(vault_database_error)?;
        if redb::ReadableTableMetadata::len(&rows).map_err(vault_database_error)?
            != index.entries.len() as u64
        {
            return Err(StateError::VaultManifestMismatch);
        }
    }
    for entry in &index.entries {
        read_primary(vault, entry)?;
    }
    let expected = index
        .entries
        .iter()
        .map(|e| e.relative_path.as_str())
        .collect::<BTreeSet<_>>();
    remove_stale_shadows(
        &vault.root,
        &vault.root.join("runtime/od1/contexts"),
        &expected,
        0,
    )?;
    for entry in &index.entries {
        let record = read_primary(vault, entry)?;
        if classify_record_kind(&record.relative_path) != StateRecordKind::DeviceIdentity {
            let path = vault.root.join(path_from_vault(&record.relative_path)?);
            crate::reject_relative_symlinks(&vault.root, Path::new(&record.relative_path))?;
            let same = if path_exists(&path)?
                && io_at(&path, fs::metadata(&path))?.len() == record.content.len() as u64
            {
                io_at(&path, fs::read(&path))? == record.content
            } else {
                false
            };
            if !same {
                vault.restore_legacy_record(&record)?;
            }
        }
    }
    Ok(())
}
fn read_primary(
    vault: &EncryptedStateVault,
    entry: &VaultManifestIndexEntry,
) -> Result<VaultRecord, StateError> {
    let cap =
        record_cap(&entry.relative_path).ok_or(StateError::OrderedState("unknown recovery row"))?;
    if entry.content_len > cap {
        return Err(StateError::OrderedState("recovery row cap"));
    }
    let tx = vault.database.begin_read().map_err(vault_database_error)?;
    let table = tx.open_table(RECORD_TABLE).map_err(vault_database_error)?;
    let key = vault.record_key(&entry.relative_path);
    let raw = table
        .get(key.as_slice())
        .map_err(vault_database_error)?
        .ok_or(StateError::VaultManifestMismatch)?;
    if raw.value().len() as u64 > cap + 512 {
        return Err(StateError::OrderedState("recovery envelope cap"));
    }
    let record = vault.decrypt_record(raw.value(), &key)?;
    if manifest_index_entry_for_record(&record)? != *entry {
        return Err(StateError::VaultManifestMismatch);
    }
    Ok(record)
}
fn remove_stale_shadows(
    root: &Path,
    dir: &Path,
    expected: &BTreeSet<&str>,
    depth: usize,
) -> Result<(), StateError> {
    if !path_exists(dir)? {
        return Ok(());
    }
    if depth > 8 {
        return Err(StateError::OrderedState("shadow directory depth"));
    }
    reject_symlink(dir)?;
    for entry in io_at(dir, fs::read_dir(dir))? {
        let entry = io_at(dir, entry)?;
        let path = entry.path();
        reject_symlink(&path)?;
        if io_at(&path, entry.file_type())?.is_dir() {
            remove_stale_shadows(root, &path, expected, depth + 1)?;
        } else {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| StateError::OrderedState("shadow scope"))?
                .to_string_lossy()
                .replace('\\', "/");
            if removable_path(&relative) && !expected.contains(relative.as_str()) {
                io_at(&path, fs::remove_file(&path))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const EVENT: &str = "events/od1/0000000000000000000000000000000000000000000000000000000000000000/1111111111111111111111111111111111111111111111111111111111111111";
    fn create(root: &Path) -> Result<OrderedVault, StateError> {
        OrderedVault::create(
            StateDirectoryLock::acquire(root)?,
            vec![OrderedMutation::put(CLAIM, b"claim".to_vec())],
        )
    }
    fn primary_digest(vault: &EncryptedStateVault) -> Result<[u8; 32], StateError> {
        let tx = vault.database.begin_read().map_err(vault_database_error)?;
        let mut digest = blake3::Hasher::new();
        let meta = tx.open_table(META_TABLE).map_err(vault_database_error)?;
        for row in meta.iter().map_err(vault_database_error)? {
            let (key, value) = row.map_err(vault_database_error)?;
            digest.update(&(key.value().len() as u64).to_le_bytes());
            digest.update(key.value().as_bytes());
            digest.update(&(value.value().len() as u64).to_le_bytes());
            digest.update(value.value());
        }
        let rows = tx.open_table(RECORD_TABLE).map_err(vault_database_error)?;
        for row in rows.iter().map_err(vault_database_error)? {
            let (key, value) = row.map_err(vault_database_error)?;
            digest.update(key.value());
            digest.update(&(value.value().len() as u64).to_le_bytes());
            digest.update(value.value());
        }
        Ok(*digest.finalize().as_bytes())
    }
    fn replace_index(vault: &EncryptedStateVault, raw: &[u8]) -> Result<(), StateError> {
        let tx = vault.database.begin_write().map_err(vault_database_error)?;
        {
            let mut meta = tx.open_table(META_TABLE).map_err(vault_database_error)?;
            meta.insert(MANIFEST_INDEX_KEY, raw)
                .map_err(vault_database_error)?;
        }
        tx.commit().map_err(vault_database_error)
    }
    #[test]
    fn ordered_recovery_caps_index_before_decryption_or_shadow_mutation() -> Result<(), StateError>
    {
        // Local persistence-corruption fixture, not a Native/Head provenance origin.
        let tmp = io_at(Path::new("."), tempfile::tempdir())?;
        let root = tmp.path().join("profile");
        let mut store = create(&root)?;
        let head = "runtime/od1/contexts/0000000000000000000000000000000000000000000000000000000000000000/head";
        store.commit(
            store.generation(),
            vec![OrderedMutation::put(head, b"primary".to_vec())],
        )?;
        let generation = store.generation();
        let valid_digest = primary_digest(&store.vault)?;
        let original = store
            .vault
            .load_metadata(MANIFEST_INDEX_KEY)?
            .ok_or(StateError::VaultManifestIndexMissing)?;
        let mut oversized = original.clone();
        oversized.resize(INDEX_CAP + 42, 0);
        assert_eq!(oversized[0], VAULT_MANIFEST_INDEX_VERSION);
        replace_index(&store.vault, &oversized)?;
        let corrupt_digest = primary_digest(&store.vault)?;
        let shadow = root.join(head);
        io_at(&shadow, fs::write(&shadow, b"stale-shadow"))?;
        let stale = root.join(head.replace("/head", "/queue/stale"));
        let parent = stale
            .parent()
            .ok_or(StateError::OrderedState("test shadow parent"))?;
        io_at(&stale, fs::create_dir_all(parent))?;
        io_at(&stale, fs::write(&stale, b"stale-queue"))?;
        prepare_journal(&root)?;
        let journal = active_directory(&root).join(crate::MANIFEST_FILE);
        let journal_before = io_at(&journal, fs::read(&journal))?;
        let manifest: TransactionManifest =
            serde_json::from_slice(&journal_before).map_err(|source| {
                StateError::InvalidManifest {
                    path: journal.clone(),
                    source,
                }
            })?;
        assert_eq!(manifest.version, 2);
        assert!(
            active_directory(&root)
                .join(crate::PREPARED_MARKER)
                .exists()
        );
        assert!(matches!(
            store.vault.is_ordered_profile(),
            Err(StateError::OrderedState("index recovery cap"))
        ));
        drop(store);
        assert!(matches!(
            StateDirectoryLock::acquire(&root),
            Err(StateError::OrderedState("index recovery cap"))
        ));
        let vault = EncryptedStateVault::open_existing_cache(&root, Some(64 * 1024 * 1024))?;
        assert_eq!(primary_digest(&vault)?, corrupt_digest);
        assert_eq!(io_at(&shadow, fs::read(&shadow))?, b"stale-shadow");
        assert_eq!(io_at(&stale, fs::read(&stale))?, b"stale-queue");
        assert_eq!(io_at(&journal, fs::read(&journal))?, journal_before);
        assert!(
            active_directory(&root)
                .join(crate::PREPARED_MARKER)
                .exists()
        );
        // Restore only this disposable test's deliberately corrupted index. The
        // unchanged genuine prepared journal must now recover from the primary.
        replace_index(&vault, &original)?;
        drop(vault);
        let lease = StateDirectoryLock::acquire(&root)?;
        assert!(!active_directory(&root).exists());
        assert_eq!(io_at(&shadow, fs::read(&shadow))?, b"primary");
        assert!(!stale.exists());
        let store = OrderedVault::open(lease)?;
        assert_eq!(store.generation(), generation);
        assert_eq!(primary_digest(&store.vault)?, valid_digest);
        assert_eq!(store.get(head)?.as_deref(), Some(b"primary".as_slice()));
        println!(
            "R1 real_prepared_v2=true ciphertext_bytes={} supported_prefix=true cap_before_decrypt=true primary_shadow_journal_unchanged=true valid_recovery=true",
            oversized.len()
        );
        Ok(())
    }
    #[test]
    fn ordered_primary_is_atomic_fenced_and_reopens_after_failed_trial() -> Result<(), StateError> {
        let tmp = io_at(Path::new("."), tempfile::tempdir())?;
        let root = tmp.path().join("profile");
        let mut store = create(&root)?;
        let path = "runtime/od1/contexts/0000000000000000000000000000000000000000000000000000000000000000/head";
        let old = store.generation();
        assert!(store.vault.migrate_legacy_snapshot().is_err());
        assert!(StateTransaction::begin(&root).is_err());
        assert!(StateDirectoryLock::acquire(&root).is_err());
        let commit = store.commit(old, vec![OrderedMutation::put(path, b"offered".to_vec())])?;
        assert!(commit.visible);
        assert!(
            store
                .commit(old, vec![OrderedMutation::put(path, b"stale".to_vec())])
                .is_err()
        );
        let generation = store.generation();
        assert!(
            store
                .commit_internal(
                    generation,
                    vec![
                        OrderedMutation::put(path, b"selected".to_vec()),
                        OrderedMutation::put(EVENT, b"genesis".to_vec())
                    ],
                    Some(1)
                )
                .is_err()
        );
        drop(store);
        let mut store = OrderedVault::open(StateDirectoryLock::acquire(&root)?)?;
        assert_eq!(store.get(path)?.as_deref(), Some(b"offered".as_slice()));
        assert!(store.get(EVENT)?.is_none());
        let g = store.generation();
        store.commit(g, vec![OrderedMutation::put(EVENT, b"genesis".to_vec())])?;
        assert!(
            store
                .commit(
                    store.generation(),
                    vec![OrderedMutation::put(EVENT, b"changed".to_vec())]
                )
                .is_err()
        );
        assert!(
            store
                .commit(store.generation(), vec![OrderedMutation::remove(MODE_PATH)])
                .is_err()
        );
        Ok(())
    }
    #[test]
    fn ordered_schema_cannot_be_relabelled_as_legacy() -> Result<(), StateError> {
        let tmp = io_at(Path::new("."), tempfile::tempdir())?;
        let store = create(&tmp.path().join("profile"))?;
        let index = store
            .vault
            .load_manifest_index()?
            .ok_or(StateError::VaultManifestIndexMissing)?;
        let od1 = store.vault.manifest_for_index_schema(&index, 4)?;
        let legacy = store.vault.manifest_for_index_schema(&index, 3)?;
        assert_ne!(od1.snapshot_id, legacy.snapshot_id);
        assert_ne!(od1, legacy);
        let mut relabelled = od1;
        relabelled.schema_version = 3;
        let encoded = postcard::to_allocvec(&relabelled)?;
        let write = store
            .vault
            .database
            .begin_write()
            .map_err(vault_database_error)?;
        {
            let mut table = write.open_table(META_TABLE).map_err(vault_database_error)?;
            table
                .insert(MANIFEST_KEY, encoded.as_slice())
                .map_err(vault_database_error)?;
        }
        write.commit().map_err(vault_database_error)?;
        assert!(store.vault.verify().is_err());
        assert!(store.vault.migrate_legacy_snapshot().is_err());
        Ok(())
    }
    #[test]
    fn ordered_maximal_index_commit_and_prepaid_reservations_are_bounded() -> Result<(), StateError>
    {
        // An encoding/credit test, deliberately not a cryptographic fixture or
        // acceptance substitute. Lengths charge every possible full record.
        let mut entries = Vec::new();
        let mut add = |path: String, len: u64| {
            entries.push(VaultManifestIndexEntry {
                relative_path: path,
                content_len: len,
                content_hash: [7; 32],
            });
        };
        for c in 0..2_u64 {
            let c = format!("{c:064x}");
            let base = format!("runtime/od1/contexts/{c}");
            add(format!("{base}/spec"), 96 * 1024);
            add(format!("{base}/head"), 4096);
            add(format!("{base}/current-auth"), 192 * 1024);
            add(format!("{base}/initial-pair"), 192 * 1024);
            add(format!("ratchet/od1-contexts/{c}/native"), 128 * 1024);
            for role in 0..2_u64 {
                for n in 0..512_u64 {
                    let id = format!("{:064x}", role * 512 + n);
                    add(format!("events/od1/{c}/{id}"), 16 * 1024);
                    add(format!("local-messages/od1/{c}/{id}"), 8 * 1024);
                    add(format!("{base}/lane/{role}/{n:020}"), 32);
                    add(format!("{base}/receipts/{role}/{n:020}"), 1024);
                    add(format!("{base}/ack-intent/{role}/{n:020}"), 1024);
                    for k in 0..4_u64 {
                        add(
                            format!("runtime/od1/auth/{:064x}", entries_id(&c, role, n, k)),
                            16 * 1024,
                        );
                    }
                }
            }
            for n in 0..128_u64 {
                add(format!("{base}/queue/{n:020}-{n:032x}"), 8192);
            }
            for n in 0..65_u64 {
                add(format!("{base}/pending/{n:020}"), 16 * 1024);
                add(format!("{base}/pending-state/{n:020}"), 2048);
            }
        }
        let index = VaultManifestIndex {
            version: VAULT_MANIFEST_INDEX_VERSION,
            entries,
        };
        validate_reservations(&index)?;
        let encoded = postcard::to_allocvec(&index)?.len();
        let plan = 2 * encoded + 8 * (128 * 1024) + 2 * (128 * 1024) + 4096;
        println!(
            "OD1 maximal_index_bytes={encoded} old_new_commit_plan_bytes={plan} records={}",
            index.entries.len()
        );
        assert!(encoded <= INDEX_CAP && plan <= DELTA_CAP as usize);
        let mut overflow = index.clone();
        overflow.entries.push(VaultManifestIndexEntry {
            relative_path: format!(
                "runtime/od1/contexts/{:064x}/queue/{:020}-{:032x}",
                0, 128, 128
            ),
            content_len: 8192,
            content_hash: [1; 32],
        });
        assert!(validate_reservations(&overflow).is_err());
        assert!(record_cap("runtime/od1/contexts/c/head").is_none());
        Ok(())
    }
    fn entries_id(c: &str, role: u64, n: u64, k: u64) -> u64 {
        let context = u64::from(c.ends_with('1'));
        context * 4096 + role * 2048 + n * 4 + k
    }
}
