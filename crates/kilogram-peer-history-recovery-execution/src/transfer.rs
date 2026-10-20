use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use kilogram_identity::{AccountId, DeviceId, DeviceIdentity, IdentityError};
use kilogram_protocol::{ConversationId, ProtocolError};
use kilogram_state::{
    EncryptedStateVault, StateDirectoryLock, StateError, StateMirrorRepository, StateTransaction,
    VaultPrimaryWriteRepository,
};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use thiserror::Error;

use crate::{
    BUNDLE_DIGEST_DOMAIN, DurableHistoryRecoveryChunk, HistoryRecoveryExecutionError,
    MAX_DURABLE_HISTORY_RECOVERY_EXECUTION_CHUNKS, OldHistoryRecoveryExecutionReadStore,
    PROJECTION_DIGEST_DOMAIN, SignedExecutionRecord, decode_projection_artifact, domain_digest,
    encode_projection_artifact, validate_bundle_length, validate_checkpoint_length,
    validate_projection_length, verify_bundle_against_record,
    verify_checkpoint_transition_against_record, verify_projections_against_bundle,
};

const TRANSFER_ENVELOPE_VERSION: u8 = 1;
const TRANSFER_SIGNATURE_DOMAIN: &[u8] = b"kilogram:history-recovery-transfer:v1\0";
const TRANSFER_ID_DOMAIN: &[u8] = b"kilogram:history-recovery-transfer-id:v1\0";
const TRANSFER_SELECTION_DOMAIN: &[u8] = b"kilogram:history-recovery-transfer-selection:v1\0";
const TRANSFER_SELECTION_TOKEN_DOMAIN: &[u8] =
    b"kilogram:history-recovery-transfer-selection-token:v1\0";
const TRANSFER_INBOX_DIRECTORY: &str = "transfer-inbox";
const TRANSFER_EXTENSION: &str = "history-transfer";

