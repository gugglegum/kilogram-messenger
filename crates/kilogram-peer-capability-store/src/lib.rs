#![forbid(unsafe_code)]

//! Durable, Device-bound memory for authenticated peer sync capabilities.
//!
//! This crate deliberately has no network dependency. It persists only
//! bounded identifiers, monotonic floors and locally signed transition
//! records. It stores neither tickets nor invitations, grants no transport
//! authority, and has no API that clears a quarantine from network input.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use kilogram_identity::{AccountId, DeviceId, DeviceIdentity, IdentityError};
use kilogram_sync_capability::{
    PeerSyncCapabilityEvidence, PeerSyncCapabilityFloor, PeerSyncCapabilityQuarantineReason,
    PeerSyncCapabilityUse, SignedCurrentPeerInvitation, SyncTransportCapabilityError,
    evaluate_peer_sync_capability_floor,
};
use redb::{
    Database, Durability, ReadOnlyDatabase, ReadableDatabase, ReadableTable, ReadableTableMetadata,
    TableDefinition,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const DATABASE_FILE: &str = "peer-sync-capability-v1.redb";
const META_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("peer-sync-capability-meta-v1");
const RECORD_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("peer-sync-capability-record-v1");
const HEAD_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("peer-sync-capability-head-v1");

const SCHEMA_KEY: &str = "schema-version";
const OWNER_DEVICE_KEY: &str = "owner-device-id";
const SCHEMA_VERSION: u8 = 1;
const RECORD_VERSION: u8 = 1;
const RECORD_SIGNATURE_DOMAIN: &[u8] = b"kilogram:peer-sync-capability-record:v1\0";
const RECORD_ID_DOMAIN: &[u8] = b"kilogram:peer-sync-capability-record-id:v1\0";

pub const MAX_PEER_CAPABILITY_RECORD_BYTES: usize = 2 * 1024;
pub const MAX_PEER_CAPABILITY_RECORDS: u64 = 65_536;
pub const MAX_PEER_CAPABILITY_RECORDS_PER_PEER: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PeerCapabilityKey {
    pub peer_account_id: AccountId,
    pub peer_device_id: DeviceId,
}

impl PeerCapabilityKey {
    pub const fn new(peer_account_id: AccountId, peer_device_id: DeviceId) -> Self {
        Self {
            peer_account_id,
            peer_device_id,
        }
    }

    fn encoded(self) -> [u8; 64] {
        let mut encoded = [0_u8; 64];
        encoded[..32].copy_from_slice(self.peer_account_id.as_bytes());
        encoded[32..].copy_from_slice(self.peer_device_id.as_bytes());
        encoded
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum PeerCapabilityRecordSource {
    CurrentInvitation {
        invitation_id: [u8; 32],
    },
    AuthenticatedObservation {
        evidence: PeerSyncCapabilityEvidence,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DurablePeerCapabilityState {
    pub record_id: [u8; 32],
    pub sequence: u64,
    pub floor: PeerSyncCapabilityFloor,
    pub source: PeerCapabilityRecordSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeerCapabilityApplyOutcome {
    pub state: Option<DurablePeerCapabilityState>,
    pub permitted_use: PeerSyncCapabilityUse,
    pub changed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PeerCapabilityRecordContent {
    version: u8,
    owner_device_id: DeviceId,
    peer_account_id: AccountId,
    peer_device_id: DeviceId,
    sequence: u64,
    previous_record_id: Option<[u8; 32]>,
    floor: PeerSyncCapabilityFloor,
    source: PeerCapabilityRecordSource,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SignedPeerCapabilityRecord {
    content: PeerCapabilityRecordContent,
    signature: Vec<u8>,
}

impl SignedPeerCapabilityRecord {
    fn issue(
        owner: &DeviceIdentity,
        key: PeerCapabilityKey,
        sequence: u64,
        previous_record_id: Option<[u8; 32]>,
        floor: PeerSyncCapabilityFloor,
        source: PeerCapabilityRecordSource,
    ) -> Result<Self, PeerCapabilityStoreError> {
        let content = PeerCapabilityRecordContent {
            version: RECORD_VERSION,
            owner_device_id: owner.device_id(),
            peer_account_id: key.peer_account_id,
            peer_device_id: key.peer_device_id,
            sequence,
            previous_record_id,
            floor,
            source,
        };
        validate_record_content(&content)?;
        let signature = owner.sign(&record_signing_bytes(&content)?).to_vec();
        let record = Self { content, signature };
        record.verify_for(owner.device_id(), key)?;
        Ok(record)
    }

    fn decode_and_verify(
        encoded: &[u8],
        owner_device_id: DeviceId,
        key: PeerCapabilityKey,
    ) -> Result<Self, PeerCapabilityStoreError> {
        if encoded.is_empty() || encoded.len() > MAX_PEER_CAPABILITY_RECORD_BYTES {
            return Err(PeerCapabilityStoreError::RecordLength(encoded.len()));
        }
        let record: Self = postcard::from_bytes(encoded)?;
        record.verify_for(owner_device_id, key)?;
        Ok(record)
    }

    fn verify_for(
        &self,
        owner_device_id: DeviceId,
        key: PeerCapabilityKey,
    ) -> Result<(), PeerCapabilityStoreError> {
        validate_record_content(&self.content)?;
        if self.content.owner_device_id != owner_device_id {
            return Err(PeerCapabilityStoreError::OwnerDeviceMismatch);
        }
        if self.content.peer_account_id != key.peer_account_id
            || self.content.peer_device_id != key.peer_device_id
        {
            return Err(PeerCapabilityStoreError::PeerBindingMismatch);
        }
        owner_device_id.verify(&record_signing_bytes(&self.content)?, &self.signature)?;
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>, PeerCapabilityStoreError> {
        let encoded = postcard::to_allocvec(self)?;
        if encoded.is_empty() || encoded.len() > MAX_PEER_CAPABILITY_RECORD_BYTES {
            return Err(PeerCapabilityStoreError::RecordLength(encoded.len()));
        }
        Ok(encoded)
    }

    fn record_id(&self) -> Result<[u8; 32], PeerCapabilityStoreError> {
        let encoded = self.encode()?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(RECORD_ID_DOMAIN);
        hasher.update(&encoded);
        Ok(*hasher.finalize().as_bytes())
    }

    fn state(&self) -> Result<DurablePeerCapabilityState, PeerCapabilityStoreError> {
        Ok(DurablePeerCapabilityState {
            record_id: self.record_id()?,
            sequence: self.content.sequence,
            floor: self.content.floor,
            source: self.content.source,
        })
    }
}

pub struct PeerCapabilityStore {
    directory: PathBuf,
    database: Database,
    owner_device_id: DeviceId,
}

/// A strictly read-only view of an already initialized peer capability store.
///
/// Opening this view never creates a directory, database, table, or recovery
/// record. Callers that merely render status must use this type instead of
/// [`PeerCapabilityStore`].
pub struct PeerCapabilityReadStore {
    directory: PathBuf,
    database: ReadOnlyDatabase,
    owner_device_id: DeviceId,
}

impl PeerCapabilityReadStore {
    pub fn open_existing(
        directory: impl AsRef<Path>,
        owner_device_id: DeviceId,
    ) -> Result<Self, PeerCapabilityStoreError> {
        let directory = directory.as_ref().to_path_buf();
        let database_path = directory.join(DATABASE_FILE);
        if !database_path.is_file() {
            return Err(PeerCapabilityStoreError::StoreNotInitialized);
        }
        let database = ReadOnlyDatabase::open(database_path).map_err(database_error)?;
        verify_database_metadata(&database, owner_device_id)?;
        verify_all_records_from_database(&database, owner_device_id)?;
        Ok(Self {
            directory,
            database,
            owner_device_id,
        })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn owner_device_id(&self) -> DeviceId {
        self.owner_device_id
    }

    pub fn load_state(
        &self,
        key: PeerCapabilityKey,
    ) -> Result<Option<DurablePeerCapabilityState>, PeerCapabilityStoreError> {
        match load_verified_head_from_database(&self.database, self.owner_device_id, key)? {
            Some(record) => Ok(Some(record.state()?)),
            None => Ok(None),
        }
    }
}

impl PeerCapabilityStore {
    pub fn is_initialized(directory: impl AsRef<Path>) -> bool {
        directory.as_ref().join(DATABASE_FILE).is_file()
    }

    pub fn open(
        directory: impl AsRef<Path>,
        owner_device_id: DeviceId,
    ) -> Result<Self, PeerCapabilityStoreError> {
        let directory = directory.as_ref().to_path_buf();
        std::fs::create_dir_all(&directory)?;
        let database = Database::create(directory.join(DATABASE_FILE)).map_err(database_error)?;
        initialize_database(&database, owner_device_id)?;
        let store = Self {
            directory,
            database,
            owner_device_id,
        };
        store.verify_all_records()?;
        Ok(store)
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn owner_device_id(&self) -> DeviceId {
        self.owner_device_id
    }

    pub fn load_state(
        &self,
        key: PeerCapabilityKey,
    ) -> Result<Option<DurablePeerCapabilityState>, PeerCapabilityStoreError> {
        match self.load_verified_head(key)? {
            Some(record) => Ok(Some(record.state()?)),
            None => Ok(None),
        }
    }

    pub fn apply_current_invitation(
        &self,
        owner: &DeviceIdentity,
        local_account_id: AccountId,
        invitation: &SignedCurrentPeerInvitation,
        now_unix_seconds: u64,
    ) -> Result<PeerCapabilityApplyOutcome, PeerCapabilityStoreError> {
        self.verify_owner(owner)?;
        invitation.verify_at(now_unix_seconds)?;
        if invitation.allowed_requester_account_id() != local_account_id {
            return Err(PeerCapabilityStoreError::InvitationRequesterMismatch);
        }
        let key = PeerCapabilityKey::new(
            invitation.listener_account_id(),
            invitation.listener_device_id(),
        );
        let current = self.load_state(key)?;
        let current_floor = current
            .map(|state| state.floor)
            .unwrap_or(PeerSyncCapabilityFloor::LegacyUnasserted);
        let invitation_id = invitation.invitation_id()?;
        let proposed = invitation.required_floor()?;
        let next = invitation_floor_transition(current_floor, proposed);
        if next == current_floor {
            return Ok(PeerCapabilityApplyOutcome {
                state: current,
                permitted_use: permitted_use_for_floor(current_floor),
                changed: false,
            });
        }
        let state = self.append_record(
            owner,
            key,
            current,
            next,
            PeerCapabilityRecordSource::CurrentInvitation { invitation_id },
        )?;
        Ok(PeerCapabilityApplyOutcome {
            state: Some(state),
            permitted_use: permitted_use_for_floor(next),
            changed: true,
        })
    }

    pub fn apply_authenticated_observation(
        &self,
        owner: &DeviceIdentity,
        key: PeerCapabilityKey,
        evidence: PeerSyncCapabilityEvidence,
    ) -> Result<PeerCapabilityApplyOutcome, PeerCapabilityStoreError> {
        self.verify_owner(owner)?;
        let current = self.load_state(key)?;
        let current_floor = current
            .map(|state| state.floor)
            .unwrap_or(PeerSyncCapabilityFloor::LegacyUnasserted);
        let transition = evaluate_peer_sync_capability_floor(current_floor, evidence);
        if transition.next == current_floor {
            return Ok(PeerCapabilityApplyOutcome {
                state: current,
                permitted_use: transition.permitted_use,
                changed: false,
            });
        }
        let state = self.append_record(
            owner,
            key,
            current,
            transition.next,
            PeerCapabilityRecordSource::AuthenticatedObservation { evidence },
        )?;
        Ok(PeerCapabilityApplyOutcome {
            state: Some(state),
            permitted_use: transition.permitted_use,
            changed: true,
        })
    }

    fn verify_owner(&self, owner: &DeviceIdentity) -> Result<(), PeerCapabilityStoreError> {
        if owner.device_id() != self.owner_device_id {
            return Err(PeerCapabilityStoreError::OwnerDeviceMismatch);
        }
        Ok(())
    }

    fn append_record(
        &self,
        owner: &DeviceIdentity,
        key: PeerCapabilityKey,
        current: Option<DurablePeerCapabilityState>,
        floor: PeerSyncCapabilityFloor,
        source: PeerCapabilityRecordSource,
    ) -> Result<DurablePeerCapabilityState, PeerCapabilityStoreError> {
        let sequence = current
            .map(|state| state.sequence)
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(PeerCapabilityStoreError::SequenceOverflow)?;
        if sequence as usize > MAX_PEER_CAPABILITY_RECORDS_PER_PEER {
            return Err(PeerCapabilityStoreError::PeerRecordLimit);
        }
        let previous_record_id = current.map(|state| state.record_id);
        let record = SignedPeerCapabilityRecord::issue(
            owner,
            key,
            sequence,
            previous_record_id,
            floor,
            source,
        )?;
        let record_id = record.record_id()?;
        let encoded = record.encode()?;
        let key_bytes = key.encoded();

        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut records = write.open_table(RECORD_TABLE).map_err(database_error)?;
            let mut heads = write.open_table(HEAD_TABLE).map_err(database_error)?;
            if records.len().map_err(database_error)? >= MAX_PEER_CAPABILITY_RECORDS {
                return Err(PeerCapabilityStoreError::GlobalRecordLimit);
            }
            let actual_head = heads
                .get(key_bytes.as_slice())
                .map_err(database_error)?
                .map(|value| copy_id(value.value()))
                .transpose()?;
            if actual_head != previous_record_id {
                return Err(PeerCapabilityStoreError::ConcurrentHeadChange);
            }
            if records
                .get(record_id.as_slice())
                .map_err(database_error)?
                .is_some()
            {
                return Err(PeerCapabilityStoreError::DuplicateRecordId);
            }
            records
                .insert(record_id.as_slice(), encoded.as_slice())
                .map_err(database_error)?;
            heads
                .insert(key_bytes.as_slice(), record_id.as_slice())
                .map_err(database_error)?;
        }
        write.commit().map_err(database_error)?;
        record.state()
    }

    fn load_verified_head(
        &self,
        key: PeerCapabilityKey,
    ) -> Result<Option<SignedPeerCapabilityRecord>, PeerCapabilityStoreError> {
        load_verified_head_from_database(&self.database, self.owner_device_id, key)
    }

    fn verify_all_records(&self) -> Result<(), PeerCapabilityStoreError> {
        verify_all_records_from_database(&self.database, self.owner_device_id)
    }
}

fn load_verified_head_from_database<D: ReadableDatabase>(
    database: &D,
    owner_device_id: DeviceId,
    key: PeerCapabilityKey,
) -> Result<Option<SignedPeerCapabilityRecord>, PeerCapabilityStoreError> {
    let read = database.begin_read().map_err(database_error)?;
    let heads = read.open_table(HEAD_TABLE).map_err(database_error)?;
    let key_bytes = key.encoded();
    let Some(head) = heads
        .get(key_bytes.as_slice())
        .map_err(database_error)?
        .map(|value| copy_id(value.value()))
        .transpose()?
    else {
        return Ok(None);
    };
    let records = read.open_table(RECORD_TABLE).map_err(database_error)?;
    let mut expected_id = head;
    let mut expected_sequence: Option<u64> = None;
    let mut chain_ids = HashSet::new();
    let mut head_record = None;

    loop {
        if chain_ids.len() >= MAX_PEER_CAPABILITY_RECORDS_PER_PEER {
            return Err(PeerCapabilityStoreError::PeerRecordLimit);
        }
        if !chain_ids.insert(expected_id) {
            return Err(PeerCapabilityStoreError::RecordCycle);
        }
        let encoded = records
            .get(expected_id.as_slice())
            .map_err(database_error)?
            .ok_or(PeerCapabilityStoreError::MissingRecord(expected_id))?;
        let record =
            SignedPeerCapabilityRecord::decode_and_verify(encoded.value(), owner_device_id, key)?;
        if record.record_id()? != expected_id {
            return Err(PeerCapabilityStoreError::RecordIdMismatch);
        }
        if head_record.is_none() {
            head_record = Some(record.clone());
        }
        match expected_sequence {
            Some(expected) if record.content.sequence != expected => {
                return Err(PeerCapabilityStoreError::SequenceGap);
            }
            _ => {}
        }
        match record.content.previous_record_id {
            Some(previous) => {
                if record.content.sequence <= 1 {
                    return Err(PeerCapabilityStoreError::SequenceGap);
                }
                expected_sequence = Some(record.content.sequence - 1);
                expected_id = previous;
            }
            None => {
                if record.content.sequence != 1 {
                    return Err(PeerCapabilityStoreError::SequenceGap);
                }
                break;
            }
        }
    }

    for item in records.iter().map_err(database_error)? {
        let (stored_id, stored_record) = item.map_err(database_error)?;
        let stored_id = copy_id(stored_id.value())?;
        let raw: SignedPeerCapabilityRecord = postcard::from_bytes(stored_record.value())?;
        let record_key =
            PeerCapabilityKey::new(raw.content.peer_account_id, raw.content.peer_device_id);
        raw.verify_for(owner_device_id, record_key)?;
        if raw.record_id()? != stored_id {
            return Err(PeerCapabilityStoreError::RecordIdMismatch);
        }
        if record_key == key && !chain_ids.contains(&stored_id) {
            return Err(PeerCapabilityStoreError::OrphanPeerRecord);
        }
    }
    Ok(head_record)
}

fn verify_all_records_from_database<D: ReadableDatabase>(
    database: &D,
    owner_device_id: DeviceId,
) -> Result<(), PeerCapabilityStoreError> {
    let read = database.begin_read().map_err(database_error)?;
    let heads = read.open_table(HEAD_TABLE).map_err(database_error)?;
    let mut keys = HashSet::new();
    for item in heads.iter().map_err(database_error)? {
        let (key, head) = item.map_err(database_error)?;
        let key = decode_peer_key(key.value())?;
        let _ = copy_id(head.value())?;
        keys.insert(key);
    }
    drop(heads);
    let records = read.open_table(RECORD_TABLE).map_err(database_error)?;
    for item in records.iter().map_err(database_error)? {
        let (stored_id, stored_record) = item.map_err(database_error)?;
        let stored_id = copy_id(stored_id.value())?;
        let record: SignedPeerCapabilityRecord = postcard::from_bytes(stored_record.value())?;
        let key = PeerCapabilityKey::new(
            record.content.peer_account_id,
            record.content.peer_device_id,
        );
        record.verify_for(owner_device_id, key)?;
        if record.record_id()? != stored_id {
            return Err(PeerCapabilityStoreError::RecordIdMismatch);
        }
        keys.insert(key);
    }
    drop(records);
    drop(read);
    for key in keys {
        if load_verified_head_from_database(database, owner_device_id, key)?.is_none() {
            return Err(PeerCapabilityStoreError::OrphanPeerRecord);
        }
    }
    Ok(())
}

fn verify_database_metadata<D: ReadableDatabase>(
    database: &D,
    owner_device_id: DeviceId,
) -> Result<(), PeerCapabilityStoreError> {
    let read = database.begin_read().map_err(database_error)?;
    let meta = read.open_table(META_TABLE).map_err(database_error)?;
    let schema = meta
        .get(SCHEMA_KEY)
        .map_err(database_error)?
        .map(|value| value.value().to_vec())
        .ok_or(PeerCapabilityStoreError::SchemaMismatch)?;
    if schema != [SCHEMA_VERSION] {
        return Err(PeerCapabilityStoreError::SchemaMismatch);
    }
    let owner = meta
        .get(OWNER_DEVICE_KEY)
        .map_err(database_error)?
        .map(|value| value.value().to_vec())
        .ok_or(PeerCapabilityStoreError::OwnerDeviceMismatch)?;
    if owner.as_slice() != owner_device_id.as_bytes() {
        return Err(PeerCapabilityStoreError::OwnerDeviceMismatch);
    }
    drop(meta);
    let records = read.open_table(RECORD_TABLE).map_err(database_error)?;
    if records.len().map_err(database_error)? > MAX_PEER_CAPABILITY_RECORDS {
        return Err(PeerCapabilityStoreError::GlobalRecordLimit);
    }
    drop(records);
    let heads = read.open_table(HEAD_TABLE).map_err(database_error)?;
    drop(heads);
    Ok(())
}

fn invitation_floor_transition(
    current: PeerSyncCapabilityFloor,
    proposed: PeerSyncCapabilityFloor,
) -> PeerSyncCapabilityFloor {
    let PeerSyncCapabilityFloor::CurrentInvitationRequired {
        invitation_id,
        required_capability_id,
        minimum_authority_revision,
        minimum_ticket_publication_epoch,
    } = proposed
    else {
        return PeerSyncCapabilityFloor::Quarantined {
            reason: PeerSyncCapabilityQuarantineReason::RequiredInvitationCapabilityMismatch,
        };
    };
    match current {
        PeerSyncCapabilityFloor::LegacyUnasserted => proposed,
        PeerSyncCapabilityFloor::CurrentInvitationRequired {
            invitation_id: current_id,
            required_capability_id: current_capability,
            minimum_authority_revision: current_revision,
            minimum_ticket_publication_epoch: current_epoch,
        } => {
            if invitation_id == current_id && required_capability_id == current_capability {
                current
            } else if minimum_authority_revision < current_revision {
                quarantined(
                    PeerSyncCapabilityQuarantineReason::AuthenticatedAuthorityRevisionRegression,
                )
            } else if minimum_ticket_publication_epoch < current_epoch {
                quarantined(PeerSyncCapabilityQuarantineReason::AuthenticatedTicketEpochRegression)
            } else if minimum_authority_revision == current_revision
                && minimum_ticket_publication_epoch == current_epoch
            {
                quarantined(
                    PeerSyncCapabilityQuarantineReason::RequiredInvitationCapabilityMismatch,
                )
            } else {
                proposed
            }
        }
        PeerSyncCapabilityFloor::CanonicalCandidatePinned {
            minimum_authority_revision: current_revision,
            minimum_ticket_publication_epoch: current_epoch,
        } => {
            if minimum_authority_revision < current_revision {
                quarantined(
                    PeerSyncCapabilityQuarantineReason::AuthenticatedAuthorityRevisionRegression,
                )
            } else if minimum_ticket_publication_epoch < current_epoch {
                quarantined(PeerSyncCapabilityQuarantineReason::AuthenticatedTicketEpochRegression)
            } else {
                PeerSyncCapabilityFloor::CurrentInvitationRequired {
                    invitation_id,
                    required_capability_id,
                    minimum_authority_revision,
                    minimum_ticket_publication_epoch,
                }
            }
        }
        PeerSyncCapabilityFloor::Quarantined { .. } => current,
    }
}

fn quarantined(reason: PeerSyncCapabilityQuarantineReason) -> PeerSyncCapabilityFloor {
    PeerSyncCapabilityFloor::Quarantined { reason }
}

fn permitted_use_for_floor(floor: PeerSyncCapabilityFloor) -> PeerSyncCapabilityUse {
    match floor {
        PeerSyncCapabilityFloor::LegacyUnasserted => {
            PeerSyncCapabilityUse::LegacyCompatibilityUnasserted
        }
        PeerSyncCapabilityFloor::CurrentInvitationRequired { .. } => {
            PeerSyncCapabilityUse::CanonicalCapabilityRequired
        }
        PeerSyncCapabilityFloor::CanonicalCandidatePinned { .. } => {
            PeerSyncCapabilityUse::AuthenticatedCanonicalCandidate
        }
        PeerSyncCapabilityFloor::Quarantined { .. } => PeerSyncCapabilityUse::Quarantined,
    }
}

fn validate_record_content(
    content: &PeerCapabilityRecordContent,
) -> Result<(), PeerCapabilityStoreError> {
    if content.version != RECORD_VERSION {
        return Err(PeerCapabilityStoreError::UnsupportedRecordVersion(
            content.version,
        ));
    }
    if content.sequence == 0
        || (content.sequence == 1) != content.previous_record_id.is_none()
        || matches!(content.floor, PeerSyncCapabilityFloor::LegacyUnasserted)
    {
        return Err(PeerCapabilityStoreError::InvalidRecordState);
    }
    Ok(())
}

fn record_signing_bytes(
    content: &PeerCapabilityRecordContent,
) -> Result<Vec<u8>, PeerCapabilityStoreError> {
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(RECORD_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(RECORD_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn initialize_database(
    database: &Database,
    owner_device_id: DeviceId,
) -> Result<(), PeerCapabilityStoreError> {
    let mut write = database.begin_write().map_err(database_error)?;
    write
        .set_durability(Durability::Immediate)
        .map_err(database_error)?;
    {
        let mut meta = write.open_table(META_TABLE).map_err(database_error)?;
        let schema = meta
            .get(SCHEMA_KEY)
            .map_err(database_error)?
            .map(|value| value.value().to_vec());
        match schema {
            Some(value) if value == [SCHEMA_VERSION] => {}
            Some(_) => return Err(PeerCapabilityStoreError::SchemaMismatch),
            None => {
                meta.insert(SCHEMA_KEY, &[SCHEMA_VERSION][..])
                    .map_err(database_error)?;
            }
        }
        let owner = meta
            .get(OWNER_DEVICE_KEY)
            .map_err(database_error)?
            .map(|value| value.value().to_vec());
        match owner {
            Some(value) if value.as_slice() == owner_device_id.as_bytes() => {}
            Some(_) => return Err(PeerCapabilityStoreError::OwnerDeviceMismatch),
            None => {
                meta.insert(OWNER_DEVICE_KEY, owner_device_id.as_bytes().as_slice())
                    .map_err(database_error)?;
            }
        }
        drop(meta);
        let records = write.open_table(RECORD_TABLE).map_err(database_error)?;
        if records.len().map_err(database_error)? > MAX_PEER_CAPABILITY_RECORDS {
            return Err(PeerCapabilityStoreError::GlobalRecordLimit);
        }
        drop(records);
        let heads = write.open_table(HEAD_TABLE).map_err(database_error)?;
        drop(heads);
    }
    write.commit().map_err(database_error)?;
    Ok(())
}

fn copy_id(bytes: &[u8]) -> Result<[u8; 32], PeerCapabilityStoreError> {
    bytes
        .try_into()
        .map_err(|_| PeerCapabilityStoreError::InvalidStoredIdLength(bytes.len()))
}

fn decode_peer_key(bytes: &[u8]) -> Result<PeerCapabilityKey, PeerCapabilityStoreError> {
    if bytes.len() != 64 {
        return Err(PeerCapabilityStoreError::InvalidPeerKeyLength(bytes.len()));
    }
    let account: [u8; 32] = bytes[..32]
        .try_into()
        .map_err(|_| PeerCapabilityStoreError::InvalidPeerKeyLength(bytes.len()))?;
    let device: [u8; 32] = bytes[32..]
        .try_into()
        .map_err(|_| PeerCapabilityStoreError::InvalidPeerKeyLength(bytes.len()))?;
    Ok(PeerCapabilityKey::new(
        AccountId::from_bytes(account),
        DeviceId::from_bytes(device),
    ))
}

fn database_error(error: impl std::fmt::Display) -> PeerCapabilityStoreError {
    PeerCapabilityStoreError::Database(error.to_string())
}

#[derive(Debug, Error)]
pub enum PeerCapabilityStoreError {
    #[error("peer capability store is not initialized")]
    StoreNotInitialized,
    #[error("peer capability store I/O failed")]
    Io(#[from] std::io::Error),
    #[error("peer capability database failed: {0}")]
    Database(String),
    #[error("peer capability store schema does not match")]
    SchemaMismatch,
    #[error("peer capability store owner Device does not match")]
    OwnerDeviceMismatch,
    #[error("current invitation does not authorize the local Account")]
    InvitationRequesterMismatch,
    #[error("peer capability record has unsupported version {0}")]
    UnsupportedRecordVersion(u8),
    #[error("peer capability record state is invalid")]
    InvalidRecordState,
    #[error("peer capability record is {0} bytes and violates its bound")]
    RecordLength(usize),
    #[error("peer capability record peer binding does not match")]
    PeerBindingMismatch,
    #[error("peer capability record ID does not match stored key")]
    RecordIdMismatch,
    #[error("peer capability record chain contains a cycle")]
    RecordCycle,
    #[error("peer capability record chain contains a sequence gap")]
    SequenceGap,
    #[error("peer capability record sequence overflowed")]
    SequenceOverflow,
    #[error("peer capability store changed concurrently")]
    ConcurrentHeadChange,
    #[error("peer capability record ID already exists")]
    DuplicateRecordId,
    #[error("peer capability record limit reached")]
    GlobalRecordLimit,
    #[error("peer capability per-peer record limit reached")]
    PeerRecordLimit,
    #[error("peer capability store contains an orphan record for a peer")]
    OrphanPeerRecord,
    #[error("peer capability record {0:?} is missing")]
    MissingRecord([u8; 32]),
    #[error("stored peer capability ID has invalid length {0}")]
    InvalidStoredIdLength(usize),
    #[error("stored peer capability key has invalid length {0}")]
    InvalidPeerKeyLength(usize),
    #[error(transparent)]
    Codec(#[from] postcard::Error),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Capability(#[from] SyncTransportCapabilityError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use kilogram_sync_capability::CurrentPeerInvitationBinding;

    fn account(byte: u8) -> AccountId {
        AccountId::from_bytes([byte; 32])
    }

    fn invitation(
        listener: &DeviceIdentity,
        requester_account: AccountId,
        revision: u64,
        epoch: u64,
        capability_id: [u8; 32],
    ) -> Result<SignedCurrentPeerInvitation, PeerCapabilityStoreError> {
        Ok(SignedCurrentPeerInvitation::issue(
            listener,
            &CurrentPeerInvitationBinding {
                listener_account_id: account(1),
                listener_device_id: listener.device_id(),
                allowed_requester_account_id: requester_account,
                authority_revision: revision,
                endpoint_id: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                ticket_publication_channel_epoch: epoch,
                issued_at_unix_seconds: 1_000,
                expires_at_unix_seconds: 2_000,
                connection_ticket: "Exact_ticket_01",
                required_capability_id: capability_id,
            },
        )?)
    }

    #[test]
    fn invitation_and_pin_survive_restart() -> Result<(), PeerCapabilityStoreError> {
        let directory = tempfile::tempdir()?;
        let owner = DeviceIdentity::generate()?;
        let listener = DeviceIdentity::generate()?;
        let local_account = account(2);
        let capability_id = [7_u8; 32];
        let invitation = invitation(&listener, local_account, 4, 9, capability_id)?;
        let key = PeerCapabilityKey::new(account(1), listener.device_id());

        let store = PeerCapabilityStore::open(directory.path(), owner.device_id())?;
        let required = store.apply_current_invitation(&owner, local_account, &invitation, 1_500)?;
        assert!(required.changed);
        assert!(matches!(
            required.state.map(|state| state.floor),
            Some(PeerSyncCapabilityFloor::CurrentInvitationRequired { .. })
        ));
        drop(store);

        let store = PeerCapabilityStore::open(directory.path(), owner.device_id())?;
        let pinned = store.apply_authenticated_observation(
            &owner,
            key,
            PeerSyncCapabilityEvidence::AuthenticatedCanonicalCandidate {
                capability_id,
                authority_revision: 4,
                ticket_publication_epoch: 9,
            },
        )?;
        assert!(pinned.changed);
        assert_eq!(pinned.state.map(|state| state.sequence), Some(2));
        drop(store);

        let store = PeerCapabilityStore::open(directory.path(), owner.device_id())?;
        assert!(matches!(
            store.load_state(key)?.map(|state| state.floor),
            Some(PeerSyncCapabilityFloor::CanonicalCandidatePinned {
                minimum_authority_revision: 4,
                minimum_ticket_publication_epoch: 9,
            })
        ));
        Ok(())
    }

    #[test]
    fn read_only_open_refuses_absent_store_without_creating_state()
    -> Result<(), PeerCapabilityStoreError> {
        let directory = tempfile::tempdir()?
            .path()
            .join("absent-peer-capability-store");
        let owner = DeviceIdentity::generate()?;

        assert!(!directory.exists());
        assert!(matches!(
            PeerCapabilityReadStore::open_existing(&directory, owner.device_id()),
            Err(PeerCapabilityStoreError::StoreNotInitialized)
        ));
        assert!(!directory.exists());
        Ok(())
    }

    #[test]
    fn read_only_open_observes_durable_quarantine_without_mutation()
    -> Result<(), PeerCapabilityStoreError> {
        let directory = tempfile::tempdir()?;
        let owner = DeviceIdentity::generate()?;
        let wrong_owner = DeviceIdentity::generate()?;
        let peer = DeviceIdentity::generate()?;
        let key = PeerCapabilityKey::new(account(1), peer.device_id());
        let store = PeerCapabilityStore::open(directory.path(), owner.device_id())?;
        store.apply_authenticated_observation(
            &owner,
            key,
            PeerSyncCapabilityEvidence::AuthenticatedCanonicalCandidate {
                capability_id: [7; 32],
                authority_revision: 4,
                ticket_publication_epoch: 9,
            },
        )?;
        store.apply_authenticated_observation(
            &owner,
            key,
            PeerSyncCapabilityEvidence::LegacyTicketWithoutExtension,
        )?;
        drop(store);

        let database_path = directory.path().join(DATABASE_FILE);
        let before = std::fs::read(&database_path)?;
        {
            let read_store =
                PeerCapabilityReadStore::open_existing(directory.path(), owner.device_id())?;
            assert_eq!(read_store.directory(), directory.path());
            assert_eq!(read_store.owner_device_id(), owner.device_id());
            let state = read_store
                .load_state(key)?
                .ok_or(PeerCapabilityStoreError::InvalidRecordState)?;
            assert_eq!(state.sequence, 2);
            assert!(matches!(
                state.floor,
                PeerSyncCapabilityFloor::Quarantined {
                    reason: PeerSyncCapabilityQuarantineReason::PinnedCanonicalCapabilityMissing
                }
            ));
        }
        assert_eq!(std::fs::read(&database_path)?, before);
        assert!(matches!(
            PeerCapabilityReadStore::open_existing(directory.path(), wrong_owner.device_id()),
            Err(PeerCapabilityStoreError::OwnerDeviceMismatch)
        ));
        Ok(())
    }

    #[test]
    fn stripping_quarantine_is_durable_and_network_irreversible()
    -> Result<(), PeerCapabilityStoreError> {
        let directory = tempfile::tempdir()?;
        let owner = DeviceIdentity::generate()?;
        let listener = DeviceIdentity::generate()?;
        let key = PeerCapabilityKey::new(account(1), listener.device_id());
        let store = PeerCapabilityStore::open(directory.path(), owner.device_id())?;
        store.apply_authenticated_observation(
            &owner,
            key,
            PeerSyncCapabilityEvidence::AuthenticatedCanonicalCandidate {
                capability_id: [7; 32],
                authority_revision: 4,
                ticket_publication_epoch: 9,
            },
        )?;
        let quarantined = store.apply_authenticated_observation(
            &owner,
            key,
            PeerSyncCapabilityEvidence::LegacyTicketWithoutExtension,
        )?;
        assert_eq!(
            quarantined.permitted_use,
            PeerSyncCapabilityUse::Quarantined
        );
        drop(store);

        let store = PeerCapabilityStore::open(directory.path(), owner.device_id())?;
        let retry = store.apply_authenticated_observation(
            &owner,
            key,
            PeerSyncCapabilityEvidence::AuthenticatedCanonicalCandidate {
                capability_id: [8; 32],
                authority_revision: 5,
                ticket_publication_epoch: 10,
            },
        )?;
        assert!(!retry.changed);
        assert_eq!(retry.permitted_use, PeerSyncCapabilityUse::Quarantined);
        assert!(matches!(
            retry.state.map(|state| state.floor),
            Some(PeerSyncCapabilityFloor::Quarantined { .. })
        ));
        Ok(())
    }

    #[test]
    fn wrong_owner_and_requester_fail_closed() -> Result<(), PeerCapabilityStoreError> {
        let directory = tempfile::tempdir()?;
        let owner = DeviceIdentity::generate()?;
        let wrong_owner = DeviceIdentity::generate()?;
        let listener = DeviceIdentity::generate()?;
        let invitation = invitation(&listener, account(2), 4, 9, [7; 32])?;
        let store = PeerCapabilityStore::open(directory.path(), owner.device_id())?;
        assert!(matches!(
            store.apply_current_invitation(&wrong_owner, account(2), &invitation, 1_500),
            Err(PeerCapabilityStoreError::OwnerDeviceMismatch)
        ));
        assert!(matches!(
            store.apply_current_invitation(&owner, account(3), &invitation, 1_500),
            Err(PeerCapabilityStoreError::InvitationRequesterMismatch)
        ));
        drop(store);
        assert!(matches!(
            PeerCapabilityStore::open(directory.path(), wrong_owner.device_id()),
            Err(PeerCapabilityStoreError::OwnerDeviceMismatch)
        ));
        Ok(())
    }

    #[test]
    fn exact_retry_is_idempotent() -> Result<(), PeerCapabilityStoreError> {
        let directory = tempfile::tempdir()?;
        let owner = DeviceIdentity::generate()?;
        let listener = DeviceIdentity::generate()?;
        let invitation = invitation(&listener, account(2), 4, 9, [7; 32])?;
        let store = PeerCapabilityStore::open(directory.path(), owner.device_id())?;
        let first = store.apply_current_invitation(&owner, account(2), &invitation, 1_500)?;
        let second = store.apply_current_invitation(&owner, account(2), &invitation, 1_500)?;
        assert!(first.changed);
        assert!(!second.changed);
        assert_eq!(first.state, second.state);
        Ok(())
    }

    #[test]
    fn retained_records_detect_head_tail_rollback() -> Result<(), PeerCapabilityStoreError> {
        let directory = tempfile::tempdir()?;
        let owner = DeviceIdentity::generate()?;
        let listener = DeviceIdentity::generate()?;
        let key = PeerCapabilityKey::new(account(1), listener.device_id());
        let invitation = invitation(&listener, account(2), 4, 9, [7; 32])?;
        let store = PeerCapabilityStore::open(directory.path(), owner.device_id())?;
        let first = store
            .apply_current_invitation(&owner, account(2), &invitation, 1_500)?
            .state
            .ok_or(PeerCapabilityStoreError::InvalidRecordState)?;
        store.apply_authenticated_observation(
            &owner,
            key,
            PeerSyncCapabilityEvidence::AuthenticatedCanonicalCandidate {
                capability_id: [7; 32],
                authority_revision: 4,
                ticket_publication_epoch: 9,
            },
        )?;
        drop(store);

        let database =
            Database::create(directory.path().join(DATABASE_FILE)).map_err(database_error)?;
        let mut write = database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut heads = write.open_table(HEAD_TABLE).map_err(database_error)?;
            heads
                .insert(key.encoded().as_slice(), first.record_id.as_slice())
                .map_err(database_error)?;
        }
        write.commit().map_err(database_error)?;
        drop(database);

        assert!(matches!(
            PeerCapabilityStore::open(directory.path(), owner.device_id()),
            Err(PeerCapabilityStoreError::OrphanPeerRecord)
        ));
        Ok(())
    }

    #[test]
    fn signed_record_corruption_fails_closed_on_reopen() -> Result<(), PeerCapabilityStoreError> {
        let directory = tempfile::tempdir()?;
        let owner = DeviceIdentity::generate()?;
        let listener = DeviceIdentity::generate()?;
        let key = PeerCapabilityKey::new(account(1), listener.device_id());
        let store = PeerCapabilityStore::open(directory.path(), owner.device_id())?;
        let state = store
            .apply_authenticated_observation(
                &owner,
                key,
                PeerSyncCapabilityEvidence::AuthenticatedCanonicalCandidate {
                    capability_id: [7; 32],
                    authority_revision: 4,
                    ticket_publication_epoch: 9,
                },
            )?
            .state
            .ok_or(PeerCapabilityStoreError::InvalidRecordState)?;
        drop(store);

        let database =
            Database::create(directory.path().join(DATABASE_FILE)).map_err(database_error)?;
        let mut write = database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut records = write.open_table(RECORD_TABLE).map_err(database_error)?;
            let mut encoded = records
                .get(state.record_id.as_slice())
                .map_err(database_error)?
                .ok_or(PeerCapabilityStoreError::MissingRecord(state.record_id))?
                .value()
                .to_vec();
            let last = encoded
                .last_mut()
                .ok_or(PeerCapabilityStoreError::InvalidRecordState)?;
            *last ^= 0x80;
            records
                .insert(state.record_id.as_slice(), encoded.as_slice())
                .map_err(database_error)?;
        }
        write.commit().map_err(database_error)?;
        drop(database);

        assert!(PeerCapabilityStore::open(directory.path(), owner.device_id()).is_err());
        Ok(())
    }
}
