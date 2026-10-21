//! Durable, Device-signed consumption authorization ledger for one exact recipient import plan.
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
    MAX_RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_PLAN_LIFETIME_SECONDS,
    RuntimeIpcRecipientImportArtifactId, RuntimeIpcRecipientImportCompletedMutationEvidence,
    RuntimeIpcRecipientImportConsumptionCommand,
    RuntimeIpcRecipientImportConsumptionExplicitApproval,
    RuntimeIpcRecipientImportConsumptionPlanSummary, RuntimeIpcRecipientImportConsumptionRequest,
    recipient_import_consumption_confirmation_phrase, recipient_import_membership_set_digest,
};
use kilogram_state::{StateDirectoryLock, StateError};
use redb::{
    Database, Durability, ReadOnlyDatabase, ReadableDatabase, ReadableTable, ReadableTableMetadata,
    TableDefinition,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::RecipientImportConsumptionReadAdapterDispatch;

const LEDGER_DIRECTORY: &str = "history-recovery";
const DATABASE_FILE: &str = "recipient-import-consumption-authorizations-v1.redb";
const META_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("recipient-import-consumption-authorization-meta-v1");
const RECORD_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("recipient-import-consumption-authorization-record-v1");
const NONCE_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("recipient-import-consumption-authorization-nonce-v1");
const HEAD_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("recipient-import-consumption-authorization-head-v1");

const SCHEMA_KEY: &str = "schema-version";
const OWNER_ACCOUNT_KEY: &str = "owner-account-id";
const OWNER_DEVICE_KEY: &str = "owner-device-id";
const HEAD_KEY: &str = "head";
const SCHEMA_VERSION: u8 = 1;
const RECORD_VERSION: u8 = 1;
const HEAD_VERSION: u8 = 1;
const RECORD_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:recipient-import-consumption-authorization-record:v1\0";
const AUTHORIZATION_ID_DOMAIN: &[u8] =
    b"kilogram:recipient-import-consumption-authorization-id:v1\0";

const MAX_RECORD_BYTES: usize = 4 * 1024;
const MAX_HEAD_BYTES: usize = 128;
pub const MAX_DURABLE_RECIPIENT_IMPORT_CONSUMPTION_AUTHORIZATIONS: u64 = 4_096;
pub const MAX_RECIPIENT_IMPORT_CONSUMPTION_AUTHORIZATION_LIFETIME_SECONDS: u64 =
    MAX_RUNTIME_IPC_RECIPIENT_IMPORT_CONSUMPTION_PLAN_LIFETIME_SECONDS;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RecipientImportConsumptionAuthorizationBinding {
    evidence: RuntimeIpcRecipientImportCompletedMutationEvidence,
    plan: RuntimeIpcRecipientImportConsumptionPlanSummary,
    membership_set_digest: [u8; 32],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RecipientImportConsumptionAuthorizationRecordContent {
    version: u8,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    sequence: u64,
    previous_authorization_id: Option<[u8; 32]>,
    binding: RecipientImportConsumptionAuthorizationBinding,
    consumption_nonce: [u8; 32],
    issued_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SignedRecipientImportConsumptionAuthorizationRecordWire {
    content: RecipientImportConsumptionAuthorizationRecordContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SignedRecipientImportConsumptionAuthorizationRecord {
    content: RecipientImportConsumptionAuthorizationRecordContent,
    signature: Vec<u8>,
}

impl SignedRecipientImportConsumptionAuthorizationRecord {
    #[allow(clippy::too_many_arguments)]
    fn issue(
        owner_account_id: AccountId,
        owner: &DeviceIdentity,
        sequence: u64,
        previous_authorization_id: Option<[u8; 32]>,
        binding: RecipientImportConsumptionAuthorizationBinding,
        consumption_nonce: [u8; 32],
        issued_at_unix_seconds: u64,
        expires_at_unix_seconds: u64,
    ) -> Result<Self, RecipientImportConsumptionAuthorizationLedgerError> {
        let content = RecipientImportConsumptionAuthorizationRecordContent {
            version: RECORD_VERSION,
            owner_account_id,
            owner_device_id: owner.device_id(),
            sequence,
            previous_authorization_id,
            binding,
            consumption_nonce,
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
    ) -> Result<Self, RecipientImportConsumptionAuthorizationLedgerError> {
        validate_record_length(encoded)?;
        let wire: SignedRecipientImportConsumptionAuthorizationRecordWire =
            postcard::from_bytes(encoded)?;
        let record = Self {
            content: wire.content,
            signature: wire.signature,
        };
        record.verify()?;
        if record.content.owner_account_id != owner_account_id {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::OwnerAccountMismatch);
        }
        if record.content.owner_device_id != owner_device_id {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::OwnerDeviceMismatch);
        }
        if record.encode()? != encoded {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::NonCanonicalRecord);
        }
        Ok(record)
    }

    fn verify(&self) -> Result<(), RecipientImportConsumptionAuthorizationLedgerError> {
        validate_record_content(&self.content)?;
        self.content
            .owner_device_id
            .verify(&record_signing_bytes(&self.content)?, &self.signature)?;
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>, RecipientImportConsumptionAuthorizationLedgerError> {
        self.verify()?;
        let encoded =
            postcard::to_allocvec(&SignedRecipientImportConsumptionAuthorizationRecordWire {
                content: self.content.clone(),
                signature: self.signature.clone(),
            })?;
        validate_record_length(&encoded)?;
        Ok(encoded)
    }

    fn authorization_id(
        &self,
    ) -> Result<[u8; 32], RecipientImportConsumptionAuthorizationLedgerError> {
        Ok(domain_digest(AUTHORIZATION_ID_DOMAIN, &self.encode()?))
    }

    fn matches_binding(
        &self,
        binding: &RecipientImportConsumptionAuthorizationBinding,
        consumption_nonce: [u8; 32],
    ) -> bool {
        self.content.binding == *binding && self.content.consumption_nonce == consumption_nonce
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RecipientImportConsumptionAuthorizationHeadWire {
    version: u8,
    sequence: u64,
    authorization_id: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableRecipientImportConsumptionAuthorization {
    authorization_id: [u8; 32],
    record: SignedRecipientImportConsumptionAuthorizationRecord,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientImportConsumptionAuthorizationSummary {
    pub authorization_id: RuntimeIpcRecipientImportArtifactId,
    pub sequence: u64,
    pub evidence: RuntimeIpcRecipientImportCompletedMutationEvidence,
    pub plan: RuntimeIpcRecipientImportConsumptionPlanSummary,
    pub membership_set_digest: RuntimeIpcRecipientImportArtifactId,
}

impl DurableRecipientImportConsumptionAuthorization {
    pub fn authorization_id(&self) -> [u8; 32] {
        self.authorization_id
    }

    pub fn consumption_nonce(&self) -> [u8; 32] {
        self.record.content.consumption_nonce
    }

    pub fn sequence(&self) -> u64 {
        self.record.content.sequence
    }

    pub fn owner_account_id(&self) -> AccountId {
        self.record.content.owner_account_id
    }

    pub fn owner_device_id(&self) -> DeviceId {
        self.record.content.owner_device_id
    }

    pub fn expired(&self, now_unix_seconds: u64) -> bool {
        now_unix_seconds > self.record.content.expires_at_unix_seconds
    }

    pub fn summary(
        &self,
    ) -> Result<
        RecipientImportConsumptionAuthorizationSummary,
        RecipientImportConsumptionAuthorizationLedgerError,
    > {
        summary_from_record(&self.record, self.authorization_id)
    }

    pub fn contains_membership_snapshot(&self) -> bool {
        false
    }

    pub fn grants_import_authority(&self) -> bool {
        false
    }

    pub fn grants_consumption_authority(&self) -> bool {
        true
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RecipientImportConsumptionAuthorizationPlanRecord {
    Existing(DurableRecipientImportConsumptionAuthorization),
    Candidate(SignedRecipientImportConsumptionAuthorizationRecord),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientImportConsumptionAuthorizationPlan {
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    binding: RecipientImportConsumptionAuthorizationBinding,
    consumption_nonce: [u8; 32],
    record: RecipientImportConsumptionAuthorizationPlanRecord,
}

impl RecipientImportConsumptionAuthorizationPlan {
    pub fn summary(
        &self,
    ) -> Result<
        RecipientImportConsumptionAuthorizationSummary,
        RecipientImportConsumptionAuthorizationLedgerError,
    > {
        match &self.record {
            RecipientImportConsumptionAuthorizationPlanRecord::Existing(durable) => {
                durable.summary()
            }
            RecipientImportConsumptionAuthorizationPlanRecord::Candidate(record) => {
                summary_from_record(record, record.authorization_id()?)
            }
        }
    }

    pub fn already_committed(&self) -> bool {
        matches!(
            self.record,
            RecipientImportConsumptionAuthorizationPlanRecord::Existing(_)
        )
    }

    pub fn persists_during_planning(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipientImportConsumptionAuthorizationStatus {
    Absent,
    Committed {
        summary: Box<RecipientImportConsumptionAuthorizationSummary>,
        expired: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecipientImportConsumptionAuthorizationCommitOutcome {
    Committed(DurableRecipientImportConsumptionAuthorization),
    AlreadyCommitted(DurableRecipientImportConsumptionAuthorization),
}

pub struct RecipientImportConsumptionAuthorizationLedger {
    state_directory: PathBuf,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

impl RecipientImportConsumptionAuthorizationLedger {
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

    pub fn plan(
        &self,
        dispatch: &RecipientImportConsumptionReadAdapterDispatch,
        request: &RuntimeIpcRecipientImportConsumptionRequest,
        owner: &DeviceIdentity,
    ) -> Result<
        RecipientImportConsumptionAuthorizationPlan,
        RecipientImportConsumptionAuthorizationLedgerError,
    > {
        if owner.device_id() != self.owner_device_id {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::OwnerDeviceMismatch);
        }
        let (binding, consumption_nonce, issued_at_unix_seconds, expires_at_unix_seconds) =
            exact_authorization_binding(dispatch, request)?;
        let snapshot = load_snapshot(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        )?;
        if let Some(existing) = snapshot.by_nonce(consumption_nonce)? {
            if !existing.record.matches_binding(&binding, consumption_nonce) {
                return Err(RecipientImportConsumptionAuthorizationLedgerError::NonceConflict);
            }
            return Ok(RecipientImportConsumptionAuthorizationPlan {
                owner_account_id: self.owner_account_id,
                owner_device_id: self.owner_device_id,
                binding,
                consumption_nonce,
                record: RecipientImportConsumptionAuthorizationPlanRecord::Existing(existing),
            });
        }
        let sequence = snapshot.head.as_ref().map_or(Ok(1_u64), |head| {
            head.sequence
                .checked_add(1)
                .ok_or(RecipientImportConsumptionAuthorizationLedgerError::SequenceExhausted)
        })?;
        let previous_authorization_id = snapshot.head.as_ref().map(|head| head.authorization_id);
        let candidate = SignedRecipientImportConsumptionAuthorizationRecord::issue(
            self.owner_account_id,
            owner,
            sequence,
            previous_authorization_id,
            binding.clone(),
            consumption_nonce,
            issued_at_unix_seconds,
            expires_at_unix_seconds,
        )?;
        Ok(RecipientImportConsumptionAuthorizationPlan {
            owner_account_id: self.owner_account_id,
            owner_device_id: self.owner_device_id,
            binding,
            consumption_nonce,
            record: RecipientImportConsumptionAuthorizationPlanRecord::Candidate(candidate),
        })
    }

    pub fn commit(
        &self,
        plan: &RecipientImportConsumptionAuthorizationPlan,
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportConsumptionAuthorizationCommitOutcome,
        RecipientImportConsumptionAuthorizationLedgerError,
    > {
        self.commit_inner(plan, now_unix_seconds, false)
    }

    fn commit_inner(
        &self,
        plan: &RecipientImportConsumptionAuthorizationPlan,
        now_unix_seconds: u64,
        fail_before_commit: bool,
    ) -> Result<
        RecipientImportConsumptionAuthorizationCommitOutcome,
        RecipientImportConsumptionAuthorizationLedgerError,
    > {
        if plan.owner_account_id != self.owner_account_id {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::OwnerAccountMismatch);
        }
        if plan.owner_device_id != self.owner_device_id {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::OwnerDeviceMismatch);
        }
        let state_lock = StateDirectoryLock::acquire(&self.state_directory)?;
        let snapshot = load_snapshot(
            state_lock.root(),
            self.owner_account_id,
            self.owner_device_id,
        )?;
        if let Some(existing) = snapshot.by_nonce(plan.consumption_nonce)? {
            if existing
                .record
                .matches_binding(&plan.binding, plan.consumption_nonce)
            {
                return Ok(
                    RecipientImportConsumptionAuthorizationCommitOutcome::AlreadyCommitted(
                        existing,
                    ),
                );
            }
            return Err(RecipientImportConsumptionAuthorizationLedgerError::NonceConflict);
        }

        let RecipientImportConsumptionAuthorizationPlanRecord::Candidate(candidate) = &plan.record
        else {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::LedgerRollback);
        };
        candidate.verify()?;
        if candidate.content.owner_account_id != self.owner_account_id {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::OwnerAccountMismatch);
        }
        if candidate.content.owner_device_id != self.owner_device_id {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::OwnerDeviceMismatch);
        }
        if !candidate.matches_binding(&plan.binding, plan.consumption_nonce) {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::PlanChanged);
        }
        if now_unix_seconds < candidate.content.issued_at_unix_seconds
            || now_unix_seconds > candidate.content.expires_at_unix_seconds
        {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::AuthorizationExpired);
        }
        let expected_sequence = snapshot.head.as_ref().map_or(Ok(1_u64), |head| {
            head.sequence
                .checked_add(1)
                .ok_or(RecipientImportConsumptionAuthorizationLedgerError::SequenceExhausted)
        })?;
        let expected_previous = snapshot.head.as_ref().map(|head| head.authorization_id);
        if candidate.content.sequence != expected_sequence
            || candidate.content.previous_authorization_id != expected_previous
        {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::LedgerChanged);
        }
        if snapshot.records.len() as u64 >= MAX_DURABLE_RECIPIENT_IMPORT_CONSUMPTION_AUTHORIZATIONS
        {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::Capacity);
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
            return Err(RecipientImportConsumptionAuthorizationLedgerError::LedgerChanged);
        }

        let authorization_id = candidate.authorization_id()?;
        if authorization_id == plan.consumption_nonce {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::InvalidRecord);
        }
        let record_bytes = candidate.encode()?;
        let head = RecipientImportConsumptionAuthorizationHeadWire {
            version: HEAD_VERSION,
            sequence: candidate.content.sequence,
            authorization_id,
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
                .get(authorization_id.as_slice())
                .map_err(database_error)?
                .is_some()
                || nonces
                    .get(plan.consumption_nonce.as_slice())
                    .map_err(database_error)?
                    .is_some()
            {
                return Err(RecipientImportConsumptionAuthorizationLedgerError::LedgerChanged);
            }
            let stored_head = heads
                .get(HEAD_KEY)
                .map_err(database_error)?
                .map(|value| decode_head(value.value()))
                .transpose()?;
            if stored_head != snapshot.head {
                return Err(RecipientImportConsumptionAuthorizationLedgerError::LedgerChanged);
            }
            records
                .insert(authorization_id.as_slice(), record_bytes.as_slice())
                .map_err(database_error)?;
            nonces
                .insert(
                    plan.consumption_nonce.as_slice(),
                    authorization_id.as_slice(),
                )
                .map_err(database_error)?;
            heads
                .insert(HEAD_KEY, head_bytes.as_slice())
                .map_err(database_error)?;
        }
        if fail_before_commit {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::InjectedBeforeCommit);
        }
        write.commit().map_err(database_error)?;

        let committed =
            snapshot_from_database(&database, self.owner_account_id, self.owner_device_id)?
                .records
                .get(&authorization_id)
                .cloned()
                .ok_or(RecipientImportConsumptionAuthorizationLedgerError::MissingRecord)?;
        if committed.record != *candidate {
            return Err(
                RecipientImportConsumptionAuthorizationLedgerError::CommittedRecordMismatch,
            );
        }
        Ok(RecipientImportConsumptionAuthorizationCommitOutcome::Committed(committed))
    }

    pub fn status_by_authorization_id(
        &self,
        authorization_id: [u8; 32],
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportConsumptionAuthorizationStatus,
        RecipientImportConsumptionAuthorizationLedgerError,
    > {
        if authorization_id == [0_u8; 32] {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::InvalidAuthorizationId);
        }
        let snapshot = load_snapshot(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        )?;
        Ok(match snapshot.records.get(&authorization_id) {
            Some(durable) => RecipientImportConsumptionAuthorizationStatus::Committed {
                summary: Box::new(durable.summary()?),
                expired: durable.expired(now_unix_seconds),
            },
            None => RecipientImportConsumptionAuthorizationStatus::Absent,
        })
    }

    pub fn load_by_authorization_id(
        &self,
        authorization_id: [u8; 32],
    ) -> Result<
        Option<DurableRecipientImportConsumptionAuthorization>,
        RecipientImportConsumptionAuthorizationLedgerError,
    > {
        if authorization_id == [0_u8; 32] {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::InvalidAuthorizationId);
        }
        Ok(load_snapshot(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        )?
        .records
        .get(&authorization_id)
        .cloned())
    }

    /// Loads the exact durable authorization bound to `consumption_nonce`.
    /// Missing storage remains missing; this read path never creates state.
    pub fn load_by_consumption_nonce(
        &self,
        consumption_nonce: [u8; 32],
    ) -> Result<
        Option<DurableRecipientImportConsumptionAuthorization>,
        RecipientImportConsumptionAuthorizationLedgerError,
    > {
        if consumption_nonce == [0_u8; 32] {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::InvalidNonce);
        }
        load_snapshot(
            &self.state_directory,
            self.owner_account_id,
            self.owner_device_id,
        )?
        .by_nonce(consumption_nonce)
    }

    #[cfg(test)]
    fn commit_with_fault_before_commit(
        &self,
        plan: &RecipientImportConsumptionAuthorizationPlan,
        now_unix_seconds: u64,
    ) -> Result<
        RecipientImportConsumptionAuthorizationCommitOutcome,
        RecipientImportConsumptionAuthorizationLedgerError,
    > {
        self.commit_inner(plan, now_unix_seconds, true)
    }

    #[cfg(test)]
    fn corrupt_head_for_test(
        &self,
    ) -> Result<(), RecipientImportConsumptionAuthorizationLedgerError> {
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
                .ok_or(RecipientImportConsumptionAuthorizationLedgerError::MissingHead)?;
            let mut bytes = encoded.value().to_vec();
            drop(encoded);
            let last = bytes
                .last_mut()
                .ok_or(RecipientImportConsumptionAuthorizationLedgerError::InvalidHead)?;
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
    records: BTreeMap<[u8; 32], DurableRecipientImportConsumptionAuthorization>,
    nonce_index: BTreeMap<[u8; 32], [u8; 32]>,
    head: Option<RecipientImportConsumptionAuthorizationHeadWire>,
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
    ) -> Result<
        Option<DurableRecipientImportConsumptionAuthorization>,
        RecipientImportConsumptionAuthorizationLedgerError,
    > {
        let Some(authorization_id) = self.nonce_index.get(&nonce) else {
            return Ok(None);
        };
        self.records
            .get(authorization_id)
            .cloned()
            .map(Some)
            .ok_or(RecipientImportConsumptionAuthorizationLedgerError::MissingRecord)
    }
}

fn exact_authorization_binding(
    dispatch: &RecipientImportConsumptionReadAdapterDispatch,
    request: &RuntimeIpcRecipientImportConsumptionRequest,
) -> Result<
    (
        RecipientImportConsumptionAuthorizationBinding,
        [u8; 32],
        u64,
        u64,
    ),
    RecipientImportConsumptionAuthorizationLedgerError,
> {
    let planned = dispatch
        .ephemeral_plan()
        .ok_or(RecipientImportConsumptionAuthorizationLedgerError::NoPlannedDispatch)?;
    let RuntimeIpcRecipientImportConsumptionCommand::Commit {
        evidence,
        memberships,
        plan,
        confirmation_phrase,
        explicit_approval,
    } = request.command()
    else {
        return Err(RecipientImportConsumptionAuthorizationLedgerError::NotAuthorizationCommand);
    };
    let request_membership_digest =
        recipient_import_membership_set_digest(memberships).map_err(contract_error)?;
    if plan.as_ref() != planned
        || *explicit_approval != RuntimeIpcRecipientImportConsumptionExplicitApproval::UserConfirmed
        || confirmation_phrase
            != &recipient_import_consumption_confirmation_phrase(&planned.consumption_plan_id)
        || confirmation_phrase != &planned.confirmation_phrase
        || evidence.intent_id != planned.intent_id
        || evidence.evidence_digest().map_err(contract_error)? != planned.evidence_digest
        || request_membership_digest != evidence.membership_set_digest
    {
        return Err(RecipientImportConsumptionAuthorizationLedgerError::PlanChanged);
    }
    validate_lifetime(
        planned.planned_at_unix_seconds,
        planned.expires_at_unix_seconds,
    )?;
    let binding = RecipientImportConsumptionAuthorizationBinding {
        evidence: evidence.clone(),
        plan: planned.clone(),
        membership_set_digest: request_membership_digest
            .to_bytes()
            .map_err(contract_error)?,
    };
    Ok((
        binding,
        planned
            .consumption_nonce
            .to_bytes()
            .map_err(contract_error)?,
        planned.planned_at_unix_seconds,
        planned.expires_at_unix_seconds,
    ))
}

fn summary_from_record(
    record: &SignedRecipientImportConsumptionAuthorizationRecord,
    authorization_id: [u8; 32],
) -> Result<
    RecipientImportConsumptionAuthorizationSummary,
    RecipientImportConsumptionAuthorizationLedgerError,
> {
    let binding = &record.content.binding;
    Ok(RecipientImportConsumptionAuthorizationSummary {
        authorization_id: artifact_id(authorization_id)?,
        sequence: record.content.sequence,
        evidence: binding.evidence.clone(),
        plan: binding.plan.clone(),
        membership_set_digest: artifact_id(binding.membership_set_digest)?,
    })
}

fn artifact_id(
    bytes: [u8; 32],
) -> Result<RuntimeIpcRecipientImportArtifactId, RecipientImportConsumptionAuthorizationLedgerError>
{
    RuntimeIpcRecipientImportArtifactId::from_bytes(bytes).map_err(contract_error)
}

fn contract_error(
    error: impl std::fmt::Display,
) -> RecipientImportConsumptionAuthorizationLedgerError {
    RecipientImportConsumptionAuthorizationLedgerError::Contract(error.to_string())
}

fn validate_lifetime(
    issued_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
) -> Result<(), RecipientImportConsumptionAuthorizationLedgerError> {
    let lifetime = expires_at_unix_seconds
        .checked_sub(issued_at_unix_seconds)
        .ok_or(RecipientImportConsumptionAuthorizationLedgerError::InvalidLifetime)?;
    if issued_at_unix_seconds == 0
        || lifetime == 0
        || lifetime > MAX_RECIPIENT_IMPORT_CONSUMPTION_AUTHORIZATION_LIFETIME_SECONDS
    {
        return Err(RecipientImportConsumptionAuthorizationLedgerError::InvalidLifetime);
    }
    Ok(())
}

fn validate_record_content(
    content: &RecipientImportConsumptionAuthorizationRecordContent,
) -> Result<(), RecipientImportConsumptionAuthorizationLedgerError> {
    validate_lifetime(
        content.issued_at_unix_seconds,
        content.expires_at_unix_seconds,
    )?;
    let binding = &content.binding;
    for artifact in [
        &binding.plan.intent_id,
        &binding.plan.consumption_plan_id,
        &binding.plan.evidence_digest,
        &binding.plan.completed_mutation_record_id,
        &binding.plan.completed_mutation_receipt_digest,
        &binding.plan.final_coordinator_plan_id,
        &binding.plan.consumption_nonce,
    ] {
        artifact.to_bytes().map_err(contract_error)?;
    }
    let membership_set_digest = binding
        .evidence
        .membership_set_digest
        .to_bytes()
        .map_err(contract_error)?;
    let lifetime_seconds = binding
        .plan
        .expires_at_unix_seconds
        .checked_sub(binding.plan.planned_at_unix_seconds)
        .ok_or(RecipientImportConsumptionAuthorizationLedgerError::InvalidLifetime)?;
    let reconstructed_plan = RuntimeIpcRecipientImportConsumptionPlanSummary::issue(
        &binding.evidence,
        binding.plan.consumption_nonce.clone(),
        binding.plan.planned_at_unix_seconds,
        lifetime_seconds,
    )
    .map_err(contract_error)?;
    if content.version != RECORD_VERSION
        || content.sequence == 0
        || (content.sequence == 1) != content.previous_authorization_id.is_none()
        || binding.membership_set_digest == [0_u8; 32]
        || binding.membership_set_digest != membership_set_digest
        || binding.plan != reconstructed_plan
        || content.consumption_nonce == [0_u8; 32]
        || content.consumption_nonce
            != binding
                .plan
                .consumption_nonce
                .to_bytes()
                .map_err(contract_error)?
        || content.issued_at_unix_seconds != binding.plan.planned_at_unix_seconds
        || content.expires_at_unix_seconds != binding.plan.expires_at_unix_seconds
        || binding.plan.confirmation_phrase
            != recipient_import_consumption_confirmation_phrase(&binding.plan.consumption_plan_id)
    {
        return Err(RecipientImportConsumptionAuthorizationLedgerError::InvalidRecord);
    }
    Ok(())
}

fn record_signing_bytes(
    content: &RecipientImportConsumptionAuthorizationRecordContent,
) -> Result<Vec<u8>, RecipientImportConsumptionAuthorizationLedgerError> {
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
) -> Result<LedgerSnapshot, RecipientImportConsumptionAuthorizationLedgerError> {
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
) -> Result<LedgerSnapshot, RecipientImportConsumptionAuthorizationLedgerError> {
    verify_database_metadata(database, owner_account_id, owner_device_id)?;
    let read = database.begin_read().map_err(database_error)?;
    let records_table = read.open_table(RECORD_TABLE).map_err(database_error)?;
    if records_table.len().map_err(database_error)?
        > MAX_DURABLE_RECIPIENT_IMPORT_CONSUMPTION_AUTHORIZATIONS
    {
        return Err(RecipientImportConsumptionAuthorizationLedgerError::Capacity);
    }
    let mut records = BTreeMap::new();
    for item in records_table.iter().map_err(database_error)? {
        let (stored_id, encoded) = item.map_err(database_error)?;
        let authorization_id = copy_id(stored_id.value())?;
        let record = SignedRecipientImportConsumptionAuthorizationRecord::decode_and_verify(
            encoded.value(),
            owner_account_id,
            owner_device_id,
        )?;
        if record.authorization_id()? != authorization_id {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::RecordIdBinding);
        }
        if records
            .insert(
                authorization_id,
                DurableRecipientImportConsumptionAuthorization {
                    authorization_id,
                    record,
                },
            )
            .is_some()
        {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::LedgerFork);
        }
    }
    drop(records_table);

    let nonce_table = read.open_table(NONCE_TABLE).map_err(database_error)?;
    if nonce_table.len().map_err(database_error)? != records.len() as u64 {
        return Err(RecipientImportConsumptionAuthorizationLedgerError::NonceIndexMismatch);
    }
    let mut nonce_index = BTreeMap::new();
    let mut indexed_records = HashSet::new();
    for item in nonce_table.iter().map_err(database_error)? {
        let (stored_nonce, stored_id) = item.map_err(database_error)?;
        let nonce = copy_id(stored_nonce.value())?;
        let authorization_id = copy_id(stored_id.value())?;
        let durable = records
            .get(&authorization_id)
            .ok_or(RecipientImportConsumptionAuthorizationLedgerError::MissingRecord)?;
        if durable.record.content.consumption_nonce != nonce
            || !indexed_records.insert(authorization_id)
            || nonce_index.insert(nonce, authorization_id).is_some()
        {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::NonceIndexMismatch);
        }
    }
    if indexed_records.len() != records.len() {
        return Err(RecipientImportConsumptionAuthorizationLedgerError::NonceIndexMismatch);
    }
    drop(nonce_table);

    let heads = read.open_table(HEAD_TABLE).map_err(database_error)?;
    if heads.len().map_err(database_error)? > 1 {
        return Err(RecipientImportConsumptionAuthorizationLedgerError::LedgerFork);
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
    records: &BTreeMap<[u8; 32], DurableRecipientImportConsumptionAuthorization>,
    head: Option<&RecipientImportConsumptionAuthorizationHeadWire>,
) -> Result<(), RecipientImportConsumptionAuthorizationLedgerError> {
    if records.is_empty() {
        return if head.is_none() {
            Ok(())
        } else {
            Err(RecipientImportConsumptionAuthorizationLedgerError::MissingRecord)
        };
    }
    let head = head.ok_or(RecipientImportConsumptionAuthorizationLedgerError::MissingHead)?;
    if head.version != HEAD_VERSION
        || head.sequence != records.len() as u64
        || head.authorization_id == [0_u8; 32]
    {
        return Err(RecipientImportConsumptionAuthorizationLedgerError::InvalidHead);
    }
    let mut expected_id = Some(head.authorization_id);
    let mut expected_sequence = head.sequence;
    let mut visited = HashSet::new();
    while let Some(authorization_id) = expected_id {
        if !visited.insert(authorization_id) {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::LedgerFork);
        }
        let durable = records
            .get(&authorization_id)
            .ok_or(RecipientImportConsumptionAuthorizationLedgerError::MissingRecord)?;
        if durable.record.content.sequence != expected_sequence {
            return Err(RecipientImportConsumptionAuthorizationLedgerError::LedgerRollback);
        }
        expected_id = durable.record.content.previous_authorization_id;
        expected_sequence = expected_sequence.saturating_sub(1);
    }
    if expected_sequence != 0 || visited.len() != records.len() {
        return Err(RecipientImportConsumptionAuthorizationLedgerError::LedgerFork);
    }
    Ok(())
}