pub const MAX_HISTORY_RECOVERY_TRANSFER_ENVELOPE_BYTES: usize = 9 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct HistoryRecoveryTransferArtifactsWire {
    execution_record: Vec<u8>,
    bundle: Vec<u8>,
    checkpoint_before: Vec<u8>,
    checkpoint_after: Vec<u8>,
    projections: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct HistoryRecoveryTransferContent {
    version: u8,
    owner_account_id: AccountId,
    source_device_id: DeviceId,
    recipient_device_id: DeviceId,
    consent_record_id: [u8; 32],
    selection_index: u16,
    execution_record_id: [u8; 32],
    execution_generation: u64,
    previous_execution_record_id: Option<[u8; 32]>,
    conversation_id: ConversationId,
    range_start: u64,
    range_end: u64,
    approved_range_end: u64,
    bundle_id: [u8; 32],
    checkpoint_before_id: [u8; 32],
    checkpoint_after_id: [u8; 32],
    exported_at_unix_seconds: u64,
    artifacts: HistoryRecoveryTransferArtifactsWire,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SignedHistoryRecoveryTransferWire {
    content: HistoryRecoveryTransferContent,
    signature: Vec<u8>,
}

/// A bounded source-Device-signed portable representation of one exact
/// committed M1.2.20 execution record and all artifacts it binds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryRecoveryTransferEnvelope {
    content: HistoryRecoveryTransferContent,
    signature: Vec<u8>,
    chunk: DurableHistoryRecoveryChunk,
}

impl HistoryRecoveryTransferEnvelope {
    #[allow(clippy::too_many_arguments)]
    pub fn export_current(
        execution: &OldHistoryRecoveryExecutionReadStore,
        consent_record_id: [u8; 32],
        selection_index: usize,
        execution_record_id: [u8; 32],
        source: &DeviceIdentity,
        exported_at_unix_seconds: u64,
    ) -> Result<Self, HistoryRecoveryTransferError> {
        let chunk = execution.committed_head_chunk(
            consent_record_id,
            selection_index,
            execution_record_id,
        )?;
        if chunk.source_device_id() != source.device_id() {
            return Err(HistoryRecoveryTransferError::WrongSourceDevice);
        }
        let content = transfer_content_for_chunk(&chunk, exported_at_unix_seconds)?;
        let signature = source.sign(&transfer_signing_bytes(&content)?).to_vec();
        let envelope = Self {
            content,
            signature,
            chunk,
        };
        envelope.verify()?;
        Ok(envelope)
    }

    pub fn encode(&self) -> Result<Vec<u8>, HistoryRecoveryTransferError> {
        self.verify()?;
        let encoded = postcard::to_allocvec(&SignedHistoryRecoveryTransferWire {
            content: self.content.clone(),
            signature: self.signature.clone(),
        })?;
        validate_envelope_length(&encoded)?;
        Ok(encoded)
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, HistoryRecoveryTransferError> {
        validate_envelope_length(bytes)?;
        let wire: SignedHistoryRecoveryTransferWire = postcard::from_bytes(bytes)?;
        let chunk = decode_transfer_chunk(&wire.content)?;
        let envelope = Self {
            content: wire.content,
            signature: wire.signature,
            chunk,
        };
        envelope.verify()?;
        if envelope.encode()? != bytes {
            return Err(HistoryRecoveryTransferError::NonCanonicalEnvelope);
        }
        Ok(envelope)
    }

    pub fn verify(&self) -> Result<(), HistoryRecoveryTransferError> {
        validate_transfer_content(&self.content)?;
        self.content
            .source_device_id
            .verify(&transfer_signing_bytes(&self.content)?, &self.signature)?;
        let decoded = decode_transfer_chunk(&self.content)?;
        if decoded != self.chunk {
            return Err(HistoryRecoveryTransferError::ChunkMismatch);
        }
        Ok(())
    }

    pub fn transfer_id(&self) -> Result<[u8; 32], HistoryRecoveryTransferError> {
        Ok(domain_digest(TRANSFER_ID_DOMAIN, &self.encode()?))
    }

    pub fn execution_record_id(&self) -> [u8; 32] {
        self.content.execution_record_id
    }

    pub fn execution_generation(&self) -> u64 {
        self.content.execution_generation
    }

    pub fn owner_account_id(&self) -> AccountId {
        self.content.owner_account_id
    }

    pub fn source_device_id(&self) -> DeviceId {
        self.content.source_device_id
    }

    pub fn recipient_device_id(&self) -> DeviceId {
        self.content.recipient_device_id
    }

    pub fn consent_record_id(&self) -> [u8; 32] {
        self.content.consent_record_id
    }

    pub fn selection_index(&self) -> usize {
        usize::from(self.content.selection_index)
    }

    pub fn encoded_len(&self) -> Result<usize, HistoryRecoveryTransferError> {
        Ok(self.encode()?.len())
    }

    pub(crate) fn chunk(&self) -> &DurableHistoryRecoveryChunk {
        &self.chunk
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableHistoryRecoveryTransfer {
    transfer_id: [u8; 32],
    envelope: HistoryRecoveryTransferEnvelope,
}

impl DurableHistoryRecoveryTransfer {
    pub fn transfer_id(&self) -> [u8; 32] {
        self.transfer_id
    }

    pub fn execution_record_id(&self) -> [u8; 32] {
        self.envelope.execution_record_id()
    }

    pub fn execution_generation(&self) -> u64 {
        self.envelope.execution_generation()
    }

    pub fn is_complete(&self) -> bool {
        self.envelope.chunk().is_complete()
    }

    pub(crate) fn chunk(&self) -> &DurableHistoryRecoveryChunk {
        self.envelope.chunk()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StageHistoryRecoveryTransferOutcome {
    Staged(DurableHistoryRecoveryTransfer),
    AlreadyStaged(DurableHistoryRecoveryTransfer),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipientHistoryTransferInboxStatus {
    Absent,
    InProgress(RecipientHistoryTransferInboxStatusView),
    Complete(RecipientHistoryTransferInboxStatusView),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientHistoryTransferInboxStatusView {
    pub selection_token: [u8; 16],
    pub staged_chunk_count: u64,
    pub next_range_start: u64,
    pub approved_range_end: u64,
    pub contains_message_bodies: bool,
    pub contains_plaintext: bool,
    pub grants_network_authority: bool,
    pub grants_import_authority: bool,
}

pub struct RecipientHistoryTransferInbox {
    state_directory: PathBuf,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

impl RecipientHistoryTransferInbox {
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
    ) -> Result<RecipientHistoryTransferInboxStatus, HistoryRecoveryTransferError> {
        let selection_index = u16::try_from(selection_index)
            .map_err(|_| HistoryRecoveryTransferError::SelectionMissing(selection_index))?;
        ensure_no_active_state_transaction(&self.state_directory)?;
        let transfers = self.load_and_verify_all()?;
        ensure_no_active_state_transaction(&self.state_directory)?;
        let mut matching = transfers
            .into_iter()
            .filter(|transfer| {
                transfer.envelope.content.consent_record_id == consent_record_id
                    && transfer.envelope.content.selection_index == selection_index
            })
            .collect::<Vec<_>>();
        if matching.is_empty() {
            return Ok(RecipientHistoryTransferInboxStatus::Absent);
        }
        matching.sort_by_key(DurableHistoryRecoveryTransfer::execution_generation);
        let last = matching
            .last()
            .ok_or(HistoryRecoveryTransferError::InvalidEnvelope)?;
        let view = RecipientHistoryTransferInboxStatusView {
            selection_token: selection_token(consent_record_id, selection_index),
            staged_chunk_count: u64::try_from(matching.len())
                .map_err(|_| HistoryRecoveryTransferError::InboxCapacity)?,
            next_range_start: last.envelope.content.range_end,
            approved_range_end: last.envelope.content.approved_range_end,
            contains_message_bodies: false,
            contains_plaintext: false,
            grants_network_authority: false,
            grants_import_authority: false,
        };
        if last.is_complete() {
            Ok(RecipientHistoryTransferInboxStatus::Complete(view))
        } else {
            Ok(RecipientHistoryTransferInboxStatus::InProgress(view))
        }
    }

    pub fn stage_envelope(
        &self,
        envelope: &HistoryRecoveryTransferEnvelope,
    ) -> Result<StageHistoryRecoveryTransferOutcome, HistoryRecoveryTransferError> {
        self.stage_envelope_bytes(&envelope.encode()?)
    }

    pub fn stage_envelope_bytes(
        &self,
        bytes: &[u8],
    ) -> Result<StageHistoryRecoveryTransferOutcome, HistoryRecoveryTransferError> {
        self.stage_envelope_bytes_inner(bytes, TransferInboxFault::None)
    }

    fn stage_envelope_bytes_inner(
        &self,
        bytes: &[u8],
        fault: TransferInboxFault,
    ) -> Result<StageHistoryRecoveryTransferOutcome, HistoryRecoveryTransferError> {
        let envelope = HistoryRecoveryTransferEnvelope::decode_and_verify(bytes)?;
        self.verify_recipient(&envelope)?;
        let transfer_id = envelope.transfer_id()?;
        let durable = DurableHistoryRecoveryTransfer {
            transfer_id,
            envelope,
        };

        let _lock = StateDirectoryLock::acquire(&self.state_directory)?;
        let vault = recover_vault_before_stage(&self.state_directory)?;
        let transfers = self.load_and_verify_all()?;
        if let Some(existing) = transfers
            .iter()
            .find(|existing| existing.transfer_id == transfer_id)
        {
            let path = transfer_path(&self.state_directory, transfer_id);
            let existing_bytes = read_bounded_envelope(&path)?;
            if existing_bytes != bytes || existing != &durable {
                return Err(HistoryRecoveryTransferError::StagedEnvelopeConflict(path));
            }
            return Ok(StageHistoryRecoveryTransferOutcome::AlreadyStaged(
                existing.clone(),
            ));
        }
        verify_candidate_successor(&transfers, &durable)?;

        if let Some(vault) = vault.as_ref() {
            vault.begin_dual_write()?;
        }
        let mut transaction = match StateTransaction::begin(&self.state_directory) {
            Ok(transaction) => transaction,
            Err(error) => {
                finish_vault_after_stage(vault.as_ref())?;
                return Err(error.into());
            }
        };
        let operation = (|| {
            let path = transfer_path(&self.state_directory, transfer_id);
            persist_exact(&path, bytes)?;
            transaction.register_append_only_receipt_path(path)?;
            if fault == TransferInboxFault::AfterEnvelope {
                return Err(HistoryRecoveryTransferError::InjectedFailure(fault.label()));
            }
            Ok::<(), HistoryRecoveryTransferError>(())
        })();

        if operation.is_ok() && fault == TransferInboxFault::CrashAfterEnvelope {
            std::mem::forget(transaction);
            return Err(HistoryRecoveryTransferError::InjectedFailure(fault.label()));
        }

        match operation {
            Ok(()) => {
                if let Some(vault) = vault.as_ref()
                    && let Err(error) = vault.commit_primary_transaction(&transaction)
                {
                    let rollback = transaction.rollback();
                    let finish = finish_vault_after_stage(Some(vault));
                    return combined_failure(error.into(), rollback, finish);
                }
                transaction.commit()?;
                if let Some(vault) = vault.as_ref() {
                    vault.confirm_primary_shadow()?;
                }
                finish_vault_after_stage(vault.as_ref())?;
                Ok(StageHistoryRecoveryTransferOutcome::Staged(durable))
            }
            Err(error) => {
                let rollback = transaction.rollback();
                let finish = finish_vault_after_stage(vault.as_ref());
                combined_failure(error, rollback, finish)
            }
        }
    }

    pub fn load_staged(
        &self,
        transfer_id: [u8; 32],
    ) -> Result<DurableHistoryRecoveryTransfer, HistoryRecoveryTransferError> {
        ensure_no_active_state_transaction(&self.state_directory)?;
        let transfer = self.load_staged_locked(transfer_id)?;
        ensure_no_active_state_transaction(&self.state_directory)?;
        Ok(transfer)
    }

    pub(crate) fn state_directory(&self) -> &Path {
        &self.state_directory
    }

    #[cfg(test)]
    pub(crate) fn current_head_transfer_locked(
        &self,
        transfer_id: [u8; 32],
    ) -> Result<DurableHistoryRecoveryTransfer, HistoryRecoveryTransferError> {
        let transfers = self.load_and_verify_all()?;
        let requested = transfers
            .iter()
            .find(|transfer| transfer.transfer_id == transfer_id)
            .cloned()
            .ok_or(HistoryRecoveryTransferError::TransferMissing)?;
        let head = transfers
            .iter()
            .filter(|transfer| {
                same_selection(&transfer.envelope.content, &requested.envelope.content)
            })
            .max_by_key(|transfer| transfer.execution_generation())
            .ok_or(HistoryRecoveryTransferError::TransferMissing)?;
        if head.transfer_id != transfer_id {
            return Err(HistoryRecoveryTransferError::StaleTransfer);
        }
        Ok(requested)
    }

    pub(crate) fn load_staged_locked(
        &self,
        transfer_id: [u8; 32],
    ) -> Result<DurableHistoryRecoveryTransfer, HistoryRecoveryTransferError> {
        self.load_and_verify_all()?
            .into_iter()
            .find(|transfer| transfer.transfer_id == transfer_id)
            .ok_or(HistoryRecoveryTransferError::TransferMissing)
    }

    fn verify_recipient(
        &self,
        envelope: &HistoryRecoveryTransferEnvelope,
    ) -> Result<(), HistoryRecoveryTransferError> {
        if envelope.owner_account_id() != self.owner_account_id {
            return Err(HistoryRecoveryTransferError::OwnerAccountMismatch);
        }
        if envelope.recipient_device_id() != self.owner_device_id {
            return Err(HistoryRecoveryTransferError::WrongRecipientDevice);
        }
        Ok(())
    }

    fn load_and_verify_all(
        &self,
    ) -> Result<Vec<DurableHistoryRecoveryTransfer>, HistoryRecoveryTransferError> {
        let directory = transfer_directory(&self.state_directory);
        reject_symlink_if_exists(&self.state_directory.join("history-recovery"))?;
        reject_symlink_if_exists(&directory)?;
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(io_error(directory, source)),
        };
        reject_symlink(&directory)?;
        let mut transfers = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| io_error(directory.clone(), source))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|source| io_error(path.clone(), source))?;
            if !file_type.is_file()
                || path.extension().and_then(|value| value.to_str()) != Some(TRANSFER_EXTENSION)
            {
                return Err(HistoryRecoveryTransferError::UnexpectedInboxEntry(path));
            }
            if transfers.len()
                >= usize::try_from(MAX_DURABLE_HISTORY_RECOVERY_EXECUTION_CHUNKS)
                    .unwrap_or(usize::MAX)
            {
                return Err(HistoryRecoveryTransferError::InboxCapacity);
            }
            let encoded = read_bounded_envelope(&path)?;
            let envelope = HistoryRecoveryTransferEnvelope::decode_and_verify(&encoded)?;
            self.verify_recipient(&envelope)?;
            let transfer_id = envelope.transfer_id()?;
            if entry.file_name() != std::ffi::OsStr::new(&transfer_file_name(transfer_id)) {
                return Err(HistoryRecoveryTransferError::TransferFileNameMismatch(path));
            }
            transfers.push(DurableHistoryRecoveryTransfer {
                transfer_id,
                envelope,
            });
        }
        verify_transfer_chains(&transfers)?;
        Ok(transfers)
    }

    #[cfg(test)]
    pub(crate) fn stage_envelope_bytes_with_fault(
        &self,
        bytes: &[u8],
        fault: TransferInboxFault,
    ) -> Result<StageHistoryRecoveryTransferOutcome, HistoryRecoveryTransferError> {
        self.stage_envelope_bytes_inner(bytes, fault)
    }
}

fn transfer_content_for_chunk(
    chunk: &DurableHistoryRecoveryChunk,
    exported_at_unix_seconds: u64,
) -> Result<HistoryRecoveryTransferContent, HistoryRecoveryTransferError> {
    let projections = encode_projection_artifact(chunk.projections())?;
    let content = HistoryRecoveryTransferContent {
        version: TRANSFER_ENVELOPE_VERSION,
        owner_account_id: chunk.owner_account_id(),
        source_device_id: chunk.source_device_id(),
        recipient_device_id: chunk.recipient_device_id(),
        consent_record_id: chunk.consent_record_id(),
        selection_index: u16::try_from(chunk.selection_index())
            .map_err(|_| HistoryRecoveryTransferError::InvalidEnvelope)?,
        execution_record_id: chunk.record_id(),
        execution_generation: chunk.generation(),
        previous_execution_record_id: chunk.previous_record_id(),
        conversation_id: chunk.conversation_id(),
        range_start: chunk.range_start(),
        range_end: chunk.range_end(),
        approved_range_end: chunk.approved_range_end(),
        bundle_id: *chunk.bundle().bundle_id()?.as_bytes(),
        checkpoint_before_id: chunk.checkpoint_before().checkpoint_id()?,
        checkpoint_after_id: chunk.checkpoint_after().checkpoint_id()?,
        exported_at_unix_seconds,
        artifacts: HistoryRecoveryTransferArtifactsWire {
            execution_record: chunk.record.encode()?,
            bundle: chunk.bundle().encode()?,
            checkpoint_before: chunk.checkpoint_before().encode()?,
            checkpoint_after: chunk.checkpoint_after().encode()?,
            projections,
        },
    };
    validate_transfer_content(&content)?;
    Ok(content)
}

fn decode_transfer_chunk(
    content: &HistoryRecoveryTransferContent,
) -> Result<DurableHistoryRecoveryChunk, HistoryRecoveryTransferError> {
    validate_transfer_content_shallow(content)?;
    let artifacts = &content.artifacts;
    let record = SignedExecutionRecord::decode_and_verify(
        &artifacts.execution_record,
        content.owner_account_id,
        content.source_device_id,
    )?;
    if record.record_id()? != content.execution_record_id {
        return Err(HistoryRecoveryTransferError::ChunkMismatch);
    }

    validate_bundle_length(&artifacts.bundle)?;
    if domain_digest(BUNDLE_DIGEST_DOMAIN, &artifacts.bundle) != record.content.bundle_digest {
        return Err(HistoryRecoveryTransferError::ChunkMismatch);
    }
    let bundle = kilogram_protocol::HistoryRewrapBundle::decode_and_verify(&artifacts.bundle)?;
    if bundle.bundle_id()?.as_bytes() != &record.content.bundle_id {
        return Err(HistoryRecoveryTransferError::ChunkMismatch);
    }
    verify_bundle_against_record(&bundle, &record.content)?;

    validate_checkpoint_length(&artifacts.checkpoint_before)?;
    validate_checkpoint_length(&artifacts.checkpoint_after)?;
    let checkpoint_before = kilogram_protocol::SignedHistoryRecoveryCheckpoint::decode_and_verify(
        &artifacts.checkpoint_before,
    )?;
    let checkpoint_after = kilogram_protocol::SignedHistoryRecoveryCheckpoint::decode_and_verify(
        &artifacts.checkpoint_after,
    )?;
    if checkpoint_before.checkpoint_id()? != record.content.checkpoint_before_id
        || checkpoint_after.checkpoint_id()? != record.content.checkpoint_after_id
    {
        return Err(HistoryRecoveryTransferError::ChunkMismatch);
    }
    verify_checkpoint_transition_against_record(
        &checkpoint_before,
        &checkpoint_after,
        &record.content,
    )?;

    validate_projection_length(&artifacts.projections)?;
    if domain_digest(PROJECTION_DIGEST_DOMAIN, &artifacts.projections)
        != record.content.projection_digest
    {
        return Err(HistoryRecoveryTransferError::ChunkMismatch);
    }
    let projections = decode_projection_artifact(&artifacts.projections)?;
    verify_projections_against_bundle(&projections, &bundle, record.content.recipient_device_id)?;
    if projections.len() != usize::from(record.content.projection_count) {
        return Err(HistoryRecoveryTransferError::ChunkMismatch);
    }

    let chunk = DurableHistoryRecoveryChunk {
        record_id: content.execution_record_id,
        record,
        bundle,
        checkpoint_before,
        checkpoint_after,
        projections,
    };
    validate_content_matches_chunk(content, &chunk)?;
    Ok(chunk)
}

fn validate_transfer_content(
    content: &HistoryRecoveryTransferContent,
) -> Result<(), HistoryRecoveryTransferError> {
    validate_transfer_content_shallow(content)?;
    let chunk = decode_transfer_chunk(content)?;
    validate_content_matches_chunk(content, &chunk)
}

fn validate_transfer_content_shallow(
    content: &HistoryRecoveryTransferContent,
) -> Result<(), HistoryRecoveryTransferError> {
    if content.version != TRANSFER_ENVELOPE_VERSION
        || content.source_device_id == content.recipient_device_id
        || content.consent_record_id == [0_u8; 32]
        || content.execution_record_id == [0_u8; 32]
        || content.bundle_id == [0_u8; 32]
        || content.checkpoint_before_id == [0_u8; 32]
        || content.checkpoint_after_id == [0_u8; 32]
        || content.range_start >= content.range_end
        || content.range_end > content.approved_range_end
        || content.exported_at_unix_seconds == 0
        || content.artifacts.execution_record.is_empty()
        || content.artifacts.bundle.is_empty()
        || content.artifacts.checkpoint_before.is_empty()
        || content.artifacts.checkpoint_after.is_empty()
        || content.artifacts.projections.is_empty()
    {
        return Err(HistoryRecoveryTransferError::InvalidEnvelope);
    }
    Ok(())
}

fn validate_content_matches_chunk(
    content: &HistoryRecoveryTransferContent,
    chunk: &DurableHistoryRecoveryChunk,
) -> Result<(), HistoryRecoveryTransferError> {
    if content.owner_account_id != chunk.owner_account_id()
        || content.source_device_id != chunk.source_device_id()
        || content.recipient_device_id != chunk.recipient_device_id()
        || content.consent_record_id != chunk.consent_record_id()
        || usize::from(content.selection_index) != chunk.selection_index()
        || content.execution_record_id != chunk.record_id()
        || content.execution_generation != chunk.generation()
        || content.previous_execution_record_id != chunk.previous_record_id()
        || content.conversation_id != chunk.conversation_id()
        || content.range_start != chunk.range_start()
        || content.range_end != chunk.range_end()
        || content.approved_range_end != chunk.approved_range_end()
        || content.bundle_id != *chunk.bundle().bundle_id()?.as_bytes()
        || content.checkpoint_before_id != chunk.checkpoint_before().checkpoint_id()?
        || content.checkpoint_after_id != chunk.checkpoint_after().checkpoint_id()?
    {
        return Err(HistoryRecoveryTransferError::ChunkMismatch);
    }
    Ok(())
}

fn verify_candidate_successor(
    existing: &[DurableHistoryRecoveryTransfer],
    candidate: &DurableHistoryRecoveryTransfer,
) -> Result<(), HistoryRecoveryTransferError> {
    let mut matching = existing
        .iter()
        .filter(|transfer| same_selection(&transfer.envelope.content, &candidate.envelope.content))
        .collect::<Vec<_>>();
    matching.sort_by_key(|transfer| transfer.execution_generation());
    match matching.last() {
        None => {
            if candidate.execution_generation() != 0
                || candidate
                    .envelope
                    .content
                    .previous_execution_record_id
                    .is_some()
            {
                return Err(HistoryRecoveryTransferError::MissingPreviousTransfer);
            }
        }
        Some(previous) => {
            let prior = &previous.envelope.content;
            let current = &candidate.envelope.content;
            if current.execution_generation != prior.execution_generation.saturating_add(1)
                || current.previous_execution_record_id != Some(prior.execution_record_id)
                || current.checkpoint_before_id != prior.checkpoint_after_id
                || current.range_start != prior.range_end
                || current.approved_range_end != prior.approved_range_end
                || current.owner_account_id != prior.owner_account_id
                || current.source_device_id != prior.source_device_id
                || current.recipient_device_id != prior.recipient_device_id
                || current.conversation_id != prior.conversation_id
            {
                return Err(HistoryRecoveryTransferError::StaleOrForkedTransfer);
            }
        }
    }
    Ok(())
}

fn verify_transfer_chains(
    transfers: &[DurableHistoryRecoveryTransfer],
) -> Result<(), HistoryRecoveryTransferError> {
    let mut ids = HashSet::new();
    let mut execution_ids = HashSet::new();
    let mut grouped: HashMap<[u8; 32], Vec<&DurableHistoryRecoveryTransfer>> = HashMap::new();
    for transfer in transfers {
        if !ids.insert(transfer.transfer_id)
            || !execution_ids.insert(transfer.execution_record_id())
        {
            return Err(HistoryRecoveryTransferError::DuplicateTransfer);
        }
        let key = transfer_selection_key(
            transfer.envelope.content.consent_record_id,
            transfer.envelope.content.selection_index,
        );
        grouped.entry(key).or_default().push(transfer);
    }
    for chain in grouped.values_mut() {
        chain.sort_by_key(|transfer| transfer.execution_generation());
        let mut previous: Option<&DurableHistoryRecoveryTransfer> = None;
        for transfer in chain.iter().copied() {
            match previous {
                None => {
                    if transfer.execution_generation() != 0
                        || transfer
                            .envelope
                            .content
                            .previous_execution_record_id
                            .is_some()
                    {
                        return Err(HistoryRecoveryTransferError::BrokenTransferChain);
                    }
                }
                Some(previous) => {
                    let prior = &previous.envelope.content;
                    let current = &transfer.envelope.content;
                    if current.execution_generation != prior.execution_generation.saturating_add(1)
                        || current.previous_execution_record_id != Some(prior.execution_record_id)
                        || current.checkpoint_before_id != prior.checkpoint_after_id
                        || current.range_start != prior.range_end
                        || current.approved_range_end != prior.approved_range_end
                        || current.owner_account_id != prior.owner_account_id
                        || current.source_device_id != prior.source_device_id
                        || current.recipient_device_id != prior.recipient_device_id
                        || current.conversation_id != prior.conversation_id
                    {
                        return Err(HistoryRecoveryTransferError::BrokenTransferChain);
                    }
                }
            }
            previous = Some(transfer);
        }
    }
    Ok(())
}

fn same_selection(
    left: &HistoryRecoveryTransferContent,
    right: &HistoryRecoveryTransferContent,
) -> bool {
    left.consent_record_id == right.consent_record_id
        && left.selection_index == right.selection_index
}

fn transfer_signing_bytes(
    content: &HistoryRecoveryTransferContent,
) -> Result<Vec<u8>, HistoryRecoveryTransferError> {
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(TRANSFER_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(TRANSFER_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn transfer_selection_key(consent_record_id: [u8; 32], selection_index: u16) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(TRANSFER_SELECTION_DOMAIN);
    hasher.update(&consent_record_id);
    hasher.update(&selection_index.to_be_bytes());
    *hasher.finalize().as_bytes()
}

fn selection_token(consent_record_id: [u8; 32], selection_index: u16) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(TRANSFER_SELECTION_TOKEN_DOMAIN);
    hasher.update(&consent_record_id);
    hasher.update(&selection_index.to_be_bytes());
    let digest = hasher.finalize();
    let mut token = [0_u8; 16];
    token.copy_from_slice(&digest.as_bytes()[..16]);
    token
}

fn validate_envelope_length(bytes: &[u8]) -> Result<(), HistoryRecoveryTransferError> {
    if bytes.is_empty() || bytes.len() > MAX_HISTORY_RECOVERY_TRANSFER_ENVELOPE_BYTES {
        return Err(HistoryRecoveryTransferError::EnvelopeLength(bytes.len()));
    }
    Ok(())
}

fn read_bounded_envelope(path: &Path) -> Result<Vec<u8>, HistoryRecoveryTransferError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| io_error(path, source))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(HistoryRecoveryTransferError::Symlink(path.to_path_buf()));
    }
    let length = usize::try_from(metadata.len())
        .map_err(|_| HistoryRecoveryTransferError::EnvelopeLength(usize::MAX))?;
    if length == 0 || length > MAX_HISTORY_RECOVERY_TRANSFER_ENVELOPE_BYTES {
        return Err(HistoryRecoveryTransferError::EnvelopeLength(length));
    }
    let bytes = fs::read(path).map_err(|source| io_error(path, source))?;
    validate_envelope_length(&bytes)?;
    Ok(bytes)
}

fn transfer_directory(state_directory: &Path) -> PathBuf {
    state_directory
        .join("history-recovery")
        .join(TRANSFER_INBOX_DIRECTORY)
}

fn transfer_path(state_directory: &Path, transfer_id: [u8; 32]) -> PathBuf {
    transfer_directory(state_directory).join(transfer_file_name(transfer_id))
}

fn transfer_file_name(transfer_id: [u8; 32]) -> String {
    format!("{}.{}", hex_id(transfer_id), TRANSFER_EXTENSION)
}

fn persist_exact(path: &Path, bytes: &[u8]) -> Result<(), HistoryRecoveryTransferError> {
    let parent = path
        .parent()
        .ok_or_else(|| HistoryRecoveryTransferError::UnsafePath(path.to_path_buf()))?;
    reject_symlink_if_exists(parent)?;
    fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
    reject_symlink(parent)?;
    reject_symlink_if_exists(path)?;
    if path.is_file() {
        let existing = fs::read(path).map_err(|source| io_error(path, source))?;
        if existing == bytes {
            return Ok(());
        }
        return Err(HistoryRecoveryTransferError::StagedEnvelopeConflict(
            path.to_path_buf(),
        ));
    }
    let mut temporary = NamedTempFile::new_in(parent).map_err(|source| io_error(parent, source))?;
    temporary
        .write_all(bytes)
        .and_then(|()| temporary.flush())
        .map_err(|source| io_error(path, source))?;
    match temporary.persist_noclobber(path) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
            let existing = fs::read(path).map_err(|source| io_error(path, source))?;
            if existing == bytes {
                Ok(())
            } else {
                Err(HistoryRecoveryTransferError::StagedEnvelopeConflict(
                    path.to_path_buf(),
                ))
            }
        }
        Err(error) => Err(io_error(path, error.error)),
    }
}

