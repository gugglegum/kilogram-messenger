#![cfg(windows)]
//! Offline Cold-B readers; no ordinary runtime entrypoint.
use anyhow::{Context, Result, ensure};
use kilogram_cli::ordered_dialog::{
    handshake::Spec,
    runtime7::Profile3,
    state::{self, Head},
};
use kilogram_protocol::{ordered_dialog_v1 as old, ordered_dialog_v2 as wire};
use kilogram_state::indexed_dialog::{self as db, migration7::live1 as store};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
#[path = "support/s53_cold/checks.rs"]
mod checks;
#[path = "support/s53_cold/fixed.rs"]
mod fixed;
#[path = "support/s53_cold/history.rs"]
mod history;
#[path = "support/s53_cold/readers.rs"]
mod readers;
#[path = "support/s53_cold/session.rs"]
mod session;
