use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use kilogram_mailbox::{
    MAX_MAILBOX_WIRE_REQUEST_BYTES, MailboxAddress, MailboxDeleteReceipt, MailboxDeleteRequest,
    MailboxDeleteResponse, MailboxEnvelope, MailboxId, MailboxItemId, MailboxPutRequest,
    MailboxPutResponse, MailboxReadCapability, MailboxReceiptId, MailboxStoreKey,
    MailboxStoredReceipt,
};
use redb::{
    Database, Durability, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition,
};
use serde::{Deserialize, Serialize};

use crate::ledger::PreparedInboundItem;

const DATABASE_FILE: &str = "mailbox-replication-ledger.redb";
const PLAN_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("mailbox-replication-plans-v1");
const RECEIPT_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("mailbox-replication-receipts-v1");
const ATTEMPT_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("mailbox-replication-attempts-v1");
const INBOUND_COMMIT_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("mailbox-replica-inbound-commits-v1");
const INBOUND_DELETED_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("mailbox-replica-inbound-deleted-v1");
const RECORD_VERSION: u8 = 1;
const ITEM_KEY_BYTES: usize = 64;
const RECEIPT_KEY_BYTES: usize = ITEM_KEY_BYTES + 32;
const MAX_PLAN_RECORD_BYTES: usize = MAX_MAILBOX_WIRE_REQUEST_BYTES + 4 * 1024;
const MAX_RECEIPT_RECORD_BYTES: usize = 4 * 1024;
const MAX_ABSOLUTE_RECORDS: u64 = 1_000_000;

pub const DEFAULT_REPLICATION_TARGETS: u8 = 3;
pub const DEFAULT_REQUIRED_REPLICA_RECEIPTS: u8 = 2;
pub const MAX_REPLICATION_TARGETS: u8 = 8;
pub const DEFAULT_MAX_REPLICATION_PLANS: u64 = 4_096;
pub const DEFAULT_MAX_REPLICA_RECEIPTS: u64 = 32_768;
pub const DEFAULT_REPLICATION_RETRY_SECONDS: u64 = 60;
pub const MAX_REPLICA_DELETE_BATCH: u8 = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MailboxReplicationLedgerConfig {
    pub data_dir: PathBuf,
    pub max_plans: u64,
    pub max_receipts: u64,
}