fn ensure_no_active_state_transaction(
    state_directory: &Path,
) -> Result<(), HistoryRecoveryTransferError> {
    let active = state_directory
        .join(".kilogram-transactions")
        .join("active");
    reject_symlink_if_exists(&active)?;
    if active
        .try_exists()
        .map_err(|source| io_error(&active, source))?
    {
        return Err(HistoryRecoveryTransferError::StateTransactionInProgress(
            active,
        ));
    }
    Ok(())
}

fn reject_symlink_if_exists(path: &Path) -> Result<(), HistoryRecoveryTransferError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(HistoryRecoveryTransferError::Symlink(path.to_path_buf()))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(io_error(path, source)),
    }
}

fn reject_symlink(path: &Path) -> Result<(), HistoryRecoveryTransferError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| io_error(path, source))?;
    if metadata.file_type().is_symlink() {
        return Err(HistoryRecoveryTransferError::Symlink(path.to_path_buf()));
    }
    Ok(())
}

fn recover_vault_before_stage(
    state_directory: &Path,
) -> Result<Option<EncryptedStateVault>, HistoryRecoveryTransferError> {
    if !EncryptedStateVault::is_initialized(state_directory)? {
        return Ok(None);
    }
    let vault = EncryptedStateVault::open_existing(state_directory)?;
    vault.recover_primary_shadow()?;
    vault.recover_pending_dual_write()?;
    Ok(Some(vault))
}

