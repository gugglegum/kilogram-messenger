#![forbid(unsafe_code)]

//! Atomic durable acceptance of one exact old-history recovery consent plan.
//!
//! This crate persists consent only. It cannot read message bodies, create a
//! history rewrap bundle, advance a recovery checkpoint, open a network
//! endpoint or authorize execution.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use kilogram_identity::{
    AccountDeviceListSnapshot, AccountId, ConversationMembershipSnapshot, DeviceId, DeviceIdentity,
    IdentityError,
};
use kilogram_peer_history_recovery::{
    HistoryRecoveryRangeSelection, MAX_HISTORY_CONVERSATIONS, OldHistoryRecoveryInventory,
    PeerHistoryRecoveryError, PeerReenrollmentHistoryBinding, SignedOldHistoryRecoveryPlan,
};
use redb::{
    Database, Durability, ReadOnlyDatabase, ReadableDatabase, ReadableTable, ReadableTableMetadata,
    TableDefinition,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const DATABASE_FILE: &str = "old-history-recovery-consent-v1.redb";
const META_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("old-history-recovery-consent-meta-v1");
const RECORD_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("old-history-recovery-consent-record-v1");
const HEAD_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("old-history-recovery-consent-head-v1");

const SCHEMA_KEY: &str = "schema-version";
const OWNER_ACCOUNT_KEY: &str = "owner-account-id";
const OWNER_DEVICE_KEY: &str = "owner-device-id";
const SCHEMA_VERSION: u8 = 1;
const RECORD_VERSION: u8 = 1;
const HEAD_VERSION: u8 = 1;
const RECORD_SIGNATURE_DOMAIN: &[u8] = b"kilogram:old-history-recovery-consent-record:v1\0";
const RECORD_ID_DOMAIN: &[u8] = b"kilogram:old-history-recovery-consent-record-id:v1\0";
const PLAN_DIGEST_DOMAIN: &[u8] = b"kilogram:old-history-recovery-consent-plan:v1\0";
const DEVICE_TOKEN_DOMAIN: &[u8] = b"kilogram:old-history-recovery-device-token:v1\0";

pub const MAX_OLD_HISTORY_RECOVERY_CONSENT_RECORD_BYTES: usize = 96 * 1024;
pub const MAX_OLD_HISTORY_RECOVERY_CONSENT_HEAD_BYTES: usize = 1024;
pub const MAX_DURABLE_OLD_HISTORY_RECOVERY_CONSENTS: u64 = 4_096;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct OldHistoryRecoveryConsentRecordContent {
    version: u8,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    plan_id: [u8; 32],
    plan_artifact_digest: [u8; 32],
    acceptance_receipt_id: [u8; 32],
    transition_id: [u8; 32],
    replacement_device_id: DeviceId,
    history_authority_revision: u64,
    history_authority_digest: [u8; 32],
    source_claim_id: [u8; 32],
    inventory_id: [u8; 32],
    selection_count: u16,
    plan_prepared_at_unix_seconds: u64,
    plan_expires_at_unix_seconds: u64,
    committed_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SignedOldHistoryRecoveryConsentRecordWire {
    content: OldHistoryRecoveryConsentRecordContent,
    signed_plan: Vec<u8>,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedOldHistoryRecoveryConsentRecord {
    content: OldHistoryRecoveryConsentRecordContent,
    signed_plan: Vec<u8>,
    signature: Vec<u8>,
}

impl SignedOldHistoryRecoveryConsentRecord {
    fn issue(
        owner: &DeviceIdentity,
        owner_account_id: AccountId,
        plan: &SignedOldHistoryRecoveryPlan,
        committed_at_unix_seconds: u64,
    ) -> Result<Self, OldHistoryRecoveryConsentStoreError> {
        verify_plan_owner(plan, owner_account_id, owner.device_id())?;
        let signed_plan = plan.encode()?;
        let content = OldHistoryRecoveryConsentRecordContent {
            version: RECORD_VERSION,
            owner_account_id,
            owner_device_id: owner.device_id(),
            plan_id: plan.plan_id()?,
            plan_artifact_digest: plan_digest(&signed_plan),
            acceptance_receipt_id: plan.acceptance_receipt_id(),
            transition_id: plan.transition_id(),
            replacement_device_id: plan.replacement_device_id(),
            history_authority_revision: plan.history_authority_revision(),
            history_authority_digest: plan.history_authority_digest(),
            source_claim_id: plan.source_claim_id(),
            inventory_id: plan.inventory_id(),
            selection_count: u16::try_from(plan.selection_count())
                .map_err(|_| OldHistoryRecoveryConsentStoreError::InvalidRecord)?,
            plan_prepared_at_unix_seconds: plan.prepared_at_unix_seconds(),
            plan_expires_at_unix_seconds: plan.expires_at_unix_seconds(),
            committed_at_unix_seconds,
        };
        validate_record_content(&content)?;
        let signature = owner.sign(&record_signing_bytes(&content)?).to_vec();
        let record = Self {
            content,
            signed_plan,
            signature,
        };
        record.verify()?;
        Ok(record)
    }

    fn decode_and_verify(
        encoded: &[u8],
        expected_owner_account_id: AccountId,
        expected_owner_device_id: DeviceId,
    ) -> Result<Self, OldHistoryRecoveryConsentStoreError> {
        validate_record_length(encoded)?;
        let wire: SignedOldHistoryRecoveryConsentRecordWire = postcard::from_bytes(encoded)?;
        let record = Self {
            content: wire.content,
            signed_plan: wire.signed_plan,
            signature: wire.signature,
        };
        record.verify()?;
        if record.content.owner_account_id != expected_owner_account_id {
            return Err(OldHistoryRecoveryConsentStoreError::OwnerAccountMismatch);
        }
        if record.content.owner_device_id != expected_owner_device_id {
            return Err(OldHistoryRecoveryConsentStoreError::OwnerDeviceMismatch);
        }
        if record.encode()? != encoded {
            return Err(OldHistoryRecoveryConsentStoreError::NonCanonicalRecord);
        }
        Ok(record)
    }

    pub fn encode(&self) -> Result<Vec<u8>, OldHistoryRecoveryConsentStoreError> {
        self.verify()?;
        let encoded = postcard::to_allocvec(&SignedOldHistoryRecoveryConsentRecordWire {
            content: self.content.clone(),
            signed_plan: self.signed_plan.clone(),
            signature: self.signature.clone(),
        })?;
        validate_record_length(&encoded)?;
        Ok(encoded)
    }

    pub fn record_id(&self) -> Result<[u8; 32], OldHistoryRecoveryConsentStoreError> {
        Ok(domain_digest(RECORD_ID_DOMAIN, &self.encode()?))
    }

    pub fn plan_id(&self) -> [u8; 32] {
        self.content.plan_id
    }

    pub fn transition_id(&self) -> [u8; 32] {
        self.content.transition_id
    }

    pub fn committed_at_unix_seconds(&self) -> u64 {
        self.content.committed_at_unix_seconds
    }

    fn verify(&self) -> Result<SignedOldHistoryRecoveryPlan, OldHistoryRecoveryConsentStoreError> {
        validate_record_content(&self.content)?;
        self.content
            .owner_device_id
            .verify(&record_signing_bytes(&self.content)?, &self.signature)?;
        let plan = SignedOldHistoryRecoveryPlan::decode_and_verify(
            &self.signed_plan,
            self.content.owner_device_id,
            self.content.replacement_device_id,
        )?;
        if plan.history_account_id() != self.content.owner_account_id
            || plan.source_device_id() != self.content.owner_device_id
            || plan.plan_id()? != self.content.plan_id
            || plan_digest(&self.signed_plan) != self.content.plan_artifact_digest
            || plan.acceptance_receipt_id() != self.content.acceptance_receipt_id
            || plan.transition_id() != self.content.transition_id
            || plan.replacement_device_id() != self.content.replacement_device_id
            || plan.history_authority_revision() != self.content.history_authority_revision
            || plan.history_authority_digest() != self.content.history_authority_digest
            || plan.source_claim_id() != self.content.source_claim_id
            || plan.inventory_id() != self.content.inventory_id
            || plan.selection_count() != usize::from(self.content.selection_count)
            || plan.prepared_at_unix_seconds() != self.content.plan_prepared_at_unix_seconds
            || plan.expires_at_unix_seconds() != self.content.plan_expires_at_unix_seconds
        {
            return Err(OldHistoryRecoveryConsentStoreError::RecordPlanMismatch);
        }
        Ok(plan)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct OldHistoryRecoveryConsentHeadWire {
    version: u8,
    record_id: [u8; 32],
    plan_id: [u8; 32],
    transition_id: [u8; 32],
    owner_device_id: DeviceId,
    replacement_device_id: DeviceId,
    selection_count: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableOldHistoryRecoveryConsent {
    record_id: [u8; 32],
    record: SignedOldHistoryRecoveryConsentRecord,
}

impl DurableOldHistoryRecoveryConsent {
    pub fn record_id(&self) -> [u8; 32] {
        self.record_id
    }

    pub fn plan_id(&self) -> [u8; 32] {
        self.record.content.plan_id
    }

    pub fn transition_id(&self) -> [u8; 32] {
        self.record.content.transition_id
    }

    pub fn signed_plan(
        &self,
    ) -> Result<SignedOldHistoryRecoveryPlan, OldHistoryRecoveryConsentStoreError> {
        self.record.verify()
    }

    pub fn status_view(&self, now_unix_seconds: u64) -> OldHistoryRecoveryConsentStatusView {
        OldHistoryRecoveryConsentStatusView {
            record_id: self.record_id,
            plan_id: self.record.content.plan_id,
            transition_id: self.record.content.transition_id,
            source_device_token: device_token(
                self.record.content.owner_account_id,
                self.record.content.owner_device_id,
            ),
            replacement_device_token: device_token(
                self.record.content.owner_account_id,
                self.record.content.replacement_device_id,
            ),
            selection_count: self.record.content.selection_count,
            committed_at_unix_seconds: self.record.content.committed_at_unix_seconds,
            plan_expired: now_unix_seconds > self.record.content.plan_expires_at_unix_seconds,
            contains_message_bodies: false,
            execution_authorized: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OldHistoryRecoveryConsentStatusView {
    pub record_id: [u8; 32],
    pub plan_id: [u8; 32],
    pub transition_id: [u8; 32],
    pub source_device_token: [u8; 16],
    pub replacement_device_token: [u8; 16],
    pub selection_count: u16,
    pub committed_at_unix_seconds: u64,
    pub plan_expired: bool,
    pub contains_message_bodies: bool,
    pub execution_authorized: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OldHistoryRecoveryConsentStatus {
    Absent,
    Committed(OldHistoryRecoveryConsentStatusView),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OldHistoryRecoveryConsentOutcome {
    Committed(DurableOldHistoryRecoveryConsent),
    AlreadyCommitted(DurableOldHistoryRecoveryConsent),
}

pub struct OldHistoryRecoveryConsentStore {
    directory: PathBuf,
    database: Database,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

pub struct OldHistoryRecoveryConsentReadStore {
    directory: PathBuf,
    database: ReadOnlyDatabase,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

impl OldHistoryRecoveryConsentReadStore {
    pub fn open_existing(
        directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Result<Self, OldHistoryRecoveryConsentStoreError> {
        let directory = directory.as_ref().to_path_buf();
        let database_path = directory.join(DATABASE_FILE);
        if !database_path.is_file() {
            return Err(OldHistoryRecoveryConsentStoreError::StoreNotInitialized);
        }
        let database = ReadOnlyDatabase::open(database_path).map_err(database_error)?;
        verify_database_metadata(&database, owner_account_id, owner_device_id)?;
        verify_all_consents(&database, owner_account_id, owner_device_id)?;
        Ok(Self {
            directory,
            database,
            owner_account_id,
            owner_device_id,
        })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn load_for_transition(
        &self,
        transition_id: [u8; 32],
    ) -> Result<Option<DurableOldHistoryRecoveryConsent>, OldHistoryRecoveryConsentStoreError> {
        load_verified_head(
            &self.database,
            self.owner_account_id,
            self.owner_device_id,
            transition_id,
        )
    }

    pub fn status_for_transition(
        &self,
        transition_id: [u8; 32],
        now_unix_seconds: u64,
    ) -> Result<OldHistoryRecoveryConsentStatus, OldHistoryRecoveryConsentStoreError> {
        Ok(match self.load_for_transition(transition_id)? {
            Some(consent) => {
                OldHistoryRecoveryConsentStatus::Committed(consent.status_view(now_unix_seconds))
            }
            None => OldHistoryRecoveryConsentStatus::Absent,
        })
    }
}

impl OldHistoryRecoveryConsentStore {
    pub fn is_initialized(directory: impl AsRef<Path>) -> bool {
        directory.as_ref().join(DATABASE_FILE).is_file()
    }

    pub fn open(
        directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Result<Self, OldHistoryRecoveryConsentStoreError> {
        let directory = directory.as_ref().to_path_buf();
        std::fs::create_dir_all(&directory)?;
        let database_path = directory.join(DATABASE_FILE);
        let existed = database_path.is_file();
        let database = Database::create(database_path).map_err(database_error)?;
        if existed {
            verify_database_metadata(&database, owner_account_id, owner_device_id)?;
        } else {
            initialize_database(&database, owner_account_id, owner_device_id)?;
        }
        verify_all_consents(&database, owner_account_id, owner_device_id)?;
        Ok(Self {
            directory,
            database,
            owner_account_id,
            owner_device_id,
        })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn load_for_transition(
        &self,
        transition_id: [u8; 32],
    ) -> Result<Option<DurableOldHistoryRecoveryConsent>, OldHistoryRecoveryConsentStoreError> {
        load_verified_head(
            &self.database,
            self.owner_account_id,
            self.owner_device_id,
            transition_id,
        )
    }

    pub fn status_for_transition(
        &self,
        transition_id: [u8; 32],
        now_unix_seconds: u64,
    ) -> Result<OldHistoryRecoveryConsentStatus, OldHistoryRecoveryConsentStoreError> {
        Ok(match self.load_for_transition(transition_id)? {
            Some(consent) => {
                OldHistoryRecoveryConsentStatus::Committed(consent.status_view(now_unix_seconds))
            }
            None => OldHistoryRecoveryConsentStatus::Absent,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn commit(
        &self,
        owner: &DeviceIdentity,
        plan: &SignedOldHistoryRecoveryPlan,
        binding: &PeerReenrollmentHistoryBinding,
        authority: &AccountDeviceListSnapshot,
        memberships: &[ConversationMembershipSnapshot],
        inventory: &OldHistoryRecoveryInventory,
        selections: &[HistoryRecoveryRangeSelection],
        now_unix_seconds: u64,
    ) -> Result<OldHistoryRecoveryConsentOutcome, OldHistoryRecoveryConsentStoreError> {
        self.commit_inner(
            owner,
            plan,
            binding,
            authority,
            memberships,
            inventory,
            selections,
            now_unix_seconds,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn commit_inner(
        &self,
        owner: &DeviceIdentity,
        plan: &SignedOldHistoryRecoveryPlan,
        binding: &PeerReenrollmentHistoryBinding,
        authority: &AccountDeviceListSnapshot,
        memberships: &[ConversationMembershipSnapshot],
        inventory: &OldHistoryRecoveryInventory,
        selections: &[HistoryRecoveryRangeSelection],
        now_unix_seconds: u64,
        fail_before_commit: bool,
    ) -> Result<OldHistoryRecoveryConsentOutcome, OldHistoryRecoveryConsentStoreError> {
        if owner.device_id() != self.owner_device_id {
            return Err(OldHistoryRecoveryConsentStoreError::OwnerDeviceMismatch);
        }
        verify_plan_owner(plan, self.owner_account_id, self.owner_device_id)?;
        let transition_id = plan.transition_id();
        let candidate_plan_id = plan.plan_id()?;
        let candidate_plan_bytes = plan.encode()?;
        let candidate_plan_digest = plan_digest(&candidate_plan_bytes);

        if let Some(existing) = self.load_for_transition(transition_id)? {
            if existing.record.content.plan_id == candidate_plan_id
                && existing.record.content.plan_artifact_digest == candidate_plan_digest
            {
                return Ok(OldHistoryRecoveryConsentOutcome::AlreadyCommitted(existing));
            }
            return Err(OldHistoryRecoveryConsentStoreError::ConsentAlreadyCommitted);
        }

        plan.verify_context(
            binding,
            authority,
            memberships,
            inventory,
            self.owner_device_id,
            selections,
            now_unix_seconds,
        )?;
        let record = SignedOldHistoryRecoveryConsentRecord::issue(
            owner,
            self.owner_account_id,
            plan,
            now_unix_seconds,
        )?;
        let record_id = record.record_id()?;
        let record_bytes = record.encode()?;
        let head = head_wire(&record, record_id);
        let head_bytes = postcard::to_allocvec(&head)?;
        validate_head_length(&head_bytes)?;

        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut records = write.open_table(RECORD_TABLE).map_err(database_error)?;
            let mut heads = write.open_table(HEAD_TABLE).map_err(database_error)?;
            if let Some(encoded_head) = heads
                .get(transition_id.as_slice())
                .map_err(database_error)?
            {
                let existing_head = decode_head(encoded_head.value(), transition_id)?;
                let encoded_existing = records
                    .get(existing_head.record_id.as_slice())
                    .map_err(database_error)?
                    .ok_or(OldHistoryRecoveryConsentStoreError::MissingRecord)?;
                let existing = SignedOldHistoryRecoveryConsentRecord::decode_and_verify(
                    encoded_existing.value(),
                    self.owner_account_id,
                    self.owner_device_id,
                )?;
                if existing.record_id()? != existing_head.record_id
                    || existing_head != head_wire(&existing, existing_head.record_id)
                {
                    return Err(OldHistoryRecoveryConsentStoreError::HeadRecordMismatch);
                }
                let durable = DurableOldHistoryRecoveryConsent {
                    record_id: existing_head.record_id,
                    record: existing,
                };
                if durable.record.content.plan_id == candidate_plan_id
                    && durable.record.content.plan_artifact_digest == candidate_plan_digest
                {
                    return Ok(OldHistoryRecoveryConsentOutcome::AlreadyCommitted(durable));
                }
                return Err(OldHistoryRecoveryConsentStoreError::ConsentAlreadyCommitted);
            }
            if records.len().map_err(database_error)? >= MAX_DURABLE_OLD_HISTORY_RECOVERY_CONSENTS
                || heads.len().map_err(database_error)? >= MAX_DURABLE_OLD_HISTORY_RECOVERY_CONSENTS
            {
                return Err(OldHistoryRecoveryConsentStoreError::ConsentCapacity);
            }
            if let Some(existing) = records.get(record_id.as_slice()).map_err(database_error)? {
                if existing.value() != record_bytes.as_slice() {
                    return Err(OldHistoryRecoveryConsentStoreError::RecordIdCollision);
                }
                return Err(OldHistoryRecoveryConsentStoreError::OrphanRecord);
            }
            records
                .insert(record_id.as_slice(), record_bytes.as_slice())
                .map_err(database_error)?;
            heads
                .insert(transition_id.as_slice(), head_bytes.as_slice())
                .map_err(database_error)?;
        }
        if fail_before_commit {
            return Err(OldHistoryRecoveryConsentStoreError::InjectedBeforeCommit);
        }
        write.commit().map_err(database_error)?;

        let durable = self
            .load_for_transition(transition_id)?
            .ok_or(OldHistoryRecoveryConsentStoreError::MissingHead)?;
        if durable.record_id != record_id || durable.record != record {
            return Err(OldHistoryRecoveryConsentStoreError::CommittedHeadMismatch);
        }
        Ok(OldHistoryRecoveryConsentOutcome::Committed(durable))
    }

    #[cfg(test)]
    fn corrupt_record_for_test(
        &self,
        transition_id: [u8; 32],
    ) -> Result<(), OldHistoryRecoveryConsentStoreError> {
        let current = self
            .load_for_transition(transition_id)?
            .ok_or(OldHistoryRecoveryConsentStoreError::MissingHead)?;
        let mut encoded = current.record.encode()?;
        let last = encoded
            .last_mut()
            .ok_or(OldHistoryRecoveryConsentStoreError::RecordLength(0))?;
        *last ^= 1;
        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut records = write.open_table(RECORD_TABLE).map_err(database_error)?;
            records
                .insert(current.record_id.as_slice(), encoded.as_slice())
                .map_err(database_error)?;
        }
        write.commit().map_err(database_error)
    }

    #[cfg(test)]
    fn remove_head_for_test(
        &self,
        transition_id: [u8; 32],
    ) -> Result<(), OldHistoryRecoveryConsentStoreError> {
        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut heads = write.open_table(HEAD_TABLE).map_err(database_error)?;
            heads
                .remove(transition_id.as_slice())
                .map_err(database_error)?;
        }
        write.commit().map_err(database_error)
    }
}

fn verify_plan_owner(
    plan: &SignedOldHistoryRecoveryPlan,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), OldHistoryRecoveryConsentStoreError> {
    plan.encode()?;
    if plan.history_account_id() != owner_account_id {
        return Err(OldHistoryRecoveryConsentStoreError::OwnerAccountMismatch);
    }
    if plan.source_device_id() != owner_device_id {
        return Err(OldHistoryRecoveryConsentStoreError::OwnerDeviceMismatch);
    }
    Ok(())
}

fn head_wire(
    record: &SignedOldHistoryRecoveryConsentRecord,
    record_id: [u8; 32],
) -> OldHistoryRecoveryConsentHeadWire {
    OldHistoryRecoveryConsentHeadWire {
        version: HEAD_VERSION,
        record_id,
        plan_id: record.content.plan_id,
        transition_id: record.content.transition_id,
        owner_device_id: record.content.owner_device_id,
        replacement_device_id: record.content.replacement_device_id,
        selection_count: record.content.selection_count,
    }
}

fn load_verified_head<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    transition_id: [u8; 32],
) -> Result<Option<DurableOldHistoryRecoveryConsent>, OldHistoryRecoveryConsentStoreError> {
    if transition_id == [0_u8; 32] {
        return Err(OldHistoryRecoveryConsentStoreError::InvalidTransitionId);
    }
    let read = database.begin_read().map_err(database_error)?;
    let heads = read.open_table(HEAD_TABLE).map_err(database_error)?;
    let Some(encoded_head) = heads
        .get(transition_id.as_slice())
        .map_err(database_error)?
    else {
        return Ok(None);
    };
    let head = decode_head(encoded_head.value(), transition_id)?;
    let records = read.open_table(RECORD_TABLE).map_err(database_error)?;
    let encoded_record = records
        .get(head.record_id.as_slice())
        .map_err(database_error)?
        .ok_or(OldHistoryRecoveryConsentStoreError::MissingRecord)?;
    let record = SignedOldHistoryRecoveryConsentRecord::decode_and_verify(
        encoded_record.value(),
        owner_account_id,
        owner_device_id,
    )?;
    if record.record_id()? != head.record_id || head != head_wire(&record, head.record_id) {
        return Err(OldHistoryRecoveryConsentStoreError::HeadRecordMismatch);
    }
    Ok(Some(DurableOldHistoryRecoveryConsent {
        record_id: head.record_id,
        record,
    }))
}

fn verify_all_consents<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), OldHistoryRecoveryConsentStoreError> {
    let read = database.begin_read().map_err(database_error)?;
    let heads = read.open_table(HEAD_TABLE).map_err(database_error)?;
    if heads.len().map_err(database_error)? > MAX_DURABLE_OLD_HISTORY_RECOVERY_CONSENTS {
        return Err(OldHistoryRecoveryConsentStoreError::ConsentCapacity);
    }
    let mut transition_ids = Vec::new();
    let mut referenced_records = HashSet::new();
    for item in heads.iter().map_err(database_error)? {
        let (stored_transition, encoded_head) = item.map_err(database_error)?;
        let transition_id = copy_id(stored_transition.value())?;
        let head = decode_head(encoded_head.value(), transition_id)?;
        if !referenced_records.insert(head.record_id) {
            return Err(OldHistoryRecoveryConsentStoreError::DuplicateHeadRecord);
        }
        transition_ids.push(transition_id);
    }
    drop(heads);
    let records = read.open_table(RECORD_TABLE).map_err(database_error)?;
    if records.len().map_err(database_error)? > MAX_DURABLE_OLD_HISTORY_RECOVERY_CONSENTS {
        return Err(OldHistoryRecoveryConsentStoreError::ConsentCapacity);
    }
    for item in records.iter().map_err(database_error)? {
        let (stored_id, encoded_record) = item.map_err(database_error)?;
        let stored_id = copy_id(stored_id.value())?;
        if !referenced_records.contains(&stored_id) {
            return Err(OldHistoryRecoveryConsentStoreError::OrphanRecord);
        }
        let record = SignedOldHistoryRecoveryConsentRecord::decode_and_verify(
            encoded_record.value(),
            owner_account_id,
            owner_device_id,
        )?;
        if record.record_id()? != stored_id {
            return Err(OldHistoryRecoveryConsentStoreError::RecordIdBinding);
        }
    }
    drop(records);
    drop(read);
    for transition_id in transition_ids {
        if load_verified_head(database, owner_account_id, owner_device_id, transition_id)?.is_none()
        {
            return Err(OldHistoryRecoveryConsentStoreError::MissingHead);
        }
    }
    Ok(())
}

fn initialize_database(
    database: &Database,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), OldHistoryRecoveryConsentStoreError> {
    let mut write = database.begin_write().map_err(database_error)?;
    write
        .set_durability(Durability::Immediate)
        .map_err(database_error)?;
    {
        let mut meta = write.open_table(META_TABLE).map_err(database_error)?;
        initialize_meta_value(
            &mut meta,
            SCHEMA_KEY,
            &[SCHEMA_VERSION],
            OldHistoryRecoveryConsentStoreError::SchemaMismatch,
        )?;
        initialize_meta_value(
            &mut meta,
            OWNER_ACCOUNT_KEY,
            owner_account_id.as_bytes(),
            OldHistoryRecoveryConsentStoreError::OwnerAccountMismatch,
        )?;
        initialize_meta_value(
            &mut meta,
            OWNER_DEVICE_KEY,
            owner_device_id.as_bytes(),
            OldHistoryRecoveryConsentStoreError::OwnerDeviceMismatch,
        )?;
    }
    write.open_table(RECORD_TABLE).map_err(database_error)?;
    write.open_table(HEAD_TABLE).map_err(database_error)?;
    write.commit().map_err(database_error)
}

fn initialize_meta_value(
    meta: &mut redb::Table<'_, &str, &[u8]>,
    key: &'static str,
    expected: &[u8],
    mismatch: OldHistoryRecoveryConsentStoreError,
) -> Result<(), OldHistoryRecoveryConsentStoreError> {
    let current = meta
        .get(key)
        .map_err(database_error)?
        .map(|value| value.value().to_vec());
    match current {
        Some(value) if value == expected => Ok(()),
        Some(_) => Err(mismatch),
        None => {
            meta.insert(key, expected).map_err(database_error)?;
            Ok(())
        }
    }
}

fn verify_database_metadata<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), OldHistoryRecoveryConsentStoreError> {
    let read = database.begin_read().map_err(database_error)?;
    let meta = read.open_table(META_TABLE).map_err(database_error)?;
    verify_meta_value(&meta, SCHEMA_KEY, &[SCHEMA_VERSION], || {
        OldHistoryRecoveryConsentStoreError::SchemaMismatch
    })?;
    verify_meta_value(
        &meta,
        OWNER_ACCOUNT_KEY,
        owner_account_id.as_bytes(),
        || OldHistoryRecoveryConsentStoreError::OwnerAccountMismatch,
    )?;
    verify_meta_value(&meta, OWNER_DEVICE_KEY, owner_device_id.as_bytes(), || {
        OldHistoryRecoveryConsentStoreError::OwnerDeviceMismatch
    })?;
    drop(meta);
    let records = read.open_table(RECORD_TABLE).map_err(database_error)?;
    drop(records);
    let heads = read.open_table(HEAD_TABLE).map_err(database_error)?;
    drop(heads);
    Ok(())
}

fn verify_meta_value(
    meta: &impl ReadableTable<&'static str, &'static [u8]>,
    key: &'static str,
    expected: &[u8],
    mismatch: impl Fn() -> OldHistoryRecoveryConsentStoreError,
) -> Result<(), OldHistoryRecoveryConsentStoreError> {
    let value = meta
        .get(key)
        .map_err(database_error)?
        .ok_or_else(&mismatch)?;
    if value.value() != expected {
        return Err(mismatch());
    }
    Ok(())
}

fn validate_record_content(
    content: &OldHistoryRecoveryConsentRecordContent,
) -> Result<(), OldHistoryRecoveryConsentStoreError> {
    if content.version != RECORD_VERSION
        || content.owner_device_id == content.replacement_device_id
        || content.plan_id == [0_u8; 32]
        || content.plan_artifact_digest == [0_u8; 32]
        || content.acceptance_receipt_id == [0_u8; 32]
        || content.transition_id == [0_u8; 32]
        || content.history_authority_revision == 0
        || content.history_authority_digest == [0_u8; 32]
        || content.source_claim_id == [0_u8; 32]
        || content.inventory_id == [0_u8; 32]
        || content.selection_count == 0
        || usize::from(content.selection_count) > MAX_HISTORY_CONVERSATIONS
        || content.plan_prepared_at_unix_seconds >= content.plan_expires_at_unix_seconds
        || content.committed_at_unix_seconds < content.plan_prepared_at_unix_seconds
        || content.committed_at_unix_seconds > content.plan_expires_at_unix_seconds
    {
        return Err(OldHistoryRecoveryConsentStoreError::InvalidRecord);
    }
    Ok(())
}

fn validate_head(
    head: &OldHistoryRecoveryConsentHeadWire,
    expected_transition_id: [u8; 32],
) -> Result<(), OldHistoryRecoveryConsentStoreError> {
    if head.version != HEAD_VERSION
        || head.record_id == [0_u8; 32]
        || head.plan_id == [0_u8; 32]
        || expected_transition_id == [0_u8; 32]
        || head.transition_id != expected_transition_id
        || head.owner_device_id == head.replacement_device_id
        || head.selection_count == 0
        || usize::from(head.selection_count) > MAX_HISTORY_CONVERSATIONS
    {
        return Err(OldHistoryRecoveryConsentStoreError::InvalidHead);
    }
    Ok(())
}

fn decode_head(
    encoded: &[u8],
    expected_transition_id: [u8; 32],
) -> Result<OldHistoryRecoveryConsentHeadWire, OldHistoryRecoveryConsentStoreError> {
    validate_head_length(encoded)?;
    let head: OldHistoryRecoveryConsentHeadWire = postcard::from_bytes(encoded)?;
    validate_head(&head, expected_transition_id)?;
    if postcard::to_allocvec(&head)? != encoded {
        return Err(OldHistoryRecoveryConsentStoreError::NonCanonicalHead);
    }
    Ok(head)
}

fn validate_record_length(encoded: &[u8]) -> Result<(), OldHistoryRecoveryConsentStoreError> {
    if encoded.is_empty() || encoded.len() > MAX_OLD_HISTORY_RECOVERY_CONSENT_RECORD_BYTES {
        return Err(OldHistoryRecoveryConsentStoreError::RecordLength(
            encoded.len(),
        ));
    }
    Ok(())
}

fn validate_head_length(encoded: &[u8]) -> Result<(), OldHistoryRecoveryConsentStoreError> {
    if encoded.is_empty() || encoded.len() > MAX_OLD_HISTORY_RECOVERY_CONSENT_HEAD_BYTES {
        return Err(OldHistoryRecoveryConsentStoreError::HeadLength(
            encoded.len(),
        ));
    }
    Ok(())
}

fn record_signing_bytes(
    content: &OldHistoryRecoveryConsentRecordContent,
) -> Result<Vec<u8>, OldHistoryRecoveryConsentStoreError> {
    validate_record_content(content)?;
    let encoded = postcard::to_allocvec(content)?;
    let mut signing = Vec::with_capacity(RECORD_SIGNATURE_DOMAIN.len() + encoded.len());
    signing.extend_from_slice(RECORD_SIGNATURE_DOMAIN);
    signing.extend_from_slice(&encoded);
    Ok(signing)
}

fn plan_digest(encoded: &[u8]) -> [u8; 32] {
    domain_digest(PLAN_DIGEST_DOMAIN, encoded)
}

fn device_token(account_id: AccountId, device_id: DeviceId) -> [u8; 16] {
    let mut bytes = Vec::with_capacity(64);
    bytes.extend_from_slice(account_id.as_bytes());
    bytes.extend_from_slice(device_id.as_bytes());
    let digest = domain_digest(DEVICE_TOKEN_DOMAIN, &bytes);
    let mut token = [0_u8; 16];
    token.copy_from_slice(&digest[..16]);
    token
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

fn copy_id(bytes: &[u8]) -> Result<[u8; 32], OldHistoryRecoveryConsentStoreError> {
    bytes
        .try_into()
        .map_err(|_| OldHistoryRecoveryConsentStoreError::StoredIdLength(bytes.len()))
}

fn database_error(error: impl std::fmt::Display) -> OldHistoryRecoveryConsentStoreError {
    OldHistoryRecoveryConsentStoreError::Database(error.to_string())
}

#[derive(Debug, Error)]
pub enum OldHistoryRecoveryConsentStoreError {
    #[error("old-history recovery consent store is not initialized")]
    StoreNotInitialized,
    #[error("old-history recovery consent store I/O failed")]
    Io(#[from] std::io::Error),
    #[error("old-history recovery consent database failed: {0}")]
    Database(String),
    #[error("old-history recovery consent schema does not match")]
    SchemaMismatch,
    #[error("old-history recovery consent owner Account does not match")]
    OwnerAccountMismatch,
    #[error("old-history recovery consent owner Device does not match")]
    OwnerDeviceMismatch,
    #[error("old-history recovery consent record is invalid")]
    InvalidRecord,
    #[error("old-history recovery consent head is invalid")]
    InvalidHead,
    #[error("old-history recovery transition ID is invalid")]
    InvalidTransitionId,
    #[error("old-history recovery consent record is {0} bytes and violates its bound")]
    RecordLength(usize),
    #[error("old-history recovery consent head is {0} bytes and violates its bound")]
    HeadLength(usize),
    #[error("old-history recovery consent record is not canonically encoded")]
    NonCanonicalRecord,
    #[error("old-history recovery consent head is not canonically encoded")]
    NonCanonicalHead,
    #[error("old-history recovery consent record does not match its signed plan")]
    RecordPlanMismatch,
    #[error("old-history recovery consent head references a missing record")]
    MissingRecord,
    #[error("old-history recovery consent store has no expected head")]
    MissingHead,
    #[error("old-history recovery consent head does not match its record")]
    HeadRecordMismatch,
    #[error("old-history recovery consent record ID does not bind its bytes")]
    RecordIdBinding,
    #[error("distinct old-history recovery consent records produced the same ID")]
    RecordIdCollision,
    #[error("old-history recovery consent store contains an orphan record")]
    OrphanRecord,
    #[error("multiple old-history recovery heads reference one record")]
    DuplicateHeadRecord,
    #[error("another old-history recovery consent is already committed for this transition")]
    ConsentAlreadyCommitted,
    #[error("old-history recovery consent committed an unexpected head")]
    CommittedHeadMismatch,
    #[error("old-history recovery consent capacity is exhausted")]
    ConsentCapacity,
    #[error("stored old-history recovery consent ID has invalid length {0}")]
    StoredIdLength(usize),
    #[error("test-only failure injected before the atomic consent commit")]
    InjectedBeforeCommit,
    #[error("old-history recovery consent encoding is invalid: {0}")]
    Codec(#[from] postcard::Error),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Recovery(#[from] PeerHistoryRecoveryError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use kilogram_identity::{AccountRootState, DeviceCapability, DeviceState};
    use kilogram_peer_capability_store::{PeerCapabilityKey, PeerCapabilityStore};
    use kilogram_peer_history_recovery::{
        ConversationHistoryCoverage, ExplicitOldHistoryRecoveryConsent, OldHistoryRecoveryPlan,
        ReadableHistoryRange, SignedHistorySourceClaim,
    };
    use kilogram_peer_reenrollment::{ExplicitPeerReenrollmentApproval, PeerReenrollmentPlan};
    use kilogram_peer_reenrollment_acceptance::{
        DurablePeerReenrollmentAcceptance, PeerReenrollmentAcceptanceOutcome,
        PeerReenrollmentAcceptanceStore,
    };
    use kilogram_peer_reenrollment_runtime::{
        PeerReenrollmentRuntimeBinding, PeerReenrollmentRuntimeConsumeOutcome,
        PeerReenrollmentRuntimePrepareOutcome, PeerReenrollmentRuntimeStore,
    };
    use kilogram_protocol::ConversationId;
    use kilogram_sync_capability::{
        CurrentPeerInvitationBinding, PeerSyncCapabilityEvidence, SignedCurrentPeerInvitation,
        SyncTransportCapabilityError,
    };
    use tempfile::TempDir;

    struct Fixture {
        root: TempDir,
        local_account_id: AccountId,
        owner_account_id: AccountId,
        source: DeviceIdentity,
        replacement: DeviceIdentity,
        authority: AccountDeviceListSnapshot,
        membership: ConversationMembershipSnapshot,
        binding: PeerReenrollmentHistoryBinding,
        inventory: OldHistoryRecoveryInventory,
        selections: Vec<HistoryRecoveryRangeSelection>,
        signed_plan: SignedOldHistoryRecoveryPlan,
    }

    fn fixture() -> Result<Fixture, Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let local_root = AccountRootState::create(root.path().join("local-root"))?;
        let local_device = DeviceState::load_or_create(root.path().join("local-device"))?;
        let local_certificate = local_root.issue_device_certificate(
            local_device.identity().device_id(),
            local_device.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let local_authority = local_root.publish_device_list(&[local_certificate])?;

        let history_root = AccountRootState::create(root.path().join("history-root"))?;
        let source_state = DeviceState::load_or_create(root.path().join("history-source"))?;
        let source_certificate = history_root.issue_device_certificate(
            source_state.identity().device_id(),
            source_state.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let old_authority =
            history_root.publish_device_list(std::slice::from_ref(&source_certificate))?;
        let old_invitation = invitation(
            source_state.identity(),
            history_root.account_id(),
            local_root.account_id(),
            old_authority.revision(),
            7,
            [7_u8; 32],
        )?;
        let capability = PeerCapabilityStore::open(
            root.path().join("peer-capability"),
            local_device.identity().device_id(),
        )?;
        capability.apply_current_invitation(
            local_device.identity(),
            local_root.account_id(),
            &old_invitation,
            1_100,
        )?;
        let old_key = PeerCapabilityKey::new(
            history_root.account_id(),
            source_state.identity().device_id(),
        );
        capability.apply_authenticated_observation(
            local_device.identity(),
            old_key,
            PeerSyncCapabilityEvidence::LegacyTicketWithoutExtension,
        )?;
        let quarantine = capability.load_quarantined_head(old_key)?;

        let replacement_state = DeviceState::load_or_create(root.path().join("replacement"))?;
        let replacement_certificate = history_root.issue_device_certificate(
            replacement_state.identity().device_id(),
            replacement_state.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let authority =
            history_root.publish_device_list(&[source_certificate, replacement_certificate])?;
        let current_invitation = invitation(
            replacement_state.identity(),
            history_root.account_id(),
            local_root.account_id(),
            authority.revision(),
            1,
            [9_u8; 32],
        )?;
        let acceptance = accept_reenrollment(
            root.path(),
            local_device.identity(),
            &local_authority,
            &quarantine,
            &authority,
            &current_invitation,
        )?;
        let runtime_store = PeerReenrollmentRuntimeStore::open(
            root.path().join("runtime-transition"),
            local_root.account_id(),
            local_device.identity().device_id(),
        )?;
        let prepared = runtime_store.prepare(
            local_device.identity(),
            &acceptance,
            &current_invitation,
            PeerReenrollmentRuntimeBinding {
                contact_id: [11_u8; 32],
                conversation_id: [12_u8; 32],
                predecessor_contact: vec![1, 2, 3],
                successor_contact: vec![4, 5, 6],
                successor_endpoint_candidate: vec![7, 8, 9],
            },
            1_350,
        )?;
        let prepared = match prepared {
            PeerReenrollmentRuntimePrepareOutcome::Prepared(value)
            | PeerReenrollmentRuntimePrepareOutcome::AlreadyPrepared(value) => value,
        };
        let consumed =
            runtime_store.consume(prepared.transition().contact_id(), prepared.transition_id())?;
        let consumed = match consumed {
            PeerReenrollmentRuntimeConsumeOutcome::Consumed(value)
            | PeerReenrollmentRuntimeConsumeOutcome::AlreadyConsumed(value) => value,
        };
        let binding = PeerReenrollmentHistoryBinding::from_consumed_transition(
            acceptance.receipt(),
            &consumed,
            &authority,
        )?;

        let conversation_id = ConversationId::from_label("durable-old-history-consent");
        let membership = history_root.create_conversation_membership(
            conversation_id.scope_id(),
            &[local_root.account_id()],
        )?;
        let source = DeviceIdentity::from_secret_bytes(source_state.identity().secret_bytes());
        let replacement =
            DeviceIdentity::from_secret_bytes(replacement_state.identity().secret_bytes());
        let coverage = ConversationHistoryCoverage::new(
            conversation_id,
            12,
            [21_u8; 32],
            vec![ReadableHistoryRange::new(0, 12)?],
        )?;
        let source_claim = SignedHistorySourceClaim::issue(&source, &authority, vec![coverage])?;
        let inventory = OldHistoryRecoveryInventory::assemble(
            &authority,
            replacement.device_id(),
            vec![source_claim],
            &[],
        )?;
        let selections = vec![HistoryRecoveryRangeSelection {
            conversation_id,
            range_start: 0,
            range_end: 12,
        }];
        let plan = OldHistoryRecoveryPlan::prepare(
            &binding,
            &authority,
            std::slice::from_ref(&membership),
            &inventory,
            source.device_id(),
            &selections,
            1_400,
        )?;
        let phrase = plan.confirmation_phrase()?;
        let signed_plan = plan.confirm_and_sign(
            &source,
            &phrase,
            ExplicitOldHistoryRecoveryConsent::HistoryOwnerApproved,
        )?;
        Ok(Fixture {
            root,
            local_account_id: local_root.account_id(),
            owner_account_id: history_root.account_id(),
            source,
            replacement,
            authority,
            membership,
            binding,
            inventory,
            selections,
            signed_plan,
        })
    }

    fn invitation(
        listener: &DeviceIdentity,
        listener_account_id: AccountId,
        requester_account_id: AccountId,
        authority_revision: u64,
        epoch: u64,
        capability_id: [u8; 32],
    ) -> Result<SignedCurrentPeerInvitation, SyncTransportCapabilityError> {
        SignedCurrentPeerInvitation::issue(
            listener,
            &CurrentPeerInvitationBinding {
                listener_account_id,
                listener_device_id: listener.device_id(),
                allowed_requester_account_id: requester_account_id,
                authority_revision,
                endpoint_id: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                ticket_publication_channel_epoch: epoch,
                issued_at_unix_seconds: 1_000,
                expires_at_unix_seconds: 2_000,
                connection_ticket: "Exact_current_ticket_01",
                required_capability_id: capability_id,
            },
        )
    }

    fn accept_reenrollment(
        root: &Path,
        local_identity: &DeviceIdentity,
        local_authority: &AccountDeviceListSnapshot,
        quarantine: &kilogram_peer_capability_store::DurablePeerCapabilityQuarantineHead,
        peer_authority: &AccountDeviceListSnapshot,
        invitation: &SignedCurrentPeerInvitation,
    ) -> Result<DurablePeerReenrollmentAcceptance, Box<dyn std::error::Error>> {
        let plan = PeerReenrollmentPlan::prepare(
            local_identity.device_id(),
            local_authority,
            quarantine,
            peer_authority,
            invitation,
            1_200,
        )?;
        let phrase = plan.confirmation_phrase()?;
        let tombstone = plan.confirm_and_sign(
            local_identity,
            &phrase,
            ExplicitPeerReenrollmentApproval::UserOrTestOperatorApproved,
        )?;
        let store = PeerReenrollmentAcceptanceStore::open(
            root.join("peer-reenrollment-acceptance"),
            local_authority.account_id(),
            local_identity.device_id(),
        )?;
        Ok(
            match store.accept(
                local_identity,
                &tombstone,
                local_authority,
                quarantine,
                peer_authority,
                invitation,
                1_300,
            )? {
                PeerReenrollmentAcceptanceOutcome::Accepted(value)
                | PeerReenrollmentAcceptanceOutcome::AlreadyAccepted(value) => value,
            },
        )
    }

    fn open_store(
        fixture: &Fixture,
        name: &str,
    ) -> Result<OldHistoryRecoveryConsentStore, OldHistoryRecoveryConsentStoreError> {
        OldHistoryRecoveryConsentStore::open(
            fixture.root.path().join(name),
            fixture.owner_account_id,
            fixture.source.device_id(),
        )
    }

    fn commit(
        store: &OldHistoryRecoveryConsentStore,
        fixture: &Fixture,
        plan: &SignedOldHistoryRecoveryPlan,
        selections: &[HistoryRecoveryRangeSelection],
        now_unix_seconds: u64,
    ) -> Result<OldHistoryRecoveryConsentOutcome, OldHistoryRecoveryConsentStoreError> {
        store.commit(
            &fixture.source,
            plan,
            &fixture.binding,
            &fixture.authority,
            std::slice::from_ref(&fixture.membership),
            &fixture.inventory,
            selections,
            now_unix_seconds,
        )
    }

    fn another_plan(
        fixture: &Fixture,
        prepared_at_unix_seconds: u64,
    ) -> Result<SignedOldHistoryRecoveryPlan, PeerHistoryRecoveryError> {
        let plan = OldHistoryRecoveryPlan::prepare(
            &fixture.binding,
            &fixture.authority,
            std::slice::from_ref(&fixture.membership),
            &fixture.inventory,
            fixture.source.device_id(),
            &fixture.selections,
            prepared_at_unix_seconds,
        )?;
        let phrase = plan.confirmation_phrase()?;
        plan.confirm_and_sign(
            &fixture.source,
            &phrase,
            ExplicitOldHistoryRecoveryConsent::HistoryOwnerApproved,
        )
    }

    #[test]
    fn atomic_commit_reopen_and_privacy_safe_status() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let directory = fixture.root.path().join("consent");
        assert!(matches!(
            OldHistoryRecoveryConsentReadStore::open_existing(
                &directory,
                fixture.owner_account_id,
                fixture.source.device_id(),
            ),
            Err(OldHistoryRecoveryConsentStoreError::StoreNotInitialized)
        ));
        let store = open_store(&fixture, "consent")?;
        assert_eq!(
            store.status_for_transition(fixture.signed_plan.transition_id(), 1_500)?,
            OldHistoryRecoveryConsentStatus::Absent
        );
        let OldHistoryRecoveryConsentOutcome::Committed(committed) = commit(
            &store,
            &fixture,
            &fixture.signed_plan,
            &fixture.selections,
            1_500,
        )?
        else {
            return Err(OldHistoryRecoveryConsentStoreError::CommittedHeadMismatch.into());
        };
        let view = committed.status_view(1_500);
        assert!(!view.contains_message_bodies);
        assert!(!view.execution_authorized);
        assert!(!view.plan_expired);
        assert_ne!(view.source_device_token, [0_u8; 16]);
        assert_ne!(view.source_device_token, view.replacement_device_token);
        assert_eq!(view.selection_count, 1);
        assert_eq!(committed.signed_plan()?.plan_id()?, view.plan_id);
        drop(store);

        let read = OldHistoryRecoveryConsentReadStore::open_existing(
            &directory,
            fixture.owner_account_id,
            fixture.source.device_id(),
        )?;
        let OldHistoryRecoveryConsentStatus::Committed(reopened) =
            read.status_for_transition(fixture.signed_plan.transition_id(), 2_500)?
        else {
            return Err(OldHistoryRecoveryConsentStoreError::MissingHead.into());
        };
        assert_eq!(reopened.record_id, view.record_id);
        assert!(reopened.plan_expired);
        assert!(!reopened.execution_authorized);
        Ok(())
    }

    #[test]
    fn exact_replay_after_expiry_is_idempotent() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let store = open_store(&fixture, "replay")?;
        let first = commit(
            &store,
            &fixture,
            &fixture.signed_plan,
            &fixture.selections,
            1_500,
        )?;
        let first = match first {
            OldHistoryRecoveryConsentOutcome::Committed(value) => value,
            OldHistoryRecoveryConsentOutcome::AlreadyCommitted(_) => {
                return Err(OldHistoryRecoveryConsentStoreError::CommittedHeadMismatch.into());
            }
        };
        let replay = commit(
            &store,
            &fixture,
            &fixture.signed_plan,
            &fixture.selections,
            9_999,
        )?;
        let OldHistoryRecoveryConsentOutcome::AlreadyCommitted(replay) = replay else {
            return Err(OldHistoryRecoveryConsentStoreError::CommittedHeadMismatch.into());
        };
        assert_eq!(replay.record_id(), first.record_id());
        assert_eq!(replay.record.committed_at_unix_seconds(), 1_500);
        Ok(())
    }

    #[test]
    fn competing_plan_for_same_transition_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let store = open_store(&fixture, "competing")?;
        let first = commit(
            &store,
            &fixture,
            &fixture.signed_plan,
            &fixture.selections,
            1_500,
        )?;
        let first_id = match first {
            OldHistoryRecoveryConsentOutcome::Committed(value) => value.record_id(),
            OldHistoryRecoveryConsentOutcome::AlreadyCommitted(_) => {
                return Err(OldHistoryRecoveryConsentStoreError::CommittedHeadMismatch.into());
            }
        };
        let competing = another_plan(&fixture, 1_401)?;
        assert!(matches!(
            commit(&store, &fixture, &competing, &fixture.selections, 1_501,),
            Err(OldHistoryRecoveryConsentStoreError::ConsentAlreadyCommitted)
        ));
        assert_eq!(
            store
                .load_for_transition(fixture.signed_plan.transition_id())?
                .ok_or(OldHistoryRecoveryConsentStoreError::MissingHead)?
                .record_id(),
            first_id
        );
        Ok(())
    }

    #[test]
    fn first_commit_rechecks_expiry_context_and_owner() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let expired = open_store(&fixture, "expired")?;
        assert!(matches!(
            commit(
                &expired,
                &fixture,
                &fixture.signed_plan,
                &fixture.selections,
                9_999,
            ),
            Err(OldHistoryRecoveryConsentStoreError::Recovery(
                PeerHistoryRecoveryError::PlanExpired
            ))
        ));
        assert!(
            expired
                .load_for_transition(fixture.signed_plan.transition_id())?
                .is_none()
        );

        let changed = open_store(&fixture, "changed")?;
        let changed_selections = [HistoryRecoveryRangeSelection {
            conversation_id: fixture.selections[0].conversation_id,
            range_start: 0,
            range_end: 6,
        }];
        assert!(matches!(
            commit(
                &changed,
                &fixture,
                &fixture.signed_plan,
                &changed_selections,
                1_500,
            ),
            Err(OldHistoryRecoveryConsentStoreError::Recovery(
                PeerHistoryRecoveryError::PlanContextMismatch
            ))
        ));
        assert!(matches!(
            changed.commit(
                &fixture.replacement,
                &fixture.signed_plan,
                &fixture.binding,
                &fixture.authority,
                std::slice::from_ref(&fixture.membership),
                &fixture.inventory,
                &fixture.selections,
                1_500,
            ),
            Err(OldHistoryRecoveryConsentStoreError::OwnerDeviceMismatch)
        ));
        drop(changed);
        assert!(matches!(
            OldHistoryRecoveryConsentReadStore::open_existing(
                fixture.root.path().join("changed"),
                fixture.local_account_id,
                fixture.source.device_id(),
            ),
            Err(OldHistoryRecoveryConsentStoreError::OwnerAccountMismatch)
        ));
        Ok(())
    }

    #[test]
    fn pre_commit_failure_is_empty_and_retryable() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let store = open_store(&fixture, "crash")?;
        assert!(matches!(
            store.commit_inner(
                &fixture.source,
                &fixture.signed_plan,
                &fixture.binding,
                &fixture.authority,
                std::slice::from_ref(&fixture.membership),
                &fixture.inventory,
                &fixture.selections,
                1_500,
                true,
            ),
            Err(OldHistoryRecoveryConsentStoreError::InjectedBeforeCommit)
        ));
        assert!(
            store
                .load_for_transition(fixture.signed_plan.transition_id())?
                .is_none()
        );
        assert!(matches!(
            commit(
                &store,
                &fixture,
                &fixture.signed_plan,
                &fixture.selections,
                1_500,
            )?,
            OldHistoryRecoveryConsentOutcome::Committed(_)
        ));
        Ok(())
    }

    #[test]
    fn corruption_and_orphaned_record_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let corrupt_directory = fixture.root.path().join("corrupt");
        let corrupt = open_store(&fixture, "corrupt")?;
        commit(
            &corrupt,
            &fixture,
            &fixture.signed_plan,
            &fixture.selections,
            1_500,
        )?;
        corrupt.corrupt_record_for_test(fixture.signed_plan.transition_id())?;
        assert!(
            corrupt
                .load_for_transition(fixture.signed_plan.transition_id())
                .is_err()
        );
        drop(corrupt);
        assert!(
            OldHistoryRecoveryConsentReadStore::open_existing(
                corrupt_directory,
                fixture.owner_account_id,
                fixture.source.device_id(),
            )
            .is_err()
        );

        let orphan_directory = fixture.root.path().join("orphan");
        let orphan = open_store(&fixture, "orphan")?;
        commit(
            &orphan,
            &fixture,
            &fixture.signed_plan,
            &fixture.selections,
            1_500,
        )?;
        orphan.remove_head_for_test(fixture.signed_plan.transition_id())?;
        drop(orphan);
        assert!(matches!(
            OldHistoryRecoveryConsentReadStore::open_existing(
                orphan_directory,
                fixture.owner_account_id,
                fixture.source.device_id(),
            ),
            Err(OldHistoryRecoveryConsentStoreError::OrphanRecord)
        ));
        Ok(())
    }
}
