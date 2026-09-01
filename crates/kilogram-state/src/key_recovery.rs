use std::{fs, path::Path};

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, Payload},
};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::{
    StateError, VaultKeyProtection, VaultReport, io_at,
    key_provider::{VAULT_KEY_BYTES, VaultMasterKey},
    reject_symlink,
};

pub(crate) const VAULT_RECOVERY_MAGIC: &[u8; 16] = b"KILOGRAM-VRECOV1";

const RECOVERY_VERSION: u8 = 1;
const RECOVERY_PLAINTEXT_VERSION: u8 = 1;
const ARGON2ID_ALGORITHM: u8 = 1;
const ARGON2_VERSION_13: u8 = 0x13;
const ARGON2_MEMORY_KIB: u32 = 64 * 1024;
const ARGON2_ITERATIONS: u32 = 3;
const ARGON2_PARALLELISM: u32 = 1;
const RECOVERY_SALT_BYTES: usize = 16;
const RECOVERY_NONCE_BYTES: usize = 24;
const RECOVERY_KEY_BYTES: usize = 32;
pub(crate) const MAX_RECOVERY_PACKAGE_BYTES: usize = 64 * 1024;
pub(crate) const MIN_RECOVERY_PASSPHRASE_BYTES: usize = 16;
pub(crate) const MAX_RECOVERY_PASSPHRASE_BYTES: usize = 4096;
const RECOVERY_AAD_DOMAIN: &[u8] = b"kilogram:vault-key-recovery-aad:v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultRecoveryWitness {
    schema_version: u64,
    mirror_generation: u64,
    snapshot_id: [u8; 32],
}

impl VaultRecoveryWitness {
    pub fn schema_version(&self) -> u64 {
        self.schema_version
    }

    pub fn mirror_generation(&self) -> u64 {
        self.mirror_generation
    }

