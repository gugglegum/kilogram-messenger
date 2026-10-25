//! Primary-only bounded V4 reader. It owns the same lifetime lock later used by Root7.
use super::super::{Config, Error, Id, Result, account_external, backend};
use super::codec::{self, Value, a, b, opt, u};
use crate::{
    StateDirectoryLock,
    vault::{self, EncryptedStateVault, VaultManifestIndexEntry},
};
use redb::{Database, ReadableDatabase, ReadableTable, ReadableTableMetadata};
use zeroize::Zeroizing;

pub const INDEX_CAP: usize = 8 * 1024 * 1024;
pub const SOURCE_CAP: u64 = 384 * 1024 * 1024;
pub(super) const MODE_PATH: &str = "runtime/od1/profile-mode";
pub(super) const MODE1: &[u8] = b"kilogram:ordered-dialog:v1\0";
pub(super) const MODE5: &[u8] = b"kilogram:ordered-dialog:v2\0";

#[derive(Clone, Debug)]
pub struct LegacyEntry {
    pub path: String,
    pub record_key: Id,
    pub cipher_hash: Id,
    pub content_hash: Id,
    pub length: u64,
    pub kind: u8,
    pub context: Option<Id>,
}
impl LegacyEntry {
    pub fn reference(&self, source: Id) -> Value {
        a([
            u(1),
            b(source),
            b(self.record_key),
            b(self.path.as_bytes()),
            u(self.kind.into()),
            opt(self.context.map(b)),
            b(self.cipher_hash),
            b(self.content_hash),
            u(self.length),
        ])
    }
}
pub struct V4Source {
    pub(super) vault: EncryptedStateVault,
    pub(super) lease: StateDirectoryLock,
    pub(super) backend: backend::BackendControl,
    pub(super) inventory: Inventory,
    pub(super) preflight: Option<super::preview::CheckedPreview>,
}
pub(super) struct Inventory {
    pub identity: Value,
    pub source_id: Id,
    pub entries: Vec<LegacyEntry>,
    pub generation: u64,
    pub manifest_raw: Vec<u8>,
    pub mode_raw: Vec<u8>,
    pub backing_digest: Id,
    pub backing_entries: u64,
    pub backing_bytes: u64,
}
fn metadata(vault: &EncryptedStateVault, key: &str, cap: usize) -> Result<Vec<u8>> {
    let tx = vault.database.begin_read().map_err(super::super::db)?;
    let table = tx.open_table(vault::META_TABLE).map_err(super::super::db)?;
    let raw = table
        .get(key)
        .map_err(super::super::db)?
        .ok_or(Error::Integrity("source metadata missing"))?;
    if raw.value().len() > cap {
        return Err(Error::Bound("source metadata cap"));
    }
    Ok(raw.value().to_vec())
}
fn encrypted(vault: &EncryptedStateVault, entry: &VaultManifestIndexEntry) -> Result<Vec<u8>> {
    let cap = vault::ordered::record_cap(&entry.relative_path)
        .ok_or(Error::Integrity("unsupported source path"))?;
    if entry.content_len > cap {
        return Err(Error::Bound("legacy typed record cap"));
    }
    let tx = vault.database.begin_read().map_err(super::super::db)?;
    let table = tx
        .open_table(vault::RECORD_TABLE)
        .map_err(super::super::db)?;
    let key = vault.record_key(&entry.relative_path);
    let raw = table
        .get(key.as_slice())
        .map_err(super::super::db)?
        .ok_or(Error::Integrity("source row missing"))?;
    if raw.value().len() as u64 > cap + 512 {
        return Err(Error::Bound("source envelope cap"));
    }
    Ok(raw.value().to_vec())
}
fn classify(path: &str) -> Result<(u8, Option<Id>)> {
    let parts = path.split('/').collect::<Vec<_>>();
    let context = match parts.as_slice() {
        ["runtime", "od1", "contexts", c, ..]
        | ["events", "od1", c, ..]
        | ["local-messages", "od1", c, ..]
        | ["ratchet", "od1-contexts", c, ..] => Some(hex_id(c)?),
        _ => None,
    };
    let kind = match path {
        MODE_PATH => 0,
        "runtime/od1/profile-claim" => 1,
        "device-secret.key" => 2,
        "device-encryption-secret.key" => 3,
        "device-certificate.cert" => 4,
        "account-authority.snapshot" => 5,
        "next-sequence" => 6,
        "ratchet/account.pickle" => 29,
        "ratchet/pickle-secret.key" => 30,
        _ => match parts.as_slice() {
            ["runtime", "od1", "auth", _] => 7,
            ["runtime", "od1", "contexts", _, "spec"] => 8,
            ["runtime", "od1", "contexts", _, "head"] => 9,
            ["ratchet", "od1-contexts", _, "native"] => 10,
            ["events", "od1", _, _] => 11,
            ["local-messages", "od1", _, _] => 12,
            ["runtime", "od1", "contexts", _, "queue", _] => 13,
            ["runtime", "od1", "contexts", _, "pending", _] => 14,
            ["runtime", "od1", "contexts", _, "pending-state", _] => 15,
            ["runtime", "od1", "contexts", _, "lane", _, _] => 16,
            ["runtime", "od1", "contexts", _, "receipts", _, _] => 17,
            ["runtime", "od1", "contexts", _, "ack-intent", _, _] => 18,
            ["runtime", "od1", "contexts", _, "outbox", _] => 19,
            ["runtime", "od1", "contexts", _, "requests", _] => 20,
            ["runtime", "od1", "contexts", _, "controls", _] => 21,
            ["runtime", "od1", "contexts", _, "ready", _] => 22,
            [
                "runtime",
                "od1",
                "contexts",
                _,
                "local-locator" | "peer-locator",
            ] => 23,
            ["runtime", "od1", "contexts", _, "consent-current"] => 24,
            ["runtime", "od1", "contexts", _, "initial-pair"] => 25,
            ["runtime", "od1", "contexts", _, "current-auth"] => 26,
            ["runtime", "od1", "contexts", _, "blocked-fork"] => 27,
            ["runtime", "od1", "authored", _, _] => 28,
            ["peer-authority", _] => 31,
            _ => return Err(Error::Integrity("unclassified source")),
        },
    };
    Ok((kind, context))
}
pub(super) fn hex_id(text: &str) -> Result<Id> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
    {
        return Err(Error::Integrity("source lowercase ID"));
    }
    let mut id = [0; 32];
    for (i, x) in id.iter_mut().enumerate() {
        *x = u8::from_str_radix(&text[2 * i..2 * i + 2], 16)
            .map_err(|_| Error::Integrity("source ID"))?;
    }
    Ok(id)
}
pub(super) fn hex(raw: &[u8]) -> String {
    raw.iter().map(|x| format!("{x:02x}")).collect()
}

