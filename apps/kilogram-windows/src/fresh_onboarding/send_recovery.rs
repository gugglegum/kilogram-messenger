//! Reconcile one protected intent with the durable local runtime queue.
use super::send_intent::{Intent, Phase, Store, valid_text};
use kilogram_runtime_ipc::ordered2::{Delivery2, MessageError2, RequestStatus2, SendReceipt2};
use std::sync::Arc;
use zeroize::Zeroizing;

pub(super) type IpcResult<T> = Result<T, MessageError2>;

pub(super) trait SendRuntime {
    fn check_send(&mut self, slot: u8) -> IpcResult<()>;
    fn lookup(&mut self, slot: u8, request: [u8; 16]) -> IpcResult<Option<RequestStatus2>>;
    fn send(&mut self, slot: u8, request: [u8; 16], text: &str) -> IpcResult<SendReceipt2>;
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Progress {
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
pub(super) enum Submission {
    Protected,
    Retained,
}

pub(super) struct Submitted {
    pub slot: u8,
    pub context: [u8; 32],
    pub request: [u8; 16],
    pub text: Zeroizing<String>,
}

#[derive(Clone)]
pub(super) struct Projection {
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
    pub(super) fn notice(self) -> Option<&'static str> {
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

    pub(super) fn submit(
        &mut self,
        runtime: &mut impl SendRuntime,
        slot: u8,
        text: Zeroizing<String>,
    ) -> Submission {
        if self.conflict {
            return Submission::Retained;
        }
        let Ok(store) = &self.store else {
            return Submission::Retained;
        };
        match store.load() {
            Ok(None) if self.known.is_none() => self.busy = false,
            Ok(Some(intent)) => {
                self.remember(&intent);
                return Submission::Retained;
            }
            Ok(None) | Err(_) => {
                self.busy = true;
                self.progress = Progress::StorageBlocked;
                return Submission::Retained;
            }
        }
        if !valid_text(&text) || store.context(slot).is_err() || runtime.check_send(slot).is_err() {
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
        match store.prepare(slot, request, text) {
            Ok(intent) => {
                self.remember(&intent);
                self.dispatch(runtime, intent);
                Submission::Protected
            }
            Err(_) => {
                self.busy = true;
                self.progress = Progress::StorageBlocked;
                Submission::Retained
            }
        }
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
        let Ok(store) = &self.store else {
            return;
        };
        let mut intent = match store.load() {
            Ok(None) => {
                if self.known.is_some() {
                    self.busy = true;
                    self.progress = Progress::StorageBlocked;
                    return;
                }
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
            Err(_) => {
                self.busy = true;
                self.progress = Progress::StorageBlocked;
                return;
            }
        };
        self.remember(&intent);
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
            Ok(Some(status)) => self.accept(intent, status),
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

    fn accept(&mut self, mut intent: Intent, status: RequestStatus2) {
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
