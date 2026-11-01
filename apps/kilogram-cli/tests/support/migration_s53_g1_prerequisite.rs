//! Exhausted-case-only admission. No preparation, retargeting or activation.
use super::*;
use kilogram_cli::ordered_dialog::runtime7::Profile3;
use kilogram_state::indexed_dialog::migration7::MigrationConfig;
use sha2::{Digest, Sha256};
#[cfg(windows)]
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::{fs::File, io::Read};

const CASE: &str = "F-s53-exhausted-integrated-01";
const RELATIVE: &str = "ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/P2-s53-exhausted-M1.candidate.json";
const M1_SHA: &str = "788076595dbebdfc7157731609d4a1c1ce613e78821c4d866bded41f16e50c77";
const D2: &str = "ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/P2-s53-exhausted-M1.D2-root-accepted.md";
const D2_SHA: &str = "9ea318a5531dc9a5b5ff605eae2bcf4328772db9cea7e84fe383baf4bbcc33a9";
const S: &str =
    r"E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-storage\l1-exhausted-observed-NCjBHb";
const R: &str = r"E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-admin\P2-s53-exhausted-roots-01-attempt-01";
const C: &str = "ef689629a47207362fbc870bf610d921c8e28ca0646471914bd93218fff43810";
const ACCOUNTS: [&str; 2] = [
    "7a87df0a69a322c4ec28fb94e175e26e77c428151a6e67336f6d0c8eb0abb2ce",
    "9a16920200c69f6507bdf7ed94ede32b3f1dd36158e32be35159b1ac9fe25f0e",
];
const DEVICES: [&str; 2] = [
    "55f714cd9cebfc84d1af03d515373e851704c0f1a87ce1d7b84cd4cffb2e69bb",
    "a227497b8c0e9029a96b668939cc3ccc96d7a42f054c47927f486d4ca3d305f8",
];
const CLASS: &str = "root-allocated public replica manifest; no preparation performed by adapter";

