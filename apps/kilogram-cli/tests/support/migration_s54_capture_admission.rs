//! Admission is a rendezvous with the existing independent outer job owner.
//! The JSON alone is not containment proof: verification also requires the
//! wrapper's actual process-handle membership and final empty-job receipt.
use super::*;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Admission {
    pub version: u8,
    pub attempt: String,
    pub owner_pid: u32,
    pub launcher_pid: u32,
    pub harness_path: PathBuf,
    pub whole_utc_ms: u64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Permit {
    attempt: String,
    owner_pid: u32,
    harness_pid: u32,
    nonce: String,
    membership_verified: bool,
}
fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    use std::io::Read;
    let mut raw = Vec::with_capacity(4097);
    File::open(path)
        .and_then(|file| file.take(4097).read_to_end(&mut raw))
        .map_err(|_| anyhow::anyhow!("capture admission read"))?;
    ensure!(raw.len() <= 4096, "capture admission cap");
    serde_json::from_slice(&raw).map_err(|_| anyhow::anyhow!("capture admission schema"))
}
impl Admission {
    pub(super) fn load(until: Instant) -> Result<(Self, Instant)> {
        let path = std::env::var_os("KILOGRAM_S54_CAPTURE_ADMISSION")
            .context("capture requires independently managed launch")?;
        let path = PathBuf::from(path)
            .canonicalize()
            .map_err(|_| anyhow::anyhow!("capture admission path"))?;
        let owned = workspace().canonicalize()?.join(".tmp");
        ensure!(path.starts_with(owned), "capture owned admission path");
        let admission: Self = read(&path)?;
        ensure!(
            admission.version == 1
                && !admission.attempt.is_empty()
                && admission.attempt.len() <= 80
                && admission
                    .attempt
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                && admission.owner_pid != 0
                && admission.launcher_pid != 0
                && admission.owner_pid != std::process::id()
                && admission.launcher_pid != std::process::id()
                && std::env::current_exe()?.canonicalize()?
                    == admission.harness_path.canonicalize()?,
            "capture admission binding"
        );
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
        ensure!(
            u128::from(admission.whole_utc_ms) > now,
            "expired capture containment"
        );
        let remaining = u64::try_from(u128::from(admission.whole_utc_ms) - now)?;
        let whole = Instant::now() + Duration::from_millis(remaining);
        let until = until.min(whole);
        let nonce = format!("{}-{now}", std::process::id());
        let parent = path.parent().context("capture admission parent")?;
        let request = parent.join(format!("join-{}.json", std::process::id()));
        let permit_path = parent.join(format!("permit-{}.json", std::process::id()));
        let permit = Permit {
            attempt: admission.attempt.clone(),
            owner_pid: admission.owner_pid,
            harness_pid: std::process::id(),
            nonce,
            membership_verified: false,
        };
        let raw = serde_json::to_vec(&permit)?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(request)?;
        use std::io::Write;
        file.write_all(&raw)?;
        drop(file);
        // An actual handle check by the ancestor must precede any capture spawn.
        loop {
            ensure!(
                Instant::now() < until,
                "capture membership rendezvous deadline"
            );
            if permit_path.exists() {
                let observed: Permit = read(&permit_path)?;
                ensure!(
                    observed.attempt == permit.attempt
                        && observed.owner_pid == permit.owner_pid
                        && observed.harness_pid == permit.harness_pid
                        && observed.nonce == permit.nonce
                        && observed.membership_verified,
                    "capture membership permit binding"
                );
                return Ok((admission, whole));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
