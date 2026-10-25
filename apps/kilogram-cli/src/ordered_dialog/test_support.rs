//! Debug-only process scheduler. It delays real committed network records;
//! it never creates credentials, sessions, decoded messages or receiver state.
use anyhow::{Context, Result, ensure};
use kilogram_protocol::ordered_dialog_v1::{Frame, Id};
use serde::Deserialize;
use std::{path::PathBuf, time::Duration};

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Schedule {
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub hold_lane: Option<u64>,
    #[serde(default)]
    pub hold_ordinals: Vec<u64>,
    #[serde(default)]
    pub pause_point: Option<String>,
    #[serde(default)]
    pub direct_batch: Option<usize>,
    #[serde(default)]
    pub disable_ack: bool,
    #[serde(default)]
    pub delay_cursor_updates: bool,
    #[serde(default)]
    pub suspend_prefix_requests: bool,
    #[serde(default)]
    pub trace_rpc: bool,
    #[serde(default)]
    pub unsupported_suite: bool,
    #[serde(default)]
    pub conflicting_offer: bool,
    #[serde(default)]
    pub tamper_event: Option<String>,
}
fn path() -> Result<Option<PathBuf>> {
    let path = std::env::var_os("KILOGRAM_TEST_OD1_SCHEDULE");
    ensure!(
        cfg!(debug_assertions) || path.is_none(),
        "OD1 scheduler is only available in debug builds"
    );
    Ok(path.map(PathBuf::from))
}
pub fn validate(root: &std::path::Path) -> Result<()> {
    if let Some(path) = path()? {
        let path = std::fs::canonicalize(path)?;
        ensure!(
            !path.starts_with(std::fs::canonicalize(root)?),
            "scheduler must be outside the device root"
        );
        load()?;
    }
    for name in [
        "KILOGRAM_TEST_OD1_CLOCK_FILE",
        "KILOGRAM_TEST_OD1_PRIMARY_PAUSE_FILE",
    ] {
        if let Some(path) = std::env::var_os(name) {
            ensure!(cfg!(debug_assertions), "OD1 test hooks are debug only");
            ensure!(
                !std::fs::canonicalize(path)?.starts_with(std::fs::canonicalize(root)?),
                "test hook must be outside root"
            );
        }
    }
    Ok(())
}
pub fn load() -> Result<Schedule> {
    let Some(path) = path()? else {
        return Ok(Schedule::default());
    };
    let raw = super::admin::read_public(&path, 16 * 1024)?;
    let value: Schedule = serde_json::from_slice(&raw)?;
    ensure!(
        value.hold_ordinals.len() <= 128
            && value.hold_ordinals.iter().all(|n| *n < 512)
            && value.hold_lane.is_none_or(|lane| lane <= 1)
            && value.direct_batch.is_none_or(|n| n > 0 && n <= 32),
        "scheduler bounds"
    );
    Ok(value)
}
impl Schedule {
    pub fn applies(&self, c: Id) -> bool {
        self.context
            .as_ref()
            .is_none_or(|s| *s == super::state::hex(&c))
    }
    pub fn held(&self, c: Id, lane: u64, n: u64) -> bool {
        self.applies(c) && self.hold_lane == Some(lane) && self.hold_ordinals.contains(&n)
    }
}
pub fn delivery(frame: Frame, signer: &kilogram_identity::DeviceIdentity) -> Result<Frame> {
    let plan = load()?;
    let f = frame.body.fields(6)?;
    if !plan.applies(f[0].fixed()?) || plan.tamper_event.is_none() {
        return Ok(frame);
    }
    let mut fields = f.to_vec();
    let mut events = Vec::new();
    for raw in f[5].list()? {
        let mut event =
            kilogram_protocol::ordered_dialog_v1::Event::parse(raw.bytes()?, signer.device_id())?;
        match plan.tamper_event.as_deref() {
            Some("missing-auth") => {
                event.core.author_auth = [99; 32];
                fields[4] = kilogram_protocol::ordered_dialog_v1::a([]);
            }
            Some("cross-context") => event.core.c = [98; 32],
            _ => anyhow::bail!("unsupported malicious-signer schedule"),
        }
        event.sign(signer)?;
        events.push(kilogram_protocol::ordered_dialog_v1::b(event.raw()?));
    }
    fields[5] = kilogram_protocol::ordered_dialog_v1::a(events);
    Ok(Frame::signed(
        frame.kind,
        frame.binding,
        frame.request,
        kilogram_protocol::ordered_dialog_v1::a(fields),
        signer,
    )?)
}
pub fn checkpoint(name: &str) -> Result<()> {
    let Some(path) = path()? else { return Ok(()) };
    if load()?.pause_point.as_deref() != Some(name) {
        return Ok(());
    }
    let hit = path.with_extension("hit");
    let mut marker = tempfile::NamedTempFile::new_in(hit.parent().context("marker parent")?)?;
    std::io::Write::write_all(marker.as_file_mut(), name.as_bytes())?;
    marker.as_file().sync_all()?;
    marker.persist(&hit)?;
    // The acceptance parent kills the real child at the durable marker. A
    // release is useful for injected-error tests without killing the process.
    while load()?.pause_point.as_deref() == Some(name) {
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}
pub async fn before_write(frame: &Frame) -> Result<()> {
    if frame.kind != 12 {
        return Ok(());
    }
    let f = frame.body.fields(10)?;
    let c = f[0].fixed()?;
    let lane = f[3].uint()?;
    loop {
        let plan = load()?;
        let start = f[4].uint()?;
        let end = f[5].uint()?;
        if !(start..end).any(|n| plan.held(c, lane, n)) {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
pub fn timing(c: Id, elapsed: Duration, delta: u64, generation: u64, native: bool) {
    if std::env::var_os("KILOGRAM_TEST_OD1_SCHEDULE").is_some() {
        eprintln!(
            "od1_commit context={} generation={} delta_bytes={} microseconds={} native={}",
            super::state::hex(&c),
            generation,
            delta,
            elapsed.as_micros(),
            native
        );
    }
}
