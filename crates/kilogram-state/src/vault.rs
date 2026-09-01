use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
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
use tempfile::{NamedTempFile, TempDir};
use zeroize::ZeroizeOnDrop;

use crate::{
    StagedStateMutation, StateError, StateTransaction, io_at, path_exists, reject_symlink,
    sync_directory, validate_relative,
};

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
const GENERATION_KEY: &str = "snapshot-generation-v1";
const MIRROR_INTENT_KEY: &str = "mirror-intent-v1";
const PRIMARY_SHADOW_INTENT_KEY: &str = "primary-shadow-intent-v1";
const MIRROR_METADATA_VERSION: u8 = 1;
const RECORD_KEY_DOMAIN: &str = "kilogram state vault record lookup v1";
const ENCRYPTION_KEY_DOMAIN: &str = "kilogram state vault encryption v1";
const SNAPSHOT_KEY_DOMAIN: &str = "kilogram state vault snapshot v1";
const GENERATION_AUTH_KEY_DOMAIN: &str = "kilogram state vault generation auth v1";
const MIRROR_INTENT_AUTH_KEY_DOMAIN: &str = "kilogram state vault mirror intent auth v1";
const PRIMARY_SHADOW_INTENT_AUTH_KEY_DOMAIN: &str =
    "kilogram state vault primary shadow intent auth v1";
const RECORD_AAD_DOMAIN: &[u8] = b"kilogram:state-vault-record-aad:v1\0";
const TRUST_FILES: [&str; 2] = ["account-authority.snapshot", "device-certificate.cert"];
const TRUST_DIRECTORIES: [&str; 2] = ["conversation-memberships", "peer-authority"];

