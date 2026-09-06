use std::{collections::BTreeMap, path::PathBuf};

use anyhow::{Context, Result, ensure};
use redb::{
    Database, Durability, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition,
};
use serde::{Deserialize, Serialize};

use crate::{
    MAX_MAILBOX_ENVELOPE_BYTES, MAX_MAILBOX_TTL_SECONDS, MIN_MAILBOX_TTL_SECONDS, MailboxAddress,
    MailboxDeleteReceipt, MailboxId, MailboxItemId, MailboxReadAuthorization, MailboxReadOperation,
    MailboxReceiptId, MailboxStoreIdentity, MailboxStoredReceipt, MailboxWriteAuthorization,
};

const DATABASE_FILE: &str = "blind-mailbox.redb";
const ITEM_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("blind-mailbox-items-v1");
const TOMBSTONE_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("blind-mailbox-tombstones-v1");
const MAILBOX_COUNT_TABLE: TableDefinition<&[u8], u64> =
    TableDefinition::new("blind-mailbox-counts-v1");
const META_TABLE: TableDefinition<&str, u64> = TableDefinition::new("blind-mailbox-meta-v1");
const IDENTITY_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("blind-mailbox-identity-v1");
const STORE_KEY: &str = "store-key";
const TOTAL_BYTES_KEY: &str = "total-envelope-bytes";
const TOTAL_ITEMS_KEY: &str = "total-live-items";
const RECORD_VERSION: u8 = 1;
const ITEM_KEY_BYTES: usize = 64;
const MAX_ABSOLUTE_ITEMS: u64 = 10_000_000;
const MAX_ABSOLUTE_TOTAL_BYTES: u64 = 1024 * 1024 * 1024 * 1024;
const MAX_ABSOLUTE_ITEMS_PER_MAILBOX: u64 = 4_096;

pub const DEFAULT_MAX_ITEMS_PER_MAILBOX: u64 = 128;
pub const DEFAULT_MAX_TOTAL_ITEMS: u64 = 100_000;
pub const DEFAULT_MAX_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MailboxStoreConfig {
    pub data_dir: PathBuf,
    pub max_envelope_bytes: usize,
    pub max_items_per_mailbox: u64,
    pub max_total_items: u64,
    pub max_total_bytes: u64,
}

