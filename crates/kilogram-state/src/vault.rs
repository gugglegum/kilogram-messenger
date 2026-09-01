use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, Payload},
};
use redb::{Database, Durability, ReadableDatabase, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use zeroize::ZeroizeOnDrop;

use crate::{StateError, io_at, path_exists, reject_symlink, sync_directory, validate_relative};

pub const STATE_VAULT_FILE: &str = "state-vault.redb";
pub const STATE_VAULT_KEY_FILE: &str = "state-vault.key";

const VAULT_SCHEMA_VERSION: u64 = 1;
const VAULT_RECORD_VERSION: u8 = 1;
const VAULT_NONCE_BYTES: usize = 24;
const VAULT_KEY_BYTES: usize = 32;
const MAX_VAULT_RECORD_BYTES: usize = 64 * 1024 * 1024;
const MAX_VAULT_RECORDS: usize = 1_000_000;
const MAX_VAULT_SNAPSHOT_BYTES: u64 = 512 * 1024 * 1024;
const MANIFEST_KEY: &str = "snapshot-manifest";
const RECORD_KEY_DOMAIN: &str = "kilogram state vault record lookup v1";
const ENCRYPTION_KEY_DOMAIN: &str = "kilogram state vault encryption v1";
const SNAPSHOT_KEY_DOMAIN: &str = "kilogram state vault snapshot v1";
const RECORD_AAD_DOMAIN: &[u8] = b"kilogram:state-vault-record-aad:v1\0";

const META_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("vault-meta-v1");
const RECORD_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("vault-records-v1");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VaultMigrationOutcome {
    Migrated,
    AlreadyCurrent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultReport {
    schema_version: u64,
    record_count: u64,
    plaintext_bytes: u64,
    snapshot_id: [u8; 32],
}

impl VaultReport {
    pub fn schema_version(&self) -> u64 {
        self.schema_version
    }

    pub fn record_count(&self) -> u64 {
        self.record_count
    }

    pub fn plaintext_bytes(&self) -> u64 {
        self.plaintext_bytes
    }

    pub fn snapshot_id(&self) -> &[u8; 32] {
        &self.snapshot_id
    }
}

#[derive(ZeroizeOnDrop)]
struct VaultMasterKey([u8; VAULT_KEY_BYTES]);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct VaultManifest {
    schema_version: u64,
    record_count: u64,
    plaintext_bytes: u64,
    snapshot_id: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct VaultRecord {
    version: u8,
    relative_path: String,
    content: Vec<u8>,
}

pub struct EncryptedStateVault {
    root: PathBuf,
    database: Database,
    master_key: VaultMasterKey,
}

impl EncryptedStateVault {
    pub fn open_or_create(state_directory: impl AsRef<Path>) -> Result<Self, StateError> {
        let requested = state_directory.as_ref();
        io_at(requested, fs::create_dir_all(requested))?;
        let root = io_at(requested, fs::canonicalize(requested))?;
        let database_path = root.join(STATE_VAULT_FILE);
        let key_path = root.join(STATE_VAULT_KEY_FILE);
        let database_exists = path_exists(&database_path)?;
        let key_exists = path_exists(&key_path)?;
        if database_exists {
            reject_symlink(&database_path)?;
        }
        if key_exists {
            reject_symlink(&key_path)?;
        }
        if database_exists && !key_exists {
            return Err(StateError::VaultKeyMissing(key_path));
        }
        let master_key = load_or_create_master_key(&key_path)?;
        let database = Database::create(&database_path).map_err(vault_database_error)?;
        Ok(Self {
            root,
            database,
            master_key,
        })
    }

    pub fn open_existing(state_directory: impl AsRef<Path>) -> Result<Self, StateError> {
        let requested = state_directory.as_ref();
        let root = io_at(requested, fs::canonicalize(requested))?;
        let database_path = root.join(STATE_VAULT_FILE);
        let key_path = root.join(STATE_VAULT_KEY_FILE);
        if !path_exists(&database_path)? {
            return Err(StateError::VaultDatabaseMissing(database_path));
        }
        reject_symlink(&database_path)?;
        if !path_exists(&key_path)? {
            return Err(StateError::VaultKeyMissing(key_path));
        }
        reject_symlink(&key_path)?;
        let master_key = load_master_key(&key_path)?;
        let database = Database::open(&database_path).map_err(vault_database_error)?;
        Ok(Self {
            root,
            database,
            master_key,
        })
    }

    pub fn migrate_legacy_snapshot(
        &self,
    ) -> Result<(VaultMigrationOutcome, VaultReport), StateError> {
        let records = collect_legacy_records(&self.root)?;
        let manifest = self.manifest_for_records(&records)?;
        match self.load_manifest()? {
            Some(existing) if existing == manifest => {
                let report = self.verify()?;
                return Ok((VaultMigrationOutcome::AlreadyCurrent, report));
            }
            Some(existing) => {
                return Err(StateError::VaultLegacyStateChanged {
                    stored: existing.snapshot_id,
                    current: manifest.snapshot_id,
                });
            }
            None => {}
        }
        self.commit_snapshot(&records, &manifest, None)?;
        let report = self.verify()?;
        Ok((VaultMigrationOutcome::Migrated, report))
    }

    pub fn verify(&self) -> Result<VaultReport, StateError> {
        let manifest = self
            .load_manifest()?
            .ok_or_else(|| StateError::VaultNotMigrated(self.root.clone()))?;
        validate_manifest(&manifest)?;
        let records = self.load_records()?;
        let observed = self.manifest_for_records(&records)?;
        if observed != manifest {
            return Err(StateError::VaultManifestMismatch);
        }
        Ok(report_from_manifest(&manifest))
    }

    pub fn verify_against_legacy(&self) -> Result<VaultReport, StateError> {
        let report = self.verify()?;
        let records = collect_legacy_records(&self.root)?;
        let current = self.manifest_for_records(&records)?;
        if current.snapshot_id != *report.snapshot_id()
            || current.record_count != report.record_count()
            || current.plaintext_bytes != report.plaintext_bytes()
        {
            return Err(StateError::VaultLegacyStateChanged {
                stored: *report.snapshot_id(),
                current: current.snapshot_id,
            });
        }
        Ok(report)
    }

    pub fn restore_to_new_directory(
        &self,
        destination: impl AsRef<Path>,
    ) -> Result<VaultReport, StateError> {
        let report = self.verify()?;
        let destination = absolute_path(destination.as_ref())?;
        if path_exists(&destination)? {
            return Err(StateError::VaultRestoreDestinationExists(destination));
        }
        let parent = destination
            .parent()
            .ok_or_else(|| StateError::UnsafeRelativePath(destination.clone()))?;
        io_at(parent, fs::create_dir_all(parent))?;
        let mut staging = TempDir::new_in(parent).map_err(|source| StateError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
        for record in self.load_records()? {
            let relative = path_from_vault(&record.relative_path)?;
            let output = staging.path().join(relative);
            if let Some(output_parent) = output.parent() {
                io_at(output_parent, fs::create_dir_all(output_parent))?;
            }
            let mut file = open_new_private_file(&output).map_err(|source| StateError::Io {
                path: output.clone(),
                source,
            })?;
            io_at(&output, file.write_all(&record.content))?;
            io_at(&output, file.sync_all())?;
        }
        let restored_records = collect_legacy_records(staging.path())?;
        let restored_manifest = self.manifest_for_records(&restored_records)?;
        if restored_manifest.snapshot_id != *report.snapshot_id()
            || restored_manifest.record_count != report.record_count()
            || restored_manifest.plaintext_bytes != report.plaintext_bytes()
        {
            return Err(StateError::VaultManifestMismatch);
        }
        sync_directory(staging.path())?;
        io_at(&destination, fs::rename(staging.path(), &destination))?;
        staging.disable_cleanup(true);
        sync_directory(parent)?;
        Ok(report)
    }

    fn commit_snapshot(
        &self,
        records: &[VaultRecord],
        manifest: &VaultManifest,
        fail_after_records: Option<usize>,
    ) -> Result<(), StateError> {
        let mut encrypted_records = Vec::with_capacity(records.len());
        for record in records {
            let record_key = self.record_key(&record.relative_path);
            let encrypted = self.encrypt_record(record, &record_key)?;
            encrypted_records.push((record_key, encrypted));
        }
        let encoded_manifest = postcard::to_allocvec(manifest)?;
        let mut write = self.database.begin_write().map_err(vault_database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(vault_database_error)?;
        {
            let mut table = write
                .open_table(RECORD_TABLE)
                .map_err(vault_database_error)?;
            table.retain(|_, _| false).map_err(vault_database_error)?;
            for (index, (key, value)) in encrypted_records.iter().enumerate() {
                table
                    .insert(key.as_slice(), value.as_slice())
                    .map_err(vault_database_error)?;
                if fail_after_records == Some(index + 1) {
                    return Err(StateError::VaultInjectedFailure(index + 1));
                }
            }
        }
        {
            let mut table = write.open_table(META_TABLE).map_err(vault_database_error)?;
            table
                .insert(MANIFEST_KEY, encoded_manifest.as_slice())
                .map_err(vault_database_error)?;
        }
        write.commit().map_err(vault_database_error)
    }

    fn load_manifest(&self) -> Result<Option<VaultManifest>, StateError> {
        let read = self.database.begin_read().map_err(vault_database_error)?;
        let table = match read.open_table(META_TABLE) {
            Ok(table) => table,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(error) => return Err(vault_database_error(error)),
        };
        let encoded = table
            .get(MANIFEST_KEY)
            .map_err(vault_database_error)?
            .map(|value| value.value().to_vec());
        encoded
            .map(|bytes| postcard::from_bytes(&bytes).map_err(StateError::from))
            .transpose()
    }

    fn load_records(&self) -> Result<Vec<VaultRecord>, StateError> {
        let read = self.database.begin_read().map_err(vault_database_error)?;
        let table = read
            .open_table(RECORD_TABLE)
            .map_err(vault_database_error)?;
        let mut records = Vec::new();
        let mut paths = BTreeSet::new();
        for entry in table.iter().map_err(vault_database_error)? {
            let (key, value) = entry.map_err(vault_database_error)?;
            let key_bytes = key.value();
            let record_key: [u8; 32] = key_bytes
                .try_into()
                .map_err(|_| StateError::InvalidVaultRecordKeyLength(key_bytes.len()))?;
            let record = self.decrypt_record(value.value(), &record_key)?;
            if self.record_key(&record.relative_path) != record_key {
                return Err(StateError::VaultRecordKeyMismatch(record.relative_path));
            }
            if !paths.insert(record.relative_path.clone()) {
                return Err(StateError::VaultDuplicatePath(record.relative_path));
            }
            records.push(record);
        }
        records.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        Ok(records)
    }

    fn manifest_for_records(&self, records: &[VaultRecord]) -> Result<VaultManifest, StateError> {
        if records.len() > MAX_VAULT_RECORDS {
            return Err(StateError::TooManyVaultRecords(records.len()));
        }
        let plaintext_bytes = records.iter().try_fold(0_u64, |total, record| {
            let size = u64::try_from(record.content.len())
                .map_err(|_| StateError::VaultRecordTooLarge(record.content.len()))?;
            total
                .checked_add(size)
                .ok_or(StateError::VaultSnapshotTooLarge(u64::MAX))
        })?;
        if plaintext_bytes > MAX_VAULT_SNAPSHOT_BYTES {
            return Err(StateError::VaultSnapshotTooLarge(plaintext_bytes));
        }
        let snapshot_key = blake3::derive_key(SNAPSHOT_KEY_DOMAIN, &self.master_key.0);
        let mut hasher = blake3::Hasher::new_keyed(&snapshot_key);
        for record in records {
            let path = record.relative_path.as_bytes();
            hasher.update(&(path.len() as u64).to_be_bytes());
            hasher.update(path);
            hasher.update(&(record.content.len() as u64).to_be_bytes());
            hasher.update(&record.content);
        }
        Ok(VaultManifest {
            schema_version: VAULT_SCHEMA_VERSION,
            record_count: records.len() as u64,
            plaintext_bytes,
            snapshot_id: *hasher.finalize().as_bytes(),
        })
    }

    fn record_key(&self, relative_path: &str) -> [u8; 32] {
        let lookup_key = blake3::derive_key(RECORD_KEY_DOMAIN, &self.master_key.0);
        *blake3::keyed_hash(&lookup_key, relative_path.as_bytes()).as_bytes()
    }

    fn encrypt_record(
        &self,
        record: &VaultRecord,
        record_key: &[u8; 32],
    ) -> Result<Vec<u8>, StateError> {
        let plaintext = postcard::to_allocvec(record)?;
        let mut nonce = [0_u8; VAULT_NONCE_BYTES];
        getrandom::fill(&mut nonce).map_err(StateError::VaultSecureRandom)?;
        let encryption_key = blake3::derive_key(ENCRYPTION_KEY_DOMAIN, &self.master_key.0);
        let cipher = XChaCha20Poly1305::new((&encryption_key).into());
        let aad = record_aad(record_key);
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| StateError::VaultEncryption)?;
        let mut encoded = Vec::with_capacity(1 + VAULT_NONCE_BYTES + ciphertext.len());
        encoded.push(VAULT_RECORD_VERSION);
        encoded.extend_from_slice(&nonce);
        encoded.extend_from_slice(&ciphertext);
        Ok(encoded)
    }

    fn decrypt_record(
        &self,
        encoded: &[u8],
        record_key: &[u8; 32],
    ) -> Result<VaultRecord, StateError> {
        if encoded.len() <= 1 + VAULT_NONCE_BYTES {
            return Err(StateError::InvalidVaultRecordEnvelope);
        }
        if encoded[0] != VAULT_RECORD_VERSION {
            return Err(StateError::UnsupportedVaultRecordVersion(encoded[0]));
        }
        let nonce = XNonce::from_slice(&encoded[1..1 + VAULT_NONCE_BYTES]);
        let encryption_key = blake3::derive_key(ENCRYPTION_KEY_DOMAIN, &self.master_key.0);
        let cipher = XChaCha20Poly1305::new((&encryption_key).into());
        let aad = record_aad(record_key);
        let plaintext = cipher
            .decrypt(
                nonce,
                Payload {
                    msg: &encoded[1 + VAULT_NONCE_BYTES..],
                    aad: &aad,
                },
            )
            .map_err(|_| StateError::VaultEncryption)?;
        let record: VaultRecord = postcard::from_bytes(&plaintext)?;
        validate_record(&record)?;
        Ok(record)
    }
}

fn collect_legacy_records(root: &Path) -> Result<Vec<VaultRecord>, StateError> {
    let mut records = Vec::new();
    collect_legacy_records_from(root, root, &mut records)?;
    records.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    if records.len() > MAX_VAULT_RECORDS {
        return Err(StateError::TooManyVaultRecords(records.len()));
    }
    Ok(records)
}

fn collect_legacy_records_from(
    root: &Path,
    directory: &Path,
    records: &mut Vec<VaultRecord>,
) -> Result<(), StateError> {
    reject_symlink(directory)?;
    for entry in io_at(directory, fs::read_dir(directory))? {
        let entry = entry.map_err(|source| StateError::Io {
            path: directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .map_err(|_| StateError::UnsafeRelativePath(path.clone()))?;
        if is_vault_excluded(relative) {
            continue;
        }
        let metadata = io_at(&path, fs::symlink_metadata(&path))?;
        if metadata.file_type().is_symlink() {
            return Err(StateError::SymbolicLink(path));
        }
        if metadata.is_dir() {
            collect_legacy_records_from(root, &path, records)?;
        } else if metadata.is_file() {
            let relative_path = path_to_vault(relative)?;
            let content = io_at(&path, fs::read(&path))?;
            if content.len() > MAX_VAULT_RECORD_BYTES {
                return Err(StateError::VaultRecordTooLarge(content.len()));
            }
            records.push(VaultRecord {
                version: VAULT_RECORD_VERSION,
                relative_path,
                content,
            });
        }
    }
    Ok(())
}

fn is_vault_excluded(relative: &Path) -> bool {
    let mut components = relative.components();
    let Some(Component::Normal(first)) = components.next() else {
        return false;
    };
    matches!(
        first.to_str(),
        Some(
            STATE_VAULT_FILE
                | STATE_VAULT_KEY_FILE
                | ".kilogram-state.lock"
                | ".kilogram-transactions"
        )
    )
}

fn validate_record(record: &VaultRecord) -> Result<(), StateError> {
    if record.version != VAULT_RECORD_VERSION {
        return Err(StateError::UnsupportedVaultRecordVersion(record.version));
    }
    path_from_vault(&record.relative_path)?;
    if record.content.len() > MAX_VAULT_RECORD_BYTES {
        return Err(StateError::VaultRecordTooLarge(record.content.len()));
    }
    Ok(())
}

fn validate_manifest(manifest: &VaultManifest) -> Result<(), StateError> {
    if manifest.schema_version != VAULT_SCHEMA_VERSION {
        return Err(StateError::UnsupportedVaultSchemaVersion(
            manifest.schema_version,
        ));
    }
    if manifest.record_count > MAX_VAULT_RECORDS as u64 {
        return Err(StateError::TooManyVaultRecords(usize::MAX));
    }
    if manifest.plaintext_bytes > MAX_VAULT_SNAPSHOT_BYTES {
        return Err(StateError::VaultSnapshotTooLarge(manifest.plaintext_bytes));
    }
    Ok(())
}

fn report_from_manifest(manifest: &VaultManifest) -> VaultReport {
    VaultReport {
        schema_version: manifest.schema_version,
        record_count: manifest.record_count,
        plaintext_bytes: manifest.plaintext_bytes,
        snapshot_id: manifest.snapshot_id,
    }
}

fn load_or_create_master_key(path: &Path) -> Result<VaultMasterKey, StateError> {
    match open_new_private_file(path) {
        Ok(mut file) => {
            let mut key = [0_u8; VAULT_KEY_BYTES];
            getrandom::fill(&mut key).map_err(StateError::VaultSecureRandom)?;
            io_at(path, file.write_all(&key))?;
            io_at(path, file.sync_all())?;
            if let Some(parent) = path.parent() {
                sync_directory(parent)?;
            }
            Ok(VaultMasterKey(key))
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            reject_symlink(path)?;
            load_master_key(path)
        }
        Err(source) => Err(StateError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn load_master_key(path: &Path) -> Result<VaultMasterKey, StateError> {
    let bytes = io_at(path, fs::read(path))?;
    let key = bytes
        .try_into()
        .map_err(|bytes: Vec<u8>| StateError::InvalidVaultKeyLength(bytes.len()))?;
    Ok(VaultMasterKey(key))
}

fn open_new_private_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    options.open(path)
}

fn path_to_vault(path: &Path) -> Result<String, StateError> {
    validate_relative(path)?;
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(part) = component else {
            return Err(StateError::UnsafeRelativePath(path.to_path_buf()));
        };
        let part = part
            .to_str()
            .ok_or_else(|| StateError::VaultNonUtf8Path(path.to_path_buf()))?;
        parts.push(part);
    }
    Ok(parts.join("/"))
}

fn path_from_vault(value: &str) -> Result<PathBuf, StateError> {
    if value.is_empty()
        || value.contains('\\')
        || value
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(StateError::UnsafeRelativePath(PathBuf::from(value)));
    }
    let path = value.split('/').fold(PathBuf::new(), |mut path, part| {
        path.push(part);
        path
    });
    validate_relative(&path)?;
    Ok(path)
}

fn record_aad(record_key: &[u8; 32]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(RECORD_AAD_DOMAIN.len() + 8 + record_key.len());
    aad.extend_from_slice(RECORD_AAD_DOMAIN);
    aad.extend_from_slice(&VAULT_SCHEMA_VERSION.to_be_bytes());
    aad.extend_from_slice(record_key);
    aad
}

fn absolute_path(path: &Path) -> Result<PathBuf, StateError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        let current = std::env::current_dir().map_err(|source| StateError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(current.join(path))
    }
}

fn vault_database_error(error: impl std::fmt::Display) -> StateError {
    StateError::VaultDatabase(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    fn write(path: &Path, value: &[u8]) -> Result<(), Box<dyn Error>> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, value)?;
        Ok(())
    }

    #[test]
    fn encrypted_vault_migrates_verifies_and_restores_exact_snapshot() -> Result<(), Box<dyn Error>>
    {
        let directory = tempfile::tempdir()?;
        write(
            &directory.path().join("device-secret.key"),
            b"secret-key-material",
        )?;
        write(
            &directory.path().join("events/chat/event.event"),
            b"ciphertext-event",
        )?;
        write(
            &directory.path().join("ratchet/session.pickle"),
            b"ratchet-state",
        )?;

        let vault = EncryptedStateVault::open_or_create(directory.path())?;
        let (outcome, migrated) = vault.migrate_legacy_snapshot()?;
        assert_eq!(outcome, VaultMigrationOutcome::Migrated);
        assert_eq!(migrated.record_count(), 3);
        assert_eq!(vault.verify_against_legacy()?, migrated);
        assert_eq!(
            vault.migrate_legacy_snapshot()?.0,
            VaultMigrationOutcome::AlreadyCurrent
        );
        drop(vault);

        let database_bytes = fs::read(directory.path().join(STATE_VAULT_FILE))?;
        assert!(
            !database_bytes
                .windows(b"secret-key-material".len())
                .any(|window| { window == b"secret-key-material" })
        );
        assert!(
            !database_bytes
                .windows(b"ciphertext-event".len())
                .any(|window| { window == b"ciphertext-event" })
        );
        assert!(
            !database_bytes
                .windows("device-secret.key".len())
                .any(|window| { window == b"device-secret.key" })
        );

        let vault = EncryptedStateVault::open_or_create(directory.path())?;
        let restored = directory.path().join("restored");
        let restored_report = vault.restore_to_new_directory(&restored)?;
        assert_eq!(restored_report, migrated);
        assert_eq!(
            fs::read(restored.join("device-secret.key"))?,
            b"secret-key-material"
        );
        assert_eq!(
            fs::read(restored.join("events/chat/event.event"))?,
            b"ciphertext-event"
        );
        assert!(!restored.join(STATE_VAULT_FILE).exists());
        assert!(!restored.join(STATE_VAULT_KEY_FILE).exists());
        Ok(())
    }

    #[test]
    fn failed_vault_transaction_is_not_visible_and_legacy_drift_is_detected()
    -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        write(&directory.path().join("first"), b"one")?;
        write(&directory.path().join("second"), b"two")?;
        let vault = EncryptedStateVault::open_or_create(directory.path())?;
        let records = collect_legacy_records(directory.path())?;
        let manifest = vault.manifest_for_records(&records)?;
        assert!(matches!(
            vault.commit_snapshot(&records, &manifest, Some(1)),
            Err(StateError::VaultInjectedFailure(1))
        ));
        assert!(matches!(
            vault.verify(),
            Err(StateError::VaultNotMigrated(_))
        ));

        vault.migrate_legacy_snapshot()?;
        write(&directory.path().join("second"), b"changed")?;
        assert!(matches!(
            vault.verify_against_legacy(),
            Err(StateError::VaultLegacyStateChanged { .. })
        ));
        assert!(matches!(
            vault.migrate_legacy_snapshot(),
            Err(StateError::VaultLegacyStateChanged { .. })
        ));
        Ok(())
    }

    #[test]
    fn vault_rejects_tampering_wrong_key_and_unsafe_restore_target() -> Result<(), Box<dyn Error>> {
        let empty = tempfile::tempdir()?;
        assert!(matches!(
            EncryptedStateVault::open_existing(empty.path()),
            Err(StateError::VaultDatabaseMissing(_))
        ));
        assert!(!empty.path().join(STATE_VAULT_FILE).exists());
        assert!(!empty.path().join(STATE_VAULT_KEY_FILE).exists());

        let directory = tempfile::tempdir()?;
        write(&directory.path().join("state"), b"private")?;
        let vault = EncryptedStateVault::open_or_create(directory.path())?;
        vault.migrate_legacy_snapshot()?;
        assert!(matches!(
            vault.restore_to_new_directory(directory.path().join("state")),
            Err(StateError::VaultRestoreDestinationExists(_))
        ));
        drop(vault);

        let key_path = directory.path().join(STATE_VAULT_KEY_FILE);
        let original_key = fs::read(&key_path)?;
        fs::write(&key_path, [7_u8; VAULT_KEY_BYTES])?;
        let wrong_key_vault = EncryptedStateVault::open_or_create(directory.path())?;
        assert!(matches!(
            wrong_key_vault.verify(),
            Err(StateError::VaultEncryption)
        ));
        drop(wrong_key_vault);
        fs::write(key_path, original_key)?;
        Ok(())
    }
}