const META_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("vault-meta-v1");
const RECORD_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("vault-records-v1");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VaultMigrationOutcome {
    Migrated,
    AlreadyCurrent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VaultMirrorOutcome {
    Mirrored,
    AlreadyCurrent,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StateRecordKind {
    DeviceIdentity,
    Ratchet,
    Event,
    LocalProjection,
    HistoryRewrap,
    HistoryRecovery,
    Trust,
    Sequence,
    Other,
}

impl StateRecordKind {
    pub const ALL: [Self; 9] = [
        Self::DeviceIdentity,
        Self::Ratchet,
        Self::Event,
        Self::LocalProjection,
        Self::HistoryRewrap,
        Self::HistoryRecovery,
        Self::Trust,
        Self::Sequence,
        Self::Other,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::DeviceIdentity => "device-identity",
            Self::Ratchet => "ratchet",
            Self::Event => "event",
            Self::LocalProjection => "local-projection",
            Self::HistoryRewrap => "history-rewrap",
            Self::HistoryRecovery => "history-recovery",
            Self::Trust => "trust",
            Self::Sequence => "sequence",
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedShadowReadReport {
    kind: StateRecordKind,
    record_count: u64,
    plaintext_bytes: u64,
}

impl TypedShadowReadReport {
    pub fn kind(&self) -> StateRecordKind {
        self.kind
    }

    pub fn record_count(&self) -> u64 {
        self.record_count
    }

    pub fn plaintext_bytes(&self) -> u64 {
        self.plaintext_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultPrimaryRecord {
    kind: StateRecordKind,
    relative_path: String,
    content: Vec<u8>,
}

impl VaultPrimaryRecord {
    pub fn kind(&self) -> StateRecordKind {
        self.kind
    }

    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    pub fn content(&self) -> &[u8] {
        &self.content
    }

    pub fn into_parts(self) -> (StateRecordKind, String, Vec<u8>) {
        (self.kind, self.relative_path, self.content)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultPrimaryRead {
    mirror_generation: u64,
    records: Vec<VaultPrimaryRecord>,
    shadow_reports: Vec<TypedShadowReadReport>,
}

impl VaultPrimaryRead {
    pub fn mirror_generation(&self) -> u64 {
        self.mirror_generation
    }

    pub fn records(&self) -> &[VaultPrimaryRecord] {
        &self.records
    }

    pub fn shadow_reports(&self) -> &[TypedShadowReadReport] {
        &self.shadow_reports
    }

    pub fn into_records(self) -> Vec<VaultPrimaryRecord> {
        self.records
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultMutableRead {
    mirror_generation: u64,
    records: Vec<VaultPrimaryRecord>,
}

impl VaultMutableRead {
    pub fn mirror_generation(&self) -> u64 {
        self.mirror_generation
    }

    pub fn records(&self) -> &[VaultPrimaryRecord] {
        &self.records
    }

    pub fn into_records(self) -> Vec<VaultPrimaryRecord> {
        self.records
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VaultMirrorDelta {
    upserted_records: u64,
    removed_records: u64,
    unchanged_records: u64,
}

impl VaultMirrorDelta {
    pub fn upserted_records(&self) -> u64 {
        self.upserted_records
    }

    pub fn removed_records(&self) -> u64 {
        self.removed_records
    }

    pub fn unchanged_records(&self) -> u64 {
        self.unchanged_records
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultMirrorCommit {
    outcome: VaultMirrorOutcome,
    report: VaultReport,
    delta: VaultMirrorDelta,
}

impl VaultMirrorCommit {
    pub fn outcome(&self) -> VaultMirrorOutcome {
        self.outcome
    }

    pub fn report(&self) -> &VaultReport {
        &self.report
    }

    pub fn delta(&self) -> VaultMirrorDelta {
        self.delta
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultReport {
    schema_version: u64,
    mirror_generation: u64,
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

    pub fn mirror_generation(&self) -> u64 {
        self.mirror_generation
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
struct VaultGenerationRecord {
    version: u8,
    generation: u64,
    snapshot_id: [u8; 32],
    authenticator: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct VaultMirrorIntent {
    version: u8,
    base_generation: u64,
    base_snapshot_id: [u8; 32],
    authenticator: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct VaultPrimaryShadowIntent {
    version: u8,
    generation: u64,
    snapshot_id: [u8; 32],
    authenticator: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct VaultRecord {
    version: u8,
    relative_path: String,
    content: Vec<u8>,
}

struct PendingVaultDelta {
    upserts: Vec<VaultRecord>,
    removals: Vec<String>,
    unchanged_records: u64,
}

impl PendingVaultDelta {
    fn report(&self) -> VaultMirrorDelta {
        VaultMirrorDelta {
            upserted_records: self.upserts.len() as u64,
            removed_records: self.removals.len() as u64,
            unchanged_records: self.unchanged_records,
        }
    }
}

pub struct EncryptedStateVault {
    root: PathBuf,
    database: Database,
    master_key: VaultMasterKey,
}

pub trait StateMirrorRepository {
    fn begin_dual_write(&self) -> Result<VaultReport, StateError>;

    fn finish_dual_write(&self) -> Result<VaultMirrorCommit, StateError>;

    fn recover_pending_dual_write(&self) -> Result<Option<VaultMirrorCommit>, StateError>;
}

pub trait TypedStateRepository {
    fn verify_typed_shadow_reads(&self) -> Result<Vec<TypedShadowReadReport>, StateError>;

    fn read_primary_canary(
        &self,
        kinds: &[StateRecordKind],
    ) -> Result<VaultPrimaryRead, StateError>;

    fn read_mutable_primary_canary(
        &self,
        kinds: &[StateRecordKind],
    ) -> Result<VaultMutableRead, StateError>;
}

/// Commits the filesystem transaction's staged state to the encrypted vault
/// before publishing the filesystem as its retained compatibility shadow.
pub trait VaultPrimaryWriteRepository {
    fn commit_primary_checkpoint(&self) -> Result<VaultMirrorCommit, StateError>;

    fn commit_primary_transaction(
        &self,
        transaction: &StateTransaction,
    ) -> Result<VaultMirrorCommit, StateError>;

    fn confirm_primary_shadow(&self) -> Result<VaultReport, StateError>;

    fn recover_primary_shadow(&self) -> Result<Option<VaultReport>, StateError>;
}

impl EncryptedStateVault {
    pub fn is_initialized(state_directory: impl AsRef<Path>) -> Result<bool, StateError> {
        let root = state_directory.as_ref();
        let database_path = root.join(STATE_VAULT_FILE);
        let key_path = root.join(STATE_VAULT_KEY_FILE);
        let database_exists = path_exists(&database_path)?;
        let key_exists = path_exists(&key_path)?;
        match (database_exists, key_exists) {
            (false, false) => Ok(false),
            (true, false) => Err(StateError::VaultKeyMissing(key_path)),
            (false, true) => Err(StateError::VaultDatabaseMissing(database_path)),
            (true, true) => {
                reject_symlink(&database_path)?;
                reject_symlink(&key_path)?;
                Ok(true)
            }
        }
    }

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
        self.ensure_no_pending_mirror()?;
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
        self.commit_snapshot(&records, &manifest, 1, true, None)?;
        let report = self.verify()?;
        Ok((VaultMigrationOutcome::Migrated, report))
    }

    pub fn verify(&self) -> Result<VaultReport, StateError> {
        self.verify_with_records().map(|(report, _)| report)
    }

    fn verify_with_records(&self) -> Result<(VaultReport, Vec<VaultRecord>), StateError> {
        let manifest = self
            .load_manifest()?
            .ok_or_else(|| StateError::VaultNotMigrated(self.root.clone()))?;
        validate_manifest(&manifest)?;
        let records = self.load_records()?;
        let observed = self.manifest_for_records(&records)?;
        if observed != manifest {
            return Err(StateError::VaultManifestMismatch);
        }
        let generation = self.load_generation(&manifest)?;
        Ok((report_from_manifest(&manifest, generation), records))
    }

    pub fn verify_against_legacy(&self) -> Result<VaultReport, StateError> {
        self.ensure_no_pending_mirror()?;
        self.verify_current_against_legacy()
    }

    fn verify_current_against_legacy(&self) -> Result<VaultReport, StateError> {
        let (report, records) = self.verify_with_records()?;
        self.typed_shadow_reports(&records)?;
        Ok(report)
    }

    fn typed_shadow_reports(
        &self,
        vault_records: &[VaultRecord],
    ) -> Result<Vec<TypedShadowReadReport>, StateError> {
        let legacy_records = collect_legacy_records(&self.root)?;
        let vault_by_path = records_by_path(vault_records);
        let legacy_by_path = records_by_path(&legacy_records);

        for relative_path in vault_by_path.keys().chain(legacy_by_path.keys()) {
            if vault_by_path.get(relative_path) != legacy_by_path.get(relative_path) {
                return Err(StateError::VaultTypedShadowReadMismatch {
                    kind: classify_record_kind(relative_path).as_str().to_owned(),
                    relative_path: (*relative_path).to_owned(),
                });
            }
        }

        let mut totals = BTreeMap::new();
        for kind in StateRecordKind::ALL {
            totals.insert(kind, (0_u64, 0_u64));
        }
        for record in vault_records {
            let entry = totals
                .entry(classify_record_kind(&record.relative_path))
                .or_insert((0, 0));
            entry.0 += 1;
            entry.1 = entry
                .1
                .checked_add(record.content.len() as u64)
                .ok_or(StateError::VaultSnapshotTooLarge(u64::MAX))?;
        }
        Ok(totals
            .into_iter()
            .map(
                |(kind, (record_count, plaintext_bytes))| TypedShadowReadReport {
                    kind,
                    record_count,
                    plaintext_bytes,
                },
            )
            .collect())
    }

    pub fn restore_to_new_directory(
        &self,
        destination: impl AsRef<Path>,
    ) -> Result<VaultReport, StateError> {
        self.ensure_no_pending_mirror()?;
        let report = self.verify()?;
        let requested_destination = absolute_path(destination.as_ref())?;
        let destination_exists = path_exists(&requested_destination)?;
        let destination = resolve_destination_path(&requested_destination)?;
        if destination.starts_with(&self.root) {
            return Err(StateError::VaultRestoreInsideSource(destination));
        }
        if destination_exists {
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
        generation: u64,
        clear_intent: bool,
        fail_after_records: Option<usize>,
    ) -> Result<(), StateError> {
        let mut encrypted_records = Vec::with_capacity(records.len());
        for record in records {
            let record_key = self.record_key(&record.relative_path);
            let encrypted = self.encrypt_record(record, &record_key)?;
            encrypted_records.push((record_key, encrypted));
        }
        let encoded_manifest = postcard::to_allocvec(manifest)?;
        let generation_record = self.generation_record(generation, manifest.snapshot_id);
        let encoded_generation = postcard::to_allocvec(&generation_record)?;
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
            table
                .insert(GENERATION_KEY, encoded_generation.as_slice())
                .map_err(vault_database_error)?;
            if clear_intent {
                table
                    .remove(MIRROR_INTENT_KEY)
                    .map_err(vault_database_error)?;
            }
        }
        write.commit().map_err(vault_database_error)
    }

    fn commit_delta(
        &self,
        delta: &PendingVaultDelta,
        manifest: &VaultManifest,
        generation: u64,
        fail_after_operations: Option<usize>,
    ) -> Result<(), StateError> {
        let mut encrypted_upserts = Vec::with_capacity(delta.upserts.len());
        for record in &delta.upserts {
            let record_key = self.record_key(&record.relative_path);
            let encrypted = self.encrypt_record(record, &record_key)?;
            encrypted_upserts.push((record_key, encrypted));
        }
        let removal_keys = delta
            .removals
            .iter()
            .map(|relative_path| self.record_key(relative_path))
            .collect::<Vec<_>>();
        let encoded_manifest = postcard::to_allocvec(manifest)?;
        let generation_record = self.generation_record(generation, manifest.snapshot_id);
        let encoded_generation = postcard::to_allocvec(&generation_record)?;
        let mut write = self.database.begin_write().map_err(vault_database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(vault_database_error)?;
        let mut completed_operations = 0_usize;
        {
            let mut table = write
                .open_table(RECORD_TABLE)
                .map_err(vault_database_error)?;
            for key in &removal_keys {
                table.remove(key.as_slice()).map_err(vault_database_error)?;
                completed_operations += 1;
                if fail_after_operations == Some(completed_operations) {
                    return Err(StateError::VaultInjectedFailure(completed_operations));
                }
            }
            for (key, value) in &encrypted_upserts {
                table
                    .insert(key.as_slice(), value.as_slice())
                    .map_err(vault_database_error)?;
                completed_operations += 1;
                if fail_after_operations == Some(completed_operations) {
                    return Err(StateError::VaultInjectedFailure(completed_operations));
                }
            }
        }
        {
            let mut table = write.open_table(META_TABLE).map_err(vault_database_error)?;
            table
                .insert(MANIFEST_KEY, encoded_manifest.as_slice())
                .map_err(vault_database_error)?;
            table
                .insert(GENERATION_KEY, encoded_generation.as_slice())
                .map_err(vault_database_error)?;
            table
                .remove(MIRROR_INTENT_KEY)
                .map_err(vault_database_error)?;
        }
        write.commit().map_err(vault_database_error)
    }

    fn commit_primary_delta(
        &self,
        delta: &PendingVaultDelta,
        manifest: &VaultManifest,
        generation: u64,
        mirror_intent: &VaultMirrorIntent,
        primary_shadow_intent: &VaultPrimaryShadowIntent,
        fail_after_operations: Option<usize>,
    ) -> Result<(), StateError> {
        let mut encrypted_upserts = Vec::with_capacity(delta.upserts.len());
        for record in &delta.upserts {
            let record_key = self.record_key(&record.relative_path);
            let encrypted = self.encrypt_record(record, &record_key)?;
            encrypted_upserts.push((record_key, encrypted));
        }
        let removal_keys = delta
            .removals
            .iter()
            .map(|relative_path| self.record_key(relative_path))
            .collect::<Vec<_>>();
        let encoded_manifest = postcard::to_allocvec(manifest)?;
        let generation_record = self.generation_record(generation, manifest.snapshot_id);
        let encoded_generation = postcard::to_allocvec(&generation_record)?;
        let encoded_mirror_intent = postcard::to_allocvec(mirror_intent)?;
        let encoded_primary_shadow_intent = postcard::to_allocvec(primary_shadow_intent)?;
        let mut write = self.database.begin_write().map_err(vault_database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(vault_database_error)?;
        let mut completed_operations = 0_usize;
        {
            let mut table = write
                .open_table(RECORD_TABLE)
                .map_err(vault_database_error)?;
            for key in &removal_keys {
                table.remove(key.as_slice()).map_err(vault_database_error)?;
                completed_operations += 1;
                if fail_after_operations == Some(completed_operations) {
                    return Err(StateError::VaultInjectedFailure(completed_operations));
                }
            }
            for (key, value) in &encrypted_upserts {
                table
                    .insert(key.as_slice(), value.as_slice())
                    .map_err(vault_database_error)?;
                completed_operations += 1;
                if fail_after_operations == Some(completed_operations) {
                    return Err(StateError::VaultInjectedFailure(completed_operations));
                }
            }
        }
        {
            let mut table = write.open_table(META_TABLE).map_err(vault_database_error)?;
            table
                .insert(MANIFEST_KEY, encoded_manifest.as_slice())
                .map_err(vault_database_error)?;
            table
                .insert(GENERATION_KEY, encoded_generation.as_slice())
                .map_err(vault_database_error)?;
            table
                .insert(MIRROR_INTENT_KEY, encoded_mirror_intent.as_slice())
                .map_err(vault_database_error)?;
            table
                .insert(
                    PRIMARY_SHADOW_INTENT_KEY,
                    encoded_primary_shadow_intent.as_slice(),
                )
                .map_err(vault_database_error)?;
        }
        write.commit().map_err(vault_database_error)
    }

    fn load_manifest(&self) -> Result<Option<VaultManifest>, StateError> {
        self.load_metadata(MANIFEST_KEY)?
            .map(|bytes| postcard::from_bytes(&bytes).map_err(StateError::from))
            .transpose()
    }

    fn load_generation(&self, manifest: &VaultManifest) -> Result<u64, StateError> {
        let Some(encoded) = self.load_metadata(GENERATION_KEY)? else {
            return Ok(1);
        };
        let generation: VaultGenerationRecord = postcard::from_bytes(&encoded)?;
        if generation.version != MIRROR_METADATA_VERSION {
            return Err(StateError::UnsupportedVaultMirrorMetadataVersion(
                generation.version,
            ));
        }
        if generation.generation == 0 {
            return Err(StateError::InvalidVaultGeneration(0));
        }
        if generation.snapshot_id != manifest.snapshot_id
            || generation.authenticator
                != self.generation_authenticator(generation.generation, generation.snapshot_id)
        {
            return Err(StateError::VaultGenerationAuthenticationFailed);
        }
        Ok(generation.generation)
    }

    fn load_mirror_intent(&self) -> Result<Option<VaultMirrorIntent>, StateError> {
        let Some(encoded) = self.load_metadata(MIRROR_INTENT_KEY)? else {
            return Ok(None);
        };
        let intent: VaultMirrorIntent = postcard::from_bytes(&encoded)?;
        if intent.version != MIRROR_METADATA_VERSION {
            return Err(StateError::UnsupportedVaultMirrorMetadataVersion(
                intent.version,
            ));
        }
        if intent.base_generation == 0
            || intent.authenticator
                != self.mirror_intent_authenticator(intent.base_generation, intent.base_snapshot_id)
        {
            return Err(StateError::VaultMirrorIntentAuthenticationFailed);
        }
        Ok(Some(intent))
    }

    fn load_primary_shadow_intent(&self) -> Result<Option<VaultPrimaryShadowIntent>, StateError> {
        let Some(encoded) = self.load_metadata(PRIMARY_SHADOW_INTENT_KEY)? else {
            return Ok(None);
        };
        let intent: VaultPrimaryShadowIntent = postcard::from_bytes(&encoded)?;
        if intent.version != MIRROR_METADATA_VERSION {
            return Err(StateError::UnsupportedVaultMirrorMetadataVersion(
                intent.version,
            ));
        }
        if intent.generation == 0
            || intent.authenticator
                != self.primary_shadow_intent_authenticator(intent.generation, intent.snapshot_id)
        {
            return Err(StateError::VaultPrimaryShadowIntentAuthenticationFailed);
        }
        Ok(Some(intent))
    }

    fn load_metadata(&self, key: &str) -> Result<Option<Vec<u8>>, StateError> {
        let read = self.database.begin_read().map_err(vault_database_error)?;
        let table = match read.open_table(META_TABLE) {
            Ok(table) => table,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(error) => return Err(vault_database_error(error)),
        };
        table
            .get(key)
            .map_err(vault_database_error)
            .map(|value| value.map(|value| value.value().to_vec()))
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

    fn ensure_no_pending_mirror(&self) -> Result<(), StateError> {
        if let Some(intent) = self.load_primary_shadow_intent()? {
            return Err(StateError::VaultPrimaryShadowRecoveryRequired {
                generation: intent.generation,
            });
        }
        if let Some(intent) = self.load_mirror_intent()? {
            return Err(StateError::VaultMirrorRecoveryRequired {
                base_generation: intent.base_generation,
            });
        }
        Ok(())
    }

    fn write_mirror_intent(&self, report: &VaultReport) -> Result<(), StateError> {
        let intent = self.mirror_intent_record(report.mirror_generation, report.snapshot_id);
        let encoded = postcard::to_allocvec(&intent)?;
        let mut write = self.database.begin_write().map_err(vault_database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(vault_database_error)?;
        {
            let mut table = write.open_table(META_TABLE).map_err(vault_database_error)?;
            table
                .insert(MIRROR_INTENT_KEY, encoded.as_slice())
                .map_err(vault_database_error)?;
        }
        write.commit().map_err(vault_database_error)
    }

    fn commit_primary_checkpoint_internal(
        &self,
        fail_after_operations: Option<usize>,
    ) -> Result<VaultMirrorCommit, StateError> {
        if let Some(intent) = self.load_primary_shadow_intent()? {
            return Err(StateError::VaultPrimaryShadowRecoveryRequired {
                generation: intent.generation,
            });
        }
        let intent = self
            .load_mirror_intent()?
            .ok_or(StateError::VaultMirrorIntentMissing)?;
        let active = self.verify()?;
        if intent.base_generation != active.mirror_generation
            || intent.base_snapshot_id != active.snapshot_id
        {
            return Err(StateError::VaultMirrorIntentBaseMismatch);
        }

        let active_records = self.load_records()?;
        let staged_records = collect_legacy_records(&self.root)?;
        let manifest = self.manifest_for_records(&staged_records)?;
        let delta = diff_records(&active_records, &staged_records);
        if delta.upserts.is_empty() && delta.removals.is_empty() {
            if !report_matches_manifest(&active, &manifest) {
                return Err(StateError::VaultManifestMismatch);
            }
            return Ok(VaultMirrorCommit {
                outcome: VaultMirrorOutcome::AlreadyCurrent,
                report: active,
                delta: delta.report(),
            });
        }

        let next_generation = active
            .mirror_generation
            .checked_add(1)
            .ok_or(StateError::VaultGenerationExhausted)?;
        let mirror_intent = self.mirror_intent_record(next_generation, manifest.snapshot_id);
        let primary_shadow_intent =
            self.primary_shadow_intent_record(next_generation, manifest.snapshot_id);
        self.commit_primary_delta(
            &delta,
            &manifest,
            next_generation,
            &mirror_intent,
            &primary_shadow_intent,
            fail_after_operations,
        )?;
        let report = self.verify_current_against_legacy()?;
        Ok(VaultMirrorCommit {
            outcome: VaultMirrorOutcome::Mirrored,
            report,
            delta: delta.report(),
        })
    }

    fn commit_primary_transaction_internal(
        &self,
        transaction: &StateTransaction,
        fail_after_operations: Option<usize>,
    ) -> Result<VaultMirrorCommit, StateError> {
        if transaction.root() != self.root {
            return Err(StateError::VaultTransactionRootMismatch {
                transaction_root: transaction.root().to_path_buf(),
                vault_root: self.root.clone(),
            });
        }
        if let Some(intent) = self.load_primary_shadow_intent()? {
            return Err(StateError::VaultPrimaryShadowRecoveryRequired {
                generation: intent.generation,
            });
        }
        let intent = self
            .load_mirror_intent()?
            .ok_or(StateError::VaultMirrorIntentMissing)?;
        let active = self.verify()?;
        if intent.base_generation != active.mirror_generation
            || intent.base_snapshot_id != active.snapshot_id
        {
            return Err(StateError::VaultMirrorIntentBaseMismatch);
        }

        let active_records = self.load_records()?;
        let mut staged_by_path = active_records
            .iter()
            .cloned()
            .map(|record| (record.relative_path.clone(), record))
            .collect::<BTreeMap<_, _>>();
        let current_trust_records = collect_legacy_trust_records(&self.root)?;
        let current_trust_paths = current_trust_records
            .iter()
            .map(|record| record.relative_path.as_str())
            .collect::<BTreeSet<_>>();
        if let Some(removed) = active_records.iter().find(|record| {
            classify_record_kind(&record.relative_path) == StateRecordKind::Trust
                && !current_trust_paths.contains(record.relative_path.as_str())
        }) {
            return Err(StateError::VaultTrustRecordRemoved(
                removed.relative_path.clone(),
            ));
        }
        for record in current_trust_records {
            staged_by_path.insert(record.relative_path.clone(), record);
        }
        let mutations = transaction.staged_mutations()?;
        let mut seen = BTreeSet::new();
        for StagedStateMutation {
            kind,
            relative_path,
            content,
        } in mutations
        {
            if !matches!(
                kind,
                StateRecordKind::Ratchet
                    | StateRecordKind::Event
                    | StateRecordKind::LocalProjection
                    | StateRecordKind::HistoryRewrap
                    | StateRecordKind::HistoryRecovery
                    | StateRecordKind::Sequence
            ) {
                return Err(StateError::VaultDirectWriteKindNotAllowed(
                    kind.as_str().to_owned(),
                ));
            }
            let relative_path = path_to_vault(&relative_path)?;
            if !seen.insert(relative_path.clone()) {
                return Err(StateError::VaultDirectWriteDuplicatePath(PathBuf::from(
                    relative_path,
                )));
            }
            let actual_kind = classify_record_kind(&relative_path);
            if actual_kind != kind {
                return Err(StateError::VaultDirectWriteKindMismatch {
                    relative_path,
                    declared_kind: kind.as_str().to_owned(),
                    actual_kind: actual_kind.as_str().to_owned(),
                });
            }
            match content {
                Some(content) => {
                    let record = VaultRecord {
                        version: VAULT_RECORD_VERSION,
                        relative_path: relative_path.clone(),
                        content,
                    };
                    validate_record(&record)?;
                    staged_by_path.insert(relative_path, record);
                }
                None => {
                    staged_by_path.remove(&relative_path);
                }
            }
        }

        let staged_records = staged_by_path.into_values().collect::<Vec<_>>();
        let manifest = self.manifest_for_records(&staged_records)?;
        let delta = diff_records(&active_records, &staged_records);
        if delta.upserts.is_empty() && delta.removals.is_empty() {
            if !report_matches_manifest(&active, &manifest) {
                return Err(StateError::VaultManifestMismatch);
            }
            return Ok(VaultMirrorCommit {
                outcome: VaultMirrorOutcome::AlreadyCurrent,
                report: active,
                delta: delta.report(),
            });
        }

        let next_generation = active
            .mirror_generation
            .checked_add(1)
            .ok_or(StateError::VaultGenerationExhausted)?;
        let mirror_intent = self.mirror_intent_record(next_generation, manifest.snapshot_id);
        let primary_shadow_intent =
            self.primary_shadow_intent_record(next_generation, manifest.snapshot_id);
        self.commit_primary_delta(
            &delta,
            &manifest,
            next_generation,
            &mirror_intent,
            &primary_shadow_intent,
            fail_after_operations,
        )?;
        let report = self.verify()?;
        Ok(VaultMirrorCommit {
            outcome: VaultMirrorOutcome::Mirrored,
            report,
            delta: delta.report(),
        })
    }

    fn confirm_primary_shadow_internal(&self) -> Result<VaultReport, StateError> {
        let active = self.verify_current_against_legacy()?;
        let mirror_intent = self
            .load_mirror_intent()?
            .ok_or(StateError::VaultMirrorIntentMissing)?;
        if mirror_intent.base_generation != active.mirror_generation
            || mirror_intent.base_snapshot_id != active.snapshot_id
        {
            return Err(StateError::VaultMirrorIntentBaseMismatch);
        }
        let Some(primary_intent) = self.load_primary_shadow_intent()? else {
            return Ok(active);
        };
        if primary_intent.generation != active.mirror_generation
            || primary_intent.snapshot_id != active.snapshot_id
        {
            return Err(StateError::VaultMirrorIntentBaseMismatch);
        }
        self.clear_primary_shadow_intent()?;
        Ok(active)
    }

    fn recover_primary_shadow_internal(&self) -> Result<Option<VaultReport>, StateError> {
        let Some(primary_intent) = self.load_primary_shadow_intent()? else {
            return Ok(None);
        };
        let active_transaction = self.root.join(".kilogram-transactions").join("active");
        if path_exists(&active_transaction)? {
            return Err(StateError::VaultPrimaryShadowBlockedByLocalTransaction {
                path: active_transaction,
            });
        }
        let (active, records) = self.verify_with_records()?;
        if primary_intent.generation != active.mirror_generation
            || primary_intent.snapshot_id != active.snapshot_id
        {
            return Err(StateError::VaultMirrorIntentBaseMismatch);
        }
        let mirror_intent = self
            .load_mirror_intent()?
            .ok_or(StateError::VaultMirrorIntentMissing)?;
        if mirror_intent.base_generation != active.mirror_generation
            || mirror_intent.base_snapshot_id != active.snapshot_id
        {
            return Err(StateError::VaultMirrorIntentBaseMismatch);
        }
        self.restore_legacy_shadow(&records)?;
        let confirmed = self.verify_current_against_legacy()?;
        self.clear_primary_shadow_intent()?;
        Ok(Some(confirmed))
    }

    fn restore_legacy_shadow(&self, records: &[VaultRecord]) -> Result<(), StateError> {
        let current = collect_legacy_records(&self.root)?;
        let expected_by_path = records_by_path(records);

        for record in records {
            if current
                .iter()
                .find(|existing| existing.relative_path == record.relative_path)
                == Some(record)
            {
                continue;
            }
            self.restore_legacy_record(record)?;
        }
        for existing in &current {
            if expected_by_path.contains_key(existing.relative_path.as_str()) {
                continue;
            }
            let relative = path_from_vault(&existing.relative_path)?;
            let path = self.root.join(relative);
            reject_symlink(&path)?;
            io_at(&path, fs::remove_file(&path))?;
            if let Some(parent) = path.parent() {
                sync_directory(parent)?;
            }
        }
        Ok(())
    }

    fn restore_legacy_record(&self, record: &VaultRecord) -> Result<(), StateError> {
        validate_record(record)?;
        let relative = path_from_vault(&record.relative_path)?;
        let destination = self.root.join(relative);
        let parent = destination
            .parent()
            .ok_or_else(|| StateError::UnsafeRelativePath(destination.clone()))?;
        io_at(parent, fs::create_dir_all(parent))?;
        reject_symlink(parent)?;
        if path_exists(&destination)? {
            reject_symlink(&destination)?;
            io_at(&destination, fs::remove_file(&destination))?;
        }
        let mut temporary = io_at(parent, NamedTempFile::new_in(parent))?;
        let temporary_path = temporary.path().to_path_buf();
        io_at(&temporary_path, temporary.write_all(&record.content))?;
        io_at(&temporary_path, temporary.as_file().sync_all())?;
        io_at(
            &destination,
            temporary
                .persist_noclobber(&destination)
                .map_err(|error| error.error),
        )?;
        sync_directory(parent)?;
        Ok(())
    }

    fn clear_primary_shadow_intent(&self) -> Result<(), StateError> {
        let mut write = self.database.begin_write().map_err(vault_database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(vault_database_error)?;
        {
            let mut table = write.open_table(META_TABLE).map_err(vault_database_error)?;
            table
                .remove(PRIMARY_SHADOW_INTENT_KEY)
                .map_err(vault_database_error)?;
        }
        write.commit().map_err(vault_database_error)
    }

    fn finish_dual_write_internal(
        &self,
        fail_after_operations: Option<usize>,
    ) -> Result<VaultMirrorCommit, StateError> {
        self.recover_primary_shadow_internal()?;
        let intent = self
            .load_mirror_intent()?
            .ok_or(StateError::VaultMirrorIntentMissing)?;
        let active = self.verify()?;
        if intent.base_generation != active.mirror_generation
            || intent.base_snapshot_id != active.snapshot_id
        {
            return Err(StateError::VaultMirrorIntentBaseMismatch);
        }

        let active_records = self.load_records()?;
        let records = collect_legacy_records(&self.root)?;
        let manifest = self.manifest_for_records(&records)?;
        let delta = diff_records(&active_records, &records);
        if delta.upserts.is_empty() && delta.removals.is_empty() {
            if !report_matches_manifest(&active, &manifest) {
                return Err(StateError::VaultManifestMismatch);
            }
            self.clear_mirror_intent(&active)?;
            return Ok(VaultMirrorCommit {
                outcome: VaultMirrorOutcome::AlreadyCurrent,
                report: active,
                delta: delta.report(),
            });
        }

        let next_generation = active
            .mirror_generation
            .checked_add(1)
            .ok_or(StateError::VaultGenerationExhausted)?;
        self.commit_delta(&delta, &manifest, next_generation, fail_after_operations)?;
        let report = self.verify_current_against_legacy()?;
        Ok(VaultMirrorCommit {
            outcome: VaultMirrorOutcome::Mirrored,
            report,
            delta: delta.report(),
        })
    }

    fn clear_mirror_intent(&self, report: &VaultReport) -> Result<(), StateError> {
        let generation = self.generation_record(report.mirror_generation, report.snapshot_id);
        let encoded_generation = postcard::to_allocvec(&generation)?;
        let mut write = self.database.begin_write().map_err(vault_database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(vault_database_error)?;
        {
            let mut table = write.open_table(META_TABLE).map_err(vault_database_error)?;
            table
                .insert(GENERATION_KEY, encoded_generation.as_slice())
                .map_err(vault_database_error)?;
            table
                .remove(MIRROR_INTENT_KEY)
                .map_err(vault_database_error)?;
        }
        write.commit().map_err(vault_database_error)
    }

    fn generation_record(&self, generation: u64, snapshot_id: [u8; 32]) -> VaultGenerationRecord {
        VaultGenerationRecord {
            version: MIRROR_METADATA_VERSION,
            generation,
            snapshot_id,
            authenticator: self.generation_authenticator(generation, snapshot_id),
        }
    }

    fn mirror_intent_record(&self, generation: u64, snapshot_id: [u8; 32]) -> VaultMirrorIntent {
        VaultMirrorIntent {
            version: MIRROR_METADATA_VERSION,
            base_generation: generation,
            base_snapshot_id: snapshot_id,
            authenticator: self.mirror_intent_authenticator(generation, snapshot_id),
        }
    }

    fn primary_shadow_intent_record(
        &self,
        generation: u64,
        snapshot_id: [u8; 32],
    ) -> VaultPrimaryShadowIntent {
        VaultPrimaryShadowIntent {
            version: MIRROR_METADATA_VERSION,
            generation,
            snapshot_id,
            authenticator: self.primary_shadow_intent_authenticator(generation, snapshot_id),
        }
    }

    fn generation_authenticator(&self, generation: u64, snapshot_id: [u8; 32]) -> [u8; 32] {
        keyed_metadata_authenticator(
            GENERATION_AUTH_KEY_DOMAIN,
            &self.master_key.0,
            generation,
            &snapshot_id,
        )
    }

    fn mirror_intent_authenticator(&self, generation: u64, snapshot_id: [u8; 32]) -> [u8; 32] {
        keyed_metadata_authenticator(
            MIRROR_INTENT_AUTH_KEY_DOMAIN,
            &self.master_key.0,
            generation,
            &snapshot_id,
        )
    }

    fn primary_shadow_intent_authenticator(
        &self,
        generation: u64,
        snapshot_id: [u8; 32],
    ) -> [u8; 32] {
        keyed_metadata_authenticator(
            PRIMARY_SHADOW_INTENT_AUTH_KEY_DOMAIN,
            &self.master_key.0,
            generation,
            &snapshot_id,
        )
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

impl StateMirrorRepository for EncryptedStateVault {
    fn begin_dual_write(&self) -> Result<VaultReport, StateError> {
        self.ensure_no_pending_mirror()?;
        let report = self.verify_current_against_legacy()?;
        self.write_mirror_intent(&report)?;
        Ok(report)
    }

    fn finish_dual_write(&self) -> Result<VaultMirrorCommit, StateError> {
        self.finish_dual_write_internal(None)
    }

    fn recover_pending_dual_write(&self) -> Result<Option<VaultMirrorCommit>, StateError> {
        if self.load_mirror_intent()?.is_none() && self.load_primary_shadow_intent()?.is_none() {
            return Ok(None);
        }
        self.finish_dual_write_internal(None).map(Some)
    }
}

impl VaultPrimaryWriteRepository for EncryptedStateVault {
    fn commit_primary_checkpoint(&self) -> Result<VaultMirrorCommit, StateError> {
        self.commit_primary_checkpoint_internal(None)
    }

    fn commit_primary_transaction(
        &self,
        transaction: &StateTransaction,
    ) -> Result<VaultMirrorCommit, StateError> {
        self.commit_primary_transaction_internal(transaction, None)
    }

    fn confirm_primary_shadow(&self) -> Result<VaultReport, StateError> {
        self.confirm_primary_shadow_internal()
    }

    fn recover_primary_shadow(&self) -> Result<Option<VaultReport>, StateError> {
        self.recover_primary_shadow_internal()
    }
}

impl TypedStateRepository for EncryptedStateVault {
    fn verify_typed_shadow_reads(&self) -> Result<Vec<TypedShadowReadReport>, StateError> {
        self.ensure_no_pending_mirror()?;
        let (_, records) = self.verify_with_records()?;
        self.typed_shadow_reports(&records)
    }

    fn read_primary_canary(
        &self,
        kinds: &[StateRecordKind],
    ) -> Result<VaultPrimaryRead, StateError> {
        if kinds.is_empty() {
            return Err(StateError::VaultPrimaryReadSelectionEmpty);
        }
        let mut selected = BTreeSet::new();
        for kind in kinds {
            if !matches!(
                kind,
                StateRecordKind::Event | StateRecordKind::LocalProjection
            ) {
                return Err(StateError::VaultPrimaryReadKindNotAllowed(
                    kind.as_str().to_owned(),
                ));
            }
            selected.insert(*kind);
        }

        if let Some(intent) = self.load_primary_shadow_intent()? {
            return Err(StateError::VaultPrimaryShadowRecoveryRequired {
                generation: intent.generation,
            });
        }

        let (report, records) = self.verify_with_records()?;
        if let Some(intent) = self.load_mirror_intent()?
            && (intent.base_generation != report.mirror_generation
                || intent.base_snapshot_id != report.snapshot_id)
        {
            return Err(StateError::VaultMirrorIntentBaseMismatch);
        }
        let shadow_reports = self
            .typed_shadow_reports(&records)?
            .into_iter()
            .filter(|typed| selected.contains(&typed.kind))
            .collect();
        let records = records
            .into_iter()
            .filter_map(|record| {
                let kind = classify_record_kind(&record.relative_path);
                selected.contains(&kind).then_some(VaultPrimaryRecord {
                    kind,
                    relative_path: record.relative_path,
                    content: record.content,
                })
            })
            .collect();
        Ok(VaultPrimaryRead {
            mirror_generation: report.mirror_generation,
            records,
            shadow_reports,
        })
    }

    fn read_mutable_primary_canary(
        &self,
        kinds: &[StateRecordKind],
    ) -> Result<VaultMutableRead, StateError> {
        if kinds.is_empty() {
            return Err(StateError::VaultPrimaryReadSelectionEmpty);
        }
        let mut selected = BTreeSet::new();
        for kind in kinds {
            if *kind != StateRecordKind::Sequence {
                return Err(StateError::VaultPrimaryReadKindNotAllowed(
                    kind.as_str().to_owned(),
                ));
            }
            selected.insert(*kind);
        }
        if let Some(intent) = self.load_primary_shadow_intent()? {
            return Err(StateError::VaultPrimaryShadowRecoveryRequired {
                generation: intent.generation,
            });
        }

        let (report, records) = self.verify_with_records()?;
        let intent = self
            .load_mirror_intent()?
            .ok_or(StateError::VaultMirrorIntentMissing)?;
        if intent.base_generation != report.mirror_generation
            || intent.base_snapshot_id != report.snapshot_id
        {
            return Err(StateError::VaultMirrorIntentBaseMismatch);
        }
        let records = records
            .into_iter()
            .filter_map(|record| {
                let kind = classify_record_kind(&record.relative_path);
                selected.contains(&kind).then_some(VaultPrimaryRecord {
                    kind,
                    relative_path: record.relative_path,
                    content: record.content,
                })
            })
            .collect();
        Ok(VaultMutableRead {
            mirror_generation: report.mirror_generation,
            records,
        })
    }
}

fn records_by_path(records: &[VaultRecord]) -> BTreeMap<&str, &VaultRecord> {
    records
        .iter()
        .map(|record| (record.relative_path.as_str(), record))
        .collect()
}

fn diff_records(active: &[VaultRecord], current: &[VaultRecord]) -> PendingVaultDelta {
    let active_by_path = records_by_path(active);
    let current_by_path = records_by_path(current);
    let upserts = current
        .iter()
        .filter(|record| active_by_path.get(record.relative_path.as_str()) != Some(record))
        .cloned()
        .collect();
    let removals = active
        .iter()
        .filter(|record| !current_by_path.contains_key(record.relative_path.as_str()))
        .map(|record| record.relative_path.clone())
        .collect();
    let unchanged_records = current
        .iter()
        .filter(|record| active_by_path.get(record.relative_path.as_str()) == Some(record))
        .count() as u64;
    PendingVaultDelta {
        upserts,
        removals,
        unchanged_records,
    }
}

fn classify_record_kind(relative_path: &str) -> StateRecordKind {
    let first = relative_path.split('/').next().unwrap_or(relative_path);
    match first {
        "device-secret.key" | "device-encryption-secret.key" => StateRecordKind::DeviceIdentity,
        "ratchet" => StateRecordKind::Ratchet,
        "events" => StateRecordKind::Event,
        "local-messages" => StateRecordKind::LocalProjection,
        "history-rewraps" => StateRecordKind::HistoryRewrap,
        "history-recovery" => StateRecordKind::HistoryRecovery,
        "account-authority.snapshot"
        | "device-certificate.cert"
        | "conversation-memberships"
        | "peer-authority" => StateRecordKind::Trust,
        "next-sequence" => StateRecordKind::Sequence,
        _ => StateRecordKind::Other,
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

fn collect_legacy_trust_records(root: &Path) -> Result<Vec<VaultRecord>, StateError> {
    let mut records = Vec::new();
    for name in TRUST_FILES {
        let path = root.join(name);
        if !path_exists(&path)? {
            continue;
        }
        reject_symlink(&path)?;
        let metadata = io_at(&path, fs::metadata(&path))?;
        if !metadata.is_file() {
            return Err(StateError::UnsafeRelativePath(path));
        }
        let content = io_at(&path, fs::read(&path))?;
        let record = VaultRecord {
            version: VAULT_RECORD_VERSION,
            relative_path: name.to_owned(),
            content,
        };
        validate_record(&record)?;
        records.push(record);
    }
    for name in TRUST_DIRECTORIES {
        let directory = root.join(name);
        if path_exists(&directory)? {
            collect_legacy_records_from(root, &directory, &mut records)?;
        }
    }
    records.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
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

fn report_from_manifest(manifest: &VaultManifest, mirror_generation: u64) -> VaultReport {
    VaultReport {
        schema_version: manifest.schema_version,
        mirror_generation,
        record_count: manifest.record_count,
        plaintext_bytes: manifest.plaintext_bytes,
        snapshot_id: manifest.snapshot_id,
    }
}

fn report_matches_manifest(report: &VaultReport, manifest: &VaultManifest) -> bool {
    report.schema_version == manifest.schema_version
        && report.record_count == manifest.record_count
        && report.plaintext_bytes == manifest.plaintext_bytes
        && report.snapshot_id == manifest.snapshot_id
}

fn keyed_metadata_authenticator(
    domain: &str,
    master_key: &[u8; VAULT_KEY_BYTES],
    generation: u64,
    snapshot_id: &[u8; 32],
) -> [u8; 32] {
    let authentication_key = blake3::derive_key(domain, master_key);
    let mut hasher = blake3::Hasher::new_keyed(&authentication_key);
    hasher.update(&[MIRROR_METADATA_VERSION]);
    hasher.update(&generation.to_be_bytes());
    hasher.update(snapshot_id);
    *hasher.finalize().as_bytes()
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

fn resolve_destination_path(path: &Path) -> Result<PathBuf, StateError> {
    let mut cursor = path;
    let mut missing_components = Vec::<OsString>::new();
    while !path_exists(cursor)? {
        let file_name = cursor
            .file_name()
            .ok_or_else(|| StateError::UnsafeRelativePath(path.to_path_buf()))?;
        missing_components.push(file_name.to_os_string());
        cursor = cursor
            .parent()
            .ok_or_else(|| StateError::UnsafeRelativePath(path.to_path_buf()))?;
    }
    let mut resolved = io_at(cursor, fs::canonicalize(cursor))?;
    for component in missing_components.iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
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

    fn encrypted_record(
        vault: &EncryptedStateVault,
        relative_path: &str,
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        let read = vault.database.begin_read()?;
        let table = read.open_table(RECORD_TABLE)?;
        let key = vault.record_key(relative_path);
        Ok(table
            .get(key.as_slice())?
            .ok_or("encrypted record is missing")?
            .value()
            .to_vec())
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
        let restore_parent = tempfile::tempdir()?;
        let restored = restore_parent.path().join("restored");
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
            vault.commit_snapshot(&records, &manifest, 1, true, Some(1)),
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
            Err(StateError::VaultTypedShadowReadMismatch { .. })
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
            Err(StateError::VaultRestoreInsideSource(_))
        ));
        let nested_inside = directory.path().join("new/restore");
        assert!(matches!(
            vault.restore_to_new_directory(&nested_inside),
            Err(StateError::VaultRestoreInsideSource(_))
        ));
        assert!(!directory.path().join("new").exists());
        let existing_destination = tempfile::tempdir()?;
        assert!(matches!(
            vault.restore_to_new_directory(existing_destination.path()),
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

    #[test]
    fn versioned_dual_write_recovers_only_with_an_authenticated_intent()
    -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        write(&directory.path().join("state"), b"one")?;
        let vault = EncryptedStateVault::open_or_create(directory.path())?;
        let initial = vault.migrate_legacy_snapshot()?.1;
        assert_eq!(initial.mirror_generation(), 1);

        let begun = vault.begin_dual_write()?;
        assert_eq!(begun, initial);
        let unchanged = vault.finish_dual_write()?;
        assert_eq!(unchanged.outcome(), VaultMirrorOutcome::AlreadyCurrent);
        assert_eq!(unchanged.report().mirror_generation(), 1);
        assert_eq!(unchanged.delta().upserted_records(), 0);
        assert_eq!(unchanged.delta().removed_records(), 0);
        assert_eq!(unchanged.delta().unchanged_records(), 1);

        vault.begin_dual_write()?;
        write(&directory.path().join("state"), b"two")?;
        assert!(matches!(
            vault.verify_against_legacy(),
            Err(StateError::VaultMirrorRecoveryRequired { .. })
        ));
        let recovered = vault
            .recover_pending_dual_write()?
            .ok_or("expected pending mirror recovery")?;
        assert_eq!(recovered.outcome(), VaultMirrorOutcome::Mirrored);
        assert_eq!(recovered.report().mirror_generation(), 2);
        assert_eq!(recovered.delta().upserted_records(), 1);
        assert_eq!(recovered.delta().removed_records(), 0);
        assert_eq!(recovered.delta().unchanged_records(), 0);
        assert_eq!(vault.verify_against_legacy()?, *recovered.report());

        vault.begin_dual_write()?;
        write(&directory.path().join("another"), b"three")?;
        assert!(matches!(
            vault.finish_dual_write_internal(Some(1)),
            Err(StateError::VaultInjectedFailure(1))
        ));
        assert_eq!(vault.verify()?.mirror_generation(), 2);
        assert!(matches!(
            vault.verify_against_legacy(),
            Err(StateError::VaultMirrorRecoveryRequired { .. })
        ));
        let third = vault
            .recover_pending_dual_write()?
            .ok_or("expected fault recovery")?;
        assert_eq!(third.report().mirror_generation(), 3);

        write(&directory.path().join("external"), b"not authorized")?;
        assert!(matches!(
            vault.begin_dual_write(),
            Err(StateError::VaultTypedShadowReadMismatch { .. })
        ));

        let tampered_directory = tempfile::tempdir()?;
        write(&tampered_directory.path().join("state"), b"private")?;
        let tampered_vault = EncryptedStateVault::open_or_create(tampered_directory.path())?;
        tampered_vault.migrate_legacy_snapshot()?;
        tampered_vault.begin_dual_write()?;
        let forged = VaultMirrorIntent {
            version: MIRROR_METADATA_VERSION,
            base_generation: 1,
            base_snapshot_id: *tampered_vault.verify()?.snapshot_id(),
            authenticator: [0_u8; 32],
        };
        let encoded = postcard::to_allocvec(&forged)?;
        let write = tampered_vault
            .database
            .begin_write()
            .map_err(vault_database_error)?;
        {
            let mut table = write.open_table(META_TABLE).map_err(vault_database_error)?;
            table
                .insert(MIRROR_INTENT_KEY, encoded.as_slice())
                .map_err(vault_database_error)?;
        }
        write.commit().map_err(vault_database_error)?;
        assert!(matches!(
            tampered_vault.recover_pending_dual_write(),
            Err(StateError::VaultMirrorIntentAuthenticationFailed)
        ));
        Ok(())
    }

    #[test]
    fn primary_checkpoint_is_atomic_and_restores_its_legacy_shadow_after_a_crash()
    -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        let ratchet_path = directory.path().join("ratchet/session.pickle");
        let event_path = directory.path().join("events/chat/new.event");
        write(&ratchet_path, b"ratchet-before")?;
        let vault = EncryptedStateVault::open_or_create(directory.path())?;
        assert_eq!(vault.migrate_legacy_snapshot()?.1.mirror_generation(), 1);

        vault.begin_dual_write()?;
        write(&ratchet_path, b"ratchet-staged")?;
        write(&event_path, b"event-staged")?;
        assert!(matches!(
            vault.commit_primary_checkpoint_internal(Some(1)),
            Err(StateError::VaultInjectedFailure(1))
        ));
        assert_eq!(vault.verify()?.mirror_generation(), 1);
        assert!(vault.load_primary_shadow_intent()?.is_none());
        write(&ratchet_path, b"ratchet-before")?;
        fs::remove_file(&event_path)?;
        assert_eq!(
            vault.finish_dual_write()?.outcome(),
            VaultMirrorOutcome::AlreadyCurrent
        );

        vault.begin_dual_write()?;
        let transaction = crate::StateTransaction::begin(directory.path())?;
        write(&ratchet_path, b"ratchet-primary")?;
        write(&event_path, b"event-primary")?;
        let primary = vault.commit_primary_checkpoint()?;
        assert_eq!(primary.outcome(), VaultMirrorOutcome::Mirrored);
        assert_eq!(primary.report().mirror_generation(), 2);
        assert_eq!(primary.delta().upserted_records(), 2);
        assert!(matches!(
            vault.read_primary_canary(&[StateRecordKind::Event]),
            Err(StateError::VaultPrimaryShadowRecoveryRequired { generation: 2 })
        ));
        assert!(matches!(
            vault.recover_primary_shadow(),
            Err(StateError::VaultPrimaryShadowBlockedByLocalTransaction { .. })
        ));
        drop(transaction);
        let state_lock = crate::StateDirectoryLock::acquire(directory.path())?;
        assert_eq!(fs::read(&ratchet_path)?, b"ratchet-before");
        assert!(!event_path.exists());
        let recovered = vault
            .recover_primary_shadow()?
            .ok_or("expected primary shadow recovery")?;
        assert_eq!(recovered.mirror_generation(), 2);
        assert_eq!(fs::read(&ratchet_path)?, b"ratchet-primary");
        assert_eq!(fs::read(&event_path)?, b"event-primary");
        assert!(vault.recover_primary_shadow()?.is_none());
        let completed = vault.finish_dual_write()?;
        assert_eq!(completed.outcome(), VaultMirrorOutcome::AlreadyCurrent);
        assert_eq!(completed.report().mirror_generation(), 2);
        drop(state_lock);

        vault.begin_dual_write()?;
        let projection_path = directory.path().join("local-messages/new.local-text");
        write(&projection_path, b"projection-primary")?;
        assert_eq!(
            vault
                .commit_primary_checkpoint()?
                .report()
                .mirror_generation(),
            3
        );
        assert_eq!(vault.confirm_primary_shadow()?.mirror_generation(), 3);
        write(
            &directory.path().join("account-authority.snapshot"),
            b"trust",
        )?;
        let final_commit = vault.finish_dual_write()?;
        assert_eq!(final_commit.report().mirror_generation(), 4);
        assert_eq!(vault.verify_against_legacy()?, *final_commit.report());

        vault.begin_dual_write()?;
        write(
            &directory.path().join("events/chat/tampered.event"),
            b"tampered-marker-fixture",
        )?;
        let tampered_commit = vault.commit_primary_checkpoint()?;
        let forged = VaultPrimaryShadowIntent {
            version: MIRROR_METADATA_VERSION,
            generation: tampered_commit.report().mirror_generation(),
            snapshot_id: *tampered_commit.report().snapshot_id(),
            authenticator: [0_u8; 32],
        };
        let encoded = postcard::to_allocvec(&forged)?;
        let write = vault.database.begin_write().map_err(vault_database_error)?;
        {
            let mut table = write.open_table(META_TABLE).map_err(vault_database_error)?;
            table
                .insert(PRIMARY_SHADOW_INTENT_KEY, encoded.as_slice())
                .map_err(vault_database_error)?;
        }
        write.commit().map_err(vault_database_error)?;
        assert!(matches!(
            vault.recover_primary_shadow(),
            Err(StateError::VaultPrimaryShadowIntentAuthenticationFailed)
        ));
        Ok(())
    }

    #[test]
    fn typed_transaction_commits_only_journal_delta_and_sequence_reads_from_db()
    -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        let changed_ratchet = directory.path().join("ratchet/changed.pickle");
        let removed_ratchet = directory.path().join("ratchet/removed.pickle");
        let sequence = directory.path().join("next-sequence");
        let new_event = directory.path().join("events/chat/new.event");
        write(&changed_ratchet, b"ratchet-before")?;
        write(&removed_ratchet, b"remove-me")?;
        write(&sequence, b"4\n")?;
        write(
            &directory.path().join("events/chat/existing.event"),
            b"existing-event",
        )?;

        let vault = EncryptedStateVault::open_or_create(directory.path())?;
        assert_eq!(vault.migrate_legacy_snapshot()?.1.mirror_generation(), 1);
        vault.begin_dual_write()?;
        let transaction = crate::StateTransaction::begin(directory.path())?;
        write(&changed_ratchet, b"ratchet-after")?;
        fs::remove_file(&removed_ratchet)?;
        write(&sequence, b"5\n")?;
        write(&new_event, b"new-event")?;
        write(
            &directory.path().join("peer-authority/peer.snapshot"),
            b"trust-update",
        )?;

        let mutable = vault.read_mutable_primary_canary(&[StateRecordKind::Sequence])?;
        assert_eq!(mutable.mirror_generation(), 1);
        assert_eq!(mutable.records().len(), 1);
        assert_eq!(mutable.records()[0].relative_path(), "next-sequence");
        assert_eq!(mutable.records()[0].content(), b"4\n");
        assert_eq!(fs::read(&sequence)?, b"5\n");

        assert!(matches!(
            vault.commit_primary_transaction_internal(&transaction, Some(1)),
            Err(StateError::VaultInjectedFailure(1))
        ));
        assert_eq!(vault.verify()?.mirror_generation(), 1);
        assert!(vault.load_primary_shadow_intent()?.is_none());

        let commit = vault.commit_primary_transaction(&transaction)?;
        assert_eq!(commit.outcome(), VaultMirrorOutcome::Mirrored);
        assert_eq!(commit.report().mirror_generation(), 2);
        assert_eq!(commit.delta().upserted_records(), 4);
        assert_eq!(commit.delta().removed_records(), 1);
        assert_eq!(commit.delta().unchanged_records(), 1);
        assert!(matches!(
            vault.read_mutable_primary_canary(&[StateRecordKind::Sequence]),
            Err(StateError::VaultPrimaryShadowRecoveryRequired { generation: 2 })
        ));
        transaction.commit()?;
        assert_eq!(vault.confirm_primary_shadow()?.mirror_generation(), 2);
        assert_eq!(
            vault.finish_dual_write()?.outcome(),
            VaultMirrorOutcome::AlreadyCurrent
        );
        assert_eq!(vault.verify_against_legacy()?.mirror_generation(), 2);

        assert!(matches!(
            vault.read_mutable_primary_canary(&[StateRecordKind::Sequence]),
            Err(StateError::VaultMirrorIntentMissing)
        ));
        vault.begin_dual_write()?;
        assert!(matches!(
            vault.read_mutable_primary_canary(&[StateRecordKind::Ratchet]),
            Err(StateError::VaultPrimaryReadKindNotAllowed(kind)) if kind == "ratchet"
        ));
        let other = tempfile::tempdir()?;
        let other_transaction = crate::StateTransaction::begin(other.path())?;
        assert!(matches!(
            vault.commit_primary_transaction(&other_transaction),
            Err(StateError::VaultTransactionRootMismatch { .. })
        ));
        other_transaction.rollback()?;
        vault.finish_dual_write()?;
        Ok(())
    }

    #[test]
    fn incremental_typed_mirror_preserves_unchanged_ciphertext_and_applies_deletions()
    -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        let fixtures = [
            ("device-secret.key", b"identity".as_slice()),
            ("ratchet/session.pickle", b"ratchet-one".as_slice()),
            ("events/chat/old.event", b"event-old".as_slice()),
            ("local-messages/old.local-text", b"projection".as_slice()),
            ("history-rewraps/transfer.rewrap", b"rewrap".as_slice()),
            ("history-recovery/page.checkpoint", b"recovery".as_slice()),
            ("account-authority.snapshot", b"trust".as_slice()),
            ("next-sequence", b"7".as_slice()),
            ("future-extension.bin", b"other".as_slice()),
        ];
        for (path, content) in fixtures {
            write(&directory.path().join(path), content)?;
        }

        let vault = EncryptedStateVault::open_or_create(directory.path())?;
        vault.migrate_legacy_snapshot()?;
        let identity_before = encrypted_record(&vault, "device-secret.key")?;
        let ratchet_before = encrypted_record(&vault, "ratchet/session.pickle")?;

        vault.begin_dual_write()?;
        write(
            &directory.path().join("ratchet/session.pickle"),
            b"ratchet-two",
        )?;
        fs::remove_file(directory.path().join("events/chat/old.event"))?;
        write(
            &directory.path().join("events/chat/new.event"),
            b"event-new",
        )?;
        let commit = vault.finish_dual_write()?;
        assert_eq!(commit.outcome(), VaultMirrorOutcome::Mirrored);
        assert_eq!(commit.report().mirror_generation(), 2);
        assert_eq!(commit.delta().upserted_records(), 2);
        assert_eq!(commit.delta().removed_records(), 1);
        assert_eq!(commit.delta().unchanged_records(), 7);
        assert_eq!(
            encrypted_record(&vault, "device-secret.key")?,
            identity_before
        );
        assert_ne!(
            encrypted_record(&vault, "ratchet/session.pickle")?,
            ratchet_before
        );
        assert!(encrypted_record(&vault, "events/chat/old.event").is_err());

        let typed = vault.verify_typed_shadow_reads()?;
        assert_eq!(typed.len(), StateRecordKind::ALL.len());
        assert!(typed.iter().all(|report| report.record_count() == 1));

        let primary = vault
            .read_primary_canary(&[StateRecordKind::Event, StateRecordKind::LocalProjection])?;
        assert_eq!(primary.mirror_generation(), 2);
        assert_eq!(primary.shadow_reports().len(), 2);
        assert_eq!(primary.records().len(), 2);
        assert!(primary.records().iter().all(|record| matches!(
            record.kind(),
            StateRecordKind::Event | StateRecordKind::LocalProjection
        )));
        assert!(matches!(
            vault.read_primary_canary(&[StateRecordKind::Ratchet]),
            Err(StateError::VaultPrimaryReadKindNotAllowed(kind)) if kind == "ratchet"
        ));
        assert!(matches!(
            vault.read_primary_canary(&[]),
            Err(StateError::VaultPrimaryReadSelectionEmpty)
        ));

        vault.begin_dual_write()?;
        assert_eq!(
            vault
                .read_primary_canary(&[StateRecordKind::Event])?
                .mirror_generation(),
            2
        );
        write(
            &directory.path().join("local-messages/old.local-text"),
            b"external-drift",
        )?;
        assert!(matches!(
            vault.read_primary_canary(&[StateRecordKind::LocalProjection]),
            Err(StateError::VaultTypedShadowReadMismatch { kind, relative_path })
                if kind == "local-projection"
                    && relative_path == "local-messages/old.local-text"
        ));
        let recovered = vault
            .recover_pending_dual_write()?
            .ok_or("expected pending canary mirror recovery")?;
        assert_eq!(recovered.report().mirror_generation(), 3);
        Ok(())
    }
}
