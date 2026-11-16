//! Per-command gates and the snapshot used when execution outlives a terminal reply.
use crate::{
    connection::LocatorReport,
    send_recovery::{RequestReport, RequestResult, RequestState},
};
use serde::Serialize;
use std::{
    future::Future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone, Serialize)]
pub struct RequestIdentity {
    pub context: String,
    pub request_id: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OperationKind {
    Read,
    Send,
    Retry,
    Locator,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Initial,
    Preparing,
    Protected,
}

struct Progress {
    cancelled: Option<Instant>,
    identity: Option<RequestIdentity>,
    phase: Phase,
    receipt: Option<RequestReport>,
    locator: Option<LocatorReport>,
    provisional_owner: Option<Arc<AtomicBool>>,
    finalised: bool,
}

pub struct OperationControl {
    kind: OperationKind,
    deadline: Instant,
    progress: Mutex<Progress>,
}

pub struct FinalSnapshot {
    pub identity: Option<RequestIdentity>,
    pub cancellation: Option<RequestResult>,
    pub locator: Option<LocatorReport>,
}

impl OperationControl {
    pub fn new(kind: OperationKind, deadline: Instant, identity: Option<RequestIdentity>) -> Self {
        Self {
            kind,
            deadline,
            progress: Mutex::new(Progress {
                cancelled: None,
                identity,
                phase: Phase::Initial,
                receipt: None,
                locator: None,
                provisional_owner: None,
                finalised: false,
            }),
        }
    }

    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    pub fn cancel(&self, now: Instant) -> bool {
        let mut p = self.progress.lock().unwrap();
        if p.finalised || p.cancelled.is_some() {
            return false;
        }
        p.cancelled = Some(now);
        if let Some(stop) = &p.provisional_owner {
            stop.store(true, Ordering::Release);
        }
        true
    }

    pub fn cancel_time(&self) -> Option<Instant> {
        self.progress.lock().unwrap().cancelled
    }

    pub fn stopped(&self) -> bool {
        Instant::now() >= self.deadline || self.cancel_time().is_some()
    }

    pub fn identify(&self, identity: RequestIdentity) {
        let mut p = self.progress.lock().unwrap();
        if !p.finalised {
            p.identity = Some(identity);
        }
    }

    pub(crate) fn provisional_owner(&self, stop: Arc<AtomicBool>) -> bool {
        let mut p = self.progress.lock().unwrap();
        if p.cancelled.is_some() || p.finalised || Instant::now() >= self.deadline {
            return false;
        }
        p.provisional_owner = Some(stop);
        true
    }

    pub(crate) fn begin_prepare(&self) -> bool {
        let mut p = self.progress.lock().unwrap();
        if p.cancelled.is_some() || p.finalised || Instant::now() >= self.deadline {
            return false;
        }
        p.phase = Phase::Preparing;
        true
    }

    pub(crate) fn protected(&self) {
        self.progress.lock().unwrap().phase = Phase::Protected;
    }

    pub(crate) fn begin_ipc(&self) -> bool {
        let p = self.progress.lock().unwrap();
        !p.finalised && p.cancelled.is_none() && Instant::now() < self.deadline
    }

    pub(crate) fn verified(&self, report: RequestReport) {
        let mut p = self.progress.lock().unwrap();
        if !p.finalised {
            p.receipt = Some(report);
        }
    }

    pub(crate) fn admit_locator(&self, report: &LocatorReport) -> bool {
        let mut p = self.progress.lock().unwrap();
        if p.cancelled.is_some() || p.finalised || Instant::now() >= self.deadline {
            return false;
        }
        p.locator = Some(report.clone());
        true
    }

    pub(crate) fn locator_progress(&self, report: &LocatorReport) {
        let mut p = self.progress.lock().unwrap();
        if !p.finalised {
            p.locator = Some(report.clone());
        }
    }

    /// Called while the terminal owns the execution generation. The frozen
    /// snapshot and cancellation use the same lock as receipt publication.
    pub fn finish(&self, fallback: bool) -> FinalSnapshot {
        let mut p = self.progress.lock().unwrap();
        assert!(!p.finalised);
        p.finalised = true;
        p.provisional_owner = None;
        let interrupted = fallback || p.cancelled.is_some();
        let locator = p.locator.take().filter(|_| interrupted);
        let cancellation = if interrupted && locator.is_none() {
            Some(if let Some(receipt) = p.receipt.take() {
                Ok(receipt)
            } else if matches!(self.kind, OperationKind::Read | OperationKind::Locator)
                || p.identity.is_none()
            {
                Err("cancelled")
            } else if self.kind == OperationKind::Send && p.phase == Phase::Initial {
                Err("cancelled_not_submitted")
            } else {
                Ok(RequestReport {
                    state: RequestState::Unconfirmed,
                    protected: (p.phase == Phase::Protected).then_some(true),
                    event_id: None,
                })
            })
        } else {
            None
        };
        FinalSnapshot {
            identity: p.identity.take(),
            cancellation,
            locator,
        }
    }
}

pub(crate) fn wait<T>(
    rt: &tokio::runtime::Runtime,
    stop: Option<&AtomicBool>,
    control: Option<&OperationControl>,
    future: impl Future<Output = T>,
) -> anyhow::Result<T> {
    let stopped = || {
        stop.is_some_and(|v| v.load(Ordering::Acquire))
            || control.is_some_and(OperationControl::stopped)
    };
    anyhow::ensure!(!stopped(), "Connection operation cancelled");
    rt.block_on(async {
        tokio::pin!(future);
        loop {
            tokio::select! {
                biased;
                result = &mut future => return Ok(result),
                _ = tokio::time::sleep(Duration::from_millis(20)) => {
                    anyhow::ensure!(!stopped(), "Connection operation cancelled");
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control() -> OperationControl {
        OperationControl::new(
            OperationKind::Send,
            Instant::now() + Duration::from_secs(10),
            Some(RequestIdentity {
                context: "context".into(),
                request_id: "ef".repeat(16),
            }),
        )
    }

    fn receipt() -> RequestReport {
        RequestReport {
            state: RequestState::Queued,
            protected: Some(true),
            event_id: None,
        }
    }

    #[test]
    fn t11c_receipt_snapshot_linearizes_before_or_after_final() {
        let before = control();
        assert!(before.begin_prepare());
        before.protected();
        assert!(before.begin_ipc());
        assert!(before.cancel(Instant::now()));
        before.verified(receipt());
        let final_result = before.finish(true);
        assert_eq!(final_result.identity.unwrap().request_id, "ef".repeat(16));
        let report = final_result.cancellation.unwrap().unwrap();
        assert_eq!(report.state, RequestState::Queued);
        assert_eq!(report.protected, Some(true));

        let after = control();
        assert!(after.begin_prepare());
        after.protected();
        assert!(after.cancel(Instant::now()));
        assert!(!after.begin_ipc());
        let final_result = after.finish(true);
        after.verified(receipt());
        assert!(after.progress.lock().unwrap().receipt.is_none());
        assert!(!after.cancel(Instant::now()));
        let report = final_result.cancellation.unwrap().unwrap();
        assert_eq!(report.state, RequestState::Unconfirmed);
        assert_eq!(report.protected, Some(true));
    }
}
