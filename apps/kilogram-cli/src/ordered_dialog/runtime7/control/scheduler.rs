//! Monotonic scheduling decisions. The serialized owner supplies actual durable
//! mode only after its outstanding commit/reopen has resolved. Network dispatch
//! must also enforce its real lease, permission, permit and RPC deadlines.
use super::*;
use std::time::{Duration, Instant};

pub const INITIAL_DISCOVERY7: Duration = Duration::from_secs(5);
pub const DISCOVERY_INTERVAL7: Duration = Duration::from_secs(60);
pub const PROBE_WHOLE7: Duration = Duration::from_secs(45);
pub const PROBE_QUIET7: Duration = Duration::from_secs(15);
pub const ORDINARY_FLOOR7: Duration = Duration::from_secs(30);
pub const ORDINARY_WHOLE7: Duration = Duration::from_secs(90);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduleDecision7 {
    Wait,
    Ordinary,
    ResolveOrdinary,
    Probe,
    Existing,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeToken7 {
    generation: u64,
    pub started: Instant,
    pub deadline: Instant,
}
#[derive(Clone, Copy)]
struct Ordinary7 {
    started: Instant,
    finished: bool,
    after_probe: bool,
}
pub struct ExistingScheduler7 {
    eligible_since: Option<Instant>,
    last_start: Option<Instant>,
    probe: Option<ProbeToken7>,
    ordinary: Option<Ordinary7>,
    next_generation: u64,
}
impl Default for ExistingScheduler7 {
    fn default() -> Self {
        Self {
            eligible_since: None,
            last_start: None,
            probe: None,
            ordinary: None,
            next_generation: 1,
        }
    }
}
impl ExistingScheduler7 {
    pub fn ordinary_bounds(&self) -> Option<(Instant, Instant)> {
        self.ordinary.map(|turn| {
            (
                turn.started + ORDINARY_FLOOR7,
                turn.started + ORDINARY_WHOLE7,
            )
        })
    }
    pub fn discovery_due(&self) -> Option<Instant> {
        self.last_start
            .map(|last| last + DISCOVERY_INTERVAL7)
            .or_else(|| self.eligible_since.map(|since| since + INITIAL_DISCOVERY7))
    }
    pub fn ordinary_after_probe(&self) -> bool {
        self.ordinary.is_some_and(|turn| turn.after_probe)
    }
    pub fn l1_returned(&mut self, now: Instant) -> Result<()> {
        ensure!(self.probe.is_none(), "resolve probe before L1 return");
        if let Some(turn) = &mut self.ordinary
            && turn.after_probe
        {
            *turn = Ordinary7 {
                started: now,
                finished: false,
                after_probe: true,
            };
        }
        Ok(())
    }
    pub fn eligibility(&mut self, now: Instant, eligible: bool) -> Result<()> {
        ensure!(
            self.probe.is_none(),
            "resolve active probe before eligibility change"
        );
        match (self.eligible_since, eligible) {
            (None, true) => {
                self.eligible_since = Some(now);
                self.ordinary = Some(Ordinary7 {
                    started: now,
                    finished: false,
                    after_probe: false,
                });
            }
            (Some(_), false) => {
                self.eligible_since = None;
                self.ordinary = None;
            }
            _ => {}
        }
        Ok(())
    }
    pub fn decision(
        &self,
        now: Instant,
        mode: store::ExistingMode,
        trigger: bool,
        l1_eligible: bool,
        rpc_in_flight: bool,
    ) -> ScheduleDecision7 {
        if mode != store::ExistingMode::L1 {
            return ScheduleDecision7::Existing;
        }
        let Some(since) = self.eligible_since else {
            return ScheduleDecision7::Wait;
        };
        if self.probe.is_some() || rpc_in_flight {
            return ScheduleDecision7::Wait;
        }
        let protected = self
            .ordinary
            .filter(|turn| l1_eligible && (turn.after_probe || !trigger));
        if let Some(turn) = protected {
            if !turn.finished {
                return if now.saturating_duration_since(turn.started) >= ORDINARY_WHOLE7 {
                    ScheduleDecision7::ResolveOrdinary
                } else {
                    ScheduleDecision7::Ordinary
                };
            }
            if now.saturating_duration_since(turn.started) < ORDINARY_FLOOR7 {
                return ScheduleDecision7::Wait;
            }
        }
        let interval = self
            .last_start
            .is_none_or(|last| now.saturating_duration_since(last) >= DISCOVERY_INTERVAL7);
        if trigger || interval && now.saturating_duration_since(since) >= INITIAL_DISCOVERY7 {
            ScheduleDecision7::Probe
        } else {
            ScheduleDecision7::Wait
        }
    }
    /// Must be called at a closed frame boundary, before handover/permit waits.
    pub fn start(
        &mut self,
        now: Instant,
        mode: store::ExistingMode,
        trigger: bool,
        l1_eligible: bool,
        rpc_in_flight: bool,
    ) -> Result<ProbeToken7> {
        ensure!(
            self.decision(now, mode, trigger, l1_eligible, rpc_in_flight)
                == ScheduleDecision7::Probe,
            "probe not due or protected ordinary turn incomplete"
        );
        let token = ProbeToken7 {
            generation: self.next_generation,
            started: now,
            deadline: now + PROBE_WHOLE7,
        };
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .context("probe generation exhausted")?;
        self.last_start = Some(now);
        self.probe = Some(token);
        self.ordinary = None;
        Ok(token)
    }
    pub fn ordinary_finished(&mut self, now: Instant) -> Result<()> {
        let turn = self
            .ordinary
            .as_mut()
            .context("no protected ordinary attempt")?;
        ensure!(now >= turn.started, "monotonic ordinary time");
        turn.finished = true;
        Ok(())
    }
    pub fn quiet_deadline(&self, token: ProbeToken7, admitted: Instant) -> Result<Instant> {
        ensure!(
            self.probe == Some(token) && admitted >= token.started && admitted < token.deadline,
            "stale/expired probe admission"
        );
        Ok((admitted + PROBE_QUIET7).min(token.deadline))
    }
    pub fn gate(&self, token: ProbeToken7, now: Instant, mode: store::ExistingMode) -> Result<()> {
        ensure!(
            self.probe == Some(token) && now >= token.started,
            "stale probe generation/time"
        );
        ensure!(
            mode != store::ExistingMode::L1 || now < token.deadline,
            "pre-P opportunity expired"
        );
        Ok(())
    }
    /// A stale timer cannot clear a successor. This never invalidates a lease or
    /// extends its expiry; the caller closes only the returned probe generation.
    pub fn resolved(
        &mut self,
        token: ProbeToken7,
        now: Instant,
        mode: store::ExistingMode,
    ) -> Result<bool> {
        if self.probe != Some(token) {
            return Ok(false);
        }
        ensure!(now >= token.started, "monotonic probe resolution");
        self.probe = None;
        self.ordinary = (mode == store::ExistingMode::L1).then_some(Ordinary7 {
            started: now,
            finished: false,
            after_probe: true,
        });
        Ok(true)
    }
}

/// Round-robin choice among the two independent per-C owners. Ref and global
/// admission permits remain enforced by the existing resource owners.
#[derive(Default)]
pub struct ExistingFairTurn7 {
    next: usize,
}
impl ExistingFairTurn7 {
    pub fn choose(&mut self, ready: [bool; 2]) -> Option<u8> {
        for delta in 0..2 {
            let c = (self.next + delta) % 2;
            if ready[c] {
                self.next = 1 - c;
                return Some(c as u8);
            }
        }
        None
    }
}
