use std::{
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    str::FromStr,
};

use bip39::{Language, Mnemonic};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use kilogram_crypto::EncryptionPublicKey;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::{DeviceId, DeviceState, IdentityError, SECRET_KEY_BYTES, open_new_secret_file};

const ACCOUNT_ROOT_SECRET_FILE: &str = "account-root-secret.key";
const ACCOUNT_ROOT_KEY_ENVELOPE_MAGIC: &[u8; 16] = b"KILOGRAM-ROOTK01";
const ACCOUNT_ROOT_KEY_ENVELOPE_VERSION: u8 = 1;
const MAX_ACCOUNT_ROOT_KEY_ENVELOPE_BYTES: usize = 64 * 1024;
const ACCOUNT_ROOT_DERIVATION_CONTEXT: &str = "Kilogram Account Root signing key v1";
const NEXT_AUTHORITY_SEQUENCE_FILE: &str = "next-authority-sequence";
const AUTHORITY_LOG_VERSION_FILE: &str = "authority-log-version";
const AUTHORITY_LOG_VERSION: &str = "1";
const REVOCATIONS_DIRECTORY: &str = "revocations";
const CONVERSATION_MEMBERSHIPS_DIRECTORY: &str = "conversation-memberships";
const DEVICE_CERTIFICATE_FILE: &str = "device-certificate.cert";
const ACCOUNT_AUTHORITY_SNAPSHOT_FILE: &str = "account-authority.snapshot";
const ACCOUNT_DEVICE_LIST_FILE: &str = "account-device-list.snapshot";
const AUTHORITY_WRITE_LOCK_FILE: &str = "authority-write.lock";
const PEER_AUTHORITY_DIRECTORY: &str = "peer-authority";
const AUTHORITY_VERSION: u8 = 1;
const DEVICE_CERTIFICATE_VERSION: u8 = 2;
const DEVICE_CERTIFICATE_SIGNATURE_DOMAIN: &[u8] = b"kilogram:device-certificate-signature:v2\0";
const DEVICE_REVOCATION_SIGNATURE_DOMAIN: &[u8] = b"kilogram:device-revocation-signature:v1\0";
const AUTHORITY_SNAPSHOT_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:account-authority-snapshot-signature:v1\0";
const CONVERSATION_MEMBERSHIP_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:conversation-membership-signature:v1\0";
const ACCOUNT_DEVICE_LIST_VERSION: u8 = 1;
const ACCOUNT_DEVICE_LIST_SIGNATURE_DOMAIN: &[u8] = b"kilogram:account-device-list-signature:v1\0";
const DEVICE_LINK_AUTHORIZATION_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:device-link-authorization-signature:v1\0";
const ACCOUNT_ROOT_RECOVERY_PACKAGE_MAGIC: &[u8; 16] = b"KILOGRAM-ARPKG01";
const ACCOUNT_ROOT_RECOVERY_WITNESS_MAGIC: &[u8; 16] = b"KILOGRAM-ARWIT01";
const ACCOUNT_ROOT_RECOVERY_VERSION: u8 = 1;
const ACCOUNT_ROOT_RECOVERY_PACKAGE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:account-root-recovery-package-signature:v1\0";
const ACCOUNT_ROOT_RECOVERY_WITNESS_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:account-root-recovery-witness-signature:v1\0";
const ACCOUNT_ROOT_RECOVERY_PACKAGE_ID_DOMAIN: &str =
    "Kilogram Account Root recovery package ID v1";

pub const MAX_ACCOUNT_ROOT_RECOVERY_PACKAGE_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_ACCOUNT_ROOT_RECOVERY_WITNESS_BYTES: usize = 4 * 1024;
pub const MAX_ACCOUNT_ROOT_RECOVERY_MEMBERSHIPS: usize = 16 * 1024;

pub const MAX_ACCOUNT_DEVICES: usize = 32;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct AccountId([u8; SECRET_KEY_BYTES]);

impl AccountId {
    pub fn from_bytes(bytes: [u8; SECRET_KEY_BYTES]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; SECRET_KEY_BYTES] {
        &self.0
    }

    fn verify(&self, message: &[u8], signature: &[u8]) -> Result<(), IdentityError> {
        let verifying_key =
            VerifyingKey::from_bytes(&self.0).map_err(IdentityError::InvalidAccountPublicKey)?;
        let signature =
            Signature::try_from(signature).map_err(IdentityError::InvalidSignatureEncoding)?;
        verifying_key
            .verify_strict(message, &signature)
            .map_err(IdentityError::InvalidSignature)
    }

    /// Verifies an Account Root signature over a canonical device-link
    /// authorization payload. The fixed domain prevents this narrow API from
    /// becoming a generic Root signature oracle.
    pub fn verify_device_link_authorization(
        &self,
        payload: &[u8],
        signature: &[u8],
    ) -> Result<(), IdentityError> {
        let mut message =
            Vec::with_capacity(DEVICE_LINK_AUTHORIZATION_SIGNATURE_DOMAIN.len() + payload.len());
        message.extend_from_slice(DEVICE_LINK_AUTHORIZATION_SIGNATURE_DOMAIN);
        message.extend_from_slice(payload);
        self.verify(&message, signature)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct ConversationScopeId([u8; 32]);

impl ConversationScopeId {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ConversationScopeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Display for AccountId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for AccountId {
    type Err = IdentityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != SECRET_KEY_BYTES * 2 {
            return Err(IdentityError::InvalidAccountIdLength(value.len()));
        }
        let mut bytes = [0_u8; SECRET_KEY_BYTES];
        let encoded = value.as_bytes();
        for (index, byte) in bytes.iter_mut().enumerate() {
            let offset = index * 2;
            let high = decode_account_hex_nibble(encoded[offset], offset)?;
            let low = decode_account_hex_nibble(encoded[offset + 1], offset + 1)?;
            *byte = (high << 4) | low;
        }
        Ok(Self(bytes))
    }
}

fn decode_account_hex_nibble(byte: u8, index: usize) -> Result<u8, IdentityError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(IdentityError::InvalidAccountIdHex(index)),
    }
}

struct AccountRootIdentity {
    signing_key: SigningKey,
}

impl AccountRootIdentity {
    fn generate() -> Result<Self, IdentityError> {
        let mut secret = [0_u8; SECRET_KEY_BYTES];
        getrandom::fill(&mut secret).map_err(IdentityError::SecureRandom)?;
        Ok(Self::from_secret_bytes(secret))
    }

    fn from_secret_bytes(mut secret: [u8; SECRET_KEY_BYTES]) -> Self {
        let identity = Self {
            signing_key: SigningKey::from_bytes(&secret),
        };
        secret.zeroize();
        identity
    }

    fn account_id(&self) -> AccountId {
        AccountId(self.signing_key.verifying_key().to_bytes())
    }

    fn secret_bytes(&self) -> [u8; SECRET_KEY_BYTES] {
        self.signing_key.to_bytes()
    }

    fn sign(&self, message: &[u8]) -> [u8; 64] {
        self.signing_key.sign(message).to_bytes()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountRootKeyProtection {
    WindowsDpapiCurrentUser,
    PlaintextDevelopment,
}

impl AccountRootKeyProtection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WindowsDpapiCurrentUser => "windows-dpapi-current-user",
            Self::PlaintextDevelopment => "plaintext-development",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountRootKeyLoadOutcome {
    Created,
    LegacyMigrated,
    AlreadyCurrent,
}

impl AccountRootKeyLoadOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::LegacyMigrated => "legacy-migrated",
            Self::AlreadyCurrent => "already-current",
        }
    }
}

pub struct AccountRecoveryPhrase(Zeroizing<String>);

impl AccountRecoveryPhrase {
    pub fn parse(value: &str) -> Result<Self, IdentityError> {
        let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
        let mnemonic = Mnemonic::parse_in_normalized(Language::English, &normalized)
            .map_err(|error| IdentityError::InvalidAccountRecoveryPhrase(error.to_string()))?;
        if mnemonic.word_count() != 24 {
            return Err(IdentityError::InvalidAccountRecoveryWordCount(
                mnemonic.word_count(),
            ));
        }
        Ok(Self(Zeroizing::new(mnemonic.to_string())))
    }

    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    pub fn account_id(&self) -> Result<AccountId, IdentityError> {
        Ok(account_root_identity_from_phrase(self)?.account_id())
    }
}

impl fmt::Debug for AccountRecoveryPhrase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AccountRecoveryPhrase([REDACTED])")
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct AccountRootRecoveryPackageContent {
    version: u8,
    account_id: AccountId,
    authority_snapshot: AccountAuthoritySnapshot,
    device_list: AccountDeviceListSnapshot,
    conversation_memberships: Vec<ConversationMembershipSnapshot>,
}

/// A Root-signed, portable checkpoint of every authority head required to
/// continue device enrollment and conversation membership updates.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountRootRecoveryPackage {
    content: AccountRootRecoveryPackageContent,
    signature: Vec<u8>,
}

