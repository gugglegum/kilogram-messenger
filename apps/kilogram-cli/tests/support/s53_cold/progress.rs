//! Fixed protected progress for the Cold-B diagnostic controller only.
use super::*;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    os::windows::fs::OpenOptionsExt,
};
pub struct Progress {
    file: File,
    started: Instant,
    until: Instant,
    records: usize,
    bytes: usize,
    side: Option<usize>,
    pass: u8,
}
impl Progress {
    pub fn fixed(until: Instant) -> Result<Self> {
        Self::create(
            &Path::new(fixed::EVIDENCE).join("progress.private.jsonl"),
            until,
        )
    }
    fn create(path: &Path, until: Instant) -> Result<Self> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(0)
            .open(path)?;
        Ok(Self {
            file,
            started: Instant::now(),
            until,
            records: 0,
            bytes: 0,
            side: None,
            pass: 0,
        })
    }
    pub fn context(&mut self, side: usize, pass: u8) {
        self.side = Some(side);
        self.pass = pass;
    }
    pub fn record(&mut self, phase: &'static str, completed: usize) -> Result<()> {
        readers::check(self.until)?;
        let raw = serde_json::to_vec(
            &json!({"phase":phase,"elapsed_ms":self.started.elapsed().as_millis(),
            "side":self.side,"pass":self.pass,"completed":completed}),
        )?;
        ensure!(
            raw.len() <= 512 && self.records < 256 && self.bytes + raw.len() < 65536,
            "protected progress cap"
        );
        self.file.write_all(&raw)?;
        self.file.write_all(b"\n")?;
        self.file.sync_all()?;
        self.records += 1;
        self.bytes += raw.len() + 1;
        Ok(())
    }
    pub fn pair(&mut self) {
        self.side = None;
        self.pass = 0;
    }
}
#[test]
fn cold_b1_progress_is_bounded_create_new_and_durable() -> Result<()> {
    let base = Path::new(fixed::WORKSPACE).join(".tmp/C-s53-cold-b1-synthetic");
    std::fs::create_dir_all(&base)?;
    let dir = tempfile::Builder::new()
        .prefix("progress-")
        .tempdir_in(base)?
        .keep();
    let path = dir.join("progress.jsonl");
    let until = Instant::now() + Duration::from_secs(60);
    let mut progress = Progress::create(&path, until)?;
    progress.context(1, 2);
    progress.record("source", 64)?;
    ensure!(
        progress.file.metadata()?.len() > 0,
        "progress persists before close"
    );
    ensure!(
        Progress::create(&path, until).is_err(),
        "progress collision refuses"
    );
    for n in 1..256 {
        progress.record("source", n)?;
    }
    let error = progress
        .record("source", 257)
        .err()
        .context("progress cap not enforced")?;
    ensure!(
        error.to_string() == "protected progress cap",
        "exact progress cap"
    );
    drop(progress);
    let raw = std::fs::read(&path)?;
    ensure!(raw.len() <= 65536, "progress byte cap");
    let text = std::str::from_utf8(&raw)?;
    ensure!(
        text.lines().count() == 256,
        "complete persisted progress records"
    );
    for line in text.lines() {
        let row: Value = serde_json::from_str(line)?;
        ensure!(row["side"] == 1 && row["pass"] == 2, "progress context");
        ensure!(
            row.as_object()
                .context("progress object")?
                .keys()
                .all(|key| ["phase", "elapsed_ms", "side", "pass", "completed"]
                    .contains(&key.as_str())),
            "progress contains only fixed aggregate fields"
        );
    }
    Ok(())
}
