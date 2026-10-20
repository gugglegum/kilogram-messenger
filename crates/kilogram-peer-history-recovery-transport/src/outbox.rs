use std::path::{Path, PathBuf};

use iroh::EndpointId;
use kilogram_identity::{AccountId, DeviceId, DeviceIdentity};
use redb::{
    Database, Durability, ReadOnlyDatabase, ReadableDatabase, ReadableTable, ReadableTableMetadata,
    TableDefinition,
};
use thiserror::Error;

use super::{
    HistoryRecoveryTransferTransportError, PreparedHistoryRecoveryTransferSession,
    SignedHistoryRecoveryTransferSessionReceipt,
};

const DATABASE_FILE: &str = "old-history-recovery-transfer-outbox-v1.redb";
const META_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("old-history-recovery-transfer-outbox-meta-v1");
const REQUEST_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("old-history-recovery-transfer-outbox-request-v1");
const HEAD_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("old-history-recovery-transfer-outbox-head-v1");
const RECEIPT_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("old-history-recovery-transfer-outbox-receipt-v1");

const SCHEMA_KEY: &str = "schema-version";
const OWNER_ACCOUNT_KEY: &str = "owner-account-id";
const SOURCE_DEVICE_KEY: &str = "source-device-id";
const SCHEMA_VERSION: u8 = 1;
const RECIPIENT_TOKEN_DOMAIN: &[u8] =
    b"kilogram:history-recovery-transfer-outbox-recipient-token:v1\0";

pub const MAX_DURABLE_HISTORY_RECOVERY_TRANSFER_INTENTS: u64 = 16_384;

pub struct HistoryRecoveryTransferOutbox {
    directory: PathBuf,
    database: Database,
    owner_account_id: AccountId,
    source_device_id: DeviceId,
}

