use std::{
    fs,
    io::{self, Write},
    path::Path,
};

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::{StateError, io_at, path_exists, reject_symlink, sync_directory};

pub(crate) const VAULT_KEY_BYTES: usize = 32;
pub(crate) const VAULT_KEY_ENVELOPE_MAGIC: &[u8; 16] = b"KILOGRAM-VAULTK1";

const VAULT_KEY_ENVELOPE_VERSION: u8 = 1;
const MAX_VAULT_KEY_ENVELOPE_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VaultKeyProtection {
    WindowsDpapiCurrentUser,
    PlaintextDevelopment,
}

impl VaultKeyProtection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WindowsDpapiCurrentUser => "windows-dpapi-current-user",
            Self::PlaintextDevelopment => "plaintext-development",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VaultKeyLoadOutcome {
    Created,
    LegacyMigrated,
    AlreadyCurrent,
}

impl VaultKeyLoadOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::LegacyMigrated => "legacy-migrated",
            Self::AlreadyCurrent => "already-current",
        }
    }
}

#[derive(ZeroizeOnDrop)]
pub(crate) struct VaultMasterKey(pub(crate) [u8; VAULT_KEY_BYTES]);

pub(crate) struct LoadedVaultMasterKey {
    pub(crate) master_key: VaultMasterKey,
    pub(crate) protection: VaultKeyProtection,
    pub(crate) load_outcome: VaultKeyLoadOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum VaultKeyProviderId {
    WindowsDpapiCurrentUser,
    PlaintextDevelopment,
}

impl VaultKeyProviderId {
    fn as_str(self) -> &'static str {
        match self {
            Self::WindowsDpapiCurrentUser => "windows-dpapi-current-user",
            Self::PlaintextDevelopment => "plaintext-development",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ZeroizeOnDrop)]
struct VaultKeyEnvelope {
    #[zeroize(skip)]
    version: u8,
    #[zeroize(skip)]
    provider: VaultKeyProviderId,
    protected_key: Vec<u8>,
}

pub(crate) fn load_or_create_master_key(path: &Path) -> Result<LoadedVaultMasterKey, StateError> {
    if path_exists(path)? {
        reject_symlink(path)?;
        return load_master_key(path);
    }

    let mut key = [0_u8; VAULT_KEY_BYTES];
    getrandom::fill(&mut key).map_err(StateError::VaultSecureRandom)?;
    let master_key = VaultMasterKey(key);
    let (encoded, protection) = encode_platform_envelope(&master_key.0)?;
    match write_new_key_file(path, &encoded) {
        Ok(()) => Ok(LoadedVaultMasterKey {
            master_key,
            protection,
            load_outcome: VaultKeyLoadOutcome::Created,
        }),
        Err(StateError::Io { source, .. }) if source.kind() == io::ErrorKind::AlreadyExists => {
            reject_symlink(path)?;
            load_master_key(path)
        }
        Err(error) => Err(error),
    }
}

pub(crate) fn load_master_key(path: &Path) -> Result<LoadedVaultMasterKey, StateError> {
    let mut bytes = io_at(path, fs::read(path))?;
    let result = if bytes.len() == VAULT_KEY_BYTES {
        migrate_legacy_key(path, &bytes)
    } else {
        decode_enveloped_key(path, &bytes)
    };
    bytes.zeroize();
    result
}

pub(crate) fn install_master_key(
    path: &Path,
    master_key: &VaultMasterKey,
) -> Result<LoadedVaultMasterKey, StateError> {
    let (encoded, protection) = encode_platform_envelope(&master_key.0)?;
    let validated = decode_enveloped_key(path, &encoded)?;
    if validated.master_key.0 != master_key.0 || validated.protection != protection {
        return Err(StateError::InvalidVaultKeyEnvelope {
            path: path.to_path_buf(),
            detail: "candidate key envelope did not round-trip before installation".to_owned(),
        });
    }
    if path_exists(path)? {
        replace_key_file(path, &encoded)?;
    } else {
        write_new_key_file(path, &encoded)?;
    }
    Ok(LoadedVaultMasterKey {
        master_key: VaultMasterKey(master_key.0),
        protection,
        load_outcome: VaultKeyLoadOutcome::AlreadyCurrent,
    })
}

fn migrate_legacy_key(path: &Path, bytes: &[u8]) -> Result<LoadedVaultMasterKey, StateError> {
    let master_key = master_key_from_slice(bytes)?;
    let (encoded, protection) = encode_platform_envelope(&master_key.0)?;
    replace_key_file(path, &encoded)?;
    Ok(LoadedVaultMasterKey {
        master_key,
        protection,
        load_outcome: VaultKeyLoadOutcome::LegacyMigrated,
    })
}

fn decode_enveloped_key(path: &Path, bytes: &[u8]) -> Result<LoadedVaultMasterKey, StateError> {
    if bytes.len() > MAX_VAULT_KEY_ENVELOPE_BYTES {
        return Err(StateError::VaultKeyEnvelopeTooLarge(bytes.len()));
    }
    let encoded = bytes
        .strip_prefix(VAULT_KEY_ENVELOPE_MAGIC)
        .ok_or_else(|| StateError::InvalidVaultKeyEnvelope {
            path: path.to_path_buf(),
            detail: "missing Kilogram key-envelope magic".to_owned(),
        })?;
    let envelope: VaultKeyEnvelope =
        postcard::from_bytes(encoded).map_err(|error| StateError::InvalidVaultKeyEnvelope {
            path: path.to_path_buf(),
            detail: error.to_string(),
        })?;
    if envelope.version != VAULT_KEY_ENVELOPE_VERSION {
        return Err(StateError::UnsupportedVaultKeyEnvelopeVersion(
            envelope.version,
        ));
    }
    open_envelope(path, envelope)
}

fn open_envelope(
    path: &Path,
    mut envelope: VaultKeyEnvelope,
) -> Result<LoadedVaultMasterKey, StateError> {
    match envelope.provider {
        VaultKeyProviderId::WindowsDpapiCurrentUser => {
            let plaintext = unprotect_windows_dpapi(&envelope.protected_key)?;
            let master_key = master_key_from_vec(plaintext)?;
            Ok(LoadedVaultMasterKey {
                master_key,
                protection: VaultKeyProtection::WindowsDpapiCurrentUser,
                load_outcome: VaultKeyLoadOutcome::AlreadyCurrent,
            })
        }
        VaultKeyProviderId::PlaintextDevelopment => {
            let plaintext = std::mem::take(&mut envelope.protected_key);
            let master_key = master_key_from_vec(plaintext)?;
            #[cfg(windows)]
            {
                let (encoded, protection) = encode_platform_envelope(&master_key.0)?;
                replace_key_file(path, &encoded)?;
                Ok(LoadedVaultMasterKey {
                    master_key,
                    protection,
                    load_outcome: VaultKeyLoadOutcome::LegacyMigrated,
                })
            }
            #[cfg(not(windows))]
            {
                let _ = path;
                Ok(LoadedVaultMasterKey {
                    master_key,
                    protection: VaultKeyProtection::PlaintextDevelopment,
                    load_outcome: VaultKeyLoadOutcome::AlreadyCurrent,
                })
            }
        }
    }
}

fn encode_platform_envelope(
    master_key: &[u8; VAULT_KEY_BYTES],
) -> Result<(Vec<u8>, VaultKeyProtection), StateError> {
    #[cfg(windows)]
    let (provider, protected_key, protection) = (
        VaultKeyProviderId::WindowsDpapiCurrentUser,
        protect_windows_dpapi(master_key)?,
        VaultKeyProtection::WindowsDpapiCurrentUser,
    );
    #[cfg(not(windows))]
    let (provider, protected_key, protection) = (
        VaultKeyProviderId::PlaintextDevelopment,
        master_key.to_vec(),
        VaultKeyProtection::PlaintextDevelopment,
    );

    let envelope = VaultKeyEnvelope {
        version: VAULT_KEY_ENVELOPE_VERSION,
        provider,
        protected_key,
    };
    let payload = postcard::to_allocvec(&envelope)?;
    let total_len = VAULT_KEY_ENVELOPE_MAGIC.len() + payload.len();
    if total_len > MAX_VAULT_KEY_ENVELOPE_BYTES {
        return Err(StateError::VaultKeyEnvelopeTooLarge(total_len));
    }
    let mut encoded = Vec::with_capacity(total_len);
    encoded.extend_from_slice(VAULT_KEY_ENVELOPE_MAGIC);
    encoded.extend_from_slice(&payload);
    Ok((encoded, protection))
}

#[cfg(windows)]
fn protect_windows_dpapi(plaintext: &[u8]) -> Result<Vec<u8>, StateError> {
    stellar_agent_windows_identity::dpapi_protect(plaintext).map_err(|error| {
        StateError::VaultKeyProtectionFailed {
            provider: VaultKeyProviderId::WindowsDpapiCurrentUser
                .as_str()
                .to_owned(),
            operation: "protect",
            detail: error.to_string(),
        }
    })
}

#[cfg(windows)]
fn unprotect_windows_dpapi(ciphertext: &[u8]) -> Result<Vec<u8>, StateError> {
    stellar_agent_windows_identity::dpapi_unprotect(ciphertext).map_err(|error| {
        StateError::VaultKeyProtectionFailed {
            provider: VaultKeyProviderId::WindowsDpapiCurrentUser
                .as_str()
                .to_owned(),
            operation: "unprotect",
            detail: error.to_string(),
        }
    })
}

#[cfg(not(windows))]
fn unprotect_windows_dpapi(_ciphertext: &[u8]) -> Result<Vec<u8>, StateError> {
    Err(StateError::VaultKeyProviderUnavailable(
        VaultKeyProviderId::WindowsDpapiCurrentUser
            .as_str()
            .to_owned(),
    ))
}

fn master_key_from_slice(bytes: &[u8]) -> Result<VaultMasterKey, StateError> {
    let key: [u8; VAULT_KEY_BYTES] = bytes
        .try_into()
        .map_err(|_| StateError::InvalidVaultKeyLength(bytes.len()))?;
    Ok(VaultMasterKey(key))
}

fn master_key_from_vec(mut bytes: Vec<u8>) -> Result<VaultMasterKey, StateError> {
    let result = master_key_from_slice(&bytes);
    bytes.zeroize();
    result
}

fn write_new_key_file(path: &Path, bytes: &[u8]) -> Result<(), StateError> {
    write_key_file(path, bytes, false)
}

fn replace_key_file(path: &Path, bytes: &[u8]) -> Result<(), StateError> {
    reject_symlink(path)?;
    write_key_file(path, bytes, true)
}

fn write_key_file(path: &Path, bytes: &[u8], replace: bool) -> Result<(), StateError> {
    let parent = path
        .parent()
        .ok_or_else(|| StateError::InvalidVaultKeyEnvelope {
            path: path.to_path_buf(),
            detail: "key file has no parent directory".to_owned(),
        })?;
    let mut temporary = io_at(parent, NamedTempFile::new_in(parent))?;
    let temporary_path = temporary.path().to_path_buf();
    io_at(&temporary_path, temporary.write_all(bytes))?;
    io_at(&temporary_path, temporary.as_file().sync_all())?;
    if replace {
        io_at(path, temporary.persist(path).map_err(|error| error.error))?;
    } else {
        io_at(
            path,
            temporary
                .persist_noclobber(path)
                .map_err(|error| error.error),
        )?;
    }
    sync_directory(parent)
}
