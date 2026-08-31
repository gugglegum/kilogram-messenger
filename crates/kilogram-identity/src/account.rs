use std::{
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    str::FromStr,
};

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::{DeviceId, DeviceState, IdentityError, SECRET_KEY_BYTES, open_new_secret_file};

const ACCOUNT_ROOT_SECRET_FILE: &str = "account-root-secret.key";
const NEXT_AUTHORITY_SEQUENCE_FILE: &str = "next-authority-sequence";
const AUTHORITY_LOG_VERSION_FILE: &str = "authority-log-version";
const AUTHORITY_LOG_VERSION: &str = "1";
const REVOCATIONS_DIRECTORY: &str = "revocations";
const CONVERSATION_MEMBERSHIPS_DIRECTORY: &str = "conversation-memberships";
const DEVICE_CERTIFICATE_FILE: &str = "device-certificate.cert";
const ACCOUNT_AUTHORITY_SNAPSHOT_FILE: &str = "account-authority.snapshot";
const PEER_AUTHORITY_DIRECTORY: &str = "peer-authority";
const AUTHORITY_VERSION: u8 = 1;
const DEVICE_CERTIFICATE_SIGNATURE_DOMAIN: &[u8] = b"kilogram:device-certificate-signature:v1\0";
const DEVICE_REVOCATION_SIGNATURE_DOMAIN: &[u8] = b"kilogram:device-revocation-signature:v1\0";
const AUTHORITY_SNAPSHOT_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:account-authority-snapshot-signature:v1\0";
const CONVERSATION_MEMBERSHIP_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:conversation-membership-signature:v1\0";

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

    fn from_secret_bytes(secret: [u8; SECRET_KEY_BYTES]) -> Self {
        Self {
            signing_key: SigningKey::from_bytes(&secret),
        }
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

pub struct AccountRootState {
    directory: PathBuf,
    identity: AccountRootIdentity,
}

impl AccountRootState {
    pub fn create(directory: impl AsRef<Path>) -> Result<Self, IdentityError> {
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
        let identity = AccountRootIdentity::generate()?;
        file.write_all(&identity.secret_bytes())?;
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
        })
    }

    pub fn load(directory: impl AsRef<Path>) -> Result<Self, IdentityError> {
        let directory = directory.as_ref().to_path_buf();
        let secret_path = directory.join(ACCOUNT_ROOT_SECRET_FILE);
        let bytes = match fs::read(&secret_path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(IdentityError::AccountRootMissing(secret_path));
            }
            Err(error) => return Err(error.into()),
        };
        let secret: [u8; SECRET_KEY_BYTES] = bytes.try_into().map_err(|bytes: Vec<u8>| {
            IdentityError::InvalidAccountRootSecretKeyLength(bytes.len())
        })?;
        Ok(Self {
            directory,
            identity: AccountRootIdentity::from_secret_bytes(secret),
        })
    }

    pub fn account_id(&self) -> AccountId {
        self.identity.account_id()
    }

    pub fn issue_device_certificate(
        &self,
        device_id: DeviceId,
        capabilities: &[DeviceCapability],
    ) -> Result<DeviceCertificate, IdentityError> {
        self.ensure_authority_log_ready()?;
        validate_requested_capabilities(capabilities)?;
        let authority_sequence = self.allocate_authority_sequence()?;
        DeviceCertificate::issue(&self.identity, device_id, authority_sequence, capabilities)
    }

    pub fn revoke_device(&self, device_id: DeviceId) -> Result<DeviceRevocation, IdentityError> {
        self.ensure_authority_log_ready()?;
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

    pub fn create_conversation_membership(
        &self,
        conversation_id: ConversationScopeId,
        members: &[AccountId],
    ) -> Result<ConversationMembershipSnapshot, IdentityError> {
        self.ensure_authority_log_ready()?;
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
        authority_sequence: u64,
        capabilities: &[DeviceCapability],
    ) -> Result<Self, IdentityError> {
        validate_requested_capabilities(capabilities)?;
        let mut capabilities = capabilities.to_vec();
        capabilities.sort_unstable();
        let content = DeviceCertificateContent {
            version: AUTHORITY_VERSION,
            account_id: root.account_id(),
            device_id,
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
    validate_authority_version(content.version)?;
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

    #[test]
    fn root_state_issues_persistent_device_certificate() -> Result<(), IdentityError> {
        let root_directory = tempdir()?;
        let device_directory = tempdir()?;
        let root = AccountRootState::create(root_directory.path())?;
        let account_id = root.account_id();
        let device = DeviceState::load_or_create(device_directory.path())?;
        let certificate = root.issue_device_certificate(
            device.identity().device_id(),
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
        let certificate =
            first.issue_device_certificate(device_id, &DeviceCapability::MESSAGING)?;

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
        let certificate = root.issue_device_certificate(device_id, &DeviceCapability::MESSAGING)?;
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

        let reissued = root.issue_device_certificate(device_id, &DeviceCapability::MESSAGING)?;
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
        let allowed_certificate = root
            .issue_device_certificate(allowed_device.device_id(), &DeviceCapability::MESSAGING)?;
        let revoked_certificate = root
            .issue_device_certificate(revoked_device.device_id(), &DeviceCapability::MESSAGING)?;
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
    fn device_snapshot_store_detects_rollback_and_equivocation() -> Result<(), IdentityError> {
        let root_directory = tempdir()?;
        let device_directory = tempdir()?;
        let peer_directory = tempdir()?;
        let root = AccountRootState::create(root_directory.path())?;
        let device = DeviceState::load_or_create(device_directory.path())?;
        let certificate = root.issue_device_certificate(
            device.identity().device_id(),
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