    pub fn snapshot_id(&self) -> &[u8; 32] {
        &self.snapshot_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultKeyRecoveryExport {
    witness: VaultRecoveryWitness,
}

impl VaultKeyRecoveryExport {
    pub fn witness(&self) -> &VaultRecoveryWitness {
        &self.witness
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultKeyRecoveryImport {
    witness: VaultRecoveryWitness,
    current: VaultReport,
    key_protection: VaultKeyProtection,
}

impl VaultKeyRecoveryImport {
    pub fn witness(&self) -> &VaultRecoveryWitness {
        &self.witness
    }

    pub fn current(&self) -> &VaultReport {
        &self.current
    }

    pub fn key_protection(&self) -> VaultKeyProtection {
        self.key_protection
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct RecoveryHeader {
    version: u8,
    algorithm: u8,
    argon2_version: u8,
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
    salt: [u8; RECOVERY_SALT_BYTES],
    nonce: [u8; RECOVERY_NONCE_BYTES],
}

#[derive(Debug, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
struct RecoveryEnvelope {
    #[zeroize(skip)]
    header: RecoveryHeader,
    ciphertext: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
struct RecoveryPlaintext {
    #[zeroize(skip)]
    version: u8,
    master_key: [u8; VAULT_KEY_BYTES],
    #[zeroize(skip)]
    schema_version: u64,
    #[zeroize(skip)]
    mirror_generation: u64,
    #[zeroize(skip)]
    snapshot_id: [u8; 32],
}

pub(crate) struct OpenedRecovery {
    pub(crate) master_key: VaultMasterKey,
    pub(crate) witness: VaultRecoveryWitness,
}

pub(crate) fn seal_recovery(
    master_key: &VaultMasterKey,
    report: &VaultReport,
    passphrase: &[u8],
) -> Result<(Vec<u8>, VaultKeyRecoveryExport), StateError> {
    validate_passphrase(passphrase)?;
    let mut salt = [0_u8; RECOVERY_SALT_BYTES];
    let mut nonce = [0_u8; RECOVERY_NONCE_BYTES];
    getrandom::fill(&mut salt).map_err(StateError::VaultSecureRandom)?;
    getrandom::fill(&mut nonce).map_err(StateError::VaultSecureRandom)?;
    let header = production_header(salt, nonce);
    let plaintext = RecoveryPlaintext {
        version: RECOVERY_PLAINTEXT_VERSION,
        master_key: master_key.0,
        schema_version: report.schema_version(),
        mirror_generation: report.mirror_generation(),
        snapshot_id: *report.snapshot_id(),
    };
    let encoded_plaintext = Zeroizing::new(postcard::to_allocvec(&plaintext)?);
    let recovery_key = Zeroizing::new(derive_recovery_key(passphrase, &header)?);
    let cipher = XChaCha20Poly1305::new((&*recovery_key).into());
    let aad = recovery_aad(&header)?;
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&header.nonce),
            Payload {
                msg: encoded_plaintext.as_slice(),
                aad: &aad,
            },
        )
        .map_err(|_| StateError::VaultRecoveryAuthenticationFailed)?;
    let envelope = RecoveryEnvelope { header, ciphertext };
    let payload = postcard::to_allocvec(&envelope)?;
    let total_len = VAULT_RECOVERY_MAGIC.len() + payload.len();
    if total_len > MAX_RECOVERY_PACKAGE_BYTES {
        return Err(StateError::VaultRecoveryPackageTooLarge(total_len));
    }
    let mut encoded = Vec::with_capacity(total_len);
    encoded.extend_from_slice(VAULT_RECOVERY_MAGIC);
    encoded.extend_from_slice(&payload);
    Ok((
        encoded,
        VaultKeyRecoveryExport {
            witness: witness_from_report(report),
        },
    ))
}

pub(crate) fn open_recovery(path: &Path, passphrase: &[u8]) -> Result<OpenedRecovery, StateError> {
    validate_passphrase(passphrase)?;
    reject_symlink(path)?;
    let metadata = io_at(path, fs::metadata(path))?;
    if metadata.len() > MAX_RECOVERY_PACKAGE_BYTES as u64 {
        return Err(StateError::VaultRecoveryPackageTooLarge(
            metadata.len() as usize
        ));
    }
    let mut encoded = io_at(path, fs::read(path))?;
    if encoded.len() > MAX_RECOVERY_PACKAGE_BYTES {
        return Err(StateError::VaultRecoveryPackageTooLarge(encoded.len()));
    }
    let payload = encoded
        .strip_prefix(VAULT_RECOVERY_MAGIC)
        .ok_or_else(|| invalid_package(path, "missing Kilogram recovery-package magic"))?;
    let mut envelope: RecoveryEnvelope =
        postcard::from_bytes(payload).map_err(|error| invalid_package(path, &error.to_string()))?;
    validate_header(&envelope.header)?;
    let recovery_key = Zeroizing::new(derive_recovery_key(passphrase, &envelope.header)?);
    let cipher = XChaCha20Poly1305::new((&*recovery_key).into());
    let aad = recovery_aad(&envelope.header)?;
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                XNonce::from_slice(&envelope.header.nonce),
                Payload {
                    msg: &envelope.ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| StateError::VaultRecoveryAuthenticationFailed)?,
    );
    envelope.zeroize();
    encoded.zeroize();
    let decoded: RecoveryPlaintext = postcard::from_bytes(plaintext.as_slice())
        .map_err(|error| invalid_package(path, &error.to_string()))?;
    if decoded.version != RECOVERY_PLAINTEXT_VERSION {
        return Err(StateError::UnsupportedVaultRecoveryVersion(decoded.version));
    }
    if decoded.mirror_generation == 0 {
        return Err(StateError::InvalidVaultGeneration(0));
    }
    let master_key = VaultMasterKey(decoded.master_key);
    let witness = VaultRecoveryWitness {
        schema_version: decoded.schema_version,
        mirror_generation: decoded.mirror_generation,
        snapshot_id: decoded.snapshot_id,
    };
    Ok(OpenedRecovery {
        master_key,
        witness,
    })
}

pub(crate) fn completed_import(
    witness: VaultRecoveryWitness,
    current: VaultReport,
    key_protection: VaultKeyProtection,
) -> VaultKeyRecoveryImport {
    VaultKeyRecoveryImport {
        witness,
        current,
        key_protection,
    }
}

fn production_header(
    salt: [u8; RECOVERY_SALT_BYTES],
    nonce: [u8; RECOVERY_NONCE_BYTES],
) -> RecoveryHeader {
    RecoveryHeader {
        version: RECOVERY_VERSION,
        algorithm: ARGON2ID_ALGORITHM,
        argon2_version: ARGON2_VERSION_13,
        memory_kib: ARGON2_MEMORY_KIB,
        iterations: ARGON2_ITERATIONS,
        parallelism: ARGON2_PARALLELISM,
        salt,
        nonce,
    }
}

fn validate_header(header: &RecoveryHeader) -> Result<(), StateError> {
    if header.version != RECOVERY_VERSION {
        return Err(StateError::UnsupportedVaultRecoveryVersion(header.version));
    }
    if header.algorithm != ARGON2ID_ALGORITHM
        || header.argon2_version != ARGON2_VERSION_13
        || header.memory_kib != ARGON2_MEMORY_KIB
        || header.iterations != ARGON2_ITERATIONS
        || header.parallelism != ARGON2_PARALLELISM
    {
        return Err(StateError::UnsupportedVaultRecoveryKdf);
    }
    Ok(())
}

fn derive_recovery_key(
    passphrase: &[u8],
    header: &RecoveryHeader,
) -> Result<[u8; RECOVERY_KEY_BYTES], StateError> {
    let params = Params::new(
        header.memory_kib,
        header.iterations,
        header.parallelism,
        Some(RECOVERY_KEY_BYTES),
    )
    .map_err(|_| StateError::UnsupportedVaultRecoveryKdf)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new([0_u8; RECOVERY_KEY_BYTES]);
    argon2
        .hash_password_into(passphrase, &header.salt, &mut *key)
        .map_err(|_| StateError::UnsupportedVaultRecoveryKdf)?;
    Ok(*key)
}

fn recovery_aad(header: &RecoveryHeader) -> Result<Vec<u8>, StateError> {
    let encoded = postcard::to_allocvec(header)?;
    let mut aad = Vec::with_capacity(RECOVERY_AAD_DOMAIN.len() + encoded.len());
    aad.extend_from_slice(RECOVERY_AAD_DOMAIN);
    aad.extend_from_slice(&encoded);
    Ok(aad)
}

fn validate_passphrase(passphrase: &[u8]) -> Result<(), StateError> {
    if passphrase.len() < MIN_RECOVERY_PASSPHRASE_BYTES {
        return Err(StateError::VaultRecoveryPassphraseTooShort(
            passphrase.len(),
        ));
    }
    if passphrase.len() > MAX_RECOVERY_PASSPHRASE_BYTES {
        return Err(StateError::VaultRecoveryPassphraseTooLong(passphrase.len()));
    }
    Ok(())
}

fn witness_from_report(report: &VaultReport) -> VaultRecoveryWitness {
    VaultRecoveryWitness {
        schema_version: report.schema_version(),
        mirror_generation: report.mirror_generation(),
        snapshot_id: *report.snapshot_id(),
    }
}

fn invalid_package(path: &Path, detail: &str) -> StateError {
    StateError::InvalidVaultRecoveryPackage {
        path: path.to_path_buf(),
        detail: detail.to_owned(),
    }
}
