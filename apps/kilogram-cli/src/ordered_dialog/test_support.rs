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
    pub ack_query_batch: Option<usize>,
    #[serde(default)]
    pub suspend_materialization: bool,
    #[serde(default)]
    pub ready_fault: Option<String>,
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
    ensure!(
        value.ack_query_batch.is_none_or(|n| (1..=32).contains(&n)),
        "ACK scheduler cap"
    );
    ensure!(
        value.ready_fault.as_deref().is_none_or(|v| [
            "arity",
            "d",
            "e",
            "sid",
            "role",
            "suite",
            "format",
            "genesis-shape",
            "genesis-conflict"
        ]
        .contains(&v)),
        "Ready scheduler case"
    );
    Ok(value)
}

/// An actual authorized debug peer signs a faulty Ready for negative ingress
/// coverage. This changes transmission only, never stored controls or Native.
pub fn ready_frame(frame: Frame, signer: &kilogram_identity::DeviceIdentity) -> Result<Frame> {
    if frame.kind != 8 {
        return Ok(frame);
    }
    let plan = load()?;
    ready_frame_planned(frame, signer, &plan)
}
fn ready_frame_planned(
    frame: Frame,
    signer: &kilogram_identity::DeviceIdentity,
    plan: &Schedule,
) -> Result<Frame> {
    let outer = frame.body.fields(4)?;
    if !plan.applies(outer[0].fixed()?) || plan.ready_fault.is_none() {
        return Ok(frame);
    }
    use kilogram_protocol::ordered_dialog_v1::{self as wire, a, b, u};
    let body = wire::control(outer[3].bytes()?, 3, signer.device_id())?;
    let mut fields = body.fields(8)?.to_vec();
    match plan.ready_fault.as_deref() {
        Some("arity") => {
            fields.pop();
        }
        Some("d") => fields[0] = b([99; 32]),
        Some("e") => fields[1] = b([99; 32]),
        Some("sid") => fields[2] = b("wrong-scope"),
        Some("role") => fields[5] = u(1 - fields[5].uint()?),
        Some("suite") => fields[6] = u(2),
        Some("format") => fields[7] = u(2),
        Some("genesis-shape") => fields[3] = b([99; 31]),
        Some("genesis-conflict") => fields[3] = b([99; 32]),
        _ => anyhow::bail!("Ready scheduler case"),
    }
    let mut outer = outer.to_vec();
    let body = a(fields);
    let control = if plan.ready_fault.as_deref() == Some("arity") {
        // The ordinary signer correctly refuses malformed shape. Only this
        // debug peer's fixed negative case signs the exact seven-field body.
        let unsigned = a([u(1), u(3), body.clone()]);
        let bytes = wire::signing_bytes(wire::CONTROL_SIGN, &unsigned, 32 * 1024)?;
        let signature = signer.sign(&bytes);
        signer.device_id().verify(&bytes, &signature)?;
        wire::encode(&a([u(1), u(3), body, b(signature)]), 32 * 1024)?
    } else {
        wire::signed_control(3, body, signer)?
    };
    outer[3] = b(control);
    Ok(Frame::signed(
        frame.kind,
        frame.binding,
        frame.request,
        a(outer),
        signer,
    )?)
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
            Some("invalid-cipher") => {
                let last = event
                    .ciphertext
                    .last_mut()
                    .context("actual ciphertext missing")?;
                *last ^= 1;
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use kilogram_identity::DeviceIdentity;
    use kilogram_protocol::ordered_dialog_v1::{self as wire, a, b, u};

    #[test]
    fn indexed_l1_ready_arity_fixture() -> Result<()> {
        let signer = DeviceIdentity::from_secret_bytes([7; 32]);
        let binding = [8; 32];
        let body = a([
            b([1; 32]),
            b([2; 32]),
            b("fixture-sid"),
            b([3; 32]),
            b([4; 32]),
            u(0),
            u(1),
            u(1),
        ]);
        let control = wire::signed_control(3, body.clone(), &signer)?;
        let normal = Frame::signed(
            8,
            binding,
            [9; 16],
            a([b([5; 32]), b([1; 32]), b([2; 32]), b(control)]),
            &signer,
        )?;
        let malformed = ready_frame_planned(
            normal.clone(),
            &signer,
            &Schedule {
                ready_fault: Some("arity".into()),
                ..Schedule::default()
            },
        )?;
        let parsed = Frame::parse(&malformed.raw()?)?;
        parsed.verify(binding, signer.device_id())?;
        ensure!(
            parsed.request == normal.request,
            "original request retained"
        );
        let raw = parsed.body.fields(4)?[3].bytes()?;
        let inner = wire::decode(raw, 32 * 1024)?;
        let f = inner.fields(4)?;
        ensure!(
            f[0].uint()? == 1 && f[1].uint()? == 3 && f[2].list()?.len() == 7,
            "actual malformed Ready shape"
        );
        signer.device_id().verify(
            &wire::signing_bytes(wire::CONTROL_SIGN, &a(f[..3].iter().cloned()), 32 * 1024)?,
            f[3].bytes()?,
        )?;
        let refusal = wire::control(raw, 3, signer.device_id())
            .err()
            .context("malformed control unexpectedly accepted")?;
        ensure!(
            refusal.to_string().contains("array arity"),
            "unchanged production control parser must refuse arity"
        );
        let unchanged = ready_frame_planned(normal.clone(), &signer, &Schedule::default())?;
        ensure!(unchanged.raw()? == normal.raw()?, "normal frame changed");
        let parsed = Frame::parse(&unchanged.raw()?)?;
        parsed.verify(binding, signer.device_id())?;
        ensure!(
            wire::control(parsed.body.fields(4)?[3].bytes()?, 3, signer.device_id())? == body,
            "normal control must retain normal behavior"
        );
        Ok(())
    }
}