pub struct HistoryRecoveryTransferOutboxReadStore {
    directory: PathBuf,
    database: ReadOnlyDatabase,
    owner_account_id: AccountId,
    source_device_id: DeviceId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryRecoveryTransferOutboxStatusView {
    pub request_id: [u8; 32],
    pub transfer_id: [u8; 32],
    pub recipient_device_token: [u8; 16],
    pub acknowledged: bool,
    pub contains_message_bodies: bool,
    pub contains_plaintext: bool,
    pub grants_network_authority: bool,
    pub grants_import_authority: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistoryRecoveryTransferOutboxStatus {
    Absent,
    Pending(HistoryRecoveryTransferOutboxStatusView),
    Acknowledged(HistoryRecoveryTransferOutboxStatusView),
}

pub enum PrepareHistoryRecoveryTransferOutcome {
    Prepared(PreparedHistoryRecoveryTransferSession),
    AlreadyPrepared(PreparedHistoryRecoveryTransferSession),
}

pub enum AcknowledgeHistoryRecoveryTransferOutcome {
    Acknowledged(SignedHistoryRecoveryTransferSessionReceipt),
    AlreadyAcknowledged(SignedHistoryRecoveryTransferSessionReceipt),
}

impl HistoryRecoveryTransferOutboxReadStore {
    pub fn open_existing(
        directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        source_device_id: DeviceId,
    ) -> Result<Self, HistoryRecoveryTransferOutboxError> {
        let directory = directory.as_ref().to_path_buf();
        let database_path = directory.join(DATABASE_FILE);
        if !database_path.is_file() {
            return Err(HistoryRecoveryTransferOutboxError::StoreNotInitialized);
        }
        let database = ReadOnlyDatabase::open(database_path).map_err(database_error)?;
        verify_database_metadata(&database, owner_account_id, source_device_id)?;
        verify_all(&database, owner_account_id, source_device_id)?;
        Ok(Self {
            directory,
            database,
            owner_account_id,
            source_device_id,
        })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn load(
        &self,
        transfer_id: [u8; 32],
    ) -> Result<Option<PreparedHistoryRecoveryTransferSession>, HistoryRecoveryTransferOutboxError>
    {
        load_verified_request(
            &self.database,
            self.owner_account_id,
            self.source_device_id,
            transfer_id,
        )
    }

    pub fn status(
        &self,
        transfer_id: [u8; 32],
    ) -> Result<HistoryRecoveryTransferOutboxStatus, HistoryRecoveryTransferOutboxError> {
        status_for(
            &self.database,
            self.owner_account_id,
            self.source_device_id,
            transfer_id,
        )
    }
}

impl HistoryRecoveryTransferOutbox {
    pub fn is_initialized(directory: impl AsRef<Path>) -> bool {
        directory.as_ref().join(DATABASE_FILE).is_file()
    }

    pub fn open(
        directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        source_device_id: DeviceId,
    ) -> Result<Self, HistoryRecoveryTransferOutboxError> {
        let directory = directory.as_ref().to_path_buf();
        std::fs::create_dir_all(&directory)?;
        let database_path = directory.join(DATABASE_FILE);
        let existed = database_path.is_file();
        let database = Database::create(database_path).map_err(database_error)?;
        if existed {
            verify_database_metadata(&database, owner_account_id, source_device_id)?;
        } else {
            initialize_database(&database, owner_account_id, source_device_id)?;
        }
        verify_all(&database, owner_account_id, source_device_id)?;
        Ok(Self {
            directory,
            database,
            owner_account_id,
            source_device_id,
        })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn load(
        &self,
        transfer_id: [u8; 32],
    ) -> Result<Option<PreparedHistoryRecoveryTransferSession>, HistoryRecoveryTransferOutboxError>
    {
        load_verified_request(
            &self.database,
            self.owner_account_id,
            self.source_device_id,
            transfer_id,
        )
    }

    pub fn status(
        &self,
        transfer_id: [u8; 32],
    ) -> Result<HistoryRecoveryTransferOutboxStatus, HistoryRecoveryTransferOutboxError> {
        status_for(
            &self.database,
            self.owner_account_id,
            self.source_device_id,
            transfer_id,
        )
    }

    pub fn prepare(
        &self,
        envelope_bytes: &[u8],
        source: &DeviceIdentity,
        source_endpoint_id: EndpointId,
        recipient_endpoint_id: EndpointId,
        session_nonce: [u8; 32],
    ) -> Result<PrepareHistoryRecoveryTransferOutcome, HistoryRecoveryTransferOutboxError> {
        self.prepare_inner(
            envelope_bytes,
            source,
            source_endpoint_id,
            recipient_endpoint_id,
            session_nonce,
            false,
        )
    }

    fn prepare_inner(
        &self,
        envelope_bytes: &[u8],
        source: &DeviceIdentity,
        source_endpoint_id: EndpointId,
        recipient_endpoint_id: EndpointId,
        session_nonce: [u8; 32],
        fail_before_commit: bool,
    ) -> Result<PrepareHistoryRecoveryTransferOutcome, HistoryRecoveryTransferOutboxError> {
        if source.device_id() != self.source_device_id {
            return Err(HistoryRecoveryTransferOutboxError::SourceDeviceMismatch);
        }
        let prepared = PreparedHistoryRecoveryTransferSession::create(
            envelope_bytes,
            source,
            source_endpoint_id,
            recipient_endpoint_id,
            session_nonce,
        )?;
        self.verify_request_owner(&prepared)?;
        let transfer_id = prepared.transfer_id();
        if let Some(existing) = self.load(transfer_id)? {
            if existing.encoded_request() == prepared.encoded_request() {
                return Ok(PrepareHistoryRecoveryTransferOutcome::AlreadyPrepared(
                    existing,
                ));
            }
            return Err(HistoryRecoveryTransferOutboxError::TransferAlreadyPrepared);
        }

        let request_id = prepared.request_id();
        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut requests = write.open_table(REQUEST_TABLE).map_err(database_error)?;
            let mut heads = write.open_table(HEAD_TABLE).map_err(database_error)?;
            if requests.len().map_err(database_error)?
                >= MAX_DURABLE_HISTORY_RECOVERY_TRANSFER_INTENTS
                || heads.len().map_err(database_error)?
                    >= MAX_DURABLE_HISTORY_RECOVERY_TRANSFER_INTENTS
            {
                return Err(HistoryRecoveryTransferOutboxError::Capacity);
            }
            if heads
                .get(transfer_id.as_slice())
                .map_err(database_error)?
                .is_some()
            {
                return Err(HistoryRecoveryTransferOutboxError::TransferAlreadyPrepared);
            }
            if let Some(existing) = requests
                .get(request_id.as_slice())
                .map_err(database_error)?
            {
                if existing.value() != prepared.encoded_request() {
                    return Err(HistoryRecoveryTransferOutboxError::RequestIdCollision);
                }
                return Err(HistoryRecoveryTransferOutboxError::OrphanRequest);
            }
            requests
                .insert(request_id.as_slice(), prepared.encoded_request())
                .map_err(database_error)?;
            heads
                .insert(transfer_id.as_slice(), request_id.as_slice())
                .map_err(database_error)?;
        }
        if fail_before_commit {
            return Err(HistoryRecoveryTransferOutboxError::InjectedBeforeCommit);
        }
        write.commit().map_err(database_error)?;
        let durable = self
            .load(transfer_id)?
            .ok_or(HistoryRecoveryTransferOutboxError::MissingHead)?;
        if durable.encoded_request() != prepared.encoded_request() {
            return Err(HistoryRecoveryTransferOutboxError::CommittedRequestMismatch);
        }
        Ok(PrepareHistoryRecoveryTransferOutcome::Prepared(durable))
    }

    pub fn acknowledge(
        &self,
        transfer_id: [u8; 32],
        receipt_bytes: &[u8],
    ) -> Result<AcknowledgeHistoryRecoveryTransferOutcome, HistoryRecoveryTransferOutboxError> {
        self.acknowledge_inner(transfer_id, receipt_bytes, false)
    }

    fn acknowledge_inner(
        &self,
        transfer_id: [u8; 32],
        receipt_bytes: &[u8],
        fail_before_commit: bool,
    ) -> Result<AcknowledgeHistoryRecoveryTransferOutcome, HistoryRecoveryTransferOutboxError> {
        let prepared = self
            .load(transfer_id)?
            .ok_or(HistoryRecoveryTransferOutboxError::MissingHead)?;
        let receipt = SignedHistoryRecoveryTransferSessionReceipt::decode_and_verify(
            receipt_bytes,
            &prepared,
        )?;
        let request_id = prepared.request_id();
        let read = self.database.begin_read().map_err(database_error)?;
        let receipts = read.open_table(RECEIPT_TABLE).map_err(database_error)?;
        if let Some(existing) = receipts
            .get(request_id.as_slice())
            .map_err(database_error)?
        {
            let existing_bytes = existing.value().to_vec();
            drop(existing);
            drop(receipts);
            drop(read);
            let existing = SignedHistoryRecoveryTransferSessionReceipt::decode_and_verify(
                &existing_bytes,
                &prepared,
            )?;
            if existing.encoded_receipt() == receipt_bytes {
                return Ok(
                    AcknowledgeHistoryRecoveryTransferOutcome::AlreadyAcknowledged(existing),
                );
            }
            return Err(HistoryRecoveryTransferOutboxError::ReceiptConflict);
        }
        drop(receipts);
        drop(read);

        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut receipts = write.open_table(RECEIPT_TABLE).map_err(database_error)?;
            if receipts
                .get(request_id.as_slice())
                .map_err(database_error)?
                .is_some()
            {
                return Err(HistoryRecoveryTransferOutboxError::ReceiptConflict);
            }
            receipts
                .insert(request_id.as_slice(), receipt_bytes)
                .map_err(database_error)?;
        }
        if fail_before_commit {
            return Err(HistoryRecoveryTransferOutboxError::InjectedBeforeCommit);
        }
        write.commit().map_err(database_error)?;
        match self.status(transfer_id)? {
            HistoryRecoveryTransferOutboxStatus::Acknowledged(_) => Ok(
                AcknowledgeHistoryRecoveryTransferOutcome::Acknowledged(receipt),
            ),
            _ => Err(HistoryRecoveryTransferOutboxError::CommittedReceiptMismatch),
        }
    }

    fn verify_request_owner(
        &self,
        request: &PreparedHistoryRecoveryTransferSession,
    ) -> Result<(), HistoryRecoveryTransferOutboxError> {
        if request.owner_account_id() != self.owner_account_id {
            return Err(HistoryRecoveryTransferOutboxError::OwnerAccountMismatch);
        }
        if request.source_device_id() != self.source_device_id {
            return Err(HistoryRecoveryTransferOutboxError::SourceDeviceMismatch);
        }
        Ok(())
    }
}

fn status_for<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    source_device_id: DeviceId,
    transfer_id: [u8; 32],
) -> Result<HistoryRecoveryTransferOutboxStatus, HistoryRecoveryTransferOutboxError> {
    let Some(request) =
        load_verified_request(database, owner_account_id, source_device_id, transfer_id)?
    else {
        return Ok(HistoryRecoveryTransferOutboxStatus::Absent);
    };
    let read = database.begin_read().map_err(database_error)?;
    let receipts = read.open_table(RECEIPT_TABLE).map_err(database_error)?;
    let acknowledged = if let Some(encoded) = receipts
        .get(request.request_id().as_slice())
        .map_err(database_error)?
    {
        SignedHistoryRecoveryTransferSessionReceipt::decode_and_verify(encoded.value(), &request)?;
        true
    } else {
        false
    };
    let view = HistoryRecoveryTransferOutboxStatusView {
        request_id: request.request_id(),
        transfer_id,
        recipient_device_token: recipient_token(owner_account_id, request.recipient_device_id()),
        acknowledged,
        contains_message_bodies: false,
        contains_plaintext: false,
        grants_network_authority: false,
        grants_import_authority: false,
    };
    Ok(if acknowledged {
        HistoryRecoveryTransferOutboxStatus::Acknowledged(view)
    } else {
        HistoryRecoveryTransferOutboxStatus::Pending(view)
    })
}

fn load_verified_request<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    source_device_id: DeviceId,
    transfer_id: [u8; 32],
) -> Result<Option<PreparedHistoryRecoveryTransferSession>, HistoryRecoveryTransferOutboxError> {
    if transfer_id == [0_u8; 32] {
        return Err(HistoryRecoveryTransferOutboxError::InvalidTransferId);
    }
    let read = database.begin_read().map_err(database_error)?;
    let heads = read.open_table(HEAD_TABLE).map_err(database_error)?;
    let Some(request_id) = heads.get(transfer_id.as_slice()).map_err(database_error)? else {
        return Ok(None);
    };
    let request_id = copy_id(request_id.value())?;
    let requests = read.open_table(REQUEST_TABLE).map_err(database_error)?;
    let encoded = requests
        .get(request_id.as_slice())
        .map_err(database_error)?
        .ok_or(HistoryRecoveryTransferOutboxError::MissingRequest)?;
    let prepared = PreparedHistoryRecoveryTransferSession::decode_and_verify(encoded.value())?;
    if prepared.request_id() != request_id || prepared.transfer_id() != transfer_id {
        return Err(HistoryRecoveryTransferOutboxError::HeadRequestMismatch);
    }
    if prepared.owner_account_id() != owner_account_id {
        return Err(HistoryRecoveryTransferOutboxError::OwnerAccountMismatch);
    }
    if prepared.source_device_id() != source_device_id {
        return Err(HistoryRecoveryTransferOutboxError::SourceDeviceMismatch);
    }
    Ok(Some(prepared))
}

