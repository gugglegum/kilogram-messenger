//! Dormant crash-safe adapter for consuming one explicitly authorized import intent.
//!
//! The exact authorization and completed import evidence are revalidated before
//! mutation. A Device-signed `Prepared` record is durable before the active
//! intent is consumed, and a Device-signed `Completed` receipt is durable
//! before success is returned. The adapter does not delete imported history,
//! open a network, or register itself with the active runtime.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Error as ContractError;
use kilogram_identity::{AccountId, DeviceId, DeviceIdentity, IdentityError};
use kilogram_runtime_ipc::{
    RuntimeIpcRecipientImportArtifactId, RuntimeIpcRecipientImportCompletedMutationEvidence,
    RuntimeIpcRecipientImportConsumptionCommand,
    RuntimeIpcRecipientImportConsumptionExplicitApproval,
    RuntimeIpcRecipientImportConsumptionHazard, RuntimeIpcRecipientImportConsumptionOutcome,
    RuntimeIpcRecipientImportConsumptionPlanSummary,
    RuntimeIpcRecipientImportConsumptionReceiptSummary,
    RuntimeIpcRecipientImportConsumptionRequest, recipient_import_membership_set_digest,
};
use kilogram_state::{StateDirectoryLock, StateError};
use redb::{
    Database, Durability, ReadOnlyDatabase, ReadableDatabase, ReadableTable, ReadableTableMetadata,
    TableDefinition,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    RecipientHistoryRecoveryImportMutationAdapter, RecipientImportConsumptionAuthorizationLedger,
    RecipientImportConsumptionAuthorizationLedgerError, RecipientImportMembershipLoader,
    RecipientImportMutationAdapterError, RecipientImportMutationReadStatus,
    execution_api::{
        DurableRecipientRecoveryImportRoundIntent, RecipientHistoryImportStore,
        RecipientHistoryTransferInbox, RecipientRecoveryCoordinationPlan,
        RecipientRecoveryImportRoundIntentConsumeOutcome, RecipientRecoveryImportRoundIntentError,
        RecipientRecoveryImportRoundIntentProgressState, RecipientRecoveryImportRoundIntentStore,
    },
    recipient_import_read_adapter::{load_exact_memberships, transient_sources},
};

const LEDGER_DIRECTORY: &str = "history-recovery";
const DATABASE_FILE: &str = "recipient-import-consumption-mutations-v1.redb";
const META_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("recipient-import-consumption-mutation-meta-v1");
const PREPARED_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("recipient-import-consumption-mutation-prepared-v1");
const COMPLETED_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("recipient-import-consumption-mutation-completed-v1");

const SCHEMA_KEY: &str = "schema-version";
const OWNER_ACCOUNT_KEY: &str = "owner-account-id";
const OWNER_DEVICE_KEY: &str = "owner-device-id";
const SCHEMA_VERSION: u8 = 1;
const RECORD_VERSION: u8 = 1;
const RECORD_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:recipient-import-consumption-mutation-record:v1\0";
const RECORD_ID_DOMAIN: &[u8] = b"kilogram:recipient-import-consumption-mutation-record-id:v1\0";
const MAX_RECORD_BYTES: usize = 16 * 1024;
pub const MAX_DURABLE_RECIPIENT_IMPORT_CONSUMPTION_MUTATIONS: u64 = 4_096;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RecipientImportConsumptionMutationBinding {
    evidence: RuntimeIpcRecipientImportCompletedMutationEvidence,
    plan: RuntimeIpcRecipientImportConsumptionPlanSummary,
    membership_set_digest: [u8; 32],
}

