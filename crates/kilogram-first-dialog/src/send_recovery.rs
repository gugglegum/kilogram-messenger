//! Reconcile one protected intent with the durable local runtime queue.
use super::send_intent::{Intent, Phase, Store, valid_text};
use crate::operation::OperationControl;
use kilogram_runtime_ipc::ordered2::{Delivery2, MessageError2, RequestStatus2, SendReceipt2};
use serde::Serialize;
use std::sync::Arc;
use zeroize::Zeroizing;

pub type IpcResult<T> = Result<T, MessageError2>;

pub(super) trait SendRuntime {
    fn operation(&self) -> Option<&OperationControl> {
        None
    }
    fn check_send(&mut self, slot: u8) -> IpcResult<()>;
    fn lookup(&mut self, slot: u8, request: [u8; 16]) -> IpcResult<Option<RequestStatus2>>;
    fn send(&mut self, slot: u8, request: [u8; 16], text: &str) -> IpcResult<SendReceipt2>;
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    Empty,
    DraftRejected,
    Unconfirmed,
    RetryAvailable,
    Queued,
    Materialized,
    Delivered,
    Conflict,
    StorageBlocked,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Submission {
    Protected,
    Retained,
}

#[derive(PartialEq, Eq)]
pub enum RequestOperation {
    Send(Zeroizing<String>),
    Status,
    Retry,
}

impl std::fmt::Debug for RequestOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Send(_) => "Send",
            Self::Status => "Status",
            Self::Retry => "Retry",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestState {
    Unknown,
    Unconfirmed,
    RetryAvailable,
    Queued,
    Materialized,
    Delivered,
}

#[derive(Clone, Debug, Serialize)]
pub struct RequestReport {
    pub state: RequestState,
    pub protected: Option<bool>,
    pub event_id: Option<String>,
}

impl RequestReport {
    pub(super) fn from_status(status: Option<RequestStatus2>) -> Self {
        let state = match status.map(|value| value.delivery) {
            None => RequestState::Unknown,
            Some(Delivery2::Queued) => RequestState::Queued,
            Some(Delivery2::Materialized) => RequestState::Materialized,
            Some(Delivery2::Delivered) => RequestState::Delivered,
        };
        Self {
            state,
            protected: None,
            event_id: status
                .and_then(|value| value.receipt.event)
                .map(|event| super::artifact::hex(&event)),
        }
    }
}

pub type RequestResult = Result<RequestReport, &'static str>;

pub struct Submitted {
    pub slot: u8,
    pub context: [u8; 32],
    pub request: [u8; 16],
    pub text: Zeroizing<String>,
}

#[derive(Clone)]
pub struct Projection {
    pub busy: bool,
    pub progress: Progress,
    pub item: Option<Arc<Submitted>>,
    pub event: Option<[u8; 32]>,
}
impl Default for Projection {
    fn default() -> Self {
        Self {
            busy: true,
            progress: Progress::Empty,
            item: None,
            event: None,
        }
    }
}
impl Progress {
    pub fn notice(self) -> Option<&'static str> {
        match self {
            Self::Empty => None,
            Self::DraftRejected => Some("The new draft was not submitted."),
            Self::Unconfirmed => Some("The saved send is awaiting confirmation."),
            Self::RetryAvailable => Some("The saved send was not found in the local runtime."),
            Self::Queued => Some("The saved send is in the local queue."),
            Self::Materialized => Some("The saved send is in history; delivery is not confirmed."),
            Self::Delivered => Some("The saved send was delivered."),
            Self::Conflict => Some("The saved send is blocked and needs inspection."),
            Self::StorageBlocked => Some("The protected send record could not be used or updated."),
        }
    }
}

pub(super) struct Recovery {
    store: anyhow::Result<Store>,
    conflict: bool,
    known: Option<([u8; 16], SendReceipt2)>,
    pub(super) progress: Progress,
    busy: bool,
    protected: bool,
    item: Option<Arc<Submitted>>,
    event: Option<[u8; 32]>,
}
impl Recovery {
    pub(super) fn new(store: anyhow::Result<Store>) -> Self {
        Self {
            progress: if store.is_ok() {
                Progress::Empty
            } else {
                Progress::StorageBlocked
            },
            store,
            conflict: false,
            known: None,
            busy: true,
            protected: false,
            item: None,
            event: None,
        }
    }

    pub(super) fn projection(&self) -> Projection {
        Projection {
            busy: self.busy,
            progress: self.progress,
            item: self.item.clone(),
            event: self.event,
        }
    }

    fn remember(&mut self, intent: &Intent) {
        self.busy = true;
        self.protected = true;
        if !self.item.as_ref().is_some_and(|item| {
            item.slot == intent.slot
                && item.context == intent.context
                && item.request == intent.request
                && item.text.as_str() == intent.text
        }) {
            self.event = None;
            self.item = Some(Arc::new(Submitted {
                slot: intent.slot,
                context: intent.context,
                request: intent.request,
                text: Zeroizing::new(intent.text.clone()),
            }));
        }
    }

