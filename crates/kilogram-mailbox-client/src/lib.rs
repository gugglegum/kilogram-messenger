//! Bounded network adapters and durable client-side state for blind mailboxes.
//!
//! The ledger stores already encrypted outbound requests and signed store
//! receipts. For inbound delivery it exposes an explicit prepare -> application
//! durable commit -> delete sequence; callers must not record the commit until
//! their own event/history transaction has completed.
//! The separate provider registry retains only verified public storage offers
//! and never mixes them with a mailbox capability or social identity.

mod http;
mod ledger;
mod provider;

pub use http::{MAILBOX_HTTP_CONTENT_TYPE, MailboxHttpClient};
pub use ledger::{
    MailboxClientCleanupReport, MailboxClientLedger, MailboxClientLedgerConfig,
    MailboxOutboundState, OutboundEnqueueOutcome, PendingMailboxUpload, PreparedInboundItem,
    StoredOutboundReceipt,
};
pub use provider::{
    DEFAULT_MAX_PROVIDER_OFFERS, MAX_PROVIDER_GOSSIP_AGE_SECONDS, MAX_PROVIDER_GOSSIP_FRAME_BYTES,
    MAX_PROVIDER_GOSSIP_FRAME_VALIDITY_SECONDS, MAX_PROVIDER_GOSSIP_HOPS,
    MAX_PROVIDER_GOSSIP_OFFER_BYTES, MAX_PROVIDER_GOSSIP_OFFERS, MAX_PROVIDER_SELECTION,
    MailboxProviderGossipEntry, MailboxProviderGossipFrame, MailboxProviderImportOutcome,
    MailboxProviderOffer, MailboxProviderOfferId, MailboxProviderRegistry,
    MailboxProviderRegistryConfig,
};
