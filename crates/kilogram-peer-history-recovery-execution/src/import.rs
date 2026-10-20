use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use kilogram_identity::{AccountId, ConversationMembershipSnapshot, DeviceId, DeviceIdentity};
use kilogram_protocol::{
    AuthorizedEvent, ConversationId, EventId, ProtocolError, SignedHistoryRecoveryCheckpoint,
};
use kilogram_state::{
    EncryptedStateVault, StateDirectoryLock, StateError, StateMirrorRepository, StateTransaction,
    VaultPrimaryWriteRepository,
};
use kilogram_store::{
    CommandEventReadOverlay, CommandLocalMessageReadOverlay, EventStore, LocalMessageStore,
    StoreError,
};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use thiserror::Error;

use crate::{
    DurableHistoryRecoveryChunk, HistoryRecoveryExecutionError, HistoryRecoveryTransferError,
    MAX_DURABLE_HISTORY_RECOVERY_EXECUTION_CHUNKS, MAX_HISTORY_RECOVERY_EXECUTION_CHUNK_EVENTS,
    OldHistoryRecoveryExecutionReadStore, RecipientHistoryTransferInbox,
};

const IMPORT_RECEIPT_VERSION: u8 = 1;
const IMPORT_RECEIPT_DIRECTORY: &str = "import-receipts";
const IMPORT_CHECKPOINT_DIRECTORY: &str = "import-checkpoints";
const IMPORT_RECEIPT_EXTENSION: &str = "history-import-receipt";
const IMPORT_RECEIPT_SIGNATURE_DOMAIN: &[u8] = b"kilogram:recipient-history-import-receipt:v1\0";
const IMPORT_RECEIPT_ID_DOMAIN: &[u8] = b"kilogram:recipient-history-import-receipt-id:v1\0";
const IMPORT_SELECTION_KEY_DOMAIN: &[u8] = b"kilogram:recipient-history-import-selection:v1\0";
const IMPORT_SELECTION_TOKEN_DOMAIN: &[u8] =
    b"kilogram:recipient-history-import-selection-token:v1\0";
