//! Guarded access to a Setup selected by the terminal user.
use super::{File, OpenOptions, Path, Setup, fs};
use std::io;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetupAccessError {
    Exists,
    Missing,
    NotEmpty,
    Locked,
    Invalid,
}

type AccessResult<T> = Result<T, SetupAccessError>;

fn metadata(path: &Path) -> AccessResult<Option<fs::Metadata>> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(SetupAccessError::Invalid),
    };
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(SetupAccessError::Invalid);
        }
    }
    if meta.is_symlink() {
        return Err(SetupAccessError::Invalid);
    }
    Ok(Some(meta))
}

fn directory(home: &Path) -> AccessResult<bool> {
    for path in home.ancestors().filter(|path| !path.as_os_str().is_empty()) {
        if metadata(path)?.is_some_and(|meta| !meta.is_dir()) {
            return Err(SetupAccessError::Invalid);
        }
    }
    Ok(metadata(home)?.is_some())
}

fn check_new(home: &Path) -> AccessResult<()> {
    if !directory(home)? {
        return Ok(());
    }
    if metadata(&home.join("setup.json"))?.is_some() {
        Setup::read_record(home).map_err(|_| SetupAccessError::Invalid)?;
        return Err(SetupAccessError::Exists);
    }
    for entry in fs::read_dir(home).map_err(|_| SetupAccessError::Invalid)? {
        let entry = entry.map_err(|_| SetupAccessError::Invalid)?;
        if entry.file_name() != ".onboarding.lock" {
            return Err(SetupAccessError::NotEmpty);
        }
        if !metadata(&entry.path())?.is_some_and(|meta| meta.is_file()) {
            return Err(SetupAccessError::Invalid);
        }
    }
    Ok(())
}

fn lock(home: &Path) -> AccessResult<File> {
    let path = home.join(".onboarding.lock");
    if metadata(&path)?.is_some_and(|meta| !meta.is_file()) {
        return Err(SetupAccessError::Invalid);
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
        options.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT
        let file = options.open(&path).map_err(|_| SetupAccessError::Invalid)?;
        let meta = file.metadata().map_err(|_| SetupAccessError::Invalid)?;
        if !meta.is_file() || meta.file_attributes() & 0x400 != 0 {
            return Err(SetupAccessError::Invalid);
        }
        file.try_lock().map_err(|_| SetupAccessError::Locked)?;
        Ok(file)
    }
    #[cfg(not(windows))]
    {
        let file = options.open(&path).map_err(|_| SetupAccessError::Invalid)?;
        file.try_lock().map_err(|_| SetupAccessError::Locked)?;
        Ok(file)
    }
}

impl Setup {
    pub fn begin_terminal(home: &Path) -> AccessResult<Self> {
        directory(home)?;
        if !home.exists() {
            fs::create_dir_all(home).map_err(|_| SetupAccessError::Invalid)?;
        }
        directory(home)?;
        let home = fs::canonicalize(home).map_err(|_| SetupAccessError::Invalid)?;
        check_new(&home)?;
        let lock = lock(&home)?;
        check_new(&home)?;
        Self::begin_locked(home, lock, None).map_err(|_| SetupAccessError::Invalid)
    }

    pub fn open_terminal(home: &Path) -> AccessResult<Self> {
        if !directory(home)? || metadata(&home.join("setup.json"))?.is_none() {
            return Err(SetupAccessError::Missing);
        }
        let home = fs::canonicalize(home).map_err(|_| SetupAccessError::Invalid)?;
        Self::read_record(&home).map_err(|_| SetupAccessError::Invalid)?;
        let lock = lock(&home)?;
        if !metadata(&home.join("setup.json"))?.is_some_and(|meta| meta.is_file()) {
            return Err(SetupAccessError::Invalid);
        }
        Self::open_locked(home, lock, true).map_err(|_| SetupAccessError::Invalid)
    }

    pub fn refresh_terminal(&mut self) -> AccessResult<()> {
        if !directory(&self.home)?
            || !metadata(&self.home.join("setup.json"))?.is_some_and(|meta| meta.is_file())
            || Self::read_record(&self.home).map_err(|_| SetupAccessError::Invalid)? != self.record
        {
            return Err(SetupAccessError::Invalid);
        }
        self.inspect_ready(true)
            .map_err(|_| SetupAccessError::Invalid)
    }

    pub fn workspace_reserved(&self) -> AccessResult<bool> {
        let path = &self.record.workspace;
        let Some(meta) = metadata(path)? else {
            return Ok(false);
        };
        if !meta.is_dir() {
            return Err(SetupAccessError::Invalid);
        }
        for name in ["public", "account-root", "device"] {
            if metadata(&path.join(name))?.is_some_and(|meta| !meta.is_dir()) {
                return Err(SetupAccessError::Invalid);
            }
        }
        for name in [
            "bootstrap-ready.json",
            "bootstrap-candidate.json",
            "bootstrap-pending.json",
            "public/credential.bin",
            "public/device-certificate.cert",
            "public/authority.snapshot",
            "public/device-list.snapshot",
        ] {
            if metadata(&path.join(name))?.is_some_and(|meta| !meta.is_file()) {
                return Err(SetupAccessError::Invalid);
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t12a_terminal_setup_guards_lock_and_foreign_directories() {
        let folder = tempfile::tempdir().unwrap();
        let home = folder.path().join("setup");
        assert!(matches!(
            Setup::open_terminal(&home),
            Err(SetupAccessError::Missing)
        ));
        assert!(!home.exists());
        let foreign = folder.path().join("foreign");
        fs::create_dir(&foreign).unwrap();
        fs::write(foreign.join("sentinel"), b"preserve").unwrap();
        assert!(matches!(
            Setup::begin_terminal(&foreign),
            Err(SetupAccessError::NotEmpty)
        ));
        assert!(!foreign.join(".onboarding.lock").exists());
        assert_eq!(fs::read(foreign.join("sentinel")).unwrap(), b"preserve");
        let setup = Setup::begin_terminal(&home).unwrap();
        assert!(matches!(
            Setup::begin_terminal(&home),
            Err(SetupAccessError::Exists)
        ));
        assert!(matches!(
            Setup::open_terminal(&home),
            Err(SetupAccessError::Locked)
        ));
        let id = setup.record.local_id.clone();
        let contexts = setup.record.contexts.clone();
        let workspace = setup.record.workspace.clone();
        assert!(!setup.workspace_reserved().unwrap());
        drop(setup);
        let reopened = Setup::open_terminal(&home).unwrap();
        assert_eq!(reopened.record.local_id, id);
        assert_eq!(reopened.record.contexts, contexts);
        drop(reopened);
        fs::create_dir(&workspace).unwrap();
        fs::write(workspace.join("bootstrap-ready.json"), b"invalid").unwrap();
        assert!(matches!(
            Setup::open_terminal(&home),
            Err(SetupAccessError::Invalid)
        ));
        assert_eq!(
            fs::read(workspace.join("bootstrap-ready.json")).unwrap(),
            b"invalid"
        );
    }
}