impl RecipientImportConsumptionMutationBinding {
    fn nonce(&self) -> Result<[u8; 32], RecipientImportConsumptionMutationAdapterError> {
        self.plan
            .consumption_nonce
            .to_bytes()
            .map_err(RecipientImportConsumptionMutationAdapterError::Contract)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum RecipientImportConsumptionMutationPhase {
    Prepared {
        authorization_id: [u8; 32],
        consumed_at_unix_seconds: u64,
        prepared_at_unix_seconds: u64,
    },
    Completed {
        prepared_record_id: [u8; 32],
        receipt: RuntimeIpcRecipientImportConsumptionReceiptSummary,
        completed_at_unix_seconds: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RecipientImportConsumptionMutationRecordContent {
    version: u8,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    binding: RecipientImportConsumptionMutationBinding,
    phase: RecipientImportConsumptionMutationPhase,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SignedRecipientImportConsumptionMutationRecordWire {
    content: RecipientImportConsumptionMutationRecordContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SignedRecipientImportConsumptionMutationRecord {
    content: RecipientImportConsumptionMutationRecordContent,
    signature: Vec<u8>,
}

impl SignedRecipientImportConsumptionMutationRecord {
    fn issue(
        owner_account_id: AccountId,
        owner: &DeviceIdentity,
        binding: RecipientImportConsumptionMutationBinding,
        phase: RecipientImportConsumptionMutationPhase,
    ) -> Result<Self, RecipientImportConsumptionMutationAdapterError> {
        let content = RecipientImportConsumptionMutationRecordContent {
            version: RECORD_VERSION,
            owner_account_id,
            owner_device_id: owner.device_id(),
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
    ) -> Result<Self, RecipientImportConsumptionMutationAdapterError> {
        validate_record_length(encoded)?;
        let wire: SignedRecipientImportConsumptionMutationRecordWire =
            postcard::from_bytes(encoded)?;
        let record = Self {
            content: wire.content,
            signature: wire.signature,
        };
        record.verify()?;
        if record.content.owner_account_id != owner_account_id {
            return Err(RecipientImportConsumptionMutationAdapterError::OwnerAccountMismatch);
        }
        if record.content.owner_device_id != owner_device_id {
            return Err(RecipientImportConsumptionMutationAdapterError::OwnerDeviceMismatch);
        }
        if record.encode()? != encoded {
            return Err(RecipientImportConsumptionMutationAdapterError::NonCanonicalRecord);
        }
        Ok(record)
    }

    fn verify(&self) -> Result<(), RecipientImportConsumptionMutationAdapterError> {
        validate_record_content(&self.content)?;
        self.content
            .owner_device_id
            .verify(&record_signing_bytes(&self.content)?, &self.signature)?;
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>, RecipientImportConsumptionMutationAdapterError> {
        self.verify()?;
        let encoded = postcard::to_allocvec(&SignedRecipientImportConsumptionMutationRecordWire {
            content: self.content.clone(),
            signature: self.signature.clone(),
        })?;
        validate_record_length(&encoded)?;
        Ok(encoded)
    }

    fn record_id(&self) -> Result<[u8; 32], RecipientImportConsumptionMutationAdapterError> {
        Ok(domain_digest(RECORD_ID_DOMAIN, &self.encode()?))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DurableMutationRecord {
    record_id: [u8; 32],
    record: SignedRecipientImportConsumptionMutationRecord,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreparedMutation {
    durable: DurableMutationRecord,
    authorization_id: [u8; 32],
    consumed_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MutationLookup {
    Absent,
    Prepared(Box<PreparedMutation>),
    Completed(Box<RuntimeIpcRecipientImportConsumptionReceiptSummary>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LedgerSnapshot {
    prepared: BTreeMap<[u8; 32], DurableMutationRecord>,
    completed: BTreeMap<[u8; 32], DurableMutationRecord>,
}

impl LedgerSnapshot {
    fn empty() -> Self {
        Self {
            prepared: BTreeMap::new(),
            completed: BTreeMap::new(),
        }
    }

    fn lookup(
        &self,
        nonce: [u8; 32],
        binding: &RecipientImportConsumptionMutationBinding,
    ) -> Result<MutationLookup, RecipientImportConsumptionMutationAdapterError> {
        let Some(prepared) = self.prepared.get(&nonce) else {
            if self.completed.contains_key(&nonce) {
                return Err(RecipientImportConsumptionMutationAdapterError::MissingPreparedRecord);
            }
            return Ok(MutationLookup::Absent);
        };
        if prepared.record.content.binding != *binding {
            return Err(RecipientImportConsumptionMutationAdapterError::NonceConflict);
        }
        let RecipientImportConsumptionMutationPhase::Prepared {
            authorization_id,
            consumed_at_unix_seconds,
            ..
        } = prepared.record.content.phase
        else {
            return Err(RecipientImportConsumptionMutationAdapterError::InvalidRecord);
        };
        match self.completed.get(&nonce) {
            None => Ok(MutationLookup::Prepared(Box::new(PreparedMutation {
                durable: prepared.clone(),
                authorization_id,
                consumed_at_unix_seconds,
            }))),
            Some(completed) => {
                if completed.record.content.binding != *binding {
                    return Err(RecipientImportConsumptionMutationAdapterError::NonceConflict);
                }
                let RecipientImportConsumptionMutationPhase::Completed {
                    prepared_record_id,
                    ref receipt,
                    ..
                } = completed.record.content.phase
                else {
                    return Err(RecipientImportConsumptionMutationAdapterError::InvalidRecord);
                };
                if prepared_record_id != prepared.record_id {
                    return Err(
                        RecipientImportConsumptionMutationAdapterError::PreparedRecordChanged,
                    );
                }
                Ok(MutationLookup::Completed(Box::new(receipt.clone())))
            }
        }
    }
}

struct RecipientImportConsumptionMutationLedger {
    state_directory: PathBuf,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

impl RecipientImportConsumptionMutationLedger {
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
        binding: &RecipientImportConsumptionMutationBinding,
    ) -> Result<MutationLookup, RecipientImportConsumptionMutationAdapterError> {
        load_snapshot(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        )?
        .lookup(nonce, binding)
    }

    fn prepare(
        &self,
        binding: &RecipientImportConsumptionMutationBinding,
        authorization_id: [u8; 32],
        owner: &DeviceIdentity,
        now_unix_seconds: u64,
    ) -> Result<PreparedMutation, RecipientImportConsumptionMutationAdapterError> {
        if owner.device_id() != self.owner_device_id {
            return Err(RecipientImportConsumptionMutationAdapterError::OwnerDeviceMismatch);
        }
        if authorization_id == [0_u8; 32]
            || !(binding.plan.planned_at_unix_seconds..=binding.plan.expires_at_unix_seconds)
                .contains(&now_unix_seconds)
        {
            return Err(RecipientImportConsumptionMutationAdapterError::AuthorizationExpired);
        }
        let state_lock = StateDirectoryLock::acquire(&self.state_directory)?;
        let snapshot = load_snapshot(
            state_lock.root(),
            self.owner_account_id,
            self.owner_device_id,
        )?;
        match snapshot.lookup(binding.nonce()?, binding)? {
            MutationLookup::Prepared(prepared) => {
                if prepared.authorization_id != authorization_id {
                    return Err(
                        RecipientImportConsumptionMutationAdapterError::PreparedRecordChanged,
                    );
                }
                return Ok(*prepared);
            }
            MutationLookup::Completed(_) => {
                return Err(RecipientImportConsumptionMutationAdapterError::AlreadyCompleted);
            }
            MutationLookup::Absent => {}
        }
        if snapshot
            .prepared
            .keys()
            .any(|nonce| !snapshot.completed.contains_key(nonce))
        {
            return Err(RecipientImportConsumptionMutationAdapterError::AnotherMutationInFlight);
        }
        if snapshot.prepared.len() as u64 >= MAX_DURABLE_RECIPIENT_IMPORT_CONSUMPTION_MUTATIONS {
            return Err(RecipientImportConsumptionMutationAdapterError::Capacity);
        }
        let candidate = SignedRecipientImportConsumptionMutationRecord::issue(
            self.owner_account_id,
            owner,
            binding.clone(),
            RecipientImportConsumptionMutationPhase::Prepared {
                authorization_id,
                consumed_at_unix_seconds: now_unix_seconds,
                prepared_at_unix_seconds: now_unix_seconds,
            },
        )?;
        let record_id = candidate.record_id()?;
        let encoded = candidate.encode()?;
        let path = prepare_database_path(state_lock.root())?;
        let existed = path.is_file();
        let database = Database::create(&path).map_err(database_error)?;
        if existed {
            verify_database(&database, self.owner_account_id, self.owner_device_id)?;
        } else {
            initialize_database(&database, self.owner_account_id, self.owner_device_id)?;
        }
        let locked =
            snapshot_from_database(&database, self.owner_account_id, self.owner_device_id)?;
        if locked != snapshot {
            return Err(RecipientImportConsumptionMutationAdapterError::LedgerChanged);
        }
        let nonce = binding.nonce()?;
        let mut write = database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut prepared = write.open_table(PREPARED_TABLE).map_err(database_error)?;
            if prepared
                .get(nonce.as_slice())
                .map_err(database_error)?
                .is_some()
            {
                return Err(RecipientImportConsumptionMutationAdapterError::LedgerChanged);
            }
            prepared
                .insert(nonce.as_slice(), encoded.as_slice())
                .map_err(database_error)?;
        }
        write.commit().map_err(database_error)?;
        Ok(PreparedMutation {
            durable: DurableMutationRecord {
                record_id,
                record: candidate,
            },
            authorization_id,
            consumed_at_unix_seconds: now_unix_seconds,
        })
    }

    fn complete(
        &self,
        prepared: &PreparedMutation,
        receipt: &RuntimeIpcRecipientImportConsumptionReceiptSummary,
        owner: &DeviceIdentity,
        now_unix_seconds: u64,
    ) -> Result<
        RuntimeIpcRecipientImportConsumptionReceiptSummary,
        RecipientImportConsumptionMutationAdapterError,
    > {
        if owner.device_id() != self.owner_device_id {
            return Err(RecipientImportConsumptionMutationAdapterError::OwnerDeviceMismatch);
        }
        if now_unix_seconds < prepared.consumed_at_unix_seconds {
            return Err(RecipientImportConsumptionMutationAdapterError::InvalidRecord);
        }
        let binding = &prepared.durable.record.content.binding;
        let nonce = binding.nonce()?;
        validate_receipt(receipt, binding)?;
        let state_lock = StateDirectoryLock::acquire(&self.state_directory)?;
        let snapshot = load_snapshot(
            state_lock.root(),
            self.owner_account_id,
            self.owner_device_id,
        )?;
        match snapshot.lookup(nonce, binding)? {
            MutationLookup::Completed(existing) => {
                if existing.as_ref() == receipt {
                    return Ok(*existing);
                }
                return Err(
                    RecipientImportConsumptionMutationAdapterError::CompletedReceiptChanged,
                );
            }
            MutationLookup::Prepared(current)
                if current.durable.record_id == prepared.durable.record_id => {}
            MutationLookup::Prepared(_) | MutationLookup::Absent => {
                return Err(RecipientImportConsumptionMutationAdapterError::PreparedRecordChanged);
            }
        }
        let candidate = SignedRecipientImportConsumptionMutationRecord::issue(
            self.owner_account_id,
            owner,
            binding.clone(),
            RecipientImportConsumptionMutationPhase::Completed {
                prepared_record_id: prepared.durable.record_id,
                receipt: receipt.clone(),
                completed_at_unix_seconds: now_unix_seconds,
            },
        )?;
        let encoded = candidate.encode()?;
        let path = database_path(state_lock.root());
        let database = Database::create(path).map_err(database_error)?;
        verify_database(&database, self.owner_account_id, self.owner_device_id)?;
        let locked =
            snapshot_from_database(&database, self.owner_account_id, self.owner_device_id)?;
        if locked != snapshot {
            return Err(RecipientImportConsumptionMutationAdapterError::LedgerChanged);
        }
        let mut write = database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut completed = write.open_table(COMPLETED_TABLE).map_err(database_error)?;
            if completed
                .get(nonce.as_slice())
                .map_err(database_error)?
                .is_some()
            {
                return Err(RecipientImportConsumptionMutationAdapterError::LedgerChanged);
            }
            completed
                .insert(nonce.as_slice(), encoded.as_slice())
                .map_err(database_error)?;
        }
        write.commit().map_err(database_error)?;
        Ok(receipt.clone())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipientImportConsumptionMutationReadStatus {
    Absent,
    Prepared,
    Completed(Box<RuntimeIpcRecipientImportConsumptionReceiptSummary>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientImportConsumptionMutationAdapterDispatch {
    outcome: RuntimeIpcRecipientImportConsumptionOutcome,
    replayed_completed_receipt: bool,
    reconciled_consumed_intent: bool,
    persisted_prepared_before_consume: bool,
    persisted_receipt_before_response: bool,
}

impl RecipientImportConsumptionMutationAdapterDispatch {
    pub fn outcome(&self) -> &RuntimeIpcRecipientImportConsumptionOutcome {
        &self.outcome
    }

    pub fn replayed_completed_receipt(&self) -> bool {
        self.replayed_completed_receipt
    }

    pub fn reconciled_consumed_intent(&self) -> bool {
        self.reconciled_consumed_intent
    }

    pub fn persisted_prepared_before_consume(&self) -> bool {
        self.persisted_prepared_before_consume
    }

    pub fn persisted_receipt_before_response(&self) -> bool {
        self.persisted_receipt_before_response
    }

    pub fn consumed_intent(&self) -> bool {
        self.outcome.consumed_intent()
    }

    pub fn opened_network(&self) -> bool {
        false
    }
}

pub struct RecipientHistoryRecoveryImportConsumptionMutationAdapter {
    state_directory: PathBuf,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

impl RecipientHistoryRecoveryImportConsumptionMutationAdapter {
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

    pub fn inspect_by_nonce(
        &self,
        request: &RuntimeIpcRecipientImportConsumptionRequest,
    ) -> Result<
        RecipientImportConsumptionMutationReadStatus,
        RecipientImportConsumptionMutationAdapterError,
    > {
        let (binding, _) = request_binding(request)?;
        let ledger = RecipientImportConsumptionMutationLedger::new(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        );
        Ok(match ledger.lookup(binding.nonce()?, &binding)? {
            MutationLookup::Absent => RecipientImportConsumptionMutationReadStatus::Absent,
            MutationLookup::Prepared(_) => RecipientImportConsumptionMutationReadStatus::Prepared,
            MutationLookup::Completed(receipt) => {
                RecipientImportConsumptionMutationReadStatus::Completed(receipt)
            }
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dispatch<L: RecipientImportMembershipLoader>(
        &self,
        request: &RuntimeIpcRecipientImportConsumptionRequest,
        authorizations: &RecipientImportConsumptionAuthorizationLedger,
        mutations: &RecipientHistoryRecoveryImportMutationAdapter,
        store: &RecipientRecoveryImportRoundIntentStore,
        current: &RecipientRecoveryCoordinationPlan,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        loader: &L,
        recipient: &DeviceIdentity,
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportConsumptionMutationAdapterDispatch,
        RecipientImportConsumptionMutationAdapterError,
    > {
        self.dispatch_inner(
            request,
            authorizations,
            mutations,
            store,
            current,
            inbox,
            imports,
            loader,
            recipient,
            now_unix_seconds,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn dispatch_inner<L: RecipientImportMembershipLoader>(
        &self,
        request: &RuntimeIpcRecipientImportConsumptionRequest,
        authorizations: &RecipientImportConsumptionAuthorizationLedger,
        mutations: &RecipientHistoryRecoveryImportMutationAdapter,
        store: &RecipientRecoveryImportRoundIntentStore,
        current: &RecipientRecoveryCoordinationPlan,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        loader: &L,
        recipient: &DeviceIdentity,
        now_unix_seconds: u64,
        fail_after_consume: bool,
    ) -> Result<
        RecipientImportConsumptionMutationAdapterDispatch,
        RecipientImportConsumptionMutationAdapterError,
    > {
        if recipient.device_id() != self.owner_device_id {
            return Err(RecipientImportConsumptionMutationAdapterError::OwnerDeviceMismatch);
        }
        let (binding, memberships) = match request_binding(request) {
            Ok(value) => value,
            Err(RecipientImportConsumptionMutationAdapterError::NotCommitCommand) => {
                return Ok(reject(
                    request,
                    RuntimeIpcRecipientImportConsumptionHazard::CommitRejected,
                ));
            }
            Err(error) => return Err(error),
        };
        let ledger = RecipientImportConsumptionMutationLedger::new(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        );
        let existing = match ledger.lookup(binding.nonce()?, &binding) {
            Ok(value) => value,
            Err(RecipientImportConsumptionMutationAdapterError::NonceConflict) => {
                return Ok(reject(
                    request,
                    RuntimeIpcRecipientImportConsumptionHazard::ConsumptionReplay,
                ));
            }
            Err(error) => return Err(error),
        };
        if let MutationLookup::Completed(receipt) = existing {
            return Ok(RecipientImportConsumptionMutationAdapterDispatch {
                outcome: RuntimeIpcRecipientImportConsumptionOutcome::AlreadyConsumed(*receipt),
                replayed_completed_receipt: true,
                reconciled_consumed_intent: false,
                persisted_prepared_before_consume: true,
                persisted_receipt_before_response: true,
            });
        }

        let authorization = match authorizations.load_by_consumption_nonce(binding.nonce()?)? {
            Some(value) => value,
            None => {
                return Ok(reject(
                    request,
                    RuntimeIpcRecipientImportConsumptionHazard::CommitRejected,
                ));
            }
        };
        let authorization_summary = authorization.summary()?;
        if authorization.owner_account_id() != self.owner_account_id
            || authorization.owner_device_id() != self.owner_device_id
            || authorization_summary.evidence != binding.evidence
            || authorization_summary.plan != binding.plan
            || authorization_summary.membership_set_digest.to_bytes()?
                != binding.membership_set_digest
        {
            return Ok(reject(
                request,
                RuntimeIpcRecipientImportConsumptionHazard::CommitRejected,
            ));
        }
        if now_unix_seconds < binding.plan.planned_at_unix_seconds {
            return Ok(reject(
                request,
                RuntimeIpcRecipientImportConsumptionHazard::PlanExpired,
            ));
        }

        if let Some(hazard) = exact_completed_mutation(&binding, mutations)? {
            return Ok(reject(request, hazard));
        }
        let durable = match store.load_by_intent_id(binding.evidence.intent_id.to_bytes()?)? {
            Some(value) => value,
            None => {
                return Ok(reject(
                    request,
                    RuntimeIpcRecipientImportConsumptionHazard::NoActiveIntent,
                ));
            }
        };

        if durable.consumed() {
            let MutationLookup::Prepared(prepared) = existing else {
                return Ok(reject(
                    request,
                    RuntimeIpcRecipientImportConsumptionHazard::IntentAlreadyConsumed,
                ));
            };
            if prepared.authorization_id != authorization.authorization_id() {
                return Ok(reject(
                    request,
                    RuntimeIpcRecipientImportConsumptionHazard::CommitRejected,
                ));
            }
            let receipt = receipt_from_consumed(&binding, &prepared, &durable)?;
            let persisted = ledger.complete(&prepared, &receipt, recipient, now_unix_seconds)?;
            return Ok(RecipientImportConsumptionMutationAdapterDispatch {
                outcome: RuntimeIpcRecipientImportConsumptionOutcome::AlreadyConsumed(persisted),
                replayed_completed_receipt: false,
                reconciled_consumed_intent: true,
                persisted_prepared_before_consume: true,
                persisted_receipt_before_response: true,
            });
        }

        if authorization.expired(now_unix_seconds) {
            return Ok(reject(
                request,
                RuntimeIpcRecipientImportConsumptionHazard::PlanExpired,
            ));
        }
        if current.plan_id() != binding.evidence.final_coordinator_plan_id.to_bytes()?
            || current.sequence() != binding.evidence.final_coordinator_sequence
        {
            return Ok(reject(
                request,
                RuntimeIpcRecipientImportConsumptionHazard::CoordinatorChanged,
            ));
        }
        if store.inspect_active()?.as_ref() != Some(&durable) {
            return Ok(reject(
                request,
                RuntimeIpcRecipientImportConsumptionHazard::NoActiveIntent,
            ));
        }
        let loaded = match load_exact_memberships(memberships, loader) {
            Ok(value) => value,
            Err(_) => {
                return Ok(reject(
                    request,
                    RuntimeIpcRecipientImportConsumptionHazard::CompletedMutationChanged,
                ));
            }
        };
        if loaded.len() != usize::from(binding.evidence.total_source_count) {
            return Ok(reject(
                request,
                RuntimeIpcRecipientImportConsumptionHazard::CompletedMutationChanged,
            ));
        }
        let sources = transient_sources(memberships, &loaded);
        let progress = match store.inspect_progress(&durable, current, &sources, inbox, imports) {
            Ok(value) => value,
            Err(_) => {
                return Ok(reject(
                    request,
                    RuntimeIpcRecipientImportConsumptionHazard::IntentIncomplete,
                ));
            }
        };
        if progress.state != RecipientRecoveryImportRoundIntentProgressState::Complete
            || progress.coordinator_refresh_required
            || progress.completed_source_count
                != usize::from(binding.evidence.completed_source_count)
            || progress.total_source_count != usize::from(binding.evidence.total_source_count)
            || u64::try_from(progress.imported_event_count).ok()
                != Some(binding.evidence.imported_event_count)
            || progress.coordinator_plan_id
                != binding.evidence.final_coordinator_plan_id.to_bytes()?
            || progress.coordinator_sequence != binding.evidence.final_coordinator_sequence
            || progress.contains_membership_snapshot
            || progress.contains_message_bodies
            || progress.contains_plaintext
            || progress.opened_network
            || progress.scheduled_background_work
        {
            return Ok(reject(
                request,
                RuntimeIpcRecipientImportConsumptionHazard::IntentIncomplete,
            ));
        }

        let prepared = match existing {
            MutationLookup::Prepared(prepared) => {
                if prepared.authorization_id != authorization.authorization_id() {
                    return Ok(reject(
                        request,
                        RuntimeIpcRecipientImportConsumptionHazard::CommitRejected,
                    ));
                }
                *prepared
            }
            MutationLookup::Absent => ledger.prepare(
                &binding,
                authorization.authorization_id(),
                recipient,
                now_unix_seconds,
            )?,
            MutationLookup::Completed(_) => unreachable!("completed replay returned above"),
        };
        if prepared.consumed_at_unix_seconds > binding.plan.expires_at_unix_seconds {
            return Ok(reject(
                request,
                RuntimeIpcRecipientImportConsumptionHazard::PlanExpired,
            ));
        }
        let consumed = store.consume(
            durable.intent().round_id(),
            durable.intent_id(),
            current,
            &sources,
            inbox,
            imports,
            recipient,
            prepared.consumed_at_unix_seconds,
        )?;
        let durable = match consumed {
            RecipientRecoveryImportRoundIntentConsumeOutcome::Consumed(value)
            | RecipientRecoveryImportRoundIntentConsumeOutcome::AlreadyConsumed(value) => value,
        };
        if fail_after_consume {
            return Err(RecipientImportConsumptionMutationAdapterError::InjectedAfterConsume);
        }
        let receipt = receipt_from_consumed(&binding, &prepared, &durable)?;
        let persisted = ledger.complete(&prepared, &receipt, recipient, now_unix_seconds)?;
        Ok(RecipientImportConsumptionMutationAdapterDispatch {
            outcome: RuntimeIpcRecipientImportConsumptionOutcome::Consumed(persisted),
            replayed_completed_receipt: false,
            reconciled_consumed_intent: false,
            persisted_prepared_before_consume: true,
            persisted_receipt_before_response: true,
        })
    }

    #[cfg(test)]
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn dispatch_with_fault_after_consume<L: RecipientImportMembershipLoader>(
        &self,
        request: &RuntimeIpcRecipientImportConsumptionRequest,
        authorizations: &RecipientImportConsumptionAuthorizationLedger,
        mutations: &RecipientHistoryRecoveryImportMutationAdapter,
        store: &RecipientRecoveryImportRoundIntentStore,
        current: &RecipientRecoveryCoordinationPlan,
        inbox: &RecipientHistoryTransferInbox,
        imports: &RecipientHistoryImportStore,
        loader: &L,
        recipient: &DeviceIdentity,
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportConsumptionMutationAdapterDispatch,
        RecipientImportConsumptionMutationAdapterError,
    > {
        self.dispatch_inner(
            request,
            authorizations,
            mutations,
            store,
            current,
            inbox,
            imports,
            loader,
            recipient,
            now_unix_seconds,
            true,
        )
    }
}

fn request_binding(
    request: &RuntimeIpcRecipientImportConsumptionRequest,
) -> Result<
    (
        RecipientImportConsumptionMutationBinding,
        &[kilogram_runtime_ipc::RuntimeIpcRecipientImportMembershipReference],
    ),
    RecipientImportConsumptionMutationAdapterError,
> {
    let RuntimeIpcRecipientImportConsumptionCommand::Commit {
        evidence,
        memberships,
        plan,
        confirmation_phrase,
        explicit_approval,
    } = request.command()
    else {
        return Err(RecipientImportConsumptionMutationAdapterError::NotCommitCommand);
    };
    if *explicit_approval != RuntimeIpcRecipientImportConsumptionExplicitApproval::UserConfirmed
        || confirmation_phrase != &plan.confirmation_phrase
    {
        return Err(RecipientImportConsumptionMutationAdapterError::NotCommitCommand);
    }
    let membership_set_digest = recipient_import_membership_set_digest(memberships)?.to_bytes()?;
    let binding = RecipientImportConsumptionMutationBinding {
        evidence: evidence.clone(),
        plan: plan.as_ref().clone(),
        membership_set_digest,
    };
    validate_binding(&binding)?;
    Ok((binding, memberships))
}

fn exact_completed_mutation(
    binding: &RecipientImportConsumptionMutationBinding,
    mutations: &RecipientHistoryRecoveryImportMutationAdapter,
) -> Result<
    Option<RuntimeIpcRecipientImportConsumptionHazard>,
    RecipientImportConsumptionMutationAdapterError,
> {
    let completed = match mutations.inspect_by_nonce(&binding.evidence.mutation_nonce)? {
        RecipientImportMutationReadStatus::Absent => {
            return Ok(Some(
                RuntimeIpcRecipientImportConsumptionHazard::CompletedMutationMissing,
            ));
        }
        RecipientImportMutationReadStatus::Prepared => {
            return Ok(Some(
                RuntimeIpcRecipientImportConsumptionHazard::IntentIncomplete,
            ));
        }
        RecipientImportMutationReadStatus::Completed(value) => value,
    };
    let exact = RuntimeIpcRecipientImportCompletedMutationEvidence::from_completed_receipt(
        completed.completed_record_id().clone(),
        completed.membership_set_digest().clone(),
        completed.receipt(),
    )?;
    Ok((exact != binding.evidence)
        .then_some(RuntimeIpcRecipientImportConsumptionHazard::CompletedMutationChanged))
}

fn receipt_from_consumed(
    binding: &RecipientImportConsumptionMutationBinding,
    prepared: &PreparedMutation,
    durable: &DurableRecipientRecoveryImportRoundIntent,
) -> Result<
    RuntimeIpcRecipientImportConsumptionReceiptSummary,
    RecipientImportConsumptionMutationAdapterError,
> {
    let consumption = durable
        .consumption()
        .ok_or(RecipientImportConsumptionMutationAdapterError::MissingConsumption)?;
    if durable.intent_id() != binding.evidence.intent_id.to_bytes()?
        || consumption.consumed_at_unix_seconds() != prepared.consumed_at_unix_seconds
        || consumption.final_coordinator_plan_id()
            != binding.evidence.final_coordinator_plan_id.to_bytes()?
        || consumption.final_coordinator_sequence() != binding.evidence.final_coordinator_sequence
        || consumption.completed_source_count()
            != usize::from(binding.evidence.completed_source_count)
        || consumption.imported_event_count() != binding.evidence.imported_event_count
    {
        return Err(RecipientImportConsumptionMutationAdapterError::ConsumedIntentChanged);
    }
    let receipt = RuntimeIpcRecipientImportConsumptionReceiptSummary {
        intent_id: binding.plan.intent_id.clone(),
        consumption_plan_id: binding.plan.consumption_plan_id.clone(),
        evidence_digest: binding.plan.evidence_digest.clone(),
        consumption_nonce: binding.plan.consumption_nonce.clone(),
        consumption_id: artifact(consumption.consumption_id(durable.intent())?)?,
        final_coordinator_plan_id: binding.plan.final_coordinator_plan_id.clone(),
        final_coordinator_sequence: binding.plan.final_coordinator_sequence,
        consumed_at_unix_seconds: consumption.consumed_at_unix_seconds(),
        active_intent_cleared: true,
    };
    validate_receipt(&receipt, binding)?;
    Ok(receipt)
}

fn validate_binding(
    binding: &RecipientImportConsumptionMutationBinding,
) -> Result<(), RecipientImportConsumptionMutationAdapterError> {
    let lifetime = binding
        .plan
        .expires_at_unix_seconds
        .checked_sub(binding.plan.planned_at_unix_seconds)
        .ok_or(RecipientImportConsumptionMutationAdapterError::InvalidRecord)?;
    let reconstructed = RuntimeIpcRecipientImportConsumptionPlanSummary::issue(
        &binding.evidence,
        binding.plan.consumption_nonce.clone(),
        binding.plan.planned_at_unix_seconds,
        lifetime,
    )?;
    if reconstructed != binding.plan
        || binding.membership_set_digest == [0_u8; 32]
        || binding.membership_set_digest != binding.evidence.membership_set_digest.to_bytes()?
    {
        return Err(RecipientImportConsumptionMutationAdapterError::InvalidRecord);
    }
    Ok(())
}

fn validate_receipt(
    receipt: &RuntimeIpcRecipientImportConsumptionReceiptSummary,
    binding: &RecipientImportConsumptionMutationBinding,
) -> Result<(), RecipientImportConsumptionMutationAdapterError> {
    for artifact in [
        &receipt.intent_id,
        &receipt.consumption_plan_id,
        &receipt.evidence_digest,
        &receipt.consumption_nonce,
        &receipt.consumption_id,
        &receipt.final_coordinator_plan_id,
    ] {
        artifact.to_bytes()?;
    }
    if receipt.intent_id != binding.plan.intent_id
        || receipt.consumption_plan_id != binding.plan.consumption_plan_id
        || receipt.evidence_digest != binding.plan.evidence_digest
        || receipt.consumption_nonce != binding.plan.consumption_nonce
        || receipt.final_coordinator_plan_id != binding.plan.final_coordinator_plan_id
        || receipt.final_coordinator_sequence != binding.plan.final_coordinator_sequence
        || receipt.consumption_id == receipt.consumption_nonce
        || !(binding.plan.planned_at_unix_seconds..=binding.plan.expires_at_unix_seconds)
            .contains(&receipt.consumed_at_unix_seconds)
        || !receipt.active_intent_cleared
    {
        return Err(RecipientImportConsumptionMutationAdapterError::InvalidReceipt);
    }
    Ok(())
}

fn validate_record_content(
    content: &RecipientImportConsumptionMutationRecordContent,
) -> Result<(), RecipientImportConsumptionMutationAdapterError> {
    if content.version != RECORD_VERSION {
        return Err(RecipientImportConsumptionMutationAdapterError::InvalidRecord);
    }
    validate_binding(&content.binding)?;
    match &content.phase {
        RecipientImportConsumptionMutationPhase::Prepared {
            authorization_id,
            consumed_at_unix_seconds,
            prepared_at_unix_seconds,
        } => {
            if *authorization_id == [0_u8; 32]
                || consumed_at_unix_seconds != prepared_at_unix_seconds
                || !(content.binding.plan.planned_at_unix_seconds
                    ..=content.binding.plan.expires_at_unix_seconds)
                    .contains(consumed_at_unix_seconds)
            {
                return Err(RecipientImportConsumptionMutationAdapterError::InvalidRecord);
            }
        }
        RecipientImportConsumptionMutationPhase::Completed {
            prepared_record_id,
            receipt,
            completed_at_unix_seconds,
        } => {
            if *prepared_record_id == [0_u8; 32] || *completed_at_unix_seconds == 0 {
                return Err(RecipientImportConsumptionMutationAdapterError::InvalidRecord);
            }
            validate_receipt(receipt, &content.binding)?;
        }
    }
    Ok(())
}

fn reject(
    request: &RuntimeIpcRecipientImportConsumptionRequest,
    hazard: RuntimeIpcRecipientImportConsumptionHazard,
) -> RecipientImportConsumptionMutationAdapterDispatch {
    RecipientImportConsumptionMutationAdapterDispatch {
        outcome: RuntimeIpcRecipientImportConsumptionOutcome::Rejected {
            intent_id: request.command().intent_id().clone(),
            hazard,
        },
        replayed_completed_receipt: false,
        reconciled_consumed_intent: false,
        persisted_prepared_before_consume: false,
        persisted_receipt_before_response: false,
    }
}

fn record_signing_bytes(
    content: &RecipientImportConsumptionMutationRecordContent,
) -> Result<Vec<u8>, RecipientImportConsumptionMutationAdapterError> {
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
) -> Result<LedgerSnapshot, RecipientImportConsumptionMutationAdapterError> {
    reject_ledger_path_symlinks(state_directory)?;
    let path = database_path(state_directory);
    if !path.is_file() {
        return Ok(LedgerSnapshot::empty());
    }
    let database = ReadOnlyDatabase::open(path).map_err(database_error)?;
    snapshot_from_database(&database, owner_account_id, owner_device_id)
}

fn snapshot_from_database<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<LedgerSnapshot, RecipientImportConsumptionMutationAdapterError> {
    verify_database_metadata(database, owner_account_id, owner_device_id)?;
    let read = database.begin_read().map_err(database_error)?;
    let prepared_table = read.open_table(PREPARED_TABLE).map_err(database_error)?;
    let completed_table = read.open_table(COMPLETED_TABLE).map_err(database_error)?;
    if prepared_table.len().map_err(database_error)?
        > MAX_DURABLE_RECIPIENT_IMPORT_CONSUMPTION_MUTATIONS
        || completed_table.len().map_err(database_error)?
            > MAX_DURABLE_RECIPIENT_IMPORT_CONSUMPTION_MUTATIONS
    {
        return Err(RecipientImportConsumptionMutationAdapterError::Capacity);
    }
    let mut prepared = BTreeMap::new();
    for row in prepared_table.iter().map_err(database_error)? {
        let (stored_nonce, encoded) = row.map_err(database_error)?;
        let nonce = copy_id(stored_nonce.value())?;
        let record = SignedRecipientImportConsumptionMutationRecord::decode_and_verify(
            encoded.value(),
            owner_account_id,
            owner_device_id,
        )?;
        if record.content.binding.nonce()? != nonce
            || !matches!(
                record.content.phase,
                RecipientImportConsumptionMutationPhase::Prepared { .. }
            )
        {
            return Err(RecipientImportConsumptionMutationAdapterError::NonceIndexMismatch);
        }
        let record_id = record.record_id()?;
        if prepared
            .insert(nonce, DurableMutationRecord { record_id, record })
            .is_some()
        {
            return Err(RecipientImportConsumptionMutationAdapterError::LedgerFork);
        }
    }
    let mut completed = BTreeMap::new();
    for row in completed_table.iter().map_err(database_error)? {
        let (stored_nonce, encoded) = row.map_err(database_error)?;
        let nonce = copy_id(stored_nonce.value())?;
        let record = SignedRecipientImportConsumptionMutationRecord::decode_and_verify(
            encoded.value(),
            owner_account_id,
            owner_device_id,
        )?;
        if record.content.binding.nonce()? != nonce
            || !matches!(
                record.content.phase,
                RecipientImportConsumptionMutationPhase::Completed { .. }
            )
        {
            return Err(RecipientImportConsumptionMutationAdapterError::NonceIndexMismatch);
        }
        let record_id = record.record_id()?;
        if completed
            .insert(nonce, DurableMutationRecord { record_id, record })
            .is_some()
        {
            return Err(RecipientImportConsumptionMutationAdapterError::LedgerFork);
        }
    }
    drop(completed_table);
    drop(prepared_table);
    drop(read);
    if completed.keys().any(|nonce| !prepared.contains_key(nonce))
        || prepared
            .keys()
            .filter(|nonce| !completed.contains_key(*nonce))
            .count()
            > 1
    {
        return Err(RecipientImportConsumptionMutationAdapterError::LedgerFork);
    }
    for (nonce, completed_record) in &completed {
        let prepared_record = prepared
            .get(nonce)
            .ok_or(RecipientImportConsumptionMutationAdapterError::MissingPreparedRecord)?;
        let RecipientImportConsumptionMutationPhase::Completed {
            prepared_record_id, ..
        } = completed_record.record.content.phase
        else {
            return Err(RecipientImportConsumptionMutationAdapterError::InvalidRecord);
        };
        if prepared_record_id != prepared_record.record_id
            || completed_record.record.content.binding != prepared_record.record.content.binding
        {
            return Err(RecipientImportConsumptionMutationAdapterError::PreparedRecordChanged);
        }
    }
    Ok(LedgerSnapshot {
        prepared,
        completed,
    })
}

fn initialize_database(
    database: &Database,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), RecipientImportConsumptionMutationAdapterError> {
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
    write.open_table(PREPARED_TABLE).map_err(database_error)?;
    write.open_table(COMPLETED_TABLE).map_err(database_error)?;
    write.commit().map_err(database_error)
}

fn verify_database<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), RecipientImportConsumptionMutationAdapterError> {
    snapshot_from_database(database, owner_account_id, owner_device_id).map(|_| ())
}

fn verify_database_metadata<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), RecipientImportConsumptionMutationAdapterError> {
    let read = database.begin_read().map_err(database_error)?;
    let meta = read.open_table(META_TABLE).map_err(database_error)?;
    verify_meta(
        &meta,
        SCHEMA_KEY,
        &[SCHEMA_VERSION],
        RecipientImportConsumptionMutationAdapterError::SchemaMismatch,
    )?;
    verify_meta(
        &meta,
        OWNER_ACCOUNT_KEY,
        owner_account_id.as_bytes(),
        RecipientImportConsumptionMutationAdapterError::OwnerAccountMismatch,
    )?;
    verify_meta(
        &meta,
        OWNER_DEVICE_KEY,
        owner_device_id.as_bytes(),
        RecipientImportConsumptionMutationAdapterError::OwnerDeviceMismatch,
    )?;
    Ok(())
}

fn verify_meta(
    meta: &impl ReadableTable<&'static str, &'static [u8]>,
    key: &'static str,
    expected: &[u8],
    mismatch: RecipientImportConsumptionMutationAdapterError,
) -> Result<(), RecipientImportConsumptionMutationAdapterError> {
    let value = meta.get(key).map_err(database_error)?.ok_or(mismatch)?;
    if value.value() != expected {
        return Err(match key {
            SCHEMA_KEY => RecipientImportConsumptionMutationAdapterError::SchemaMismatch,
            OWNER_ACCOUNT_KEY => {
                RecipientImportConsumptionMutationAdapterError::OwnerAccountMismatch
            }
            _ => RecipientImportConsumptionMutationAdapterError::OwnerDeviceMismatch,
        });
    }
    Ok(())
}

fn prepare_database_path(
    root: &Path,
) -> Result<PathBuf, RecipientImportConsumptionMutationAdapterError> {
    reject_ledger_path_symlinks(root)?;
    let directory = root.join(LEDGER_DIRECTORY);
    fs::create_dir_all(&directory)?;
    reject_ledger_path_symlinks(root)?;
    let canonical_directory = fs::canonicalize(&directory)?;
    if !canonical_directory.starts_with(root) {
        return Err(RecipientImportConsumptionMutationAdapterError::UnsafePath);
    }
    Ok(canonical_directory.join(DATABASE_FILE))
}

fn reject_ledger_path_symlinks(
    root: &Path,
) -> Result<(), RecipientImportConsumptionMutationAdapterError> {
    for path in [root.join(LEDGER_DIRECTORY), database_path(root)] {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(RecipientImportConsumptionMutationAdapterError::UnsafePath);
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

fn validate_record_length(
    encoded: &[u8],
) -> Result<(), RecipientImportConsumptionMutationAdapterError> {
    if encoded.is_empty() || encoded.len() > MAX_RECORD_BYTES {
        return Err(RecipientImportConsumptionMutationAdapterError::RecordLength(encoded.len()));
    }
    Ok(())
}

fn copy_id(bytes: &[u8]) -> Result<[u8; 32], RecipientImportConsumptionMutationAdapterError> {
    bytes
        .try_into()
        .map_err(|_| RecipientImportConsumptionMutationAdapterError::StoredIdLength(bytes.len()))
}

fn artifact(
    bytes: [u8; 32],
) -> Result<RuntimeIpcRecipientImportArtifactId, RecipientImportConsumptionMutationAdapterError> {
    RuntimeIpcRecipientImportArtifactId::from_bytes(bytes)
        .map_err(RecipientImportConsumptionMutationAdapterError::Contract)
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

fn database_error(error: impl std::fmt::Display) -> RecipientImportConsumptionMutationAdapterError {
    RecipientImportConsumptionMutationAdapterError::Database(error.to_string())
}

#[derive(Debug, Error)]
pub enum RecipientImportConsumptionMutationAdapterError {
    #[error("recipient import consumption request is not a commit command")]
    NotCommitCommand,
    #[error("recipient import consumption mutation owner Account does not match")]
    OwnerAccountMismatch,
    #[error("recipient import consumption mutation owner Device does not match")]
    OwnerDeviceMismatch,
    #[error("recipient import consumption mutation authorization expired")]
    AuthorizationExpired,
    #[error("recipient import consumption mutation nonce is bound to another request")]
    NonceConflict,
    #[error("another recipient import consumption mutation is in flight")]
    AnotherMutationInFlight,
    #[error("recipient import consumption mutation is already completed")]
    AlreadyCompleted,
    #[error("recipient import consumption mutation prepared record is missing")]
    MissingPreparedRecord,
    #[error("recipient import consumption mutation prepared record changed")]
    PreparedRecordChanged,
    #[error("recipient import consumption mutation receipt changed")]
    CompletedReceiptChanged,
    #[error("recipient import consumption mutation durable consumption is missing")]
    MissingConsumption,
    #[error("recipient import consumption mutation durable consumption changed")]
    ConsumedIntentChanged,
    #[error("recipient import consumption mutation receipt is invalid")]
    InvalidReceipt,
    #[error("recipient import consumption mutation durable ledger changed")]
    LedgerChanged,
    #[error("recipient import consumption mutation durable ledger forked")]
    LedgerFork,
    #[error("recipient import consumption mutation schema is invalid")]
    SchemaMismatch,
    #[error("recipient import consumption mutation record is invalid")]
    InvalidRecord,
    #[error("recipient import consumption mutation record is not canonical")]
    NonCanonicalRecord,
    #[error("recipient import consumption mutation nonce index is inconsistent")]
    NonceIndexMismatch,
    #[error("recipient import consumption mutation capacity is exhausted")]
    Capacity,
    #[error("recipient import consumption mutation path is unsafe")]
    UnsafePath,
    #[error("recipient import consumption mutation record length {0} is invalid")]
    RecordLength(usize),
    #[error("recipient import consumption mutation stored ID length {0} is invalid")]
    StoredIdLength(usize),
    #[error("recipient import consumption mutation database failed: {0}")]
    Database(String),
    #[error("injected failure after durable intent consumption")]
    InjectedAfterConsume,
    #[error(transparent)]
    Authorization(#[from] RecipientImportConsumptionAuthorizationLedgerError),
    #[error(transparent)]
    Mutation(#[from] RecipientImportMutationAdapterError),
    #[error(transparent)]
    Intent(Box<RecipientRecoveryImportRoundIntentError>),
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

impl From<RecipientRecoveryImportRoundIntentError>
    for RecipientImportConsumptionMutationAdapterError
{
    fn from(error: RecipientRecoveryImportRoundIntentError) -> Self {
        Self::Intent(Box::new(error))
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;

    struct Fixture {
        root: tempfile::TempDir,
        owner: DeviceIdentity,
        account_id: AccountId,
        binding: RecipientImportConsumptionMutationBinding,
        receipt: RuntimeIpcRecipientImportConsumptionReceiptSummary,
    }

    fn fixture() -> Result<Fixture, Box<dyn Error>> {
        let root = tempfile::tempdir()?;
        let owner = DeviceIdentity::generate()?;
        let account_id = AccountId::from_bytes([31_u8; 32]);
        let evidence = RuntimeIpcRecipientImportCompletedMutationEvidence {
            intent_id: artifact([1_u8; 32])?,
            execution_plan_id: artifact([2_u8; 32])?,
            confirmation_id: artifact([3_u8; 32])?,
            mutation_nonce: artifact([4_u8; 32])?,
            completed_mutation_record_id: artifact([5_u8; 32])?,
            completed_mutation_receipt_digest: artifact([6_u8; 32])?,
            membership_set_digest: artifact([7_u8; 32])?,
            final_coordinator_plan_id: artifact([8_u8; 32])?,
            final_coordinator_sequence: 9,
            completed_source_count: 1,
            total_source_count: 1,
            imported_event_count: 10,
        };
        let plan = RuntimeIpcRecipientImportConsumptionPlanSummary::issue(
            &evidence,
            artifact([11_u8; 32])?,
            1_000,
            120,
        )?;
        let binding = RecipientImportConsumptionMutationBinding {
            evidence,
            plan: plan.clone(),
            membership_set_digest: [7_u8; 32],
        };
        let receipt = RuntimeIpcRecipientImportConsumptionReceiptSummary {
            intent_id: plan.intent_id.clone(),
            consumption_plan_id: plan.consumption_plan_id.clone(),
            evidence_digest: plan.evidence_digest.clone(),
            consumption_nonce: plan.consumption_nonce.clone(),
            consumption_id: artifact([12_u8; 32])?,
            final_coordinator_plan_id: plan.final_coordinator_plan_id.clone(),
            final_coordinator_sequence: plan.final_coordinator_sequence,
            consumed_at_unix_seconds: 1_010,
            active_intent_cleared: true,
        };
        Ok(Fixture {
            root,
            owner,
            account_id,
            binding,
            receipt,
        })
    }

    fn ledger(fixture: &Fixture) -> RecipientImportConsumptionMutationLedger {
        RecipientImportConsumptionMutationLedger::new(
            fixture.root.path(),
            fixture.account_id,
            fixture.owner.device_id(),
        )
    }

    #[test]
    fn recipient_import_consumption_mutation_ledger_survives_restart_and_replays()
    -> Result<(), Box<dyn Error>> {
        let fixture = fixture()?;
        let mutation_ledger = ledger(&fixture);
        assert!(matches!(
            mutation_ledger.lookup(fixture.binding.nonce()?, &fixture.binding)?,
            MutationLookup::Absent
        ));
        let prepared =
            mutation_ledger.prepare(&fixture.binding, [13_u8; 32], &fixture.owner, 1_010)?;
        let replayed =
            mutation_ledger.prepare(&fixture.binding, [13_u8; 32], &fixture.owner, 1_010)?;
        assert_eq!(prepared, replayed);
        assert!(matches!(
            mutation_ledger.prepare(&fixture.binding, [14_u8; 32], &fixture.owner, 1_010),
            Err(RecipientImportConsumptionMutationAdapterError::PreparedRecordChanged)
        ));
        assert!(matches!(
            mutation_ledger.complete(&prepared, &fixture.receipt, &fixture.owner, 1_009),
            Err(RecipientImportConsumptionMutationAdapterError::InvalidRecord)
        ));
        mutation_ledger.complete(&prepared, &fixture.receipt, &fixture.owner, 1_011)?;
        let reopened = ledger(&fixture);
        assert!(matches!(
            reopened.lookup(fixture.binding.nonce()?, &fixture.binding)?,
            MutationLookup::Completed(receipt) if *receipt == fixture.receipt
        ));
        assert_eq!(
            reopened.complete(&prepared, &fixture.receipt, &fixture.owner, 1_012)?,
            fixture.receipt
        );
        Ok(())
    }

    #[test]
    fn recipient_import_consumption_mutation_ledger_rejects_nonce_rebinding_and_wrong_owner()
    -> Result<(), Box<dyn Error>> {
        let fixture = fixture()?;
        let mutation_ledger = ledger(&fixture);
        mutation_ledger.prepare(&fixture.binding, [13_u8; 32], &fixture.owner, 1_010)?;
        let mut changed = fixture.binding.clone();
        changed.evidence.final_coordinator_sequence = 10;
        changed.plan = RuntimeIpcRecipientImportConsumptionPlanSummary::issue(
            &changed.evidence,
            fixture.binding.plan.consumption_nonce.clone(),
            1_000,
            120,
        )?;
        assert!(matches!(
            mutation_ledger.lookup(changed.nonce()?, &changed),
            Err(RecipientImportConsumptionMutationAdapterError::NonceConflict)
        ));
        let wrong_owner = DeviceIdentity::generate()?;
        assert!(matches!(
            RecipientImportConsumptionMutationLedger::new(
                fixture.root.path().join("wrong-owner"),
                fixture.account_id,
                fixture.owner.device_id(),
            )
            .prepare(&fixture.binding, [13_u8; 32], &wrong_owner, 1_010),
            Err(RecipientImportConsumptionMutationAdapterError::OwnerDeviceMismatch)
        ));
        Ok(())
    }
}
