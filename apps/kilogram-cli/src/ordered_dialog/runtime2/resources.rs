//! Measurements follow actual permits. Limits are acquired before TLS/task/frame work.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};
#[derive(Default)]
pub(super) struct Resources2 {
    active: AtomicUsize,
    peak: AtomicUsize,
    refused: AtomicU64,
    mailbox: Mutex<Option<tokio::sync::mpsc::WeakSender<super::runtime::Job2>>>,
    max_queued: AtomicUsize,
}
pub(super) struct ConnectionCount2(Arc<Resources2>);
impl Resources2 {
    pub fn acquired(self: &Arc<Self>) -> ConnectionCount2 {
        let n = self.active.fetch_add(1, Ordering::AcqRel) + 1;
        debug_assert!(n <= 2);
        self.peak.fetch_max(n, Ordering::Relaxed);
        ConnectionCount2(self.clone())
    }
    pub fn refused(&self) {
        self.refused.fetch_add(1, Ordering::Relaxed);
    }
    pub fn bind_mailbox(&self, sender: &tokio::sync::mpsc::Sender<super::runtime::Job2>) {
        *self
            .mailbox
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(sender.downgrade());
    }
    pub fn observed_mailbox(&self, count: usize) {
        self.max_queued.fetch_max(count, Ordering::Relaxed);
    }
    pub fn snapshot(&self) -> serde_json::Value {
        let queued = self
            .mailbox
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .and_then(|s| s.upgrade())
            .map_or(0, |s| s.max_capacity() - s.capacity());
        serde_json::json!({"connections":self.active.load(Ordering::Acquire),"peak_connections":self.peak.load(Ordering::Acquire),"refused_before_spawn":self.refused.load(Ordering::Acquire),"mailbox":queued,"peak_mailbox":self.max_queued.load(Ordering::Acquire),"connection_limit":2,"mailbox_limit":16,"frame_limit":1024*1024,"bidi_limit":4,"uni_limit":0})
    }
}
impl Drop for ConnectionCount2 {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::AcqRel);
    }
}