impl AccountRootRecoveryPackage {
    fn issue(
        root: &AccountRootIdentity,
        authority_snapshot: AccountAuthoritySnapshot,
        device_list: AccountDeviceListSnapshot,
        conversation_memberships: Vec<ConversationMembershipSnapshot>,
    ) -> Result<Self, IdentityError> {
        let content = AccountRootRecoveryPackageContent {
            version: ACCOUNT_ROOT_RECOVERY_VERSION,
            account_id: root.account_id(),
            authority_snapshot,
            device_list,
            conversation_memberships,
        };
        validate_account_root_recovery_package_content(&content)?;
        let signature = root
            .sign(&account_root_recovery_package_signing_bytes(&content)?)
            .to_vec();
        Ok(Self { content, signature })
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, IdentityError> {
        if bytes.len() > MAX_ACCOUNT_ROOT_RECOVERY_PACKAGE_BYTES {
            return Err(IdentityError::AccountRootRecoveryPackageTooLarge(
                bytes.len(),
            ));
        }
        let payload = bytes
            .strip_prefix(ACCOUNT_ROOT_RECOVERY_PACKAGE_MAGIC)
            .ok_or(IdentityError::InvalidAccountRootRecoveryPackageMagic)?;
        let package: Self = postcard::from_bytes(payload)?;
        package.verify()?;
        Ok(package)
    }

    pub fn encode(&self) -> Result<Vec<u8>, IdentityError> {
        self.verify()?;
        let payload = postcard::to_allocvec(self)?;
        let total_len = ACCOUNT_ROOT_RECOVERY_PACKAGE_MAGIC.len() + payload.len();
        if total_len > MAX_ACCOUNT_ROOT_RECOVERY_PACKAGE_BYTES {
            return Err(IdentityError::AccountRootRecoveryPackageTooLarge(total_len));
        }
        let mut encoded = Vec::with_capacity(total_len);
        encoded.extend_from_slice(ACCOUNT_ROOT_RECOVERY_PACKAGE_MAGIC);
        encoded.extend_from_slice(&payload);
        Ok(encoded)
    }

    pub fn verify(&self) -> Result<(), IdentityError> {
        validate_account_root_recovery_package_content(&self.content)?;
        self.content.account_id.verify(
            &account_root_recovery_package_signing_bytes(&self.content)?,
            &self.signature,
        )
    }

    pub fn account_id(&self) -> AccountId {
        self.content.account_id
    }

    pub fn authority_revision(&self) -> u64 {
        self.content.authority_snapshot.revision()
    }

    pub fn device_count(&self) -> usize {
        self.content.device_list.devices().len()
    }

    pub fn conversation_membership_count(&self) -> usize {
        self.content.conversation_memberships.len()
    }

    pub fn package_id(&self) -> Result<[u8; 32], IdentityError> {
        Ok(blake3::derive_key(
            ACCOUNT_ROOT_RECOVERY_PACKAGE_ID_DOMAIN,
            &self.encode()?,
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct AccountRootRecoveryWitnessContent {
    version: u8,
    account_id: AccountId,
    authority_revision: u64,
    package_id: [u8; 32],
}

/// A small latest-known checkpoint which must be retained independently from
/// the recovery package whose digest it pins.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountRootRecoveryWitness {
    content: AccountRootRecoveryWitnessContent,
    signature: Vec<u8>,
}

impl AccountRootRecoveryWitness {
    fn issue(
        root: &AccountRootIdentity,
        package: &AccountRootRecoveryPackage,
    ) -> Result<Self, IdentityError> {
        let content = AccountRootRecoveryWitnessContent {
            version: ACCOUNT_ROOT_RECOVERY_VERSION,
            account_id: package.account_id(),
            authority_revision: package.authority_revision(),
            package_id: package.package_id()?,
        };
        let signature = root
            .sign(&account_root_recovery_witness_signing_bytes(&content)?)
            .to_vec();
        Ok(Self { content, signature })
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, IdentityError> {
        if bytes.len() > MAX_ACCOUNT_ROOT_RECOVERY_WITNESS_BYTES {
            return Err(IdentityError::AccountRootRecoveryWitnessTooLarge(
                bytes.len(),
            ));
        }
        let payload = bytes
            .strip_prefix(ACCOUNT_ROOT_RECOVERY_WITNESS_MAGIC)
            .ok_or(IdentityError::InvalidAccountRootRecoveryWitnessMagic)?;
        let witness: Self = postcard::from_bytes(payload)?;
        witness.verify()?;
        Ok(witness)
    }

    pub fn encode(&self) -> Result<Vec<u8>, IdentityError> {
        self.verify()?;
        let payload = postcard::to_allocvec(self)?;
        let total_len = ACCOUNT_ROOT_RECOVERY_WITNESS_MAGIC.len() + payload.len();
        if total_len > MAX_ACCOUNT_ROOT_RECOVERY_WITNESS_BYTES {
            return Err(IdentityError::AccountRootRecoveryWitnessTooLarge(total_len));
        }
        let mut encoded = Vec::with_capacity(total_len);
        encoded.extend_from_slice(ACCOUNT_ROOT_RECOVERY_WITNESS_MAGIC);
        encoded.extend_from_slice(&payload);
        Ok(encoded)
    }

    pub fn verify(&self) -> Result<(), IdentityError> {
        if self.content.version != ACCOUNT_ROOT_RECOVERY_VERSION {
            return Err(IdentityError::UnsupportedAccountRootRecoveryVersion(
                self.content.version,
            ));
        }
        self.content.account_id.verify(
            &account_root_recovery_witness_signing_bytes(&self.content)?,
            &self.signature,
        )
    }

    pub fn verify_package(
        &self,
        package: &AccountRootRecoveryPackage,
    ) -> Result<(), IdentityError> {
        self.verify()?;
        package.verify()?;
        let package_id = package.package_id()?;
        if self.content.account_id != package.account_id()
            || self.content.authority_revision != package.authority_revision()
            || self.content.package_id != package_id
        {
            return Err(IdentityError::AccountRootRecoveryWitnessMismatch);
        }
        Ok(())
    }

    pub fn account_id(&self) -> AccountId {
        self.content.account_id
    }

    pub fn authority_revision(&self) -> u64 {
        self.content.authority_revision
    }

    pub fn package_id(&self) -> &[u8; 32] {
        &self.content.package_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum AccountRootKeyProviderId {
    WindowsDpapiCurrentUser,
    PlaintextDevelopment,
}

impl AccountRootKeyProviderId {
    fn as_str(self) -> &'static str {
        match self {
            Self::WindowsDpapiCurrentUser => "windows-dpapi-current-user",
            Self::PlaintextDevelopment => "plaintext-development",
        }
    }
}

#[derive(Serialize, Deserialize, ZeroizeOnDrop)]
struct AccountRootKeyEnvelope {
    #[zeroize(skip)]
    version: u8,
    #[zeroize(skip)]
    provider: AccountRootKeyProviderId,
    protected_key: Vec<u8>,
}

struct LoadedAccountRootKey {
    identity: AccountRootIdentity,
    protection: AccountRootKeyProtection,
    load_outcome: AccountRootKeyLoadOutcome,
}

pub struct AccountRootState {
    directory: PathBuf,
    identity: AccountRootIdentity,
    key_protection: AccountRootKeyProtection,
    key_load_outcome: AccountRootKeyLoadOutcome,
}

impl AccountRootState {
    pub fn create(directory: impl AsRef<Path>) -> Result<Self, IdentityError> {
        Self::create_with_identity(directory, AccountRootIdentity::generate()?)
    }

    pub fn create_recoverable(
        directory: impl AsRef<Path>,
    ) -> Result<(Self, AccountRecoveryPhrase), IdentityError> {
        let mut entropy = [0_u8; 32];
        getrandom::fill(&mut entropy).map_err(IdentityError::SecureRandom)?;
        let mnemonic = Mnemonic::from_entropy(&entropy)
            .map_err(|error| IdentityError::InvalidAccountRecoveryPhrase(error.to_string()))?;
        entropy.zeroize();
        let phrase = AccountRecoveryPhrase(Zeroizing::new(mnemonic.to_string()));
        let identity = account_root_identity_from_phrase(&phrase)?;
        let root = Self::create_with_identity(directory, identity)?;
        Ok((root, phrase))
    }

    fn create_with_identity(
        directory: impl AsRef<Path>,
        identity: AccountRootIdentity,
    ) -> Result<Self, IdentityError> {
        let directory = directory.as_ref().to_path_buf();
        fs::create_dir_all(&directory)?;
        let secret_path = directory.join(ACCOUNT_ROOT_SECRET_FILE);
        let mut file = match open_new_secret_file(&secret_path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                return Err(IdentityError::AccountRootAlreadyExists(secret_path));
            }
            Err(error) => return Err(error.into()),
        };
        let secret = Zeroizing::new(identity.secret_bytes());
        let (encoded, key_protection) = encode_account_root_key(&secret)?;
        let encoded = Zeroizing::new(encoded);
        file.write_all(&encoded)?;
        file.sync_all()?;
        write_new_file(
            &directory.join(AUTHORITY_LOG_VERSION_FILE),
            format!("{AUTHORITY_LOG_VERSION}\n").as_bytes(),
        )?;
        fs::create_dir_all(directory.join(REVOCATIONS_DIRECTORY))?;
        fs::create_dir_all(directory.join(CONVERSATION_MEMBERSHIPS_DIRECTORY))?;
        Ok(Self {
            directory,
            identity,
            key_protection,
            key_load_outcome: AccountRootKeyLoadOutcome::Created,
        })
    }

    pub fn load(directory: impl AsRef<Path>) -> Result<Self, IdentityError> {
        let directory = directory.as_ref().to_path_buf();
        let secret_path = directory.join(ACCOUNT_ROOT_SECRET_FILE);
        let bytes = match fs::read(&secret_path) {
            Ok(bytes) => Zeroizing::new(bytes),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(IdentityError::AccountRootMissing(secret_path));
            }
            Err(error) => return Err(error.into()),
        };
        let loaded = if bytes.len() == SECRET_KEY_BYTES {
            migrate_legacy_account_root_key(&secret_path, &bytes)?
        } else {
            decode_account_root_key(&secret_path, &bytes)?
        };
        Ok(Self {
            directory,
            identity: loaded.identity,
            key_protection: loaded.protection,
            key_load_outcome: loaded.load_outcome,
        })
    }

    pub fn account_id(&self) -> AccountId {
        self.identity.account_id()
    }

    pub fn key_protection(&self) -> AccountRootKeyProtection {
        self.key_protection
    }

    pub fn key_load_outcome(&self) -> AccountRootKeyLoadOutcome {
        self.key_load_outcome
    }

    /// Captures one consistent, Root-signed recovery checkpoint. The returned
    /// witness must be retained independently and kept at its latest version.
    pub fn export_recovery(
        &self,
    ) -> Result<(AccountRootRecoveryPackage, AccountRootRecoveryWitness), IdentityError> {
        self.ensure_authority_log_ready()?;
        let _lock = self.acquire_authority_write_lock()?;
        let authority_snapshot = self.authority_snapshot()?;
        let device_list = self.published_device_list()?;
        let conversation_memberships = self.root_conversation_memberships()?;
        let package = AccountRootRecoveryPackage::issue(
            &self.identity,
            authority_snapshot,
            device_list,
            conversation_memberships,
        )?;
        let witness = AccountRootRecoveryWitness::issue(&self.identity, &package)?;
        Ok((package, witness))
    }

    /// Reconstructs an Account Root only in a new path after the phrase,
    /// package signatures and independently retained witness all agree.
    pub fn recover(
        directory: impl AsRef<Path>,
        phrase: &AccountRecoveryPhrase,
        package: &AccountRootRecoveryPackage,
        witness: &AccountRootRecoveryWitness,
    ) -> Result<Self, IdentityError> {
        package.verify()?;
        witness.verify_package(package)?;
        let identity = account_root_identity_from_phrase(phrase)?;
        if identity.account_id() != package.account_id() {
            return Err(IdentityError::AccountRecoveryPhraseAccountMismatch {
                expected: package.account_id(),
                actual: identity.account_id(),
            });
        }

        let requested = directory.as_ref();
        if requested.as_os_str().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Account Root recovery path is empty",
            )
            .into());
        }
        let lexical = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            std::env::current_dir()?.join(requested)
        };
        if lexical.exists() {
            return Err(IdentityError::AccountRootRecoveryTargetExists(lexical));
        }
        let name = lexical.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Account Root recovery path has no final component",
            )
        })?;
        let parent = lexical.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Account Root recovery path has no parent",
            )
        })?;
        fs::create_dir_all(parent)?;
        let parent = fs::canonicalize(parent)?;
        let resolved = parent.join(name);
        if resolved.exists() {
            return Err(IdentityError::AccountRootRecoveryTargetExists(resolved));
        }

        let staging = tempfile::Builder::new()
            .prefix(".kilogram-root-recovery-")
            .tempdir_in(&parent)?;
        let staged_root = Self::create_with_identity(staging.path(), identity)?;
        write_new_file(
            &staging.path().join(NEXT_AUTHORITY_SEQUENCE_FILE),
            format!("{}\n", package.content.authority_snapshot.revision()).as_bytes(),
        )?;
        for revocation in package.content.authority_snapshot.revocations() {
            write_new_file(
                &staged_root.revocation_path(revocation.device_id()),
                &revocation.encode()?,
            )?;
        }
        write_new_file(
            &staging.path().join(ACCOUNT_DEVICE_LIST_FILE),
            &package.content.device_list.encode()?,
        )?;
        for membership in &package.content.conversation_memberships {
            write_new_file(
                &staged_root.conversation_membership_path(membership.conversation_id()),
                &membership.encode()?,
            )?;
        }

        if staged_root.authority_snapshot()? != package.content.authority_snapshot
            || staged_root.published_device_list()? != package.content.device_list
            || staged_root.root_conversation_memberships()?
                != package.content.conversation_memberships
        {
            return Err(IdentityError::AccountRootRecoveryVerificationFailed);
        }
        drop(staged_root);
        fs::rename(staging.path(), &resolved)?;
        Self::load(resolved)
    }

    /// Signs one canonical device-link authorization payload with the Account
    /// Root under a protocol-specific domain.
    pub fn sign_device_link_authorization(&self, payload: &[u8]) -> Vec<u8> {
        let mut message =
            Vec::with_capacity(DEVICE_LINK_AUTHORIZATION_SIGNATURE_DOMAIN.len() + payload.len());
        message.extend_from_slice(DEVICE_LINK_AUTHORIZATION_SIGNATURE_DOMAIN);
        message.extend_from_slice(payload);
        self.identity.sign(&message).to_vec()
    }

    /// Loads the latest complete Root-signed device list kept by this root.
    pub fn published_device_list(&self) -> Result<AccountDeviceListSnapshot, IdentityError> {
        let path = self.directory.join(ACCOUNT_DEVICE_LIST_FILE);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(IdentityError::AccountDeviceListMissing(path));
            }
            Err(error) => return Err(error.into()),
        };
        let list = AccountDeviceListSnapshot::decode_and_verify(&bytes)?;
        list.verify_for_account(self.account_id())?;
        Ok(list)
    }

    /// Idempotently enrolls an exact device and atomically publishes the new
    /// complete device list. A certificate that was issued but not published
    /// is never sufficient authorization.
    pub fn enroll_device(
        &self,
        device_id: DeviceId,
        encryption_public_key: EncryptionPublicKey,
        capabilities: &[DeviceCapability],
    ) -> Result<(DeviceCertificate, AccountDeviceListSnapshot), IdentityError> {
        self.ensure_authority_log_ready()?;
        validate_requested_capabilities(capabilities)?;
        let mut canonical_capabilities = capabilities.to_vec();
        canonical_capabilities.sort_unstable();
        let _lock = self.acquire_authority_write_lock()?;

        let current = self.published_device_list()?;
        let current_authority = self.authority_snapshot()?;
        if let Some(existing) = current.certificate_for(device_id) {
            if existing.encryption_public_key() != encryption_public_key
                || existing.capabilities() != canonical_capabilities
            {
                return Err(IdentityError::DeviceEnrollmentIdentityConflict(device_id));
            }
            verify_device_authorization_with_snapshot(
                self.account_id(),
                existing,
                &current_authority,
                &canonical_capabilities,
            )?;
            if current.revision() == current_authority.revision() {
                return Ok((existing.clone(), current));
            }
            let active = current
                .devices()
                .iter()
                .filter(|certificate| {
                    !current_authority
                        .revocations()
                        .iter()
                        .any(|revocation| revocation.device_id() == certificate.device_id())
                })
                .cloned()
                .collect::<Vec<_>>();
            let refreshed = self.publish_device_list_unlocked(&active)?;
            return Ok((existing.clone(), refreshed));
        }

        let revoked = current_authority
            .revocations()
            .iter()
            .any(|revocation| revocation.device_id() == device_id);
        if revoked {
            return Err(IdentityError::DeviceRevoked(device_id));
        }
        let mut certificates = current
            .devices()
            .iter()
            .filter(|certificate| {
                !current_authority
                    .revocations()
                    .iter()
                    .any(|revocation| revocation.device_id() == certificate.device_id())
            })
            .cloned()
            .collect::<Vec<_>>();
        if certificates.len() >= MAX_ACCOUNT_DEVICES {
            return Err(IdentityError::TooManyAccountDevices(
                certificates.len().saturating_add(1),
            ));
        }
        let certificate = self.issue_device_certificate_unlocked(
            device_id,
            encryption_public_key,
            &canonical_capabilities,
        )?;
        certificates.push(certificate.clone());
        let list = self.publish_device_list_unlocked(&certificates)?;
        Ok((certificate, list))
    }

    pub fn issue_device_certificate(
        &self,
        device_id: DeviceId,
        encryption_public_key: EncryptionPublicKey,
        capabilities: &[DeviceCapability],
    ) -> Result<DeviceCertificate, IdentityError> {
        self.ensure_authority_log_ready()?;
        validate_requested_capabilities(capabilities)?;
        let _lock = self.acquire_authority_write_lock()?;
        self.issue_device_certificate_unlocked(device_id, encryption_public_key, capabilities)
    }

    fn issue_device_certificate_unlocked(
        &self,
        device_id: DeviceId,
        encryption_public_key: EncryptionPublicKey,
        capabilities: &[DeviceCapability],
    ) -> Result<DeviceCertificate, IdentityError> {
        let authority_sequence = self.allocate_authority_sequence()?;
        DeviceCertificate::issue(
            &self.identity,
            device_id,
            encryption_public_key,
            authority_sequence,
            capabilities,
        )
    }

    pub fn revoke_device(&self, device_id: DeviceId) -> Result<DeviceRevocation, IdentityError> {
        self.ensure_authority_log_ready()?;
        let _lock = self.acquire_authority_write_lock()?;
        let path = self.revocation_path(device_id);
        if path.exists() {
            return Err(IdentityError::DeviceAlreadyRevoked(device_id));
        }
        let authority_sequence = self.allocate_authority_sequence()?;
        let revocation = DeviceRevocation::issue(&self.identity, device_id, authority_sequence)?;
        write_new_file(&path, &revocation.encode()?)?;
        Ok(revocation)
    }

    pub fn authority_snapshot(&self) -> Result<AccountAuthoritySnapshot, IdentityError> {
        self.ensure_authority_log_ready()?;
        let revision = self.read_next_authority_sequence()?;
        let mut revocations = Vec::new();
        for entry in fs::read_dir(self.directory.join(REVOCATIONS_DIRECTORY))? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            revocations.push(DeviceRevocation::decode_and_verify(&fs::read(
                entry.path(),
            )?)?);
        }
        revocations.sort_by_key(|revocation| *revocation.device_id().as_bytes());
        AccountAuthoritySnapshot::issue(&self.identity, revision, revocations)
    }

    pub fn publish_device_list(
        &self,
        certificates: &[DeviceCertificate],
    ) -> Result<AccountDeviceListSnapshot, IdentityError> {
        self.ensure_authority_log_ready()?;
        let _lock = self.acquire_authority_write_lock()?;
        self.publish_device_list_unlocked(certificates)
    }

    fn publish_device_list_unlocked(
        &self,
        certificates: &[DeviceCertificate],
    ) -> Result<AccountDeviceListSnapshot, IdentityError> {
        let snapshot = self.authority_snapshot()?;
        let candidate =
            AccountDeviceListSnapshot::issue(&self.identity, snapshot, certificates.to_vec())?;
        let path = self.directory.join(ACCOUNT_DEVICE_LIST_FILE);
        match fs::read(&path) {
            Ok(bytes) => {
                let existing = AccountDeviceListSnapshot::decode_and_verify(&bytes)?;
                if existing.revision() > candidate.revision() {
                    return Err(IdentityError::AccountDeviceListRollback {
                        stored_revision: existing.revision(),
                        received_revision: candidate.revision(),
                    });
                }
                if existing.revision() == candidate.revision() {
                    if existing == candidate {
                        return Ok(existing);
                    }
                    return Err(IdentityError::AccountDeviceListAlreadyPublished(
                        candidate.revision(),
                    ));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        replace_file_atomically(&path, &candidate.encode()?)?;
        Ok(candidate)
    }

    fn acquire_authority_write_lock(&self) -> Result<fs::File, IdentityError> {
        let lock_path = self.directory.join(AUTHORITY_WRITE_LOCK_FILE);
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)?;
        if let Err(source) = lock.try_lock() {
            return match source {
                fs::TryLockError::WouldBlock => Err(IdentityError::AccountAuthorityWriteLocked),
                fs::TryLockError::Error(source) => Err(source.into()),
            };
        }
        Ok(lock)
    }

    pub fn create_conversation_membership(
        &self,
        conversation_id: ConversationScopeId,
        members: &[AccountId],
    ) -> Result<ConversationMembershipSnapshot, IdentityError> {
        self.ensure_authority_log_ready()?;
        let _lock = self.acquire_authority_write_lock()?;
        let path = self.conversation_membership_path(conversation_id);
        if path.exists() {
            return Err(IdentityError::ConversationMembershipAlreadyExists(
                conversation_id,
            ));
        }
        let snapshot = ConversationMembershipSnapshot::issue(
            &self.identity,
            conversation_id,
            1,
            canonical_members(self.account_id(), members),
        )?;
        write_new_file(&path, &snapshot.encode()?)?;
        Ok(snapshot)
    }

    pub fn add_conversation_members(
        &self,
        conversation_id: ConversationScopeId,
        additions: &[AccountId],
    ) -> Result<ConversationMembershipSnapshot, IdentityError> {
        self.ensure_authority_log_ready()?;
        let _lock = self.acquire_authority_write_lock()?;
        let path = self.conversation_membership_path(conversation_id);
        let current = self.load_root_conversation_membership(conversation_id)?;
        current.verify_for_owner(self.account_id())?;
        let members = canonical_members(
            current.owner_account_id(),
            &[current.members(), additions].concat(),
        );
        if members == current.members() {
            return Ok(current);
        }
        let revision = current
            .revision()
            .checked_add(1)
            .ok_or(IdentityError::ConversationMembershipRevisionExhausted)?;
        let updated = ConversationMembershipSnapshot::issue(
            &self.identity,
            conversation_id,
            revision,
            members,
        )?;
        replace_file_atomically(&path, &updated.encode()?)?;
        Ok(updated)
    }

    pub fn load_root_conversation_membership(
        &self,
        conversation_id: ConversationScopeId,
    ) -> Result<ConversationMembershipSnapshot, IdentityError> {
        let path = self.conversation_membership_path(conversation_id);
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(IdentityError::ConversationMembershipMissing(
                    conversation_id,
                ));
            }
            Err(error) => return Err(error.into()),
        };
        let membership = ConversationMembershipSnapshot::decode_and_verify(&bytes)?;
        membership.verify_for_owner(self.account_id())?;
        Ok(membership)
    }

    fn root_conversation_memberships(
        &self,
    ) -> Result<Vec<ConversationMembershipSnapshot>, IdentityError> {
        let mut memberships = Vec::new();
        for entry in fs::read_dir(self.directory.join(CONVERSATION_MEMBERSHIPS_DIRECTORY))? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let membership =
                ConversationMembershipSnapshot::decode_and_verify(&fs::read(entry.path())?)?;
            membership.verify_for_owner(self.account_id())?;
            memberships.push(membership);
            if memberships.len() > MAX_ACCOUNT_ROOT_RECOVERY_MEMBERSHIPS {
                return Err(IdentityError::TooManyAccountRootRecoveryMemberships(
                    memberships.len(),
                ));
            }
        }
        memberships.sort_by_key(|membership| *membership.conversation_id().as_bytes());
        Ok(memberships)
    }

    fn allocate_authority_sequence(&self) -> Result<u64, IdentityError> {
        let sequence_path = self.directory.join(NEXT_AUTHORITY_SEQUENCE_FILE);
        let current = self.read_next_authority_sequence()?;
        let next = current
            .checked_add(1)
            .ok_or(IdentityError::AuthoritySequenceExhausted)?;
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(sequence_path)?;
        writeln!(file, "{next}")?;
        file.sync_all()?;
        Ok(current)
    }

    fn read_next_authority_sequence(&self) -> Result<u64, IdentityError> {
        match fs::read_to_string(self.directory.join(NEXT_AUTHORITY_SEQUENCE_FILE)) {
            Ok(value) => value.trim().parse().map_err(IdentityError::InvalidSequence),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(0),
            Err(error) => Err(error.into()),
        }
    }

    fn ensure_authority_log_ready(&self) -> Result<(), IdentityError> {
        let version_path = self.directory.join(AUTHORITY_LOG_VERSION_FILE);
        match fs::read_to_string(&version_path) {
            Ok(version) if version.trim() == AUTHORITY_LOG_VERSION => {
                fs::create_dir_all(self.directory.join(REVOCATIONS_DIRECTORY))?;
                fs::create_dir_all(self.directory.join(CONVERSATION_MEMBERSHIPS_DIRECTORY))?;
                Ok(())
            }
            Ok(version) => Err(IdentityError::UnsupportedAuthorityLogVersion(
                version.trim().to_owned(),
            )),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if self.read_next_authority_sequence()? != 0 {
                    return Err(IdentityError::LegacyAuthorityState);
                }
                write_new_file(
                    &version_path,
                    format!("{AUTHORITY_LOG_VERSION}\n").as_bytes(),
                )?;
                fs::create_dir_all(self.directory.join(REVOCATIONS_DIRECTORY))?;
                fs::create_dir_all(self.directory.join(CONVERSATION_MEMBERSHIPS_DIRECTORY))?;
                Ok(())
            }
            Err(error) => Err(error.into()),
        }
    }

    fn revocation_path(&self, device_id: DeviceId) -> PathBuf {
        self.directory
            .join(REVOCATIONS_DIRECTORY)
            .join(format!("{device_id}.revocation"))
    }

    fn conversation_membership_path(&self, conversation_id: ConversationScopeId) -> PathBuf {
        self.directory
            .join(CONVERSATION_MEMBERSHIPS_DIRECTORY)
            .join(format!("{conversation_id}.membership"))
    }
}

