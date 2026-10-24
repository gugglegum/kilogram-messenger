use super::{Config, Error, HEADROOM, Result};
use redb::StorageBackend;
use serde::Serialize;
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Clone, Debug, Default, Serialize)]
pub struct BackendStats {
    pub physical_quota: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub reads: u64,
    pub writes: u64,
    pub set_len_calls: u64,
    pub growth_bytes: u64,
    pub sync_calls: u64,
    pub growth_nanos: u64,
    pub sync_nanos: u64,
    pub max_growth: u64,
    pub length: u64,
    pub last_requested_length: u64,
    pub last_failure: Option<String>,
}
#[derive(Debug, Default)]
struct Control {
    stats: BackendStats,
    write_epoch: u64,
    grown_since_sync: bool,
    #[cfg(test)]
    fault: Option<(&'static str, u64)>,
}
#[derive(Clone, Debug, Default)]
pub(super) struct BackendControl(Arc<Mutex<Control>>);
impl BackendControl {
    pub fn stats(&self) -> Result<BackendStats> {
        Ok(self
            .0
            .lock()
            .map_err(|_| Error::Integrity("backend lock"))?
            .stats
            .clone())
    }
    pub fn begin(&self) -> Result<()> {
        let mut c = self
            .0
            .lock()
            .map_err(|_| Error::Integrity("backend lock"))?;
        c.write_epoch = 0;
        c.stats.last_failure = None;
        Ok(())
    }
    #[cfg(test)]
    pub fn fail(&self, at: &'static str, n: u64) -> Result<()> {
        self.0
            .lock()
            .map_err(|_| Error::Integrity("backend lock"))?
            .fault = Some((at, n));
        Ok(())
    }
    fn fault(&self, point: &str) -> io::Result<()> {
        #[cfg(test)]
        {
            let mut c = self.lock()?;
            if let Some((p, n)) = c.fault.as_mut()
                && (*p == point || *p == "storage_full" && point == "before_write")
            {
                if *n == 0 {
                    let full = *p == "storage_full";
                    c.fault = None;
                    return Err(io::Error::new(
                        if full {
                            io::ErrorKind::StorageFull
                        } else {
                            io::ErrorKind::Other
                        },
                        "injected indexed I/O",
                    ));
                }
                *n -= 1;
            }
        }
        let _ = point;
        Ok(())
    }
    fn lock(&self) -> io::Result<std::sync::MutexGuard<'_, Control>> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("indexed backend poisoned"))
    }
}

