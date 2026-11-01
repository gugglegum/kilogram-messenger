//! Bounded metadata from the real accepted Head, never protocol authority.
use super::*;
#[derive(Clone, Copy)]
pub(super) struct AcceptedHead {
    pub context: Id,
    pub binding: Id,
    pub generation: u64,
    pub received: u64,
}
impl AcceptedHead {
    pub(super) fn check(self, context: Id, binding: Id, generation: u64, after: u64) -> Result<()> {
        ensure!(
            self.context == context
                && self.binding == binding
                && self.generation == generation
                && self.received == after
                && generation > 0,
            "S54 selector accepted Head current C/binding/generation/frontier"
        );
        Ok(())
    }
}