impl MailboxReplicationLedgerConfig {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            max_plans: DEFAULT_MAX_REPLICATION_PLANS,
            max_receipts: DEFAULT_MAX_REPLICA_RECEIPTS,
        }
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            !self.data_dir.as_os_str().is_empty(),
            "mailbox replication ledger directory is empty"
        );
        ensure!(
            (1..=MAX_ABSOLUTE_RECORDS).contains(&self.max_plans)
                && (1..=MAX_ABSOLUTE_RECORDS).contains(&self.max_receipts),
            "mailbox replication ledger limits are invalid"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ReplicationPlanRecord {
    version: u8,
    mailbox_id: MailboxId,
    item_id: MailboxItemId,
    request_digest: [u8; 32],
    request: MailboxPutRequest,
    dispatch_binding: [u8; 32],
    selection_salt: [u8; 32],
    requested_replicas: u8,
    required_receipts: u8,
    created_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
}

impl ReplicationPlanRecord {
    fn validate(&self) -> Result<()> {
        MailboxPutRequest::decode_and_verify(&self.request.encode()?)?;
        let envelope = MailboxEnvelope::decode(self.request.envelope())?;
        ensure!(
            self.version == RECORD_VERSION
                && self.mailbox_id == self.request.mailbox_id()
                && self.item_id == self.request.item_id()
                && self.request_digest == request_digest(&self.request)?
                && self.dispatch_binding != [0_u8; 32]
                && self.selection_salt != [0_u8; 32]
                && (1..=MAX_REPLICATION_TARGETS).contains(&self.requested_replicas)
                && (1..=self.requested_replicas).contains(&self.required_receipts)
                && self.created_at_unix_seconds < self.expires_at_unix_seconds
                && self.expires_at_unix_seconds == envelope.expires_at_unix_seconds(),
            "mailbox replication plan metadata is invalid"
        );
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = postcard::to_allocvec(self).context("encode mailbox replication plan")?;
        ensure!(
            bytes.len() <= MAX_PLAN_RECORD_BYTES,
            "mailbox replication plan is too large"
        );
        Ok(bytes)
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_PLAN_RECORD_BYTES,
            "mailbox replication plan is too large"
        );
        let record: Self =
            postcard::from_bytes(bytes).context("decode mailbox replication plan")?;
        record.validate()?;
        Ok(record)
    }

    fn into_public(self) -> MailboxReplicationPlan {
        MailboxReplicationPlan {
            mailbox_id: self.mailbox_id,
            item_id: self.item_id,
            request: self.request,
            dispatch_binding: self.dispatch_binding,
            selection_salt: self.selection_salt,
            requested_replicas: self.requested_replicas,
            required_receipts: self.required_receipts,
            expires_at_unix_seconds: self.expires_at_unix_seconds,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MailboxReplicationPlan {
    mailbox_id: MailboxId,
    item_id: MailboxItemId,
    request: MailboxPutRequest,
    dispatch_binding: [u8; 32],
    selection_salt: [u8; 32],
    requested_replicas: u8,
    required_receipts: u8,
    expires_at_unix_seconds: u64,
}

impl MailboxReplicationPlan {
    pub fn mailbox_id(&self) -> MailboxId {
        self.mailbox_id
    }

    pub fn item_id(&self) -> MailboxItemId {
        self.item_id
    }

    pub fn request(&self) -> &MailboxPutRequest {
        &self.request
    }

    pub fn dispatch_binding(&self) -> &[u8; 32] {
        &self.dispatch_binding
    }

    pub fn selection_salt(&self) -> [u8; 32] {
        self.selection_salt
    }

    pub fn requested_replicas(&self) -> u8 {
        self.requested_replicas
    }

    pub fn required_receipts(&self) -> u8 {
        self.required_receipts
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.expires_at_unix_seconds
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplicationPlanOutcome {
    Created,
    AlreadyPresent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ReplicaReceiptRecord {
    version: u8,
    mailbox_id: MailboxId,
    item_id: MailboxItemId,
    request_digest: [u8; 32],
    dispatch_binding: [u8; 32],
    transport_identity: [u8; 32],
    store_key: MailboxStoreKey,
    receipt: MailboxStoredReceipt,
    recorded_at_unix_seconds: u64,
}

impl ReplicaReceiptRecord {
    fn validate(&self) -> Result<()> {
        self.receipt.verify_signature()?;
        ensure!(
            self.version == RECORD_VERSION
                && self.dispatch_binding != [0_u8; 32]
                && self.transport_identity != [0_u8; 32]
                && self.receipt.mailbox_id() == self.mailbox_id
                && self.receipt.item_id() == self.item_id
                && self.receipt.store_key() == self.store_key,
            "mailbox replica receipt metadata is invalid"
        );
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        encode_receipt_record(self, "mailbox replica receipt")
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        let record: Self = decode_receipt_record(bytes, "mailbox replica receipt")?;
        record.validate()?;
        Ok(record)
    }

    fn into_public(self) -> MailboxReplicaReceipt {
        MailboxReplicaReceipt {
            transport_identity: self.transport_identity,
            store_key: self.store_key,
            receipt: self.receipt,
            recorded_at_unix_seconds: self.recorded_at_unix_seconds,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MailboxReplicaReceipt {
    transport_identity: [u8; 32],
    store_key: MailboxStoreKey,
    receipt: MailboxStoredReceipt,
    recorded_at_unix_seconds: u64,
}

impl MailboxReplicaReceipt {
    pub fn transport_identity(&self) -> &[u8; 32] {
        &self.transport_identity
    }

    pub fn store_key(&self) -> MailboxStoreKey {
        self.store_key
    }

    pub fn receipt(&self) -> &MailboxStoredReceipt {
        &self.receipt
    }

    pub fn recorded_at_unix_seconds(&self) -> u64 {
        self.recorded_at_unix_seconds
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ReplicaInboundCommitRecord {
    version: u8,
    mailbox_id: MailboxId,
    item_id: MailboxItemId,
    transport_identity: [u8; 32],
    expected_store_key: MailboxStoreKey,
    stored_receipt: MailboxStoredReceipt,
    application_commit_id: [u8; 32],
    committed_at_unix_seconds: u64,
}

impl ReplicaInboundCommitRecord {
    fn validate(&self) -> Result<()> {
        self.stored_receipt.verify_signature()?;
        ensure!(
            self.version == RECORD_VERSION
                && self.transport_identity != [0_u8; 32]
                && self.stored_receipt.mailbox_id() == self.mailbox_id
                && self.stored_receipt.item_id() == self.item_id
                && self.stored_receipt.store_key() == self.expected_store_key,
            "inbound replica commit metadata is invalid"
        );
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        encode_receipt_record(self, "inbound replica commit")
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        let record: Self = decode_receipt_record(bytes, "inbound replica commit")?;
        record.validate()?;
        Ok(record)
    }

    fn same_logical_record(&self, other: &Self) -> bool {
        self.version == other.version
            && self.mailbox_id == other.mailbox_id
            && self.item_id == other.item_id
            && self.transport_identity == other.transport_identity
            && self.expected_store_key == other.expected_store_key
            && self.stored_receipt == other.stored_receipt
            && self.application_commit_id == other.application_commit_id
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ReplicaInboundDeletedRecord {
    version: u8,
    mailbox_id: MailboxId,
    item_id: MailboxItemId,
    transport_identity: [u8; 32],
    expected_store_key: MailboxStoreKey,
    stored_receipt_id: MailboxReceiptId,
    delete_receipt: MailboxDeleteReceipt,
    recorded_at_unix_seconds: u64,
}

impl ReplicaInboundDeletedRecord {
    fn validate(&self) -> Result<()> {
        self.delete_receipt.verify()?;
        ensure!(
            self.version == RECORD_VERSION
                && self.transport_identity != [0_u8; 32]
                && self.delete_receipt.mailbox_id() == self.mailbox_id
                && self.delete_receipt.item_id() == self.item_id
                && self.delete_receipt.store_key() == self.expected_store_key
                && self.delete_receipt.stored_receipt_id() == self.stored_receipt_id,
            "deleted inbound replica metadata is invalid"
        );
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        encode_receipt_record(self, "deleted inbound replica")
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        let record: Self = decode_receipt_record(bytes, "deleted inbound replica")?;
        record.validate()?;
        Ok(record)
    }

    fn same_logical_record(&self, other: &Self) -> bool {
        self.version == other.version
            && self.mailbox_id == other.mailbox_id
            && self.item_id == other.item_id
            && self.transport_identity == other.transport_identity
            && self.expected_store_key == other.expected_store_key
            && self.stored_receipt_id == other.stored_receipt_id
            && self.delete_receipt == other.delete_receipt
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingReplicaDelete {
    request: MailboxDeleteRequest,
    expected_store_key: MailboxStoreKey,
    transport_identity: [u8; 32],
}

impl PendingReplicaDelete {
    pub fn request(&self) -> &MailboxDeleteRequest {
        &self.request
    }

    pub fn expected_store_key(&self) -> MailboxStoreKey {
        self.expected_store_key
    }

    pub fn transport_identity(&self) -> &[u8; 32] {
        &self.transport_identity
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MailboxReplicationStatus {
    pub plan: MailboxReplicationPlan,
    pub receipts: Vec<MailboxReplicaReceipt>,
}

impl MailboxReplicationStatus {
    pub fn is_satisfied(&self) -> bool {
        self.receipts.len() >= usize::from(self.plan.required_receipts())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MailboxReplicationCleanupReport {
    pub removed_plans: u64,
    pub removed_receipts: u64,
    pub removed_attempts: u64,
    pub removed_inbound_commits: u64,
    pub removed_inbound_deletions: u64,
}

pub struct MailboxReplicationLedger {
    database: Database,
    config: MailboxReplicationLedgerConfig,
}

impl MailboxReplicationLedger {
    pub fn open(config: MailboxReplicationLedgerConfig) -> Result<Self> {
        config.validate()?;
        std::fs::create_dir_all(&config.data_dir).with_context(|| {
            format!(
                "create mailbox replication ledger directory {}",
                config.data_dir.display()
            )
        })?;
        let metadata = std::fs::symlink_metadata(&config.data_dir)
            .context("inspect mailbox replication ledger directory")?;
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "mailbox replication ledger directory must be a real directory, not a symlink"
        );
        let canonical = std::fs::canonicalize(&config.data_dir)
            .context("canonicalize mailbox replication ledger directory")?;
        let database = Database::create(canonical.join(DATABASE_FILE))
            .context("open mailbox replication ledger")?;
        let write = database
            .begin_write()
            .context("begin mailbox replication ledger initialization")?;
        write.open_table(PLAN_TABLE)?;
        write.open_table(RECEIPT_TABLE)?;
        write.open_table(ATTEMPT_TABLE)?;
        write.open_table(INBOUND_COMMIT_TABLE)?;
        write.open_table(INBOUND_DELETED_TABLE)?;
        write
            .commit()
            .context("commit mailbox replication ledger initialization")?;
        Ok(Self { database, config })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn ensure_plan(
        &self,
        request: &MailboxPutRequest,
        dispatch_binding: [u8; 32],
        candidate_selection_salt: [u8; 32],
        requested_replicas: u8,
        required_receipts: u8,
        created_at_unix_seconds: u64,
    ) -> Result<(ReplicationPlanOutcome, MailboxReplicationPlan)> {
        MailboxPutRequest::decode_and_verify(&request.encode()?)?;
        let envelope = MailboxEnvelope::decode(request.envelope())?;
        let record = ReplicationPlanRecord {
            version: RECORD_VERSION,
            mailbox_id: request.mailbox_id(),
            item_id: request.item_id(),
            request_digest: request_digest(request)?,
            request: request.clone(),
            dispatch_binding,
            selection_salt: candidate_selection_salt,
            requested_replicas,
            required_receipts,
            created_at_unix_seconds,
            expires_at_unix_seconds: envelope.expires_at_unix_seconds(),
        };
        record.validate()?;
        let encoded = record.encode()?;
        let key = item_key(record.mailbox_id, record.item_id);
        let mut write = self.database.begin_write()?;
        write.set_durability(Durability::Immediate)?;
        if let Some(current) = read_value(&write, PLAN_TABLE, key.as_slice())? {
            let current = ReplicationPlanRecord::decode(&current)?;
            ensure!(
                current.mailbox_id == record.mailbox_id
                    && current.item_id == record.item_id
                    && current.request_digest == record.request_digest
                    && current.request == record.request
                    && current.dispatch_binding == record.dispatch_binding
                    && current.requested_replicas == record.requested_replicas
                    && current.required_receipts == record.required_receipts
                    && current.expires_at_unix_seconds == record.expires_at_unix_seconds,
                "mailbox replication plan conflicts with durable state"
            );
            let public = current.into_public();
            write.commit()?;
            return Ok((ReplicationPlanOutcome::AlreadyPresent, public));
        }
        ensure!(
            write.open_table(PLAN_TABLE)?.len()? < self.config.max_plans,
            "mailbox replication plan capacity exceeded"
        );
        write
            .open_table(PLAN_TABLE)?
            .insert(key.as_slice(), encoded.as_slice())?;
        write.commit()?;
        Ok((ReplicationPlanOutcome::Created, record.into_public()))
    }

    pub fn status(
        &self,
        mailbox_id: MailboxId,
        item_id: MailboxItemId,
    ) -> Result<Option<MailboxReplicationStatus>> {
        let read = self.database.begin_read()?;
        let item_key = item_key(mailbox_id, item_id);
        let Some(plan) = read.open_table(PLAN_TABLE)?.get(item_key.as_slice())? else {
            return Ok(None);
        };
        let plan = ReplicationPlanRecord::decode(plan.value())?;
        let mut receipts = Vec::new();
        for entry in read.open_table(RECEIPT_TABLE)?.iter()? {
            let (key, value) = entry?;
            ensure!(
                key.value().len() == RECEIPT_KEY_BYTES,
                "mailbox replication ledger contains an invalid receipt key"
            );
            if key.value()[..ITEM_KEY_BYTES] == item_key {
                let receipt = ReplicaReceiptRecord::decode(value.value())?;
                ensure!(
                    receipt.request_digest == plan.request_digest
                        && receipt.dispatch_binding == plan.dispatch_binding
                        && key.value()
                            == receipt_key(receipt.mailbox_id, receipt.item_id, receipt.store_key),
                    "mailbox replica receipt does not match its durable plan"
                );
                receipts.push(receipt.into_public());
            }
        }
        receipts.sort_by_key(MailboxReplicaReceipt::store_key);
        Ok(Some(MailboxReplicationStatus {
            plan: plan.into_public(),
            receipts,
        }))
    }

    pub fn record_receipt(
        &self,
        request: &MailboxPutRequest,
        dispatch_binding: [u8; 32],
        transport_identity: [u8; 32],
        store_key: MailboxStoreKey,
        response: &MailboxPutResponse,
        recorded_at_unix_seconds: u64,
    ) -> Result<MailboxReplicaReceipt> {
        let receipt = response
            .stored_receipt()
            .context("volunteer mailbox response does not prove durable acceptance")?;
        MailboxPutResponse::decode_and_verify(
            &response.encode(request, store_key)?,
            request,
            store_key,
        )?;
        let record = ReplicaReceiptRecord {
            version: RECORD_VERSION,
            mailbox_id: request.mailbox_id(),
            item_id: request.item_id(),
            request_digest: request_digest(request)?,
            dispatch_binding,
            transport_identity,
            store_key,
            receipt: receipt.clone(),
            recorded_at_unix_seconds,
        };
        let encoded = record.encode()?;
        let item_key = item_key(record.mailbox_id, record.item_id);
        let receipt_key = receipt_key(record.mailbox_id, record.item_id, store_key);
        let mut write = self.database.begin_write()?;
        write.set_durability(Durability::Immediate)?;
        let plan = read_value(&write, PLAN_TABLE, item_key.as_slice())?
            .context("mailbox replication plan is absent before receipt commit")?;
        let plan = ReplicationPlanRecord::decode(&plan)?;
        ensure!(
            plan.request_digest == record.request_digest
                && plan.dispatch_binding == record.dispatch_binding,
            "mailbox replica receipt does not match its durable plan"
        );
        if let Some(current) = read_value(&write, RECEIPT_TABLE, receipt_key.as_slice())? {
            let current = ReplicaReceiptRecord::decode(&current)?;
            ensure!(
                current == record,
                "mailbox replica receipt replay conflicts"
            );
            let public = current.into_public();
            write.commit()?;
            return Ok(public);
        }
        for entry in write.open_table(RECEIPT_TABLE)?.iter()? {
            let (key, value) = entry?;
            if key.value().len() == RECEIPT_KEY_BYTES && key.value()[..ITEM_KEY_BYTES] == item_key {
                let current = ReplicaReceiptRecord::decode(value.value())?;
                ensure!(
                    current.transport_identity != transport_identity,
                    "mailbox replica receipts do not represent transport-distinct providers"
                );
            }
        }
        ensure!(
            write.open_table(RECEIPT_TABLE)?.len()? < self.config.max_receipts,
            "mailbox replica receipt capacity exceeded"
        );
        write
            .open_table(RECEIPT_TABLE)?
            .insert(receipt_key.as_slice(), encoded.as_slice())?;
        write.commit()?;
        Ok(record.into_public())
    }

    pub fn mark_attempt(
        &self,
        plan: &MailboxReplicationPlan,
        attempted_at_unix_seconds: u64,
        retry_seconds: u64,
    ) -> Result<bool> {
        ensure!(
            retry_seconds != 0 && attempted_at_unix_seconds < plan.expires_at_unix_seconds(),
            "mailbox replication attempt is outside the plan lifetime"
        );
        let key = item_key(plan.mailbox_id(), plan.item_id());
        let mut write = self.database.begin_write()?;
        write.set_durability(Durability::Immediate)?;
        let record = read_value(&write, PLAN_TABLE, key.as_slice())?
            .context("mailbox replication plan is absent before attempt")?;
        let record = ReplicationPlanRecord::decode(&record)?;
        ensure!(
            record.dispatch_binding == *plan.dispatch_binding()
                && record.request == *plan.request(),
            "mailbox replication attempt does not match its durable plan"
        );
        if let Some(current) = read_value(&write, ATTEMPT_TABLE, key.as_slice())? {
            let current = decode_attempt_time(&current)?;
            ensure!(
                attempted_at_unix_seconds >= current,
                "mailbox replication attempt time moved backwards"
            );
            if attempted_at_unix_seconds.saturating_sub(current) < retry_seconds {
                write.commit()?;
                return Ok(false);
            }
        }
        write.open_table(ATTEMPT_TABLE)?.insert(
            key.as_slice(),
            attempted_at_unix_seconds.to_le_bytes().as_slice(),
        )?;
        write.commit()?;
        Ok(true)
    }

    pub fn next_due(
        &self,
        now_unix_seconds: u64,
        retry_seconds: u64,
    ) -> Result<Option<MailboxReplicationPlan>> {
        ensure!(
            retry_seconds != 0,
            "mailbox replication retry interval is zero"
        );
        let read = self.database.begin_read()?;
        let attempts = read.open_table(ATTEMPT_TABLE)?;
        let receipts = read.open_table(RECEIPT_TABLE)?;
        let mut due = Vec::new();
        for entry in read.open_table(PLAN_TABLE)?.iter()? {
            let (key, value) = entry?;
            ensure!(
                key.value().len() == ITEM_KEY_BYTES,
                "mailbox replication ledger contains an invalid plan key"
            );
            let plan = ReplicationPlanRecord::decode(value.value())?;
            if plan.expires_at_unix_seconds <= now_unix_seconds {
                continue;
            }
            let mut receipt_count = 0_usize;
            let mut receipt_transports = std::collections::BTreeSet::new();
            for receipt_entry in receipts.iter()? {
                let (receipt_key, receipt_value) = receipt_entry?;
                ensure!(
                    receipt_key.value().len() == RECEIPT_KEY_BYTES,
                    "mailbox replication ledger contains an invalid receipt key"
                );
                if receipt_key.value()[..ITEM_KEY_BYTES] == key.value()[..] {
                    let receipt = ReplicaReceiptRecord::decode(receipt_value.value())?;
                    ensure!(
                        receipt.request_digest == plan.request_digest
                            && receipt.dispatch_binding == plan.dispatch_binding
                            && receipt_key.value()
                                == crate::replication::receipt_key(
                                    receipt.mailbox_id,
                                    receipt.item_id,
                                    receipt.store_key,
                                )
                            && receipt_transports.insert(receipt.transport_identity),
                        "mailbox replica receipt does not match its durable plan"
                    );
                    receipt_count += 1;
                }
            }
            if receipt_count >= usize::from(plan.required_receipts) {
                continue;
            }
            let last_attempt = attempts
                .get(key.value())?
                .map(|value| decode_attempt_time(value.value()))
                .transpose()?;
            if last_attempt
                .is_none_or(|last| now_unix_seconds.saturating_sub(last) >= retry_seconds)
            {
                due.push(plan);
            }
        }
        due.sort_by_key(|plan| (plan.created_at_unix_seconds, plan.item_id));
        Ok(due
            .into_iter()
            .next()
            .map(ReplicationPlanRecord::into_public))
    }

    /// Record this only after the caller's event/history transaction has
    /// durably committed. Each provider source is tracked independently so an
    /// already committed replicated item can still be deleted from every store.
    pub fn record_inbound_commit(
        &self,
        prepared: &PreparedInboundItem,
        transport_identity: [u8; 32],
        application_commit_id: [u8; 32],
        committed_at_unix_seconds: u64,
    ) -> Result<()> {
        let record = ReplicaInboundCommitRecord {
            version: RECORD_VERSION,
            mailbox_id: prepared.mailbox_id(),
            item_id: prepared.item_id(),
            transport_identity,
            expected_store_key: prepared.expected_store_key(),
            stored_receipt: prepared.stored_receipt().clone(),
            application_commit_id,
            committed_at_unix_seconds,
        };
        let encoded = record.encode()?;
        let key = receipt_key(record.mailbox_id, record.item_id, record.expected_store_key);
        let mut write = self.database.begin_write()?;
        write.set_durability(Durability::Immediate)?;
        if let Some(deleted) = read_value(&write, INBOUND_DELETED_TABLE, key.as_slice())? {
            let deleted = ReplicaInboundDeletedRecord::decode(&deleted)?;
            ensure!(
                deleted.transport_identity == transport_identity
                    && deleted.expected_store_key == prepared.expected_store_key()
                    && deleted.stored_receipt_id == prepared.stored_receipt_id()?,
                "already-deleted inbound replica conflicts with prepared input"
            );
            write.commit()?;
            return Ok(());
        }
        if let Some(current) = read_value(&write, INBOUND_COMMIT_TABLE, key.as_slice())? {
            let current = ReplicaInboundCommitRecord::decode(&current)?;
            ensure!(
                current.same_logical_record(&record),
                "inbound replica commit replay conflicts"
            );
            write.commit()?;
            return Ok(());
        }
        ensure!(
            write.open_table(INBOUND_COMMIT_TABLE)?.len()? < self.config.max_receipts,
            "inbound replica commit capacity exceeded"
        );
        write
            .open_table(INBOUND_COMMIT_TABLE)?
            .insert(key.as_slice(), encoded.as_slice())?;
        write.commit()?;
        Ok(())
    }

    pub fn pending_inbound_delete(
        &self,
        address: MailboxAddress,
        read_capability: &MailboxReadCapability,
    ) -> Result<Option<PendingReplicaDelete>> {
        Ok(self
            .pending_inbound_deletes(address, read_capability, 1)?
            .into_iter()
            .next())
    }

    pub fn pending_inbound_delete_for(
        &self,
        address: MailboxAddress,
        read_capability: &MailboxReadCapability,
        item_id: MailboxItemId,
        expected_store_key: MailboxStoreKey,
    ) -> Result<Option<PendingReplicaDelete>> {
        ensure!(
            address.read_key() == read_capability.read_key(),
            "mailbox read capability does not match replica delete address"
        );
        let key = receipt_key(address.mailbox_id(), item_id, expected_store_key);
        let read = self.database.begin_read()?;
        let table = read.open_table(INBOUND_COMMIT_TABLE)?;
        let Some(value) = table.get(key.as_slice())? else {
            return Ok(None);
        };
        let record = ReplicaInboundCommitRecord::decode(value.value())?;
        ensure!(
            record.mailbox_id == address.mailbox_id()
                && record.item_id == item_id
                && record.expected_store_key == expected_store_key,
            "inbound replica commit key does not match its record"
        );
        let authorization = read_capability.authorize_delete(
            address,
            item_id,
            record.stored_receipt.receipt_id()?,
        )?;
        Ok(Some(PendingReplicaDelete {
            request: MailboxDeleteRequest::new(address, authorization)?,
            expected_store_key,
            transport_identity: record.transport_identity,
        }))
    }

    pub fn pending_inbound_deletes(
        &self,
        address: MailboxAddress,
        read_capability: &MailboxReadCapability,
        limit: u8,
    ) -> Result<Vec<PendingReplicaDelete>> {
        ensure!(
            address.read_key() == read_capability.read_key(),
            "mailbox read capability does not match replica delete address"
        );
        ensure!(
            (1..=MAX_REPLICA_DELETE_BATCH).contains(&limit),
            "mailbox replica delete batch size is invalid"
        );
        let read = self.database.begin_read()?;
        let table = read.open_table(INBOUND_COMMIT_TABLE)?;
        let mut pending = Vec::new();
        for entry in table.iter()? {
            let (key, value) = entry?;
            ensure!(
                key.value().len() == RECEIPT_KEY_BYTES,
                "mailbox replication ledger contains an invalid inbound commit key"
            );
            let record = ReplicaInboundCommitRecord::decode(value.value())?;
            ensure!(
                key.value()
                    == receipt_key(record.mailbox_id, record.item_id, record.expected_store_key),
                "inbound replica commit key does not match its record"
            );
            if record.mailbox_id != address.mailbox_id() {
                continue;
            }
            let authorization = read_capability.authorize_delete(
                address,
                record.item_id,
                record.stored_receipt.receipt_id()?,
            )?;
            pending.push(PendingReplicaDelete {
                request: MailboxDeleteRequest::new(address, authorization)?,
                expected_store_key: record.expected_store_key,
                transport_identity: record.transport_identity,
            });
            if pending.len() == usize::from(limit) {
                break;
            }
        }
        Ok(pending)
    }

    pub fn mark_inbound_deleted(
        &self,
        pending: &PendingReplicaDelete,
        response: &MailboxDeleteResponse,
        recorded_at_unix_seconds: u64,
    ) -> Result<()> {
        let delete_receipt = response
            .delete_receipt()
            .context("volunteer mailbox delete response has no signed deletion proof")?;
        MailboxDeleteResponse::decode_and_verify(
            &response.encode(pending.request(), pending.expected_store_key())?,
            pending.request(),
            pending.expected_store_key(),
        )?;
        let record = ReplicaInboundDeletedRecord {
            version: RECORD_VERSION,
            mailbox_id: pending.request().mailbox_id(),
            item_id: pending.request().item_id(),
            transport_identity: *pending.transport_identity(),
            expected_store_key: pending.expected_store_key(),
            stored_receipt_id: pending.request().stored_receipt_id(),
            delete_receipt: delete_receipt.clone(),
            recorded_at_unix_seconds,
        };
        let encoded = record.encode()?;
        let key = receipt_key(record.mailbox_id, record.item_id, record.expected_store_key);
        let mut write = self.database.begin_write()?;
        write.set_durability(Durability::Immediate)?;
        if let Some(current) = read_value(&write, INBOUND_DELETED_TABLE, key.as_slice())? {
            let current = ReplicaInboundDeletedRecord::decode(&current)?;
            ensure!(
                current.same_logical_record(&record),
                "deleted inbound replica replay conflicts"
            );
            write.commit()?;
            return Ok(());
        }
        let committed = read_value(&write, INBOUND_COMMIT_TABLE, key.as_slice())?
            .context("application commit is absent before replica deletion")?;
        let committed = ReplicaInboundCommitRecord::decode(&committed)?;
        ensure!(
            committed.transport_identity == record.transport_identity
                && committed.stored_receipt.receipt_id()? == record.stored_receipt_id,
            "replica deletion does not match its durable application commit"
        );
        ensure!(
            write.open_table(INBOUND_DELETED_TABLE)?.len()? < self.config.max_receipts,
            "deleted inbound replica capacity exceeded"
        );
        write
            .open_table(INBOUND_DELETED_TABLE)?
            .insert(key.as_slice(), encoded.as_slice())?;
        write
            .open_table(INBOUND_COMMIT_TABLE)?
            .remove(key.as_slice())?;
        write.commit()?;
        Ok(())
    }

    pub fn inbound_counts(&self) -> Result<(u64, u64)> {
        let read = self.database.begin_read()?;
        Ok((
            read.open_table(INBOUND_COMMIT_TABLE)?.len()?,
            read.open_table(INBOUND_DELETED_TABLE)?.len()?,
        ))
    }

    pub fn cleanup(&self, now_unix_seconds: u64) -> Result<MailboxReplicationCleanupReport> {
        let mut write = self.database.begin_write()?;
        write.set_durability(Durability::Immediate)?;
        let expired_items = {
            let table = write.open_table(PLAN_TABLE)?;
            let mut expired = Vec::new();
            for entry in table.iter()? {
                let (key, value) = entry?;
                ensure!(
                    key.value().len() == ITEM_KEY_BYTES,
                    "mailbox replication ledger contains an invalid plan key"
                );
                if ReplicationPlanRecord::decode(value.value())?.expires_at_unix_seconds
                    <= now_unix_seconds
                {
                    expired.push(key.value().to_vec());
                }
            }
            expired
        };
        for key in &expired_items {
            write.open_table(PLAN_TABLE)?.remove(key.as_slice())?;
        }
        let mut removed_attempts = 0_u64;
        for key in &expired_items {
            if write
                .open_table(ATTEMPT_TABLE)?
                .remove(key.as_slice())?
                .is_some()
            {
                removed_attempts += 1;
            }
        }
        let expired_receipts = {
            let table = write.open_table(RECEIPT_TABLE)?;
            let mut expired = Vec::new();
            for entry in table.iter()? {
                let (key, value) = entry?;
                ensure!(
                    key.value().len() == RECEIPT_KEY_BYTES,
                    "mailbox replication ledger contains an invalid receipt key"
                );
                if expired_items
                    .iter()
                    .any(|item| key.value()[..ITEM_KEY_BYTES] == item[..])
                    || ReplicaReceiptRecord::decode(value.value())?
                        .receipt
                        .expires_at_unix_seconds()
                        <= now_unix_seconds
                {
                    expired.push(key.value().to_vec());
                }
            }
            expired
        };
        for key in &expired_receipts {
            write.open_table(RECEIPT_TABLE)?.remove(key.as_slice())?;
        }
        let expired_inbound_commits = {
            let table = write.open_table(INBOUND_COMMIT_TABLE)?;
            let mut expired = Vec::new();
            for entry in table.iter()? {
                let (key, value) = entry?;
                ensure!(
                    key.value().len() == RECEIPT_KEY_BYTES,
                    "mailbox replication ledger contains an invalid inbound commit key"
                );
                if ReplicaInboundCommitRecord::decode(value.value())?
                    .stored_receipt
                    .expires_at_unix_seconds()
                    <= now_unix_seconds
                {
                    expired.push(key.value().to_vec());
                }
            }
            expired
        };
        for key in &expired_inbound_commits {
            write
                .open_table(INBOUND_COMMIT_TABLE)?
                .remove(key.as_slice())?;
        }
        let expired_inbound_deletions = {
            let table = write.open_table(INBOUND_DELETED_TABLE)?;
            let mut expired = Vec::new();
            for entry in table.iter()? {
                let (key, value) = entry?;
                ensure!(
                    key.value().len() == RECEIPT_KEY_BYTES,
                    "mailbox replication ledger contains an invalid inbound deletion key"
                );
                if ReplicaInboundDeletedRecord::decode(value.value())?
                    .delete_receipt
                    .expires_at_unix_seconds()
                    <= now_unix_seconds
                {
                    expired.push(key.value().to_vec());
                }
            }
            expired
        };
        for key in &expired_inbound_deletions {
            write
                .open_table(INBOUND_DELETED_TABLE)?
                .remove(key.as_slice())?;
        }
        write.commit()?;
        Ok(MailboxReplicationCleanupReport {
            removed_plans: expired_items.len() as u64,
            removed_receipts: expired_receipts.len() as u64,
            removed_attempts,
            removed_inbound_commits: expired_inbound_commits.len() as u64,
            removed_inbound_deletions: expired_inbound_deletions.len() as u64,
        })
    }
}

fn request_digest(request: &MailboxPutRequest) -> Result<[u8; 32]> {
    Ok(*blake3::hash(&request.encode()?).as_bytes())
}

fn read_value(
    write: &redb::WriteTransaction,
    definition: TableDefinition<&[u8], &[u8]>,
    key: &[u8],
) -> Result<Option<Vec<u8>>> {
    Ok(write
        .open_table(definition)?
        .get(key)?
        .map(|value| value.value().to_vec()))
}

fn item_key(mailbox_id: MailboxId, item_id: MailboxItemId) -> [u8; ITEM_KEY_BYTES] {
    let mut key = [0_u8; ITEM_KEY_BYTES];
    key[..32].copy_from_slice(mailbox_id.as_bytes());
    key[32..].copy_from_slice(item_id.as_bytes());
    key
}

fn receipt_key(
    mailbox_id: MailboxId,
    item_id: MailboxItemId,
    store_key: MailboxStoreKey,
) -> [u8; RECEIPT_KEY_BYTES] {
    let mut key = [0_u8; RECEIPT_KEY_BYTES];
    key[..ITEM_KEY_BYTES].copy_from_slice(&item_key(mailbox_id, item_id));
    key[ITEM_KEY_BYTES..].copy_from_slice(store_key.as_bytes());
    key
}

fn encode_receipt_record<T: Serialize>(value: &T, kind: &str) -> Result<Vec<u8>> {
    let bytes = postcard::to_allocvec(value).with_context(|| format!("encode {kind}"))?;
    ensure!(
        bytes.len() <= MAX_RECEIPT_RECORD_BYTES,
        "{kind} is too large"
    );
    Ok(bytes)
}

fn decode_receipt_record<T: for<'de> Deserialize<'de>>(bytes: &[u8], kind: &str) -> Result<T> {
    ensure!(
        bytes.len() <= MAX_RECEIPT_RECORD_BYTES,
        "{kind} is too large"
    );
    postcard::from_bytes(bytes).with_context(|| format!("decode {kind}"))
}

fn decode_attempt_time(bytes: &[u8]) -> Result<u64> {
    ensure!(
        bytes.len() == std::mem::size_of::<u64>(),
        "mailbox replication attempt time is invalid"
    );
    let mut value = [0_u8; 8];
    value.copy_from_slice(bytes);
    Ok(u64::from_le_bytes(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kilogram_crypto::DeviceEncryptionIdentity;
    use kilogram_mailbox::{
        BlindMailboxStore, MailboxAddress, MailboxStoreConfig, MailboxStoreIdentity,
        MailboxWriteCapability,
    };

    #[test]
    fn plan_and_transport_distinct_receipts_survive_restart() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let ledger_dir = directory.path().join("replication");
        let ledger = MailboxReplicationLedger::open(MailboxReplicationLedgerConfig::new(
            ledger_dir.clone(),
        ))?;
        let write = MailboxWriteCapability::from_secret_bytes([1_u8; 32]);
        let read = kilogram_mailbox::MailboxReadCapability::from_secret_bytes([2_u8; 32]);
        let address = MailboxAddress::new(read.read_key(), write.write_key());
        let item_id = MailboxItemId::from_bytes([3_u8; 32]);
        let recipient = DeviceEncryptionIdentity::from_secret_bytes([4_u8; 32]);
        let envelope = MailboxEnvelope::seal(
            address.mailbox_id(),
            item_id,
            1_000,
            1_600,
            recipient.public_key(),
            b"opaque mailbox event",
        )?
        .encode()?;
        let request = MailboxPutRequest::new(
            address,
            write.authorize(address, item_id, 600, &envelope)?,
            envelope,
        )?;
        let (outcome, plan) = ledger.ensure_plan(
            &request,
            [5_u8; 32],
            [6_u8; 32],
            DEFAULT_REPLICATION_TARGETS,
            DEFAULT_REQUIRED_REPLICA_RECEIPTS,
            1_000,
        )?;
        assert_eq!(outcome, ReplicationPlanOutcome::Created);
        assert_eq!(plan.selection_salt(), [6_u8; 32]);
        assert!(ledger.next_due(1_000, 60)?.is_some());
        assert!(ledger.mark_attempt(&plan, 1_000, 60)?);
        assert!(!ledger.mark_attempt(&plan, 1_001, 60)?);
        assert!(ledger.next_due(1_059, 60)?.is_none());
        assert!(ledger.next_due(1_060, 60)?.is_some());

        for (secret, transport) in [(7_u8, 8_u8), (9_u8, 10_u8)] {
            let identity = MailboxStoreIdentity::from_secret_bytes([secret; 32]);
            let store_key = identity.store_key();
            let store = BlindMailboxStore::open(
                MailboxStoreConfig::new(directory.path().join(format!("store-{secret}"))),
                identity,
            )?;
            let (put_address, authorization, envelope) = request.clone().into_parts();
            let response = MailboxPutResponse::from_outcome(store.put(
                put_address,
                &authorization,
                envelope,
                1_001,
            )?);
            ledger.record_receipt(
                &request,
                [5_u8; 32],
                [transport; 32],
                store_key,
                &response,
                1_001,
            )?;
        }
        let status = ledger
            .status(address.mailbox_id(), item_id)?
            .context("replication status")?;
        assert!(status.is_satisfied());
        assert_eq!(status.receipts.len(), 2);
        assert!(ledger.next_due(1_001, 60)?.is_none());
        drop(ledger);

        let ledger =
            MailboxReplicationLedger::open(MailboxReplicationLedgerConfig::new(ledger_dir))?;
        let (outcome, replayed) = ledger.ensure_plan(
            &request,
            [5_u8; 32],
            [11_u8; 32],
            DEFAULT_REPLICATION_TARGETS,
            DEFAULT_REQUIRED_REPLICA_RECEIPTS,
            1_010,
        )?;
        assert_eq!(outcome, ReplicationPlanOutcome::AlreadyPresent);
        assert_eq!(replayed.selection_salt(), [6_u8; 32]);
        assert_eq!(replayed.request(), &request);
        assert!(
            ledger
                .status(address.mailbox_id(), item_id)?
                .context("reopened replication status")?
                .is_satisfied()
        );
        assert_eq!(
            ledger.cleanup(1_600)?,
            MailboxReplicationCleanupReport {
                removed_plans: 1,
                removed_receipts: 2,
                removed_attempts: 1,
                removed_inbound_commits: 0,
                removed_inbound_deletions: 0,
            }
        );
        Ok(())
    }

    #[test]
    fn replica_delete_requires_durable_application_commit() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let replication = MailboxReplicationLedger::open(MailboxReplicationLedgerConfig::new(
            directory.path().join("replication"),
        ))?;
        let client = crate::MailboxClientLedger::open(crate::MailboxClientLedgerConfig::new(
            directory.path().join("client"),
        ))?;
        let identity = MailboxStoreIdentity::from_secret_bytes([21_u8; 32]);
        let store_key = identity.store_key();
        let store = BlindMailboxStore::open(
            MailboxStoreConfig::new(directory.path().join("store")),
            identity,
        )?;
        let write = MailboxWriteCapability::from_secret_bytes([22_u8; 32]);
        let read = kilogram_mailbox::MailboxReadCapability::from_secret_bytes([23_u8; 32]);
        let address = MailboxAddress::new(read.read_key(), write.write_key());
        let item_id = MailboxItemId::from_bytes([24_u8; 32]);
        let recipient = DeviceEncryptionIdentity::from_secret_bytes([25_u8; 32]);
        let envelope = MailboxEnvelope::seal(
            address.mailbox_id(),
            item_id,
            2_000,
            2_600,
            recipient.public_key(),
            b"replicated opaque event",
        )?
        .encode()?;
        let authorization = write.authorize(address, item_id, 600, &envelope)?;
        store.put(address, &authorization, envelope, 2_000)?;
        let list_authorization = read.authorize_list(
            address,
            kilogram_mailbox::MailboxRequestNonce::from_bytes([26_u8; 32]),
        )?;
        let item = store
            .list(address, &list_authorization, 2_001)?
            .into_iter()
            .next()
            .context("stored replica")?;
        let prepared = client.prepare_inbound(address, item, store_key, &recipient, 2_001)?;
        assert!(
            replication
                .pending_inbound_delete(address, &read)?
                .is_none()
        );
        replication.record_inbound_commit(&prepared, [27_u8; 32], [28_u8; 32], 2_002)?;
        assert!(
            replication
                .pending_inbound_delete_for(address, &read, item_id, store.store_key())?
                .is_some()
        );
        let pending = replication
            .pending_inbound_delete(address, &read)?
            .context("replica delete after application commit")?;
        let delete_response = MailboxDeleteResponse::from_outcome(store.delete(
            address,
            pending.request().authorization(),
            2_003,
        )?);
        replication.mark_inbound_deleted(&pending, &delete_response, 2_003)?;
        replication.mark_inbound_deleted(&pending, &delete_response, 2_004)?;
        assert_eq!(replication.inbound_counts()?, (0, 1));
        assert!(store.list(address, &list_authorization, 2_004)?.is_empty());
        let cleanup = replication.cleanup(2_600)?;
        assert_eq!(cleanup.removed_inbound_commits, 0);
        assert_eq!(cleanup.removed_inbound_deletions, 1);
        assert_eq!(replication.inbound_counts()?, (0, 0));
        Ok(())
    }
}
