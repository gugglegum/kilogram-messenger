//! Opt-in numerical diagnostic counters. No protocol/state values are retained.
use std::{
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

macro_rules! metrics {
    ($($name:ident),+ $(,)?) => {
        #[derive(Clone, Copy)]
        pub(super) enum Metric { $($name),+ }
        const NAMES: &[&str] = &[$(stringify!($name)),+];
    };
}
metrics!(
    MailboxSubmit,
    MailboxQueue,
    OwnerJob,
    OwnerIdle,
    LeaseCheck,
    Status,
    MaterializeTurn,
    MaterializeActive,
    DrainTurn,
    DrainActive,
    TrialSend,
    TrialReceive,
    QueueOperation,
    IngestOperation,
    AcknowledgeOperation,
    NativeEncrypt,
    NativeDecrypt,
    NativeFork,
    NativeSeal,
    NativeOpen,
    Snapshot,
    Current,
    Selected,
    Require,
    Seen,
    ParentRead,
    Credential,
    ClosedCommit,
    HooksLoad,
    PublicRead,
    LiveWindow,
    DebtPage,
    DeliveryItems,
    VerifyFrame,
    FrameSign,
    HeadBody,
    PrefixRequest,
    PrefixResponse,
    AckBody,
    AckItems,
    AckQuery,
    AckProgress,
    TakeDelivery,
    TakeAck,
    TakePrefix,
    Rpc,
    DataRound,
    PushLive,
    ServerSession,
    ReadRound,
);

#[derive(Default)]
struct Counter {
    count: AtomicU64,
    nanos: AtomicU64,
    max_nanos: AtomicU64,
}
static ENABLED: OnceLock<bool> = OnceLock::new();
static COUNTERS: OnceLock<Vec<Counter>> = OnceLock::new();
fn enabled() -> bool {
    cfg!(debug_assertions)
        && *ENABLED
            .get_or_init(|| std::env::var("KILOGRAM_TEST_OD2_TIMINGS").is_ok_and(|s| s == "1"))
}
pub(super) fn instant() -> Option<Instant> {
    enabled().then(Instant::now)
}
pub(super) fn record(metric: Metric, start: Option<Instant>) {
    if let Some(start) = start {
        let counters = COUNTERS.get_or_init(|| NAMES.iter().map(|_| Counter::default()).collect());
        let value = u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX);
        let counter = &counters[metric as usize];
        counter.count.fetch_add(1, Ordering::Relaxed);
        counter.nanos.fetch_add(value, Ordering::Relaxed);
        counter.max_nanos.fetch_max(value, Ordering::Relaxed);
    }
}
pub(super) struct Span {
    metric: Metric,
    start: Option<Instant>,
}
pub(super) fn span(metric: Metric) -> Span {
    Span {
        metric,
        start: instant(),
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        record(self.metric, self.start);
    }
}
pub(super) fn idle_sleep(duration: Duration) {
    let _timing = span(Metric::OwnerIdle);
    std::thread::sleep(duration);
}
/// Runtime guard emits one bounded summary after graceful shutdown/error unwind.
pub(super) struct Report;
impl Drop for Report {
    fn drop(&mut self) {
        if !enabled() {
            return;
        }
        if let Some(counters) = COUNTERS.get() {
            for (name, counter) in NAMES.iter().zip(counters) {
                let count = counter.count.load(Ordering::Relaxed);
                if count > 0 {
                    eprintln!(
                        "od2_diag pid={} metric={name} count={count} nanos={} max_nanos={}",
                        std::process::id(),
                        counter.nanos.load(Ordering::Relaxed),
                        counter.max_nanos.load(Ordering::Relaxed)
                    );
                }
            }
        }
    }
}
