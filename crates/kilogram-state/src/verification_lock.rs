//! Windows-only, non-default offline verification support.
//!
//! Inputs originate from our no-create default-stream opener, never arbitrary
//! imported Files. Same-process clones preserve that provenance. This module
//! provides no raw-handle import. Borrowing is only for outbound duplication.
use super::{
    ACTIVE_DIRECTORY, LOCK_FILE, StateDirectoryLock, StateError, TRANSACTION_DIRECTORY, io_at,
};
use std::{
    fs::{self, File, OpenOptions},
    io,
    os::windows::{
        fs::OpenOptionsExt,
        io::{AsHandle, BorrowedHandle},
    },
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use winapi_util::file::{self, Information};

const DIRECTORY: u64 = 0x10;
const REPARSE: u64 = 0x400;
const BACKUP_SEMANTICS: u32 = 0x0200_0000;
const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const SHARE_READ_WRITE: u32 = 3;

/// Actual volume/file identifier captured from an owned handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileIdentity {
    pub volume_serial_number: u64,
    pub file_index: u64,
}
impl FileIdentity {
    pub fn from_file(file: &File) -> io::Result<Self> {
        Ok(identity(&file::information(file)?))
    }
}
fn identity(info: &Information) -> FileIdentity {
    FileIdentity {
        volume_serial_number: info.volume_serial_number(),
        file_index: info.file_index(),
    }
}
fn refuse(reason: &'static str) -> StateError {
    StateError::OrderedState(reason)
}

fn checked(file: &File, path: &Path, directory: bool) -> Result<Information, StateError> {
    let info = io_at(path, file::information(file))?;
    if !io_at(path, file::typ(file))?.is_disk()
        || info.file_attributes() & REPARSE != 0
        || (info.file_attributes() & DIRECTORY != 0) != directory
        || info.file_index() == 0
    {
        return Err(refuse("verification adoption object type/identity"));
    }
    if !directory && (info.number_of_links() != 1 || info.file_size() != 0) {
        return Err(refuse(
            "verification adoption lock must be single-link and empty",
        ));
    }
    Ok(info)
}

// OPEN_EXISTING, no data access; permits metadata alongside share0 and pins
// entries against DELETE. No symlink/reparse traversal at the opened component.
fn guard(path: &Path, directory: bool) -> Result<File, StateError> {
    let file = io_at(
        path,
        OpenOptions::new()
            .access_mode(0)
            .share_mode(SHARE_READ_WRITE)
            .custom_flags(OPEN_REPARSE_POINT | BACKUP_SEMANTICS)
            .open(path),
    )?;
    checked(&file, path, directory)?;
    Ok(file)
}
fn literal(path: &Path) -> Result<PathBuf, StateError> {
    let text = path
        .to_str()
        .ok_or_else(|| refuse("verification adoption requires a literal Unicode drive path"))?;
    let text = text.strip_prefix(r"\\?\").unwrap_or(text);
    let bytes = text.as_bytes();
    if bytes.len() < 4
        || text.encode_utf16().count() > 32760
        || !bytes[0].is_ascii_alphabetic()
        || bytes[1] != b':'
        || !matches!(bytes[2], b'\\' | b'/')
        || text[3..].split(['\\', '/']).any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || part.ends_with(['.', ' '])
                || part.chars().any(|ch| ch == ':' || ch.is_control())
        })
    {
        return Err(refuse("verification adoption requires an exact drive path"));
    }
    Ok(PathBuf::from(text.replace('/', r"\")))
}
fn no_active_journal(root: &Path, guards: &mut Vec<File>) -> Result<(), StateError> {
    let transactions = root.join(TRANSACTION_DIRECTORY);
    match fs::symlink_metadata(&transactions) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        result => {
            io_at(&transactions, result)?;
            guards.push(guard(&transactions, true)?);
        }
    }
    let active = transactions.join(ACTIVE_DIRECTORY);
    match fs::symlink_metadata(&active) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(StateError::Io {
            path: active,
            source,
        }),
        Ok(_) => Err(StateError::TransactionAlreadyActive { path: active }),
    }
}