fn verify_all<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    source_device_id: DeviceId,
) -> Result<(), HistoryRecoveryTransferOutboxError> {
    let read = database.begin_read().map_err(database_error)?;
    let heads = read.open_table(HEAD_TABLE).map_err(database_error)?;
    let requests = read.open_table(REQUEST_TABLE).map_err(database_error)?;
    let receipts = read.open_table(RECEIPT_TABLE).map_err(database_error)?;
    if heads.len().map_err(database_error)? > MAX_DURABLE_HISTORY_RECOVERY_TRANSFER_INTENTS
        || requests.len().map_err(database_error)? > MAX_DURABLE_HISTORY_RECOVERY_TRANSFER_INTENTS
        || receipts.len().map_err(database_error)? > MAX_DURABLE_HISTORY_RECOVERY_TRANSFER_INTENTS
    {
        return Err(HistoryRecoveryTransferOutboxError::Capacity);
    }
    let mut referenced = std::collections::HashSet::new();
    let mut transfer_ids = Vec::new();
    for entry in heads.iter().map_err(database_error)? {
        let (transfer_id, request_id) = entry.map_err(database_error)?;
        let transfer_id = copy_id(transfer_id.value())?;
        let request_id = copy_id(request_id.value())?;
        if !referenced.insert(request_id) {
            return Err(HistoryRecoveryTransferOutboxError::DuplicateRequestHead);
        }
        transfer_ids.push(transfer_id);
    }
    for entry in requests.iter().map_err(database_error)? {
        let (request_id, encoded) = entry.map_err(database_error)?;
        let request_id = copy_id(request_id.value())?;
        if !referenced.contains(&request_id) {
            return Err(HistoryRecoveryTransferOutboxError::OrphanRequest);
        }
        let request = PreparedHistoryRecoveryTransferSession::decode_and_verify(encoded.value())?;
        if request.request_id() != request_id
            || request.owner_account_id() != owner_account_id
            || request.source_device_id() != source_device_id
        {
            return Err(HistoryRecoveryTransferOutboxError::RequestBinding);
        }
    }
    for entry in receipts.iter().map_err(database_error)? {
        let (request_id, encoded) = entry.map_err(database_error)?;
        let request_id = copy_id(request_id.value())?;
        let request_bytes = requests
            .get(request_id.as_slice())
            .map_err(database_error)?
            .ok_or(HistoryRecoveryTransferOutboxError::OrphanReceipt)?;
        let request =
            PreparedHistoryRecoveryTransferSession::decode_and_verify(request_bytes.value())?;
        SignedHistoryRecoveryTransferSessionReceipt::decode_and_verify(encoded.value(), &request)?;
    }
    drop(receipts);
    drop(requests);
    drop(heads);
    drop(read);
    for transfer_id in transfer_ids {
        if load_verified_request(database, owner_account_id, source_device_id, transfer_id)?
            .is_none()
        {
            return Err(HistoryRecoveryTransferOutboxError::MissingHead);
        }
    }
    Ok(())
}

