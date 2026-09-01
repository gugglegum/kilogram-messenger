use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use thiserror::Error;

mod key_provider;
mod vault;

pub use key_provider::{VaultKeyLoadOutcome, VaultKeyProtection};
pub use vault::{
    EncryptedStateVault, STATE_VAULT_FILE, STATE_VAULT_KEY_FILE, StateMirrorRepository,
    StateRecordKind, TrustStateRepository, TypedShadowReadReport, TypedStateRepository,
    VaultManifestIndexMode, VaultMigrationOutcome, VaultMirrorCommit, VaultMirrorDelta,
    VaultMirrorOutcome, VaultMutableRead, VaultPrimaryRead, VaultPrimaryRecord,
    VaultPrimaryWriteRepository, VaultReport,
};

const LOCK_FILE: &str = ".kilogram-state.lock";
const TRANSACTION_DIRECTORY: &str = ".kilogram-transactions";
const ACTIVE_DIRECTORY: &str = "active";
const BACKUP_DIRECTORY: &str = "backup";
const PRIMARY_BACKUP_DIRECTORY: &str = "primary-backup";
const MANIFEST_FILE: &str = "manifest.json";
const PREPARED_MARKER: &str = "prepared";
const COMMITTED_MARKER: &str = "committed";
const ROLLED_BACK_MARKER: &str = "rolled-back";
const RATCHET_DIRECTORY: &str = "ratchet";
const NEXT_SEQUENCE_FILE: &str = "next-sequence";
const TRUST_FILES: [&str; 2] = ["account-authority.snapshot", "device-certificate.cert"];
const TRUST_DIRECTORIES: [&str; 2] = ["conversation-memberships", "peer-authority"];
const TRUST_PRIMARY_BACKUP_DIRECTORY: &str = "trust";
const APPEND_ONLY_ROOTS: [&str; 4] = [
    "events",
    "local-messages",
    "history-rewraps",
    "history-recovery",
];
const MANIFEST_VERSION: u8 = 1;

#[derive(Debug, Error)]
pub enum StateError {
    #[error("state directory {path} is already locked by another Kilogram process")]
    AlreadyLocked { path: PathBuf },

    #[error("state transaction I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("decode state transaction manifest at {path}: {source}")]
    InvalidManifest {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("unsupported state transaction manifest version {0}")]
    UnsupportedManifestVersion(u8),

    #[error("another local state transaction is already active at {path}")]
    TransactionAlreadyActive { path: PathBuf },

    #[error("state transaction path is not a safe relative path: {0}")]
    UnsafeRelativePath(PathBuf),

    #[error("state transaction backup is missing or has the wrong type: {0}")]
    InvalidBackup(PathBuf),

    #[error("symbolic links are not allowed in transaction-managed state: {0}")]
    SymbolicLink(PathBuf),

    #[error("state vault database operation failed: {0}")]
    VaultDatabase(String),

