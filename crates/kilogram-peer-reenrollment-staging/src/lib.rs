//! Bounded, local-only staging for peer re-enrollment invitations.
//!
//! Import is an explicit offline action. Runtime readers can subsequently
//! inspect only a content-addressed reference and never receive a filesystem
//! path, connection ticket, endpoint address, or acceptance authority.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::fs::File;

use kilogram_identity::{AccountId, DeviceId, DeviceIdentity, IdentityError};
use kilogram_sync_capability::{
    MAX_CURRENT_PEER_INVITATION_BYTES, SignedCurrentPeerInvitation, SyncTransportCapabilityError,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const RECORD_VERSION: u8 = 1;
const SIGNATURE_DOMAIN: &[u8] = b"kilogram:peer-reenrollment-staging-record:v1\0";
const STAGE_ID_DOMAIN: &[u8] = b"kilogram:peer-reenrollment-staging-id:v1\0";
const FILE_EXTENSION: &str = "stage";

pub const MAX_PEER_REENROLLMENT_STAGING_RECORD_BYTES: usize =
    MAX_CURRENT_PEER_INVITATION_BYTES + 16 * 1024;
pub const MAX_PEER_REENROLLMENT_STAGING_RECORDS: usize = 4_096;

#[derive(Debug, Error)]
pub enum PeerReenrollmentStagingError {
    #[error("peer re-enrollment staging I/O failed")]
    Io(#[from] std::io::Error),
    #[error("peer re-enrollment staging codec failed")]
    Codec(#[from] postcard::Error),
    #[error("peer re-enrollment staging identity check failed")]
    Identity(#[from] IdentityError),
    #[error("peer re-enrollment invitation verification failed")]
    Invitation(#[from] SyncTransportCapabilityError),
    #[error("peer re-enrollment staging directory is not initialized")]
    StoreNotInitialized,
    #[error("peer re-enrollment staging path is not a plain local directory")]
    UnsafeDirectory,
    #[error("peer re-enrollment staging entry is not a regular file")]
    UnsafeEntry,
    #[error("peer re-enrollment staging contains an unexpected entry")]
    UnexpectedEntry,
    #[error("peer re-enrollment staging record has invalid length {0}")]
    RecordLength(usize),
    #[error("peer re-enrollment staging record is not canonical")]
    NonCanonicalRecord,
    #[error("peer re-enrollment staging record belongs to another Account")]
    OwnerAccountMismatch,
    #[error("peer re-enrollment staging record belongs to another Device")]
    OwnerDeviceMismatch,
    #[error("peer re-enrollment staging record has an invalid binding")]
    InvalidBinding,
    #[error("peer re-enrollment staging file name does not match its content")]
    StageIdMismatch,
    #[error("peer re-enrollment staging reached its bounded capacity")]
    Capacity,
    #[error("peer re-enrollment staging digest collision")]
    StageIdCollision,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct StagedPeerReenrollmentInvitationContent {
    version: u8,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    contact_id: [u8; 32],
    invitation_id: [u8; 32],
    invitation: Vec<u8>,
    staged_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct StagedPeerReenrollmentInvitationWire {
    content: StagedPeerReenrollmentInvitationContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedPeerReenrollmentInvitation {
    wire: StagedPeerReenrollmentInvitationWire,
}

impl StagedPeerReenrollmentInvitation {
    pub fn issue(
        owner_account_id: AccountId,
        owner: &DeviceIdentity,
        contact_id: [u8; 32],
        invitation: &SignedCurrentPeerInvitation,
        staged_at_unix_seconds: u64,
    ) -> Result<Self, PeerReenrollmentStagingError> {
        invitation.verify()?;
        let invitation_bytes = serde_json::to_vec(invitation)
            .map_err(|_| PeerReenrollmentStagingError::InvalidBinding)?;
        if invitation_bytes.is_empty() || invitation_bytes.len() > MAX_CURRENT_PEER_INVITATION_BYTES
        {
            return Err(PeerReenrollmentStagingError::RecordLength(
                invitation_bytes.len(),
            ));
        }
        let content = StagedPeerReenrollmentInvitationContent {
            version: RECORD_VERSION,
            owner_account_id,
            owner_device_id: owner.device_id(),
            contact_id,
            invitation_id: invitation.invitation_id()?,
            invitation: invitation_bytes,
            staged_at_unix_seconds,
        };
        validate_content(&content)?;
        let signature = owner.sign(&signing_bytes(&content)?).to_vec();
        let record = Self {
            wire: StagedPeerReenrollmentInvitationWire { content, signature },
        };
        record.verify(owner_account_id, owner.device_id())?;
        Ok(record)
    }

    pub fn decode_and_verify(
        encoded: &[u8],
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Result<Self, PeerReenrollmentStagingError> {
        if encoded.is_empty() || encoded.len() > MAX_PEER_REENROLLMENT_STAGING_RECORD_BYTES {
            return Err(PeerReenrollmentStagingError::RecordLength(encoded.len()));
        }
        let record = Self {
            wire: postcard::from_bytes(encoded)?,
        };
        record.verify(owner_account_id, owner_device_id)?;
        if record.encode()? != encoded {
            return Err(PeerReenrollmentStagingError::NonCanonicalRecord);
        }
        Ok(record)
    }

    pub fn encode(&self) -> Result<Vec<u8>, PeerReenrollmentStagingError> {
        validate_content(&self.wire.content)?;
        let encoded = postcard::to_allocvec(&self.wire)?;
        if encoded.is_empty() || encoded.len() > MAX_PEER_REENROLLMENT_STAGING_RECORD_BYTES {
            return Err(PeerReenrollmentStagingError::RecordLength(encoded.len()));
        }
        Ok(encoded)
    }

    pub fn stage_id(&self) -> Result<[u8; 32], PeerReenrollmentStagingError> {
        Ok(domain_digest(STAGE_ID_DOMAIN, &self.encode()?))
    }

    pub fn owner_account_id(&self) -> AccountId {
        self.wire.content.owner_account_id
    }

    pub fn owner_device_id(&self) -> DeviceId {
        self.wire.content.owner_device_id
    }

    pub fn contact_id(&self) -> [u8; 32] {
        self.wire.content.contact_id
    }

    pub fn invitation_id(&self) -> [u8; 32] {
        self.wire.content.invitation_id
    }

    pub fn staged_at_unix_seconds(&self) -> u64 {
        self.wire.content.staged_at_unix_seconds
    }

    pub fn invitation(&self) -> Result<SignedCurrentPeerInvitation, PeerReenrollmentStagingError> {
        let invitation: SignedCurrentPeerInvitation =
            serde_json::from_slice(&self.wire.content.invitation)
                .map_err(|_| PeerReenrollmentStagingError::InvalidBinding)?;
        invitation.verify()?;
        let canonical = serde_json::to_vec(&invitation)
            .map_err(|_| PeerReenrollmentStagingError::InvalidBinding)?;
        if canonical != self.wire.content.invitation
            || invitation.invitation_id()? != self.wire.content.invitation_id
        {
            return Err(PeerReenrollmentStagingError::NonCanonicalRecord);
        }
        Ok(invitation)
    }

    fn verify(
        &self,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Result<(), PeerReenrollmentStagingError> {
        validate_content(&self.wire.content)?;
        if self.owner_account_id() != owner_account_id {
            return Err(PeerReenrollmentStagingError::OwnerAccountMismatch);
        }
        if self.owner_device_id() != owner_device_id {
            return Err(PeerReenrollmentStagingError::OwnerDeviceMismatch);
        }
        owner_device_id.verify(&signing_bytes(&self.wire.content)?, &self.wire.signature)?;
        let invitation = self.invitation()?;
        if invitation.allowed_requester_account_id() != owner_account_id
            || invitation.listener_account_id() == owner_account_id
        {
            return Err(PeerReenrollmentStagingError::InvalidBinding);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerReenrollmentStagingOutcome {
    Inserted([u8; 32]),
    AlreadyPresent([u8; 32]),
}

pub struct PeerReenrollmentStagingStore {
    directory: PathBuf,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

impl PeerReenrollmentStagingStore {
    pub fn open(
        directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Result<Self, PeerReenrollmentStagingError> {
        let directory = directory.as_ref().to_path_buf();
        fs::create_dir_all(&directory)?;
        ensure_plain_directory(&directory)?;
        Ok(Self {
            directory,
            owner_account_id,
            owner_device_id,
        })
    }

    pub fn import(
        &self,
        record: &StagedPeerReenrollmentInvitation,
    ) -> Result<PeerReenrollmentStagingOutcome, PeerReenrollmentStagingError> {
        record.verify(self.owner_account_id, self.owner_device_id)?;
        let current = PeerReenrollmentStagingReadStore::open_existing(
            &self.directory,
            self.owner_account_id,
            self.owner_device_id,
        )?
        .load_all()?;
        if current.len() >= MAX_PEER_REENROLLMENT_STAGING_RECORDS {
            return Err(PeerReenrollmentStagingError::Capacity);
        }
        let encoded = record.encode()?;
        let stage_id = record.stage_id()?;
        let destination = self.directory.join(stage_file_name(stage_id));
        if destination.exists() {
            let existing = read_record_file(&destination)?;
            if existing == encoded {
                return Ok(PeerReenrollmentStagingOutcome::AlreadyPresent(stage_id));
            }
            return Err(PeerReenrollmentStagingError::StageIdCollision);
        }
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing = read_record_file(&destination)?;
                if existing == encoded {
                    return Ok(PeerReenrollmentStagingOutcome::AlreadyPresent(stage_id));
                }
                return Err(PeerReenrollmentStagingError::StageIdCollision);
            }
            Err(error) => return Err(error.into()),
        };
        if let Err(error) = file.write_all(&encoded).and_then(|_| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(&destination);
            return Err(error.into());
        }
        sync_directory(&self.directory)?;
        Ok(PeerReenrollmentStagingOutcome::Inserted(stage_id))
    }
}

pub struct PeerReenrollmentStagingReadStore {
    directory: PathBuf,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
}

impl PeerReenrollmentStagingReadStore {
    pub fn is_initialized(directory: impl AsRef<Path>) -> bool {
        directory.as_ref().is_dir()
    }

    pub fn open_existing(
        directory: impl AsRef<Path>,
        owner_account_id: AccountId,
        owner_device_id: DeviceId,
    ) -> Result<Self, PeerReenrollmentStagingError> {
        let directory = directory.as_ref().to_path_buf();
        if !directory.exists() {
            return Err(PeerReenrollmentStagingError::StoreNotInitialized);
        }
        ensure_plain_directory(&directory)?;
        Ok(Self {
            directory,
            owner_account_id,
            owner_device_id,
        })
    }

    pub fn load_all(
        &self,
    ) -> Result<Vec<StagedPeerReenrollmentInvitation>, PeerReenrollmentStagingError> {
        let mut paths = fs::read_dir(&self.directory)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?;
        paths.sort();
        if paths.len() > MAX_PEER_REENROLLMENT_STAGING_RECORDS {
            return Err(PeerReenrollmentStagingError::Capacity);
        }
        let mut records = Vec::with_capacity(paths.len());
        for path in paths {
            if path.extension().and_then(|value| value.to_str()) != Some(FILE_EXTENSION) {
                return Err(PeerReenrollmentStagingError::UnexpectedEntry);
            }
            let bytes = read_record_file(&path)?;
            let record = StagedPeerReenrollmentInvitation::decode_and_verify(
                &bytes,
                self.owner_account_id,
                self.owner_device_id,
            )?;
            if path.file_name().and_then(|value| value.to_str())
                != Some(stage_file_name(record.stage_id()?).as_str())
            {
                return Err(PeerReenrollmentStagingError::StageIdMismatch);
            }
            records.push(record);
        }
        Ok(records)
    }

    pub fn load(
        &self,
        stage_id: [u8; 32],
    ) -> Result<Option<StagedPeerReenrollmentInvitation>, PeerReenrollmentStagingError> {
        let path = self.directory.join(stage_file_name(stage_id));
        if !path.exists() {
            return Ok(None);
        }
        let bytes = read_record_file(&path)?;
        let record = StagedPeerReenrollmentInvitation::decode_and_verify(
            &bytes,
            self.owner_account_id,
            self.owner_device_id,
        )?;
        if record.stage_id()? != stage_id {
            return Err(PeerReenrollmentStagingError::StageIdMismatch);
        }
        Ok(Some(record))
    }
}

fn validate_content(
    content: &StagedPeerReenrollmentInvitationContent,
) -> Result<(), PeerReenrollmentStagingError> {
    if content.version != RECORD_VERSION
        || content.owner_account_id == AccountId::from_bytes([0_u8; 32])
        || content.contact_id == [0_u8; 32]
        || content.invitation_id == [0_u8; 32]
        || content.invitation.is_empty()
        || content.invitation.len() > MAX_CURRENT_PEER_INVITATION_BYTES
        || content.staged_at_unix_seconds == 0
    {
        return Err(PeerReenrollmentStagingError::InvalidBinding);
    }
    Ok(())
}

fn signing_bytes(
    content: &StagedPeerReenrollmentInvitationContent,
) -> Result<Vec<u8>, PeerReenrollmentStagingError> {
    validate_content(content)?;
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

fn stage_file_name(stage_id: [u8; 32]) -> String {
    let mut value = String::with_capacity(64 + 1 + FILE_EXTENSION.len());
    for byte in stage_id {
        use std::fmt::Write as _;
        let _ = write!(&mut value, "{byte:02x}");
    }
    value.push('.');
    value.push_str(FILE_EXTENSION);
    value
}

fn ensure_plain_directory(directory: &Path) -> Result<(), PeerReenrollmentStagingError> {
    let metadata = fs::symlink_metadata(directory)?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(PeerReenrollmentStagingError::UnsafeDirectory);
    }
    Ok(())
}

fn read_record_file(path: &Path) -> Result<Vec<u8>, PeerReenrollmentStagingError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(PeerReenrollmentStagingError::UnsafeEntry);
    }
    if metadata.len() == 0 || metadata.len() > MAX_PEER_REENROLLMENT_STAGING_RECORD_BYTES as u64 {
        return Err(PeerReenrollmentStagingError::RecordLength(
            usize::try_from(metadata.len()).unwrap_or(usize::MAX),
        ));
    }
    Ok(fs::read(path)?)
}

fn sync_directory(directory: &Path) -> Result<(), PeerReenrollmentStagingError> {
    #[cfg(unix)]
    File::open(directory)?.sync_all()?;
    #[cfg(windows)]
    {
        let _ = directory;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kilogram_sync_capability::CurrentPeerInvitationBinding;

    fn invitation(
        listener: &DeviceIdentity,
        owner_account_id: AccountId,
    ) -> Result<SignedCurrentPeerInvitation, SyncTransportCapabilityError> {
        SignedCurrentPeerInvitation::issue(
            listener,
            &CurrentPeerInvitationBinding {
                listener_account_id: AccountId::from_bytes([8_u8; 32]),
                listener_device_id: listener.device_id(),
                allowed_requester_account_id: owner_account_id,
                authority_revision: 2,
                endpoint_id: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                ticket_publication_channel_epoch: 3,
                issued_at_unix_seconds: 1_000,
                expires_at_unix_seconds: 2_000,
                connection_ticket: "Exact_current_ticket_01",
                required_capability_id: [9_u8; 32],
            },
        )
    }

    #[test]
    fn explicit_import_is_idempotent_and_read_only_reopen_does_not_create()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let directory = root.path().join("staging");
        let owner = DeviceIdentity::generate()?;
        let owner_account_id = AccountId::from_bytes([7_u8; 32]);
        let invitation = invitation(&DeviceIdentity::generate()?, owner_account_id)?;
        let record = StagedPeerReenrollmentInvitation::issue(
            owner_account_id,
            &owner,
            [6_u8; 32],
            &invitation,
            1_100,
        )?;

        assert!(matches!(
            PeerReenrollmentStagingReadStore::open_existing(
                &directory,
                owner_account_id,
                owner.device_id()
            ),
            Err(PeerReenrollmentStagingError::StoreNotInitialized)
        ));
        assert!(!directory.exists());

        let store =
            PeerReenrollmentStagingStore::open(&directory, owner_account_id, owner.device_id())?;
        let stage_id = match store.import(&record)? {
            PeerReenrollmentStagingOutcome::Inserted(value) => value,
            PeerReenrollmentStagingOutcome::AlreadyPresent(_) => {
                return Err("first import was unexpectedly idempotent".into());
            }
        };
        assert_eq!(
            store.import(&record)?,
            PeerReenrollmentStagingOutcome::AlreadyPresent(stage_id)
        );
        let read = PeerReenrollmentStagingReadStore::open_existing(
            &directory,
            owner_account_id,
            owner.device_id(),
        )?;
        assert_eq!(read.load_all()?, vec![record.clone()]);
        assert_eq!(read.load(stage_id)?, Some(record));
        Ok(())
    }

    #[test]
    fn wrong_owner_and_changed_bytes_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let owner = DeviceIdentity::generate()?;
        let owner_account_id = AccountId::from_bytes([7_u8; 32]);
        let record = StagedPeerReenrollmentInvitation::issue(
            owner_account_id,
            &owner,
            [6_u8; 32],
            &invitation(&DeviceIdentity::generate()?, owner_account_id)?,
            1_100,
        )?;
        let directory = root.path().join("staging");
        let store =
            PeerReenrollmentStagingStore::open(&directory, owner_account_id, owner.device_id())?;
        let stage_id = match store.import(&record)? {
            PeerReenrollmentStagingOutcome::Inserted(value)
            | PeerReenrollmentStagingOutcome::AlreadyPresent(value) => value,
        };

        let foreign = PeerReenrollmentStagingReadStore::open_existing(
            &directory,
            AccountId::from_bytes([5_u8; 32]),
            owner.device_id(),
        )?;
        assert!(matches!(
            foreign.load_all(),
            Err(PeerReenrollmentStagingError::OwnerAccountMismatch)
        ));

        let path = directory.join(stage_file_name(stage_id));
        let mut bytes = fs::read(&path)?;
        bytes[0] ^= 1;
        fs::write(&path, bytes)?;
        let read = PeerReenrollmentStagingReadStore::open_existing(
            &directory,
            owner_account_id,
            owner.device_id(),
        )?;
        assert!(read.load_all().is_err());
        Ok(())
    }

    #[test]
    fn oversized_and_unexpected_entries_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let directory = root.path().join("staging");
        fs::create_dir_all(&directory)?;
        fs::write(directory.join("unexpected.txt"), b"x")?;
        let read = PeerReenrollmentStagingReadStore::open_existing(
            &directory,
            AccountId::from_bytes([7_u8; 32]),
            DeviceIdentity::generate()?.device_id(),
        )?;
        assert!(matches!(
            read.load_all(),
            Err(PeerReenrollmentStagingError::UnexpectedEntry)
        ));
        fs::remove_file(directory.join("unexpected.txt"))?;
        fs::write(
            directory.join(format!("{}.stage", "01".repeat(32))),
            vec![0_u8; MAX_PEER_REENROLLMENT_STAGING_RECORD_BYTES + 1],
        )?;
        assert!(matches!(
            read.load_all(),
            Err(PeerReenrollmentStagingError::RecordLength(_))
        ));
        Ok(())
    }
}
