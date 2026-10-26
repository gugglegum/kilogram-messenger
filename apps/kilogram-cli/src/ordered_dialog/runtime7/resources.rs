//! Actual global pre-spawn permits and typed mailbox measurements.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};
#[derive(Default)]
pub(super) struct Resources7 {
    active: AtomicUsize,
    peak: AtomicUsize,
    refused: AtomicU64,
    max_queued: AtomicUsize,
    sender: Mutex<Option<tokio::sync::mpsc::WeakSender<super::runtime::Job7>>>,
}
pub(super) struct ConnectionCount7(Arc<Resources7>);
impl Resources7 {
    pub fn acquired(self: &Arc<Self>) -> ConnectionCount7 {
        let n = self.active.fetch_add(1, Ordering::AcqRel) + 1;
        debug_assert!(n <= 2);
        self.peak.fetch_max(n, Ordering::Relaxed);
        ConnectionCount7(self.clone())
    }
    pub fn refused(&self) {
        self.refused.fetch_add(1, Ordering::Relaxed);
    }
    pub fn mailbox(&self, sender: &tokio::sync::mpsc::Sender<super::runtime::Job7>) {
        *self
            .sender
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(sender.downgrade());
    }
    pub fn queued(&self, n: usize) {
        self.max_queued.fetch_max(n, Ordering::Relaxed);
    }
    pub fn snapshot(&self) -> serde_json::Value {
        let n = self
            .sender
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .and_then(|s| s.upgrade())
            .map_or(0, |s| s.max_capacity() - s.capacity());
        serde_json::json!({"connections":self.active.load(Ordering::Acquire),"peak_connections":self.peak.load(Ordering::Acquire),"refused_before_spawn":self.refused.load(Ordering::Acquire),"mailbox":n,"peak_mailbox":self.max_queued.load(Ordering::Acquire),"connection_limit":2,"mailbox_limit":16,"frame_limit":1024*1024,"bidi_limit":4,"uni_limit":0})
    }
}
impl Drop for ConnectionCount7 {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::AcqRel);
    }
}
