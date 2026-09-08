use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use kilogram_crypto::DeviceEncryptionIdentity;
use kilogram_mailbox::{
    MAX_MAILBOX_WIRE_REQUEST_BYTES, MailboxAddress, MailboxDeleteRequest, MailboxDeleteResponse,
    MailboxEnvelope, MailboxId, MailboxItemId, MailboxPutRequest, MailboxPutResponse,
    MailboxReadCapability, MailboxReceiptId, MailboxStoreKey, MailboxStoredReceipt,
    StoredMailboxItem,
};
use redb::{
    Database, Durability, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition,
};
use serde::{Deserialize, Serialize};

const DATABASE_FILE: &str = "mailbox-client-ledger.redb";
const PENDING_OUTBOUND_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("mailbox-client-pending-outbound-v1");
const STORED_OUTBOUND_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("mailbox-client-stored-outbound-v1");
const INBOUND_COMMIT_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("mailbox-client-inbound-commit-v1");
const DELETED_INBOUND_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("mailbox-client-deleted-inbound-v1");
const RECORD_VERSION: u8 = 1;
const MAILBOX_ITEM_KEY_BYTES: usize = 64;
const MAX_COMPACT_RECORD_BYTES: usize = 4 * 1024;
const MAX_ABSOLUTE_RECORDS: u64 = 1_000_000;