fn account_root_identity_from_phrase(
    phrase: &AccountRecoveryPhrase,
) -> Result<AccountRootIdentity, IdentityError> {
    let mnemonic = Mnemonic::parse_in_normalized(Language::English, phrase.expose_secret())
        .map_err(|error| IdentityError::InvalidAccountRecoveryPhrase(error.to_string()))?;
    let mut seed = mnemonic.to_seed_normalized("");
    let mut secret = blake3::derive_key(ACCOUNT_ROOT_DERIVATION_CONTEXT, &seed);
    seed.zeroize();
    let identity = AccountRootIdentity::from_secret_bytes(secret);
    secret.zeroize();
    Ok(identity)
}

fn validate_account_root_recovery_package_content(
    content: &AccountRootRecoveryPackageContent,
) -> Result<(), IdentityError> {
    if content.version != ACCOUNT_ROOT_RECOVERY_VERSION {
        return Err(IdentityError::UnsupportedAccountRootRecoveryVersion(
            content.version,
        ));
    }
    content
        .authority_snapshot
        .verify_for_account(content.account_id)?;
    content.device_list.verify_for_account(content.account_id)?;
    if content.device_list.revision() > content.authority_snapshot.revision() {
        return Err(IdentityError::AccountRootRecoveryDeviceListAhead {
            device_list_revision: content.device_list.revision(),
            authority_revision: content.authority_snapshot.revision(),
        });
    }
    if content.conversation_memberships.len() > MAX_ACCOUNT_ROOT_RECOVERY_MEMBERSHIPS {
        return Err(IdentityError::TooManyAccountRootRecoveryMemberships(
            content.conversation_memberships.len(),
        ));
    }
    for membership in &content.conversation_memberships {
        membership.verify_for_owner(content.account_id)?;
    }
    for pair in content.conversation_memberships.windows(2) {
        match pair[0]
            .conversation_id()
            .as_bytes()
            .cmp(pair[1].conversation_id().as_bytes())
        {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => {
                return Err(IdentityError::DuplicateAccountRootRecoveryMembership(
                    pair[0].conversation_id(),
                ));
            }
            std::cmp::Ordering::Greater => {
                return Err(IdentityError::NonCanonicalAccountRootRecoveryMemberships);
            }
        }
    }
    Ok(())
}

