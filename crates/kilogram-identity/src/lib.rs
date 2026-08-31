use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    num::ParseIntError,
    path::{Path, PathBuf},
    str::FromStr,
};

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod account;

pub use account::{
    AccountAuthoritySnapshot, AccountId, AccountRootState, AuthoritySnapshotStoreOutcome,
    AuthorizedDevice, ConversationMembershipSnapshot, ConversationMembershipStoreOutcome,
    ConversationScopeId, DeviceCapability, DeviceCertificate, DeviceRevocation,
    verify_device_authorization, verify_device_authorization_with_snapshot,
};

const DEVICE_SECRET_FILE: &str = "device-secret.key";
const NEXT_SEQUENCE_FILE: &str = "next-sequence";
const SECRET_KEY_BYTES: usize = 32;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct DeviceId([u8; SECRET_KEY_BYTES]);

impl DeviceId {
    pub fn from_bytes(bytes: [u8; SECRET_KEY_BYTES]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; SECRET_KEY_BYTES] {
        &self.0
    }

    pub fn verify(&self, message: &[u8], signature: &[u8]) -> Result<(), IdentityError> {
        let verifying_key =
            VerifyingKey::from_bytes(&self.0).map_err(IdentityError::InvalidDevicePublicKey)?;
        let signature =
            Signature::try_from(signature).map_err(IdentityError::InvalidSignatureEncoding)?;
        verifying_key
            .verify_strict(message, &signature)
            .map_err(IdentityError::InvalidSignature)
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for DeviceId {
    type Err = IdentityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != SECRET_KEY_BYTES * 2 {
            return Err(IdentityError::InvalidDeviceIdLength(value.len()));
        }
        let mut bytes = [0_u8; SECRET_KEY_BYTES];
        let encoded = value.as_bytes();
        for (index, byte) in bytes.iter_mut().enumerate() {
            let offset = index * 2;
            let high = decode_hex_nibble(encoded[offset], offset)?;
            let low = decode_hex_nibble(encoded[offset + 1], offset + 1)?;
            *byte = (high << 4) | low;
        }
        Ok(Self(bytes))
    }
}

fn decode_hex_nibble(byte: u8, index: usize) -> Result<u8, IdentityError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(IdentityError::InvalidDeviceIdHex(index)),
    }
}

pub struct DeviceIdentity {
    signing_key: SigningKey,
}

impl DeviceIdentity {
    pub fn generate() -> Result<Self, IdentityError> {
        let mut secret = [0_u8; SECRET_KEY_BYTES];
        getrandom::fill(&mut secret).map_err(IdentityError::SecureRandom)?;
        Ok(Self::from_secret_bytes(secret))
    }

    pub fn from_secret_bytes(secret: [u8; SECRET_KEY_BYTES]) -> Self {
        Self {
            signing_key: SigningKey::from_bytes(&secret),
        }
    }

    pub fn device_id(&self) -> DeviceId {
        DeviceId(self.signing_key.verifying_key().to_bytes())
    }

    pub fn secret_bytes(&self) -> [u8; SECRET_KEY_BYTES] {
        self.signing_key.to_bytes()
    }

    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        self.signing_key.sign(message).to_bytes()
    }
}

pub struct DeviceState {
    directory: PathBuf,
    identity: DeviceIdentity,
}

impl DeviceState {
    pub fn load_or_create(directory: impl AsRef<Path>) -> Result<Self, IdentityError> {
        let directory = directory.as_ref().to_path_buf();
        fs::create_dir_all(&directory)?;
        let secret_path = directory.join(DEVICE_SECRET_FILE);
        let identity = load_or_create_identity(&secret_path)?;

        Ok(Self {
            directory,
            identity,
        })
    }

    pub fn identity(&self) -> &DeviceIdentity {
        &self.identity
    }

    pub fn allocate_sequence(&self) -> Result<u64, IdentityError> {
        let sequence_path = self.directory.join(NEXT_SEQUENCE_FILE);
        let current: u64 = match fs::read_to_string(&sequence_path) {
            Ok(value) => value
                .trim()
                .parse()
                .map_err(IdentityError::InvalidSequence)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
            Err(error) => return Err(error.into()),
        };
        let next = current
            .checked_add(1)
            .ok_or(IdentityError::SequenceExhausted)?;

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(sequence_path)?;
        writeln!(file, "{next}")?;
        file.sync_all()?;
        Ok(current)
    }
}

