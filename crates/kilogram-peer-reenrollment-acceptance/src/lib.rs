//! Atomic durable acceptance of one exact peer re-enrollment successor.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use kilogram_identity::{
    AccountDeviceListSnapshot, AccountId, DeviceId, DeviceIdentity, IdentityError,
};
use kilogram_peer_capability_store::DurablePeerCapabilityQuarantineHead;
use kilogram_sync_capability::{
    PeerSyncCapabilityFloor, PeerSyncCapabilityQuarantineReason, SignedCurrentPeerInvitation,
    SyncTransportCapabilityError,
};
use redb::{
    Database, Durability, ReadOnlyDatabase, ReadableDatabase, ReadableTable, ReadableTableMetadata,
    TableDefinition,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use kilogram_peer_reenrollment::{
    PeerReenrollmentError, SignedPeerReenrollmentTombstone, peer_reenrollment_authority_digest,
};

const DATABASE_FILE: &str = "peer-reenrollment-acceptance-v1.redb";
const META_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("peer-reenrollment-acceptance-meta-v1");
const RECEIPT_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("peer-reenrollment-acceptance-receipt-v1");
const HEAD_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("peer-reenrollment-acceptance-head-v1");

const SCHEMA_KEY: &str = "schema-version";
const OWNER_ACCOUNT_KEY: &str = "owner-account-id";
const OWNER_DEVICE_KEY: &str = "owner-device-id";
const SCHEMA_VERSION: u8 = 1;
const RECEIPT_VERSION: u8 = 1;
const HEAD_VERSION: u8 = 1;
const RECEIPT_SIGNATURE_DOMAIN: &[u8] = b"kilogram:peer-reenrollment-acceptance-receipt:v1\0";
const RECEIPT_ID_DOMAIN: &[u8] = b"kilogram:peer-reenrollment-acceptance-id:v1\0";
const TOMBSTONE_DIGEST_DOMAIN: &[u8] = b"kilogram:peer-reenrollment-accepted-tombstone:v1\0";

pub const MAX_PEER_REENROLLMENT_ACCEPTANCE_RECEIPT_BYTES: usize = 4 * 1024;
pub const MAX_PEER_REENROLLMENT_ACCEPTANCE_HEAD_BYTES: usize = 1024;
pub const MAX_DURABLE_PEER_REENROLLMENT_ACCEPTANCES: u64 = 4_096;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PeerReenrollmentAcceptanceReceiptContent {
    version: u8,
    local_account_id: AccountId,
    local_device_id: DeviceId,
    local_authority_revision: u64,
    local_authority_digest: [u8; 32],
    peer_account_id: AccountId,
    quarantined_peer_device_id: DeviceId,
    quarantine_record_id: [u8; 32],
    quarantine_sequence: u64,
    quarantine_reason: PeerSyncCapabilityQuarantineReason,
    predecessor_record_id: [u8; 32],
    predecessor_floor: PeerSyncCapabilityFloor,
    tombstone_plan_id: [u8; 32],
    tombstone_artifact_digest: [u8; 32],
    peer_authority_revision: u64,
    peer_authority_digest: [u8; 32],
    replacement_peer_device_id: DeviceId,
    successor_floor: PeerSyncCapabilityFloor,
    invitation_id: [u8; 32],
    required_capability_id: [u8; 32],
    ticket_publication_epoch: u64,
    tombstone_prepared_at_unix_seconds: u64,
    tombstone_expires_at_unix_seconds: u64,
    accepted_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SignedPeerReenrollmentAcceptanceReceiptWire {
    content: PeerReenrollmentAcceptanceReceiptContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedPeerReenrollmentAcceptanceReceipt {
    content: PeerReenrollmentAcceptanceReceiptContent,
    signature: Vec<u8>,
}

impl SignedPeerReenrollmentAcceptanceReceipt {
    fn issue(
        owner: &DeviceIdentity,
        tombstone: &SignedPeerReenrollmentTombstone,
        accepted_at_unix_seconds: u64,
    ) -> Result<Self, PeerReenrollmentAcceptanceStoreError> {
        let tombstone_bytes = tombstone.encode()?;
        let content = PeerReenrollmentAcceptanceReceiptContent {
            version: RECEIPT_VERSION,
            local_account_id: tombstone.local_account_id(),
            local_device_id: tombstone.local_device_id(),
            local_authority_revision: tombstone.local_authority_revision(),
            local_authority_digest: tombstone.local_authority_digest(),
            peer_account_id: tombstone.peer_account_id(),
            quarantined_peer_device_id: tombstone.quarantined_peer_device_id(),
            quarantine_record_id: tombstone.quarantine_record_id(),
            quarantine_sequence: tombstone.quarantine_sequence(),
            quarantine_reason: tombstone.quarantine_reason(),
            predecessor_record_id: tombstone.predecessor_record_id(),
            predecessor_floor: tombstone.predecessor_floor(),
            tombstone_plan_id: tombstone.plan_id()?,
            tombstone_artifact_digest: tombstone_digest(&tombstone_bytes),
            peer_authority_revision: tombstone.peer_authority_revision(),
            peer_authority_digest: tombstone.peer_authority_digest(),
            replacement_peer_device_id: tombstone.replacement_peer_device_id(),
            successor_floor: tombstone.successor_floor(),
            invitation_id: tombstone.invitation_id(),
            required_capability_id: tombstone.required_capability_id(),
            ticket_publication_epoch: tombstone.ticket_publication_epoch(),
            tombstone_prepared_at_unix_seconds: tombstone.prepared_at_unix_seconds(),
            tombstone_expires_at_unix_seconds: tombstone.expires_at_unix_seconds(),
            accepted_at_unix_seconds,
        };
        validate_receipt_content(&content)?;
        if owner.device_id() != content.local_device_id {
            return Err(PeerReenrollmentAcceptanceStoreError::OwnerDeviceMismatch);
        }
        let signature = owner.sign(&receipt_signing_bytes(&content)?).to_vec();
        let receipt = Self { content, signature };
        receipt.verify()?;
        Ok(receipt)
    }

    fn decode_and_verify(
        encoded: &[u8],
        expected_local_account_id: AccountId,
        expected_local_device_id: DeviceId,
    ) -> Result<Self, PeerReenrollmentAcceptanceStoreError> {
        if encoded.is_empty() || encoded.len() > MAX_PEER_REENROLLMENT_ACCEPTANCE_RECEIPT_BYTES {
            return Err(PeerReenrollmentAcceptanceStoreError::ReceiptLength(
                encoded.len(),
            ));
        }
        let wire: SignedPeerReenrollmentAcceptanceReceiptWire = postcard::from_bytes(encoded)?;
        let receipt = Self {
            content: wire.content,
            signature: wire.signature,
        };
        receipt.verify()?;
        if receipt.local_account_id() != expected_local_account_id {
            return Err(PeerReenrollmentAcceptanceStoreError::OwnerAccountMismatch);
        }
        if receipt.local_device_id() != expected_local_device_id {
            return Err(PeerReenrollmentAcceptanceStoreError::OwnerDeviceMismatch);
        }
        if receipt.encode()? != encoded {
            return Err(PeerReenrollmentAcceptanceStoreError::NonCanonicalReceipt);
        }
        Ok(receipt)
    }

    pub fn encode(&self) -> Result<Vec<u8>, PeerReenrollmentAcceptanceStoreError> {
        self.verify()?;
        let encoded = postcard::to_allocvec(&SignedPeerReenrollmentAcceptanceReceiptWire {
            content: self.content.clone(),
            signature: self.signature.clone(),
        })?;
        if encoded.is_empty() || encoded.len() > MAX_PEER_REENROLLMENT_ACCEPTANCE_RECEIPT_BYTES {
            return Err(PeerReenrollmentAcceptanceStoreError::ReceiptLength(
                encoded.len(),
            ));
        }
        Ok(encoded)
    }

    pub fn receipt_id(&self) -> Result<[u8; 32], PeerReenrollmentAcceptanceStoreError> {
        let encoded = self.encode()?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(RECEIPT_ID_DOMAIN);
        hasher.update(&encoded);
        Ok(*hasher.finalize().as_bytes())
    }

    pub fn local_account_id(&self) -> AccountId {
        self.content.local_account_id
    }

    pub fn local_device_id(&self) -> DeviceId {
        self.content.local_device_id
    }

    pub fn peer_account_id(&self) -> AccountId {
        self.content.peer_account_id
    }

    pub fn local_authority_revision(&self) -> u64 {
        self.content.local_authority_revision
    }

    pub fn local_authority_digest(&self) -> [u8; 32] {
        self.content.local_authority_digest
    }

    pub fn quarantined_peer_device_id(&self) -> DeviceId {
        self.content.quarantined_peer_device_id
    }

    pub fn quarantine_record_id(&self) -> [u8; 32] {
        self.content.quarantine_record_id
    }

    pub fn quarantine_sequence(&self) -> u64 {
        self.content.quarantine_sequence
    }

    pub fn quarantine_reason(&self) -> PeerSyncCapabilityQuarantineReason {
        self.content.quarantine_reason
    }

    pub fn predecessor_record_id(&self) -> [u8; 32] {
        self.content.predecessor_record_id
    }

    pub fn predecessor_floor(&self) -> PeerSyncCapabilityFloor {
        self.content.predecessor_floor
    }

    pub fn tombstone_plan_id(&self) -> [u8; 32] {
        self.content.tombstone_plan_id
    }

    pub fn matches_tombstone(
        &self,
        tombstone: &SignedPeerReenrollmentTombstone,
    ) -> Result<bool, PeerReenrollmentAcceptanceStoreError> {
        let encoded = tombstone.encode()?;
        Ok(tombstone.plan_id()? == self.content.tombstone_plan_id
            && tombstone_digest(&encoded) == self.content.tombstone_artifact_digest)
    }

    pub fn replacement_peer_device_id(&self) -> DeviceId {
        self.content.replacement_peer_device_id
    }

    pub fn peer_authority_revision(&self) -> u64 {
        self.content.peer_authority_revision
    }

    pub fn peer_authority_digest(&self) -> [u8; 32] {
        self.content.peer_authority_digest
    }

    pub fn invitation_id(&self) -> [u8; 32] {
        self.content.invitation_id
    }

    pub fn required_capability_id(&self) -> [u8; 32] {
        self.content.required_capability_id
    }

    pub fn ticket_publication_epoch(&self) -> u64 {
        self.content.ticket_publication_epoch
    }

    pub fn successor_floor(&self) -> PeerSyncCapabilityFloor {
        self.content.successor_floor
    }

    pub fn accepted_at_unix_seconds(&self) -> u64 {
        self.content.accepted_at_unix_seconds
    }

    pub fn tombstone_prepared_at_unix_seconds(&self) -> u64 {
        self.content.tombstone_prepared_at_unix_seconds
    }

    pub fn tombstone_expires_at_unix_seconds(&self) -> u64 {
        self.content.tombstone_expires_at_unix_seconds
    }

    /// Revalidates the complete immutable acceptance context without applying
    /// a new wall-clock freshness rule. The short invitation lifetime gates
    /// the original approval; a later fresh-start replay must instead prove it
    /// is consuming the exact artifact that was already accepted.
    pub fn verify_bound_context(
        &self,
        local_authority: &AccountDeviceListSnapshot,
        quarantine: &DurablePeerCapabilityQuarantineHead,
        peer_authority: &AccountDeviceListSnapshot,
        invitation: &SignedCurrentPeerInvitation,
    ) -> Result<(), PeerReenrollmentAcceptanceStoreError> {
        self.verify()?;
        local_authority.verify()?;
        peer_authority.verify()?;
        invitation.verify()?;
        let predecessor = quarantine.predecessor();
        let head = quarantine.head();
        if local_authority.account_id() != self.local_account_id()
            || local_authority.revision() != self.local_authority_revision()
            || peer_reenrollment_authority_digest(local_authority)? != self.local_authority_digest()
            || peer_authority.account_id() != self.peer_account_id()
            || peer_authority.revision() != self.peer_authority_revision()
            || peer_reenrollment_authority_digest(peer_authority)? != self.peer_authority_digest()
            || quarantine.key().peer_account_id != self.peer_account_id()
            || quarantine.key().peer_device_id != self.quarantined_peer_device_id()
            || head.record_id != self.quarantine_record_id()
            || head.sequence != self.quarantine_sequence()
            || quarantine.reason() != self.quarantine_reason()
            || predecessor.record_id != self.predecessor_record_id()
            || predecessor.floor != self.predecessor_floor()
            || invitation.listener_account_id() != self.peer_account_id()
            || invitation.listener_device_id() != self.replacement_peer_device_id()
            || invitation.allowed_requester_account_id() != self.local_account_id()
            || invitation.authority_revision() != self.peer_authority_revision()
            || invitation.invitation_id()? != self.invitation_id()
            || invitation.required_capability_id() != self.required_capability_id()
            || invitation.ticket_publication_channel_epoch() != self.ticket_publication_epoch()
            || invitation.required_floor()? != self.successor_floor()
        {
            return Err(PeerReenrollmentAcceptanceStoreError::LiveContextMismatch);
        }
        Ok(())
    }

    fn verify(&self) -> Result<(), PeerReenrollmentAcceptanceStoreError> {
        validate_receipt_content(&self.content)?;
        self.content
            .local_device_id
            .verify(&receipt_signing_bytes(&self.content)?, &self.signature)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PeerReenrollmentAcceptanceHeadWire {
    version: u8,
    receipt_id: [u8; 32],
    peer_account_id: AccountId,
    quarantined_peer_device_id: DeviceId,
    quarantine_record_id: [u8; 32],
    replacement_peer_device_id: DeviceId,
    successor_floor: PeerSyncCapabilityFloor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurablePeerReenrollmentAcceptance {
    receipt_id: [u8; 32],
    receipt: SignedPeerReenrollmentAcceptanceReceipt,
}

impl DurablePeerReenrollmentAcceptance {
    pub fn receipt_id(&self) -> [u8; 32] {
        self.receipt_id
    }

    pub fn receipt(&self) -> &SignedPeerReenrollmentAcceptanceReceipt {
        &self.receipt
    }

    pub fn peer_account_id(&self) -> AccountId {
        self.receipt.peer_account_id()
    }

    pub fn replacement_peer_device_id(&self) -> DeviceId {
        self.receipt.replacement_peer_device_id()
    }

    pub fn successor_floor(&self) -> PeerSyncCapabilityFloor {
        self.receipt.successor_floor()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PeerReenrollmentAcceptanceOutcome {
    Accepted(DurablePeerReenrollmentAcceptance),
    AlreadyAccepted(DurablePeerReenrollmentAcceptance),
}

pub struct PeerReenrollmentAcceptanceStore {
    directory: PathBuf,
    database: Database,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

pub struct PeerReenrollmentAcceptanceReadStore {
    directory: PathBuf,
    database: ReadOnlyDatabase,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

impl PeerReenrollmentAcceptanceReadStore {
    pub fn open_existing(
        directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Result<Self, PeerReenrollmentAcceptanceStoreError> {
        let directory = directory.as_ref().to_path_buf();
        let database_path = directory.join(DATABASE_FILE);
        if !database_path.is_file() {
            return Err(PeerReenrollmentAcceptanceStoreError::StoreNotInitialized);
        }
        let database = ReadOnlyDatabase::open(database_path).map_err(database_error)?;
        verify_database_metadata(&database, owner_account_id, owner_device_id)?;
        verify_all_acceptances(&database, owner_account_id, owner_device_id)?;
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

    pub fn load_for_peer(
        &self,
        peer_account_id: AccountId,
    ) -> Result<Option<DurablePeerReenrollmentAcceptance>, PeerReenrollmentAcceptanceStoreError>
    {
        load_verified_head(
            &self.database,
            self.owner_account_id,
            self.owner_device_id,
            peer_account_id,
        )
    }
}

impl PeerReenrollmentAcceptanceStore {
    pub fn is_initialized(directory: impl AsRef<Path>) -> bool {
        directory.as_ref().join(DATABASE_FILE).is_file()
    }

    pub fn open(
        directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Result<Self, PeerReenrollmentAcceptanceStoreError> {
        let directory = directory.as_ref().to_path_buf();
        std::fs::create_dir_all(&directory)?;
        let database = Database::create(directory.join(DATABASE_FILE)).map_err(database_error)?;
        initialize_database(&database, owner_account_id, owner_device_id)?;
        verify_all_acceptances(&database, owner_account_id, owner_device_id)?;
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

    pub fn load_for_peer(
        &self,
        peer_account_id: AccountId,
    ) -> Result<Option<DurablePeerReenrollmentAcceptance>, PeerReenrollmentAcceptanceStoreError>
    {
        load_verified_head(
            &self.database,
            self.owner_account_id,
            self.owner_device_id,
            peer_account_id,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn accept(
        &self,
        owner: &DeviceIdentity,
        tombstone: &SignedPeerReenrollmentTombstone,
        local_authority: &AccountDeviceListSnapshot,
        quarantine: &DurablePeerCapabilityQuarantineHead,
        peer_authority: &AccountDeviceListSnapshot,
        invitation: &SignedCurrentPeerInvitation,
        now_unix_seconds: u64,
    ) -> Result<PeerReenrollmentAcceptanceOutcome, PeerReenrollmentAcceptanceStoreError> {
        self.accept_inner(
            owner,
            tombstone,
            local_authority,
            quarantine,
            peer_authority,
            invitation,
            now_unix_seconds,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn accept_inner(
        &self,
        owner: &DeviceIdentity,
        tombstone: &SignedPeerReenrollmentTombstone,
        local_authority: &AccountDeviceListSnapshot,
        quarantine: &DurablePeerCapabilityQuarantineHead,
        peer_authority: &AccountDeviceListSnapshot,
        invitation: &SignedCurrentPeerInvitation,
        now_unix_seconds: u64,
        fail_before_commit: bool,
    ) -> Result<PeerReenrollmentAcceptanceOutcome, PeerReenrollmentAcceptanceStoreError> {
        if owner.device_id() != self.owner_device_id
            || tombstone.local_device_id() != self.owner_device_id
        {
            return Err(PeerReenrollmentAcceptanceStoreError::OwnerDeviceMismatch);
        }
        if tombstone.local_account_id() != self.owner_account_id
            || local_authority.account_id() != self.owner_account_id
        {
            return Err(PeerReenrollmentAcceptanceStoreError::OwnerAccountMismatch);
        }

        let peer_account_id = tombstone.peer_account_id();
        let tombstone_bytes = tombstone.encode()?;
        let candidate_plan_id = tombstone.plan_id()?;
        let candidate_digest = tombstone_digest(&tombstone_bytes);

        if let Some(existing) = self.load_for_peer(peer_account_id)? {
            tombstone.verify_bound_context(
                local_authority,
                quarantine,
                peer_authority,
                invitation,
            )?;
            if existing.receipt.content.tombstone_plan_id == candidate_plan_id
                && existing.receipt.content.tombstone_artifact_digest == candidate_digest
            {
                return Ok(PeerReenrollmentAcceptanceOutcome::AlreadyAccepted(existing));
            }
            return Err(PeerReenrollmentAcceptanceStoreError::SuccessorAlreadyAccepted);
        }

        tombstone.verify_context(
            local_authority,
            quarantine,
            peer_authority,
            invitation,
            now_unix_seconds,
        )?;
        let receipt =
            SignedPeerReenrollmentAcceptanceReceipt::issue(owner, tombstone, now_unix_seconds)?;
        let receipt_id = receipt.receipt_id()?;
        let receipt_bytes = receipt.encode()?;
        let head = head_wire(&receipt, receipt_id);
        let head_bytes = postcard::to_allocvec(&head)?;
        if head_bytes.is_empty() || head_bytes.len() > MAX_PEER_REENROLLMENT_ACCEPTANCE_HEAD_BYTES {
            return Err(PeerReenrollmentAcceptanceStoreError::HeadLength(
                head_bytes.len(),
            ));
        }

        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut receipts = write.open_table(RECEIPT_TABLE).map_err(database_error)?;
            let mut heads = write.open_table(HEAD_TABLE).map_err(database_error)?;
            let account_key = peer_account_id.as_bytes();
            if let Some(encoded_head) = heads.get(account_key.as_slice()).map_err(database_error)? {
                let head: PeerReenrollmentAcceptanceHeadWire =
                    postcard::from_bytes(encoded_head.value())?;
                validate_head(&head, peer_account_id)?;
                let encoded_existing = receipts
                    .get(head.receipt_id.as_slice())
                    .map_err(database_error)?
                    .ok_or(PeerReenrollmentAcceptanceStoreError::MissingReceipt)?;
                let existing = SignedPeerReenrollmentAcceptanceReceipt::decode_and_verify(
                    encoded_existing.value(),
                    self.owner_account_id,
                    self.owner_device_id,
                )?;
                if existing.receipt_id()? != head.receipt_id
                    || head != head_wire(&existing, head.receipt_id)
                {
                    return Err(PeerReenrollmentAcceptanceStoreError::HeadReceiptMismatch);
                }
                let durable = DurablePeerReenrollmentAcceptance {
                    receipt_id: head.receipt_id,
                    receipt: existing,
                };
                if durable.receipt.content.tombstone_plan_id == candidate_plan_id
                    && durable.receipt.content.tombstone_artifact_digest == candidate_digest
                {
                    return Ok(PeerReenrollmentAcceptanceOutcome::AlreadyAccepted(durable));
                }
                return Err(PeerReenrollmentAcceptanceStoreError::SuccessorAlreadyAccepted);
            }
            if receipts.len().map_err(database_error)? >= MAX_DURABLE_PEER_REENROLLMENT_ACCEPTANCES
            {
                return Err(PeerReenrollmentAcceptanceStoreError::AcceptanceCapacity);
            }
            if let Some(existing) = receipts
                .get(receipt_id.as_slice())
                .map_err(database_error)?
            {
                if existing.value() != receipt_bytes.as_slice() {
                    return Err(PeerReenrollmentAcceptanceStoreError::ReceiptIdCollision);
                }
                return Err(PeerReenrollmentAcceptanceStoreError::OrphanReceipt);
            }
            receipts
                .insert(receipt_id.as_slice(), receipt_bytes.as_slice())
                .map_err(database_error)?;
            heads
                .insert(account_key.as_slice(), head_bytes.as_slice())
                .map_err(database_error)?;
        }
        if fail_before_commit {
            return Err(PeerReenrollmentAcceptanceStoreError::InjectedBeforeCommit);
        }
        write.commit().map_err(database_error)?;

        let durable = self
            .load_for_peer(peer_account_id)?
            .ok_or(PeerReenrollmentAcceptanceStoreError::MissingHead)?;
        if durable.receipt_id != receipt_id || durable.receipt != receipt {
            return Err(PeerReenrollmentAcceptanceStoreError::CommittedHeadMismatch);
        }
        Ok(PeerReenrollmentAcceptanceOutcome::Accepted(durable))
    }

    #[cfg(test)]
    fn corrupt_receipt_for_test(
        &self,
        peer_account_id: AccountId,
    ) -> Result<(), PeerReenrollmentAcceptanceStoreError> {
        let current = self
            .load_for_peer(peer_account_id)?
            .ok_or(PeerReenrollmentAcceptanceStoreError::MissingHead)?;
        let mut encoded = current.receipt.encode()?;
        let last = encoded
            .last_mut()
            .ok_or(PeerReenrollmentAcceptanceStoreError::ReceiptLength(0))?;
        *last ^= 1;
        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut receipts = write.open_table(RECEIPT_TABLE).map_err(database_error)?;
            receipts
                .insert(current.receipt_id.as_slice(), encoded.as_slice())
                .map_err(database_error)?;
        }
        write.commit().map_err(database_error)
    }
}

fn head_wire(
    receipt: &SignedPeerReenrollmentAcceptanceReceipt,
    receipt_id: [u8; 32],
) -> PeerReenrollmentAcceptanceHeadWire {
    PeerReenrollmentAcceptanceHeadWire {
        version: HEAD_VERSION,
        receipt_id,
        peer_account_id: receipt.peer_account_id(),
        quarantined_peer_device_id: receipt.quarantined_peer_device_id(),
        quarantine_record_id: receipt.quarantine_record_id(),
        replacement_peer_device_id: receipt.replacement_peer_device_id(),
        successor_floor: receipt.successor_floor(),
    }
}

fn load_verified_head<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    peer_account_id: AccountId,
) -> Result<Option<DurablePeerReenrollmentAcceptance>, PeerReenrollmentAcceptanceStoreError> {
    let read = database.begin_read().map_err(database_error)?;
    let heads = read.open_table(HEAD_TABLE).map_err(database_error)?;
    let Some(encoded_head) = heads
        .get(peer_account_id.as_bytes().as_slice())
        .map_err(database_error)?
    else {
        return Ok(None);
    };
    if encoded_head.value().is_empty()
        || encoded_head.value().len() > MAX_PEER_REENROLLMENT_ACCEPTANCE_HEAD_BYTES
    {
        return Err(PeerReenrollmentAcceptanceStoreError::HeadLength(
            encoded_head.value().len(),
        ));
    }
    let head: PeerReenrollmentAcceptanceHeadWire = postcard::from_bytes(encoded_head.value())?;
    validate_head(&head, peer_account_id)?;
    let receipts = read.open_table(RECEIPT_TABLE).map_err(database_error)?;
    let encoded_receipt = receipts
        .get(head.receipt_id.as_slice())
        .map_err(database_error)?
        .ok_or(PeerReenrollmentAcceptanceStoreError::MissingReceipt)?;
    let receipt = SignedPeerReenrollmentAcceptanceReceipt::decode_and_verify(
        encoded_receipt.value(),
        owner_account_id,
        owner_device_id,
    )?;
    if receipt.receipt_id()? != head.receipt_id || head != head_wire(&receipt, head.receipt_id) {
        return Err(PeerReenrollmentAcceptanceStoreError::HeadReceiptMismatch);
    }
    Ok(Some(DurablePeerReenrollmentAcceptance {
        receipt_id: head.receipt_id,
        receipt,
    }))
}

fn verify_all_acceptances<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), PeerReenrollmentAcceptanceStoreError> {
    let read = database.begin_read().map_err(database_error)?;
    let heads = read.open_table(HEAD_TABLE).map_err(database_error)?;
    if heads.len().map_err(database_error)? > MAX_DURABLE_PEER_REENROLLMENT_ACCEPTANCES {
        return Err(PeerReenrollmentAcceptanceStoreError::AcceptanceCapacity);
    }
    let mut peer_accounts = Vec::new();
    let mut referenced_receipts = HashSet::new();
    for item in heads.iter().map_err(database_error)? {
        let (account, encoded_head) = item.map_err(database_error)?;
        let peer_account_id = decode_account_id(account.value())?;
        if encoded_head.value().is_empty()
            || encoded_head.value().len() > MAX_PEER_REENROLLMENT_ACCEPTANCE_HEAD_BYTES
        {
            return Err(PeerReenrollmentAcceptanceStoreError::HeadLength(
                encoded_head.value().len(),
            ));
        }
        let head: PeerReenrollmentAcceptanceHeadWire = postcard::from_bytes(encoded_head.value())?;
        validate_head(&head, peer_account_id)?;
        if !referenced_receipts.insert(head.receipt_id) {
            return Err(PeerReenrollmentAcceptanceStoreError::DuplicateHeadReceipt);
        }
        peer_accounts.push(peer_account_id);
    }
    drop(heads);
    let receipts = read.open_table(RECEIPT_TABLE).map_err(database_error)?;
    if receipts.len().map_err(database_error)? > MAX_DURABLE_PEER_REENROLLMENT_ACCEPTANCES {
        return Err(PeerReenrollmentAcceptanceStoreError::AcceptanceCapacity);
    }
    for item in receipts.iter().map_err(database_error)? {
        let (stored_id, encoded_receipt) = item.map_err(database_error)?;
        let stored_id = copy_id(stored_id.value())?;
        if !referenced_receipts.contains(&stored_id) {
            return Err(PeerReenrollmentAcceptanceStoreError::OrphanReceipt);
        }
        let receipt = SignedPeerReenrollmentAcceptanceReceipt::decode_and_verify(
            encoded_receipt.value(),
            owner_account_id,
            owner_device_id,
        )?;
        if receipt.receipt_id()? != stored_id {
            return Err(PeerReenrollmentAcceptanceStoreError::ReceiptIdBinding);
        }
    }
    drop(receipts);
    drop(read);
    for peer_account_id in peer_accounts {
        if load_verified_head(database, owner_account_id, owner_device_id, peer_account_id)?
            .is_none()
        {
            return Err(PeerReenrollmentAcceptanceStoreError::MissingHead);
        }
    }
    Ok(())
}

fn initialize_database(
    database: &Database,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), PeerReenrollmentAcceptanceStoreError> {
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
            PeerReenrollmentAcceptanceStoreError::SchemaMismatch,
        )?;
        initialize_meta_value(
            &mut meta,
            OWNER_ACCOUNT_KEY,
            owner_account_id.as_bytes(),
            PeerReenrollmentAcceptanceStoreError::OwnerAccountMismatch,
        )?;
        initialize_meta_value(
            &mut meta,
            OWNER_DEVICE_KEY,
            owner_device_id.as_bytes(),
            PeerReenrollmentAcceptanceStoreError::OwnerDeviceMismatch,
        )?;
    }
    write.open_table(RECEIPT_TABLE).map_err(database_error)?;
    write.open_table(HEAD_TABLE).map_err(database_error)?;
    write.commit().map_err(database_error)
}

fn initialize_meta_value(
    meta: &mut redb::Table<'_, &str, &[u8]>,
    key: &'static str,
    expected: &[u8],
    mismatch: PeerReenrollmentAcceptanceStoreError,
) -> Result<(), PeerReenrollmentAcceptanceStoreError> {
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
) -> Result<(), PeerReenrollmentAcceptanceStoreError> {
    let read = database.begin_read().map_err(database_error)?;
    let meta = read.open_table(META_TABLE).map_err(database_error)?;
    verify_meta_value(&meta, SCHEMA_KEY, &[SCHEMA_VERSION], || {
        PeerReenrollmentAcceptanceStoreError::SchemaMismatch
    })?;
    verify_meta_value(
        &meta,
        OWNER_ACCOUNT_KEY,
        owner_account_id.as_bytes(),
        || PeerReenrollmentAcceptanceStoreError::OwnerAccountMismatch,
    )?;
    verify_meta_value(&meta, OWNER_DEVICE_KEY, owner_device_id.as_bytes(), || {
        PeerReenrollmentAcceptanceStoreError::OwnerDeviceMismatch
    })?;
    drop(meta);
    let receipts = read.open_table(RECEIPT_TABLE).map_err(database_error)?;
    drop(receipts);
    let heads = read.open_table(HEAD_TABLE).map_err(database_error)?;
    drop(heads);
    Ok(())
}

fn verify_meta_value(
    meta: &impl ReadableTable<&'static str, &'static [u8]>,
    key: &'static str,
    expected: &[u8],
    mismatch: impl Fn() -> PeerReenrollmentAcceptanceStoreError,
) -> Result<(), PeerReenrollmentAcceptanceStoreError> {
    let value = meta
        .get(key)
        .map_err(database_error)?
        .ok_or_else(&mismatch)?;
    if value.value() != expected {
        return Err(mismatch());
    }
    Ok(())
}

fn validate_receipt_content(
    content: &PeerReenrollmentAcceptanceReceiptContent,
) -> Result<(), PeerReenrollmentAcceptanceStoreError> {
    let expected_floor = PeerSyncCapabilityFloor::CurrentInvitationRequired {
        invitation_id: content.invitation_id,
        required_capability_id: content.required_capability_id,
        minimum_authority_revision: content.peer_authority_revision,
        minimum_ticket_publication_epoch: content.ticket_publication_epoch,
    };
    if content.version != RECEIPT_VERSION
        || content.local_account_id == content.peer_account_id
        || content.local_authority_revision == 0
        || content.peer_authority_revision == 0
        || content.local_authority_digest == [0_u8; 32]
        || content.peer_authority_digest == [0_u8; 32]
        || content.quarantine_record_id == [0_u8; 32]
        || content.quarantine_sequence < 2
        || content.predecessor_record_id == [0_u8; 32]
        || content.tombstone_plan_id == [0_u8; 32]
        || content.tombstone_artifact_digest == [0_u8; 32]
        || content.invitation_id == [0_u8; 32]
        || content.required_capability_id == [0_u8; 32]
        || content.successor_floor != expected_floor
        || !matches!(
            content.predecessor_floor,
            PeerSyncCapabilityFloor::CurrentInvitationRequired { .. }
                | PeerSyncCapabilityFloor::CanonicalCandidatePinned { .. }
        )
        || content.accepted_at_unix_seconds < content.tombstone_prepared_at_unix_seconds
        || content.accepted_at_unix_seconds > content.tombstone_expires_at_unix_seconds
    {
        return Err(PeerReenrollmentAcceptanceStoreError::InvalidReceipt);
    }
    Ok(())
}

fn validate_head(
    head: &PeerReenrollmentAcceptanceHeadWire,
    expected_peer_account_id: AccountId,
) -> Result<(), PeerReenrollmentAcceptanceStoreError> {
    if head.version != HEAD_VERSION
        || head.receipt_id == [0_u8; 32]
        || head.peer_account_id != expected_peer_account_id
        || head.quarantine_record_id == [0_u8; 32]
        || !matches!(
            head.successor_floor,
            PeerSyncCapabilityFloor::CurrentInvitationRequired { .. }
        )
    {
        return Err(PeerReenrollmentAcceptanceStoreError::InvalidHead);
    }
    Ok(())
}

fn receipt_signing_bytes(
    content: &PeerReenrollmentAcceptanceReceiptContent,
) -> Result<Vec<u8>, PeerReenrollmentAcceptanceStoreError> {
    validate_receipt_content(content)?;
    let encoded = postcard::to_allocvec(content)?;
    let mut signing = Vec::with_capacity(RECEIPT_SIGNATURE_DOMAIN.len() + encoded.len());
    signing.extend_from_slice(RECEIPT_SIGNATURE_DOMAIN);
    signing.extend_from_slice(&encoded);
    Ok(signing)
}

fn tombstone_digest(encoded: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(TOMBSTONE_DIGEST_DOMAIN);
    hasher.update(encoded);
    *hasher.finalize().as_bytes()
}

fn copy_id(bytes: &[u8]) -> Result<[u8; 32], PeerReenrollmentAcceptanceStoreError> {
    bytes
        .try_into()
        .map_err(|_| PeerReenrollmentAcceptanceStoreError::StoredIdLength(bytes.len()))
}

fn decode_account_id(bytes: &[u8]) -> Result<AccountId, PeerReenrollmentAcceptanceStoreError> {
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| PeerReenrollmentAcceptanceStoreError::AccountIdLength(bytes.len()))?;
    Ok(AccountId::from_bytes(bytes))
}

fn database_error(error: impl std::fmt::Display) -> PeerReenrollmentAcceptanceStoreError {
    PeerReenrollmentAcceptanceStoreError::Database(error.to_string())
}

#[derive(Debug, Error)]
pub enum PeerReenrollmentAcceptanceStoreError {
    #[error("peer re-enrollment acceptance store is not initialized")]
    StoreNotInitialized,
    #[error("peer re-enrollment acceptance store I/O failed")]
    Io(#[from] std::io::Error),
    #[error("peer re-enrollment acceptance database failed: {0}")]
    Database(String),
    #[error("peer re-enrollment acceptance schema does not match")]
    SchemaMismatch,
    #[error("peer re-enrollment acceptance owner Account does not match")]
    OwnerAccountMismatch,
    #[error("peer re-enrollment acceptance owner Device does not match")]
    OwnerDeviceMismatch,
    #[error("peer re-enrollment acceptance receipt is invalid")]
    InvalidReceipt,
    #[error("peer re-enrollment acceptance head is invalid")]
    InvalidHead,
    #[error("peer re-enrollment acceptance receipt is {0} bytes and violates its bound")]
    ReceiptLength(usize),
    #[error("peer re-enrollment acceptance head is {0} bytes and violates its bound")]
    HeadLength(usize),
    #[error("peer re-enrollment acceptance receipt is not canonically encoded")]
    NonCanonicalReceipt,
    #[error("peer re-enrollment acceptance head references a missing receipt")]
    MissingReceipt,
    #[error("peer re-enrollment acceptance store has no expected head")]
    MissingHead,
    #[error("peer re-enrollment acceptance head does not match its receipt")]
    HeadReceiptMismatch,
    #[error("peer re-enrollment acceptance receipt ID does not bind its bytes")]
    ReceiptIdBinding,
    #[error("distinct peer re-enrollment receipts produced the same ID")]
    ReceiptIdCollision,
    #[error("peer re-enrollment acceptance store contains an orphan receipt")]
    OrphanReceipt,
    #[error("multiple peer re-enrollment heads reference one receipt")]
    DuplicateHeadReceipt,
    #[error("another peer re-enrollment successor is already accepted")]
    SuccessorAlreadyAccepted,
    #[error("peer re-enrollment acceptance no longer matches its live signed context")]
    LiveContextMismatch,
    #[error("peer re-enrollment acceptance head changed concurrently")]
    ConcurrentHeadChange,
    #[error("peer re-enrollment acceptance committed an unexpected head")]
    CommittedHeadMismatch,
    #[error("peer re-enrollment acceptance capacity is exhausted")]
    AcceptanceCapacity,
    #[error("stored peer re-enrollment receipt ID has invalid length {0}")]
    StoredIdLength(usize),
    #[error("stored peer re-enrollment Account ID has invalid length {0}")]
    AccountIdLength(usize),
    #[error("test-only failure injected before the atomic commit")]
    InjectedBeforeCommit,
    #[error("peer re-enrollment acceptance encoding is invalid: {0}")]
    Codec(#[from] postcard::Error),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Reenrollment(#[from] PeerReenrollmentError),
    #[error(transparent)]
    SyncCapability(#[from] SyncTransportCapabilityError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use kilogram_identity::{AccountRootState, DeviceCapability, DeviceState};
    use kilogram_peer_capability_store::{
        PeerCapabilityKey, PeerCapabilityReadStore, PeerCapabilityStore,
    };
    use kilogram_peer_reenrollment::{ExplicitPeerReenrollmentApproval, PeerReenrollmentPlan};
    use kilogram_sync_capability::{
        CurrentPeerInvitationBinding, PeerSyncCapabilityEvidence, SyncTransportCapabilityError,
    };
    use tempfile::TempDir;

    struct Fixture {
        root: TempDir,
        local_identity: DeviceIdentity,
        local_authority: AccountDeviceListSnapshot,
        quarantine: DurablePeerCapabilityQuarantineHead,
        peer_authority: AccountDeviceListSnapshot,
        invitation: SignedCurrentPeerInvitation,
        peer_new_identity: DeviceIdentity,
        second_peer_authority: AccountDeviceListSnapshot,
        second_invitation: SignedCurrentPeerInvitation,
        store_path: PathBuf,
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

        let peer_root = AccountRootState::create(root.path().join("peer-root"))?;
        let peer_old = DeviceState::load_or_create(root.path().join("peer-old"))?;
        let old_certificate = peer_root.issue_device_certificate(
            peer_old.identity().device_id(),
            peer_old.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let old_authority =
            peer_root.publish_device_list(std::slice::from_ref(&old_certificate))?;
        let old_invitation = invitation(
            peer_old.identity(),
            peer_root.account_id(),
            local_root.account_id(),
            old_authority.revision(),
            7,
            [7_u8; 32],
        )?;

        let store_path = root.path().join("peer-capability-store");
        let store = PeerCapabilityStore::open(&store_path, local_device.identity().device_id())?;
        store.apply_current_invitation(
            local_device.identity(),
            local_root.account_id(),
            &old_invitation,
            1_100,
        )?;
        let old_key =
            PeerCapabilityKey::new(peer_root.account_id(), peer_old.identity().device_id());
        store.apply_authenticated_observation(
            local_device.identity(),
            old_key,
            PeerSyncCapabilityEvidence::LegacyTicketWithoutExtension,
        )?;
        let quarantine = store.load_quarantined_head(old_key)?;
        drop(store);

        let peer_new = DeviceState::load_or_create(root.path().join("peer-new"))?;
        let new_certificate = peer_root.issue_device_certificate(
            peer_new.identity().device_id(),
            peer_new.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let peer_authority =
            peer_root.publish_device_list(&[old_certificate.clone(), new_certificate.clone()])?;
        let primary_invitation = invitation(
            peer_new.identity(),
            peer_root.account_id(),
            local_root.account_id(),
            peer_authority.revision(),
            1,
            [9_u8; 32],
        )?;
        let peer_second = DeviceState::load_or_create(root.path().join("peer-second"))?;
        let second_certificate = peer_root.issue_device_certificate(
            peer_second.identity().device_id(),
            peer_second.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let second_peer_authority = peer_root.publish_device_list(&[
            old_certificate,
            new_certificate,
            second_certificate,
        ])?;
        let second_invitation = invitation(
            peer_second.identity(),
            peer_root.account_id(),
            local_root.account_id(),
            second_peer_authority.revision(),
            1,
            [10_u8; 32],
        )?;

        Ok(Fixture {
            root,
            local_identity: DeviceIdentity::from_secret_bytes(
                local_device.identity().secret_bytes(),
            ),
            local_authority,
            quarantine,
            peer_authority,
            invitation: primary_invitation,
            peer_new_identity: DeviceIdentity::from_secret_bytes(
                peer_new.identity().secret_bytes(),
            ),
            second_peer_authority,
            second_invitation,
            store_path,
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

    fn signed(fixture: &Fixture) -> Result<SignedPeerReenrollmentTombstone, PeerReenrollmentError> {
        let plan = PeerReenrollmentPlan::prepare(
            fixture.local_identity.device_id(),
            &fixture.local_authority,
            &fixture.quarantine,
            &fixture.peer_authority,
            &fixture.invitation,
            1_200,
        )?;
        let phrase = plan.confirmation_phrase()?;
        plan.confirm_and_sign(
            &fixture.local_identity,
            &phrase,
            ExplicitPeerReenrollmentApproval::UserOrTestOperatorApproved,
        )
    }

    fn acceptance_store(
        fixture: &Fixture,
    ) -> Result<PeerReenrollmentAcceptanceStore, PeerReenrollmentAcceptanceStoreError> {
        PeerReenrollmentAcceptanceStore::open(
            fixture.root.path().join("peer-reenrollment-acceptance"),
            fixture.local_authority.account_id(),
            fixture.local_identity.device_id(),
        )
    }

    #[test]
    fn acceptance_atomically_commits_signed_receipt_and_preserves_quarantine()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let capability_database = fixture.store_path.join("peer-sync-capability-v1.redb");
        let capability_before = std::fs::read(&capability_database)?;
        let tombstone = signed(&fixture)?;
        let store = acceptance_store(&fixture)?;
        let accepted = store.accept(
            &fixture.local_identity,
            &tombstone,
            &fixture.local_authority,
            &fixture.quarantine,
            &fixture.peer_authority,
            &fixture.invitation,
            1_300,
        )?;
        let PeerReenrollmentAcceptanceOutcome::Accepted(accepted) = accepted else {
            return Err(PeerReenrollmentAcceptanceStoreError::CommittedHeadMismatch.into());
        };
        assert_eq!(
            accepted.peer_account_id(),
            fixture.peer_authority.account_id()
        );
        assert_eq!(
            accepted.replacement_peer_device_id(),
            fixture.peer_new_identity.device_id()
        );
        assert_eq!(accepted.receipt().tombstone_plan_id(), tombstone.plan_id()?);
        assert!(
            accepted.receipt().encode()?.len() <= MAX_PEER_REENROLLMENT_ACCEPTANCE_RECEIPT_BYTES
        );
        assert!(matches!(
            accepted.successor_floor(),
            PeerSyncCapabilityFloor::CurrentInvitationRequired { .. }
        ));
        assert_eq!(std::fs::read(&capability_database)?, capability_before);
        let capability = PeerCapabilityReadStore::open_existing(
            &fixture.store_path,
            fixture.local_identity.device_id(),
        )?;
        assert_eq!(
            capability.load_quarantined_head(fixture.quarantine.key())?,
            fixture.quarantine
        );

        drop(store);
        let read = PeerReenrollmentAcceptanceReadStore::open_existing(
            fixture.root.path().join("peer-reenrollment-acceptance"),
            fixture.local_authority.account_id(),
            fixture.local_identity.device_id(),
        )?;
        let reopened = read
            .load_for_peer(fixture.peer_authority.account_id())?
            .ok_or(PeerReenrollmentAcceptanceStoreError::MissingHead)?;
        assert_eq!(reopened.receipt_id(), accepted.receipt_id());
        Ok(())
    }

    #[test]
    fn exact_replay_is_idempotent_even_after_short_plan_expiry()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let tombstone = signed(&fixture)?;
        let store = acceptance_store(&fixture)?;
        let first = store.accept(
            &fixture.local_identity,
            &tombstone,
            &fixture.local_authority,
            &fixture.quarantine,
            &fixture.peer_authority,
            &fixture.invitation,
            1_300,
        )?;
        let first_id = match first {
            PeerReenrollmentAcceptanceOutcome::Accepted(value) => value.receipt_id(),
            PeerReenrollmentAcceptanceOutcome::AlreadyAccepted(_) => {
                return Err(PeerReenrollmentAcceptanceStoreError::CommittedHeadMismatch.into());
            }
        };
        let replay = store.accept(
            &fixture.local_identity,
            &tombstone,
            &fixture.local_authority,
            &fixture.quarantine,
            &fixture.peer_authority,
            &fixture.invitation,
            tombstone.expires_at_unix_seconds() + 10_000,
        )?;
        let PeerReenrollmentAcceptanceOutcome::AlreadyAccepted(replay) = replay else {
            return Err(PeerReenrollmentAcceptanceStoreError::CommittedHeadMismatch.into());
        };
        assert_eq!(replay.receipt_id(), first_id);
        Ok(())
    }

    #[test]
    fn changed_context_and_second_successor_fail_closed_without_replacing_head()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let tombstone = signed(&fixture)?;
        let store = acceptance_store(&fixture)?;
        let accepted = store.accept(
            &fixture.local_identity,
            &tombstone,
            &fixture.local_authority,
            &fixture.quarantine,
            &fixture.peer_authority,
            &fixture.invitation,
            1_300,
        )?;
        let accepted_id = match accepted {
            PeerReenrollmentAcceptanceOutcome::Accepted(value) => value.receipt_id(),
            PeerReenrollmentAcceptanceOutcome::AlreadyAccepted(_) => {
                return Err(PeerReenrollmentAcceptanceStoreError::CommittedHeadMismatch.into());
            }
        };
        let substituted = invitation(
            &fixture.peer_new_identity,
            fixture.peer_authority.account_id(),
            fixture.local_authority.account_id(),
            fixture.peer_authority.revision(),
            2,
            [24_u8; 32],
        )?;
        assert!(matches!(
            store.accept(
                &fixture.local_identity,
                &tombstone,
                &fixture.local_authority,
                &fixture.quarantine,
                &fixture.peer_authority,
                &substituted,
                1_300,
            ),
            Err(PeerReenrollmentAcceptanceStoreError::Reenrollment(
                PeerReenrollmentError::ContextMismatch
            ))
        ));

        let second_plan = PeerReenrollmentPlan::prepare(
            fixture.local_identity.device_id(),
            &fixture.local_authority,
            &fixture.quarantine,
            &fixture.second_peer_authority,
            &fixture.second_invitation,
            1_200,
        )?;
        assert_ne!(
            second_plan.replacement_peer_device_id(),
            tombstone.replacement_peer_device_id()
        );
        let second_phrase = second_plan.confirmation_phrase()?;
        let second = second_plan.confirm_and_sign(
            &fixture.local_identity,
            &second_phrase,
            ExplicitPeerReenrollmentApproval::UserOrTestOperatorApproved,
        )?;
        assert!(matches!(
            store.accept(
                &fixture.local_identity,
                &second,
                &fixture.local_authority,
                &fixture.quarantine,
                &fixture.second_peer_authority,
                &fixture.second_invitation,
                1_300,
            ),
            Err(PeerReenrollmentAcceptanceStoreError::SuccessorAlreadyAccepted)
        ));
        let original = store
            .load_for_peer(fixture.peer_authority.account_id())?
            .ok_or(PeerReenrollmentAcceptanceStoreError::MissingHead)?;
        assert_eq!(original.receipt_id(), accepted_id);
        Ok(())
    }

    #[test]
    fn pre_commit_failure_leaves_no_receipt_or_successor_head()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let tombstone = signed(&fixture)?;
        let store = acceptance_store(&fixture)?;
        assert!(matches!(
            store.accept_inner(
                &fixture.local_identity,
                &tombstone,
                &fixture.local_authority,
                &fixture.quarantine,
                &fixture.peer_authority,
                &fixture.invitation,
                1_300,
                true,
            ),
            Err(PeerReenrollmentAcceptanceStoreError::InjectedBeforeCommit)
        ));
        assert!(
            store
                .load_for_peer(fixture.peer_authority.account_id())?
                .is_none()
        );
        drop(store);
        let reopened = acceptance_store(&fixture)?;
        assert!(
            reopened
                .load_for_peer(fixture.peer_authority.account_id())?
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn owner_mismatch_and_corrupt_receipt_are_rejected_on_reopen()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let directory = fixture.root.path().join("peer-reenrollment-acceptance");
        let tombstone = signed(&fixture)?;
        let store = acceptance_store(&fixture)?;
        store.accept(
            &fixture.local_identity,
            &tombstone,
            &fixture.local_authority,
            &fixture.quarantine,
            &fixture.peer_authority,
            &fixture.invitation,
            1_300,
        )?;
        drop(store);
        assert!(matches!(
            PeerReenrollmentAcceptanceReadStore::open_existing(
                &directory,
                AccountId::from_bytes([91_u8; 32]),
                fixture.local_identity.device_id(),
            ),
            Err(PeerReenrollmentAcceptanceStoreError::OwnerAccountMismatch)
        ));
        let store = acceptance_store(&fixture)?;
        store.corrupt_receipt_for_test(fixture.peer_authority.account_id())?;
        drop(store);
        assert!(
            PeerReenrollmentAcceptanceReadStore::open_existing(
                &directory,
                fixture.local_authority.account_id(),
                fixture.local_identity.device_id(),
            )
            .is_err()
        );
        Ok(())
    }
}
