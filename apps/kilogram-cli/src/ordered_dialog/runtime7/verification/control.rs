//! A pinned, bounded, outside-state debug control file; never protocol authority.
use super::*;
use serde::Deserialize;
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

pub(super) const ENV: &str = "KILOGRAM_TEST_S54_SCHEDULE";
const CAP: u64 = 16 * 1024;
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Phase {
    Observe,
    Mixed,
    Pressure,
    CatchUp,
    Relay,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Schedule {
    pub context: String,
    pub lane: u8,
    pub phase: Phase,
    #[serde(default)]
    pub hold_source64: bool,
    pub source64_after: Option<u64>,
    #[serde(default)]
    pub hold_ordinals: Vec<u64>,
    #[serde(default)]
    pub suspend_live: bool,
    #[serde(default)]
    pub suspend_prefix: bool,
    pub pause_after_normal_page: Option<u64>,
    #[serde(default)]
    pub hold_ack: bool,
    pub max_pause_ms: u64,
    pub retry_live: Option<super::retry::Request>,
}
impl Schedule {
    pub(super) fn parse(raw: &[u8], debug: bool) -> Result<Self> {
        ensure!(
            debug && raw.len() <= CAP as usize,
            "S54 debug control bound/release"
        );
        let s: Self = serde_json::from_slice(raw)?;
        ensure!(
            s.source64_after.is_none_or(|n| s.phase == Phase::Pressure
                && s.hold_source64
                && s.lane == 0
                && (1..=4032).contains(&n)
                && n.checked_add(64).is_some()),
            "S54 successor selector Pressure/A/bound"
        );
        if let Some(request) = &s.retry_live {
            request.validate()?;
            ensure!(
                s.phase == Phase::Mixed
                    && s.lane == 1
                    && !s.suspend_live
                    && !s.hold_ordinals.contains(&request.from),
                "S54 retry Mixed/B/unheld source only"
            );
        }
        ensure!(
            state::hex(&state::parse_id(&s.context)?) == s.context,
            "S54 canonical context"
        );
        ensure!(
            s.lane < 2 && (1..=120_000).contains(&s.max_pause_ms),
            "S54 lane/pause bound"
        );
        ensure!(
            s.hold_ordinals.len() <= 128
                && s.hold_ordinals.iter().all(|n| (1..=4096).contains(n))
                && s.hold_ordinals.windows(2).all(|p| p[0] < p[1]),
            "S54 ordinal bounds/order"
        );
        ensure!(
            !s.hold_source64 || s.phase == Phase::Pressure,
            "S54 pressure phase"
        );
        ensure!(!s.hold_ack || s.phase == Phase::CatchUp, "S54 ACK phase");
        ensure!(
            s.pause_after_normal_page
                .is_none_or(|n| s.phase == Phase::CatchUp && (1..=256).contains(&n)),
            "S54 page phase/bound"
        );
        ensure!(
            s.phase != Phase::Observe
                || (!s.hold_source64
                    && !s.hold_ack
                    && !s.suspend_live
                    && !s.suspend_prefix
                    && s.hold_ordinals.is_empty()
                    && s.pause_after_normal_page.is_none()),
            "S54 observe only"
        );
        Ok(s)
    }
    pub(super) fn applies(&self, c: Id, role: u8) -> bool {
        self.context == state::hex(&c) && self.lane == role
    }
}
#[derive(Clone)]
pub(super) struct Location {
    requested: PathBuf,
    canonical: PathBuf,
    root: PathBuf,
    _parents: Arc<Vec<File>>,
}
impl Location {
    pub(super) fn new(path: &Path, root: &Path, debug: bool) -> Result<Self> {
        ensure!(debug && path.is_absolute(), "S54 release/absolute path");
        #[cfg(windows)]
        {
            use std::path::{Component, Prefix};
            ensure!(
                matches!(path.components().next(),Some(Component::Prefix(p)) if matches!(p.kind(),Prefix::Disk(_)|Prefix::VerbatimDisk(_))),
                "S54 local drive control only"
            );
        }
        let root = root.canonicalize()?;
        let canonical = path.canonicalize()?;
        ensure!(
            !canonical.starts_with(&root) && canonical.is_file(),
            "S54 outside-state file"
        );
        let parents = pin_parents(canonical.parent().context("S54 file parent")?)?;
        let value = Self {
            requested: path.into(),
            canonical,
            root,
            _parents: Arc::new(parents),
        };
        value.load()?;
        Ok(value)
    }
    fn namespace(&self) -> Result<()> {
        ensure!(
            self.requested.canonicalize()? == self.canonical
                && !self.canonical.starts_with(&self.root),
            "S54 changed control namespace"
        );
        Ok(())
    }
    pub(super) fn load(&self) -> Result<Schedule> {
        self.namespace()?;
        let file = open_control(&self.canonical)?;
        self.verify_handle(&file)?;
        let mut raw = Vec::new();
        file.take(CAP + 1).read_to_end(&mut raw)?;
        self.namespace()?;
        Schedule::parse(&raw, cfg!(debug_assertions))
    }
    #[cfg(windows)]
    fn verify_handle(&self, file: &File) -> Result<()> {
        use std::os::windows::fs::MetadataExt;
        let meta = file.metadata()?;
        ensure!(
            meta.is_file() && meta.file_attributes() & 0x400 == 0,
            "S54 opened reparse/non-file"
        );
        self.namespace()
    }
    #[cfg(unix)]
    fn verify_handle(&self, file: &File) -> Result<()> {
        use std::os::unix::fs::MetadataExt;
        let expected = std::fs::metadata(&self.canonical)?;
        let actual = file.metadata()?;
        ensure!(
            actual.dev() == expected.dev() && actual.ino() == expected.ino(),
            "S54 opened file identity"
        );
        self.namespace()
    }
    pub(super) fn marker(&self, tag: &str, value: &serde_json::Value) -> Result<()> {
        use std::io::Write;
        self.namespace()?;
        let target = self.canonical.with_extension(format!("s54-{tag}.json"));
        let raw = serde_json::to_vec(value)?;
        ensure!(raw.len() <= 4096, "S54 marker cap");
        let mut file =
            tempfile::NamedTempFile::new_in(self.canonical.parent().context("S54 parent")?)?;
        file.write_all(&raw)?;
        file.as_file().sync_all()?;
        self.namespace()?;
        file.persist(target).map_err(|e| e.error)?;
        Ok(())
    }
}
#[cfg(windows)]
fn pin_parents(parent: &Path) -> Result<Vec<File>> {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    let mut held = Vec::new();
    // GENERIC_READ (not metadata-only access0) denies directory rename with
    // this share mode, while still permitting atomic child-file updates.
    // Unlike the control file itself these handles are not replaced on updates.
    for path in parent.ancestors().collect::<Vec<_>>().into_iter().rev() {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(3)
            .custom_flags(0x0020_0000 | 0x0200_0000)
            .open(path)?;
        let meta = file.metadata()?;
        ensure!(
            meta.is_dir() && meta.file_attributes() & 0x400 == 0,
            "S54 canonical parent reparse"
        );
        held.push(file);
    }
    Ok(held)
}
#[cfg(not(windows))]
fn pin_parents(_: &Path) -> Result<Vec<File>> {
    Ok(Vec::new())
}
fn open_control(path: &Path) -> std::io::Result<File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000);
    }
    options.open(path)
}