    #[error("state vault encoding failed: {0}")]
    VaultEncoding(#[from] postcard::Error),

    #[error("state vault secure random generation failed: {0}")]
    VaultSecureRandom(getrandom::Error),

    #[error("state vault authenticated encryption failed")]
    VaultEncryption,

    #[error("state vault key is missing at {0}")]
    VaultKeyMissing(PathBuf),

    #[error("state vault database is missing at {0}")]
    VaultDatabaseMissing(PathBuf),

    #[error("state vault key has {0} bytes; expected 32")]
    InvalidVaultKeyLength(usize),

    #[error("invalid state vault key envelope at {path}: {detail}")]
    InvalidVaultKeyEnvelope { path: PathBuf, detail: String },

    #[error("state vault key envelope has {0} bytes; maximum is 64 KiB")]
    VaultKeyEnvelopeTooLarge(usize),

    #[error("unsupported state vault key envelope version {0}")]
    UnsupportedVaultKeyEnvelopeVersion(u8),

    #[error("state vault key provider {0} is unavailable on this platform")]
    VaultKeyProviderUnavailable(String),

    #[error("state vault key provider {provider} failed to {operation}: {detail}")]
    VaultKeyProtectionFailed {
        provider: String,
        operation: &'static str,
        detail: String,
    },

    #[error("state vault at {0} does not contain a committed migration")]
    VaultNotMigrated(PathBuf),

    #[error("unsupported state vault schema version {0}")]
    UnsupportedVaultSchemaVersion(u64),

    #[error("unsupported state vault manifest index version {0}")]
    UnsupportedVaultManifestIndexVersion(u8),

    #[error("state vault schema requires an authenticated manifest index")]
    VaultManifestIndexMissing,

    #[error("unsupported state vault record version {0}")]
    UnsupportedVaultRecordVersion(u8),

    #[error("state vault record envelope is truncated")]
    InvalidVaultRecordEnvelope,

    #[error("state vault record key has {0} bytes; expected 32")]
    InvalidVaultRecordKeyLength(usize),

    #[error("state vault record key does not match encrypted path {0}")]
    VaultRecordKeyMismatch(String),

    #[error("state vault contains duplicate encrypted path {0}")]
    VaultDuplicatePath(String),

    #[error("state vault record has {0} bytes; maximum is 64 MiB")]
    VaultRecordTooLarge(usize),

    #[error("state vault contains {0} records; maximum is 1000000")]
    TooManyVaultRecords(usize),

    #[error("state vault snapshot has {0} plaintext bytes; maximum is 512 MiB")]
    VaultSnapshotTooLarge(u64),

    #[error("state vault manifest does not match decrypted records")]
    VaultManifestMismatch,

    #[error("legacy state changed after vault snapshot (stored {stored:?}, current {current:?})")]
    VaultLegacyStateChanged { stored: [u8; 32], current: [u8; 32] },

    #[error("state vault typed shadow read mismatch for {kind} record at {relative_path}")]
    VaultTypedShadowReadMismatch { kind: String, relative_path: String },

    #[error("state vault primary-read canary requires at least one repository kind")]
    VaultPrimaryReadSelectionEmpty,

    #[error("state vault primary-read canary does not allow repository kind {0}")]
    VaultPrimaryReadKindNotAllowed(String),

    #[error("state vault path is not valid UTF-8: {0}")]
    VaultNonUtf8Path(PathBuf),

    #[error("state vault restore destination already exists: {0}")]
    VaultRestoreDestinationExists(PathBuf),

    #[error("state vault restore destination is inside the source state directory: {0}")]
    VaultRestoreInsideSource(PathBuf),

    #[error("unsupported state vault mirror metadata version {0}")]
    UnsupportedVaultMirrorMetadataVersion(u8),

    #[error("state vault mirror generation must be greater than zero, got {0}")]
    InvalidVaultGeneration(u64),

    #[error("state vault mirror generation authentication failed")]
    VaultGenerationAuthenticationFailed,

    #[error("state vault mirror intent authentication failed")]
    VaultMirrorIntentAuthenticationFailed,

    #[error("state vault mirror recovery is required from generation {base_generation}")]
    VaultMirrorRecoveryRequired { base_generation: u64 },

    #[error("state vault mirror intent is missing")]
    VaultMirrorIntentMissing,

    #[error("state vault mirror intent does not match the active snapshot")]
    VaultMirrorIntentBaseMismatch,

    #[error("state vault primary-shadow intent authentication failed")]
    VaultPrimaryShadowIntentAuthenticationFailed,

    #[error(
        "state vault primary commit at generation {generation} must restore or confirm its legacy shadow"
    )]
    VaultPrimaryShadowRecoveryRequired { generation: u64 },

    #[error("state vault primary shadow recovery is blocked by active local transaction at {path}")]
    VaultPrimaryShadowBlockedByLocalTransaction { path: PathBuf },

    #[error("state vault transaction belongs to {transaction_root}, not vault root {vault_root}")]
    VaultTransactionRootMismatch {
        transaction_root: PathBuf,
        vault_root: PathBuf,
    },

    #[error("state vault direct transaction does not allow repository kind {0}")]
    VaultDirectWriteKindNotAllowed(String),

    #[error("state vault direct transaction contains duplicate path {0}")]
    VaultDirectWriteDuplicatePath(PathBuf),

    #[error(
        "state vault direct transaction classified {relative_path} as {declared_kind}, but the canonical path kind is {actual_kind}"
    )]
    VaultDirectWriteKindMismatch {
        relative_path: String,
        declared_kind: String,
        actual_kind: String,
    },

    #[error("append-only state record disappeared during a transaction: {0}")]
    AppendOnlyRecordRemoved(PathBuf),

    #[error("append-only state record was modified in place: {0}")]
    AppendOnlyRecordModified(String),

    #[error("append-only state record is no longer a regular file: {0}")]
    AppendOnlyRecordTypeChanged(PathBuf),

    #[error("state transaction append-only write is outside an allowed repository: {0}")]
    AppendOnlyWriteKindNotAllowed(PathBuf),

    #[error("state transaction already prepared its DB-primary ratchet workspace")]
    RatchetWorkspaceAlreadyPrepared,

    #[error(
        "DB-primary ratchet workspace belongs to {workspace_root}, not transaction root {transaction_root}"
    )]
    RatchetWorkspaceRootMismatch {
        workspace_root: PathBuf,
        transaction_root: PathBuf,
    },

    #[error("DB-primary ratchet workspace contains non-ratchet record {0}")]
    RatchetWorkspaceKindMismatch(String),

    #[error("state transaction already prepared its DB-primary sequence workspace")]
    SequenceWorkspaceAlreadyPrepared,

    #[error("state transaction already prepared its DB-primary trust workspace")]
    TrustWorkspaceAlreadyPrepared,

    #[error(
        "DB-primary trust workspace belongs to {workspace_root}, not transaction root {transaction_root}"
    )]
    TrustWorkspaceRootMismatch {
        workspace_root: PathBuf,
        transaction_root: PathBuf,
    },

    #[error("DB-primary trust workspace contains non-trust record {0}")]
    TrustWorkspaceKindMismatch(String),

    #[error(
        "DB-primary sequence workspace belongs to {workspace_root}, not transaction root {transaction_root}"
    )]
    SequenceWorkspaceRootMismatch {
        workspace_root: PathBuf,
        transaction_root: PathBuf,
    },

    #[error("DB-primary sequence workspace contains unexpected record {0}")]
    SequenceWorkspaceKindMismatch(String),

    #[error("retained trust record disappeared before a direct vault transaction: {0}")]
    VaultTrustRecordRemoved(String),

    #[error("trust state changed outside the DB-primary trust repository: {0}")]
    VaultUnregisteredTrustMutation(String),

    #[error("state vault mirror generation is exhausted")]
    VaultGenerationExhausted,

    #[error("injected state vault failure after {0} records")]
    VaultInjectedFailure(usize),
}

pub struct StateDirectoryLock {
    root: PathBuf,
    _file: File,
}

