use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use thiserror::Error;

mod vault;

pub use vault::{
    EncryptedStateVault, STATE_VAULT_FILE, STATE_VAULT_KEY_FILE, StateMirrorRepository,
    StateRecordKind, TypedShadowReadReport, TypedStateRepository, VaultMigrationOutcome,
    VaultMirrorCommit, VaultMirrorDelta, VaultMirrorOutcome, VaultPrimaryRead, VaultPrimaryRecord,
    VaultPrimaryWriteRepository, VaultReport,
};

const LOCK_FILE: &str = ".kilogram-state.lock";
const TRANSACTION_DIRECTORY: &str = ".kilogram-transactions";
const ACTIVE_DIRECTORY: &str = "active";
const BACKUP_DIRECTORY: &str = "backup";
const MANIFEST_FILE: &str = "manifest.json";
const PREPARED_MARKER: &str = "prepared";
const COMMITTED_MARKER: &str = "committed";
const ROLLED_BACK_MARKER: &str = "rolled-back";
const RATCHET_DIRECTORY: &str = "ratchet";
const NEXT_SEQUENCE_FILE: &str = "next-sequence";
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

    #[error("state vault at {0} does not contain a committed migration")]
    VaultNotMigrated(PathBuf),

    #[error("unsupported state vault schema version {0}")]
    UnsupportedVaultSchemaVersion(u64),

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

        Ok(Self { root, active })
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
}

#[derive(Debug, Deserialize, Serialize)]
struct TransactionManifest {
    version: u8,
    ratchet_existed: bool,
    next_sequence_existed: bool,
    append_only_files: Vec<PathBuf>,
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

    let ratchet_backup = active.join(BACKUP_DIRECTORY).join(RATCHET_DIRECTORY);
    if manifest.ratchet_existed {
        if !path_exists(&ratchet_backup)? {
            return Err(StateError::InvalidBackup(ratchet_backup));
        }
        let mut backup_files = Vec::new();
        collect_relative_files(active, &ratchet_backup, &mut backup_files)?;
    }
    let next_sequence_backup = active.join(BACKUP_DIRECTORY).join(NEXT_SEQUENCE_FILE);
    if manifest.next_sequence_existed {
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
    if manifest.ratchet_existed {
        copy_tree(&ratchet_backup, &ratchet)?;
    }

    let next_sequence = root.join(NEXT_SEQUENCE_FILE);
    remove_file_if_present(&next_sequence)?;
    if manifest.next_sequence_existed {
        copy_file(&next_sequence_backup, &next_sequence)?;
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

        let transaction = StateTransaction::begin(directory.path())?;
        write(&directory.path().join("ratchet/session.bin"), "new-ratchet")?;
        write(&directory.path().join("ratchet/new.bin"), "new")?;
        write(&directory.path().join("next-sequence"), "8")?;
        write(&directory.path().join("events/new.event"), "new-event")?;
        write(
            &directory.path().join("local-messages/new.local-text"),
            "projection",
        )?;
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