fn initialize_database(
    database: &Database,
    owner_account_id: AccountId,
    source_device_id: DeviceId,
) -> Result<(), HistoryRecoveryTransferOutboxError> {
    let mut write = database.begin_write().map_err(database_error)?;
    write
        .set_durability(Durability::Immediate)
        .map_err(database_error)?;
    {
        let mut meta = write.open_table(META_TABLE).map_err(database_error)?;
        initialize_meta_value(&mut meta, SCHEMA_KEY, &[SCHEMA_VERSION])?;
        initialize_meta_value(&mut meta, OWNER_ACCOUNT_KEY, owner_account_id.as_bytes())?;
        initialize_meta_value(&mut meta, SOURCE_DEVICE_KEY, source_device_id.as_bytes())?;
    }
    write.open_table(REQUEST_TABLE).map_err(database_error)?;
    write.open_table(HEAD_TABLE).map_err(database_error)?;
    write.open_table(RECEIPT_TABLE).map_err(database_error)?;
    write.commit().map_err(database_error)
}

fn initialize_meta_value(
    meta: &mut redb::Table<'_, &str, &[u8]>,
    key: &'static str,
    expected: &[u8],
) -> Result<(), HistoryRecoveryTransferOutboxError> {
    if let Some(value) = meta.get(key).map_err(database_error)? {
        if value.value() != expected {
            return Err(HistoryRecoveryTransferOutboxError::MetadataMismatch);
        }
    } else {
        meta.insert(key, expected).map_err(database_error)?;
    }
    Ok(())
}