impl MailboxStoreConfig {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            max_envelope_bytes: MAX_MAILBOX_ENVELOPE_BYTES,
            max_items_per_mailbox: DEFAULT_MAX_ITEMS_PER_MAILBOX,
            max_total_items: DEFAULT_MAX_TOTAL_ITEMS,
            max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
        }
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            !self.data_dir.as_os_str().is_empty(),
            "mailbox data directory is empty"
        );
        ensure!(
            (1..=MAX_MAILBOX_ENVELOPE_BYTES).contains(&self.max_envelope_bytes),
            "mailbox envelope limit is outside protocol bounds"
        );
        ensure!(
            (1..=MAX_ABSOLUTE_ITEMS_PER_MAILBOX).contains(&self.max_items_per_mailbox),
            "mailbox per-address item limit is invalid"
        );
        ensure!(
            (self.max_items_per_mailbox..=MAX_ABSOLUTE_ITEMS).contains(&self.max_total_items),
            "mailbox total item limit is invalid"
        );
        ensure!(
            (self.max_envelope_bytes as u64..=MAX_ABSOLUTE_TOTAL_BYTES)
                .contains(&self.max_total_bytes),
            "mailbox total byte limit is invalid"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct StoredRecord {
    version: u8,
    requested_ttl_seconds: u64,
    envelope: Vec<u8>,
    receipt: MailboxStoredReceipt,
}

impl StoredRecord {
    fn new(
        requested_ttl_seconds: u64,
        envelope: Vec<u8>,
        receipt: MailboxStoredReceipt,
    ) -> Result<Self> {
        let record = Self {
            version: RECORD_VERSION,
            requested_ttl_seconds,
            envelope,
            receipt,
        };
        record.validate()?;
        Ok(record)
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.version == RECORD_VERSION,
            "unsupported mailbox record version"
        );
        ensure!(
            (MIN_MAILBOX_TTL_SECONDS..=MAX_MAILBOX_TTL_SECONDS)
                .contains(&self.requested_ttl_seconds),
            "stored mailbox TTL is invalid"
        );
        ensure!(
            !self.envelope.is_empty() && self.envelope.len() <= MAX_MAILBOX_ENVELOPE_BYTES,
            "stored mailbox envelope is invalid"
        );
        self.receipt
            .verify(&self.envelope)
            .context("verify stored mailbox receipt")?;
        ensure!(
            self.receipt
                .expires_at_unix_seconds()
                .saturating_sub(self.receipt.stored_at_unix_seconds())
                == self.requested_ttl_seconds,
            "stored mailbox receipt TTL mismatch"
        );
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        postcard::to_allocvec(self).context("encode mailbox record")
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_MAILBOX_ENVELOPE_BYTES.saturating_add(2_048),
            "mailbox record is too large"
        );
        let record: Self = postcard::from_bytes(bytes).context("decode mailbox record")?;
        record.validate()?;
        Ok(record)
    }

    fn expired_at(&self, now: u64) -> bool {
        self.receipt.expires_at_unix_seconds() <= now
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct StoredTombstone {
    version: u8,
    stored_receipt_id: MailboxReceiptId,
    delete_receipt: MailboxDeleteReceipt,
}

impl StoredTombstone {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.version == RECORD_VERSION,
            "unsupported mailbox tombstone version"
        );
        self.delete_receipt
            .verify()
            .context("verify mailbox delete receipt")?;
        ensure!(
            self.delete_receipt.stored_receipt_id() == self.stored_receipt_id,
            "mailbox tombstone receipt mismatch"
        );
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        postcard::to_allocvec(self).context("encode mailbox tombstone")
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() <= 2_048, "mailbox tombstone is too large");
        let tombstone: Self = postcard::from_bytes(bytes).context("decode mailbox tombstone")?;
        tombstone.validate()?;
        Ok(tombstone)
    }

    fn expired_at(&self, now: u64) -> bool {
        self.delete_receipt.expires_at_unix_seconds() <= now
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MailboxPutOutcome {
    Created(MailboxStoredReceipt),
    AlreadyPresent(MailboxStoredReceipt),
    Conflict,
    Tombstoned,
    CapacityExceeded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredMailboxItem {
    pub item_id: MailboxItemId,
    pub expires_at_unix_seconds: u64,
    pub envelope: Vec<u8>,
    pub receipt: MailboxStoredReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeleteOutcome {
    Deleted(MailboxDeleteReceipt),
    AlreadyDeleted(MailboxDeleteReceipt),
    Absent,
    ReceiptMismatch,
    TombstoneCapacityExceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CleanupReport {
    pub removed_items: u64,
    pub removed_tombstones: u64,
    pub live_items: u64,
    pub live_envelope_bytes: u64,
}

pub struct BlindMailboxStore {
    database: Database,
    identity: MailboxStoreIdentity,
    config: MailboxStoreConfig,
}

impl BlindMailboxStore {
    pub fn open(config: MailboxStoreConfig, identity: MailboxStoreIdentity) -> Result<Self> {
        config.validate()?;
        std::fs::create_dir_all(&config.data_dir).with_context(|| {
            format!(
                "create mailbox data directory {}",
                config.data_dir.display()
            )
        })?;
        let metadata = std::fs::symlink_metadata(&config.data_dir).with_context(|| {
            format!(
                "inspect mailbox data directory {}",
                config.data_dir.display()
            )
        })?;
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "mailbox data directory must be a real directory, not a symlink"
        );
        let canonical = std::fs::canonicalize(&config.data_dir).with_context(|| {
            format!(
                "canonicalize mailbox data directory {}",
                config.data_dir.display()
            )
        })?;
        let path = canonical.join(DATABASE_FILE);
        let database = Database::create(&path)
            .with_context(|| format!("open mailbox database {}", path.display()))?;
        let write = database
            .begin_write()
            .context("begin mailbox initialization")?;
        {
            write
                .open_table(ITEM_TABLE)
                .context("open mailbox item table")?;
            write
                .open_table(TOMBSTONE_TABLE)
                .context("open mailbox tombstone table")?;
            write
                .open_table(MAILBOX_COUNT_TABLE)
                .context("open mailbox count table")?;
        }
        {
            let mut meta = write
                .open_table(META_TABLE)
                .context("open mailbox meta table")?;
            for key in [TOTAL_BYTES_KEY, TOTAL_ITEMS_KEY] {
                if meta.get(key).context("read mailbox accounting")?.is_none() {
                    meta.insert(key, 0)
                        .context("initialize mailbox accounting")?;
                }
            }
        }
        {
            let mut table = write
                .open_table(IDENTITY_TABLE)
                .context("open mailbox identity table")?;
            let expected = identity.store_key();
            let current = table
                .get(STORE_KEY)
                .context("read mailbox store identity")?
                .map(|value| value.value().to_vec());
            match current {
                Some(current) => ensure!(
                    current.as_slice() == expected.as_bytes(),
                    "mailbox store identity does not match existing database"
                ),
                None => {
                    table
                        .insert(STORE_KEY, expected.as_bytes().as_slice())
                        .context("initialize mailbox store identity")?;
                }
            }
        }
        write.commit().context("commit mailbox initialization")?;
        let store = Self {
            database,
            identity,
            config,
        };
        let (items, bytes) = store.live_usage()?;
        ensure!(
            items <= store.config.max_total_items && bytes <= store.config.max_total_bytes,
            "existing mailbox data exceeds configured capacity"
        );
        Ok(store)
    }

    pub fn store_key(&self) -> crate::MailboxStoreKey {
        self.identity.store_key()
    }

    pub fn put(
        &self,
        address: MailboxAddress,
        authorization: &MailboxWriteAuthorization,
        envelope: Vec<u8>,
        now_unix_seconds: u64,
    ) -> Result<MailboxPutOutcome> {
        authorization
            .verify(address, &envelope)
            .context("verify mailbox write authorization")?;
        ensure!(
            envelope.len() <= self.config.max_envelope_bytes,
            "mailbox envelope exceeds configured limit"
        );
        let expires = now_unix_seconds
            .checked_add(authorization.requested_ttl_seconds())
            .context("mailbox expiry overflow")?;
        let mailbox_id = address.mailbox_id();
        let item_id = authorization.item_id();
        let key = item_key(mailbox_id, item_id);
        let receipt = self.identity.stored_receipt(
            mailbox_id,
            item_id,
            &envelope,
            now_unix_seconds,
            expires,
        )?;
        let candidate = StoredRecord::new(
            authorization.requested_ttl_seconds(),
            envelope,
            receipt.clone(),
        )?;

        let mut write = self.database.begin_write().context("begin mailbox put")?;
        write
            .set_durability(Durability::Immediate)
            .context("set mailbox put durability")?;
        let current_bytes = {
            let table = write.open_table(ITEM_TABLE).context("open items for put")?;
            table
                .get(key.as_slice())
                .context("read mailbox item for put")?
                .map(|value| value.value().to_vec())
        };
        if let Some(ref current_bytes) = current_bytes {
            let current = StoredRecord::decode(current_bytes)?;
            if !current.expired_at(now_unix_seconds) {
                let outcome = if current.requested_ttl_seconds
                    == authorization.requested_ttl_seconds()
                    && current.envelope == candidate.envelope
                {
                    MailboxPutOutcome::AlreadyPresent(current.receipt)
                } else {
                    MailboxPutOutcome::Conflict
                };
                write.commit().context("commit idempotent mailbox put")?;
                return Ok(outcome);
            }
        }

        let tombstone_bytes = {
            let table = write
                .open_table(TOMBSTONE_TABLE)
                .context("open tombstones for put")?;
            table
                .get(key.as_slice())
                .context("read mailbox tombstone for put")?
                .map(|value| value.value().to_vec())
        };
        if let Some(tombstone_bytes) = tombstone_bytes {
            let tombstone = StoredTombstone::decode(&tombstone_bytes)?;
            if !tombstone.expired_at(now_unix_seconds) {
                write.commit().context("commit tombstoned mailbox put")?;
                return Ok(MailboxPutOutcome::Tombstoned);
            }
            write
                .open_table(TOMBSTONE_TABLE)
                .context("open expired tombstone for put")?
                .remove(key.as_slice())
                .context("remove expired mailbox tombstone")?;
        }

        let (mut total_items, mut total_bytes) = read_meta(&write)?;
        let mut mailbox_count = read_mailbox_count(&write, mailbox_id)?;
        if let Some(ref current_bytes) = current_bytes {
            let current = StoredRecord::decode(current_bytes)?;
            write
                .open_table(ITEM_TABLE)
                .context("open expired item for put")?
                .remove(key.as_slice())
                .context("remove expired mailbox item")?;
            total_items = total_items.saturating_sub(1);
            total_bytes = total_bytes.saturating_sub(current.envelope.len() as u64);
            mailbox_count = mailbox_count.saturating_sub(1);
        }
        if total_items >= self.config.max_total_items
            || mailbox_count >= self.config.max_items_per_mailbox
            || total_bytes.saturating_add(candidate.envelope.len() as u64)
                > self.config.max_total_bytes
        {
            write
                .commit()
                .context("commit capacity-rejected mailbox put")?;
            return Ok(MailboxPutOutcome::CapacityExceeded);
        }

        let encoded = candidate.encode()?;
        write
            .open_table(ITEM_TABLE)
            .context("open items for insert")?
            .insert(key.as_slice(), encoded.as_slice())
            .context("insert mailbox item")?;
        write_mailbox_count(&write, mailbox_id, mailbox_count + 1)?;
        write_meta(
            &write,
            total_items + 1,
            total_bytes.saturating_add(candidate.envelope.len() as u64),
        )?;
        write.commit().context("commit mailbox put")?;
        Ok(MailboxPutOutcome::Created(receipt))
    }

    pub fn list(
        &self,
        address: MailboxAddress,
        authorization: &MailboxReadAuthorization,
        now_unix_seconds: u64,
    ) -> Result<Vec<StoredMailboxItem>> {
        authorization
            .verify(address)
            .context("verify mailbox list authorization")?;
        ensure!(
            matches!(authorization.operation(), MailboxReadOperation::List { .. }),
            "mailbox read authorization is not a list operation"
        );
        self.cleanup(now_unix_seconds)?;
        let mailbox_id = address.mailbox_id();
        let read = self.database.begin_read().context("begin mailbox list")?;
        let table = read.open_table(ITEM_TABLE).context("open items for list")?;
        let mut items = Vec::new();
        for entry in table.iter().context("iterate mailbox items")? {
            let (key, value) = entry.context("read mailbox item entry")?;
            let key = key.value();
            if key.len() != ITEM_KEY_BYTES || &key[..32] != mailbox_id.as_bytes() {
                continue;
            }
            let mut item_bytes = [0_u8; 32];
            item_bytes.copy_from_slice(&key[32..]);
            let item_id = MailboxItemId::from_bytes(item_bytes);
            let record = StoredRecord::decode(value.value())?;
            ensure!(
                record.receipt.mailbox_id() == mailbox_id && record.receipt.item_id() == item_id,
                "mailbox record key does not match signed receipt"
            );
            items.push(StoredMailboxItem {
                item_id,
                expires_at_unix_seconds: record.receipt.expires_at_unix_seconds(),
                envelope: record.envelope,
                receipt: record.receipt,
            });
        }
        ensure!(
            items.len() as u64 <= self.config.max_items_per_mailbox,
            "mailbox list exceeds configured bound"
        );
        Ok(items)
    }

    pub fn delete(
        &self,
        address: MailboxAddress,
        authorization: &MailboxReadAuthorization,
        now_unix_seconds: u64,
    ) -> Result<DeleteOutcome> {
        authorization
            .verify(address)
            .context("verify mailbox delete authorization")?;
        let MailboxReadOperation::Delete {
            item_id,
            receipt_id,
        } = authorization.operation()
        else {
            anyhow::bail!("mailbox read authorization is not a delete operation");
        };
        self.cleanup(now_unix_seconds)?;
        let mailbox_id = address.mailbox_id();
        let key = item_key(mailbox_id, item_id);
        let mut write = self
            .database
            .begin_write()
            .context("begin mailbox delete")?;
        write
            .set_durability(Durability::Immediate)
            .context("set mailbox delete durability")?;
        let current_bytes = {
            let table = write
                .open_table(ITEM_TABLE)
                .context("open items for delete")?;
            table
                .get(key.as_slice())
                .context("read mailbox item for delete")?
                .map(|value| value.value().to_vec())
        };
        if let Some(current_bytes) = current_bytes {
            let current = StoredRecord::decode(&current_bytes)?;
            if current.receipt.receipt_id()? != receipt_id {
                write.commit().context("commit receipt-mismatched delete")?;
                return Ok(DeleteOutcome::ReceiptMismatch);
            }
            let tombstone_count = write
                .open_table(TOMBSTONE_TABLE)
                .context("open tombstones for capacity")?
                .len()
                .context("count mailbox tombstones")?;
            if tombstone_count >= self.config.max_total_items {
                write.commit().context("commit tombstone-capacity delete")?;
                return Ok(DeleteOutcome::TombstoneCapacityExceeded);
            }
            let delete_receipt = self.identity.delete_receipt(
                mailbox_id,
                item_id,
                receipt_id,
                now_unix_seconds,
                current.receipt.expires_at_unix_seconds(),
            )?;
            let tombstone = StoredTombstone {
                version: RECORD_VERSION,
                stored_receipt_id: receipt_id,
                delete_receipt: delete_receipt.clone(),
            };
            let encoded_tombstone = tombstone.encode()?;
            write
                .open_table(ITEM_TABLE)
                .context("open items for removal")?
                .remove(key.as_slice())
                .context("remove mailbox item")?;
            write
                .open_table(TOMBSTONE_TABLE)
                .context("open tombstones for insert")?
                .insert(key.as_slice(), encoded_tombstone.as_slice())
                .context("insert mailbox tombstone")?;
            let (total_items, total_bytes) = read_meta(&write)?;
            write_meta(
                &write,
                total_items.saturating_sub(1),
                total_bytes.saturating_sub(current.envelope.len() as u64),
            )?;
            let mailbox_count = read_mailbox_count(&write, mailbox_id)?;
            write_mailbox_count(&write, mailbox_id, mailbox_count.saturating_sub(1))?;
            write.commit().context("commit mailbox delete")?;
            return Ok(DeleteOutcome::Deleted(delete_receipt));
        }

        let tombstone = {
            let table = write
                .open_table(TOMBSTONE_TABLE)
                .context("open tombstones for delete replay")?;
            table
                .get(key.as_slice())
                .context("read mailbox tombstone for delete replay")?
                .map(|value| StoredTombstone::decode(value.value()))
                .transpose()?
        };
        write.commit().context("commit absent mailbox delete")?;
        Ok(match tombstone {
            Some(tombstone) if tombstone.stored_receipt_id == receipt_id => {
                DeleteOutcome::AlreadyDeleted(tombstone.delete_receipt)
            }
            Some(_) => DeleteOutcome::ReceiptMismatch,
            None => DeleteOutcome::Absent,
        })
    }

    pub fn cleanup(&self, now_unix_seconds: u64) -> Result<CleanupReport> {
        let mut write = self
            .database
            .begin_write()
            .context("begin mailbox cleanup")?;
        write
            .set_durability(Durability::Immediate)
            .context("set mailbox cleanup durability")?;
        let mut live_items = 0_u64;
        let mut live_bytes = 0_u64;
        let mut mailbox_counts = BTreeMap::<[u8; 32], u64>::new();
        let removed_items;
        {
            let mut table = write
                .open_table(ITEM_TABLE)
                .context("open items for cleanup")?;
            let before = table.len().context("count items before cleanup")?;
            table
                .retain(|key, value| {
                    let record = StoredRecord::decode(value).ok();
                    let retain = key.len() == ITEM_KEY_BYTES
                        && record
                            .as_ref()
                            .is_some_and(|record| !record.expired_at(now_unix_seconds));
                    if retain {
                        let mut mailbox = [0_u8; 32];
                        mailbox.copy_from_slice(&key[..32]);
                        let record = record.as_ref();
                        if let Some(record) = record {
                            live_items = live_items.saturating_add(1);
                            live_bytes = live_bytes.saturating_add(record.envelope.len() as u64);
                            let count = mailbox_counts.entry(mailbox).or_default();
                            *count = count.saturating_add(1);
                        }
                    }
                    retain
                })
                .context("retain live mailbox items")?;
            removed_items = before.saturating_sub(table.len().context("count live items")?);
        }
        let removed_tombstones;
        {
            let mut table = write
                .open_table(TOMBSTONE_TABLE)
                .context("open tombstones for cleanup")?;
            let before = table.len().context("count tombstones before cleanup")?;
            table
                .retain(|key, value| {
                    key.len() == ITEM_KEY_BYTES
                        && StoredTombstone::decode(value)
                            .is_ok_and(|tombstone| !tombstone.expired_at(now_unix_seconds))
                })
                .context("retain live mailbox tombstones")?;
            removed_tombstones =
                before.saturating_sub(table.len().context("count live tombstones")?);
        }
        {
            let mut counts = write
                .open_table(MAILBOX_COUNT_TABLE)
                .context("open mailbox counts for cleanup")?;
            counts
                .retain(|_, _| false)
                .context("clear mailbox counts")?;
            for (mailbox, count) in mailbox_counts {
                counts
                    .insert(mailbox.as_slice(), count)
                    .context("rebuild mailbox count")?;
            }
        }
        write_meta(&write, live_items, live_bytes)?;
        write.commit().context("commit mailbox cleanup")?;
        Ok(CleanupReport {
            removed_items,
            removed_tombstones,
            live_items,
            live_envelope_bytes: live_bytes,
        })
    }

    pub fn live_usage(&self) -> Result<(u64, u64)> {
        let read = self
            .database
            .begin_read()
            .context("begin mailbox usage read")?;
        let meta = read
            .open_table(META_TABLE)
            .context("open mailbox usage meta")?;
        let items = meta
            .get(TOTAL_ITEMS_KEY)
            .context("read mailbox total items")?
            .map_or(0, |value| value.value());
        let bytes = meta
            .get(TOTAL_BYTES_KEY)
            .context("read mailbox total bytes")?
            .map_or(0, |value| value.value());
        Ok((items, bytes))
    }
}

fn item_key(mailbox_id: MailboxId, item_id: MailboxItemId) -> [u8; ITEM_KEY_BYTES] {
    let mut key = [0_u8; ITEM_KEY_BYTES];
    key[..32].copy_from_slice(mailbox_id.as_bytes());
    key[32..].copy_from_slice(item_id.as_bytes());
    key
}

fn read_meta(write: &redb::WriteTransaction) -> Result<(u64, u64)> {
    let meta = write
        .open_table(META_TABLE)
        .context("open mailbox accounting")?;
    let items = meta
        .get(TOTAL_ITEMS_KEY)
        .context("read mailbox item accounting")?
        .map_or(0, |value| value.value());
    let bytes = meta
        .get(TOTAL_BYTES_KEY)
        .context("read mailbox byte accounting")?
        .map_or(0, |value| value.value());
    Ok((items, bytes))
}

fn write_meta(write: &redb::WriteTransaction, items: u64, bytes: u64) -> Result<()> {
    let mut meta = write
        .open_table(META_TABLE)
        .context("open mailbox accounting update")?;
    meta.insert(TOTAL_ITEMS_KEY, items)
        .context("write mailbox item accounting")?;
    meta.insert(TOTAL_BYTES_KEY, bytes)
        .context("write mailbox byte accounting")?;
    Ok(())
}

fn read_mailbox_count(write: &redb::WriteTransaction, mailbox_id: MailboxId) -> Result<u64> {
    Ok(write
        .open_table(MAILBOX_COUNT_TABLE)
        .context("open mailbox count")?
        .get(mailbox_id.as_bytes().as_slice())
        .context("read mailbox count")?
        .map_or(0, |value| value.value()))
}

fn write_mailbox_count(
    write: &redb::WriteTransaction,
    mailbox_id: MailboxId,
    count: u64,
) -> Result<()> {
    let mut table = write
        .open_table(MAILBOX_COUNT_TABLE)
        .context("open mailbox count update")?;
    if count == 0 {
        table
            .remove(mailbox_id.as_bytes().as_slice())
            .context("remove empty mailbox count")?;
    } else {
        table
            .insert(mailbox_id.as_bytes().as_slice(), count)
            .context("write mailbox count")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use kilogram_crypto::DeviceEncryptionIdentity;

    use super::*;
    use crate::{
        MailboxEnvelope, MailboxReadCapability, MailboxRequestNonce, MailboxWriteCapability,
    };

    #[test]
    fn durable_delivery_is_bounded_receipted_and_replay_safe() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut config = MailboxStoreConfig::new(directory.path().to_path_buf());
        config.max_envelope_bytes = 4_096;
        config.max_items_per_mailbox = 1;
        config.max_total_items = 2;
        config.max_total_bytes = 4_096;
        let store_identity = MailboxStoreIdentity::from_secret_bytes([9_u8; 32]);
        let expected_store_key = store_identity.store_key();
        let store = BlindMailboxStore::open(config.clone(), store_identity)?;
        assert_eq!(store.store_key(), expected_store_key);

        let read = MailboxReadCapability::from_secret_bytes([1_u8; 32]);
        let write = MailboxWriteCapability::from_secret_bytes([2_u8; 32]);
        let address = MailboxAddress::new(read.read_key(), write.write_key());
        let recipient = DeviceEncryptionIdentity::from_secret_bytes([3_u8; 32]);
        let item = MailboxItemId::from_bytes([4_u8; 32]);
        let envelope = MailboxEnvelope::seal(
            address.mailbox_id(),
            item,
            1_000,
            1_600,
            recipient.public_key(),
            b"signed-authorized-event",
        )?
        .encode()?;
        let write_authorization = write.authorize(address, item, 600, &envelope)?;
        let created = store.put(address, &write_authorization, envelope.clone(), 1_000)?;
        let receipt = match created {
            MailboxPutOutcome::Created(receipt) => receipt,
            outcome => anyhow::bail!("unexpected mailbox put outcome: {outcome:?}"),
        };
        receipt.verify(&envelope)?;
        let receipt = MailboxStoredReceipt::decode_and_verify(&receipt.encode()?, &envelope)?;
        assert_eq!(receipt.store_key(), expected_store_key);
        assert!(matches!(
            store.put(address, &write_authorization, envelope.clone(), 1_001)?,
            MailboxPutOutcome::AlreadyPresent(_)
        ));

        let second_item = MailboxItemId::from_bytes([5_u8; 32]);
        let second_envelope = MailboxEnvelope::seal(
            address.mailbox_id(),
            second_item,
            1_001,
            1_601,
            recipient.public_key(),
            b"second",
        )?
        .encode()?;
        let second_authorization = write.authorize(address, second_item, 600, &second_envelope)?;
        assert_eq!(
            store.put(address, &second_authorization, second_envelope, 1_001)?,
            MailboxPutOutcome::CapacityExceeded
        );

        let list_authorization =
            read.authorize_list(address, MailboxRequestNonce::from_bytes([6_u8; 32]))?;
        let listed = store.list(address, &list_authorization, 1_002)?;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].item_id, item);
        listed[0].receipt.verify(&listed[0].envelope)?;
        let opened = MailboxEnvelope::decode(&listed[0].envelope)?.open(
            address.mailbox_id(),
            item,
            &recipient,
            1_002,
        )?;
        assert_eq!(opened, b"signed-authorized-event");

        let receipt_id = receipt.receipt_id()?;
        let delete_authorization = read.authorize_delete(address, item, receipt_id)?;
        let deleted = store.delete(address, &delete_authorization, 1_003)?;
        let delete_receipt = match deleted {
            DeleteOutcome::Deleted(receipt) => receipt,
            outcome => anyhow::bail!("unexpected mailbox delete outcome: {outcome:?}"),
        };
        delete_receipt.verify()?;
        let delete_receipt = MailboxDeleteReceipt::decode_and_verify(&delete_receipt.encode()?)?;
        assert_eq!(delete_receipt.stored_receipt_id(), receipt_id);
        assert!(matches!(
            store.delete(address, &delete_authorization, 1_004)?,
            DeleteOutcome::AlreadyDeleted(_)
        ));
        assert_eq!(store.live_usage()?, (0, 0));
        assert!(store.list(address, &list_authorization, 1_004)?.is_empty());
        assert_eq!(
            store.put(address, &write_authorization, envelope, 1_004)?,
            MailboxPutOutcome::Tombstoned
        );

        drop(store);
        assert!(
            BlindMailboxStore::open(
                config.clone(),
                MailboxStoreIdentity::from_secret_bytes([8_u8; 32]),
            )
            .is_err()
        );
        let reopened =
            BlindMailboxStore::open(config, MailboxStoreIdentity::from_secret_bytes([9_u8; 32]))?;
        assert!(matches!(
            reopened.delete(address, &delete_authorization, 1_005)?,
            DeleteOutcome::AlreadyDeleted(_)
        ));
        let cleanup = reopened.cleanup(1_600)?;
        assert_eq!(cleanup.removed_tombstones, 1);
        Ok(())
    }

    #[test]
    fn wrong_capabilities_and_receipts_fail_closed() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = BlindMailboxStore::open(
            MailboxStoreConfig::new(directory.path().to_path_buf()),
            MailboxStoreIdentity::from_secret_bytes([9_u8; 32]),
        )?;
        let read = MailboxReadCapability::from_secret_bytes([1_u8; 32]);
        let write = MailboxWriteCapability::from_secret_bytes([2_u8; 32]);
        let address = MailboxAddress::new(read.read_key(), write.write_key());
        let item = MailboxItemId::from_bytes([4_u8; 32]);
        let recipient = DeviceEncryptionIdentity::from_secret_bytes([3_u8; 32]);
        let envelope = MailboxEnvelope::seal(
            address.mailbox_id(),
            item,
            1_000,
            1_600,
            recipient.public_key(),
            b"payload",
        )?
        .encode()?;
        let authorization = write.authorize(address, item, 600, &envelope)?;
        let receipt = match store.put(address, &authorization, envelope, 1_000)? {
            MailboxPutOutcome::Created(receipt) => receipt,
            outcome => anyhow::bail!("unexpected put outcome: {outcome:?}"),
        };
        let wrong_read = MailboxReadCapability::from_secret_bytes([8_u8; 32]);
        assert!(
            wrong_read
                .authorize_list(address, MailboxRequestNonce::from_bytes([7_u8; 32]))
                .is_err()
        );
        let wrong_delete =
            read.authorize_delete(address, item, MailboxReceiptId::from_bytes([0_u8; 32]))?;
        assert_eq!(
            store.delete(address, &wrong_delete, 1_001)?,
            DeleteOutcome::ReceiptMismatch
        );
        let expired_list =
            read.authorize_list(address, MailboxRequestNonce::from_bytes([6_u8; 32]))?;
        assert!(store.list(address, &expired_list, 1_600)?.is_empty());
        assert_eq!(store.live_usage()?, (0, 0));
        assert!(receipt.receipt_id().is_ok());
        Ok(())
    }
}