fn finish_vault_after_stage(
    vault: Option<&EncryptedStateVault>,
) -> Result<(), HistoryRecoveryTransferError> {
    if let Some(vault) = vault {
        vault.recover_primary_shadow()?;
        vault.finish_dual_write()?;
    }
    Ok(())
}

fn combined_failure<T>(
    operation: HistoryRecoveryTransferError,
    rollback: Result<(), StateError>,
    finish: Result<(), HistoryRecoveryTransferError>,
) -> Result<T, HistoryRecoveryTransferError> {
    match (rollback, finish) {
        (Ok(()), Ok(())) => Err(operation),
        (rollback, finish) => Err(HistoryRecoveryTransferError::RollbackFailure {
            operation: operation.to_string(),
            rollback: format!("state={rollback:?}; vault={finish:?}"),
        }),
    }
}

fn io_error(path: impl AsRef<Path>, source: io::Error) -> HistoryRecoveryTransferError {
    HistoryRecoveryTransferError::Io {
        path: path.as_ref().to_path_buf(),
        source,
    }
}

fn hex_id(id: [u8; 32]) -> String {
    id.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TransferInboxFault {
    None,
    AfterEnvelope,
    CrashAfterEnvelope,
}

impl TransferInboxFault {
    fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::AfterEnvelope => "after-envelope",
            Self::CrashAfterEnvelope => "crash-after-envelope",
        }
    }
}

