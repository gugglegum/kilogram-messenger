//! Bounded network adapter and durable client-side state machine for blind mailboxes.
//!
//! The ledger stores already encrypted outbound requests and signed store
//! receipts. For inbound delivery it exposes an explicit prepare -> application
//! durable commit -> delete sequence; callers must not record the commit until
//! their own event/history transaction has completed.

mod http;
mod ledger;

pub use http::{MAILBOX_HTTP_CONTENT_TYPE, MailboxHttpClient};
pub use ledger::{
    MailboxClientCleanupReport, MailboxClientLedger, MailboxClientLedgerConfig,
    MailboxOutboundState, OutboundEnqueueOutcome, PendingMailboxUpload, PreparedInboundItem,
    StoredOutboundReceipt,
};
