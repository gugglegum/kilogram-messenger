//! Offline verification only: close the database while retaining its actual lease.
use super::*;

impl L1Repository {
    pub fn verification_close_to_held_lease(self) -> StateDirectoryLock {
        // The existing core primitive drops database/backend/crypto before it
        // returns the very same lease; no reopen, acquisition or recovery.
        self.core.into_owner_lease().0
    }
}
