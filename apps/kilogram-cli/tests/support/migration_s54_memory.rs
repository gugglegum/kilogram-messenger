//! A8a core only. No shared profile lifecycle/status hooks until A8b.
use super::*;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
#[path = "migration_s54_memory_query.rs"]
pub(super) mod query;
#[path = "migration_s54_memory_tests.rs"]
mod tests;

type Id = [u8; 32];
const SPAN: u64 = 2_400_000;
const CADENCE: u64 = 5_000;
const LIVES: usize = 36;
const ROWS: usize = 1032;
const PRIVATE: u64 = 384 * 1024 * 1024;
pub(super) fn parse_memory(raw: &[u8], pid: u32) -> Result<Value> {
    query::parse(raw, pid)
}
#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
pub(super) enum Reason {
    Start,
    Progress,
    BeforeStop,
}
#[derive(Clone, Copy, Serialize)]
pub(super) enum Phase {
    Setup,
    Stage(u8),
    Closing,
    Cleanup,
}
#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
pub(super) enum Stop {
    Kill,
    Normal,
}
#[derive(Clone, Copy, Serialize)]
pub(super) enum Evidence {
    Injected,
    WindowsChildBracket,
}
#[derive(Clone, Copy, Serialize)]
pub(super) struct Profile {
    pub owner: u8,
    pub profile: Id,
    pub image: Id,
    pub current: bool,
    pub pid: u32,
}
#[derive(Clone, Copy, Serialize)]
pub(super) struct Binding {
    pub owner: u8,
    pub profile: Id,
    pub source: Id,
    pub nonce: Option<Id>,
}
#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
pub(super) struct Token {
    ledger: u64,
    ordinal: usize,
    pid: u32,
}
#[derive(Clone, Copy)]
pub(super) struct Attempt {
    token: Token,
    row: usize,
}
#[derive(Clone, Copy, Serialize)]
pub(super) struct Reading {
    pid: u32,
    ticks: u64,
    bytes: u64,
    evidence: Evidence,
}
#[derive(Clone, Serialize)]
pub(super) struct Row {
    token: Token,
    reason: Reason,
    phase: Phase,
    start_ms: u64,
    end_ms: Option<u64>,
    reading: Option<Reading>,
    failed: bool,
}
#[derive(Clone, Copy, Serialize)]
struct Stats {
    native: u64,
    commit: u64,
}
#[derive(Clone, Serialize)]
struct ObservedStats {
    first_ms: u64,
    last_ms: u64,
    maxima: Stats,
}
#[derive(Serialize)]
struct Life {
    profile: Profile,
    binding: Option<Binding>,
    born_ms: u64,
    closed_ms: Option<u64>,
    ticks: Option<u64>,
    next_progress_ms: u64,
    start_attempted: bool,
    start_ok: bool,
    stop_attempted: bool,
    stop_ok: bool,
    closing: Option<Stop>,
    waited: bool,
    code: Option<i32>,
    inflight: Option<usize>,
    sampled_max: Option<u64>,
    last_sample_start: Option<u64>,
    last_sample_end: Option<u64>,
    first_sample_end: Option<u64>,
    max_gap_ms: u64,
    missed_slots: u64,
    stats_last: Option<Stats>,
    pending: Option<ObservedStats>,
    stats: Option<ObservedStats>,
}
#[derive(Serialize)]
pub(super) struct Summary {
    pub interpretation: &'static str,
    pub coverage_valid: bool,
    pub run: Id,
    pub span_ms: u64,
    pub attempts: usize,
    pub lifetimes: usize,
    pub profiles: Vec<Value>,
}
pub(super) struct Ledger {
    run: Id,
    unique: u64,
    span_ms: u64,
    origin: Instant,
    last_ms: u64,
    failed: bool,
    sealed: bool,
    lives: Vec<Life>,
    rows: Vec<Row>,
}
impl Ledger {
    pub(super) fn new(run: Id, span_ms: u64) -> Result<Self> {
        ensure!(
            run != [0; 32] && span_ms > 0 && span_ms <= SPAN,
            "memory original run/span"
        );
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Ok(Self {
            run,
            unique: NEXT.fetch_add(1, Ordering::Relaxed),
            span_ms,
            origin: Instant::now(),
            last_ms: 0,
            failed: false,
            sealed: false,
            lives: Vec::with_capacity(LIVES),
            rows: Vec::with_capacity(ROWS),
        })
    }
    pub(super) fn now_ms(&self) -> u64 {
        self.origin
            .elapsed()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX)
    }
    fn checked<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        ensure!(
            !self.failed && !self.sealed,
            "memory scope failed or finalized"
        );
        let result = f(self);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn time(&mut self, at: u64) -> Result<()> {
        ensure!(
            at >= self.last_ms && at <= self.span_ms,
            "memory monotonic original span"
        );
        self.last_ms = at;
        Ok(())
    }
    fn index(&self, t: Token) -> Result<usize> {
        ensure!(
            t.ledger == self.unique
                && self
                    .lives
                    .get(t.ordinal)
                    .is_some_and(|l| l.profile.pid == t.pid),
            "memory foreign token"
        );
        Ok(t.ordinal)
    }
    /// Consume remaining live due slots once. Only a boundary starting exactly
    /// on a due instant serves that slot; a late boundary leaves it missed.
    /// Advancing the cursor prevents cleanup/closure from counting it twice.
    fn settle_live_slots(l: &mut Life, at: u64, boundary: bool) {
        if at >= l.next_progress_ms {
            let due = (at - l.next_progress_ms) / CADENCE + 1;
            let latest = l.next_progress_ms + (due - 1) * CADENCE;
            l.missed_slots += due - u64::from(boundary && at == latest);
            l.next_progress_ms += due * CADENCE;
        }
    }
    pub(super) fn register_at(&mut self, at: u64, p: Profile) -> Result<Token> {
        self.checked(|s| {
            s.time(at)?;
            ensure!(
                p.owner < 2
                    && p.pid > 0
                    && p.profile != [0; 32]
                    && p.image != [0; 32]
                    && s.lives.len() < LIVES,
                "memory registration bound"
            );
            ensure!(
                s.lives.iter().all(|l| {
                    (l.profile.owner != p.owner || l.profile.profile == p.profile)
                        && (l.profile.owner == p.owner || l.profile.profile != p.profile)
                        && (l.closed_ms.is_some()
                            || (l.profile.owner != p.owner && l.profile.pid != p.pid))
                }),
                "memory original profile/overlap"
            );
            let token = Token {
                ledger: s.unique,
                ordinal: s.lives.len(),
                pid: p.pid,
            };
            s.lives.push(Life {
                profile: p,
                binding: None,
                born_ms: at,
                closed_ms: None,
                ticks: None,
                next_progress_ms: at + CADENCE,
                start_attempted: false,
                start_ok: false,
                stop_attempted: false,
                stop_ok: false,
                closing: None,
                waited: false,
                code: None,
                inflight: None,
                sampled_max: None,
                last_sample_start: None,
                last_sample_end: None,
                first_sample_end: None,
                max_gap_ms: 0,
                missed_slots: 0,
                stats_last: None,
                pending: None,
                stats: None,
            });
            Ok(token)
        })
    }
    pub(super) fn bind(&mut self, t: Token, b: Binding) -> Result<()> {
        self.checked(|s| {
            let i = s.index(t)?;
            let life = &s.lives[i];
            ensure!(
                life.binding.is_none()
                    && b.owner == life.profile.owner
                    && b.profile == life.profile.profile
                    && b.source != [0; 32]
                    && (if life.profile.current {
                        b.nonce.is_some_and(|n| n != [0; 32])
                    } else {
                        b.nonce.is_none()
                    }),
                "memory verified binding"
            );
            ensure!(
                s.lives.iter().all(|l| l
                    .binding
                    .is_none_or(|old| (old.owner != b.owner || old.source == b.source)
                        && (old.owner == b.owner || old.source != b.source)
                        && b.nonce.is_none_or(|n| old.nonce != Some(n)))),
                "memory foreign source/reused incarnation"
            );
            s.lives[i].binding = Some(b);
            Ok(())
        })
    }
    /// Polling a not-yet-due/closing lifetime does not create an attempt or a
    /// row. A8b can call this from frequent checks without per-message queries.
    pub(super) fn progress_due_at(&mut self, t: Token, at: u64) -> Result<bool> {
        self.checked(|s| {
            s.time(at)?;
            let l = &s.lives[s.index(t)?];
            Ok(l.start_ok
                && l.closed_ms.is_none()
                && l.closing.is_none()
                && !l.stop_attempted
                && l.inflight.is_none()
                && at >= l.next_progress_ms)
        })
    }
    pub(super) fn begin_at(
        &mut self,
        t: Token,
        reason: Reason,
        phase: Phase,
        at: u64,
    ) -> Result<Attempt> {
        self.checked(|s| {
            s.time(at)?;
            let i = s.index(t)?;
            let n = s.rows.len();
            ensure!(
                n < ROWS && !matches!(phase,Phase::Stage(n) if n>12),
                "memory row/phase bound"
            );
            let l = &mut s.lives[i];
            ensure!(
                l.closed_ms.is_none() && l.closing.is_none() && l.inflight.is_none(),
                "memory closed/closing/inflight"
            );
            match reason {
                Reason::Start => {
                    ensure!(!l.start_attempted, "memory duplicate start");
                    l.start_attempted = true;
                }
                Reason::BeforeStop => {
                    ensure!(l.start_ok && !l.stop_attempted, "memory stop boundary");
                    Self::settle_live_slots(l, at, true);
                    l.stop_attempted = true;
                }
                Reason::Progress => {
                    ensure!(
                        l.start_ok && at >= l.next_progress_ms,
                        "memory five-second cadence"
                    );
                    l.missed_slots += (at - l.next_progress_ms) / CADENCE;
                    l.next_progress_ms = at + CADENCE;
                }
            }
            l.inflight = Some(n);
            s.rows.push(Row {
                token: t,
                reason,
                phase,
                start_ms: at,
                end_ms: None,
                reading: None,
                failed: false,
            });
            Ok(Attempt { token: t, row: n })
        })
    }
    pub(super) fn complete_at(
        &mut self,
        a: Attempt,
        end: u64,
        result: Result<Reading>,
    ) -> Result<()> {
        // This must precede the error handler as well as checked(): rejected
        // post-final calls cannot mutate any previously finalized evidence.
        ensure!(!self.sealed, "memory finalized evidence is immutable");
        let result = self.checked(|s| {
            let i = s.index(a.token)?;
            let row = s.rows.get_mut(a.row).context("memory attempt index")?;
            ensure!(
                row.token == a.token && row.end_ms.is_none(),
                "memory duplicate/foreign completion"
            );
            row.end_ms = Some(end);
            let reading = match result {
                Ok(r) => r,
                Err(_) => {
                    row.failed = true;
                    s.lives[i].inflight = None;
                    anyhow::bail!("memory query failed; not zero");
                }
            };
            let start = row.start_ms;
            let reason = row.reason;
            s.time(end)?;
            ensure!(
                reading.pid == a.token.pid
                    && reading.ticks > 0
                    && reading.bytes > 0
                    && reading.bytes <= PRIVATE,
                "memory reading identity/range"
            );
            ensure!(
                s.lives.iter().enumerate().all(|(n, l)| n == i
                    || l.profile.pid != reading.pid
                    || l.ticks != Some(reading.ticks)),
                "memory duplicate OS lifetime"
            );
            let l = &mut s.lives[i];
            ensure!(
                l.inflight == Some(a.row) && l.ticks.is_none_or(|old| old == reading.ticks),
                "memory creation identity"
            );
            l.ticks = Some(reading.ticks);
            l.inflight = None;
            l.sampled_max = Some(l.sampled_max.unwrap_or(0).max(reading.bytes));
            l.max_gap_ms = l
                .max_gap_ms
                .max(end - l.last_sample_start.unwrap_or(l.born_ms));
            l.first_sample_end.get_or_insert(end);
            l.last_sample_start = Some(start);
            l.last_sample_end = Some(end);
            match reason {
                Reason::Start => l.start_ok = true,
                Reason::BeforeStop => l.stop_ok = true,
                Reason::Progress => (),
            }
            s.rows[a.row].reading = Some(reading);
            Ok(())
        });
        if result.is_err() {
            if let Some(row) = self.rows.get_mut(a.row).filter(|r| r.token == a.token) {
                row.failed = true;
            }
            if let Ok(i) = self.index(a.token)
                && self.lives[i].inflight == Some(a.row)
            {
                self.lives[i].inflight = None;
            }
        }
        result
    }
    pub(super) fn candidate_at(&mut self, t: Token, at: u64, v: &Value) -> Result<()> {
        self.checked(|s| {
            s.time(at)?;
            let i = s.index(t)?;
            let l = &mut s.lives[i];
            let b = l.binding.context("memory Stats verified binding")?;
            ensure!(
                l.profile.current
                    && l.closed_ms.is_none()
                    && l.closing.is_none()
                    && v["version"] == 3
                    && v["profile_mode"] == "ordered-dialog-indexed"
                    && v["storage_stage"] == "Ready"
                    && v["source_id"] == kilogram_cli::ordered_dialog::state::hex(&b.source),
                "memory Stats identity/schema"
            );
            let numeric = |key: &str| {
                v["stats"][key]
                    .as_u64()
                    .context("memory Stats numeric field")
            };
            let next = Stats {
                native: numeric("max_native_bytes")?,
                commit: numeric("max_commit_app_bytes")?,
            };
            ensure!(
                next.native <= 128 * 1024
                    && next.commit <= 192 * 1024 * 1024
                    && l.stats_last
                        .is_none_or(|old| next.native >= old.native && next.commit >= old.commit),
                "memory Stats cap/regression"
            );
            // Validate EVERY candidate before aggregation; a later value cannot
            // overwrite an earlier malformed/regressing candidate into success.
            l.stats_last = Some(next);
            match l.pending.as_mut() {
                Some(p) => {
                    p.last_ms = at;
                    p.maxima = next;
                }
                None => {
                    l.pending = Some(ObservedStats {
                        first_ms: at,
                        last_ms: at,
                        maxima: next,
                    })
                }
            }
            Ok(())
        })
    }
    fn accept_pending(&mut self, t: Token) -> Result<()> {
        let i = self.index(t)?;
        let l = &mut self.lives[i];
        ensure!(
            l.closed_ms.is_none() && l.closing.is_none(),
            "memory Stats closed checkpoint"
        );
        if let Some(p) = l.pending.take() {
            match l.stats.as_mut() {
                Some(old) => {
                    old.last_ms = p.last_ms;
                    old.maxima = p.maxima;
                }
                None => l.stats = Some(p),
            }
        }
        Ok(())
    }
    /// Query adapter is the only real handle checkpoint in A8a; A8b adds the
    /// shared status checkpoints without trusting a Value/PID alone.
    pub(super) fn sample(
        &mut self,
        t: Token,
        reason: Reason,
        phase: Phase,
        child: &mut Child,
        shell: &query::Shell,
    ) -> Result<()> {
        let a = self.begin_at(t, reason, phase, self.now_ms())?;
        let result = query::known(shell, child, t.pid);
        self.complete_at(a, self.now_ms(), result)?;
        self.checked(|s| s.accept_pending(t))
    }
    #[cfg(test)]
    fn injected_checkpoint(&mut self, t: Token) -> Result<()> {
        self.checked(|s| s.accept_pending(t))
    }
    pub(super) fn request_stop_at(&mut self, t: Token, kind: Stop, at: u64) -> Result<()> {
        ensure!(!self.sealed, "memory finalized evidence is immutable");
        // Closing/cleanup facts must remain recordable after an observation
        // failure; the poisoned functional result is never reset.
        let result = (|| {
            self.time(at)?;
            let i = self.index(t)?;
            let l = &mut self.lives[i];
            ensure!(
                l.closed_ms.is_none() && l.closing.is_none(),
                "memory stop requested once"
            );
            if !l.stop_attempted {
                Self::settle_live_slots(l, at, false);
            }
            l.closing = Some(kind);
            ensure!(
                l.stop_ok && l.pending.is_none() && l.inflight.is_none(),
                "memory missing pre-stop/unchecked Stats"
            );
            Ok(())
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    pub(super) fn close_at(
        &mut self,
        t: Token,
        at: u64,
        waited: bool,
        code: Option<i32>,
    ) -> Result<()> {
        ensure!(!self.sealed, "memory finalized evidence is immutable");
        let result = (|| {
            let i = self.index(t)?;
            let l = &mut self.lives[i];
            ensure!(l.closed_ms.is_none(), "memory duplicate close");
            if l.closing.is_none() && !l.stop_attempted {
                // Cleanup can be reported after expiry; only slots within the
                // original observation span were eligible for progress.
                Self::settle_live_slots(l, at.min(self.span_ms), false);
            }
            l.closed_ms = Some(at);
            l.waited = waited;
            l.code = code;
            let normal = l.closing == Some(Stop::Normal);
            ensure!(
                l.closing.is_some()
                    && waited
                    && code.is_some()
                    && (if normal {
                        code == Some(0)
                    } else {
                        code != Some(0)
                    }),
                "memory unexpected/unconfirmed outcome"
            );
            self.time(at)?;
            Ok(())
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    pub(super) fn rows(&self) -> &[Row] {
        &self.rows
    }
    pub(super) fn diagnostic(&self) -> Summary {
        Summary{interpretation:"core sampled PrivateBytes and observed live Stats only; not terminal peaks or whole S54",coverage_valid:!self.failed && self.complete(),
            run:self.run,span_ms:self.span_ms,attempts:self.rows.len(),lifetimes:self.lives.len(),
            profiles:self.lives.iter().map(|l|json!({"profile":l.profile,"binding":l.binding,"born_ms":l.born_ms,"closed_ms":l.closed_ms,
                "creation_ticks":l.ticks,"sampled_max_private_bytes":l.sampled_max,"first_sample_end_ms":l.first_sample_end,
                "last_sample_start_ms":l.last_sample_start,"last_sample_end_ms":l.last_sample_end,
                "max_conservative_query_gap_ms":l.max_gap_ms.max(l.closed_ms.zip(l.last_sample_start).map_or(0,|(end,start)|end.saturating_sub(start))),"missed_periodic_slots":l.missed_slots,
                "closing_tail_ms":l.closed_ms.zip(l.last_sample_start).map(|(end,start)|end.saturating_sub(start)),
                "observed_stats":l.stats,"pending_stats":l.pending,"stats_end_gap_ms":l.closed_ms.zip(l.stats.as_ref()).map(|(end,s)|end.saturating_sub(s.last_ms)),
                "waited":l.waited,"code":l.code})).collect()}
    }
    fn complete(&self) -> bool {
        !self.lives.is_empty()
            && self.lives.iter().all(|l| {
                l.closed_ms.is_some()
                    && l.binding.is_some()
                    && l.start_ok
                    && l.stop_ok
                    && l.waited
                    && l.inflight.is_none()
                    && l.pending.is_none()
                    && (!l.profile.current || l.stats.is_some())
            })
    }
    pub(super) fn finish(&mut self) -> Result<Summary> {
        self.checked(|s| {
            ensure!(s.complete(), "memory incomplete coverage");
            Ok(())
        })?;
        self.sealed = true;
        Ok(self.diagnostic())
    }
}