impl V4Source {
    pub fn open(lease: StateDirectoryLock, config: Config) -> Result<Self> {
        let key =
            crate::key_provider::load_master_key(&lease.root().join(vault::STATE_VAULT_KEY_FILE))?;
        crate::reject_symlink(&lease.root().join(vault::STATE_VAULT_FILE))?;
        let config = account_external(
            lease.root(),
            super::preview::physical_config(lease.root(), config)?,
        )?;
        let (backend, control) =
            backend::Backend::open(&lease.root().join(vault::STATE_VAULT_FILE), false, config)?;
        let mut builder = Database::builder();
        builder.set_cache_size(64 * 1024 * 1024);
        let database = builder
            .create_with_backend(backend)
            .map_err(super::super::db)?;
        let vault = EncryptedStateVault {
            root: lease.root().to_path_buf(),
            database,
            master_key: key.master_key,
            key_protection: key.protection,
            key_load_outcome: key.load_outcome,
        };
        let inventory = Inventory::load(&vault)?;
        Ok(Self {
            vault,
            lease,
            backend: control,
            inventory,
            preflight: None,
        })
    }
    pub fn entries(&self) -> &[LegacyEntry] {
        &self.inventory.entries
    }
    pub fn generation(&self) -> u64 {
        self.inventory.generation
    }
    pub fn source_id(&self) -> Id {
        self.inventory.source_id
    }
    pub fn root(&self) -> &std::path::Path {
        self.lease.root()
    }
    pub fn get(&self, path: &str) -> Result<Option<Zeroizing<Vec<u8>>>> {
        self.inventory.get(&self.vault, path, None)
    }
    pub fn required(&self, path: &str) -> Result<Zeroizing<Vec<u8>>> {
        self.get(path)?
            .ok_or(Error::Integrity("required legacy row absent"))
    }
}
impl Inventory {
    fn load(vault: &EncryptedStateVault) -> Result<Self> {
        Self::load_with(vault, None)
    }
    pub(super) fn resume(
        vault: &EncryptedStateVault,
        identity: &Value,
        mode: &[u8],
    ) -> Result<Self> {
        Self::load_with(vault, Some((identity, mode)))
    }
    fn load_with(vault: &EncryptedStateVault, saved: Option<(&Value, &[u8])>) -> Result<Self> {
        if saved.is_none() {
            let tx = vault.database.begin_read().map_err(super::super::db)?;
            let meta = tx.open_table(vault::META_TABLE).map_err(super::super::db)?;
            if meta
                .get(super::super::ANCHOR)
                .map_err(super::super::db)?
                .is_some()
            {
                return Err(Error::CreationConflict);
            }
        }
        let manifest_raw = if let Some((identity, _)) = saved {
            identity.fields(9)?[1].bytes()?.to_vec()
        } else {
            metadata(vault, vault::MANIFEST_KEY, 256)?
        };
        let manifest: vault::VaultManifest =
            postcard::from_bytes(&manifest_raw).map_err(|_| Error::Integrity("source manifest"))?;
        if manifest.schema_version != 4 || manifest.plaintext_bytes > SOURCE_CAP {
            return Err(Error::ExistingStateRequiresMigration);
        }
        let index_cipher = metadata(vault, vault::MANIFEST_INDEX_KEY, INDEX_CAP + 41)?;
        if let Some((identity, _)) = saved
            && identity.fields(9)?[2].fixed::<32>()? != *blake3::hash(&index_cipher).as_bytes()
        {
            return Err(Error::Integrity("retained source index changed"));
        }
        let index = vault.decrypt_manifest_index(&index_cipher)?;
        if index
            .entries
            .windows(2)
            .any(|v| v[0].relative_path >= v[1].relative_path)
        {
            return Err(Error::Integrity("source index canonical order"));
        }
        if vault.manifest_for_index_schema(&index, 4)? != manifest {
            return Err(Error::Integrity("source index MAC"));
        }
        vault::ordered::validate_reservations(&index)?;
        let generation_raw = metadata(vault, vault::GENERATION_KEY, 1024)?;
        if let Some((identity, _)) = saved
            && identity.fields(9)?[4].fixed::<32>()? != *blake3::hash(&generation_raw).as_bytes()
        {
            return Err(Error::Integrity("retained source generation changed"));
        }
        let generation = vault.load_generation(&manifest)?;
        let mut entries = Vec::with_capacity(index.entries.len());
        let mut fold = [0; 32];
        let mut mode_raw = None;
        for entry in &index.entries {
            let cipher = if entry.relative_path == MODE_PATH {
                if let Some((_, mode)) = saved {
                    let current = encrypted(vault, entry)?;
                    if vault
                        .decrypt_record(&current, &vault.record_key(MODE_PATH))?
                        .content
                        != MODE5
                    {
                        return Err(Error::Integrity("required current mode fence"));
                    }
                    mode.to_vec()
                } else {
                    encrypted(vault, entry)?
                }
            } else {
                encrypted(vault, entry)?
            };
            let record = vault.decrypt_record(&cipher, &vault.record_key(&entry.relative_path))?;
            if vault::manifest_index_entry_for_record(&record)? != *entry {
                return Err(Error::Integrity("source index membership"));
            }
            if entry.relative_path == MODE_PATH {
                if record.content != MODE1 {
                    return Err(Error::Integrity("source mode"));
                }
                mode_raw = Some(cipher.clone());
            }
            let (kind, context) = classify(&entry.relative_path)?;
            let item = LegacyEntry {
                path: entry.relative_path.clone(),
                record_key: vault.record_key(&entry.relative_path),
                cipher_hash: *blake3::hash(&cipher).as_bytes(),
                content_hash: entry.content_hash,
                length: entry.content_len,
                kind,
                context,
            };
            fold = codec::digest(
                b"kg:od2:migration-inventory\0",
                &a([b(fold), item.reference([0; 32])]),
            )?;
            entries.push(item);
        }
        // Every physical source row must belong to the authenticated index; no hidden orphan adoption.
        let tx = vault.database.begin_read().map_err(super::super::db)?;
        let records = tx
            .open_table(vault::RECORD_TABLE)
            .map_err(super::super::db)?;
        if records.len().map_err(super::super::db)? != entries.len() as u64 {
            return Err(Error::Integrity("source record cardinality"));
        }
        let mode_raw = mode_raw.ok_or(Error::Integrity("source mode absent"))?;
        let inventory_digest = fold;
        let claim_entry = entries
            .iter()
            .find(|e| e.kind == 1)
            .ok_or(Error::CreationConflict)?;
        let raw = encrypted(
            vault,
            &VaultManifestIndexEntry {
                relative_path: claim_entry.path.clone(),
                content_len: claim_entry.length,
                content_hash: claim_entry.content_hash,
            },
        )?;
        let claim = vault.decrypt_record(&raw, &claim_entry.record_key)?.content;
        let c = codec::decode(&claim, 1024)?;
        let cf = c.fields(5)?;
        if cf[0].uint()? != 1 || cf[1].uint()? != 4 {
            return Err(Error::CreationConflict);
        }
        let identity = a([
            u(1),
            b(&manifest_raw),
            b(blake3::hash(&index_cipher).as_bytes()),
            u(generation),
            b(blake3::hash(&generation_raw).as_bytes()),
            b(&claim),
            b(cf[3].fixed::<32>()?),
            b(cf[4].fixed::<32>()?),
            b(inventory_digest),
        ]);
        let source_id = codec::digest(b"kg:od2:migration-source\0", &identity)?;
        if saved.is_some_and(|(old, _)| *old != identity) {
            return Err(Error::Integrity("source inventory/identity changed"));
        }
        let (backing_digest, backing_entries, backing_bytes) = Self::backing(vault, None)?;
        Ok(Self {
            identity,
            source_id,
            entries,
            generation,
            manifest_raw,
            mode_raw,
            backing_digest,
            backing_entries,
            backing_bytes,
        })
    }
    pub(super) fn backing(
        vault: &EncryptedStateVault,
        replacement: Option<(Id, &[u8])>,
    ) -> Result<(Id, u64, u64)> {
        let tx = vault.database.begin_read().map_err(super::super::db)?;
        let records = tx
            .open_table(vault::RECORD_TABLE)
            .map_err(super::super::db)?;
        let mut digest = blake3::Hasher::new();
        digest.update(b"kg:od2:retained-source-entries\0");
        let mut count = 0u64;
        let mut bytes = 0u64;
        for entry in records.iter().map_err(super::super::db)? {
            let (key, value) = entry.map_err(super::super::db)?;
            let body = replacement
                .filter(|(id, _)| id.as_slice() == key.value())
                .map_or(value.value(), |(_, raw)| raw);
            if key.value().len() != 32 || body.len() > 192 * 1024 + 512 {
                return Err(Error::Integrity("source physical row cap"));
            }
            digest.update(&codec::encode(
                &a([
                    b(key.value()),
                    b(blake3::hash(body).as_bytes()),
                    u(body.len() as u64),
                ]),
                256,
            )?);
            bytes = bytes
                .checked_add((key.value().len() + body.len()) as u64)
                .ok_or(Error::Bound("backing bytes"))?;
            count += 1;
        }
        for name in [vault::MANIFEST_INDEX_KEY, vault::GENERATION_KEY] {
            let value = metadata(vault, name, INDEX_CAP + 41)?;
            digest.update(&codec::encode(
                &a([
                    b(name.as_bytes()),
                    b(blake3::hash(&value).as_bytes()),
                    u(value.len() as u64),
                ]),
                256,
            )?);
            bytes = bytes
                .checked_add((name.len() + value.len()) as u64)
                .ok_or(Error::Bound("backing bytes"))?;
            count += 1;
        }
        Ok((*digest.finalize().as_bytes(), count, bytes))
    }
    pub(super) fn get(
        &self,
        vault: &EncryptedStateVault,
        path: &str,
        mode_override: Option<&[u8]>,
    ) -> Result<Option<Zeroizing<Vec<u8>>>> {
        let Ok(position) = self
            .entries
            .binary_search_by(|entry| entry.path.as_str().cmp(path))
        else {
            return Ok(None);
        };
        let entry = &self.entries[position];
        let index = VaultManifestIndexEntry {
            relative_path: entry.path.clone(),
            content_len: entry.length,
            content_hash: entry.content_hash,
        };
        let cipher = if path == MODE_PATH {
            mode_override
                .map(ToOwned::to_owned)
                .map(Ok)
                .unwrap_or_else(|| encrypted(vault, &index))?
        } else {
            encrypted(vault, &index)?
        };
        if *blake3::hash(&cipher).as_bytes() != entry.cipher_hash {
            return Err(Error::Integrity("source cipher changed"));
        }
        let record = vault.decrypt_record(&cipher, &entry.record_key)?;
        if vault::manifest_index_entry_for_record(&record)? != index {
            return Err(Error::Integrity("source ref membership"));
        }
        Ok(Some(Zeroizing::new(record.content)))
    }
}
