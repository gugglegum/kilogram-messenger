use super::*;
use std::{
    collections::BTreeMap, error::Error, io::Write, os::windows::fs::MetadataExt, sync::Barrier,
    time::SystemTime,
};
type TestResult = Result<(), Box<dyn Error>>;
type Tree = BTreeMap<PathBuf, (u32, u64, Option<SystemTime>, Vec<u8>)>;

fn fixture() -> Result<PathBuf, Box<dyn Error>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/C-s53-cold-b0a-synthetic");
    fs::create_dir_all(&base)?;
    let base = fs::canonicalize(base)?;
    let run = tempfile::Builder::new()
        .prefix("b0a-")
        .tempdir_in(base)?
        .keep();
    let root = run.join("state");
    fs::create_dir(&root)?;
    fs::write(root.join(LOCK_FILE), [])?;
    Ok(root)
}
fn ids(root: &Path) -> Result<(FileIdentity, FileIdentity), Box<dyn Error>> {
    fn id(path: &Path) -> io::Result<FileIdentity> {
        let file = OpenOptions::new()
            .access_mode(0)
            .share_mode(7)
            .custom_flags(BACKUP_SEMANTICS | OPEN_REPARSE_POINT)
            .open(path)?;
        FileIdentity::from_file(&file)
    }
    Ok((id(root)?, id(&root.join(LOCK_FILE))?))
}
fn inputs(root: &Path) -> Result<ExistingStateLockInputs, Box<dyn Error>> {
    let (dir, lock) = ids(root)?;
    Ok(ExistingStateLockInputs::open_existing(root, dir, lock)?)
}
fn read_lock(root: &Path, sharing: u32) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .share_mode(sharing)
        .custom_flags(OPEN_REPARSE_POINT)
        .open(root.join(LOCK_FILE))
}
fn refused<T>(result: Result<T, StateError>) -> Result<StateError, Box<dyn Error>> {
    match result {
        Err(error) => Ok(error),
        Ok(_) => Err("unexpected acceptance".into()),
    }
}
fn tree(root: &Path) -> Result<Tree, Box<dyn Error>> {
    fn visit(root: &Path, path: &Path, out: &mut Tree) -> TestResult {
        let meta = fs::symlink_metadata(path)?;
        let attrs = meta.file_attributes();
        let bytes = if attrs as u64 & REPARSE != 0 {
            fs::read_link(path)?.as_os_str().as_encoded_bytes().to_vec()
        } else if meta.is_file() && meta.len() != 0 {
            fs::read(path)?
        } else {
            Vec::new()
        };
        out.insert(
            path.strip_prefix(root)?.to_owned(),
            (attrs, meta.len(), meta.modified().ok(), bytes),
        );
        if meta.is_dir() && attrs as u64 & REPARSE == 0 {
            for entry in fs::read_dir(path)? {
                visit(root, &entry?.path(), out)?;
            }
        }
        Ok(())
    }
    let mut out = Tree::new();
    visit(root, root, &mut out)?;
    Ok(out)
}

#[test]
fn readonly_share0_duplicates_hold_exclusion_and_directory() -> TestResult {
    let root = fixture()?;
    let original = inputs(&root)?;
    let survivor = original.try_clone()?;
    let mut rights_probe = original.lock.try_clone()?;
    let error = match rights_probe.write_all(b"x") {
        Err(error) => error,
        Ok(()) => return Err("read-only duplicate allowed write".into()),
    };
    assert_eq!(error.raw_os_error(), Some(5));
    let lease = StateDirectoryLock::adopt_existing_no_recovery(original.try_clone()?)?;
    assert_eq!(lease.root(), fs::canonicalize(&root)?);
    let extra = lease._file.try_clone()?;
    drop(original);
    drop(survivor);
    drop(rights_probe);
    drop(extra);
    let error = match read_lock(&root, 7) {
        Err(error) => error,
        Ok(_) => return Err("share0 exclusion lost".into()),
    };
    assert_eq!(error.raw_os_error(), Some(32));
    let renamed = root.with_file_name("renamed");
    assert!(fs::rename(&root, &renamed).is_err());
    lease.release_adopted()?;
    drop(read_lock(&root, 7)?);
    fs::rename(&root, &renamed)?;
    assert!(fs::read(renamed.join(LOCK_FILE))?.is_empty());
    Ok(())
}