fn account_root_recovery_package_signing_bytes(
    content: &AccountRootRecoveryPackageContent,
) -> Result<Vec<u8>, IdentityError> {
    authority_signing_bytes(ACCOUNT_ROOT_RECOVERY_PACKAGE_SIGNATURE_DOMAIN, content)
}

fn account_root_recovery_witness_signing_bytes(
    content: &AccountRootRecoveryWitnessContent,
) -> Result<Vec<u8>, IdentityError> {
    authority_signing_bytes(ACCOUNT_ROOT_RECOVERY_WITNESS_SIGNATURE_DOMAIN, content)
}

fn migrate_legacy_account_root_key(
    path: &Path,
    bytes: &[u8],
) -> Result<LoadedAccountRootKey, IdentityError> {
    let mut secret: [u8; SECRET_KEY_BYTES] = bytes
        .try_into()
        .map_err(|_| IdentityError::InvalidAccountRootSecretKeyLength(bytes.len()))?;
    let (encoded, protection) = encode_account_root_key(&secret)?;
    let encoded = Zeroizing::new(encoded);
    replace_file_atomically(path, &encoded)?;
    let identity = AccountRootIdentity::from_secret_bytes(secret);
    secret.zeroize();
    Ok(LoadedAccountRootKey {
        identity,
        protection,
        load_outcome: AccountRootKeyLoadOutcome::LegacyMigrated,
    })
}

fn decode_account_root_key(
    path: &Path,
    bytes: &[u8],
) -> Result<LoadedAccountRootKey, IdentityError> {
    if bytes.len() > MAX_ACCOUNT_ROOT_KEY_ENVELOPE_BYTES {
        return Err(IdentityError::AccountRootKeyEnvelopeTooLarge(bytes.len()));
    }
    let encoded = bytes
        .strip_prefix(ACCOUNT_ROOT_KEY_ENVELOPE_MAGIC)
        .ok_or_else(|| IdentityError::InvalidAccountRootKeyEnvelope {
            path: path.to_path_buf(),
            detail: "missing Kilogram Account Root key-envelope magic".to_owned(),
        })?;
    let envelope: AccountRootKeyEnvelope = postcard::from_bytes(encoded).map_err(|error| {
        IdentityError::InvalidAccountRootKeyEnvelope {
            path: path.to_path_buf(),
            detail: error.to_string(),
        }
    })?;
    if envelope.version != ACCOUNT_ROOT_KEY_ENVELOPE_VERSION {
        return Err(IdentityError::UnsupportedAccountRootKeyEnvelopeVersion(
            envelope.version,
        ));
    }
    open_account_root_key(path, envelope)
}

fn open_account_root_key(
    path: &Path,
    mut envelope: AccountRootKeyEnvelope,
) -> Result<LoadedAccountRootKey, IdentityError> {
    match envelope.provider {
        AccountRootKeyProviderId::WindowsDpapiCurrentUser => {
            let plaintext = unprotect_account_root_key_windows_dpapi(&envelope.protected_key)?;
            loaded_account_root_key_from_vec(
                plaintext,
                AccountRootKeyProtection::WindowsDpapiCurrentUser,
                AccountRootKeyLoadOutcome::AlreadyCurrent,
            )
        }
        AccountRootKeyProviderId::PlaintextDevelopment => {
            let plaintext = std::mem::take(&mut envelope.protected_key);
            #[cfg(windows)]
            {
                let mut secret = secret_from_vec(plaintext)?;
                let (encoded, protection) = encode_account_root_key(&secret)?;
                let encoded = Zeroizing::new(encoded);
                replace_file_atomically(path, &encoded)?;
                let identity = AccountRootIdentity::from_secret_bytes(secret);
                secret.zeroize();
                Ok(LoadedAccountRootKey {
                    identity,
                    protection,
                    load_outcome: AccountRootKeyLoadOutcome::LegacyMigrated,
                })
            }
            #[cfg(not(windows))]
            {
                let _ = path;
                loaded_account_root_key_from_vec(
                    plaintext,
                    AccountRootKeyProtection::PlaintextDevelopment,
                    AccountRootKeyLoadOutcome::AlreadyCurrent,
                )
            }
        }
    }
}

fn loaded_account_root_key_from_vec(
    bytes: Vec<u8>,
    protection: AccountRootKeyProtection,
    load_outcome: AccountRootKeyLoadOutcome,
) -> Result<LoadedAccountRootKey, IdentityError> {
    let bytes = Zeroizing::new(bytes);
    let mut secret = secret_from_slice(&bytes)?;
    let identity = AccountRootIdentity::from_secret_bytes(secret);
    secret.zeroize();
    Ok(LoadedAccountRootKey {
        identity,
        protection,
        load_outcome,
    })
}

fn secret_from_vec(mut bytes: Vec<u8>) -> Result<[u8; SECRET_KEY_BYTES], IdentityError> {
    let result = secret_from_slice(&bytes);
    bytes.zeroize();
    result
}

fn secret_from_slice(bytes: &[u8]) -> Result<[u8; SECRET_KEY_BYTES], IdentityError> {
    bytes
        .try_into()
        .map_err(|_| IdentityError::InvalidAccountRootSecretKeyLength(bytes.len()))
}

fn encode_account_root_key(
    secret: &[u8; SECRET_KEY_BYTES],
) -> Result<(Vec<u8>, AccountRootKeyProtection), IdentityError> {
    #[cfg(windows)]
    let (provider, protected_key, protection) = (
        AccountRootKeyProviderId::WindowsDpapiCurrentUser,
        protect_account_root_key_windows_dpapi(secret)?,
        AccountRootKeyProtection::WindowsDpapiCurrentUser,
    );
    #[cfg(not(windows))]
    let (provider, protected_key, protection) = (
        AccountRootKeyProviderId::PlaintextDevelopment,
        secret.to_vec(),
        AccountRootKeyProtection::PlaintextDevelopment,
    );
    let envelope = AccountRootKeyEnvelope {
        version: ACCOUNT_ROOT_KEY_ENVELOPE_VERSION,
        provider,
        protected_key,
    };
    let mut payload = postcard::to_allocvec(&envelope)?;
    let total_len = ACCOUNT_ROOT_KEY_ENVELOPE_MAGIC.len() + payload.len();
    if total_len > MAX_ACCOUNT_ROOT_KEY_ENVELOPE_BYTES {
        payload.zeroize();
        return Err(IdentityError::AccountRootKeyEnvelopeTooLarge(total_len));
    }
    let mut encoded = Vec::with_capacity(total_len);
    encoded.extend_from_slice(ACCOUNT_ROOT_KEY_ENVELOPE_MAGIC);
    encoded.append(&mut payload);
    Ok((encoded, protection))
}

#[cfg(windows)]
fn protect_account_root_key_windows_dpapi(plaintext: &[u8]) -> Result<Vec<u8>, IdentityError> {
    stellar_agent_windows_identity::dpapi_protect(plaintext).map_err(|error| {
        IdentityError::AccountRootKeyProtectionFailed {
            provider: AccountRootKeyProviderId::WindowsDpapiCurrentUser
                .as_str()
                .to_owned(),
            operation: "protect",
            detail: error.to_string(),
        }
    })
}

#[cfg(windows)]
fn unprotect_account_root_key_windows_dpapi(ciphertext: &[u8]) -> Result<Vec<u8>, IdentityError> {
    stellar_agent_windows_identity::dpapi_unprotect(ciphertext).map_err(|error| {
        IdentityError::AccountRootKeyProtectionFailed {
            provider: AccountRootKeyProviderId::WindowsDpapiCurrentUser
                .as_str()
                .to_owned(),
            operation: "unprotect",
            detail: error.to_string(),
        }
    })
}