fn verify_database_metadata<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    source_device_id: DeviceId,
) -> Result<(), HistoryRecoveryTransferOutboxError> {
    let read = database.begin_read().map_err(database_error)?;
    let meta = read.open_table(META_TABLE).map_err(database_error)?;
    verify_meta_value(&meta, SCHEMA_KEY, &[SCHEMA_VERSION])?;
    verify_meta_value(&meta, OWNER_ACCOUNT_KEY, owner_account_id.as_bytes())?;
    verify_meta_value(&meta, SOURCE_DEVICE_KEY, source_device_id.as_bytes())?;
    drop(meta);
    let requests = read.open_table(REQUEST_TABLE).map_err(database_error)?;
    drop(requests);
    let heads = read.open_table(HEAD_TABLE).map_err(database_error)?;
    drop(heads);
    let receipts = read.open_table(RECEIPT_TABLE).map_err(database_error)?;
    drop(receipts);
    Ok(())
}

fn verify_meta_value(
    meta: &impl ReadableTable<&'static str, &'static [u8]>,
    key: &'static str,
    expected: &[u8],
) -> Result<(), HistoryRecoveryTransferOutboxError> {
    let value = meta
        .get(key)
        .map_err(database_error)?
        .ok_or(HistoryRecoveryTransferOutboxError::MetadataMismatch)?;
    if value.value() != expected {
        return Err(HistoryRecoveryTransferOutboxError::MetadataMismatch);
    }
    Ok(())
}