#[test]
fn separate_owner_is_rejected_before_adoption() -> TestResult {
    let root = fixture()?;
    let (dir_id, lock_id) = ids(&root)?;
    let owner = OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(7)
        .open(root.join(LOCK_FILE))?;
    owner.try_lock()?;
    let error = refused(ExistingStateLockInputs::open_existing(
        &root, dir_id, lock_id,
    ))?;
    assert!(
        matches!(error, StateError::Io { ref source, .. } if source.raw_os_error() == Some(32)),
        "{error}"
    );
    owner.unlock()?;
    drop(owner);
    StateDirectoryLock::adopt_existing_no_recovery(inputs(&root)?)?.release_adopted()?;
    Ok(())
}

#[test]
fn unlocks_on_drop_explicit_release_and_post_lock_refusal() -> TestResult {
    for outcome in ["drop", "release", "post-lock-refusal"] {
        let root = fixture()?;
        let (dir_id, lock_id) = ids(&root)?;
        // Private test-only compatible sharing makes range lifetime independently
        // observable. Same default-stream opener; public constructor is share0.
        let original = ExistingStateLockInputs::open_default_stream(&root, dir_id, lock_id, 7)?;
        let survivor = original.try_clone()?;
        if outcome == "post-lock-refusal" {
            POST_LOCK_HOOK.with(|slot| {
                *slot.borrow_mut() = Some(Box::new(|path| {
                    let active = path.join(TRANSACTION_DIRECTORY).join(ACTIVE_DIRECTORY);
                    io_at(&active, fs::create_dir_all(&active))
                }));
            });
            let error = refused(StateDirectoryLock::adopt_existing_no_recovery(
                original.try_clone()?,
            ))?;
            assert!(
                matches!(error, StateError::TransactionAlreadyActive { .. }),
                "{error}"
            );
            assert!(
                root.join(TRANSACTION_DIRECTORY)
                    .join(ACTIVE_DIRECTORY)
                    .is_dir()
            );
        } else {
            let lease = StateDirectoryLock::adopt_existing_no_recovery(original.try_clone()?)?;
            // A surviving original must not leak OR lose range ownership when
            // an unrelated duplicate from the same lineage is closed.
            drop(survivor.try_clone()?);
            let competitor = read_lock(&root, 7)?;
            assert!(matches!(
                competitor.try_lock(),
                Err(fs::TryLockError::WouldBlock)
            ));
            if outcome == "release" {
                lease.release_adopted()?;
            } else {
                drop(lease);
            }
        }
        let error = refused(StateDirectoryLock::adopt_existing_no_recovery(
            survivor.try_clone()?,
        ))?;
        assert!(matches!(
            error,
            StateError::OrderedState("verification adoption lineage already spent")
        ));
        let competitor = read_lock(&root, 7)?;
        competitor.try_lock()?;
        competitor.unlock()?;
        assert_eq!(original.lock.metadata()?.len(), 0);
        assert_eq!(survivor.lock.metadata()?.len(), 0);
    }
    Ok(())
}

