//! Dormant explicit mutation adapter for one confirmed recipient import round.
//!
//! A Device-signed, append-only nonce ledger is prepared before the first
//! import. The exact completed receipt is persisted before it is returned.
//! An interrupted operation may only move forward within its original bounded
//! step budget; it never consumes the import intent and never opens a network.

use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::Error as ContractError;
use kilogram_identity::{AccountId, DeviceId, DeviceIdentity, IdentityError};
use kilogram_runtime_ipc::{
    RuntimeIpcRecipientImportArtifactId, RuntimeIpcRecipientImportExecutionCommand,
    RuntimeIpcRecipientImportExecutionHazard, RuntimeIpcRecipientImportExecutionOutcome,
    RuntimeIpcRecipientImportExecutionReceiptSummary, RuntimeIpcRecipientImportExecutionRequest,
    RuntimeIpcRecipientImportStepDisposition, RuntimeIpcRecipientImportStepReceipt,
};
use kilogram_state::{StateDirectoryLock, StateError};
use redb::{
    Database, Durability, ReadOnlyDatabase, ReadableDatabase, ReadableTable, ReadableTableMetadata,
    TableDefinition,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    AuthenticatedGuidedRecipientImportCompletion, DurableRecipientImportConfirmation,
    RecipientImportConfirmationLedger, RecipientImportConfirmationLedgerError,
    RecipientImportMembershipLoader,
    execution_api::{
        RecipientHistoryImportStore, RecipientHistoryRecoveryCoordinator,
        RecipientHistoryRecoveryImportRoundExecutor, RecipientHistoryTransferInbox,
        RecipientRecoveryCoordinationPlan, RecipientRecoveryImportRoundExecutionBudget,
        RecipientRecoveryImportRoundExecutionReceipt, RecipientRecoveryImportRoundIntentStore,
        RecipientRecoveryImportRoundStepDisposition,
    },
    recipient_import_read_adapter::{execution_hazard, load_exact_memberships, transient_sources},
};

const LEDGER_DIRECTORY: &str = "history-recovery";
const DATABASE_FILE: &str = "recipient-import-mutations-v1.redb";
const META_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("recipient-import-mutation-meta-v1");
const RECORD_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("recipient-import-mutation-record-v1");
const NONCE_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("recipient-import-mutation-nonce-v1");
const HEAD_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("recipient-import-mutation-head-v1");