fn copy_id(bytes: &[u8]) -> Result<[u8; 32], HistoryRecoveryTransferOutboxError> {
    bytes
        .try_into()
        .map_err(|_| HistoryRecoveryTransferOutboxError::InvalidStoredId)
}

fn recipient_token(owner_account_id: AccountId, recipient_device_id: DeviceId) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(RECIPIENT_TOKEN_DOMAIN);
    hasher.update(owner_account_id.as_bytes());
    hasher.update(recipient_device_id.as_bytes());
    let digest = hasher.finalize();
    let mut token = [0_u8; 16];
    token.copy_from_slice(&digest.as_bytes()[..16]);
    token
}

fn database_error(error: impl std::fmt::Display) -> HistoryRecoveryTransferOutboxError {
    HistoryRecoveryTransferOutboxError::Database(error.to_string())
}

#[derive(Debug, Error)]
pub enum HistoryRecoveryTransferOutboxError {
    #[error("history recovery transfer outbox is not initialized")]
    StoreNotInitialized,
    #[error("history recovery transfer outbox metadata does not match")]
    MetadataMismatch,
    #[error("history recovery transfer outbox owner Account does not match")]
    OwnerAccountMismatch,
    #[error("history recovery transfer outbox source Device does not match")]
    SourceDeviceMismatch,
    #[error("history recovery transfer ID is invalid")]
    InvalidTransferId,
    #[error("history recovery transfer outbox capacity was reached")]
    Capacity,
    #[error("a different request is already durable for this transfer")]
    TransferAlreadyPrepared,
    #[error("history recovery transfer request ID collision")]
    RequestIdCollision,
    #[error("history recovery transfer request is missing")]
    MissingRequest,
    #[error("history recovery transfer head is missing")]
    MissingHead,
    #[error("history recovery transfer request is orphaned")]
    OrphanRequest,
    #[error("history recovery transfer receipt is orphaned")]
    OrphanReceipt,
    #[error("history recovery transfer head and request do not match")]
    HeadRequestMismatch,
    #[error("history recovery transfer request binding is invalid")]
    RequestBinding,
    #[error("history recovery transfer request has more than one head")]
    DuplicateRequestHead,
    #[error("a different receipt is already durable for this request")]
    ReceiptConflict,
    #[error("committed history recovery request did not round-trip")]
    CommittedRequestMismatch,
    #[error("committed history recovery receipt did not round-trip")]
    CommittedReceiptMismatch,
    #[error("stored history recovery transfer ID has invalid length")]
    InvalidStoredId,
    #[error("injected failure before durable commit")]
    InjectedBeforeCommit,
    #[error("history recovery transfer outbox database failed: {0}")]
    Database(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Transport(#[from] HistoryRecoveryTransferTransportError),
}

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use iroh::SecretKey;
    use kilogram_peer_history_recovery_execution::HistoryRecoveryTransferEnvelope;

    use super::*;

    type PreparedFixture = (
        tempfile::TempDir,
        HistoryRecoveryTransferEnvelope,
        DeviceIdentity,
        PreparedHistoryRecoveryTransferSession,
        SignedHistoryRecoveryTransferSessionReceipt,
    );
    use crate::{TransferStageDisposition, VerifiedTransferSessionResponse};

    const SOURCE_SECRET_HEX: &str =
        "3fc00be615014da89b58a3a10ceb9b98e369449f6de6c702a521d638935a4e80";
    const RECIPIENT_SECRET_HEX: &str =
        "57d154005415eb97298e29fea08e518d6a35238dc9aa32aa688e401f526efcd8";

    #[test]
    fn outbox_restart_preserves_exact_prepared_request() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let envelope = HistoryRecoveryTransferEnvelope::decode_and_verify(&fixture)?;
        let source = source_identity()?;
        let state = tempfile::tempdir()?;
        let source_endpoint = SecretKey::generate().public();
        let recipient_endpoint = SecretKey::generate().public();
        let encoded = {
            let outbox = HistoryRecoveryTransferOutbox::open(
                state.path(),
                envelope.owner_account_id(),
                source.device_id(),
            )?;
            let prepared = prepared_from_outcome(outbox.prepare(
                &fixture,
                &source,
                source_endpoint,
                recipient_endpoint,
                [0x71; 32],
            )?)?;
            assert!(matches!(
                outbox.status(envelope.transfer_id()?)?,
                HistoryRecoveryTransferOutboxStatus::Pending(_)
            ));
            prepared.encoded_request().to_vec()
        };
        let reopened = HistoryRecoveryTransferOutboxReadStore::open_existing(
            state.path(),
            envelope.owner_account_id(),
            source.device_id(),
        )?;
        let loaded = reopened
            .load(envelope.transfer_id()?)?
            .ok_or("missing durable request")?;
        assert_eq!(loaded.encoded_request(), encoded);
        assert_eq!(loaded.session_nonce(), [0x71; 32]);
        Ok(())
    }