#[cfg(not(windows))]
fn unprotect_account_root_key_windows_dpapi(_ciphertext: &[u8]) -> Result<Vec<u8>, IdentityError> {
    Err(IdentityError::AccountRootKeyProviderUnavailable(
        AccountRootKeyProviderId::WindowsDpapiCurrentUser
            .as_str()
            .to_owned(),
    ))
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), IdentityError> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "authority file has no parent")
    })?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist_noclobber(path).map_err(|error| error.error)?;
    Ok(())
}

fn replace_file_atomically(path: &Path, bytes: &[u8]) -> Result<(), IdentityError> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "authority file has no parent")
    })?;
    fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum DeviceCapability {
    SignEvents,
    SyncHistory,
}

impl DeviceCapability {
    pub const MESSAGING: [Self; 2] = [Self::SignEvents, Self::SyncHistory];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::SignEvents => "sign-events",
            Self::SyncHistory => "sync-history",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct DeviceCertificateContent {
    version: u8,
    account_id: AccountId,
    device_id: DeviceId,
    encryption_public_key: EncryptionPublicKey,
    authority_sequence: u64,
    capabilities: Vec<DeviceCapability>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeviceCertificate {
    content: DeviceCertificateContent,
    signature: Vec<u8>,
}

impl DeviceCertificate {
    fn issue(
        root: &AccountRootIdentity,
        device_id: DeviceId,
        encryption_public_key: EncryptionPublicKey,
        authority_sequence: u64,
        capabilities: &[DeviceCapability],
    ) -> Result<Self, IdentityError> {
        validate_requested_capabilities(capabilities)?;
        let mut capabilities = capabilities.to_vec();
        capabilities.sort_unstable();
        let content = DeviceCertificateContent {
            version: DEVICE_CERTIFICATE_VERSION,
            account_id: root.account_id(),
            device_id,
            encryption_public_key,
            authority_sequence,
            capabilities,
        };
        validate_certificate_content(&content)?;
        let signature = root.sign(&certificate_signing_bytes(&content)?).to_vec();
        Ok(Self { content, signature })
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, IdentityError> {
        let certificate: Self = postcard::from_bytes(bytes)?;
        certificate.verify()?;
        Ok(certificate)
    }

    pub fn encode(&self) -> Result<Vec<u8>, IdentityError> {
        self.verify()?;
        Ok(postcard::to_allocvec(self)?)
    }

    pub fn verify(&self) -> Result<(), IdentityError> {
        validate_certificate_content(&self.content)?;
        self.content
            .account_id
            .verify(&certificate_signing_bytes(&self.content)?, &self.signature)
    }

    pub fn verify_for_account(&self, expected: AccountId) -> Result<(), IdentityError> {
        self.verify()?;
        if self.content.account_id != expected {
            return Err(IdentityError::AccountMismatch {
                expected,
                actual: self.content.account_id,
            });
        }
        Ok(())
    }

    pub fn account_id(&self) -> AccountId {
        self.content.account_id
    }

    pub fn device_id(&self) -> DeviceId {
        self.content.device_id
    }

    pub fn authority_sequence(&self) -> u64 {
        self.content.authority_sequence
    }

    pub fn encryption_public_key(&self) -> EncryptionPublicKey {
        self.content.encryption_public_key
    }

    pub fn capabilities(&self) -> &[DeviceCapability] {
        &self.content.capabilities
    }

    pub fn has_capability(&self, capability: DeviceCapability) -> bool {
        self.content.capabilities.binary_search(&capability).is_ok()
    }
}

fn validate_requested_capabilities(capabilities: &[DeviceCapability]) -> Result<(), IdentityError> {
    if capabilities.is_empty() {
        return Err(IdentityError::EmptyDeviceCapabilities);
    }
    for (index, capability) in capabilities.iter().enumerate() {
        if capabilities[index + 1..].contains(capability) {
            return Err(IdentityError::DuplicateDeviceCapability);
        }
    }
    Ok(())
}

fn validate_certificate_content(content: &DeviceCertificateContent) -> Result<(), IdentityError> {
    if content.version != DEVICE_CERTIFICATE_VERSION {
        return Err(IdentityError::UnsupportedAccountAuthorityVersion(
            content.version,
        ));
    }
    if content.capabilities.is_empty() {
        return Err(IdentityError::EmptyDeviceCapabilities);
    }
    for pair in content.capabilities.windows(2) {
        if pair[0] == pair[1] {
            return Err(IdentityError::DuplicateDeviceCapability);
        }
        if pair[0] > pair[1] {
            return Err(IdentityError::NonCanonicalDeviceCapabilities);
        }
    }
    Ok(())
}

fn certificate_signing_bytes(content: &DeviceCertificateContent) -> Result<Vec<u8>, IdentityError> {
    authority_signing_bytes(DEVICE_CERTIFICATE_SIGNATURE_DOMAIN, content)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct DeviceRevocationContent {
    version: u8,
    account_id: AccountId,
    device_id: DeviceId,
    authority_sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeviceRevocation {
    content: DeviceRevocationContent,
    signature: Vec<u8>,
}

impl DeviceRevocation {
    fn issue(
        root: &AccountRootIdentity,
        device_id: DeviceId,
        authority_sequence: u64,
    ) -> Result<Self, IdentityError> {
        let content = DeviceRevocationContent {
            version: AUTHORITY_VERSION,
            account_id: root.account_id(),
            device_id,
            authority_sequence,
        };
        let signature = root.sign(&revocation_signing_bytes(&content)?).to_vec();
        Ok(Self { content, signature })
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, IdentityError> {
        let revocation: Self = postcard::from_bytes(bytes)?;
        revocation.verify()?;
        Ok(revocation)
    }

    pub fn encode(&self) -> Result<Vec<u8>, IdentityError> {
        self.verify()?;
        Ok(postcard::to_allocvec(self)?)
    }

    pub fn verify(&self) -> Result<(), IdentityError> {
        validate_authority_version(self.content.version)?;
        self.content
            .account_id
            .verify(&revocation_signing_bytes(&self.content)?, &self.signature)
    }

    pub fn verify_for_account(&self, expected: AccountId) -> Result<(), IdentityError> {
        self.verify()?;
        if self.content.account_id != expected {
            return Err(IdentityError::AccountMismatch {
                expected,
                actual: self.content.account_id,
            });
        }
        Ok(())
    }

    pub fn account_id(&self) -> AccountId {
        self.content.account_id
    }

    pub fn device_id(&self) -> DeviceId {
        self.content.device_id
    }

    pub fn authority_sequence(&self) -> u64 {
        self.content.authority_sequence
    }
}

fn revocation_signing_bytes(content: &DeviceRevocationContent) -> Result<Vec<u8>, IdentityError> {
    authority_signing_bytes(DEVICE_REVOCATION_SIGNATURE_DOMAIN, content)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct AccountAuthoritySnapshotContent {
    version: u8,
    account_id: AccountId,
    revision: u64,
    revocations: Vec<DeviceRevocation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountAuthoritySnapshot {
    content: AccountAuthoritySnapshotContent,
    signature: Vec<u8>,
}

impl AccountAuthoritySnapshot {
    fn issue(
        root: &AccountRootIdentity,
        revision: u64,
        revocations: Vec<DeviceRevocation>,
    ) -> Result<Self, IdentityError> {
        let content = AccountAuthoritySnapshotContent {
            version: AUTHORITY_VERSION,
            account_id: root.account_id(),
            revision,
            revocations,
        };
        validate_authority_snapshot_content(&content)?;
        let signature = root
            .sign(&authority_snapshot_signing_bytes(&content)?)
            .to_vec();
        Ok(Self { content, signature })
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, IdentityError> {
        let snapshot: Self = postcard::from_bytes(bytes)?;
        snapshot.verify()?;
        Ok(snapshot)
    }

    pub fn encode(&self) -> Result<Vec<u8>, IdentityError> {
        self.verify()?;
        Ok(postcard::to_allocvec(self)?)
    }

    pub fn verify(&self) -> Result<(), IdentityError> {
        validate_authority_snapshot_content(&self.content)?;
        self.content.account_id.verify(
            &authority_snapshot_signing_bytes(&self.content)?,
            &self.signature,
        )
    }

    pub fn verify_for_account(&self, expected: AccountId) -> Result<(), IdentityError> {
        self.verify()?;
        if self.content.account_id != expected {
            return Err(IdentityError::AccountMismatch {
                expected,
                actual: self.content.account_id,
            });
        }
        Ok(())
    }

    pub fn account_id(&self) -> AccountId {
        self.content.account_id
    }

    pub fn revision(&self) -> u64 {
        self.content.revision
    }

    pub fn revocations(&self) -> &[DeviceRevocation] {
        &self.content.revocations
    }
}

fn validate_authority_snapshot_content(
    content: &AccountAuthoritySnapshotContent,
) -> Result<(), IdentityError> {
    validate_authority_version(content.version)?;
    for revocation in &content.revocations {
        revocation.verify_for_account(content.account_id)?;
        if revocation.authority_sequence() >= content.revision {
            return Err(IdentityError::RevocationOutsideSnapshot {
                revocation_sequence: revocation.authority_sequence(),
                snapshot_revision: content.revision,
            });
        }
    }
    for pair in content.revocations.windows(2) {
        let first = pair[0].device_id();
        let second = pair[1].device_id();
        match first.as_bytes().cmp(second.as_bytes()) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => {
                return Err(IdentityError::DuplicateDeviceRevocation(first));
            }
            std::cmp::Ordering::Greater => {
                return Err(IdentityError::NonCanonicalAuthoritySnapshot);
            }
        }
    }
    Ok(())
}

fn authority_snapshot_signing_bytes(
    content: &AccountAuthoritySnapshotContent,
) -> Result<Vec<u8>, IdentityError> {
    authority_signing_bytes(AUTHORITY_SNAPSHOT_SIGNATURE_DOMAIN, content)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct AccountDeviceListSnapshotContent {
    version: u8,
    authority_snapshot: AccountAuthoritySnapshot,
    devices: Vec<DeviceCertificate>,
}

/// A complete, root-signed list of messaging devices at one authority revision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountDeviceListSnapshot {
    content: AccountDeviceListSnapshotContent,
    signature: Vec<u8>,
}

impl AccountDeviceListSnapshot {
    fn issue(
        root: &AccountRootIdentity,
        authority_snapshot: AccountAuthoritySnapshot,
        mut devices: Vec<DeviceCertificate>,
    ) -> Result<Self, IdentityError> {
        devices.sort_by_key(|certificate| *certificate.device_id().as_bytes());
        let content = AccountDeviceListSnapshotContent {
            version: ACCOUNT_DEVICE_LIST_VERSION,
            authority_snapshot,
            devices,
        };
        validate_account_device_list_content(&content)?;
        if content.authority_snapshot.account_id() != root.account_id() {
            return Err(IdentityError::AccountMismatch {
                expected: root.account_id(),
                actual: content.authority_snapshot.account_id(),
            });
        }
        let signature = root
            .sign(&account_device_list_signing_bytes(&content)?)
            .to_vec();
        Ok(Self { content, signature })
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, IdentityError> {
        let snapshot: Self = postcard::from_bytes(bytes)?;
        snapshot.verify()?;
        Ok(snapshot)
    }

    pub fn encode(&self) -> Result<Vec<u8>, IdentityError> {
        self.verify()?;
        Ok(postcard::to_allocvec(self)?)
    }

    pub fn verify(&self) -> Result<(), IdentityError> {
        validate_account_device_list_content(&self.content)?;
        self.account_id().verify(
            &account_device_list_signing_bytes(&self.content)?,
            &self.signature,
        )
    }

    pub fn verify_for_account(&self, expected: AccountId) -> Result<(), IdentityError> {
        self.verify()?;
        if self.account_id() != expected {
            return Err(IdentityError::AccountMismatch {
                expected,
                actual: self.account_id(),
            });
        }
        Ok(())
    }

    pub fn account_id(&self) -> AccountId {
        self.content.authority_snapshot.account_id()
    }

    pub fn revision(&self) -> u64 {
        self.content.authority_snapshot.revision()
    }

    pub fn authority_snapshot(&self) -> &AccountAuthoritySnapshot {
        &self.content.authority_snapshot
    }

    pub fn devices(&self) -> &[DeviceCertificate] {
        &self.content.devices
    }

    pub fn certificate_for(&self, device_id: DeviceId) -> Option<&DeviceCertificate> {
        self.content
            .devices
            .binary_search_by(|certificate| certificate.device_id().cmp(&device_id))
            .ok()
            .map(|index| &self.content.devices[index])
    }
}

fn validate_account_device_list_content(
    content: &AccountDeviceListSnapshotContent,
) -> Result<(), IdentityError> {
    if content.version != ACCOUNT_DEVICE_LIST_VERSION {
        return Err(IdentityError::UnsupportedAccountDeviceListVersion(
            content.version,
        ));
    }
    content.authority_snapshot.verify()?;
    if content.devices.is_empty() {
        return Err(IdentityError::EmptyAccountDeviceList);
    }
    if content.devices.len() > MAX_ACCOUNT_DEVICES {
        return Err(IdentityError::TooManyAccountDevices(content.devices.len()));
    }
    for certificate in &content.devices {
        verify_device_authorization_with_snapshot(
            content.authority_snapshot.account_id(),
            certificate,
            &content.authority_snapshot,
            &DeviceCapability::MESSAGING,
        )?;
    }
    for pair in content.devices.windows(2) {
        match pair[0]
            .device_id()
            .as_bytes()
            .cmp(pair[1].device_id().as_bytes())
        {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => {
                return Err(IdentityError::DuplicateAccountDevice(pair[0].device_id()));
            }
            std::cmp::Ordering::Greater => {
                return Err(IdentityError::NonCanonicalAccountDeviceList);
            }
        }
    }
    Ok(())
}

fn account_device_list_signing_bytes(
    content: &AccountDeviceListSnapshotContent,
) -> Result<Vec<u8>, IdentityError> {
    authority_signing_bytes(ACCOUNT_DEVICE_LIST_SIGNATURE_DOMAIN, content)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ConversationMembershipSnapshotContent {
    version: u8,
    conversation_id: ConversationScopeId,
    revision: u64,
    owner_account_id: AccountId,
    members: Vec<AccountId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConversationMembershipSnapshot {
    content: ConversationMembershipSnapshotContent,
    signature: Vec<u8>,
}

impl ConversationMembershipSnapshot {
    fn issue(
        root: &AccountRootIdentity,
        conversation_id: ConversationScopeId,
        revision: u64,
        members: Vec<AccountId>,
    ) -> Result<Self, IdentityError> {
        let content = ConversationMembershipSnapshotContent {
            version: AUTHORITY_VERSION,
            conversation_id,
            revision,
            owner_account_id: root.account_id(),
            members,
        };
        validate_conversation_membership_content(&content)?;
        let signature = root
            .sign(&conversation_membership_signing_bytes(&content)?)
            .to_vec();
        Ok(Self { content, signature })
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, IdentityError> {
        let membership: Self = postcard::from_bytes(bytes)?;
        membership.verify()?;
        Ok(membership)
    }

    pub fn encode(&self) -> Result<Vec<u8>, IdentityError> {
        self.verify()?;
        Ok(postcard::to_allocvec(self)?)
    }

    pub fn verify(&self) -> Result<(), IdentityError> {
        validate_conversation_membership_content(&self.content)?;
        self.content.owner_account_id.verify(
            &conversation_membership_signing_bytes(&self.content)?,
            &self.signature,
        )
    }

    pub fn verify_for_owner(&self, expected_owner: AccountId) -> Result<(), IdentityError> {
        self.verify()?;
        if self.content.owner_account_id != expected_owner {
            return Err(IdentityError::ConversationMembershipOwnerMismatch {
                expected: expected_owner,
                actual: self.content.owner_account_id,
            });
        }
        Ok(())
    }

    pub fn require_member(&self, account_id: AccountId) -> Result<(), IdentityError> {
        self.verify()?;
        if !self.content.members.contains(&account_id) {
            return Err(IdentityError::AccountNotConversationMember(account_id));
        }
        Ok(())
    }

    pub fn conversation_id(&self) -> ConversationScopeId {
        self.content.conversation_id
    }

    pub fn revision(&self) -> u64 {
        self.content.revision
    }

    pub fn owner_account_id(&self) -> AccountId {
        self.content.owner_account_id
    }

    pub fn members(&self) -> &[AccountId] {
        &self.content.members
    }
}

fn canonical_members(owner: AccountId, members: &[AccountId]) -> Vec<AccountId> {
    let mut members = members.to_vec();
    members.push(owner);
    members.sort_by_key(|account_id| *account_id.as_bytes());
    members.dedup();
    members
}

fn validate_conversation_membership_content(
    content: &ConversationMembershipSnapshotContent,
) -> Result<(), IdentityError> {
    validate_authority_version(content.version)?;
    if content.revision == 0 {
        return Err(IdentityError::InvalidConversationMembershipRevision);
    }
    for pair in content.members.windows(2) {
        match pair[0].as_bytes().cmp(pair[1].as_bytes()) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => {
                return Err(IdentityError::DuplicateConversationMember(pair[0]));
            }
            std::cmp::Ordering::Greater => {
                return Err(IdentityError::NonCanonicalConversationMembers);
            }
        }
    }
    if !content.members.contains(&content.owner_account_id) {
        return Err(IdentityError::AccountNotConversationMember(
            content.owner_account_id,
        ));
    }
    Ok(())
}

fn conversation_membership_signing_bytes(
    content: &ConversationMembershipSnapshotContent,
) -> Result<Vec<u8>, IdentityError> {
    authority_signing_bytes(CONVERSATION_MEMBERSHIP_SIGNATURE_DOMAIN, content)
}

fn authority_signing_bytes<T: Serialize>(
    domain: &[u8],
    content: &T,
) -> Result<Vec<u8>, IdentityError> {
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(domain.len() + encoded.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn validate_authority_version(version: u8) -> Result<(), IdentityError> {
    if version != AUTHORITY_VERSION {
        return Err(IdentityError::UnsupportedAccountAuthorityVersion(version));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedDevice {
    account_id: AccountId,
    device_id: DeviceId,
    certificate_authority_sequence: u64,
    capabilities: Vec<DeviceCapability>,
}

impl AuthorizedDevice {
    pub fn account_id(&self) -> AccountId {
        self.account_id
    }

    pub fn device_id(&self) -> DeviceId {
        self.device_id
    }

    pub fn certificate_authority_sequence(&self) -> u64 {
        self.certificate_authority_sequence
    }

    pub fn capabilities(&self) -> &[DeviceCapability] {
        &self.capabilities
    }
}

pub fn verify_device_authorization(
    expected_account: AccountId,
    certificate: &DeviceCertificate,
    revocations: &[DeviceRevocation],
    required_capabilities: &[DeviceCapability],
) -> Result<AuthorizedDevice, IdentityError> {
    certificate.verify_for_account(expected_account)?;
    for capability in required_capabilities {
        if !certificate.has_capability(*capability) {
            return Err(IdentityError::MissingDeviceCapability(*capability));
        }
    }
    for revocation in revocations {
        revocation.verify_for_account(expected_account)?;
        if revocation.device_id() == certificate.device_id() {
            return Err(IdentityError::DeviceRevoked(certificate.device_id()));
        }
    }
    Ok(AuthorizedDevice {
        account_id: certificate.account_id(),
        device_id: certificate.device_id(),
        certificate_authority_sequence: certificate.authority_sequence(),
        capabilities: certificate.capabilities().to_vec(),
    })
}

pub fn verify_device_authorization_with_snapshot(
    expected_account: AccountId,
    certificate: &DeviceCertificate,
    snapshot: &AccountAuthoritySnapshot,
    required_capabilities: &[DeviceCapability],
) -> Result<AuthorizedDevice, IdentityError> {
    snapshot.verify_for_account(expected_account)?;
    if certificate.authority_sequence() >= snapshot.revision() {
        return Err(IdentityError::CertificateOutsideSnapshot {
            certificate_sequence: certificate.authority_sequence(),
            snapshot_revision: snapshot.revision(),
        });
    }
    verify_device_authorization(
        expected_account,
        certificate,
        snapshot.revocations(),
        required_capabilities,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoritySnapshotStoreOutcome {
    Installed,
    Updated,
    Unchanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConversationMembershipStoreOutcome {
    Installed,
    Updated,
    Unchanged,
}

impl DeviceState {
    pub fn install_certificate(
        &self,
        certificate: &DeviceCertificate,
    ) -> Result<(), IdentityError> {
        certificate.verify()?;
        let expected = self.identity.device_id();
        if certificate.device_id() != expected {
            return Err(IdentityError::DeviceCertificateDeviceMismatch {
                expected,
                actual: certificate.device_id(),
            });
        }
        if certificate.encryption_public_key() != self.encryption.public_key() {
            return Err(IdentityError::DeviceCertificateEncryptionKeyMismatch);
        }
        let encoded = certificate.encode()?;
        let path = self.directory.join(DEVICE_CERTIFICATE_FILE);
        match fs::read(&path) {
            Ok(existing) if existing == encoded => return Ok(()),
            Ok(_) => return Err(IdentityError::DeviceCertificateAlreadyInstalled),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(&encoded)?;
        file.sync_all()?;
        Ok(())
    }

    pub fn load_certificate(&self) -> Result<DeviceCertificate, IdentityError> {
        let path = self.directory.join(DEVICE_CERTIFICATE_FILE);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(IdentityError::DeviceCertificateMissing);
            }
            Err(error) => return Err(error.into()),
        };
        let certificate = DeviceCertificate::decode_and_verify(&bytes)?;
        let expected = self.identity.device_id();
        if certificate.device_id() != expected {
            return Err(IdentityError::DeviceCertificateDeviceMismatch {
                expected,
                actual: certificate.device_id(),
            });
        }
        if certificate.encryption_public_key() != self.encryption.public_key() {
            return Err(IdentityError::DeviceCertificateEncryptionKeyMismatch);
        }
        Ok(certificate)
    }

    pub fn install_own_authority_snapshot(
        &self,
        snapshot: &AccountAuthoritySnapshot,
    ) -> Result<AuthoritySnapshotStoreOutcome, IdentityError> {
        let certificate = self.load_certificate()?;
        snapshot.verify_for_account(certificate.account_id())?;
        if certificate.authority_sequence() >= snapshot.revision() {
            return Err(IdentityError::CertificateOutsideSnapshot {
                certificate_sequence: certificate.authority_sequence(),
                snapshot_revision: snapshot.revision(),
            });
        }
        store_authority_snapshot(
            &self.directory.join(ACCOUNT_AUTHORITY_SNAPSHOT_FILE),
            snapshot,
        )
    }

    pub fn load_own_authority_snapshot(&self) -> Result<AccountAuthoritySnapshot, IdentityError> {
        let path = self.directory.join(ACCOUNT_AUTHORITY_SNAPSHOT_FILE);
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(IdentityError::AccountAuthoritySnapshotMissing);
            }
            Err(error) => return Err(error.into()),
        };
        let snapshot = AccountAuthoritySnapshot::decode_and_verify(&bytes)?;
        let certificate = self.load_certificate()?;
        snapshot.verify_for_account(certificate.account_id())?;
        Ok(snapshot)
    }

    pub fn pin_peer_authority_snapshot(
        &self,
        snapshot: &AccountAuthoritySnapshot,
    ) -> Result<AuthoritySnapshotStoreOutcome, IdentityError> {
        snapshot.verify()?;
        let directory = self.directory.join(PEER_AUTHORITY_DIRECTORY);
        fs::create_dir_all(&directory)?;
        store_authority_snapshot(
            &directory.join(format!("{}.snapshot", snapshot.account_id())),
            snapshot,
        )
    }

    pub fn load_peer_authority_snapshot(
        &self,
        account_id: AccountId,
    ) -> Result<AccountAuthoritySnapshot, IdentityError> {
        let path = self
            .directory
            .join(PEER_AUTHORITY_DIRECTORY)
            .join(format!("{account_id}.snapshot"));
        let snapshot = AccountAuthoritySnapshot::decode_and_verify(&fs::read(path)?)?;
        snapshot.verify_for_account(account_id)?;
        Ok(snapshot)
    }

    pub fn install_conversation_membership(
        &self,
        membership: &ConversationMembershipSnapshot,
    ) -> Result<ConversationMembershipStoreOutcome, IdentityError> {
        membership.verify()?;
        let directory = self.directory.join(CONVERSATION_MEMBERSHIPS_DIRECTORY);
        fs::create_dir_all(&directory)?;
        let path = directory.join(format!("{}.membership", membership.conversation_id()));
        let encoded = membership.encode()?;
        let outcome = match fs::read(&path) {
            Ok(existing) => {
                let stored = ConversationMembershipSnapshot::decode_and_verify(&existing)?;
                stored.verify_for_owner(membership.owner_account_id())?;
                if membership.revision() < stored.revision() {
                    return Err(IdentityError::ConversationMembershipRollback {
                        conversation_id: membership.conversation_id(),
                        stored_revision: stored.revision(),
                        received_revision: membership.revision(),
                    });
                }
                if membership.revision() == stored.revision() {
                    if existing == encoded {
                        return Ok(ConversationMembershipStoreOutcome::Unchanged);
                    }
                    return Err(IdentityError::ConversationMembershipEquivocation {
                        conversation_id: membership.conversation_id(),
                        revision: membership.revision(),
                    });
                }
                if stored
                    .members()
                    .iter()
                    .any(|account_id| !membership.members().contains(account_id))
                {
                    return Err(IdentityError::ConversationMembershipNotAddOnly(
                        membership.conversation_id(),
                    ));
                }
                ConversationMembershipStoreOutcome::Updated
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                ConversationMembershipStoreOutcome::Installed
            }
            Err(error) => return Err(error.into()),
        };
        replace_file_atomically(&path, &encoded)?;
        Ok(outcome)
    }

    pub fn load_conversation_membership(
        &self,
        conversation_id: ConversationScopeId,
    ) -> Result<ConversationMembershipSnapshot, IdentityError> {
        let path = self
            .directory
            .join(CONVERSATION_MEMBERSHIPS_DIRECTORY)
            .join(format!("{conversation_id}.membership"));
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(IdentityError::ConversationMembershipMissing(
                    conversation_id,
                ));
            }
            Err(error) => return Err(error.into()),
        };
        let membership = ConversationMembershipSnapshot::decode_and_verify(&bytes)?;
        if membership.conversation_id() != conversation_id {
            return Err(IdentityError::ConversationMembershipMissing(
                conversation_id,
            ));
        }
        Ok(membership)
    }
}

fn store_authority_snapshot(
    path: &Path,
    snapshot: &AccountAuthoritySnapshot,
) -> Result<AuthoritySnapshotStoreOutcome, IdentityError> {
    let encoded = snapshot.encode()?;
    let outcome = match fs::read(path) {
        Ok(existing) => {
            let stored = AccountAuthoritySnapshot::decode_and_verify(&existing)?;
            if stored.account_id() != snapshot.account_id() {
                return Err(IdentityError::AccountMismatch {
                    expected: stored.account_id(),
                    actual: snapshot.account_id(),
                });
            }
            if snapshot.revision() < stored.revision() {
                return Err(IdentityError::AuthoritySnapshotRollback {
                    account_id: snapshot.account_id(),
                    stored_revision: stored.revision(),
                    received_revision: snapshot.revision(),
                });
            }
            if snapshot.revision() == stored.revision() {
                if existing == encoded {
                    return Ok(AuthoritySnapshotStoreOutcome::Unchanged);
                }
                return Err(IdentityError::AuthoritySnapshotEquivocation {
                    account_id: snapshot.account_id(),
                    revision: snapshot.revision(),
                });
            }
            AuthoritySnapshotStoreOutcome::Updated
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            AuthoritySnapshotStoreOutcome::Installed
        }
        Err(error) => return Err(error.into()),
    };

    replace_file_atomically(path, &encoded)?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DeviceState;
    use tempfile::tempdir;

    fn test_encryption_public_key() -> Result<EncryptionPublicKey, IdentityError> {
        Ok(EncryptionPublicKey::from_bytes([7_u8; 32])?)
    }

    #[test]
    fn recovery_phrase_is_stable_and_root_key_is_enveloped() -> Result<(), IdentityError> {
        let directory = tempdir()?;
        let (root, phrase) = AccountRootState::create_recoverable(directory.path())?;
        let account_id = root.account_id();
        assert_eq!(phrase.expose_secret().split_whitespace().count(), 24);
        assert_eq!(
            AccountRecoveryPhrase::parse(phrase.expose_secret())?.account_id()?,
            account_id
        );
        let stored = fs::read(directory.path().join(ACCOUNT_ROOT_SECRET_FILE))?;
        assert_ne!(stored.len(), SECRET_KEY_BYTES);
        assert!(stored.starts_with(ACCOUNT_ROOT_KEY_ENVELOPE_MAGIC));
        assert_eq!(
            AccountRootState::load(directory.path())?.account_id(),
            account_id
        );
        assert!(AccountRecoveryPhrase::parse("not a valid recovery phrase").is_err());
        Ok(())
    }

    #[test]
    fn legacy_plaintext_root_key_migrates_without_changing_account() -> Result<(), IdentityError> {
        let directory = tempdir()?;
        let root = AccountRootState::create(directory.path())?;
        let account_id = root.account_id();
        let secret = root.identity.secret_bytes();
        drop(root);
        let secret_path = directory.path().join(ACCOUNT_ROOT_SECRET_FILE);
        fs::write(&secret_path, secret)?;

        let migrated = AccountRootState::load(directory.path())?;
        assert_eq!(migrated.account_id(), account_id);
        assert_eq!(
            migrated.key_load_outcome(),
            AccountRootKeyLoadOutcome::LegacyMigrated
        );
        let stored = fs::read(secret_path)?;
        assert!(stored.starts_with(ACCOUNT_ROOT_KEY_ENVELOPE_MAGIC));
        Ok(())
    }

    #[test]
    fn root_state_issues_persistent_device_certificate() -> Result<(), IdentityError> {
        let root_directory = tempdir()?;
        let device_directory = tempdir()?;
        let root = AccountRootState::create(root_directory.path())?;
        let account_id = root.account_id();
        let device = DeviceState::load_or_create(device_directory.path())?;
        let certificate = root.issue_device_certificate(
            device.identity().device_id(),
            device.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        device.install_certificate(&certificate)?;
        device.install_certificate(&certificate)?;
        drop(root);

        let reloaded_root = AccountRootState::load(root_directory.path())?;
        let reloaded_device = DeviceState::load_or_create(device_directory.path())?;
        let reloaded_certificate = reloaded_device.load_certificate()?;
        let authorization = verify_device_authorization(
            account_id,
            &reloaded_certificate,
            &[],
            &DeviceCapability::MESSAGING,
        )?;

        assert_eq!(reloaded_root.account_id(), account_id);
        assert_eq!(authorization.account_id(), account_id);
        assert_eq!(authorization.device_id(), device.identity().device_id());
        assert_eq!(authorization.certificate_authority_sequence(), 0);
        Ok(())
    }

    #[test]
    fn wrong_account_and_tampering_are_rejected() -> Result<(), IdentityError> {
        let first_directory = tempdir()?;
        let second_directory = tempdir()?;
        let first = AccountRootState::create(first_directory.path())?;
        let second = AccountRootState::create(second_directory.path())?;
        let device_id = crate::DeviceIdentity::generate()?.device_id();
        let certificate = first.issue_device_certificate(
            device_id,
            test_encryption_public_key()?,
            &DeviceCapability::MESSAGING,
        )?;

        assert!(matches!(
            certificate.verify_for_account(second.account_id()),
            Err(IdentityError::AccountMismatch { .. })
        ));

        let mut tampered = certificate;
        tampered.content.device_id = crate::DeviceIdentity::generate()?.device_id();
        assert!(tampered.verify().is_err());
        Ok(())
    }

    #[test]
    fn root_signed_revocation_permanently_rejects_device_key() -> Result<(), IdentityError> {
        let root_directory = tempdir()?;
        let root = AccountRootState::create(root_directory.path())?;
        let device_id = crate::DeviceIdentity::generate()?.device_id();
        let certificate = root.issue_device_certificate(
            device_id,
            test_encryption_public_key()?,
            &DeviceCapability::MESSAGING,
        )?;
        let revocation = root.revoke_device(device_id)?;
        let mut tampered_revocation = revocation.clone();
        tampered_revocation.content.device_id = crate::DeviceIdentity::generate()?.device_id();
        assert!(tampered_revocation.verify().is_err());

        assert_eq!(certificate.authority_sequence(), 0);
        assert_eq!(revocation.authority_sequence(), 1);
        assert!(matches!(
            verify_device_authorization(
                root.account_id(),
                &certificate,
                std::slice::from_ref(&revocation),
                &DeviceCapability::MESSAGING
            ),
            Err(IdentityError::DeviceRevoked(revoked)) if revoked == device_id
        ));

        let reissued = root.issue_device_certificate(
            device_id,
            test_encryption_public_key()?,
            &DeviceCapability::MESSAGING,
        )?;
        assert_eq!(reissued.authority_sequence(), 2);
        assert!(matches!(
            verify_device_authorization(
                root.account_id(),
                &reissued,
                &[revocation],
                &DeviceCapability::MESSAGING
            ),
            Err(IdentityError::DeviceRevoked(revoked)) if revoked == device_id
        ));
        Ok(())
    }

    #[test]
    fn missing_capability_and_mismatched_device_are_rejected() -> Result<(), IdentityError> {
        let root_directory = tempdir()?;
        let first_device_directory = tempdir()?;
        let second_device_directory = tempdir()?;
        let root = AccountRootState::create(root_directory.path())?;
        let first_device = DeviceState::load_or_create(first_device_directory.path())?;
        let second_device = DeviceState::load_or_create(second_device_directory.path())?;
        let certificate = root.issue_device_certificate(
            first_device.identity().device_id(),
            first_device.encryption().public_key(),
            &[DeviceCapability::SignEvents],
        )?;

        assert!(matches!(
            verify_device_authorization(
                root.account_id(),
                &certificate,
                &[],
                &[DeviceCapability::SyncHistory]
            ),
            Err(IdentityError::MissingDeviceCapability(
                DeviceCapability::SyncHistory
            ))
        ));
        assert!(matches!(
            second_device.install_certificate(&certificate),
            Err(IdentityError::DeviceCertificateDeviceMismatch { .. })
        ));
        Ok(())
    }

    #[test]
    fn duplicate_capabilities_and_root_recreation_are_rejected() -> Result<(), IdentityError> {
        let root_directory = tempdir()?;
        let root = AccountRootState::create(root_directory.path())?;
        let device_id = crate::DeviceIdentity::generate()?.device_id();

        assert!(matches!(
            root.issue_device_certificate(
                device_id,
                test_encryption_public_key()?,
                &[DeviceCapability::SignEvents, DeviceCapability::SignEvents]
            ),
            Err(IdentityError::DuplicateDeviceCapability)
        ));
        assert!(matches!(
            AccountRootState::create(root_directory.path()),
            Err(IdentityError::AccountRootAlreadyExists(_))
        ));
        Ok(())
    }

    #[test]
    fn authority_snapshot_is_complete_durable_and_enforces_revocation() -> Result<(), IdentityError>
    {
        let root_directory = tempdir()?;
        let root = AccountRootState::create(root_directory.path())?;
        let allowed_device = crate::DeviceIdentity::generate()?;
        let revoked_device = crate::DeviceIdentity::generate()?;
        let allowed_certificate = root.issue_device_certificate(
            allowed_device.device_id(),
            test_encryption_public_key()?,
            &DeviceCapability::MESSAGING,
        )?;
        let revoked_certificate = root.issue_device_certificate(
            revoked_device.device_id(),
            test_encryption_public_key()?,
            &DeviceCapability::MESSAGING,
        )?;
        root.revoke_device(revoked_device.device_id())?;
        drop(root);

        let reloaded = AccountRootState::load(root_directory.path())?;
        let snapshot = reloaded.authority_snapshot()?;
        assert_eq!(snapshot.revision(), 3);
        assert_eq!(snapshot.revocations().len(), 1);
        assert_eq!(
            snapshot.revocations()[0].device_id(),
            revoked_device.device_id()
        );
        verify_device_authorization_with_snapshot(
            reloaded.account_id(),
            &allowed_certificate,
            &snapshot,
            &DeviceCapability::MESSAGING,
        )?;
        assert!(matches!(
            verify_device_authorization_with_snapshot(
                reloaded.account_id(),
                &revoked_certificate,
                &snapshot,
                &DeviceCapability::MESSAGING,
            ),
            Err(IdentityError::DeviceRevoked(device_id))
                if device_id == revoked_device.device_id()
        ));
        assert!(matches!(
            reloaded.revoke_device(revoked_device.device_id()),
            Err(IdentityError::DeviceAlreadyRevoked(device_id))
                if device_id == revoked_device.device_id()
        ));
        Ok(())
    }

    #[test]
    fn root_publishes_one_complete_canonical_device_list_per_authority_revision()
    -> Result<(), IdentityError> {
        let root_directory = tempdir()?;
        let root = AccountRootState::create(root_directory.path())?;
        let first_device = crate::DeviceIdentity::generate()?;
        let second_device = crate::DeviceIdentity::generate()?;
        let first_certificate = root.issue_device_certificate(
            first_device.device_id(),
            test_encryption_public_key()?,
            &DeviceCapability::MESSAGING,
        )?;
        let second_certificate = root.issue_device_certificate(
            second_device.device_id(),
            test_encryption_public_key()?,
            &DeviceCapability::MESSAGING,
        )?;

        let published =
            root.publish_device_list(&[second_certificate.clone(), first_certificate.clone()])?;
        published.verify_for_account(root.account_id())?;
        assert_eq!(published.revision(), 2);
        assert_eq!(published.devices().len(), 2);
        assert!(published.devices()[0].device_id() < published.devices()[1].device_id());
        assert_eq!(
            AccountDeviceListSnapshot::decode_and_verify(&published.encode()?)?,
            published
        );
        assert!(matches!(
            root.publish_device_list(std::slice::from_ref(&first_certificate)),
            Err(IdentityError::AccountDeviceListAlreadyPublished(2))
        ));

        root.revoke_device(second_device.device_id())?;
        let updated = root.publish_device_list(std::slice::from_ref(&first_certificate))?;
        assert_eq!(updated.revision(), 3);
        assert_eq!(updated.devices(), std::slice::from_ref(&first_certificate));
        assert!(matches!(
            root.publish_device_list(&[first_certificate, second_certificate]),
            Err(IdentityError::DeviceRevoked(device_id))
                if device_id == second_device.device_id()
        ));
        Ok(())
    }

    #[test]
    fn device_snapshot_store_detects_rollback_and_equivocation() -> Result<(), IdentityError> {
        let root_directory = tempdir()?;
        let device_directory = tempdir()?;
        let peer_directory = tempdir()?;
        let root = AccountRootState::create(root_directory.path())?;
        let device = DeviceState::load_or_create(device_directory.path())?;
        let certificate = root.issue_device_certificate(
            device.identity().device_id(),
            device.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        device.install_certificate(&certificate)?;
        let first = root.authority_snapshot()?;
        assert_eq!(
            device.install_own_authority_snapshot(&first)?,
            AuthoritySnapshotStoreOutcome::Installed
        );
        assert_eq!(
            device.install_own_authority_snapshot(&first)?,
            AuthoritySnapshotStoreOutcome::Unchanged
        );

        root.issue_device_certificate(
            crate::DeviceIdentity::generate()?.device_id(),
            test_encryption_public_key()?,
            &DeviceCapability::MESSAGING,
        )?;
        let second = root.authority_snapshot()?;
        assert_eq!(
            device.install_own_authority_snapshot(&second)?,
            AuthoritySnapshotStoreOutcome::Updated
        );
        assert!(matches!(
            device.install_own_authority_snapshot(&first),
            Err(IdentityError::AuthoritySnapshotRollback { .. })
        ));

        let peer = DeviceState::load_or_create(peer_directory.path())?;
        assert_eq!(
            peer.pin_peer_authority_snapshot(&second)?,
            AuthoritySnapshotStoreOutcome::Installed
        );
        let alternate_revocation = DeviceRevocation::issue(
            &root.identity,
            crate::DeviceIdentity::generate()?.device_id(),
            0,
        )?;
        let conflicting = AccountAuthoritySnapshot::issue(
            &root.identity,
            second.revision(),
            vec![alternate_revocation],
        )?;
        assert!(matches!(
            peer.pin_peer_authority_snapshot(&conflicting),
            Err(IdentityError::AuthoritySnapshotEquivocation { .. })
        ));
        Ok(())
    }

    #[test]
    fn legacy_root_with_prior_operations_cannot_claim_complete_snapshot()
    -> Result<(), IdentityError> {
        let root_directory = tempdir()?;
        let root = AccountRootState::create(root_directory.path())?;
        root.issue_device_certificate(
            crate::DeviceIdentity::generate()?.device_id(),
            test_encryption_public_key()?,
            &DeviceCapability::MESSAGING,
        )?;
        fs::remove_file(root_directory.path().join(AUTHORITY_LOG_VERSION_FILE))?;

        assert!(matches!(
            root.authority_snapshot(),
            Err(IdentityError::LegacyAuthorityState)
        ));
        Ok(())
    }

    #[test]
    fn device_enrollment_is_complete_idempotent_and_conflict_safe() -> Result<(), IdentityError> {
        let root_directory = tempdir()?;
        let first_directory = tempdir()?;
        let joining_directory = tempdir()?;
        let root = AccountRootState::create(root_directory.path())?;
        let first = DeviceState::load_or_create(first_directory.path())?;
        let first_certificate = root.issue_device_certificate(
            first.identity().device_id(),
            first.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        root.publish_device_list(std::slice::from_ref(&first_certificate))?;
        let joining = DeviceState::load_or_create(joining_directory.path())?;

        let (certificate, list) = root.enroll_device(
            joining.identity().device_id(),
            joining.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        assert_eq!(list.revision(), 2);
        assert_eq!(list.devices().len(), 2);
        assert_eq!(
            list.certificate_for(joining.identity().device_id()),
            Some(&certificate)
        );
        let (repeated, repeated_list) = root.enroll_device(
            joining.identity().device_id(),
            joining.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        assert_eq!(repeated, certificate);
        assert_eq!(repeated_list, list);
        assert!(matches!(
            root.enroll_device(
                joining.identity().device_id(),
                test_encryption_public_key()?,
                &DeviceCapability::MESSAGING,
            ),
            Err(IdentityError::DeviceEnrollmentIdentityConflict(id))
                if id == joining.identity().device_id()
        ));
        Ok(())
    }

    #[test]
    fn conversation_membership_is_durable_add_only_and_rollback_safe() -> Result<(), IdentityError>
    {
        let owner_directory = tempdir()?;
        let device_directory = tempdir()?;
        let first_member_directory = tempdir()?;
        let second_member_directory = tempdir()?;
        let third_member_directory = tempdir()?;
        let owner = AccountRootState::create(owner_directory.path())?;
        let first_member = AccountRootState::create(first_member_directory.path())?;
        let second_member = AccountRootState::create(second_member_directory.path())?;
        let third_member = AccountRootState::create(third_member_directory.path())?;
        let device = DeviceState::load_or_create(device_directory.path())?;
        let conversation_id = ConversationScopeId::from_bytes([42; 32]);

        assert!(matches!(
            ConversationMembershipSnapshot::issue(
                &owner.identity,
                conversation_id,
                0,
                vec![owner.account_id()],
            ),
            Err(IdentityError::InvalidConversationMembershipRevision)
        ));

        let first = owner.create_conversation_membership(
            conversation_id,
            &[first_member.account_id(), first_member.account_id()],
        )?;
        assert_eq!(first.revision(), 1);
        assert_eq!(first.owner_account_id(), owner.account_id());
        assert_eq!(first.members().len(), 2);
        assert_eq!(
            device.install_conversation_membership(&first)?,
            ConversationMembershipStoreOutcome::Installed
        );
        assert_eq!(
            device.install_conversation_membership(&first)?,
            ConversationMembershipStoreOutcome::Unchanged
        );

        let second = owner.add_conversation_members(
            conversation_id,
            &[second_member.account_id(), first_member.account_id()],
        )?;
        assert_eq!(second.revision(), 2);
        assert_eq!(second.members().len(), 3);
        assert_eq!(
            device.install_conversation_membership(&second)?,
            ConversationMembershipStoreOutcome::Updated
        );
        assert!(matches!(
            device.install_conversation_membership(&first),
            Err(IdentityError::ConversationMembershipRollback { .. })
        ));

        let conflicting = ConversationMembershipSnapshot::issue(
            &owner.identity,
            conversation_id,
            second.revision(),
            canonical_members(
                owner.account_id(),
                &[first_member.account_id(), third_member.account_id()],
            ),
        )?;
        assert!(matches!(
            device.install_conversation_membership(&conflicting),
            Err(IdentityError::ConversationMembershipEquivocation { .. })
        ));

        let removing = ConversationMembershipSnapshot::issue(
            &owner.identity,
            conversation_id,
            3,
            canonical_members(owner.account_id(), &[second_member.account_id()]),
        )?;
        assert!(matches!(
            device.install_conversation_membership(&removing),
            Err(IdentityError::ConversationMembershipNotAddOnly(id)) if id == conversation_id
        ));

        drop(owner);
        let reloaded = AccountRootState::load(owner_directory.path())?
            .load_root_conversation_membership(conversation_id)?;
        assert_eq!(reloaded, second);
        assert_eq!(
            device.load_conversation_membership(conversation_id)?,
            second
        );
        Ok(())
    }
}
