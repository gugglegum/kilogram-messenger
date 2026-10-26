//! Fresh production OD2. One owner; no legacy Engine or component writer fallback.
use super::{
    admin as shared_admin,
    state::{hex, parse_id},
};
use anyhow::{Context, Result, ensure};
use kilogram_protocol::ordered_dialog_v2 as wire;
use kilogram_state::indexed_dialog as db;
use wire::{Id, Value, a, b, opt, u};
pub mod admin;
mod admission;
pub mod config;
mod data;
mod handshake;
mod hooks;
mod native;
mod network;
pub mod owner;
mod policy;
mod public_import;
mod resources;
pub mod runtime;
mod timing;
pub(super) mod window;

pub fn now() -> Result<u64> {
    if let Some(wall) = hooks::clock()? {
        return Ok(wall);
    }
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("ClockBlocked")?
        .as_secs())
}
fn auth(xs: &[&wire::RootVerifiedCredential]) -> Vec<db::HistoryAuth> {
    xs.iter()
        .map(|c| (c.reference(), c.raw().to_vec()))
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_iter()
        .map(|(id, bytes)| db::HistoryAuth { id, bytes })
        .collect()
}