fn load_or_create_identity(path: &Path) -> Result<DeviceIdentity, IdentityError> {
    match open_new_secret_file(path) {
        Ok(mut file) => {
            let identity = DeviceIdentity::generate()?;
            file.write_all(&identity.secret_bytes())?;
            file.sync_all()?;
            Ok(identity)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => load_identity(path),
        Err(error) => Err(error.into()),
    }
}

fn open_new_secret_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }

    options.open(path)
}

fn load_identity(path: &Path) -> Result<DeviceIdentity, IdentityError> {
    let bytes = fs::read(path)?;
    let secret: [u8; SECRET_KEY_BYTES] = bytes
        .try_into()
        .map_err(|bytes: Vec<u8>| IdentityError::InvalidSecretKeyLength(bytes.len()))?;
    Ok(DeviceIdentity::from_secret_bytes(secret))
}

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("device state I/O failed")]
    Io(#[from] io::Error),

    #[error("secure random generation failed: {0}")]
    SecureRandom(getrandom::Error),

    #[error("device secret key has {0} bytes; expected {SECRET_KEY_BYTES}")]
    InvalidSecretKeyLength(usize),

    #[error("device ID has {0} hexadecimal characters; expected 64")]
    InvalidDeviceIdLength(usize),

    #[error("device ID contains invalid hexadecimal at character {0}")]
    InvalidDeviceIdHex(usize),

    #[error("device public key is invalid")]
    InvalidDevicePublicKey(#[source] ed25519_dalek::SignatureError),

    #[error("signature encoding is invalid")]
    InvalidSignatureEncoding(#[source] ed25519_dalek::SignatureError),

    #[error("signature verification failed")]
    InvalidSignature(#[source] ed25519_dalek::SignatureError),

    #[error("stored device sequence is invalid")]
    InvalidSequence(#[source] ParseIntError),

    #[error("device sequence is exhausted")]
    SequenceExhausted,

    #[error("account authority encoding is invalid")]
    AuthorityEncoding(#[from] postcard::Error),

    #[error("account root already exists at {0}")]
    AccountRootAlreadyExists(PathBuf),

    #[error("account root does not exist at {0}")]
    AccountRootMissing(PathBuf),

    #[error("account root secret key has {0} bytes; expected {SECRET_KEY_BYTES}")]
    InvalidAccountRootSecretKeyLength(usize),

    #[error("account ID has {0} hexadecimal characters; expected 64")]
    InvalidAccountIdLength(usize),

    #[error("account ID contains invalid hexadecimal at character {0}")]
    InvalidAccountIdHex(usize),

    #[error("account public key is invalid")]
    InvalidAccountPublicKey(#[source] ed25519_dalek::SignatureError),

    #[error("unsupported account authority version: {0}")]
    UnsupportedAccountAuthorityVersion(u8),

    #[error("a device certificate must contain at least one capability")]
    EmptyDeviceCapabilities,

    #[error("a device certificate contains duplicate capabilities")]
    DuplicateDeviceCapability,

    #[error("device certificate capabilities are not in canonical order")]
    NonCanonicalDeviceCapabilities,

    #[error("authority object belongs to account {actual}; expected {expected}")]
    AccountMismatch {
        expected: AccountId,
        actual: AccountId,
    },

    #[error("device certificate belongs to device {actual}; expected {expected}")]
    DeviceCertificateDeviceMismatch {
        expected: DeviceId,
        actual: DeviceId,
    },

    #[error("device certificate is missing required capability {0:?}")]
    MissingDeviceCapability(DeviceCapability),

    #[error("device {0} has been permanently revoked by its account root")]
    DeviceRevoked(DeviceId),

    #[error("device certificate is not installed")]
    DeviceCertificateMissing,

    #[error("a different device certificate is already installed")]
    DeviceCertificateAlreadyInstalled,

    #[error("account authority sequence is exhausted")]
    AuthoritySequenceExhausted,

    #[error(
        "legacy Account Root state has authority operations but no complete revocation log; create a fresh M0.6 root or migrate it explicitly"
    )]
    LegacyAuthorityState,

    #[error("unsupported account authority log version: {0}")]
    UnsupportedAuthorityLogVersion(String),

    #[error("device {0} is already permanently revoked")]
    DeviceAlreadyRevoked(DeviceId),

    #[error("authority snapshot contains revocations in non-canonical order")]
    NonCanonicalAuthoritySnapshot,

    #[error("authority snapshot contains duplicate revocations for device {0}")]
    DuplicateDeviceRevocation(DeviceId),

    #[error(
        "revocation sequence {revocation_sequence} is not covered by authority snapshot revision {snapshot_revision}"
    )]
    RevocationOutsideSnapshot {
        revocation_sequence: u64,
        snapshot_revision: u64,
    },

    #[error(
        "device certificate sequence {certificate_sequence} is not covered by authority snapshot revision {snapshot_revision}"
    )]
    CertificateOutsideSnapshot {
        certificate_sequence: u64,
        snapshot_revision: u64,
    },

    #[error("account authority snapshot is not installed for this device")]
    AccountAuthoritySnapshotMissing,

    #[error(
        "authority snapshot rollback detected for account {account_id}: stored revision {stored_revision}, received revision {received_revision}"
    )]
    AuthoritySnapshotRollback {
        account_id: AccountId,
        stored_revision: u64,
        received_revision: u64,
    },

    #[error(
        "conflicting authority snapshots have the same revision {revision} for account {account_id}"
    )]
    AuthoritySnapshotEquivocation {
        account_id: AccountId,
        revision: u64,
    },

    #[error("conversation membership already exists for {0}")]
    ConversationMembershipAlreadyExists(ConversationScopeId),

    #[error("conversation membership is missing for {0}")]
    ConversationMembershipMissing(ConversationScopeId),

    #[error("conversation membership owner is {actual}; expected trusted owner {expected}")]
    ConversationMembershipOwnerMismatch {
        expected: AccountId,
        actual: AccountId,
    },

    #[error("conversation membership does not contain account {0}")]
    AccountNotConversationMember(AccountId),

    #[error("conversation membership contains duplicate account {0}")]
    DuplicateConversationMember(AccountId),

    #[error("conversation membership members are not in canonical order")]
    NonCanonicalConversationMembers,

    #[error("conversation membership revision is exhausted")]
    ConversationMembershipRevisionExhausted,

    #[error("conversation membership revision must be greater than zero")]
    InvalidConversationMembershipRevision,

    #[error(
        "conversation membership rollback detected for {conversation_id}: stored revision {stored_revision}, received revision {received_revision}"
    )]
    ConversationMembershipRollback {
        conversation_id: ConversationScopeId,
        stored_revision: u64,
        received_revision: u64,
    },

    #[error("conflicting conversation memberships have revision {revision} for {conversation_id}")]
    ConversationMembershipEquivocation {
        conversation_id: ConversationScopeId,
        revision: u64,
    },

    #[error("conversation membership update for {0} is not add-only")]
    ConversationMembershipNotAddOnly(ConversationScopeId),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn identity_persists_across_reloads() -> Result<(), IdentityError> {
        let directory = tempdir()?;
        let first = DeviceState::load_or_create(directory.path())?;
        let first_id = first.identity().device_id();
        drop(first);

        let second = DeviceState::load_or_create(directory.path())?;
        assert_eq!(second.identity().device_id(), first_id);
        Ok(())
    }

    #[test]
    fn sequence_survives_reloads() -> Result<(), IdentityError> {
        let directory = tempdir()?;
        let first = DeviceState::load_or_create(directory.path())?;
        assert_eq!(first.allocate_sequence()?, 0);
        assert_eq!(first.allocate_sequence()?, 1);
        drop(first);

        let second = DeviceState::load_or_create(directory.path())?;
        assert_eq!(second.allocate_sequence()?, 2);
        Ok(())
    }

    #[test]
    fn a_signature_is_bound_to_its_device_and_message() -> Result<(), IdentityError> {
        let signer = DeviceIdentity::generate()?;
        let other = DeviceIdentity::generate()?;
        let signature = signer.sign(b"message");

        signer.device_id().verify(b"message", &signature)?;
        assert!(signer.device_id().verify(b"tampered", &signature).is_err());
        assert!(other.device_id().verify(b"message", &signature).is_err());
        Ok(())
    }

    #[test]
    fn device_id_text_round_trips() -> Result<(), IdentityError> {
        let device_id = DeviceIdentity::generate()?.device_id();
        assert_eq!(device_id.to_string().parse::<DeviceId>()?, device_id);
        assert!("not-a-device-id".parse::<DeviceId>().is_err());
        Ok(())
    }
}