const PROJECTION_DIGEST_DOMAIN: &[u8] = b"kilogram:recipient-history-import-projection:v1\0";
const MAX_IMPORT_RECEIPT_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RecipientHistoryImportReceiptContent {
    version: u8,
    recipient_account_id: AccountId,
    source_device_id: DeviceId,
    recipient_device_id: DeviceId,
    consent_record_id: [u8; 32],
    selection_index: u16,
    execution_record_id: [u8; 32],
    execution_generation: u64,
    conversation_id: ConversationId,
    bundle_id: [u8; 32],
    checkpoint_before_id: [u8; 32],
    checkpoint_after_id: [u8; 32],
    range_start: u64,
    range_end: u64,
    approved_range_end: u64,
    event_ids: Vec<EventId>,
    projection_digests: Vec<[u8; 32]>,
    previous_receipt_id: Option<[u8; 32]>,
    committed_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SignedRecipientHistoryImportReceiptWire {
    content: RecipientHistoryImportReceiptContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SignedRecipientHistoryImportReceipt {
    content: RecipientHistoryImportReceiptContent,
    signature: Vec<u8>,
}

impl SignedRecipientHistoryImportReceipt {
    fn issue(
        recipient: &DeviceIdentity,
        content: RecipientHistoryImportReceiptContent,
    ) -> Result<Self, RecipientHistoryImportError> {
        validate_receipt_content(&content)?;
        if recipient.device_id() != content.recipient_device_id {
            return Err(RecipientHistoryImportError::WrongRecipientDevice);
        }
        let signature = recipient.sign(&receipt_signing_bytes(&content)?).to_vec();
        let receipt = Self { content, signature };
        receipt.verify()?;
        Ok(receipt)
    }

    fn encode(&self) -> Result<Vec<u8>, RecipientHistoryImportError> {
        self.verify()?;
        let encoded = postcard::to_allocvec(&SignedRecipientHistoryImportReceiptWire {
            content: self.content.clone(),
            signature: self.signature.clone(),
        })?;
        validate_receipt_length(&encoded)?;
        Ok(encoded)
    }

    fn decode_and_verify(bytes: &[u8]) -> Result<Self, RecipientHistoryImportError> {
        validate_receipt_length(bytes)?;
        let wire: SignedRecipientHistoryImportReceiptWire = postcard::from_bytes(bytes)?;
        let receipt = Self {
            content: wire.content,
            signature: wire.signature,
        };
        receipt.verify()?;
        if receipt.encode()? != bytes {
            return Err(RecipientHistoryImportError::NonCanonicalReceipt);
        }
        Ok(receipt)
    }

    fn verify(&self) -> Result<(), RecipientHistoryImportError> {
        validate_receipt_content(&self.content)?;
        self.content
            .recipient_device_id
            .verify(&receipt_signing_bytes(&self.content)?, &self.signature)?;
        Ok(())
    }

    fn receipt_id(&self) -> Result<[u8; 32], RecipientHistoryImportError> {
        Ok(domain_digest(IMPORT_RECEIPT_ID_DOMAIN, &self.encode()?))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableRecipientHistoryImportReceipt {
    receipt_id: [u8; 32],
    receipt: SignedRecipientHistoryImportReceipt,
}

impl DurableRecipientHistoryImportReceipt {
    pub fn receipt_id(&self) -> [u8; 32] {
        self.receipt_id
    }

    pub fn execution_record_id(&self) -> [u8; 32] {
        self.receipt.content.execution_record_id
    }

    pub fn imported_event_count(&self) -> usize {
        self.receipt.content.event_ids.len()
    }

    pub fn generation(&self) -> u64 {
        self.receipt.content.execution_generation
    }

    pub fn is_complete(&self) -> bool {
        self.receipt.content.range_end >= self.receipt.content.approved_range_end
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipientHistoryImportOutcome {
    Imported(DurableRecipientHistoryImportReceipt),
    AlreadyImported(DurableRecipientHistoryImportReceipt),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipientHistoryImportStatus {
    Absent,
    InProgress(RecipientHistoryImportStatusView),
    Complete(RecipientHistoryImportStatusView),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientHistoryImportStatusView {
    pub selection_token: [u8; 16],
    pub imported_chunk_count: u64,
    pub imported_event_count: u64,
    pub next_range_start: u64,
    pub approved_range_end: u64,
    pub contains_message_bodies: bool,
    pub contains_plaintext: bool,
    pub grants_network_authority: bool,
    pub grants_execution_authority: bool,
    pub whole_database_rollback_supported: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RecipientHistoryImportCoordinatorHead {
    pub receipt_id: [u8; 32],
    pub imported_chunk_count: u64,
    pub imported_event_count: u64,
    pub next_range_start: u64,
    pub approved_range_end: u64,
    pub complete: bool,
}

pub struct RecipientHistoryImportStore {
    state_directory: PathBuf,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

impl RecipientHistoryImportStore {
    pub fn new(
        state_directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Self {
        Self {
            state_directory: state_directory.as_ref().to_path_buf(),
            owner_account_id,
            owner_device_id,
        }
    }

    pub fn status(
        &self,
        consent_record_id: [u8; 32],
        selection_index: usize,
    ) -> Result<RecipientHistoryImportStatus, RecipientHistoryImportError> {
        let Some(head) = self.coordinator_head(consent_record_id, selection_index)? else {
            return Ok(RecipientHistoryImportStatus::Absent);
        };
        let selection_index = u16::try_from(selection_index)
            .map_err(|_| RecipientHistoryImportError::SelectionMissing(selection_index))?;
        let view = RecipientHistoryImportStatusView {
            selection_token: selection_token(consent_record_id, selection_index),
            imported_chunk_count: head.imported_chunk_count,
            imported_event_count: head.imported_event_count,
            next_range_start: head.next_range_start,
            approved_range_end: head.approved_range_end,
            contains_message_bodies: false,
            contains_plaintext: false,
            grants_network_authority: false,
            grants_execution_authority: false,
            whole_database_rollback_supported: false,
        };
        if head.complete {
            Ok(RecipientHistoryImportStatus::Complete(view))
        } else {
            Ok(RecipientHistoryImportStatus::InProgress(view))
        }
    }

    pub(crate) fn state_directory(&self) -> &Path {
        &self.state_directory
    }

    pub(crate) fn coordinator_head(
        &self,
        consent_record_id: [u8; 32],
        selection_index: usize,
    ) -> Result<Option<RecipientHistoryImportCoordinatorHead>, RecipientHistoryImportError> {
        let selection_index = u16::try_from(selection_index)
            .map_err(|_| RecipientHistoryImportError::SelectionMissing(selection_index))?;
        ensure_no_active_state_transaction(&self.state_directory)?;
        let receipts = load_and_verify_receipts(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        )?;
        let mut matching = receipts
            .into_iter()
            .filter(|receipt| {
                receipt.receipt.content.consent_record_id == consent_record_id
                    && receipt.receipt.content.selection_index == selection_index
            })
            .collect::<Vec<_>>();
        ensure_no_active_state_transaction(&self.state_directory)?;
        if matching.is_empty() {
            return Ok(None);
        }
        matching.sort_by_key(DurableRecipientHistoryImportReceipt::generation);
        let last = matching
            .last()
            .ok_or(RecipientHistoryImportError::InvalidReceipt)?;
        let imported_event_count = matching.iter().try_fold(0_u64, |total, receipt| {
            total
                .checked_add(
                    u64::try_from(receipt.imported_event_count())
                        .map_err(|_| RecipientHistoryImportError::ImportCapacity)?,
                )
                .ok_or(RecipientHistoryImportError::ImportCapacity)
        })?;
        Ok(Some(RecipientHistoryImportCoordinatorHead {
            receipt_id: last.receipt_id,
            imported_chunk_count: u64::try_from(matching.len())
                .map_err(|_| RecipientHistoryImportError::ImportCapacity)?,
            imported_event_count,
            next_range_start: last.receipt.content.range_end,
            approved_range_end: last.receipt.content.approved_range_end,
            complete: last.is_complete(),
        }))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn import_committed_chunk(
        &self,
        execution: &OldHistoryRecoveryExecutionReadStore,
        consent_record_id: [u8; 32],
        selection_index: usize,
        execution_record_id: [u8; 32],
        recipient: &DeviceIdentity,
        membership: &ConversationMembershipSnapshot,
        committed_at_unix_seconds: u64,
    ) -> Result<RecipientHistoryImportOutcome, RecipientHistoryImportError> {
        self.import_chunk_inner(
            ImportChunkSource::Execution {
                execution,
                consent_record_id,
                selection_index,
                execution_record_id,
            },
            recipient,
            membership,
            committed_at_unix_seconds,
            ImportFault::None,
        )
    }

    pub fn import_staged_transfer(
        &self,
        inbox: &RecipientHistoryTransferInbox,
        transfer_id: [u8; 32],
        recipient: &DeviceIdentity,
        membership: &ConversationMembershipSnapshot,
        committed_at_unix_seconds: u64,
    ) -> Result<RecipientHistoryImportOutcome, RecipientHistoryImportError> {
        if !same_state_directory(&self.state_directory, inbox.state_directory())? {
            return Err(RecipientHistoryImportError::TransferInboxMismatch);
        }
        self.import_chunk_inner(
            ImportChunkSource::Transfer { inbox, transfer_id },
            recipient,
            membership,
            committed_at_unix_seconds,
            ImportFault::None,
        )
    }

    fn import_chunk_inner(
        &self,
        source: ImportChunkSource<'_>,
        recipient: &DeviceIdentity,
        membership: &ConversationMembershipSnapshot,
        committed_at_unix_seconds: u64,
        fault: ImportFault,
    ) -> Result<RecipientHistoryImportOutcome, RecipientHistoryImportError> {
        if recipient.device_id() != self.owner_device_id {
            return Err(RecipientHistoryImportError::WrongRecipientDevice);
        }
        let _lock = StateDirectoryLock::acquire(&self.state_directory)?;
        let vault = recover_vault_before_import(&self.state_directory)?;
        let receipts = load_and_verify_receipts(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        )?;
        let chunk = source.load_candidate()?;
        let consent_record_id = source.consent_record_id(&chunk);
        let selection_index = source.selection_index(&chunk);
        let execution_record_id = chunk.record_id();
        let selection_index_u16 = u16::try_from(selection_index)
            .map_err(|_| RecipientHistoryImportError::SelectionMissing(selection_index))?;
        let artifacts = validate_chunk_context(
            &chunk,
            self.owner_account_id,
            self.owner_device_id,
            consent_record_id,
            selection_index,
            membership,
        )?;

        if let Some(receipt) = receipts
            .iter()
            .find(|receipt| receipt.execution_record_id() == execution_record_id)
        {
            verify_receipt_matches_chunk(receipt, &chunk, &artifacts)?;
            verify_imported_artifacts(
                &self.state_directory,
                membership,
                &chunk,
                &artifacts,
                receipt,
            )?;
            return Ok(RecipientHistoryImportOutcome::AlreadyImported(
                receipt.clone(),
            ));
        }

        let validated_chunk = source.load_import_candidate(&chunk)?;
        if validated_chunk != chunk {
            return Err(RecipientHistoryImportError::StagedTransferMismatch);
        }
        let chunk = validated_chunk;
        let previous =
            expected_previous_receipt(&receipts, consent_record_id, selection_index_u16, &chunk)?;
        let receipt = build_receipt(
            recipient,
            self.owner_account_id,
            consent_record_id,
            selection_index_u16,
            &chunk,
            &artifacts,
            previous.as_ref(),
            committed_at_unix_seconds,
        )?;
        let durable = DurableRecipientHistoryImportReceipt {
            receipt_id: receipt.receipt_id()?,
            receipt,
        };

        if let Some(vault) = vault.as_ref() {
            vault.begin_dual_write()?;
        }
        let mut transaction = match StateTransaction::begin(&self.state_directory) {
            Ok(transaction) => transaction,
            Err(error) => {
                finish_vault_after_import(vault.as_ref())?;
                return Err(error.into());
            }
        };
        let operation = (|| {
            reject_store_root_symlinks(&self.state_directory)?;
            let event_store = EventStore::open(self.state_directory.join("events"))?;
            let projection_store =
                LocalMessageStore::open(self.state_directory.join("local-messages"))?;

            let event_overlay = CommandEventReadOverlay::new(Box::new(EventStore::open(
                self.state_directory.join("events"),
            )?));
            event_overlay.stage_committed(&artifacts.events, membership)?;
            let projection_overlay = CommandLocalMessageReadOverlay::new(Box::new(
                LocalMessageStore::open(self.state_directory.join("local-messages"))?,
            ));
            projection_overlay.stage_committed(chunk.projections())?;

            let event_receipt =
                event_store.put_authorized_batch_with_receipt(&artifacts.events, membership)?;
            register_store_paths(&mut transaction, event_receipt.paths())?;
            inject_fault(fault, ImportFault::AfterEvents)?;

            for projection in chunk.projections() {
                let (_, projection_receipt) = projection_store.put_with_receipt(projection)?;
                register_store_paths(&mut transaction, projection_receipt.paths())?;
            }
            inject_fault(fault, ImportFault::AfterProjections)?;

            for checkpoint in [chunk.checkpoint_before(), chunk.checkpoint_after()] {
                let checkpoint_path = persist_checkpoint(&self.state_directory, checkpoint)?;
                transaction.register_append_only_receipt_path(checkpoint_path)?;
            }
            inject_fault(fault, ImportFault::AfterCheckpoints)?;

            let receipt_path = persist_import_receipt(&self.state_directory, &durable)?;
            transaction.register_append_only_receipt_path(receipt_path)?;
            inject_fault(fault, ImportFault::AfterReceipt)?;
            Ok::<(), RecipientHistoryImportError>(())
        })();

        if operation.is_ok() && fault == ImportFault::CrashAfterReceipt {
            std::mem::forget(transaction);
            return Err(RecipientHistoryImportError::InjectedFailure(fault.label()));
        }

        match operation {
            Ok(()) => {
                if let Some(vault) = vault.as_ref()
                    && let Err(error) = vault.commit_primary_transaction(&transaction)
                {
                    let rollback = transaction.rollback();
                    let finish = finish_vault_after_import(Some(vault));
                    return combined_failure(error.into(), rollback, finish);
                }
                transaction.commit()?;
                if let Some(vault) = vault.as_ref() {
                    vault.confirm_primary_shadow()?;
                }
                finish_vault_after_import(vault.as_ref())?;
                Ok(RecipientHistoryImportOutcome::Imported(durable))
            }
            Err(error) => {
                let rollback = transaction.rollback();
                let finish = finish_vault_after_import(vault.as_ref());
                combined_failure(error, rollback, finish)
            }
        }
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn import_committed_chunk_with_fault(
        &self,
        execution: &OldHistoryRecoveryExecutionReadStore,
        consent_record_id: [u8; 32],
        selection_index: usize,
        execution_record_id: [u8; 32],
        recipient: &DeviceIdentity,
        membership: &ConversationMembershipSnapshot,
        committed_at_unix_seconds: u64,
        fault: ImportFault,
    ) -> Result<RecipientHistoryImportOutcome, RecipientHistoryImportError> {
        self.import_chunk_inner(
            ImportChunkSource::Execution {
                execution,
                consent_record_id,
                selection_index,
                execution_record_id,
            },
            recipient,
            membership,
            committed_at_unix_seconds,
            fault,
        )
    }
}

fn same_state_directory(left: &Path, right: &Path) -> Result<bool, RecipientHistoryImportError> {
    if left == right {
        return Ok(true);
    }
    let canonical_left =
        fs::canonicalize(left).map_err(|source| RecipientHistoryImportError::Io {
            path: left.to_path_buf(),
            source,
        })?;
    let canonical_right =
        fs::canonicalize(right).map_err(|source| RecipientHistoryImportError::Io {
            path: right.to_path_buf(),
            source,
        })?;
    Ok(canonical_left == canonical_right)
}

enum ImportChunkSource<'a> {
    Execution {
        execution: &'a OldHistoryRecoveryExecutionReadStore,
        consent_record_id: [u8; 32],
        selection_index: usize,
        execution_record_id: [u8; 32],
    },
    Transfer {
        inbox: &'a RecipientHistoryTransferInbox,
        transfer_id: [u8; 32],
    },
}

impl ImportChunkSource<'_> {
    fn load_candidate(&self) -> Result<DurableHistoryRecoveryChunk, RecipientHistoryImportError> {
        match self {
            Self::Execution {
                execution,
                execution_record_id,
                ..
            } => Ok(execution.committed_chunk(*execution_record_id)?),
            Self::Transfer { inbox, transfer_id } => {
                Ok(inbox.load_staged_locked(*transfer_id)?.chunk().clone())
            }
        }
    }

    fn load_import_candidate(
        &self,
        candidate: &DurableHistoryRecoveryChunk,
    ) -> Result<DurableHistoryRecoveryChunk, RecipientHistoryImportError> {
        match self {
            Self::Execution {
                execution,
                consent_record_id,
                selection_index,
                execution_record_id,
            } => Ok(execution.committed_head_chunk(
                *consent_record_id,
                *selection_index,
                *execution_record_id,
            )?),
            Self::Transfer { inbox, transfer_id } => {
                let current = inbox.load_staged_locked(*transfer_id)?;
                if current.execution_record_id() != candidate.record_id() {
                    return Err(RecipientHistoryImportError::StagedTransferMismatch);
                }
                Ok(current.chunk().clone())
            }
        }
    }

    fn consent_record_id(&self, chunk: &DurableHistoryRecoveryChunk) -> [u8; 32] {
        match self {
            Self::Execution {
                consent_record_id, ..
            } => *consent_record_id,
            Self::Transfer { .. } => chunk.consent_record_id(),
        }
    }

    fn selection_index(&self, chunk: &DurableHistoryRecoveryChunk) -> usize {
        match self {
            Self::Execution {
                selection_index, ..
            } => *selection_index,
            Self::Transfer { .. } => chunk.selection_index(),
        }
    }
}

struct ChunkArtifacts {
    events: Vec<AuthorizedEvent>,
    event_ids: Vec<EventId>,
    projection_digests: Vec<[u8; 32]>,
}

fn validate_chunk_context(
    chunk: &DurableHistoryRecoveryChunk,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    consent_record_id: [u8; 32],
    selection_index: usize,
    membership: &ConversationMembershipSnapshot,
) -> Result<ChunkArtifacts, RecipientHistoryImportError> {
    membership.verify()?;
    if chunk.owner_account_id() != owner_account_id {
        return Err(RecipientHistoryImportError::OwnerAccountMismatch);
    }
    if chunk.recipient_device_id() != owner_device_id {
        return Err(RecipientHistoryImportError::WrongRecipientDevice);
    }
    if chunk.consent_record_id() != consent_record_id || chunk.selection_index() != selection_index
    {
        return Err(RecipientHistoryImportError::ExecutionSelectionMismatch);
    }
    if chunk.conversation_id().scope_id() != membership.conversation_id() {
        return Err(RecipientHistoryImportError::MembershipMismatch);
    }
    if chunk.bundle().entries().len() != chunk.projections().len()
        || chunk.bundle().entries().is_empty()
        || chunk.bundle().entries().len() > MAX_HISTORY_RECOVERY_EXECUTION_CHUNK_EVENTS
    {
        return Err(RecipientHistoryImportError::ChunkArtifactMismatch);
    }
    let mut events = Vec::with_capacity(chunk.bundle().entries().len());
    let mut event_ids = Vec::with_capacity(chunk.bundle().entries().len());
    let mut projection_digests = Vec::with_capacity(chunk.projections().len());
    for (entry, projection) in chunk.bundle().entries().iter().zip(chunk.projections()) {
        let event = entry.event();
        event.verify_for_membership(membership)?;
        let event_id = event.event().event_id()?;
        if projection.event_id() != event_id
            || projection.local_device_id() != owner_device_id
            || projection.history_rewrap_manifest() != Some(chunk.bundle().manifest())
        {
            return Err(RecipientHistoryImportError::ChunkArtifactMismatch);
        }
        events.push(event.clone());
        event_ids.push(event_id);
        projection_digests.push(domain_digest(
            PROJECTION_DIGEST_DOMAIN,
            &projection.encode()?,
        ));
    }
    Ok(ChunkArtifacts {
        events,
        event_ids,
        projection_digests,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_receipt(
    recipient: &DeviceIdentity,
    recipient_account_id: AccountId,
    consent_record_id: [u8; 32],
    selection_index: u16,
    chunk: &DurableHistoryRecoveryChunk,
    artifacts: &ChunkArtifacts,
    previous: Option<&DurableRecipientHistoryImportReceipt>,
    committed_at_unix_seconds: u64,
) -> Result<SignedRecipientHistoryImportReceipt, RecipientHistoryImportError> {
    SignedRecipientHistoryImportReceipt::issue(
        recipient,
        RecipientHistoryImportReceiptContent {
            version: IMPORT_RECEIPT_VERSION,
            recipient_account_id,
            source_device_id: chunk.source_device_id(),
            recipient_device_id: chunk.recipient_device_id(),
            consent_record_id,
            selection_index,
            execution_record_id: chunk.record_id(),
            execution_generation: chunk.generation(),
            conversation_id: chunk.conversation_id(),
            bundle_id: *chunk.bundle().bundle_id()?.as_bytes(),
            checkpoint_before_id: chunk.checkpoint_before().checkpoint_id()?,
            checkpoint_after_id: chunk.checkpoint_after().checkpoint_id()?,
            range_start: chunk.range_start(),
            range_end: chunk.range_end(),
            approved_range_end: chunk.approved_range_end(),
            event_ids: artifacts.event_ids.clone(),
            projection_digests: artifacts.projection_digests.clone(),
            previous_receipt_id: previous.map(DurableRecipientHistoryImportReceipt::receipt_id),
            committed_at_unix_seconds,
        },
    )
}

fn expected_previous_receipt(
    receipts: &[DurableRecipientHistoryImportReceipt],
    consent_record_id: [u8; 32],
    selection_index: u16,
    chunk: &DurableHistoryRecoveryChunk,
) -> Result<Option<DurableRecipientHistoryImportReceipt>, RecipientHistoryImportError> {
    let matching = receipts
        .iter()
        .filter(|receipt| {
            receipt.receipt.content.consent_record_id == consent_record_id
                && receipt.receipt.content.selection_index == selection_index
        })
        .cloned()
        .collect::<Vec<_>>();
    match chunk.generation() {
        0 if matching.is_empty() => Ok(None),
        0 => Err(RecipientHistoryImportError::ImportHeadMismatch),
        generation => {
            let previous = matching
                .into_iter()
                .find(|receipt| receipt.generation() == generation - 1)
                .ok_or(RecipientHistoryImportError::MissingPreviousReceipt)?;
            if chunk.previous_record_id() != Some(previous.execution_record_id())
                || previous.receipt.content.checkpoint_after_id
                    != chunk.checkpoint_before().checkpoint_id()?
                || previous.receipt.content.range_end != chunk.range_start()
            {
                return Err(RecipientHistoryImportError::BrokenReceiptChain);
            }
            Ok(Some(previous))
        }
    }
}

fn verify_receipt_matches_chunk(
    receipt: &DurableRecipientHistoryImportReceipt,
    chunk: &DurableHistoryRecoveryChunk,
    artifacts: &ChunkArtifacts,
) -> Result<(), RecipientHistoryImportError> {
    let content = &receipt.receipt.content;
    if content.recipient_account_id != chunk.owner_account_id()
        || content.source_device_id != chunk.source_device_id()
        || content.recipient_device_id != chunk.recipient_device_id()
        || content.consent_record_id != chunk.consent_record_id()
        || usize::from(content.selection_index) != chunk.selection_index()
        || content.execution_record_id != chunk.record_id()
        || content.execution_generation != chunk.generation()
        || content.conversation_id != chunk.conversation_id()
        || content.bundle_id != *chunk.bundle().bundle_id()?.as_bytes()
        || content.checkpoint_before_id != chunk.checkpoint_before().checkpoint_id()?
        || content.checkpoint_after_id != chunk.checkpoint_after().checkpoint_id()?
        || content.range_start != chunk.range_start()
        || content.range_end != chunk.range_end()
        || content.approved_range_end != chunk.approved_range_end()
        || content.event_ids != artifacts.event_ids
        || content.projection_digests != artifacts.projection_digests
    {
        return Err(RecipientHistoryImportError::ReceiptChunkMismatch);
    }
    Ok(())
}

fn verify_imported_artifacts(
    state_directory: &Path,
    membership: &ConversationMembershipSnapshot,
    chunk: &DurableHistoryRecoveryChunk,
    artifacts: &ChunkArtifacts,
    receipt: &DurableRecipientHistoryImportReceipt,
) -> Result<(), RecipientHistoryImportError> {
    reject_store_root_symlinks(state_directory)?;
    let event_root = state_directory.join("events");
    let projection_root = state_directory.join("local-messages");
    if !event_root.is_dir() || !projection_root.is_dir() {
        return Err(RecipientHistoryImportError::ImportedArtifactMismatch);
    }
    let event_store = EventStore::open(event_root)?;
    let stored = event_store.authorized_events_by_id(
        chunk.conversation_id(),
        &artifacts.event_ids,
        membership,
    )?;
    if stored != artifacts.events {
        return Err(RecipientHistoryImportError::ImportedArtifactMismatch);
    }
    let projection_store = LocalMessageStore::open(projection_root)?;
    for projection in chunk.projections() {
        if projection_store.get(projection.event_id())? != *projection {
            return Err(RecipientHistoryImportError::ImportedArtifactMismatch);
        }
    }
    for checkpoint in [chunk.checkpoint_before(), chunk.checkpoint_after()] {
        verify_checkpoint_file(state_directory, checkpoint)?;
    }
    let path = import_receipt_path(state_directory, receipt.receipt_id);
    let encoded = fs::read(&path).map_err(|source| RecipientHistoryImportError::Io {
        path: path.clone(),
        source,
    })?;
    if encoded != receipt.receipt.encode()? {
        return Err(RecipientHistoryImportError::ImportedArtifactMismatch);
    }
    Ok(())
}

fn load_and_verify_receipts(
    state_directory: &Path,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<Vec<DurableRecipientHistoryImportReceipt>, RecipientHistoryImportError> {
    let directory = import_receipt_directory(state_directory);
    reject_symlink_if_exists(&directory)?;
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(RecipientHistoryImportError::Io {
                path: directory,
                source,
            });
        }
    };
    reject_symlink(&directory)?;
    let mut receipts = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| RecipientHistoryImportError::Io {
            path: directory.clone(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| RecipientHistoryImportError::Io {
                path: path.clone(),
                source,
            })?;
        if !file_type.is_file()
            || path.extension().and_then(|value| value.to_str()) != Some(IMPORT_RECEIPT_EXTENSION)
        {
            return Err(RecipientHistoryImportError::UnexpectedReceiptEntry(path));
        }
        if receipts.len()
            >= usize::try_from(MAX_DURABLE_HISTORY_RECOVERY_EXECUTION_CHUNKS).unwrap_or(usize::MAX)
        {
            return Err(RecipientHistoryImportError::ImportCapacity);
        }
        let encoded = fs::read(&path).map_err(|source| RecipientHistoryImportError::Io {
            path: path.clone(),
            source,
        })?;
        let receipt = SignedRecipientHistoryImportReceipt::decode_and_verify(&encoded)?;
        if receipt.content.recipient_account_id != owner_account_id {
            return Err(RecipientHistoryImportError::OwnerAccountMismatch);
        }
        if receipt.content.recipient_device_id != owner_device_id {
            return Err(RecipientHistoryImportError::WrongRecipientDevice);
        }
        let receipt_id = receipt.receipt_id()?;
        let expected_name = receipt_file_name(receipt_id);
        if entry.file_name() != std::ffi::OsStr::new(&expected_name) {
            return Err(RecipientHistoryImportError::ReceiptFileNameMismatch(path));
        }
        receipts.push(DurableRecipientHistoryImportReceipt {
            receipt_id,
            receipt,
        });
    }
    verify_receipt_chains(&receipts)?;
    Ok(receipts)
}

fn verify_receipt_chains(
    receipts: &[DurableRecipientHistoryImportReceipt],
) -> Result<(), RecipientHistoryImportError> {
    let mut ids = HashSet::new();
    let mut execution_ids = HashSet::new();
    let mut grouped: HashMap<[u8; 32], Vec<&DurableRecipientHistoryImportReceipt>> = HashMap::new();
    for receipt in receipts {
        if !ids.insert(receipt.receipt_id)
            || !execution_ids.insert(receipt.receipt.content.execution_record_id)
        {
            return Err(RecipientHistoryImportError::DuplicateReceipt);
        }
        let key = import_selection_key(
            receipt.receipt.content.consent_record_id,
            receipt.receipt.content.selection_index,
        );
        grouped.entry(key).or_default().push(receipt);
    }
    for chain in grouped.values_mut() {
        chain.sort_by_key(|receipt| receipt.generation());
        let mut previous: Option<&DurableRecipientHistoryImportReceipt> = None;
        for receipt in chain.iter().copied() {
            let content = &receipt.receipt.content;
            match previous {
                None => {
                    if content.execution_generation != 0 || content.previous_receipt_id.is_some() {
                        return Err(RecipientHistoryImportError::BrokenReceiptChain);
                    }
                }
                Some(previous) => {
                    let prior = &previous.receipt.content;
                    if content.execution_generation != prior.execution_generation.saturating_add(1)
                        || content.previous_receipt_id != Some(previous.receipt_id)
                        || content.recipient_account_id != prior.recipient_account_id
                        || content.source_device_id != prior.source_device_id
                        || content.recipient_device_id != prior.recipient_device_id
                        || content.consent_record_id != prior.consent_record_id
                        || content.selection_index != prior.selection_index
                        || content.conversation_id != prior.conversation_id
                        || content.checkpoint_before_id != prior.checkpoint_after_id
                        || content.range_start != prior.range_end
                        || content.approved_range_end != prior.approved_range_end
                    {
                        return Err(RecipientHistoryImportError::BrokenReceiptChain);
                    }
                }
            }
            previous = Some(receipt);
        }
    }
    Ok(())
}

fn validate_receipt_content(
    content: &RecipientHistoryImportReceiptContent,
) -> Result<(), RecipientHistoryImportError> {
    if content.version != IMPORT_RECEIPT_VERSION
        || content.source_device_id == content.recipient_device_id
        || content.consent_record_id == [0_u8; 32]
        || content.execution_record_id == [0_u8; 32]
        || content.bundle_id == [0_u8; 32]
        || content.checkpoint_before_id == [0_u8; 32]
        || content.checkpoint_after_id == [0_u8; 32]
        || content.range_start >= content.range_end
        || content.range_end > content.approved_range_end
        || content.event_ids.is_empty()
        || content.event_ids.len() > MAX_HISTORY_RECOVERY_EXECUTION_CHUNK_EVENTS
        || content.event_ids.len() != content.projection_digests.len()
        || u64::try_from(content.event_ids.len()).ok()
            != Some(content.range_end.saturating_sub(content.range_start))
        || content.event_ids.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(RecipientHistoryImportError::InvalidReceipt);
    }
    Ok(())
}

fn receipt_signing_bytes(
    content: &RecipientHistoryImportReceiptContent,
) -> Result<Vec<u8>, RecipientHistoryImportError> {
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(IMPORT_RECEIPT_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(IMPORT_RECEIPT_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn persist_checkpoint(
    state_directory: &Path,
    checkpoint: &SignedHistoryRecoveryCheckpoint,
) -> Result<PathBuf, RecipientHistoryImportError> {
    let path = checkpoint_path(state_directory, checkpoint)?;
    persist_exact(&path, &checkpoint.encode()?)?;
    Ok(path)
}

fn verify_checkpoint_file(
    state_directory: &Path,
    checkpoint: &SignedHistoryRecoveryCheckpoint,
) -> Result<(), RecipientHistoryImportError> {
    let path = checkpoint_path(state_directory, checkpoint)?;
    let encoded = fs::read(&path).map_err(|source| RecipientHistoryImportError::Io {
        path: path.clone(),
        source,
    })?;
    if encoded != checkpoint.encode()? {
        return Err(RecipientHistoryImportError::CheckpointConflict(path));
    }
    let decoded = SignedHistoryRecoveryCheckpoint::decode_and_verify(&encoded)?;
    if decoded != *checkpoint {
        return Err(RecipientHistoryImportError::CheckpointConflict(path));
    }
    Ok(())
}

fn checkpoint_path(
    state_directory: &Path,
    checkpoint: &SignedHistoryRecoveryCheckpoint,
) -> Result<PathBuf, RecipientHistoryImportError> {
    let checkpoint_id = checkpoint.checkpoint_id()?;
    Ok(state_directory
        .join("history-recovery")
        .join(IMPORT_CHECKPOINT_DIRECTORY)
        .join(format!("{}.checkpoint", hex_id(checkpoint_id))))
}

fn persist_import_receipt(
    state_directory: &Path,
    receipt: &DurableRecipientHistoryImportReceipt,
) -> Result<PathBuf, RecipientHistoryImportError> {
    let path = import_receipt_path(state_directory, receipt.receipt_id);
    persist_exact(&path, &receipt.receipt.encode()?)?;
    Ok(path)
}

fn persist_exact(path: &Path, bytes: &[u8]) -> Result<(), RecipientHistoryImportError> {
    let parent = path
        .parent()
        .ok_or_else(|| RecipientHistoryImportError::UnsafePath(path.to_path_buf()))?;
    reject_symlink_if_exists(parent)?;
    fs::create_dir_all(parent).map_err(|source| RecipientHistoryImportError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    reject_symlink(parent)?;
    reject_symlink_if_exists(path)?;
    if path.is_file() {
        let existing = fs::read(path).map_err(|source| RecipientHistoryImportError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if existing == bytes {
            return Ok(());
        }
        return Err(RecipientHistoryImportError::AppendOnlyConflict(
            path.to_path_buf(),
        ));
    }
    let mut temporary =
        NamedTempFile::new_in(parent).map_err(|source| RecipientHistoryImportError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    temporary
        .write_all(bytes)
        .and_then(|()| temporary.flush())
        .map_err(|source| RecipientHistoryImportError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    match temporary.persist_noclobber(path) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
            let existing = fs::read(path).map_err(|source| RecipientHistoryImportError::Io {
                path: path.to_path_buf(),
                source,
            })?;
            if existing == bytes {
                Ok(())
            } else {
                Err(RecipientHistoryImportError::AppendOnlyConflict(
                    path.to_path_buf(),
                ))
            }
        }
        Err(error) => Err(RecipientHistoryImportError::Io {
            path: path.to_path_buf(),
            source: error.error,
        }),
    }
}

fn reject_store_root_symlinks(state_directory: &Path) -> Result<(), RecipientHistoryImportError> {
    for root in ["events", "local-messages", "history-recovery"] {
        reject_symlink_if_exists(&state_directory.join(root))?;
    }
    reject_symlink_if_exists(&import_receipt_directory(state_directory))?;
    Ok(())
}

fn ensure_no_active_state_transaction(
    state_directory: &Path,
) -> Result<(), RecipientHistoryImportError> {
    let active = state_directory
        .join(".kilogram-transactions")
        .join("active");
    reject_symlink_if_exists(&active)?;
    if active
        .try_exists()
        .map_err(|source| RecipientHistoryImportError::Io {
            path: active.clone(),
            source,
        })?
    {
        return Err(RecipientHistoryImportError::StateTransactionInProgress(
            active,
        ));
    }
    Ok(())
}

fn reject_symlink_if_exists(path: &Path) -> Result<(), RecipientHistoryImportError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(RecipientHistoryImportError::Symlink(path.to_path_buf()))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(RecipientHistoryImportError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn reject_symlink(path: &Path) -> Result<(), RecipientHistoryImportError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|source| RecipientHistoryImportError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    if metadata.file_type().is_symlink() {
        return Err(RecipientHistoryImportError::Symlink(path.to_path_buf()));
    }
    Ok(())
}

fn register_store_paths(
    transaction: &mut StateTransaction,
    paths: &[PathBuf],
) -> Result<(), RecipientHistoryImportError> {
    for path in paths {
        transaction.register_append_only_receipt_path(path)?;
    }
    Ok(())
}

fn recover_vault_before_import(
    state_directory: &Path,
) -> Result<Option<EncryptedStateVault>, RecipientHistoryImportError> {
    if !EncryptedStateVault::is_initialized(state_directory)? {
        return Ok(None);
    }
    let vault = EncryptedStateVault::open_existing(state_directory)?;
    vault.recover_primary_shadow()?;
    vault.recover_pending_dual_write()?;
    Ok(Some(vault))
}

fn finish_vault_after_import(
    vault: Option<&EncryptedStateVault>,
) -> Result<(), RecipientHistoryImportError> {
    if let Some(vault) = vault {
        vault.recover_primary_shadow()?;
        vault.finish_dual_write()?;
    }
    Ok(())
}

fn combined_failure<T>(
    operation: RecipientHistoryImportError,
    rollback: Result<(), StateError>,
    finish: Result<(), RecipientHistoryImportError>,
) -> Result<T, RecipientHistoryImportError> {
    match (rollback, finish) {
        (Ok(()), Ok(())) => Err(operation),
        (rollback, finish) => Err(RecipientHistoryImportError::RollbackFailure {
            operation: operation.to_string(),
            rollback: format!("state={rollback:?}; vault={finish:?}"),
        }),
    }
}

fn import_receipt_directory(state_directory: &Path) -> PathBuf {
    state_directory
        .join("history-recovery")
        .join(IMPORT_RECEIPT_DIRECTORY)
}

fn import_receipt_path(state_directory: &Path, receipt_id: [u8; 32]) -> PathBuf {
    import_receipt_directory(state_directory).join(receipt_file_name(receipt_id))
}

fn receipt_file_name(receipt_id: [u8; 32]) -> String {
    format!("{}.{}", hex_id(receipt_id), IMPORT_RECEIPT_EXTENSION)
}

fn import_selection_key(consent_record_id: [u8; 32], selection_index: u16) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(IMPORT_SELECTION_KEY_DOMAIN);
    hasher.update(&consent_record_id);
    hasher.update(&selection_index.to_be_bytes());
    *hasher.finalize().as_bytes()
}

fn selection_token(consent_record_id: [u8; 32], selection_index: u16) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(IMPORT_SELECTION_TOKEN_DOMAIN);
    hasher.update(&consent_record_id);
    hasher.update(&selection_index.to_be_bytes());
    let digest = hasher.finalize();
    let mut token = [0_u8; 16];
    token.copy_from_slice(&digest.as_bytes()[..16]);
    token
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

fn validate_receipt_length(bytes: &[u8]) -> Result<(), RecipientHistoryImportError> {
    if bytes.is_empty() || bytes.len() > MAX_IMPORT_RECEIPT_BYTES {
        return Err(RecipientHistoryImportError::ReceiptLength(bytes.len()));
    }
    Ok(())
}

fn hex_id(id: [u8; 32]) -> String {
    id.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImportFault {
    None,
    AfterEvents,
    AfterProjections,
    AfterCheckpoints,
    AfterReceipt,
    CrashAfterReceipt,
}

fn inject_fault(
    actual: ImportFault,
    expected: ImportFault,
) -> Result<(), RecipientHistoryImportError> {
    if actual == expected {
        return Err(RecipientHistoryImportError::InjectedFailure(
            expected.label(),
        ));
    }
    Ok(())
}

impl ImportFault {
    fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::AfterEvents => "after-events",
            Self::AfterProjections => "after-projections",
            Self::AfterCheckpoints => "after-checkpoints",
            Self::AfterReceipt => "after-receipt",
            Self::CrashAfterReceipt => "crash-after-receipt",
        }
    }
}

#[derive(Debug, Error)]
pub enum RecipientHistoryImportError {
    #[error("recipient history import I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("recipient history import owner Account does not match")]
    OwnerAccountMismatch,
    #[error("recipient history import Device does not match")]
    WrongRecipientDevice,
    #[error("recipient history import selection {0} is missing")]
    SelectionMissing(usize),
    #[error("recipient history import execution selection does not match")]
    ExecutionSelectionMismatch,
    #[error("recipient history import conversation membership does not match")]
    MembershipMismatch,
    #[error("recipient history import chunk artifacts do not match")]
    ChunkArtifactMismatch,
    #[error("recipient history import receipt is invalid")]
    InvalidReceipt,
    #[error("recipient history import receipt is not canonical")]
    NonCanonicalReceipt,
    #[error("recipient history import receipt has invalid length {0}")]
    ReceiptLength(usize),
    #[error("recipient history import receipt does not match the committed chunk")]
    ReceiptChunkMismatch,
    #[error("recipient history import durable artifacts do not match the receipt")]
    ImportedArtifactMismatch,
    #[error("recipient history import head does not match the requested generation")]
    ImportHeadMismatch,
    #[error("recipient history import predecessor receipt is missing")]
    MissingPreviousReceipt,
    #[error("recipient history import receipt chain is broken")]
    BrokenReceiptChain,
    #[error("recipient history import receipt is duplicated or reuses an execution record")]
    DuplicateReceipt,
    #[error("recipient history import capacity is exhausted")]
    ImportCapacity,
    #[error("recipient history import append-only path conflicts: {0}")]
    AppendOnlyConflict(PathBuf),
    #[error("recipient history import checkpoint conflicts: {0}")]
    CheckpointConflict(PathBuf),
    #[error("recipient history import receipt filename does not bind its contents: {0}")]
    ReceiptFileNameMismatch(PathBuf),
    #[error("recipient history import directory contains an unexpected entry: {0}")]
    UnexpectedReceiptEntry(PathBuf),
    #[error("recipient history import path is unsafe: {0}")]
    UnsafePath(PathBuf),
    #[error("recipient history import refuses symlink path: {0}")]
    Symlink(PathBuf),
    #[error("recipient history import status refuses an active or crashed state transaction: {0}")]
    StateTransactionInProgress(PathBuf),
    #[error("recipient history import and transfer inbox do not share one state directory")]
    TransferInboxMismatch,
    #[error("recipient history transfer changed while its staged candidate was validated")]
    StagedTransferMismatch,
    #[error("recipient history import rollback failed after '{operation}': {rollback}")]
    RollbackFailure { operation: String, rollback: String },
    #[error("test-only recipient history import failure injected at {0}")]
    InjectedFailure(&'static str),
    #[error(transparent)]
    Execution(#[from] HistoryRecoveryExecutionError),
    #[error(transparent)]
    Transfer(#[from] HistoryRecoveryTransferError),
    #[error(transparent)]
    State(#[from] StateError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error(transparent)]
    Identity(#[from] kilogram_identity::IdentityError),
    #[error(transparent)]
    Codec(#[from] postcard::Error),
}