#[derive(Debug, Error)]
pub enum HistoryRecoveryTransferError {
    #[error("history recovery transfer I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("history recovery transfer owner Account does not match")]
    OwnerAccountMismatch,
    #[error("history recovery transfer source Device does not match")]
    WrongSourceDevice,
    #[error("history recovery transfer recipient Device does not match")]
    WrongRecipientDevice,
    #[error("history recovery transfer selection {0} is missing")]
    SelectionMissing(usize),
    #[error("history recovery transfer envelope is invalid")]
    InvalidEnvelope,
    #[error("history recovery transfer envelope is not canonical")]
    NonCanonicalEnvelope,
    #[error("history recovery transfer envelope has invalid length {0}")]
    EnvelopeLength(usize),
    #[error("history recovery transfer artifacts do not match their signed execution record")]
    ChunkMismatch,
    #[error("history recovery transfer predecessor is missing")]
    MissingPreviousTransfer,
    #[error("history recovery transfer is stale or forks the staged chain")]
    StaleOrForkedTransfer,
    #[error("history recovery transfer is not the current staged head")]
    StaleTransfer,
    #[error("history recovery transfer chain is broken")]
    BrokenTransferChain,
    #[error("history recovery transfer is duplicated or reuses an execution record")]
    DuplicateTransfer,
    #[error("history recovery transfer inbox capacity is exhausted")]
    InboxCapacity,
    #[error("history recovery transfer is missing")]
    TransferMissing,
    #[error("history recovery transfer inbox path conflicts: {0}")]
    StagedEnvelopeConflict(PathBuf),
    #[error("history recovery transfer filename does not bind its contents: {0}")]
    TransferFileNameMismatch(PathBuf),
    #[error("history recovery transfer inbox contains an unexpected entry: {0}")]
    UnexpectedInboxEntry(PathBuf),
    #[error("history recovery transfer path is unsafe: {0}")]
    UnsafePath(PathBuf),
    #[error("history recovery transfer refuses symlink path: {0}")]
    Symlink(PathBuf),
    #[error("history recovery transfer status refuses an active or crashed state transaction: {0}")]
    StateTransactionInProgress(PathBuf),
    #[error("history recovery transfer rollback failed after '{operation}': {rollback}")]
    RollbackFailure { operation: String, rollback: String },
    #[error("test-only history recovery transfer failure injected at {0}")]
    InjectedFailure(&'static str),
    #[error(transparent)]
    Execution(#[from] HistoryRecoveryExecutionError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error(transparent)]
    State(#[from] StateError),
    #[error(transparent)]
    Encoding(#[from] postcard::Error),
}