#[test]
fn concurrent_clones_have_one_adoption_attempt() -> TestResult {
    let root = fixture()?;
    let original = inputs(&root)?;
    let gate = Arc::new(Barrier::new(2));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let candidate = original.try_clone()?;
        let gate = Arc::clone(&gate);
        workers.push(std::thread::spawn(move || -> Result<bool, StateError> {
            gate.wait();
            match StateDirectoryLock::adopt_existing_no_recovery(candidate) {
                Ok(lease) => {
                    lease.release_adopted()?;
                    Ok(true)
                }
                Err(StateError::OrderedState("verification adoption lineage already spent")) => {
                    Ok(false)
                }
                Err(error) => Err(error),
            }
        }));
    }
    let mut wins = 0;
    for worker in workers {
        wins += usize::from(worker.join().map_err(|_| "adoption thread panicked")??);
    }
    assert_eq!(wins, 1);
    let error = refused(StateDirectoryLock::adopt_existing_no_recovery(original))?;
    assert!(matches!(
        error,
        StateError::OrderedState("verification adoption lineage already spent")
    ));
    Ok(())
}

#[test]
fn wrong_identity_path_type_and_nonempty_refuse_without_writes() -> TestResult {
    for case in [
        "identity",
        "path",
        "directory-type",
        "lock-type",
        "nonempty",
        "hardlink",
        "relative",
        "traversal",
        "ads-spelling",
    ] {
        let root = fixture()?;
        if case == "directory-type" {
            fs::write(root.join("not-a-directory"), [])?;
        }
        if case == "lock-type" {
            fs::remove_file(root.join(LOCK_FILE))?;
            fs::create_dir(root.join(LOCK_FILE))?;
        }
        if case == "nonempty" {
            fs::write(root.join(LOCK_FILE), b"retained")?;
        }
        if case == "hardlink" {
            fs::hard_link(root.join(LOCK_FILE), root.join("alias"))?;
        }
        let other = fixture()?;
        let before = tree(&root)?;
        let other_before = tree(&other)?;
        let (mut dir_id, lock_id) = ids(&root)?;
        if case == "identity" {
            dir_id.file_index ^= 1;
        }
        let mut traversal = root.as_os_str().to_os_string();
        traversal.push(r"\..\state");
        let mut ads = root.as_os_str().to_os_string();
        ads.push(":adoption-probe");
        let requested = match case {
            "path" => other.clone(),
            "directory-type" => root.join("not-a-directory"),
            "relative" => PathBuf::from("state"),
            "traversal" => PathBuf::from(traversal),
            "ads-spelling" => PathBuf::from(ads),
            _ => root.clone(),
        };
        let error = refused(ExistingStateLockInputs::open_existing(
            &requested, dir_id, lock_id,
        ))?;
        let reason = match case {
            "identity" | "path" => "verification adoption path identity mismatch",
            "directory-type" | "lock-type" => "verification adoption object type/identity",
            "nonempty" | "hardlink" => "verification adoption lock must be single-link and empty",
            "relative" | "traversal" | "ads-spelling" => {
                "verification adoption requires an exact drive path"
            }
            _ => return Err("unknown fixture".into()),
        };
        assert!(
            matches!(error, StateError::OrderedState(actual) if actual == reason),
            "{case}: {error}"
        );
        assert_eq!(tree(&root)?, before, "{case}");
        assert_eq!(tree(&other)?, other_before, "{case}");
    }
    Ok(())
}

#[test]
fn missing_existing_paths_are_not_created() -> TestResult {
    for missing in ["lock", "directory"] {
        let root = fixture()?;
        let (dir_id, lock_id) = ids(&root)?;
        let requested = if missing == "lock" {
            fs::rename(root.join(LOCK_FILE), root.join("moved.lock"))?;
            root.clone()
        } else {
            root.with_file_name("missing-root")
        };
        let parent = root.parent().ok_or("fixture parent")?;
        let before = tree(parent)?;
        let error = refused(ExistingStateLockInputs::open_existing(
            &requested, dir_id, lock_id,
        ))?;
        assert!(
            matches!(error, StateError::Io { ref source, .. } if source.kind() == io::ErrorKind::NotFound),
            "{error}"
        );
        assert_eq!(tree(parent)?, before);
    }
    Ok(())
}

