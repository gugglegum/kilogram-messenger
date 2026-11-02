//! Closed Windows query: exact shell identity, original Child live bracket,
//! A7 bounded transport/sequence/Job admission. No PATH/process enumeration.
use super::*;
use sha2::{Digest, Sha256};
use std::io::Read;
#[derive(Clone)]
pub(crate) struct Shell {
    path: PathBuf,
    bytes: u64,
    sha256: String,
}
impl Shell {
    pub(crate) fn validated(path: &Path, bytes: u64, sha256: &str) -> Result<Self> {
        let s = Self {
            path: path.into(),
            bytes,
            sha256: sha256.into(),
        };
        s.verify()?;
        Ok(s)
    }
    pub(super) fn verify(&self) -> Result<()> {
        ensure!(
            cfg!(all(windows, target_pointer_width = "64")),
            "memory x64 Windows shell"
        );
        let expected = Path::new("C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe");
        ensure!(
            self.path.is_absolute()
                && self.path.canonicalize()? == expected.canonicalize()?
                && self.bytes > 0
                && self.sha256.len() == 64
                && self
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
            "memory admitted shell descriptor"
        );
        let metadata = std::fs::symlink_metadata(&self.path)?;
        ensure!(
            metadata.is_file()
                && metadata.len() == self.bytes
                && !metadata.file_type().is_symlink(),
            "memory shell metadata"
        );
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            ensure!(
                metadata.file_attributes() & 0x400 == 0,
                "memory shell reparse point"
            );
        }
        for dir in [
            workspace(),
            workspace().join("target/verification-scale/deps"),
        ] {
            ensure!(
                !dir.join("powershell.exe").exists(),
                "memory local shell shadow"
            );
        }
        let mut file = std::fs::File::open(&self.path)?;
        let mut hash = Sha256::new();
        let mut chunk = [0; 8192];
        loop {
            let n = file.read(&mut chunk)?;
            if n == 0 {
                break;
            }
            hash.update(&chunk[..n]);
        }
        ensure!(
            format!("{:x}", hash.finalize()) == self.sha256,
            "memory shell hash"
        );
        Ok(())
    }
}
pub(super) fn parse(raw: &[u8], pid: u32) -> Result<Value> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Wire {
        pid: u32,
        ticks: u64,
        bytes: u64,
    }
    ensure!(raw.len() <= 4096, "memory wire cap");
    let v: Wire = serde_json::from_slice(raw).map_err(|_| anyhow::anyhow!("memory wire schema"))?;
    ensure!(
        v.pid == pid && pid > 0 && v.ticks > 0 && v.bytes > 0,
        "memory wire identity/value"
    );
    Ok(json!({"pid":v.pid,"ticks":v.ticks,"bytes":v.bytes}))
}
pub(super) fn known(shell: &Shell, child: &mut Child, pid: u32) -> Result<Reading> {
    let until = Instant::now() + Duration::from_secs(2);
    s53_deadline::check()?;
    ensure!(
        child.id() == pid && child.try_wait()?.is_none(),
        "memory original child live before query"
    );
    shell.verify()?;
    ensure!(Instant::now() < until, "memory query verification deadline");
    let v = s54_capture::memory_query(&shell.path, pid, until)?;
    ensure!(
        child.try_wait()?.is_none(),
        "memory original child live after query"
    );
    shell.verify()?;
    s53_deadline::check()?;
    ensure!(Instant::now() < until, "memory query total work ceiling");
    Ok(Reading {
        pid,
        ticks: v["ticks"].as_u64().context("memory parsed ticks")?,
        bytes: v["bytes"].as_u64().context("memory parsed bytes")?,
        evidence: Evidence::WindowsChildBracket,
    })
}
