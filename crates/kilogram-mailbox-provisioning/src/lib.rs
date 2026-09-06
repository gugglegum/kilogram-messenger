//! Recipient-bound provisioning for blind mailbox capabilities.
//!
//! A mailbox write capability is a spam authority and therefore must never be
//! published in a connection ticket or a public endpoint announcement. This
//! crate keeps the full local read/write capability encrypted to its owning
//! Device and exports only the write half inside a Device-signed HPKE envelope
//! addressed to one exact peer Device.

use std::{collections::BTreeMap, fmt, net::IpAddr};

use anyhow::{Context, Result, bail, ensure};
use kilogram_crypto::{DeviceEncryptionIdentity, EncryptionPublicKey, SealedMessage};
use kilogram_identity::{AccountId, DeviceCertificate, DeviceId, DeviceIdentity};
use kilogram_mailbox::{
    MailboxAddress, MailboxReadCapability, MailboxStoreKey, MailboxWriteCapability,
};
use serde::{Deserialize, Serialize};
use url::Url;
use zeroize::Zeroize;

const VERSION: u8 = 1;
const KEY_BYTES: usize = 32;
const MAX_SERVICE_URL_BYTES: usize = 2_048;
const MAX_PROVISIONING_BYTES: usize = 16 * 1024;
const LOCAL_SIGNATURE_DOMAIN: &[u8] = b"kilogram:mailbox-local-binding:v1\0";
const OFFER_SIGNATURE_DOMAIN: &[u8] = b"kilogram:mailbox-device-offer:v1\0";
const LOCAL_HPKE_INFO: &[u8] = b"kilogram:mailbox-local-binding-hpke:v1\0";
const OFFER_HPKE_INFO: &[u8] = b"kilogram:mailbox-device-offer-hpke:v1\0";
const BINDING_ID_DOMAIN: &[u8] = b"kilogram:mailbox-binding-id:v1\0";
const CAPABILITY_UPDATE_VERSION: u8 = 1;
const CAPABILITY_UPDATE_SIGNATURE_DOMAIN: &[u8] = b"kilogram:mailbox-capability-update:v1\0";
const CAPABILITY_UPDATE_ID_DOMAIN: &[u8] = b"kilogram:mailbox-capability-update-id:v1\0";
const CAPABILITY_ACKNOWLEDGEMENT_VERSION: u8 = 1;
const CAPABILITY_ACKNOWLEDGEMENT_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-mailbox-capability-acknowledgement:v1\0";
pub const MAX_MAILBOX_CAPABILITY_UPDATE_BYTES: usize = 32 * 1024;
pub const MAX_MAILBOX_CAPABILITY_ACKNOWLEDGEMENT_BYTES: usize = 4 * 1024;

pub const MIN_MAILBOX_OFFER_VALIDITY_SECONDS: u64 = 60;
pub const MAX_MAILBOX_OFFER_VALIDITY_SECONDS: u64 = 30 * 24 * 60 * 60;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MailboxBindingId([u8; KEY_BYTES]);

impl MailboxBindingId {
    pub fn as_bytes(&self) -> &[u8; KEY_BYTES] {
        &self.0
    }
}