const SCHEMA_KEY: &str = "schema-version";
const OWNER_ACCOUNT_KEY: &str = "owner-account-id";
const OWNER_DEVICE_KEY: &str = "owner-device-id";
const HEAD_KEY: &str = "head";
const SCHEMA_VERSION: u8 = 1;
const RECORD_VERSION: u8 = 1;
const HEAD_VERSION: u8 = 1;
const RECORD_SIGNATURE_DOMAIN: &[u8] = b"kilogram:recipient-import-mutation-record:v1\0";
const RECORD_ID_DOMAIN: &[u8] = b"kilogram:recipient-import-mutation-record-id:v1\0";
const MAX_RECORD_BYTES: usize = 16 * 1024;
const MAX_HEAD_BYTES: usize = 128;
pub const MAX_DURABLE_RECIPIENT_IMPORT_MUTATIONS: u64 = 4_096;
pub const MAX_DURABLE_RECIPIENT_IMPORT_MUTATION_RECORDS: u64 =
    MAX_DURABLE_RECIPIENT_IMPORT_MUTATIONS * 2;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RecipientImportMutationBinding {
    intent_id: [u8; 32],
    execution_plan_id: [u8; 32],
    coordinator_plan_id: [u8; 32],
    coordinator_sequence: u64,
    membership_set_digest: [u8; 32],
    confirmation_id: [u8; 32],
    mutation_nonce: [u8; 32],
    max_steps: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum RecipientImportMutationPhase {
    Prepared {
        starting_completed_source_count: u8,
        starting_imported_event_count: u64,
        total_source_count: u8,
        prepared_at_unix_seconds: u64,
    },
    Completed {
        prepared_record_id: [u8; 32],
        receipt: RuntimeIpcRecipientImportExecutionReceiptSummary,
        completed_at_unix_seconds: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RecipientImportMutationRecordContent {
    version: u8,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    sequence: u64,
    previous_record_id: Option<[u8; 32]>,
    binding: RecipientImportMutationBinding,
    phase: RecipientImportMutationPhase,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SignedRecipientImportMutationRecordWire {
    content: RecipientImportMutationRecordContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SignedRecipientImportMutationRecord {
    content: RecipientImportMutationRecordContent,
    signature: Vec<u8>,
}

impl SignedRecipientImportMutationRecord {
    fn issue(
        owner_account_id: AccountId,
        owner: &DeviceIdentity,
        sequence: u64,
        previous_record_id: Option<[u8; 32]>,
        binding: RecipientImportMutationBinding,
        phase: RecipientImportMutationPhase,
    ) -> Result<Self, RecipientImportMutationAdapterError> {
        let content = RecipientImportMutationRecordContent {
            version: RECORD_VERSION,
            owner_account_id,
            owner_device_id: owner.device_id(),
            sequence,
            previous_record_id,
            binding,
            phase,
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
    ) -> Result<Self, RecipientImportMutationAdapterError> {
        validate_record_length(encoded)?;
        let wire: SignedRecipientImportMutationRecordWire = postcard::from_bytes(encoded)?;
        let record = Self {
            content: wire.content,
            signature: wire.signature,
        };
        record.verify()?;
        if record.content.owner_account_id != owner_account_id {
            return Err(RecipientImportMutationAdapterError::OwnerAccountMismatch);
        }
        if record.content.owner_device_id != owner_device_id {
            return Err(RecipientImportMutationAdapterError::OwnerDeviceMismatch);
        }
        if record.encode()? != encoded {
            return Err(RecipientImportMutationAdapterError::NonCanonicalRecord);
        }
        Ok(record)
    }

    fn verify(&self) -> Result<(), RecipientImportMutationAdapterError> {
        validate_record_content(&self.content)?;
        self.content
            .owner_device_id
            .verify(&record_signing_bytes(&self.content)?, &self.signature)?;
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>, RecipientImportMutationAdapterError> {
        self.verify()?;
        let encoded = postcard::to_allocvec(&SignedRecipientImportMutationRecordWire {
            content: self.content.clone(),
            signature: self.signature.clone(),
        })?;
        validate_record_length(&encoded)?;
        Ok(encoded)
    }

    fn record_id(&self) -> Result<[u8; 32], RecipientImportMutationAdapterError> {
        Ok(domain_digest(RECORD_ID_DOMAIN, &self.encode()?))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RecipientImportMutationHeadWire {
    version: u8,
    sequence: u64,
    record_id: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DurableRecipientImportMutationRecord {
    record_id: [u8; 32],
    record: SignedRecipientImportMutationRecord,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreparedRecipientImportMutation {
    durable: DurableRecipientImportMutationRecord,
    starting_completed_source_count: u8,
    starting_imported_event_count: u64,
    total_source_count: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MutationLookup {
    Absent,
    Prepared(Box<PreparedRecipientImportMutation>),
    Completed(Box<RuntimeIpcRecipientImportExecutionReceiptSummary>),
}

/// Read-only projection of one exact completed mutation. It contains only the
/// signed ledger record identity, the membership-set digest already bound by
/// the mutation, and the canonical execution receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableRecipientImportCompletedMutation {
    completed_record_id: RuntimeIpcRecipientImportArtifactId,
    membership_set_digest: RuntimeIpcRecipientImportArtifactId,
    receipt: RuntimeIpcRecipientImportExecutionReceiptSummary,
}

impl DurableRecipientImportCompletedMutation {
    pub fn completed_record_id(&self) -> &RuntimeIpcRecipientImportArtifactId {
        &self.completed_record_id
    }

    pub fn membership_set_digest(&self) -> &RuntimeIpcRecipientImportArtifactId {
        &self.membership_set_digest
    }

    pub fn receipt(&self) -> &RuntimeIpcRecipientImportExecutionReceiptSummary {
        &self.receipt
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipientImportMutationReadStatus {
    Absent,
    Prepared,
    Completed(Box<DurableRecipientImportCompletedMutation>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MutationLedgerSnapshot {
    records: BTreeMap<[u8; 32], DurableRecipientImportMutationRecord>,
    nonce_index: BTreeMap<[u8; 32], [u8; 32]>,
    head: Option<RecipientImportMutationHeadWire>,
}

impl MutationLedgerSnapshot {
    fn empty() -> Self {
        Self {
            records: BTreeMap::new(),
            nonce_index: BTreeMap::new(),
            head: None,
        }
    }

    fn lookup(
        &self,
        nonce: [u8; 32],
        binding: &RecipientImportMutationBinding,
    ) -> Result<MutationLookup, RecipientImportMutationAdapterError> {
        let Some(record_id) = self.nonce_index.get(&nonce) else {
            return Ok(MutationLookup::Absent);
        };
        let durable = self
            .records
            .get(record_id)
            .ok_or(RecipientImportMutationAdapterError::MissingRecord)?;
        if durable.record.content.binding != *binding {
            return Err(RecipientImportMutationAdapterError::NonceConflict);
        }
        match &durable.record.content.phase {
            RecipientImportMutationPhase::Prepared {
                starting_completed_source_count,
                starting_imported_event_count,
                total_source_count,
                ..
            } => Ok(MutationLookup::Prepared(Box::new(
                PreparedRecipientImportMutation {
                    durable: durable.clone(),
                    starting_completed_source_count: *starting_completed_source_count,
                    starting_imported_event_count: *starting_imported_event_count,
                    total_source_count: *total_source_count,
                },
            ))),
            RecipientImportMutationPhase::Completed { receipt, .. } => {
                Ok(MutationLookup::Completed(Box::new(receipt.clone())))
            }
        }
    }
}

struct RecipientImportMutationLedger {
    state_directory: PathBuf,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

impl RecipientImportMutationLedger {
    fn new(
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

    fn lookup(
        &self,
        nonce: [u8; 32],
        binding: &RecipientImportMutationBinding,
    ) -> Result<MutationLookup, RecipientImportMutationAdapterError> {
        load_snapshot(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        )?
        .lookup(nonce, binding)
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare(
        &self,
        binding: &RecipientImportMutationBinding,
        starting_completed_source_count: u8,
        starting_imported_event_count: u64,
        total_source_count: u8,
        owner: &DeviceIdentity,
        prepared_at_unix_seconds: u64,
    ) -> Result<PreparedRecipientImportMutation, RecipientImportMutationAdapterError> {
        if owner.device_id() != self.owner_device_id {
            return Err(RecipientImportMutationAdapterError::OwnerDeviceMismatch);
        }
        let state_lock = StateDirectoryLock::acquire(&self.state_directory)?;
        let snapshot = load_snapshot(
            state_lock.root(),
            self.owner_account_id,
            self.owner_device_id,
        )?;
        match snapshot.lookup(binding.mutation_nonce, binding) {
            Ok(MutationLookup::Prepared(prepared)) => return Ok(*prepared),
            Ok(MutationLookup::Completed(_)) => {
                return Err(RecipientImportMutationAdapterError::AlreadyCompleted);
            }
            Ok(MutationLookup::Absent) => {}
            Err(error) => return Err(error),
        }
        if snapshot
            .head
            .as_ref()
            .and_then(|head| snapshot.records.get(&head.record_id))
            .is_some_and(|head| {
                matches!(
                    head.record.content.phase,
                    RecipientImportMutationPhase::Prepared { .. }
                )
            })
        {
            return Err(RecipientImportMutationAdapterError::AnotherMutationInFlight);
        }
        if snapshot.nonce_index.len() as u64 >= MAX_DURABLE_RECIPIENT_IMPORT_MUTATIONS
            || snapshot.records.len() as u64 >= MAX_DURABLE_RECIPIENT_IMPORT_MUTATION_RECORDS
        {
            return Err(RecipientImportMutationAdapterError::Capacity);
        }
        let sequence = next_sequence(snapshot.head.as_ref())?;
        let previous_record_id = snapshot.head.as_ref().map(|head| head.record_id);
        let record = SignedRecipientImportMutationRecord::issue(
            self.owner_account_id,
            owner,
            sequence,
            previous_record_id,
            binding.clone(),
            RecipientImportMutationPhase::Prepared {
                starting_completed_source_count,
                starting_imported_event_count,
                total_source_count,
                prepared_at_unix_seconds,
            },
        )?;
        let durable = append_record(
            state_lock.root(),
            self.owner_account_id,
            self.owner_device_id,
            &snapshot,
            record,
            None,
        )?;
        Ok(PreparedRecipientImportMutation {
            durable,
            starting_completed_source_count,
            starting_imported_event_count,
            total_source_count,
        })
    }

    fn complete(
        &self,
        prepared: &PreparedRecipientImportMutation,
        receipt: &RuntimeIpcRecipientImportExecutionReceiptSummary,
        owner: &DeviceIdentity,
        completed_at_unix_seconds: u64,
    ) -> Result<RuntimeIpcRecipientImportExecutionReceiptSummary, RecipientImportMutationAdapterError>
    {
        if owner.device_id() != self.owner_device_id {
            return Err(RecipientImportMutationAdapterError::OwnerDeviceMismatch);
        }
        let binding = &prepared.durable.record.content.binding;
        validate_completed_receipt(prepared, receipt)?;
        let state_lock = StateDirectoryLock::acquire(&self.state_directory)?;
        let snapshot = load_snapshot(
            state_lock.root(),
            self.owner_account_id,
            self.owner_device_id,
        )?;
        match snapshot.lookup(binding.mutation_nonce, binding)? {
            MutationLookup::Completed(existing) => {
                if *existing == *receipt {
                    return Ok(*existing);
                }
                return Err(RecipientImportMutationAdapterError::CompletedReceiptChanged);
            }
            MutationLookup::Prepared(current)
                if current.durable.record_id == prepared.durable.record_id => {}
            MutationLookup::Prepared(_) | MutationLookup::Absent => {
                return Err(RecipientImportMutationAdapterError::LedgerChanged);
            }
        }
        if snapshot.records.len() as u64 >= MAX_DURABLE_RECIPIENT_IMPORT_MUTATION_RECORDS {
            return Err(RecipientImportMutationAdapterError::Capacity);
        }
        let sequence = next_sequence(snapshot.head.as_ref())?;
        let previous_record_id = snapshot.head.as_ref().map(|head| head.record_id);
        if previous_record_id != Some(prepared.durable.record_id) {
            return Err(RecipientImportMutationAdapterError::LedgerChanged);
        }
        let record = SignedRecipientImportMutationRecord::issue(
            self.owner_account_id,
            owner,
            sequence,
            previous_record_id,
            binding.clone(),
            RecipientImportMutationPhase::Completed {
                prepared_record_id: prepared.durable.record_id,
                receipt: receipt.clone(),
                completed_at_unix_seconds,
            },
        )?;
        append_record(
            state_lock.root(),
            self.owner_account_id,
            self.owner_device_id,
            &snapshot,
            record,
            Some(prepared.durable.record_id),
        )?;
        Ok(receipt.clone())
    }
}

pub struct RecipientImportMutationAdapterDispatch {
    outcome: RuntimeIpcRecipientImportExecutionOutcome,
    replayed_completed_receipt: bool,
    recovered_step_count: usize,
    persisted_receipt_before_response: bool,
    authenticated_guided_completion: Option<AuthenticatedGuidedRecipientImportCompletion>,
}

impl RecipientImportMutationAdapterDispatch {
    pub fn outcome(&self) -> &RuntimeIpcRecipientImportExecutionOutcome {
        &self.outcome
    }

    pub fn replayed_completed_receipt(&self) -> bool {
        self.replayed_completed_receipt
    }

    pub fn recovered_step_count(&self) -> usize {
        self.recovered_step_count
    }

    pub fn persisted_receipt_before_response(&self) -> bool {
        self.persisted_receipt_before_response
    }

    pub fn consumed_intent(&self) -> bool {
        false
    }

    pub fn opened_network(&self) -> bool {
        false
    }

    pub fn has_authenticated_guided_completion(&self) -> bool {
        self.authenticated_guided_completion.is_some()
    }

    pub fn into_authenticated_guided_completion(
        self,
    ) -> Option<AuthenticatedGuidedRecipientImportCompletion> {
        self.authenticated_guided_completion
    }
}

pub struct RecipientHistoryRecoveryImportMutationAdapter {
    state_directory: PathBuf,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

impl RecipientHistoryRecoveryImportMutationAdapter {
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

    /// Reads and verifies the exact mutation-ledger entry named by `nonce`.
    /// Missing storage remains missing: this path never creates a directory,
    /// database, table, lock, or replacement record.
    pub fn inspect_by_nonce(
        &self,
        nonce: &RuntimeIpcRecipientImportArtifactId,
    ) -> Result<RecipientImportMutationReadStatus, RecipientImportMutationAdapterError> {
        let nonce = nonce.to_bytes()?;
        let snapshot = load_snapshot(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        )?;
        let Some(record_id) = snapshot.nonce_index.get(&nonce) else {
            return Ok(RecipientImportMutationReadStatus::Absent);
        };
        let durable = snapshot
            .records
            .get(record_id)
            .ok_or(RecipientImportMutationAdapterError::MissingRecord)?;
        if durable.record.content.binding.mutation_nonce != nonce {
            return Err(RecipientImportMutationAdapterError::NonceIndexMismatch);
        }
        match &durable.record.content.phase {
            RecipientImportMutationPhase::Prepared { .. } => {
                Ok(RecipientImportMutationReadStatus::Prepared)
            }
            RecipientImportMutationPhase::Completed { receipt, .. } => {
                Ok(RecipientImportMutationReadStatus::Completed(Box::new(
                    DurableRecipientImportCompletedMutation {
                        completed_record_id: artifact(durable.record_id)?,
                        membership_set_digest: artifact(
                            durable.record.content.binding.membership_set_digest,
                        )?,
                        receipt: receipt.clone(),
                    },
                )))
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dispatch<L: RecipientImportMembershipLoader>(
        &self,
        request: &RuntimeIpcRecipientImportExecutionRequest,
        store: &RecipientRecoveryImportRoundIntentStore,
        current: &RecipientRecoveryCoordinationPlan,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        loader: &L,
        recipient: &DeviceIdentity,
        now_unix_seconds: u64,
    ) -> Result<RecipientImportMutationAdapterDispatch, RecipientImportMutationAdapterError> {
        if recipient.device_id() != self.owner_device_id {
            return Err(RecipientImportMutationAdapterError::OwnerDeviceMismatch);
        }
        let (binding, memberships, apply_next) = match request_binding(request) {
            Ok(value) => value,
            Err(RecipientImportMutationAdapterError::NotMutationCommand) => {
                return Ok(reject(
                    request,
                    RuntimeIpcRecipientImportExecutionHazard::ImportRejected,
                ));
            }
            Err(error) => return Err(error),
        };
        let mutation_ledger = RecipientImportMutationLedger::new(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        );
        let existing = match mutation_ledger.lookup(binding.mutation_nonce, &binding) {
            Ok(existing) => existing,
            Err(RecipientImportMutationAdapterError::NonceConflict) => {
                return Ok(reject(
                    request,
                    RuntimeIpcRecipientImportExecutionHazard::MutationReplay,
                ));
            }
            Err(error) => return Err(error),
        };
        if let MutationLookup::Completed(receipt) = existing {
            return Ok(RecipientImportMutationAdapterDispatch {
                outcome: RuntimeIpcRecipientImportExecutionOutcome::Executed(*receipt),
                replayed_completed_receipt: true,
                recovered_step_count: 0,
                persisted_receipt_before_response: true,
                authenticated_guided_completion: None,
            });
        }

        let confirmation_ledger = RecipientImportConfirmationLedger::new(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        );
        let confirmation =
            match confirmation_ledger.load_by_confirmation_id(binding.confirmation_id)? {
                Some(confirmation) => confirmation,
                None => {
                    return Ok(reject(
                        request,
                        RuntimeIpcRecipientImportExecutionHazard::ConfirmationRequired,
                    ));
                }
            };
        if !confirmation_record_is_exact_owner(
            &confirmation,
            self.owner_account_id,
            self.owner_device_id,
        ) || !confirmation_matches(&confirmation, &binding)?
        {
            return Ok(reject(
                request,
                RuntimeIpcRecipientImportExecutionHazard::ConfirmationRequired,
            ));
        }

        let loaded = match load_exact_memberships(memberships, loader) {
            Ok(loaded) => loaded,
            Err(hazard) => return Ok(reject(request, hazard)),
        };
        let sources = transient_sources(memberships, &loaded);
        let requested_budget =
            RecipientRecoveryImportRoundExecutionBudget::new(usize::from(binding.max_steps))
                .map_err(RecipientImportMutationAdapterError::Execution)?;

        let prepared = match existing {
            MutationLookup::Prepared(prepared) => *prepared,
            MutationLookup::Absent => {
                if confirmation.expired(now_unix_seconds) {
                    return Ok(reject(
                        request,
                        RuntimeIpcRecipientImportExecutionHazard::ConfirmationExpired,
                    ));
                }
                if current.plan_id() != binding.coordinator_plan_id
                    || current.sequence() != binding.coordinator_sequence
                {
                    return Ok(reject(
                        request,
                        RuntimeIpcRecipientImportExecutionHazard::CoordinatorChanged,
                    ));
                }
                let exact = match RecipientHistoryRecoveryImportRoundExecutor::plan(
                    store,
                    current,
                    &sources,
                    requested_budget,
                    inbox,
                    imports,
                ) {
                    Ok(plan) => plan,
                    Err(error) => return Ok(reject(request, execution_hazard(&error))),
                };
                if exact.plan_id() != binding.execution_plan_id
                    || exact.intent_id() != binding.intent_id
                {
                    return Ok(reject(
                        request,
                        RuntimeIpcRecipientImportExecutionHazard::PlanChanged,
                    ));
                }
                mutation_ledger.prepare(
                    &binding,
                    to_u8(exact.progress().completed_source_count)?,
                    to_u64(exact.progress().imported_event_count)?,
                    to_u8(exact.progress().total_source_count)?,
                    recipient,
                    now_unix_seconds,
                )?
            }
            MutationLookup::Completed(_) => unreachable!("completed replay returned above"),
        };

        if prepared.durable.record.content.binding != binding {
            return Ok(reject(
                request,
                RuntimeIpcRecipientImportExecutionHazard::MutationReplay,
            ));
        }
        let authorized_steps = usize::from(binding.max_steps).min(
            usize::from(prepared.total_source_count)
                .saturating_sub(usize::from(prepared.starting_completed_source_count)),
        );
        let probe_budget =
            RecipientRecoveryImportRoundExecutionBudget::new(authorized_steps.max(1))
                .map_err(RecipientImportMutationAdapterError::Execution)?;
        let mut live = match RecipientHistoryRecoveryImportRoundExecutor::plan(
            store,
            current,
            &sources,
            probe_budget,
            inbox,
            imports,
        ) {
            Ok(plan) => plan,
            Err(error) => return Ok(reject(request, execution_hazard(&error))),
        };
        if live.intent_id() != binding.intent_id
            || live.progress().total_source_count != usize::from(prepared.total_source_count)
        {
            return Ok(reject(
                request,
                RuntimeIpcRecipientImportExecutionHazard::PlanChanged,
            ));
        }
        let starting = usize::from(prepared.starting_completed_source_count);
        let observed = live.progress().completed_source_count;
        let recovered_count = match observed.checked_sub(starting) {
            Some(delta) if delta <= authorized_steps => delta,
            _ => {
                return Ok(reject(
                    request,
                    RuntimeIpcRecipientImportExecutionHazard::PlanChanged,
                ));
            }
        };
        let remaining = authorized_steps - recovered_count;
        let remaining_budget = RecipientRecoveryImportRoundExecutionBudget::new(remaining.max(1))
            .map_err(RecipientImportMutationAdapterError::Execution)?;
        if live.progress().coordinator_refresh_required {
            live = match RecipientHistoryRecoveryImportRoundExecutor::reconcile_pending(
                &live,
                store,
                &sources,
                inbox,
                imports,
                remaining_budget,
            ) {
                Ok(plan) => plan,
                Err(error) => return Ok(reject(request, execution_hazard(&error))),
            };
        }

        let mut recovered_steps = recovered_steps(
            store,
            binding.intent_id,
            starting,
            recovered_count,
            live.progress().coordinator_sequence,
        )?;
        let recovered_event_count = recovered_steps
            .iter()
            .try_fold(0_u64, |total, step| {
                total.checked_add(step.imported_event_count)
            })
            .ok_or(RecipientImportMutationAdapterError::ProjectionOverflow)?;
        let expected_observed_event_count = prepared
            .starting_imported_event_count
            .checked_add(recovered_event_count)
            .ok_or(RecipientImportMutationAdapterError::ProjectionOverflow)?;
        if to_u64(live.progress().imported_event_count)? != expected_observed_event_count {
            return Ok(reject(
                request,
                RuntimeIpcRecipientImportExecutionHazard::PlanChanged,
            ));
        }
        let observed_final_plan_id = live.progress().coordinator_plan_id;
        let observed_final_sequence = live.progress().coordinator_sequence;

        let execution = if remaining == 0 {
            None
        } else {
            if confirmation.expired(now_unix_seconds) {
                return Ok(reject(
                    request,
                    RuntimeIpcRecipientImportExecutionHazard::ConfirmationExpired,
                ));
            }
            let fresh = if live.budget().max_steps() == remaining {
                live
            } else {
                return Ok(reject(
                    request,
                    RuntimeIpcRecipientImportExecutionHazard::PlanChanged,
                ));
            };
            let receipt = if apply_next {
                RecipientHistoryRecoveryImportRoundExecutor::apply_next(
                    &fresh,
                    store,
                    &sources,
                    inbox,
                    imports,
                    recipient,
                    now_unix_seconds,
                )
            } else {
                RecipientHistoryRecoveryImportRoundExecutor::apply_bounded(
                    &fresh,
                    store,
                    &sources,
                    inbox,
                    imports,
                    recipient,
                    now_unix_seconds,
                )
            };
            match receipt {
                Ok(receipt) => Some(receipt),
                Err(error) => return Ok(reject(request, execution_hazard(&error))),
            }
        };

        let receipt = combined_receipt(
            &binding,
            &prepared,
            &mut recovered_steps,
            recovered_event_count,
            execution.as_ref(),
            observed_final_plan_id,
            observed_final_sequence,
        )?;
        let authenticated_guided_completion = if receipt.complete {
            let final_coordination =
                RecipientHistoryRecoveryCoordinator::refresh_after_authenticated_import(
                    current, inbox, imports,
                )?;
            Some(
                AuthenticatedGuidedRecipientImportCompletion::from_completed_mutation(
                    &self.state_directory,
                    self.owner_account_id,
                    self.owner_device_id,
                    final_coordination,
                    memberships,
                    loaded,
                    binding.membership_set_digest,
                    receipt.final_coordinator_plan_id.to_bytes()?,
                    receipt.final_coordinator_sequence,
                )?,
            )
        } else {
            None
        };
        let persisted =
            mutation_ledger.complete(&prepared, &receipt, recipient, now_unix_seconds)?;
        // Keep the prepared object alive through the durable commit. This also
        // makes accidental future reordering of execute-before-prepare obvious.
        prepared.durable.record.verify()?;
        Ok(RecipientImportMutationAdapterDispatch {
            outcome: RuntimeIpcRecipientImportExecutionOutcome::Executed(persisted),
            replayed_completed_receipt: false,
            recovered_step_count: recovered_count,
            persisted_receipt_before_response: true,
            authenticated_guided_completion,
        })
    }
}

fn request_binding(
    request: &RuntimeIpcRecipientImportExecutionRequest,
) -> Result<
    (
        RecipientImportMutationBinding,
        &[kilogram_runtime_ipc::RuntimeIpcRecipientImportMembershipReference],
        bool,
    ),
    RecipientImportMutationAdapterError,
> {
    let (
        intent_id,
        execution_plan_id,
        coordinator_plan_id,
        coordinator_sequence,
        memberships,
        confirmation_id,
        mutation_nonce,
        max_steps,
        apply_next,
    ) = match request.command() {
        RuntimeIpcRecipientImportExecutionCommand::ApplyNext {
            intent_id,
            execution_plan_id,
            coordinator_plan_id,
            coordinator_sequence,
            memberships,
            confirmation_id,
            mutation_nonce,
        } => (
            intent_id,
            execution_plan_id,
            coordinator_plan_id,
            *coordinator_sequence,
            memberships,
            confirmation_id,
            mutation_nonce,
            1_u8,
            true,
        ),
        RuntimeIpcRecipientImportExecutionCommand::ApplyBounded {
            intent_id,
            execution_plan_id,
            coordinator_plan_id,
            coordinator_sequence,
            memberships,
            confirmation_id,
            mutation_nonce,
            max_steps,
        } => (
            intent_id,
            execution_plan_id,
            coordinator_plan_id,
            *coordinator_sequence,
            memberships,
            confirmation_id,
            mutation_nonce,
            *max_steps,
            false,
        ),
        _ => return Err(RecipientImportMutationAdapterError::NotMutationCommand),
    };
    let membership_set_digest = request
        .command()
        .membership_set_digest()?
        .ok_or(RecipientImportMutationAdapterError::MissingMembershipDigest)?;
    Ok((
        RecipientImportMutationBinding {
            intent_id: intent_id.to_bytes()?,
            execution_plan_id: execution_plan_id.to_bytes()?,
            coordinator_plan_id: coordinator_plan_id.to_bytes()?,
            coordinator_sequence,
            membership_set_digest: membership_set_digest.to_bytes()?,
            confirmation_id: confirmation_id.to_bytes()?,
            mutation_nonce: mutation_nonce.to_bytes()?,
            max_steps,
        },
        memberships,
        apply_next,
    ))
}

fn confirmation_matches(
    confirmation: &DurableRecipientImportConfirmation,
    binding: &RecipientImportMutationBinding,
) -> Result<bool, RecipientImportMutationAdapterError> {
    let summary = confirmation.summary()?;
    Ok(summary.intent_id.to_bytes()? == binding.intent_id
        && summary.execution_plan_id.to_bytes()? == binding.execution_plan_id
        && summary.coordinator_plan_id.to_bytes()? == binding.coordinator_plan_id
        && summary.coordinator_sequence == binding.coordinator_sequence
        && summary.membership_set_digest.to_bytes()? == binding.membership_set_digest
        && summary.confirmation_id.to_bytes()? == binding.confirmation_id)
}

fn recovered_steps(
    store: &RecipientRecoveryImportRoundIntentStore,
    intent_id: [u8; 32],
    starting: usize,
    count: usize,
    coordinator_sequence: u64,
) -> Result<Vec<RuntimeIpcRecipientImportStepReceipt>, RecipientImportMutationAdapterError> {
    let durable = store
        .load_by_intent_id(intent_id)?
        .ok_or(RecipientImportMutationAdapterError::MissingIntent)?;
    if durable.consumed() {
        return Err(RecipientImportMutationAdapterError::IntentConsumed);
    }
    let ordinals: Vec<_> = durable.intent().source_ordinals().collect();
    let event_counts: Vec<_> = durable.intent().planned_source_event_counts().collect();
    let end = starting
        .checked_add(count)
        .ok_or(RecipientImportMutationAdapterError::ProjectionOverflow)?;
    if end > ordinals.len() || end > event_counts.len() {
        return Err(RecipientImportMutationAdapterError::ProgressChanged);
    }
    ordinals[starting..end]
        .iter()
        .zip(&event_counts[starting..end])
        .map(|(ordinal, event_count)| {
            Ok(RuntimeIpcRecipientImportStepReceipt {
                source_ordinal: u16::try_from(*ordinal)
                    .map_err(|_| RecipientImportMutationAdapterError::ProjectionOverflow)?,
                disposition: RuntimeIpcRecipientImportStepDisposition::AlreadyImported,
                imported_event_count: to_u64(*event_count)?,
                coordinator_sequence,
            })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn combined_receipt(
    binding: &RecipientImportMutationBinding,
    prepared: &PreparedRecipientImportMutation,
    recovered_steps: &mut Vec<RuntimeIpcRecipientImportStepReceipt>,
    recovered_event_count: u64,
    execution: Option<&RecipientRecoveryImportRoundExecutionReceipt>,
    observed_final_plan_id: [u8; 32],
    observed_final_sequence: u64,
) -> Result<RuntimeIpcRecipientImportExecutionReceiptSummary, RecipientImportMutationAdapterError> {
    let (
        final_completed_source_count,
        total_source_count,
        imported_now_count,
        already_imported_count,
        imported_event_count,
        final_coordinator_plan_id,
        final_coordinator_sequence,
        complete,
    ) = if let Some(execution) = execution {
        if execution.starting_completed_source_count
            != usize::from(prepared.starting_completed_source_count) + recovered_steps.len()
            || execution.total_source_count != usize::from(prepared.total_source_count)
            || execution.contains_membership_snapshot
            || execution.contains_message_bodies
            || execution.contains_plaintext
            || execution.opened_network
            || execution.scheduled_background_work
            || execution.concurrent_imports != 1
        {
            return Err(RecipientImportMutationAdapterError::ExecutionReceiptChanged);
        }
        for step in &execution.steps {
            recovered_steps.push(RuntimeIpcRecipientImportStepReceipt {
                source_ordinal: u16::try_from(step.source_ordinal)
                    .map_err(|_| RecipientImportMutationAdapterError::ProjectionOverflow)?,
                disposition: match step.disposition {
                    RecipientRecoveryImportRoundStepDisposition::Imported => {
                        RuntimeIpcRecipientImportStepDisposition::Imported
                    }
                    RecipientRecoveryImportRoundStepDisposition::AlreadyImported => {
                        RuntimeIpcRecipientImportStepDisposition::AlreadyImported
                    }
                },
                imported_event_count: to_u64(step.imported_event_count)?,
                coordinator_sequence: step.coordinator_sequence,
            });
        }
        (
            to_u8(execution.final_completed_source_count)?,
            to_u8(execution.total_source_count)?,
            to_u8(execution.imported_now_count)?,
            to_u8(
                execution
                    .already_imported_count
                    .checked_add(recovered_steps.len() - execution.steps.len())
                    .ok_or(RecipientImportMutationAdapterError::ProjectionOverflow)?,
            )?,
            recovered_event_count
                .checked_add(to_u64(execution.imported_event_count)?)
                .ok_or(RecipientImportMutationAdapterError::ProjectionOverflow)?,
            execution.final_coordinator_plan_id,
            execution.final_coordinator_sequence,
            execution.complete,
        )
    } else {
        let final_count = usize::from(prepared.starting_completed_source_count)
            .checked_add(recovered_steps.len())
            .ok_or(RecipientImportMutationAdapterError::ProjectionOverflow)?;
        (
            to_u8(final_count)?,
            prepared.total_source_count,
            0,
            to_u8(recovered_steps.len())?,
            recovered_event_count,
            observed_final_plan_id,
            observed_final_sequence,
            final_count == usize::from(prepared.total_source_count),
        )
    };
    let receipt = RuntimeIpcRecipientImportExecutionReceiptSummary {
        intent_id: artifact(binding.intent_id)?,
        execution_plan_id: artifact(binding.execution_plan_id)?,
        confirmation_id: artifact(binding.confirmation_id)?,
        mutation_nonce: artifact(binding.mutation_nonce)?,
        starting_completed_source_count: prepared.starting_completed_source_count,
        final_completed_source_count,
        total_source_count,
        steps: recovered_steps.clone(),
        imported_now_count,
        already_imported_count,
        imported_event_count,
        final_coordinator_plan_id: artifact(final_coordinator_plan_id)?,
        final_coordinator_sequence,
        complete,
    };
    validate_completed_receipt(prepared, &receipt)?;
    Ok(receipt)
}

fn validate_completed_receipt(
    prepared: &PreparedRecipientImportMutation,
    receipt: &RuntimeIpcRecipientImportExecutionReceiptSummary,
) -> Result<(), RecipientImportMutationAdapterError> {
    let binding = &prepared.durable.record.content.binding;
    let delta = receipt
        .final_completed_source_count
        .checked_sub(receipt.starting_completed_source_count)
        .ok_or(RecipientImportMutationAdapterError::ExecutionReceiptChanged)?;
    let authorized = binding.max_steps.min(
        prepared
            .total_source_count
            .saturating_sub(prepared.starting_completed_source_count),
    );
    let imported_and_existing = receipt
        .imported_now_count
        .checked_add(receipt.already_imported_count)
        .ok_or(RecipientImportMutationAdapterError::ProjectionOverflow)?;
    let event_count = receipt.steps.iter().try_fold(0_u64, |total, step| {
        total.checked_add(step.imported_event_count)
    });
    if receipt.intent_id.to_bytes()? != binding.intent_id
        || receipt.execution_plan_id.to_bytes()? != binding.execution_plan_id
        || receipt.confirmation_id.to_bytes()? != binding.confirmation_id
        || receipt.mutation_nonce.to_bytes()? != binding.mutation_nonce
        || receipt.starting_completed_source_count != prepared.starting_completed_source_count
        || receipt.total_source_count != prepared.total_source_count
        || delta > authorized
        || usize::from(delta) != receipt.steps.len()
        || imported_and_existing != delta
        || event_count != Some(receipt.imported_event_count)
        || receipt.complete != (receipt.final_completed_source_count == receipt.total_source_count)
    {
        return Err(RecipientImportMutationAdapterError::ExecutionReceiptChanged);
    }
    Ok(())
}

fn confirmation_record_is_exact_owner(
    record: &DurableRecipientImportConfirmation,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> bool {
    record.owner_account_id() == owner_account_id && record.owner_device_id() == owner_device_id
}

fn reject(
    request: &RuntimeIpcRecipientImportExecutionRequest,
    hazard: RuntimeIpcRecipientImportExecutionHazard,
) -> RecipientImportMutationAdapterDispatch {
    RecipientImportMutationAdapterDispatch {
        outcome: RuntimeIpcRecipientImportExecutionOutcome::Rejected {
            intent_id: request.command().intent_id().clone(),
            hazard,
        },
        replayed_completed_receipt: false,
        recovered_step_count: 0,
        persisted_receipt_before_response: false,
        authenticated_guided_completion: None,
    }
}

fn validate_record_content(
    content: &RecipientImportMutationRecordContent,
) -> Result<(), RecipientImportMutationAdapterError> {
    let binding = &content.binding;
    if content.version != RECORD_VERSION
        || content.sequence == 0
        || (content.sequence == 1) != content.previous_record_id.is_none()
        || content.owner_account_id.as_bytes() == &[0_u8; 32]
        || content.owner_device_id.as_bytes() == &[0_u8; 32]
        || binding.intent_id == [0_u8; 32]
        || binding.execution_plan_id == [0_u8; 32]
        || binding.coordinator_plan_id == [0_u8; 32]
        || binding.membership_set_digest == [0_u8; 32]
        || binding.confirmation_id == [0_u8; 32]
        || binding.mutation_nonce == [0_u8; 32]
        || binding.confirmation_id == binding.mutation_nonce
        || binding.max_steps == 0
        || usize::from(binding.max_steps)
            > crate::execution_api::RecipientRecoveryImportRoundExecutionBudget::new(8)
                .map_err(RecipientImportMutationAdapterError::Execution)?
                .max_steps()
    {
        return Err(RecipientImportMutationAdapterError::InvalidRecord);
    }
    match &content.phase {
        RecipientImportMutationPhase::Prepared {
            starting_completed_source_count,
            starting_imported_event_count: _,
            total_source_count,
            prepared_at_unix_seconds,
        } => {
            if *prepared_at_unix_seconds == 0
                || *total_source_count == 0
                || starting_completed_source_count > total_source_count
            {
                return Err(RecipientImportMutationAdapterError::InvalidRecord);
            }
        }
        RecipientImportMutationPhase::Completed {
            prepared_record_id,
            completed_at_unix_seconds,
            ..
        } => {
            if *prepared_record_id == [0_u8; 32] || *completed_at_unix_seconds == 0 {
                return Err(RecipientImportMutationAdapterError::InvalidRecord);
            }
        }
    }
    Ok(())
}

fn record_signing_bytes(
    content: &RecipientImportMutationRecordContent,
) -> Result<Vec<u8>, RecipientImportMutationAdapterError> {
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
) -> Result<MutationLedgerSnapshot, RecipientImportMutationAdapterError> {
    reject_ledger_path_symlinks(state_directory)?;
    let path = database_path(state_directory);
    if !path.is_file() {
        return Ok(MutationLedgerSnapshot::empty());
    }
    let database = ReadOnlyDatabase::open(&path).map_err(database_error)?;
    snapshot_from_database(&database, owner_account_id, owner_device_id)
}

fn snapshot_from_database<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<MutationLedgerSnapshot, RecipientImportMutationAdapterError> {
    verify_database_metadata(database, owner_account_id, owner_device_id)?;
    let read = database.begin_read().map_err(database_error)?;
    let table = read.open_table(RECORD_TABLE).map_err(database_error)?;
    if table.len().map_err(database_error)? > MAX_DURABLE_RECIPIENT_IMPORT_MUTATION_RECORDS {
        return Err(RecipientImportMutationAdapterError::Capacity);
    }
    let mut records = BTreeMap::new();
    for item in table.iter().map_err(database_error)? {
        let (stored_id, encoded) = item.map_err(database_error)?;
        let record_id = copy_id(stored_id.value())?;
        let record = SignedRecipientImportMutationRecord::decode_and_verify(
            encoded.value(),
            owner_account_id,
            owner_device_id,
        )?;
        if record.record_id()? != record_id
            || records
                .insert(
                    record_id,
                    DurableRecipientImportMutationRecord { record_id, record },
                )
                .is_some()
        {
            return Err(RecipientImportMutationAdapterError::RecordIdBinding);
        }
    }
    drop(table);
    let nonce_table = read.open_table(NONCE_TABLE).map_err(database_error)?;
    if nonce_table.len().map_err(database_error)? > MAX_DURABLE_RECIPIENT_IMPORT_MUTATIONS {
        return Err(RecipientImportMutationAdapterError::Capacity);
    }
    let mut nonce_index = BTreeMap::new();
    for item in nonce_table.iter().map_err(database_error)? {
        let (stored_nonce, stored_id) = item.map_err(database_error)?;
        let nonce = copy_id(stored_nonce.value())?;
        let record_id = copy_id(stored_id.value())?;
        if nonce_index.insert(nonce, record_id).is_some() {
            return Err(RecipientImportMutationAdapterError::NonceIndexMismatch);
        }
    }
    drop(nonce_table);
    let heads = read.open_table(HEAD_TABLE).map_err(database_error)?;
    if heads.len().map_err(database_error)? > 1 {
        return Err(RecipientImportMutationAdapterError::LedgerFork);
    }
    let head = heads
        .get(HEAD_KEY)
        .map_err(database_error)?
        .map(|value| decode_head(value.value()))
        .transpose()?;
    drop(heads);
    drop(read);
    verify_chain_and_indexes(&records, &nonce_index, head.as_ref())?;
    Ok(MutationLedgerSnapshot {
        records,
        nonce_index,
        head,
    })
}

fn verify_chain_and_indexes(
    records: &BTreeMap<[u8; 32], DurableRecipientImportMutationRecord>,
    nonce_index: &BTreeMap<[u8; 32], [u8; 32]>,
    head: Option<&RecipientImportMutationHeadWire>,
) -> Result<(), RecipientImportMutationAdapterError> {
    if records.is_empty() {
        return if head.is_none() && nonce_index.is_empty() {
            Ok(())
        } else {
            Err(RecipientImportMutationAdapterError::MissingRecord)
        };
    }
    let head = head.ok_or(RecipientImportMutationAdapterError::MissingHead)?;
    if head.version != HEAD_VERSION
        || head.sequence != records.len() as u64
        || head.record_id == [0_u8; 32]
    {
        return Err(RecipientImportMutationAdapterError::InvalidHead);
    }
    let mut ids = Vec::with_capacity(records.len());
    let mut expected_id = Some(head.record_id);
    let mut expected_sequence = head.sequence;
    let mut visited = HashSet::new();
    while let Some(record_id) = expected_id {
        if !visited.insert(record_id) {
            return Err(RecipientImportMutationAdapterError::LedgerFork);
        }
        let durable = records
            .get(&record_id)
            .ok_or(RecipientImportMutationAdapterError::MissingRecord)?;
        if durable.record.content.sequence != expected_sequence {
            return Err(RecipientImportMutationAdapterError::LedgerRollback);
        }
        ids.push(record_id);
        expected_id = durable.record.content.previous_record_id;
        expected_sequence = expected_sequence.saturating_sub(1);
    }
    if expected_sequence != 0 || visited.len() != records.len() {
        return Err(RecipientImportMutationAdapterError::LedgerFork);
    }
    ids.reverse();
    let mut latest = BTreeMap::new();
    let mut prepared_by_nonce = BTreeMap::new();
    let mut pending_nonce = None;
    for record_id in ids {
        let durable = records
            .get(&record_id)
            .ok_or(RecipientImportMutationAdapterError::MissingRecord)?;
        let nonce = durable.record.content.binding.mutation_nonce;
        match &durable.record.content.phase {
            RecipientImportMutationPhase::Prepared { .. } => {
                if pending_nonce.is_some() || latest.contains_key(&nonce) {
                    return Err(RecipientImportMutationAdapterError::LedgerFork);
                }
                pending_nonce = Some(nonce);
                prepared_by_nonce.insert(nonce, record_id);
            }
            RecipientImportMutationPhase::Completed {
                prepared_record_id,
                receipt,
                ..
            } => {
                if pending_nonce != Some(nonce)
                    || prepared_by_nonce.get(&nonce) != Some(prepared_record_id)
                {
                    return Err(RecipientImportMutationAdapterError::LedgerFork);
                }
                let prepared_record = records
                    .get(prepared_record_id)
                    .ok_or(RecipientImportMutationAdapterError::MissingRecord)?;
                let RecipientImportMutationPhase::Prepared {
                    starting_completed_source_count,
                    starting_imported_event_count,
                    total_source_count,
                    ..
                } = prepared_record.record.content.phase
                else {
                    return Err(RecipientImportMutationAdapterError::LedgerFork);
                };
                let prepared = PreparedRecipientImportMutation {
                    durable: prepared_record.clone(),
                    starting_completed_source_count,
                    starting_imported_event_count,
                    total_source_count,
                };
                validate_completed_receipt(&prepared, receipt)?;
                pending_nonce = None;
            }
        }
        latest.insert(nonce, record_id);
    }
    if &latest != nonce_index {
        return Err(RecipientImportMutationAdapterError::NonceIndexMismatch);
    }
    Ok(())
}

fn append_record(
    root: &Path,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    snapshot: &MutationLedgerSnapshot,
    record: SignedRecipientImportMutationRecord,
    expected_nonce_head: Option<[u8; 32]>,
) -> Result<DurableRecipientImportMutationRecord, RecipientImportMutationAdapterError> {
    record.verify()?;
    let expected_sequence = next_sequence(snapshot.head.as_ref())?;
    let expected_previous = snapshot.head.as_ref().map(|head| head.record_id);
    if record.content.sequence != expected_sequence
        || record.content.previous_record_id != expected_previous
        || record.content.owner_account_id != owner_account_id
        || record.content.owner_device_id != owner_device_id
    {
        return Err(RecipientImportMutationAdapterError::LedgerChanged);
    }
    let path = prepare_database_path(root)?;
    let existed = path.is_file();
    let database = Database::create(&path).map_err(database_error)?;
    if existed {
        verify_database(&database, owner_account_id, owner_device_id)?;
    } else {
        initialize_database(&database, owner_account_id, owner_device_id)?;
    }
    let locked = snapshot_from_database(&database, owner_account_id, owner_device_id)?;
    if locked.head != snapshot.head
        || locked.records.keys().ne(snapshot.records.keys())
        || locked.nonce_index != snapshot.nonce_index
    {
        return Err(RecipientImportMutationAdapterError::LedgerChanged);
    }
    let record_id = record.record_id()?;
    let nonce = record.content.binding.mutation_nonce;
    let record_bytes = record.encode()?;
    let head = RecipientImportMutationHeadWire {
        version: HEAD_VERSION,
        sequence: record.content.sequence,
        record_id,
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
            .get(record_id.as_slice())
            .map_err(database_error)?
            .is_some()
        {
            return Err(RecipientImportMutationAdapterError::LedgerChanged);
        }
        let stored_nonce_head = nonces
            .get(nonce.as_slice())
            .map_err(database_error)?
            .map(|value| copy_id(value.value()))
            .transpose()?;
        if stored_nonce_head != expected_nonce_head {
            return Err(RecipientImportMutationAdapterError::LedgerChanged);
        }
        let stored_head = heads
            .get(HEAD_KEY)
            .map_err(database_error)?
            .map(|value| decode_head(value.value()))
            .transpose()?;
        if stored_head != snapshot.head {
            return Err(RecipientImportMutationAdapterError::LedgerChanged);
        }
        records
            .insert(record_id.as_slice(), record_bytes.as_slice())
            .map_err(database_error)?;
        nonces
            .insert(nonce.as_slice(), record_id.as_slice())
            .map_err(database_error)?;
        heads
            .insert(HEAD_KEY, head_bytes.as_slice())
            .map_err(database_error)?;
    }
    write.commit().map_err(database_error)?;
    let committed = snapshot_from_database(&database, owner_account_id, owner_device_id)?
        .records
        .get(&record_id)
        .cloned()
        .ok_or(RecipientImportMutationAdapterError::MissingRecord)?;
    if committed.record != record {
        return Err(RecipientImportMutationAdapterError::CommittedRecordMismatch);
    }
    Ok(committed)
}

fn initialize_database(
    database: &Database,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), RecipientImportMutationAdapterError> {
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
) -> Result<(), RecipientImportMutationAdapterError> {
    snapshot_from_database(database, owner_account_id, owner_device_id).map(|_| ())
}

fn verify_database_metadata<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), RecipientImportMutationAdapterError> {
    let read = database.begin_read().map_err(database_error)?;
    let meta = read.open_table(META_TABLE).map_err(database_error)?;
    verify_meta(&meta, SCHEMA_KEY, &[SCHEMA_VERSION], || {
        RecipientImportMutationAdapterError::SchemaMismatch
    })?;
    verify_meta(
        &meta,
        OWNER_ACCOUNT_KEY,
        owner_account_id.as_bytes(),
        || RecipientImportMutationAdapterError::OwnerAccountMismatch,
    )?;
    verify_meta(&meta, OWNER_DEVICE_KEY, owner_device_id.as_bytes(), || {
        RecipientImportMutationAdapterError::OwnerDeviceMismatch
    })?;
    Ok(())
}

fn verify_meta(
    meta: &impl ReadableTable<&'static str, &'static [u8]>,
    key: &'static str,
    expected: &[u8],
    mismatch: impl Fn() -> RecipientImportMutationAdapterError,
) -> Result<(), RecipientImportMutationAdapterError> {
    let value = meta
        .get(key)
        .map_err(database_error)?
        .ok_or_else(&mismatch)?;
    if value.value() != expected {
        return Err(mismatch());
    }
    Ok(())
}

fn next_sequence(
    head: Option<&RecipientImportMutationHeadWire>,
) -> Result<u64, RecipientImportMutationAdapterError> {
    head.map_or(Ok(1), |head| {
        head.sequence
            .checked_add(1)
            .ok_or(RecipientImportMutationAdapterError::SequenceExhausted)
    })
}

fn prepare_database_path(root: &Path) -> Result<PathBuf, RecipientImportMutationAdapterError> {
    reject_ledger_path_symlinks(root)?;
    let directory = root.join(LEDGER_DIRECTORY);
    fs::create_dir_all(&directory)?;
    reject_ledger_path_symlinks(root)?;
    let canonical_directory = fs::canonicalize(&directory)?;
    if !canonical_directory.starts_with(root) {
        return Err(RecipientImportMutationAdapterError::UnsafePath);
    }
    Ok(canonical_directory.join(DATABASE_FILE))
}

fn reject_ledger_path_symlinks(root: &Path) -> Result<(), RecipientImportMutationAdapterError> {
    for path in [root.join(LEDGER_DIRECTORY), database_path(root)] {
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(RecipientImportMutationAdapterError::UnsafePath);
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

fn encode_head(
    head: &RecipientImportMutationHeadWire,
) -> Result<Vec<u8>, RecipientImportMutationAdapterError> {
    if head.version != HEAD_VERSION || head.sequence == 0 || head.record_id == [0_u8; 32] {
        return Err(RecipientImportMutationAdapterError::InvalidHead);
    }
    let encoded = postcard::to_allocvec(head)?;
    if encoded.is_empty() || encoded.len() > MAX_HEAD_BYTES {
        return Err(RecipientImportMutationAdapterError::HeadLength(
            encoded.len(),
        ));
    }
    Ok(encoded)
}

fn decode_head(
    encoded: &[u8],
) -> Result<RecipientImportMutationHeadWire, RecipientImportMutationAdapterError> {
    if encoded.is_empty() || encoded.len() > MAX_HEAD_BYTES {
        return Err(RecipientImportMutationAdapterError::HeadLength(
            encoded.len(),
        ));
    }
    let head: RecipientImportMutationHeadWire = postcard::from_bytes(encoded)?;
    if encode_head(&head)? != encoded {
        return Err(RecipientImportMutationAdapterError::NonCanonicalHead);
    }
    Ok(head)
}

fn validate_record_length(encoded: &[u8]) -> Result<(), RecipientImportMutationAdapterError> {
    if encoded.is_empty() || encoded.len() > MAX_RECORD_BYTES {
        return Err(RecipientImportMutationAdapterError::RecordLength(
            encoded.len(),
        ));
    }
    Ok(())
}

fn copy_id(bytes: &[u8]) -> Result<[u8; 32], RecipientImportMutationAdapterError> {
    bytes
        .try_into()
        .map_err(|_| RecipientImportMutationAdapterError::StoredIdLength(bytes.len()))
}

fn artifact(
    bytes: [u8; 32],
) -> Result<RuntimeIpcRecipientImportArtifactId, RecipientImportMutationAdapterError> {
    RuntimeIpcRecipientImportArtifactId::from_bytes(bytes).map_err(Into::into)
}

fn to_u8(value: usize) -> Result<u8, RecipientImportMutationAdapterError> {
    u8::try_from(value).map_err(|_| RecipientImportMutationAdapterError::ProjectionOverflow)
}

fn to_u64(value: usize) -> Result<u64, RecipientImportMutationAdapterError> {
    u64::try_from(value).map_err(|_| RecipientImportMutationAdapterError::ProjectionOverflow)
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

fn database_error(error: impl std::fmt::Display) -> RecipientImportMutationAdapterError {
    RecipientImportMutationAdapterError::Database(error.to_string())
}

#[derive(Debug, Error)]
pub enum RecipientImportMutationAdapterError {
    #[error("recipient import mutation request is not an apply command")]
    NotMutationCommand,
    #[error("recipient import mutation request has no membership-set digest")]
    MissingMembershipDigest,
    #[error("recipient import mutation owner Account does not match the ledger")]
    OwnerAccountMismatch,
    #[error("recipient import mutation owner Device does not match the ledger")]
    OwnerDeviceMismatch,
    #[error("recipient import mutation nonce is already bound to another request")]
    NonceConflict,
    #[error("another recipient import mutation is already in flight")]
    AnotherMutationInFlight,
    #[error("recipient import mutation was already completed")]
    AlreadyCompleted,
    #[error("recipient import mutation durable ledger changed")]
    LedgerChanged,
    #[error("recipient import mutation durable ledger forked")]
    LedgerFork,
    #[error("recipient import mutation durable ledger rolled back internally")]
    LedgerRollback,
    #[error("recipient import mutation durable ledger has no head")]
    MissingHead,
    #[error("recipient import mutation durable ledger head is invalid")]
    InvalidHead,
    #[error("recipient import mutation durable ledger schema is invalid")]
    SchemaMismatch,
    #[error("recipient import mutation durable record is missing")]
    MissingRecord,
    #[error("recipient import mutation durable record is invalid")]
    InvalidRecord,
    #[error("recipient import mutation durable record ID is inconsistent")]
    RecordIdBinding,
    #[error("recipient import mutation durable nonce index is inconsistent")]
    NonceIndexMismatch,
    #[error("recipient import mutation durable record is not canonical")]
    NonCanonicalRecord,
    #[error("recipient import mutation durable head is not canonical")]
    NonCanonicalHead,
    #[error("recipient import mutation durable receipt changed")]
    CompletedReceiptChanged,
    #[error("recipient import mutation execution receipt changed")]
    ExecutionReceiptChanged,
    #[error("recipient import mutation committed record did not round-trip")]
    CommittedRecordMismatch,
    #[error("recipient import mutation sequence is exhausted")]
    SequenceExhausted,
    #[error("recipient import mutation ledger capacity is exhausted")]
    Capacity,
    #[error("recipient import mutation state path is unsafe")]
    UnsafePath,
    #[error("recipient import mutation source intent is missing")]
    MissingIntent,
    #[error("recipient import mutation source intent was consumed")]
    IntentConsumed,
    #[error("recipient import mutation durable progress changed")]
    ProgressChanged,
    #[error("recipient import mutation projection exceeds the IPC bounds")]
    ProjectionOverflow,
    #[error("recipient import mutation durable record length {0} is invalid")]
    RecordLength(usize),
    #[error("recipient import mutation durable head length {0} is invalid")]
    HeadLength(usize),
    #[error("recipient import mutation durable ID length {0} is invalid")]
    StoredIdLength(usize),
    #[error("recipient import mutation database operation failed: {0}")]
    Database(String),
    #[error(transparent)]
    Confirmation(#[from] RecipientImportConfirmationLedgerError),
    #[error(transparent)]
    Execution(#[from] crate::execution_api::RecipientRecoveryImportRoundExecutionError),
    #[error(transparent)]
    Coordinator(Box<crate::execution_api::RecipientRecoveryCoordinatorError>),
    #[error(transparent)]
    RuntimeAttachment(Box<crate::RecipientImportConsumptionRuntimeAttachmentError>),
    #[error(transparent)]
    Intent(Box<crate::execution_api::RecipientRecoveryImportRoundIntentError>),
    #[error(transparent)]
    Contract(#[from] ContractError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    State(#[from] StateError),
    #[error(transparent)]
    Postcard(#[from] postcard::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<crate::execution_api::RecipientRecoveryImportRoundIntentError>
    for RecipientImportMutationAdapterError
{
    fn from(error: crate::execution_api::RecipientRecoveryImportRoundIntentError) -> Self {
        Self::Intent(Box::new(error))
    }
}

impl From<crate::execution_api::RecipientRecoveryCoordinatorError>
    for RecipientImportMutationAdapterError
{
    fn from(error: crate::execution_api::RecipientRecoveryCoordinatorError) -> Self {
        Self::Coordinator(Box::new(error))
    }
}

impl From<crate::RecipientImportConsumptionRuntimeAttachmentError>
    for RecipientImportMutationAdapterError
{
    fn from(error: crate::RecipientImportConsumptionRuntimeAttachmentError) -> Self {
        Self::RuntimeAttachment(Box::new(error))
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    struct Fixture {
        root: TempDir,
        account_id: AccountId,
        owner: DeviceIdentity,
        binding: RecipientImportMutationBinding,
    }

    fn fixture() -> Result<Fixture, Box<dyn std::error::Error>> {
        Ok(Fixture {
            root: tempfile::tempdir()?,
            account_id: AccountId::from_bytes([41_u8; 32]),
            owner: DeviceIdentity::from_secret_bytes([42_u8; 32]),
            binding: RecipientImportMutationBinding {
                intent_id: [1_u8; 32],
                execution_plan_id: [2_u8; 32],
                coordinator_plan_id: [3_u8; 32],
                coordinator_sequence: 7,
                membership_set_digest: [4_u8; 32],
                confirmation_id: [5_u8; 32],
                mutation_nonce: [6_u8; 32],
                max_steps: 2,
            },
        })
    }

    fn ledger(fixture: &Fixture) -> RecipientImportMutationLedger {
        RecipientImportMutationLedger::new(
            fixture.root.path(),
            fixture.account_id,
            fixture.owner.device_id(),
        )
    }

    fn receipt(
        binding: &RecipientImportMutationBinding,
    ) -> Result<RuntimeIpcRecipientImportExecutionReceiptSummary, Box<dyn std::error::Error>> {
        Ok(RuntimeIpcRecipientImportExecutionReceiptSummary {
            intent_id: artifact(binding.intent_id)?,
            execution_plan_id: artifact(binding.execution_plan_id)?,
            confirmation_id: artifact(binding.confirmation_id)?,
            mutation_nonce: artifact(binding.mutation_nonce)?,
            starting_completed_source_count: 0,
            final_completed_source_count: 2,
            total_source_count: 2,
            steps: vec![
                RuntimeIpcRecipientImportStepReceipt {
                    source_ordinal: 0,
                    disposition: RuntimeIpcRecipientImportStepDisposition::Imported,
                    imported_event_count: 3,
                    coordinator_sequence: 8,
                },
                RuntimeIpcRecipientImportStepReceipt {
                    source_ordinal: 1,
                    disposition: RuntimeIpcRecipientImportStepDisposition::AlreadyImported,
                    imported_event_count: 4,
                    coordinator_sequence: 9,
                },
            ],
            imported_now_count: 1,
            already_imported_count: 1,
            imported_event_count: 7,
            final_coordinator_plan_id: artifact([7_u8; 32])?,
            final_coordinator_sequence: 9,
            complete: true,
        })
    }

    #[test]
    fn mutation_ledger_prepares_completes_and_replays_exact_receipt()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let ledger = ledger(&fixture);
        let adapter = RecipientHistoryRecoveryImportMutationAdapter::new(
            fixture.root.path(),
            fixture.account_id,
            fixture.owner.device_id(),
        );
        assert!(matches!(
            ledger.lookup(fixture.binding.mutation_nonce, &fixture.binding)?,
            MutationLookup::Absent
        ));
        assert!(matches!(
            adapter.inspect_by_nonce(&artifact(fixture.binding.mutation_nonce)?)?,
            RecipientImportMutationReadStatus::Absent
        ));
        assert!(!database_path(fixture.root.path()).exists());
        let prepared = ledger.prepare(&fixture.binding, 0, 0, 2, &fixture.owner, 100)?;
        assert_eq!(prepared.durable.record.content.sequence, 1);
        assert!(matches!(
            ledger.lookup(fixture.binding.mutation_nonce, &fixture.binding)?,
            MutationLookup::Prepared(_)
        ));
        assert!(matches!(
            adapter.inspect_by_nonce(&artifact(fixture.binding.mutation_nonce)?)?,
            RecipientImportMutationReadStatus::Prepared
        ));
        let expected = receipt(&fixture.binding)?;
        assert_eq!(
            ledger.complete(&prepared, &expected, &fixture.owner, 101)?,
            expected
        );
        assert!(matches!(
            ledger.lookup(fixture.binding.mutation_nonce, &fixture.binding)?,
            MutationLookup::Completed(value) if *value == expected
        ));
        let RecipientImportMutationReadStatus::Completed(completed) =
            adapter.inspect_by_nonce(&artifact(fixture.binding.mutation_nonce)?)?
        else {
            return Err("completed mutation projection missing".into());
        };
        assert_eq!(completed.receipt(), &expected);
        assert_eq!(
            completed.membership_set_digest().to_bytes()?,
            fixture.binding.membership_set_digest
        );
        Ok(())
    }

    #[test]
    fn mutation_ledger_rejects_parallel_and_rebound_nonce() -> Result<(), Box<dyn std::error::Error>>
    {
        let fixture = fixture()?;
        let ledger = ledger(&fixture);
        let prepared = ledger.prepare(&fixture.binding, 0, 0, 2, &fixture.owner, 100)?;
        let mut parallel = fixture.binding.clone();
        parallel.mutation_nonce = [8_u8; 32];
        assert!(matches!(
            ledger.prepare(&parallel, 0, 0, 2, &fixture.owner, 100),
            Err(RecipientImportMutationAdapterError::AnotherMutationInFlight)
        ));
        let mut rebound = fixture.binding.clone();
        rebound.execution_plan_id = [9_u8; 32];
        assert!(matches!(
            ledger.lookup(fixture.binding.mutation_nonce, &rebound),
            Err(RecipientImportMutationAdapterError::NonceConflict)
        ));
        let expected = receipt(&fixture.binding)?;
        ledger.complete(&prepared, &expected, &fixture.owner, 101)?;
        assert!(matches!(
            ledger.lookup(fixture.binding.mutation_nonce, &rebound),
            Err(RecipientImportMutationAdapterError::NonceConflict)
        ));
        Ok(())
    }

    #[test]
    fn mutation_ledger_rejects_foreign_device_and_corrupt_head()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let ledger = ledger(&fixture);
        let foreign = DeviceIdentity::from_secret_bytes([99_u8; 32]);
        assert!(matches!(
            ledger.prepare(&fixture.binding, 0, 0, 2, &foreign, 100),
            Err(RecipientImportMutationAdapterError::OwnerDeviceMismatch)
        ));
        ledger.prepare(&fixture.binding, 0, 0, 2, &fixture.owner, 100)?;
        let database = Database::create(database_path(fixture.root.path()))?;
        let write = database.begin_write()?;
        {
            let mut heads = write.open_table(HEAD_TABLE)?;
            let encoded = heads.get(HEAD_KEY)?.ok_or("head missing")?;
            let mut bytes = encoded.value().to_vec();
            drop(encoded);
            let last = bytes.last_mut().ok_or("empty head")?;
            *last ^= 1;
            heads.insert(HEAD_KEY, bytes.as_slice())?;
        }
        write.commit()?;
        drop(database);
        assert!(
            ledger
                .lookup(fixture.binding.mutation_nonce, &fixture.binding)
                .is_err()
        );
        Ok(())
    }
}
