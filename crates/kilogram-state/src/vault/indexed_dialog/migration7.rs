//! Explicit V4 migration boundary; no live runtime or generic writer fallback.
#[path = "migration7/admin.rs"]
mod admin;
#[path = "migration7/codec.rs"]
pub mod codec;
#[path = "migration7/copy.rs"]
mod copy;
#[path = "migration7/fence.rs"]
mod fence;
#[path = "migration7/load.rs"]
mod load;
#[path = "migration7/operations.rs"]
mod operations;
#[path = "migration7/preview.rs"]
mod preview;
#[path = "migration7/records.rs"]
mod records;
#[path = "migration7/source.rs"]
mod source;
#[cfg(test)]
#[path = "migration7/tests.rs"]
mod tests;
use super::{Id, Result};
pub use admin::MigrationStatus;
pub use copy::MigrationProjection;
pub(in crate::vault::indexed_dialog) use operations::Operation;
pub use preview::{MigrationInput, MigrationPreview};
pub use records::{MigrationConfig, MigrationContextFacts, MigrationOriginFacts, MigrationStage};
pub use source::{LegacyEntry, V4Source};
use zeroize::Zeroizing;

pub struct MigrationRepository {
    core: super::IndexedDialogVault,
    inventory: source::Inventory,
}
impl MigrationRepository {
    pub fn fence(
        source: V4Source,
        origin: &MigrationOriginFacts,
        config: &MigrationConfig,
    ) -> Result<Self> {
        fence::fence(source, origin, config)
    }
    pub fn generation(&self) -> u64 {
        self.core.generation()
    }
    pub fn source_id(&self) -> Id {
        self.inventory.source_id
    }
    pub fn open(
        lease: crate::StateDirectoryLock,
        account: Id,
        device: Id,
        recovery: Option<&MigrationConfig>,
    ) -> Result<Self> {
        load::open_repository(lease, account, device, recovery)
    }
}
impl LegacyRead for MigrationRepository {
    fn entries(&self) -> &[LegacyEntry] {
        &self.inventory.entries
    }
    fn source_id(&self) -> Id {
        self.inventory.source_id
    }
    fn root(&self) -> &std::path::Path {
        self.core._lease.root()
    }
    fn get(&self, path: &str) -> Result<Option<Zeroizing<Vec<u8>>>> {
        if self.core.poisoned {
            return Err(super::Error::ReopenRequired);
        }
        self.inventory
            .get(&self.core.vault, path, Some(&self.inventory.mode_raw))
    }
}
fn checkpoint(root: &std::path::Path, point: &str) -> Result<()> {
    use std::io::Write;
    let Some(path) = std::env::var_os("KILOGRAM_TEST_MIGRATION_PAUSE_FILE") else {
        return Ok(());
    };
    if !cfg!(debug_assertions) {
        return Err(super::Error::Bound("migration pause is debug-only"));
    }
    let path = std::path::PathBuf::from(path).canonicalize()?;
    if path.starts_with(root) || std::fs::metadata(&path)?.len() > 1024 {
        return Err(super::Error::Bound("pause control outside state"));
    }
    if std::fs::read(&path)? != point.as_bytes() {
        return Ok(());
    }
    let hit = path.with_extension("hit");
    let mut marker =
        tempfile::NamedTempFile::new_in(hit.parent().ok_or(super::Error::Bound("marker parent"))?)?;
    marker.write_all(point.as_bytes())?;
    marker.as_file().sync_all()?;
    marker.persist(&hit).map_err(|error| error.error)?;
    crate::sync_directory(hit.parent().ok_or(super::Error::Bound("marker parent"))?)?;
    while std::fs::read(&path)? == point.as_bytes() {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    Ok(())
}

/// A bounded primary-source view, held under the migration owner's lifetime lock.
pub trait LegacyRead {
    fn entries(&self) -> &[LegacyEntry];
    fn source_id(&self) -> Id;
    fn root(&self) -> &std::path::Path;
    fn get(&self, path: &str) -> Result<Option<Zeroizing<Vec<u8>>>>;
    fn required(&self, path: &str) -> Result<Zeroizing<Vec<u8>>> {
        self.get(path)?
            .ok_or(super::Error::Integrity("required source row absent"))
    }
}
impl LegacyRead for V4Source {
    fn entries(&self) -> &[LegacyEntry] {
        self.entries()
    }
    fn source_id(&self) -> Id {
        self.source_id()
    }
    fn root(&self) -> &std::path::Path {
        self.root()
    }
    fn get(&self, path: &str) -> Result<Option<Zeroizing<Vec<u8>>>> {
        self.get(path)
    }
}
