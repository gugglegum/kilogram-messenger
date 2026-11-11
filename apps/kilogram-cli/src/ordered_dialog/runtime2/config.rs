use super::*;
use kilogram_runtime_ipc::ordered2::{Profile2Document, ProfileRoute2, ProfileRoutePolicy2};
use kilogram_transport_iroh::ordered2::RouteConfig2;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize, Deserialize)]
#[serde(try_from = "Profile2Document", into = "Profile2Document")]
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
    pub logical_quota: [u64; 2],
    pub physical_quota: u64,
}
impl TryFrom<Profile2Document> for Profile2 {
    type Error = anyhow::Error;
    fn try_from(d: Profile2Document) -> Result<Self> {
        let defaults = db::Config::default();
        Ok(Self {
            version: d.version,
            profile_mode: d.profile_mode,
            state_dir: d.state_dir,
            account: d.account,
            claim: d.claim,
            device: d.device,
            contexts: d.contexts,
            enabled: d.enabled,
            ipc_file: d.ipc_file,
            locator_directory: d.locator_directory,
            route: RouteConfig2 {
                policy: match d.route.policy {
                    ProfileRoutePolicy2::Auto => kilogram_transport_iroh::RoutePolicy::Auto,
                    ProfileRoutePolicy2::DirectOnly => {
                        kilogram_transport_iroh::RoutePolicy::DirectOnly
                    }
                    ProfileRoutePolicy2::RelayOnly => {
                        kilogram_transport_iroh::RoutePolicy::RelayOnly
                    }
                },
                bind: d.route.bind,
                relay: d.route.relay,
            },
            logical_quota: d.logical_quota.unwrap_or(defaults.logical_quota),
            physical_quota: d.physical_quota.unwrap_or(defaults.physical_quota),
        })
    }
}
impl From<Profile2> for Profile2Document {
    fn from(d: Profile2) -> Self {
        Self {
            version: d.version,
            profile_mode: d.profile_mode,
            state_dir: d.state_dir,
            account: d.account,
            claim: d.claim,
            device: d.device,
            contexts: d.contexts,
            enabled: d.enabled,
            ipc_file: d.ipc_file,
            locator_directory: d.locator_directory,
            route: ProfileRoute2 {
                policy: match d.route.policy {
                    kilogram_transport_iroh::RoutePolicy::Auto => ProfileRoutePolicy2::Auto,
                    kilogram_transport_iroh::RoutePolicy::DirectOnly => {
                        ProfileRoutePolicy2::DirectOnly
                    }
                    kilogram_transport_iroh::RoutePolicy::RelayOnly => {
                        ProfileRoutePolicy2::RelayOnly
                    }
                },
                bind: d.route.bind,
                relay: d.route.relay,
            },
            logical_quota: Some(d.logical_quota),
            physical_quota: Some(d.physical_quota),
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cs32b_profile_defaults_null_and_authoritative_validation() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let root = tmp.path().join("state");
        std::fs::create_dir(&root)?;
        let path = tmp.path().join("profile.json");
        let value = serde_json::json!({"version":2,"profile_mode":"ordered-dialog-v2","state_dir":root,"account":"11".repeat(32),"claim":"22".repeat(32),"device":"33".repeat(32),"contexts":["44".repeat(32),"55".repeat(32)],"enabled":[0,1],"ipc_file":tmp.path().join("ipc.json"),"locator_directory":tmp.path().join("locators"),"route":{"policy":"direct-only","bind":"127.0.0.1:0","relay":null}});
        std::fs::write(&path, serde_json::to_vec(&value)?)?;
        let profile = Profile2::load(&path)?;
        ensure!(
            profile.logical_quota == db::Config::default().logical_quota
                && profile.physical_quota == db::Config::default().physical_quota,
            "default quotas drift"
        );
        let emitted = serde_json::to_vec(&profile)?;
        let doc = Profile2Document::decode(&emitted)?;
        ensure!(
            doc.logical_quota == Some(profile.logical_quota)
                && doc.physical_quota == Some(profile.physical_quota),
            "emitted schema"
        );
        for field in ["logical_quota", "physical_quota"] {
            let mut bad = value.clone();
            bad[field] = serde_json::Value::Null;
            std::fs::write(&path, serde_json::to_vec(&bad)?)?;
            ensure!(Profile2::load(&path).is_err(), "explicit null relaxed");
        }
        let mut low = value.clone();
        low["physical_quota"] = 0.into();
        std::fs::write(&path, serde_json::to_vec(&low)?)?;
        ensure!(Profile2::load(&path).is_err(), "quota gate bypassed");
        let mut route = value;
        route["route"]["bind"] = "192.0.2.1:0".into();
        std::fs::write(&path, serde_json::to_vec(&route)?)?;
        ensure!(Profile2::load(&path).is_err(), "route gate bypassed");
        Ok(())
    }
}