pub const DEFAULT_MAX_PENDING_UPLOADS: u64 = 4_096;
pub const DEFAULT_MAX_RETAINED_RECEIPTS: u64 = 100_000;
pub const DEFAULT_MAX_INBOUND_COMMITS: u64 = 100_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MailboxClientCleanupReport {
    pub removed_pending_uploads: u64,
    pub removed_stored_receipts: u64,
    pub removed_inbound_commits: u64,
    pub removed_deleted_receipts: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MailboxClientLedgerConfig {
    pub data_dir: PathBuf,
    pub max_pending_uploads: u64,
    pub max_retained_receipts: u64,
    pub max_inbound_commits: u64,
}

impl MailboxClientLedgerConfig {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            max_pending_uploads: DEFAULT_MAX_PENDING_UPLOADS,
            max_retained_receipts: DEFAULT_MAX_RETAINED_RECEIPTS,
            max_inbound_commits: DEFAULT_MAX_INBOUND_COMMITS,
        }
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            !self.data_dir.as_os_str().is_empty(),
            "mailbox client ledger directory is empty"
        );
        for (name, value) in [
            ("pending upload", self.max_pending_uploads),
            ("retained receipt", self.max_retained_receipts),
            ("inbound commit", self.max_inbound_commits),
        ] {
            ensure!(
                (1..=MAX_ABSOLUTE_RECORDS).contains(&value),
                "mailbox client {name} limit is invalid"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PendingOutboundRecord {
    version: u8,
    request: MailboxPutRequest,
    expected_store_key: MailboxStoreKey,
    queued_at_unix_seconds: u64,
}

impl PendingOutboundRecord {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.version == RECORD_VERSION,
            "unsupported pending mailbox record"
        );
        MailboxPutRequest::decode_and_verify(&self.request.encode()?)?;
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = postcard::to_allocvec(self).context("encode pending mailbox upload")?;
        ensure!(
            bytes.len() <= MAX_MAILBOX_WIRE_REQUEST_BYTES.saturating_add(512),
            "pending mailbox upload record is too large"
        );
        Ok(bytes)
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_MAILBOX_WIRE_REQUEST_BYTES.saturating_add(512),
            "pending mailbox upload record is too large"
        );
        let value: Self = postcard::from_bytes(bytes).context("decode pending mailbox upload")?;
        value.validate()?;
        Ok(value)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StoredOutboundReceipt {
    version: u8,
    mailbox_id: MailboxId,
    item_id: MailboxItemId,
    request_digest: [u8; 32],
    expected_store_key: MailboxStoreKey,
    receipt: MailboxStoredReceipt,
    recorded_at_unix_seconds: u64,
}

impl StoredOutboundReceipt {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.version == RECORD_VERSION,
            "unsupported stored mailbox receipt"
        );
        self.receipt.verify_signature()?;
        ensure!(
            self.receipt.mailbox_id() == self.mailbox_id
                && self.receipt.item_id() == self.item_id
                && self.receipt.store_key() == self.expected_store_key,
            "stored outbound mailbox receipt is inconsistent"
        );
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        encode_compact(self, "stored outbound mailbox receipt")
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        let value: Self = decode_compact(bytes, "stored outbound mailbox receipt")?;
        value.validate()?;
        Ok(value)
    }

    pub fn mailbox_id(&self) -> MailboxId {
        self.mailbox_id
    }

    pub fn item_id(&self) -> MailboxItemId {
        self.item_id
    }

    pub fn receipt(&self) -> &MailboxStoredReceipt {
        &self.receipt
    }

    pub fn recorded_at_unix_seconds(&self) -> u64 {
        self.recorded_at_unix_seconds
    }

    fn same_logical_record(&self, other: &Self) -> bool {
        self.version == other.version
            && self.mailbox_id == other.mailbox_id
            && self.item_id == other.item_id
            && self.request_digest == other.request_digest
            && self.expected_store_key == other.expected_store_key
            && self.receipt == other.receipt
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct InboundCommitRecord {
    version: u8,
    mailbox_id: MailboxId,
    item_id: MailboxItemId,
    expected_store_key: MailboxStoreKey,
    stored_receipt: MailboxStoredReceipt,
    application_commit_id: [u8; 32],
    committed_at_unix_seconds: u64,
}

impl InboundCommitRecord {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.version == RECORD_VERSION,
            "unsupported inbound mailbox commit"
        );
        self.stored_receipt.verify_signature()?;
        ensure!(
            self.stored_receipt.mailbox_id() == self.mailbox_id
                && self.stored_receipt.item_id() == self.item_id
                && self.stored_receipt.store_key() == self.expected_store_key,
            "inbound mailbox commit receipt is inconsistent"
        );
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        encode_compact(self, "inbound mailbox commit")
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        let value: Self = decode_compact(bytes, "inbound mailbox commit")?;
        value.validate()?;
        Ok(value)
    }

    fn same_logical_record(&self, other: &Self) -> bool {
        self.version == other.version
            && self.mailbox_id == other.mailbox_id
            && self.item_id == other.item_id
            && self.expected_store_key == other.expected_store_key
            && self.stored_receipt == other.stored_receipt
            && self.application_commit_id == other.application_commit_id
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct DeletedInboundRecord {
    version: u8,
    mailbox_id: MailboxId,
    item_id: MailboxItemId,
    expected_store_key: MailboxStoreKey,
    stored_receipt_id: MailboxReceiptId,
    delete_receipt: kilogram_mailbox::MailboxDeleteReceipt,
    recorded_at_unix_seconds: u64,
}

impl DeletedInboundRecord {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.version == RECORD_VERSION,
            "unsupported deleted mailbox record"
        );
        self.delete_receipt.verify()?;
        ensure!(
            self.delete_receipt.mailbox_id() == self.mailbox_id
                && self.delete_receipt.item_id() == self.item_id
                && self.delete_receipt.store_key() == self.expected_store_key
                && self.delete_receipt.stored_receipt_id() == self.stored_receipt_id,
            "deleted inbound mailbox receipt is inconsistent"
        );
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        encode_compact(self, "deleted inbound mailbox receipt")
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        let value: Self = decode_compact(bytes, "deleted inbound mailbox receipt")?;
        value.validate()?;
        Ok(value)
    }

    fn same_logical_record(&self, other: &Self) -> bool {
        self.version == other.version
            && self.mailbox_id == other.mailbox_id
            && self.item_id == other.item_id
            && self.expected_store_key == other.expected_store_key
            && self.stored_receipt_id == other.stored_receipt_id
            && self.delete_receipt == other.delete_receipt
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingMailboxUpload {
    pub request: MailboxPutRequest,
    pub expected_store_key: MailboxStoreKey,
    pub queued_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutboundEnqueueOutcome {
    Created,
    AlreadyPending,
    AlreadyStored,
    CapacityExceeded,
    Conflict,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MailboxOutboundState {
    Pending(PendingMailboxUpload),
    Stored(StoredOutboundReceipt),
}

pub struct PreparedInboundItem {
    address: MailboxAddress,
    item: StoredMailboxItem,
    expected_store_key: MailboxStoreKey,
    plaintext: Vec<u8>,
}

impl PreparedInboundItem {
    pub fn address(&self) -> MailboxAddress {
        self.address
    }

    pub fn mailbox_id(&self) -> MailboxId {
        self.address.mailbox_id()
    }

    pub fn item_id(&self) -> MailboxItemId {
        self.item.item_id
    }

    pub fn plaintext(&self) -> &[u8] {
        &self.plaintext
    }

    pub fn expected_store_key(&self) -> MailboxStoreKey {
        self.expected_store_key
    }

    pub fn stored_receipt(&self) -> &MailboxStoredReceipt {
        &self.item.receipt
    }

    pub fn stored_receipt_id(&self) -> Result<MailboxReceiptId> {
        self.item.receipt.receipt_id().map_err(Into::into)
    }
}

pub struct MailboxClientLedger {
    database: Database,
    config: MailboxClientLedgerConfig,
}

impl MailboxClientLedger {
    pub fn open(config: MailboxClientLedgerConfig) -> Result<Self> {
        config.validate()?;
        std::fs::create_dir_all(&config.data_dir).with_context(|| {
            format!(
                "create mailbox client ledger directory {}",
                config.data_dir.display()
            )
        })?;
        let metadata = std::fs::symlink_metadata(&config.data_dir)
            .context("inspect mailbox client ledger directory")?;
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "mailbox client ledger directory must be a real directory, not a symlink"
        );
        let canonical = std::fs::canonicalize(&config.data_dir)
            .context("canonicalize mailbox client ledger directory")?;
        let database = Database::create(canonical.join(DATABASE_FILE))
            .context("open mailbox client ledger")?;
        let write = database
            .begin_write()
            .context("begin mailbox client initialization")?;
        write.open_table(PENDING_OUTBOUND_TABLE)?;
        write.open_table(STORED_OUTBOUND_TABLE)?;
        write.open_table(INBOUND_COMMIT_TABLE)?;
        write.open_table(DELETED_INBOUND_TABLE)?;
        write
            .commit()
            .context("commit mailbox client initialization")?;
        Ok(Self { database, config })
    }

    pub fn enqueue_outbound(
        &self,
        request: MailboxPutRequest,
        expected_store_key: MailboxStoreKey,
        queued_at_unix_seconds: u64,
    ) -> Result<OutboundEnqueueOutcome> {
        let record = PendingOutboundRecord {
            version: RECORD_VERSION,
            request,
            expected_store_key,
            queued_at_unix_seconds,
        };
        let encoded = record.encode()?;
        let key = item_key(record.request.mailbox_id(), record.request.item_id());
        let mut write = self
            .database
            .begin_write()
            .context("begin mailbox enqueue")?;
        write
            .set_durability(Durability::Immediate)
            .context("set mailbox enqueue durability")?;
        if let Some(current) = read_value(&write, STORED_OUTBOUND_TABLE, &key)? {
            let current = StoredOutboundReceipt::decode(&current)?;
            let outcome = if current.request_digest == request_digest(&record.request)?
                && current.expected_store_key == expected_store_key
            {
                OutboundEnqueueOutcome::AlreadyStored
            } else {
                OutboundEnqueueOutcome::Conflict
            };
            write
                .commit()
                .context("commit stored mailbox enqueue replay")?;
            return Ok(outcome);
        }
        if let Some(current) = read_value(&write, PENDING_OUTBOUND_TABLE, &key)? {
            let outcome = if current == encoded {
                OutboundEnqueueOutcome::AlreadyPending
            } else {
                OutboundEnqueueOutcome::Conflict
            };
            write
                .commit()
                .context("commit pending mailbox enqueue replay")?;
            return Ok(outcome);
        }
        let pending_count = write
            .open_table(PENDING_OUTBOUND_TABLE)?
            .len()
            .context("count pending mailbox uploads")?;
        if pending_count >= self.config.max_pending_uploads {
            write
                .commit()
                .context("commit mailbox enqueue capacity result")?;
            return Ok(OutboundEnqueueOutcome::CapacityExceeded);
        }
        write
            .open_table(PENDING_OUTBOUND_TABLE)?
            .insert(key.as_slice(), encoded.as_slice())?;
        write.commit().context("commit mailbox enqueue")?;
        Ok(OutboundEnqueueOutcome::Created)
    }

    pub fn next_pending_outbound(&self) -> Result<Option<PendingMailboxUpload>> {
        let read = self
            .database
            .begin_read()
            .context("begin pending mailbox read")?;
        let table = read.open_table(PENDING_OUTBOUND_TABLE)?;
        let Some(entry) = table.iter()?.next() else {
            return Ok(None);
        };
        let (_, value) = entry.context("read pending mailbox upload")?;
        let record = PendingOutboundRecord::decode(value.value())?;
        Ok(Some(PendingMailboxUpload {
            request: record.request,
            expected_store_key: record.expected_store_key,
            queued_at_unix_seconds: record.queued_at_unix_seconds,
        }))
    }

    pub fn outbound_state(
        &self,
        mailbox_id: MailboxId,
        item_id: MailboxItemId,
    ) -> Result<Option<MailboxOutboundState>> {
        let read = self
            .database
            .begin_read()
            .context("begin mailbox outbound-state read")?;
        let key = item_key(mailbox_id, item_id);
        if let Some(value) = read
            .open_table(STORED_OUTBOUND_TABLE)?
            .get(key.as_slice())?
        {
            return Ok(Some(MailboxOutboundState::Stored(
                StoredOutboundReceipt::decode(value.value())?,
            )));
        }
        if let Some(value) = read
            .open_table(PENDING_OUTBOUND_TABLE)?
            .get(key.as_slice())?
        {
            let record = PendingOutboundRecord::decode(value.value())?;
            return Ok(Some(MailboxOutboundState::Pending(PendingMailboxUpload {
                request: record.request,
                expected_store_key: record.expected_store_key,
                queued_at_unix_seconds: record.queued_at_unix_seconds,
            })));
        }
        Ok(None)
    }

    pub fn mark_outbound_stored(
        &self,
        request: &MailboxPutRequest,
        response: &MailboxPutResponse,
        expected_store_key: MailboxStoreKey,
        recorded_at_unix_seconds: u64,
    ) -> Result<StoredOutboundReceipt> {
        let receipt = response
            .stored_receipt()
            .context("mailbox put response does not prove durable acceptance")?;
        MailboxPutResponse::decode_and_verify(
            &response.encode(request, expected_store_key)?,
            request,
            expected_store_key,
        )?;
        let record = StoredOutboundReceipt {
            version: RECORD_VERSION,
            mailbox_id: request.mailbox_id(),
            item_id: request.item_id(),
            request_digest: request_digest(request)?,
            expected_store_key,
            receipt: receipt.clone(),
            recorded_at_unix_seconds,
        };
        let encoded = record.encode()?;
        let key = item_key(record.mailbox_id, record.item_id);
        let mut write = self
            .database
            .begin_write()
            .context("begin mailbox stored commit")?;
        write
            .set_durability(Durability::Immediate)
            .context("set mailbox stored durability")?;
        let pending = read_value(&write, PENDING_OUTBOUND_TABLE, &key)?;
        if let Some(pending) = pending {
            let pending = PendingOutboundRecord::decode(&pending)?;
            ensure!(
                pending.request == *request && pending.expected_store_key == expected_store_key,
                "pending mailbox upload changed before receipt commit"
            );
        } else if let Some(current) = read_value(&write, STORED_OUTBOUND_TABLE, &key)? {
            let current = StoredOutboundReceipt::decode(&current)?;
            ensure!(
                current.same_logical_record(&record),
                "stored mailbox receipt replay conflicts"
            );
            write.commit().context("commit mailbox receipt replay")?;
            return Ok(current);
        } else {
            anyhow::bail!("pending mailbox upload disappeared before receipt commit");
        }
        let receipt_count = write
            .open_table(STORED_OUTBOUND_TABLE)?
            .len()
            .context("count stored mailbox receipts")?;
        ensure!(
            receipt_count < self.config.max_retained_receipts,
            "stored mailbox receipt capacity exceeded"
        );
        write
            .open_table(STORED_OUTBOUND_TABLE)?
            .insert(key.as_slice(), encoded.as_slice())?;
        write
            .open_table(PENDING_OUTBOUND_TABLE)?
            .remove(key.as_slice())?;
        write.commit().context("commit mailbox stored receipt")?;
        Ok(record)
    }

    pub fn prepare_inbound(
        &self,
        address: MailboxAddress,
        item: StoredMailboxItem,
        expected_store_key: MailboxStoreKey,
        recipient: &DeviceEncryptionIdentity,
        now_unix_seconds: u64,
    ) -> Result<PreparedInboundItem> {
        item.receipt.verify(&item.envelope)?;
        ensure!(
            item.receipt.store_key() == expected_store_key
                && item.receipt.mailbox_id() == address.mailbox_id()
                && item.receipt.item_id() == item.item_id
                && item.receipt.expires_at_unix_seconds() == item.expires_at_unix_seconds,
            "inbound mailbox item does not match address or store"
        );
        let plaintext = MailboxEnvelope::decode(&item.envelope)?.open(
            address.mailbox_id(),
            item.item_id,
            recipient,
            now_unix_seconds,
        )?;
        Ok(PreparedInboundItem {
            address,
            item,
            expected_store_key,
            plaintext,
        })
    }

    /// Record this only after the caller's application event/history transaction
    /// has durably committed and returned its stable commit identifier.
    pub fn record_inbound_commit(
        &self,
        prepared: &PreparedInboundItem,
        application_commit_id: [u8; 32],
        committed_at_unix_seconds: u64,
    ) -> Result<()> {
        let record = InboundCommitRecord {
            version: RECORD_VERSION,
            mailbox_id: prepared.mailbox_id(),
            item_id: prepared.item_id(),
            expected_store_key: prepared.expected_store_key,
            stored_receipt: prepared.item.receipt.clone(),
            application_commit_id,
            committed_at_unix_seconds,
        };
        let encoded = record.encode()?;
        let key = item_key(record.mailbox_id, record.item_id);
        let mut write = self
            .database
            .begin_write()
            .context("begin inbound mailbox commit")?;
        write
            .set_durability(Durability::Immediate)
            .context("set inbound mailbox commit durability")?;
        if let Some(deleted) = read_value(&write, DELETED_INBOUND_TABLE, &key)? {
            let deleted = DeletedInboundRecord::decode(&deleted)?;
            ensure!(
                deleted.stored_receipt_id == prepared.stored_receipt_id()?
                    && deleted.expected_store_key == prepared.expected_store_key,
                "already-deleted mailbox item conflicts with prepared input"
            );
            write.commit().context("commit deleted inbound replay")?;
            return Ok(());
        }
        if let Some(current) = read_value(&write, INBOUND_COMMIT_TABLE, &key)? {
            let current = InboundCommitRecord::decode(&current)?;
            ensure!(
                current.same_logical_record(&record),
                "inbound mailbox commit replay conflicts"
            );
            write.commit().context("commit inbound mailbox replay")?;
            return Ok(());
        }
        let count = write.open_table(INBOUND_COMMIT_TABLE)?.len()?;
        ensure!(
            count < self.config.max_inbound_commits,
            "inbound mailbox commit capacity exceeded"
        );
        write
            .open_table(INBOUND_COMMIT_TABLE)?
            .insert(key.as_slice(), encoded.as_slice())?;
        write.commit().context("commit inbound mailbox receipt")?;
        Ok(())
    }

    pub fn pending_delete(
        &self,
        address: MailboxAddress,
        read_capability: &MailboxReadCapability,
    ) -> Result<Option<MailboxDeleteRequest>> {
        ensure!(
            address.read_key() == read_capability.read_key(),
            "mailbox read capability does not match pending-delete address"
        );
        let read = self
            .database
            .begin_read()
            .context("begin pending delete read")?;
        let table = read.open_table(INBOUND_COMMIT_TABLE)?;
        let prefix = address.mailbox_id();
        for entry in table.iter()? {
            let (key, value) = entry?;
            if key.value().len() != MAILBOX_ITEM_KEY_BYTES
                || &key.value()[..32] != prefix.as_bytes()
            {
                continue;
            }
            let record = InboundCommitRecord::decode(value.value())?;
            let receipt_id = record.stored_receipt.receipt_id()?;
            let authorization =
                read_capability.authorize_delete(address, record.item_id, receipt_id)?;
            return Ok(Some(MailboxDeleteRequest::new(address, authorization)?));
        }
        Ok(None)
    }

    pub fn mark_inbound_deleted(
        &self,
        request: &MailboxDeleteRequest,
        response: &MailboxDeleteResponse,
        expected_store_key: MailboxStoreKey,
        recorded_at_unix_seconds: u64,
    ) -> Result<()> {
        let delete_receipt = response
            .delete_receipt()
            .context("mailbox delete response has no signed deletion proof")?;
        MailboxDeleteResponse::decode_and_verify(
            &response.encode(request, expected_store_key)?,
            request,
            expected_store_key,
        )?;
        let record = DeletedInboundRecord {
            version: RECORD_VERSION,
            mailbox_id: request.mailbox_id(),
            item_id: request.item_id(),
            expected_store_key,
            stored_receipt_id: request.stored_receipt_id(),
            delete_receipt: delete_receipt.clone(),
            recorded_at_unix_seconds,
        };
        let encoded = record.encode()?;
        let key = item_key(record.mailbox_id, record.item_id);
        let mut write = self
            .database
            .begin_write()
            .context("begin inbound delete commit")?;
        write
            .set_durability(Durability::Immediate)
            .context("set inbound delete durability")?;
        if let Some(current) = read_value(&write, DELETED_INBOUND_TABLE, &key)? {
            let current = DeletedInboundRecord::decode(&current)?;
            ensure!(
                current.same_logical_record(&record),
                "deleted mailbox receipt replay conflicts"
            );
            write.commit().context("commit mailbox deletion replay")?;
            return Ok(());
        }
        let committed = read_value(&write, INBOUND_COMMIT_TABLE, &key)?
            .context("application commit is absent before mailbox deletion")?;
        let committed = InboundCommitRecord::decode(&committed)?;
        ensure!(
            committed.stored_receipt.receipt_id()? == request.stored_receipt_id()
                && committed.expected_store_key == expected_store_key,
            "mailbox delete does not match durable application commit"
        );
        let count = write.open_table(DELETED_INBOUND_TABLE)?.len()?;
        ensure!(
            count < self.config.max_retained_receipts,
            "deleted mailbox receipt capacity exceeded"
        );
        write
            .open_table(DELETED_INBOUND_TABLE)?
            .insert(key.as_slice(), encoded.as_slice())?;
        write
            .open_table(INBOUND_COMMIT_TABLE)?
            .remove(key.as_slice())?;
        write.commit().context("commit inbound mailbox deletion")?;
        Ok(())
    }

    pub fn cleanup(&self, now_unix_seconds: u64) -> Result<MailboxClientCleanupReport> {
        let mut write = self
            .database
            .begin_write()
            .context("begin mailbox client cleanup")?;
        write
            .set_durability(Durability::Immediate)
            .context("set mailbox client cleanup durability")?;
        let removed_pending_uploads =
            remove_expired_records(&write, PENDING_OUTBOUND_TABLE, |bytes| {
                let record = PendingOutboundRecord::decode(bytes)?;
                let envelope = MailboxEnvelope::decode(record.request.envelope())?;
                Ok(envelope.expires_at_unix_seconds() > now_unix_seconds)
            })?;
        let removed_stored_receipts =
            remove_expired_records(&write, STORED_OUTBOUND_TABLE, |bytes| {
                let record = StoredOutboundReceipt::decode(bytes)?;
                Ok(record.receipt.expires_at_unix_seconds() > now_unix_seconds)
            })?;
        let removed_inbound_commits =
            remove_expired_records(&write, INBOUND_COMMIT_TABLE, |bytes| {
                let record = InboundCommitRecord::decode(bytes)?;
                Ok(record.stored_receipt.expires_at_unix_seconds() > now_unix_seconds)
            })?;
        let removed_deleted_receipts =
            remove_expired_records(&write, DELETED_INBOUND_TABLE, |bytes| {
                let record = DeletedInboundRecord::decode(bytes)?;
                Ok(record.delete_receipt.expires_at_unix_seconds() > now_unix_seconds)
            })?;
        write.commit().context("commit mailbox client cleanup")?;
        Ok(MailboxClientCleanupReport {
            removed_pending_uploads,
            removed_stored_receipts,
            removed_inbound_commits,
            removed_deleted_receipts,
        })
    }

    pub fn counts(&self) -> Result<(u64, u64, u64, u64)> {
        let read = self
            .database
            .begin_read()
            .context("begin mailbox ledger counts")?;
        Ok((
            read.open_table(PENDING_OUTBOUND_TABLE)?.len()?,
            read.open_table(STORED_OUTBOUND_TABLE)?.len()?,
            read.open_table(INBOUND_COMMIT_TABLE)?.len()?,
            read.open_table(DELETED_INBOUND_TABLE)?.len()?,
        ))
    }
}

fn request_digest(request: &MailboxPutRequest) -> Result<[u8; 32]> {
    Ok(*blake3::hash(&request.encode()?).as_bytes())
}

fn item_key(mailbox_id: MailboxId, item_id: MailboxItemId) -> [u8; MAILBOX_ITEM_KEY_BYTES] {
    let mut key = [0_u8; MAILBOX_ITEM_KEY_BYTES];
    key[..32].copy_from_slice(mailbox_id.as_bytes());
    key[32..].copy_from_slice(item_id.as_bytes());
    key
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

fn remove_expired_records<F>(
    write: &redb::WriteTransaction,
    definition: TableDefinition<&[u8], &[u8]>,
    mut is_live: F,
) -> Result<u64>
where
    F: FnMut(&[u8]) -> Result<bool>,
{
    let expired = {
        let table = write.open_table(definition)?;
        let mut expired = Vec::new();
        for entry in table.iter()? {
            let (key, value) = entry?;
            ensure!(
                key.value().len() == MAILBOX_ITEM_KEY_BYTES,
                "mailbox client ledger contains an invalid item key"
            );
            if !is_live(value.value())? {
                expired.push(key.value().to_vec());
            }
        }
        expired
    };
    let mut table = write.open_table(definition)?;
    for key in &expired {
        table.remove(key.as_slice())?;
    }
    Ok(expired.len() as u64)
}

fn encode_compact<T: Serialize>(value: &T, kind: &str) -> Result<Vec<u8>> {
    let bytes = postcard::to_allocvec(value).with_context(|| format!("encode {kind}"))?;
    ensure!(
        bytes.len() <= MAX_COMPACT_RECORD_BYTES,
        "{kind} is too large"
    );
    Ok(bytes)
}

fn decode_compact<T: for<'de> Deserialize<'de>>(bytes: &[u8], kind: &str) -> Result<T> {
    ensure!(
        bytes.len() <= MAX_COMPACT_RECORD_BYTES,
        "{kind} is too large"
    );
    postcard::from_bytes(bytes).with_context(|| format!("decode {kind}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kilogram_mailbox::{
        BlindMailboxStore, MailboxPutResponse, MailboxStoreConfig, MailboxStoreIdentity,
        MailboxWriteCapability,
    };

    #[test]
    fn durable_ledger_requires_application_commit_before_delete() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let ledger = MailboxClientLedger::open(MailboxClientLedgerConfig::new(
            directory.path().join("client"),
        ))?;
        let store_identity = MailboxStoreIdentity::from_secret_bytes([9_u8; 32]);
        let store_key = store_identity.store_key();
        let store = BlindMailboxStore::open(
            MailboxStoreConfig::new(directory.path().join("store")),
            store_identity,
        )?;
        let read_capability = MailboxReadCapability::from_secret_bytes([1_u8; 32]);
        let write_capability = MailboxWriteCapability::from_secret_bytes([2_u8; 32]);
        let address = MailboxAddress::new(read_capability.read_key(), write_capability.write_key());
        let recipient = DeviceEncryptionIdentity::from_secret_bytes([3_u8; 32]);
        let item_id = MailboxItemId::from_bytes([4_u8; 32]);
        let envelope = MailboxEnvelope::seal(
            address.mailbox_id(),
            item_id,
            1_000,
            1_600,
            recipient.public_key(),
            b"opaque-authorized-event",
        )?
        .encode()?;
        let request = MailboxPutRequest::new(
            address,
            write_capability.authorize(address, item_id, 600, &envelope)?,
            envelope,
        )?;
        assert_eq!(
            ledger.enqueue_outbound(request.clone(), store_key, 1_000)?,
            OutboundEnqueueOutcome::Created
        );
        assert!(matches!(
            ledger.outbound_state(address.mailbox_id(), item_id)?,
            Some(MailboxOutboundState::Pending(_))
        ));
        let pending = ledger
            .next_pending_outbound()?
            .context("pending outbound mailbox upload")?;
        let (put_address, authorization, envelope) = pending.request.clone().into_parts();
        let put_response = MailboxPutResponse::from_outcome(store.put(
            put_address,
            &authorization,
            envelope,
            1_000,
        )?);
        ledger.mark_outbound_stored(&pending.request, &put_response, store_key, 1_001)?;
        assert!(matches!(
            ledger.outbound_state(address.mailbox_id(), item_id)?,
            Some(MailboxOutboundState::Stored(_))
        ));
        let replayed =
            ledger.mark_outbound_stored(&pending.request, &put_response, store_key, 1_002)?;
        assert_eq!(replayed.recorded_at_unix_seconds(), 1_001);
        assert_eq!(ledger.counts()?, (0, 1, 0, 0));

        let list_authorization = read_capability.authorize_list(
            address,
            kilogram_mailbox::MailboxRequestNonce::from_bytes([5_u8; 32]),
        )?;
        let item = store
            .list(address, &list_authorization, 1_002)?
            .into_iter()
            .next()
            .context("stored mailbox item")?;
        let prepared = ledger.prepare_inbound(address, item, store_key, &recipient, 1_002)?;
        assert_eq!(prepared.plaintext(), b"opaque-authorized-event");
        assert!(ledger.pending_delete(address, &read_capability)?.is_none());

        ledger.record_inbound_commit(&prepared, [7_u8; 32], 1_003)?;
        ledger.record_inbound_commit(&prepared, [7_u8; 32], 1_004)?;
        drop(ledger);
        let ledger = MailboxClientLedger::open(MailboxClientLedgerConfig::new(
            directory.path().join("client"),
        ))?;
        let delete_request = ledger
            .pending_delete(address, &read_capability)?
            .context("delete must become available after durable commit")?;
        let delete_response = MailboxDeleteResponse::from_outcome(store.delete(
            address,
            delete_request.authorization(),
            1_004,
        )?);
        ledger.mark_inbound_deleted(&delete_request, &delete_response, store_key, 1_004)?;
        ledger.mark_inbound_deleted(&delete_request, &delete_response, store_key, 1_005)?;
        assert_eq!(ledger.counts()?, (0, 1, 0, 1));
        assert!(store.list(address, &list_authorization, 1_005)?.is_empty());
        assert_eq!(
            ledger.cleanup(1_600)?,
            MailboxClientCleanupReport {
                removed_pending_uploads: 0,
                removed_stored_receipts: 1,
                removed_inbound_commits: 0,
                removed_deleted_receipts: 1,
            }
        );
        assert_eq!(ledger.counts()?, (0, 0, 0, 0));
        assert!(
            ledger
                .outbound_state(address.mailbox_id(), item_id)?
                .is_none()
        );
        Ok(())
    }
}