    #[test]
    fn outbox_acknowledgement_and_exact_replay_are_idempotent()
    -> Result<(), Box<dyn std::error::Error>> {
        let (state, envelope, source, prepared, receipt) = prepared_fixture()?;
        let outbox = HistoryRecoveryTransferOutbox::open(
            state.path(),
            envelope.owner_account_id(),
            source.device_id(),
        )?;
        let first = outbox.acknowledge(envelope.transfer_id()?, receipt.encoded_receipt())?;
        assert!(matches!(
            first,
            AcknowledgeHistoryRecoveryTransferOutcome::Acknowledged(_)
        ));
        let replay = outbox.acknowledge(envelope.transfer_id()?, receipt.encoded_receipt())?;
        assert!(matches!(
            replay,
            AcknowledgeHistoryRecoveryTransferOutcome::AlreadyAcknowledged(_)
        ));
        assert!(matches!(
            outbox.status(prepared.transfer_id())?,
            HistoryRecoveryTransferOutboxStatus::Acknowledged(_)
        ));
        Ok(())
    }

    #[test]
    fn outbox_rejects_competing_intent_and_receipt() -> Result<(), Box<dyn std::error::Error>> {
        let (state, envelope, source, prepared, receipt) = prepared_fixture()?;
        let outbox = HistoryRecoveryTransferOutbox::open(
            state.path(),
            envelope.owner_account_id(),
            source.device_id(),
        )?;
        let competing = outbox.prepare(
            &fixture()?,
            &source,
            SecretKey::generate().public(),
            SecretKey::generate().public(),
            [0x72; 32],
        );
        assert!(matches!(
            competing,
            Err(HistoryRecoveryTransferOutboxError::TransferAlreadyPrepared)
        ));
        outbox.acknowledge(prepared.transfer_id(), receipt.encoded_receipt())?;
        let recipient = recipient_identity()?;
        let alternative = VerifiedTransferSessionResponse::create(
            &prepared.request,
            &recipient,
            TransferStageDisposition::AlreadyStaged,
        )?;
        assert!(matches!(
            outbox.acknowledge(prepared.transfer_id(), &alternative.encode()?),
            Err(HistoryRecoveryTransferOutboxError::ReceiptConflict)
        ));
        Ok(())
    }

