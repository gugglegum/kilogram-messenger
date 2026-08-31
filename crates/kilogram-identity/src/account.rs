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
const DEVICE_CERTIFICATE_FILE: &str = "device-certificate.cert";
const AUTHORITY_VERSION: u8 = 1;
const DEVICE_CERTIFICATE_SIGNATURE_DOMAIN: &[u8] = b"kilogram:device-certificate-signature:v1\0";
const DEVICE_REVOCATION_SIGNATURE_DOMAIN: &[u8] = b"kilogram:device-revocation-signature:v1\0";

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
        validate_requested_capabilities(capabilities)?;
        let authority_sequence = self.allocate_authority_sequence()?;
        DeviceCertificate::issue(&self.identity, device_id, authority_sequence, capabilities)
    }

    pub fn revoke_device(&self, device_id: DeviceId) -> Result<DeviceRevocation, IdentityError> {
        let authority_sequence = self.allocate_authority_sequence()?;
        DeviceRevocation::issue(&self.identity, device_id, authority_sequence)
    }

    fn allocate_authority_sequence(&self) -> Result<u64, IdentityError> {
        let sequence_path = self.directory.join(NEXT_AUTHORITY_SEQUENCE_FILE);
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
}
