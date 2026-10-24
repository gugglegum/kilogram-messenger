//! Bounded, test-only, opt-in profiling. Timings are inclusive and never added together.
use super::Id;
use serde::Serialize;
use std::{cell::RefCell, collections::BTreeSet, time::Instant};

#[derive(Clone, Copy)]
#[repr(usize)]
pub(super) enum Phase {
    ValueGet,
    TreeFind,
    NodeOpen,
    NodeProof,
    Prefix,
    Common,
    ValueOpen,
    ValueCheck,
    ProjectionDecode,
    BindingDecode,
    IdentityHeadsDecode,
    SourceCheck,
    Cbor,
    Json,
    WriterValidation,
    NodeSeal,
    ValueSeal,
    HistoryApply,
    CommitWrite,
}
pub(super) const PHASE_NAMES: [&str; 19] = [
    "value_get",
    "tree_find",
    "node_open",
    "node_proof",
    "prefix",
    "common",
    "value_open",
    "value_check",
    "projection_decode",
    "binding_decode",
    "identity_heads_decode",
    "source_check",
    "cbor",
    "json",
    "writer_validation",
    "node_seal",
    "value_seal",
    "history_apply",
    "commit_write",
];
#[derive(Clone, Copy, Default, Serialize)]
pub(super) struct Time {
    pub calls: u64,
    pub nanos: u64,
}
#[derive(Clone, Copy, Default, Serialize)]
pub(super) struct NamespaceProfile {
    pub gets: u64,
    pub repeated_gets: u64,
    pub opens: u64,
    pub cipher_bytes: u64,
}
#[derive(Clone, Serialize)]
pub(super) struct Stats {
    pub phases: [Time; 19],
    pub namespaces: [NamespaceProfile; 21],
    pub works: u64,
    pub key_tracking_truncated: bool,
    pub cache_hits: u64,
    pub negative_cache_hits: u64,
}
impl Default for Stats {
    fn default() -> Self {
        Self {
            phases: [Time::default(); 19],
            namespaces: [NamespaceProfile::default(); 21],
            works: 0,
            key_tracking_truncated: false,
            cache_hits: 0,
            negative_cache_hits: 0,
        }
    }
}
impl Stats {
    pub fn add(&mut self, other: Self) {
        for (a, b) in self.phases.iter_mut().zip(other.phases) {
            a.calls += b.calls;
            a.nanos += b.nanos;
        }
        for (a, b) in self.namespaces.iter_mut().zip(other.namespaces) {
            a.gets += b.gets;
            a.repeated_gets += b.repeated_gets;
            a.opens += b.opens;
            a.cipher_bytes += b.cipher_bytes;
        }
        self.works += other.works;
        self.key_tracking_truncated |= other.key_tracking_truncated;
        self.cache_hits += other.cache_hits;
        self.negative_cache_hits += other.negative_cache_hits;
    }
}
#[derive(Default)]
struct State {
    enabled: bool,
    stats: Stats,
    seen: BTreeSet<(u64, u8, Id)>,
}
thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}

pub(super) fn measure<T>(phase: Phase, operation: impl FnOnce() -> T) -> T {
    let enabled = STATE.with(|s| s.borrow().enabled);
    if !enabled {
        return operation();
    }
    let start = Instant::now();
    let result = operation();
    let nanos = start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let time = &mut s.stats.phases[phase as usize];
        time.calls += 1;
        time.nanos += nanos;
    });
    result
}
pub(super) fn new_work() -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.enabled {
            s.stats.works += 1;
            s.stats.works
        } else {
            0
        }
    })
}
pub(super) fn get(work: u64, slot: u8, key: Id) {
    if work == 0 {
        return;
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let ns = if slot < 48 {
            usize::from(slot % 24)
        } else {
            17 + usize::from(slot - 48)
        };
        if ns >= 21 {
            return;
        }
        s.stats.namespaces[ns].gets += 1;
        if s.seen.contains(&(work, slot, key)) {
            s.stats.namespaces[ns].repeated_gets += 1;
        } else if s.seen.len() < 4096 {
            s.seen.insert((work, slot, key));
        } else {
            s.stats.key_tracking_truncated = true;
        }
    });
}
pub(super) fn value_open(ns: usize, bytes: usize) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.enabled {
            s.stats.namespaces[ns].opens += 1;
            s.stats.namespaces[ns].cipher_bytes += bytes as u64;
        }
    });
}
pub(super) fn capture<T>(operation: impl FnOnce() -> T) -> (T, Stats) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            STATE.with(|s| s.borrow_mut().enabled = false);
        }
    }
    STATE.with(|s| {
        *s.borrow_mut() = State {
            enabled: true,
            ..State::default()
        }
    });
    let reset = Reset;
    let result = operation();
    let stats = STATE.with(|s| s.borrow().stats.clone());
    drop(reset);
    (result, stats)
}
pub(super) fn json(stats: Stats) -> serde_json::Value {
    let timings: serde_json::Map<_, _> = PHASE_NAMES
        .iter()
        .zip(stats.phases)
        .map(|(name, time)| {
            (
                (*name).to_owned(),
                serde_json::json!({"calls":time.calls,"ms":time.nanos as f64/1e6}),
            )
        })
        .collect();
    serde_json::json!({"inclusive_timings":timings,"namespaces":stats.namespaces,"works":stats.works,
        "key_tracking_cap":4096,"key_tracking_truncated":stats.key_tracking_truncated,
        "cache_hits":stats.cache_hits,"negative_cache_hits":stats.negative_cache_hits})
}