fn initialize_database(
    database: &Database,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), RecipientImportConsumptionAuthorizationLedgerError> {
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
) -> Result<(), RecipientImportConsumptionAuthorizationLedgerError> {
    snapshot_from_database(database, owner_account_id, owner_device_id).map(|_| ())
}

fn verify_database_metadata<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), RecipientImportConsumptionAuthorizationLedgerError> {
    let read = database.begin_read().map_err(database_error)?;
    let meta = read.open_table(META_TABLE).map_err(database_error)?;
    verify_meta(&meta, SCHEMA_KEY, &[SCHEMA_VERSION], || {
        RecipientImportConsumptionAuthorizationLedgerError::SchemaMismatch
    })?;
    verify_meta(
        &meta,
        OWNER_ACCOUNT_KEY,
        owner_account_id.as_bytes(),
        || RecipientImportConsumptionAuthorizationLedgerError::OwnerAccountMismatch,
    )?;
    verify_meta(&meta, OWNER_DEVICE_KEY, owner_device_id.as_bytes(), || {
        RecipientImportConsumptionAuthorizationLedgerError::OwnerDeviceMismatch
    })?;
    drop(meta);
    drop(read);
    Ok(())
}

fn verify_meta(
    meta: &impl ReadableTable<&'static str, &'static [u8]>,
    key: &'static str,
    expected: &[u8],
    mismatch: impl Fn() -> RecipientImportConsumptionAuthorizationLedgerError,
) -> Result<(), RecipientImportConsumptionAuthorizationLedgerError> {
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
    head: &RecipientImportConsumptionAuthorizationHeadWire,
) -> Result<Vec<u8>, RecipientImportConsumptionAuthorizationLedgerError> {
    if head.version != HEAD_VERSION || head.sequence == 0 || head.authorization_id == [0_u8; 32] {
        return Err(RecipientImportConsumptionAuthorizationLedgerError::InvalidHead);
    }
    let encoded = postcard::to_allocvec(head)?;
    if encoded.is_empty() || encoded.len() > MAX_HEAD_BYTES {
        return Err(RecipientImportConsumptionAuthorizationLedgerError::HeadLength(encoded.len()));
    }
    Ok(encoded)
}

