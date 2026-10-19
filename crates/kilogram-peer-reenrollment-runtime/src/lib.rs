//! Durable, fresh-start-only consumption of accepted peer successors.
//!
//! The store deliberately has two phases. An explicit local acceptance
//! ceremony prepares a complete signed transition, while a later runtime
//! process may publish its consumed head only during startup and before it
//! opens a network endpoint. Both the successor contact artifact and its
//! mandatory capability floor live in the same signed transition.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use kilogram_identity::{AccountId, DeviceId, DeviceIdentity, IdentityError};
use kilogram_peer_reenrollment_acceptance::DurablePeerReenrollmentAcceptance;
use kilogram_sync_capability::{
    MAX_CURRENT_PEER_INVITATION_BYTES, PeerSyncCapabilityFloor, SignedCurrentPeerInvitation,
    SyncTransportCapabilityError,
};
use redb::{
    Database, Durability, ReadOnlyDatabase, ReadableDatabase, ReadableTable, ReadableTableMetadata,
    TableDefinition,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const DATABASE_FILE: &str = "peer-reenrollment-runtime-v1.redb";
const META_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("peer-reenrollment-runtime-meta-v1");
const PREPARED_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("peer-reenrollment-runtime-prepared-v1");
const CONSUMED_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("peer-reenrollment-runtime-consumed-v1");

const SCHEMA_KEY: &str = "schema-version";
const OWNER_ACCOUNT_KEY: &str = "owner-account-id";
const OWNER_DEVICE_KEY: &str = "owner-device-id";
const SCHEMA_VERSION: u8 = 1;
const TRANSITION_VERSION: u8 = 1;
const SIGNATURE_DOMAIN: &[u8] = b"kilogram:peer-reenrollment-runtime-transition:v1\0";
const TRANSITION_ID_DOMAIN: &[u8] = b"kilogram:peer-reenrollment-runtime-transition-id:v1\0";
const PREDECESSOR_DIGEST_DOMAIN: &[u8] =
    b"kilogram:peer-reenrollment-runtime-predecessor-contact:v1\0";
const SUCCESSOR_DIGEST_DOMAIN: &[u8] = b"kilogram:peer-reenrollment-runtime-successor-contact:v1\0";
const CANDIDATE_DIGEST_DOMAIN: &[u8] =
    b"kilogram:peer-reenrollment-runtime-endpoint-candidate:v1\0";

pub const MAX_PEER_REENROLLMENT_RUNTIME_TRANSITION_BYTES: usize =
    MAX_CURRENT_PEER_INVITATION_BYTES + 64 * 1024;
pub const MAX_PEER_REENROLLMENT_RUNTIME_CONTACT_ARTIFACT_BYTES: usize = 16 * 1024;
pub const MAX_PEER_REENROLLMENT_RUNTIME_TRANSITIONS: u64 = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerReenrollmentRuntimeBinding {
    pub contact_id: [u8; 32],
    pub conversation_id: [u8; 32],
    pub predecessor_contact: Vec<u8>,
    pub successor_contact: Vec<u8>,
    pub successor_endpoint_candidate: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PeerReenrollmentRuntimeTransitionContent {
    version: u8,
    local_account_id: AccountId,
    local_device_id: DeviceId,
    peer_account_id: AccountId,
    quarantined_peer_device_id: DeviceId,
    replacement_peer_device_id: DeviceId,
    acceptance_receipt_id: [u8; 32],
    quarantine_record_id: [u8; 32],
    contact_id: [u8; 32],
    conversation_id: [u8; 32],
    predecessor_contact_digest: [u8; 32],
    successor_contact_digest: [u8; 32],
    successor_endpoint_candidate_digest: [u8; 32],
    successor_contact: Vec<u8>,
    successor_endpoint_candidate: Vec<u8>,
    invitation_id: [u8; 32],
    invitation: Vec<u8>,
    successor_floor: PeerSyncCapabilityFloor,
    prepared_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SignedPeerReenrollmentRuntimeTransitionWire {
    content: PeerReenrollmentRuntimeTransitionContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedPeerReenrollmentRuntimeTransition {
    content: PeerReenrollmentRuntimeTransitionContent,
    signature: Vec<u8>,
}

impl SignedPeerReenrollmentRuntimeTransition {
    fn issue(
        owner: &DeviceIdentity,
        acceptance: &DurablePeerReenrollmentAcceptance,
        invitation: &SignedCurrentPeerInvitation,
        binding: PeerReenrollmentRuntimeBinding,
        prepared_at_unix_seconds: u64,
    ) -> Result<Self, PeerReenrollmentRuntimeStoreError> {
        validate_binding(&binding)?;
        invitation.verify()?;
        let invitation_bytes = serde_json::to_vec(invitation)?;
        if invitation_bytes.len() > MAX_CURRENT_PEER_INVITATION_BYTES {
            return Err(PeerReenrollmentRuntimeStoreError::InvitationLength(
                invitation_bytes.len(),
            ));
        }
        let receipt = acceptance.receipt();
        if owner.device_id() != receipt.local_device_id() {
            return Err(PeerReenrollmentRuntimeStoreError::OwnerDeviceMismatch);
        }
        if invitation.listener_account_id() != receipt.peer_account_id()
            || invitation.listener_device_id() != receipt.replacement_peer_device_id()
            || invitation.allowed_requester_account_id() != receipt.local_account_id()
            || invitation.invitation_id()? != receipt.invitation_id()
            || invitation.required_floor()? != receipt.successor_floor()
        {
            return Err(PeerReenrollmentRuntimeStoreError::InvitationMismatch);
        }
        if prepared_at_unix_seconds < receipt.accepted_at_unix_seconds() {
            return Err(PeerReenrollmentRuntimeStoreError::InvalidTransition);
        }
        let content = PeerReenrollmentRuntimeTransitionContent {
            version: TRANSITION_VERSION,
            local_account_id: receipt.local_account_id(),
            local_device_id: receipt.local_device_id(),
            peer_account_id: receipt.peer_account_id(),
            quarantined_peer_device_id: receipt.quarantined_peer_device_id(),
            replacement_peer_device_id: receipt.replacement_peer_device_id(),
            acceptance_receipt_id: acceptance.receipt_id(),
            quarantine_record_id: receipt.quarantine_record_id(),
            contact_id: binding.contact_id,
            conversation_id: binding.conversation_id,
            predecessor_contact_digest: artifact_digest(
                PREDECESSOR_DIGEST_DOMAIN,
                &binding.predecessor_contact,
            ),
            successor_contact_digest: artifact_digest(
                SUCCESSOR_DIGEST_DOMAIN,
                &binding.successor_contact,
            ),
            successor_endpoint_candidate_digest: artifact_digest(
                CANDIDATE_DIGEST_DOMAIN,
                &binding.successor_endpoint_candidate,
            ),
            successor_contact: binding.successor_contact,
            successor_endpoint_candidate: binding.successor_endpoint_candidate,
            invitation_id: receipt.invitation_id(),
            invitation: invitation_bytes,
            successor_floor: receipt.successor_floor(),
            prepared_at_unix_seconds,
        };
        validate_content(&content)?;
        let signature = owner.sign(&signing_bytes(&content)?).to_vec();
        let transition = Self { content, signature };
        transition.verify()?;
        Ok(transition)
    }

    fn decode_and_verify(
        encoded: &[u8],
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Result<Self, PeerReenrollmentRuntimeStoreError> {
        if encoded.is_empty() || encoded.len() > MAX_PEER_REENROLLMENT_RUNTIME_TRANSITION_BYTES {
            return Err(PeerReenrollmentRuntimeStoreError::TransitionLength(
                encoded.len(),
            ));
        }
        let wire: SignedPeerReenrollmentRuntimeTransitionWire = postcard::from_bytes(encoded)?;
        let transition = Self {
            content: wire.content,
            signature: wire.signature,
        };
        transition.verify()?;
        if transition.local_account_id() != owner_account_id {
            return Err(PeerReenrollmentRuntimeStoreError::OwnerAccountMismatch);
        }
        if transition.local_device_id() != owner_device_id {
            return Err(PeerReenrollmentRuntimeStoreError::OwnerDeviceMismatch);
        }
        if transition.encode()? != encoded {
            return Err(PeerReenrollmentRuntimeStoreError::NonCanonicalTransition);
        }
        Ok(transition)
    }

    pub fn encode(&self) -> Result<Vec<u8>, PeerReenrollmentRuntimeStoreError> {
        self.verify()?;
        let encoded = postcard::to_allocvec(&SignedPeerReenrollmentRuntimeTransitionWire {
            content: self.content.clone(),
            signature: self.signature.clone(),
        })?;
        if encoded.is_empty() || encoded.len() > MAX_PEER_REENROLLMENT_RUNTIME_TRANSITION_BYTES {
            return Err(PeerReenrollmentRuntimeStoreError::TransitionLength(
                encoded.len(),
            ));
        }
        Ok(encoded)
    }

    pub fn transition_id(&self) -> Result<[u8; 32], PeerReenrollmentRuntimeStoreError> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(TRANSITION_ID_DOMAIN);
        hasher.update(&self.encode()?);
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

    pub fn quarantined_peer_device_id(&self) -> DeviceId {
        self.content.quarantined_peer_device_id
    }

    pub fn replacement_peer_device_id(&self) -> DeviceId {
        self.content.replacement_peer_device_id
    }

    pub fn acceptance_receipt_id(&self) -> [u8; 32] {
        self.content.acceptance_receipt_id
    }

    pub fn quarantine_record_id(&self) -> [u8; 32] {
        self.content.quarantine_record_id
    }

    pub fn contact_id(&self) -> [u8; 32] {
        self.content.contact_id
    }

    pub fn conversation_id(&self) -> [u8; 32] {
        self.content.conversation_id
    }

    pub fn predecessor_contact_matches(&self, encoded: &[u8]) -> bool {
        artifact_digest(PREDECESSOR_DIGEST_DOMAIN, encoded)
            == self.content.predecessor_contact_digest
    }

    pub fn successor_contact(&self) -> &[u8] {
        &self.content.successor_contact
    }

    pub fn successor_endpoint_candidate(&self) -> &[u8] {
        &self.content.successor_endpoint_candidate
    }

    pub fn invitation(
        &self,
    ) -> Result<SignedCurrentPeerInvitation, PeerReenrollmentRuntimeStoreError> {
        let invitation: SignedCurrentPeerInvitation =
            serde_json::from_slice(&self.content.invitation)?;
        invitation.verify()?;
        if serde_json::to_vec(&invitation)? != self.content.invitation {
            return Err(PeerReenrollmentRuntimeStoreError::NonCanonicalInvitation);
        }
        Ok(invitation)
    }

    pub fn successor_floor(&self) -> PeerSyncCapabilityFloor {
        self.content.successor_floor
    }

    pub fn prepared_at_unix_seconds(&self) -> u64 {
        self.content.prepared_at_unix_seconds
    }

    fn verify(&self) -> Result<(), PeerReenrollmentRuntimeStoreError> {
        validate_content(&self.content)?;
        self.content
            .local_device_id
            .verify(&signing_bytes(&self.content)?, &self.signature)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurablePeerReenrollmentRuntimeTransition {
    transition_id: [u8; 32],
    transition: SignedPeerReenrollmentRuntimeTransition,
    consumed: bool,
}

impl DurablePeerReenrollmentRuntimeTransition {
    pub fn transition_id(&self) -> [u8; 32] {
        self.transition_id
    }

    pub fn transition(&self) -> &SignedPeerReenrollmentRuntimeTransition {
        &self.transition
    }

    pub fn consumed(&self) -> bool {
        self.consumed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PeerReenrollmentRuntimePrepareOutcome {
    Prepared(DurablePeerReenrollmentRuntimeTransition),
    AlreadyPrepared(DurablePeerReenrollmentRuntimeTransition),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PeerReenrollmentRuntimeConsumeOutcome {
    Consumed(DurablePeerReenrollmentRuntimeTransition),
    AlreadyConsumed(DurablePeerReenrollmentRuntimeTransition),
}

pub struct PeerReenrollmentRuntimeStore {
    directory: PathBuf,
    database: Database,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

pub struct PeerReenrollmentRuntimeReadStore {
    directory: PathBuf,
    database: ReadOnlyDatabase,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

impl PeerReenrollmentRuntimeStore {
    pub fn is_initialized(directory: impl AsRef<Path>) -> bool {
        directory.as_ref().join(DATABASE_FILE).is_file()
    }

    pub fn open(
        directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Result<Self, PeerReenrollmentRuntimeStoreError> {
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

    pub fn prepare(
        &self,
        owner: &DeviceIdentity,
        acceptance: &DurablePeerReenrollmentAcceptance,
        invitation: &SignedCurrentPeerInvitation,
        binding: PeerReenrollmentRuntimeBinding,
        prepared_at_unix_seconds: u64,
    ) -> Result<PeerReenrollmentRuntimePrepareOutcome, PeerReenrollmentRuntimeStoreError> {
        self.prepare_inner(
            owner,
            acceptance,
            invitation,
            binding,
            prepared_at_unix_seconds,
            false,
        )
    }

    fn prepare_inner(
        &self,
        owner: &DeviceIdentity,
        acceptance: &DurablePeerReenrollmentAcceptance,
        invitation: &SignedCurrentPeerInvitation,
        binding: PeerReenrollmentRuntimeBinding,
        prepared_at_unix_seconds: u64,
        fail_before_commit: bool,
    ) -> Result<PeerReenrollmentRuntimePrepareOutcome, PeerReenrollmentRuntimeStoreError> {
        if owner.device_id() != self.owner_device_id {
            return Err(PeerReenrollmentRuntimeStoreError::OwnerDeviceMismatch);
        }
        if acceptance.receipt().local_account_id() != self.owner_account_id {
            return Err(PeerReenrollmentRuntimeStoreError::OwnerAccountMismatch);
        }
        let candidate = SignedPeerReenrollmentRuntimeTransition::issue(
            owner,
            acceptance,
            invitation,
            binding,
            prepared_at_unix_seconds,
        )?;
        let transition_id = candidate.transition_id()?;
        let encoded = candidate.encode()?;
        let key = candidate.contact_id();

        if let Some(existing) = load_transition(
            &self.database,
            self.owner_account_id,
            self.owner_device_id,
            key,
        )? {
            if existing.transition_id == transition_id && existing.transition == candidate {
                return Ok(PeerReenrollmentRuntimePrepareOutcome::AlreadyPrepared(
                    existing,
                ));
            }
            return Err(PeerReenrollmentRuntimeStoreError::ContactAlreadyPrepared);
        }

        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut prepared = write.open_table(PREPARED_TABLE).map_err(database_error)?;
            if prepared.len().map_err(database_error)? >= MAX_PEER_REENROLLMENT_RUNTIME_TRANSITIONS
            {
                return Err(PeerReenrollmentRuntimeStoreError::TransitionCapacity);
            }
            if let Some(existing) = prepared.get(key.as_slice()).map_err(database_error)? {
                let durable = decode_durable(
                    existing.value(),
                    self.owner_account_id,
                    self.owner_device_id,
                    false,
                )?;
                if durable.transition_id == transition_id && durable.transition == candidate {
                    return Ok(PeerReenrollmentRuntimePrepareOutcome::AlreadyPrepared(
                        durable,
                    ));
                }
                return Err(PeerReenrollmentRuntimeStoreError::ContactAlreadyPrepared);
            }
            prepared
                .insert(key.as_slice(), encoded.as_slice())
                .map_err(database_error)?;
        }
        if fail_before_commit {
            return Err(PeerReenrollmentRuntimeStoreError::InjectedBeforeCommit);
        }
        write.commit().map_err(database_error)?;
        let durable = load_transition(
            &self.database,
            self.owner_account_id,
            self.owner_device_id,
            key,
        )?
        .ok_or(PeerReenrollmentRuntimeStoreError::MissingPreparedTransition)?;
        Ok(PeerReenrollmentRuntimePrepareOutcome::Prepared(durable))
    }

    pub fn consume(
        &self,
        contact_id: [u8; 32],
        expected_transition_id: [u8; 32],
    ) -> Result<PeerReenrollmentRuntimeConsumeOutcome, PeerReenrollmentRuntimeStoreError> {
        let existing = load_transition(
            &self.database,
            self.owner_account_id,
            self.owner_device_id,
            contact_id,
        )?
        .ok_or(PeerReenrollmentRuntimeStoreError::MissingPreparedTransition)?;
        if existing.transition_id != expected_transition_id {
            return Err(PeerReenrollmentRuntimeStoreError::TransitionChanged);
        }
        if existing.consumed {
            return Ok(PeerReenrollmentRuntimeConsumeOutcome::AlreadyConsumed(
                existing,
            ));
        }

        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        let already_consumed;
        {
            let prepared = write.open_table(PREPARED_TABLE).map_err(database_error)?;
            let current_bytes = prepared
                .get(contact_id.as_slice())
                .map_err(database_error)?
                .ok_or(PeerReenrollmentRuntimeStoreError::MissingPreparedTransition)?
                .value()
                .to_vec();
            let current = SignedPeerReenrollmentRuntimeTransition::decode_and_verify(
                &current_bytes,
                self.owner_account_id,
                self.owner_device_id,
            )?;
            if current.transition_id()? != expected_transition_id {
                return Err(PeerReenrollmentRuntimeStoreError::TransitionChanged);
            }
            drop(prepared);
            let mut consumed = write.open_table(CONSUMED_TABLE).map_err(database_error)?;
            let current_head = consumed
                .get(contact_id.as_slice())
                .map_err(database_error)?
                .map(|head| copy_id(head.value()))
                .transpose()?;
            already_consumed = match current_head {
                Some(head) if head == expected_transition_id => true,
                Some(_) => return Err(PeerReenrollmentRuntimeStoreError::ConsumedHeadConflict),
                None => {
                    consumed
                        .insert(contact_id.as_slice(), expected_transition_id.as_slice())
                        .map_err(database_error)?;
                    false
                }
            };
        }
        if already_consumed {
            drop(write);
            let durable = load_transition(
                &self.database,
                self.owner_account_id,
                self.owner_device_id,
                contact_id,
            )?
            .ok_or(PeerReenrollmentRuntimeStoreError::MissingPreparedTransition)?;
            return Ok(PeerReenrollmentRuntimeConsumeOutcome::AlreadyConsumed(
                durable,
            ));
        }
        write.commit().map_err(database_error)?;
        let durable = load_transition(
            &self.database,
            self.owner_account_id,
            self.owner_device_id,
            contact_id,
        )?
        .ok_or(PeerReenrollmentRuntimeStoreError::MissingPreparedTransition)?;
        if !durable.consumed {
            return Err(PeerReenrollmentRuntimeStoreError::ConsumedHeadMissing);
        }
        Ok(PeerReenrollmentRuntimeConsumeOutcome::Consumed(durable))
    }

    pub fn load_all(
        &self,
    ) -> Result<Vec<DurablePeerReenrollmentRuntimeTransition>, PeerReenrollmentRuntimeStoreError>
    {
        load_all(&self.database, self.owner_account_id, self.owner_device_id)
    }
}

impl PeerReenrollmentRuntimeReadStore {
    pub fn open_existing(
        directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Result<Self, PeerReenrollmentRuntimeStoreError> {
        let directory = directory.as_ref().to_path_buf();
        let path = directory.join(DATABASE_FILE);
        if !path.is_file() {
            return Err(PeerReenrollmentRuntimeStoreError::StoreNotInitialized);
        }
        let database = ReadOnlyDatabase::open(path).map_err(database_error)?;
        verify_metadata(&database, owner_account_id, owner_device_id)?;
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

    pub fn load_all(
        &self,
    ) -> Result<Vec<DurablePeerReenrollmentRuntimeTransition>, PeerReenrollmentRuntimeStoreError>
    {
        load_all(&self.database, self.owner_account_id, self.owner_device_id)
    }
}

fn validate_binding(
    binding: &PeerReenrollmentRuntimeBinding,
) -> Result<(), PeerReenrollmentRuntimeStoreError> {
    if binding.contact_id == [0_u8; 32]
        || binding.conversation_id == [0_u8; 32]
        || binding.predecessor_contact.is_empty()
        || binding.successor_contact.is_empty()
        || binding.successor_endpoint_candidate.is_empty()
        || binding.predecessor_contact.len() > MAX_PEER_REENROLLMENT_RUNTIME_CONTACT_ARTIFACT_BYTES
        || binding.successor_contact.len() > MAX_PEER_REENROLLMENT_RUNTIME_CONTACT_ARTIFACT_BYTES
        || binding.successor_endpoint_candidate.len()
            > MAX_PEER_REENROLLMENT_RUNTIME_CONTACT_ARTIFACT_BYTES
    {
        return Err(PeerReenrollmentRuntimeStoreError::InvalidBinding);
    }
    Ok(())
}

fn validate_content(
    content: &PeerReenrollmentRuntimeTransitionContent,
) -> Result<(), PeerReenrollmentRuntimeStoreError> {
    if content.version != TRANSITION_VERSION
        || content.local_account_id == content.peer_account_id
        || content.acceptance_receipt_id == [0_u8; 32]
        || content.quarantine_record_id == [0_u8; 32]
        || content.contact_id == [0_u8; 32]
        || content.conversation_id == [0_u8; 32]
        || content.predecessor_contact_digest == [0_u8; 32]
        || content.successor_contact_digest == [0_u8; 32]
        || content.successor_endpoint_candidate_digest == [0_u8; 32]
        || content.quarantined_peer_device_id == content.replacement_peer_device_id
        || content.invitation_id == [0_u8; 32]
        || content.prepared_at_unix_seconds == 0
        || content.successor_contact.is_empty()
        || content.successor_contact.len() > MAX_PEER_REENROLLMENT_RUNTIME_CONTACT_ARTIFACT_BYTES
        || content.successor_endpoint_candidate.is_empty()
        || content.successor_endpoint_candidate.len()
            > MAX_PEER_REENROLLMENT_RUNTIME_CONTACT_ARTIFACT_BYTES
        || content.invitation.is_empty()
        || content.invitation.len() > MAX_CURRENT_PEER_INVITATION_BYTES
        || artifact_digest(SUCCESSOR_DIGEST_DOMAIN, &content.successor_contact)
            != content.successor_contact_digest
        || artifact_digest(
            CANDIDATE_DIGEST_DOMAIN,
            &content.successor_endpoint_candidate,
        ) != content.successor_endpoint_candidate_digest
        || !matches!(
            content.successor_floor,
            PeerSyncCapabilityFloor::CurrentInvitationRequired { .. }
        )
    {
        return Err(PeerReenrollmentRuntimeStoreError::InvalidTransition);
    }
    let invitation: SignedCurrentPeerInvitation = serde_json::from_slice(&content.invitation)?;
    invitation.verify()?;
    if serde_json::to_vec(&invitation)? != content.invitation
        || invitation.invitation_id()? != content.invitation_id
        || invitation.listener_account_id() != content.peer_account_id
        || invitation.listener_device_id() != content.replacement_peer_device_id
        || invitation.allowed_requester_account_id() != content.local_account_id
        || invitation.required_floor()? != content.successor_floor
    {
        return Err(PeerReenrollmentRuntimeStoreError::InvitationMismatch);
    }
    Ok(())
}

fn signing_bytes(
    content: &PeerReenrollmentRuntimeTransitionContent,
) -> Result<Vec<u8>, PeerReenrollmentRuntimeStoreError> {
    validate_content(content)?;
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn artifact_digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

fn decode_durable(
    bytes: &[u8],
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    consumed: bool,
) -> Result<DurablePeerReenrollmentRuntimeTransition, PeerReenrollmentRuntimeStoreError> {
    let transition = SignedPeerReenrollmentRuntimeTransition::decode_and_verify(
        bytes,
        owner_account_id,
        owner_device_id,
    )?;
    Ok(DurablePeerReenrollmentRuntimeTransition {
        transition_id: transition.transition_id()?,
        transition,
        consumed,
    })
}

fn load_transition<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    contact_id: [u8; 32],
) -> Result<Option<DurablePeerReenrollmentRuntimeTransition>, PeerReenrollmentRuntimeStoreError> {
    let read = database.begin_read().map_err(database_error)?;
    let prepared = read.open_table(PREPARED_TABLE).map_err(database_error)?;
    let Some(bytes) = prepared
        .get(contact_id.as_slice())
        .map_err(database_error)?
    else {
        return Ok(None);
    };
    let consumed_table = read.open_table(CONSUMED_TABLE).map_err(database_error)?;
    let consumed_id = consumed_table
        .get(contact_id.as_slice())
        .map_err(database_error)?
        .map(|value| copy_id(value.value()))
        .transpose()?;
    let durable = decode_durable(
        bytes.value(),
        owner_account_id,
        owner_device_id,
        consumed_id.is_some(),
    )?;
    if durable.transition.contact_id() != contact_id {
        return Err(PeerReenrollmentRuntimeStoreError::ContactKeyMismatch);
    }
    if consumed_id.is_some_and(|value| value != durable.transition_id) {
        return Err(PeerReenrollmentRuntimeStoreError::ConsumedHeadConflict);
    }
    Ok(Some(durable))
}

fn load_all<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<Vec<DurablePeerReenrollmentRuntimeTransition>, PeerReenrollmentRuntimeStoreError> {
    let read = database.begin_read().map_err(database_error)?;
    let prepared = read.open_table(PREPARED_TABLE).map_err(database_error)?;
    let mut contact_ids = Vec::new();
    for item in prepared.iter().map_err(database_error)? {
        let (key, _) = item.map_err(database_error)?;
        contact_ids.push(copy_id(key.value())?);
    }
    drop(prepared);
    drop(read);
    let mut transitions = contact_ids
        .into_iter()
        .map(|contact_id| {
            load_transition(database, owner_account_id, owner_device_id, contact_id)?
                .ok_or(PeerReenrollmentRuntimeStoreError::MissingPreparedTransition)
        })
        .collect::<Result<Vec<_>, _>>()?;
    transitions.sort_by_key(|value| value.transition.contact_id());
    Ok(transitions)
}

fn initialize_database(
    database: &Database,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), PeerReenrollmentRuntimeStoreError> {
    let mut write = database.begin_write().map_err(database_error)?;
    write
        .set_durability(Durability::Immediate)
        .map_err(database_error)?;
    {
        let mut meta = write.open_table(META_TABLE).map_err(database_error)?;
        initialize_meta(&mut meta, SCHEMA_KEY, &[SCHEMA_VERSION], || {
            PeerReenrollmentRuntimeStoreError::SchemaMismatch
        })?;
        initialize_meta(
            &mut meta,
            OWNER_ACCOUNT_KEY,
            owner_account_id.as_bytes(),
            || PeerReenrollmentRuntimeStoreError::OwnerAccountMismatch,
        )?;
        initialize_meta(
            &mut meta,
            OWNER_DEVICE_KEY,
            owner_device_id.as_bytes(),
            || PeerReenrollmentRuntimeStoreError::OwnerDeviceMismatch,
        )?;
    }
    write.open_table(PREPARED_TABLE).map_err(database_error)?;
    write.open_table(CONSUMED_TABLE).map_err(database_error)?;
    write.commit().map_err(database_error)
}

fn initialize_meta(
    table: &mut redb::Table<'_, &str, &[u8]>,
    key: &'static str,
    expected: &[u8],
    mismatch: impl Fn() -> PeerReenrollmentRuntimeStoreError,
) -> Result<(), PeerReenrollmentRuntimeStoreError> {
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

fn verify_metadata<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), PeerReenrollmentRuntimeStoreError> {
    let read = database.begin_read().map_err(database_error)?;
    let meta = read.open_table(META_TABLE).map_err(database_error)?;
    verify_meta(&meta, SCHEMA_KEY, &[SCHEMA_VERSION], || {
        PeerReenrollmentRuntimeStoreError::SchemaMismatch
    })?;
    verify_meta(
        &meta,
        OWNER_ACCOUNT_KEY,
        owner_account_id.as_bytes(),
        || PeerReenrollmentRuntimeStoreError::OwnerAccountMismatch,
    )?;
    verify_meta(&meta, OWNER_DEVICE_KEY, owner_device_id.as_bytes(), || {
        PeerReenrollmentRuntimeStoreError::OwnerDeviceMismatch
    })?;
    drop(meta);
    drop(read.open_table(PREPARED_TABLE).map_err(database_error)?);
    drop(read.open_table(CONSUMED_TABLE).map_err(database_error)?);
    Ok(())
}

fn verify_meta(
    table: &impl ReadableTable<&'static str, &'static [u8]>,
    key: &'static str,
    expected: &[u8],
    mismatch: impl Fn() -> PeerReenrollmentRuntimeStoreError,
) -> Result<(), PeerReenrollmentRuntimeStoreError> {
    let value = table
        .get(key)
        .map_err(database_error)?
        .ok_or_else(&mismatch)?;
    if value.value() != expected {
        return Err(mismatch());
    }
    Ok(())
}

fn verify_all<D: ReadableDatabase>(
    database: &D,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
) -> Result<(), PeerReenrollmentRuntimeStoreError> {
    verify_metadata(database, owner_account_id, owner_device_id)?;
    let read = database.begin_read().map_err(database_error)?;
    let prepared = read.open_table(PREPARED_TABLE).map_err(database_error)?;
    if prepared.len().map_err(database_error)? > MAX_PEER_REENROLLMENT_RUNTIME_TRANSITIONS {
        return Err(PeerReenrollmentRuntimeStoreError::TransitionCapacity);
    }
    let mut prepared_ids = HashSet::new();
    for item in prepared.iter().map_err(database_error)? {
        let (key, bytes) = item.map_err(database_error)?;
        let contact_id = copy_id(key.value())?;
        let transition = SignedPeerReenrollmentRuntimeTransition::decode_and_verify(
            bytes.value(),
            owner_account_id,
            owner_device_id,
        )?;
        if transition.contact_id() != contact_id {
            return Err(PeerReenrollmentRuntimeStoreError::ContactKeyMismatch);
        }
        prepared_ids.insert((contact_id, transition.transition_id()?));
    }
    drop(prepared);
    let consumed = read.open_table(CONSUMED_TABLE).map_err(database_error)?;
    if consumed.len().map_err(database_error)? > MAX_PEER_REENROLLMENT_RUNTIME_TRANSITIONS {
        return Err(PeerReenrollmentRuntimeStoreError::TransitionCapacity);
    }
    for item in consumed.iter().map_err(database_error)? {
        let (key, value) = item.map_err(database_error)?;
        let pair = (copy_id(key.value())?, copy_id(value.value())?);
        if !prepared_ids.contains(&pair) {
            return Err(PeerReenrollmentRuntimeStoreError::ConsumedHeadConflict);
        }
    }
    Ok(())
}

fn copy_id(bytes: &[u8]) -> Result<[u8; 32], PeerReenrollmentRuntimeStoreError> {
    bytes
        .try_into()
        .map_err(|_| PeerReenrollmentRuntimeStoreError::StoredIdLength(bytes.len()))
}

fn database_error(error: impl std::fmt::Display) -> PeerReenrollmentRuntimeStoreError {
    PeerReenrollmentRuntimeStoreError::Database(error.to_string())
}

#[derive(Debug, Error)]
pub enum PeerReenrollmentRuntimeStoreError {
    #[error("peer re-enrollment runtime store is not initialized")]
    StoreNotInitialized,
    #[error("peer re-enrollment runtime store I/O failed")]
    Io(#[from] std::io::Error),
    #[error("peer re-enrollment runtime database failed: {0}")]
    Database(String),
    #[error("peer re-enrollment runtime schema does not match")]
    SchemaMismatch,
    #[error("peer re-enrollment runtime owner Account does not match")]
    OwnerAccountMismatch,
    #[error("peer re-enrollment runtime owner Device does not match")]
    OwnerDeviceMismatch,
    #[error("peer re-enrollment runtime binding is invalid")]
    InvalidBinding,
    #[error("peer re-enrollment runtime transition is invalid")]
    InvalidTransition,
    #[error("peer re-enrollment invitation does not match the accepted successor")]
    InvitationMismatch,
    #[error("peer re-enrollment invitation is not canonically encoded")]
    NonCanonicalInvitation,
    #[error("peer re-enrollment invitation is {0} bytes and violates its bound")]
    InvitationLength(usize),
    #[error("peer re-enrollment runtime transition is {0} bytes and violates its bound")]
    TransitionLength(usize),
    #[error("peer re-enrollment runtime transition is not canonically encoded")]
    NonCanonicalTransition,
    #[error("a different peer successor is already prepared for this contact")]
    ContactAlreadyPrepared,
    #[error("peer re-enrollment runtime transition capacity is exhausted")]
    TransitionCapacity,
    #[error("peer re-enrollment prepared transition is missing")]
    MissingPreparedTransition,
    #[error("peer re-enrollment prepared transition changed before startup consumption")]
    TransitionChanged,
    #[error("peer re-enrollment consumed head conflicts with the prepared transition")]
    ConsumedHeadConflict,
    #[error("peer re-enrollment consumed head was not durable")]
    ConsumedHeadMissing,
    #[error("peer re-enrollment contact key does not match its signed transition")]
    ContactKeyMismatch,
    #[error("stored peer re-enrollment runtime ID has invalid length {0}")]
    StoredIdLength(usize),
    #[error("test-only failure injected before the atomic commit")]
    InjectedBeforeCommit,
    #[error("peer re-enrollment runtime encoding is invalid: {0}")]
    Codec(#[from] postcard::Error),
    #[error("peer re-enrollment runtime JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    SyncCapability(#[from] SyncTransportCapabilityError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use kilogram_identity::{
        AccountDeviceListSnapshot, AccountRootState, DeviceCapability, DeviceState,
    };
    use kilogram_peer_capability_store::{
        DurablePeerCapabilityQuarantineHead, PeerCapabilityKey, PeerCapabilityStore,
    };
    use kilogram_peer_reenrollment::{ExplicitPeerReenrollmentApproval, PeerReenrollmentPlan};
    use kilogram_peer_reenrollment_acceptance::{
        PeerReenrollmentAcceptanceOutcome, PeerReenrollmentAcceptanceStore,
    };
    use kilogram_sync_capability::{CurrentPeerInvitationBinding, PeerSyncCapabilityEvidence};
    use tempfile::TempDir;

    struct Fixture {
        root: TempDir,
        local_identity: DeviceIdentity,
        acceptance: DurablePeerReenrollmentAcceptance,
        invitation: SignedCurrentPeerInvitation,
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
        let old_key =
            PeerCapabilityKey::new(peer_root.account_id(), peer_old.identity().device_id());
        capability.apply_authenticated_observation(
            local_device.identity(),
            old_key,
            PeerSyncCapabilityEvidence::LegacyTicketWithoutExtension,
        )?;
        let quarantine = capability.load_quarantined_head(old_key)?;

        let peer_new = DeviceState::load_or_create(root.path().join("peer-new"))?;
        let new_certificate = peer_root.issue_device_certificate(
            peer_new.identity().device_id(),
            peer_new.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let peer_authority = peer_root.publish_device_list(&[old_certificate, new_certificate])?;
        let invitation = invitation(
            peer_new.identity(),
            peer_root.account_id(),
            local_root.account_id(),
            peer_authority.revision(),
            1,
            [9_u8; 32],
        )?;
        let acceptance = accept(
            root.path(),
            local_device.identity(),
            &local_authority,
            &quarantine,
            &peer_authority,
            &invitation,
        )?;
        Ok(Fixture {
            root,
            local_identity: DeviceIdentity::from_secret_bytes(
                local_device.identity().secret_bytes(),
            ),
            acceptance,
            invitation,
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

    fn accept(
        root: &Path,
        local_identity: &DeviceIdentity,
        local_authority: &AccountDeviceListSnapshot,
        quarantine: &DurablePeerCapabilityQuarantineHead,
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
        match store.accept(
            local_identity,
            &tombstone,
            local_authority,
            quarantine,
            peer_authority,
            invitation,
            1_300,
        )? {
            PeerReenrollmentAcceptanceOutcome::Accepted(value) => Ok(value),
            PeerReenrollmentAcceptanceOutcome::AlreadyAccepted(value) => Ok(value),
        }
    }

    fn binding() -> PeerReenrollmentRuntimeBinding {
        PeerReenrollmentRuntimeBinding {
            contact_id: [11_u8; 32],
            conversation_id: [12_u8; 32],
            predecessor_contact: vec![1, 2, 3],
            successor_contact: vec![4, 5, 6],
            successor_endpoint_candidate: vec![7, 8, 9],
        }
    }

    fn open_store(
        fixture: &Fixture,
    ) -> Result<PeerReenrollmentRuntimeStore, PeerReenrollmentRuntimeStoreError> {
        PeerReenrollmentRuntimeStore::open(
            fixture.root.path().join("runtime-transition"),
            fixture.acceptance.receipt().local_account_id(),
            fixture.local_identity.device_id(),
        )
    }

    #[test]
    fn prepare_consume_and_reopen_preserve_exact_transition()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let store = open_store(&fixture)?;
        let prepared = store.prepare(
            &fixture.local_identity,
            &fixture.acceptance,
            &fixture.invitation,
            binding(),
            1_400,
        )?;
        let PeerReenrollmentRuntimePrepareOutcome::Prepared(prepared) = prepared else {
            return Err(PeerReenrollmentRuntimeStoreError::ContactAlreadyPrepared.into());
        };
        assert!(!prepared.consumed());
        let consumed =
            store.consume(prepared.transition().contact_id(), prepared.transition_id())?;
        let PeerReenrollmentRuntimeConsumeOutcome::Consumed(consumed) = consumed else {
            return Err(PeerReenrollmentRuntimeStoreError::ConsumedHeadMissing.into());
        };
        assert!(consumed.consumed());
        assert_eq!(consumed.transition(), prepared.transition());
        drop(store);

        let reopened = PeerReenrollmentRuntimeReadStore::open_existing(
            fixture.root.path().join("runtime-transition"),
            fixture.acceptance.receipt().local_account_id(),
            fixture.local_identity.device_id(),
        )?;
        let values = reopened.load_all()?;
        assert_eq!(values.len(), 1);
        assert!(values[0].consumed());
        assert_eq!(values[0].transition_id(), consumed.transition_id());
        Ok(())
    }

    #[test]
    fn exact_prepare_and_consume_replay_are_idempotent() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let store = open_store(&fixture)?;
        let first = store.prepare(
            &fixture.local_identity,
            &fixture.acceptance,
            &fixture.invitation,
            binding(),
            1_400,
        )?;
        let first = match first {
            PeerReenrollmentRuntimePrepareOutcome::Prepared(value) => value,
            PeerReenrollmentRuntimePrepareOutcome::AlreadyPrepared(_) => {
                return Err(PeerReenrollmentRuntimeStoreError::ContactAlreadyPrepared.into());
            }
        };
        assert!(matches!(
            store.prepare(
                &fixture.local_identity,
                &fixture.acceptance,
                &fixture.invitation,
                binding(),
                1_400,
            )?,
            PeerReenrollmentRuntimePrepareOutcome::AlreadyPrepared(_)
        ));
        assert!(matches!(
            store.consume(first.transition().contact_id(), first.transition_id())?,
            PeerReenrollmentRuntimeConsumeOutcome::Consumed(_)
        ));
        assert!(matches!(
            store.consume(first.transition().contact_id(), first.transition_id())?,
            PeerReenrollmentRuntimeConsumeOutcome::AlreadyConsumed(_)
        ));
        Ok(())
    }

    #[test]
    fn changed_successor_for_same_contact_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let store = open_store(&fixture)?;
        store.prepare(
            &fixture.local_identity,
            &fixture.acceptance,
            &fixture.invitation,
            binding(),
            1_400,
        )?;
        let mut changed = binding();
        changed.successor_contact.push(42);
        assert!(matches!(
            store.prepare(
                &fixture.local_identity,
                &fixture.acceptance,
                &fixture.invitation,
                changed,
                1_400,
            ),
            Err(PeerReenrollmentRuntimeStoreError::ContactAlreadyPrepared)
        ));
        assert_eq!(store.load_all()?.len(), 1);
        Ok(())
    }

    #[test]
    fn pre_commit_failure_leaves_no_prepared_or_consumed_record()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let store = open_store(&fixture)?;
        assert!(matches!(
            store.prepare_inner(
                &fixture.local_identity,
                &fixture.acceptance,
                &fixture.invitation,
                binding(),
                1_400,
                true,
            ),
            Err(PeerReenrollmentRuntimeStoreError::InjectedBeforeCommit)
        ));
        assert!(store.load_all()?.is_empty());
        drop(store);
        assert!(open_store(&fixture)?.load_all()?.is_empty());
        Ok(())
    }

    #[test]
    fn wrong_transition_id_and_corrupt_record_are_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = fixture()?;
        let store = open_store(&fixture)?;
        let prepared = store.prepare(
            &fixture.local_identity,
            &fixture.acceptance,
            &fixture.invitation,
            binding(),
            1_400,
        )?;
        let prepared = match prepared {
            PeerReenrollmentRuntimePrepareOutcome::Prepared(value) => value,
            PeerReenrollmentRuntimePrepareOutcome::AlreadyPrepared(_) => {
                return Err(PeerReenrollmentRuntimeStoreError::ContactAlreadyPrepared.into());
            }
        };
        assert!(matches!(
            store.consume(prepared.transition().contact_id(), [99_u8; 32]),
            Err(PeerReenrollmentRuntimeStoreError::TransitionChanged)
        ));
        assert!(!store.load_all()?[0].consumed());

        let mut write = store.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut table = write.open_table(PREPARED_TABLE).map_err(database_error)?;
            table
                .insert(prepared.transition().contact_id().as_slice(), &[0_u8][..])
                .map_err(database_error)?;
        }
        write.commit().map_err(database_error)?;
        drop(store);
        assert!(
            PeerReenrollmentRuntimeReadStore::open_existing(
                fixture.root.path().join("runtime-transition"),
                fixture.acceptance.receipt().local_account_id(),
                fixture.local_identity.device_id(),
            )
            .is_err()
        );
        Ok(())
    }
}
