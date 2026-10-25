use super::*;
use kilogram_transport_iroh::ordered2::RouteConfig2;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile2 {
    pub version: u8,
    pub profile_mode: String,
    pub state_dir: PathBuf,
    pub account: String,
    pub claim: String,
    pub device: String,
    pub contexts: [String; 2],
    pub enabled: Vec<u8>,
    pub ipc_file: PathBuf,
    pub locator_directory: PathBuf,
    pub route: RouteConfig2,
    #[serde(default = "logical_quota")]
    pub logical_quota: [u64; 2],
    #[serde(default = "physical_quota")]
    pub physical_quota: u64,
}
fn logical_quota() -> [u64; 2] {
    db::Config::default().logical_quota
}
fn physical_quota() -> u64 {
    db::Config::default().physical_quota
}
impl Profile2 {
    pub fn load(path: &Path) -> Result<Self> {
        let profile: Self = serde_json::from_slice(&shared_admin::read_public(path, 16 * 1024)?)?;
        profile.validate()?;
        Ok(profile)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.version == 2 && self.profile_mode == "ordered-dialog-v2",
            "UnsupportedProfile"
        );
        parse_id(&self.account)?;
        parse_id(&self.claim)?;
        parse_id(&self.device)?;
        ensure!(
            parse_id(&self.contexts[0])? != parse_id(&self.contexts[1])?,
            "duplicate contexts"
        );
        ensure!(
            !self.enabled.is_empty()
                && self.enabled.len() <= 2
                && self.enabled.iter().all(|c| *c <= 1)
                && (self.enabled.len() == 1 || self.enabled[0] != self.enabled[1]),
            "enabled contexts"
        );
        self.route.effective()?;
        ensure!(
            self.logical_quota
                .iter()
                .all(|q| *q >= db::REPAIR_POOL + db::TEMP_CREDIT)
                && self.physical_quota >= db::HEADROOM,
            "configured quota below protected floor"
        );
        let root = std::fs::canonicalize(&self.state_dir)?;
        hooks::validate(&root)?;
        for path in [&self.ipc_file, &self.locator_directory] {
            let mut absolute = if path.is_absolute() {
                path.clone()
            } else {
                std::env::current_dir()?.join(path)
            };
            while !absolute.exists() {
                ensure!(absolute.pop(), "artifact ancestor");
            }
            ensure!(
                !std::fs::canonicalize(absolute)?.starts_with(&root),
                "public artifact inside state"
            );
        }
        Ok(())
    }
    pub fn owner(&self) -> Result<db::ExpectedOwner> {
        Ok(db::ExpectedOwner {
            account: parse_id(&self.account)?,
            claim: parse_id(&self.claim)?,
        })
    }
    pub fn db_config(&self) -> db::Config {
        db::Config {
            logical_quota: self.logical_quota,
            physical_quota: self.physical_quota,
            retained_external_bytes: 0,
        }
    }
}
