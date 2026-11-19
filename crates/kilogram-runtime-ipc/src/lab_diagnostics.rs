//! Opt-in, local lab receipts. Never a protocol input or an application success condition.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub const DIRECTORY: &str = "lab-diagnostics-v1";
pub const MARKER: &str = "enabled.json";
pub const RECORD_CAP: usize = 16 * 1024;
const FILE_CAP: usize = 16;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Marker {
    schema: u8,
    run_nonce: String,
}

#[derive(Clone)]
pub struct Sink {
    directory: PathBuf,
    nonce: String,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    RuntimeStopping,
    RuntimeNetworkStopped,
    RuntimeFinal,
    ConnectionFinal,
    WorkerFinal,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::RuntimeStopping => "runtime-stopping",
            Self::RuntimeNetworkStopped => "runtime-network-stopped",
            Self::RuntimeFinal => "runtime-final",
            Self::ConnectionFinal => "connection-final",
            Self::WorkerFinal => "worker-final",
        }
    }
}

fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
}
fn plain(path: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if metadata.file_type().is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    true
}
impl Sink {
    /// The harness creates the marker only in a fresh, owned Setup directory.
    /// Missing/invalid/inaccessible diagnostics never change application behavior.
    pub fn open(home: &Path) -> Option<Self> {
        let directory = home.join(DIRECTORY);
        for parent in directory.ancestors() {
            if !plain(parent) {
                return None;
            }
        }
        if !directory.is_dir()
            || fs::read_dir(&directory).ok()?.take(FILE_CAP + 1).count() > FILE_CAP
        {
            return None;
        }
        let marker = directory.join(MARKER);
        if !plain(&marker) || fs::metadata(&marker).ok()?.len() > 256 {
            return None;
        }
        let marker: Marker = serde_json::from_slice(&fs::read(marker).ok()?).ok()?;
        if marker.schema != 1 || !hex(&marker.run_nonce, 32) {
            return None;
        }
        Some(Self {
            directory,
            nonce: marker.run_nonce,
        })
    }

    /// Callers supply fixed typed booleans/counters/enums, never errors or frame bytes.
    /// One immutable file per kind/PID; no arbitrary destination or unbounded append.
    pub fn record<T: Serialize>(
        &self,
        kind: Kind,
        runtime_pid: Option<u32>,
        instance: Option<&str>,
        data: &T,
    ) -> bool {
        if instance.is_some_and(|v| !hex(v, 64))
            || !Self::open(self.directory.parent().unwrap_or(Path::new("")))
                .is_some_and(|v| v.nonce == self.nonce)
        {
            return false;
        }
        #[derive(Serialize)]
        struct Envelope<'a, T> {
            schema: u8,
            kind: Kind,
            run_nonce: &'a str,
            pid: u32,
            runtime_pid: Option<u32>,
            runtime_instance: Option<&'a str>,
            data: &'a T,
        }
        let record = Envelope {
            schema: 1,
            kind,
            run_nonce: &self.nonce,
            pid: std::process::id(),
            runtime_pid,
            runtime_instance: instance,
            data,
        };
        let Ok(bytes) = serde_json::to_vec(&record) else {
            return false;
        };
        if bytes.len() > RECORD_CAP {
            return false;
        }
        let path = self
            .directory
            .join(format!("{}-{}.json", kind.name(), std::process::id()));
        let Ok(mut file) = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
        else {
            return false;
        };
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn t13diag_marker_receipt_and_caps() -> anyhow::Result<()> {
        let home = tempfile::tempdir()?;
        assert!(Sink::open(home.path()).is_none());
        let root = home.path().join(DIRECTORY);
        fs::create_dir(&root)?;
        fs::write(
            root.join(MARKER),
            br#"{"schema":1,"run_nonce":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
        )?;
        let sink = Sink::open(home.path()).expect("valid lab marker");
        assert!(sink.record(
            Kind::ConnectionFinal,
            Some(42),
            Some(&"ab".repeat(32)),
            &true
        ));
        assert!(!sink.record(Kind::ConnectionFinal, Some(42), None, &false));
        assert!(!sink.record(
            Kind::WorkerFinal,
            None,
            Some("not-a-public-instance"),
            &true
        ));
        assert!(!sink.record(Kind::WorkerFinal, None, None, &"x".repeat(RECORD_CAP)));
        fs::write(
            root.join(MARKER),
            br#"{"schema":1,"run_nonce":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}"#,
        )?;
        assert!(Sink::open(home.path()).is_some());
        assert!(!sink.record(Kind::RuntimeFinal, None, None, &true));
        fs::write(
            root.join(MARKER),
            br#"{"schema":1,"run_nonce":"bad","extra":"invalid"}"#,
        )?;
        assert!(Sink::open(home.path()).is_none());
        assert!(!sink.record(Kind::RuntimeFinal, None, None, &true));
        Ok(())
    }
}