fn decode_head(
    encoded: &[u8],
) -> Result<
    RecipientImportConsumptionAuthorizationHeadWire,
    RecipientImportConsumptionAuthorizationLedgerError,
> {
    if encoded.is_empty() || encoded.len() > MAX_HEAD_BYTES {
        return Err(RecipientImportConsumptionAuthorizationLedgerError::HeadLength(encoded.len()));
    }
    let head: RecipientImportConsumptionAuthorizationHeadWire = postcard::from_bytes(encoded)?;
    if encode_head(&head)? != encoded {
        return Err(RecipientImportConsumptionAuthorizationLedgerError::NonCanonicalHead);
    }
    Ok(head)
}

fn validate_record_length(
    encoded: &[u8],
) -> Result<(), RecipientImportConsumptionAuthorizationLedgerError> {
    if encoded.is_empty() || encoded.len() > MAX_RECORD_BYTES {
        return Err(
            RecipientImportConsumptionAuthorizationLedgerError::RecordLength(encoded.len()),
        );
    }
    Ok(())
}

fn prepare_database_path(
    root: &Path,
) -> Result<PathBuf, RecipientImportConsumptionAuthorizationLedgerError> {
    reject_ledger_path_symlinks(root)?;
    let directory = root.join(LEDGER_DIRECTORY);
    fs::create_dir_all(&directory)?;
    reject_ledger_path_symlinks(root)?;
    let canonical_directory = fs::canonicalize(&directory)?;
    if !canonical_directory.starts_with(root) {
        return Err(RecipientImportConsumptionAuthorizationLedgerError::UnsafePath);
    }
    Ok(canonical_directory.join(DATABASE_FILE))
}

