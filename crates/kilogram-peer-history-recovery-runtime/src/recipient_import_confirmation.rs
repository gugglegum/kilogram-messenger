//! Durable, Device-signed confirmation ledger for one exact recipient import plan.
//!
//! Planning is read-only. Committing records explicit user approval, but never
//! imports history, consumes an intent, or dispatches a mutation command.

use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use kilogram_identity::{AccountId, DeviceId, DeviceIdentity, IdentityError};
use kilogram_runtime_ipc::{
    RuntimeIpcRecipientImportArtifactId, RuntimeIpcRecipientImportExecutionCommand,
    RuntimeIpcRecipientImportExecutionConfirmationSummary,
    RuntimeIpcRecipientImportExecutionRequest,
};
use kilogram_state::{StateDirectoryLock, StateError};
use redb::{
    Database, Durability, ReadOnlyDatabase, ReadableDatabase, ReadableTable, ReadableTableMetadata,
    TableDefinition,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::RecipientImportReadAdapterDispatch;

const LEDGER_DIRECTORY: &str = "history-recovery";
const DATABASE_FILE: &str = "recipient-import-confirmations-v1.redb";
const META_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("recipient-import-confirmation-meta-v1");
const RECORD_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("recipient-import-confirmation-record-v1");
const NONCE_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("recipient-import-confirmation-nonce-v1");
const HEAD_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("recipient-import-confirmation-head-v1");

const SCHEMA_KEY: &str = "schema-version";
const OWNER_ACCOUNT_KEY: &str = "owner-account-id";
const OWNER_DEVICE_KEY: &str = "owner-device-id";
const HEAD_KEY: &str = "head";
const SCHEMA_VERSION: u8 = 1;
const RECORD_VERSION: u8 = 1;
const HEAD_VERSION: u8 = 1;
const RECORD_SIGNATURE_DOMAIN: &[u8] = b"kilogram:recipient-import-confirmation-record:v1\0";
const CONFIRMATION_ID_DOMAIN: &[u8] = b"kilogram:recipient-import-confirmation-id:v1\0";

const MAX_RECORD_BYTES: usize = 4 * 1024;
const MAX_HEAD_BYTES: usize = 128;
pub const MAX_DURABLE_RECIPIENT_IMPORT_CONFIRMATIONS: u64 = 4_096;
pub const MAX_RECIPIENT_IMPORT_CONFIRMATION_LIFETIME_SECONDS: u64 = 15 * 60;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RecipientImportConfirmationBinding {
    intent_id: [u8; 32],
    execution_plan_id: [u8; 32],
    coordinator_plan_id: [u8; 32],
    coordinator_sequence: u64,
    membership_set_digest: [u8; 32],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RecipientImportConfirmationRecordContent {
    version: u8,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    sequence: u64,
    previous_confirmation_id: Option<[u8; 32]>,
    binding: RecipientImportConfirmationBinding,
    confirmation_nonce: [u8; 32],
    issued_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SignedRecipientImportConfirmationRecordWire {
    content: RecipientImportConfirmationRecordContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SignedRecipientImportConfirmationRecord {
    content: RecipientImportConfirmationRecordContent,
    signature: Vec<u8>,
}

impl SignedRecipientImportConfirmationRecord {
    #[allow(clippy::too_many_arguments)]
    fn issue(
        owner_account_id: AccountId,
        owner: &DeviceIdentity,
        sequence: u64,
        previous_confirmation_id: Option<[u8; 32]>,
        binding: RecipientImportConfirmationBinding,
        confirmation_nonce: [u8; 32],
        issued_at_unix_seconds: u64,
        expires_at_unix_seconds: u64,
    ) -> Result<Self, RecipientImportConfirmationLedgerError> {
        let content = RecipientImportConfirmationRecordContent {
            version: RECORD_VERSION,
            owner_account_id,
            owner_device_id: owner.device_id(),
            sequence,
            previous_confirmation_id,
            binding,
            confirmation_nonce,
            issued_at_unix_seconds,
            expires_at_unix_seconds,
        };
        validate_record_content(&content)?;
        let signature = owner.sign(&record_signing_bytes(&content)?).to_vec();
        let record = Self { content, signature };
        record.verify()?;
        Ok(record)
    }

    fn decode_and_verify(
        encoded: &[u8],
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Result<Self, RecipientImportConfirmationLedgerError> {
        validate_record_length(encoded)?;
        let wire: SignedRecipientImportConfirmationRecordWire = postcard::from_bytes(encoded)?;
        let record = Self {
            content: wire.content,
            signature: wire.signature,
        };
        record.verify()?;
        if record.content.owner_account_id != owner_account_id {
            return Err(RecipientImportConfirmationLedgerError::OwnerAccountMismatch);
        }
        if record.content.owner_device_id != owner_device_id {
            return Err(RecipientImportConfirmationLedgerError::OwnerDeviceMismatch);
        }
        if record.encode()? != encoded {
            return Err(RecipientImportConfirmationLedgerError::NonCanonicalRecord);
        }
        Ok(record)
    }

    fn verify(&self) -> Result<(), RecipientImportConfirmationLedgerError> {
        validate_record_content(&self.content)?;
        self.content
            .owner_device_id
            .verify(&record_signing_bytes(&self.content)?, &self.signature)?;
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>, RecipientImportConfirmationLedgerError> {
        self.verify()?;
        let encoded = postcard::to_allocvec(&SignedRecipientImportConfirmationRecordWire {
            content: self.content.clone(),
            signature: self.signature.clone(),
        })?;
        validate_record_length(&encoded)?;
        Ok(encoded)
    }

    fn confirmation_id(&self) -> Result<[u8; 32], RecipientImportConfirmationLedgerError> {
        Ok(domain_digest(CONFIRMATION_ID_DOMAIN, &self.encode()?))
    }

    fn matches_binding(
        &self,
        binding: &RecipientImportConfirmationBinding,
        confirmation_nonce: [u8; 32],
    ) -> bool {
        self.content.binding == *binding && self.content.confirmation_nonce == confirmation_nonce
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RecipientImportConfirmationHeadWire {
    version: u8,
    sequence: u64,
    confirmation_id: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableRecipientImportConfirmation {
    confirmation_id: [u8; 32],
    record: SignedRecipientImportConfirmationRecord,
}

impl DurableRecipientImportConfirmation {
    pub fn confirmation_id(&self) -> [u8; 32] {
        self.confirmation_id
    }

    pub fn confirmation_nonce(&self) -> [u8; 32] {
        self.record.content.confirmation_nonce
    }

    pub fn sequence(&self) -> u64 {
        self.record.content.sequence
    }

    pub fn expired(&self, now_unix_seconds: u64) -> bool {
        now_unix_seconds > self.record.content.expires_at_unix_seconds
    }

    pub fn summary(
        &self,
    ) -> Result<
        RuntimeIpcRecipientImportExecutionConfirmationSummary,
        RecipientImportConfirmationLedgerError,
    > {
        summary_from_record(&self.record, self.confirmation_id)
    }

    pub fn contains_membership_snapshot(&self) -> bool {
        false
    }

    pub fn grants_import_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RecipientImportConfirmationPlanRecord {
    Existing(DurableRecipientImportConfirmation),
    Candidate(SignedRecipientImportConfirmationRecord),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientImportConfirmationPlan {
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    binding: RecipientImportConfirmationBinding,
    confirmation_nonce: [u8; 32],
    record: RecipientImportConfirmationPlanRecord,
}

impl RecipientImportConfirmationPlan {
    pub fn summary(
        &self,
    ) -> Result<
        RuntimeIpcRecipientImportExecutionConfirmationSummary,
        RecipientImportConfirmationLedgerError,
    > {
        match &self.record {
            RecipientImportConfirmationPlanRecord::Existing(durable) => durable.summary(),
            RecipientImportConfirmationPlanRecord::Candidate(record) => {
                summary_from_record(record, record.confirmation_id()?)
            }
        }
    }

    pub fn already_committed(&self) -> bool {
        matches!(
            self.record,
            RecipientImportConfirmationPlanRecord::Existing(_)
        )
    }

    pub fn persists_during_planning(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipientImportConfirmationStatus {
    Absent,
    Committed {
        summary: RuntimeIpcRecipientImportExecutionConfirmationSummary,
        expired: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipientImportConfirmationCommitOutcome {
    Committed(DurableRecipientImportConfirmation),
    AlreadyCommitted(DurableRecipientImportConfirmation),
}

pub struct RecipientImportConfirmationLedger {
    state_directory: PathBuf,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

impl RecipientImportConfirmationLedger {
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

    pub fn is_initialized(&self) -> bool {
        database_path(&self.state_directory).is_file()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn plan(
        &self,
        dispatch: &RecipientImportReadAdapterDispatch,
        request: &RuntimeIpcRecipientImportExecutionRequest,
        owner: &DeviceIdentity,
        issued_at_unix_seconds: u64,
        expires_at_unix_seconds: u64,
    ) -> Result<RecipientImportConfirmationPlan, RecipientImportConfirmationLedgerError> {
        if owner.device_id() != self.owner_device_id {
            return Err(RecipientImportConfirmationLedgerError::OwnerDeviceMismatch);
        }
        validate_lifetime(issued_at_unix_seconds, expires_at_unix_seconds)?;
        let (binding, confirmation_nonce) = exact_confirmation_binding(dispatch, request)?;
        let snapshot = load_snapshot(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        )?;
        if let Some(existing) = snapshot.by_nonce(confirmation_nonce)? {
            if !existing
                .record
                .matches_binding(&binding, confirmation_nonce)
            {
                return Err(RecipientImportConfirmationLedgerError::NonceConflict);
            }
            return Ok(RecipientImportConfirmationPlan {
                owner_account_id: self.owner_account_id,
                owner_device_id: self.owner_device_id,
                binding,
                confirmation_nonce,
                record: RecipientImportConfirmationPlanRecord::Existing(existing),
            });
        }
        let sequence = snapshot.head.as_ref().map_or(Ok(1_u64), |head| {
            head.sequence
                .checked_add(1)
                .ok_or(RecipientImportConfirmationLedgerError::SequenceExhausted)
        })?;
        let previous_confirmation_id = snapshot.head.as_ref().map(|head| head.confirmation_id);
        let candidate = SignedRecipientImportConfirmationRecord::issue(
            self.owner_account_id,
            owner,
            sequence,
            previous_confirmation_id,
            binding.clone(),
            confirmation_nonce,
            issued_at_unix_seconds,
            expires_at_unix_seconds,
        )?;
        Ok(RecipientImportConfirmationPlan {
            owner_account_id: self.owner_account_id,
            owner_device_id: self.owner_device_id,
            binding,
            confirmation_nonce,
            record: RecipientImportConfirmationPlanRecord::Candidate(candidate),
        })
    }

    pub fn commit(
        &self,
        plan: &RecipientImportConfirmationPlan,
        now_unix_seconds: u64,
    ) -> Result<RecipientImportConfirmationCommitOutcome, RecipientImportConfirmationLedgerError>
    {
        self.commit_inner(plan, now_unix_seconds, false)
    }

    fn commit_inner(
        &self,
        plan: &RecipientImportConfirmationPlan,
        now_unix_seconds: u64,
        fail_before_commit: bool,
    ) -> Result<RecipientImportConfirmationCommitOutcome, RecipientImportConfirmationLedgerError>
    {
        if plan.owner_account_id != self.owner_account_id {
            return Err(RecipientImportConfirmationLedgerError::OwnerAccountMismatch);
        }
        if plan.owner_device_id != self.owner_device_id {
            return Err(RecipientImportConfirmationLedgerError::OwnerDeviceMismatch);
        }
        let state_lock = StateDirectoryLock::acquire(&self.state_directory)?;
        let snapshot = load_snapshot(
            state_lock.root(),
            self.owner_account_id,
            self.owner_device_id,
        )?;
        if let Some(existing) = snapshot.by_nonce(plan.confirmation_nonce)? {
            if existing
                .record
                .matches_binding(&plan.binding, plan.confirmation_nonce)
            {
                return Ok(RecipientImportConfirmationCommitOutcome::AlreadyCommitted(
                    existing,
                ));
            }
            return Err(RecipientImportConfirmationLedgerError::NonceConflict);
        }

        let RecipientImportConfirmationPlanRecord::Candidate(candidate) = &plan.record else {
            return Err(RecipientImportConfirmationLedgerError::LedgerRollback);
        };
        candidate.verify()?;
        if candidate.content.owner_account_id != self.owner_account_id {
            return Err(RecipientImportConfirmationLedgerError::OwnerAccountMismatch);
        }
        if candidate.content.owner_device_id != self.owner_device_id {
            return Err(RecipientImportConfirmationLedgerError::OwnerDeviceMismatch);
        }
        if !candidate.matches_binding(&plan.binding, plan.confirmation_nonce) {
            return Err(RecipientImportConfirmationLedgerError::PlanChanged);
        }
        if now_unix_seconds < candidate.content.issued_at_unix_seconds
            || now_unix_seconds > candidate.content.expires_at_unix_seconds
        {
            return Err(RecipientImportConfirmationLedgerError::ConfirmationExpired);
        }
        let expected_sequence = snapshot.head.as_ref().map_or(Ok(1_u64), |head| {
            head.sequence
                .checked_add(1)
                .ok_or(RecipientImportConfirmationLedgerError::SequenceExhausted)
        })?;
        let expected_previous = snapshot.head.as_ref().map(|head| head.confirmation_id);
        if candidate.content.sequence != expected_sequence
            || candidate.content.previous_confirmation_id != expected_previous
        {
            return Err(RecipientImportConfirmationLedgerError::LedgerChanged);
        }
        if snapshot.records.len() as u64 >= MAX_DURABLE_RECIPIENT_IMPORT_CONFIRMATIONS {
            return Err(RecipientImportConfirmationLedgerError::Capacity);
        }

        let database_path = prepare_database_path(state_lock.root())?;
        let existed = database_path.is_file();
        let database = Database::create(&database_path).map_err(database_error)?;
        if existed {
            verify_database(&database, self.owner_account_id, self.owner_device_id)?;
        } else {
            initialize_database(&database, self.owner_account_id, self.owner_device_id)?;
        }
        let locked_snapshot =
            snapshot_from_database(&database, self.owner_account_id, self.owner_device_id)?;
        if locked_snapshot.head != snapshot.head
            || locked_snapshot.records.keys().ne(snapshot.records.keys())
        {
            return Err(RecipientImportConfirmationLedgerError::LedgerChanged);
        }

        let confirmation_id = candidate.confirmation_id()?;
        if confirmation_id == plan.confirmation_nonce {
            return Err(RecipientImportConfirmationLedgerError::InvalidRecord);
        }
        let record_bytes = candidate.encode()?;
        let head = RecipientImportConfirmationHeadWire {
            version: HEAD_VERSION,
            sequence: candidate.content.sequence,
            confirmation_id,
        };
        let head_bytes = encode_head(&head)?;
        let mut write = database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut records = write.open_table(RECORD_TABLE).map_err(database_error)?;
            let mut nonces = write.open_table(NONCE_TABLE).map_err(database_error)?;
            let mut heads = write.open_table(HEAD_TABLE).map_err(database_error)?;
            if records
                .get(confirmation_id.as_slice())
                .map_err(database_error)?
                .is_some()
                || nonces
                    .get(plan.confirmation_nonce.as_slice())
                    .map_err(database_error)?
                    .is_some()
            {
                return Err(RecipientImportConfirmationLedgerError::LedgerChanged);
            }
            let stored_head = heads
                .get(HEAD_KEY)
                .map_err(database_error)?
                .map(|value| decode_head(value.value()))
                .transpose()?;
            if stored_head != snapshot.head {
                return Err(RecipientImportConfirmationLedgerError::LedgerChanged);
            }
            records
                .insert(confirmation_id.as_slice(), record_bytes.as_slice())
                .map_err(database_error)?;
            nonces
                .insert(
                    plan.confirmation_nonce.as_slice(),
                    confirmation_id.as_slice(),
                )
                .map_err(database_error)?;
            heads
                .insert(HEAD_KEY, head_bytes.as_slice())
                .map_err(database_error)?;
        }
        if fail_before_commit {
            return Err(RecipientImportConfirmationLedgerError::InjectedBeforeCommit);
        }
        write.commit().map_err(database_error)?;

        let committed =
            snapshot_from_database(&database, self.owner_account_id, self.owner_device_id)?
                .records
                .get(&confirmation_id)
                .cloned()
                .ok_or(RecipientImportConfirmationLedgerError::MissingRecord)?;
        if committed.record != *candidate {
            return Err(RecipientImportConfirmationLedgerError::CommittedRecordMismatch);
        }
        Ok(RecipientImportConfirmationCommitOutcome::Committed(
            committed,
        ))
    }

    pub fn status_by_confirmation_id(
        &self,
        confirmation_id: [u8; 32],
        now_unix_seconds: u64,
    ) -> Result<RecipientImportConfirmationStatus, RecipientImportConfirmationLedgerError> {
        if confirmation_id == [0_u8; 32] {
            return Err(RecipientImportConfirmationLedgerError::InvalidConfirmationId);
        }
        let snapshot = load_snapshot(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        )?;
        Ok(match snapshot.records.get(&confirmation_id) {
            Some(durable) => RecipientImportConfirmationStatus::Committed {
                summary: durable.summary()?,
                expired: durable.expired(now_unix_seconds),
            },
            None => RecipientImportConfirmationStatus::Absent,
        })
    }

    #[cfg(test)]
    fn commit_with_fault_before_commit(
        &self,
        plan: &RecipientImportConfirmationPlan,
        now_unix_seconds: u64,
    ) -> Result<RecipientImportConfirmationCommitOutcome, RecipientImportConfirmationLedgerError>
    {
        self.commit_inner(plan, now_unix_seconds, true)
    }

    #[cfg(test)]
    fn corrupt_head_for_test(&self) -> Result<(), RecipientImportConfirmationLedgerError> {
        let path = database_path(&self.state_directory);
        let database = Database::create(path).map_err(database_error)?;
        let mut write = database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut heads = write.open_table(HEAD_TABLE).map_err(database_error)?;
            let encoded = heads
                .get(HEAD_KEY)
                .map_err(database_error)?
                .ok_or(RecipientImportConfirmationLedgerError::MissingHead)?;
            let mut bytes = encoded.value().to_vec();
            drop(encoded);
            let last = bytes
                .last_mut()
                .ok_or(RecipientImportConfirmationLedgerError::InvalidHead)?;
            *last ^= 1;
            heads
                .insert(HEAD_KEY, bytes.as_slice())
                .map_err(database_error)?;
        }
        write.commit().map_err(database_error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LedgerSnapshot {
    records: BTreeMap<[u8; 32], DurableRecipientImportConfirmation>,
    nonce_index: BTreeMap<[u8; 32], [u8; 32]>,
    head: Option<RecipientImportConfirmationHeadWire>,
}

impl LedgerSnapshot {
    fn empty() -> Self {
        Self {
            records: BTreeMap::new(),
            nonce_index: BTreeMap::new(),
            head: None,
        }
    }

    fn by_nonce(
        &self,
        nonce: [u8; 32],
    ) -> Result<Option<DurableRecipientImportConfirmation>, RecipientImportConfirmationLedgerError>
    {
        let Some(confirmation_id) = self.nonce_index.get(&nonce) else {
            return Ok(None);
        };
        self.records
            .get(confirmation_id)
            .cloned()
            .map(Some)
            .ok_or(RecipientImportConfirmationLedgerError::MissingRecord)
    }
}

fn exact_confirmation_binding(
    dispatch: &RecipientImportReadAdapterDispatch,
    request: &RuntimeIpcRecipientImportExecutionRequest,
) -> Result<(RecipientImportConfirmationBinding, [u8; 32]), RecipientImportConfirmationLedgerError>
{
    let planned = dispatch
        .planned_summary()
        .ok_or(RecipientImportConfirmationLedgerError::NoPlannedDispatch)?;
    let RuntimeIpcRecipientImportExecutionCommand::Confirm {
        intent_id,
        execution_plan_id,
        coordinator_plan_id,
        coordinator_sequence,
        memberships,
        confirmation_nonce,
        ..
    } = request.command()
    else {
        return Err(RecipientImportConfirmationLedgerError::NotConfirmationCommand);
    };
    if memberships != dispatch.exact_membership_references() {
        return Err(RecipientImportConfirmationLedgerError::MembershipChanged);
    }
    let request_membership_digest = request
        .command()
        .membership_set_digest()
        .map_err(contract_error)?
        .ok_or(RecipientImportConfirmationLedgerError::MembershipChanged)?;
    if intent_id != &planned.intent_id
        || execution_plan_id != &planned.execution_plan_id
        || coordinator_plan_id != &planned.coordinator_plan_id
        || *coordinator_sequence != planned.coordinator_sequence
        || request_membership_digest != planned.membership_set_digest
    {
        return Err(RecipientImportConfirmationLedgerError::PlanChanged);
    }
    let binding = RecipientImportConfirmationBinding {
        intent_id: intent_id.to_bytes().map_err(contract_error)?,
        execution_plan_id: execution_plan_id.to_bytes().map_err(contract_error)?,
        coordinator_plan_id: coordinator_plan_id.to_bytes().map_err(contract_error)?,
        coordinator_sequence: *coordinator_sequence,
        membership_set_digest: request_membership_digest
            .to_bytes()
            .map_err(contract_error)?,
    };
    Ok((
        binding,
        confirmation_nonce.to_bytes().map_err(contract_error)?,
    ))
}

fn summary_from_record(
    record: &SignedRecipientImportConfirmationRecord,
    confirmation_id: [u8; 32],
) -> Result<
    RuntimeIpcRecipientImportExecutionConfirmationSummary,
    RecipientImportConfirmationLedgerError,
> {
    let binding = &record.content.binding;
    Ok(RuntimeIpcRecipientImportExecutionConfirmationSummary {
        intent_id: artifact_id(binding.intent_id)?,
        execution_plan_id: artifact_id(binding.execution_plan_id)?,
        coordinator_plan_id: artifact_id(binding.coordinator_plan_id)?,
        coordinator_sequence: binding.coordinator_sequence,
        membership_set_digest: artifact_id(binding.membership_set_digest)?,
        confirmation_id: artifact_id(confirmation_id)?,
        confirmation_nonce: artifact_id(record.content.confirmation_nonce)?,
        expires_at_unix_seconds: record.content.expires_at_unix_seconds,
    })
}

fn artifact_id(
    bytes: [u8; 32],
) -> Result<RuntimeIpcRecipientImportArtifactId, RecipientImportConfirmationLedgerError> {
    RuntimeIpcRecipientImportArtifactId::from_bytes(bytes).map_err(contract_error)
}

fn contract_error(error: impl std::fmt::Display) -> RecipientImportConfirmationLedgerError {
    RecipientImportConfirmationLedgerError::Contract(error.to_string())
}

fn validate_lifetime(
    issued_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
) -> Result<(), RecipientImportConfirmationLedgerError> {
    let lifetime = expires_at_unix_seconds
        .checked_sub(issued_at_unix_seconds)
        .ok_or(RecipientImportConfirmationLedgerError::InvalidLifetime)?;
    if issued_at_unix_seconds == 0
        || lifetime == 0
        || lifetime > MAX_RECIPIENT_IMPORT_CONFIRMATION_LIFETIME_SECONDS
    {
        return Err(RecipientImportConfirmationLedgerError::InvalidLifetime);
    }
    Ok(())
}

fn validate_record_content(
    content: &RecipientImportConfirmationRecordContent,
) -> Result<(), RecipientImportConfirmationLedgerError> {
    validate_lifetime(
        content.issued_at_unix_seconds,
        content.expires_at_unix_seconds,
    )?;
    let binding = &content.binding;
    if content.version != RECORD_VERSION
        || content.sequence == 0
        || (content.sequence == 1) != content.previous_confirmation_id.is_none()
        || binding.intent_id == [0_u8; 32]
        || binding.execution_plan_id == [0_u8; 32]
        || binding.coordinator_plan_id == [0_u8; 32]
        || binding.membership_set_digest == [0_u8; 32]
        || content.confirmation_nonce == [0_u8; 32]
    {
        return Err(RecipientImportConfirmationLedgerError::InvalidRecord);
    }
    Ok(())
}

fn record_signing_bytes(
    content: &RecipientImportConfirmationRecordContent,
) -> Result<Vec<u8>, RecipientImportConfirmationLedgerError> {
    let encoded = postcard::to_allocvec(content)?;
    let mut signing = Vec::with_capacity(RECORD_SIGNATURE_DOMAIN.len() + encoded.len());
    signing.extend_from_slice(RECORD_SIGNATURE_DOMAIN);
    signing.extend_from_slice(&encoded);
    Ok(signing)
}

fn load_snapshot(
    state_directory: &Path,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<LedgerSnapshot, RecipientImportConfirmationLedgerError> {
    reject_ledger_path_symlinks(state_directory)?;
    let path = database_path(state_directory);
    if !path.is_file() {
        return Ok(LedgerSnapshot::empty());
    }
    let database = ReadOnlyDatabase::open(&path).map_err(database_error)?;
    snapshot_from_database(&database, owner_account_id, owner_device_id)
}

fn snapshot_from_database<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<LedgerSnapshot, RecipientImportConfirmationLedgerError> {
    verify_database_metadata(database, owner_account_id, owner_device_id)?;
    let read = database.begin_read().map_err(database_error)?;
    let records_table = read.open_table(RECORD_TABLE).map_err(database_error)?;
    if records_table.len().map_err(database_error)? > MAX_DURABLE_RECIPIENT_IMPORT_CONFIRMATIONS {
        return Err(RecipientImportConfirmationLedgerError::Capacity);
    }
    let mut records = BTreeMap::new();
    for item in records_table.iter().map_err(database_error)? {
        let (stored_id, encoded) = item.map_err(database_error)?;
        let confirmation_id = copy_id(stored_id.value())?;
        let record = SignedRecipientImportConfirmationRecord::decode_and_verify(
            encoded.value(),
            owner_account_id,
            owner_device_id,
        )?;
        if record.confirmation_id()? != confirmation_id {
            return Err(RecipientImportConfirmationLedgerError::RecordIdBinding);
        }
        if records
            .insert(
                confirmation_id,
                DurableRecipientImportConfirmation {
                    confirmation_id,
                    record,
                },
            )
            .is_some()
        {
            return Err(RecipientImportConfirmationLedgerError::LedgerFork);
        }
    }
    drop(records_table);

    let nonce_table = read.open_table(NONCE_TABLE).map_err(database_error)?;
    if nonce_table.len().map_err(database_error)? != records.len() as u64 {
        return Err(RecipientImportConfirmationLedgerError::NonceIndexMismatch);
    }
    let mut nonce_index = BTreeMap::new();
    let mut indexed_records = HashSet::new();
    for item in nonce_table.iter().map_err(database_error)? {
        let (stored_nonce, stored_id) = item.map_err(database_error)?;
        let nonce = copy_id(stored_nonce.value())?;
        let confirmation_id = copy_id(stored_id.value())?;
        let durable = records
            .get(&confirmation_id)
            .ok_or(RecipientImportConfirmationLedgerError::MissingRecord)?;
        if durable.record.content.confirmation_nonce != nonce
            || !indexed_records.insert(confirmation_id)
            || nonce_index.insert(nonce, confirmation_id).is_some()
        {
            return Err(RecipientImportConfirmationLedgerError::NonceIndexMismatch);
        }
    }
    if indexed_records.len() != records.len() {
        return Err(RecipientImportConfirmationLedgerError::NonceIndexMismatch);
    }
    drop(nonce_table);

    let heads = read.open_table(HEAD_TABLE).map_err(database_error)?;
    if heads.len().map_err(database_error)? > 1 {
        return Err(RecipientImportConfirmationLedgerError::LedgerFork);
    }
    let head = heads
        .get(HEAD_KEY)
        .map_err(database_error)?
        .map(|encoded| decode_head(encoded.value()))
        .transpose()?;
    drop(heads);
    drop(read);
    verify_chain(&records, head.as_ref())?;
    Ok(LedgerSnapshot {
        records,
        nonce_index,
        head,
    })
}

fn verify_chain(
    records: &BTreeMap<[u8; 32], DurableRecipientImportConfirmation>,
    head: Option<&RecipientImportConfirmationHeadWire>,
) -> Result<(), RecipientImportConfirmationLedgerError> {
    if records.is_empty() {
        return if head.is_none() {
            Ok(())
        } else {
            Err(RecipientImportConfirmationLedgerError::MissingRecord)
        };
    }
    let head = head.ok_or(RecipientImportConfirmationLedgerError::MissingHead)?;
    if head.version != HEAD_VERSION
        || head.sequence != records.len() as u64
        || head.confirmation_id == [0_u8; 32]
    {
        return Err(RecipientImportConfirmationLedgerError::InvalidHead);
    }
    let mut expected_id = Some(head.confirmation_id);
    let mut expected_sequence = head.sequence;
    let mut visited = HashSet::new();
    while let Some(confirmation_id) = expected_id {
        if !visited.insert(confirmation_id) {
            return Err(RecipientImportConfirmationLedgerError::LedgerFork);
        }
        let durable = records
            .get(&confirmation_id)
            .ok_or(RecipientImportConfirmationLedgerError::MissingRecord)?;
        if durable.record.content.sequence != expected_sequence {
            return Err(RecipientImportConfirmationLedgerError::LedgerRollback);
        }
        expected_id = durable.record.content.previous_confirmation_id;
        expected_sequence = expected_sequence.saturating_sub(1);
    }
    if expected_sequence != 0 || visited.len() != records.len() {
        return Err(RecipientImportConfirmationLedgerError::LedgerFork);
    }
    Ok(())
}

fn initialize_database(
    database: &Database,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), RecipientImportConfirmationLedgerError> {
    let mut write = database.begin_write().map_err(database_error)?;
    write
        .set_durability(Durability::Immediate)
        .map_err(database_error)?;
    {
        let mut meta = write.open_table(META_TABLE).map_err(database_error)?;
        meta.insert(SCHEMA_KEY, &[SCHEMA_VERSION][..])
            .map_err(database_error)?;
        meta.insert(OWNER_ACCOUNT_KEY, owner_account_id.as_bytes().as_slice())
            .map_err(database_error)?;
        meta.insert(OWNER_DEVICE_KEY, owner_device_id.as_bytes().as_slice())
            .map_err(database_error)?;
    }
    write.open_table(RECORD_TABLE).map_err(database_error)?;
    write.open_table(NONCE_TABLE).map_err(database_error)?;
    write.open_table(HEAD_TABLE).map_err(database_error)?;
    write.commit().map_err(database_error)
}

fn verify_database<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), RecipientImportConfirmationLedgerError> {
    snapshot_from_database(database, owner_account_id, owner_device_id).map(|_| ())
}

fn verify_database_metadata<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), RecipientImportConfirmationLedgerError> {
    let read = database.begin_read().map_err(database_error)?;
    let meta = read.open_table(META_TABLE).map_err(database_error)?;
    verify_meta(&meta, SCHEMA_KEY, &[SCHEMA_VERSION], || {
        RecipientImportConfirmationLedgerError::SchemaMismatch
    })?;
    verify_meta(
        &meta,
        OWNER_ACCOUNT_KEY,
        owner_account_id.as_bytes(),
        || RecipientImportConfirmationLedgerError::OwnerAccountMismatch,
    )?;
    verify_meta(&meta, OWNER_DEVICE_KEY, owner_device_id.as_bytes(), || {
        RecipientImportConfirmationLedgerError::OwnerDeviceMismatch
    })?;
    drop(meta);
    drop(read);
    Ok(())
}

fn verify_meta(
    meta: &impl ReadableTable<&'static str, &'static [u8]>,
    key: &'static str,
    expected: &[u8],
    mismatch: impl Fn() -> RecipientImportConfirmationLedgerError,
) -> Result<(), RecipientImportConfirmationLedgerError> {
    let value = meta
        .get(key)
        .map_err(database_error)?
        .ok_or_else(&mismatch)?;
    if value.value() != expected {
        return Err(mismatch());
    }
    Ok(())
}

fn encode_head(
    head: &RecipientImportConfirmationHeadWire,
) -> Result<Vec<u8>, RecipientImportConfirmationLedgerError> {
    if head.version != HEAD_VERSION || head.sequence == 0 || head.confirmation_id == [0_u8; 32] {
        return Err(RecipientImportConfirmationLedgerError::InvalidHead);
    }
    let encoded = postcard::to_allocvec(head)?;
    if encoded.is_empty() || encoded.len() > MAX_HEAD_BYTES {
        return Err(RecipientImportConfirmationLedgerError::HeadLength(
            encoded.len(),
        ));
    }
    Ok(encoded)
}

fn decode_head(
    encoded: &[u8],
) -> Result<RecipientImportConfirmationHeadWire, RecipientImportConfirmationLedgerError> {
    if encoded.is_empty() || encoded.len() > MAX_HEAD_BYTES {
        return Err(RecipientImportConfirmationLedgerError::HeadLength(
            encoded.len(),
        ));
    }
    let head: RecipientImportConfirmationHeadWire = postcard::from_bytes(encoded)?;
    if encode_head(&head)? != encoded {
        return Err(RecipientImportConfirmationLedgerError::NonCanonicalHead);
    }
    Ok(head)
}

fn validate_record_length(encoded: &[u8]) -> Result<(), RecipientImportConfirmationLedgerError> {
    if encoded.is_empty() || encoded.len() > MAX_RECORD_BYTES {
        return Err(RecipientImportConfirmationLedgerError::RecordLength(
            encoded.len(),
        ));
    }
    Ok(())
}

fn prepare_database_path(root: &Path) -> Result<PathBuf, RecipientImportConfirmationLedgerError> {
    reject_ledger_path_symlinks(root)?;
    let directory = root.join(LEDGER_DIRECTORY);
    fs::create_dir_all(&directory)?;
    reject_ledger_path_symlinks(root)?;
    let canonical_directory = fs::canonicalize(&directory)?;
    if !canonical_directory.starts_with(root) {
        return Err(RecipientImportConfirmationLedgerError::UnsafePath);
    }
    Ok(canonical_directory.join(DATABASE_FILE))
}

fn reject_ledger_path_symlinks(root: &Path) -> Result<(), RecipientImportConfirmationLedgerError> {
    for path in [root.join(LEDGER_DIRECTORY), database_path(root)] {
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(RecipientImportConfirmationLedgerError::UnsafePath);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn database_path(root: &Path) -> PathBuf {
    root.join(LEDGER_DIRECTORY).join(DATABASE_FILE)
}

fn copy_id(bytes: &[u8]) -> Result<[u8; 32], RecipientImportConfirmationLedgerError> {
    bytes
        .try_into()
        .map_err(|_| RecipientImportConfirmationLedgerError::StoredIdLength(bytes.len()))
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

fn database_error(error: impl std::fmt::Display) -> RecipientImportConfirmationLedgerError {
    RecipientImportConfirmationLedgerError::Database(error.to_string())
}

#[derive(Debug, Error)]
pub enum RecipientImportConfirmationLedgerError {
    #[error("recipient import confirmation ledger I/O failed")]
    Io(#[from] std::io::Error),
    #[error("recipient import confirmation database failed: {0}")]
    Database(String),
    #[error("recipient import confirmation contract failed: {0}")]
    Contract(String),
    #[error("recipient import confirmation ledger schema does not match")]
    SchemaMismatch,
    #[error("recipient import confirmation owner Account does not match")]
    OwnerAccountMismatch,
    #[error("recipient import confirmation owner Device does not match")]
    OwnerDeviceMismatch,
    #[error("recipient import confirmation request is not a confirmation command")]
    NotConfirmationCommand,
    #[error("recipient import confirmation has no exact read-adapter plan")]
    NoPlannedDispatch,
    #[error("recipient import confirmation plan changed")]
    PlanChanged,
    #[error("recipient import confirmation membership references changed")]
    MembershipChanged,
    #[error("recipient import confirmation nonce is already bound to another plan")]
    NonceConflict,
    #[error("recipient import confirmation lifetime is invalid")]
    InvalidLifetime,
    #[error("recipient import confirmation expired before commit")]
    ConfirmationExpired,
    #[error("recipient import confirmation record is invalid")]
    InvalidRecord,
    #[error("recipient import confirmation record is not canonical")]
    NonCanonicalRecord,
    #[error("recipient import confirmation head is invalid")]
    InvalidHead,
    #[error("recipient import confirmation head is not canonical")]
    NonCanonicalHead,
    #[error("recipient import confirmation record is {0} bytes and violates its bound")]
    RecordLength(usize),
    #[error("recipient import confirmation head is {0} bytes and violates its bound")]
    HeadLength(usize),
    #[error("recipient import confirmation record ID does not bind its bytes")]
    RecordIdBinding,
    #[error("recipient import confirmation ledger references a missing record")]
    MissingRecord,
    #[error("recipient import confirmation ledger has no required head")]
    MissingHead,
    #[error("recipient import confirmation nonce index is inconsistent")]
    NonceIndexMismatch,
    #[error("recipient import confirmation ledger fork detected")]
    LedgerFork,
    #[error("recipient import confirmation ledger rollback detected")]
    LedgerRollback,
    #[error("recipient import confirmation ledger changed after planning")]
    LedgerChanged,
    #[error("recipient import confirmation committed an unexpected record")]
    CommittedRecordMismatch,
    #[error("recipient import confirmation capacity is exhausted")]
    Capacity,
    #[error("recipient import confirmation sequence is exhausted")]
    SequenceExhausted,
    #[error("recipient import confirmation ID is invalid")]
    InvalidConfirmationId,
    #[error("recipient import confirmation ledger path is unsafe")]
    UnsafePath,
    #[error("stored recipient import confirmation ID has invalid length {0}")]
    StoredIdLength(usize),
    #[error("test-only failure injected before atomic confirmation commit")]
    InjectedBeforeCommit,
    #[error("recipient import confirmation encoding is invalid: {0}")]
    Codec(#[from] postcard::Error),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    State(#[from] StateError),
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use kilogram_runtime_ipc::{
        RuntimeIpcDescriptor, RuntimeIpcRecipientImportExecutionCommand,
        RuntimeIpcRecipientImportExecutionPlanSummary, RuntimeIpcRecipientImportExplicitApproval,
        RuntimeIpcRecipientImportMembershipReference, RuntimeIpcRequestId,
        recipient_import_confirmation_phrase,
    };
    use tempfile::TempDir;

    use super::*;

    struct Fixture {
        root: TempDir,
        owner: DeviceIdentity,
        account_id: AccountId,
        dispatch: RecipientImportReadAdapterDispatch,
        request: RuntimeIpcRecipientImportExecutionRequest,
    }

    fn artifact(byte: u8) -> RuntimeIpcRecipientImportArtifactId {
        match RuntimeIpcRecipientImportArtifactId::from_bytes([byte; 32]) {
            Ok(value) => value,
            Err(_) => unreachable!("the test artifact byte is nonzero"),
        }
    }

    fn fixture(plan_byte: u8, nonce_byte: u8) -> Result<Fixture, Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let owner = DeviceIdentity::from_secret_bytes([7_u8; 32]);
        let account_id = AccountId::from_bytes([8_u8; 32]);
        let memberships = vec![RuntimeIpcRecipientImportMembershipReference {
            source_ordinal: 0,
            authority_reference_id: artifact(10),
            membership_digest: artifact(11),
            authenticated_context_id: artifact(12),
        }];
        let digest = RuntimeIpcRecipientImportExecutionCommand::Plan {
            intent_id: artifact(1),
            coordinator_plan_id: artifact(3),
            coordinator_sequence: 9,
            max_steps: 1,
            memberships: memberships.clone(),
        }
        .membership_set_digest()?
        .ok_or("membership digest missing")?;
        let execution_plan_id = artifact(plan_byte);
        let summary = RuntimeIpcRecipientImportExecutionPlanSummary {
            intent_id: artifact(1),
            execution_plan_id: execution_plan_id.clone(),
            coordinator_plan_id: artifact(3),
            coordinator_sequence: 9,
            membership_set_digest: digest,
            completed_source_count: 0,
            total_source_count: 1,
            max_steps: 1,
            planned_step_count: 1,
            next_source_ordinal: Some(0),
            coordinator_refresh_required: false,
            confirmation_phrase: recipient_import_confirmation_phrase(&execution_plan_id),
        };
        let dispatch = RecipientImportReadAdapterDispatch::planned_for_test(
            summary.clone(),
            memberships.clone(),
        );
        let descriptor = RuntimeIpcDescriptor::issue_local(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 41_231),
            [13_u8; 32],
            account_id,
            &owner,
        )?;
        let request = RuntimeIpcRecipientImportExecutionRequest::new(
            &descriptor,
            RuntimeIpcRequestId::generate()?,
            RuntimeIpcRecipientImportExecutionCommand::Confirm {
                intent_id: summary.intent_id,
                execution_plan_id: summary.execution_plan_id,
                coordinator_plan_id: summary.coordinator_plan_id,
                coordinator_sequence: summary.coordinator_sequence,
                memberships,
                confirmation_nonce: artifact(nonce_byte),
                confirmation_phrase: summary.confirmation_phrase,
                explicit_approval: RuntimeIpcRecipientImportExplicitApproval::UserConfirmed,
            },
        )?;
        Ok(Fixture {
            root,
            owner,
            account_id,
            dispatch,
            request,
        })
    }

    fn ledger(fixture: &Fixture) -> RecipientImportConfirmationLedger {
        RecipientImportConfirmationLedger::new(
            fixture.root.path(),
            fixture.account_id,
            fixture.owner.device_id(),
        )
    }

    #[test]
    fn confirmation_planning_is_read_only_and_privacy_bounded()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture(2, 20)?;
        let ledger = ledger(&fixture);
        let plan = ledger.plan(
            &fixture.dispatch,
            &fixture.request,
            &fixture.owner,
            100,
            400,
        )?;
        assert!(!ledger.is_initialized());
        assert!(!plan.already_committed());
        assert!(!plan.persists_during_planning());
        let summary = plan.summary()?;
        assert_eq!(summary.execution_plan_id, artifact(2));
        assert_eq!(summary.confirmation_nonce, artifact(20));
        assert_eq!(summary.expires_at_unix_seconds, 400);
        Ok(())
    }

    #[test]
    fn exact_confirmation_commits_and_replays_idempotently()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture(2, 21)?;
        let ledger = ledger(&fixture);
        let plan = ledger.plan(
            &fixture.dispatch,
            &fixture.request,
            &fixture.owner,
            100,
            400,
        )?;
        let committed = match ledger.commit(&plan, 110)? {
            RecipientImportConfirmationCommitOutcome::Committed(value) => value,
            other => return Err(format!("unexpected outcome: {other:?}").into()),
        };
        assert_eq!(committed.sequence(), 1);
        assert!(!committed.contains_membership_snapshot());
        assert!(!committed.grants_import_authority());
        let replay = ledger.plan(
            &fixture.dispatch,
            &fixture.request,
            &fixture.owner,
            120,
            420,
        )?;
        assert!(replay.already_committed());
        let replayed = match ledger.commit(&replay, 130)? {
            RecipientImportConfirmationCommitOutcome::AlreadyCommitted(value) => value,
            other => return Err(format!("unexpected replay outcome: {other:?}").into()),
        };
        assert_eq!(replayed.confirmation_id(), committed.confirmation_id());
        assert!(matches!(
            ledger.status_by_confirmation_id(committed.confirmation_id(), 200)?,
            RecipientImportConfirmationStatus::Committed { expired: false, .. }
        ));
        Ok(())
    }

    #[test]
    fn reused_nonce_for_another_plan_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
        let first = fixture(2, 22)?;
        let ledger = ledger(&first);
        let plan = ledger.plan(&first.dispatch, &first.request, &first.owner, 100, 400)?;
        ledger.commit(&plan, 110)?;
        let second = fixture(4, 22)?;
        let second_ledger = RecipientImportConfirmationLedger::new(
            first.root.path(),
            first.account_id,
            first.owner.device_id(),
        );
        assert!(matches!(
            second_ledger.plan(&second.dispatch, &second.request, &first.owner, 120, 420),
            Err(RecipientImportConfirmationLedgerError::NonceConflict)
        ));
        Ok(())
    }

    #[test]
    fn concurrent_stale_plan_cannot_fork_the_chain() -> Result<(), Box<dyn std::error::Error>> {
        let first = fixture(2, 23)?;
        let ledger = ledger(&first);
        let first_plan = ledger.plan(&first.dispatch, &first.request, &first.owner, 100, 400)?;
        let second = fixture(4, 24)?;
        let second_plan = ledger.plan(&second.dispatch, &second.request, &first.owner, 100, 400)?;
        ledger.commit(&first_plan, 110)?;
        assert!(matches!(
            ledger.commit(&second_plan, 110),
            Err(RecipientImportConfirmationLedgerError::LedgerChanged)
        ));
        Ok(())
    }

    #[test]
    fn expiry_and_foreign_device_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture(2, 25)?;
        let ledger = ledger(&fixture);
        let plan = ledger.plan(
            &fixture.dispatch,
            &fixture.request,
            &fixture.owner,
            100,
            200,
        )?;
        assert!(matches!(
            ledger.commit(&plan, 201),
            Err(RecipientImportConfirmationLedgerError::ConfirmationExpired)
        ));
        let foreign = DeviceIdentity::from_secret_bytes([9_u8; 32]);
        assert!(matches!(
            ledger.plan(&fixture.dispatch, &fixture.request, &foreign, 100, 200),
            Err(RecipientImportConfirmationLedgerError::OwnerDeviceMismatch)
        ));
        Ok(())
    }

    #[test]
    fn injected_precommit_failure_is_retryable() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture(2, 26)?;
        let ledger = ledger(&fixture);
        let plan = ledger.plan(
            &fixture.dispatch,
            &fixture.request,
            &fixture.owner,
            100,
            400,
        )?;
        assert!(matches!(
            ledger.commit_with_fault_before_commit(&plan, 110),
            Err(RecipientImportConfirmationLedgerError::InjectedBeforeCommit)
        ));
        assert!(matches!(
            ledger.status_by_confirmation_id(plan.summary()?.confirmation_id.to_bytes()?, 110)?,
            RecipientImportConfirmationStatus::Absent
        ));
        assert!(matches!(
            ledger.commit(&plan, 110)?,
            RecipientImportConfirmationCommitOutcome::Committed(_)
        ));
        Ok(())
    }

    #[test]
    fn corrupt_head_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture(2, 27)?;
        let ledger = ledger(&fixture);
        let plan = ledger.plan(
            &fixture.dispatch,
            &fixture.request,
            &fixture.owner,
            100,
            400,
        )?;
        let committed = match ledger.commit(&plan, 110)? {
            RecipientImportConfirmationCommitOutcome::Committed(value) => value,
            other => return Err(format!("unexpected outcome: {other:?}").into()),
        };
        ledger.corrupt_head_for_test()?;
        assert!(
            ledger
                .status_by_confirmation_id(committed.confirmation_id(), 120)
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn changed_exact_membership_references_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture(2, 28)?;
        let ledger = ledger(&fixture);
        let descriptor = RuntimeIpcDescriptor::issue_local(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 41_232),
            [14_u8; 32],
            fixture.account_id,
            &fixture.owner,
        )?;
        let changed_memberships = vec![RuntimeIpcRecipientImportMembershipReference {
            source_ordinal: 0,
            authority_reference_id: artifact(30),
            membership_digest: artifact(31),
            authenticated_context_id: artifact(32),
        }];
        let planned = fixture.dispatch.planned_summary().ok_or("plan missing")?;
        let changed = RuntimeIpcRecipientImportExecutionRequest::new(
            &descriptor,
            RuntimeIpcRequestId::generate()?,
            RuntimeIpcRecipientImportExecutionCommand::Confirm {
                intent_id: planned.intent_id.clone(),
                execution_plan_id: planned.execution_plan_id.clone(),
                coordinator_plan_id: planned.coordinator_plan_id.clone(),
                coordinator_sequence: planned.coordinator_sequence,
                memberships: changed_memberships,
                confirmation_nonce: artifact(28),
                confirmation_phrase: planned.confirmation_phrase.clone(),
                explicit_approval: RuntimeIpcRecipientImportExplicitApproval::UserConfirmed,
            },
        )?;
        assert!(matches!(
            ledger.plan(&fixture.dispatch, &changed, &fixture.owner, 100, 400),
            Err(RecipientImportConfirmationLedgerError::MembershipChanged)
        ));
        Ok(())
    }
}
