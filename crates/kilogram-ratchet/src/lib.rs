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
const SESSION_FILE_SUFFIX: &str = ".session";
const PICKLE_SECRET_BYTES: usize = 32;
const RATCHET_IDENTITY_VERSION: u8 = 1;
const PREKEY_BUNDLE_VERSION: u8 = 1;
const PREKEY_DIRECTORY_VERSION: u8 = 1;
const CIPHERTEXT_VERSION: u8 = 1;
const SESSION_RECORD_VERSION: u8 = 1;
const RATCHET_IDENTITY_SIGNATURE_DOMAIN: &[u8] = b"kilogram:ratchet-identity-signature:v1\0";
const PREKEY_BUNDLE_SIGNATURE_DOMAIN: &[u8] = b"kilogram:prekey-bundle-signature:v1\0";
const MAX_RATCHET_CIPHERTEXT_BYTES: usize = 128 * 1024;
const MAX_PLAINTEXT_BYTES: usize = 64 * 1024;

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

/// A root-complete account device list paired with one device-signed prekey
/// bundle for every authorized device.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountPrekeyDirectory {
    version: u8,
    device_list: AccountDeviceListSnapshot,
    bundles: Vec<SignedPrekeyBundle>,
}

impl AccountPrekeyDirectory {
    pub fn new(
        device_list: AccountDeviceListSnapshot,
        mut bundles: Vec<SignedPrekeyBundle>,
    ) -> Result<Self, RatchetError> {
        bundles.sort_by_key(|bundle| *bundle.device_id().as_bytes());
        let directory = Self {
            version: PREKEY_DIRECTORY_VERSION,
            device_list,
            bundles,
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
        if self.bundles.len() != self.device_list.devices().len() {
            return Err(RatchetError::PrekeyDirectoryCoverage {
                devices: self.device_list.devices().len(),
                bundles: self.bundles.len(),
            });
        }
        for (certificate, bundle) in self.device_list.devices().iter().zip(&self.bundles) {
            bundle.verify()?;
            if certificate.device_id() != bundle.device_id() {
                return Err(RatchetError::PrekeyDirectoryDeviceMismatch {
                    certificate: certificate.device_id(),
                    bundle: bundle.device_id(),
                });
            }
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

    pub fn bundles(&self) -> &[SignedPrekeyBundle] {
        &self.bundles
    }

    pub fn certificate_for(&self, device_id: DeviceId) -> Option<&DeviceCertificate> {
        self.device_list.certificate_for(device_id)
    }

    pub fn bundle_for(&self, device_id: DeviceId) -> Option<&SignedPrekeyBundle> {
        self.bundles
            .binary_search_by(|bundle| bundle.device_id().cmp(&device_id))
            .ok()
            .map(|index| &self.bundles[index])
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
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct SessionRecord {
    version: u8,
    peer_device_id: DeviceId,
    peer_curve25519_key: [u8; 32],
    encrypted_pickle: String,
}

/// Persistent Olm account and one pairwise session per peer device.
///
/// M0.7.3 assumes exclusive access to a device state directory. A production
/// client will put these records in the same transactional database as event
/// acceptance and local projections.
pub struct RatchetState {
    directory: PathBuf,
    sessions_directory: PathBuf,
    pickle_secret: Zeroizing<[u8; PICKLE_SECRET_BYTES]>,
    account: Account,
    current_bundle: Option<SignedPrekeyBundle>,
}

impl RatchetState {
    pub fn load_or_create(state_directory: impl AsRef<Path>) -> Result<Self, RatchetError> {
        let directory = state_directory.as_ref().join(STATE_DIRECTORY);
        let sessions_directory = directory.join(SESSIONS_DIRECTORY);
        fs::create_dir_all(&sessions_directory)?;
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

        Ok(Self {
            directory,
            sessions_directory,
            pickle_secret,
            account,
            current_bundle,
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

    pub fn has_session(&self, peer_device_id: DeviceId) -> bool {
        self.session_path(peer_device_id).is_file()
    }

    pub fn encrypt(
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
        let (mut session, session_created) = match self.load_session(peer_device_id)? {
            Some((session, stored_peer_key)) => {
                ensure_stored_peer_key(stored_peer_key, peer_curve_key, peer_device_id)?;
                (session, false)
            }
            None => (
                self.account.create_outbound_session(
                    SessionConfig::version_1(),
                    peer_curve_key,
                    peer_bundle.one_time_key(),
                )?,
                true,
            ),
        };
        let message = session.encrypt(plaintext.as_bytes())?;
        let ciphertext = RatchetCiphertext::from_olm(&message)?;
        let operation = RatchetOperation {
            session_id: session.session_id(),
            session_created,
            message_kind: ciphertext.kind()?,
        };
        self.persist_session(peer_device_id, peer_curve_key, &session)?;
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
        let (session, plaintext, session_created) = match self.load_session(peer_device_id)? {
            Some((mut session, stored_peer_key)) => {
                ensure_stored_peer_key(stored_peer_key, peer_curve_key, peer_device_id)?;
                let plaintext = session.decrypt(&message)?;
                (session, plaintext, false)
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
                (result.session, result.plaintext, true)
            }
        };
        let body = String::from_utf8(plaintext)?;
        validate_plaintext(&body)?;
        let operation = RatchetOperation {
            session_id: session.session_id(),
            session_created,
            message_kind: ciphertext.kind()?,
        };
        self.persist_session(peer_device_id, peer_curve_key, &session)?;
        if session_created {
            persist_account(
                &self.directory.join(ACCOUNT_FILE),
                &self.account,
                &self.pickle_secret,
            )?;
            let _ = self.create_new_prekey_bundle(identity)?;
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

    fn load_session(
        &self,
        peer_device_id: DeviceId,
    ) -> Result<Option<(Session, Curve25519PublicKey)>, RatchetError> {
        let path = self.session_path(peer_device_id);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let record: SessionRecord = postcard::from_bytes(&bytes)?;
        if record.version != SESSION_RECORD_VERSION {
            return Err(RatchetError::UnsupportedSessionRecordVersion(
                record.version,
            ));
        }
        if record.peer_device_id != peer_device_id {
            return Err(RatchetError::SessionFileNameMismatch {
                expected: peer_device_id,
                actual: record.peer_device_id,
            });
        }
        let peer_curve25519_key = Curve25519PublicKey::from_bytes(record.peer_curve25519_key);
        let session = Session::from_pickle(SessionPickle::from_encrypted(
            &record.encrypted_pickle,
            &self.pickle_secret,
        )?);
        Ok(Some((session, peer_curve25519_key)))
    }

    fn persist_session(
        &self,
        peer_device_id: DeviceId,
        peer_curve25519_key: Curve25519PublicKey,
        session: &Session,
    ) -> Result<(), RatchetError> {
        let record = SessionRecord {
            version: SESSION_RECORD_VERSION,
            peer_device_id,
            peer_curve25519_key: peer_curve25519_key.to_bytes(),
            encrypted_pickle: session.pickle().encrypt(&self.pickle_secret),
        };
        atomic_write(
            &self.session_path(peer_device_id),
            &postcard::to_allocvec(&record)?,
        )
    }

    fn session_path(&self, peer_device_id: DeviceId) -> PathBuf {
        self.sessions_directory
            .join(format!("{peer_device_id}{SESSION_FILE_SUFFIX}"))
    }
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

    #[error("unsupported account prekey-directory version: {0}")]
    UnsupportedPrekeyDirectoryVersion(u8),

    #[error("prekey directory contains {bundles} bundles for {devices} devices")]
    PrekeyDirectoryCoverage { devices: usize, bundles: usize },

    #[error(
        "prekey directory certificate names device {certificate}, but aligned bundle names {bundle}"
    )]
    PrekeyDirectoryDeviceMismatch {
        certificate: DeviceId,
        bundle: DeviceId,
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

    #[error("Olm did not produce a one-time key")]
    PrekeyGenerationFailed,

    #[error("refusing to create a ratchet session with local device {0}")]
    SelfSession(DeviceId),

    #[error("peer {0} changed its signed ratchet identity key")]
    PeerRatchetIdentityChanged(DeviceId),

    #[error("normal Olm message arrived before a session existed for peer {0}")]
    NormalMessageWithoutSession(DeviceId),

    #[error("ratchet session file names peer {actual}; expected {expected}")]
    SessionFileNameMismatch {
        expected: DeviceId,
        actual: DeviceId,
    },

    #[error("persistent path has no parent: {0}")]
    PathHasNoParent(PathBuf),
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
        let first_bundle =
            RatchetState::load_or_create(state_root.path().join("first"))?.prekey_bundle(&first)?;
        let second_bundle = RatchetState::load_or_create(state_root.path().join("second"))?
            .prekey_bundle(&second)?;

        let directory = AccountPrekeyDirectory::new(
            device_list.clone(),
            vec![second_bundle.clone(), first_bundle.clone()],
        )?;
        assert_eq!(directory.account_id(), root.account_id());
        assert_eq!(directory.revision(), 2);
        assert_eq!(directory.bundles().len(), 2);
        assert_eq!(directory.bundle_for(first.device_id()), Some(&first_bundle));
        assert_eq!(
            AccountPrekeyDirectory::decode(&directory.encode()?)?,
            directory
        );
        assert!(matches!(
            AccountPrekeyDirectory::new(device_list, vec![second_bundle]),
            Err(RatchetError::PrekeyDirectoryCoverage {
                devices: 2,
                bundles: 1
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