struct Origin {
    root: PathBuf,
    directory_id: FileIdentity,
    lock_id: FileIdentity,
    directory_guard: usize,
    guards: Vec<File>,
    claimed: AtomicBool,
}

/// Owned inputs created only by the checked default-stream path constructor.
///
/// Fields are private: FileIdentity alone cannot distinguish an ADS from the
/// default stream, so importing an arbitrary File is deliberately impossible.
pub struct ExistingStateLockInputs {
    origin: Arc<Origin>,
    lock: File,
}
impl ExistingStateLockInputs {
    /// Borrow the actual default-stream handle for outbound duplication by an
    /// owned worker. This grants no raw-handle import or new adoption attempt.
    pub fn borrow_lock_handle(&self) -> BorrowedHandle<'_> {
        self.lock.as_handle()
    }

    /// Open only existing objects: read-only/share0 fixed LOCK_FILE, retained
    /// no-follow ancestry/default-stream metadata guards, no create or recovery.
    pub fn open_existing(
        path: &Path,
        directory_id: FileIdentity,
        lock_id: FileIdentity,
    ) -> Result<Self, StateError> {
        Self::open_default_stream(path, directory_id, lock_id, 0)
    }

    // Production has exactly one caller, above, with share0. Compatible sharing
    // in unit tests permits independent measurement of byte-range lifetime,
    // separately from production share0 exclusion. This is not a public mode.
    fn open_default_stream(
        path: &Path,
        directory_id: FileIdentity,
        lock_id: FileIdentity,
        sharing: u32,
    ) -> Result<Self, StateError> {
        let requested = literal(path)?;
        let ancestors: Vec<_> = requested.ancestors().take(65).collect();
        if ancestors.len() > 64 {
            return Err(refuse("verification adoption ancestry cap"));
        }
        let mut guards = Vec::with_capacity(ancestors.len() + 2);
        for ancestor in ancestors.into_iter().rev() {
            guards.push(guard(ancestor, true)?);
        }
        let directory_guard = guards.len() - 1;
        if identity(&checked(&guards[directory_guard], &requested, true)?) != directory_id {
            return Err(refuse("verification adoption path identity mismatch"));
        }
        let root = io_at(&requested, fs::canonicalize(&requested))?;
        let expected_canonical = format!(r"\\?\{}", requested.display());
        if root != Path::new(&expected_canonical) {
            return Err(refuse("verification adoption canonical path mismatch"));
        }
        let lock_path = root.join(LOCK_FILE);
        let path_guard = guard(&lock_path, false)?;
        if identity(&checked(&path_guard, &lock_path, false)?) != lock_id {
            return Err(refuse("verification adoption path identity mismatch"));
        }
        guards.push(path_guard);
        no_active_journal(&root, &mut guards)?;
        // The path has no stream spelling and all its ancestors/entry are held.
        // Unlike volume/file ID comparison, this open establishes stream origin.
        let lock = io_at(
            &lock_path,
            OpenOptions::new()
                .read(true)
                .share_mode(sharing)
                .custom_flags(OPEN_REPARSE_POINT)
                .open(&lock_path),
        )?;
        if identity(&checked(&lock, &lock_path, false)?) != lock_id {
            return Err(refuse("verification adoption opened identity mismatch"));
        }
        Ok(Self {
            origin: Arc::new(Origin {
                root,
                directory_id,
                lock_id,
                directory_guard,
                guards,
                claimed: AtomicBool::new(false),
            }),
            lock,
        })
    }

    /// Duplicate only this lineage's actual default-stream File. The same
    /// one-shot claim is shared; cloning never grants another adoption attempt.
    pub fn try_clone(&self) -> Result<Self, StateError> {
        Ok(Self {
            origin: Arc::clone(&self.origin),
            lock: io_at(&self.origin.root.join(LOCK_FILE), self.lock.try_clone())?,
        })
    }
}