    #[test]
    fn outbox_uncommitted_transactions_leave_no_ambiguous_state()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let envelope = HistoryRecoveryTransferEnvelope::decode_and_verify(&fixture)?;
        let source = source_identity()?;
        let state = tempfile::tempdir()?;
        let outbox = HistoryRecoveryTransferOutbox::open(
            state.path(),
            envelope.owner_account_id(),
            source.device_id(),
        )?;
        let source_endpoint = SecretKey::generate().public();
        let recipient_endpoint = SecretKey::generate().public();
        assert!(matches!(
            outbox.prepare_inner(
                &fixture,
                &source,
                source_endpoint,
                recipient_endpoint,
                [0x73; 32],
                true,
            ),
            Err(HistoryRecoveryTransferOutboxError::InjectedBeforeCommit)
        ));
        assert!(matches!(
            outbox.status(envelope.transfer_id()?)?,
            HistoryRecoveryTransferOutboxStatus::Absent
        ));
        let prepared = prepared_from_outcome(outbox.prepare(
            &fixture,
            &source,
            source_endpoint,
            recipient_endpoint,
            [0x73; 32],
        )?)?;
        let response = VerifiedTransferSessionResponse::create(
            &prepared.request,
            &recipient_identity()?,
            TransferStageDisposition::Staged,
        )?;
        assert!(matches!(
            outbox.acknowledge_inner(prepared.transfer_id(), &response.encode()?, true,),
            Err(HistoryRecoveryTransferOutboxError::InjectedBeforeCommit)
        ));
        assert!(matches!(
            outbox.status(envelope.transfer_id()?)?,
            HistoryRecoveryTransferOutboxStatus::Pending(_)
        ));
        Ok(())
    }

    fn prepared_fixture() -> Result<PreparedFixture, Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let envelope = HistoryRecoveryTransferEnvelope::decode_and_verify(&fixture)?;
        let source = source_identity()?;
        let recipient = recipient_identity()?;
        let state = tempfile::tempdir()?;
        let outbox = HistoryRecoveryTransferOutbox::open(
            state.path(),
            envelope.owner_account_id(),
            source.device_id(),
        )?;
        let prepared = prepared_from_outcome(outbox.prepare(
            &fixture,
            &source,
            SecretKey::generate().public(),
            SecretKey::generate().public(),
            [0x74; 32],
        )?)?;
        let response = VerifiedTransferSessionResponse::create(
            &prepared.request,
            &recipient,
            TransferStageDisposition::Staged,
        )?;
        let receipt = SignedHistoryRecoveryTransferSessionReceipt::decode_and_verify(
            &response.encode()?,
            &prepared,
        )?;
        drop(outbox);
        Ok((state, envelope, source, prepared, receipt))
    }

    fn prepared_from_outcome(
        outcome: PrepareHistoryRecoveryTransferOutcome,
    ) -> Result<PreparedHistoryRecoveryTransferSession, Box<dyn std::error::Error>> {
        match outcome {
            PrepareHistoryRecoveryTransferOutcome::Prepared(value)
            | PrepareHistoryRecoveryTransferOutcome::AlreadyPrepared(value) => Ok(value),
        }
    }

    fn fixture() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let compact = include_str!("../tests/fixtures/m1222-transfer-envelope.b64")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        Ok(STANDARD.decode(compact)?)
    }

    fn source_identity() -> Result<DeviceIdentity, Box<dyn std::error::Error>> {
        Ok(DeviceIdentity::from_secret_bytes(hex_secret(
            SOURCE_SECRET_HEX,
        )?))
    }

    fn recipient_identity() -> Result<DeviceIdentity, Box<dyn std::error::Error>> {
        Ok(DeviceIdentity::from_secret_bytes(hex_secret(
            RECIPIENT_SECRET_HEX,
        )?))
    }

    fn hex_secret(value: &str) -> Result<[u8; 32], Box<dyn std::error::Error>> {
        let decoded = (0..value.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&value[index..index + 2], 16))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(decoded.try_into().map_err(|_| "invalid secret length")?)
    }
}
