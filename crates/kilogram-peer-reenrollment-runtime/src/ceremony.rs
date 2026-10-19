use std::path::{Path, PathBuf};

use kilogram_identity::{AccountId, DeviceId, DeviceIdentity, IdentityError};
use kilogram_peer_reenrollment::{
    MAX_PEER_REENROLLMENT_PLAN_BYTES, PeerReenrollmentError, PeerReenrollmentPlan,
    SignedPeerReenrollmentTombstone,
};
use redb::{
    Database, Durability, ReadOnlyDatabase, ReadableDatabase, ReadableTable, ReadableTableMetadata,
    TableDefinition,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const DATABASE_FILE: &str = "peer-reenrollment-runtime-ceremony-v1.redb";
const META_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("peer-reenrollment-runtime-ceremony-meta-v1");
const CEREMONY_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("peer-reenrollment-runtime-ceremony-v1");

const SCHEMA_KEY: &str = "schema-version";
const OWNER_ACCOUNT_KEY: &str = "owner-account-id";
const OWNER_DEVICE_KEY: &str = "owner-device-id";
const SCHEMA_VERSION: u8 = 1;
const RECORD_VERSION: u8 = 1;
const SIGNATURE_DOMAIN: &[u8] = b"kilogram:peer-reenrollment-runtime-ceremony:v1\0";
const TOMBSTONE_ID_DOMAIN: &[u8] = b"kilogram:peer-reenrollment-tombstone-id:v1\0";

pub const MAX_PEER_REENROLLMENT_RUNTIME_CEREMONIES: u64 = 4_096;
pub const MAX_PEER_REENROLLMENT_RUNTIME_CEREMONY_BYTES: usize =
    (MAX_PEER_REENROLLMENT_PLAN_BYTES * 2) + 4 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum PeerReenrollmentRuntimeCeremonyPhase {
    Planned,
    Approved,
    AcceptanceCommitted,
    Complete,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PeerReenrollmentRuntimeCeremonyContent {
    version: u8,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    contact_id: [u8; 32],
    staged_invitation_id: [u8; 32],
    invitation_id: [u8; 32],
    quarantine_record_id: [u8; 32],
    plan: Vec<u8>,
    tombstone: Option<Vec<u8>>,
    acceptance_receipt_id: Option<[u8; 32]>,
    runtime_transition_id: Option<[u8; 32]>,
    phase: PeerReenrollmentRuntimeCeremonyPhase,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SignedPeerReenrollmentRuntimeCeremonyWire {
    content: PeerReenrollmentRuntimeCeremonyContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SignedPeerReenrollmentRuntimeCeremony {
    content: PeerReenrollmentRuntimeCeremonyContent,
    signature: Vec<u8>,
}

impl SignedPeerReenrollmentRuntimeCeremony {
    fn issue(
        owner: &DeviceIdentity,
        content: PeerReenrollmentRuntimeCeremonyContent,
    ) -> Result<Self, PeerReenrollmentRuntimeCeremonyError> {
        if owner.device_id() != content.owner_device_id {
            return Err(PeerReenrollmentRuntimeCeremonyError::OwnerDeviceMismatch);
        }
        validate_content(&content)?;
        let signature = owner.sign(&signing_bytes(&content)?).to_vec();
        let record = Self { content, signature };
        record.verify()?;
        Ok(record)
    }

    fn decode_and_verify(
        encoded: &[u8],
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Result<Self, PeerReenrollmentRuntimeCeremonyError> {
        if encoded.is_empty() || encoded.len() > MAX_PEER_REENROLLMENT_RUNTIME_CEREMONY_BYTES {
            return Err(PeerReenrollmentRuntimeCeremonyError::RecordLength(
                encoded.len(),
            ));
        }
        let wire: SignedPeerReenrollmentRuntimeCeremonyWire = postcard::from_bytes(encoded)?;
        let record = Self {
            content: wire.content,
            signature: wire.signature,
        };
        record.verify()?;
        if record.content.owner_account_id != owner_account_id {
            return Err(PeerReenrollmentRuntimeCeremonyError::OwnerAccountMismatch);
        }
        if record.content.owner_device_id != owner_device_id {
            return Err(PeerReenrollmentRuntimeCeremonyError::OwnerDeviceMismatch);
        }
        if record.encode()? != encoded {
            return Err(PeerReenrollmentRuntimeCeremonyError::NonCanonicalRecord);
        }
        Ok(record)
    }

    fn encode(&self) -> Result<Vec<u8>, PeerReenrollmentRuntimeCeremonyError> {
        self.verify()?;
        let encoded = postcard::to_allocvec(&SignedPeerReenrollmentRuntimeCeremonyWire {
            content: self.content.clone(),
            signature: self.signature.clone(),
        })?;
        if encoded.is_empty() || encoded.len() > MAX_PEER_REENROLLMENT_RUNTIME_CEREMONY_BYTES {
            return Err(PeerReenrollmentRuntimeCeremonyError::RecordLength(
                encoded.len(),
            ));
        }
        Ok(encoded)
    }

    fn verify(&self) -> Result<(), PeerReenrollmentRuntimeCeremonyError> {
        validate_content(&self.content)?;
        self.content
            .owner_device_id
            .verify(&signing_bytes(&self.content)?, &self.signature)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurablePeerReenrollmentRuntimeCeremony {
    record: SignedPeerReenrollmentRuntimeCeremony,
}

impl DurablePeerReenrollmentRuntimeCeremony {
    pub fn owner_account_id(&self) -> AccountId {
        self.record.content.owner_account_id
    }

    pub fn owner_device_id(&self) -> DeviceId {
        self.record.content.owner_device_id
    }

    pub fn contact_id(&self) -> [u8; 32] {
        self.record.content.contact_id
    }

    pub fn staged_invitation_id(&self) -> [u8; 32] {
        self.record.content.staged_invitation_id
    }

    pub fn invitation_id(&self) -> [u8; 32] {
        self.record.content.invitation_id
    }

    pub fn quarantine_record_id(&self) -> [u8; 32] {
        self.record.content.quarantine_record_id
    }

    pub fn phase(&self) -> PeerReenrollmentRuntimeCeremonyPhase {
        self.record.content.phase
    }

    pub fn plan(&self) -> Result<PeerReenrollmentPlan, PeerReenrollmentRuntimeCeremonyError> {
        Ok(PeerReenrollmentPlan::decode_and_validate(
            &self.record.content.plan,
            self.owner_device_id(),
        )?)
    }

    pub fn plan_id(&self) -> Result<[u8; 32], PeerReenrollmentRuntimeCeremonyError> {
        Ok(self.plan()?.plan_id()?)
    }

    pub fn tombstone(
        &self,
    ) -> Result<Option<SignedPeerReenrollmentTombstone>, PeerReenrollmentRuntimeCeremonyError> {
        self.record
            .content
            .tombstone
            .as_deref()
            .map(|encoded| {
                SignedPeerReenrollmentTombstone::decode_and_verify(encoded, self.owner_device_id())
                    .map_err(Into::into)
            })
            .transpose()
    }

    pub fn tombstone_id(&self) -> Result<Option<[u8; 32]>, PeerReenrollmentRuntimeCeremonyError> {
        self.record
            .content
            .tombstone
            .as_deref()
            .map(peer_reenrollment_tombstone_id_from_bytes)
            .transpose()
    }

    pub fn acceptance_receipt_id(&self) -> Option<[u8; 32]> {
        self.record.content.acceptance_receipt_id
    }

    pub fn runtime_transition_id(&self) -> Option<[u8; 32]> {
        self.record.content.runtime_transition_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PeerReenrollmentRuntimeCeremonyWriteOutcome {
    Written(DurablePeerReenrollmentRuntimeCeremony),
    AlreadyPresent(DurablePeerReenrollmentRuntimeCeremony),
}

impl PeerReenrollmentRuntimeCeremonyWriteOutcome {
    pub fn ceremony(&self) -> &DurablePeerReenrollmentRuntimeCeremony {
        match self {
            Self::Written(ceremony) | Self::AlreadyPresent(ceremony) => ceremony,
        }
    }
}

pub struct PeerReenrollmentRuntimeCeremonyStore {
    directory: PathBuf,
    database: Database,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

pub struct PeerReenrollmentRuntimeCeremonyReadStore {
    directory: PathBuf,
    database: ReadOnlyDatabase,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

impl PeerReenrollmentRuntimeCeremonyStore {
    pub fn is_initialized(directory: impl AsRef<Path>) -> bool {
        directory.as_ref().join(DATABASE_FILE).is_file()
    }

    pub fn open(
        directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Result<Self, PeerReenrollmentRuntimeCeremonyError> {
        let directory = directory.as_ref().to_path_buf();
        std::fs::create_dir_all(&directory)?;
        let database = Database::create(directory.join(DATABASE_FILE)).map_err(database_error)?;
        initialize_database(&database, owner_account_id, owner_device_id)?;
        verify_all(&database, owner_account_id, owner_device_id)?;
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

    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &self,
        owner: &DeviceIdentity,
        contact_id: [u8; 32],
        staged_invitation_id: [u8; 32],
        invitation_id: [u8; 32],
        quarantine_record_id: [u8; 32],
        plan: &PeerReenrollmentPlan,
    ) -> Result<PeerReenrollmentRuntimeCeremonyWriteOutcome, PeerReenrollmentRuntimeCeremonyError>
    {
        self.verify_owner(owner)?;
        let plan_bytes = plan.encode()?;
        let candidate = SignedPeerReenrollmentRuntimeCeremony::issue(
            owner,
            PeerReenrollmentRuntimeCeremonyContent {
                version: RECORD_VERSION,
                owner_account_id: self.owner_account_id,
                owner_device_id: self.owner_device_id,
                contact_id,
                staged_invitation_id,
                invitation_id,
                quarantine_record_id,
                plan: plan_bytes,
                tombstone: None,
                acceptance_receipt_id: None,
                runtime_transition_id: None,
                phase: PeerReenrollmentRuntimeCeremonyPhase::Planned,
            },
        )?;
        if let Some(existing) = self.load(contact_id)? {
            if ceremony_binding_matches(&existing.record.content, &candidate.content) {
                return Ok(PeerReenrollmentRuntimeCeremonyWriteOutcome::AlreadyPresent(
                    existing,
                ));
            }
            return Err(PeerReenrollmentRuntimeCeremonyError::ContactCeremonyConflict);
        }
        self.write_exact(None, candidate)
    }

    pub fn approve(
        &self,
        owner: &DeviceIdentity,
        contact_id: [u8; 32],
        expected_plan_id: [u8; 32],
        tombstone: &SignedPeerReenrollmentTombstone,
    ) -> Result<PeerReenrollmentRuntimeCeremonyWriteOutcome, PeerReenrollmentRuntimeCeremonyError>
    {
        self.verify_owner(owner)?;
        let existing = self
            .load(contact_id)?
            .ok_or(PeerReenrollmentRuntimeCeremonyError::CeremonyNotPrepared)?;
        if existing.plan_id()? != expected_plan_id || tombstone.plan_id()? != expected_plan_id {
            return Err(PeerReenrollmentRuntimeCeremonyError::PlanChanged);
        }
        let tombstone_bytes = tombstone.encode()?;
        if existing.record.content.tombstone.as_deref() == Some(tombstone_bytes.as_slice()) {
            return Ok(PeerReenrollmentRuntimeCeremonyWriteOutcome::AlreadyPresent(
                existing,
            ));
        }
        if existing.phase() != PeerReenrollmentRuntimeCeremonyPhase::Planned {
            return Err(PeerReenrollmentRuntimeCeremonyError::CeremonyAlreadyAdvanced);
        }
        let mut content = existing.record.content.clone();
        content.tombstone = Some(tombstone_bytes);
        content.phase = PeerReenrollmentRuntimeCeremonyPhase::Approved;
        let candidate = SignedPeerReenrollmentRuntimeCeremony::issue(owner, content)?;
        self.write_exact(Some(&existing), candidate)
    }

    pub fn mark_acceptance_committed(
        &self,
        owner: &DeviceIdentity,
        contact_id: [u8; 32],
        expected_tombstone_id: [u8; 32],
        acceptance_receipt_id: [u8; 32],
    ) -> Result<PeerReenrollmentRuntimeCeremonyWriteOutcome, PeerReenrollmentRuntimeCeremonyError>
    {
        self.verify_owner(owner)?;
        if acceptance_receipt_id == [0_u8; 32] {
            return Err(PeerReenrollmentRuntimeCeremonyError::InvalidRecord);
        }
        let existing = self
            .load(contact_id)?
            .ok_or(PeerReenrollmentRuntimeCeremonyError::CeremonyNotPrepared)?;
        if existing.tombstone_id()? != Some(expected_tombstone_id) {
            return Err(PeerReenrollmentRuntimeCeremonyError::TombstoneChanged);
        }
        if existing.acceptance_receipt_id() == Some(acceptance_receipt_id) {
            return Ok(PeerReenrollmentRuntimeCeremonyWriteOutcome::AlreadyPresent(
                existing,
            ));
        }
        if existing.phase() != PeerReenrollmentRuntimeCeremonyPhase::Approved {
            return Err(PeerReenrollmentRuntimeCeremonyError::CeremonyAlreadyAdvanced);
        }
        let mut content = existing.record.content.clone();
        content.acceptance_receipt_id = Some(acceptance_receipt_id);
        content.phase = PeerReenrollmentRuntimeCeremonyPhase::AcceptanceCommitted;
        let candidate = SignedPeerReenrollmentRuntimeCeremony::issue(owner, content)?;
        self.write_exact(Some(&existing), candidate)
    }

    pub fn complete(
        &self,
        owner: &DeviceIdentity,
        contact_id: [u8; 32],
        expected_acceptance_receipt_id: [u8; 32],
        runtime_transition_id: [u8; 32],
    ) -> Result<PeerReenrollmentRuntimeCeremonyWriteOutcome, PeerReenrollmentRuntimeCeremonyError>
    {
        self.verify_owner(owner)?;
        if runtime_transition_id == [0_u8; 32] {
            return Err(PeerReenrollmentRuntimeCeremonyError::InvalidRecord);
        }
        let existing = self
            .load(contact_id)?
            .ok_or(PeerReenrollmentRuntimeCeremonyError::CeremonyNotPrepared)?;
        if existing.acceptance_receipt_id() != Some(expected_acceptance_receipt_id) {
            return Err(PeerReenrollmentRuntimeCeremonyError::AcceptanceChanged);
        }
        if existing.runtime_transition_id() == Some(runtime_transition_id) {
            return Ok(PeerReenrollmentRuntimeCeremonyWriteOutcome::AlreadyPresent(
                existing,
            ));
        }
        if existing.phase() != PeerReenrollmentRuntimeCeremonyPhase::AcceptanceCommitted {
            return Err(PeerReenrollmentRuntimeCeremonyError::CeremonyAlreadyAdvanced);
        }
        let mut content = existing.record.content.clone();
        content.runtime_transition_id = Some(runtime_transition_id);
        content.phase = PeerReenrollmentRuntimeCeremonyPhase::Complete;
        let candidate = SignedPeerReenrollmentRuntimeCeremony::issue(owner, content)?;
        self.write_exact(Some(&existing), candidate)
    }

    pub fn load(
        &self,
        contact_id: [u8; 32],
    ) -> Result<Option<DurablePeerReenrollmentRuntimeCeremony>, PeerReenrollmentRuntimeCeremonyError>
    {
        load_record(
            &self.database,
            self.owner_account_id,
            self.owner_device_id,
            contact_id,
        )
    }

    pub fn load_all(
        &self,
    ) -> Result<Vec<DurablePeerReenrollmentRuntimeCeremony>, PeerReenrollmentRuntimeCeremonyError>
    {
        load_all(&self.database, self.owner_account_id, self.owner_device_id)
    }

    fn verify_owner(
        &self,
        owner: &DeviceIdentity,
    ) -> Result<(), PeerReenrollmentRuntimeCeremonyError> {
        if owner.device_id() != self.owner_device_id {
            return Err(PeerReenrollmentRuntimeCeremonyError::OwnerDeviceMismatch);
        }
        Ok(())
    }

    fn write_exact(
        &self,
        expected: Option<&DurablePeerReenrollmentRuntimeCeremony>,
        candidate: SignedPeerReenrollmentRuntimeCeremony,
    ) -> Result<PeerReenrollmentRuntimeCeremonyWriteOutcome, PeerReenrollmentRuntimeCeremonyError>
    {
        let key = candidate.content.contact_id;
        let encoded = candidate.encode()?;
        let expected_bytes = expected
            .map(|current| current.record.encode())
            .transpose()?;
        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut table = write.open_table(CEREMONY_TABLE).map_err(database_error)?;
            let current = table
                .get(key.as_slice())
                .map_err(database_error)?
                .map(|bytes| bytes.value().to_vec());
            match (&expected_bytes, current) {
                (None, None) => {
                    if table.len().map_err(database_error)?
                        >= MAX_PEER_REENROLLMENT_RUNTIME_CEREMONIES
                    {
                        return Err(PeerReenrollmentRuntimeCeremonyError::CeremonyCapacity);
                    }
                }
                (Some(expected), Some(current)) if *expected == current => {}
                _ => return Err(PeerReenrollmentRuntimeCeremonyError::CeremonyChanged),
            }
            table
                .insert(key.as_slice(), encoded.as_slice())
                .map_err(database_error)?;
        }
        write.commit().map_err(database_error)?;
        let durable = self
            .load(key)?
            .ok_or(PeerReenrollmentRuntimeCeremonyError::CeremonyNotPrepared)?;
        if durable.record != candidate {
            return Err(PeerReenrollmentRuntimeCeremonyError::CeremonyChanged);
        }
        Ok(PeerReenrollmentRuntimeCeremonyWriteOutcome::Written(
            durable,
        ))
    }
}

impl PeerReenrollmentRuntimeCeremonyReadStore {
    pub fn open_existing(
        directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Result<Self, PeerReenrollmentRuntimeCeremonyError> {
        let directory = directory.as_ref().to_path_buf();
        let path = directory.join(DATABASE_FILE);
        if !path.is_file() {
            return Err(PeerReenrollmentRuntimeCeremonyError::StoreNotInitialized);
        }
        let database = ReadOnlyDatabase::open(path).map_err(database_error)?;
        verify_all(&database, owner_account_id, owner_device_id)?;
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

    pub fn load(
        &self,
        contact_id: [u8; 32],
    ) -> Result<Option<DurablePeerReenrollmentRuntimeCeremony>, PeerReenrollmentRuntimeCeremonyError>
    {
        load_record(
            &self.database,
            self.owner_account_id,
            self.owner_device_id,
            contact_id,
        )
    }

    pub fn load_all(
        &self,
    ) -> Result<Vec<DurablePeerReenrollmentRuntimeCeremony>, PeerReenrollmentRuntimeCeremonyError>
    {
        load_all(&self.database, self.owner_account_id, self.owner_device_id)
    }
}

pub fn peer_reenrollment_tombstone_id(
    tombstone: &SignedPeerReenrollmentTombstone,
) -> Result<[u8; 32], PeerReenrollmentRuntimeCeremonyError> {
    peer_reenrollment_tombstone_id_from_bytes(&tombstone.encode()?)
}

fn peer_reenrollment_tombstone_id_from_bytes(
    encoded: &[u8],
) -> Result<[u8; 32], PeerReenrollmentRuntimeCeremonyError> {
    if encoded.is_empty() || encoded.len() > MAX_PEER_REENROLLMENT_PLAN_BYTES {
        return Err(PeerReenrollmentRuntimeCeremonyError::InvalidRecord);
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(TOMBSTONE_ID_DOMAIN);
    hasher.update(encoded);
    Ok(*hasher.finalize().as_bytes())
}

fn ceremony_binding_matches(
    left: &PeerReenrollmentRuntimeCeremonyContent,
    right: &PeerReenrollmentRuntimeCeremonyContent,
) -> bool {
    left.owner_account_id == right.owner_account_id
        && left.owner_device_id == right.owner_device_id
        && left.contact_id == right.contact_id
        && left.staged_invitation_id == right.staged_invitation_id
        && left.invitation_id == right.invitation_id
        && left.quarantine_record_id == right.quarantine_record_id
        && left.plan == right.plan
}

fn validate_content(
    content: &PeerReenrollmentRuntimeCeremonyContent,
) -> Result<(), PeerReenrollmentRuntimeCeremonyError> {
    if content.version != RECORD_VERSION
        || content.contact_id == [0_u8; 32]
        || content.staged_invitation_id == [0_u8; 32]
        || content.invitation_id == [0_u8; 32]
        || content.quarantine_record_id == [0_u8; 32]
        || content.plan.is_empty()
        || content.plan.len() > MAX_PEER_REENROLLMENT_PLAN_BYTES
    {
        return Err(PeerReenrollmentRuntimeCeremonyError::InvalidRecord);
    }
    let plan = PeerReenrollmentPlan::decode_and_validate(&content.plan, content.owner_device_id)?;
    if plan.local_account_id() != content.owner_account_id
        || plan.local_device_id() != content.owner_device_id
        || plan.invitation_id() != content.invitation_id
        || plan.quarantine_record_id() != content.quarantine_record_id
    {
        return Err(PeerReenrollmentRuntimeCeremonyError::InvalidRecord);
    }
    let tombstone = content
        .tombstone
        .as_deref()
        .map(|encoded| {
            SignedPeerReenrollmentTombstone::decode_and_verify(encoded, content.owner_device_id)
        })
        .transpose()?;
    if tombstone
        .as_ref()
        .is_some_and(|value| value.plan_id().ok() != plan.plan_id().ok())
    {
        return Err(PeerReenrollmentRuntimeCeremonyError::PlanChanged);
    }
    let valid_shape = match content.phase {
        PeerReenrollmentRuntimeCeremonyPhase::Planned => {
            tombstone.is_none()
                && content.acceptance_receipt_id.is_none()
                && content.runtime_transition_id.is_none()
        }
        PeerReenrollmentRuntimeCeremonyPhase::Approved => {
            tombstone.is_some()
                && content.acceptance_receipt_id.is_none()
                && content.runtime_transition_id.is_none()
        }
        PeerReenrollmentRuntimeCeremonyPhase::AcceptanceCommitted => {
            tombstone.is_some()
                && content
                    .acceptance_receipt_id
                    .is_some_and(|value| value != [0_u8; 32])
                && content.runtime_transition_id.is_none()
        }
        PeerReenrollmentRuntimeCeremonyPhase::Complete => {
            tombstone.is_some()
                && content
                    .acceptance_receipt_id
                    .is_some_and(|value| value != [0_u8; 32])
                && content
                    .runtime_transition_id
                    .is_some_and(|value| value != [0_u8; 32])
        }
    };
    if !valid_shape {
        return Err(PeerReenrollmentRuntimeCeremonyError::InvalidRecord);
    }
    Ok(())
}

fn signing_bytes(
    content: &PeerReenrollmentRuntimeCeremonyContent,
) -> Result<Vec<u8>, PeerReenrollmentRuntimeCeremonyError> {
    validate_content(content)?;
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn load_record<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    contact_id: [u8; 32],
) -> Result<Option<DurablePeerReenrollmentRuntimeCeremony>, PeerReenrollmentRuntimeCeremonyError> {
    let read = database.begin_read().map_err(database_error)?;
    let table = read.open_table(CEREMONY_TABLE).map_err(database_error)?;
    let Some(bytes) = table.get(contact_id.as_slice()).map_err(database_error)? else {
        return Ok(None);
    };
    let record = SignedPeerReenrollmentRuntimeCeremony::decode_and_verify(
        bytes.value(),
        owner_account_id,
        owner_device_id,
    )?;
    if record.content.contact_id != contact_id {
        return Err(PeerReenrollmentRuntimeCeremonyError::ContactKeyMismatch);
    }
    Ok(Some(DurablePeerReenrollmentRuntimeCeremony { record }))
}

fn load_all<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<Vec<DurablePeerReenrollmentRuntimeCeremony>, PeerReenrollmentRuntimeCeremonyError> {
    let read = database.begin_read().map_err(database_error)?;
    let table = read.open_table(CEREMONY_TABLE).map_err(database_error)?;
    let mut values = Vec::new();
    for item in table.iter().map_err(database_error)? {
        let (key, bytes) = item.map_err(database_error)?;
        let contact_id = copy_id(key.value())?;
        let record = SignedPeerReenrollmentRuntimeCeremony::decode_and_verify(
            bytes.value(),
            owner_account_id,
            owner_device_id,
        )?;
        if record.content.contact_id != contact_id {
            return Err(PeerReenrollmentRuntimeCeremonyError::ContactKeyMismatch);
        }
        values.push(DurablePeerReenrollmentRuntimeCeremony { record });
    }
    values.sort_by_key(DurablePeerReenrollmentRuntimeCeremony::contact_id);
    Ok(values)
}

fn initialize_database(
    database: &Database,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), PeerReenrollmentRuntimeCeremonyError> {
    let mut write = database.begin_write().map_err(database_error)?;
    write
        .set_durability(Durability::Immediate)
        .map_err(database_error)?;
    {
        let mut meta = write.open_table(META_TABLE).map_err(database_error)?;
        initialize_meta(&mut meta, SCHEMA_KEY, &[SCHEMA_VERSION], || {
            PeerReenrollmentRuntimeCeremonyError::SchemaMismatch
        })?;
        initialize_meta(
            &mut meta,
            OWNER_ACCOUNT_KEY,
            owner_account_id.as_bytes(),
            || PeerReenrollmentRuntimeCeremonyError::OwnerAccountMismatch,
        )?;
        initialize_meta(
            &mut meta,
            OWNER_DEVICE_KEY,
            owner_device_id.as_bytes(),
            || PeerReenrollmentRuntimeCeremonyError::OwnerDeviceMismatch,
        )?;
    }
    write.open_table(CEREMONY_TABLE).map_err(database_error)?;
    write.commit().map_err(database_error)
}

fn initialize_meta(
    table: &mut redb::Table<'_, &str, &[u8]>,
    key: &'static str,
    expected: &[u8],
    mismatch: impl Fn() -> PeerReenrollmentRuntimeCeremonyError,
) -> Result<(), PeerReenrollmentRuntimeCeremonyError> {
    let current = table
        .get(key)
        .map_err(database_error)?
        .map(|value| value.value().to_vec());
    match current {
        Some(value) if value == expected => Ok(()),
        Some(_) => Err(mismatch()),
        None => {
            table.insert(key, expected).map_err(database_error)?;
            Ok(())
        }
    }
}

fn verify_all<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), PeerReenrollmentRuntimeCeremonyError> {
    let read = database.begin_read().map_err(database_error)?;
    let meta = read.open_table(META_TABLE).map_err(database_error)?;
    verify_meta(&meta, SCHEMA_KEY, &[SCHEMA_VERSION], || {
        PeerReenrollmentRuntimeCeremonyError::SchemaMismatch
    })?;
    verify_meta(
        &meta,
        OWNER_ACCOUNT_KEY,
        owner_account_id.as_bytes(),
        || PeerReenrollmentRuntimeCeremonyError::OwnerAccountMismatch,
    )?;
    verify_meta(&meta, OWNER_DEVICE_KEY, owner_device_id.as_bytes(), || {
        PeerReenrollmentRuntimeCeremonyError::OwnerDeviceMismatch
    })?;
    drop(meta);
    let table = read.open_table(CEREMONY_TABLE).map_err(database_error)?;
    if table.len().map_err(database_error)? > MAX_PEER_REENROLLMENT_RUNTIME_CEREMONIES {
        return Err(PeerReenrollmentRuntimeCeremonyError::CeremonyCapacity);
    }
    for item in table.iter().map_err(database_error)? {
        let (key, bytes) = item.map_err(database_error)?;
        let contact_id = copy_id(key.value())?;
        let record = SignedPeerReenrollmentRuntimeCeremony::decode_and_verify(
            bytes.value(),
            owner_account_id,
            owner_device_id,
        )?;
        if record.content.contact_id != contact_id {
            return Err(PeerReenrollmentRuntimeCeremonyError::ContactKeyMismatch);
        }
    }
    Ok(())
}

fn verify_meta(
    table: &impl ReadableTable<&'static str, &'static [u8]>,
    key: &'static str,
    expected: &[u8],
    mismatch: impl Fn() -> PeerReenrollmentRuntimeCeremonyError,
) -> Result<(), PeerReenrollmentRuntimeCeremonyError> {
    let value = table
        .get(key)
        .map_err(database_error)?
        .ok_or_else(&mismatch)?;
    if value.value() != expected {
        return Err(mismatch());
    }
    Ok(())
}

fn copy_id(bytes: &[u8]) -> Result<[u8; 32], PeerReenrollmentRuntimeCeremonyError> {
    bytes
        .try_into()
        .map_err(|_| PeerReenrollmentRuntimeCeremonyError::StoredIdLength(bytes.len()))
}

fn database_error(error: impl std::fmt::Display) -> PeerReenrollmentRuntimeCeremonyError {
    PeerReenrollmentRuntimeCeremonyError::Database(error.to_string())
}

#[derive(Debug, Error)]
pub enum PeerReenrollmentRuntimeCeremonyError {
    #[error("peer re-enrollment runtime ceremony store is not initialized")]
    StoreNotInitialized,
    #[error("peer re-enrollment runtime ceremony store I/O failed")]
    Io(#[from] std::io::Error),
    #[error("peer re-enrollment runtime ceremony database failed: {0}")]
    Database(String),
    #[error("peer re-enrollment runtime ceremony schema does not match")]
    SchemaMismatch,
    #[error("peer re-enrollment runtime ceremony owner Account does not match")]
    OwnerAccountMismatch,
    #[error("peer re-enrollment runtime ceremony owner Device does not match")]
    OwnerDeviceMismatch,
    #[error("peer re-enrollment runtime ceremony record is invalid")]
    InvalidRecord,
    #[error("peer re-enrollment runtime ceremony record is {0} bytes and violates its bound")]
    RecordLength(usize),
    #[error("peer re-enrollment runtime ceremony record is not canonical")]
    NonCanonicalRecord,
    #[error("peer re-enrollment runtime ceremony capacity is exhausted")]
    CeremonyCapacity,
    #[error("a different peer re-enrollment ceremony already exists for this contact")]
    ContactCeremonyConflict,
    #[error("peer re-enrollment runtime ceremony is not prepared")]
    CeremonyNotPrepared,
    #[error("peer re-enrollment runtime ceremony changed concurrently")]
    CeremonyChanged,
    #[error("peer re-enrollment runtime ceremony has already advanced")]
    CeremonyAlreadyAdvanced,
    #[error("peer re-enrollment plan changed")]
    PlanChanged,
    #[error("peer re-enrollment tombstone changed")]
    TombstoneChanged,
    #[error("peer re-enrollment acceptance receipt changed")]
    AcceptanceChanged,
    #[error("peer re-enrollment ceremony contact key does not match its signed record")]
    ContactKeyMismatch,
    #[error("stored peer re-enrollment ceremony ID has invalid length {0}")]
    StoredIdLength(usize),
    #[error("peer re-enrollment runtime ceremony encoding is invalid: {0}")]
    Codec(#[from] postcard::Error),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    PeerReenrollment(#[from] PeerReenrollmentError),
}