impl StateDirectoryLock {
    pub fn acquire(state_directory: impl AsRef<Path>) -> Result<Self, StateError> {
        let requested = state_directory.as_ref();
        io_at(requested, fs::create_dir_all(requested))?;
        let root = io_at(requested, fs::canonicalize(requested))?;
        let lock_path = root.join(LOCK_FILE);
        let file = io_at(
            &lock_path,
            OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(&lock_path),
        )?;
        if let Err(source) = file.try_lock() {
            return match source {
                fs::TryLockError::WouldBlock => Err(StateError::AlreadyLocked { path: root }),
                fs::TryLockError::Error(source) => Err(StateError::Io {
                    path: lock_path,
                    source,
                }),
            };
        }
        recover(&root)?;
        Ok(Self { root, _file: file })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[derive(Debug)]
#[must_use = "a prepared state transaction must be explicitly committed or rolled back"]
pub struct StateTransaction {
    root: PathBuf,
    active: PathBuf,
    ratchet_primary_baseline: Option<BTreeMap<PathBuf, Vec<u8>>>,
    sequence_primary_prepared: bool,
    trust_primary_baseline: Option<BTreeMap<PathBuf, Vec<u8>>>,
    append_only_writes: BTreeSet<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StagedStateMutation {
    pub kind: StateRecordKind,
    pub relative_path: PathBuf,
    pub content: Option<Vec<u8>>,
}

impl StateTransaction {
    pub fn begin(state_directory: impl AsRef<Path>) -> Result<Self, StateError> {
        let requested = state_directory.as_ref();
        io_at(requested, fs::create_dir_all(requested))?;
        let root = io_at(requested, fs::canonicalize(requested))?;
        let active = active_directory(&root);
        if path_exists(&active)? {
            return Err(StateError::TransactionAlreadyActive { path: active });
        }
        io_at(&active, fs::create_dir_all(active.join(BACKUP_DIRECTORY)))?;

        let manifest = prepare_snapshot(&root, &active)?;
        write_manifest(&active, &manifest)?;
        write_marker(&active.join(PREPARED_MARKER))?;

        Ok(Self {
            root,
            active,
            ratchet_primary_baseline: None,
            sequence_primary_prepared: false,
            trust_primary_baseline: None,
            append_only_writes: BTreeSet::new(),
        })
    }

    pub fn commit(self) -> Result<(), StateError> {
        write_marker(&self.active.join(COMMITTED_MARKER))?;
        remove_tree_if_present(&self.active)?;
        Ok(())
    }

    pub fn rollback(self) -> Result<(), StateError> {
        rollback_active(&self.root, &self.active)?;
        Ok(())
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub fn ratchet_workspace_prepared(&self) -> bool {
        self.ratchet_primary_baseline.is_some()
    }

    pub fn trust_workspace_prepared(&self) -> bool {
        self.trust_primary_baseline.is_some()
    }

    pub fn registered_append_only_write_count(&self) -> usize {
        self.append_only_writes.len()
    }

    pub fn prepare_ratchet_workspace(&mut self, read: &VaultMutableRead) -> Result<(), StateError> {
        if self.ratchet_primary_baseline.is_some() {
            return Err(StateError::RatchetWorkspaceAlreadyPrepared);
        }
        if read.state_root() != self.root {
            return Err(StateError::RatchetWorkspaceRootMismatch {
                workspace_root: read.state_root().to_path_buf(),
                transaction_root: self.root.clone(),
            });
        }
        if read.selected_kind_count() != 1 || !read.includes_kind(StateRecordKind::Ratchet) {
            return Err(StateError::RatchetWorkspaceKindMismatch(
                "mutable read selection".to_owned(),
            ));
        }

        let mut baseline = BTreeMap::new();
        for record in read.records() {
            if record.kind() != StateRecordKind::Ratchet {
                return Err(StateError::RatchetWorkspaceKindMismatch(
                    record.relative_path().to_owned(),
                ));
            }
            let relative_path = PathBuf::from(record.relative_path());
            validate_relative(&relative_path)?;
            if state_record_kind_for_path(&relative_path) != StateRecordKind::Ratchet {
                return Err(StateError::RatchetWorkspaceKindMismatch(
                    record.relative_path().to_owned(),
                ));
            }
            if baseline
                .insert(relative_path.clone(), record.content().to_vec())
                .is_some()
            {
                return Err(StateError::VaultDirectWriteDuplicatePath(relative_path));
            }
        }

        let mut manifest = read_manifest(&self.active)?;
        let primary_backup = self
            .active
            .join(PRIMARY_BACKUP_DIRECTORY)
            .join(RATCHET_DIRECTORY);
        remove_tree_if_present(&primary_backup)?;
        for (relative_path, content) in &baseline {
            let relative_ratchet = relative_path.strip_prefix(RATCHET_DIRECTORY).map_err(|_| {
                StateError::RatchetWorkspaceKindMismatch(relative_path.display().to_string())
            })?;
            write_staged_file(&primary_backup.join(relative_ratchet), content)?;
        }
        manifest.ratchet_primary_existed = Some(!baseline.is_empty());
        write_manifest(&self.active, &manifest)?;

        let ratchet = self.root.join(RATCHET_DIRECTORY);
        remove_tree_if_present(&ratchet)?;
        for (relative_path, content) in &baseline {
            write_staged_file(&self.root.join(relative_path), content)?;
        }
        self.ratchet_primary_baseline = Some(baseline);
        Ok(())
    }

    pub fn prepare_sequence_workspace(
        &mut self,
        read: &VaultMutableRead,
    ) -> Result<(), StateError> {
        if self.sequence_primary_prepared {
            return Err(StateError::SequenceWorkspaceAlreadyPrepared);
        }
        if read.state_root() != self.root {
            return Err(StateError::SequenceWorkspaceRootMismatch {
                workspace_root: read.state_root().to_path_buf(),
                transaction_root: self.root.clone(),
            });
        }
        if read.selected_kind_count() != 1 || !read.includes_kind(StateRecordKind::Sequence) {
            return Err(StateError::SequenceWorkspaceKindMismatch(
                "mutable read selection".to_owned(),
            ));
        }
        if read.records().len() > 1 {
            return Err(StateError::SequenceWorkspaceKindMismatch(
                "multiple sequence records".to_owned(),
            ));
        }
        let content = match read.records().first() {
            Some(record)
                if record.kind() == StateRecordKind::Sequence
                    && record.relative_path() == NEXT_SEQUENCE_FILE =>
            {
                Some(record.content())
            }
            Some(record) => {
                return Err(StateError::SequenceWorkspaceKindMismatch(
                    record.relative_path().to_owned(),
                ));
            }
            None => None,
        };

        let mut manifest = read_manifest(&self.active)?;
        let primary_backup = self
            .active
            .join(PRIMARY_BACKUP_DIRECTORY)
            .join(NEXT_SEQUENCE_FILE);
        remove_file_if_present(&primary_backup)?;
        if let Some(content) = content {
            write_staged_file(&primary_backup, content)?;
        }
        manifest.next_sequence_primary_existed = Some(content.is_some());
        write_manifest(&self.active, &manifest)?;

        let sequence = self.root.join(NEXT_SEQUENCE_FILE);
        remove_file_if_present(&sequence)?;
        if let Some(content) = content {
            write_staged_file(&sequence, content)?;
        }
        self.sequence_primary_prepared = true;
        Ok(())
    }

    pub fn prepare_trust_workspace(&mut self, read: &VaultMutableRead) -> Result<(), StateError> {
        if self.trust_primary_baseline.is_some() {
            return Err(StateError::TrustWorkspaceAlreadyPrepared);
        }
        if read.state_root() != self.root {
            return Err(StateError::TrustWorkspaceRootMismatch {
                workspace_root: read.state_root().to_path_buf(),
                transaction_root: self.root.clone(),
            });
        }
        if read.selected_kind_count() != 1 || !read.includes_kind(StateRecordKind::Trust) {
            return Err(StateError::TrustWorkspaceKindMismatch(
                "mutable read selection".to_owned(),
            ));
        }

        let mut baseline = BTreeMap::new();
        for record in read.records() {
            if record.kind() != StateRecordKind::Trust {
                return Err(StateError::TrustWorkspaceKindMismatch(
                    record.relative_path().to_owned(),
                ));
            }
            let relative_path = PathBuf::from(record.relative_path());
            validate_relative(&relative_path)?;
            if state_record_kind_for_path(&relative_path) != StateRecordKind::Trust {
                return Err(StateError::TrustWorkspaceKindMismatch(
                    record.relative_path().to_owned(),
                ));
            }
            if baseline
                .insert(relative_path.clone(), record.content().to_vec())
                .is_some()
            {
                return Err(StateError::VaultDirectWriteDuplicatePath(relative_path));
            }
        }
        self.prepare_trust_workspace_from_baseline(baseline)
    }

    pub fn prepare_legacy_trust_workspace(&mut self) -> Result<(), StateError> {
        if self.trust_primary_baseline.is_some() {
            return Err(StateError::TrustWorkspaceAlreadyPrepared);
        }
        let baseline = collect_trust_contents(&self.root)?;
        self.prepare_trust_workspace_from_baseline(baseline)
    }

    fn prepare_trust_workspace_from_baseline(
        &mut self,
        baseline: BTreeMap<PathBuf, Vec<u8>>,
    ) -> Result<(), StateError> {
        let primary_backup = self
            .active
            .join(PRIMARY_BACKUP_DIRECTORY)
            .join(TRUST_PRIMARY_BACKUP_DIRECTORY);
        remove_tree_if_present(&primary_backup)?;
        io_at(&primary_backup, fs::create_dir_all(&primary_backup))?;
        for (relative_path, content) in &baseline {
            write_staged_file(&primary_backup.join(relative_path), content)?;
        }

        let mut manifest = read_manifest(&self.active)?;
        manifest.trust_primary_prepared = true;
        write_manifest(&self.active, &manifest)?;

        clear_trust_contents(&self.root)?;
        for (relative_path, content) in &baseline {
            write_staged_file(&self.root.join(relative_path), content)?;
        }
        self.trust_primary_baseline = Some(baseline);
        Ok(())
    }

    pub fn register_append_only_write(
        &mut self,
        relative_path: impl AsRef<Path>,
    ) -> Result<(), StateError> {
        let relative_path = relative_path.as_ref();
        validate_relative(relative_path)?;
        if !matches!(
            state_record_kind_for_path(relative_path),
            StateRecordKind::Event
                | StateRecordKind::LocalProjection
                | StateRecordKind::HistoryRewrap
                | StateRecordKind::HistoryRecovery
        ) {
            return Err(StateError::AppendOnlyWriteKindNotAllowed(
                relative_path.to_path_buf(),
            ));
        }
        reject_relative_symlinks(&self.root, relative_path)?;
        self.append_only_writes.insert(relative_path.to_path_buf());
        Ok(())
    }

    /// Registers an absolute path returned by an append-only repository.
    ///
    /// Repository receipts are rooted at their canonical store directory; the
    /// transaction owns the conversion back to its canonical state-relative
    /// path and rejects receipts from any other state tree.
    pub fn register_append_only_receipt_path(
        &mut self,
        absolute_path: impl AsRef<Path>,
    ) -> Result<(), StateError> {
        let absolute_path = absolute_path.as_ref();
        if !absolute_path.is_absolute() {
            return Err(StateError::UnsafeRelativePath(absolute_path.to_path_buf()));
        }
        let canonical_path = io_at(absolute_path, fs::canonicalize(absolute_path))?;
        let relative_path = canonical_path
            .strip_prefix(&self.root)
            .map_err(|_| StateError::UnsafeRelativePath(canonical_path.clone()))?;
        self.register_append_only_write(relative_path)
    }

    pub(crate) fn staged_mutations(&self) -> Result<Vec<StagedStateMutation>, StateError> {
        let manifest = read_manifest(&self.active)?;
        let mut mutations = BTreeMap::<PathBuf, StagedStateMutation>::new();

        if let Some(baseline) = &self.ratchet_primary_baseline {
            collect_primary_tree_mutations(
                &self.root,
                &self.root.join(RATCHET_DIRECTORY),
                baseline,
                StateRecordKind::Ratchet,
                &mut mutations,
            )?;
        } else {
            collect_mutable_tree_mutations(
                &self.root,
                &self.root.join(RATCHET_DIRECTORY),
                &self.active.join(BACKUP_DIRECTORY).join(RATCHET_DIRECTORY),
                manifest.ratchet_existed,
                StateRecordKind::Ratchet,
                &mut mutations,
            )?;
        }
        collect_single_file_mutation(
            &self.root,
            &self.root.join(NEXT_SEQUENCE_FILE),
            &self.active.join(BACKUP_DIRECTORY).join(NEXT_SEQUENCE_FILE),
            manifest.next_sequence_existed,
            StateRecordKind::Sequence,
            &mut mutations,
        )?;
        if let Some(baseline) = &self.trust_primary_baseline {
            let current = collect_trust_contents(&self.root)?;
            let paths: BTreeSet<_> = current.keys().chain(baseline.keys()).cloned().collect();
            for relative_path in paths {
                if current.get(&relative_path) == baseline.get(&relative_path) {
                    continue;
                }
                insert_staged_mutation(
                    &mut mutations,
                    StagedStateMutation {
                        kind: StateRecordKind::Trust,
                        relative_path: relative_path.clone(),
                        content: current.get(&relative_path).cloned(),
                    },
                )?;
            }
        }

        let baseline: BTreeSet<_> = manifest.append_only_files.into_iter().collect();
        for relative_path in &baseline {
            let path = self.root.join(relative_path);
            if !path_exists(&path)? {
                return Err(StateError::AppendOnlyRecordRemoved(relative_path.clone()));
            }
            reject_relative_symlinks(&self.root, relative_path)?;
            if !io_at(&path, fs::metadata(&path))?.is_file() {
                return Err(StateError::AppendOnlyRecordTypeChanged(
                    relative_path.clone(),
                ));
            }
        }
        for relative_path in &self.append_only_writes {
            let path = self.root.join(relative_path);
            reject_relative_symlinks(&self.root, relative_path)?;
            if !io_at(&path, fs::metadata(&path))?.is_file() {
                return Err(StateError::AppendOnlyRecordTypeChanged(
                    relative_path.clone(),
                ));
            }
            let content = io_at(&path, fs::read(&path))?;
            insert_staged_mutation(
                &mut mutations,
                StagedStateMutation {
                    kind: state_record_kind_for_path(relative_path),
                    relative_path: relative_path.clone(),
                    content: Some(content),
                },
            )?;
        }

        Ok(mutations.into_values().collect())
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct TransactionManifest {
    version: u8,
    ratchet_existed: bool,
    next_sequence_existed: bool,
    append_only_files: Vec<PathBuf>,
    #[serde(default)]
    ratchet_primary_existed: Option<bool>,
    #[serde(default)]
    next_sequence_primary_existed: Option<bool>,
    #[serde(default)]
    trust_primary_prepared: bool,
}

fn prepare_snapshot(root: &Path, active: &Path) -> Result<TransactionManifest, StateError> {
    let ratchet = root.join(RATCHET_DIRECTORY);
    let ratchet_existed = path_exists(&ratchet)?;
    if ratchet_existed {
        copy_tree(
            &ratchet,
            &active.join(BACKUP_DIRECTORY).join(RATCHET_DIRECTORY),
        )?;
    }

    let next_sequence = root.join(NEXT_SEQUENCE_FILE);
    let next_sequence_existed = path_exists(&next_sequence)?;
    if next_sequence_existed {
        copy_file(
            &next_sequence,
            &active.join(BACKUP_DIRECTORY).join(NEXT_SEQUENCE_FILE),
        )?;
    }

    let mut append_only_files = Vec::new();
    for name in APPEND_ONLY_ROOTS {
        collect_relative_files(root, &root.join(name), &mut append_only_files)?;
    }
    append_only_files.sort();

    Ok(TransactionManifest {
        version: MANIFEST_VERSION,
        ratchet_existed,
        next_sequence_existed,
        append_only_files,
        ratchet_primary_existed: None,
        next_sequence_primary_existed: None,
        trust_primary_prepared: false,
    })
}

fn recover(root: &Path) -> Result<(), StateError> {
    let active = active_directory(root);
    if !path_exists(&active)? {
        return Ok(());
    }
    if !path_exists(&active.join(PREPARED_MARKER))?
        || path_exists(&active.join(COMMITTED_MARKER))?
        || path_exists(&active.join(ROLLED_BACK_MARKER))?
    {
        return remove_tree_if_present(&active);
    }
    rollback_active(root, &active)
}

fn rollback_active(root: &Path, active: &Path) -> Result<(), StateError> {
    let manifest_path = active.join(MANIFEST_FILE);
    let encoded = io_at(&manifest_path, fs::read(&manifest_path))?;
    let manifest: TransactionManifest =
        serde_json::from_slice(&encoded).map_err(|source| StateError::InvalidManifest {
            path: manifest_path,
            source,
        })?;
    if manifest.version != MANIFEST_VERSION {
        return Err(StateError::UnsupportedManifestVersion(manifest.version));
    }
    for path in &manifest.append_only_files {
        validate_relative(path)?;
        if !APPEND_ONLY_ROOTS.iter().any(|root| path.starts_with(root)) {
            return Err(StateError::UnsafeRelativePath(path.clone()));
        }
    }

    let (ratchet_existed, ratchet_backup) = match manifest.ratchet_primary_existed {
        Some(existed) => (
            existed,
            active
                .join(PRIMARY_BACKUP_DIRECTORY)
                .join(RATCHET_DIRECTORY),
        ),
        None => (
            manifest.ratchet_existed,
            active.join(BACKUP_DIRECTORY).join(RATCHET_DIRECTORY),
        ),
    };
    if ratchet_existed {
        if !path_exists(&ratchet_backup)? {
            return Err(StateError::InvalidBackup(ratchet_backup));
        }
        let mut backup_files = Vec::new();
        collect_relative_files(active, &ratchet_backup, &mut backup_files)?;
    }
    let (next_sequence_existed, next_sequence_backup) = match manifest.next_sequence_primary_existed
    {
        Some(existed) => (
            existed,
            active
                .join(PRIMARY_BACKUP_DIRECTORY)
                .join(NEXT_SEQUENCE_FILE),
        ),
        None => (
            manifest.next_sequence_existed,
            active.join(BACKUP_DIRECTORY).join(NEXT_SEQUENCE_FILE),
        ),
    };
    if next_sequence_existed {
        if !path_exists(&next_sequence_backup)? {
            return Err(StateError::InvalidBackup(next_sequence_backup));
        }
        reject_symlink(&next_sequence_backup)?;
        let metadata = io_at(&next_sequence_backup, fs::metadata(&next_sequence_backup))?;
        if !metadata.is_file() {
            return Err(StateError::InvalidBackup(next_sequence_backup));
        }
    }

    let ratchet = root.join(RATCHET_DIRECTORY);
    remove_tree_if_present(&ratchet)?;
    if ratchet_existed {
        copy_tree(&ratchet_backup, &ratchet)?;
    }

    let next_sequence = root.join(NEXT_SEQUENCE_FILE);
    remove_file_if_present(&next_sequence)?;
    if next_sequence_existed {
        copy_file(&next_sequence_backup, &next_sequence)?;
    }
    if manifest.trust_primary_prepared {
        let trust_backup = active
            .join(PRIMARY_BACKUP_DIRECTORY)
            .join(TRUST_PRIMARY_BACKUP_DIRECTORY);
        if !path_exists(&trust_backup)? {
            return Err(StateError::InvalidBackup(trust_backup));
        }
        clear_trust_contents(root)?;
        let backup = collect_tree_contents(&trust_backup, &trust_backup)?;
        for (relative_path, content) in backup {
            if state_record_kind_for_path(&relative_path) != StateRecordKind::Trust {
                return Err(StateError::TrustWorkspaceKindMismatch(
                    relative_path.display().to_string(),
                ));
            }
            write_staged_file(&root.join(relative_path), &content)?;
        }
    }
    let baseline: HashSet<_> = manifest.append_only_files.into_iter().collect();
    for name in APPEND_ONLY_ROOTS {
        remove_new_files(root, &root.join(name), &baseline)?;
    }
    write_marker(&active.join(ROLLED_BACK_MARKER))?;
    remove_tree_if_present(active)
}

fn write_manifest(active: &Path, manifest: &TransactionManifest) -> Result<(), StateError> {
    let encoded = serde_json::to_vec(manifest).map_err(|source| StateError::InvalidManifest {
        path: active.join(MANIFEST_FILE),
        source,
    })?;
    let mut temporary = io_at(active, NamedTempFile::new_in(active))?;
    let temporary_path = temporary.path().to_path_buf();
    io_at(&temporary_path, temporary.write_all(&encoded))?;
    io_at(&temporary_path, temporary.as_file().sync_all())?;
    let destination = active.join(MANIFEST_FILE);
    io_at(
        &destination,
        temporary.persist(&destination).map_err(|error| error.error),
    )?;
    sync_directory(active)?;
    Ok(())
}

fn read_manifest(active: &Path) -> Result<TransactionManifest, StateError> {
    let manifest_path = active.join(MANIFEST_FILE);
    let encoded = io_at(&manifest_path, fs::read(&manifest_path))?;
    let manifest: TransactionManifest =
        serde_json::from_slice(&encoded).map_err(|source| StateError::InvalidManifest {
            path: manifest_path,
            source,
        })?;
    if manifest.version != MANIFEST_VERSION {
        return Err(StateError::UnsupportedManifestVersion(manifest.version));
    }
    Ok(manifest)
}

fn collect_mutable_tree_mutations(
    root: &Path,
    current_directory: &Path,
    backup_directory: &Path,
    backup_existed: bool,
    kind: StateRecordKind,
    mutations: &mut BTreeMap<PathBuf, StagedStateMutation>,
) -> Result<(), StateError> {
    let current = collect_tree_contents(current_directory, current_directory)?;
    let backup = if backup_existed {
        collect_tree_contents(backup_directory, backup_directory)?
    } else {
        BTreeMap::new()
    };
    let prefix = current_directory
        .strip_prefix(root)
        .map_err(|_| StateError::UnsafeRelativePath(current_directory.to_path_buf()))?;
    let paths: BTreeSet<_> = current.keys().chain(backup.keys()).cloned().collect();
    for relative in &paths {
        if current.get(relative) == backup.get(relative) {
            continue;
        }
        insert_staged_mutation(
            mutations,
            StagedStateMutation {
                kind,
                relative_path: prefix.join(relative),
                content: current.get(relative).cloned(),
            },
        )?;
    }
    Ok(())
}

fn collect_primary_tree_mutations(
    root: &Path,
    current_directory: &Path,
    baseline: &BTreeMap<PathBuf, Vec<u8>>,
    kind: StateRecordKind,
    mutations: &mut BTreeMap<PathBuf, StagedStateMutation>,
) -> Result<(), StateError> {
    let current = collect_tree_contents(current_directory, root)?;
    let paths: BTreeSet<_> = current.keys().chain(baseline.keys()).cloned().collect();
    for relative_path in paths {
        if current.get(&relative_path) == baseline.get(&relative_path) {
            continue;
        }
        insert_staged_mutation(
            mutations,
            StagedStateMutation {
                kind,
                relative_path: relative_path.clone(),
                content: current.get(&relative_path).cloned(),
            },
        )?;
    }
    Ok(())
}

fn collect_single_file_mutation(
    root: &Path,
    current_path: &Path,
    backup_path: &Path,
    backup_existed: bool,
    kind: StateRecordKind,
    mutations: &mut BTreeMap<PathBuf, StagedStateMutation>,
) -> Result<(), StateError> {
    let current = read_optional_file(current_path)?;
    let backup = if backup_existed {
        Some(io_at(backup_path, fs::read(backup_path))?)
    } else {
        None
    };
    if current == backup {
        return Ok(());
    }
    let relative_path = current_path
        .strip_prefix(root)
        .map_err(|_| StateError::UnsafeRelativePath(current_path.to_path_buf()))?
        .to_path_buf();
    insert_staged_mutation(
        mutations,
        StagedStateMutation {
            kind,
            relative_path,
            content: current,
        },
    )
}

fn collect_tree_contents(
    directory: &Path,
    relative_root: &Path,
) -> Result<BTreeMap<PathBuf, Vec<u8>>, StateError> {
    let mut result = BTreeMap::new();
    if !path_exists(directory)? {
        return Ok(result);
    }
    reject_symlink(directory)?;
    for entry in io_at(directory, fs::read_dir(directory))? {
        let entry = entry.map_err(|source| StateError::Io {
            path: directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let metadata = io_at(&path, fs::symlink_metadata(&path))?;
        if metadata.file_type().is_symlink() {
            return Err(StateError::SymbolicLink(path));
        }
        if metadata.is_dir() {
            result.extend(collect_tree_contents(&path, relative_root)?);
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(relative_root)
                .map_err(|_| StateError::UnsafeRelativePath(path.clone()))?
                .to_path_buf();
            validate_relative(&relative)?;
            result.insert(relative, io_at(&path, fs::read(&path))?);
        }
    }
    Ok(result)
}

fn read_optional_file(path: &Path) -> Result<Option<Vec<u8>>, StateError> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(StateError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn write_staged_file(path: &Path, content: &[u8]) -> Result<(), StateError> {
    let parent = path
        .parent()
        .ok_or_else(|| StateError::UnsafeRelativePath(path.to_path_buf()))?;
    io_at(parent, fs::create_dir_all(parent))?;
    reject_symlink(parent)?;
    let mut temporary = io_at(parent, NamedTempFile::new_in(parent))?;
    let temporary_path = temporary.path().to_path_buf();
    io_at(&temporary_path, temporary.write_all(content))?;
    io_at(&temporary_path, temporary.as_file().sync_all())?;
    io_at(path, temporary.persist(path).map_err(|error| error.error))?;
    sync_directory(parent)
}

fn insert_staged_mutation(
    mutations: &mut BTreeMap<PathBuf, StagedStateMutation>,
    mutation: StagedStateMutation,
) -> Result<(), StateError> {
    if mutations.contains_key(&mutation.relative_path) {
        return Err(StateError::VaultDirectWriteDuplicatePath(
            mutation.relative_path,
        ));
    }
    mutations.insert(mutation.relative_path.clone(), mutation);
    Ok(())
}

fn state_record_kind_for_path(path: &Path) -> StateRecordKind {
    match path.components().next() {
        Some(Component::Normal(first)) if first == RATCHET_DIRECTORY => StateRecordKind::Ratchet,
        Some(Component::Normal(first)) if first == "events" => StateRecordKind::Event,
        Some(Component::Normal(first)) if first == "local-messages" => {
            StateRecordKind::LocalProjection
        }
        Some(Component::Normal(first)) if first == "history-rewraps" => {
            StateRecordKind::HistoryRewrap
        }
        Some(Component::Normal(first)) if first == "history-recovery" => {
            StateRecordKind::HistoryRecovery
        }
        Some(Component::Normal(first))
            if TRUST_FILES.iter().any(|name| first == *name)
                || TRUST_DIRECTORIES.iter().any(|name| first == *name) =>
        {
            StateRecordKind::Trust
        }
        Some(Component::Normal(first)) if first == NEXT_SEQUENCE_FILE => StateRecordKind::Sequence,
        _ => StateRecordKind::Other,
    }
}

fn collect_trust_contents(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>, StateError> {
    let mut records = BTreeMap::new();
    for name in TRUST_FILES {
        let path = root.join(name);
        if let Some(content) = read_optional_file(&path)? {
            reject_symlink(&path)?;
            records.insert(PathBuf::from(name), content);
        }
    }
    for name in TRUST_DIRECTORIES {
        let directory = root.join(name);
        records.extend(collect_tree_contents(&directory, root)?);
    }
    Ok(records)
}

fn clear_trust_contents(root: &Path) -> Result<(), StateError> {
    for name in TRUST_FILES {
        remove_file_if_present(&root.join(name))?;
    }
    for name in TRUST_DIRECTORIES {
        remove_tree_if_present(&root.join(name))?;
    }
    Ok(())
}

fn write_marker(path: &Path) -> Result<(), StateError> {
    let mut file = io_at(
        path,
        OpenOptions::new().create_new(true).write(true).open(path),
    )?;
    io_at(path, file.write_all(b"1\n"))?;
    io_at(path, file.sync_all())?;
    if let Some(parent) = path.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}

fn active_directory(root: &Path) -> PathBuf {
    root.join(TRANSACTION_DIRECTORY).join(ACTIVE_DIRECTORY)
}

fn collect_relative_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), StateError> {
    if !path_exists(directory)? {
        return Ok(());
    }
    reject_symlink(directory)?;
    for entry in io_at(directory, fs::read_dir(directory))? {
        let entry = entry.map_err(|source| StateError::Io {
            path: directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let metadata = io_at(&path, fs::symlink_metadata(&path))?;
        if metadata.file_type().is_symlink() {
            return Err(StateError::SymbolicLink(path));
        }
        if metadata.is_dir() {
            collect_relative_files(root, &path, files)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| StateError::UnsafeRelativePath(path.clone()))?
                .to_path_buf();
            validate_relative(&relative)?;
            files.push(relative);
        }
    }
    Ok(())
}

fn remove_new_files(
    root: &Path,
    directory: &Path,
    baseline: &HashSet<PathBuf>,
) -> Result<(), StateError> {
    if !path_exists(directory)? {
        return Ok(());
    }
    reject_symlink(directory)?;
    let mut child_directories = Vec::new();
    for entry in io_at(directory, fs::read_dir(directory))? {
        let entry = entry.map_err(|source| StateError::Io {
            path: directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let metadata = io_at(&path, fs::symlink_metadata(&path))?;
        if metadata.file_type().is_symlink() {
            return Err(StateError::SymbolicLink(path));
        }
        if metadata.is_dir() {
            child_directories.push(path);
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| StateError::UnsafeRelativePath(path.clone()))?
                .to_path_buf();
            validate_relative(&relative)?;
            if !baseline.contains(&relative) {
                io_at(&path, fs::remove_file(&path))?;
            }
        }
    }
    for child in child_directories {
        remove_new_files(root, &child, baseline)?;
        if io_at(&child, fs::read_dir(&child))?.next().is_none() {
            io_at(&child, fs::remove_dir(&child))?;
        }
    }
    sync_directory(directory)?;
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), StateError> {
    reject_symlink(source)?;
    io_at(destination, fs::create_dir_all(destination))?;
    for entry in io_at(source, fs::read_dir(source))? {
        let entry = entry.map_err(|source_error| StateError::Io {
            path: source.to_path_buf(),
            source: source_error,
        })?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = io_at(&source_path, fs::symlink_metadata(&source_path))?;
        if metadata.file_type().is_symlink() {
            return Err(StateError::SymbolicLink(source_path));
        }
        if metadata.is_dir() {
            copy_tree(&source_path, &destination_path)?;
        } else if metadata.is_file() {
            copy_file(&source_path, &destination_path)?;
        }
    }
    sync_directory(destination)?;
    Ok(())
}

fn copy_file(source: &Path, destination: &Path) -> Result<(), StateError> {
    reject_symlink(source)?;
    if let Some(parent) = destination.parent() {
        io_at(parent, fs::create_dir_all(parent))?;
    }
    io_at(destination, fs::copy(source, destination))?;
    let file = io_at(
        destination,
        OpenOptions::new().read(true).write(true).open(destination),
    )?;
    io_at(destination, file.sync_all())?;
    if let Some(parent) = destination.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}

fn validate_relative(path: &Path) -> Result<(), StateError> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(StateError::UnsafeRelativePath(path.to_path_buf()));
    }
    Ok(())
}

fn reject_symlink(path: &Path) -> Result<(), StateError> {
    let metadata = io_at(path, fs::symlink_metadata(path))?;
    if metadata.file_type().is_symlink() {
        return Err(StateError::SymbolicLink(path.to_path_buf()));
    }
    Ok(())
}

fn reject_relative_symlinks(root: &Path, relative_path: &Path) -> Result<(), StateError> {
    validate_relative(relative_path)?;
    let mut current = root.to_path_buf();
    for component in relative_path.components() {
        let Component::Normal(component) = component else {
            return Err(StateError::UnsafeRelativePath(relative_path.to_path_buf()));
        };
        current.push(component);
        if path_exists(&current)? {
            reject_symlink(&current)?;
        }
    }
    Ok(())
}

fn path_exists(path: &Path) -> Result<bool, StateError> {
    path.try_exists().map_err(|source| StateError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn remove_tree_if_present(path: &Path) -> Result<(), StateError> {
    if !path_exists(path)? {
        return Ok(());
    }
    reject_symlink(path)?;
    io_at(path, fs::remove_dir_all(path))?;
    if let Some(parent) = path.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}

fn remove_file_if_present(path: &Path) -> Result<(), StateError> {
    if path_exists(path)? {
        reject_symlink(path)?;
        io_at(path, fs::remove_file(path))?;
        if let Some(parent) = path.parent() {
            sync_directory(parent)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), StateError> {
    let directory = io_at(path, File::open(path))?;
    io_at(path, directory.sync_all())
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), StateError> {
    Ok(())
}

fn io_at<T>(path: &Path, result: io::Result<T>) -> Result<T, StateError> {
    result.map_err(|source| StateError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    fn write(path: &Path, value: &str) -> Result<(), Box<dyn Error>> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, value)?;
        Ok(())
    }

    #[test]
    fn state_directory_lock_is_exclusive_and_reusable() -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        let first = StateDirectoryLock::acquire(directory.path())?;
        assert!(matches!(
            StateDirectoryLock::acquire(directory.path()),
            Err(StateError::AlreadyLocked { .. })
        ));
        drop(first);
        StateDirectoryLock::acquire(directory.path())?;
        Ok(())
    }

    #[test]
    fn rollback_restores_mutable_state_and_removes_new_append_only_files()
    -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        write(&directory.path().join("ratchet/session.bin"), "old-ratchet")?;
        write(&directory.path().join("next-sequence"), "7")?;
        write(&directory.path().join("events/existing.event"), "old-event")?;

        let mut transaction = StateTransaction::begin(directory.path())?;
        write(&directory.path().join("ratchet/session.bin"), "new-ratchet")?;
        write(&directory.path().join("ratchet/new.bin"), "new")?;
        write(&directory.path().join("next-sequence"), "8")?;
        write(&directory.path().join("events/new.event"), "new-event")?;
        write(
            &directory.path().join("local-messages/new.local-text"),
            "projection",
        )?;
        write(
            &directory.path().join("history-rewraps/unregistered.rewrap"),
            "not-in-write-set",
        )?;
        transaction.register_append_only_receipt_path(directory.path().join("events/new.event"))?;
        transaction.register_append_only_write("local-messages/new.local-text")?;
        let outside = tempfile::tempdir()?;
        write(&outside.path().join("events/outside.event"), "outside")?;
        assert!(matches!(
            transaction
                .register_append_only_receipt_path(outside.path().join("events/outside.event")),
            Err(StateError::UnsafeRelativePath(_))
        ));
        write(
            &directory.path().join("history-recovery/new.checkpoint"),
            "checkpoint",
        )?;
        transaction.rollback()?;

        assert_eq!(
            fs::read_to_string(directory.path().join("ratchet/session.bin"))?,
            "old-ratchet"
        );
        assert!(!directory.path().join("ratchet/new.bin").exists());
        assert_eq!(
            fs::read_to_string(directory.path().join("next-sequence"))?,
            "7"
        );
        assert!(directory.path().join("events/existing.event").exists());
        assert!(!directory.path().join("events/new.event").exists());
        assert!(
            !directory
                .path()
                .join("local-messages/new.local-text")
                .exists()
        );
        assert!(
            !directory
                .path()
                .join("history-recovery/new.checkpoint")
                .exists()
        );
        assert!(
            !directory
                .path()
                .join("history-rewraps/unregistered.rewrap")
                .exists()
        );
        Ok(())
    }

    #[test]
    fn transaction_reports_only_typed_changed_records() -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        write(&directory.path().join("ratchet/changed.bin"), "before")?;
        write(&directory.path().join("ratchet/removed.bin"), "remove")?;
        write(&directory.path().join("next-sequence"), "4\n")?;
        write(&directory.path().join("events/existing.event"), "existing")?;

        let mut transaction = StateTransaction::begin(directory.path())?;
        write(&directory.path().join("ratchet/changed.bin"), "after")?;
        fs::remove_file(directory.path().join("ratchet/removed.bin"))?;
        write(&directory.path().join("ratchet/added.bin"), "added")?;
        write(&directory.path().join("next-sequence"), "5\n")?;
        write(&directory.path().join("events/new.event"), "new")?;
        write(
            &directory.path().join("local-messages/new.local-text"),
            "projection",
        )?;
        write(
            &directory.path().join("history-rewraps/unregistered.rewrap"),
            "not-in-write-set",
        )?;
        transaction.register_append_only_write("events/new.event")?;
        transaction.register_append_only_write("local-messages/new.local-text")?;

        let mutations = transaction.staged_mutations()?;
        let by_path = mutations
            .iter()
            .map(|mutation| (mutation.relative_path.as_path(), mutation))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(by_path.len(), 6);
        assert_eq!(
            by_path[Path::new("ratchet/changed.bin")].content.as_deref(),
            Some(b"after".as_slice())
        );
        assert_eq!(by_path[Path::new("ratchet/removed.bin")].content, None);
        assert_eq!(
            by_path[Path::new("ratchet/added.bin")].kind,
            StateRecordKind::Ratchet
        );
        assert_eq!(
            by_path[Path::new("next-sequence")].kind,
            StateRecordKind::Sequence
        );
        assert_eq!(
            by_path[Path::new("events/new.event")].kind,
            StateRecordKind::Event
        );
        assert_eq!(
            by_path[Path::new("local-messages/new.local-text")].kind,
            StateRecordKind::LocalProjection
        );
        assert!(!by_path.contains_key(Path::new("events/existing.event")));
        assert!(!by_path.contains_key(Path::new("history-rewraps/unregistered.rewrap")));
        assert!(matches!(
            transaction.register_append_only_write("ratchet/not-append-only"),
            Err(StateError::AppendOnlyWriteKindNotAllowed(path))
                if path == Path::new("ratchet/not-append-only")
        ));
        transaction.rollback()?;
        Ok(())
    }

    #[test]
    fn transaction_rejects_append_only_removal_from_direct_delta() -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        let event = directory.path().join("events/existing.event");
        write(&event, "existing")?;
        let transaction = StateTransaction::begin(directory.path())?;
        fs::remove_file(&event)?;
        assert!(matches!(
            transaction.staged_mutations(),
            Err(StateError::AppendOnlyRecordRemoved(path))
                if path == Path::new("events/existing.event")
        ));
        transaction.rollback()?;
        Ok(())
    }

    #[test]
    fn next_lock_recovers_an_interrupted_prepared_transaction() -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        write(&directory.path().join("ratchet/session.bin"), "before")?;
        write(&directory.path().join("next-sequence"), "3")?;
        let transaction = StateTransaction::begin(directory.path())?;
        assert!(matches!(
            StateTransaction::begin(directory.path()),
            Err(StateError::TransactionAlreadyActive { .. })
        ));
        write(&directory.path().join("ratchet/session.bin"), "after")?;
        write(&directory.path().join("next-sequence"), "4")?;
        write(
            &directory.path().join("events/interrupted.event"),
            "partial",
        )?;
        drop(transaction);

        StateDirectoryLock::acquire(directory.path())?;
        assert_eq!(
            fs::read_to_string(directory.path().join("ratchet/session.bin"))?,
            "before"
        );
        assert_eq!(
            fs::read_to_string(directory.path().join("next-sequence"))?,
            "3"
        );
        assert!(!directory.path().join("events/interrupted.event").exists());
        assert!(!active_directory(directory.path()).exists());
        Ok(())
    }

    #[test]
    fn committed_marker_survives_interrupted_cleanup() -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        write(&directory.path().join("next-sequence"), "10")?;
        let transaction = StateTransaction::begin(directory.path())?;
        write(&directory.path().join("next-sequence"), "11")?;
        write(&directory.path().join("events/committed.event"), "event")?;
        write_marker(&transaction.active.join(COMMITTED_MARKER))?;
        drop(transaction);

        StateDirectoryLock::acquire(directory.path())?;
        assert_eq!(
            fs::read_to_string(directory.path().join("next-sequence"))?,
            "11"
        );
        assert!(directory.path().join("events/committed.event").exists());
        Ok(())
    }

    #[test]
    fn recovery_rejects_an_unsafe_manifest_path() -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        write(&directory.path().join("ratchet/session.bin"), "before")?;
        let transaction = StateTransaction::begin(directory.path())?;
        write(&directory.path().join("ratchet/session.bin"), "after")?;
        let manifest_path = transaction.active.join(MANIFEST_FILE);
        let mut manifest: TransactionManifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
        manifest.append_only_files.push(PathBuf::from("../outside"));
        fs::write(&manifest_path, serde_json::to_vec(&manifest)?)?;
        drop(transaction);

        assert!(matches!(
            StateDirectoryLock::acquire(directory.path()),
            Err(StateError::UnsafeRelativePath(path)) if path == Path::new("../outside")
        ));
        assert_eq!(
            fs::read_to_string(directory.path().join("ratchet/session.bin"))?,
            "after"
        );
        Ok(())
    }
}