#[derive(Debug)]
pub(super) struct Backend {
    file: Mutex<File>,
    root: PathBuf,
    config: Config,
    control: BackendControl,
}
impl Backend {
    pub fn open(path: &Path, create: bool, config: Config) -> Result<(Self, BackendControl)> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(create)
            .open(path)?;
        file.try_lock()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        if !create && file.metadata()?.len() == 0 {
            return Err(Error::Integrity("empty existing database"));
        }
        let root = path
            .parent()
            .ok_or(Error::Bound("database parent"))?
            .to_path_buf();
        let control = BackendControl::default();
        let requested = file.metadata()?.len();
        if requested
            .checked_add(config.retained_external_bytes)
            .and_then(|v| v.checked_add(HEADROOM))
            .is_none_or(|v| v > config.physical_quota)
        {
            return Err(Error::ConfiguredCapacity {
                requested,
                quota: config.physical_quota,
            });
        }
        control
            .0
            .lock()
            .map_err(|_| Error::Integrity("backend lock"))?
            .stats
            .length = file.metadata()?.len();
        control.lock()?.stats.physical_quota = config.physical_quota;
        Ok((
            Self {
                file: Mutex::new(file),
                root,
                config,
                control: control.clone(),
            },
            control,
        ))
    }
    fn file(&self) -> io::Result<std::sync::MutexGuard<'_, File>> {
        self.file
            .lock()
            .map_err(|_| io::Error::other("indexed file lock"))
    }
}
impl StorageBackend for Backend {
    fn len(&self) -> io::Result<u64> {
        Ok(self.file()?.metadata()?.len())
    }
    fn read(&self, offset: u64, out: &mut [u8]) -> io::Result<()> {
        let mut file = self.file()?;
        file.seek(SeekFrom::Start(offset))?;
        file.read_exact(out)?;
        let mut c = self.control.lock()?;
        c.stats.reads = c.stats.reads.saturating_add(1);
        c.stats.read_bytes = c.stats.read_bytes.saturating_add(out.len() as u64);
        Ok(())
    }
    fn set_len(&self, len: u64) -> io::Result<()> {
        self.control.fault("before_resize")?;
        let old = self.len()?;
        {
            let mut c = self.control.lock()?;
            c.stats.last_requested_length = len;
            let total = len
                .checked_add(self.config.retained_external_bytes)
                .and_then(|v| v.checked_add(HEADROOM));
            if total.is_none_or(|v| v > self.config.physical_quota) {
                c.stats.last_failure = Some("ConfiguredCapacity".into());
                return Err(io::Error::other(format!(
                    "ConfiguredCapacity requested={len} quota={}",
                    self.config.physical_quota
                )));
            }
        }
        let t = Instant::now();
        if len > old && free_space(&self.root)? < len - old + HEADROOM {
            self.control.lock()?.stats.last_failure = Some("ObservedFreeSpace".into());
            return Err(io::Error::other("ObservedFreeSpace; not an OS reservation"));
        }
        self.file()?.set_len(len)?;
        let mut c = self.control.lock()?;
        c.stats.set_len_calls += 1;
        c.stats.length = len;
        let grow = len.saturating_sub(old);
        c.grown_since_sync |= grow > 0;
        c.stats.growth_bytes = c.stats.growth_bytes.saturating_add(grow);
        c.stats.max_growth = c.stats.max_growth.max(grow);
        c.stats.growth_nanos = c
            .stats
            .growth_nanos
            .saturating_add(t.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);
        drop(c);
        self.control.fault("after_resize")?;
        Ok(())
    }
    fn sync_data(&self) -> io::Result<()> {
        self.control.fault("before_sync")?;
        let t = Instant::now();
        self.file()?.sync_all()?;
        let mut c = self.control.lock()?;
        c.stats.sync_calls += 1;
        let grew = c.grown_since_sync;
        c.grown_since_sync = false;
        c.stats.sync_nanos = c
            .stats
            .sync_nanos
            .saturating_add(t.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);
        drop(c);
        if grew {
            super::checkpoint("after_extension_sync");
        }
        self.control.fault("after_sync")?;
        Ok(())
    }
    fn write(&self, offset: u64, data: &[u8]) -> io::Result<()> {
        self.control.fault("before_write")?;
        {
            let mut c = self.control.lock()?;
            c.write_epoch = c
                .write_epoch
                .checked_add(data.len() as u64)
                .ok_or_else(|| io::Error::other("write overflow"))?;
            if c.write_epoch > 128 * 1024 * 1024 {
                c.stats.last_failure = Some("ExplicitWriteBudget".into());
                return Err(io::Error::other("ExplicitWriteBudget"));
            }
        }
        let mut file = self.file()?;
        let end = offset
            .checked_add(data.len() as u64)
            .ok_or_else(|| io::Error::other("write address"))?;
        if end > file.metadata()?.len() {
            return Err(io::Error::other("write outside exact allocated length"));
        }
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(data)?;
        drop(file);
        let mut c = self.control.lock()?;
        c.stats.writes += 1;
        c.stats.write_bytes = c.stats.write_bytes.saturating_add(data.len() as u64);
        drop(c);
        self.control.fault("after_write")?;
        Ok(())
    }
}

// std has no safe cross-platform free-space API. Query the OS only at actual growth,
// without adding a native dependency/unsafe code; no fallback on probe failure.
#[cfg(windows)]
fn free_space(root: &Path) -> io::Result<u64> {
    use std::os::windows::process::CommandExt;
    use std::path::{Component, Prefix};
    let drive = match root.components().next() {
        Some(Component::Prefix(p)) => match p.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                format!("{}:\\", char::from(letter))
            }
            _ => return Err(io::Error::other("profile needs a local volume space probe")),
        },
        _ => return Err(io::Error::other("profile volume prefix missing")),
    };
    let script = format!("[IO.DriveInfo]::new('{}').AvailableFreeSpace", drive);
    let out = std::process::Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &script,
        ])
        .creation_flags(0x0800_0000)
        .output()?;
    if !out.status.success() {
        return Err(io::Error::other("free space OS query failed"));
    }
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .map_err(|_| io::Error::other("free space result"))
}
#[cfg(not(windows))]
fn free_space(root: &Path) -> io::Result<u64> {
    let out = std::process::Command::new("df")
        .env("LC_ALL", "C")
        .arg("-Pk")
        .arg(root)
        .output()?;
    if !out.status.success() {
        return Err(io::Error::other("free space OS query failed"));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let fields: Vec<_> = text
        .lines()
        .last()
        .ok_or_else(|| io::Error::other("df result"))?
        .split_whitespace()
        .collect();
    fields
        .get(fields.len().saturating_sub(3))
        .and_then(|v| v.parse::<u64>().ok())
        .and_then(|v| v.checked_mul(1024))
        .ok_or_else(|| io::Error::other("free space result"))
}
