//! Authenticated, persistent pairwise Double Ratchet sessions.
//!
//! This crate deliberately keeps the vodozemac types behind a small API. The
//! rest of Kilogram only sees device-signed public key material and opaque Olm
//! ciphertexts. Account and session pickles are encrypted before they reach
//! disk.

#![forbid(unsafe_code)]

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use kilogram_identity::{
    AccountAuthoritySnapshot, AccountDeviceListSnapshot, AccountId, DeviceCertificate, DeviceId,
    DeviceIdentity, IdentityError,
};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use thiserror::Error;
use vodozemac::{
    Curve25519PublicKey, DecodeError, PickleError,
    olm::{
        Account, AccountPickle, DecryptionError, EncryptionError, OlmMessage, Session,
        SessionConfig, SessionCreationError, SessionPickle,
    },
};
use zeroize::Zeroizing;

const STATE_DIRECTORY: &str = "ratchet";
const SESSIONS_DIRECTORY: &str = "sessions";
const PICKLE_SECRET_FILE: &str = "pickle-secret.key";
const ACCOUNT_FILE: &str = "account.pickle";
const PREKEY_BUNDLE_FILE: &str = "prekey-bundle.bin";
const PREKEY_POOL_FILE: &str = "prekey-pool.bin";
const PEER_PREKEY_POOLS_DIRECTORY: &str = "peer-prekey-pools";
const PEER_PREKEY_POOL_FILE_SUFFIX: &str = ".pool";
const SESSION_FILE_SUFFIX: &str = ".session";
const PICKLE_SECRET_BYTES: usize = 32;
const RATCHET_IDENTITY_VERSION: u8 = 1;
const PREKEY_BUNDLE_VERSION: u8 = 1;
const PREKEY_POOL_VERSION: u8 = 1;
const PREKEY_DIRECTORY_VERSION: u8 = 2;
const CIPHERTEXT_VERSION: u8 = 1;
const LEGACY_SESSION_RECORD_VERSION: u8 = 1;
const SESSION_RECORD_VERSION: u8 = 2;
const PEER_PREKEY_OBSERVATION_VERSION: u8 = 1;
const RATCHET_IDENTITY_SIGNATURE_DOMAIN: &[u8] = b"kilogram:ratchet-identity-signature:v1\0";
const PREKEY_BUNDLE_SIGNATURE_DOMAIN: &[u8] = b"kilogram:prekey-bundle-signature:v1\0";
const PREKEY_POOL_SIGNATURE_DOMAIN: &[u8] = b"kilogram:prekey-pool-signature:v1\0";
const PREKEY_POOL_ID_DOMAIN: &[u8] = b"kilogram:prekey-pool-id:v1\0";
const PREKEY_SELECTION_DOMAIN: &[u8] = b"kilogram:prekey-selection:v1\0";
const MAX_RATCHET_CIPHERTEXT_BYTES: usize = 128 * 1024;
const MAX_PLAINTEXT_BYTES: usize = 64 * 1024;
pub const DEFAULT_PREKEY_POOL_SIZE: usize = 16;
pub const MAX_PREKEY_POOL_SIZE: usize = 64;
pub const DEFAULT_PREKEY_POOL_VALIDITY_SECONDS: u64 = 7 * 24 * 60 * 60;
pub const MAX_PREKEY_POOL_VALIDITY_SECONDS: u64 = 30 * 24 * 60 * 60;
pub const PREKEY_CLOCK_SKEW_SECONDS: u64 = 5 * 60;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct RatchetIdentityContent {
    version: u8,
    device_id: DeviceId,
    curve25519_key: [u8; 32],
    ed25519_key: [u8; 32],
}

/// Stable Olm identity keys, authenticated by the existing Kilogram device
/// signing key.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignedRatchetIdentity {
    content: RatchetIdentityContent,
    signature: Vec<u8>,
}

impl SignedRatchetIdentity {
    fn sign(identity: &DeviceIdentity, account: &Account) -> Result<Self, RatchetError> {
        let content = RatchetIdentityContent {
            version: RATCHET_IDENTITY_VERSION,
            device_id: identity.device_id(),
            curve25519_key: account.curve25519_key().to_bytes(),
            ed25519_key: *account.ed25519_key().as_bytes(),
        };
        let signature = identity
            .sign(&ratchet_identity_signing_bytes(&content)?)
            .to_vec();
        Ok(Self { content, signature })
    }

    pub fn verify(&self) -> Result<(), RatchetError> {
        if self.content.version != RATCHET_IDENTITY_VERSION {
            return Err(RatchetError::UnsupportedRatchetIdentityVersion(
                self.content.version,
            ));
        }
        self.content.device_id.verify(
            &ratchet_identity_signing_bytes(&self.content)?,
            &self.signature,
        )?;
        Ok(())
    }

    pub fn device_id(&self) -> DeviceId {
        self.content.device_id
    }

    pub fn curve25519_key_bytes(&self) -> [u8; 32] {
        self.content.curve25519_key
    }

    pub fn ed25519_key_bytes(&self) -> [u8; 32] {
        self.content.ed25519_key
    }

    fn curve25519_key(&self) -> Curve25519PublicKey {
        Curve25519PublicKey::from_bytes(self.content.curve25519_key)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct PrekeyBundleContent {
    version: u8,
    ratchet_identity: SignedRatchetIdentity,
    sequence: u64,
    one_time_key: [u8; 32],
}

/// A single-use asynchronous session-establishment key, signed by the
/// recipient's Kilogram device identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignedPrekeyBundle {
    content: PrekeyBundleContent,
    signature: Vec<u8>,
}

impl SignedPrekeyBundle {
    fn sign(
        identity: &DeviceIdentity,
        ratchet_identity: SignedRatchetIdentity,
        sequence: u64,
        one_time_key: Curve25519PublicKey,
    ) -> Result<Self, RatchetError> {
        let content = PrekeyBundleContent {
            version: PREKEY_BUNDLE_VERSION,
            ratchet_identity,
            sequence,
            one_time_key: one_time_key.to_bytes(),
        };
        let signature = identity
            .sign(&prekey_bundle_signing_bytes(&content)?)
            .to_vec();
        Ok(Self { content, signature })
    }

