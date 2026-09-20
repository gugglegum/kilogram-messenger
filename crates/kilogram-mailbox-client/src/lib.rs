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
mod replication;

pub use http::{MAILBOX_HTTP_CONTENT_TYPE, MailboxHttpClient};
pub use ledger::{
    MailboxClientCleanupReport, MailboxClientLedger, MailboxClientLedgerConfig,
    MailboxClientReadOnlyInspection, MailboxOutboundState, OutboundEnqueueOutcome,
    PendingMailboxUpload, PreparedInboundItem, ReplicatedOutboundCommit, StoredOutboundReceipt,
};
pub use provider::{
    DEFAULT_MAX_PROVIDER_OFFERS, DEFAULT_PROVIDER_ADMISSION_WORK_BITS,
    MAX_AUTHENTICATED_PROVIDER_OBSERVATIONS_PER_OFFER, MAX_PROVIDER_GOSSIP_AGE_SECONDS,
    MAX_PROVIDER_GOSSIP_FRAME_BYTES, MAX_PROVIDER_GOSSIP_FRAME_VALIDITY_SECONDS,
    MAX_PROVIDER_GOSSIP_HOPS, MAX_PROVIDER_GOSSIP_OFFER_BYTES, MAX_PROVIDER_GOSSIP_OFFERS,
    MAX_PROVIDER_SELECTION, MIN_AUTHENTICATED_PROVIDER_OBSERVATIONS_FOR_PREFERENCE,
    MailboxProviderGossipEntry, MailboxProviderGossipFrame, MailboxProviderImportOutcome,
    MailboxProviderLocalObserverTag, MailboxProviderObservationOutcome, MailboxProviderOffer,
    MailboxProviderOfferId, MailboxProviderRegistry, MailboxProviderRegistryConfig,
};
pub use replication::{
    DEFAULT_REPLICATION_RETRY_SECONDS, DEFAULT_REPLICATION_TARGETS,
    DEFAULT_REQUIRED_REPLICA_RECEIPTS, MAX_REPLICA_DELETE_BATCH, MAX_REPLICATION_TARGETS,
    MailboxReplicaReceipt, MailboxReplicaSetLocator, MailboxReplicationCleanupReport,
    MailboxReplicationLedger, MailboxReplicationLedgerConfig, MailboxReplicationPlan,
    MailboxReplicationReadOnlyInspection, MailboxReplicationStatus, PendingReplicaDelete,
    ReplicationPlanOutcome,
};
