//! Debug verification controls. They delay/kill real operations; they create no native records.
use super::*;
use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Schedule2 {
    pub slot: Option<u8>,
    pub pause: Option<String>,
    pub clock: Option<u64>,
    #[serde(default)]
    pub clock_failure: bool,
    #[serde(default)]
    pub disable_ack: bool,
    #[serde(default)]
    pub suspend_prefix: bool,
    #[serde(default)]
    pub hold_ordinals: Vec<u64>,
    pub hold_lane: Option<u8>,
    #[serde(default)]
    pub hold_window64: bool,
    #[serde(default)]
    pub suspend_live: bool,
    pub pause_prefix_page: Option<u64>,
    #[serde(default)]
    pub authority_read_failure: bool,
    #[serde(default)]
    pub operational_after_trial: bool,
    pub operational_after_trial_number: Option<u8>,
    #[serde(default)]
    pub suspend_drain: bool,
    pub lease_seconds: Option<u64>,
    pub io_fault: Option<String>,
    pub io_operation: Option<String>,
    pub io_token: Option<String>,
    #[serde(default)]
    pub io_after: u64,
}
fn path() -> Result<Option<PathBuf>> {
    let path = std::env::var_os("KILOGRAM_TEST_OD2_SCHEDULE");
    ensure!(
        cfg!(debug_assertions) || path.is_none(),
        "OD2 verification controls require debug assertions"
    );
    Ok(path.map(PathBuf::from))
}
pub(super) fn validate(root: &Path) -> Result<()> {
    if let Some(path) = path()? {
        ensure!(
            !path.canonicalize()?.starts_with(root.canonicalize()?),
            "verification controls inside state"
        );
        load()?;
    }
    Ok(())
}
pub(super) fn load() -> Result<Schedule2> {
    let _timing = timing::span(timing::Metric::HooksLoad);
    let Some(path) = path()? else {
        return Ok(Schedule2::default());
    };
    let raw = {
        let _timing = timing::span(timing::Metric::PublicRead);
        shared_admin::read_public(&path, 16 * 1024)?
    };
    let schedule: Schedule2 = serde_json::from_slice(&raw)?;
    ensure!(
        schedule.slot.is_none_or(|c| c < 2)
            && schedule.hold_lane.is_none_or(|c| c < 2)
            && schedule.hold_ordinals.len() <= 128
            && schedule.pause.as_ref().is_none_or(|s| s.len() <= 128)
            && schedule.io_token.as_ref().is_none_or(|s| s.len() <= 64)
            && schedule.io_fault.as_ref().is_none_or(|s| s.len() <= 32)
            && schedule.io_operation.as_ref().is_none_or(|s| s.len() <= 64)
            && schedule
                .operational_after_trial_number
                .is_none_or(|n| (1..=4).contains(&n))
            && schedule
                .lease_seconds
                .is_none_or(|n| (1..=300).contains(&n)),
        "verification control bounds"
    );
    Ok(schedule)
}
impl Schedule2 {
    pub fn applies(&self, c: u8) -> bool {
        self.slot.is_none_or(|slot| slot == c)
    }
    pub fn held(&self, c: u8, lane: u8, n: u64) -> bool {
        self.applies(c) && self.hold_lane == Some(lane) && self.hold_ordinals.contains(&n)
    }
}
pub(super) fn clock() -> Result<Option<u64>> {
    let schedule = load()?;
    ensure!(
        !schedule.clock_failure,
        "ClockBlocked: injected fallible clock"
    );
    Ok(schedule.clock)
}
pub(super) fn after_native_trial(c: u8, number: u8) -> Result<()> {
    let schedule = load()?;
    if schedule.applies(c)
        && (schedule.operational_after_trial
            || schedule.operational_after_trial_number == Some(number))
    {
        return Err(kilogram_ratchet::ordered::NativeFailure::Operational(
            "verification failure after actual Native trial",
        )
        .into());
    }
    Ok(())
}
pub(super) fn checkpoint(name: &str, c: Option<u8>, public: serde_json::Value) -> Result<()> {
    let Some(path) = path()? else { return Ok(()) };
    let schedule = load()?;
    if schedule.pause.as_deref() != Some(name) || c.is_some_and(|c| !schedule.applies(c)) {
        return Ok(());
    }
    let hit = path.with_extension("hit");
    std::fs::write(
        &hit,
        serde_json::to_vec(
            &serde_json::json!({"pid":std::process::id(),"point":name,"slot":c,"public":public}),
        )?,
    )?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(&hit)?
        .sync_all()?;
    while {
        let s = load()?;
        s.pause.as_deref() == Some(name) && c.is_none_or(|c| s.applies(c))
    } {
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}
pub(super) async fn transport_barrier(c: u8, kind: wire::FrameKind, ids: &[Id]) -> Result<()> {
    let Some(path) = path()? else { return Ok(()) };
    let blocked = |s: &Schedule2| {
        s.applies(c)
            && ((s.hold_window64 && ids.len() == 64)
                || (s.disable_ack && matches!(kind, wire::FrameKind::AckBatch) && !ids.is_empty()))
    };
    if !blocked(&load()?) {
        return Ok(());
    }
    let hit = path.with_extension("transport-hit");
    std::fs::write(
        &hit,
        serde_json::to_vec(
            &serde_json::json!({"pid":std::process::id(),"slot":c,"kind":kind as u8,"count":ids.len(),"ids":ids.iter().map(|id|hex(id)).collect::<Vec<_>>()}),
        )?,
    )?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(hit)?
        .sync_all()?;
    while blocked(&load()?) {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Ok(())
}
pub(super) fn prefix_saved(c: u8, page: u64, public: serde_json::Value) -> Result<()> {
    let Some(path) = path()? else { return Ok(()) };
    let s = load()?;
    if !s.applies(c) || s.pause_prefix_page != Some(page) {
        return Ok(());
    }
    let hit = path.with_extension("prefix-hit");
    std::fs::write(
        &hit,
        serde_json::to_vec(
            &serde_json::json!({"pid":std::process::id(),"slot":c,"page":page,"public":public}),
        )?,
    )?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(&hit)?
        .sync_all()?;
    while {
        let s = load()?;
        s.applies(c) && s.pause_prefix_page == Some(page)
    } {
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}