fn reject_ledger_path_symlinks(
    root: &Path,
) -> Result<(), RecipientImportConsumptionAuthorizationLedgerError> {
    for path in [root.join(LEDGER_DIRECTORY), database_path(root)] {
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(RecipientImportConsumptionAuthorizationLedgerError::UnsafePath);
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

fn copy_id(bytes: &[u8]) -> Result<[u8; 32], RecipientImportConsumptionAuthorizationLedgerError> {
    bytes.try_into().map_err(|_| {
        RecipientImportConsumptionAuthorizationLedgerError::StoredIdLength(bytes.len())
    })
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

fn database_error(
    error: impl std::fmt::Display,
) -> RecipientImportConsumptionAuthorizationLedgerError {
    RecipientImportConsumptionAuthorizationLedgerError::Database(error.to_string())
}

#[derive(Debug, Error)]
pub enum RecipientImportConsumptionAuthorizationLedgerError {
    #[error("recipient import consumption authorization ledger I/O failed")]
    Io(#[from] std::io::Error),
    #[error("recipient import consumption authorization database failed: {0}")]
    Database(String),
    #[error("recipient import consumption authorization contract failed: {0}")]
    Contract(String),
    #[error("recipient import consumption authorization ledger schema does not match")]
    SchemaMismatch,
    #[error("recipient import consumption authorization owner Account does not match")]
    OwnerAccountMismatch,
    #[error("recipient import consumption authorization owner Device does not match")]
    OwnerDeviceMismatch,
    #[error("recipient import consumption authorization request is not a confirmation command")]
    NotAuthorizationCommand,
    #[error("recipient import consumption authorization has no exact read-adapter plan")]
    NoPlannedDispatch,
    #[error("recipient import consumption authorization plan changed")]
    PlanChanged,
    #[error("recipient import consumption authorization membership references changed")]
    MembershipChanged,
    #[error("recipient import consumption authorization nonce is already bound to another plan")]
    NonceConflict,
    #[error("recipient import consumption authorization lifetime is invalid")]
    InvalidLifetime,
    #[error("recipient import consumption authorization expired before commit")]
    AuthorizationExpired,
    #[error("recipient import consumption authorization record is invalid")]
    InvalidRecord,
    #[error("recipient import consumption authorization record is not canonical")]
    NonCanonicalRecord,
    #[error("recipient import consumption authorization head is invalid")]
    InvalidHead,
    #[error("recipient import consumption authorization head is not canonical")]
    NonCanonicalHead,
    #[error(
        "recipient import consumption authorization record is {0} bytes and violates its bound"
    )]
    RecordLength(usize),
    #[error("recipient import consumption authorization head is {0} bytes and violates its bound")]
    HeadLength(usize),
    #[error("recipient import consumption authorization record ID does not bind its bytes")]
    RecordIdBinding,
    #[error("recipient import consumption authorization ledger references a missing record")]
    MissingRecord,
    #[error("recipient import consumption authorization ledger has no required head")]
    MissingHead,
    #[error("recipient import consumption authorization nonce index is inconsistent")]
    NonceIndexMismatch,
    #[error("recipient import consumption authorization ledger fork detected")]
    LedgerFork,
    #[error("recipient import consumption authorization ledger rollback detected")]
    LedgerRollback,
    #[error("recipient import consumption authorization ledger changed after planning")]
    LedgerChanged,
    #[error("recipient import consumption authorization committed an unexpected record")]
    CommittedRecordMismatch,
    #[error("recipient import consumption authorization capacity is exhausted")]
    Capacity,
    #[error("recipient import consumption authorization sequence is exhausted")]
    SequenceExhausted,
    #[error("recipient import consumption authorization ID is invalid")]
    InvalidAuthorizationId,
    #[error("recipient import consumption authorization nonce is invalid")]
    InvalidNonce,
    #[error("recipient import consumption authorization ledger path is unsafe")]
    UnsafePath,
    #[error("stored recipient import consumption authorization ID has invalid length {0}")]
    StoredIdLength(usize),
    #[error("test-only failure injected before atomic confirmation commit")]
    InjectedBeforeCommit,
    #[error("recipient import consumption authorization encoding is invalid: {0}")]
    Codec(#[from] postcard::Error),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    State(#[from] StateError),
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        net::{IpAddr, SocketAddr},
    };

    use kilogram_runtime_ipc::{
        RuntimeIpcDescriptor, RuntimeIpcRecipientImportCompletedMutationEvidence,
        RuntimeIpcRecipientImportConsumptionCommand,
        RuntimeIpcRecipientImportConsumptionExplicitApproval,
        RuntimeIpcRecipientImportExecutionReceiptSummary,
        RuntimeIpcRecipientImportMembershipReference, RuntimeIpcRecipientImportStepDisposition,
        RuntimeIpcRecipientImportStepReceipt, RuntimeIpcRequestId,
    };
    use tempfile::TempDir;

    use super::*;

    struct Fixture {
        root: TempDir,
        owner: DeviceIdentity,
        account_id: AccountId,
        dispatch: RecipientImportConsumptionReadAdapterDispatch,
        request: RuntimeIpcRecipientImportConsumptionRequest,
        evidence: RuntimeIpcRecipientImportCompletedMutationEvidence,
        memberships: Vec<RuntimeIpcRecipientImportMembershipReference>,
    }

    fn artifact(byte: u8) -> RuntimeIpcRecipientImportArtifactId {
        RuntimeIpcRecipientImportArtifactId::from_bytes([byte; 32])
            .unwrap_or_else(|error| unreachable!("nonzero test artifact must be valid: {error}"))
    }

    fn request_id(byte: u8) -> RuntimeIpcRequestId {
        format!("{byte:064x}")
            .parse()
            .unwrap_or_else(|error| unreachable!("test request ID must be valid: {error}"))
    }

    fn memberships() -> Vec<RuntimeIpcRecipientImportMembershipReference> {
        vec![
            RuntimeIpcRecipientImportMembershipReference {
                source_ordinal: 2,
                authority_reference_id: artifact(20),
                membership_digest: artifact(21),
                authenticated_context_id: artifact(22),
            },
            RuntimeIpcRecipientImportMembershipReference {
                source_ordinal: 7,
                authority_reference_id: artifact(23),
                membership_digest: artifact(24),
                authenticated_context_id: artifact(25),
            },
        ]
    }

    fn execution_receipt() -> RuntimeIpcRecipientImportExecutionReceiptSummary {
        RuntimeIpcRecipientImportExecutionReceiptSummary {
            intent_id: artifact(1),
            execution_plan_id: artifact(2),
            confirmation_id: artifact(3),
            mutation_nonce: artifact(4),
            starting_completed_source_count: 0,
            final_completed_source_count: 2,
            total_source_count: 2,
            steps: vec![
                RuntimeIpcRecipientImportStepReceipt {
                    source_ordinal: 1,
                    disposition: RuntimeIpcRecipientImportStepDisposition::Imported,
                    imported_event_count: 3,
                    coordinator_sequence: 10,
                },
                RuntimeIpcRecipientImportStepReceipt {
                    source_ordinal: 2,
                    disposition: RuntimeIpcRecipientImportStepDisposition::AlreadyImported,
                    imported_event_count: 4,
                    coordinator_sequence: 11,
                },
            ],
            imported_now_count: 1,
            already_imported_count: 1,
            imported_event_count: 7,
            final_coordinator_plan_id: artifact(5),
            final_coordinator_sequence: 11,
            complete: true,
        }
    }

    fn fixture_with_plan(
        planned_at_unix_seconds: u64,
        consumption_nonce: RuntimeIpcRecipientImportArtifactId,
    ) -> Result<Fixture, Box<dyn Error>> {
        let root = tempfile::tempdir()?;
        let owner = DeviceIdentity::generate()?;
        let account_id = AccountId::from_bytes([7_u8; 32]);
        let memberships = memberships();
        let evidence = RuntimeIpcRecipientImportCompletedMutationEvidence::from_completed_receipt(
            artifact(6),
            recipient_import_membership_set_digest(&memberships)?,
            &execution_receipt(),
        )?;
        let plan = RuntimeIpcRecipientImportConsumptionPlanSummary::issue(
            &evidence,
            consumption_nonce,
            planned_at_unix_seconds,
            120,
        )?;
        let descriptor = RuntimeIpcDescriptor::issue_local(
            SocketAddr::new(IpAddr::from([127, 0, 0, 1]), 42101),
            [8_u8; 32],
            account_id,
            &owner,
        )?;
        let request = RuntimeIpcRecipientImportConsumptionRequest::new(
            &descriptor,
            request_id(1),
            RuntimeIpcRecipientImportConsumptionCommand::Commit {
                evidence: evidence.clone(),
                memberships: memberships.clone(),
                confirmation_phrase: plan.confirmation_phrase.clone(),
                explicit_approval:
                    RuntimeIpcRecipientImportConsumptionExplicitApproval::UserConfirmed,
                plan: Box::new(plan.clone()),
            },
        )?;
        Ok(Fixture {
            root,
            owner,
            account_id,
            dispatch: RecipientImportConsumptionReadAdapterDispatch::planned_for_test(plan, 2),
            request,
            evidence,
            memberships,
        })
    }

    fn fixture() -> Result<Fixture, Box<dyn Error>> {
        fixture_with_plan(1_000, artifact(7))
    }

    fn ledger(fixture: &Fixture) -> RecipientImportConsumptionAuthorizationLedger {
        RecipientImportConsumptionAuthorizationLedger::new(
            fixture.root.path(),
            fixture.account_id,
            fixture.owner.device_id(),
        )
    }

    #[test]
    fn planning_is_read_only_and_commit_survives_restart() -> Result<(), Box<dyn Error>> {
        let fixture = fixture()?;
        let ledger = ledger(&fixture);
        assert!(!ledger.is_initialized());

        let plan = ledger.plan(&fixture.dispatch, &fixture.request, &fixture.owner)?;
        assert!(!ledger.is_initialized());
        assert!(!plan.already_committed());
        assert!(!plan.persists_during_planning());

        let summary = plan.summary()?;
        assert_eq!(summary.sequence, 1);
        assert_eq!(
            summary.plan,
            fixture
                .dispatch
                .ephemeral_plan()
                .cloned()
                .ok_or("missing test plan")?
        );
        assert_eq!(
            summary.membership_set_digest,
            recipient_import_membership_set_digest(&fixture.memberships)?
        );

        let committed = match ledger.commit(&plan, 1_050)? {
            RecipientImportConsumptionAuthorizationCommitOutcome::Committed(value) => value,
            RecipientImportConsumptionAuthorizationCommitOutcome::AlreadyCommitted(_) => {
                return Err("first commit unexpectedly replayed".into());
            }
        };
        assert!(ledger.is_initialized());
        assert!(committed.grants_consumption_authority());
        assert!(!committed.grants_import_authority());
        assert!(!committed.contains_membership_snapshot());

        let reopened = RecipientImportConsumptionAuthorizationLedger::new(
            fixture.root.path(),
            fixture.account_id,
            fixture.owner.device_id(),
        );
        let loaded = reopened
            .load_by_authorization_id(committed.authorization_id())?
            .ok_or("committed authorization did not survive restart")?;
        assert_eq!(loaded, committed);
        assert_eq!(
            reopened.status_by_authorization_id(committed.authorization_id(), 1_050)?,
            RecipientImportConsumptionAuthorizationStatus::Committed {
                summary: Box::new(committed.summary()?),
                expired: false,
            }
        );
        Ok(())
    }

    #[test]
    fn exact_replay_is_idempotent_without_a_second_record() -> Result<(), Box<dyn Error>> {
        let fixture = fixture()?;
        let ledger = ledger(&fixture);
        let first_plan = ledger.plan(&fixture.dispatch, &fixture.request, &fixture.owner)?;
        let first = match ledger.commit(&first_plan, 1_050)? {
            RecipientImportConsumptionAuthorizationCommitOutcome::Committed(value) => value,
            RecipientImportConsumptionAuthorizationCommitOutcome::AlreadyCommitted(_) => {
                return Err("first commit unexpectedly replayed".into());
            }
        };

        let replay_plan = ledger.plan(&fixture.dispatch, &fixture.request, &fixture.owner)?;
        assert!(replay_plan.already_committed());
        let replay = match ledger.commit(&replay_plan, 1_060)? {
            RecipientImportConsumptionAuthorizationCommitOutcome::AlreadyCommitted(value) => value,
            RecipientImportConsumptionAuthorizationCommitOutcome::Committed(_) => {
                return Err("replay created a second record".into());
            }
        };
        assert_eq!(replay, first);
        assert_eq!(replay.sequence(), 1);
        Ok(())
    }

    #[test]
    fn plan_command_and_mismatched_ephemeral_plan_fail_closed() -> Result<(), Box<dyn Error>> {
        let fixture = fixture()?;
        let ledger = ledger(&fixture);
        let descriptor = RuntimeIpcDescriptor::issue_local(
            SocketAddr::new(IpAddr::from([127, 0, 0, 1]), 42102),
            [9_u8; 32],
            fixture.account_id,
            &fixture.owner,
        )?;
        let plan_request = RuntimeIpcRecipientImportConsumptionRequest::new(
            &descriptor,
            request_id(2),
            RuntimeIpcRecipientImportConsumptionCommand::Plan {
                evidence: fixture.evidence.clone(),
                memberships: fixture.memberships.clone(),
                consumption_nonce: artifact(7),
                requested_lifetime_seconds: 120,
            },
        )?;
        assert!(matches!(
            ledger.plan(&fixture.dispatch, &plan_request, &fixture.owner),
            Err(RecipientImportConsumptionAuthorizationLedgerError::NotAuthorizationCommand)
        ));

        let other = RuntimeIpcRecipientImportConsumptionPlanSummary::issue(
            &fixture.evidence,
            artifact(9),
            1_000,
            120,
        )?;
        let other_dispatch =
            RecipientImportConsumptionReadAdapterDispatch::planned_for_test(other, 2);
        assert!(matches!(
            ledger.plan(&other_dispatch, &fixture.request, &fixture.owner),
            Err(RecipientImportConsumptionAuthorizationLedgerError::PlanChanged)
        ));
        assert!(!ledger.is_initialized());
        Ok(())
    }

    #[test]
    fn wrong_device_and_expired_commit_fail_closed() -> Result<(), Box<dyn Error>> {
        let fixture = fixture()?;
        let ledger = ledger(&fixture);
        let other = DeviceIdentity::generate()?;
        assert!(matches!(
            ledger.plan(&fixture.dispatch, &fixture.request, &other),
            Err(RecipientImportConsumptionAuthorizationLedgerError::OwnerDeviceMismatch)
        ));

        let plan = ledger.plan(&fixture.dispatch, &fixture.request, &fixture.owner)?;
        assert!(matches!(
            ledger.commit(&plan, 1_121),
            Err(RecipientImportConsumptionAuthorizationLedgerError::AuthorizationExpired)
        ));
        assert!(!ledger.is_initialized());
        Ok(())
    }

    #[test]
    fn competing_head_and_nonce_rebinding_fail_closed() -> Result<(), Box<dyn Error>> {
        let fixture = fixture()?;
        let ledger = ledger(&fixture);
        let first_plan = ledger.plan(&fixture.dispatch, &fixture.request, &fixture.owner)?;

        let second_plan_summary = RuntimeIpcRecipientImportConsumptionPlanSummary::issue(
            &fixture.evidence,
            artifact(9),
            1_000,
            120,
        )?;
        let descriptor = RuntimeIpcDescriptor::issue_local(
            SocketAddr::new(IpAddr::from([127, 0, 0, 1]), 42103),
            [10_u8; 32],
            fixture.account_id,
            &fixture.owner,
        )?;
        let second_request = RuntimeIpcRecipientImportConsumptionRequest::new(
            &descriptor,
            request_id(3),
            RuntimeIpcRecipientImportConsumptionCommand::Commit {
                evidence: fixture.evidence.clone(),
                memberships: fixture.memberships.clone(),
                confirmation_phrase: second_plan_summary.confirmation_phrase.clone(),
                explicit_approval:
                    RuntimeIpcRecipientImportConsumptionExplicitApproval::UserConfirmed,
                plan: Box::new(second_plan_summary.clone()),
            },
        )?;
        let second_dispatch =
            RecipientImportConsumptionReadAdapterDispatch::planned_for_test(second_plan_summary, 2);
        let stale_second = ledger.plan(&second_dispatch, &second_request, &fixture.owner)?;

        ledger.commit(&first_plan, 1_050)?;
        assert!(matches!(
            ledger.commit(&stale_second, 1_050),
            Err(RecipientImportConsumptionAuthorizationLedgerError::LedgerChanged)
        ));

        let rebound_plan_summary = RuntimeIpcRecipientImportConsumptionPlanSummary::issue(
            &fixture.evidence,
            artifact(7),
            1_001,
            120,
        )?;
        let rebound_request = RuntimeIpcRecipientImportConsumptionRequest::new(
            &descriptor,
            request_id(4),
            RuntimeIpcRecipientImportConsumptionCommand::Commit {
                evidence: fixture.evidence.clone(),
                memberships: fixture.memberships.clone(),
                confirmation_phrase: rebound_plan_summary.confirmation_phrase.clone(),
                explicit_approval:
                    RuntimeIpcRecipientImportConsumptionExplicitApproval::UserConfirmed,
                plan: Box::new(rebound_plan_summary.clone()),
            },
        )?;
        let rebound_dispatch = RecipientImportConsumptionReadAdapterDispatch::planned_for_test(
            rebound_plan_summary,
            2,
        );
        assert!(matches!(
            ledger.plan(&rebound_dispatch, &rebound_request, &fixture.owner),
            Err(RecipientImportConsumptionAuthorizationLedgerError::NonceConflict)
        ));
        Ok(())
    }

    #[test]
    fn crash_before_atomic_commit_leaves_no_authorization() -> Result<(), Box<dyn Error>> {
        let fixture = fixture()?;
        let ledger = ledger(&fixture);
        let plan = ledger.plan(&fixture.dispatch, &fixture.request, &fixture.owner)?;
        assert!(matches!(
            ledger.commit_with_fault_before_commit(&plan, 1_050),
            Err(RecipientImportConsumptionAuthorizationLedgerError::InjectedBeforeCommit)
        ));
        let authorization_id = plan.summary()?.authorization_id.to_bytes()?;
        assert_eq!(
            ledger.status_by_authorization_id(authorization_id, 1_050)?,
            RecipientImportConsumptionAuthorizationStatus::Absent
        );
        assert!(matches!(
            ledger.commit(&plan, 1_050)?,
            RecipientImportConsumptionAuthorizationCommitOutcome::Committed(_)
        ));
        Ok(())
    }

    #[test]
    fn corrupted_head_and_wrong_owner_are_rejected_after_restart() -> Result<(), Box<dyn Error>> {
        let fixture = fixture()?;
        let ledger = ledger(&fixture);
        let plan = ledger.plan(&fixture.dispatch, &fixture.request, &fixture.owner)?;
        ledger.commit(&plan, 1_050)?;

        let wrong_owner = RecipientImportConsumptionAuthorizationLedger::new(
            fixture.root.path(),
            AccountId::from_bytes([11_u8; 32]),
            fixture.owner.device_id(),
        );
        assert!(matches!(
            wrong_owner.load_by_authorization_id([12_u8; 32]),
            Err(RecipientImportConsumptionAuthorizationLedgerError::OwnerAccountMismatch)
        ));

        ledger.corrupt_head_for_test()?;
        assert!(
            ledger
                .load_by_authorization_id(plan.summary()?.authorization_id.to_bytes()?)
                .is_err()
        );
        Ok(())
    }
}