fn sha(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}
fn literal(path: &Path) -> Result<String> {
    let raw = path.to_str().context("G1 path Unicode")?;
    let text = raw.strip_prefix(r"\\?\").unwrap_or(raw).replace('/', r"\");
    ensure!(
        text.len() > 3
            && text.as_bytes()[0].is_ascii_alphabetic()
            && text.as_bytes()[1] == b':'
            && text.as_bytes()[2] == b'\\',
        "G1 absolute drive path"
    );
    ensure!(
        text[3..].split('\\').all(|p| !p.is_empty()
            && p != "."
            && p != ".."
            && !p.ends_with(['.', ' '])
            && !p.contains([':', '<', '>', '"', '|', '?', '*'])),
        "G1 path components"
    );
    Ok(text)
}
fn exact(path: &Path, expected: &Path) -> Result<()> {
    ensure!(literal(path)? == literal(expected)?, "G1 exact fixed path");
    Ok(())
}
fn ordinary(path: &Path, directory: bool) -> Result<()> {
    literal(path)?;
    for (n, p) in path.ancestors().enumerate() {
        let m = std::fs::symlink_metadata(p)?;
        ensure!(
            !m.file_type().is_symlink() && m.is_dir() == (directory || n > 0),
            "G1 ordinary ancestor/kind"
        );
        #[cfg(windows)]
        ensure!(m.file_attributes() & 0x400 == 0, "G1 reparse input");
    }
    Ok(())
}
fn artifact(path: &Path, directory: bool) -> Result<()> {
    literal(path)?;
    match std::fs::symlink_metadata(path) {
        Ok(_) => ordinary(path, directory),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            ordinary(path.parent().context("G1 artifact parent")?, true)
        }
        Err(error) => Err(error.into()),
    }
}
fn read_pinned(path: &Path, bytes: u64, digest: &str) -> Result<(File, Vec<u8>)> {
    ensure!(bytes > 0 && bytes <= 65536, "G1 input byte cap");
    ordinary(path, false)?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    options.share_mode(1).custom_flags(0x0020_0000);
    let file = options.open(path)?;
    let meta = file.metadata()?;
    ensure!(meta.is_file() && meta.len() == bytes, "G1 input length");
    #[cfg(windows)]
    ensure!(meta.file_attributes() & 0x400 == 0, "G1 reparse handle");
    let mut raw = Vec::new();
    (&file).take(bytes + 1).read_to_end(&mut raw)?;
    ensure!(
        raw.len() as u64 == bytes && file.metadata()?.len() == bytes && sha(&raw) == digest,
        "G1 input SHA256"
    );
    Ok((file, raw))
}
fn inventory_binding(input: &Value, allocated: &Path, raw: &[u8]) -> Result<Value> {
    exact(allocated, &workspace().join(RELATIVE))?;
    ensure!(
        input["case"] == CASE && input["classification"] == "bound_pre_run_inventory",
        "G1 bound case inventory"
    );
    let rows = input["prerequisites"]
        .as_array()
        .context("G1 prerequisite array")?;
    ensure!(rows.len() == 1, "G1 exactly one prerequisite");
    let row = &rows[0];
    ensure!(
        row.as_object().is_some_and(|o| o.len() == 4)
            && row["classification"] == CLASS
            && row["bytes"] == 23116
            && row["sha256"] == M1_SHA,
        "G1 exact prerequisite tuple"
    );
    exact(
        Path::new(row["path"].as_str().context("G1 prerequisite path")?),
        allocated,
    )?;
    ensure!(
        raw.len() == 23116 && sha(raw) == M1_SHA,
        "G1 manifest SHA256"
    );
    let m: Value = serde_json::from_slice(raw)?;
    manifest_fields(&m)?;
    Ok(m)
}
fn manifest_fields(m: &Value) -> Result<()> {
    ensure!(
        m["schema_version"] == 1
            && m["classification"] == "s53-exhausted-exact-path-public-prerequisite-manifest"
            && m["case"] == CASE
            && m["runtime_activated"] == false,
        "G1 manifest schema/case"
    );
    ensure!(
        m["context"] == C
            && m["original_session_id"] == "TflibOtoo5vbulwTkVuDsE6DrSVTFMHdGT2ZHDDx+oE"
            && m["exact_original_state_path"] == true,
        "G1 original scope"
    );
    ensure!(
        m["root_decision_sha256"]
            == "17cb30232565f8d59c7467c9512a519cbfeac4a49ee6759951f4f13f2af06f72"
            && m["source_preservation_sha256"]
                == "a0efc865407cf0202644b7fa8ce584d564f16b0747ba8b2a31290f15d4996b06"
            && m["copy_inventory_sha256"]
                == "af94e87d1f383fd11db68a0ba6c197a960ad3ca50b6880d221a74d14c6fc161d",
        "G1 accepted prior evidence"
    );
    ensure!(
        m["single_active_copy_per_native_domain"].is_object()
            && m["current_own_roots_available"].is_object()
            && m["temporal_limits"]["future_genuine_Consent2_requires_fresh_current_snapshot_and_stored_floor_checks"]
                == true,
        "G1 bounded observations, not perpetual permission"
    );
    let nodes = m["nodes"].as_array().context("G1 nodes")?;
    ensure!(nodes.len() == 2, "G1 exact node count");
    for (i, node) in nodes.iter().enumerate() {
        ensure!(
            node["role"] == ["A", "B"][i]
                && node["local_index"] == i
                && node["producer_side"] == ["two", "one"][i]
                && node["account"] == ACCOUNTS[i]
                && node["device"] == DEVICES[i],
            "G1 original role/account/device"
        );
        let base = Path::new(S).join(["two", "one"][i]);
        for (field, path) in [
            ("base", base.clone()),
            ("profile", base.join("indexed.profile")),
            ("exact_state_path", base.join("state")),
            ("migration_config", base.join("migration.json")),
            ("ipc", base.join("indexed.ipc")),
            ("locators", base.join("indexed.locators")),
            (
                "own_root",
                Path::new(R).join(format!("{}.account", ["two", "one"][i])),
            ),
        ] {
            exact(
                Path::new(node[field].as_str().context("G1 fixed node path")?),
                &path,
            )?;
        }
    }
    Ok(())
}
fn raw_profile(node: &Value, raw: &[u8], config_raw: &[u8]) -> Result<Profile3> {
    ensure!(
        node["profile_pin"]["bytes"] == raw.len()
            && node["profile_pin"]["sha256"] == sha(raw)
            && node["config_pin"]["bytes"] == config_raw.len()
            && node["config_pin"]["sha256"] == sha(config_raw),
        "G1 profile/config pin"
    );
    let p: Profile3 = serde_json::from_slice(raw)?;
    let c: MigrationConfig = serde_json::from_slice(config_raw)?;
    ensure!(
        p.version == 3
            && p.profile_mode == "ordered-dialog-indexed"
            && p.route_policy == "direct-only"
            && p.config_revision > 0,
        "G1 raw profile kind"
    );
    p.route_config()?.effective()?;
    for id in [
        &p.account,
        &p.claim,
        &p.device,
        &p.source_id,
        &p.migration_id,
    ] {
        kilogram_cli::ordered_dialog::state::parse_id(id)?;
    }
    ensure!(
        p.account == node["account"]
            && p.device == node["device"]
            && p.enabled_contexts == [C]
            && p.logical_quota == c.logical_quota
            && p.physical_capacity == c.physical_capacity,
        "G1 raw identity/quotas"
    );
    for (a, b) in [
        (&p.state_dir, "exact_state_path"),
        (&p.ipc_file, "ipc"),
        (&p.locator_directory, "locators"),
        (&c.output_profile, "profile"),
        (&c.control_ipc_file, "ipc"),
        (&c.locator_directory, "locators"),
    ] {
        exact(a, Path::new(node[b].as_str().context("G1 raw fixed path")?))?;
    }
    Ok(p)
}
pub(super) struct Prepared {
    manifest: Value,
    profiles: [Profile3; 2],
    _guards: Vec<File>,
}
impl Prepared {
    pub(super) fn manifest_path(&self) -> PathBuf {
        workspace().join(RELATIVE)
    }
    pub(super) fn load(managed: &Managed) -> Result<Self> {
        let attempt_result::Provenance::BoundPreRunInventory {
            path,
            blake3: bound,
            ..
        } = &managed.provenance
        else {
            anyhow::bail!("G1 bound inventory provenance")
        };
        let inventory = read_managed_inventory(path)?;
        ensure!(
            blake3::hash(&inventory).to_hex().as_str() == bound,
            "G1 inventory unchanged since admission"
        );
        let input: Value = serde_json::from_slice(&inventory)?;
        let allocated = PathBuf::from(
            std::env::var_os("KILOGRAM_S53_EXHAUSTED_MANIFEST").context("G1 allocated manifest")?,
        );
        exact(&allocated, &workspace().join(RELATIVE))?;
        let (d2, _) = read_pinned(&workspace().join(D2), 5661, D2_SHA)?;
        let (m1, raw) = read_pinned(&allocated, 23116, M1_SHA)?;
        let m = inventory_binding(&input, &allocated, &raw)?;
        let mut guards = vec![d2, m1];
        let mut profiles = Vec::new();
        for node in m["nodes"].as_array().context("G1 pair")? {
            let pp = Path::new(node["profile"].as_str().context("G1 profile")?);
            let cp = Path::new(node["migration_config"].as_str().context("G1 config")?);
            let (pf, praw) = read_pinned(
                pp,
                node["profile_pin"]["bytes"]
                    .as_u64()
                    .context("G1 profile bytes")?,
                node["profile_pin"]["sha256"]
                    .as_str()
                    .context("G1 profile hash")?,
            )?;
            let (cf, craw) = read_pinned(
                cp,
                node["config_pin"]["bytes"]
                    .as_u64()
                    .context("G1 config bytes")?,
                node["config_pin"]["sha256"]
                    .as_str()
                    .context("G1 config hash")?,
            )?;
            profiles.push(raw_profile(node, &praw, &craw)?);
            guards.extend([pf, cf]);
        }
        // Both raw files for BOTH replicas are accepted before any typed open.
        // Share-READ guards retain their exact bytes through both constructors.
        for (node, p) in m["nodes"]
            .as_array()
            .context("G1 pair")?
            .iter()
            .zip(&profiles)
        {
            ordinary(&p.state_dir, true)?;
            artifact(&p.ipc_file, false)?;
            artifact(&p.locator_directory, true)?;
            for name in [
                ".kilogram-state.lock",
                "state-vault.key",
                "state-vault.redb",
            ] {
                ordinary(&p.state_dir.join(name), false)?;
            }
            ensure!(
                std::fs::metadata(p.state_dir.join(".kilogram-state.lock"))?.len() == 0,
                "G1 existing empty state lock"
            );
            for (name, cap) in [
                ("state-vault.key", 65536),
                ("state-vault.redb", 64 * 1024 * 1024),
            ] {
                let bytes = std::fs::metadata(p.state_dir.join(name))?.len();
                ensure!(bytes > 0 && bytes <= cap, "G1 existing key/database bound");
            }
            let loaded =
                Profile3::load(Path::new(node["profile"].as_str().context("G1 profile")?))?;
            ensure!(
                serde_json::to_vec(&loaded)? == serde_json::to_vec(p)?,
                "G1 profile readback"
            );
        }
        let profiles = profiles
            .try_into()
            .map_err(|_| anyhow::anyhow!("G1 pair count"))?;
        Ok(Self {
            manifest: m,
            profiles,
            _guards: guards,
        })
    }
    pub(super) fn nodes(self) -> Result<([Node; 2], [PathBuf; 2], String)> {
        let mut nodes = Vec::new();
        let mut roots = Vec::new();
        for (i, p) in self.profiles.into_iter().enumerate() {
            let n = &self.manifest["nodes"][i];
            let base = PathBuf::from(n["base"].as_str().context("G1 base")?);
            let profile_path = base.join("indexed.profile");
            let repository = p.open(&profile_path)?;
            ensure!(
                repository.contexts()
                    == [(kilogram_cli::ordered_dialog::state::parse_id(C)?, i as u8)],
                "G1 exact stored context/role"
            );
            drop(repository);
            nodes.push(Node {
                branch: Branch {
                    base: base.clone(),
                    state: p.state_dir,
                    account: p.account,
                    device: p.device,
                    config: base.join("migration.json"),
                    context: C.into(),
                    claim: p.claim,
                },
                profile: profile_path,
                ipc: p.ipc_file,
                locators: p.locator_directory,
                indexed: true,
                slot: 0,
            });
            roots.push(PathBuf::from(
                n["own_root"].as_str().context("G1 own Root")?,
            ));
        }
        Ok((
            nodes
                .try_into()
                .map_err(|_| anyhow::anyhow!("G1 node count"))?,
            roots
                .try_into()
                .map_err(|_| anyhow::anyhow!("G1 root count"))?,
            C.into(),
        ))
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    const RAW: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/P2-s53-exhausted-M1.candidate.json"
    ));
    fn input() -> Value {
        json!({"case":CASE,"classification":"bound_pre_run_inventory","prerequisites":[{"path":workspace().join(RELATIVE),"bytes":23116,"sha256":M1_SHA,"classification":CLASS}]})
    }
    #[test]
    fn g1a_exact_manifest_inventory_and_refusals() -> Result<()> {
        let path = workspace().join(RELATIVE);
        let valid = input();
        inventory_binding(&valid, &path, RAW)?;
        let mut changed = RAW.to_vec();
        changed[0] ^= 1;
        ensure!(
            inventory_binding(&valid, &path, &changed).is_err(),
            "changed bytes refused"
        );
        for rows in [
            json!([]),
            json!([
                valid["prerequisites"][0].clone(),
                valid["prerequisites"][0].clone()
            ]),
        ] {
            let mut bad = valid.clone();
            bad["prerequisites"] = rows;
            ensure!(
                inventory_binding(&bad, &path, RAW)
                    .err()
                    .context("count refusal")?
                    .to_string()
                    .contains("exactly one"),
                "exact count cause"
            );
        }
        let mut bad = valid.clone();
        bad["prerequisites"][0]["sha256"] = json!("0".repeat(64));
        ensure!(
            inventory_binding(&bad, &path, RAW).is_err(),
            "wrong tuple hash"
        );
        ensure!(
            inventory_binding(&valid, &path.with_file_name("other.json"), RAW).is_err(),
            "env cannot retarget"
        );
        let m: Value = serde_json::from_slice(RAW)?;
        let mut wrong = m.clone();
        wrong["nodes"][0]["producer_side"] = json!("one");
        ensure!(
            manifest_fields(&wrong)
                .err()
                .context("role refusal")?
                .to_string()
                .contains("role/account"),
            "role cause"
        );
        let mut wrong = m;
        wrong["nodes"][1]["exact_state_path"] = json!(format!("{S}\\other\\state"));
        ensure!(manifest_fields(&wrong).is_err(), "wrong private path");
        Ok(())
    }
    fn synthetic(node: &mut Value) -> Result<(Vec<u8>, Vec<u8>)> {
        let p = json!({"version":3,"profile_mode":"ordered-dialog-indexed","account":node["account"],"device":node["device"],"claim":"11".repeat(32),"source_id":"22".repeat(32),"migration_id":"33".repeat(32),"config_revision":1,"state_dir":node["exact_state_path"],"ipc_file":node["ipc"],"locator_directory":node["locators"],"logical_quota":[4096,4096],"physical_capacity":65536,"route_policy":"direct-only","enabled_contexts":[C]});
        let c = json!({"output_profile":node["profile"],"control_ipc_file":node["ipc"],"locator_directory":node["locators"],"logical_quota":[4096,4096],"physical_capacity":65536});
        let a = serde_json::to_vec(&p)?;
        let b = serde_json::to_vec(&c)?;
        node["profile_pin"] = json!({"bytes":a.len(),"sha256":sha(&a)});
        node["config_pin"] = json!({"bytes":b.len(),"sha256":sha(&b)});
        Ok((a, b))
    }
    #[test]
    fn g1a_pure_profile_bindings_and_no_create_reader() -> Result<()> {
        let mut m: Value = serde_json::from_slice(RAW)?;
        for node in m["nodes"].as_array_mut().context("nodes")? {
            let (p, c) = synthetic(node)?;
            raw_profile(node, &p, &c)?;
            let mut changed = p.clone();
            changed[0] ^= 1;
            ensure!(raw_profile(node, &changed, &c).is_err(), "raw hash refusal");
            let mut wrong: Value = serde_json::from_slice(&p)?;
            wrong["state_dir"] = json!(format!("{S}\\alias\\state"));
            let wrong = serde_json::to_vec(&wrong)?;
            let mut expected = node.clone();
            expected["profile_pin"] = json!({"bytes":wrong.len(),"sha256":sha(&wrong)});
            ensure!(
                raw_profile(&expected, &wrong, &c)
                    .err()
                    .context("path refusal")?
                    .to_string()
                    .contains("fixed path"),
                "valid bytes with wrong internal path"
            );
        }
        let base = workspace().join(".tmp");
        std::fs::create_dir_all(&base)?;
        let tmp = tempfile::Builder::new()
            .prefix("g1a-")
            .tempdir_in(base)?
            .keep();
        let path = tmp.join("profile.json");
        std::fs::write(&path, b"tiny")?;
        let (guard, raw) = read_pinned(&path, 4, &sha(b"tiny"))?;
        ensure!(raw == b"tiny", "ordinary exact reader");
        ensure!(
            std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .err()
                .context("writer unexpectedly admitted")?
                .raw_os_error()
                == Some(32),
            "actual share READ blocks writer"
        );
        drop(guard);
        ensure!(
            read_pinned(&path, 4, &"0".repeat(64))
                .err()
                .context("wrong hash admitted")?
                .to_string()
                .contains("G1 input SHA256"),
            "actual changed hash refusal"
        );
        let missing = tmp.join("missing");
        artifact(&missing, false)?;
        let locators = tmp.join("locators");
        std::fs::create_dir(&locators)?;
        artifact(&locators, true)?;
        ensure!(artifact(&locators, false).is_err(), "wrong artifact kind");
        ensure!(
            read_pinned(&missing, 4, &sha(b"tiny")).is_err() && !missing.exists(),
            "reader never creates"
        );
        Ok(())
    }
}