    fn pending(&mut self) -> Result<Option<Intent>, &'static str> {
        let pending = self.store.as_ref().ok().and_then(|store| store.load().ok());
        let expected = self.item.as_ref().filter(|_| self.protected);
        match pending {
            Some(intent)
                if expected.is_none_or(|item| {
                    intent.as_ref().is_some_and(|saved| {
                        saved.slot == item.slot
                            && saved.context == item.context
                            && saved.request == item.request
                            && saved.text == item.text.as_str()
                    })
                }) =>
            {
                Ok(intent)
            }
            _ => {
                self.busy = true;
                self.progress = Progress::StorageBlocked;
                Err("storage_blocked")
            }
        }
    }

    pub(super) fn submit(
        &mut self,
        runtime: &mut impl SendRuntime,
        slot: u8,
        text: Zeroizing<String>,
    ) -> Submission {
        if self.conflict {
            return Submission::Retained;
        }
        match self.pending() {
            Ok(None) => self.busy = false,
            Ok(Some(intent)) => {
                self.remember(&intent);
                return Submission::Retained;
            }
            Err(_) => return Submission::Retained,
        }
        if !valid_text(&text)
            || !self
                .store
                .as_ref()
                .is_ok_and(|store| store.context(slot).is_ok())
            || runtime.check_send(slot).is_err()
        {
            self.progress = Progress::DraftRejected;
            return Submission::Retained;
        }
        let mut request = [0; 16];
        if getrandom::fill(&mut request).is_err()
            || !matches!(runtime.lookup(slot, request), Ok(None))
        {
            self.progress = Progress::DraftRejected;
            return Submission::Retained;
        }
        self.prepare_and_dispatch(runtime, slot, request, text)
    }

    fn prepare_and_dispatch(
        &mut self,
        runtime: &mut impl SendRuntime,
        slot: u8,
        request: [u8; 16],
        text: Zeroizing<String>,
    ) -> Submission {
        if runtime.operation().is_some_and(|op| !op.begin_prepare()) {
            return Submission::Retained;
        }
        match self
            .store
            .as_ref()
            .ok()
            .and_then(|store| store.prepare(slot, request, text).ok())
        {
            Some(intent) => {
                self.remember(&intent);
                if let Some(op) = runtime.operation() {
                    op.protected();
                }
                self.dispatch(runtime, intent);
                Submission::Protected
            }
            None => {
                self.busy = true;
                self.progress = Progress::StorageBlocked;
                Submission::Retained
            }
        }
    }

    pub(super) fn request(
        &mut self,
        runtime: &mut impl SendRuntime,
        slot: u8,
        request: [u8; 16],
        operation: RequestOperation,
    ) -> RequestResult {
        if slot >= 2 {
            return Err("invalid_context");
        }
        if matches!(&operation, RequestOperation::Send(text) if !valid_text(text)) {
            return Err("invalid_text");
        }
        if self.conflict {
            return Err("request_conflict");
        }
        let pending = self.pending()?;
        if let Some(intent) = pending {
            self.remember(&intent);
            if intent.request == request {
                if intent.slot != slot {
                    return Err("context_conflict");
                }
                if let RequestOperation::Send(text) = operation {
                    return Err(if intent.text == text.as_str() {
                        "request_known"
                    } else {
                        "text_conflict"
                    });
                }
                if let Some(op) = runtime.operation() {
                    op.protected();
                }
                self.reconcile_intent(
                    runtime,
                    intent,
                    matches!(operation, RequestOperation::Retry),
                );
                return self.request_report();
            }
            if matches!(operation, RequestOperation::Send(_)) {
                return Err("pending_request");
            }
        }
        let status = lookup_request(runtime, slot, request)?;
        if status.is_none() && lookup_request(runtime, 1 - slot, request)?.is_some() {
            return Err("context_conflict");
        }
        match operation {
            RequestOperation::Send(text) => {
                if status.is_some() {
                    return Err("request_known");
                }
                runtime.check_send(slot).map_err(|_| "send_unavailable")?;
                self.prepare_and_dispatch(runtime, slot, request, text);
                self.request_report()
            }
            RequestOperation::Retry if status.is_none() => Err("unknown_request"),
            RequestOperation::Status | RequestOperation::Retry => {
                let report = RequestReport::from_status(status);
                if let (Some(_), Some(op)) = (status, runtime.operation()) {
                    op.verified(report.clone());
                }
                Ok(report)
            }
        }
    }

    fn request_report(&self) -> RequestResult {
        let state = match self.progress {
            Progress::Unconfirmed => RequestState::Unconfirmed,
            Progress::RetryAvailable => RequestState::RetryAvailable,
            Progress::Queued => RequestState::Queued,
            Progress::Materialized => RequestState::Materialized,
            Progress::Delivered => RequestState::Delivered,
            Progress::Conflict => return Err("request_conflict"),
            _ => return Err("storage_blocked"),
        };
        Ok(RequestReport {
            state,
            protected: Some(self.protected),
            event_id: self.event.map(|event| super::artifact::hex(&event)),
        })
    }

    pub(super) fn refresh(&mut self, runtime: &mut impl SendRuntime) {
        self.reconcile(runtime, false);
    }

    pub(super) fn retry(&mut self, runtime: &mut impl SendRuntime) {
        self.reconcile(runtime, true);
    }

    fn reconcile(&mut self, runtime: &mut impl SendRuntime, retry: bool) {
        if self.conflict {
            return;
        }
        let intent = match self.pending() {
            Ok(None) => {
                self.busy = false;
                if !matches!(
                    self.progress,
                    Progress::Materialized | Progress::Delivered | Progress::DraftRejected
                ) {
                    self.progress = Progress::Empty;
                    self.item = None;
                    self.event = None;
                }
                return;
            }
            Ok(Some(intent)) => intent,
            Err(_) => return,
        };
        self.remember(&intent);
        self.reconcile_intent(runtime, intent, retry);
    }

    fn reconcile_intent(
        &mut self,
        runtime: &mut impl SendRuntime,
        mut intent: Intent,
        retry: bool,
    ) {
        if intent.phase == Phase::Conflict {
            self.conflict = true;
            self.progress = Progress::Conflict;
            return;
        }
        if self
            .known
            .is_some_and(|(request, _)| request != intent.request)
        {
            self.conflict = true;
            self.progress = Progress::StorageBlocked;
            return;
        }
        match runtime.lookup(intent.slot, intent.request) {
            Ok(Some(status)) => self.accept(intent, status, runtime.operation()),
            Ok(None) if self.known.is_some() || matches!(intent.phase, Phase::Accepted { .. }) => {
                self.block_conflict(&mut intent);
            }
            Ok(None) if retry => self.dispatch(runtime, intent),
            Ok(None) => self.progress = Progress::RetryAvailable,
            Err(_) => self.progress = Progress::Unconfirmed,
        }
    }

    fn dispatch(&mut self, runtime: &mut impl SendRuntime, mut intent: Intent) {
        match runtime.send(intent.slot, intent.request, &intent.text) {
            Ok(receipt) => {
                if let Some(op) = runtime.operation() {
                    let mut report = RequestReport::from_status(Some(RequestStatus2 {
                        receipt,
                        delivery: if receipt.event.is_some() {
                            Delivery2::Materialized
                        } else {
                            Delivery2::Queued
                        },
                    }));
                    report.protected = Some(true);
                    op.verified(report);
                }
                self.known = Some((intent.request, receipt));
                self.event = receipt.event;
                let phase = Phase::Accepted {
                    queue_sequence: receipt.queue_sequence,
                };
                self.progress = if self
                    .store
                    .as_ref()
                    .is_ok_and(|s| s.transition(&mut intent, phase).is_ok())
                {
                    if receipt.event.is_some() {
                        Progress::Materialized
                    } else {
                        Progress::Queued
                    }
                } else {
                    Progress::StorageBlocked
                };
            }
            Err(MessageError2::RequestIdConflict) => self.block_conflict(&mut intent),
            Err(_) => self.progress = Progress::Unconfirmed,
        }
    }

    fn accept(
        &mut self,
        mut intent: Intent,
        status: RequestStatus2,
        control: Option<&OperationControl>,
    ) {
        let phase = Phase::Accepted {
            queue_sequence: status.receipt.queue_sequence,
        };
        if matches!(intent.phase, Phase::Accepted { .. }) && intent.phase != phase
            || self.known.is_some_and(|(_, prior)| {
                prior.queue_sequence != status.receipt.queue_sequence
                    || prior.event.is_some() && prior.event != status.receipt.event
            })
        {
            self.block_conflict(&mut intent);
            return;
        }
        if let Some(op) = control {
            let mut report = RequestReport::from_status(Some(status));
            report.protected = Some(true);
            op.verified(report);
        }
        self.known = Some((intent.request, status.receipt));
        self.event = status.receipt.event;
        let Ok(store) = &self.store else {
            return;
        };
        if store.transition(&mut intent, phase).is_err() {
            self.progress = Progress::StorageBlocked;
            return;
        }
        self.progress = match status.delivery {
            Delivery2::Queued => Progress::Queued,
            _ if store.remove_accepted(&intent).is_err() => Progress::StorageBlocked,
            Delivery2::Materialized => Progress::Materialized,
            Delivery2::Delivered => Progress::Delivered,
        };
        if matches!(self.progress, Progress::Materialized | Progress::Delivered) {
            self.known = None;
            self.busy = false;
            self.protected = false;
            if let Some(op) = control {
                let mut report = RequestReport::from_status(Some(status));
                report.protected = Some(false);
                op.verified(report);
            }
        }
    }

    fn block_conflict(&mut self, intent: &mut Intent) {
        // Keep the in-memory latch even if persisting Conflict itself fails.
        self.conflict = true;
        self.progress = Progress::Conflict;
        if let Ok(store) = &self.store {
            let _ = store.transition(intent, Phase::Conflict);
        }
    }
}

fn lookup_request(
    runtime: &mut impl SendRuntime,
    slot: u8,
    request: [u8; 16],
) -> Result<Option<RequestStatus2>, &'static str> {
    runtime
        .lookup(slot, request)
        .map_err(|_| "status_unavailable")
}