impl fmt::Display for MailboxBindingId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Opaque application scope supplied by the runtime (currently the
/// conversation ID). The provisioning layer deliberately does not depend on
/// the messaging protocol or expose it to the mailbox store.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MailboxScope([u8; KEY_BYTES]);

impl MailboxScope {
    pub fn from_bytes(bytes: [u8; KEY_BYTES]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; KEY_BYTES] {
        &self.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MailboxServiceDescriptor {
    base_url: String,
    expected_store_key: MailboxStoreKey,
}

impl MailboxServiceDescriptor {
    pub fn new(base_url: &str, expected_store_key: MailboxStoreKey) -> Result<Self> {
        let base_url = normalize_service_url(base_url)?;
        Ok(Self {
            base_url,
            expected_store_key,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn expected_store_key(&self) -> MailboxStoreKey {
        self.expected_store_key
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            normalize_service_url(&self.base_url)? == self.base_url,
            "mailbox service URL is not canonical"
        );
        let reparsed = self
            .expected_store_key
            .to_string()
            .parse::<MailboxStoreKey>()
            .context("mailbox store public key is invalid")?;
        ensure!(
            reparsed == self.expected_store_key,
            "mailbox store public key is not canonical"
        );
        Ok(())
    }
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
struct LocalBindingContent {
    version: u8,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    peer_account_id: AccountId,
    peer_device_id: DeviceId,
    scope: MailboxScope,
    service: MailboxServiceDescriptor,
    address: MailboxAddress,
    read_secret: [u8; KEY_BYTES],
    write_secret: [u8; KEY_BYTES],
    created_at_unix_seconds: u64,
}

impl Drop for LocalBindingContent {
    fn drop(&mut self) {
        self.read_secret.zeroize();
        self.write_secret.zeroize();
    }
}

#[derive(Deserialize, Serialize)]
struct SignedLocalBinding {
    content: LocalBindingContent,
    signature: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct LocalBindingHeader {
    version: u8,
    binding_id: MailboxBindingId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SealedLocalMailboxBinding {
    header: LocalBindingHeader,
    sealed: SealedMessage,
}

impl SealedLocalMailboxBinding {
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        owner_identity: &DeviceIdentity,
        owner_encryption: &DeviceEncryptionIdentity,
        owner_certificate: &DeviceCertificate,
        peer_certificate: &DeviceCertificate,
        scope: MailboxScope,
        service: MailboxServiceDescriptor,
        created_at_unix_seconds: u64,
    ) -> Result<Self> {
        validate_owner(owner_identity, owner_encryption, owner_certificate)?;
        peer_certificate.verify()?;
        ensure!(
            peer_certificate.account_id() != owner_certificate.account_id(),
            "contact mailbox peer must belong to another account"
        );
        service.validate()?;
        ensure!(
            created_at_unix_seconds != 0,
            "mailbox creation time is zero"
        );
        let read = MailboxReadCapability::generate()?;
        let write = MailboxWriteCapability::generate()?;
        let content = LocalBindingContent {
            version: VERSION,
            owner_account_id: owner_certificate.account_id(),
            owner_device_id: owner_certificate.device_id(),
            peer_account_id: peer_certificate.account_id(),
            peer_device_id: peer_certificate.device_id(),
            scope,
            service,
            address: MailboxAddress::new(read.read_key(), write.write_key()),
            read_secret: read.secret_bytes(),
            write_secret: write.secret_bytes(),
            created_at_unix_seconds,
        };
        content.validate()?;
        let binding_id = binding_id(&content)?;
        let signature = owner_identity
            .sign(&signing_bytes(LOCAL_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let signed = SignedLocalBinding { content, signature };
        let plaintext = zeroize::Zeroizing::new(
            postcard::to_allocvec(&signed).context("encode local mailbox binding")?,
        );
        ensure!(
            plaintext.len() <= MAX_PROVISIONING_BYTES,
            "local mailbox binding is too large"
        );
        let header = LocalBindingHeader {
            version: VERSION,
            binding_id,
        };
        let aad = postcard::to_allocvec(&header).context("encode local mailbox header")?;
        let sealed = owner_encryption
            .public_key()
            .seal(&plaintext, LOCAL_HPKE_INFO, &aad)
            .context("seal local mailbox capability")?;
        Ok(Self { header, sealed })
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate_shape()?;
        let bytes = postcard::to_allocvec(self).context("encode sealed local mailbox binding")?;
        ensure!(
            bytes.len() <= MAX_PROVISIONING_BYTES,
            "sealed local mailbox binding is too large"
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        validate_encoded_size(bytes, "sealed local mailbox binding")?;
        let binding: Self =
            postcard::from_bytes(bytes).context("decode sealed local mailbox binding")?;
        binding.validate_shape()?;
        Ok(binding)
    }

    pub fn binding_id(&self) -> MailboxBindingId {
        self.header.binding_id
    }

    pub fn open(
        &self,
        owner_identity: &DeviceIdentity,
        owner_encryption: &DeviceEncryptionIdentity,
        owner_certificate: &DeviceCertificate,
    ) -> Result<LocalMailboxBinding> {
        self.validate_shape()?;
        validate_owner(owner_identity, owner_encryption, owner_certificate)?;
        let aad = postcard::to_allocvec(&self.header).context("encode local mailbox header")?;
        let plaintext = zeroize::Zeroizing::new(
            owner_encryption
                .open(&self.sealed, LOCAL_HPKE_INFO, &aad)
                .context("open local mailbox capability")?,
        );
        validate_encoded_size(&plaintext, "opened local mailbox binding")?;
        let signed: SignedLocalBinding =
            postcard::from_bytes(&plaintext).context("decode opened local mailbox binding")?;
        signed.content.validate()?;
        ensure!(
            signed.content.owner_account_id == owner_certificate.account_id()
                && signed.content.owner_device_id == owner_certificate.device_id(),
            "local mailbox binding belongs to another owner"
        );
        signed.content.owner_device_id.verify(
            &signing_bytes(LOCAL_SIGNATURE_DOMAIN, &signed.content)?,
            &signed.signature,
        )?;
        ensure!(
            binding_id(&signed.content)? == self.binding_id(),
            "local mailbox binding ID mismatch"
        );
        Ok(LocalMailboxBinding {
            content: signed.content,
        })
    }

    fn validate_shape(&self) -> Result<()> {
        ensure!(
            self.header.version == VERSION
                && self.sealed.encapsulated_key.len() == KEY_BYTES
                && !self.sealed.ciphertext.is_empty(),
            "sealed local mailbox binding is invalid"
        );
        Ok(())
    }
}

pub struct LocalMailboxBinding {
    content: LocalBindingContent,
}

impl LocalMailboxBinding {
    pub fn binding_id(&self) -> Result<MailboxBindingId> {
        binding_id(&self.content)
    }

    pub fn owner_account_id(&self) -> AccountId {
        self.content.owner_account_id
    }

    pub fn owner_device_id(&self) -> DeviceId {
        self.content.owner_device_id
    }

    pub fn peer_account_id(&self) -> AccountId {
        self.content.peer_account_id
    }

    pub fn peer_device_id(&self) -> DeviceId {
        self.content.peer_device_id
    }

    pub fn scope(&self) -> MailboxScope {
        self.content.scope
    }

    pub fn service(&self) -> &MailboxServiceDescriptor {
        &self.content.service
    }

    pub fn address(&self) -> MailboxAddress {
        self.content.address
    }

    pub fn read_capability(&self) -> MailboxReadCapability {
        MailboxReadCapability::from_secret_bytes(self.content.read_secret)
    }

    pub fn write_capability(&self) -> MailboxWriteCapability {
        MailboxWriteCapability::from_secret_bytes(self.content.write_secret)
    }

    pub fn created_at_unix_seconds(&self) -> u64 {
        self.content.created_at_unix_seconds
    }

    pub fn offer_for(
        &self,
        owner_identity: &DeviceIdentity,
        owner_certificate: &DeviceCertificate,
        peer_certificate: &DeviceCertificate,
        expires_at_unix_seconds: u64,
    ) -> Result<EncryptedMailboxOffer> {
        ensure!(
            owner_identity.device_id() == self.owner_device_id()
                && owner_certificate.account_id() == self.owner_account_id()
                && owner_certificate.device_id() == self.owner_device_id()
                && peer_certificate.account_id() == self.peer_account_id()
                && peer_certificate.device_id() == self.peer_device_id(),
            "mailbox offer certificates do not match the local binding"
        );
        owner_certificate.verify()?;
        peer_certificate.verify()?;
        let validity = expires_at_unix_seconds
            .checked_sub(self.created_at_unix_seconds())
            .context("mailbox offer expires before it was created")?;
        ensure!(
            (MIN_MAILBOX_OFFER_VALIDITY_SECONDS..=MAX_MAILBOX_OFFER_VALIDITY_SECONDS)
                .contains(&validity),
            "mailbox offer validity is outside protocol bounds"
        );
        let content = MailboxOfferContent {
            version: VERSION,
            owner_account_id: self.owner_account_id(),
            owner_device_id: self.owner_device_id(),
            owner_encryption_public_key: owner_certificate.encryption_public_key(),
            recipient_account_id: self.peer_account_id(),
            recipient_device_id: self.peer_device_id(),
            scope: self.scope(),
            service: self.service().clone(),
            address: self.address(),
            write_secret: self.content.write_secret,
            created_at_unix_seconds: self.created_at_unix_seconds(),
            expires_at_unix_seconds,
        };
        content.validate()?;
        let signature = owner_identity
            .sign(&signing_bytes(OFFER_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let signed = SignedMailboxOffer { content, signature };
        let plaintext = zeroize::Zeroizing::new(
            postcard::to_allocvec(&signed).context("encode mailbox offer")?,
        );
        ensure!(
            plaintext.len() <= MAX_PROVISIONING_BYTES,
            "mailbox offer is too large"
        );
        let header = MailboxOfferHeader {
            version: VERSION,
            binding_id: self.binding_id()?,
        };
        let aad = postcard::to_allocvec(&header).context("encode mailbox offer header")?;
        let sealed = peer_certificate
            .encryption_public_key()
            .seal(&plaintext, OFFER_HPKE_INFO, &aad)
            .context("seal mailbox offer to peer Device")?;
        Ok(EncryptedMailboxOffer { header, sealed })
    }
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
struct MailboxOfferContent {
    version: u8,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    owner_encryption_public_key: EncryptionPublicKey,
    recipient_account_id: AccountId,
    recipient_device_id: DeviceId,
    scope: MailboxScope,
    service: MailboxServiceDescriptor,
    address: MailboxAddress,
    write_secret: [u8; KEY_BYTES],
    created_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
}

impl Drop for MailboxOfferContent {
    fn drop(&mut self) {
        self.write_secret.zeroize();
    }
}

impl MailboxOfferContent {
    fn validate(&self) -> Result<()> {
        ensure!(self.version == VERSION, "unsupported mailbox offer version");
        ensure!(
            self.owner_account_id != self.recipient_account_id,
            "contact mailbox offer cannot target the owner account"
        );
        self.service.validate()?;
        self.address.verify()?;
        ensure!(
            MailboxWriteCapability::from_secret_bytes(self.write_secret).write_key()
                == self.address.write_key(),
            "mailbox offer write secret does not match its address"
        );
        let validity = self
            .expires_at_unix_seconds
            .checked_sub(self.created_at_unix_seconds)
            .context("mailbox offer expires before it was created")?;
        ensure!(
            (MIN_MAILBOX_OFFER_VALIDITY_SECONDS..=MAX_MAILBOX_OFFER_VALIDITY_SECONDS)
                .contains(&validity),
            "mailbox offer validity is outside protocol bounds"
        );
        Ok(())
    }
}

impl LocalBindingContent {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.version == VERSION,
            "unsupported local mailbox binding version"
        );
        ensure!(
            self.owner_account_id != self.peer_account_id,
            "contact mailbox binding cannot target the owner account"
        );
        ensure!(
            self.created_at_unix_seconds != 0,
            "mailbox creation time is zero"
        );
        self.service.validate()?;
        self.address.verify()?;
        ensure!(
            MailboxReadCapability::from_secret_bytes(self.read_secret).read_key()
                == self.address.read_key()
                && MailboxWriteCapability::from_secret_bytes(self.write_secret).write_key()
                    == self.address.write_key(),
            "local mailbox secrets do not match the address"
        );
        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
struct SignedMailboxOffer {
    content: MailboxOfferContent,
    signature: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct MailboxOfferHeader {
    version: u8,
    binding_id: MailboxBindingId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EncryptedMailboxOffer {
    header: MailboxOfferHeader,
    sealed: SealedMessage,
}

impl EncryptedMailboxOffer {
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate_shape()?;
        let bytes = postcard::to_allocvec(self).context("encode encrypted mailbox offer")?;
        ensure!(
            bytes.len() <= MAX_PROVISIONING_BYTES,
            "mailbox offer is too large"
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        validate_encoded_size(bytes, "encrypted mailbox offer")?;
        let offer: Self = postcard::from_bytes(bytes).context("decode encrypted mailbox offer")?;
        offer.validate_shape()?;
        Ok(offer)
    }

    pub fn binding_id(&self) -> MailboxBindingId {
        self.header.binding_id
    }

    pub fn open(
        &self,
        recipient_encryption: &DeviceEncryptionIdentity,
        recipient_certificate: &DeviceCertificate,
        expected_owner_certificate: &DeviceCertificate,
        expected_scope: MailboxScope,
        now_unix_seconds: u64,
    ) -> Result<PeerMailboxBinding> {
        self.validate_shape()?;
        recipient_certificate.verify()?;
        expected_owner_certificate.verify()?;
        ensure!(
            recipient_encryption.public_key() == recipient_certificate.encryption_public_key(),
            "mailbox recipient encryption identity does not match its Device certificate"
        );
        let aad = postcard::to_allocvec(&self.header).context("encode mailbox offer header")?;
        let plaintext = zeroize::Zeroizing::new(
            recipient_encryption
                .open(&self.sealed, OFFER_HPKE_INFO, &aad)
                .context("open mailbox offer")?,
        );
        validate_encoded_size(&plaintext, "opened mailbox offer")?;
        let signed: SignedMailboxOffer =
            postcard::from_bytes(&plaintext).context("decode opened mailbox offer")?;
        signed.content.validate()?;
        ensure!(
            signed.content.owner_account_id == expected_owner_certificate.account_id()
                && signed.content.owner_device_id == expected_owner_certificate.device_id()
                && signed.content.owner_encryption_public_key
                    == expected_owner_certificate.encryption_public_key(),
            "mailbox offer owner does not match the expected Device certificate"
        );
        ensure!(
            signed.content.recipient_account_id == recipient_certificate.account_id()
                && signed.content.recipient_device_id == recipient_certificate.device_id()
                && signed.content.scope == expected_scope,
            "mailbox offer recipient or application scope mismatch"
        );
        ensure!(
            signed.content.expires_at_unix_seconds > now_unix_seconds,
            "mailbox offer has expired"
        );
        signed.content.owner_device_id.verify(
            &signing_bytes(OFFER_SIGNATURE_DOMAIN, &signed.content)?,
            &signed.signature,
        )?;
        let expected_binding_id = offer_binding_id(&signed.content)?;
        ensure!(
            expected_binding_id == self.binding_id(),
            "mailbox offer binding ID mismatch"
        );
        Ok(PeerMailboxBinding {
            content: signed.content,
        })
    }

    fn validate_shape(&self) -> Result<()> {
        ensure!(
            self.header.version == VERSION
                && self.sealed.encapsulated_key.len() == KEY_BYTES
                && !self.sealed.ciphertext.is_empty(),
            "encrypted mailbox offer is invalid"
        );
        Ok(())
    }
}

pub struct PeerMailboxBinding {
    content: MailboxOfferContent,
}

impl PeerMailboxBinding {
    pub fn binding_id(&self) -> Result<MailboxBindingId> {
        offer_binding_id(&self.content)
    }

    pub fn owner_account_id(&self) -> AccountId {
        self.content.owner_account_id
    }

    pub fn owner_device_id(&self) -> DeviceId {
        self.content.owner_device_id
    }

    pub fn recipient_account_id(&self) -> AccountId {
        self.content.recipient_account_id
    }

    pub fn recipient_device_id(&self) -> DeviceId {
        self.content.recipient_device_id
    }

    pub fn recipient_encryption_public_key(&self) -> EncryptionPublicKey {
        self.content.owner_encryption_public_key
    }

    pub fn scope(&self) -> MailboxScope {
        self.content.scope
    }

    pub fn service(&self) -> &MailboxServiceDescriptor {
        &self.content.service
    }

    pub fn address(&self) -> MailboxAddress {
        self.content.address
    }

    pub fn write_capability(&self) -> MailboxWriteCapability {
        MailboxWriteCapability::from_secret_bytes(self.content.write_secret)
    }

    pub fn created_at_unix_seconds(&self) -> u64 {
        self.content.created_at_unix_seconds
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.content.expires_at_unix_seconds
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MailboxCapabilityUpdateId([u8; KEY_BYTES]);

impl MailboxCapabilityUpdateId {
    pub fn as_bytes(&self) -> &[u8; KEY_BYTES] {
        &self.0
    }
}

impl fmt::Display for MailboxCapabilityUpdateId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum MailboxCapabilityUpdateAction {
    Activate {
        binding_id: MailboxBindingId,
        encrypted_offer: Vec<u8>,
    },
    Revoke {
        binding_id: MailboxBindingId,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct MailboxCapabilityUpdateContent {
    version: u8,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    recipient_account_id: AccountId,
    recipient_device_id: DeviceId,
    scope: MailboxScope,
    generation: u64,
    previous_update_id: Option<MailboxCapabilityUpdateId>,
    created_at_unix_seconds: u64,
    action: MailboxCapabilityUpdateAction,
}

/// Device-signed ordered lifecycle for one recipient-bound mailbox capability.
///
/// Updates are sent only inside an already authenticated encrypted Device
/// session. The active offer remains independently HPKE-encrypted to the exact
/// recipient Device, while the signed generation chain prevents rollback,
/// gaps and same-generation forks in retained local state.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedMailboxCapabilityUpdate {
    content: MailboxCapabilityUpdateContent,
    signature: Vec<u8>,
}

impl SignedMailboxCapabilityUpdate {
    #[allow(clippy::too_many_arguments)]
    pub fn activate(
        owner_identity: &DeviceIdentity,
        owner_certificate: &DeviceCertificate,
        recipient_certificate: &DeviceCertificate,
        scope: MailboxScope,
        generation: u64,
        previous_update_id: Option<MailboxCapabilityUpdateId>,
        created_at_unix_seconds: u64,
        offer: &EncryptedMailboxOffer,
    ) -> Result<Self> {
        owner_certificate.verify()?;
        recipient_certificate.verify()?;
        ensure!(
            owner_identity.device_id() == owner_certificate.device_id(),
            "mailbox capability update signer does not match its owner certificate"
        );
        let content = MailboxCapabilityUpdateContent {
            version: CAPABILITY_UPDATE_VERSION,
            owner_account_id: owner_certificate.account_id(),
            owner_device_id: owner_certificate.device_id(),
            recipient_account_id: recipient_certificate.account_id(),
            recipient_device_id: recipient_certificate.device_id(),
            scope,
            generation,
            previous_update_id,
            created_at_unix_seconds,
            action: MailboxCapabilityUpdateAction::Activate {
                binding_id: offer.binding_id(),
                encrypted_offer: offer.encode()?,
            },
        };
        Self::sign(owner_identity, content)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn revoke(
        owner_identity: &DeviceIdentity,
        owner_certificate: &DeviceCertificate,
        recipient_certificate: &DeviceCertificate,
        scope: MailboxScope,
        generation: u64,
        previous_update_id: MailboxCapabilityUpdateId,
        revoked_binding_id: MailboxBindingId,
        created_at_unix_seconds: u64,
    ) -> Result<Self> {
        owner_certificate.verify()?;
        recipient_certificate.verify()?;
        ensure!(
            owner_identity.device_id() == owner_certificate.device_id(),
            "mailbox capability revocation signer does not match its owner certificate"
        );
        let content = MailboxCapabilityUpdateContent {
            version: CAPABILITY_UPDATE_VERSION,
            owner_account_id: owner_certificate.account_id(),
            owner_device_id: owner_certificate.device_id(),
            recipient_account_id: recipient_certificate.account_id(),
            recipient_device_id: recipient_certificate.device_id(),
            scope,
            generation,
            previous_update_id: Some(previous_update_id),
            created_at_unix_seconds,
            action: MailboxCapabilityUpdateAction::Revoke {
                binding_id: revoked_binding_id,
            },
        };
        Self::sign(owner_identity, content)
    }

    fn sign(identity: &DeviceIdentity, content: MailboxCapabilityUpdateContent) -> Result<Self> {
        let signature = identity
            .sign(&signing_bytes(
                CAPABILITY_UPDATE_SIGNATURE_DOMAIN,
                &content,
            )?)
            .to_vec();
        let update = Self { content, signature };
        update.verify_signature()?;
        Ok(update)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify_signature()?;
        let bytes = postcard::to_allocvec(self).context("encode mailbox capability update")?;
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_MAILBOX_CAPABILITY_UPDATE_BYTES,
            "mailbox capability update is too large"
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_MAILBOX_CAPABILITY_UPDATE_BYTES,
            "mailbox capability update size is invalid"
        );
        let update: Self =
            postcard::from_bytes(bytes).context("decode mailbox capability update")?;
        update.verify_signature()?;
        Ok(update)
    }

    pub fn verify_signature(&self) -> Result<()> {
        ensure!(
            self.content.version == CAPABILITY_UPDATE_VERSION
                && self.owner_account_id() != self.recipient_account_id()
                && self.owner_device_id() != self.recipient_device_id()
                && self.generation() != 0
                && self.created_at_unix_seconds() != 0
                && ((self.generation() == 1 && self.previous_update_id().is_none())
                    || (self.generation() > 1 && self.previous_update_id().is_some())),
            "mailbox capability update metadata is invalid"
        );
        match &self.content.action {
            MailboxCapabilityUpdateAction::Activate {
                binding_id,
                encrypted_offer,
            } => {
                let offer = EncryptedMailboxOffer::decode(encrypted_offer)?;
                ensure!(
                    offer.binding_id() == *binding_id,
                    "mailbox capability update offer binding changed"
                );
            }
            MailboxCapabilityUpdateAction::Revoke { .. } => ensure!(
                self.generation() > 1,
                "initial mailbox capability update cannot be a revocation"
            ),
        }
        self.owner_device_id()
            .verify(
                &signing_bytes(CAPABILITY_UPDATE_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify mailbox capability update signature")
    }

    pub fn verify_chain_link(&self, previous: Option<&Self>) -> Result<()> {
        self.verify_signature()?;
        match previous {
            None => ensure!(
                self.generation() == 1
                    && self.previous_update_id().is_none()
                    && self.offer()?.is_some(),
                "mailbox capability chain must begin with generation-one activation"
            ),
            Some(previous) => {
                previous.verify_signature()?;
                let expected_generation = previous
                    .generation()
                    .checked_add(1)
                    .context("mailbox capability generation overflows")?;
                ensure!(
                    self.owner_account_id() == previous.owner_account_id()
                        && self.owner_device_id() == previous.owner_device_id()
                        && self.recipient_account_id() == previous.recipient_account_id()
                        && self.recipient_device_id() == previous.recipient_device_id()
                        && self.scope() == previous.scope()
                        && self.generation() == expected_generation
                        && self.previous_update_id() == Some(previous.update_id()?),
                    "mailbox capability update is not the exact next chain generation"
                );
                ensure!(
                    self.created_at_unix_seconds() >= previous.created_at_unix_seconds(),
                    "mailbox capability update creation time regressed"
                );
                if self.is_revocation() {
                    ensure!(
                        !previous.is_revocation() && self.binding_id() == previous.binding_id(),
                        "mailbox capability revocation does not target the active predecessor"
                    );
                } else {
                    ensure!(
                        self.binding_id() != previous.binding_id(),
                        "mailbox capability rotation must install a different binding"
                    );
                }
            }
        }
        Ok(())
    }

    pub fn update_id(&self) -> Result<MailboxCapabilityUpdateId> {
        self.verify_signature()?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(CAPABILITY_UPDATE_ID_DOMAIN);
        hasher.update(&postcard::to_allocvec(&self.content)?);
        hasher.update(&self.signature);
        Ok(MailboxCapabilityUpdateId(*hasher.finalize().as_bytes()))
    }

    pub fn owner_account_id(&self) -> AccountId {
        self.content.owner_account_id
    }

    pub fn owner_device_id(&self) -> DeviceId {
        self.content.owner_device_id
    }

    pub fn recipient_account_id(&self) -> AccountId {
        self.content.recipient_account_id
    }

    pub fn recipient_device_id(&self) -> DeviceId {
        self.content.recipient_device_id
    }

    pub fn scope(&self) -> MailboxScope {
        self.content.scope
    }

    pub fn generation(&self) -> u64 {
        self.content.generation
    }

    pub fn previous_update_id(&self) -> Option<MailboxCapabilityUpdateId> {
        self.content.previous_update_id
    }

    pub fn created_at_unix_seconds(&self) -> u64 {
        self.content.created_at_unix_seconds
    }

    pub fn binding_id(&self) -> MailboxBindingId {
        match self.content.action {
            MailboxCapabilityUpdateAction::Activate { binding_id, .. }
            | MailboxCapabilityUpdateAction::Revoke { binding_id } => binding_id,
        }
    }

    pub fn is_revocation(&self) -> bool {
        matches!(
            self.content.action,
            MailboxCapabilityUpdateAction::Revoke { .. }
        )
    }

    pub fn offer(&self) -> Result<Option<EncryptedMailboxOffer>> {
        match &self.content.action {
            MailboxCapabilityUpdateAction::Activate {
                encrypted_offer, ..
            } => Ok(Some(EncryptedMailboxOffer::decode(encrypted_offer)?)),
            MailboxCapabilityUpdateAction::Revoke { .. } => Ok(None),
        }
    }
}

/// Opaque binding to the authenticated transport session that carried a
/// capability update. The provisioning layer intentionally treats these bytes
/// as an application-supplied value and has no transport dependency.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MailboxCapabilitySessionBinding([u8; KEY_BYTES]);

impl MailboxCapabilitySessionBinding {
    pub fn from_bytes(bytes: [u8; KEY_BYTES]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; KEY_BYTES] {
        &self.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct MailboxCapabilityAcknowledgementContent {
    version: u8,
    session_binding: MailboxCapabilitySessionBinding,
    update_id: MailboxCapabilityUpdateId,
    owner_account_id: AccountId,
    owner_device_id: DeviceId,
    recipient_account_id: AccountId,
    recipient_device_id: DeviceId,
    generation: u64,
    binding_id: MailboxBindingId,
    revoked: bool,
}

/// Recipient-signed proof that one exact ordered capability update was
/// accepted during one authenticated session.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedMailboxCapabilityAcknowledgement {
    content: MailboxCapabilityAcknowledgementContent,
    signature: Vec<u8>,
}

impl SignedMailboxCapabilityAcknowledgement {
    pub fn sign(
        recipient_identity: &DeviceIdentity,
        session_binding: MailboxCapabilitySessionBinding,
        update: &SignedMailboxCapabilityUpdate,
    ) -> Result<Self> {
        update.verify_signature()?;
        ensure!(
            recipient_identity.device_id() == update.recipient_device_id(),
            "mailbox capability acknowledgement signer is not the update recipient"
        );
        let content = MailboxCapabilityAcknowledgementContent {
            version: CAPABILITY_ACKNOWLEDGEMENT_VERSION,
            session_binding,
            update_id: update.update_id()?,
            owner_account_id: update.owner_account_id(),
            owner_device_id: update.owner_device_id(),
            recipient_account_id: update.recipient_account_id(),
            recipient_device_id: update.recipient_device_id(),
            generation: update.generation(),
            binding_id: update.binding_id(),
            revoked: update.is_revocation(),
        };
        let signature = recipient_identity
            .sign(&signing_bytes(
                CAPABILITY_ACKNOWLEDGEMENT_SIGNATURE_DOMAIN,
                &content,
            )?)
            .to_vec();
        let acknowledgement = Self { content, signature };
        acknowledgement.verify_signature()?;
        Ok(acknowledgement)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify_signature()?;
        let bytes =
            postcard::to_allocvec(self).context("encode mailbox capability acknowledgement")?;
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_MAILBOX_CAPABILITY_ACKNOWLEDGEMENT_BYTES,
            "mailbox capability acknowledgement is too large"
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_MAILBOX_CAPABILITY_ACKNOWLEDGEMENT_BYTES,
            "mailbox capability acknowledgement size is invalid"
        );
        let acknowledgement: Self =
            postcard::from_bytes(bytes).context("decode mailbox capability acknowledgement")?;
        acknowledgement.verify_signature()?;
        Ok(acknowledgement)
    }

    pub fn verify_for(
        &self,
        session_binding: MailboxCapabilitySessionBinding,
        update: &SignedMailboxCapabilityUpdate,
    ) -> Result<()> {
        self.verify_signature()?;
        update.verify_signature()?;
        ensure!(
            self.content.session_binding == session_binding
                && self.update_id() == update.update_id()?
                && self.content.owner_account_id == update.owner_account_id()
                && self.content.owner_device_id == update.owner_device_id()
                && self.content.recipient_account_id == update.recipient_account_id()
                && self.content.recipient_device_id == update.recipient_device_id()
                && self.content.generation == update.generation()
                && self.content.binding_id == update.binding_id()
                && self.content.revoked == update.is_revocation(),
            "mailbox capability acknowledgement does not match this update or session"
        );
        Ok(())
    }

    pub fn verify_signature(&self) -> Result<()> {
        ensure!(
            self.content.version == CAPABILITY_ACKNOWLEDGEMENT_VERSION
                && self.content.owner_account_id != self.content.recipient_account_id
                && self.content.owner_device_id != self.content.recipient_device_id
                && self.content.generation != 0,
            "mailbox capability acknowledgement metadata is invalid"
        );
        self.content.recipient_device_id.verify(
            &signing_bytes(CAPABILITY_ACKNOWLEDGEMENT_SIGNATURE_DOMAIN, &self.content)?,
            &self.signature,
        )?;
        Ok(())
    }

    pub fn update_id(&self) -> MailboxCapabilityUpdateId {
        self.content.update_id
    }

    pub fn session_binding(&self) -> MailboxCapabilitySessionBinding {
        self.content.session_binding
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MailboxCapabilityBindingState {
    /// No ordered lifecycle exists yet, so legacy bindings remain eligible.
    Unmanaged,
    /// The binding is the active chain head.
    Current,
    /// The binding is the active predecessor retained while its rotation has
    /// not yet been acknowledged by the recipient.
    RotationOverlap,
    /// The binding is revoked, superseded, or unrelated to this chain.
    Inactive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MailboxCapabilityInboundDisposition {
    AlreadyPresent,
    Append,
}

/// Transport-independent convergence view over one exact
/// owner/recipient/scope capability chain.
///
/// Callers reconstruct this value from durable signed updates and ACKs after
/// every restart. No timers, sockets, mutable hidden state, or delivery-side
/// effects participate in its decisions.
#[derive(Debug)]
pub struct MailboxCapabilityConvergence<'a> {
    ordered_updates: Vec<&'a SignedMailboxCapabilityUpdate>,
    acknowledgements:
        BTreeMap<MailboxCapabilityUpdateId, &'a SignedMailboxCapabilityAcknowledgement>,
}

impl<'a> MailboxCapabilityConvergence<'a> {
    pub fn new(
        updates: impl IntoIterator<Item = &'a SignedMailboxCapabilityUpdate>,
        acknowledgements: impl IntoIterator<Item = &'a SignedMailboxCapabilityAcknowledgement>,
    ) -> Result<Self> {
        let mut ordered_updates = updates.into_iter().collect::<Vec<_>>();
        ordered_updates.sort_by_key(|update| update.generation());
        let mut previous = None;
        let mut updates_by_id = BTreeMap::new();
        for update in &ordered_updates {
            update.verify_chain_link(previous)?;
            let update_id = update.update_id()?;
            ensure!(
                updates_by_id.insert(update_id, *update).is_none(),
                "duplicate mailbox capability update ID"
            );
            previous = Some(*update);
        }

        let mut acknowledgements_by_id = BTreeMap::new();
        for acknowledgement in acknowledgements {
            let update_id = acknowledgement.update_id();
            let update = updates_by_id
                .get(&update_id)
                .context("mailbox capability acknowledgement references an absent update")?;
            acknowledgement.verify_for(acknowledgement.session_binding(), update)?;
            ensure!(
                acknowledgements_by_id
                    .insert(update_id, acknowledgement)
                    .is_none(),
                "duplicate mailbox capability acknowledgement"
            );
        }

        Ok(Self {
            ordered_updates,
            acknowledgements: acknowledgements_by_id,
        })
    }

    /// Owner-side retained history cannot contain generation N+1 unless the
    /// recipient acknowledged generation N first.
    pub fn validate_owner_progression(&self) -> Result<()> {
        for update in self.ordered_updates.iter().skip(1) {
            let predecessor = update
                .previous_update_id()
                .context("non-initial mailbox capability update has no predecessor")?;
            ensure!(
                self.acknowledgements.contains_key(&predecessor),
                "mailbox capability chain advanced before its predecessor was acknowledged"
            );
        }
        Ok(())
    }

    pub fn head(&self) -> Option<&'a SignedMailboxCapabilityUpdate> {
        self.ordered_updates.last().copied()
    }

    /// Returns the one transition eligible for retry. Later generations stay
    /// blocked until their exact predecessor ACK has been durably retained.
    pub fn next_outbound_update(&self) -> Result<Option<&'a SignedMailboxCapabilityUpdate>> {
        self.validate_owner_progression()?;
        for update in &self.ordered_updates {
            let update_id = update.update_id()?;
            if !self.acknowledgements.contains_key(&update_id)
                && update
                    .previous_update_id()
                    .is_none_or(|predecessor| self.acknowledgements.contains_key(&predecessor))
            {
                return Ok(Some(*update));
            }
        }
        Ok(None)
    }

    pub fn is_fully_acknowledged(&self) -> Result<bool> {
        Ok(self.next_outbound_update()?.is_none()
            && self.ordered_updates.iter().all(|update| {
                update
                    .update_id()
                    .is_ok_and(|id| self.acknowledgements.contains_key(&id))
            }))
    }

    /// Binding eligibility on the capability owner's receive side. The old
    /// binding overlaps only while an active rotation head lacks its ACK.
    pub fn owner_receive_binding_state(
        &self,
        binding_id: MailboxBindingId,
    ) -> Result<MailboxCapabilityBindingState> {
        let Some(head) = self.head() else {
            return Ok(MailboxCapabilityBindingState::Unmanaged);
        };
        if head.is_revocation() {
            return Ok(MailboxCapabilityBindingState::Inactive);
        }
        if head.binding_id() == binding_id {
            return Ok(MailboxCapabilityBindingState::Current);
        }
        if self.acknowledgements.contains_key(&head.update_id()?) {
            return Ok(MailboxCapabilityBindingState::Inactive);
        }
        let Some(previous_id) = head.previous_update_id() else {
            return Ok(MailboxCapabilityBindingState::Inactive);
        };
        Ok(self
            .ordered_updates
            .iter()
            .find(|update| update.update_id().is_ok_and(|id| id == previous_id))
            .filter(|previous| !previous.is_revocation() && previous.binding_id() == binding_id)
            .map_or(MailboxCapabilityBindingState::Inactive, |_| {
                MailboxCapabilityBindingState::RotationOverlap
            }))
    }

    /// Binding eligibility on the recipient's write side. A recipient switches
    /// to an appended activation immediately and never writes through overlap.
    pub fn recipient_write_binding_state(
        &self,
        binding_id: MailboxBindingId,
    ) -> MailboxCapabilityBindingState {
        match self.head() {
            None => MailboxCapabilityBindingState::Unmanaged,
            Some(head) if !head.is_revocation() && head.binding_id() == binding_id => {
                MailboxCapabilityBindingState::Current
            }
            Some(_) => MailboxCapabilityBindingState::Inactive,
        }
    }

    pub fn classify_inbound_update(
        &self,
        update: &SignedMailboxCapabilityUpdate,
    ) -> Result<MailboxCapabilityInboundDisposition> {
        update.verify_signature()?;
        let update_id = update.update_id()?;
        if let Some(existing) = self
            .ordered_updates
            .iter()
            .find(|existing| existing.update_id().is_ok_and(|id| id == update_id))
        {
            ensure!(
                *existing == update,
                "mailbox capability update ID already exists with different content"
            );
            return Ok(MailboxCapabilityInboundDisposition::AlreadyPresent);
        }
        update.verify_chain_link(self.head())?;
        Ok(MailboxCapabilityInboundDisposition::Append)
    }
}

fn binding_id(content: &LocalBindingContent) -> Result<MailboxBindingId> {
    content.validate()?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(BINDING_ID_DOMAIN);
    hasher.update(content.owner_account_id.as_bytes());
    hasher.update(content.owner_device_id.as_bytes());
    hasher.update(content.peer_account_id.as_bytes());
    hasher.update(content.peer_device_id.as_bytes());
    hasher.update(content.scope.as_bytes());
    hasher.update(content.address.mailbox_id().as_bytes());
    Ok(MailboxBindingId(*hasher.finalize().as_bytes()))
}

fn offer_binding_id(content: &MailboxOfferContent) -> Result<MailboxBindingId> {
    content.validate()?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(BINDING_ID_DOMAIN);
    hasher.update(content.owner_account_id.as_bytes());
    hasher.update(content.owner_device_id.as_bytes());
    hasher.update(content.recipient_account_id.as_bytes());
    hasher.update(content.recipient_device_id.as_bytes());
    hasher.update(content.scope.as_bytes());
    hasher.update(content.address.mailbox_id().as_bytes());
    Ok(MailboxBindingId(*hasher.finalize().as_bytes()))
}

fn validate_owner(
    identity: &DeviceIdentity,
    encryption: &DeviceEncryptionIdentity,
    certificate: &DeviceCertificate,
) -> Result<()> {
    certificate.verify()?;
    ensure!(
        identity.device_id() == certificate.device_id()
            && encryption.public_key() == certificate.encryption_public_key(),
        "mailbox owner identity does not match its Device certificate"
    );
    Ok(())
}

fn signing_bytes<T: Serialize>(domain: &[u8], content: &T) -> Result<Vec<u8>> {
    let encoded = postcard::to_allocvec(content).context("encode mailbox signed content")?;
    let mut bytes = Vec::with_capacity(domain.len() + encoded.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn validate_encoded_size(bytes: &[u8], name: &str) -> Result<()> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_PROVISIONING_BYTES,
        "{name} size is invalid"
    );
    Ok(())
}

fn normalize_service_url(value: &str) -> Result<String> {
    ensure!(
        !value.is_empty() && value.len() <= MAX_SERVICE_URL_BYTES,
        "mailbox service URL size is invalid"
    );
    let mut url = Url::parse(value).context("parse mailbox service URL")?;
    ensure!(
        url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "mailbox service URL must not contain credentials, query, or fragment"
    );
    match url.scheme() {
        "https" => {}
        "http" => {
            let host = url.host_str().context("mailbox HTTP URL has no host")?;
            let ip: IpAddr = host
                .trim_start_matches('[')
                .trim_end_matches(']')
                .parse()
                .context("plain HTTP mailbox is allowed only for a loopback IP")?;
            ensure!(
                ip.is_loopback(),
                "plain HTTP mailbox is allowed only for a loopback IP"
            );
        }
        _ => bail!("mailbox service URL must use HTTPS"),
    }
    ensure!(
        url.path_segments().is_some(),
        "mailbox service URL cannot be a base"
    );
    let normalized_path = format!("{}/", url.path().trim_end_matches('/'));
    url.set_path(&normalized_path);
    let normalized = url.to_string();
    ensure!(
        normalized.len() <= MAX_SERVICE_URL_BYTES,
        "canonical mailbox service URL is too large"
    );
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kilogram_identity::{AccountRootState, DeviceCapability, DeviceState};
    use kilogram_mailbox::{
        MailboxEnvelope, MailboxItemId, MailboxListRequest, MailboxPutRequest, MailboxRequestNonce,
        MailboxStoreIdentity,
    };
    use tempfile::tempdir;

    struct TestDevice {
        state: DeviceState,
        certificate: DeviceCertificate,
    }

    fn test_device() -> Result<TestDevice> {
        let root_directory = tempdir()?;
        let device_directory = tempdir()?;
        let root = AccountRootState::create(root_directory.path())?;
        let state = DeviceState::load_or_create(device_directory.path())?;
        let certificate = root.issue_device_certificate(
            state.identity().device_id(),
            state.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        Ok(TestDevice { state, certificate })
    }

    fn service() -> Result<MailboxServiceDescriptor> {
        MailboxServiceDescriptor::new(
            "https://mailbox.example.invalid/kilogram",
            MailboxStoreIdentity::generate()?.store_key(),
        )
    }

    fn capability_activation(
        owner: &TestDevice,
        recipient: &TestDevice,
        scope: MailboxScope,
        generation: u64,
        previous_update_id: Option<MailboxCapabilityUpdateId>,
        created_at_unix_seconds: u64,
    ) -> Result<SignedMailboxCapabilityUpdate> {
        let sealed = SealedLocalMailboxBinding::create(
            owner.state.identity(),
            owner.state.encryption(),
            &owner.certificate,
            &recipient.certificate,
            scope,
            service()?,
            created_at_unix_seconds,
        )?;
        let local = sealed.open(
            owner.state.identity(),
            owner.state.encryption(),
            &owner.certificate,
        )?;
        let offer = local.offer_for(
            owner.state.identity(),
            &owner.certificate,
            &recipient.certificate,
            created_at_unix_seconds + 1_000,
        )?;
        SignedMailboxCapabilityUpdate::activate(
            owner.state.identity(),
            &owner.certificate,
            &recipient.certificate,
            scope,
            generation,
            previous_update_id,
            created_at_unix_seconds,
            &offer,
        )
    }

    #[test]
    fn local_binding_and_recipient_offer_round_trip() -> Result<()> {
        let alice = test_device()?;
        let bob = test_device()?;
        let scope = MailboxScope::from_bytes([7_u8; 32]);
        let sealed = SealedLocalMailboxBinding::create(
            alice.state.identity(),
            alice.state.encryption(),
            &alice.certificate,
            &bob.certificate,
            scope,
            service()?,
            1_000,
        )?;
        let reopened = SealedLocalMailboxBinding::decode(&sealed.encode()?)?.open(
            alice.state.identity(),
            alice.state.encryption(),
            &alice.certificate,
        )?;
        assert_eq!(reopened.peer_device_id(), bob.certificate.device_id());
        assert_eq!(
            reopened.address().read_key(),
            reopened.read_capability().read_key()
        );
        let offer = reopened.offer_for(
            alice.state.identity(),
            &alice.certificate,
            &bob.certificate,
            2_000,
        )?;
        let imported = EncryptedMailboxOffer::decode(&offer.encode()?)?.open(
            bob.state.encryption(),
            &bob.certificate,
            &alice.certificate,
            scope,
            1_500,
        )?;
        assert_eq!(imported.binding_id()?, reopened.binding_id()?);
        assert_eq!(imported.owner_device_id(), alice.certificate.device_id());
        assert_eq!(imported.recipient_device_id(), bob.certificate.device_id());
        assert_eq!(
            imported.address().write_key(),
            imported.write_capability().write_key()
        );
        assert_eq!(
            imported.recipient_encryption_public_key(),
            alice.certificate.encryption_public_key()
        );
        let item_id = MailboxItemId::generate()?;
        let envelope = MailboxEnvelope::seal(
            imported.address().mailbox_id(),
            item_id,
            1_500,
            1_600,
            imported.recipient_encryption_public_key(),
            b"recipient-bound payload",
        )?
        .encode()?;
        let authorization =
            imported
                .write_capability()
                .authorize(imported.address(), item_id, 100, &envelope)?;
        let put = MailboxPutRequest::new(imported.address(), authorization, envelope)?;
        MailboxPutRequest::decode_and_verify(&put.encode()?)?;
        let list_authorization = reopened
            .read_capability()
            .authorize_list(reopened.address(), MailboxRequestNonce::generate()?)?;
        let list = MailboxListRequest::new(reopened.address(), list_authorization)?;
        MailboxListRequest::decode_and_verify(&list.encode()?)?;
        Ok(())
    }

    #[test]
    fn offer_is_recipient_source_scope_and_expiry_bound() -> Result<()> {
        let alice = test_device()?;
        let bob = test_device()?;
        let mallory = test_device()?;
        let scope = MailboxScope::from_bytes([9_u8; 32]);
        let sealed = SealedLocalMailboxBinding::create(
            alice.state.identity(),
            alice.state.encryption(),
            &alice.certificate,
            &bob.certificate,
            scope,
            service()?,
            5_000,
        )?;
        let local = sealed.open(
            alice.state.identity(),
            alice.state.encryption(),
            &alice.certificate,
        )?;
        let offer = local.offer_for(
            alice.state.identity(),
            &alice.certificate,
            &bob.certificate,
            6_000,
        )?;
        assert!(
            offer
                .open(
                    mallory.state.encryption(),
                    &mallory.certificate,
                    &alice.certificate,
                    scope,
                    5_500,
                )
                .is_err()
        );
        assert!(
            offer
                .open(
                    bob.state.encryption(),
                    &bob.certificate,
                    &mallory.certificate,
                    scope,
                    5_500,
                )
                .is_err()
        );
        assert!(
            offer
                .open(
                    bob.state.encryption(),
                    &bob.certificate,
                    &alice.certificate,
                    MailboxScope::from_bytes([10_u8; 32]),
                    5_500,
                )
                .is_err()
        );
        assert!(
            offer
                .open(
                    bob.state.encryption(),
                    &bob.certificate,
                    &alice.certificate,
                    scope,
                    6_000,
                )
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn tampering_is_rejected() -> Result<()> {
        let alice = test_device()?;
        let bob = test_device()?;
        let scope = MailboxScope::from_bytes([11_u8; 32]);
        let sealed = SealedLocalMailboxBinding::create(
            alice.state.identity(),
            alice.state.encryption(),
            &alice.certificate,
            &bob.certificate,
            scope,
            service()?,
            7_000,
        )?;
        let local = sealed.open(
            alice.state.identity(),
            alice.state.encryption(),
            &alice.certificate,
        )?;
        let mut bytes = local
            .offer_for(
                alice.state.identity(),
                &alice.certificate,
                &bob.certificate,
                8_000,
            )?
            .encode()?;
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        let tampered = EncryptedMailboxOffer::decode(&bytes)?;
        assert!(
            tampered
                .open(
                    bob.state.encryption(),
                    &bob.certificate,
                    &alice.certificate,
                    scope,
                    7_500,
                )
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn capability_update_chain_rejects_gap_fork_and_wrong_revocation() -> Result<()> {
        let alice = test_device()?;
        let bob = test_device()?;
        let scope = MailboxScope::from_bytes([12_u8; 32]);
        let first_sealed = SealedLocalMailboxBinding::create(
            alice.state.identity(),
            alice.state.encryption(),
            &alice.certificate,
            &bob.certificate,
            scope,
            service()?,
            10_000,
        )?;
        let first_local = first_sealed.open(
            alice.state.identity(),
            alice.state.encryption(),
            &alice.certificate,
        )?;
        let first_offer = first_local.offer_for(
            alice.state.identity(),
            &alice.certificate,
            &bob.certificate,
            11_000,
        )?;
        let first = SignedMailboxCapabilityUpdate::activate(
            alice.state.identity(),
            &alice.certificate,
            &bob.certificate,
            scope,
            1,
            None,
            10_001,
            &first_offer,
        )?;
        first.verify_chain_link(None)?;

        let revoked = SignedMailboxCapabilityUpdate::revoke(
            alice.state.identity(),
            &alice.certificate,
            &bob.certificate,
            scope,
            2,
            first.update_id()?,
            first.binding_id(),
            10_002,
        )?;
        revoked.verify_chain_link(Some(&first))?;

        let second_sealed = SealedLocalMailboxBinding::create(
            alice.state.identity(),
            alice.state.encryption(),
            &alice.certificate,
            &bob.certificate,
            scope,
            service()?,
            10_003,
        )?;
        let second_local = second_sealed.open(
            alice.state.identity(),
            alice.state.encryption(),
            &alice.certificate,
        )?;
        let second_offer = second_local.offer_for(
            alice.state.identity(),
            &alice.certificate,
            &bob.certificate,
            11_000,
        )?;
        let rotated = SignedMailboxCapabilityUpdate::activate(
            alice.state.identity(),
            &alice.certificate,
            &bob.certificate,
            scope,
            3,
            Some(revoked.update_id()?),
            10_003,
            &second_offer,
        )?;
        rotated.verify_chain_link(Some(&revoked))?;
        assert_ne!(rotated.binding_id(), first.binding_id());
        assert_eq!(
            SignedMailboxCapabilityUpdate::decode(&rotated.encode()?)?.update_id()?,
            rotated.update_id()?
        );

        let gap = SignedMailboxCapabilityUpdate::activate(
            alice.state.identity(),
            &alice.certificate,
            &bob.certificate,
            scope,
            4,
            Some(first.update_id()?),
            10_004,
            &second_offer,
        )?;
        assert!(gap.verify_chain_link(Some(&revoked)).is_err());
        let wrong_revocation = SignedMailboxCapabilityUpdate::revoke(
            alice.state.identity(),
            &alice.certificate,
            &bob.certificate,
            scope,
            4,
            rotated.update_id()?,
            first.binding_id(),
            10_004,
        )?;
        assert!(wrong_revocation.verify_chain_link(Some(&rotated)).is_err());
        Ok(())
    }

    #[test]
    fn convergence_survives_lost_ack_restart_rotation_and_revocation() -> Result<()> {
        let alice = test_device()?;
        let bob = test_device()?;
        let scope = MailboxScope::from_bytes([13_u8; 32]);
        let first = capability_activation(&alice, &bob, scope, 1, None, 20_000)?;
        let first_binding_id = first.binding_id();

        // Activation is immediately usable by its owner, but remains the one
        // deterministic retry candidate until an exact recipient ACK is kept.
        let mut owner_updates = vec![SignedMailboxCapabilityUpdate::decode(&first.encode()?)?];
        let mut owner_acknowledgements = Vec::new();
        {
            let owner = MailboxCapabilityConvergence::new(&owner_updates, &owner_acknowledgements)?;
            assert_eq!(
                owner
                    .next_outbound_update()?
                    .context("initial activation is not pending")?
                    .update_id()?,
                first.update_id()?
            );
            assert_eq!(
                owner.owner_receive_binding_state(first_binding_id)?,
                MailboxCapabilityBindingState::Current
            );
            assert!(!owner.is_fully_acknowledged()?);
        }

        let mut recipient_updates = Vec::new();
        {
            let recipient =
                MailboxCapabilityConvergence::new(&recipient_updates, std::iter::empty())?;
            assert_eq!(
                recipient.classify_inbound_update(&first)?,
                MailboxCapabilityInboundDisposition::Append
            );
        }
        recipient_updates.push(SignedMailboxCapabilityUpdate::decode(&first.encode()?)?);
        let lost_ack = SignedMailboxCapabilityAcknowledgement::sign(
            bob.state.identity(),
            MailboxCapabilitySessionBinding::from_bytes([1_u8; 32]),
            &first,
        )?;
        assert!(
            lost_ack
                .verify_for(
                    MailboxCapabilitySessionBinding::from_bytes([2_u8; 32]),
                    &first,
                )
                .is_err()
        );

        // After an owner restart with no retained ACK, the same update is
        // selected again and the recipient treats it as an idempotent replay.
        owner_updates = owner_updates
            .iter()
            .map(|update| SignedMailboxCapabilityUpdate::decode(&update.encode()?))
            .collect::<Result<Vec<_>>>()?;
        {
            let restarted =
                MailboxCapabilityConvergence::new(&owner_updates, &owner_acknowledgements)?;
            assert_eq!(
                restarted
                    .next_outbound_update()?
                    .context("lost activation ACK did not schedule a retry")?
                    .update_id()?,
                first.update_id()?
            );
        }
        {
            let recipient =
                MailboxCapabilityConvergence::new(&recipient_updates, std::iter::empty())?;
            assert_eq!(
                recipient.classify_inbound_update(&first)?,
                MailboxCapabilityInboundDisposition::AlreadyPresent
            );
        }
        let first_ack = SignedMailboxCapabilityAcknowledgement::sign(
            bob.state.identity(),
            MailboxCapabilitySessionBinding::from_bytes([3_u8; 32]),
            &first,
        )?;
        owner_acknowledgements.push(SignedMailboxCapabilityAcknowledgement::decode(
            &first_ack.encode()?,
        )?);
        {
            let converged =
                MailboxCapabilityConvergence::new(&owner_updates, &owner_acknowledgements)?;
            assert!(converged.is_fully_acknowledged()?);
        }

        let rotation =
            capability_activation(&alice, &bob, scope, 2, Some(first.update_id()?), 20_001)?;
        let rotation_binding_id = rotation.binding_id();
        owner_updates.push(SignedMailboxCapabilityUpdate::decode(&rotation.encode()?)?);
        {
            let rotating =
                MailboxCapabilityConvergence::new(&owner_updates, &owner_acknowledgements)?;
            assert_eq!(
                rotating
                    .next_outbound_update()?
                    .context("rotation is not pending")?
                    .update_id()?,
                rotation.update_id()?
            );
            assert_eq!(
                rotating.owner_receive_binding_state(rotation_binding_id)?,
                MailboxCapabilityBindingState::Current
            );
            assert_eq!(
                rotating.owner_receive_binding_state(first_binding_id)?,
                MailboxCapabilityBindingState::RotationOverlap
            );
        }
        {
            let recipient =
                MailboxCapabilityConvergence::new(&recipient_updates, std::iter::empty())?;
            assert_eq!(
                recipient.classify_inbound_update(&rotation)?,
                MailboxCapabilityInboundDisposition::Append
            );
        }
        recipient_updates.push(SignedMailboxCapabilityUpdate::decode(&rotation.encode()?)?);

        // A crash after the recipient append but before ACK retention again
        // yields the rotation as the sole retry and preserves receive overlap.
        owner_updates = owner_updates
            .iter()
            .map(|update| SignedMailboxCapabilityUpdate::decode(&update.encode()?))
            .collect::<Result<Vec<_>>>()?;
        owner_acknowledgements = owner_acknowledgements
            .iter()
            .map(|acknowledgement| {
                SignedMailboxCapabilityAcknowledgement::decode(&acknowledgement.encode()?)
            })
            .collect::<Result<Vec<_>>>()?;
        {
            let restarted =
                MailboxCapabilityConvergence::new(&owner_updates, &owner_acknowledgements)?;
            assert_eq!(
                restarted
                    .next_outbound_update()?
                    .context("lost rotation ACK did not schedule a retry")?
                    .update_id()?,
                rotation.update_id()?
            );
            assert_eq!(
                restarted.owner_receive_binding_state(first_binding_id)?,
                MailboxCapabilityBindingState::RotationOverlap
            );
        }
        let rotation_ack = SignedMailboxCapabilityAcknowledgement::sign(
            bob.state.identity(),
            MailboxCapabilitySessionBinding::from_bytes([4_u8; 32]),
            &rotation,
        )?;
        owner_acknowledgements.push(rotation_ack);
        {
            let rotated =
                MailboxCapabilityConvergence::new(&owner_updates, &owner_acknowledgements)?;
            assert_eq!(
                rotated.owner_receive_binding_state(first_binding_id)?,
                MailboxCapabilityBindingState::Inactive
            );
            assert!(rotated.is_fully_acknowledged()?);
        }

        let revocation = SignedMailboxCapabilityUpdate::revoke(
            alice.state.identity(),
            &alice.certificate,
            &bob.certificate,
            scope,
            3,
            rotation.update_id()?,
            rotation_binding_id,
            20_002,
        )?;
        owner_updates.push(SignedMailboxCapabilityUpdate::decode(
            &revocation.encode()?,
        )?);
        {
            let revoking =
                MailboxCapabilityConvergence::new(&owner_updates, &owner_acknowledgements)?;
            assert_eq!(
                revoking
                    .next_outbound_update()?
                    .context("revocation is not pending")?
                    .update_id()?,
                revocation.update_id()?
            );
            assert_eq!(
                revoking.owner_receive_binding_state(rotation_binding_id)?,
                MailboxCapabilityBindingState::Inactive
            );
        }
        {
            let recipient =
                MailboxCapabilityConvergence::new(&recipient_updates, std::iter::empty())?;
            assert_eq!(
                recipient.classify_inbound_update(&revocation)?,
                MailboxCapabilityInboundDisposition::Append
            );
        }
        recipient_updates.push(revocation.clone());
        let recipient = MailboxCapabilityConvergence::new(&recipient_updates, std::iter::empty())?;
        assert_eq!(
            recipient.recipient_write_binding_state(rotation_binding_id),
            MailboxCapabilityBindingState::Inactive
        );

        let revocation_ack = SignedMailboxCapabilityAcknowledgement::sign(
            bob.state.identity(),
            MailboxCapabilitySessionBinding::from_bytes([5_u8; 32]),
            &revocation,
        )?;
        owner_acknowledgements.push(revocation_ack);
        let converged = MailboxCapabilityConvergence::new(&owner_updates, &owner_acknowledgements)?;
        assert!(converged.is_fully_acknowledged()?);

        let no_acknowledgements: Vec<SignedMailboxCapabilityAcknowledgement> = Vec::new();
        assert!(
            MailboxCapabilityConvergence::new(&owner_updates, &no_acknowledgements)?
                .validate_owner_progression()
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn service_url_is_fail_closed() -> Result<()> {
        let key = MailboxStoreIdentity::generate()?.store_key();
        assert!(MailboxServiceDescriptor::new("https://example.invalid/base", key).is_ok());
        assert!(MailboxServiceDescriptor::new("http://127.0.0.1:8787/base", key).is_ok());
        assert!(MailboxServiceDescriptor::new("http://[::1]:8787/base", key).is_ok());
        assert!(MailboxServiceDescriptor::new("http://localhost:8787/base", key).is_err());
        assert!(MailboxServiceDescriptor::new("http://192.0.2.1/base", key).is_err());
        assert!(MailboxServiceDescriptor::new("file:///tmp/store", key).is_err());
        assert!(MailboxServiceDescriptor::new("https://user@example.invalid/base", key).is_err());
        assert!(MailboxServiceDescriptor::new("https://example.invalid/base?q=1", key).is_err());
        Ok(())
    }
}
