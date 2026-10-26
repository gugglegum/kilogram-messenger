use super::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile3 {
    pub version: u8,
    pub profile_mode: String,
    pub account: String,
    pub claim: String,
    pub device: String,
    pub source_id: String,
    pub migration_id: String,
    pub config_revision: u64,
    pub state_dir: PathBuf,
    pub ipc_file: PathBuf,
    pub locator_directory: PathBuf,
    pub logical_quota: [u64; 2],
    pub physical_capacity: u64,
    pub route_policy: String,
    pub enabled_contexts: Vec<String>,
}
impl Profile3 {
    pub fn load(path: &Path) -> Result<Self> {
        let profile: Self = serde_json::from_slice(&admin::read_public(path, 64 * 1024)?)?;
        ensure!(
            profile.version == 3 && profile.profile_mode == "ordered-dialog-indexed",
            "UnsupportedProfile3"
        );
        ensure!(
            profile.route_policy == "direct-only",
            "L1RequiresDirectOnly"
        );
        for id in [
            &profile.account,
            &profile.claim,
            &profile.device,
            &profile.source_id,
            &profile.migration_id,
        ] {
            state::parse_id(id)?;
        }
        ensure!(
            profile.config_revision > 0 && profile.enabled_contexts.len() <= 2,
            "indexed config/context bounds"
        );
        let mut unique = std::collections::BTreeSet::new();
        for id in &profile.enabled_contexts {
            ensure!(
                unique.insert(state::parse_id(id)?),
                "duplicate enabled context"
            );
        }
        let root = std::fs::canonicalize(&profile.state_dir)?;
        super::super::test_support::validate(&root)?;
        for path in [&profile.ipc_file, &profile.locator_directory] {
            ensure!(path.is_absolute(), "indexed public path must be absolute");
            let mut ancestor = path.clone();
            while !ancestor.exists() {
                ensure!(ancestor.pop(), "public artifact ancestor");
            }
            ensure!(
                !ancestor.canonicalize()?.starts_with(&root),
                "public artifact inside state"
            );
        }
        Ok(profile)
    }
    pub fn open(&self, profile_path: &Path) -> Result<store::L1Repository> {
        let root = kilogram_state::StateDirectoryLock::acquire(&self.state_dir)?;
        let config = db::migration7::MigrationConfig {
            logical_quota: self.logical_quota,
            physical_capacity: self.physical_capacity,
            output_profile: std::fs::canonicalize(profile_path)?,
            control_ipc_file: self.ipc_file.clone(),
            locator_directory: self.locator_directory.clone(),
        };
        let repository = store::L1Repository::open_ready(
            root,
            db::ExpectedOwner {
                account: state::parse_id(&self.account)?,
                claim: state::parse_id(&self.claim)?,
            },
            state::parse_id(&self.device)?,
            config,
        )?;
        ensure!(
            repository.source_id() == state::parse_id(&self.source_id)?
                && repository.migration_id() == state::parse_id(&self.migration_id)?
                && repository.config_revision() == self.config_revision,
            "Profile3 stored origin/config mismatch"
        );
        Ok(repository)
    }
}
