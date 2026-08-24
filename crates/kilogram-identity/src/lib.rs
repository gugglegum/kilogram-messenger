use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    num::ParseIntError,
    path::{Path, PathBuf},
};

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
}
