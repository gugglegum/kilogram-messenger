//! Hard per-context shares of the profile scheduling window. Debt remains on disk.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
pub(super) struct RefWindow2 {
    limits: [usize; 2],
    current: [AtomicUsize; 2],
    peak: [AtomicUsize; 2],
}
pub(super) struct RefPermit2 {
    window: Arc<RefWindow2>,
    c: usize,
    count: usize,
}
impl RefWindow2 {
    pub fn new(enabled: &[u8]) -> Arc<Self> {
        let mut limits = [0; 2];
        for c in enabled {
            limits[usize::from(*c)] = if enabled.len() == 2 { 64 } else { 128 };
        }
        Arc::new(Self {
            limits,
            current: [AtomicUsize::new(0), AtomicUsize::new(0)],
            peak: [AtomicUsize::new(0), AtomicUsize::new(0)],
        })
    }
    pub fn reserve(self: &Arc<Self>, c: u8, count: usize) -> Result<RefPermit2> {
        ensure!(c < 2 && count > 0, "window reservation");
        let c = usize::from(c);
        let old = self.current[c]
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(count).filter(|n| *n <= self.limits[c])
            })
            .map_err(|_| anyhow::anyhow!("SchedulingWindowBusy"))?;
        self.peak[c].fetch_max(old + count, Ordering::Relaxed);
        Ok(RefPermit2 {
            window: self.clone(),
            c,
            count,
        })
    }
    pub fn snapshot(&self) -> ([usize; 2], [usize; 2], [usize; 2]) {
        (
            self.limits,
            std::array::from_fn(|c| self.current[c].load(Ordering::Acquire)),
            std::array::from_fn(|c| self.peak[c].load(Ordering::Acquire)),
        )
    }
}
impl Drop for RefPermit2 {
    fn drop(&mut self) {
        self.window.current[self.c].fetch_sub(self.count, Ordering::AcqRel);
    }
}