#[test]
fn active_journals_are_preserved_without_recovery() -> TestResult {
    for marker in ["unprepared", "prepared", "committed", "rolled-back"] {
        let root = fixture()?;
        let (dir_id, lock_id) = ids(&root)?;
        let active = root.join(TRANSACTION_DIRECTORY).join(ACTIVE_DIRECTORY);
        fs::create_dir_all(&active)?;
        fs::write(
            active.join("manifest.json"),
            b"invalid-manifest-must-not-be-read",
        )?;
        if marker != "unprepared" {
            fs::write(active.join(marker), [])?;
        }
        fs::write(root.join("payload"), b"original")?;
        let before = tree(&root)?;
        let error = refused(ExistingStateLockInputs::open_existing(
            &root, dir_id, lock_id,
        ))?;
        assert!(
            matches!(error, StateError::TransactionAlreadyActive { .. }),
            "{error}"
        );
        assert_eq!(tree(&root)?, before);
        drop(read_lock(&root, 0)?);
    }
    Ok(())
}

#[test]
fn directory_junction_is_rejected_without_following() -> TestResult {
    let root = fixture()?;
    let target = fixture()?;
    let (dir_id, lock_id) = ids(&target)?;
    let link = root.join("junction");
    let status = std::process::Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(literal(&link)?)
        .arg(literal(&target)?)
        .status()?;
    assert!(status.success(), "junction fixture unavailable");
    let before = tree(&root)?;
    let error = refused(ExistingStateLockInputs::open_existing(
        &link, dir_id, lock_id,
    ))?;
    assert!(
        matches!(
            error,
            StateError::OrderedState("verification adoption object type/identity")
        ),
        "{error}"
    );
    assert_eq!(tree(&root)?, before);
    Ok(())
}

#[test]
fn default_stream_provenance_with_existing_ads() -> TestResult {
    let root = fixture()?;
    let mut stream = root.join(LOCK_FILE).as_os_str().to_os_string();
    stream.push(":adoption-probe");
    fs::write(Path::new(&stream), [])?;
    let original = inputs(&root)?;
    let ads = OpenOptions::new()
        .read(true)
        .share_mode(7)
        .custom_flags(OPEN_REPARSE_POINT)
        .open(Path::new(&stream))?;
    assert_eq!(
        FileIdentity::from_file(&ads)?,
        FileIdentity::from_file(&original.lock)?
    );
    let lease = StateDirectoryLock::adopt_existing_no_recovery(original.try_clone()?)?;
    // ADS remains independently lockable: adoption targeted our default stream.
    ads.try_lock()?;
    ads.unlock()?;
    let error = match read_lock(&root, 7) {
        Err(error) => error,
        Ok(_) => return Err("default stream not share0-excluded".into()),
    };
    assert_eq!(error.raw_os_error(), Some(32));
    let metadata = guard(&root.join(LOCK_FILE), false)?;
    assert_eq!(FileIdentity::from_file(&metadata)?, original.origin.lock_id);
    drop(metadata);
    lease.release_adopted()?;
    drop(original);
    drop(ads);
    let default = read_lock(&root, 7)?;
    default.try_lock()?;
    default.unlock()?;
    Ok(())
}

#[test]
fn ordinary_acquire_still_creates_and_recovers() -> TestResult {
    let root = fixture()?;
    let active = root.join(TRANSACTION_DIRECTORY).join(ACTIVE_DIRECTORY);
    fs::create_dir_all(&active)?;
    fs::write(active.join("unprepared-payload"), b"old default recovery")?;
    let lease = StateDirectoryLock::acquire(&root)?;
    assert!(!active.exists());
    drop(lease);
    let fresh = root.with_file_name("default-created");
    let lease = StateDirectoryLock::acquire(&fresh)?;
    assert!(fresh.join(LOCK_FILE).is_file());
    drop(lease);
    Ok(())
}
