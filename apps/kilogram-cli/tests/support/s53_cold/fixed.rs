//! Fixed original lineage. These literals do not inspect/activate private data.
use super::*;
use kilogram_state::StateDirectoryLock;
pub const WORKSPACE: &str = r"E:\AI-Projects\kilogram-p2p-messenger";
pub const STATE: &str =
    r"E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-storage\l1-exhausted-observed-NCjBHb";
pub const PROOF: &str = r"E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-admin\P2-s53-exhausted-roots-01-attempt-01\proof";
pub const ATTEMPT: &str = "P2-s53-exhausted-cold-b-01-attempt-02";
pub const EVIDENCE: &str = r"E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-preparation-evidence\P2-s53-exhausted-cold-b-01-attempt-02";
pub const SHELL: &str = r"C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe";
pub const ACCOUNTS: [&str; 2] = [
    "7a87df0a69a322c4ec28fb94e175e26e77c428151a6e67336f6d0c8eb0abb2ce",
    "9a16920200c69f6507bdf7ed94ede32b3f1dd36158e32be35159b1ac9fe25f0e",
];
pub const DEVICES: [&str; 2] = [
    "55f714cd9cebfc84d1af03d515373e851704c0f1a87ce1d7b84cd4cffb2e69bb",
    "a227497b8c0e9029a96b668939cc3ccc96d7a42f054c47927f486d4ca3d305f8",
];
pub const CONTEXT: &str = "ef689629a47207362fbc870bf610d921c8e28ca0646471914bd93218fff43810";
pub fn future_ids() -> [[u8; 16]; 2] {
    [600u64, 601].map(|n| {
        let mut id = [0; 16];
        id[0] = 0xfa;
        id[8..].copy_from_slice(&n.to_be_bytes());
        id
    })
}
pub struct Expected {
    pub local: usize,
    pub c: old::Id,
    pub accounts: [old::Id; 2],
    pub devices: [old::Id; 2],
    pub sid: String,
    pub next: [u64; 2],
    pub native_generation: u64,
    pub native_hash: String,
}
impl Expected {
    pub fn production(local: usize) -> Result<Self> {
        Ok(Self {
            local,
            c: state::parse_id(CONTEXT)?,
            accounts: [state::parse_id(ACCOUNTS[0])?, state::parse_id(ACCOUNTS[1])?],
            devices: [state::parse_id(DEVICES[0])?, state::parse_id(DEVICES[1])?],
            sid: "TflibOtoo5vbulwTkVuDsE6DrSVTFMHdGT2ZHDDx+oE".into(),
            next: [512, 18],
            native_generation: [517, 88][local],
            native_hash: [
                "6c50ea198a9d1c9fc6338c873a7bcda7d169c421d32d65fbe5369c3695c2d247",
                "e6e4abc55d1216a3514b40261005d6eb29e9fdecbc0312a78bddfc21fccccf1f",
            ][local]
                .into(),
        })
    }
    pub fn scope(&self, scope: &wire::VerifiedExistingScope1) -> Result<()> {
        ensure!(
            scope.c() == self.c && scope.sid() == self.sid,
            "original context/SID"
        );
        for role in 0..2 {
            ensure!(
                scope.peers()[role].certificate().account_id().as_bytes() == &self.accounts[role]
                    && scope.peers()[role].certificate().device_id().as_bytes()
                        == &self.devices[role],
                "original account/device/role"
            );
        }
        Ok(())
    }
    pub fn text(&self, role: usize, n: u64) -> ([u8; 16], String) {
        let mut request = [0; 16];
        if role == 1 && n == 17 {
            request[0] = 0xfa;
            request[8..].copy_from_slice(&9001u64.to_be_bytes());
            return (
                request,
                "frozen B may send17 while indexed A own lane is exhausted".into(),
            );
        }
        request[..8].copy_from_slice(&(if role == 0 { 2u64 } else { 1 }).to_be_bytes());
        request[8..].copy_from_slice(&n.to_be_bytes());
        (
            request,
            format!("OD1/{}/{n}/Привет", if role == 0 { "two" } else { "one" }),
        )
    }
}
pub fn literal(path: &Path) -> Result<String> {
    let text = path.to_str().context("Unicode drive path")?;
    let text = text
        .strip_prefix(r"\\?\")
        .unwrap_or(text)
        .replace('/', r"\");
    ensure!(
        text.len() > 3
            && text.as_bytes()[1] == b':'
            && text.as_bytes()[2] == b'\\'
            && !text[3..].split('\\').any(|s| s.is_empty()
                || s == "."
                || s == ".."
                || s.contains(':')
                || s.ends_with(['.', ' '])),
        "literal drive path"
    );
    Ok(text)
}
fn exact(path: &Path, expected: &Path) -> Result<()> {
    ensure!(
        literal(path)? == literal(expected)?,
        "fixed original path mismatch"
    );
    Ok(())
}
fn existing(path: &Path, directory: bool, cap: u64) -> Result<()> {
    use std::os::windows::fs::MetadataExt;
    for component in path.ancestors() {
        let meta = std::fs::symlink_metadata(component)?;
        ensure!(
            meta.file_attributes() & 0x400 == 0,
            "original reparse input"
        );
    }
    let meta = std::fs::metadata(path)?;
    ensure!(
        meta.is_dir() == directory
            && if directory {
                true
            } else {
                meta.is_file() && meta.len() > 0 && meta.len() <= cap
            },
        "original existing input kind/cap"
    );
    Ok(())
}
pub struct Prepared {
    pub profile: Profile3,
    pub config: db::migration7::MigrationConfig,
    pub profile_raw: Vec<u8>,
    pub config_raw: Vec<u8>,
}
pub fn prepare(base: &Path, local: usize) -> Result<Prepared> {
    let expected = Path::new(STATE).join(if local == 0 { "two" } else { "one" });
    exact(base, &expected)?;
    let profile_path = base.join("indexed.profile");
    let config_path = base.join("migration.json");
    for path in [
        &profile_path,
        &config_path,
        &base.join("state/state-vault.key"),
    ] {
        existing(path, false, 64 * 1024)?;
    }
    existing(
        &base.join("state/state-vault.redb"),
        false,
        64 * 1024 * 1024,
    )?;
    let profile_raw = std::fs::read(&profile_path)?;
    let config_raw = std::fs::read(&config_path)?;
    // Deserialize and validate all path/identity fields BEFORE Profile3::load or
    // any key/database loader. The held raw baseline pins these original bytes.
    let profile: Profile3 = serde_json::from_slice(&profile_raw)?;
    let config: db::migration7::MigrationConfig = serde_json::from_slice(&config_raw)?;
    exact(&profile.state_dir, &base.join("state"))?;
    exact(&profile.ipc_file, &base.join("indexed.ipc"))?;
    exact(&profile.locator_directory, &base.join("indexed.locators"))?;
    exact(&config.output_profile, &profile_path)?;
    exact(&config.control_ipc_file, &profile.ipc_file)?;
    exact(&config.locator_directory, &profile.locator_directory)?;
    ensure!(
        profile.account == ACCOUNTS[local]
            && profile.device == DEVICES[local]
            && profile.enabled_contexts == [CONTEXT]
            && profile.logical_quota == config.logical_quota
            && profile.physical_capacity == config.physical_capacity,
        "original profile/config identity/quotas"
    );
    existing(&profile.state_dir, true, 0)?;
    let checked = Profile3::load(&profile_path)?;
    ensure!(
        serde_json::to_vec(&checked)? == serde_json::to_vec(&profile)?,
        "profile preflight/readback drift"
    );
    Ok(Prepared {
        profile,
        config,
        profile_raw,
        config_raw,
    })
}
pub fn open(prepared: &Prepared, lease: StateDirectoryLock) -> Result<store::L1Repository> {
    let p = &prepared.profile;
    let config = db::migration7::MigrationConfig {
        output_profile: std::fs::canonicalize(&prepared.config.output_profile)?,
        control_ipc_file: p.ipc_file.clone(),
        locator_directory: p.locator_directory.clone(),
        logical_quota: p.logical_quota,
        physical_capacity: p.physical_capacity,
    };
    let repo = store::L1Repository::open_ready(
        lease,
        db::ExpectedOwner {
            account: state::parse_id(&p.account)?,
            claim: state::parse_id(&p.claim)?,
        },
        state::parse_id(&p.device)?,
        config,
    )?;
    ensure!(
        repo.source_id() == state::parse_id(&p.source_id)?
            && repo.migration_id() == state::parse_id(&p.migration_id)?
            && repo.config_revision() == p.config_revision,
        "stored original profile origin/config"
    );
    Ok(repo)
}
