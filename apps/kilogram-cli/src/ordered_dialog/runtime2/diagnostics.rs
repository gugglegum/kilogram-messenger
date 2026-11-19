//! Passive lab measurements. No frame/error text, protocol decisions or persistent owner state.
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering::Relaxed};
use std::time::Instant;

macro_rules! events {
    ($($event:ident),+ $(,)?) => {
        #[derive(Clone, Copy)]
        #[repr(usize)]
        pub(super) enum Event { $($event),+ }
        const EVENTS: &[Event] = &[$(Event::$event),+];
        impl Event { fn name(self) -> &'static str { match self { $(Self::$event => stringify!($event)),+ } } }
    };
}
events!(
    MetadataStart,
    MetadataStartError,
    MetadataReady,
    MetadataError,
    MetadataLate,
    MetadataRevoked,
    MetadataCleanupTimeout,
    MetadataCleanupError,
    SocketOpen,
    SocketOpenError,
    SendAttempt,
    SendOk,
    SendError,
    ReceiveError,
    ReceivePacket,
    ReceiveStale,
    PeerFilterRejected,
    QuotaDrop,
    InboxQueued,
    InboxDrop,
    AnnounceAttempt,
    AnnounceOk,
    AnnounceError,
    PublicationQueued,
    PublicationDrop,
    CaptureWithoutObservation,
    LocatorAttempt,
    LocatorOpened,
    LocatorValidated,
    LocatorDuplicate,
    LocatorCommitted,
    LocatorError,
    ListenerIncoming,
    ListenerPeerRejected,
    ListenerTlsError,
    InboundAdmissionError,
    InboundLease,
    SocketCancelError,
    SocketCleanupTimeout,
    SocketJoinError,
    SocketWorkerError,
    EndpointBindError,
    EndpointBound,
    EndpointAttachError,
    EndpointCleanupError,
    TransportCleanupError,
    OwnerDetachError,
    SupervisorError,
    MetadataMonitorError,
    MetadataMonitorJoinError,
    OutboundAdmissionError,
    OutboundAdmissionTimeout
);

#[derive(Clone, Copy)]
#[repr(u64)]
pub(super) enum DialStage {
    Idle,
    RoleCheck,
    NotDialer,
    WaitingTarget,
    PeerSelection,
    Connect,
    RoutePolicy,
    BeginAdmission,
    Hello,
    Proof,
    Admitted,
    AuthenticatedLease,
    FreshProtocol,
    Serving,
    Ended,
}
impl DialStage {
    fn name(value: u64) -> &'static str {
        match value {
            0 => "idle",
            1 => "role_check",
            2 => "not_dialer",
            3 => "waiting_target",
            4 => "peer_selection",
            5 => "connect",
            6 => "route_policy",
            7 => "begin_admission",
            8 => "hello",
            9 => "proof",
            10 => "admitted",
            11 => "authenticated_lease",
            12 => "fresh_protocol",
            13 => "serving",
            14 => "ended",
            _ => "unknown",
        }
    }
}
#[derive(Default)]
struct Slot {
    stage: AtomicU64,
    attempts: AtomicU64,
    errors: AtomicU64,
    timeouts: AtomicU64,
    leases: AtomicU64,
    fresh_ok: AtomicU64,
    fresh_errors: AtomicU64,
}
pub(super) struct Counters {
    enabled: bool,
    started: Instant,
    counts: Vec<AtomicU64>,
    os_errors: Vec<AtomicI32>,
    slots: [Slot; 2],
}
fn increment(value: &AtomicU64) {
    let _ = value.fetch_update(Relaxed, Relaxed, |n| Some(n.saturating_add(1)));
}
impl Counters {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            started: Instant::now(),
            counts: EVENTS.iter().map(|_| AtomicU64::new(0)).collect(),
            os_errors: EVENTS.iter().map(|_| AtomicI32::new(0)).collect(),
            slots: std::array::from_fn(|_| {
                let slot = Slot::default();
                slot.stage.store(DialStage::Idle as u64, Relaxed);
                slot
            }),
        }
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn event(&self, event: Event) {
        if self.enabled {
            increment(&self.counts[event as usize]);
        }
    }
    pub fn error(&self, event: Event, error: &anyhow::Error) {
        if self.enabled {
            let code = error
                .chain()
                .find_map(|e| {
                    e.downcast_ref::<std::io::Error>()
                        .and_then(std::io::Error::raw_os_error)
                })
                .unwrap_or(0);
            self.os_errors[event as usize].store(code, Relaxed);
            self.event(event);
        }
    }
    pub fn stage(&self, slot: u8, stage: DialStage) {
        if self.enabled
            && let Some(value) = self.slots.get(usize::from(slot))
        {
            value.stage.store(stage as u64, Relaxed);
        }
    }
    pub fn attempt(&self, slot: u8) {
        if self.enabled
            && let Some(value) = self.slots.get(usize::from(slot))
        {
            increment(&value.attempts);
        }
    }
    pub fn admission_error(&self, slot: u8, timeout: bool) {
        if self.enabled
            && let Some(value) = self.slots.get(usize::from(slot))
        {
            increment(if timeout {
                &value.timeouts
            } else {
                &value.errors
            });
        }
    }
    pub fn lease(&self, slot: u8) {
        if self.enabled
            && let Some(value) = self.slots.get(usize::from(slot))
        {
            increment(&value.leases);
        }
    }
    pub fn fresh(&self, slot: u8, ok: bool) {
        if self.enabled
            && let Some(value) = self.slots.get(usize::from(slot))
        {
            increment(if ok {
                &value.fresh_ok
            } else {
                &value.fresh_errors
            });
        }
    }
    /// Independent atomics: this is a bounded observation, never a transactional authority view.
    pub fn snapshot(&self) -> serde_json::Value {
        let counts: serde_json::Map<_, _> = EVENTS
            .iter()
            .map(|event| {
                (
                    event.name().to_owned(),
                    serde_json::json!(self.counts[*event as usize].load(Relaxed)),
                )
            })
            .collect();
        let errors: serde_json::Map<_, _> = EVENTS
            .iter()
            .filter_map(|event| {
                let code = self.os_errors[*event as usize].load(Relaxed);
                (code != 0).then(|| (event.name().to_owned(), serde_json::json!(code)))
            })
            .collect();
        let slots:Vec<_>=self.slots.iter().enumerate().map(|(i,s)|serde_json::json!({"slot":i,"stage":DialStage::name(s.stage.load(Relaxed)),"attempts":s.attempts.load(Relaxed),"errors":s.errors.load(Relaxed),"timeouts":s.timeouts.load(Relaxed),"leases":s.leases.load(Relaxed),"fresh_ok":s.fresh_ok.load(Relaxed),"fresh_errors":s.fresh_errors.load(Relaxed)})).collect();
        serde_json::json!({"schema":1,"enabled":self.enabled,"elapsed_ms":u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX),"counters":counts,"last_os_error_by_stage":errors,"slots":slots})
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn t13diag_counters_are_bounded_nonsecret_and_saturating() {
        let counters = Counters::new(true);
        counters.counts[Event::SendOk as usize].store(u64::MAX, Relaxed);
        counters.event(Event::SendOk);
        counters.error(
            Event::ReceiveError,
            &anyhow::Error::from(std::io::Error::from_raw_os_error(10013)),
        );
        counters.stage(1, DialStage::Connect);
        counters.attempt(1);
        counters.admission_error(1, true);
        counters.stage(255, DialStage::Ended);
        let snapshot = counters.snapshot();
        assert_eq!(snapshot["counters"]["SendOk"], u64::MAX);
        assert_eq!(snapshot["last_os_error_by_stage"]["ReceiveError"], 10013);
        assert_eq!(snapshot["slots"][1]["stage"], "connect");
        assert_eq!(snapshot["slots"][1]["timeouts"], 1);
        assert!(serde_json::to_vec(&snapshot).expect("fixed schema").len() < 8192);
        let disabled = Counters::new(false);
        disabled.event(Event::SendOk);
        assert_eq!(disabled.snapshot()["counters"]["SendOk"], 0);
    }
}