    pub fn verify(&self) -> Result<(), RatchetError> {
        if self.content.version != PREKEY_BUNDLE_VERSION {
            return Err(RatchetError::UnsupportedPrekeyBundleVersion(
                self.content.version,
            ));
        }
        self.content.ratchet_identity.verify()?;
        self.device_id().verify(
            &prekey_bundle_signing_bytes(&self.content)?,
            &self.signature,
        )?;
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>, RatchetError> {
        self.verify()?;
        Ok(postcard::to_allocvec(self)?)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, RatchetError> {
        let bundle: Self = postcard::from_bytes(bytes)?;
        bundle.verify()?;
        Ok(bundle)
    }

    pub fn device_id(&self) -> DeviceId {
        self.content.ratchet_identity.device_id()
    }

    pub fn ratchet_identity(&self) -> &SignedRatchetIdentity {
        &self.content.ratchet_identity
    }

    pub fn sequence(&self) -> u64 {
        self.content.sequence
    }

    pub fn one_time_key_bytes(&self) -> [u8; 32] {
        self.content.one_time_key
    }

    fn one_time_key(&self) -> Curve25519PublicKey {
        Curve25519PublicKey::from_bytes(self.content.one_time_key)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct PrekeyPoolContent {
    version: u8,
    ratchet_identity: SignedRatchetIdentity,
    generation: u64,
    published_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
    bundles: Vec<SignedPrekeyBundle>,
}

/// A bounded device-signed set of independently consumable Olm one-time keys.
///
/// The outer signature authenticates generation and freshness metadata. Each
/// entry remains an ordinary `SignedPrekeyBundle`, which keeps the actual OTK
/// usable by the existing Olm session-establishment boundary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignedPrekeyPool {
    content: PrekeyPoolContent,
    signature: Vec<u8>,
}

impl SignedPrekeyPool {
    fn sign(
        identity: &DeviceIdentity,
        ratchet_identity: SignedRatchetIdentity,
        generation: u64,
        published_at_unix_seconds: u64,
        expires_at_unix_seconds: u64,
        bundles: Vec<SignedPrekeyBundle>,
    ) -> Result<Self, RatchetError> {
        let content = PrekeyPoolContent {
            version: PREKEY_POOL_VERSION,
            ratchet_identity,
            generation,
            published_at_unix_seconds,
            expires_at_unix_seconds,
            bundles,
        };
        let signature = identity
            .sign(&prekey_pool_signing_bytes(&content)?)
            .to_vec();
        let pool = Self { content, signature };
        pool.verify()?;
        Ok(pool)
    }

    pub fn verify(&self) -> Result<(), RatchetError> {
        if self.content.version != PREKEY_POOL_VERSION {
            return Err(RatchetError::UnsupportedPrekeyPoolVersion(
                self.content.version,
            ));
        }
        self.content.ratchet_identity.verify()?;
        let count = self.content.bundles.len();
        if !(1..=MAX_PREKEY_POOL_SIZE).contains(&count) {
            return Err(RatchetError::InvalidPrekeyPoolSize(count));
        }
        if self.content.published_at_unix_seconds >= self.content.expires_at_unix_seconds {
            return Err(RatchetError::InvalidPrekeyPoolValidity {
                published_at: self.content.published_at_unix_seconds,
                expires_at: self.content.expires_at_unix_seconds,
            });
        }
        let validity = self
            .content
            .expires_at_unix_seconds
            .saturating_sub(self.content.published_at_unix_seconds);
        if validity > MAX_PREKEY_POOL_VALIDITY_SECONDS {
            return Err(RatchetError::PrekeyPoolValidityTooLong(validity));
        }
        for (index, bundle) in self.content.bundles.iter().enumerate() {
            bundle.verify()?;
            if bundle.ratchet_identity() != &self.content.ratchet_identity {
                return Err(RatchetError::PrekeyPoolRatchetIdentityMismatch {
                    pool_device: self.device_id(),
                    bundle_device: bundle.device_id(),
                });
            }
            let expected_sequence =
                self.first_sequence()
                    .checked_add(u64::try_from(index).map_err(|_| {
                        RatchetError::InvalidPrekeyPoolSize(self.content.bundles.len())
                    })?)
                    .ok_or(RatchetError::PrekeySequenceExhausted)?;
            if bundle.sequence() != expected_sequence {
                return Err(RatchetError::NonContiguousPrekeyPoolSequence {
                    expected: expected_sequence,
                    actual: bundle.sequence(),
                });
            }
            if self.content.bundles[..index]
                .iter()
                .any(|previous| previous.one_time_key_bytes() == bundle.one_time_key_bytes())
            {
                return Err(RatchetError::DuplicatePrekeyInPool);
            }
        }
        self.device_id()
            .verify(&prekey_pool_signing_bytes(&self.content)?, &self.signature)?;
        Ok(())
    }

    pub fn verify_at(&self, now_unix_seconds: u64) -> Result<(), RatchetError> {
        self.verify()?;
        if now_unix_seconds.saturating_add(PREKEY_CLOCK_SKEW_SECONDS)
            < self.content.published_at_unix_seconds
        {
            return Err(RatchetError::PrekeyPoolNotYetValid {
                published_at: self.content.published_at_unix_seconds,
                now: now_unix_seconds,
            });
        }
        if now_unix_seconds
            > self
                .content
                .expires_at_unix_seconds
                .saturating_add(PREKEY_CLOCK_SKEW_SECONDS)
        {
            return Err(RatchetError::PrekeyPoolExpired {
                expires_at: self.content.expires_at_unix_seconds,
                now: now_unix_seconds,
            });
        }
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>, RatchetError> {
        self.verify()?;
        Ok(postcard::to_allocvec(self)?)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, RatchetError> {
        let pool: Self = postcard::from_bytes(bytes)?;
        pool.verify()?;
        Ok(pool)
    }

    pub fn device_id(&self) -> DeviceId {
        self.content.ratchet_identity.device_id()
    }

    pub fn ratchet_identity(&self) -> &SignedRatchetIdentity {
        &self.content.ratchet_identity
    }

    pub fn generation(&self) -> u64 {
        self.content.generation
    }

    pub fn published_at_unix_seconds(&self) -> u64 {
        self.content.published_at_unix_seconds
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.content.expires_at_unix_seconds
    }

    pub fn bundles(&self) -> &[SignedPrekeyBundle] {
        &self.content.bundles
    }

    pub fn first_sequence(&self) -> u64 {
        self.content
            .bundles
            .first()
            .map_or(0, SignedPrekeyBundle::sequence)
    }

    pub fn last_sequence(&self) -> u64 {
        self.content
            .bundles
            .last()
            .map_or(0, SignedPrekeyBundle::sequence)
    }

    pub fn pool_id(&self) -> Result<[u8; 32], RatchetError> {
        self.verify()?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(PREKEY_POOL_ID_DOMAIN);
        hasher.update(&postcard::to_allocvec(self)?);
        Ok(*hasher.finalize().as_bytes())
    }

    pub fn select_for(
        &self,
        initiator_device_id: DeviceId,
    ) -> Result<&SignedPrekeyBundle, RatchetError> {
        self.verify()?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(PREKEY_SELECTION_DOMAIN);
        hasher.update(initiator_device_id.as_bytes());
        hasher.update(self.device_id().as_bytes());
        hasher.update(&self.content.generation.to_le_bytes());
        let digest = hasher.finalize();
        let mut index_bytes = [0_u8; 8];
        index_bytes.copy_from_slice(&digest.as_bytes()[..8]);
        let count = u64::try_from(self.content.bundles.len())
            .map_err(|_| RatchetError::InvalidPrekeyPoolSize(self.content.bundles.len()))?;
        let index = usize::try_from(u64::from_le_bytes(index_bytes) % count)
            .map_err(|_| RatchetError::InvalidPrekeyPoolSize(self.content.bundles.len()))?;
        Ok(&self.content.bundles[index])
    }
}

/// A root-complete account device list paired with one fresh signed prekey pool
/// for every authorized device.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountPrekeyDirectory {
    version: u8,
    device_list: AccountDeviceListSnapshot,
    pools: Vec<SignedPrekeyPool>,
}

impl AccountPrekeyDirectory {
    pub fn new(
        device_list: AccountDeviceListSnapshot,
        mut pools: Vec<SignedPrekeyPool>,
    ) -> Result<Self, RatchetError> {
        pools.sort_by_key(|pool| *pool.device_id().as_bytes());
        let directory = Self {
            version: PREKEY_DIRECTORY_VERSION,
            device_list,
            pools,
        };
        directory.verify()?;
        Ok(directory)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, RatchetError> {
        let directory: Self = postcard::from_bytes(bytes)?;
        directory.verify()?;
        Ok(directory)
    }

    pub fn encode(&self) -> Result<Vec<u8>, RatchetError> {
        self.verify()?;
        Ok(postcard::to_allocvec(self)?)
    }

    pub fn verify(&self) -> Result<(), RatchetError> {
        if self.version != PREKEY_DIRECTORY_VERSION {
            return Err(RatchetError::UnsupportedPrekeyDirectoryVersion(
                self.version,
            ));
        }
        self.device_list.verify()?;
        if self.pools.len() != self.device_list.devices().len() {
            return Err(RatchetError::PrekeyDirectoryCoverage {
                devices: self.device_list.devices().len(),
                pools: self.pools.len(),
            });
        }
        for (certificate, pool) in self.device_list.devices().iter().zip(&self.pools) {
            pool.verify()?;
            if certificate.device_id() != pool.device_id() {
                return Err(RatchetError::PrekeyDirectoryDeviceMismatch {
                    certificate: certificate.device_id(),
                    pool: pool.device_id(),
                });
            }
        }
        Ok(())
    }

    pub fn verify_at(&self, now_unix_seconds: u64) -> Result<(), RatchetError> {
        self.verify()?;
        for pool in &self.pools {
            pool.verify_at(now_unix_seconds)?;
        }
        Ok(())
    }

    pub fn account_id(&self) -> AccountId {
        self.device_list.account_id()
    }

    pub fn revision(&self) -> u64 {
        self.device_list.revision()
    }

    pub fn authority_snapshot(&self) -> &AccountAuthoritySnapshot {
        self.device_list.authority_snapshot()
    }

    pub fn device_list(&self) -> &AccountDeviceListSnapshot {
        &self.device_list
    }

    pub fn certificates(&self) -> &[DeviceCertificate] {
        self.device_list.devices()
    }

    pub fn pools(&self) -> &[SignedPrekeyPool] {
        &self.pools
    }

    pub fn certificate_for(&self, device_id: DeviceId) -> Option<&DeviceCertificate> {
        self.device_list.certificate_for(device_id)
    }

    pub fn pool_for(&self, device_id: DeviceId) -> Option<&SignedPrekeyPool> {
        self.pools
            .binary_search_by(|pool| pool.device_id().cmp(&device_id))
            .ok()
            .map(|index| &self.pools[index])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RatchetMessageKind {
    PreKey,
    Normal,
}

impl std::fmt::Display for RatchetMessageKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PreKey => formatter.write_str("pre-key"),
            Self::Normal => formatter.write_str("normal"),
        }
    }
}

/// A validated, transport-safe representation of an Olm message.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RatchetCiphertext {
    version: u8,
    message_type: u8,
    ciphertext: Vec<u8>,
}

impl RatchetCiphertext {
    fn from_olm(message: &OlmMessage) -> Result<Self, RatchetError> {
        let (message_type, ciphertext) = message.to_parts();
        let message_type = u8::try_from(message_type)
            .map_err(|_| RatchetError::InvalidMessageType(message_type))?;
        let value = Self {
            version: CIPHERTEXT_VERSION,
            message_type,
            ciphertext,
        };
        value.validate()?;
        Ok(value)
    }

    fn to_olm(&self) -> Result<OlmMessage, RatchetError> {
        self.validate()?;
        Ok(OlmMessage::from_parts(
            usize::from(self.message_type),
            &self.ciphertext,
        )?)
    }

    pub fn validate(&self) -> Result<(), RatchetError> {
        if self.version != CIPHERTEXT_VERSION {
            return Err(RatchetError::UnsupportedCiphertextVersion(self.version));
        }
        if self.ciphertext.is_empty() || self.ciphertext.len() > MAX_RATCHET_CIPHERTEXT_BYTES {
            return Err(RatchetError::InvalidCiphertextLength(self.ciphertext.len()));
        }
        let _ = OlmMessage::from_parts(usize::from(self.message_type), &self.ciphertext)?;
        Ok(())
    }

    pub fn kind(&self) -> Result<RatchetMessageKind, RatchetError> {
        self.validate()?;
        match self.message_type {
            0 => Ok(RatchetMessageKind::PreKey),
            1 => Ok(RatchetMessageKind::Normal),
            other => Err(RatchetError::InvalidMessageType(usize::from(other))),
        }
    }

    pub fn ciphertext(&self) -> &[u8] {
        &self.ciphertext
    }
}

/// Plaintext that can only be constructed by a successful ratchet decrypt.
pub struct DecryptedMessage(String);

impl DecryptedMessage {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RatchetOperation {
    pub session_id: String,
    pub session_created: bool,
    pub message_kind: RatchetMessageKind,
    pub concurrent_session_resolved: bool,
    pub retained_session_count: usize,
}

/// Exact local mutable records removed when a device is retired.
///
/// The caller is responsible for running this mutation inside Kilogram's
/// crash-consistent state transaction. Missing records are an idempotent
/// success, which makes a directory refresh safe to retry after an uncertain
/// IPC response.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RatchetRetirement {
    pub session_removed: bool,
    pub prekey_observation_removed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct LegacySessionRecord {
    version: u8,
    peer_device_id: DeviceId,
    peer_curve25519_key: [u8; 32],
    encrypted_pickle: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum SessionRole {
    Outbound,
    Inbound,
    Legacy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct SessionCandidateRecord {
    session_id: String,
    role: SessionRole,
    encrypted_pickle: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct SessionRecord {
    version: u8,
    peer_device_id: DeviceId,
    peer_curve25519_key: [u8; 32],
    active_session_id: String,
    active_confirmed: bool,
    candidates: Vec<SessionCandidateRecord>,
}

struct SessionCandidate {
    role: SessionRole,
    session: Session,
}

struct SessionSet {
    peer_device_id: DeviceId,
    peer_curve25519_key: Curve25519PublicKey,
    active_session_id: String,
    active_confirmed: bool,
    candidates: Vec<SessionCandidate>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct PeerPrekeyPoolObservation {
    version: u8,
    peer_device_id: DeviceId,
    peer_curve25519_key: [u8; 32],
    generation: u64,
    first_sequence: u64,
    last_sequence: u64,
    published_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
    pool_id: [u8; 32],
}

/// Persistent Olm account and a bounded active/retained session set per peer.
///
/// M0.7.3 assumes exclusive access to a device state directory. A production
/// client will put these records in the same transactional database as event
/// acceptance and local projections.
pub struct RatchetState {
    directory: PathBuf,
    sessions_directory: PathBuf,
    peer_prekey_pools_directory: PathBuf,
    pickle_secret: Zeroizing<[u8; PICKLE_SECRET_BYTES]>,
    account: Account,
    current_bundle: Option<SignedPrekeyBundle>,
    current_pool: Option<SignedPrekeyPool>,
}

impl RatchetState {
    pub fn load_or_create(state_directory: impl AsRef<Path>) -> Result<Self, RatchetError> {
        let directory = state_directory.as_ref().join(STATE_DIRECTORY);
        let sessions_directory = directory.join(SESSIONS_DIRECTORY);
        let peer_prekey_pools_directory = directory.join(PEER_PREKEY_POOLS_DIRECTORY);
        fs::create_dir_all(&sessions_directory)?;
        fs::create_dir_all(&peer_prekey_pools_directory)?;
        let pickle_secret =
            Zeroizing::new(load_or_create_secret(&directory.join(PICKLE_SECRET_FILE))?);
        let account_path = directory.join(ACCOUNT_FILE);
        let account = match fs::read_to_string(&account_path) {
            Ok(ciphertext) => Account::from_pickle(AccountPickle::from_encrypted(
                ciphertext.trim(),
                &pickle_secret,
            )?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let account = Account::new();
                persist_account(&account_path, &account, &pickle_secret)?;
                account
            }
            Err(error) => return Err(error.into()),
        };
        let bundle_path = directory.join(PREKEY_BUNDLE_FILE);
        let current_bundle = match fs::read(&bundle_path) {
            Ok(bytes) => Some(SignedPrekeyBundle::decode(&bytes)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        if let Some(bundle) = &current_bundle {
            ensure_bundle_matches_account(bundle, &account)?;
        }
        let pool_path = directory.join(PREKEY_POOL_FILE);
        let current_pool = match fs::read(&pool_path) {
            Ok(bytes) => Some(SignedPrekeyPool::decode(&bytes)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        if let Some(pool) = &current_pool {
            ensure_pool_matches_account(pool, &account)?;
        }

        Ok(Self {
            directory,
            sessions_directory,
            peer_prekey_pools_directory,
            pickle_secret,
            account,
            current_bundle,
            current_pool,
        })
    }

    pub fn signed_identity(
        &self,
        identity: &DeviceIdentity,
    ) -> Result<SignedRatchetIdentity, RatchetError> {
        SignedRatchetIdentity::sign(identity, &self.account)
    }

    pub fn prekey_bundle(
        &mut self,
        identity: &DeviceIdentity,
    ) -> Result<SignedPrekeyBundle, RatchetError> {
        if let Some(bundle) = &self.current_bundle {
            if identity.device_id() != bundle.device_id() {
                return Err(RatchetError::PrekeyBundleDeviceMismatch {
                    expected: bundle.device_id(),
                    actual: identity.device_id(),
                });
            }
            bundle.verify()?;
            ensure_bundle_matches_account(bundle, &self.account)?;
            if self.account.stored_one_time_key_count() > 0 {
                return Ok(bundle.clone());
            }
        }

        self.create_new_prekey_bundle(identity)
    }

    pub fn prekey_pool(
        &mut self,
        identity: &DeviceIdentity,
        count: usize,
        now_unix_seconds: u64,
        validity_seconds: u64,
    ) -> Result<SignedPrekeyPool, RatchetError> {
        validate_prekey_pool_request(count, validity_seconds)?;
        if let Some(pool) = &self.current_pool {
            if identity.device_id() != pool.device_id() {
                return Err(RatchetError::PrekeyBundleDeviceMismatch {
                    expected: pool.device_id(),
                    actual: identity.device_id(),
                });
            }
            pool.verify()?;
            ensure_pool_matches_account(pool, &self.account)?;
            match pool.verify_at(now_unix_seconds) {
                Ok(()) if pool.bundles().len() == count => return Ok(pool.clone()),
                Ok(()) | Err(RatchetError::PrekeyPoolExpired { .. }) => {}
                Err(error) => return Err(error),
            }
        }
        self.create_new_prekey_pool(identity, count, now_unix_seconds, validity_seconds)
    }

    pub fn refresh_prekey_pool(
        &mut self,
        identity: &DeviceIdentity,
        count: usize,
        now_unix_seconds: u64,
        validity_seconds: u64,
    ) -> Result<SignedPrekeyPool, RatchetError> {
        validate_prekey_pool_request(count, validity_seconds)?;
        self.create_new_prekey_pool(identity, count, now_unix_seconds, validity_seconds)
    }

    pub fn has_session(&self, peer_device_id: DeviceId) -> bool {
        self.session_path(peer_device_id).is_file()
    }

    pub fn retire_peer_device(
        &self,
        peer_device_id: DeviceId,
    ) -> Result<RatchetRetirement, RatchetError> {
        Ok(RatchetRetirement {
            session_removed: remove_file_if_present(&self.session_path(peer_device_id))?,
            prekey_observation_removed: remove_file_if_present(
                &self.peer_prekey_pool_path(peer_device_id),
            )?,
        })
    }

    pub fn encrypt(
        &mut self,
        identity: &DeviceIdentity,
        peer_bundle: &SignedPrekeyBundle,
        plaintext: &str,
    ) -> Result<(SignedRatchetIdentity, RatchetCiphertext, RatchetOperation), RatchetError> {
        self.encrypt_with_bundle(identity, peer_bundle, plaintext)
    }

    pub fn encrypt_with_pool(
        &mut self,
        identity: &DeviceIdentity,
        peer_pool: &SignedPrekeyPool,
        plaintext: &str,
        now_unix_seconds: u64,
    ) -> Result<(SignedRatchetIdentity, RatchetCiphertext, RatchetOperation), RatchetError> {
        self.observe_prekey_pool(peer_pool, now_unix_seconds)?;
        let bundle = peer_pool.select_for(identity.device_id())?;
        self.encrypt_with_bundle(identity, bundle, plaintext)
    }

    pub fn observe_prekey_pool(
        &self,
        peer_pool: &SignedPrekeyPool,
        now_unix_seconds: u64,
    ) -> Result<(), RatchetError> {
        peer_pool.verify_at(now_unix_seconds)?;
        self.observe_peer_prekey_pool(peer_pool)
    }

    pub fn observe_prekey_directory(
        &self,
        directory: &AccountPrekeyDirectory,
        now_unix_seconds: u64,
    ) -> Result<(), RatchetError> {
        directory.verify_at(now_unix_seconds)?;
        for pool in directory.pools() {
            self.observe_peer_prekey_pool(pool)?;
        }
        Ok(())
    }

    fn encrypt_with_bundle(
        &mut self,
        identity: &DeviceIdentity,
        peer_bundle: &SignedPrekeyBundle,
        plaintext: &str,
    ) -> Result<(SignedRatchetIdentity, RatchetCiphertext, RatchetOperation), RatchetError> {
        peer_bundle.verify()?;
        validate_plaintext(plaintext)?;
        let peer_device_id = peer_bundle.device_id();
        if peer_device_id == identity.device_id() {
            return Err(RatchetError::SelfSession(peer_device_id));
        }
        let peer_curve_key = peer_bundle.ratchet_identity().curve25519_key();
        let (mut session_set, session_created) = match self.load_session_set(peer_device_id)? {
            Some(session_set) => {
                ensure_stored_peer_key(
                    session_set.peer_curve25519_key,
                    peer_curve_key,
                    peer_device_id,
                )?;
                (session_set, false)
            }
            None => {
                let session = self.account.create_outbound_session(
                    SessionConfig::version_1(),
                    peer_curve_key,
                    peer_bundle.one_time_key(),
                )?;
                let session_id = session.session_id();
                (
                    SessionSet {
                        peer_device_id,
                        peer_curve25519_key: peer_curve_key,
                        active_session_id: session_id,
                        active_confirmed: false,
                        candidates: vec![SessionCandidate {
                            role: SessionRole::Outbound,
                            session,
                        }],
                    },
                    true,
                )
            }
        };
        let active_index = session_set.active_index()?;
        let session = &mut session_set.candidates[active_index].session;
        let message = session.encrypt(plaintext.as_bytes())?;
        let ciphertext = RatchetCiphertext::from_olm(&message)?;
        let operation = RatchetOperation {
            session_id: session.session_id(),
            session_created,
            message_kind: ciphertext.kind()?,
            concurrent_session_resolved: false,
            retained_session_count: session_set.retained_session_count(),
        };
        self.persist_session_set(&session_set)?;
        Ok((self.signed_identity(identity)?, ciphertext, operation))
    }

    pub fn decrypt(
        &mut self,
        identity: &DeviceIdentity,
        sender_identity: &SignedRatchetIdentity,
        ciphertext: &RatchetCiphertext,
    ) -> Result<(DecryptedMessage, RatchetOperation), RatchetError> {
        sender_identity.verify()?;
        ciphertext.validate()?;
        let peer_device_id = sender_identity.device_id();
        if peer_device_id == identity.device_id() {
            return Err(RatchetError::SelfSession(peer_device_id));
        }
        let peer_curve_key = sender_identity.curve25519_key();
        let message = ciphertext.to_olm()?;
        let mut concurrent_session_resolved = false;
        let (session_set, plaintext, session_created, used_session_id) =
            match self.load_session_set(peer_device_id)? {
                Some(mut session_set) => {
                    ensure_stored_peer_key(
                        session_set.peer_curve25519_key,
                        peer_curve_key,
                        peer_device_id,
                    )?;
                    match &message {
                        OlmMessage::PreKey(prekey_message) => {
                            let incoming_session_id = prekey_message.session_id();
                            if let Some(index) = session_set.index_of(&incoming_session_id) {
                                let plaintext = decrypt_with_candidate(
                                    &mut session_set.candidates[index],
                                    &message,
                                )?;
                                (session_set, plaintext, false, incoming_session_id)
                            } else {
                                if session_set.active_confirmed {
                                    return Err(RatchetError::UnexpectedConcurrentSession {
                                        peer: peer_device_id,
                                        incoming_session_id,
                                    });
                                }
                                if session_set.candidates.len() >= 2 {
                                    return Err(RatchetError::TooManyConcurrentSessions(
                                        peer_device_id,
                                    ));
                                }
                                let result = self.account.create_inbound_session(
                                    SessionConfig::version_1(),
                                    peer_curve_key,
                                    prekey_message,
                                )?;
                                let new_session_id = result.session.session_id();
                                session_set.candidates.push(SessionCandidate {
                                    role: SessionRole::Inbound,
                                    session: result.session,
                                });
                                session_set.active_session_id = session_set
                                    .candidates
                                    .iter()
                                    .map(|candidate| candidate.session.session_id())
                                    .min()
                                    .ok_or(RatchetError::EmptySessionSet(peer_device_id))?;
                                session_set.active_confirmed = true;
                                concurrent_session_resolved = true;
                                (session_set, result.plaintext, true, new_session_id)
                            }
                        }
                        OlmMessage::Normal(_) => {
                            let (plaintext, used_session_id) =
                                decrypt_normal_with_session_set(&mut session_set, &message)?;
                            if used_session_id == session_set.active_session_id {
                                session_set.active_confirmed = true;
                            }
                            (session_set, plaintext, false, used_session_id)
                        }
                    }
                }
                None => {
                    let OlmMessage::PreKey(prekey_message) = &message else {
                        return Err(RatchetError::NormalMessageWithoutSession(peer_device_id));
                    };
                    let result = self.account.create_inbound_session(
                        SessionConfig::version_1(),
                        peer_curve_key,
                        prekey_message,
                    )?;
                    let session_id = result.session.session_id();
                    (
                        SessionSet {
                            peer_device_id,
                            peer_curve25519_key: peer_curve_key,
                            active_session_id: session_id.clone(),
                            active_confirmed: true,
                            candidates: vec![SessionCandidate {
                                role: SessionRole::Inbound,
                                session: result.session,
                            }],
                        },
                        result.plaintext,
                        true,
                        session_id,
                    )
                }
            };
        let body = String::from_utf8(plaintext)?;
        validate_plaintext(&body)?;
        let operation = RatchetOperation {
            session_id: used_session_id,
            session_created,
            message_kind: ciphertext.kind()?,
            concurrent_session_resolved,
            retained_session_count: session_set.retained_session_count(),
        };
        self.persist_session_set(&session_set)?;
        if session_created {
            persist_account(
                &self.directory.join(ACCOUNT_FILE),
                &self.account,
                &self.pickle_secret,
            )?;
            if let Some(pool) = &self.current_pool {
                let count = pool.bundles().len();
                let validity = pool
                    .expires_at_unix_seconds()
                    .saturating_sub(pool.published_at_unix_seconds());
                let _ = self.create_new_prekey_pool(identity, count, unix_time_now()?, validity)?;
            } else {
                let _ = self.create_new_prekey_bundle(identity)?;
            }
        }
        Ok((DecryptedMessage(body), operation))
    }

    fn create_new_prekey_bundle(
        &mut self,
        identity: &DeviceIdentity,
    ) -> Result<SignedPrekeyBundle, RatchetError> {
        let sequence = self.current_bundle.as_ref().map_or(Ok(0), |bundle| {
            bundle
                .sequence()
                .checked_add(1)
                .ok_or(RatchetError::PrekeySequenceExhausted)
        })?;
        self.account.generate_one_time_keys(1);
        let one_time_key = self
            .account
            .one_time_keys()
            .values()
            .next()
            .copied()
            .ok_or(RatchetError::PrekeyGenerationFailed)?;
        let bundle = SignedPrekeyBundle::sign(
            identity,
            self.signed_identity(identity)?,
            sequence,
            one_time_key,
        )?;
        self.account.mark_keys_as_published();
        persist_account(
            &self.directory.join(ACCOUNT_FILE),
            &self.account,
            &self.pickle_secret,
        )?;
        atomic_write(&self.directory.join(PREKEY_BUNDLE_FILE), &bundle.encode()?)?;
        self.current_bundle = Some(bundle.clone());
        Ok(bundle)
    }

    fn create_new_prekey_pool(
        &mut self,
        identity: &DeviceIdentity,
        count: usize,
        now_unix_seconds: u64,
        validity_seconds: u64,
    ) -> Result<SignedPrekeyPool, RatchetError> {
        validate_prekey_pool_request(count, validity_seconds)?;
        let generation = self.current_pool.as_ref().map_or(Ok(0), |pool| {
            pool.generation()
                .checked_add(1)
                .ok_or(RatchetError::PrekeyPoolGenerationExhausted)
        })?;
        let first_sequence = match &self.current_pool {
            Some(pool) => pool
                .last_sequence()
                .checked_add(1)
                .ok_or(RatchetError::PrekeySequenceExhausted)?,
            None => self.current_bundle.as_ref().map_or(Ok(0), |bundle| {
                bundle
                    .sequence()
                    .checked_add(1)
                    .ok_or(RatchetError::PrekeySequenceExhausted)
            })?,
        };
        let expires_at_unix_seconds = now_unix_seconds.checked_add(validity_seconds).ok_or(
            RatchetError::InvalidPrekeyPoolValidity {
                published_at: now_unix_seconds,
                expires_at: u64::MAX,
            },
        )?;
        self.account.generate_one_time_keys(count);
        let mut one_time_keys = self
            .account
            .one_time_keys()
            .into_values()
            .collect::<Vec<_>>();
        if one_time_keys.len() != count {
            return Err(RatchetError::PrekeyPoolGenerationFailed {
                expected: count,
                actual: one_time_keys.len(),
            });
        }
        one_time_keys.sort_by_key(Curve25519PublicKey::to_bytes);
        let ratchet_identity = self.signed_identity(identity)?;
        let mut bundles = Vec::with_capacity(count);
        for (index, one_time_key) in one_time_keys.into_iter().enumerate() {
            let index =
                u64::try_from(index).map_err(|_| RatchetError::InvalidPrekeyPoolSize(count))?;
            let sequence = first_sequence
                .checked_add(index)
                .ok_or(RatchetError::PrekeySequenceExhausted)?;
            bundles.push(SignedPrekeyBundle::sign(
                identity,
                ratchet_identity.clone(),
                sequence,
                one_time_key,
            )?);
        }
        let pool = SignedPrekeyPool::sign(
            identity,
            ratchet_identity,
            generation,
            now_unix_seconds,
            expires_at_unix_seconds,
            bundles,
        )?;
        self.account.mark_keys_as_published();
        persist_account(
            &self.directory.join(ACCOUNT_FILE),
            &self.account,
            &self.pickle_secret,
        )?;
        atomic_write(&self.directory.join(PREKEY_POOL_FILE), &pool.encode()?)?;
        self.current_pool = Some(pool.clone());
        Ok(pool)
    }

    fn observe_peer_prekey_pool(&self, pool: &SignedPrekeyPool) -> Result<(), RatchetError> {
        let observation = PeerPrekeyPoolObservation {
            version: PEER_PREKEY_OBSERVATION_VERSION,
            peer_device_id: pool.device_id(),
            peer_curve25519_key: pool.ratchet_identity().curve25519_key_bytes(),
            generation: pool.generation(),
            first_sequence: pool.first_sequence(),
            last_sequence: pool.last_sequence(),
            published_at_unix_seconds: pool.published_at_unix_seconds(),
            expires_at_unix_seconds: pool.expires_at_unix_seconds(),
            pool_id: pool.pool_id()?,
        };
        let path = self.peer_prekey_pool_path(pool.device_id());
        let previous = match fs::read(&path) {
            Ok(bytes) => Some(decode_peer_prekey_observation(&bytes, pool.device_id())?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        if let Some(previous) = previous {
            if previous.peer_curve25519_key != observation.peer_curve25519_key {
                return Err(RatchetError::PeerRatchetIdentityChanged(pool.device_id()));
            }
            if observation.generation < previous.generation {
                return Err(RatchetError::PrekeyPoolGenerationRollback {
                    peer: pool.device_id(),
                    previous: previous.generation,
                    observed: observation.generation,
                });
            }
            if observation.generation == previous.generation {
                if observation.pool_id != previous.pool_id {
                    return Err(RatchetError::PrekeyPoolEquivocation {
                        peer: pool.device_id(),
                        generation: observation.generation,
                    });
                }
                return Ok(());
            }
            if observation.first_sequence <= previous.last_sequence {
                return Err(RatchetError::PrekeyPoolSequenceRollback {
                    peer: pool.device_id(),
                    previous_last: previous.last_sequence,
                    observed_first: observation.first_sequence,
                });
            }
            if observation.published_at_unix_seconds < previous.published_at_unix_seconds {
                return Err(RatchetError::PrekeyPoolTimestampRollback {
                    peer: pool.device_id(),
                    previous: previous.published_at_unix_seconds,
                    observed: observation.published_at_unix_seconds,
                });
            }
        }
        atomic_write(&path, &postcard::to_allocvec(&observation)?)
    }

    fn load_session_set(
        &self,
        peer_device_id: DeviceId,
    ) -> Result<Option<SessionSet>, RatchetError> {
        let path = self.session_path(peer_device_id);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let version = bytes
            .first()
            .copied()
            .ok_or(RatchetError::EmptySessionRecord)?;
        let record = match version {
            LEGACY_SESSION_RECORD_VERSION => {
                let legacy: LegacySessionRecord = postcard::from_bytes(&bytes)?;
                let session = Session::from_pickle(SessionPickle::from_encrypted(
                    &legacy.encrypted_pickle,
                    &self.pickle_secret,
                )?);
                SessionRecord {
                    version: SESSION_RECORD_VERSION,
                    peer_device_id: legacy.peer_device_id,
                    peer_curve25519_key: legacy.peer_curve25519_key,
                    active_session_id: session.session_id(),
                    active_confirmed: true,
                    candidates: vec![SessionCandidateRecord {
                        session_id: session.session_id(),
                        role: SessionRole::Legacy,
                        encrypted_pickle: legacy.encrypted_pickle,
                    }],
                }
            }
            SESSION_RECORD_VERSION => postcard::from_bytes(&bytes)?,
            other => return Err(RatchetError::UnsupportedSessionRecordVersion(other)),
        };
        validate_session_record(&record, peer_device_id)?;
        let mut candidates = Vec::with_capacity(record.candidates.len());
        for candidate in record.candidates {
            let session = Session::from_pickle(SessionPickle::from_encrypted(
                &candidate.encrypted_pickle,
                &self.pickle_secret,
            )?);
            if session.session_id() != candidate.session_id {
                return Err(RatchetError::SessionIdMismatch {
                    stored: candidate.session_id,
                    actual: session.session_id(),
                });
            }
            candidates.push(SessionCandidate {
                role: candidate.role,
                session,
            });
        }
        Ok(Some(SessionSet {
            peer_device_id: record.peer_device_id,
            peer_curve25519_key: Curve25519PublicKey::from_bytes(record.peer_curve25519_key),
            active_session_id: record.active_session_id,
            active_confirmed: record.active_confirmed,
            candidates,
        }))
    }

    fn persist_session_set(&self, session_set: &SessionSet) -> Result<(), RatchetError> {
        session_set.validate()?;
        let mut candidates = session_set
            .candidates
            .iter()
            .map(|candidate| SessionCandidateRecord {
                session_id: candidate.session.session_id(),
                role: candidate.role,
                encrypted_pickle: candidate.session.pickle().encrypt(&self.pickle_secret),
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.session_id.cmp(&right.session_id));
        let record = SessionRecord {
            version: SESSION_RECORD_VERSION,
            peer_device_id: session_set.peer_device_id,
            peer_curve25519_key: session_set.peer_curve25519_key.to_bytes(),
            active_session_id: session_set.active_session_id.clone(),
            active_confirmed: session_set.active_confirmed,
            candidates,
        };
        atomic_write(
            &self.session_path(session_set.peer_device_id),
            &postcard::to_allocvec(&record)?,
        )
    }

    fn session_path(&self, peer_device_id: DeviceId) -> PathBuf {
        self.sessions_directory
            .join(format!("{peer_device_id}{SESSION_FILE_SUFFIX}"))
    }

    fn peer_prekey_pool_path(&self, peer_device_id: DeviceId) -> PathBuf {
        self.peer_prekey_pools_directory
            .join(format!("{peer_device_id}{PEER_PREKEY_POOL_FILE_SUFFIX}"))
    }
}

impl SessionSet {
    fn active_index(&self) -> Result<usize, RatchetError> {
        self.index_of(&self.active_session_id)
            .ok_or_else(|| RatchetError::MissingActiveSession {
                peer: self.peer_device_id,
                session_id: self.active_session_id.clone(),
            })
    }

    fn index_of(&self, session_id: &str) -> Option<usize> {
        self.candidates
            .iter()
            .position(|candidate| candidate.session.session_id() == session_id)
    }

    fn retained_session_count(&self) -> usize {
        self.candidates.len().saturating_sub(1)
    }

    fn validate(&self) -> Result<(), RatchetError> {
        if self.candidates.is_empty() {
            return Err(RatchetError::EmptySessionSet(self.peer_device_id));
        }
        if self.candidates.len() > 2 {
            return Err(RatchetError::TooManyConcurrentSessions(self.peer_device_id));
        }
        let _ = self.active_index()?;
        for (index, candidate) in self.candidates.iter().enumerate() {
            let session_id = candidate.session.session_id();
            if self.candidates[..index]
                .iter()
                .any(|previous| previous.session.session_id() == session_id)
            {
                return Err(RatchetError::DuplicateSessionId(session_id));
            }
        }
        Ok(())
    }
}

fn decrypt_with_candidate(
    candidate: &mut SessionCandidate,
    message: &OlmMessage,
) -> Result<Vec<u8>, RatchetError> {
    let mut trial = Session::from_pickle(candidate.session.pickle());
    let plaintext = trial.decrypt(message)?;
    candidate.session = trial;
    Ok(plaintext)
}

fn decrypt_normal_with_session_set(
    session_set: &mut SessionSet,
    message: &OlmMessage,
) -> Result<(Vec<u8>, String), RatchetError> {
    let active_index = session_set.active_index()?;
    let mut order = Vec::with_capacity(session_set.candidates.len());
    order.push(active_index);
    order.extend((0..session_set.candidates.len()).filter(|candidate| *candidate != active_index));
    for index in order {
        let session_id = session_set.candidates[index].session.session_id();
        if let Ok(plaintext) = decrypt_with_candidate(&mut session_set.candidates[index], message) {
            return Ok((plaintext, session_id));
        }
    }
    Err(RatchetError::NoMatchingSession(session_set.peer_device_id))
}

fn validate_session_record(
    record: &SessionRecord,
    expected_peer_device_id: DeviceId,
) -> Result<(), RatchetError> {
    if record.version != SESSION_RECORD_VERSION {
        return Err(RatchetError::UnsupportedSessionRecordVersion(
            record.version,
        ));
    }
    if record.peer_device_id != expected_peer_device_id {
        return Err(RatchetError::SessionFileNameMismatch {
            expected: expected_peer_device_id,
            actual: record.peer_device_id,
        });
    }
    if record.candidates.is_empty() {
        return Err(RatchetError::EmptySessionSet(expected_peer_device_id));
    }
    if record.candidates.len() > 2 {
        return Err(RatchetError::TooManyConcurrentSessions(
            expected_peer_device_id,
        ));
    }
    if !record
        .candidates
        .iter()
        .any(|candidate| candidate.session_id == record.active_session_id)
    {
        return Err(RatchetError::MissingActiveSession {
            peer: expected_peer_device_id,
            session_id: record.active_session_id.clone(),
        });
    }
    for (index, candidate) in record.candidates.iter().enumerate() {
        if record.candidates[..index]
            .iter()
            .any(|previous| previous.session_id == candidate.session_id)
        {
            return Err(RatchetError::DuplicateSessionId(
                candidate.session_id.clone(),
            ));
        }
    }
    Ok(())
}

fn decode_peer_prekey_observation(
    bytes: &[u8],
    expected_peer_device_id: DeviceId,
) -> Result<PeerPrekeyPoolObservation, RatchetError> {
    let observation: PeerPrekeyPoolObservation = postcard::from_bytes(bytes)?;
    if observation.version != PEER_PREKEY_OBSERVATION_VERSION {
        return Err(RatchetError::UnsupportedPeerPrekeyObservationVersion(
            observation.version,
        ));
    }
    if observation.peer_device_id != expected_peer_device_id {
        return Err(RatchetError::PeerPrekeyObservationFileNameMismatch {
            expected: expected_peer_device_id,
            actual: observation.peer_device_id,
        });
    }
    Ok(observation)
}

fn ensure_bundle_matches_account(
    bundle: &SignedPrekeyBundle,
    account: &Account,
) -> Result<(), RatchetError> {
    if bundle.ratchet_identity().curve25519_key() != account.curve25519_key()
        || bundle.ratchet_identity().ed25519_key_bytes() != *account.ed25519_key().as_bytes()
    {
        return Err(RatchetError::PrekeyBundleAccountMismatch);
    }
    Ok(())
}

fn ensure_pool_matches_account(
    pool: &SignedPrekeyPool,
    account: &Account,
) -> Result<(), RatchetError> {
    if pool.ratchet_identity().curve25519_key() != account.curve25519_key()
        || pool.ratchet_identity().ed25519_key_bytes() != *account.ed25519_key().as_bytes()
    {
        return Err(RatchetError::PrekeyBundleAccountMismatch);
    }
    Ok(())
}

fn validate_prekey_pool_request(count: usize, validity_seconds: u64) -> Result<(), RatchetError> {
    if !(1..=MAX_PREKEY_POOL_SIZE).contains(&count) {
        return Err(RatchetError::InvalidPrekeyPoolSize(count));
    }
    if validity_seconds == 0 || validity_seconds > MAX_PREKEY_POOL_VALIDITY_SECONDS {
        return Err(RatchetError::PrekeyPoolValidityTooLong(validity_seconds));
    }
    Ok(())
}

fn ensure_stored_peer_key(
    stored_peer_key: Curve25519PublicKey,
    expected_peer_key: Curve25519PublicKey,
    peer_device_id: DeviceId,
) -> Result<(), RatchetError> {
    if stored_peer_key != expected_peer_key {
        return Err(RatchetError::PeerRatchetIdentityChanged(peer_device_id));
    }
    Ok(())
}

fn validate_plaintext(plaintext: &str) -> Result<(), RatchetError> {
    if plaintext.is_empty() || plaintext.len() > MAX_PLAINTEXT_BYTES {
        return Err(RatchetError::InvalidPlaintextLength(plaintext.len()));
    }
    Ok(())
}

fn ratchet_identity_signing_bytes(
    content: &RatchetIdentityContent,
) -> Result<Vec<u8>, RatchetError> {
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(RATCHET_IDENTITY_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(RATCHET_IDENTITY_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn prekey_bundle_signing_bytes(content: &PrekeyBundleContent) -> Result<Vec<u8>, RatchetError> {
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(PREKEY_BUNDLE_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(PREKEY_BUNDLE_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn prekey_pool_signing_bytes(content: &PrekeyPoolContent) -> Result<Vec<u8>, RatchetError> {
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(PREKEY_POOL_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(PREKEY_POOL_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

pub fn unix_time_now() -> Result<u64, RatchetError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| RatchetError::SystemTimeBeforeUnixEpoch)
}

fn load_or_create_secret(path: &Path) -> Result<[u8; PICKLE_SECRET_BYTES], RatchetError> {
    match open_new_secret_file(path) {
        Ok(mut file) => {
            let mut secret = [0_u8; PICKLE_SECRET_BYTES];
            getrandom::fill(&mut secret).map_err(RatchetError::SecureRandom)?;
            file.write_all(&secret)?;
            file.sync_all()?;
            Ok(secret)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let bytes = fs::read(path)?;
            bytes
                .try_into()
                .map_err(|bytes: Vec<u8>| RatchetError::InvalidPickleSecretLength(bytes.len()))
        }
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

fn persist_account(
    path: &Path,
    account: &Account,
    pickle_secret: &[u8; PICKLE_SECRET_BYTES],
) -> Result<(), RatchetError> {
    atomic_write(path, account.pickle().encrypt(pickle_secret).as_bytes())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), RatchetError> {
    let parent = path
        .parent()
        .ok_or_else(|| RatchetError::PathHasNoParent(path.to_path_buf()))?;
    fs::create_dir_all(parent)?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .map_err(|error| RatchetError::Io(error.error))?;
    Ok(())
}

fn remove_file_if_present(path: &Path) -> Result<bool, RatchetError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

#[derive(Debug, Error)]
pub enum RatchetError {
    #[error("ratchet state I/O failed")]
    Io(#[from] io::Error),

    #[error("secure random generation failed: {0}")]
    SecureRandom(getrandom::Error),

    #[error("ratchet encoding is invalid")]
    Encoding(#[from] postcard::Error),

    #[error("device identity validation failed")]
    Identity(#[from] IdentityError),

    #[error("encrypted ratchet state is invalid")]
    Pickle(#[from] PickleError),

    #[error("Olm message encoding is invalid")]
    MessageDecode(#[from] DecodeError),

    #[error("Olm session creation failed")]
    SessionCreation(#[from] SessionCreationError),

    #[error("Olm message encryption failed")]
    Encryption(#[from] EncryptionError),

    #[error("Olm message decryption failed")]
    Decryption(#[from] DecryptionError),

    #[error("decrypted ratchet message is not UTF-8")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),

    #[error("unsupported ratchet identity version: {0}")]
    UnsupportedRatchetIdentityVersion(u8),

    #[error("unsupported prekey bundle version: {0}")]
    UnsupportedPrekeyBundleVersion(u8),

    #[error("unsupported signed prekey-pool version: {0}")]
    UnsupportedPrekeyPoolVersion(u8),

    #[error("signed prekey pool contains {0} entries; expected 1..={MAX_PREKEY_POOL_SIZE}")]
    InvalidPrekeyPoolSize(usize),

    #[error("signed prekey pool has invalid validity interval {published_at}..{expires_at}")]
    InvalidPrekeyPoolValidity { published_at: u64, expires_at: u64 },

    #[error(
        "signed prekey pool validity is {0} seconds; expected 1..={MAX_PREKEY_POOL_VALIDITY_SECONDS}"
    )]
    PrekeyPoolValidityTooLong(u64),

    #[error("signed prekey pool is not yet valid: published_at={published_at}, now={now}")]
    PrekeyPoolNotYetValid { published_at: u64, now: u64 },

    #[error("signed prekey pool expired at {expires_at}; now={now}")]
    PrekeyPoolExpired { expires_at: u64, now: u64 },

    #[error("prekey pool ratchet identity names {pool_device}, but an entry names {bundle_device}")]
    PrekeyPoolRatchetIdentityMismatch {
        pool_device: DeviceId,
        bundle_device: DeviceId,
    },

    #[error("prekey pool sequence is not contiguous: expected {expected}, got {actual}")]
    NonContiguousPrekeyPoolSequence { expected: u64, actual: u64 },

    #[error("prekey pool contains the same one-time key more than once")]
    DuplicatePrekeyInPool,

    #[error("unsupported account prekey-directory version: {0}")]
    UnsupportedPrekeyDirectoryVersion(u8),

    #[error("prekey directory contains {pools} pools for {devices} devices")]
    PrekeyDirectoryCoverage { devices: usize, pools: usize },

    #[error(
        "prekey directory certificate names device {certificate}, but aligned pool names {pool}"
    )]
    PrekeyDirectoryDeviceMismatch {
        certificate: DeviceId,
        pool: DeviceId,
    },

    #[error("unsupported ratchet ciphertext version: {0}")]
    UnsupportedCiphertextVersion(u8),

    #[error("unsupported persistent ratchet session version: {0}")]
    UnsupportedSessionRecordVersion(u8),

    #[error("invalid Olm message type: {0}")]
    InvalidMessageType(usize),

    #[error("ratchet ciphertext has invalid length: {0}")]
    InvalidCiphertextLength(usize),

    #[error("ratchet plaintext has invalid length: {0}")]
    InvalidPlaintextLength(usize),

    #[error("ratchet pickle secret has {0} bytes; expected {PICKLE_SECRET_BYTES}")]
    InvalidPickleSecretLength(usize),

    #[error("prekey bundle does not match the stored Olm account")]
    PrekeyBundleAccountMismatch,

    #[error("ratchet state belongs to device {expected}; current device is {actual}")]
    PrekeyBundleDeviceMismatch {
        expected: DeviceId,
        actual: DeviceId,
    },

    #[error("prekey sequence is exhausted")]
    PrekeySequenceExhausted,

    #[error("prekey pool generation is exhausted")]
    PrekeyPoolGenerationExhausted,

    #[error("Olm did not produce a one-time key")]
    PrekeyGenerationFailed,

    #[error("Olm produced {actual} prekeys for a requested pool of {expected}")]
    PrekeyPoolGenerationFailed { expected: usize, actual: usize },

    #[error("peer {peer} prekey pool generation rolled back from {previous} to {observed}")]
    PrekeyPoolGenerationRollback {
        peer: DeviceId,
        previous: u64,
        observed: u64,
    },

    #[error("peer {peer} equivocated at prekey pool generation {generation}")]
    PrekeyPoolEquivocation { peer: DeviceId, generation: u64 },

    #[error(
        "peer {peer} prekey sequence rolled back: previous last={previous_last}, observed first={observed_first}"
    )]
    PrekeyPoolSequenceRollback {
        peer: DeviceId,
        previous_last: u64,
        observed_first: u64,
    },

    #[error("peer {peer} prekey publication timestamp rolled back from {previous} to {observed}")]
    PrekeyPoolTimestampRollback {
        peer: DeviceId,
        previous: u64,
        observed: u64,
    },

    #[error("unsupported peer prekey observation version: {0}")]
    UnsupportedPeerPrekeyObservationVersion(u8),

    #[error("peer prekey observation names {actual}; file names {expected}")]
    PeerPrekeyObservationFileNameMismatch {
        expected: DeviceId,
        actual: DeviceId,
    },

    #[error("refusing to create a ratchet session with local device {0}")]
    SelfSession(DeviceId),

    #[error("peer {0} changed its signed ratchet identity key")]
    PeerRatchetIdentityChanged(DeviceId),

    #[error("normal Olm message arrived before a session existed for peer {0}")]
    NormalMessageWithoutSession(DeviceId),

    #[error("no retained Olm session could decrypt a normal message from peer {0}")]
    NoMatchingSession(DeviceId),

    #[error("peer {peer} attempted unexpected concurrent session {incoming_session_id}")]
    UnexpectedConcurrentSession {
        peer: DeviceId,
        incoming_session_id: String,
    },

    #[error("more than two concurrent sessions exist for peer {0}")]
    TooManyConcurrentSessions(DeviceId),

    #[error("persistent session set for peer {0} is empty")]
    EmptySessionSet(DeviceId),

    #[error("active session {session_id} is missing for peer {peer}")]
    MissingActiveSession { peer: DeviceId, session_id: String },

    #[error("persistent session set contains duplicate session ID {0}")]
    DuplicateSessionId(String),

    #[error("stored session ID {stored} does not match decrypted pickle ID {actual}")]
    SessionIdMismatch { stored: String, actual: String },

    #[error("persistent ratchet session record is empty")]
    EmptySessionRecord,

    #[error("ratchet session file names peer {actual}; expected {expected}")]
    SessionFileNameMismatch {
        expected: DeviceId,
        actual: DeviceId,
    },

    #[error("persistent path has no parent: {0}")]
    PathHasNoParent(PathBuf),

    #[error("system clock is before the Unix epoch")]
    SystemTimeBeforeUnixEpoch,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kilogram_identity::{
        AccountRootState, DeviceCapability, DeviceEncryptionIdentity, DeviceIdentity,
    };
    use tempfile::tempdir;

    #[test]
    fn persistent_prekey_reply_and_normal_message_complete_a_ratchet_cycle()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempdir()?;
        let alice_dir = root.path().join("alice");
        let bob_dir = root.path().join("bob");
        let alice_identity = DeviceIdentity::generate()?;
        let bob_identity = DeviceIdentity::generate()?;

        let bob_bundle = RatchetState::load_or_create(&bob_dir)?.prekey_bundle(&bob_identity)?;
        let mut alice = RatchetState::load_or_create(&alice_dir)?;
        let (alice_ratchet_identity, first, first_op) =
            alice.encrypt(&alice_identity, &bob_bundle, "first")?;
        assert!(first_op.session_created);
        assert_eq!(first_op.message_kind, RatchetMessageKind::PreKey);
        drop(alice);

        let mut bob = RatchetState::load_or_create(&bob_dir)?;
        let (plaintext, receive_op) =
            bob.decrypt(&bob_identity, &alice_ratchet_identity, &first)?;
        assert_eq!(plaintext.as_str(), "first");
        assert!(receive_op.session_created);
        assert_eq!(receive_op.session_id, first_op.session_id);
        let alice_bundle =
            RatchetState::load_or_create(&alice_dir)?.prekey_bundle(&alice_identity)?;
        let (bob_ratchet_identity, reply, reply_op) =
            bob.encrypt(&bob_identity, &alice_bundle, "reply")?;
        assert!(!reply_op.session_created);
        assert_eq!(reply_op.message_kind, RatchetMessageKind::Normal);
        drop(bob);

        let mut alice = RatchetState::load_or_create(&alice_dir)?;
        let (plaintext, reply_receive_op) =
            alice.decrypt(&alice_identity, &bob_ratchet_identity, &reply)?;
        assert_eq!(plaintext.as_str(), "reply");
        assert!(!reply_receive_op.session_created);
        let (alice_ratchet_identity, subsequent, subsequent_op) =
            alice.encrypt(&alice_identity, &bob_bundle, "subsequent")?;
        assert!(!subsequent_op.session_created);
        assert_eq!(subsequent_op.message_kind, RatchetMessageKind::Normal);
        drop(alice);

        let mut bob = RatchetState::load_or_create(&bob_dir)?;
        let (plaintext, subsequent_receive_op) =
            bob.decrypt(&bob_identity, &alice_ratchet_identity, &subsequent)?;
        assert_eq!(plaintext.as_str(), "subsequent");
        assert!(!subsequent_receive_op.session_created);
        assert_eq!(subsequent_receive_op.session_id, first_op.session_id);
        Ok(())
    }

    #[test]
    fn bundle_is_device_signed_and_tampering_is_rejected() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempdir()?;
        let identity = DeviceIdentity::generate()?;
        let bundle = RatchetState::load_or_create(directory.path())?.prekey_bundle(&identity)?;
        bundle.verify()?;
        let mut encoded = bundle.encode()?;
        let last = encoded.len().checked_sub(1).ok_or("empty bundle")?;
        encoded[last] ^= 1;
        assert!(SignedPrekeyBundle::decode(&encoded).is_err());
        Ok(())
    }

    #[test]
    fn signed_prekey_pool_has_freshness_contiguous_sequences_and_stable_selection()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let identity = DeviceIdentity::generate()?;
        let initiator = DeviceIdentity::generate()?;
        let now = 2_000_000_000;
        let mut state = RatchetState::load_or_create(directory.path())?;
        let first = state.prekey_pool(&identity, 8, now, 3_600)?;
        first.verify_at(now)?;
        assert_eq!(first.generation(), 0);
        assert_eq!(first.bundles().len(), 8);
        assert_eq!(first.first_sequence(), 0);
        assert_eq!(first.last_sequence(), 7);
        assert_eq!(
            first.select_for(initiator.device_id())?,
            first.select_for(initiator.device_id())?
        );
        assert!(matches!(
            first.verify_at(now + 3_600 + PREKEY_CLOCK_SKEW_SECONDS + 1),
            Err(RatchetError::PrekeyPoolExpired { .. })
        ));

        let second = state.prekey_pool(
            &identity,
            8,
            now + 3_600 + PREKEY_CLOCK_SKEW_SECONDS + 1,
            3_600,
        )?;
        assert_eq!(second.generation(), 1);
        assert_eq!(second.first_sequence(), 8);
        assert_eq!(second.last_sequence(), 15);
        assert_ne!(first.pool_id()?, second.pool_id()?);
        assert_eq!(SignedPrekeyPool::decode(&second.encode()?)?, second);

        let mut tampered = second.encode()?;
        let last = tampered.len().checked_sub(1).ok_or("empty pool")?;
        tampered[last] ^= 1;
        assert!(SignedPrekeyPool::decode(&tampered).is_err());
        Ok(())
    }

    #[test]
    fn peer_prekey_high_water_rejects_rollback_and_equivocation()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempdir()?;
        let alice_identity = DeviceIdentity::generate()?;
        let bob_identity = DeviceIdentity::generate()?;
        let now = 2_000_000_000;
        let mut bob = RatchetState::load_or_create(root.path().join("bob"))?;
        let old_pool = bob.prekey_pool(&bob_identity, 4, now, 3_600)?;
        let new_pool = bob.refresh_prekey_pool(&bob_identity, 4, now + 1, 3_600)?;
        let mut alice = RatchetState::load_or_create(root.path().join("alice"))?;
        let _ = alice.encrypt_with_pool(&alice_identity, &new_pool, "observe newest", now + 1)?;
        assert!(matches!(
            alice.encrypt_with_pool(&alice_identity, &old_pool, "rollback", now + 1),
            Err(RatchetError::PrekeyPoolGenerationRollback { .. })
        ));

        let equivocation = SignedPrekeyPool::sign(
            &bob_identity,
            old_pool.ratchet_identity().clone(),
            new_pool.generation(),
            new_pool.published_at_unix_seconds(),
            new_pool.expires_at_unix_seconds(),
            old_pool.bundles().to_vec(),
        )?;
        assert!(matches!(
            alice.encrypt_with_pool(&alice_identity, &equivocation, "equivocation", now + 1),
            Err(RatchetError::PrekeyPoolEquivocation { .. })
        ));
        Ok(())
    }

    #[test]
    fn simultaneous_outbound_sessions_converge_on_one_active_session()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempdir()?;
        let alice_dir = root.path().join("alice");
        let bob_dir = root.path().join("bob");
        let alice_identity = DeviceIdentity::generate()?;
        let bob_identity = DeviceIdentity::generate()?;
        let now = 2_000_000_000;
        let alice_pool = RatchetState::load_or_create(&alice_dir)?.prekey_pool(
            &alice_identity,
            4,
            now,
            3_600,
        )?;
        let bob_pool =
            RatchetState::load_or_create(&bob_dir)?.prekey_pool(&bob_identity, 4, now, 3_600)?;

        let (alice_ratchet_identity, alice_first, alice_send) = RatchetState::load_or_create(
            &alice_dir,
        )?
        .encrypt_with_pool(&alice_identity, &bob_pool, "alice crossed first", now)?;
        let (bob_ratchet_identity, bob_first, bob_send) = RatchetState::load_or_create(&bob_dir)?
            .encrypt_with_pool(
            &bob_identity,
            &alice_pool,
            "bob crossed first",
            now,
        )?;
        assert_ne!(alice_send.session_id, bob_send.session_id);

        let (bob_plaintext, bob_receive) = RatchetState::load_or_create(&bob_dir)?.decrypt(
            &bob_identity,
            &alice_ratchet_identity,
            &alice_first,
        )?;
        let (alice_plaintext, alice_receive) = RatchetState::load_or_create(&alice_dir)?.decrypt(
            &alice_identity,
            &bob_ratchet_identity,
            &bob_first,
        )?;
        assert_eq!(bob_plaintext.as_str(), "alice crossed first");
        assert_eq!(alice_plaintext.as_str(), "bob crossed first");
        assert!(bob_receive.concurrent_session_resolved);
        assert!(alice_receive.concurrent_session_resolved);
        assert_eq!(bob_receive.retained_session_count, 1);
        assert_eq!(alice_receive.retained_session_count, 1);

        let alice_active = RatchetState::load_or_create(&alice_dir)?
            .load_session_set(bob_identity.device_id())?
            .ok_or("missing alice session set")?
            .active_session_id;
        let bob_active = RatchetState::load_or_create(&bob_dir)?
            .load_session_set(alice_identity.device_id())?
            .ok_or("missing bob session set")?
            .active_session_id;
        assert_eq!(alice_active, bob_active);
        assert_eq!(alice_active, alice_send.session_id.min(bob_send.session_id));

        let (alice_ratchet_identity, next, next_send) = RatchetState::load_or_create(&alice_dir)?
            .encrypt_with_pool(
            &alice_identity,
            &bob_pool,
            "after convergence",
            now,
        )?;
        assert_eq!(next_send.session_id, alice_active);
        let (plaintext, next_receive) = RatchetState::load_or_create(&bob_dir)?.decrypt(
            &bob_identity,
            &alice_ratchet_identity,
            &next,
        )?;
        assert_eq!(plaintext.as_str(), "after convergence");
        assert_eq!(next_receive.session_id, alice_active);
        Ok(())
    }

    #[test]
    fn legacy_single_session_record_is_read_and_rewritten_as_v2()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempdir()?;
        let alice_dir = root.path().join("alice");
        let bob_dir = root.path().join("bob");
        let alice_identity = DeviceIdentity::generate()?;
        let bob_identity = DeviceIdentity::generate()?;
        let bob_bundle = RatchetState::load_or_create(&bob_dir)?.prekey_bundle(&bob_identity)?;
        let alice = RatchetState::load_or_create(&alice_dir)?;
        let peer_curve_key = bob_bundle.ratchet_identity().curve25519_key();
        let session = alice.account.create_outbound_session(
            SessionConfig::version_1(),
            peer_curve_key,
            bob_bundle.one_time_key(),
        )?;
        let legacy = LegacySessionRecord {
            version: LEGACY_SESSION_RECORD_VERSION,
            peer_device_id: bob_identity.device_id(),
            peer_curve25519_key: peer_curve_key.to_bytes(),
            encrypted_pickle: session.pickle().encrypt(&alice.pickle_secret),
        };
        atomic_write(
            &alice.session_path(bob_identity.device_id()),
            &postcard::to_allocvec(&legacy)?,
        )?;
        drop(alice);

        let (_, _, operation) = RatchetState::load_or_create(&alice_dir)?.encrypt(
            &alice_identity,
            &bob_bundle,
            "migrate legacy session",
        )?;
        assert!(!operation.session_created);
        assert_eq!(
            fs::read(
                RatchetState::load_or_create(&alice_dir)?.session_path(bob_identity.device_id())
            )?
            .first()
            .copied(),
            Some(SESSION_RECORD_VERSION)
        );
        Ok(())
    }

    #[test]
    fn existing_ratchet_state_cannot_be_rebound_to_another_device()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let original = DeviceIdentity::generate()?;
        let replacement = DeviceIdentity::generate()?;
        RatchetState::load_or_create(directory.path())?.prekey_bundle(&original)?;

        let error =
            match RatchetState::load_or_create(directory.path())?.prekey_bundle(&replacement) {
                Ok(_) => return Err("ratchet state was rebound to another device".into()),
                Err(error) => error,
            };
        assert!(matches!(
            error,
            RatchetError::PrekeyBundleDeviceMismatch { expected, actual }
                if expected == original.device_id() && actual == replacement.device_id()
        ));
        Ok(())
    }

    #[test]
    fn prekey_directory_requires_exact_root_signed_device_list_coverage()
    -> Result<(), Box<dyn std::error::Error>> {
        let root_directory = tempdir()?;
        let state_root = tempdir()?;
        let root = AccountRootState::create(root_directory.path())?;
        let first = DeviceIdentity::generate()?;
        let second = DeviceIdentity::generate()?;
        let first_certificate = root.issue_device_certificate(
            first.device_id(),
            DeviceEncryptionIdentity::generate()?.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let second_certificate = root.issue_device_certificate(
            second.device_id(),
            DeviceEncryptionIdentity::generate()?.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let device_list =
            root.publish_device_list(&[second_certificate.clone(), first_certificate.clone()])?;
        let now = 2_000_000_000;
        let first_pool = RatchetState::load_or_create(state_root.path().join("first"))?
            .prekey_pool(&first, 4, now, DEFAULT_PREKEY_POOL_VALIDITY_SECONDS)?;
        let second_pool = RatchetState::load_or_create(state_root.path().join("second"))?
            .prekey_pool(&second, 4, now, DEFAULT_PREKEY_POOL_VALIDITY_SECONDS)?;

        let directory = AccountPrekeyDirectory::new(
            device_list.clone(),
            vec![second_pool.clone(), first_pool.clone()],
        )?;
        assert_eq!(directory.account_id(), root.account_id());
        assert_eq!(directory.revision(), 2);
        assert_eq!(directory.pools().len(), 2);
        assert_eq!(directory.pool_for(first.device_id()), Some(&first_pool));
        directory.verify_at(now)?;
        assert_eq!(
            AccountPrekeyDirectory::decode(&directory.encode()?)?,
            directory
        );
        assert!(matches!(
            AccountPrekeyDirectory::new(device_list, vec![second_pool]),
            Err(RatchetError::PrekeyDirectoryCoverage {
                devices: 2,
                pools: 1
            })
        ));
        Ok(())
    }

    #[test]
    fn account_session_and_bundle_survive_reload_without_plaintext_on_disk()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempdir()?;
        let alice_dir = root.path().join("alice");
        let bob_dir = root.path().join("bob");
        let alice_identity = DeviceIdentity::generate()?;
        let bob_identity = DeviceIdentity::generate()?;
        let mut bob = RatchetState::load_or_create(&bob_dir)?;
        let bob_bundle = bob.prekey_bundle(&bob_identity)?;
        let bob_curve_key = bob_bundle.ratchet_identity().curve25519_key_bytes();
        drop(bob);

        let secret_text = "ratchet plaintext must not be persisted";
        let mut alice = RatchetState::load_or_create(&alice_dir)?;
        let (_, ciphertext, _) = alice.encrypt(&alice_identity, &bob_bundle, secret_text)?;
        drop(alice);
        let alice_reloaded = RatchetState::load_or_create(&alice_dir)?;
        assert!(alice_reloaded.has_session(bob_identity.device_id()));
        assert_eq!(
            RatchetState::load_or_create(&bob_dir)?
                .prekey_bundle(&bob_identity)?
                .ratchet_identity()
                .curve25519_key_bytes(),
            bob_curve_key
        );

        for entry in walk_files(&alice_dir)? {
            let bytes = fs::read(entry)?;
            assert!(
                !bytes
                    .windows(secret_text.len())
                    .any(|window| window == secret_text.as_bytes())
            );
        }
        assert!(!ciphertext.ciphertext().is_empty());
        Ok(())
    }

    #[test]
    fn peer_retirement_removes_session_and_prekey_observation_idempotently()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempdir()?;
        let local_dir = root.path().join("local");
        let peer_dir = root.path().join("peer");
        let local = DeviceIdentity::generate()?;
        let peer = DeviceIdentity::generate()?;
        let now = 2_000_000_000;
        let peer_pool = RatchetState::load_or_create(&peer_dir)?.prekey_pool(
            &peer,
            4,
            now,
            DEFAULT_PREKEY_POOL_VALIDITY_SECONDS,
        )?;
        let mut state = RatchetState::load_or_create(&local_dir)?;
        state.observe_prekey_pool(&peer_pool, now)?;
        state.encrypt_with_pool(&local, &peer_pool, "create session", now)?;
        assert!(state.has_session(peer.device_id()));
        assert!(state.peer_prekey_pool_path(peer.device_id()).is_file());

        assert_eq!(
            state.retire_peer_device(peer.device_id())?,
            RatchetRetirement {
                session_removed: true,
                prekey_observation_removed: true,
            }
        );
        assert!(!state.has_session(peer.device_id()));
        assert!(!state.peer_prekey_pool_path(peer.device_id()).exists());
        assert_eq!(
            state.retire_peer_device(peer.device_id())?,
            RatchetRetirement::default()
        );
        Ok(())
    }

    fn walk_files(directory: &Path) -> Result<Vec<PathBuf>, io::Error> {
        let mut files = Vec::new();
        let mut pending = vec![directory.to_path_buf()];
        while let Some(next) = pending.pop() {
            for entry in fs::read_dir(next)? {
                let path = entry?.path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    files.push(path);
                }
            }
        }
        Ok(files)
    }
}