// Boxed to avoid inflating existing owner/error values in feature-enabled builds.
pub(super) struct Adoption {
    origin: Arc<Origin>,
    journal_guards: Vec<File>,
    locked: bool,
}
impl StateDirectoryLock {
    /// Fallible cleanup proof while producer duplicates survive. Drop is only
    /// best effort; a reported unlock error must remain a cleanup failure.
    pub fn release_adopted(mut self) -> Result<(), StateError> {
        let adoption = self
            ._verification_adoption
            .as_mut()
            .ok_or_else(|| refuse("verification release requires an adopted lease"))?;
        if !adoption.locked {
            return Err(refuse("verification release requires an adopted lease"));
        }
        io_at(&self.root.join(LOCK_FILE), self._file.unlock())?;
        adoption.locked = false;
        Ok(())
    }

    /// Consume typed existing inputs, never arbitrary imported Files.
    ///
    /// A lineage allows ONE attempt, including refusals. Repeated/concurrent
    /// clones fail before OS try_lock, avoiding its unspecified already-locked
    /// duplicate semantics. A refused lineage is not reusable; release all its
    /// clones and open fresh inputs for a separately justified new attempt.
    pub fn adopt_existing_no_recovery(inputs: ExistingStateLockInputs) -> Result<Self, StateError> {
        inputs
            .origin
            .claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| refuse("verification adoption lineage already spent"))?;
        let ExistingStateLockInputs { origin, lock } = inputs;
        let root = origin.root.clone();
        let lock_path = root.join(LOCK_FILE);
        if identity(&checked(
            &origin.guards[origin.directory_guard],
            &root,
            true,
        )?) != origin.directory_id
            || identity(&checked(&lock, &lock_path, false)?) != origin.lock_id
        {
            return Err(refuse("verification adoption expected identity mismatch"));
        }
        // All allocations needed for the RAII owner precede try_lock.
        let mut lease = Self {
            root,
            _file: lock,
            _verification_adoption: Some(Box::new(Adoption {
                origin,
                journal_guards: Vec::with_capacity(2),
                locked: false,
            })),
        };
        let adoption = lease
            ._verification_adoption
            .as_mut()
            .ok_or_else(|| refuse("verification adoption state missing"))?;
        no_active_journal(&lease.root, &mut adoption.journal_guards)?;
        if let Err(source) = lease._file.try_lock() {
            return match source {
                fs::TryLockError::WouldBlock => Err(StateError::AlreadyLocked {
                    path: lease.root.clone(),
                }),
                fs::TryLockError::Error(source) => Err(StateError::Io {
                    path: lock_path,
                    source,
                }),
            };
        }
        adoption.locked = true;
        let validation = (|| {
            #[cfg(test)]
            POST_LOCK_HOOK.with(|slot| match slot.borrow_mut().take() {
                Some(hook) => hook(&lease.root),
                None => Ok(()),
            })?;
            let adoption = lease
                ._verification_adoption
                .as_mut()
                .ok_or_else(|| refuse("verification adoption state missing"))?;
            no_active_journal(&lease.root, &mut adoption.journal_guards)?;
            if identity(&checked(&lease._file, &lock_path, false)?) != adoption.origin.lock_id {
                return Err(refuse("verification adoption locked identity mismatch"));
            }
            for held in adoption
                .origin
                .guards
                .iter()
                .chain(&adoption.journal_guards)
            {
                if io_at(&lease.root, file::information(held))?.file_attributes() & REPARSE != 0 {
                    return Err(refuse("verification adoption reparse drift"));
                }
            }
            Ok(())
        })();
        if let Err(error) = validation {
            lease.release_adopted()?;
            return Err(error);
        }
        Ok(lease)
    }
}
impl Drop for StateDirectoryLock {
    fn drop(&mut self) {
        if self
            ._verification_adoption
            .as_ref()
            .is_some_and(|adoption| adoption.locked)
        {
            let _ = self._file.unlock();
        }
    }
}
#[cfg(test)]
type PostLockHook = Box<dyn FnOnce(&Path) -> Result<(), StateError>>;
#[cfg(test)]
thread_local! {
    static POST_LOCK_HOOK: std::cell::RefCell<Option<PostLockHook>> = const { std::cell::RefCell::new(None) };
}
#[cfg(test)]
#[path = "verification_lock_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "verification_worker_tests.rs"]
mod worker_tests;
